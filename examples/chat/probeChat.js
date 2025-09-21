import 'dotenv/config';
import { ProbeAgent } from '@probelabs/probe/agent';
import { TokenUsageDisplay } from './tokenUsageDisplay.js';
import { writeFileSync, existsSync } from 'fs';
import { join } from 'path';
import { TelemetryConfig } from './telemetry.js';
import { trace } from '@opentelemetry/api';
import { appTracer } from './appTracer.js';

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

/**
 * Extract image URLs from message text
 * @param {string} message - The message text to analyze
 * @param {boolean} debug - Whether to log debug information
 * @returns {Array} Array of { url: string, cleanedMessage: string }
 */
function extractImageUrls(message, debug = false) {
  // This function should be called within the session context, so it will inherit the trace ID
  const tracer = trace.getTracer('probe-chat', '1.0.0');
  return tracer.startActiveSpan('content.image.extract', (span) => {
    try {
      // Pattern to match image URLs and base64 data:
      // 1. GitHub private-user-images URLs (always images, regardless of extension)
      // 2. GitHub user-attachments/assets URLs (always images, regardless of extension)
      // 3. URLs with common image extensions (PNG, JPG, JPEG, WebP, GIF)
      // 4. Base64 data URLs (data:image/...)
      // Updated to stop at quotes, spaces, or common HTML/XML delimiters
      const imageUrlPattern = /(?:data:image\/[a-zA-Z]*;base64,[A-Za-z0-9+/=]+|https?:\/\/(?:(?:private-user-images\.githubusercontent\.com|github\.com\/user-attachments\/assets)\/[^\s"'<>]+|[^\s"'<>]+\.(?:png|jpg|jpeg|webp|gif)(?:\?[^\s"'<>]*)?))/gi;

      span.setAttributes({
        'message.length': message.length,
        'debug.enabled': debug
      });

      if (debug) {
        console.log(`[DEBUG] Scanning message for image URLs. Message length: ${message.length}`);
        console.log(`[DEBUG] Image URL pattern: ${imageUrlPattern.toString()}`);
      }

      const urls = [];
      let match;

      while ((match = imageUrlPattern.exec(message)) !== null) {
        urls.push(match[0]);
        if (debug) {
          console.log(`[DEBUG] Found image URL: ${match[0]}`);
        }
      }

      // Clean the message by removing found URLs
      let cleanedMessage = message;
      urls.forEach(url => {
        cleanedMessage = cleanedMessage.replace(url, '').trim();
      });

      // Clean up any remaining extra whitespace
      cleanedMessage = cleanedMessage.replace(/\s+/g, ' ').trim();

      span.setAttributes({
        'images.found': urls.length,
        'message.cleaned_length': cleanedMessage.length
      });

      if (debug) {
        console.log(`[DEBUG] Extracted ${urls.length} image URLs`);
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
   * @param {string} [options.provider] - Force specific AI provider
   * @param {string} [options.model] - Override model name
   * @param {boolean} [options.debug] - Enable debug mode
   * @param {boolean} [options.enableMcp=false] - Enable MCP tool integration
   * @param {Array} [options.mcpServers] - MCP server configurations
   */
  constructor(options = {}) {
    this.isNonInteractive = options.isNonInteractive || process.env.PROBE_NON_INTERACTIVE === '1';
    this.debug = options.debug || process.env.DEBUG_CHAT === '1';

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
   * Answer a question using the agentic flow with optional image support
   * @param {string} message - The user's question
   * @param {Object} [options] - Optional configuration
   * @param {string} [options.schema] - JSON schema for structured output
   * @param {Array} [options.images] - Array of image data (base64 strings or URLs)
   * @returns {Promise<string>} - The final answer
   */
  async chat(message, options = {}) {
    if (!message || typeof message !== 'string' || message.trim().length === 0) {
      throw new Error('Message is required and must be a non-empty string');
    }

    // Extract images from the message text if not provided in options
    let images = options.images || [];
    let cleanedMessage = message;

    if (!images.length) {
      const extracted = extractImageUrls(message, this.debug);
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
   * Get usage summary for the current session
   */
  getUsageSummary() {
    return this.agent.tokenCounter.getUsageSummary();
  }

  /**
   * Clear conversation history
   */
  clearHistory() {
    this.agent.clearHistory();
    this.tokenUsage.clear();
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
    await this.agent.cleanup();
  }
}

// Create the default instance using environment variables
export const chat = new ProbeChat({
  enableMcp: process.env.ENABLE_MCP === '1',
  debug: process.env.DEBUG_CHAT === '1'
});

export default chat;