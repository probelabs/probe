// Core ProbeAgent class adapted from examples/chat/probeChat.js

// Load .env file if present (silent fail if not found)
import dotenv from 'dotenv';
dotenv.config();

import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { createAmazonBedrock } from '@ai-sdk/amazon-bedrock';
import { streamText } from 'ai';
import { randomUUID } from 'crypto';
import { EventEmitter } from 'events';
import { existsSync } from 'fs';
import { readFile, stat, readdir } from 'fs/promises';
import { resolve, isAbsolute, dirname, basename, normalize, sep } from 'path';
import { TokenCounter } from './tokenCounter.js';
import { InMemoryStorageAdapter } from './storage/InMemoryStorageAdapter.js';
import { HookManager, HOOK_TYPES } from './hooks/HookManager.js';
import { SUPPORTED_IMAGE_EXTENSIONS, IMAGE_MIME_TYPES, isFormatSupportedByProvider } from './imageConfig.js';
import {
  createTools,
  searchToolDefinition,
  queryToolDefinition,
  extractToolDefinition,
  delegateToolDefinition,
  bashToolDefinition,
  listFilesToolDefinition,
  searchFilesToolDefinition,
  listSkillsToolDefinition,
  useSkillToolDefinition,
  readImageToolDefinition,
  attemptCompletionToolDefinition,
  implementToolDefinition,
  editToolDefinition,
  createToolDefinition,
  attemptCompletionSchema,
  parseXmlToolCallWithThinking
} from './tools.js';
import { createMessagePreview } from '../tools/common.js';
import {
  createWrappedTools,
  listFilesToolInstance,
  searchFilesToolInstance,
  clearToolExecutionData
} from './probeTool.js';
import { createMockProvider } from './mockProvider.js';
import { listFilesByLevel } from '../index.js';
import {
  cleanSchemaResponse,
  isJsonSchema,
  validateJsonResponse,
  createJsonCorrectionPrompt,
  generateSchemaInstructions,
  isJsonSchemaDefinition,
  createSchemaDefinitionCorrectionPrompt,
  validateAndFixMermaidResponse
} from './schemaUtils.js';
import { removeThinkingTags } from './xmlParsingUtils.js';
import { predefinedPrompts } from './shared/prompts.js';
import {
  MCPXmlBridge,
  parseHybridXmlToolCall,
  loadMCPConfigurationFromPath
} from './mcp/index.js';
import { SkillRegistry } from './skills/registry.js';
import { formatAvailableSkillsXml } from './skills/formatting.js';
import { createSkillToolInstances } from './skills/tools.js';
import { RetryManager, createRetryManagerFromEnv } from './RetryManager.js';
import { FallbackManager, createFallbackManagerFromEnv, buildFallbackProvidersFromEnv } from './FallbackManager.js';
import { handleContextLimitError } from './contextCompactor.js';
import {
  TaskManager,
  createTaskTool,
  taskToolDefinition,
  taskSystemPrompt,
  taskGuidancePrompt,
  createTaskCompletionBlockedMessage
} from './tasks/index.js';

// Maximum tool iterations to prevent infinite loops - configurable via MAX_TOOL_ITERATIONS env var
const MAX_TOOL_ITERATIONS = (() => {
  const val = parseInt(process.env.MAX_TOOL_ITERATIONS || '30', 10);
  if (isNaN(val) || val < 1 || val > 200) {
    console.warn('[ProbeAgent] MAX_TOOL_ITERATIONS must be between 1 and 200, using default: 30');
    return 30;
  }
  return val;
})();
const MAX_HISTORY_MESSAGES = 100;

// Supported image file extensions (imported from shared config)

// Maximum image file size (20MB) to prevent OOM attacks
const MAX_IMAGE_FILE_SIZE = 20 * 1024 * 1024;

/**
 * ProbeAgent class to handle AI interactions with code search capabilities
 */
export class ProbeAgent {
  /**
   * Create a new ProbeAgent instance
   * @param {Object} options - Configuration options
   * @param {string} [options.sessionId] - Optional session ID
   * @param {string} [options.customPrompt] - Custom prompt to replace the default system message
   * @param {string} [options.systemPrompt] - Alias for customPrompt; takes precedence when both are provided
   * @param {string} [options.promptType] - Predefined prompt type (code-explorer, code-searcher, architect, code-review, support)
   * @param {boolean} [options.allowEdit=false] - Allow the use of the 'implement' tool
   * @param {boolean} [options.enableDelegate=false] - Enable the delegate tool for task distribution to subagents
   * @param {string} [options.architectureFileName] - Architecture context filename to embed from repo root (defaults to AGENTS.md with CLAUDE.md fallback; ARCHITECTURE.md is always included when present)
   * @param {string} [options.path] - Search directory path
   * @param {string} [options.cwd] - Working directory for resolving relative paths (independent of allowedFolders)
   * @param {string} [options.provider] - Force specific AI provider
   * @param {string} [options.model] - Override model name
   * @param {boolean} [options.debug] - Enable debug mode
   * @param {boolean} [options.outline] - Enable outline-xml format for search results
   * @param {boolean} [options.searchDelegate=true] - Use a delegated code-search subagent for the search tool
   * @param {number} [options.maxResponseTokens] - Maximum tokens for AI responses
   * @param {number} [options.maxIterations] - Maximum tool iterations (overrides MAX_TOOL_ITERATIONS env var)
   * @param {boolean} [options.disableMermaidValidation=false] - Disable automatic mermaid diagram validation and fixing
   * @param {boolean} [options.disableJsonValidation=false] - Disable automatic JSON validation and fixing (prevents infinite recursion in JsonFixingAgent)
   * @param {boolean} [options.enableSkills=true] - Enable agent skills discovery and activation
   * @param {boolean} [options.disableSkills=false] - Disable agent skills (overrides enableSkills)
   * @param {Array<string>} [options.skillDirs] - Skill directories to scan relative to repo root
   * @param {boolean} [options.enableMcp=false] - Enable MCP tool integration
   * @param {string} [options.mcpConfigPath] - Path to MCP configuration file
   * @param {Object} [options.mcpConfig] - MCP configuration object (overrides mcpConfigPath)
   * @param {Array} [options.mcpServers] - Deprecated, use mcpConfig instead
   * @param {boolean} [options.enableTasks=false] - Enable task management system for tracking progress
   * @param {Object} [options.storageAdapter] - Custom storage adapter for history management
   * @param {Object} [options.hooks] - Hook callbacks for events (e.g., {'tool:start': callback})
   * @param {Array<string>|null} [options.allowedTools] - List of allowed tool names. Use ['*'] for all tools (default), [] or null for no tools (raw AI mode), or specific tool names like ['search', 'query', 'extract']. Supports exclusion with '!' prefix (e.g., ['*', '!bash'])
   * @param {boolean} [options.disableTools=false] - Convenience flag to disable all tools (equivalent to allowedTools: []). Takes precedence over allowedTools if set.
   * @param {Object} [options.retry] - Retry configuration
   * @param {number} [options.retry.maxRetries=3] - Maximum retry attempts per provider
   * @param {number} [options.retry.initialDelay=1000] - Initial delay in ms
   * @param {number} [options.retry.maxDelay=30000] - Maximum delay in ms
   * @param {number} [options.retry.backoffFactor=2] - Exponential backoff multiplier
   * @param {Array<string>} [options.retry.retryableErrors] - List of retryable error patterns
   * @param {Object} [options.fallback] - Fallback configuration
   * @param {string} [options.fallback.strategy] - Fallback strategy: 'same-model', 'same-provider', 'any', 'custom'
   * @param {Array<string>} [options.fallback.models] - List of models for same-provider fallback
   * @param {Array<Object>} [options.fallback.providers] - List of provider configurations for custom fallback
   * @param {boolean} [options.fallback.stopOnSuccess=true] - Stop on first success
   * @param {number} [options.fallback.maxTotalAttempts=10] - Maximum total attempts across all providers
   * @param {string} [options.completionPrompt] - Custom prompt to run after attempt_completion for validation/review (runs before mermaid/JSON validation)
   */
  constructor(options = {}) {
    // Basic configuration
    this.sessionId = options.sessionId || randomUUID();
    // Support systemPrompt alias (overrides customPrompt when both are provided)
    this.customPrompt = options.systemPrompt || options.customPrompt || null;
    this.promptType = options.promptType || 'code-explorer';
    this.allowEdit = !!options.allowEdit;
    this.enableDelegate = !!options.enableDelegate;
    this.debug = options.debug || process.env.DEBUG === '1';
    this.cancelled = false;
    this.tracer = options.tracer || null;
    this.outline = !!options.outline;
    this.searchDelegate = options.searchDelegate !== undefined ? !!options.searchDelegate : true;
    this.maxResponseTokens = options.maxResponseTokens || (() => {
      const val = parseInt(process.env.MAX_RESPONSE_TOKENS || '0', 10);
      if (isNaN(val) || val < 0 || val > 200000) {
        return null;
      }
      return val || null;
    })();
    this.maxIterations = options.maxIterations || null;
    this.disableMermaidValidation = !!options.disableMermaidValidation;
    this.disableJsonValidation = !!options.disableJsonValidation;
    this.enableSkills = options.disableSkills ? false : (options.enableSkills !== undefined ? !!options.enableSkills : true);
    if (Array.isArray(options.skillDirs)) {
      this.skillDirs = options.skillDirs;
    } else if (typeof options.skillDirs === 'string') {
      this.skillDirs = options.skillDirs.split(',').map(dir => dir.trim()).filter(Boolean);
    } else {
      this.skillDirs = null;
    }
    this.skillsRegistry = null;
    this.activeSkills = new Map();

    // Completion prompt for post-completion validation/review
    this.completionPrompt = options.completionPrompt || null;

    // Tool filtering configuration
    // Parse allowedTools option: ['*'] = all tools, [] or null = no tools, ['tool1', 'tool2'] = specific tools
    // Supports exclusion with '!' prefix: ['*', '!bash'] = all tools except bash
    // disableTools is a convenience flag that overrides allowedTools to []
    const effectiveAllowedTools = options.disableTools ? [] : options.allowedTools;
    this.allowedTools = this._parseAllowedTools(effectiveAllowedTools);

    // Storage adapter (defaults to in-memory)
    this.storageAdapter = options.storageAdapter || new InMemoryStorageAdapter();

    // Hook manager
    this.hooks = new HookManager();

    // Register hooks from options
    if (options.hooks) {
      for (const [hookName, callback] of Object.entries(options.hooks)) {
        this.hooks.on(hookName, callback);
      }
    }

    // Bash configuration
    this.enableBash = !!options.enableBash;
    this.bashConfig = options.bashConfig || {};

    // Architecture context configuration
    const configuredArchitectureFileName =
      typeof options.architectureFileName === 'string' && options.architectureFileName.trim()
        ? options.architectureFileName
        : null;
    this.architectureFileName = configuredArchitectureFileName;
    this.architectureContext = null;
    this._architectureContextLoaded = false;

    // Search configuration - support both path (single) and allowedFolders (array)
    if (options.allowedFolders && Array.isArray(options.allowedFolders)) {
      this.allowedFolders = options.allowedFolders;
    } else if (options.path) {
      this.allowedFolders = [options.path];
    } else {
      this.allowedFolders = [process.cwd()];
    }

    // Working directory for resolving relative paths (separate from allowedFolders security)
    this.cwd = options.cwd || null;

    // API configuration
    this.clientApiProvider = options.provider || null;
    this.clientApiModel = options.model || null;
    this.clientApiKey = null; // Will be set from environment
    this.clientApiUrl = null;

    // Initialize token counter
    this.tokenCounter = new TokenCounter();

    if (this.debug) {
      console.log(`[DEBUG] Generated session ID for agent: ${this.sessionId}`);
      console.log(`[DEBUG] Maximum tool iterations configured: ${MAX_TOOL_ITERATIONS}`);
      console.log(`[DEBUG] Allow Edit (implement tool): ${this.allowEdit}`);
      console.log(`[DEBUG] Search delegation enabled: ${this.searchDelegate}`);
    }

    // Initialize tools
    this.initializeTools();

    // Initialize chat history
    this.history = [];

    // Initialize image tracking for agentic loop
    this.pendingImages = new Map(); // Map<imagePath, base64Data> to avoid reloading
    this.currentImages = []; // Currently active images for AI calls

    // Initialize event emitter for tool execution updates
    this.events = new EventEmitter();

    // MCP configuration
    this.enableMcp = !!options.enableMcp || process.env.ENABLE_MCP === '1';
    this.mcpConfigPath = options.mcpConfigPath || null;
    this.mcpConfig = options.mcpConfig || null;
    this.mcpServers = options.mcpServers || null; // Deprecated, keep for backward compatibility
    this.mcpBridge = null;
    this._mcpInitialized = false; // Track if MCP initialization has been attempted

    // Task management configuration
    this.enableTasks = !!options.enableTasks;
    this.taskManager = null; // Initialized per-request in answer()

    // Retry configuration
    this.retryConfig = options.retry || {};
    this.retryManager = null; // Will be initialized lazily when needed

    // Fallback configuration
    this.fallbackConfig = options.fallback || null;
    this.fallbackManager = null; // Will be initialized in initializeModel

    // Engine support - minimal interface for multi-engine compatibility
    this.engine = null; // Will be set in initializeModel or getEngine

    // Initialize the AI model
    this.initializeModel();

    // Note: MCP initialization is now done in initialize() method
    // Constructor must remain synchronous for backward compatibility
  }

  /**
   * Parse allowedTools configuration
   * @param {Array<string>|null|undefined} allowedTools - Tool filtering configuration
   * @returns {Object} Parsed configuration with isEnabled method
   * @private
   */
  _parseAllowedTools(allowedTools) {
    // Helper to check if tool matches a pattern (supports * wildcard)
    const matchesPattern = (toolName, pattern) => {
      if (!pattern.includes('*')) {
        return toolName === pattern;
      }
      const regexPattern = pattern.replace(/\*/g, '.*');
      return new RegExp(`^${regexPattern}$`).test(toolName);
    };

    // Default: all tools allowed
    if (!allowedTools || (Array.isArray(allowedTools) && allowedTools.includes('*'))) {
      const exclusions = Array.isArray(allowedTools)
        ? allowedTools.filter(t => t.startsWith('!')).map(t => t.slice(1))
        : [];

      return {
        mode: 'all',
        exclusions,
        isEnabled: (toolName) => !exclusions.some(pattern => matchesPattern(toolName, pattern))
      };
    }

    // Empty array or null: no tools (raw AI mode)
    if (Array.isArray(allowedTools) && allowedTools.length === 0) {
      return {
        mode: 'none',
        isEnabled: () => false
      };
    }

    // Specific tools allowed (with wildcard support)
    const allowedPatterns = allowedTools.filter(t => !t.startsWith('!'));
    return {
      mode: 'whitelist',
      allowed: allowedPatterns,
      isEnabled: (toolName) => allowedPatterns.some(pattern => matchesPattern(toolName, pattern))
    };
  }

  /**
   * Check if an MCP tool is allowed based on allowedTools configuration
   * Uses mcp__ prefix convention (like Claude Code)
   * @param {string} toolName - The MCP tool name (without mcp__ prefix)
   * @returns {boolean} - Whether the tool is allowed
   * @private
   */
  _isMcpToolAllowed(toolName) {
    const mcpToolName = `mcp__${toolName}`;
    return this.allowedTools.isEnabled(mcpToolName) || this.allowedTools.isEnabled(toolName);
  }

  /**
   * Filter MCP tools based on allowedTools configuration
   * @param {string[]} mcpToolNames - Array of MCP tool names
   * @returns {string[]} - Filtered array of allowed MCP tool names
   * @private
   */
  _filterMcpTools(mcpToolNames) {
    return mcpToolNames.filter(toolName => this._isMcpToolAllowed(toolName));
  }

  /**
   * Initialize the agent asynchronously (must be called after constructor)
   * This method initializes MCP and merges MCP tools into the tool list, and loads history from storage
   */
  async initialize() {
    // Check if we need to auto-detect claude-code or codex provider
    // This happens when no API keys are set and no provider is specified
    if (!this.provider && !this.clientApiProvider && this.apiType !== 'claude-code' && this.apiType !== 'codex') {
      // Check if initializeModel marked as uninitialized (no API keys)
      if (this.apiType === 'uninitialized') {
        const claudeAvailable = await this.isClaudeCommandAvailable();
        const codexAvailable = await this.isCodexCommandAvailable();

        if (claudeAvailable) {
          if (this.debug) {
            console.log('[DEBUG] No API keys found, but claude command detected');
            console.log('[DEBUG] Auto-switching to claude-code provider');
          }
          // Set provider to claude-code
          this.clientApiProvider = 'claude-code';
          this.provider = null;
          this.model = this.clientApiModel || 'claude-3-5-sonnet-20241022';
          this.apiType = 'claude-code';
        } else if (codexAvailable) {
          if (this.debug) {
            console.log('[DEBUG] No API keys found, but codex command detected');
            console.log('[DEBUG] Auto-switching to codex provider');
          }
          // Set provider to codex
          this.clientApiProvider = 'codex';
          this.provider = null;
          this.model = this.clientApiModel || 'gpt-4o';
          this.apiType = 'codex';
        } else {
          // Neither API keys nor CLI commands available
          throw new Error('No API key provided and neither claude nor codex command found. Please either:\n' +
            '1. Set an API key: ANTHROPIC_API_KEY, OPENAI_API_KEY, GOOGLE_GENERATIVE_AI_API_KEY, or AWS credentials\n' +
            '2. Install claude command from https://docs.claude.com/en/docs/claude-code\n' +
            '3. Install codex command from https://openai.com/codex');
        }
      }
    }

    // Load history from storage adapter
    try {
      const history = await this.storageAdapter.loadHistory(this.sessionId);
      this.history = history;

      if (this.debug && history.length > 0) {
        console.log(`[DEBUG] Loaded ${history.length} messages from storage for session ${this.sessionId}`);
      }

      // Emit storage load hook
      await this.hooks.emit(HOOK_TYPES.STORAGE_LOAD, {
        sessionId: this.sessionId,
        messages: history
      });
    } catch (error) {
      console.error(`[ERROR] Failed to load history from storage:`, error);
      // Continue with empty history if storage fails
      this.history = [];
    }

    // Initialize MCP if enabled and not already initialized
    if (this.enableMcp && !this._mcpInitialized) {
      this._mcpInitialized = true; // Prevent multiple initialization attempts
      try {
        await this.initializeMCP();

        // Merge MCP tools into toolImplementations for unified access
        // Apply allowedTools filtering using mcp__ prefix (like Claude Code)
        if (this.mcpBridge) {
          const mcpTools = this.mcpBridge.mcpTools || {};
          for (const [toolName, toolImpl] of Object.entries(mcpTools)) {
            if (this._isMcpToolAllowed(toolName)) {
              this.toolImplementations[toolName] = toolImpl;
            } else if (this.debug) {
              console.error(`[DEBUG] MCP tool '${toolName}' filtered out by allowedTools`);
            }
          }
        }

        // Log all available tools after MCP initialization
        if (this.debug) {
          const allToolNames = Object.keys(this.toolImplementations);
          const nativeToolCount = allToolNames.filter(name => !this.mcpBridge?.mcpTools?.[name]).length;
          const mcpToolCount = allToolNames.length - nativeToolCount;

          console.error('\n[DEBUG] ========================================');
          console.error('[DEBUG] All Tools Initialized');
          console.error(`[DEBUG] Native tools: ${nativeToolCount}, MCP tools: ${mcpToolCount}`);
          console.error('[DEBUG] Available tools:');
          for (const toolName of allToolNames) {
            const isMCP = this.mcpBridge?.mcpTools?.[toolName] ? ' (MCP)' : '';
            console.error(`[DEBUG]   - ${toolName}${isMCP}`);
          }
          console.error('[DEBUG] ========================================\n');
        }
      } catch (error) {
        console.error('[MCP ERROR] Failed to initialize MCP:', error.message);
        if (this.debug) {
          console.error('[MCP DEBUG] Full error details:', error);
        }
        this.mcpBridge = null;
      }
    }

    // Emit agent initialized hook
    await this.hooks.emit(HOOK_TYPES.AGENT_INITIALIZED, {
      sessionId: this.sessionId,
      agent: this
    });
  }

