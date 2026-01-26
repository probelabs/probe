import 'dotenv/config';
import { ProbeAgent } from '@probelabs/probe/agent';
import { TokenUsageDisplay } from './tokenUsageDisplay.js';
import { writeFileSync, existsSync } from 'fs';
import { readFile, stat } from 'fs/promises';
import { join, resolve, isAbsolute } from 'path';
import { TelemetryConfig } from './telemetry.js';
import { trace } from '@opentelemetry/api';
import { appTracer } from './appTracer.js';

// Image configuration (duplicated from @probelabs/probe/agent/imageConfig for compatibility)
// TODO: Import from '@probelabs/probe/agent/imageConfig' after next package publish
const IMAGE_MIME_TYPES = {
  'png': 'image/png',
  'jpg': 'image/jpeg',
  'jpeg': 'image/jpeg',
  'webp': 'image/webp',
  'bmp': 'image/bmp',
  'svg': 'image/svg+xml'
};
const SUPPORTED_IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'svg'];
const getExtensionPattern = (extensions = SUPPORTED_IMAGE_EXTENSIONS) => extensions.join('|');

// Parse and validate allowed folders from environment variable
const allowedFolders = process.env.ALLOWED_FOLDERS
  ? process.env.ALLOWED_FOLDERS.split(',').map(folder => folder.trim()).filter(Boolean)
  : [];

// Validate folders exist on startup - will be handled by index.js in non-interactive mode
// This is kept for backward compatibility with direct ProbeChat usage
const validateFolders = () => {
  if (allowedFolders.length > 0) {
    for (const folder of allowedFolders) {
      const exists = existsSync(folder);
      // Only log if not in non-interactive mode or if in debug mode
      if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
        console.log(`- ${folder} ${exists ? '✓' : '✗ (not found)'}`);
        if (!exists) {
          console.warn(`Warning: Folder "${folder}" does not exist or is not accessible`);
        }
      }
    }
  } else {
    // Only log if not in non-interactive mode or if in debug mode
    if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
      console.warn('No folders configured via ALLOWED_FOLDERS. Tools might default to current directory or require explicit paths.');
    }
  }
};

// Only validate folders on startup if not in non-interactive mode
if (typeof process !== 'undefined' && !process.env.PROBE_CHAT_SKIP_FOLDER_VALIDATION) {
  validateFolders();
}

// Maximum image file size (20MB) to prevent OOM attacks
const MAX_IMAGE_FILE_SIZE = 20 * 1024 * 1024;

/**
 * Security validation for local file paths
 * @param {string} filePath - The file path to validate
 * @param {string} baseDir - The base directory to restrict access to
 * @returns {boolean} - Whether the path is safe to access
 */
function isSecureFilePath(filePath, baseDir = process.cwd()) {
  try {
    // Resolve the absolute path
    const absolutePath = isAbsolute(filePath) ? filePath : resolve(baseDir, filePath);
    const normalizedBase = resolve(baseDir);
    
    // Ensure the resolved path is within the allowed directory
    return absolutePath.startsWith(normalizedBase);
  } catch (error) {
    return false;
  }
}

/**
 * Convert local image file to base64 data URL
 * @param {string} filePath - Path to the image file
 * @param {boolean} debug - Whether to log debug information
 * @returns {Promise<string|null>} - Base64 data URL or null if failed
 */
async function convertImageFileToBase64(filePath, debug = false) {
  try {
    // Security check: validate the file path against all allowed directories
    const allowedDirs = allowedFolders.length > 0 ? allowedFolders : [process.cwd()];
    const isPathAllowed = allowedDirs.some(dir => isSecureFilePath(filePath, dir));
    
    if (!isPathAllowed) {
      if (debug) {
        console.log(`[DEBUG] Security check failed for path: ${filePath}`);
      }
      return null;
    }

    // Resolve the path - for relative paths, use the first allowed directory as base
    const baseDir = allowedDirs[0];
    const absolutePath = isAbsolute(filePath) ? filePath : resolve(baseDir, filePath);
    
    // Check if file exists and get file stats
    let fileStats;
    try {
      fileStats = await stat(absolutePath);
    } catch (error) {
      if (debug) {
        console.log(`[DEBUG] File not found: ${absolutePath}`);
      }
      return null;
    }

    // Validate file size to prevent OOM attacks
    if (fileStats.size > MAX_IMAGE_FILE_SIZE) {
      if (debug) {
        console.log(`[DEBUG] Image file too large: ${absolutePath} (${fileStats.size} bytes, max: ${MAX_IMAGE_FILE_SIZE})`);
      }
      return null;
    }

    // Determine MIME type based on file extension (from shared config)
    const extension = absolutePath.toLowerCase().split('.').pop();
    const mimeType = IMAGE_MIME_TYPES[extension];
    if (!mimeType) {
      if (debug) {
        console.log(`[DEBUG] Unsupported image format: ${extension}`);
      }
      return null;
    }

    // Read file and convert to base64 asynchronously
    const fileBuffer = await readFile(absolutePath);
    const base64Data = fileBuffer.toString('base64');
    const dataUrl = `data:${mimeType};base64,${base64Data}`;
    
    if (debug) {
      console.log(`[DEBUG] Successfully converted ${absolutePath} to base64 (${fileBuffer.length} bytes)`);
    }
    
    return dataUrl;
  } catch (error) {
    if (debug) {
      console.log(`[DEBUG] Error converting file to base64: ${error.message}`);
    }
    return null;
  }
}