  /**
   * Initialize tools with configuration
   */
  initializeTools() {
    const isToolAllowed = (toolName) => this.allowedTools.isEnabled(toolName);

    const configOptions = {
      sessionId: this.sessionId,
      debug: this.debug,
      // Use explicit cwd if set, otherwise fall back to first allowed folder
      cwd: this.cwd || (this.allowedFolders.length > 0 ? this.allowedFolders[0] : process.cwd()),
      allowedFolders: this.allowedFolders,
      outline: this.outline,
      searchDelegate: this.searchDelegate,
      allowEdit: this.allowEdit,
      enableDelegate: this.enableDelegate,
      enableBash: this.enableBash,
      bashConfig: this.bashConfig,
      tracer: this.tracer,
      allowedTools: this.allowedTools,
      architectureFileName: this.architectureFileName,
      provider: this.clientApiProvider,
      model: this.clientApiModel,
      isToolAllowed
    };

    // Create base tools
    const baseTools = createTools(configOptions);
    
    // Create wrapped tools with event emission
    const wrappedTools = createWrappedTools(baseTools);

    // Store tool instances for execution (respect allowedTools + feature flags)
    this.toolImplementations = {};

    if (wrappedTools.searchToolInstance && isToolAllowed('search')) {
      this.toolImplementations.search = wrappedTools.searchToolInstance;
    }
    if (wrappedTools.queryToolInstance && isToolAllowed('query')) {
      this.toolImplementations.query = wrappedTools.queryToolInstance;
    }
    if (wrappedTools.extractToolInstance && isToolAllowed('extract')) {
      this.toolImplementations.extract = wrappedTools.extractToolInstance;
    }
    if (this.enableDelegate && wrappedTools.delegateToolInstance && isToolAllowed('delegate')) {
      this.toolImplementations.delegate = wrappedTools.delegateToolInstance;
    }

    // File browsing tools
    if (isToolAllowed('listFiles')) {
      this.toolImplementations.listFiles = listFilesToolInstance;
    }
    if (isToolAllowed('searchFiles')) {
      this.toolImplementations.searchFiles = searchFilesToolInstance;
    }

    if (this.enableSkills) {
      const registry = this._getSkillsRegistry();
      const { listSkillsToolInstance, useSkillToolInstance } = createSkillToolInstances({
        registry,
        activeSkills: this.activeSkills
      });

      if (isToolAllowed('listSkills')) {
        this.toolImplementations.listSkills = listSkillsToolInstance;
      }
      if (isToolAllowed('useSkill')) {
        this.toolImplementations.useSkill = useSkillToolInstance;
      }
    }

    // Image loading tool
    if (isToolAllowed('readImage')) {
      this.toolImplementations.readImage = {
        execute: async (params) => {
          const imagePath = params.path;
          if (!imagePath) {
            throw new Error('Image path is required');
          }

          // Validate extension before attempting to load
          // Use basename to prevent path traversal attacks (e.g., 'malicious.jpg/../../../etc/passwd')
          const filename = basename(imagePath);
          const extension = filename.toLowerCase().split('.').pop();

          // Always validate extension is in allowed list (defense-in-depth)
          if (!extension || !SUPPORTED_IMAGE_EXTENSIONS.includes(extension)) {
            throw new Error(`Invalid or unsupported image extension: ${extension}. Supported formats: ${SUPPORTED_IMAGE_EXTENSIONS.join(', ')}`);
          }

          // Check provider-specific format restrictions (e.g., SVG not supported by Google Gemini)
          if (this.apiType && !isFormatSupportedByProvider(extension, this.apiType)) {
            throw new Error(`Image format '${extension}' is not supported by the current AI provider (${this.apiType}). Try using a different image format like PNG or JPEG.`);
          }

          // Load the image using the existing loadImageIfValid method
          const loaded = await this.loadImageIfValid(imagePath);

          if (!loaded) {
            throw new Error(`Failed to load image: ${imagePath}. The file may not exist, be too large, have an unsupported format, or be outside allowed directories.`);
          }

          return `Image loaded successfully: ${imagePath}. The image is now available for analysis in the conversation.`;
        }
      };
    }

    // Add bash tool if enabled and allowed
    if (this.enableBash && wrappedTools.bashToolInstance && isToolAllowed('bash')) {
      this.toolImplementations.bash = wrappedTools.bashToolInstance;
    }

    // Add edit and create tools if enabled and allowed
    if (this.allowEdit) {
      if (wrappedTools.editToolInstance && isToolAllowed('edit')) {
        this.toolImplementations.edit = wrappedTools.editToolInstance;
      }
      if (wrappedTools.createToolInstance && isToolAllowed('create')) {
        this.toolImplementations.create = wrappedTools.createToolInstance;
      }
    }

    // Store wrapped tools for ACP system
    this.wrappedTools = wrappedTools;

    // Note: Task tool is registered dynamically in answer() when enableTasks is true
    // This is because TaskManager is created per-request (request-scoped)

    // Log available tools in debug mode
    if (this.debug) {
      console.error('\n[DEBUG] ========================================');
      console.error('[DEBUG] ProbeAgent Tools Initialized');
      console.error('[DEBUG] Session ID:', this.sessionId);
      console.error('[DEBUG] Available tools:');
      for (const toolName of Object.keys(this.toolImplementations)) {
        console.error(`[DEBUG]   - ${toolName}`);
      }
      console.error('[DEBUG] Allowed folders:', this.allowedFolders);
      console.error('[DEBUG] Outline mode:', this.outline);
      console.error('[DEBUG] ========================================\n');
    }
  }

  /**
   * Check if claude command is available on the system
   * Uses execFile instead of exec to avoid shell injection risks
   * @returns {Promise<boolean>} True if claude command is available
   * @private
   */
  async isClaudeCommandAvailable() {
    try {
      const { execFile } = await import('child_process');
      const { promisify } = await import('util');
      const execFileAsync = promisify(execFile);
      await execFileAsync('claude', ['--version'], { timeout: 5000 });
      return true;
    } catch (error) {
      return false;
    }
  }

  /**
   * Check if codex command is available on the system
   * Uses execFile instead of exec to avoid shell injection risks
   * @returns {Promise<boolean>} True if codex command is available
   * @private
   */
  async isCodexCommandAvailable() {
    try {
      const { execFile } = await import('child_process');
      const { promisify } = await import('util');
      const execFileAsync = promisify(execFile);
      await execFileAsync('codex', ['--version'], { timeout: 5000 });
      return true;
    } catch (error) {
      return false;
    }
  }

  /**
   * Initialize the AI model based on available API keys and forced provider setting
   */
  initializeModel() {
    // Get model override if provided (options.model takes precedence over environment variable)
    const modelName = this.clientApiModel || process.env.MODEL_NAME;

    // Check if we're in test mode and should use mock provider
    if (process.env.NODE_ENV === 'test' || process.env.USE_MOCK_AI === 'true') {
      this.initializeMockModel(modelName);
      return;
    }

    // Skip API key requirement for Claude Code (uses built-in access in Claude Code)
    if (this.clientApiProvider === 'claude-code' || process.env.USE_CLAUDE_CODE === 'true') {
      // Claude Code engine will be initialized lazily in getEngine()
      // Set minimal defaults for compatibility
      this.provider = null;
      this.model = modelName || 'claude-3-5-sonnet-20241022';
      this.apiType = 'claude-code';
      if (this.debug) {
        console.log('[DEBUG] Claude Code engine selected - will use built-in access if available');
      }
      return;
    }

    // Skip API key requirement for Codex CLI (uses built-in access in Codex CLI)
    if (this.clientApiProvider === 'codex' || process.env.USE_CODEX === 'true') {
      // Codex CLI engine will be initialized lazily in getEngine()
      // Set minimal defaults for compatibility
      this.provider = null;
      // Only set model if explicitly provided, otherwise let Codex use account default
      this.model = modelName || null;
      this.apiType = 'codex';
      if (this.debug) {
        console.log('[DEBUG] Codex CLI engine selected - will use built-in access if available');
        if (this.model) {
          console.log(`[DEBUG] Using model: ${this.model}`);
        } else {
          console.log('[DEBUG] Using Codex account default model');
        }
      }
      return;
    }

    // Get API keys from environment variables
    // Support both ANTHROPIC_API_KEY and ANTHROPIC_AUTH_TOKEN (used by Z.AI)
    const anthropicApiKey = process.env.ANTHROPIC_API_KEY || process.env.ANTHROPIC_AUTH_TOKEN;
    const openaiApiKey = process.env.OPENAI_API_KEY;
    // Support both GOOGLE_GENERATIVE_AI_API_KEY (official) and GOOGLE_API_KEY (legacy)
    const googleApiKey = process.env.GOOGLE_GENERATIVE_AI_API_KEY || process.env.GOOGLE_API_KEY;
    const awsAccessKeyId = process.env.AWS_ACCESS_KEY_ID;
    const awsSecretAccessKey = process.env.AWS_SECRET_ACCESS_KEY;
    const awsRegion = process.env.AWS_REGION;
    const awsSessionToken = process.env.AWS_SESSION_TOKEN;
    const awsApiKey = process.env.AWS_BEDROCK_API_KEY;

    // Get custom API URLs if provided
    const llmBaseUrl = process.env.LLM_BASE_URL;
    const anthropicApiUrl = process.env.ANTHROPIC_API_URL || process.env.ANTHROPIC_BASE_URL || llmBaseUrl;
    const openaiApiUrl = process.env.OPENAI_API_URL || llmBaseUrl;
    const googleApiUrl = process.env.GOOGLE_API_URL || llmBaseUrl;
    const awsBedrockBaseUrl = process.env.AWS_BEDROCK_BASE_URL || llmBaseUrl;

    // Use client-forced provider or environment variable
    const forceProvider = this.clientApiProvider || (process.env.FORCE_PROVIDER ? process.env.FORCE_PROVIDER.toLowerCase() : null);

    if (this.debug) {
      const hasAwsCredentials = !!(awsAccessKeyId && awsSecretAccessKey && awsRegion);
      const hasAwsApiKey = !!awsApiKey;
      console.log(`[DEBUG] Available API keys: Anthropic=${!!anthropicApiKey}, OpenAI=${!!openaiApiKey}, Google=${!!googleApiKey}, AWS Bedrock=${hasAwsCredentials || hasAwsApiKey}`);
      if (hasAwsCredentials) console.log(`[DEBUG] AWS credentials: AccessKey=${!!awsAccessKeyId}, SecretKey=${!!awsSecretAccessKey}, Region=${awsRegion}, SessionToken=${!!awsSessionToken}`);
      if (hasAwsApiKey) console.log(`[DEBUG] AWS API Key provided`);
      if (awsBedrockBaseUrl) console.log(`[DEBUG] AWS Bedrock base URL: ${awsBedrockBaseUrl}`);
      console.log(`[DEBUG] Force provider: ${forceProvider || '(not set)'}`);
      if (modelName) console.log(`[DEBUG] Model override: ${modelName}`);
    }

    // Check if a specific provider is forced
    if (forceProvider) {
      if (forceProvider === 'anthropic' && anthropicApiKey) {
        this.initializeAnthropicModel(anthropicApiKey, anthropicApiUrl, modelName);
        this.initializeFallbackManager(forceProvider, modelName);
        return;
      } else if (forceProvider === 'openai' && openaiApiKey) {
        this.initializeOpenAIModel(openaiApiKey, openaiApiUrl, modelName);
        this.initializeFallbackManager(forceProvider, modelName);
        return;
      } else if (forceProvider === 'google' && googleApiKey) {
        this.initializeGoogleModel(googleApiKey, googleApiUrl, modelName);
        this.initializeFallbackManager(forceProvider, modelName);
        return;
      } else if (forceProvider === 'bedrock' && ((awsAccessKeyId && awsSecretAccessKey && awsRegion) || awsApiKey)) {
        this.initializeBedrockModel(awsAccessKeyId, awsSecretAccessKey, awsRegion, awsSessionToken, awsApiKey, awsBedrockBaseUrl, modelName);
        this.initializeFallbackManager(forceProvider, modelName);
        return;
      }
      console.warn(`WARNING: Forced provider "${forceProvider}" selected but required API key is missing or invalid! Falling back to auto-detection.`);
    }

    // If no provider is forced or forced provider failed, use the first available API key
    if (anthropicApiKey) {
      this.initializeAnthropicModel(anthropicApiKey, anthropicApiUrl, modelName);
      this.initializeFallbackManager('anthropic', modelName);
    } else if (openaiApiKey) {
      this.initializeOpenAIModel(openaiApiKey, openaiApiUrl, modelName);
      this.initializeFallbackManager('openai', modelName);
    } else if (googleApiKey) {
      this.initializeGoogleModel(googleApiKey, googleApiUrl, modelName);
      this.initializeFallbackManager('google', modelName);
    } else if ((awsAccessKeyId && awsSecretAccessKey && awsRegion) || awsApiKey) {
      this.initializeBedrockModel(awsAccessKeyId, awsSecretAccessKey, awsRegion, awsSessionToken, awsApiKey, awsBedrockBaseUrl, modelName);
      this.initializeFallbackManager('bedrock', modelName);
    } else {
      // No API keys found - mark for potential claude-code auto-detection in initialize()
      this.apiType = 'uninitialized';
      if (this.debug) {
        console.log('[DEBUG] No API keys found - will check for claude command in initialize()');
      }
      // Don't throw error yet - will be checked in initialize() method
    }
  }

  /**
   * Initialize fallback manager based on configuration
   * @param {string} primaryProvider - The primary provider being used
   * @param {string} primaryModel - The primary model being used
   * @private
   */
  initializeFallbackManager(primaryProvider, primaryModel) {
    // Skip fallback initialization if explicitly disabled or in test mode
    if (this.fallbackConfig === false || process.env.DISABLE_FALLBACK === '1') {
      return;
    }

    // If fallback config is provided explicitly, use it
    if (this.fallbackConfig && this.fallbackConfig.providers) {
      try {
        this.fallbackManager = new FallbackManager({
          ...this.fallbackConfig,
          debug: this.debug
        });

        if (this.debug) {
          console.log(`[DEBUG] Fallback manager initialized with ${this.fallbackManager.providers.length} providers`);
        }
      } catch (error) {
        console.error('[WARNING] Failed to initialize fallback manager:', error.message);
      }
      return;
    }

    // Try to load from environment variables
    const envFallbackManager = createFallbackManagerFromEnv(this.debug);
    if (envFallbackManager) {
      this.fallbackManager = envFallbackManager;
      if (this.debug) {
        console.log(`[DEBUG] Fallback manager initialized from environment variables`);
      }
      return;
    }

    // Auto-build fallback from available providers if enabled
    if (process.env.AUTO_FALLBACK === '1' || this.fallbackConfig?.auto) {
      const providers = buildFallbackProvidersFromEnv({
        primaryProvider,
        primaryModel
      });

      if (providers.length > 1) {
        try {
          this.fallbackManager = new FallbackManager({
            strategy: 'custom',
            providers,
            debug: this.debug
          });

          if (this.debug) {
            console.log(`[DEBUG] Auto-fallback enabled with ${providers.length} providers`);
          }
        } catch (error) {
          console.error('[WARNING] Failed to initialize auto-fallback:', error.message);
        }
      }
    }
  }

  /**
   * Execute streamText with retry and fallback support
   * @param {Object} options - streamText options
   * @returns {Promise<Object>} - streamText result
   * @private
   */
  async streamTextWithRetryAndFallback(options) {
    // Check if we should use Claude Code engine
    if (this.clientApiProvider === 'claude-code' || process.env.USE_CLAUDE_CODE === 'true') {
      try {
        const engine = await this.getEngine();
        if (engine && engine.query) {
          // Convert Vercel AI SDK format to engine format
          // Extract the ORIGINAL user message as the main prompt (skip any warning messages)
          // Look for user messages that are NOT the warning message
          const userMessages = options.messages.filter(m =>
            m.role === 'user' &&
            !m.content.includes('WARNING: You have reached the maximum tool iterations limit')
          );
          const lastUserMessage = userMessages[userMessages.length - 1];
          const prompt = lastUserMessage ? lastUserMessage.content : '';

          // Pass system message and other options
          const engineOptions = {
            maxTokens: options.maxTokens,
            temperature: options.temperature,
            messages: options.messages,
            systemPrompt: options.messages.find(m => m.role === 'system')?.content
          };

          // Get the engine's query result (async generator)
          const engineStream = engine.query(prompt, engineOptions);

          // Create a text stream that extracts text from engine messages
          async function* createTextStream() {
            for await (const message of engineStream) {
              if (message.type === 'text' && message.content) {
                yield message.content;
              } else if (typeof message === 'string') {
                // If engine returns plain strings, pass them through
                yield message;
              }
              // Ignore other message types for the text stream
            }
          }

          // Wrap the engine result to match streamText interface
          return {
            textStream: createTextStream(),
            usage: Promise.resolve({}), // Engine should handle its own usage tracking
            // Add other streamText-compatible properties as needed
          };
        }
      } catch (error) {
        if (this.debug) {
          console.log(`[DEBUG] Failed to use Claude Code engine, falling back to Vercel:`, error.message);
        }
        // Fall through to use Vercel engine as fallback
      }
    }

    // Check if we should use Codex engine
    if (this.clientApiProvider === 'codex' || process.env.USE_CODEX === 'true') {
      try {
        const engine = await this.getEngine();
        if (engine && engine.query) {
          // Convert Vercel AI SDK format to engine format
          // Extract the ORIGINAL user message as the main prompt (skip any warning messages)
          // Look for user messages that are NOT the warning message
          const userMessages = options.messages.filter(m =>
            m.role === 'user' &&
            !m.content.includes('WARNING: You have reached the maximum tool iterations limit')
          );
          const lastUserMessage = userMessages[userMessages.length - 1];
          const prompt = lastUserMessage ? lastUserMessage.content : '';

          // Pass system message and other options
          const engineOptions = {
            maxTokens: options.maxTokens,
            temperature: options.temperature,
            messages: options.messages,
            systemPrompt: options.messages.find(m => m.role === 'system')?.content
          };

          // Get the engine's query result (async generator)
          const engineStream = engine.query(prompt, engineOptions);

          // Create a text stream that extracts text from engine messages
          async function* createTextStream() {
            for await (const message of engineStream) {
              if (message.type === 'text' && message.content) {
                yield message.content;
              } else if (typeof message === 'string') {
                // If engine returns plain strings, pass them through
                yield message;
              }
              // Ignore other message types for the text stream
            }
          }

          // Wrap the engine result to match streamText interface
          return {
            textStream: createTextStream(),
            usage: Promise.resolve({}), // Engine should handle its own usage tracking
            // Add other streamText-compatible properties as needed
          };
        }
      } catch (error) {
        if (this.debug) {
          console.log(`[DEBUG] Failed to use Codex engine, falling back to Vercel:`, error.message);
        }
        // Fall through to use Vercel engine as fallback
      }
    }

    // Initialize retry manager if not already created
    if (!this.retryManager) {
      this.retryManager = new RetryManager({
        maxRetries: this.retryConfig.maxRetries ?? 3,
        initialDelay: this.retryConfig.initialDelay ?? 1000,
        maxDelay: this.retryConfig.maxDelay ?? 30000,
        backoffFactor: this.retryConfig.backoffFactor ?? 2,
        retryableErrors: this.retryConfig.retryableErrors,
        debug: this.debug
      });
    }

    // If no fallback manager, just use retry with current provider
    if (!this.fallbackManager) {
      return await this.retryManager.executeWithRetry(
        () => streamText(options),
        {
          provider: this.apiType,
          model: this.model
        }
      );
    }

    // Use fallback manager with retry for each provider
    return await this.fallbackManager.executeWithFallback(
      async (provider, model, config) => {
        // Create options with the fallback provider
        const fallbackOptions = {
          ...options,
          model: provider(model)
        };

        // Create a retry manager for this specific provider
        const providerRetryManager = new RetryManager({
          maxRetries: config.maxRetries ?? this.retryConfig.maxRetries ?? 3,
          initialDelay: this.retryConfig.initialDelay ?? 1000,
          maxDelay: this.retryConfig.maxDelay ?? 30000,
          backoffFactor: this.retryConfig.backoffFactor ?? 2,
          retryableErrors: this.retryConfig.retryableErrors,
          debug: this.debug
        });

        // Execute with retry for this provider
        return await providerRetryManager.executeWithRetry(
          () => streamText(fallbackOptions),
          {
            provider: config.provider,
            model: model
          }
        );
      }
    );
  }