// Export the extractImageUrls function for testing
export { extractImageUrls };

/**
 * Extract image URLs and local file paths from message text
 * @param {string} message - The message text to analyze
 * @param {boolean} debug - Whether to log debug information
 * @returns {Promise<Object>} Promise resolving to { urls: Array, cleanedMessage: string }
 */
async function extractImageUrls(message, debug = false) {
  // This function should be called within the session context, so it will inherit the trace ID
  const tracer = trace.getTracer('probe-chat', '1.0.0');
  return tracer.startActiveSpan('content.image.extract', async (span) => {
    try {
      // Pattern to match image URLs, base64 data, and local file paths:
      // 1. GitHub private-user-images URLs (always images, regardless of extension)
      // 2. GitHub user-attachments/assets URLs (always images, regardless of extension)
      // 3. URLs with common image extensions (PNG, JPG, JPEG, WebP, BMP, SVG)
      // 4. Base64 data URLs (data:image/...)
      // 5. Local file paths with image extensions (relative and absolute)
      // Updated to stop at quotes, spaces, or common HTML/XML delimiters
      // Pattern dynamically generated from shared config
      const extPattern = getExtensionPattern();
      const imageUrlPattern = new RegExp(`(?:data:image/[a-zA-Z]*;base64,[A-Za-z0-9+/=]+|https?://(?:(?:private-user-images\\.githubusercontent\\.com|github\\.com/user-attachments/assets)/[^\\s"'<>]+|[^\\s"'<>]+\\.(?:${extPattern})(?:\\?[^\\s"'<>]*)?)|(?:\\.?\\.?/)?[^\\s"'<>]*\\.(?:${extPattern}))`, 'gi');

      span.setAttributes({
        'message.length': message.length,
        'debug.enabled': debug
      });

      if (debug) {
        console.log(`[DEBUG] Scanning message for image URLs. Message length: ${message.length}`);
        console.log(`[DEBUG] Image URL pattern: ${imageUrlPattern.toString()}`);
      }

      const urls = [];
      const foundPatterns = [];
      let match;

      while ((match = imageUrlPattern.exec(message)) !== null) {
        foundPatterns.push(match[0]);
        if (debug) {
          console.log(`[DEBUG] Found image pattern: ${match[0]}`);
        }
      }

      // Process each found pattern - convert local files to base64, keep URLs as-is
      for (const pattern of foundPatterns) {
        // Check if it's already a URL or base64 data
        if (pattern.startsWith('http') || pattern.startsWith('data:image/')) {
          urls.push(pattern);
          if (debug) {
            console.log(`[DEBUG] Using URL/base64 as-is: ${pattern.substring(0, 50)}...`);
          }
        } else {
          // It's a local file path - convert to base64
          const base64Data = await convertImageFileToBase64(pattern, debug);
          if (base64Data) {
            urls.push(base64Data);
            if (debug) {
              console.log(`[DEBUG] Converted local file ${pattern} to base64`);
            }
          } else {
            if (debug) {
              console.log(`[DEBUG] Failed to convert local file: ${pattern}`);
            }
          }
        }
      }

      // Clean the message by removing found patterns
      let cleanedMessage = message;
      foundPatterns.forEach(pattern => {
        cleanedMessage = cleanedMessage.replace(pattern, '').trim();
      });

      // Clean up any remaining extra whitespace
      cleanedMessage = cleanedMessage.replace(/\s+/g, ' ').trim();

      span.setAttributes({
        'patterns.found': foundPatterns.length,
        'images.processed': urls.length,
        'message.cleaned_length': cleanedMessage.length
      });

      if (debug) {
        console.log(`[DEBUG] Found ${foundPatterns.length} patterns, processed ${urls.length} images`);
        console.log(`[DEBUG] Cleaned message length: ${cleanedMessage.length}`);
      }

      return { urls, cleanedMessage };
    } finally {
      span.end();
    }
  });
}

/**
 * ProbeChat class using ProbeAgent with MCP support
 */
export class ProbeChat {
  /**
   * Create a new ProbeChat instance
   * @param {Object} options - Configuration options
   * @param {string} [options.sessionId] - Optional session ID
   * @param {boolean} [options.isNonInteractive=false] - Suppress internal logs if true
   * @param {string} [options.customPrompt] - Custom prompt to replace the default system message
   * @param {string} [options.promptType] - Predefined prompt type (architect, code-review, support)
   * @param {boolean} [options.allowEdit=false] - Allow the use of the 'implement' tool
   * @param {string} [options.architectureFileName] - Architecture context filename to embed from repo root (default: ARCHITECTURE.md)
   * @param {string} [options.provider] - Force specific AI provider
   * @param {string} [options.model] - Override model name
   * @param {boolean} [options.debug] - Enable debug mode
   * @param {boolean} [options.enableMcp=false] - Enable MCP tool integration
   * @param {Array} [options.mcpServers] - MCP server configurations
   * @param {boolean} [options.enableBash=false] - Enable bash command execution
   * @param {Object} [options.bashConfig] - Bash configuration options
   * @param {string} [options.completionPrompt] - Custom prompt to run after attempt_completion for validation/review (runs before mermaid/JSON validation)
   */
  constructor(options = {}) {
    this.isNonInteractive = options.isNonInteractive || process.env.PROBE_NON_INTERACTIVE === '1';
    this.debug = options.debug || process.env.DEBUG_CHAT === '1';
    this._initialized = false;
    this._initializing = null; // Promise to prevent concurrent initialization
    this._initError = null; // Store initialization error to prevent infinite retries

    // Initialize ProbeAgent with MCP support
    const agentOptions = {
      ...options,
      path: allowedFolders.length > 0 ? allowedFolders[0] : process.cwd(),
      enableMcp: options.enableMcp || process.env.ENABLE_MCP === '1',
      mcpServers: options.mcpServers
    };

    this.agent = new ProbeAgent(agentOptions);

    // Initialize telemetry and token display
    this.telemetryConfig = new TelemetryConfig();
    this.tokenUsage = new TokenUsageDisplay();

    if (this.debug) {
      console.log(`[DEBUG] ProbeChat initialized with MCP ${agentOptions.enableMcp ? 'enabled' : 'disabled'}`);
    }
  }

  /**
   * Initialize the chat agent asynchronously.
   *
   * This method is safe to call multiple times - subsequent calls will return
   * immediately if already initialized, or wait for the ongoing initialization.
   *
   * If initialization fails, the error is stored and rethrown on subsequent calls
   * to prevent infinite retry loops.
   *
   * This initializes the ProbeAgent which handles:
   * - API key validation
   * - Auto-detection of claude-code or codex CLI as fallback
   * - MCP initialization
   * - History loading from storage
   *
   * @returns {Promise<void>}
   * @throws {Error} If no valid AI provider is available
   */
  async initialize() {
    // If previous initialization failed, rethrow the stored error
    if (this._initError) {
      throw this._initError;
    }

    // Return existing initialization promise if in progress (prevents concurrent init)
    if (this._initializing) {
      return this._initializing;
    }

    // Already initialized successfully
    if (this._initialized) {
      return;
    }

    this._initializing = (async () => {
      try {
        await this.agent.initialize();

        // Mark as initialized immediately after agent init succeeds
        this._initialized = true;

        // Debug logging wrapped in try-catch to not affect initialization state
        if (this.debug) {
          try {
            const provider = this.agent.clientApiProvider || this.agent.apiType || 'unknown';
            console.log(`[DEBUG] ProbeChat agent initialized with provider: ${provider}`);
            this.logAvailableTools();
          } catch (logError) {
            console.error('[DEBUG] Logging failed:', logError.message);
          }
        }
      } catch (error) {
        // Store error to prevent infinite retry loops
        this._initError = error;
        throw error;
      } finally {
        // Always clear the initializing promise
        this._initializing = null;
      }
    })();

    return this._initializing;
  }

  /**
   * Ensure the agent is initialized before use.
   * Called automatically by chat() - you don't need to call this directly.
   * @private
   */
  async _ensureInitialized() {
    if (!this._initialized) {
      await this.initialize();
    }
  }