  /**
   * Initialize Anthropic model
   */
  initializeAnthropicModel(apiKey, apiUrl, modelName) {
    this.provider = createAnthropic({
      apiKey: apiKey,
      ...(apiUrl && { baseURL: apiUrl }),
    });
    this.model = modelName || 'claude-sonnet-4-5-20250929';
    this.apiType = 'anthropic';
    
    if (this.debug) {
      console.log(`Using Anthropic API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl})` : ''}`);
    }
  }

  /**
   * Initialize OpenAI model
   */
  initializeOpenAIModel(apiKey, apiUrl, modelName) {
    this.provider = createOpenAI({
      compatibility: 'strict',
      apiKey: apiKey,
      ...(apiUrl && { baseURL: apiUrl }),
    });
    this.model = modelName || 'gpt-5-thinking';
    this.apiType = 'openai';
    
    if (this.debug) {
      console.log(`Using OpenAI API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl})` : ''}`);
    }
  }

  /**
   * Initialize Google model
   */
  initializeGoogleModel(apiKey, apiUrl, modelName) {
    this.provider = createGoogleGenerativeAI({
      apiKey: apiKey,
      ...(apiUrl && { baseURL: apiUrl }),
    });
    this.model = modelName || 'gemini-2.5-pro';
    this.apiType = 'google';

    if (this.debug) {
      console.log(`Using Google API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl})` : ''}`);
    }
  }

  /**
   * Initialize AWS Bedrock model
   */
  initializeBedrockModel(accessKeyId, secretAccessKey, region, sessionToken, apiKey, baseURL, modelName) {
    // Build configuration object, only including defined values
    const config = {};
    
    // Authentication - prefer API key if provided, otherwise use AWS credentials
    if (apiKey) {
      config.apiKey = apiKey;
    } else if (accessKeyId && secretAccessKey) {
      config.accessKeyId = accessKeyId;
      config.secretAccessKey = secretAccessKey;
      if (sessionToken) {
        config.sessionToken = sessionToken;
      }
    }
    
    // Region is required for AWS credentials but optional for API key
    if (region) {
      config.region = region;
    }
    
    // Optional base URL
    if (baseURL) {
      config.baseURL = baseURL;
    }
    
    this.provider = createAmazonBedrock(config);
    this.model = modelName || 'anthropic.claude-sonnet-4-20250514-v1:0';
    this.apiType = 'bedrock';

    if (this.debug) {
      const authMethod = apiKey ? 'API Key' : 'AWS Credentials';
      const regionInfo = region ? ` (Region: ${region})` : '';
      const baseUrlInfo = baseURL ? ` (Base URL: ${baseURL})` : '';
      console.log(`Using AWS Bedrock API with model: ${this.model}${regionInfo} [Auth: ${authMethod}]${baseUrlInfo}`);
    }
  }

  /**
   * Get or create the AI engine based on configuration
   * @returns {Promise<Object>} Engine interface
   * @private
   */
  async getEngine() {
    // If engine already created, return it
    if (this.engine) {
      return this.engine;
    }

    // Try Claude Code engine if requested
    if (this.clientApiProvider === 'claude-code' || process.env.USE_CLAUDE_CODE === 'true') {
      try {
        const { createEnhancedClaudeCLIEngine } = await import('./engines/enhanced-claude-code.js');

        // For Claude Code, use a cleaner system prompt without XML formatting
        // since it has native MCP support for tools
        const systemPrompt = await this.getClaudeNativeSystemPrompt();

        this.engine = await createEnhancedClaudeCLIEngine({
          agent: this, // Pass reference to ProbeAgent for tool access
          systemPrompt: systemPrompt,
          customPrompt: this.customPrompt,
          sessionId: this.options?.sessionId,
          debug: this.debug,
          allowedTools: this.allowedTools  // Pass tool filtering configuration
        });
        if (this.debug) {
          console.log('[DEBUG] Using Claude Code engine with Probe tools');
          if (this.customPrompt) {
            console.log('[DEBUG] Using custom prompt/persona');
          }
        }
        return this.engine;
      } catch (error) {
        console.warn('[WARNING] Failed to load Claude Code engine:', error.message);
        console.warn('[WARNING] Falling back to Vercel AI SDK');
        this.clientApiProvider = null;
      }
    }

    // Try Codex CLI engine if requested
    if (this.clientApiProvider === 'codex' || process.env.USE_CODEX === 'true') {
      try {
        const { createCodexEngine } = await import('./engines/codex.js');

        // For Codex CLI, use a cleaner system prompt without XML formatting
        // since it has native MCP support for tools
        const systemPrompt = await this.getCodexNativeSystemPrompt();

        this.engine = await createCodexEngine({
          agent: this, // Pass reference to ProbeAgent for tool access
          systemPrompt: systemPrompt,
          customPrompt: this.customPrompt,
          sessionId: this.options?.sessionId,
          debug: this.debug,
          allowedTools: this.allowedTools,  // Pass tool filtering configuration
          model: this.model  // Pass model name (e.g., gpt-4o, o3, etc.)
        });
        if (this.debug) {
          console.log('[DEBUG] Using Codex CLI engine with Probe tools');
          if (this.customPrompt) {
            console.log('[DEBUG] Using custom prompt/persona');
          }
        }
        return this.engine;
      } catch (error) {
        console.warn('[WARNING] Failed to load Codex CLI engine:', error.message);
        console.warn('[WARNING] Falling back to Vercel AI SDK');
        this.clientApiProvider = null;
      }
    }

    // Default to enhanced Vercel AI SDK (wraps existing logic)
    const { createEnhancedVercelEngine } = await import('./engines/enhanced-vercel.js');
    this.engine = createEnhancedVercelEngine(this);
    if (this.debug) {
      console.log('[DEBUG] Using Vercel AI SDK engine');
    }
    return this.engine;
  }

  /**
   * Get session information including thread ID for resumability
   * @returns {Object} Session info with sessionId, threadId, messageCount
   */
  getSessionInfo() {
    if (this.engine && this.engine.getSession) {
      return this.engine.getSession();
    }
    return {
      id: this.sessionId,
      threadId: null,
      messageCount: 0
    };
  }

  /**
   * Close the agent and clean up resources (e.g., MCP servers)
   * @returns {Promise<void>}
   */
  async close() {
    if (this.engine && this.engine.close) {
      await this.engine.close();
    }
    if (this.mcpBridge) {
      // Clean up MCP bridge if needed
      this.mcpBridge = null;
    }
  }

  /**
   * Process assistant response content and detect/load image references
   * @param {string} content - The assistant's response content
   * @returns {Promise<void>}
   */
  async processImageReferences(content) {
    if (!content) return;

    // First, try to parse listFiles output format to extract directory context
    const listFilesDirectories = this.extractListFilesDirectories(content);

    // Enhanced pattern to detect image file mentions in various contexts
    // Looks for: "image", "file", "screenshot", etc. followed by path-like strings with image extensions
    const extensionsPattern = `(?:${SUPPORTED_IMAGE_EXTENSIONS.join('|')})`;
    const imagePatterns = [
      // Direct file path mentions: "./screenshot.png", "/path/to/image.jpg", etc.
      new RegExp(`(?:\\.?\\.\\/)?[^\\s"'<>\\[\\]]+\\\.${extensionsPattern}(?!\\w)`, 'gi'),
      // Contextual mentions: "look at image.png", "the file screenshot.jpg shows"
      new RegExp(`(?:image|file|screenshot|diagram|photo|picture|graphic)\\s*:?\\s*([^\\s"'<>\\[\\]]+\\.${extensionsPattern})(?!\\w)`, 'gi'),
      // Tool result mentions: often contain file paths
      new RegExp(`(?:found|saved|created|generated).*?([^\\s"'<>\\[\\]]+\\.${extensionsPattern})(?!\\w)`, 'gi')
    ];

    const foundPaths = new Set();

    // Extract potential image paths using all patterns
    for (const pattern of imagePatterns) {
      let match;
      while ((match = pattern.exec(content)) !== null) {
        // For patterns with capture groups, use the captured path; otherwise use the full match
        const imagePath = match[1] || match[0];
        if (imagePath && imagePath.length > 0) {
          foundPaths.add(imagePath.trim());
        }
      }
    }

    if (foundPaths.size === 0) return;

    if (this.debug) {
      console.log(`[DEBUG] Found ${foundPaths.size} potential image references:`, Array.from(foundPaths));
    }

    // Process each found path
    for (const imagePath of foundPaths) {
      // Try to resolve the path with directory context from listFiles output
      let resolvedPath = imagePath;

      // If the path is just a filename (no directory separator), try to find it in listFiles directories
      if (!imagePath.includes('/') && !imagePath.includes('\\')) {
        for (const dir of listFilesDirectories) {
          const potentialPath = resolve(dir, imagePath);
          // Check if this file exists by attempting to load it
          const loaded = await this.loadImageIfValid(potentialPath);
          if (loaded) {
            // Successfully loaded with this directory context
            if (this.debug) {
              console.log(`[DEBUG] Resolved ${imagePath} to ${potentialPath} using listFiles context`);
            }
            break; // Found it, no need to try other directories
          }
        }
      } else {
        // Path already has directory info, load as-is
        await this.loadImageIfValid(resolvedPath);
      }
    }
  }

  /**
   * Extract directory paths from tool output (both listFiles and extract tool)
   * @param {string} content - Tool output content
   * @returns {string[]} - Array of directory paths
   */
  extractListFilesDirectories(content) {
    const directories = [];

    // Pattern 1: Extract directory from extract tool "File:" header
    // Format: "File: /path/to/file.md" or "File: ./relative/path/file.md"
    const fileHeaderPattern = /^File:\s+(.+)$/gm;

    let match;
    while ((match = fileHeaderPattern.exec(content)) !== null) {
      const filePath = match[1].trim();
      // Get directory from file path
      const dir = dirname(filePath);
      if (dir && dir !== '.') {
        directories.push(dir);
        if (this.debug) {
          console.log(`[DEBUG] Extracted directory context from File header: ${dir}`);
        }
      }
    }

    // Pattern 2: Extract directory from listFiles output format: "/path/to/directory:"
    // Matches absolute paths (/path/to/dir:) or current directory markers (.:) or Windows paths (C:\path:) at start of line
    // Very strict to avoid matching random text like ".Something:" or "./Some text:"
    const dirPattern = /^(\/[^\n:]+|[A-Z]:\\[^\n:]+|\.\.?(?:\/[^\n:]+)?):\s*$/gm;

    while ((match = dirPattern.exec(content)) !== null) {
      const dirPath = match[1].trim();

      // Strict validation: must look like an actual filesystem path
      // Reject if contains spaces or other characters that wouldn't be in listFiles output
      const hasInvalidChars = /\s/.test(dirPath); // Contains whitespace

      // Validate this looks like an actual path, not random text
      // Must be either: absolute path (Unix or Windows), or ./ or ../ followed by valid path chars
      const isValidPath = (
        !hasInvalidChars && (
          dirPath.startsWith('/') ||  // Unix absolute path
          /^[A-Z]:\\/.test(dirPath) || // Windows absolute path (C:\)
          dirPath === '.' ||           // Current directory
          dirPath === '..' ||          // Parent directory
          (dirPath.startsWith('./') && dirPath.length > 2 && !dirPath.includes(' ')) ||   // ./something (no spaces)
          (dirPath.startsWith('../') && dirPath.length > 3 && !dirPath.includes(' '))     // ../something (no spaces)
        )
      );

      if (isValidPath) {
        // Avoid duplicates
        if (!directories.includes(dirPath)) {
          directories.push(dirPath);
          if (this.debug) {
            console.log(`[DEBUG] Extracted directory context from listFiles: ${dirPath}`);
          }
        }
      }
    }

    return directories;
  }

  /**
   * Load and cache an image if it's valid and accessible
   * @param {string} imagePath - Path to the image file
   * @returns {Promise<boolean>} - True if image was loaded successfully
   */
  async loadImageIfValid(imagePath) {
    try {
      // Skip if already loaded
      if (this.pendingImages.has(imagePath)) {
        if (this.debug) {
          console.log(`[DEBUG] Image already loaded: ${imagePath}`);
        }
        return true;
      }

      // Security validation: check if path is within any allowed directory
      // Use normalize() after resolve() to handle path traversal attempts (e.g., '/allowed/../etc/passwd')
      const allowedDirs = this.allowedFolders && this.allowedFolders.length > 0 ? this.allowedFolders : [process.cwd()];

      let absolutePath;
      let isPathAllowed = false;

      // If absolute path, check if it's within any allowed directory
      if (isAbsolute(imagePath)) {
        // Normalize to resolve any '..' sequences
        absolutePath = normalize(resolve(imagePath));
        isPathAllowed = allowedDirs.some(dir => {
          const normalizedDir = normalize(resolve(dir));
          // Ensure the path is within the allowed directory (add separator to prevent prefix attacks)
          return absolutePath === normalizedDir || absolutePath.startsWith(normalizedDir + sep);
        });
      } else {
        // For relative paths, try resolving against each allowed directory
        for (const dir of allowedDirs) {
          const normalizedDir = normalize(resolve(dir));
          const resolvedPath = normalize(resolve(dir, imagePath));
          // Ensure the resolved path is within the allowed directory
          if (resolvedPath === normalizedDir || resolvedPath.startsWith(normalizedDir + sep)) {
            absolutePath = resolvedPath;
            isPathAllowed = true;
            break;
          }
        }
      }
      
      // Security check: ensure path is within at least one allowed directory
      if (!isPathAllowed) {
        if (this.debug) {
          console.log(`[DEBUG] Image path outside allowed directories: ${imagePath}`);
        }
        return false;
      }

      // Check if file exists and get file stats
      let fileStats;
      try {
        fileStats = await stat(absolutePath);
      } catch (error) {
        if (this.debug) {
          console.log(`[DEBUG] Image file not found: ${absolutePath}`);
        }
        return false;
      }

      // Validate file size to prevent OOM attacks
      if (fileStats.size > MAX_IMAGE_FILE_SIZE) {
        if (this.debug) {
          console.log(`[DEBUG] Image file too large: ${absolutePath} (${fileStats.size} bytes, max: ${MAX_IMAGE_FILE_SIZE})`);
        }
        return false;
      }

      // Validate file extension
      const extension = absolutePath.toLowerCase().split('.').pop();
      if (!SUPPORTED_IMAGE_EXTENSIONS.includes(extension)) {
        if (this.debug) {
          console.log(`[DEBUG] Unsupported image format: ${extension}`);
        }
        return false;
      }

      // Note: Provider-specific format validation (e.g., SVG not supported by Google Gemini)
      // is handled by the readImage tool which provides explicit error messages.
      // loadImageIfValid is a lower-level method that only checks general format support.

      // Determine MIME type (from shared config)
      const mimeType = IMAGE_MIME_TYPES[extension];

      // Read and encode file asynchronously
      const fileBuffer = await readFile(absolutePath);
      const base64Data = fileBuffer.toString('base64');
      const dataUrl = `data:${mimeType};base64,${base64Data}`;

      // Cache the loaded image
      this.pendingImages.set(imagePath, dataUrl);

      if (this.debug) {
        console.log(`[DEBUG] Successfully loaded image: ${imagePath} (${fileBuffer.length} bytes)`);
      }

      return true;
    } catch (error) {
      if (this.debug) {
        console.log(`[DEBUG] Failed to load image ${imagePath}: ${error.message}`);
      }
      return false;
    }
  }

  /**
   * Get all currently loaded images as an array for AI model consumption
   * @returns {Array<string>} - Array of base64 data URLs
   */
  getCurrentImages() {
    return Array.from(this.pendingImages.values());
  }

  /**
   * Clear loaded images (useful for new conversations)
   */
  clearLoadedImages() {
    this.pendingImages.clear();
    this.currentImages = [];
    if (this.debug) {
      console.log('[DEBUG] Cleared all loaded images');
    }
  }

  /**
   * Prepare messages for AI consumption, adding images to the latest user message if available
   * @param {Array} messages - Current conversation messages
   * @returns {Array} - Messages formatted for AI SDK with potential image content
   */
  prepareMessagesWithImages(messages) {
    const loadedImages = this.getCurrentImages();
    
    // If no images loaded, return messages as-is
    if (loadedImages.length === 0) {
      return messages;
    }

    // Clone messages to avoid mutating the original
    const messagesWithImages = [...messages];
    
    // Find the last user message to attach images to
    const lastUserMessageIndex = messagesWithImages.map(m => m.role).lastIndexOf('user');
    
    if (lastUserMessageIndex === -1) {
      if (this.debug) {
        console.log('[DEBUG] No user messages found to attach images to');
      }
      return messages;
    }

    const lastUserMessage = messagesWithImages[lastUserMessageIndex];
    
    // Convert to multimodal format if we have images
    if (typeof lastUserMessage.content === 'string') {
      messagesWithImages[lastUserMessageIndex] = {
        ...lastUserMessage,
        content: [
          { type: 'text', text: lastUserMessage.content },
          ...loadedImages.map(imageData => ({
            type: 'image',
            image: imageData
          }))
        ]
      };

      if (this.debug) {
        console.log(`[DEBUG] Added ${loadedImages.length} images to the latest user message`);
      }
    }

    return messagesWithImages;
  }

  /**
   * Initialize mock model for testing
   */
  initializeMockModel(modelName) {
    this.provider = createMockProvider();
    this.model = modelName || 'mock-model';
    this.apiType = 'mock';

    if (this.debug) {
      console.log(`Using Mock API with model: ${this.model}`);
    }
  }

  /**
   * Initialize MCP bridge and load tools
   */
  async initializeMCP() {
    if (!this.enableMcp) return;

    try {
      let mcpConfig = null;

      // Priority order: mcpConfig > mcpConfigPath > mcpServers (deprecated) > auto-discovery
      if (this.mcpConfig) {
        // Direct config object provided (SDK usage)
        mcpConfig = this.mcpConfig;
        if (this.debug) {
          console.error('[MCP DEBUG] Using provided MCP config object');
        }
      } else if (this.mcpConfigPath) {
        // Explicit config path provided
        try {
          mcpConfig = loadMCPConfigurationFromPath(this.mcpConfigPath);
          if (this.debug) {
            console.error(`[MCP DEBUG] Loaded MCP config from: ${this.mcpConfigPath}`);
          }
        } catch (error) {
          throw new Error(`Failed to load MCP config from ${this.mcpConfigPath}: ${error.message}`);
        }
      } else if (this.mcpServers) {
        // Backward compatibility: convert old mcpServers format
        mcpConfig = { mcpServers: this.mcpServers };
        if (this.debug) {
          console.error('[MCP DEBUG] Using deprecated mcpServers option. Consider using mcpConfig instead.');
        }
      } else {
        // No explicit config provided - will attempt auto-discovery
        // This is important for CLI usage where config files may exist
        if (this.debug) {
          console.error('[MCP DEBUG] No explicit MCP config provided, will attempt auto-discovery');
        }
        // Pass null to trigger auto-discovery in MCPXmlBridge
        mcpConfig = null;
      }

      // Initialize the MCP XML bridge
      this.mcpBridge = new MCPXmlBridge({ debug: this.debug });
      await this.mcpBridge.initialize(mcpConfig);

      const mcpToolNames = this.mcpBridge.getToolNames();
      const mcpToolCount = mcpToolNames.length;
      if (mcpToolCount > 0) {
        if (this.debug) {
          console.error('\n[MCP DEBUG] ========================================');
          console.error(`[MCP DEBUG] MCP Tools Initialized (${mcpToolCount} tools)`);
          console.error('[MCP DEBUG] Available MCP tools:');
          for (const toolName of mcpToolNames) {
            console.error(`[MCP DEBUG]   - ${toolName}`);
          }
          console.error('[MCP DEBUG] ========================================\n');
        }
      } else {
        // For backward compatibility: if no tools were loaded, set bridge to null
        // This maintains the behavior expected by existing tests
        if (this.debug) {
          console.error('[MCP DEBUG] No MCP tools loaded, setting bridge to null');
        }
        this.mcpBridge = null;
      }
    } catch (error) {
      console.error('[MCP ERROR] Error initializing MCP:', error.message);
      if (this.debug) {
        console.error('[MCP DEBUG] Full error details:', error);
      }
      this.mcpBridge = null;
    }
  }

  /**
   * Load architecture context from repository root (case-insensitive filename match)
   * @returns {Promise<Object|null>} Architecture context with { name, path, content, sources, primarySource, guidanceSource, architectureSource } or null
   */
  async loadArchitectureContext() {
    if (this._architectureContextLoaded) {
      return this.architectureContext;
    }

    const rootDirectory = this.allowedFolders.length > 0 ? this.allowedFolders[0] : process.cwd();
    const configuredName =
      typeof this.architectureFileName === 'string' ? this.architectureFileName.trim() : '';
    const hasConfiguredName = !!configuredName;
    let guidanceCandidates = [];

    if (hasConfiguredName) {
      const targetName = basename(configuredName);

      // Only allow simple filenames (no path separators or traversal)
      if (
        configuredName !== targetName ||
        configuredName.includes('/') ||
        configuredName.includes('\\') ||
        configuredName.includes('..') ||
        isAbsolute(configuredName)
      ) {
        console.warn(`[WARN] Invalid architectureFileName (must be a simple filename): ${configuredName}`);
      } else if (targetName) {
        const targetLower = targetName.toLowerCase();
        if (targetLower === 'agents.md') {
          guidanceCandidates = ['agents.md', 'claude.md'];
        } else {
          guidanceCandidates = [targetName];
        }
      }
    } else {
      guidanceCandidates = ['agents.md', 'claude.md'];
    }

    if (!existsSync(rootDirectory)) {
      this._architectureContextLoaded = true;
      return null;
    }

    let entries;
    try {
      entries = await readdir(rootDirectory, { withFileTypes: true });
    } catch (error) {
      this.architectureContext = null;
      if (error && (error.code === 'EACCES' || error.code === 'EPERM')) {
        console.warn(`[WARN] Cannot read architecture context directory: ${rootDirectory} (${error.code})`);
      } else if (this.debug) {
        console.log(`[DEBUG] Could not list architecture context directory: ${error.message}`);
      }
      return null;
    }

    const entryByLower = new Map();
    for (const entry of entries) {
      if (entry.isSymbolicLink()) {
        continue;
      }
      if (!entry.isFile()) {
        continue;
      }
      entryByLower.set(entry.name.toLowerCase(), entry);
    }

    let guidanceMatch = null;
    for (const candidateName of guidanceCandidates) {
      const entry = entryByLower.get(candidateName.toLowerCase());
      if (entry) {
        guidanceMatch = entry;
        break;
      }
    }

    const architectureMatch = entryByLower.get('architecture.md');
    const guidanceKey = guidanceMatch ? guidanceMatch.name.toLowerCase() : null;
    const architectureKey = architectureMatch ? architectureMatch.name.toLowerCase() : null;

    if (!guidanceMatch && !architectureMatch) {
      this._architectureContextLoaded = true;
      return null;
    }

    const uniqueEntries = [];
    const seen = new Set();
    const pushEntry = (entry) => {
      if (!entry) return;
      const key = entry.name.toLowerCase();
      if (seen.has(key)) return;
      seen.add(key);
      uniqueEntries.push(entry);
    };

    pushEntry(guidanceMatch);
    pushEntry(architectureMatch);

    const contexts = [];
    for (const entry of uniqueEntries) {
      const filePath = resolve(rootDirectory, entry.name);
      try {
        const content = await readFile(filePath, 'utf8');
        let kind = 'other';
        const entryKey = entry.name.toLowerCase();
        if (guidanceKey && entryKey === guidanceKey) {
          kind = 'guidance';
        } else if (architectureKey && entryKey === architectureKey) {
          kind = 'architecture';
        }
        contexts.push({
          name: entry.name,
          path: filePath,
          content,
          kind
        });
      } catch (error) {
        if (error && (error.code === 'EACCES' || error.code === 'EPERM')) {
          console.warn(`[WARN] Cannot read architecture context file: ${filePath} (${error.code})`);
        } else if (error && error.code === 'ENOENT') {
          if (this.debug) {
            console.log(`[DEBUG] Architecture context file disappeared: ${filePath}`);
          }
        } else {
          console.warn(`[WARN] Failed to read architecture context file: ${filePath} (${error.message})`);
        }
      }
    }

    if (!contexts.length) {
      this.architectureContext = null;
      this._architectureContextLoaded = true;
      return null;
    }

    const guidanceSource = contexts.find((context) => context.kind === 'guidance') || null;
    const architectureSource = contexts.find((context) => context.kind === 'architecture') || null;
    const primarySource = guidanceSource || architectureSource || contexts[0];

    this.architectureContext = {
      name: primarySource?.name || null,
      path: primarySource?.path || null,
      content: contexts.map((context) => context.content).join('\n\n'),
      sources: contexts,
      primarySource,
      guidanceSource,
      architectureSource
    };
    this._architectureContextLoaded = true;

    return this.architectureContext;
  }

  /**
   * Format architecture context for prompt inclusion
   * @returns {string} Architecture section or empty string
   */
  getArchitectureSection() {
    if (!this.architectureContext?.content) {
      return '';
    }

    return `\n\n# Architecture\n\n${this.architectureContext.content}\n`;
  }

  _getSkillsRepoRoot() {
    if (this.allowedFolders && this.allowedFolders.length > 0) {
      return resolve(this.allowedFolders[0]);
    }
    return process.cwd();
  }

  _getSkillsRegistry() {
    if (!this.skillsRegistry) {
      this.skillsRegistry = new SkillRegistry({
        repoRoot: this._getSkillsRepoRoot(),
        skillDirs: this.skillDirs || undefined,
        debug: this.debug
      });
    }
    return this.skillsRegistry;
  }

  async _loadSkillsMetadata() {
    if (!this.enableSkills) return [];
    return await this._getSkillsRegistry().loadSkills();
  }

  async _getAvailableSkillsXml() {
    const skills = await this._loadSkillsMetadata();
    if (!skills.length) return '';
    return formatAvailableSkillsXml(skills);
  }

  /**
   * Get system prompt for Claude native engines (CLI/SDK) without XML formatting
   * These engines have native MCP support and don't need XML instructions
   */
  async getClaudeNativeSystemPrompt() {
    await this.loadArchitectureContext();
    let systemPrompt = '';

    // Add persona/role if configured
    if (this.customPrompt) {
      systemPrompt += this.customPrompt + '\n\n';
    } else if (this.promptType && predefinedPrompts[this.promptType]) {
      systemPrompt += predefinedPrompts[this.promptType] + '\n\n';
    } else {
      // Use default code-explorer prompt
      systemPrompt += predefinedPrompts['code-explorer'] + '\n\n';
    }

    // Add high-level instructions about when to use tools
    systemPrompt += `You have access to powerful code search and analysis tools through MCP:
- search: Find code patterns using semantic search
- extract: Extract specific code sections with context
- query: Use AST patterns for structural code matching
- listFiles: Browse directory contents
- searchFiles: Find files by name patterns`;

    if (this.enableBash) {
      systemPrompt += `\n- bash: Execute bash commands for system operations`;
    }

    const searchGuidance = this.searchDelegate
      ? '1. Start with search to retrieve extracted code blocks'
      : '1. Start with search to find relevant code patterns';
    const extractGuidance = this.searchDelegate
      ? '2. Use extract only if you need more context or a full file'
      : '2. Use extract to get detailed context when needed';

    systemPrompt += `\n
When exploring code:
${searchGuidance}
${extractGuidance}
3. Prefer focused, specific searches over broad queries
4. Combine multiple tools to build complete understanding`;

    // Add workspace context
    if (this.allowedFolders && this.allowedFolders.length > 0) {
      systemPrompt += `\n\nWorkspace: ${this.allowedFolders.join(', ')}`;
    }

    // Add repository structure if available
    if (this.fileList) {
      systemPrompt += `\n\n# Repository Structure\n`;
      systemPrompt += `You are working with a repository located at: ${this.allowedFolders[0]}\n\n`;
      systemPrompt += `Here's an overview of the repository structure (showing up to 100 most relevant files):\n\n`;
      systemPrompt += '```\n' + this.fileList + '\n```\n';
    }

    // Add architecture context if available
    systemPrompt += this.getArchitectureSection();

    return systemPrompt;
  }

  /**
   * Get system prompt for Codex CLI (similar to Claude but optimized for Codex)
   */
  async getCodexNativeSystemPrompt() {
    await this.loadArchitectureContext();
    let systemPrompt = '';

    // Add persona/role if configured
    if (this.customPrompt) {
      systemPrompt += this.customPrompt + '\n\n';
    } else if (this.promptType && predefinedPrompts[this.promptType]) {
      systemPrompt += predefinedPrompts[this.promptType] + '\n\n';
    } else {
      // Use default code-explorer prompt
      systemPrompt += predefinedPrompts['code-explorer'] + '\n\n';
    }

    // Add high-level instructions about when to use tools
    systemPrompt += `You have access to powerful code search and analysis tools through MCP:
- search: Find code patterns using semantic search
- extract: Extract specific code sections with context
- query: Use AST patterns for structural code matching
- listFiles: Browse directory contents
- searchFiles: Find files by name patterns`;

    if (this.enableBash) {
      systemPrompt += `\n- bash: Execute bash commands for system operations`;
    }

    const searchGuidance = this.searchDelegate
      ? '1. Start with search to retrieve extracted code blocks'
      : '1. Start with search to find relevant code patterns';
    const extractGuidance = this.searchDelegate
      ? '2. Use extract only if you need more context or a full file'
      : '2. Use extract to get detailed context when needed';

    systemPrompt += `\n
When exploring code:
${searchGuidance}
${extractGuidance}
3. Prefer focused, specific searches over broad queries
4. Combine multiple tools to build complete understanding`;

    // Add workspace context
    if (this.allowedFolders && this.allowedFolders.length > 0) {
      systemPrompt += `\n\nWorkspace: ${this.allowedFolders.join(', ')}`;
    }

    // Add repository structure if available
    if (this.fileList) {
      systemPrompt += `\n\n# Repository Structure\n`;
      systemPrompt += `You are working with a repository located at: ${this.allowedFolders[0]}\n\n`;
      systemPrompt += `Here's an overview of the repository structure (showing up to 100 most relevant files):\n\n`;
      systemPrompt += '```\n' + this.fileList + '\n```\n';
    }

    // Add architecture context if available
    systemPrompt += this.getArchitectureSection();

    return systemPrompt;
  }

  /**
   * Get the system message with instructions for the AI (XML Tool Format)
   */
  async getSystemMessage() {
    // Lazy initialize MCP if enabled but not yet initialized
    if (this.enableMcp && !this.mcpBridge && !this._mcpInitialized) {
      this._mcpInitialized = true; // Prevent multiple initialization attempts
      try {
        await this.initializeMCP();

        // Merge MCP tools into toolImplementations for unified access
        // Apply allowedTools filtering using mcp__ prefix (like Claude Code)
        if (this.mcpBridge) {
          const mcpTools = this.mcpBridge.mcpTools || {};
          for (const [toolName, toolImpl] of Object.entries(mcpTools)) {
            if (this._isMcpToolAllowed(toolName)) {
              this.toolImplementations[toolName] = toolImpl;
            } else if (this.debug) {
              console.error(`[DEBUG] MCP tool '${toolName}' filtered out by allowedTools`);
            }
          }
        }
      } catch (error) {
        console.error('[MCP ERROR] Failed to lazy-initialize MCP:', error.message);
        if (this.debug) {
          console.error('[MCP DEBUG] Full error details:', error);
        }
      }
    }

    // Build tool definitions based on allowedTools configuration
    let toolDefinitions = '';

    // Helper to check if a tool is allowed
    const isToolAllowed = (toolName) => this.allowedTools.isEnabled(toolName);

    // Core tools (filtered by allowedTools)
    if (isToolAllowed('search')) {
      const searchDefinition = this.searchDelegate
        ? `${searchToolDefinition}\n**Note:** This search tool delegates code searching to a dedicated subagent and returns extracted code blocks. Use extract only to expand context or if search returns no code.`
        : searchToolDefinition;
      toolDefinitions += `${searchDefinition}\n`;
    }
    if (isToolAllowed('query')) {
      toolDefinitions += `${queryToolDefinition}\n`;
    }
    if (isToolAllowed('extract')) {
      toolDefinitions += `${extractToolDefinition}\n`;
    }
    if (isToolAllowed('listFiles')) {
      toolDefinitions += `${listFilesToolDefinition}\n`;
    }
    if (isToolAllowed('searchFiles')) {
      toolDefinitions += `${searchFilesToolDefinition}\n`;
    }
    if (this.enableSkills && isToolAllowed('listSkills')) {
      toolDefinitions += `${listSkillsToolDefinition}\n`;
    }
    if (this.enableSkills && isToolAllowed('useSkill')) {
      toolDefinitions += `${useSkillToolDefinition}\n`;
    }
    if (isToolAllowed('readImage')) {
      toolDefinitions += `${readImageToolDefinition}\n`;
    }

    // Edit tools (require both allowEdit flag AND allowedTools permission)
    if (this.allowEdit && isToolAllowed('implement')) {
      toolDefinitions += `${implementToolDefinition}\n`;
    }
    if (this.allowEdit && isToolAllowed('edit')) {
      toolDefinitions += `${editToolDefinition}\n`;
    }
    if (this.allowEdit && isToolAllowed('create')) {
      toolDefinitions += `${createToolDefinition}\n`;
    }

    // Bash tool (require both enableBash flag AND allowedTools permission)
    if (this.enableBash && isToolAllowed('bash')) {
      toolDefinitions += `${bashToolDefinition}\n`;
    }

    // Task tool (require both enableTasks flag AND allowedTools permission)
    if (this.enableTasks && isToolAllowed('task')) {
      toolDefinitions += `${taskToolDefinition}\n`;
    }

    // Always include attempt_completion (unless explicitly disabled in raw AI mode)
    if (isToolAllowed('attempt_completion')) {
      toolDefinitions += `${attemptCompletionToolDefinition}\n`;
    }

    // Delegate tool (require both enableDelegate flag AND allowedTools permission)
    // Place after attempt_completion as it's an optional tool
    if (this.enableDelegate && isToolAllowed('delegate')) {
      toolDefinitions += `${delegateToolDefinition}\n`;
    }

    // Build XML tool guidelines
    let xmlToolGuidelines = `
# Tool Use Formatting

Tool use MUST be formatted using XML-style tags. Each tool call requires BOTH opening and closing tags with the exact tool name. Each parameter is similarly enclosed within its own set of opening and closing tags. You MUST use exactly ONE tool call per message until you are ready to complete the task.

**CRITICAL: Every XML tag MUST have both opening <tag> and closing </tag> parts.**

Structure (note the closing tags):
<tool_name>
<parameter1_name>value1</parameter1_name>
<parameter2_name>value2</parameter2_name>
...
</tool_name>

Examples:
<search>
<query>error handling</query>
<path>src/search</path>
</search>

<extract>
<targets>src/config.js:15-25</targets>
</extract>

<attempt_completion>
The configuration is loaded from src/config.js lines 15-25 which contains the database settings.
</attempt_completion>

# Special Case: Quick Completion
If your previous response was already correct and complete, you may respond with just:
<attempt_complete>
This signals to use your previous response as the final answer without repeating content.

# Thinking Process

Before using a tool, analyze the situation within <thinking></thinking> tags. This helps you organize your thoughts and make better decisions.

Example:
<thinking>
I need to find code related to error handling in the search module. The most appropriate tool for this is the search tool, which requires a query parameter and a path parameter. I have both the query ("error handling") and the path ("src/search"), so I can proceed with the search.
</thinking>

# Tool Use Guidelines

1. Think step-by-step about how to achieve the user's goal.
2. Use <thinking></thinking> tags to analyze the situation and determine the appropriate tool.
3. Choose **one** tool that helps achieve the current step.
4. Format the tool call using the specified XML format with BOTH opening and closing tags. Ensure all required parameters are included.
5. **You MUST respond with exactly one tool call in the specified XML format in each turn.**
6. Wait for the tool execution result, which will be provided in the next message (within a <tool_result> block).
7. Analyze the tool result and decide the next step. If more tool calls are needed, repeat steps 2-6.
8. If the task is fully complete and all previous steps were successful, use the \`<attempt_completion>\` tool to provide the final answer. This is the ONLY way to finish the task.
9. If you cannot proceed (e.g., missing information, invalid request), use \`<attempt_completion>\` to explain the issue clearly with an appropriate message directly inside the tags.
10. If your previous response was already correct and complete, you may use \`<attempt_complete>\` as a shorthand.

Available Tools:
- search: Search code using keyword queries${this.searchDelegate ? ' (returns extracted code blocks via a dedicated subagent)' : ''}.
- query: Search code using structural AST patterns.
- extract: Extract specific code blocks or lines from files.
- listFiles: List files and directories in a specified location.
- searchFiles: Find files matching a glob pattern with recursive search capability.
${this.enableSkills ? '- listSkills: List available agent skills discovered in the repository.\n- useSkill: Load and activate a specific skill\'s instructions.\n' : ''}- readImage: Read and load an image file for AI analysis.
${this.allowEdit ? '- implement: Implement a feature or fix a bug using aider.\n- edit: Edit files using exact string replacement.\n- create: Create new files with specified content.\n' : ''}${this.enableDelegate ? '- delegate: Delegate big distinct tasks to specialized probe subagents.\n' : ''}${this.enableBash ? '- bash: Execute bash commands for system operations.\n' : ''}${this.enableTasks ? '- task: Manage tasks for tracking progress (create, update, complete, delete, list).\n' : ''}- attempt_completion: Finalize the task and provide the result to the user.
- attempt_complete: Quick completion using previous response (shorthand).
`;

    // Common instructions
    const commonInstructions = `<instructions>
Follow these instructions carefully:
1. Analyze the user's request.
2. Use <thinking></thinking> tags to analyze the situation and determine the appropriate tool for each step.
3. Use the available tools step-by-step to fulfill the request.
4. You should always prefer the \`search\` tool for code-related questions.${this.searchDelegate ? ' It already returns extracted code blocks; use \`extract\` only to expand context or read full files.' : ' Read full files only if really necessary.'}
5. Ensure to get really deep and understand the full picture before answering.
6. You MUST respond with exactly ONE tool call per message, using the specified XML format, until the task is complete.
7. Wait for the tool execution result (provided in the next user message in a <tool_result> block) before proceeding to the next step.
8. Once the task is fully completed, use the '<attempt_completion>' tool to provide the final result. This is the ONLY way to signal completion.
9. Prefer concise and focused search queries. Use specific keywords and phrases to narrow down results.${this.allowEdit ? `
10. When modifying files, choose the appropriate tool:
    - Use 'edit' for precise changes to existing files (requires exact string match)
    - Use 'create' for new files or complete file rewrites` : ''}
</instructions>
`;

    // Use predefined prompts from shared module (imported at top of file)
    let systemMessage = '';

    // Use custom prompt if provided
    if (this.customPrompt) {
      systemMessage = "<role>" + this.customPrompt + "</role>";
      if (this.debug) {
        console.log(`[DEBUG] Using custom prompt`);
      }
    }
    // Use predefined prompt if specified
    else if (this.promptType && predefinedPrompts[this.promptType]) {
      systemMessage = "<role>" + predefinedPrompts[this.promptType] + "</role>";
      if (this.debug) {
        console.log(`[DEBUG] Using predefined prompt: ${this.promptType}`);
      }
      // Add common instructions to predefined prompts
      systemMessage += commonInstructions;
    } else {
      // Use the default prompt (code explorer) if no prompt type is specified
      systemMessage = "<role>" + predefinedPrompts['code-explorer'] + "</role>";
      if (this.debug) {
        console.log(`[DEBUG] Using default prompt: code explorer`);
      }
      // Add common instructions to the default prompt
      systemMessage += commonInstructions;
    }

    // Add XML Tool Guidelines
    systemMessage += `\n${xmlToolGuidelines}\n`;

    // Add Tool Definitions
    systemMessage += `\n# Tools Available\n${toolDefinitions}\n`;

    // Add available skills (metadata only)
    if (this.enableSkills) {
      const skillsXml = await this._getAvailableSkillsXml();
      if (skillsXml) {
        systemMessage += `\n# Available Skills\n${skillsXml}\n\nTo use a skill, call the useSkill tool with its name.\n`;
      }
    }

    // Add task management system prompt if enabled
    if (this.enableTasks) {
      systemMessage += `\n${taskSystemPrompt}\n`;
    }

    // Add MCP tools if available (filtered by allowedTools)
    if (this.mcpBridge && this.mcpBridge.getToolNames().length > 0) {
      const allMcpTools = this.mcpBridge.getToolNames();
      const allowedMcpTools = this._filterMcpTools(allMcpTools);

      if (allowedMcpTools.length > 0) {
        systemMessage += `\n## MCP Tools (JSON parameters in <params> tag)\n`;
        // Get only allowed MCP tool definitions
        systemMessage += this.mcpBridge.getXmlToolDefinitions(allowedMcpTools);
        systemMessage += `\n\nFor MCP tools, use JSON format within the params tag, e.g.:\n<mcp_tool>\n<params>\n{"key": "value"}\n</params>\n</mcp_tool>\n`;
      }
    }

    // Add folder information
    const searchDirectory = this.allowedFolders.length > 0 ? this.allowedFolders[0] : process.cwd();
    if (this.debug) {
      console.log(`[DEBUG] Generating file list for base directory: ${searchDirectory}...`);
    }

    try {
      const files = await listFilesByLevel({
        directory: searchDirectory,
        maxFiles: 100,
        respectGitignore: !process.env.PROBE_NO_GITIGNORE || process.env.PROBE_NO_GITIGNORE === '',
        cwd: process.cwd()
      });

      systemMessage += `\n# Repository Structure\n\nYou are working with a repository located at: ${searchDirectory}\n\nHere's an overview of the repository structure (showing up to 100 most relevant files):\n\n\`\`\`\n${files}\n\`\`\`\n\n`;
    } catch (error) {
      if (this.debug) {
        console.log(`[DEBUG] Could not generate file list: ${error.message}`);
      }
      systemMessage += `\n# Repository Structure\n\nYou are working with a repository located at: ${searchDirectory}\n\n`;
    }

    // Add architecture context if available
    await this.loadArchitectureContext();
    systemMessage += this.getArchitectureSection();

    if (this.allowedFolders.length > 0) {
      systemMessage += `\n**Important**: For security reasons, you can only search within these allowed folders: ${this.allowedFolders.join(', ')}\n\n`;
    }

    return systemMessage;
  }

  /**
   * Answer a question using the agentic flow
   * @param {string} message - The user's question
   * @param {Array} [images] - Optional array of image data (base64 strings or URLs)
   * @param {Object|string} [schemaOrOptions] - Can be either:
   *   - A string: JSON schema for structured output (backwards compatible)
   *   - An object: Options object with schema and other options
   * @param {string} [schemaOrOptions.schema] - JSON schema string for structured output
   * @returns {Promise<string>} - The final answer
   */
  async answer(message, images = [], schemaOrOptions = {}) {
    if (!message || typeof message !== 'string' || message.trim().length === 0) {
      throw new Error('Message is required and must be a non-empty string');
    }

    // Handle backwards compatibility - if third argument is a string, treat it as schema
    let options = {};
    if (typeof schemaOrOptions === 'string') {
      options = { schema: schemaOrOptions };
    } else {
      options = schemaOrOptions || {};
    }

    try {
      // Track initial history length for storage
      const oldHistoryLength = this.history.length;

      // START CHECKPOINT: Initialize task management for this request
      if (this.enableTasks) {
        // Create fresh TaskManager for each request (request-scoped)
        this.taskManager = new TaskManager({ debug: this.debug });

        // Register task tool for this request
        const isToolAllowed = (toolName) => this.allowedTools.isEnabled(toolName);
        if (isToolAllowed('task')) {
          this.toolImplementations.task = createTaskTool({
            taskManager: this.taskManager,
            debug: this.debug
          });
        }

        if (this.debug) {
          console.log('[DEBUG] Task management initialized for this request');
        }
      }

      // Emit user message hook
      await this.hooks.emit(HOOK_TYPES.MESSAGE_USER, {
        sessionId: this.sessionId,
        message,
        images
      });

      // Generate system message
      const systemMessage = await this.getSystemMessage();

      // Create user message with optional image support
      let userMessage = { role: 'user', content: message.trim() };

      // START CHECKPOINT: Inject task guidance if tasks are enabled
      if (this.enableTasks) {
        userMessage.content = userMessage.content + '\n\n' + taskGuidancePrompt;
        if (this.debug) {
          console.log('[DEBUG] Task guidance injected into user message');
        }
      }

      // If schema is provided, prepend JSON format requirement to user message
      if (options.schema && !options._schemaFormatted) {
        const schemaInstructions = generateSchemaInstructions(options.schema, { debug: this.debug });
        userMessage.content = message.trim() + schemaInstructions;
      }

      // If images are provided, use multi-modal message format
      if (images && images.length > 0) {
        const textContent = userMessage.content;
        userMessage.content = [
          { type: 'text', text: textContent },
          ...images.map(image => ({
            type: 'image',
            image: image
          }))
        ];
      }

      // Initialize conversation with existing history + new user message
      // If history already contains a system message (from session cloning), reuse it for cache efficiency
      // Otherwise add a fresh system message
      const hasSystemMessage = this.history.length > 0 && this.history[0].role === 'system';
      let currentMessages;

      if (hasSystemMessage) {
        // Reuse existing system message from history for cache efficiency
        currentMessages = [
          ...this.history,
          userMessage
        ];
        if (this.debug) {
          console.log('[DEBUG] Reusing existing system message from history for cache efficiency');
        }
      } else {
        // Add fresh system message (first call or empty history)
        currentMessages = [
          { role: 'system', content: systemMessage },
          ...this.history, // Include previous conversation history
          userMessage
        ];
      }

      let currentIteration = 0;
      let completionAttempted = false;
      let finalResult = 'I was unable to complete your request due to reaching the maximum number of tool iterations.';

      // Adjust max iterations if schema is provided
      // +1 for schema formatting
      // +2 for potential Mermaid validation retries (can be multiple diagrams)
      // +1 for potential JSON correction
      const baseMaxIterations = this.maxIterations || MAX_TOOL_ITERATIONS;
      const maxIterations = options.schema ? baseMaxIterations + 4 : baseMaxIterations;

      // Check if we're using CLI-based engines which handle their own agentic loop
      const isClaudeCode = this.clientApiProvider === 'claude-code' || process.env.USE_CLAUDE_CODE === 'true';
      const isCodex = this.clientApiProvider === 'codex' || process.env.USE_CODEX === 'true';

      if (isClaudeCode) {
        // For Claude Code, bypass the tool loop entirely - it handles its own internal dialogue
        if (this.debug) {
          console.log(`[DEBUG] Using Claude Code engine - bypassing tool loop (black box mode)`);
          console.log(`[DEBUG] Sending question directly to Claude Code: ${message.substring(0, 100)}...`);
        }

        // Send the message directly to Claude Code and collect the response
        try {
          const engine = await this.getEngine();
          if (engine && engine.query) {
            let assistantResponseContent = '';
            let toolBatch = null;

            // Query Claude Code directly with the message and schema
            for await (const chunk of engine.query(message, options)) {
              if (chunk.type === 'text' && chunk.content) {
                assistantResponseContent += chunk.content;
                if (options.onStream) {
                  options.onStream(chunk.content);
                }
              } else if (chunk.type === 'toolBatch' && chunk.tools) {
                // Store tool batch for processing after response
                toolBatch = chunk.tools;
                if (this.debug) {
                  console.log(`[DEBUG] Received batch of ${chunk.tools.length} tool events from Claude Code`);
                }
              } else if (chunk.type === 'error') {
                throw chunk.error;
              }
            }

            // Emit tool events after response is complete (batch mode)
            if (toolBatch && toolBatch.length > 0 && this.events) {
              if (this.debug) {
                console.log(`[DEBUG] Emitting ${toolBatch.length} tool events from Claude Code batch`);
              }
              for (const toolEvent of toolBatch) {
                this.events.emit('toolCall', toolEvent);
              }
            }

            // Update history with the exchange
            this.history.push(userMessage);
            this.history.push({
              role: 'assistant',
              content: assistantResponseContent
            });

            // Store conversation history
            // TODO: storeConversationHistory is not yet implemented for Claude Code
            // await this.storeConversationHistory(this.history, oldHistoryLength);

            // Emit completion hook
            await this.hooks.emit(HOOK_TYPES.COMPLETION, {
              sessionId: this.sessionId,
              prompt: message,
              response: assistantResponseContent
            });

            return assistantResponseContent;
          }
        } catch (error) {
          if (this.debug) {
            console.error('[DEBUG] Claude Code error:', error);
          }
          throw error;
        }
      }

      // Handle Codex engine (same pattern as Claude Code)
      if (isCodex) {
        // For Codex, bypass the tool loop entirely - it handles its own internal dialogue
        if (this.debug) {
          console.log(`[DEBUG] Using Codex engine - bypassing tool loop (black box mode)`);
          console.log(`[DEBUG] Sending question directly to Codex: ${message.substring(0, 100)}...`);
        }

        // Send the message directly to Codex and collect the response
        try {
          const engine = await this.getEngine();
          if (engine && engine.query) {
            let assistantResponseContent = '';
            let toolBatch = null;

            // Query Codex directly with the message and schema
            for await (const chunk of engine.query(message, options)) {
              if (chunk.type === 'text' && chunk.content) {
                assistantResponseContent += chunk.content;
                if (options.onStream) {
                  options.onStream(chunk.content);
                }
              } else if (chunk.type === 'toolBatch' && chunk.tools) {
                // Store tool batch for processing after response
                toolBatch = chunk.tools;
                if (this.debug) {
                  console.log(`[DEBUG] Received batch of ${chunk.tools.length} tool events from Codex`);
                }
              } else if (chunk.type === 'error') {
                throw chunk.error;
              }
            }

            // Emit tool events after response is complete (batch mode)
            if (toolBatch && toolBatch.length > 0 && this.events) {
              if (this.debug) {
                console.log(`[DEBUG] Emitting ${toolBatch.length} tool events from Codex batch`);
              }
              for (const toolEvent of toolBatch) {
                this.events.emit('toolCall', toolEvent);
              }
            }

            // Update history with the exchange
            this.history.push(userMessage);
            this.history.push({
              role: 'assistant',
              content: assistantResponseContent
            });

            // Store conversation history
            // TODO: storeConversationHistory is not yet implemented for Codex
            // await this.storeConversationHistory(this.history, oldHistoryLength);

            // Emit completion hook
            await this.hooks.emit(HOOK_TYPES.COMPLETION, {
              sessionId: this.sessionId,
              prompt: message,
              response: assistantResponseContent
            });

            return assistantResponseContent;
          }
        } catch (error) {
          if (this.debug) {
            console.error('[DEBUG] Codex error:', error);
          }
          throw error;
        }
      }

      if (this.debug) {
        console.log(`[DEBUG] Starting agentic flow for question: ${message.substring(0, 100)}...`);
        if (options.schema) {
          console.log(`[DEBUG] Schema provided, using extended iteration limit: ${maxIterations} (base: ${baseMaxIterations})`);
        }
      }

      // Tool iteration loop (only for non-CLI engines like Vercel/Anthropic/OpenAI)
      while (currentIteration < maxIterations && !completionAttempted) {
        currentIteration++;
        if (this.cancelled) throw new Error('Request was cancelled by the user');

        if (this.debug) {
          console.log(`\n[DEBUG] --- Tool Loop Iteration ${currentIteration}/${maxIterations} ---`);
          console.log(`[DEBUG] Current messages count for AI call: ${currentMessages.length}`);
          
          // Log preview of the latest user message (helpful for debugging loops)
          const lastUserMessage = [...currentMessages].reverse().find(msg => msg.role === 'user');
          if (lastUserMessage && lastUserMessage.content) {
            const userPreview = createMessagePreview(lastUserMessage.content);
            console.log(`[DEBUG] Latest user message (${lastUserMessage.content.length} chars): ${userPreview}`);
          }
        }

        // Add iteration tracing event
        if (this.tracer) {
          this.tracer.addEvent('iteration.start', {
            'iteration': currentIteration,
            'max_iterations': maxIterations,
            'message_count': currentMessages.length
          });
        }

        // Add warning message when reaching the last iteration
        if (currentIteration === maxIterations) {
          const warningMessage = `⚠️ WARNING: You have reached the maximum tool iterations limit (${maxIterations}). This is your final message. Please respond with the data you have so far. If something was not completed, honestly state what was not done and provide any partial results or recommendations you can offer.`;
          
          currentMessages.push({
            role: 'user',
            content: warningMessage
          });
          
          if (this.debug) {
            console.log(`[DEBUG] Added max iterations warning message at iteration ${currentIteration}`);
          }
        }

        // Calculate context size
        this.tokenCounter.calculateContextSize(currentMessages);
        if (this.debug) {
          console.log(`[DEBUG] Estimated context tokens BEFORE LLM call (Iter ${currentIteration}): ${this.tokenCounter.contextSize}`);
        }

        let maxResponseTokens = this.maxResponseTokens;
        if (!maxResponseTokens) {
          // Use model-based defaults if not explicitly configured
          maxResponseTokens = 4000;
          if (this.model.includes('opus') || this.model.includes('sonnet') || this.model.startsWith('gpt-4-')) {
            maxResponseTokens = 8192;
          } else if (this.model.startsWith('gpt-4o')) {
            maxResponseTokens = 8192;
          } else if (this.model.startsWith('gemini')) {
            maxResponseTokens = 32000;
          }
        }

        // Make AI request
        let assistantResponseContent = '';
        let compactionAttempted = false;

        // Retry loop for context compaction - separate from streamTextWithRetryAndFallback
        // which handles transient errors (rate limits, network issues, etc.)
        while (true) {
          try {
            // Wrap AI request with tracing if available
            const executeAIRequest = async () => {
              // Prepare messages with potential image content
              const messagesForAI = this.prepareMessagesWithImages(currentMessages);

              const result = await this.streamTextWithRetryAndFallback({
                model: this.provider ? this.provider(this.model) : this.model,
                messages: messagesForAI,
                maxTokens: maxResponseTokens,
                temperature: 0.3,
              });

              // Get the promise reference BEFORE consuming stream (doesn't lock it)
              const usagePromise = result.usage;

              // Collect the streamed response - stream all content for now
              for await (const delta of result.textStream) {
                assistantResponseContent += delta;
                // For now, stream everything - we'll handle segmentation after tools execute
                if (options.onStream) {
                  options.onStream(delta);
                }
              }

              // Record token usage - await the promise AFTER stream is consumed
              const usage = await usagePromise;
              if (usage) {
                this.tokenCounter.recordUsage(usage, result.experimental_providerMetadata);
              }

              return result;
            };

            if (this.tracer) {
              await this.tracer.withSpan('ai.request', executeAIRequest, {
                'ai.model': this.model,
                'ai.provider': this.clientApiProvider || 'auto',
                'iteration': currentIteration,
                'max_tokens': maxResponseTokens,
                'temperature': 0.3,
                'message_count': currentMessages.length
              });
            } else {
              await executeAIRequest();
            }

            // Success - break out of compaction retry loop
            break;

          } catch (error) {
            // Check if this is a context limit error (only try compaction once per iteration)
            if (!compactionAttempted && handleContextLimitError) {
              const compactionResult = handleContextLimitError(error, currentMessages, {
                keepLastSegment: true,
                minSegmentsToKeep: 1
              });

              if (compactionResult) {
                // Context limit error detected - compact and retry once
                const { messages: compactedMessages, stats } = compactionResult;

                // Check if compaction actually reduced message count
                if (stats.removed === 0) {
                  // No messages removed - compaction won't help, fail immediately
                  console.error(`[ERROR] Context window exceeded but no messages can be compacted.`);
                  console.error(`[ERROR] The conversation history is already minimal (${stats.originalCount} messages).`);
                  finalResult = `Error: Context window limit exceeded and conversation cannot be compacted further. Consider starting a new session or reducing system message size.`;
                  throw new Error(finalResult);
                }

                compactionAttempted = true;

                console.log(`[INFO] Context window limit exceeded. Compacting conversation...`);
                console.log(`[INFO] Removed ${stats.removed} messages (${stats.reductionPercent}% reduction)`);
                console.log(`[INFO] Estimated token savings: ${stats.tokensSaved} tokens`);

                if (this.debug) {
                  console.log(`[DEBUG] Compaction stats:`, stats);
                  console.log(`[DEBUG] Original message count: ${stats.originalCount}`);
                  console.log(`[DEBUG] Compacted message count: ${stats.compactedCount}`);
                }

                // Replace currentMessages with compacted version (creates new array reference)
                // This ensures we don't mutate the original history array
                currentMessages = [...compactedMessages];

                // Log compaction event if tracer is available
                if (this.tracer) {
                  this.tracer.addEvent('context.compacted', {
                    'iteration': currentIteration,
                    'original_count': stats.originalCount,
                    'compacted_count': stats.compactedCount,
                    'reduction_percent': stats.reductionPercent,
                    'tokens_saved': stats.tokensSaved
                  });
                }

                // Continue to retry with compacted messages
                continue;
              }
            }

            // Not a context limit error, compaction already attempted, or compaction not available
            // IMPORTANT: This break prevents infinite loop if compacted messages still exceed limit
            console.error(`Error during streamText (Iter ${currentIteration}):`, error);
            finalResult = `Error: Failed to get response from AI model during iteration ${currentIteration}. ${error.message}`;
            throw new Error(finalResult);
          }
        }

        // Log preview of assistant response for debugging loops
        if (this.debug && assistantResponseContent) {
          const assistantPreview = createMessagePreview(assistantResponseContent);
          console.log(`[DEBUG] Assistant response (${assistantResponseContent.length} chars): ${assistantPreview}`);
        }

        // Images in assistant responses are not automatically processed
        // AI can use the readImage tool to explicitly request reading an image

        // Parse tool call from response with valid tools list
        // Build validTools based on allowedTools configuration (same pattern as getSystemMessage)
        // When _disableTools is set, only allow attempt_completion for JSON correction flows
        const validTools = [];
        if (options._disableTools) {
          // Only allow attempt_completion for JSON correction - no search/query/edit tools
          validTools.push('attempt_completion');
          if (this.debug) {
            console.log(`[DEBUG] Tools disabled for this call - only attempt_completion allowed`);
          }
        } else {
          if (this.allowedTools.isEnabled('search')) validTools.push('search');
          if (this.allowedTools.isEnabled('query')) validTools.push('query');
          if (this.allowedTools.isEnabled('extract')) validTools.push('extract');
          if (this.allowedTools.isEnabled('listFiles')) validTools.push('listFiles');
          if (this.allowedTools.isEnabled('searchFiles')) validTools.push('searchFiles');
          if (this.enableSkills && this.allowedTools.isEnabled('listSkills')) validTools.push('listSkills');
          if (this.enableSkills && this.allowedTools.isEnabled('useSkill')) validTools.push('useSkill');
          if (this.allowedTools.isEnabled('readImage')) validTools.push('readImage');
          // Always allow attempt_completion - it's a completion signal, not a tool
          // This ensures agents can complete even when disableTools: true is set (fixes #333)
          validTools.push('attempt_completion');

          // Edit tools (require both allowEdit flag AND allowedTools permission)
          if (this.allowEdit && this.allowedTools.isEnabled('implement')) {
            validTools.push('implement', 'edit', 'create');
          }
          // Bash tool (require both enableBash flag AND allowedTools permission)
          if (this.enableBash && this.allowedTools.isEnabled('bash')) {
            validTools.push('bash');
          }
          // Delegate tool (require both enableDelegate flag AND allowedTools permission)
          if (this.enableDelegate && this.allowedTools.isEnabled('delegate')) {
            validTools.push('delegate');
          }
        }

        // Try parsing with hybrid parser that supports both native and MCP tools
        // When _disableTools is set, skip MCP tools entirely
        const nativeTools = validTools;
        const parsedTool = (this.mcpBridge && !options._disableTools)
          ? parseHybridXmlToolCall(assistantResponseContent, nativeTools, this.mcpBridge)
          : parseXmlToolCallWithThinking(assistantResponseContent, validTools);
        if (parsedTool) {
          const { toolName, params } = parsedTool;
          if (this.debug) console.log(`[DEBUG] Parsed tool call: ${toolName} with params:`, params);

          if (toolName === 'attempt_completion') {
            completionAttempted = true;

            // END CHECKPOINT: Block completion if there are incomplete tasks
            if (this.enableTasks && this.taskManager && this.taskManager.hasIncompleteTasks()) {
              const taskSummary = this.taskManager.getTaskSummary();
              const blockedMessage = createTaskCompletionBlockedMessage(taskSummary);

              if (this.debug) {
                console.log('[DEBUG] Task checkpoint: Blocking completion due to incomplete tasks');
                console.log('[DEBUG] Incomplete tasks:', taskSummary);
              }

              // Add reminder message and continue the loop
              currentMessages.push({
                role: 'assistant',
                content: assistantResponseContent
              });
              currentMessages.push({
                role: 'user',
                content: blockedMessage
              });

              completionAttempted = false; // Reset to allow more iterations
              continue; // Skip the break and continue the loop
            }

            // Handle attempt_complete shorthand - use previous response
            if (params.result === '__PREVIOUS_RESPONSE__') {
              // Find the last assistant message with actual content (not tool calls)
              const lastAssistantMessage = [...currentMessages].reverse().find(msg =>
                msg.role === 'assistant' &&
                msg.content &&
                !(this.mcpBridge
                  ? parseHybridXmlToolCall(msg.content, validTools, this.mcpBridge)
                  : parseXmlToolCallWithThinking(msg.content, validTools))
              );

              if (lastAssistantMessage) {
                finalResult = lastAssistantMessage.content;
                if (this.debug) console.log(`[DEBUG] Using previous response as completion: ${finalResult.substring(0, 100)}...`);
              } else {
                finalResult = 'Error: No previous response found to use as completion.';
                if (this.debug) console.log(`[DEBUG] No suitable previous response found for attempt_complete shorthand`);
              }
            } else {
              // Standard attempt_completion handling
              const validation = attemptCompletionSchema.safeParse(params);
              if (validation.success) {
                finalResult = validation.data.result;

                // Stream the final result if callback is provided
                if (options.onStream && finalResult) {
                  const chunkSize = 50; // Characters per chunk for smoother streaming
                  for (let i = 0; i < finalResult.length; i += chunkSize) {
                    const chunk = finalResult.slice(i, Math.min(i + chunkSize, finalResult.length));
                    options.onStream(chunk);
                  }
                }

                if (this.debug) console.log(`[DEBUG] Task completed successfully with result: ${finalResult.substring(0, 100)}...`);
              } else {
                console.error(`[ERROR] Invalid attempt_completion parameters:`, validation.error);
                finalResult = 'Error: Invalid completion attempt. The task could not be completed properly.';
              }
            }
            break;
          } else {
            // Check tool type and execute accordingly
            const { type } = parsedTool;

            if (type === 'mcp' && this.mcpBridge && this.mcpBridge.isMcpTool(toolName)) {
              // Execute MCP tool
              try {
                // Log MCP tool execution in debug mode
                if (this.debug) {
                  console.error(`\n[DEBUG] ========================================`);
                  console.error(`[DEBUG] Executing MCP tool: ${toolName}`);
                  console.error(`[DEBUG] Arguments:`);
                  for (const [key, value] of Object.entries(params)) {
                    const displayValue = typeof value === 'string' && value.length > 100
                      ? value.substring(0, 100) + '...'
                      : value;
                    console.error(`[DEBUG]   ${key}: ${JSON.stringify(displayValue)}`);
                  }
                  console.error(`[DEBUG] ========================================\n`);
                }

                // Execute MCP tool through the bridge
                const executionResult = await this.mcpBridge.mcpTools[toolName].execute(params);

                const toolResultContent = typeof executionResult === 'string' ? executionResult : JSON.stringify(executionResult, null, 2);

                // Log MCP tool result in debug mode
                if (this.debug) {
                  const preview = toolResultContent.length > 500 ? toolResultContent.substring(0, 500) + '...' : toolResultContent;
                  console.error(`[DEBUG] ========================================`);
                  console.error(`[DEBUG] MCP tool '${toolName}' completed successfully`);
                  console.error(`[DEBUG] Result preview:`);
                  console.error(preview);
                  console.error(`[DEBUG] ========================================\n`);
                }

                currentMessages.push({ role: 'user', content: `<tool_result>\n${toolResultContent}\n</tool_result>` });
              } catch (error) {
                console.error(`Error executing MCP tool ${toolName}:`, error);
                const toolResultContent = `Error executing MCP tool ${toolName}: ${error.message}`;

                // Log MCP tool error in debug mode
                if (this.debug) {
                  console.error(`[DEBUG] ========================================`);
                  console.error(`[DEBUG] MCP tool '${toolName}' failed with error:`);
                  console.error(`[DEBUG] ${error.message}`);
                  console.error(`[DEBUG] ========================================\n`);
                }

                currentMessages.push({ role: 'user', content: `<tool_result>\n${toolResultContent}\n</tool_result>` });
              }
            } else if (this.toolImplementations[toolName]) {
              // Execute native tool
              try {
                // Add sessionId and workingDirectory to params for tool execution
                // Validate and resolve workingDirectory
                // Priority: explicit cwd > first allowed folder > process.cwd()
                let resolvedWorkingDirectory = this.cwd || (this.allowedFolders && this.allowedFolders[0]) || process.cwd();
                if (params.workingDirectory) {
                  // Resolve relative paths against the current working directory context, not process.cwd()
                  const requestedDir = isAbsolute(params.workingDirectory)
                    ? resolve(params.workingDirectory)
                    : resolve(resolvedWorkingDirectory, params.workingDirectory);
                  // Check if the requested directory is within allowed folders
                  const isWithinAllowed = !this.allowedFolders || this.allowedFolders.length === 0 ||
                    this.allowedFolders.some(folder => {
                      const resolvedFolder = resolve(folder);
                      return requestedDir === resolvedFolder || requestedDir.startsWith(resolvedFolder + sep);
                    });
                  if (isWithinAllowed) {
                    resolvedWorkingDirectory = requestedDir;
                  } else if (this.debug) {
                    console.error(`[DEBUG] Rejected workingDirectory "${params.workingDirectory}" - not within allowed folders`);
                  }
                }
                const toolParams = {
                  ...params,
                  sessionId: this.sessionId,
                  workingDirectory: resolvedWorkingDirectory
                };

                // Log tool execution in debug mode
                if (this.debug) {
                  console.error(`\n[DEBUG] ========================================`);
                  console.error(`[DEBUG] Executing tool: ${toolName}`);
                  console.error(`[DEBUG] Arguments:`);
                  for (const [key, value] of Object.entries(params)) {
                    const displayValue = typeof value === 'string' && value.length > 100
                      ? value.substring(0, 100) + '...'
                      : value;
                    console.error(`[DEBUG]   ${key}: ${JSON.stringify(displayValue)}`);
                  }
                  console.error(`[DEBUG] ========================================\n`);
                }

                // Emit tool start event with stream pause signal
                this.events.emit('toolCall', {
                  timestamp: new Date().toISOString(),
                  name: toolName,
                  args: toolParams,
                  status: 'started',
                  pauseStream: true  // Signal to pause text streaming
                });
                
                // Execute tool with tracing if available
                const executeToolCall = async () => {
                  // For delegate tool, pass current iteration, max iterations, session ID, and config
                  if (toolName === 'delegate') {
                    const enhancedParams = {
                      ...toolParams,
                      currentIteration,
                      maxIterations,
                      parentSessionId: this.sessionId,  // Pass parent session ID for tracking
                      path: this.searchPath,            // Inherit search path
                      provider: this.apiType,           // Inherit AI provider (string identifier)
                      model: this.model,                // Inherit model
                      searchDelegate: this.searchDelegate,
                      debug: this.debug,
                      tracer: this.tracer
                    };

                    if (this.debug) {
                      console.log(`[DEBUG] Executing delegate tool at iteration ${currentIteration}/${maxIterations}`);
                      console.log(`[DEBUG] Parent session: ${this.sessionId}`);
                      console.log(`[DEBUG] Inherited config: path=${this.searchPath}, provider=${this.apiType}, model=${this.model}`);
                      console.log(`[DEBUG] Delegate task: ${toolParams.task?.substring(0, 100)}...`);
                    }
                    
                    // Record delegation start in telemetry
                    if (this.tracer) {
                      this.tracer.recordDelegationEvent('tool_started', {
                        'delegation.iteration': currentIteration,
                        'delegation.max_iterations': maxIterations,
                        'delegation.task_preview': toolParams.task?.substring(0, 200) + (toolParams.task?.length > 200 ? '...' : '')
                      });
                    }
                    
                    return await this.toolImplementations[toolName].execute(enhancedParams);
                  }
                  return await this.toolImplementations[toolName].execute(toolParams);
                };

                let toolResult;
                try {
                  if (this.tracer) {
                    toolResult = await this.tracer.withSpan('tool.call', executeToolCall, {
                      'tool.name': toolName,
                      'tool.params': JSON.stringify(toolParams).substring(0, 500),
                      'iteration': currentIteration
                    });
                  } else {
                    toolResult = await executeToolCall();
                  }
                  
                  // Log tool result in debug mode
                  if (this.debug) {
                    const resultPreview = typeof toolResult === 'string'
                      ? (toolResult.length > 500 ? toolResult.substring(0, 500) + '...' : toolResult)
                      : (toolResult ? JSON.stringify(toolResult, null, 2).substring(0, 500) + '...' : 'No Result');
                    console.error(`[DEBUG] ========================================`);
                    console.error(`[DEBUG] Tool '${toolName}' completed successfully`);
                    console.error(`[DEBUG] Result preview:`);
                    console.error(resultPreview);
                    console.error(`[DEBUG] ========================================\n`);
                  }

                  // Emit tool success event
                  this.events.emit('toolCall', {
                    timestamp: new Date().toISOString(),
                    name: toolName,
                    args: toolParams,
                    resultPreview: typeof toolResult === 'string'
                      ? (toolResult.length > 200 ? toolResult.substring(0, 200) + '...' : toolResult)
                      : (toolResult ? JSON.stringify(toolResult).substring(0, 200) + '...' : 'No Result'),
                    status: 'completed'
                  });

                } catch (toolError) {
                  // Log tool error in debug mode
                  if (this.debug) {
                    console.error(`[DEBUG] ========================================`);
                    console.error(`[DEBUG] Tool '${toolName}' failed with error:`);
                    console.error(`[DEBUG] ${toolError.message}`);
                    console.error(`[DEBUG] ========================================\n`);
                  }

                  // Emit tool error event
                  this.events.emit('toolCall', {
                    timestamp: new Date().toISOString(),
                    name: toolName,
                    args: toolParams,
                    error: toolError.message || 'Unknown error',
                    status: 'error'
                  });
                  throw toolError; // Re-throw to be handled by outer catch
                }
                
                // Add assistant response and tool result to conversation
                currentMessages.push({ role: 'assistant', content: assistantResponseContent });
                
                const toolResultContent = typeof toolResult === 'string' ? toolResult : JSON.stringify(toolResult, null, 2);
                const toolResultMessage = `<tool_result>\n${toolResultContent}\n</tool_result>`;
                
                currentMessages.push({
                  role: 'user',
                  content: toolResultMessage
                });

                // NOTE: Automatic image processing removed (GitHub issue #305)
                // Images are now only loaded when the AI explicitly calls the readImage tool
                // This prevents: 1) implicit behavior that users don't expect
                //                2) crashes with unsupported MIME types (e.g., SVG on Gemini)

                if (this.debug) {
                  console.log(`[DEBUG] Tool ${toolName} executed successfully. Result length: ${typeof toolResult === 'string' ? toolResult.length : JSON.stringify(toolResult).length}`);
                }
              } catch (error) {
                console.error(`[ERROR] Tool execution failed for ${toolName}:`, error);
                currentMessages.push({ role: 'assistant', content: assistantResponseContent });
                currentMessages.push({
                  role: 'user', 
                  content: `<tool_result>\nError: ${error.message}\n</tool_result>`
                });
              }
            } else {
              console.error(`[ERROR] Unknown tool: ${toolName}`);
              currentMessages.push({ role: 'assistant', content: assistantResponseContent });

              // Build list of available tools including MCP tools
              const nativeTools = Object.keys(this.toolImplementations);
              const mcpTools = this.mcpBridge ? this.mcpBridge.getToolNames() : [];
              const allAvailableTools = [...nativeTools, ...mcpTools];

              currentMessages.push({
                role: 'user',
                content: `<tool_result>\nError: Unknown tool '${toolName}'. Available tools: ${allAvailableTools.join(', ')}\n</tool_result>`
              });
            }
          }
        } else {
          // No tool call found
          // Special case: If response contains a mermaid code block and no schema was provided,
          // treat it as a valid completion (for mermaid diagram fixing workflow)
          const hasMermaidCodeBlock = /```mermaid\s*\n[\s\S]*?\n```/.test(assistantResponseContent);
          const hasNoSchemaOrTools = !options.schema && validTools.length === 0;

          if (hasMermaidCodeBlock && hasNoSchemaOrTools) {
            // Accept mermaid code block as final answer for diagram fixing
            finalResult = assistantResponseContent;
            completionAttempted = true;
            if (this.debug) {
              console.error(`[DEBUG] Accepting mermaid code block as valid completion (no schema, no tools)`);
            }
            break;
          }

          // Add assistant response and ask for tool usage
          currentMessages.push({ role: 'assistant', content: assistantResponseContent });

          // Standard reminder - schema was already provided in initial message
          const reminderContent = `Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information to provide a final answer.

Remember: Use proper XML format with BOTH opening and closing tags:

<tool_name>
<parameter>value</parameter>
</tool_name>

Or for quick completion if your previous response was already correct and complete:
<attempt_complete>

IMPORTANT: When using <attempt_complete>, this must be the ONLY content in your response. No additional text, explanations, or other content should be included. This tag signals to reuse your previous response as the final answer.`;

          currentMessages.push({
            role: 'user',
            content: reminderContent
          });
          if (this.debug) {
            console.log(`[DEBUG] No tool call detected in assistant response. Prompting for tool use.`);
          }
        }

        // Keep message history manageable
        if (currentMessages.length > MAX_HISTORY_MESSAGES) {
          const messagesBefore = currentMessages.length;
          const systemMsg = currentMessages[0]; // Keep system message
          const recentMessages = currentMessages.slice(-MAX_HISTORY_MESSAGES + 1);
          currentMessages = [systemMsg, ...recentMessages];
          
          if (this.debug) {
            console.log(`[DEBUG] Trimmed message history from ${messagesBefore} to ${currentMessages.length} messages`);
          }
        }
      }

      if (currentIteration >= maxIterations && !completionAttempted) {
        console.warn(`[WARN] Max tool iterations (${maxIterations}) reached for session ${this.sessionId}. Returning current error state.`);
      }

      // Store final history
      this.history = currentMessages.map(msg => ({ ...msg }));
      if (this.history.length > MAX_HISTORY_MESSAGES) {
        const messagesBefore = this.history.length;
        this.history = this.history.slice(-MAX_HISTORY_MESSAGES);
        if (this.debug) {
          console.log(`[DEBUG] Trimmed stored history from ${messagesBefore} to ${this.history.length} messages`);
        }
      }

      // Update token counter with final history
      this.tokenCounter.updateHistory(this.history);

      // Save new messages to storage (save only the new ones added in this turn)
      try {
        const messagesToSave = currentMessages.slice(oldHistoryLength);
        for (const message of messagesToSave) {
          await this.storageAdapter.saveMessage(this.sessionId, message);
          await this.hooks.emit(HOOK_TYPES.STORAGE_SAVE, {
            sessionId: this.sessionId,
            message
          });
        }
      } catch (error) {
        console.error(`[ERROR] Failed to save messages to storage:`, error);
        // Continue even if storage fails
      }

      // Completion prompt handling - run a follow-up prompt after attempt_completion for validation/review
      // This runs BEFORE mermaid validation and JSON schema validation
      // Skip if we're already in a completion prompt follow-up call or if no completion prompt is configured
      if (completionAttempted && this.completionPrompt && !options._completionPromptProcessed) {
        if (this.debug) {
          console.log('[DEBUG] Running completion prompt for post-completion validation/review...');
        }

        try {
          // Record completion prompt start in telemetry
          if (this.tracer) {
            this.tracer.recordEvent('completion_prompt.started', {
              'completion_prompt.original_result_length': finalResult?.length || 0
            });
          }

          // Create the completion prompt with the current result as context
          const completionPromptMessage = `${this.completionPrompt}

Here is the result to review:
<result>
${finalResult}
</result>

After reviewing, provide your final answer using attempt_completion.`;

          // Make a follow-up call with the completion prompt
          // Pass _completionPromptProcessed to prevent infinite loops
          const completionResult = await this.answer(completionPromptMessage, [], {
            ...options,
            _completionPromptProcessed: true
          });

          // Update finalResult with the result from the completion prompt
          finalResult = completionResult;

          if (this.debug) {
            console.log(`[DEBUG] Completion prompt finished. New result length: ${finalResult?.length || 0}`);
          }

          // Record completion prompt completion in telemetry
          if (this.tracer) {
            this.tracer.recordEvent('completion_prompt.completed', {
              'completion_prompt.final_result_length': finalResult?.length || 0
            });
          }
        } catch (error) {
          console.error('[ERROR] Completion prompt failed:', error);
          // Keep the original result if completion prompt fails
          if (this.tracer) {
            this.tracer.recordEvent('completion_prompt.error', {
              'completion_prompt.error': error.message
            });
          }
        }
      }

      // Schema handling - format response according to provided schema
      // Skip schema processing if result came from attempt_completion tool
      // Don't apply schema formatting if we failed due to max iterations
      const reachedMaxIterations = currentIteration >= maxIterations && !completionAttempted;
      if (options.schema && !options._schemaFormatted && !completionAttempted && !reachedMaxIterations) {
        if (this.debug) {
          console.log('[DEBUG] Schema provided, applying automatic formatting...');
        }
        
        try {
          // Step 1: Make a follow-up call to format according to schema
          const schemaPrompt = `CRITICAL: You MUST respond with ONLY valid JSON DATA that conforms to this schema structure. DO NOT return the schema definition itself.

Schema to follow (this is just the structure - provide ACTUAL DATA):
${options.schema}

REQUIREMENTS:
- Return ONLY the JSON object/array with REAL DATA that matches the schema structure
- DO NOT return the schema definition itself (no "$schema", "$id", "type", "properties", etc.)
- NO additional text, explanations, or markdown formatting
- NO code blocks or backticks
- The JSON must be parseable by JSON.parse()
- Fill in actual values that make sense based on your previous response content

EXAMPLE:
If schema defines {type: "object", properties: {name: {type: "string"}, age: {type: "number"}}}
Return: {"name": "John Doe", "age": 25}
NOT: {"type": "object", "properties": {"name": {"type": "string"}}}

Convert your previous response content into actual JSON data that follows this schema structure.`;
          
          // Call answer recursively with _schemaFormatted flag to prevent infinite loop
          finalResult = await this.answer(schemaPrompt, [], {
            ...options,
            _schemaFormatted: true
          });

          // Step 2: Validate and fix Mermaid diagrams if present (BEFORE cleaning schema)
          // This ensures mermaid validation sees the full response before JSON extraction strips content
          if (!this.disableMermaidValidation) {
            try {
              if (this.debug) {
                console.log(`[DEBUG] Mermaid validation: Starting enhanced mermaid validation...`);
              }
              
              // Record mermaid validation start in telemetry
              if (this.tracer) {
                this.tracer.recordMermaidValidationEvent('schema_processing_started', {
                  'mermaid_validation.context': 'schema_processing',
                  'mermaid_validation.response_length': finalResult.length
                });
              }
              
              const mermaidValidation = await validateAndFixMermaidResponse(finalResult, {
                debug: this.debug,
                path: this.allowedFolders[0],
                provider: this.clientApiProvider,
                model: this.model,
                tracer: this.tracer
              });
              
              if (mermaidValidation.wasFixed) {
                finalResult = mermaidValidation.fixedResponse;
                if (this.debug) {
                  console.log(`[DEBUG] Mermaid validation: Diagrams successfully fixed`);
                  
                  if (mermaidValidation.performanceMetrics) {
                    const metrics = mermaidValidation.performanceMetrics;
                    console.log(`[DEBUG] Mermaid validation: Performance - total: ${metrics.totalTimeMs}ms, AI fixing: ${metrics.aiFixingTimeMs}ms`);
                    console.log(`[DEBUG] Mermaid validation: Results - ${metrics.diagramsFixed}/${metrics.diagramsProcessed} diagrams fixed`);
                  }
                  
                  if (mermaidValidation.fixingResults) {
                    mermaidValidation.fixingResults.forEach((fixResult, index) => {
                      if (fixResult.wasFixed) {
                        const method = fixResult.fixedWithHtmlDecoding ? 'HTML entity decoding' : 'AI correction';
                        const time = fixResult.aiFixingTimeMs ? ` in ${fixResult.aiFixingTimeMs}ms` : '';
                        console.log(`[DEBUG] Mermaid validation: Fixed diagram ${fixResult.diagramIndex + 1} with ${method}${time}`);
                        console.log(`[DEBUG] Mermaid validation: Original error: ${fixResult.originalError}`);
                      } else {
                        console.log(`[DEBUG] Mermaid validation: Failed to fix diagram ${fixResult.diagramIndex + 1}: ${fixResult.fixingError}`);
                      }
                    });
                  }
                }
              } else if (this.debug) {
                console.log(`[DEBUG] Mermaid validation: No fixes needed or fixes unsuccessful`);
                if (mermaidValidation.diagrams?.length > 0) {
                  console.log(`[DEBUG] Mermaid validation: Found ${mermaidValidation.diagrams.length} diagrams, all valid: ${mermaidValidation.isValid}`);
                }
              }
            } catch (error) {
              if (this.debug) {
                console.log(`[DEBUG] Mermaid validation: Process failed with error: ${error.message}`);
                console.log(`[DEBUG] Mermaid validation: Stack trace: ${error.stack}`);
              }
            }
          } else if (this.debug) {
            console.log(`[DEBUG] Mermaid validation: Skipped due to disableMermaidValidation option`);
          }

          // Step 3: Clean the response (remove code blocks, extract JSON)
          // This happens AFTER mermaid validation to preserve full content for validation
          finalResult = cleanSchemaResponse(finalResult);

          // Step 4: Validate and potentially correct JSON responses
          if (isJsonSchema(options.schema)) {
            if (this.debug) {
              console.log(`[DEBUG] JSON validation: Starting validation process for schema response`);
              console.log(`[DEBUG] JSON validation: Cleaned response length: ${finalResult.length} chars`);
            }

            // Record JSON validation start in telemetry
            if (this.tracer) {
              this.tracer.recordJsonValidationEvent('started', {
                'json_validation.response_length': finalResult.length,
                'json_validation.schema_type': 'JSON'
              });
            }

            let validation = validateJsonResponse(finalResult, { debug: this.debug, schema: options.schema });
            let retryCount = 0;
            const maxRetries = 3;

            // First check if the response is valid JSON but is actually a schema definition
            if (validation.isValid && isJsonSchemaDefinition(finalResult, { debug: this.debug })) {
              if (this.debug) {
                console.log(`[DEBUG] JSON validation: Response is a JSON schema definition instead of data, needs correction...`);
              }
              // Mark as invalid so it goes through the fixing process
              validation = {
                isValid: false,
                error: 'Response is a JSON schema definition instead of actual data',
                enhancedError: 'Response is a JSON schema definition instead of actual data. Please return data that conforms to the schema, not the schema itself.'
              };
            }

            // Use separate JsonFixingAgent for JSON corrections (isolates session like Mermaid fixing)
            if (!validation.isValid) {
              if (this.debug) {
                console.log(`[DEBUG] JSON validation: Starting separate JsonFixingAgent session...`);
              }

              const { JsonFixingAgent } = await import('./schemaUtils.js');
              const jsonFixer = new JsonFixingAgent({
                path: this.allowedFolders[0],
                provider: this.clientApiProvider,
                model: this.model,
                debug: this.debug,
                tracer: this.tracer
              });

              let currentResult = finalResult;
              let currentValidation = validation;

              while (!currentValidation.isValid && retryCount < maxRetries) {
                if (this.debug) {
                  console.log(`[DEBUG] JSON validation: Validation failed (attempt ${retryCount + 1}/${maxRetries}):`, currentValidation.error);
                  console.log(`[DEBUG] JSON validation: Invalid response sample: ${currentResult.substring(0, 300)}${currentResult.length > 300 ? '...' : ''}`);
                }

                try {
                  // Use specialized JsonFixingAgent to fix the JSON in a separate session
                  currentResult = await jsonFixer.fixJson(
                    currentResult,
                    options.schema,
                    currentValidation,
                    retryCount + 1
                  );

                  // Validate the corrected response
                  currentValidation = validateJsonResponse(currentResult, { debug: this.debug, schema: options.schema });
                  retryCount++;

                  if (this.debug) {
                    if (!currentValidation.isValid && retryCount < maxRetries) {
                      console.log(`[DEBUG] JSON validation: Still invalid after correction ${retryCount}, retrying...`);
                      console.log(`[DEBUG] JSON validation: Corrected response sample: ${currentResult.substring(0, 300)}${currentResult.length > 300 ? '...' : ''}`);
                    } else if (currentValidation.isValid) {
                      console.log(`[DEBUG] JSON validation: Successfully corrected after ${retryCount} attempts with JsonFixingAgent`);
                    }
                  }
                } catch (error) {
                  if (this.debug) {
                    console.error(`[DEBUG] JSON validation: JsonFixingAgent error on attempt ${retryCount + 1}:`, error.message);
                  }
                  // If JsonFixingAgent fails, break out of loop
                  break;
                }
              }

              // Update finalResult with the fixed version
              finalResult = currentResult;
              validation = currentValidation;

              if (!validation.isValid && this.debug) {
                console.log(`[DEBUG] JSON validation: Still invalid after ${maxRetries} correction attempts with JsonFixingAgent:`, validation.error);
                console.log(`[DEBUG] JSON validation: Final invalid response: ${finalResult.substring(0, 500)}${finalResult.length > 500 ? '...' : ''}`);
              } else if (validation.isValid && this.debug) {
                console.log(`[DEBUG] JSON validation: Final validation successful`);
              }
            }
            
            // Record JSON validation completion in telemetry
            if (this.tracer) {
              this.tracer.recordJsonValidationEvent('completed', {
                'json_validation.success': validation.isValid,
                'json_validation.retry_count': retryCount,
                'json_validation.max_retries': maxRetries,
                'json_validation.final_response_length': finalResult.length,
                'json_validation.error': validation.isValid ? null : validation.error
              });
            }
          }
        } catch (error) {
          console.error('[ERROR] Schema formatting failed:', error);
          // Return the original result if schema formatting fails
        }
      } else if (reachedMaxIterations && options.schema && this.debug) {
        console.log('[DEBUG] Skipping schema formatting due to max iterations reached without completion');
      } else if (completionAttempted && options.schema && !options._schemaFormatted && !options._skipValidation) {
        // For attempt_completion results with schema, validate mermaid diagrams BEFORE cleaning schema
        // This ensures mermaid validation sees the full response before JSON extraction strips content
        // Skip this validation if we're in a recursive correction call (_skipValidation flag)
        try {
          // Validate and fix Mermaid diagrams if present (BEFORE schema cleaning)
          if (!this.disableMermaidValidation) {
            if (this.debug) {
              console.log(`[DEBUG] Mermaid validation: Validating attempt_completion result BEFORE schema cleaning...`);
            }

            const mermaidValidation = await validateAndFixMermaidResponse(finalResult, {
              debug: this.debug,
              path: this.allowedFolders[0],
              provider: this.clientApiProvider,
              model: this.model,
              tracer: this.tracer
            });

            if (mermaidValidation.wasFixed) {
              finalResult = mermaidValidation.fixedResponse;
              if (this.debug) {
                console.log(`[DEBUG] Mermaid validation: attempt_completion diagrams fixed`);
                if (mermaidValidation.performanceMetrics) {
                  console.log(`[DEBUG] Mermaid validation: Fixed in ${mermaidValidation.performanceMetrics.totalTimeMs}ms`);
                }
              }
            } else if (this.debug) {
              console.log(`[DEBUG] Mermaid validation: attempt_completion result validation completed (no fixes needed)`);
            }
          } else if (this.debug) {
            console.log(`[DEBUG] Mermaid validation: Skipped for attempt_completion result due to disableMermaidValidation option`);
          }

          // Now clean the schema response (may extract JSON and discard other content)
          finalResult = cleanSchemaResponse(finalResult);
          
          // Validate and potentially correct JSON for attempt_completion results
          if (isJsonSchema(options.schema)) {
            if (this.debug) {
              console.log(`[DEBUG] JSON validation: Starting validation process for attempt_completion result`);
              console.log(`[DEBUG] JSON validation: Response length: ${finalResult.length} chars`);
            }
            
            // Record JSON validation start in telemetry
            if (this.tracer) {
              this.tracer.recordJsonValidationEvent('attempt_completion_started', {
                'json_validation.response_length': finalResult.length,
                'json_validation.schema_type': 'JSON',
                'json_validation.context': 'attempt_completion'
              });
            }
            
            let validation = validateJsonResponse(finalResult, { debug: this.debug });
            let retryCount = 0;
            const maxRetries = 3;
            
            // First check if the response is valid JSON but is actually a schema definition
            if (validation.isValid && isJsonSchemaDefinition(finalResult, { debug: this.debug })) {
              if (this.debug) {
                console.log(`[DEBUG] JSON validation: attempt_completion response is a JSON schema definition instead of data, correcting...`);
              }
              
              // Use specialized correction prompt for schema definition confusion
              const schemaDefinitionPrompt = createSchemaDefinitionCorrectionPrompt(
                finalResult,
                options.schema,
                0
              );
              
              finalResult = await this.answer(schemaDefinitionPrompt, [], {
                ...options,
                _schemaFormatted: true,
                _skipValidation: true  // Skip validation in recursive correction calls to prevent loops
              });
              finalResult = cleanSchemaResponse(finalResult);
              validation = validateJsonResponse(finalResult);
              retryCount = 1; // Start at 1 since we already did one correction
            }
            
            while (!validation.isValid && retryCount < maxRetries) {
              if (this.debug) {
                console.log(`[DEBUG] JSON validation: attempt_completion validation failed (attempt ${retryCount + 1}/${maxRetries}):`, validation.error);
                console.log(`[DEBUG] JSON validation: Invalid response sample: ${finalResult.substring(0, 300)}${finalResult.length > 300 ? '...' : ''}`);
              }
              
              // Check if the invalid response is actually a schema definition
              let correctionPrompt;
              try {
                if (isJsonSchemaDefinition(finalResult, { debug: this.debug })) {
                  if (this.debug) {
                    console.log(`[DEBUG] JSON validation: attempt_completion response is still a schema definition, using specialized correction`);
                  }
                  correctionPrompt = createSchemaDefinitionCorrectionPrompt(
                    finalResult,
                    options.schema,
                    retryCount
                  );
                } else {
                  correctionPrompt = createJsonCorrectionPrompt(
                    finalResult, 
                    options.schema, 
                    validation.error,
                    retryCount
                  );
                }
              } catch (error) {
                // If we can't parse to check if it's a schema definition, use regular correction
                correctionPrompt = createJsonCorrectionPrompt(
                  finalResult, 
                  options.schema, 
                  validation.error,
                  retryCount
                );
              }
              
              finalResult = await this.answer(correctionPrompt, [], {
                ...options,
                _schemaFormatted: true,
                _skipValidation: true,  // Skip validation in recursive correction calls to prevent loops
                _disableTools: true     // Only allow attempt_completion - prevent AI from using search/query tools
              });
              finalResult = cleanSchemaResponse(finalResult);
              
              // Validate the corrected response
              validation = validateJsonResponse(finalResult, { debug: this.debug });
              retryCount++;
              
              if (this.debug) {
                if (validation.isValid) {
                  console.log(`[DEBUG] JSON validation: attempt_completion correction successful on attempt ${retryCount}`);
                } else {
                  console.log(`[DEBUG] JSON validation: attempt_completion correction failed on attempt ${retryCount}: ${validation.error}`);
                }
              }
            }
            
            // Record final validation result
            if (this.tracer) {
              this.tracer.recordJsonValidationEvent('attempt_completion_completed', {
                'json_validation.success': validation.isValid,
                'json_validation.retry_count': retryCount,
                'json_validation.final_response_length': finalResult.length
              });
            }
            
            if (!validation.isValid && this.debug) {
              console.log(`[DEBUG] JSON validation: attempt_completion result validation failed after ${maxRetries} attempts: ${validation.error}`);
              console.log(`[DEBUG] JSON validation: Final attempt_completion response: ${finalResult.substring(0, 500)}${finalResult.length > 500 ? '...' : ''}`);
            } else if (validation.isValid && this.debug) {
              console.log(`[DEBUG] JSON validation: attempt_completion result validation successful`);
            }
          }
        } catch (error) {
          if (this.debug) {
            console.log(`[DEBUG] attempt_completion result cleanup failed: ${error.message}`);
          }
        }
      }

      // Final mermaid validation for all responses (regardless of schema or attempt_completion)
      if (!this.disableMermaidValidation && !options._schemaFormatted) {
        try {
          if (this.debug) {
            console.log(`[DEBUG] Mermaid validation: Performing final mermaid validation on result...`);
          }
          
          const finalMermaidValidation = await validateAndFixMermaidResponse(finalResult, {
            debug: this.debug,
            path: this.allowedFolders[0],
            provider: this.clientApiProvider,
            model: this.model,
            tracer: this.tracer
          });
          
          if (finalMermaidValidation.wasFixed) {
            finalResult = finalMermaidValidation.fixedResponse;
            if (this.debug) {
              console.log(`[DEBUG] Mermaid validation: Final result diagrams fixed`);
              if (finalMermaidValidation.performanceMetrics) {
                console.log(`[DEBUG] Mermaid validation: Final validation took ${finalMermaidValidation.performanceMetrics.totalTimeMs}ms`);
              }
            }
          } else if (this.debug && finalMermaidValidation.diagrams?.length > 0) {
            console.log(`[DEBUG] Mermaid validation: Final result validation completed (${finalMermaidValidation.diagrams.length} diagrams found, no fixes needed)`);
          }
        } catch (error) {
          if (this.debug) {
            console.log(`[DEBUG] Mermaid validation: Final validation failed with error: ${error.message}`);
          }
          // Don't fail the entire request if final mermaid validation fails
        }
      } else if (this.debug) {
        console.log(`[DEBUG] Mermaid validation: Skipped final validation due to disableMermaidValidation option`);
      }

      // Remove thinking tags from final result before returning to user
      if (!options._schemaFormatted) {
        finalResult = removeThinkingTags(finalResult);
        if (this.debug) {
          console.log(`[DEBUG] Removed thinking tags from final result`);
        }
      }

      return finalResult;

    } catch (error) {
      console.error(`[ERROR] ProbeAgent.answer failed:`, error);
      
      // Clean up tool execution data
      clearToolExecutionData(this.sessionId);
      
      throw error;
    }
  }

  /**
   * Get token usage information
   * @returns {Object} Token usage data
   */
  getTokenUsage() {
    return this.tokenCounter.getTokenUsage();
  }

  /**
   * Clear conversation history and reset counters
   */
  async clearHistory() {
    // Clear in storage
    try {
      await this.storageAdapter.clearHistory(this.sessionId);
    } catch (error) {
      console.error(`[ERROR] Failed to clear history in storage:`, error);
    }

    // Clear in-memory
    this.history = [];
    this.tokenCounter.clear();
    clearToolExecutionData(this.sessionId);

    // Emit hook
    await this.hooks.emit(HOOK_TYPES.STORAGE_CLEAR, {
      sessionId: this.sessionId
    });

    if (this.debug) {
      console.log(`[DEBUG] Cleared conversation history and reset counters for session ${this.sessionId}`);
    }
  }

  /**
   * Manually compact conversation history
   * Removes intermediate monologues from older segments while preserving
   * user messages, final answers, and the most recent segment
   *
   * @param {Object} options - Compaction options
   * @param {boolean} [options.keepLastSegment=true] - Keep the most recent segment intact
   * @param {number} [options.minSegmentsToKeep=1] - Number of recent segments to preserve fully
   * @returns {Object} Compaction statistics
   */
  async compactHistory(options = {}) {
    const { compactMessages, calculateCompactionStats } = await import('./contextCompactor.js');

    if (this.history.length === 0) {
      if (this.debug) {
        console.log(`[DEBUG] No history to compact for session ${this.sessionId}`);
      }
      return {
        originalCount: 0,
        compactedCount: 0,
        removed: 0,
        reductionPercent: 0,
        originalTokens: 0,
        compactedTokens: 0,
        tokensSaved: 0
      };
    }

    // Perform compaction
    const compactedMessages = compactMessages(this.history, options);
    const stats = calculateCompactionStats(this.history, compactedMessages);

    // Update history
    this.history = compactedMessages;

    // Save to storage (clear old history first, then save compacted messages)
    try {
      // Clear existing history to avoid duplicates
      await this.storageAdapter.clearHistory(this.sessionId);

      // Save compacted messages
      // Note: Using sequential saves as storage adapter interface doesn't support batch operations
      // For large histories, consider implementing a batch save method in your custom adapter
      for (const message of compactedMessages) {
        await this.storageAdapter.saveMessage(this.sessionId, message);
      }
    } catch (error) {
      console.error(`[ERROR] Failed to save compacted messages to storage:`, error);
    }

    // Log results
    console.log(`[INFO] Manually compacted conversation history`);
    console.log(`[INFO] Removed ${stats.removed} messages (${stats.reductionPercent}% reduction)`);
    console.log(`[INFO] Estimated token savings: ${stats.tokensSaved} tokens`);

    if (this.debug) {
      console.log(`[DEBUG] Compaction stats:`, stats);
    }

    // Emit hook
    await this.hooks.emit(HOOK_TYPES.STORAGE_SAVE, {
      sessionId: this.sessionId,
      compacted: true,
      stats
    });

    return stats;
  }

  /**
   * Clone this agent's session to create a new agent with shared conversation history
   * @param {Object} options - Clone options
   * @param {string} [options.sessionId] - Session ID for the cloned agent (defaults to new UUID)
   * @param {boolean} [options.stripInternalMessages=true] - Remove internal messages (schema reminders, mermaid fixes, etc.)
   * @param {boolean} [options.keepSystemMessage=true] - Keep the system message in cloned history
   * @param {boolean} [options.deepCopy=true] - Deep copy messages to prevent mutations
   * @param {Object} [options.overrides] - Override any ProbeAgent constructor options
   * @returns {ProbeAgent} New agent instance with cloned history
   */
  clone(options = {}) {
    const {
      sessionId = randomUUID(),
      stripInternalMessages = true,
      keepSystemMessage = true,
      deepCopy = true,
      overrides = {}
    } = options;

    // Clone the history
    let clonedHistory = deepCopy
      ? JSON.parse(JSON.stringify(this.history))
      : [...this.history];

    // Strip internal messages if requested
    if (stripInternalMessages) {
      clonedHistory = this._stripInternalMessages(clonedHistory, keepSystemMessage);
    }

    // Create new agent with same configuration
    // Reconstruct the original allowedTools array from the parsed configuration
    let allowedToolsArray = null;
    if (this.allowedTools.mode === 'whitelist') {
      allowedToolsArray = [...this.allowedTools.allowed];
    } else if (this.allowedTools.mode === 'none') {
      allowedToolsArray = [];
    } else if (this.allowedTools.mode === 'all' && this.allowedTools.exclusions.length > 0) {
      allowedToolsArray = ['*', ...this.allowedTools.exclusions.map(t => '!' + t)];
    }
    // If mode is 'all' with no exclusions, leave as null (default)

    const clonedAgent = new ProbeAgent({
      // Copy current agent's config
      customPrompt: this.customPrompt,
      promptType: this.promptType,
      allowEdit: this.allowEdit,
      enableDelegate: this.enableDelegate,
      architectureFileName: this.architectureFileName,
      path: this.allowedFolders[0], // Use first allowed folder as primary path
      allowedFolders: [...this.allowedFolders],
      cwd: this.cwd, // Preserve explicit working directory
      provider: this.clientApiProvider,
      model: this.clientApiModel,
      debug: this.debug,
      outline: this.outline,
      searchDelegate: this.searchDelegate,
      maxResponseTokens: this.maxResponseTokens,
      maxIterations: this.maxIterations,
      disableMermaidValidation: this.disableMermaidValidation,
      disableJsonValidation: this.disableJsonValidation,
      completionPrompt: this.completionPrompt,
      enableSkills: this.enableSkills,
      skillDirs: this.skillDirs ? [...this.skillDirs] : null,
      allowedTools: allowedToolsArray,
      enableMcp: !!this.mcpBridge,
      mcpConfig: this.mcpConfig,
      enableBash: this.enableBash,
      bashConfig: this.bashConfig,
      storageAdapter: this.storageAdapter,
      // Override with any provided options
      sessionId,
      ...overrides
    });

    // Set the cloned history directly (before initialization to avoid overwriting)
    clonedAgent.history = clonedHistory;

    if (this.debug) {
      console.log(`[DEBUG] Cloned session ${this.sessionId} -> ${sessionId}`);
      console.log(`[DEBUG] Cloned ${clonedHistory.length} messages (stripInternal: ${stripInternalMessages})`);
    }

    return clonedAgent;
  }

  /**
   * Internal method to strip internal/temporary messages from history
   * Strategy: Find the FIRST schema-related message and truncate everything from that point onwards.
   * This ensures that all schema formatting iterations (IMPORTANT, CRITICAL, corrections, etc.) are removed.
   * Keeps: system message, user messages, assistant responses, tool results up to the first schema message
   * @private
   */
  _stripInternalMessages(history, keepSystemMessage = true) {
    // Find the first schema-related message index
    let firstSchemaMessageIndex = -1;

    for (let i = 0; i < history.length; i++) {
      const message = history[i];

      // Skip system messages
      if (message.role === 'system') {
        continue;
      }

      // Check if this is a schema-related message
      if (this._isSchemaMessage(message)) {
        firstSchemaMessageIndex = i;
        if (this.debug) {
          console.log(`[DEBUG] Found first schema message at index ${i}, truncating from here`);
        }
        break;
      }
    }

    // If no schema message found, try to find other internal messages and remove them individually
    if (firstSchemaMessageIndex === -1) {
      return this._stripNonSchemaInternalMessages(history, keepSystemMessage);
    }

    // Truncate at the first schema message, then also filter non-schema internal messages
    // from the remaining history before the schema
    const truncated = history.slice(0, firstSchemaMessageIndex);

    // Now filter non-schema internal messages from the truncated history
    const filtered = this._stripNonSchemaInternalMessages(truncated, keepSystemMessage);

    if (this.debug) {
      const removedCount = history.length - filtered.length;
      console.log(`[DEBUG] Truncated at schema message (index ${firstSchemaMessageIndex}) and filtered non-schema internal messages`);
      console.log(`[DEBUG] Removed ${removedCount} messages total (${history.length} → ${filtered.length})`);
    }

    return filtered;
  }

  /**
   * Strip non-schema internal messages (mermaid fixes, tool reminders, etc.) individually
   * Used when no schema messages are present in history
   * @private
   */
  _stripNonSchemaInternalMessages(history, keepSystemMessage = true) {
    const filtered = [];

    for (let i = 0; i < history.length; i++) {
      const message = history[i];

      // Handle system message
      if (message.role === 'system') {
        if (keepSystemMessage) {
          filtered.push(message);
        } else if (this.debug) {
          console.log(`[DEBUG] Removing system message at index ${i}`);
        }
        continue;
      }

      // Check if this is a non-schema internal message (mermaid, tool reminders)
      if (this._isNonSchemaInternalMessage(message)) {
        if (this.debug) {
          console.log(`[DEBUG] Stripping non-schema internal message at index ${i}: ${message.role}`);
        }
        continue;
      }

      // Keep this message
      filtered.push(message);
    }

    return filtered;
  }

  /**
   * Check if a message is schema-related (IMPORTANT, CRITICAL, etc.)
   * @private
   */
  _isSchemaMessage(message) {
    if (message.role !== 'user') {
      return false;
    }

    if (!message.content) {
      return false;
    }

    let content;
    try {
      content = typeof message.content === 'string'
        ? message.content
        : JSON.stringify(message.content);
    } catch (error) {
      // If content cannot be stringified (e.g., circular reference), skip this message
      if (this.debug) {
        console.log(`[DEBUG] Could not stringify message content in _isSchemaMessage: ${error.message}`);
      }
      return false;
    }

    // Schema reminder messages
    if (content.includes('IMPORTANT: A schema was provided') ||
        content.includes('You MUST respond with data that matches this schema') ||
        content.includes('Your response must conform to this schema:') ||
        content.includes('CRITICAL: You MUST respond with ONLY valid JSON DATA') ||
        content.includes('Schema to follow (this is just the structure')) {
      return true;
    }

    return false;
  }

  /**
   * Check if a message is a non-schema internal message (mermaid, tool reminders, JSON corrections)
   * @private
   */
  _isNonSchemaInternalMessage(message) {
    if (message.role !== 'user') {
      return false;
    }

    if (!message.content) {
      return false;
    }

    let content;
    try {
      content = typeof message.content === 'string'
        ? message.content
        : JSON.stringify(message.content);
    } catch (error) {
      // If content cannot be stringified (e.g., circular reference), skip this message
      if (this.debug) {
        console.log(`[DEBUG] Could not stringify message content in _isNonSchemaInternalMessage: ${error.message}`);
      }
      return false;
    }

    // Tool use reminder messages
    if (content.includes('Please use one of the available tools') &&
        content.includes('or use attempt_completion') &&
        content.includes('Remember: Use proper XML format')) {
      return true;
    }

    // Mermaid fix prompts
    if (content.includes('The mermaid diagram in your response has syntax errors') ||
        content.includes('Please fix the mermaid syntax errors') ||
        content.includes('Here is the corrected version:')) {
      return true;
    }

    // JSON correction prompts
    if (content.includes('Your response does not match the expected JSON schema') ||
        content.includes('Please provide a valid JSON response') ||
        content.includes('Schema validation error:')) {
      return true;
    }

    // Empty attempt_complete reminders
    if (content.includes('When using <attempt_complete>') &&
        content.includes('this must be the ONLY content in your response')) {
      return true;
    }

    return false;
  }


  /**
   * Clean up resources (including MCP connections)
   */
  async cleanup() {
    // Clean up MCP bridge
    if (this.mcpBridge) {
      try {
        await this.mcpBridge.cleanup();
        if (this.debug) {
          console.log('[DEBUG] MCP bridge cleaned up');
        }
      } catch (error) {
        console.error('Error cleaning up MCP bridge:', error);
      }
    }

    // Clear history and other resources
    this.clearHistory();
  }

  /**
   * Cancel the current request
   */
  cancel() {
    this.cancelled = true;
    if (this.debug) {
      console.log(`[DEBUG] Agent cancelled for session ${this.sessionId}`);
    }
  }
}