  /**
   * Log all available tools (native + MCP) in debug mode
   */
  logAvailableTools() {
    if (!this.debug) return;

    console.log('\n[DEBUG] ========================================');
    console.log('[DEBUG] All Available Tools:');
    console.log('[DEBUG] ========================================');

    // Get native tools from agent
    if (this.agent.toolImplementations) {
      console.log('[DEBUG] Native Tools:');
      const nativeTools = Object.keys(this.agent.toolImplementations);
      nativeTools.forEach(toolName => {
        const tool = this.agent.toolImplementations[toolName];
        const desc = tool.description || 'No description';
        console.log(`[DEBUG]   - ${toolName}: ${desc}`);
      });
    }

    // Get MCP tools if available
    if (this.agent.mcpBridge && this.agent.mcpBridge.mcpTools) {
      const mcpTools = Object.keys(this.agent.mcpBridge.mcpTools);
      if (mcpTools.length > 0) {
        console.log('[DEBUG] MCP Tools:');
        mcpTools.forEach(toolName => {
          const tool = this.agent.mcpBridge.mcpTools[toolName];
          const desc = tool.description || 'No description';
          console.log(`[DEBUG]   - ${toolName}: ${desc}`);
        });
      } else {
        console.log('[DEBUG] MCP Tools: None loaded');
      }
    } else {
      console.log('[DEBUG] MCP Tools: MCP not enabled or not initialized');
    }

    console.log('[DEBUG] ========================================\n');
  }

  /**
   * Send a chat message and get AI response.
   *
   * This method automatically initializes the agent on first call if not already
   * initialized. For explicit control over initialization (e.g., to handle errors
   * separately), call initialize() first.
   *
   * @param {string} message - The user's question
   * @param {Object} [options] - Optional configuration
   * @param {string} [options.schema] - JSON schema for structured output
   * @param {Array} [options.images] - Array of image data (base64 strings or URLs)
   * @returns {Promise<Object>} - The AI response with token usage
   * @throws {Error} If message is empty or if no valid AI provider is available
   */
  async chat(message, options = {}) {
    if (!message || typeof message !== 'string' || message.trim().length === 0) {
      throw new Error('Message is required and must be a non-empty string');
    }

    // Ensure the agent is initialized (handles API key validation, claude-code/codex fallback)
    await this._ensureInitialized();

    // Extract images from the message text if not provided in options
    let images = options.images || [];
    let cleanedMessage = message;

    if (!images.length) {
      const extracted = await extractImageUrls(message, this.debug);
      images = extracted.urls;
      cleanedMessage = extracted.cleanedMessage;

      if (this.debug && images.length > 0) {
        console.log(`[DEBUG] Extracted ${images.length} images from message`);
      }
    }

    // Use ProbeAgent to answer the question
    const result = await this.agent.answer(cleanedMessage, images, options);

    // Update token usage display
    this.tokenUsage.updateFromTokenCounter(this.agent.tokenCounter);

    if (!this.isNonInteractive) {
      this.tokenUsage.display();
    }

    return result;
  }

  /**
   * Get session ID
   */
  getSessionId() {
    return this.agent.sessionId;
  }

  /**
   * Get provider information
   * @returns {Object} - Provider info with type and model
   */
  getProviderInfo() {
    return {
      provider: this.agent.clientApiProvider || this.agent.apiType || 'unknown',
      model: this.agent.model || 'unknown',
      isCliProvider: ['claude-code', 'codex'].includes(this.agent.clientApiProvider || this.agent.apiType)
    };
  }

  /**
   * Check if the agent is initialized
   * @returns {boolean}
   */
  isInitialized() {
    return this._initialized;
  }

  /**
   * Get usage summary for the current session
   */
  getUsageSummary() {
    return this.agent.tokenCounter.getUsageSummary();
  }

  /**
   * Clear conversation history and return the new session ID
   * @returns {string} - The new session ID
   */
  clearHistory() {
    this.agent.clearHistory();
    this.tokenUsage.clear();
    return this.agent.sessionId;
  }

  /**
   * Export conversation history
   */
  exportHistory() {
    return this.agent.history.map(msg => ({ ...msg }));
  }

  /**
   * Save conversation history to file
   */
  saveHistory(filename) {
    if (!filename) {
      filename = `probe-chat-history-${this.agent.sessionId}-${new Date().toISOString().slice(0, 19).replace(/:/g, '-')}.json`;
    }

    const historyData = {
      sessionId: this.agent.sessionId,
      timestamp: new Date().toISOString(),
      messages: this.exportHistory(),
      usage: this.getUsageSummary()
    };

    writeFileSync(filename, JSON.stringify(historyData, null, 2));

    if (!this.isNonInteractive) {
      console.log(`Conversation history saved to: ${filename}`);
    }

    return filename;
  }

  /**
   * Cancel current request
   */
  cancel() {
    this.agent.cancel();
  }

  /**
   * Clean up resources (including MCP connections)
   */
  async cleanup() {
    try {
      await this.agent.cleanup();
    } catch (error) {
      // Log the error but don't throw to ensure graceful cleanup
      if (!this.isNonInteractive) {
        console.warn('Warning during cleanup:', error.message);
      }
    }
  }
}

// Create the default instance using environment variables
export const chat = new ProbeChat({
  enableMcp: process.env.ENABLE_MCP === '1',
  debug: process.env.DEBUG_CHAT === '1'
});

export default chat;
