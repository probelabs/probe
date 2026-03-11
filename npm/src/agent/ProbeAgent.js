// Core ProbeAgent class adapted from examples/chat/probeChat.js

// Load .env file if present (silent fail if not found)
import dotenv from 'dotenv';
dotenv.config();

// ============================================================================
// Timeout Configuration Constants
// ============================================================================

/**
 * Default activity timeout for engine streams (3 minutes).
 * This is the time allowed between stream chunks before considering the stream stalled.
 * Conservative default to handle extended thinking models that may not stream during thinking.
 */
export const ENGINE_ACTIVITY_TIMEOUT_DEFAULT = 180000;

/**
 * Minimum allowed activity timeout (5 seconds).
 * Prevents unreasonably short timeouts that could cause premature failures.
 */
export const ENGINE_ACTIVITY_TIMEOUT_MIN = 5000;

/**
 * Maximum allowed activity timeout (10 minutes).
 * Prevents excessively long waits for stalled streams.
 */
export const ENGINE_ACTIVITY_TIMEOUT_MAX = 600000;

import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { createAmazonBedrock } from '@ai-sdk/amazon-bedrock';
import { streamText, tool, stepCountIs, jsonSchema, Output } from 'ai';
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
  searchSchema,
  querySchema,
  extractSchema,
  delegateSchema,
  analyzeAllSchema,
  executePlanSchema,
  cleanupExecutePlanSchema,
  bashSchema,
  editSchema,
  createSchema,
  multiEditSchema,
  listFilesSchema,
  searchFilesSchema,
  readImageSchema,
  listSkillsSchema,
  useSkillSchema
} from './tools.js';
import { createMessagePreview, detectStuckResponse } from '../tools/common.js';
import { taskSchema } from './tasks/taskTool.js';
import { FileTracker } from '../tools/fileTracker.js';
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
  validateAndFixMermaidResponse,
  tryAutoWrapForSimpleSchema,
  tryExtractValidJsonPrefix
} from './schemaUtils.js';
import { predefinedPrompts } from './shared/prompts.js';
import {
  MCPXmlBridge,
  loadMCPConfigurationFromPath
} from './mcp/index.js';
import { SkillRegistry } from './skills/registry.js';
import { formatAvailableSkillsXml as formatAvailableSkills } from './skills/formatting.js';
import { createSkillToolInstances } from './skills/tools.js';
import { RetryManager, createRetryManagerFromEnv } from './RetryManager.js';
import { FallbackManager, createFallbackManagerFromEnv, buildFallbackProvidersFromEnv } from './FallbackManager.js';
import { handleContextLimitError, compactMessages, calculateCompactionStats } from './contextCompactor.js';
import { formatErrorForAI, ParameterError } from '../utils/error-types.js';
import { getCommonPrefix, toRelativePath, safeRealpath } from '../utils/path-validation.js';
import { truncateIfNeeded, getMaxOutputTokens } from './outputTruncator.js';
import { DelegationManager } from '../delegate.js';
import { extractRawOutputBlocks } from '../tools/executePlan.js';
import {
  TaskManager,
  createTaskTool,
  taskSystemPrompt,
  createTaskCompletionBlockedMessage
} from './tasks/index.js';
import { z } from 'zod';

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
 * Truncate a string for debug logging, showing first and last portion.
 */
export function debugTruncate(s, limit = 200) {
  if (s.length <= limit) return s;
  const half = Math.floor(limit / 2);
  return s.substring(0, half) + ` ... [${s.length} chars] ... ` + s.substring(s.length - half);
}

/**
 * Log tool results details for debug output.
 */
export function debugLogToolResults(toolResults) {
  if (!toolResults || toolResults.length === 0) return;
  for (const tr of toolResults) {
    const argsStr = tr.args != null ? JSON.stringify(tr.args) : '<no args>';
    const resultStr = tr.result != null ? (typeof tr.result === 'string' ? tr.result : JSON.stringify(tr.result)) : '<no result>';
    console.log(`[DEBUG]   tool: ${tr.toolName} | args: ${debugTruncate(argsStr)} | result: ${debugTruncate(resultStr)}`);
  }
}

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
   * @param {boolean} [options.allowEdit=false] - Allow the use of the 'edit' and 'create' tools
   * @param {boolean} [options.enableDelegate=false] - Enable the delegate tool for task distribution to subagents
   * @param {boolean} [options.enableExecutePlan=false] - Enable the execute_plan DSL orchestration tool
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
   * @param {string} [options.completionPrompt] - Custom prompt to run after completion for validation/review (runs before mermaid/JSON validation)
   * @param {number} [options.maxOutputTokens] - Maximum tokens for tool output before truncation (default: 20000, can also be set via PROBE_MAX_OUTPUT_TOKENS env var)
   * @param {number} [options.requestTimeout] - Timeout in ms for AI requests (default: 120000 or REQUEST_TIMEOUT env var). Used to abort hung requests.
   * @param {number} [options.maxOperationTimeout] - Maximum timeout in ms for the entire operation including all retries and fallbacks (default: 300000 or MAX_OPERATION_TIMEOUT env var). This is the absolute maximum time for streamTextWithRetryAndFallback.
   * @param {string|number} [options.thinkingEffort] - Native thinking/reasoning effort level: 'low', 'medium', 'high', or a number (budget tokens). When set, passes provider-specific thinking options to the LLM via providerOptions.
   */
  constructor(options = {}) {
    // Basic configuration
    this.sessionId = options.sessionId || randomUUID();
    // Support systemPrompt alias (overrides customPrompt when both are provided)
    this.customPrompt = options.systemPrompt || options.customPrompt || null;
    this.promptType = options.promptType || 'code-explorer';
    this.allowEdit = !!options.allowEdit;
    this.hashLines = options.hashLines !== undefined ? !!options.hashLines : this.allowEdit;
    this.enableDelegate = !!options.enableDelegate;
    this.enableExecutePlan = !!options.enableExecutePlan;
    this.debug = options.debug || process.env.DEBUG === '1';
    this.cancelled = false;
    this._abortController = new AbortController();
    this.tracer = options.tracer || null;
    this.outline = !!options.outline;
    this.searchDelegate = options.searchDelegate !== undefined ? !!options.searchDelegate : true;
    this.searchDelegateProvider = options.searchDelegateProvider || null;
    this.searchDelegateModel = options.searchDelegateModel || null;
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
    // Skills are disabled by default; enable via allowSkills or enableSkills
    this.enableSkills = options.disableSkills ? false : !!(options.allowSkills || options.enableSkills);
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

    // Native thinking/reasoning effort for LLM providers
    // Accepted values: 'off' (default), 'low', 'medium', 'high', or a number (budget tokens)
    this.thinkingEffort = options.thinkingEffort || null;

    // Tool filtering configuration
    // Parse allowedTools option: ['*'] = all tools, [] or null = no tools, ['tool1', 'tool2'] = specific tools
    // Supports exclusion with '!' prefix: ['*', '!bash'] = all tools except bash
    // disableTools is a convenience flag that overrides allowedTools to []
    const effectiveAllowedTools = options.disableTools ? [] : options.allowedTools;
    this._rawAllowedTools = options.allowedTools; // Keep raw value for explicit tool checks
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

    // Compute workspace root as common prefix of all allowed folders
    // This provides a single "root" for relative path resolution and default cwd
    // IMPORTANT: workspaceRoot is NOT a security boundary - all security checks
    // must be performed against this.allowedFolders, not workspaceRoot
    this.workspaceRoot = getCommonPrefix(this.allowedFolders);

    // Working directory for resolving relative paths
    // If not explicitly provided, use workspace root for consistency
    this.cwd = options.cwd || this.workspaceRoot;

    // API configuration
    this.clientApiProvider = options.provider || null;
    this.clientApiModel = options.model || null;
    this.clientApiKey = null; // Will be set from environment
    this.clientApiUrl = null;

    // Initialize token counter
    this.tokenCounter = new TokenCounter();

    // Maximum output tokens for tool results (truncate if exceeded)
    this.maxOutputTokens = getMaxOutputTokens(options.maxOutputTokens);

    if (this.debug) {
      console.log(`[DEBUG] Generated session ID for agent: ${this.sessionId}`);
      console.log(`[DEBUG] Maximum tool iterations configured: ${MAX_TOOL_ITERATIONS}`);
      console.log(`[DEBUG] Allow Edit: ${this.allowEdit}`);
      console.log(`[DEBUG] Hash Lines: ${this.hashLines}`);
      console.log(`[DEBUG] Search delegation enabled: ${this.searchDelegate}`);
      console.log(`[DEBUG] Workspace root: ${this.workspaceRoot}`);
      console.log(`[DEBUG] Working directory (cwd): ${this.cwd}`);
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

    // Per-instance delegation manager for concurrent delegation limits
    // Each ProbeAgent instance has its own limits, not shared globally
    this.delegationManager = new DelegationManager();

    // Optional global concurrency limiter shared across all ProbeAgent instances.
    // When set, every AI API call acquires a slot before calling the provider.
    this.concurrencyLimiter = options.concurrencyLimiter || null;

    // Request timeout configuration (default 2 minutes)
    // Validates env var to prevent NaN or unreasonable values
    this.requestTimeout = options.requestTimeout ?? (() => {
      if (process.env.REQUEST_TIMEOUT) {
        const parsed = parseInt(process.env.REQUEST_TIMEOUT, 10);
        // Validate: must be positive number between 1s and 1 hour
        if (isNaN(parsed) || parsed < 1000 || parsed > 3600000) {
          return 120000; // Default 2 minutes
        }
        return parsed;
      }
      return 120000;
    })();
    if (this.debug) {
      console.log(`[DEBUG] Request timeout: ${this.requestTimeout}ms`);
    }

    // Maximum operation timeout for entire streamTextWithRetryAndFallback operation (default 5 minutes)
    // This is the absolute maximum time including all retries and fallbacks
    // Validates env var to prevent NaN or unreasonable values
    this.maxOperationTimeout = options.maxOperationTimeout ?? (() => {
      if (process.env.MAX_OPERATION_TIMEOUT) {
        const parsed = parseInt(process.env.MAX_OPERATION_TIMEOUT, 10);
        // Validate: must be positive number between 1s and 2 hours
        if (isNaN(parsed) || parsed < 1000 || parsed > 7200000) {
          return 300000; // Default 5 minutes
        }
        return parsed;
      }
      return 300000;
    })();
    if (this.debug) {
      console.log(`[DEBUG] Max operation timeout: ${this.maxOperationTimeout}ms`);
    }

    // Timeout behavior: 'graceful' (default) winds down with bonus steps, 'hard' aborts immediately
    this.timeoutBehavior = options.timeoutBehavior ?? (() => {
      const val = process.env.TIMEOUT_BEHAVIOR;
      if (val === 'hard') return 'hard';
      return 'graceful';
    })();

    // Number of bonus steps during graceful timeout wind-down (default 4)
    this.gracefulTimeoutBonusSteps = options.gracefulTimeoutBonusSteps ?? (() => {
      const parsed = parseInt(process.env.GRACEFUL_TIMEOUT_BONUS_STEPS, 10);
      return (isNaN(parsed) || parsed < 1 || parsed > 20) ? 4 : parsed;
    })();

    if (this.debug) {
      console.log(`[DEBUG] Timeout behavior: ${this.timeoutBehavior}, bonus steps: ${this.gracefulTimeoutBonusSteps}`);
    }

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

    // Gemini built-in tools (provider-defined, server-side)
    // These are enabled automatically when the provider is Google
    this._geminiToolsEnabled = this._initializeGeminiBuiltinTools();

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
   * Check if query tool was explicitly listed in allowedTools (not via wildcard).
   * Query (ast-grep) is excluded by default because models struggle with AST pattern syntax.
   * @returns {boolean}
   * @private
   */
  _isQueryExplicitlyAllowed() {
    if (!this._rawAllowedTools) return false;
    return Array.isArray(this._rawAllowedTools) && this._rawAllowedTools.includes('query');
  }

  /**
   * Check if tracer is AppTracer (expects sessionId as first param) vs SimpleAppTracer
   * @returns {boolean} - True if tracer is AppTracer style (requires sessionId)
   * @private
   */
  _isAppTracerStyle() {
    // AppTracer has recordThinkingContent(sessionId, iteration, content) signature
    // SimpleAppTracer has recordThinkingContent(content, metadata) signature
    // We detect by checking if there's a sessionSpans map (AppTracer-specific)
    return this.tracer && typeof this.tracer.sessionSpans !== 'undefined';
  }

  /**
   * Record an error classification event for telemetry
   * Provides unified error recording across all error types
   * @param {string} errorType - Error type (wrapped_tool, unrecognized_tool, no_tool_call, circuit_breaker)
   * @param {string} message - Error message
   * @param {Object} context - Additional context data
   * @param {number} iteration - Current iteration number
   * @private
   */
  _recordErrorTelemetry(errorType, message, context, iteration) {
    if (!this.tracer) return;

    if (this._isAppTracerStyle() && typeof this.tracer.recordErrorClassification === 'function') {
      // AppTracer style: (sessionId, iteration, errorType, details)
      this.tracer.recordErrorClassification(this.sessionId, iteration, errorType, {
        message,
        context
      });
    } else if (typeof this.tracer.recordErrorEvent === 'function') {
      // SimpleAppTracer style: (errorType, details)
      this.tracer.recordErrorEvent(errorType, {
        message,
        context: { ...context, iteration }
      });
    } else {
      this.tracer.addEvent(`error.${errorType}`, {
        'error.type': errorType,
        'error.message': message,
        'error.recoverable': errorType !== 'circuit_breaker',
        'error.context': JSON.stringify(context).substring(0, 1000),
        'iteration': iteration
      });
    }
  }

  /**
   * Record AI tool decision for telemetry
   * @param {string} toolName - The tool name
   * @param {Object} params - Tool parameters
   * @param {number} responseLength - Length of AI response
   * @param {number} iteration - Current iteration number
   * @private
   */
  _recordToolDecisionTelemetry(toolName, params, responseLength, iteration) {
    if (!this.tracer) return;

    if (this._isAppTracerStyle() && typeof this.tracer.recordAIToolDecision === 'function') {
      // AppTracer style: (sessionId, iteration, toolName, params)
      this.tracer.recordAIToolDecision(this.sessionId, iteration, toolName, params);
    } else if (typeof this.tracer.recordToolDecision === 'function') {
      // SimpleAppTracer style: (toolName, params, metadata)
      this.tracer.recordToolDecision(toolName, params, {
        iteration,
        'ai.tool_decision.raw_response_length': responseLength
      });
    } else {
      this.tracer.addEvent('ai.tool_decision', {
        'ai.tool_decision.name': toolName,
        'ai.tool_decision.params': JSON.stringify(params || {}).substring(0, 2000),
        'ai.tool_decision.raw_response_length': responseLength,
        'iteration': iteration
      });
    }
  }

  /**
   * Record tool result for telemetry
   * @param {string} toolName - The tool name
   * @param {string|Object} result - Tool result
   * @param {boolean} success - Whether tool succeeded
   * @param {number} durationMs - Execution duration in milliseconds
   * @param {number} iteration - Current iteration number
   * @private
   */
  _recordToolResultTelemetry(toolName, result, success, durationMs, iteration) {
    if (!this.tracer) return;

    if (this._isAppTracerStyle() && typeof this.tracer.recordToolResult === 'function') {
      // AppTracer style: (sessionId, iteration, toolName, result, success, durationMs)
      this.tracer.recordToolResult(this.sessionId, iteration, toolName, result, success, durationMs);
    } else if (typeof this.tracer.recordToolResult === 'function') {
      // SimpleAppTracer style: (toolName, result, success, durationMs, metadata)
      this.tracer.recordToolResult(toolName, result, success, durationMs, { iteration });
    } else {
      const resultStr = typeof result === 'string' ? result : JSON.stringify(result || '');
      this.tracer.addEvent('tool.result', {
        'tool.name': toolName,
        'tool.result': resultStr.substring(0, 10000),
        'tool.result.length': resultStr.length,
        'tool.duration_ms': durationMs,
        'tool.success': success,
        'iteration': iteration
      });
    }
  }

  /**
   * Record MCP tool lifecycle event for telemetry
   * @param {string} phase - 'start' or 'end'
   * @param {string} toolName - MCP tool name
   * @param {Object} params - Tool parameters (for start) or null (for end)
   * @param {number} iteration - Current iteration number
   * @param {Object} [endData] - Additional data for end phase (result, success, durationMs, error)
   * @private
   */
  _recordMcpToolTelemetry(phase, toolName, params, iteration, endData = null) {
    if (!this.tracer) return;

    if (phase === 'start') {
      if (this._isAppTracerStyle() && typeof this.tracer.recordMcpToolStart === 'function') {
        // AppTracer style: (sessionId, iteration, toolName, serverName, params)
        this.tracer.recordMcpToolStart(this.sessionId, iteration, toolName, 'mcp', params);
      } else if (typeof this.tracer.recordMcpToolStart === 'function') {
        // SimpleAppTracer style: (toolName, serverName, params, metadata)
        this.tracer.recordMcpToolStart(toolName, 'mcp', params, { iteration });
      } else {
        this.tracer.addEvent('mcp.tool.start', {
          'mcp.tool.name': toolName,
          'mcp.tool.server': 'mcp',
          'mcp.tool.params': JSON.stringify(params || {}).substring(0, 2000),
          'iteration': iteration
        });
      }
    } else if (phase === 'end' && endData) {
      const { result, success, durationMs, error } = endData;
      if (this._isAppTracerStyle() && typeof this.tracer.recordMcpToolEnd === 'function') {
        // AppTracer style: (sessionId, iteration, toolName, serverName, result, success, durationMs, error)
        this.tracer.recordMcpToolEnd(this.sessionId, iteration, toolName, 'mcp', result, success, durationMs, error);
      } else if (typeof this.tracer.recordMcpToolEnd === 'function') {
        // SimpleAppTracer style: (toolName, serverName, result, success, durationMs, error, metadata)
        this.tracer.recordMcpToolEnd(toolName, 'mcp', result, success, durationMs, error, { iteration });
      } else {
        const resultStr = typeof result === 'string' ? result : JSON.stringify(result || '');
        this.tracer.addEvent('mcp.tool.end', {
          'mcp.tool.name': toolName,
          'mcp.tool.server': 'mcp',
          'mcp.tool.result': resultStr.substring(0, 10000),
          'mcp.tool.result.length': resultStr.length,
          'mcp.tool.duration_ms': durationMs,
          'mcp.tool.success': success,
          'mcp.tool.error': error,
          'iteration': iteration
        });
      }
    }
  }

  /**
   * Record iteration lifecycle event for telemetry
   * @param {string} phase - 'end' (start is already handled elsewhere)
   * @param {number} iteration - Current iteration number
   * @param {Object} data - Additional iteration data
   * @private
   */
  _recordIterationTelemetry(phase, iteration, data = {}) {
    if (!this.tracer) return;

    if (typeof this.tracer.recordIterationEvent === 'function') {
      this.tracer.recordIterationEvent(phase, iteration, data);
    } else {
      this.tracer.addEvent(`iteration.${phase}`, {
        'iteration': iteration,
        ...data
      });
    }
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
          this.model = this.clientApiModel || 'claude-sonnet-4-6';
          this.apiType = 'claude-code';
        } else if (codexAvailable) {
          if (this.debug) {
            console.log('[DEBUG] No API keys found, but codex command detected');
            console.log('[DEBUG] Auto-switching to codex provider');
          }
          // Set provider to codex
          this.clientApiProvider = 'codex';
          this.provider = null;
          this.model = this.clientApiModel || 'gpt-5.2';
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

    // Output buffer for DSL output() function — shared mutable object,
    // reset at the start of each answer() call
    this._outputBuffer = { items: [] };

    // Separate accumulator for extracted RAW_OUTPUT blocks from tool results.
    // This is distinct from _outputBuffer to prevent the cycle where:
    // formatSuccess wraps → extract re-adds → next execute_plan re-wraps (issue #438)
    this._extractedRawBlocks = [];

    const configOptions = {
      sessionId: this.sessionId,
      debug: this.debug,
      // Use cwd (which defaults to workspaceRoot in constructor)
      cwd: this.cwd,
      workspaceRoot: this.workspaceRoot,
      allowedFolders: this.allowedFolders,
      // File state tracking for safe multi-edit workflows (only when editing is enabled)
      fileTracker: this.allowEdit ? new FileTracker({ debug: this.debug }) : null,
      outline: this.outline,
      searchDelegate: this.searchDelegate,
      allowEdit: this.allowEdit,
      hashLines: this.hashLines,
      enableDelegate: this.enableDelegate,
      enableExecutePlan: this.enableExecutePlan,
      enableBash: this.enableBash,
      bashConfig: this.bashConfig,
      tracer: this.tracer,
      allowedTools: this.allowedTools,
      architectureFileName: this.architectureFileName,
      provider: this.clientApiProvider,
      model: this.clientApiModel,
      searchDelegateProvider: this.searchDelegateProvider,
      searchDelegateModel: this.searchDelegateModel,
      delegationManager: this.delegationManager,  // Per-instance delegation limits
      parentAbortSignal: this._abortController.signal,  // Propagate cancellation to delegations
      outputBuffer: this._outputBuffer,
      concurrencyLimiter: this.concurrencyLimiter,  // Global AI concurrency limiter
      isToolAllowed,
      // Lazy MCP getters — MCP is initialized after tools are created, so we use
      // getter functions that resolve at call-time to get the current MCP state
      getMcpBridge: () => this.mcpBridge,
      getMcpTools: () => this.mcpBridge?.mcpTools || {},
      isMcpToolAllowed: (toolName) => this._isMcpToolAllowed(toolName),
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
    // query tool (ast-grep) is not exposed to AI by default — models struggle with AST pattern syntax.
    // Only register it when explicitly listed in allowedTools (not via wildcard '*').
    if (wrappedTools.queryToolInstance && isToolAllowed('query') && this._isQueryExplicitlyAllowed()) {
      this.toolImplementations.query = wrappedTools.queryToolInstance;
    }
    if (wrappedTools.extractToolInstance && isToolAllowed('extract')) {
      this.toolImplementations.extract = wrappedTools.extractToolInstance;
    }
    if (this.enableDelegate && wrappedTools.delegateToolInstance && isToolAllowed('delegate')) {
      this.toolImplementations.delegate = wrappedTools.delegateToolInstance;
    }
    if (this.enableExecutePlan && wrappedTools.executePlanToolInstance && isToolAllowed('execute_plan')) {
      this.toolImplementations.execute_plan = wrappedTools.executePlanToolInstance;
      // cleanup_execute_plan is enabled together with execute_plan
      if (wrappedTools.cleanupExecutePlanToolInstance && isToolAllowed('cleanup_execute_plan')) {
        this.toolImplementations.cleanup_execute_plan = wrappedTools.cleanupExecutePlanToolInstance;
      }
    } else if (wrappedTools.analyzeAllToolInstance && isToolAllowed('analyze_all')) {
      // analyze_all is fallback when execute_plan is not enabled
      this.toolImplementations.analyze_all = wrappedTools.analyzeAllToolInstance;
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
      if (wrappedTools.multiEditToolInstance && isToolAllowed('multi_edit')) {
        this.toolImplementations.multi_edit = wrappedTools.multiEditToolInstance;
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
      this.model = modelName || 'claude-sonnet-4-6';
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
   * Create a streamText-compatible result from an engine stream with timeout handling
   * @param {AsyncGenerator} engineStream - The engine's query result
   * @param {AbortSignal} abortSignal - Signal for aborting the operation
   * @param {number} requestTimeout - Per-request timeout in ms
   * @param {Object} timeoutState - Object with timeoutId property (mutable for cleanup)
   * @returns {Object} - streamText-compatible result with textStream
   * @private
   */
  _createEngineTextStreamResult(engineStream, abortSignal, requestTimeout, timeoutState) {
    // Activity timeout for engine stream - validates env var against defined bounds
    const activityTimeout = (() => {
      const parsed = parseInt(process.env.ENGINE_ACTIVITY_TIMEOUT, 10);
      return isNaN(parsed) || parsed < ENGINE_ACTIVITY_TIMEOUT_MIN || parsed > ENGINE_ACTIVITY_TIMEOUT_MAX
        ? ENGINE_ACTIVITY_TIMEOUT_DEFAULT
        : parsed;
    })();
    const startTime = Date.now();

    // Create a text stream that extracts text from engine messages with timeout
    // The generator clears the operation timeout when done to handle the case
    // where the stream is returned immediately but consumed later
    async function* createTextStream() {
      let lastActivity = Date.now();

      try {
        for await (const message of engineStream) {
          // Check for abort signal
          if (abortSignal.aborted) {
            const abortError = new Error('Operation aborted');
            abortError.name = 'AbortError';
            throw abortError;
          }

          const now = Date.now();

          // Check for activity timeout (no data received for too long)
          if (now - lastActivity > activityTimeout) {
            throw new Error(`Engine stream timeout - no activity for ${activityTimeout}ms`);
          }

          // Check for overall request timeout
          if (requestTimeout > 0 && now - startTime > requestTimeout) {
            throw new Error(`Engine stream timeout - request exceeded ${requestTimeout}ms`);
          }

          lastActivity = now;

          if (message.type === 'text' && message.content) {
            yield message.content;
          } else if (typeof message === 'string') {
            // If engine returns plain strings, pass them through
            yield message;
          }
          // Ignore other message types for the text stream
        }
      } finally {
        // Clear operation timeout when stream completes (success or error)
        // This is done here because for engine paths, the stream is returned
        // immediately but consumed later by the caller
        if (timeoutState.timeoutId) {
          clearTimeout(timeoutState.timeoutId);
          timeoutState.timeoutId = null;
        }
      }
    }

    // Wrap the engine result to match streamText interface
    // Note: maxOperationTimeout cleanup is handled by the generator's finally block
    // since the stream is consumed after this function returns.
    return {
      textStream: createTextStream(),
      usage: Promise.resolve({}), // Engine should handle its own usage tracking
      // Add other streamText-compatible properties as needed
    };
  }

  /**
   * Try to use an engine (claude-code or codex) for streaming
   * @param {Object} options - streamText options
   * @param {AbortController} controller - Abort controller for the operation
   * @param {Object} timeoutState - Mutable timeout state for cleanup
   * @returns {Promise<Object|null>} - Stream result or null if engine unavailable
   * @private
   */
  async _tryEngineStreamPath(options, controller, timeoutState) {
    const engine = await this.getEngine();
    if (!engine || !engine.query) {
      return null;
    }

    // Extract the ORIGINAL user message as the main prompt (skip any warning messages)
    const userMessages = options.messages.filter(m =>
      m.role === 'user' &&
      !m.content.includes('WARNING: You have reached the maximum tool iterations limit')
    );
    const lastUserMessage = userMessages[userMessages.length - 1];
    const prompt = lastUserMessage ? lastUserMessage.content : '';

    // Pass system message and other options including abort signal
    const engineOptions = {
      maxTokens: options.maxTokens,
      temperature: options.temperature,
      messages: options.messages,
      systemPrompt: options.messages.find(m => m.role === 'system')?.content,
      abortSignal: controller.signal
    };

    // Get the engine's query result and wrap with timeout handling
    const engineStream = engine.query(prompt, engineOptions);
    return this._createEngineTextStreamResult(
      engineStream, controller.signal, this.requestTimeout, timeoutState
    );
  }

  /**
   * Execute streamText with Vercel AI SDK using retry/fallback logic
   * @param {Object} options - streamText options
   * @param {AbortController} controller - Abort controller for the operation
   * @returns {Promise<Object>} - Stream result
   * @private
   */
  async _executeWithVercelProvider(options, controller) {
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
        () => streamText({ ...options, abortSignal: controller.signal }),
        {
          provider: this.apiType,
          model: this.model,
          signal: controller.signal
        }
      );
    }

    // Use fallback manager with retry for each provider
    return await this.fallbackManager.executeWithFallback(
      async (provider, model, config) => {
        // Wrap fallback model with per-call concurrency limiter if configured.
        // The original options.model was wrapped in streamTextWithRetryAndFallback,
        // but fallback replaces it with a new model that needs wrapping too.
        let fallbackModel = provider(model);
        if (this.concurrencyLimiter) {
          fallbackModel = ProbeAgent._wrapModelWithLimiter(fallbackModel, this.concurrencyLimiter, this.debug);
        }
        const fallbackOptions = {
          ...options,
          model: fallbackModel,
          abortSignal: controller.signal
        };

        // Strip Gemini provider-defined tools when falling back to non-Google provider
        // These tools have no execute function and would cause errors on other providers
        if (config.provider !== 'google' && fallbackOptions.tools) {
          delete fallbackOptions.tools;
          if (this.debug) {
            console.error(`[DEBUG] Stripped Gemini built-in tools for fallback to ${config.provider} provider`);
          }
        }

        const providerRetryManager = new RetryManager({
          maxRetries: config.maxRetries ?? this.retryConfig.maxRetries ?? 3,
          initialDelay: this.retryConfig.initialDelay ?? 1000,
          maxDelay: this.retryConfig.maxDelay ?? 30000,
          backoffFactor: this.retryConfig.backoffFactor ?? 2,
          retryableErrors: this.retryConfig.retryableErrors,
          debug: this.debug
        });

        return await providerRetryManager.executeWithRetry(
          () => streamText(fallbackOptions),
          {
            provider: config.provider,
            model: model,
            signal: controller.signal
          }
        );
      }
    );
  }

  /**
   * Wrap a LanguageModelV1 model so each doStream/doGenerate call acquires and
   * releases a concurrency limiter slot. This gates individual LLM API calls
   * (seconds each) instead of entire multi-step agent sessions (minutes).
   *
   * @param {Object} model - LanguageModelV1 model instance
   * @param {Object} limiter - Concurrency limiter with acquire/release/getStats
   * @param {boolean} debug - Enable debug logging
   * @returns {Object} Wrapped model with per-call concurrency gating
   * @private
   */
  static _wrapModelWithLimiter(model, limiter, debug) {
    return new Proxy(model, {
      get(target, prop) {
        if (prop === 'doStream') {
          return async function (...args) {
            await limiter.acquire(null);
            if (debug) {
              const stats = limiter.getStats();
              console.log(`[DEBUG] Acquired AI slot for LLM call (${stats.globalActive}/${stats.maxConcurrent}, queue: ${stats.queueSize})`);
            }
            try {
              const result = await target.doStream(...args);

              // Wrap the ReadableStream to release the slot when it completes,
              // errors, or is cancelled — covering all stream termination paths.
              // Guard against double-release: if cancel() races with an in-flight
              // pull() that is awaiting originalReader.read(), both paths could
              // try to release. The flag ensures exactly one release.
              const originalStream = result.stream;
              const originalReader = originalStream.getReader();
              let released = false;
              const releaseOnce = () => {
                if (released) return;
                released = true;
                limiter.release(null);
              };
              const wrappedStream = new ReadableStream({
                async pull(controller) {
                  try {
                    const { done, value } = await originalReader.read();
                    if (done) {
                      controller.close();
                      releaseOnce();
                      if (debug) {
                        const stats = limiter.getStats();
                        console.log(`[DEBUG] Released AI slot after LLM stream complete (${stats.globalActive}/${stats.maxConcurrent})`);
                      }
                    } else {
                      controller.enqueue(value);
                    }
                  } catch (err) {
                    releaseOnce();
                    if (debug) {
                      console.log(`[DEBUG] Released AI slot on LLM stream error`);
                    }
                    controller.error(err);
                  }
                },
                cancel() {
                  releaseOnce();
                  if (debug) {
                    console.log(`[DEBUG] Released AI slot on LLM stream cancel`);
                  }
                  originalReader.cancel();
                }
              });

              return { ...result, stream: wrappedStream };
            } catch (err) {
              limiter.release(null);
              if (debug) {
                console.log(`[DEBUG] Released AI slot on doStream error`);
              }
              throw err;
            }
          };
        }

        if (prop === 'doGenerate') {
          return async function (...args) {
            await limiter.acquire(null);
            if (debug) {
              const stats = limiter.getStats();
              console.log(`[DEBUG] Acquired AI slot for LLM generate (${stats.globalActive}/${stats.maxConcurrent})`);
            }
            try {
              const result = await target.doGenerate(...args);
              return result;
            } finally {
              limiter.release(null);
              if (debug) {
                const stats = limiter.getStats();
                console.log(`[DEBUG] Released AI slot after LLM generate (${stats.globalActive}/${stats.maxConcurrent})`);
              }
            }
          };
        }

        const value = target[prop];
        return typeof value === 'function' ? value.bind(target) : value;
      }
    });
  }

  /**
   * Wrap an engine stream result so its textStream async generator acquires
   * and releases a concurrency limiter slot. Acquire happens when iteration
   * begins; release happens in finally (completion, error, or break).
   *
   * @param {Object} result - Engine result with { textStream, usage, ... }
   * @param {Object} limiter - Concurrency limiter with acquire/release/getStats
   * @param {boolean} debug - Enable debug logging
   * @returns {Object} Result with wrapped textStream
   * @private
   */
  static _wrapEngineStreamWithLimiter(result, limiter, debug) {
    const originalStream = result.textStream;
    async function* gatedStream() {
      await limiter.acquire(null);
      if (debug) {
        const stats = limiter.getStats();
        console.log(`[DEBUG] Acquired AI slot for engine stream (${stats.globalActive}/${stats.maxConcurrent}, queue: ${stats.queueSize})`);
      }
      try {
        yield* originalStream;
      } finally {
        limiter.release(null);
        if (debug) {
          const stats = limiter.getStats();
          console.log(`[DEBUG] Released AI slot after engine stream (${stats.globalActive}/${stats.maxConcurrent})`);
        }
      }
    }
    return { ...result, textStream: gatedStream() };
  }

  /**
   * Execute streamText with retry and fallback support
   * @param {Object} options - streamText options
   * @returns {Promise<Object>} - streamText result
   * @private
   */
  async streamTextWithRetryAndFallback(options) {
    // Wrap the model with per-call concurrency gating if limiter is configured.
    // This acquires/releases the slot around each individual LLM API call (doStream/doGenerate)
    // instead of holding it for the entire multi-step agent session.
    const limiter = this.concurrencyLimiter;
    if (limiter && options.model) {
      options = { ...options, model: ProbeAgent._wrapModelWithLimiter(options.model, limiter, this.debug) };
    }

    // Create AbortController for overall operation timeout
    const controller = new AbortController();
    const timeoutState = { timeoutId: null };

    // Link agent-level abort to this operation's controller
    // so that cancel() / cleanup() stops the current streamText call
    if (this._abortController.signal.aborted) {
      controller.abort();
    } else {
      const onAgentAbort = () => controller.abort();
      this._abortController.signal.addEventListener('abort', onAgentAbort, { once: true });
      // Clean up listener when this controller aborts (from any source)
      controller.signal.addEventListener('abort', () => {
        this._abortController.signal.removeEventListener('abort', onAgentAbort);
      }, { once: true });
    }

    // Set up overall operation timeout (default 5 minutes)
    // NOTE: For Vercel AI SDK paths, streamText() returns immediately and the
    // actual tool loop runs asynchronously. The graceful timeout timer is set up
    // in the run() method where results are actually awaited, not here.
    // This timer only handles the hard abort for non-graceful mode and engine paths.
    if (this.maxOperationTimeout && this.maxOperationTimeout > 0) {
      const gts = this._gracefulTimeoutState;
      if (this.timeoutBehavior === 'graceful' && gts) {
        // Graceful mode: timer is managed in run() method.
        // Only set up the AbortController link (no timer here).
      } else {
        // Hard mode: immediate abort (legacy behavior)
        timeoutState.timeoutId = setTimeout(() => {
          controller.abort();
          if (this.debug) {
            console.log(`[DEBUG] Operation timed out after ${this.maxOperationTimeout}ms (max operation timeout)`);
          }
        }, this.maxOperationTimeout);
      }
    }

    try {
      // Try engine paths (claude-code or codex)
      const useClaudeCode = this.clientApiProvider === 'claude-code' || process.env.USE_CLAUDE_CODE === 'true';
      const useCodex = this.clientApiProvider === 'codex' || process.env.USE_CODEX === 'true';

      let result;
      if (useClaudeCode || useCodex) {
        try {
          result = await this._tryEngineStreamPath(options, controller, timeoutState);
          // Gate engine stream with concurrency limiter if configured.
          // Engine paths bypass the Vercel model wrapper, so we wrap the
          // textStream async generator with acquire/release instead.
          if (result && limiter) {
            result = ProbeAgent._wrapEngineStreamWithLimiter(result, limiter, this.debug);
          }
        } catch (error) {
          if (this.debug) {
            const engineType = useClaudeCode ? 'Claude Code' : 'Codex';
            console.log(`[DEBUG] Failed to use ${engineType} engine, falling back to Vercel:`, error.message);
          }
          // Fall through to Vercel provider
        }
      }

      if (!result) {
        // Use Vercel AI SDK with retry/fallback
        result = await this._executeWithVercelProvider(options, controller);
      }

      return result;
    } finally {
      // Clean up timeout (for non-engine paths; engine paths clean up in the generator)
      if (timeoutState.timeoutId) {
        clearTimeout(timeoutState.timeoutId);
        timeoutState.timeoutId = null;
      }
    }
  }

  /**
   * Initialize Anthropic model
   */
  initializeAnthropicModel(apiKey, apiUrl, modelName) {
    this.provider = createAnthropic({
      apiKey: apiKey,
      ...(apiUrl && { baseURL: apiUrl }),
    });
    this.model = modelName || 'claude-sonnet-4-6';
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
    this.model = modelName || 'gpt-5.2';
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
   * Initialize Gemini built-in tools (gemini_google_search, gemini_url_context).
   * These are provider-defined tools that execute server-side on Google's infrastructure.
   * They are only available when the provider is Google Gemini.
   * @returns {{ googleSearch: boolean, urlContext: boolean }} Which tools were enabled
   * @private
   */
  _initializeGeminiBuiltinTools() {
    const isToolAllowed = (toolName) => this.allowedTools.isEnabled(toolName);
    const result = { googleSearch: false, urlContext: false };

    if (this.apiType !== 'google') {
      // Log info about unavailability for non-Google providers
      if (isToolAllowed('gemini_google_search') || isToolAllowed('gemini_url_context')) {
        if (this.debug) {
          console.error(`[DEBUG] Gemini built-in tools (gemini_google_search, gemini_url_context) are not available: provider is '${this.apiType}', not 'google'. These tools require the Google Gemini provider.`);
        }
      }
      return result;
    }

    // Check SDK support
    if (!this.provider || !this.provider.tools) {
      console.error('[ProbeAgent] Gemini built-in tools unavailable: @ai-sdk/google does not expose provider.tools. Upgrade to @ai-sdk/google v2.0.14+.');
      return result;
    }

    if (isToolAllowed('gemini_google_search')) {
      result.googleSearch = true;
      if (this.debug) {
        console.error('[DEBUG] Gemini built-in tool enabled: gemini_google_search');
      }
    }

    if (isToolAllowed('gemini_url_context')) {
      result.urlContext = true;
      if (this.debug) {
        console.error('[DEBUG] Gemini built-in tool enabled: gemini_url_context');
      }
    }

    return result;
  }

  /**
   * Build Gemini provider-defined tools object for streamText().
   * Returns undefined if no Gemini tools are enabled.
   * @returns {Object|undefined}
   * @private
   */
  _buildGeminiProviderTools() {
    if (this.apiType !== 'google' || !this._geminiToolsEnabled) {
      return undefined;
    }

    const { googleSearch, urlContext } = this._geminiToolsEnabled;
    if (!googleSearch && !urlContext) {
      return undefined;
    }

    if (!this.provider || !this.provider.tools) {
      return undefined;
    }

    const tools = {};
    const providerTools = this.provider.tools;

    if (googleSearch && providerTools.googleSearch) {
      tools.google_search = providerTools.googleSearch({});
    }
    if (urlContext && providerTools.urlContext) {
      tools.url_context = providerTools.urlContext({});
    }

    return Object.keys(tools).length > 0 ? tools : undefined;
  }

  /**
   * Build providerOptions for native thinking/reasoning based on thinkingEffort setting.
   * Maps effort levels to provider-specific parameters.
   * @param {number} maxResponseTokens - Current max response tokens for budget calculation
   * @returns {Object|undefined} providerOptions object or undefined if thinking is off
   * @private
   */
  _buildThinkingProviderOptions(maxResponseTokens) {
    if (!this.thinkingEffort) return undefined;

    const effort = this.thinkingEffort;

    // Map string effort levels to budget tokens
    const effortToBudget = {
      low: 4000,
      medium: 10000,
      high: 32000,
    };

    if (this.apiType === 'anthropic') {
      const budgetTokens = typeof effort === 'number'
        ? effort
        : effortToBudget[effort];
      if (!budgetTokens) return undefined;
      return {
        anthropic: {
          thinking: { type: 'enabled', budgetTokens },
        },
      };
    }

    if (this.apiType === 'openai') {
      // OpenAI reasoning models use reasoningEffort: 'low' | 'medium' | 'high'
      const reasoningEffort = typeof effort === 'number'
        ? (effort <= 4000 ? 'low' : effort <= 10000 ? 'medium' : 'high')
        : effort;
      if (!['low', 'medium', 'high'].includes(reasoningEffort)) return undefined;
      return {
        openai: {
          reasoningEffort,
        },
      };
    }

    if (this.apiType === 'google') {
      const thinkingBudget = typeof effort === 'number'
        ? effort
        : effortToBudget[effort];
      if (!thinkingBudget) return undefined;
      return {
        google: {
          thinkingConfig: { thinkingBudget },
        },
      };
    }

    return undefined;
  }

  /**
   * Build native Vercel AI SDK tools object for use with streamText().
   * Each tool wraps the existing toolImplementations with:
   * - sessionId and workingDirectory injection
   * - Event emission
   * - Output truncation
   * - Raw output block extraction
   * - Telemetry recording
   * - Delegate tool param injection
   *
   * @param {Object} options - Options from the answer() call
   * @param {Object} context - Execution context { maxIterations, currentMessages }
   * @returns {Object} Tools object for streamText()
   * @private
   */
  _buildNativeTools(options, context = {}) {
    const { maxIterations = 30 } = context;
    const nativeTools = {};
    const isToolAllowed = (toolName) => this.allowedTools.isEnabled(toolName);

    // Helper to wrap a tool implementation into a Vercel AI SDK tool
    const wrapTool = (toolName, schema, description, executeFn) => {
      // Auto-wrap plain JSON Schema objects with jsonSchema() for AI SDK 5 compatibility
      // Zod schemas have a _def property; plain objects need wrapping
      const resolvedSchema = schema && schema._def ? schema : jsonSchema(schema);
      return tool({
        description,
        inputSchema: resolvedSchema,
        execute: async (params) => {
          // Add sessionId and workingDirectory to params
          let resolvedWorkingDirectory = this.workspaceRoot || this.cwd || (this.allowedFolders && this.allowedFolders[0]) || process.cwd();
          if (params.workingDirectory) {
            const requestedDir = safeRealpath(isAbsolute(params.workingDirectory)
              ? resolve(params.workingDirectory)
              : resolve(resolvedWorkingDirectory, params.workingDirectory));
            const isWithinAllowed = !this.allowedFolders || this.allowedFolders.length === 0 ||
              this.allowedFolders.some(folder => {
                const resolvedFolder = safeRealpath(folder);
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

          // Emit tool start event
          this.events.emit('toolCall', {
            timestamp: new Date().toISOString(),
            name: toolName,
            args: toolParams,
            status: 'started',
            pauseStream: true
          });

          const toolStartTime = Date.now();
          try {
            // For delegate tool, inject additional params
            let result;
            if (toolName === 'delegate') {
              let allowedToolsForDelegate = null;
              if (this.allowedTools.mode === 'whitelist') {
                allowedToolsForDelegate = [...this.allowedTools.allowed];
              } else if (this.allowedTools.mode === 'none') {
                allowedToolsForDelegate = [];
              } else if (this.allowedTools.mode === 'all' && this.allowedTools.exclusions?.length > 0) {
                allowedToolsForDelegate = ['*', ...this.allowedTools.exclusions.map(t => '!' + t)];
              }

              const enhancedParams = {
                ...toolParams,
                currentIteration: context.currentIteration || 0,
                maxIterations,
                parentSessionId: this.sessionId,
                path: this.searchPath,
                provider: this.apiType,
                model: this.model,
                searchDelegate: this.searchDelegate,
                enableTasks: this.enableTasks,
                enableMcp: !!this.mcpBridge,
                mcpConfig: this.mcpConfig,
                mcpConfigPath: this.mcpConfigPath,
                enableBash: this.enableBash,
                bashConfig: this.bashConfig,
                allowEdit: this.allowEdit,
                allowedTools: allowedToolsForDelegate,
                debug: this.debug,
                tracer: this.tracer,
                parentAbortSignal: this._abortController.signal
              };

              if (this.debug) {
                console.log(`[DEBUG] Executing delegate tool`);
                console.log(`[DEBUG] Parent session: ${this.sessionId}`);
              }

              if (this.tracer) {
                this.tracer.recordDelegationEvent('tool_started', {
                  'delegation.task_preview': toolParams.task?.substring(0, 200)
                });
              }

              result = await executeFn(enhancedParams);
            } else {
              result = await executeFn(toolParams);
            }

            const toolDurationMs = Date.now() - toolStartTime;
            this._recordToolResultTelemetry(toolName, result, true, toolDurationMs, context.currentIteration || 0);

            // Emit tool success event
            this.events.emit('toolCall', {
              timestamp: new Date().toISOString(),
              name: toolName,
              args: toolParams,
              resultPreview: typeof result === 'string'
                ? (result.length > 200 ? result.substring(0, 200) + '...' : result)
                : (result ? JSON.stringify(result).substring(0, 200) + '...' : 'No Result'),
              status: 'completed'
            });

            let toolResultContent = typeof result === 'string' ? result : JSON.stringify(result, null, 2);

            // Convert absolute workspace paths to relative
            if (this.workspaceRoot && toolResultContent) {
              const wsPrefix = this.workspaceRoot.endsWith(sep) ? this.workspaceRoot : this.workspaceRoot + sep;
              toolResultContent = toolResultContent.split(wsPrefix).join('');
            }

            // Extract raw output blocks from tool result (before truncation)
            const { cleanedContent, extractedBlocks } = extractRawOutputBlocks(toolResultContent);
            if (extractedBlocks.length > 0) {
              toolResultContent = cleanedContent;
              this._extractedRawBlocks.push(...extractedBlocks);
              if (this.debug) {
                console.log(`[DEBUG] Extracted ${extractedBlocks.length} raw output blocks from tool result`);
              }
            }

            // Truncate if output exceeds token limit
            try {
              const truncateResult = await truncateIfNeeded(toolResultContent, this.tokenCounter, this.sessionId, this.maxOutputTokens);
              if (truncateResult.truncated) {
                toolResultContent = truncateResult.content;
                if (this.debug) {
                  console.log(`[DEBUG] Tool output truncated: ${truncateResult.originalTokens} tokens`);
                }
              }
            } catch (truncateError) {
              console.error(`[WARN] Tool output truncation failed: ${truncateError.message}`);
            }

            if (this.debug) {
              console.log(`[DEBUG] Tool ${toolName} executed successfully. Result length: ${toolResultContent.length}`);
            }

            return toolResultContent;
          } catch (error) {
            const toolDurationMs = Date.now() - toolStartTime;
            this._recordToolResultTelemetry(toolName, null, false, toolDurationMs, context.currentIteration || 0);

            // Emit tool error event
            this.events.emit('toolCall', {
              timestamp: new Date().toISOString(),
              name: toolName,
              args: toolParams,
              error: error.message || 'Unknown error',
              status: 'error'
            });

            if (this.debug) {
              console.error(`[DEBUG] Tool '${toolName}' failed: ${error.message}`);
            }

            // Format error for AI
            const errorMsg = formatErrorForAI(error);
            return errorMsg;
          }
        }
      });
    };

    // When _disableTools is set, provide no tools — the model responds with text directly
    if (options._disableTools) {
      return nativeTools;
    }

    // Add all enabled tools from toolImplementations
    // Note: MCP tools are also in toolImplementations but have no schema in _getToolSchemaAndDescription.
    // They are handled separately via mcpBridge.getVercelTools() below, so we skip them here.
    for (const [toolName, toolImpl] of Object.entries(this.toolImplementations)) {
      // Get schema and description for this tool
      const toolInfo = this._getToolSchemaAndDescription(toolName);
      if (!toolInfo) continue;
      const { schema, description } = toolInfo;
      if (schema && description) {
        nativeTools[toolName] = wrapTool(toolName, schema, description, toolImpl.execute);
      }
    }

    // Add MCP tools if available
    if (this.mcpBridge && !options._disableTools) {
      const mcpTools = this.mcpBridge.getVercelTools(this._filterMcpTools(this.mcpBridge.getToolNames()));
      for (const [name, mcpTool] of Object.entries(mcpTools)) {
        // MCP tools have raw JSON Schema inputSchema that must be wrapped with jsonSchema()
        // for the Vercel AI SDK. Without wrapping, asSchema() misidentifies them as Zod schemas.
        const mcpSchema = mcpTool.inputSchema || mcpTool.parameters;
        const wrappedSchema = mcpSchema && mcpSchema._def ? mcpSchema : jsonSchema(mcpSchema || { type: 'object', properties: {} });
        nativeTools[name] = tool({
          description: mcpTool.description || `MCP tool: ${name}`,
          inputSchema: wrappedSchema,
          execute: mcpTool.execute,
        });
      }
    }

    // Add Gemini provider tools as wrapper function tools.
    // The Gemini API does not allow mixing provider-defined tools with function tools
    // in the same request. To work around this, we create regular function tools that
    // internally make a separate API call using only the provider-defined tool.
    if (this.apiType === 'google' && this._geminiToolsEnabled && !options._disableTools) {
      const { googleSearch, urlContext } = this._geminiToolsEnabled;

      if (googleSearch && isToolAllowed('gemini_google_search')) {
        nativeTools.google_search = tool({
          description: 'Search the web using Google Search for current information, recent events, or real-time data.',
          inputSchema: z.object({
            query: z.string().describe('The search query to find information on the web')
          }),
          execute: async ({ query }) => {
            if (this.debug) {
              console.log(`[DEBUG] google_search wrapper: querying "${query}"`);
            }
            try {
              const { generateText: genText } = await import('ai');
              const searchResult = await genText({
                model: this.provider(this.model.includes('flash') ? this.model : this.model.replace('pro', 'flash')),
                messages: [{ role: 'user', content: query }],
                tools: { google_search: this.provider.tools.googleSearch({}) },
                stopWhen: stepCountIs(2),
                maxTokens: 4000
              });
              return searchResult.text || 'No search results found.';
            } catch (err) {
              if (this.debug) console.error(`[DEBUG] google_search wrapper error:`, err.message);
              return `Search failed: ${err.message}`;
            }
          }
        });
      }

      if (urlContext && isToolAllowed('gemini_url_context')) {
        nativeTools.url_context = tool({
          description: 'Fetch and analyze content from a specific URL. Use this to read web pages, documentation, or online resources.',
          inputSchema: z.object({
            url: z.string().describe('The URL to fetch and analyze')
          }),
          execute: async ({ url }) => {
            if (this.debug) {
              console.log(`[DEBUG] url_context wrapper: fetching "${url}"`);
            }
            try {
              const { generateText: genText } = await import('ai');
              const fetchResult = await genText({
                model: this.provider(this.model.includes('flash') ? this.model : this.model.replace('pro', 'flash')),
                messages: [{ role: 'user', content: `Summarize the content at this URL: ${url}` }],
                tools: { url_context: this.provider.tools.urlContext({}) },
                stopWhen: stepCountIs(2),
                maxTokens: 4000
              });
              return fetchResult.text || 'Could not fetch URL content.';
            } catch (err) {
              if (this.debug) console.error(`[DEBUG] url_context wrapper error:`, err.message);
              return `URL fetch failed: ${err.message}`;
            }
          }
        });
      }
    }

    return nativeTools;
  }

  /**
   * Get the Zod schema and description for a tool by name
   * @param {string} toolName - Tool name
   * @returns {{ schema: z.ZodObject, description: string } | null}
   * @private
   */
  _getToolSchemaAndDescription(toolName) {
    const toolMap = {
      search: {
        schema: searchSchema,
        description: this.searchDelegate
          ? 'Search code in the repository by asking a question. Accepts natural language questions — a subagent breaks them into targeted keyword searches and returns extracted code blocks. Do NOT formulate keyword queries yourself.'
          : 'Search code in the repository using keyword queries with Elasticsearch syntax. Handles stemming, case-insensitive matching, and camelCase/snake_case splitting automatically — do NOT try keyword variations manually.'
      },
      // query tool (ast-grep) removed from AI-facing tools — models struggle with pattern syntax
      // query: {
      //   schema: querySchema,
      //   description: 'Search code using ast-grep structural pattern matching.'
      // },
      extract: {
        schema: extractSchema,
        description: 'Extract code blocks from files based on file paths and optional line numbers.'
      },
      delegate: {
        schema: delegateSchema,
        description: 'Delegate big distinct tasks to specialized probe subagents.'
      },
      analyze_all: {
        schema: analyzeAllSchema,
        description: 'Process ALL data matching a query using map-reduce for aggregate questions.'
      },
      execute_plan: {
        schema: executePlanSchema,
        description: 'Execute a DSL program to orchestrate tool calls.'
      },
      cleanup_execute_plan: {
        schema: cleanupExecutePlanSchema,
        description: 'Clean up output buffer and session store from previous execute_plan calls.'
      },
      bash: {
        schema: bashSchema,
        description: 'Execute bash commands for system exploration and development tasks.'
      },
      edit: {
        schema: editSchema,
        description: 'Edit files using text replacement, AST-aware symbol operations, or line-targeted editing.'
      },
      create: {
        schema: createSchema,
        description: 'Create new files with specified content.'
      },
      multi_edit: {
        schema: multiEditSchema,
        description: 'Apply multiple file edits in one call using a JSON array of operations.'
      },
      listFiles: {
        schema: listFilesSchema,
        description: 'List files and directories in a specified location.'
      },
      searchFiles: {
        schema: searchFilesSchema,
        description: 'Find files matching a glob pattern with recursive search capability.'
      },
      readImage: {
        schema: readImageSchema,
        description: 'Read and load an image file for AI analysis.'
      },
      listSkills: {
        schema: listSkillsSchema,
        description: 'List available agent skills discovered in the repository.'
      },
      useSkill: {
        schema: useSkillSchema,
        description: 'Load and activate a specific skill\'s instructions.'
      },
      task: {
        schema: taskSchema,
        description: 'Manage tasks for tracking progress (create, update, complete, delete, list).'
      }
    };

    return toolMap[toolName] || null;
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
    this.model = modelName || 'anthropic.claude-sonnet-4-6';
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
          model: this.model  // Pass model name (e.g., gpt-5.2, o3, etc.)
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
      // Use safeRealpath() to resolve symlinks and handle path traversal attempts (e.g., '/allowed/../etc/passwd')
      // This prevents symlink bypass attacks (e.g., /tmp -> /private/tmp on macOS)
      const allowedDirs = this.allowedFolders && this.allowedFolders.length > 0 ? this.allowedFolders : [process.cwd()];

      let absolutePath;
      let isPathAllowed = false;

      // If absolute path, check if it's within any allowed directory
      if (isAbsolute(imagePath)) {
        // Use safeRealpath to resolve symlinks for security
        absolutePath = safeRealpath(resolve(imagePath));
        isPathAllowed = allowedDirs.some(dir => {
          const resolvedDir = safeRealpath(dir);
          // Ensure the path is within the allowed directory (add separator to prevent prefix attacks)
          return absolutePath === resolvedDir || absolutePath.startsWith(resolvedDir + sep);
        });
      } else {
        // For relative paths, try resolving against each allowed directory
        for (const dir of allowedDirs) {
          const resolvedDir = safeRealpath(dir);
          const resolvedPath = safeRealpath(resolve(dir, imagePath));
          // Ensure the resolved path is within the allowed directory
          if (resolvedPath === resolvedDir || resolvedPath.startsWith(resolvedDir + sep)) {
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

    // Use workspaceRoot for consistent path handling
    const rootDirectory = this.workspaceRoot || (this.allowedFolders.length > 0 ? this.allowedFolders[0] : process.cwd());
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
    // Use workspaceRoot for consistent path handling
    if (this.workspaceRoot) {
      return resolve(this.workspaceRoot);
    }
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
    return formatAvailableSkills(skills);
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
    const searchToolDesc1 = this.searchDelegate
      ? '- search: Ask natural language questions to find code (e.g., "How does authentication work?"). A subagent handles keyword searches and returns extracted code blocks. Do NOT formulate keyword queries — just ask questions.'
      : '- search: Find code patterns using keyword queries with Elasticsearch syntax. Handles stemming and case variations automatically — do NOT try manual keyword variations.';
    systemPrompt += `You have access to powerful code search and analysis tools through MCP:
${searchToolDesc1}
- extract: Extract specific code sections with context
- listFiles: Browse directory contents
- searchFiles: Find files by name patterns`;

    if (this.enableBash) {
      systemPrompt += `\n- bash: Execute bash commands for system operations (building, running tests, git, etc.). NEVER use bash for code exploration (no grep, cat, find, head, tail) — always use search and extract tools instead, they are faster and more accurate.`;
    }

    const searchGuidance1 = this.searchDelegate
      ? '1. Start with search — ask a question about what you want to understand. It returns extracted code blocks directly.'
      : '1. Start with search to find relevant code patterns. One search per concept is usually enough — probe handles stemming and case variations.';
    const extractGuidance1 = this.searchDelegate
      ? '2. Use extract only if you need more context or a full file'
      : '2. Use extract to get detailed context when needed';

    systemPrompt += `\n
When exploring code:
${searchGuidance1}
${extractGuidance1}
3. Prefer focused, specific searches over broad queries
4. Do NOT repeat the same search or try trivial keyword variations — probe handles stemming and case variations automatically
5. If 2-3 consecutive searches return no results for a concept, stop searching for it — the term likely does not exist in that codebase
6. Combine multiple tools to build complete understanding`;

    // Add workspace context
    if (this.allowedFolders && this.allowedFolders.length > 0) {
      systemPrompt += `\n\nWorkspace: ${this.allowedFolders.join(', ')}`;
    }

    // Add repository structure if available
    if (this.fileList) {
      systemPrompt += `\n\n# Repository Structure\n`;
      systemPrompt += `You are working with a repository located at: ${this.workspaceRoot}\n\n`;
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
    const searchToolDesc2 = this.searchDelegate
      ? '- search: Ask natural language questions to find code (e.g., "How does authentication work?"). A subagent handles keyword searches and returns extracted code blocks. Do NOT formulate keyword queries — just ask questions.'
      : '- search: Find code patterns using keyword queries with Elasticsearch syntax. Handles stemming and case variations automatically — do NOT try manual keyword variations.';
    systemPrompt += `You have access to powerful code search and analysis tools through MCP:
${searchToolDesc2}
- extract: Extract specific code sections with context
- listFiles: Browse directory contents
- searchFiles: Find files by name patterns`;

    if (this.enableBash) {
      systemPrompt += `\n- bash: Execute bash commands for system operations (building, running tests, git, etc.). NEVER use bash for code exploration (no grep, cat, find, head, tail) — always use search and extract tools instead, they are faster and more accurate.`;
    }

    const searchGuidance2 = this.searchDelegate
      ? '1. Start with search — ask a question about what you want to understand. It returns extracted code blocks directly.'
      : '1. Start with search to find relevant code patterns. One search per concept is usually enough — probe handles stemming and case variations.';
    const extractGuidance2 = this.searchDelegate
      ? '2. Use extract only if you need more context or a full file'
      : '2. Use extract to get detailed context when needed';

    systemPrompt += `\n
When exploring code:
${searchGuidance2}
${extractGuidance2}
3. Prefer focused, specific searches over broad queries
4. Do NOT repeat the same search or try trivial keyword variations — probe handles stemming and case variations automatically
5. If 2-3 consecutive searches return no results for a concept, stop searching for it — the term likely does not exist in that codebase
6. Combine multiple tools to build complete understanding`;

    // Add workspace context
    if (this.allowedFolders && this.allowedFolders.length > 0) {
      systemPrompt += `\n\nWorkspace: ${this.allowedFolders.join(', ')}`;
    }

    // Add repository structure if available
    if (this.fileList) {
      systemPrompt += `\n\n# Repository Structure\n`;
      systemPrompt += `You are working with a repository located at: ${this.workspaceRoot}\n\n`;
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

    // Common instructions (simplified - tools are now provided via native tool calling)
    const commonInstructions = `<instructions>
Follow these instructions carefully:
1. Analyze the user's request.
2. Use the available tools step-by-step to fulfill the request.
3. You MUST use the search tool before answering ANY code-related question. NEVER answer from memory or general knowledge — your answers must be grounded in actual code found via search/extract.${this.searchDelegate ? ' Ask natural language questions — the search subagent handles keyword formulation and returns extracted code blocks. Use extract only to expand context or read full files.' : ' Search handles stemming and case variations automatically — do NOT try keyword variations manually. Read full files only if really necessary.'}
4. Ensure to get really deep and understand the full picture before answering. Follow call chains — if function A calls B, search for B too. Look for related subsystems (e.g., if asked about rate limiting, also check for quota, throttling, smoothing).
5. Once the task is fully completed, provide your final answer directly as text. Always cite specific files and line numbers as evidence. Do NOT output planning or thinking text — go straight to the answer.
6. ${this.searchDelegate ? 'Ask clear, specific questions when searching. Each search should target a distinct concept or question.' : 'Prefer concise and focused search queries. Use specific keywords and phrases to narrow down results.'}
7. NEVER use bash for code exploration (no grep, cat, find, head, tail, awk, sed) — always use search and extract tools instead. Bash is only for system operations like building, running tests, or git commands.${this.allowEdit ? `
7. When modifying files, choose the appropriate tool:
    - Use 'edit' for all code modifications:
      * PREFERRED: Use start_line (and optionally end_line) for line-targeted editing — this is the safest and most precise approach.${this.hashLines ? ' Use the line:hash references from extract/search output (e.g. "42:ab") for integrity verification.' : ''} Always use extract first to see line numbers${this.hashLines ? ' and hashes' : ''}, then edit by line reference.
      * For editing inside large functions: first use extract with the symbol target (e.g. "file.js#myFunction") to see the function with line numbers${this.hashLines ? ' and hashes' : ''}, then use start_line/end_line to surgically edit specific lines within it.
      * For rewriting entire functions/classes/methods, use the symbol parameter instead (no exact text matching needed).
      * FALLBACK ONLY: Use old_string + new_string for simple single-line changes where the text is unique. Copy old_string verbatim from the file. Keep old_string as small as possible.
      * IMPORTANT: After multiple edits to the same file, re-read the changed areas before continuing — use extract with a targeted symbol (e.g. "file.js#myFunction") or a line range (e.g. "file.js:50-80") instead of re-reading the full file.
    - Use 'create' for new files or complete file rewrites.
    - If an edit fails, read the error message — it tells you exactly how to fix the call and retry.
    - The system tracks which files you've seen via search/extract. If you try to edit a file you haven't read, or one that changed since you last read it, the edit will fail with instructions to re-read first. Always use extract before editing to ensure you have current file content.` : ''}
</instructions>
`;

    // Use predefined prompts from shared module (imported at top of file)
    let systemMessage = '';

    // Build system message from predefined prompt + optional custom prompt
    if (this.customPrompt && this.promptType && predefinedPrompts[this.promptType]) {
      // Both: use predefined as base, append custom wrapped in tag
      systemMessage = "<role>" + predefinedPrompts[this.promptType] + "</role>";
      systemMessage += commonInstructions;
      systemMessage += "\n<custom-instructions>\n" + this.customPrompt + "\n</custom-instructions>";
      if (this.debug) {
        console.log(`[DEBUG] Using predefined prompt: ${this.promptType} + custom prompt`);
      }
    } else if (this.customPrompt) {
      // Only custom prompt
      systemMessage = "<role>" + this.customPrompt + "</role>";
      if (this.debug) {
        console.log(`[DEBUG] Using custom prompt`);
      }
    } else if (this.promptType && predefinedPrompts[this.promptType]) {
      // Only predefined prompt
      systemMessage = "<role>" + predefinedPrompts[this.promptType] + "</role>";
      if (this.debug) {
        console.log(`[DEBUG] Using predefined prompt: ${this.promptType}`);
      }
      systemMessage += commonInstructions;
    } else {
      // Default: code explorer
      systemMessage = "<role>" + predefinedPrompts['code-explorer'] + "</role>";
      if (this.debug) {
        console.log(`[DEBUG] Using default prompt: code explorer`);
      }
      systemMessage += commonInstructions;
    }

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

    // Add folder information using workspace root and relative paths
    const searchDirectory = this.workspaceRoot;
    if (this.debug) {
      console.log(`[DEBUG] Generating file list for workspace root: ${searchDirectory}...`);
    }

    // Convert allowed folders to relative paths for cleaner AI context
    // Add ./ prefix to make it clear these are relative paths
    const relativeWorkspaces = this.allowedFolders.map(f => {
      const rel = toRelativePath(f, this.workspaceRoot);
      // Add ./ prefix if not already starting with . and not an absolute path
      if (rel && rel !== '.' && !rel.startsWith('.') && !rel.startsWith('/')) {
        return './' + rel;
      }
      return rel;
    }).filter(f => f && f !== '.');

    // Describe available paths in a user-friendly way
    let workspaceDesc;
    if (relativeWorkspaces.length === 0) {
      workspaceDesc = '. (current directory)';
    } else {
      workspaceDesc = relativeWorkspaces.join(', ');
    }

    try {
      const files = await listFilesByLevel({
        directory: searchDirectory,
        maxFiles: 100,
        respectGitignore: !process.env.PROBE_NO_GITIGNORE || process.env.PROBE_NO_GITIGNORE === '',
        cwd: this.workspaceRoot
      });

      systemMessage += `\n# Repository Structure\n\nYou are working with a workspace. Available paths: ${workspaceDesc}\n\nHere's an overview of the repository structure (showing up to 100 most relevant files):\n\n\`\`\`\n${files}\n\`\`\`\n\n`;
    } catch (error) {
      if (this.debug) {
        console.log(`[DEBUG] Could not generate file list: ${error.message}`);
      }
      systemMessage += `\n# Repository Structure\n\nYou are working with a workspace. Available paths: ${workspaceDesc}\n\n`;
    }

    // Add architecture context if available
    await this.loadArchitectureContext();
    systemMessage += this.getArchitectureSection();

    if (this.allowedFolders.length > 0) {
      const relativeAllowed = this.allowedFolders.map(f => {
        const rel = toRelativePath(f, this.workspaceRoot);
        // Add ./ prefix if not already starting with . and not an absolute path
        if (rel && rel !== '.' && !rel.startsWith('.') && !rel.startsWith('/')) {
          return './' + rel;
        }
        return rel;
      });
      systemMessage += `\n**Important**: For security reasons, you can only access these paths: ${relativeAllowed.join(', ')}\n\n`;
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

      // Reset output buffer for this answer() call — but NOT during recursive calls.
      // _schemaFormatted: recursive call to fix JSON formatting
      // _completionPromptProcessed: recursive call for completionPrompt follow-up
      // Both must preserve the output buffer so the parent call can append it.
      if (this._outputBuffer && !options?._schemaFormatted && !options?._completionPromptProcessed) {
        this._outputBuffer.items = [];
        // Also reset the extracted blocks accumulator (issue #438)
        this._extractedRawBlocks = [];
      }

      // START CHECKPOINT: Initialize task management for this request
      if (this.enableTasks) {
        try {
          // Create fresh TaskManager for each request (request-scoped)
          this.taskManager = new TaskManager({ debug: this.debug });

          // Register task tool for this request
          const isToolAllowed = (toolName) => this.allowedTools.isEnabled(toolName);
          if (isToolAllowed('task')) {
            this.toolImplementations.task = createTaskTool({
              taskManager: this.taskManager,
              tracer: this.tracer,
              debug: this.debug
            });
          }

          // Record telemetry for task initialization
          if (this.tracer && typeof this.tracer.recordTaskEvent === 'function') {
            this.tracer.recordTaskEvent('session_started', {
              'task.enabled': true
            });
          }

          if (this.debug) {
            console.log('[DEBUG] Task management initialized for this request');
          }
        } catch (taskInitError) {
          // Log error but don't fail the request - task management is optional
          console.error('[ProbeAgent] Failed to initialize task management:', taskInitError.message);
          this.taskManager = null;
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

      // If schema is provided, prepend JSON format requirement to user message
      // Skip when _disableTools is set — native Output.object() handles schema constraint
      if (options.schema && !options._schemaFormatted && !options._disableTools) {
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

      // Proactively compact for multi-turn conversations.
      // On turn 2+, previous turns contain full tool call/result history which can
      // be 50K+ tokens. This drowns out the new user message and causes the model to
      // focus on prior context rather than the new question.
      // compactMessages strips intermediate monologue from completed segments,
      // keeping user messages + final answers from prior turns.
      // Must run AFTER adding the new user message so the compactor sees 2+ segments
      // (completed prior turns + the new incomplete turn), preserving the latest segment.
      if (this.history.length > 0) {
        const compacted = compactMessages(currentMessages, { keepLastSegment: true, minSegmentsToKeep: 1 });
        if (compacted.length < currentMessages.length) {
          const stats = calculateCompactionStats(currentMessages, compacted);
          if (this.debug) {
            console.log(`[DEBUG] Proactive history compaction: ${currentMessages.length} → ${compacted.length} messages (${stats.reductionPercent}% reduction, ~${stats.tokensSaved} tokens saved)`);
          }
          currentMessages = compacted;
        }
      }

      let currentIteration = 0;
      let finalResult = 'I was unable to complete your request due to reaching the maximum number of tool iterations.';

      // Adjust max iterations if schema is provided
      // +1 for schema formatting
      // +2 for potential Mermaid validation retries (can be multiple diagrams)
      // +1 for potential JSON correction
      // _maxIterationsOverride: used by correction calls to cap iterations (issue #447)
      const baseMaxIterations = options._maxIterationsOverride || this.maxIterations || MAX_TOOL_ITERATIONS;
      const maxIterations = (options._maxIterationsOverride) ? baseMaxIterations : (options.schema ? baseMaxIterations + 4 : baseMaxIterations);

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

      // Iteration counter for telemetry

      // Native tool calling via Vercel AI SDK streamText + maxSteps
      const toolContext = { maxIterations, currentIteration: 0, currentMessages };

      const tools = this._buildNativeTools(options, toolContext);

      if (this.debug) {
        const toolNames = Object.keys(tools);
        console.log(`[DEBUG] Agent tools registered (${toolNames.length}): ${toolNames.join(', ')}`);
      }

      let maxResponseTokens = this.maxResponseTokens;
      if (!maxResponseTokens) {
        maxResponseTokens = 4000;
        if (this.model && this.model.includes('opus') || this.model && this.model.includes('sonnet') || this.model && this.model.startsWith('gpt-4') || this.model && this.model.startsWith('gpt-5')) {
          maxResponseTokens = 8192;
        } else if (this.model && this.model.startsWith('gemini')) {
          maxResponseTokens = 32000;
        }
      }

      // Track whether completionPrompt has been injected into the loop
      let completionPromptInjected = false;
      let preCompletionResult = null; // Stores the result before completionPrompt for fallback

      // Graceful timeout state — shared between setTimeout (in streamTextWithRetryAndFallback)
      // and prepareStep/stopWhen callbacks (in streamText loop)
      const gracefulTimeoutState = {
        triggered: false,      // Set to true when soft timeout fires
        bonusStepsUsed: 0,     // Steps taken after soft timeout
        bonusStepsMax: this.gracefulTimeoutBonusSteps
      };
      this._gracefulTimeoutState = gracefulTimeoutState;

      // Context compaction retry loop
      let compactionAttempted = false;
      while (true) {
        try {
          const messagesForAI = this.prepareMessagesWithImages(currentMessages);

          const streamOptions = {
            model: this.provider ? this.provider(this.model) : this.model,
            messages: messagesForAI,
            tools,
            stopWhen: ({ steps }) => {
              // Graceful timeout wind-down: override normal limits, stop only when bonus steps exhausted
              if (gracefulTimeoutState.triggered) {
                if (gracefulTimeoutState.bonusStepsUsed >= gracefulTimeoutState.bonusStepsMax) {
                  if (this.debug) {
                    console.log(`[DEBUG] stopWhen: graceful timeout bonus steps exhausted (${gracefulTimeoutState.bonusStepsUsed}/${gracefulTimeoutState.bonusStepsMax}), forcing stop`);
                  }
                  return true;
                }
                return false; // Allow more bonus steps
              }

              // Hard limit
              if (steps.length >= maxIterations) return true;

              const lastStep = steps[steps.length - 1];
              const modelWantsToStop = lastStep?.finishReason === 'stop'
                && (!lastStep?.toolCalls || lastStep.toolCalls.length === 0);

              if (modelWantsToStop) {
                // Task blocking: force continuation when tasks are incomplete
                if (this.enableTasks && this.taskManager?.hasIncompleteTasks()) {
                  const highIterationCount = steps.length > maxIterations * 0.7;
                  if (!highIterationCount) return false; // Force continuation
                }

                // Completion prompt: force one more round for review
                if (this.completionPrompt && !options._completionPromptProcessed && !completionPromptInjected) {
                  // Save the pre-completion result for fallback
                  preCompletionResult = lastStep.text || null;
                  return false; // Force continuation — prepareStep will inject the prompt
                }
              }

              // Circuit breaker: consecutive no-tool steps
              let trailingNoTool = 0;
              for (let i = steps.length - 1; i >= 0; i--) {
                if (!steps[i].toolCalls?.length) trailingNoTool++;
                else break;
              }
              if (trailingNoTool >= 5) return true;

              // Circuit breaker: identical/stuck responses
              if (trailingNoTool >= 3) {
                const recentTexts = steps.slice(-3).map(s => s.text);
                if (recentTexts.every(t => t && t === recentTexts[0])) return true;
                if (recentTexts.every(t => detectStuckResponse(t))) return true;
              }

              // Circuit breaker: repeated identical tool calls (e.g. model ignores dedup message)
              if (steps.length >= 3) {
                const last3 = steps.slice(-3);
                const allHaveTools = last3.every(s => s.toolCalls?.length === 1);
                if (allHaveTools) {
                  const signatures = last3.map(s => {
                    const tc = s.toolCalls[0];
                    return `${tc.toolName}::${JSON.stringify(tc.args ?? tc.input)}`;
                  });
                  if (signatures[0] === signatures[1] && signatures[1] === signatures[2]) {
                    if (this.debug) {
                      console.log(`[DEBUG] Circuit breaker: 3 consecutive identical tool calls detected (${last3[0].toolCalls[0].toolName}), forcing stop`);
                    }
                    return true;
                  }
                }

              }

              return false;
            },
            prepareStep: ({ steps, stepNumber }) => {
              // Graceful timeout wind-down: force text-only response with wrap-up reminder
              if (gracefulTimeoutState.triggered) {
                gracefulTimeoutState.bonusStepsUsed++;
                const remaining = gracefulTimeoutState.bonusStepsMax - gracefulTimeoutState.bonusStepsUsed;

                if (gracefulTimeoutState.bonusStepsUsed === 1) {
                  // First wind-down step: inject wrap-up message
                  if (this.debug) {
                    console.log(`[DEBUG] prepareStep: graceful timeout wind-down step 1/${gracefulTimeoutState.bonusStepsMax}`);
                  }
                  if (this.tracer) {
                    this.tracer.addEvent('graceful_timeout.wind_down_started', {
                      bonus_steps_max: gracefulTimeoutState.bonusStepsMax,
                      current_iteration: currentIteration,
                      max_iterations: maxIterations
                    });
                  }
                  return {
                    toolChoice: 'none',
                    userMessage: `⚠️ TIME LIMIT REACHED. You are running out of time. You have ${remaining} step(s) remaining. Provide your BEST answer NOW using the information you have already gathered. Do NOT call any more tools. Summarize your findings and respond completely. If something was not completed, honestly state what was not done and provide any partial results or recommendations you can offer.`
                  };
                }

                if (this.debug) {
                  console.log(`[DEBUG] prepareStep: graceful timeout wind-down step ${gracefulTimeoutState.bonusStepsUsed}/${gracefulTimeoutState.bonusStepsMax} (${remaining} remaining)`);
                }
                return { toolChoice: 'none' };
              }

              // Last-iteration warning
              if (stepNumber === maxIterations - 1) {
                return {
                  toolChoice: 'none',
                };
              }

              // Force text-only response after 2 consecutive identical tool calls
              if (steps.length >= 2) {
                const last2 = steps.slice(-2);
                if (last2.every(s => s.toolCalls?.length === 1)) {
                  const tc1 = last2[0].toolCalls[0];
                  const tc2 = last2[1].toolCalls[0];
                  const sig1 = `${tc1.toolName}::${JSON.stringify(tc1.args ?? tc1.input)}`;
                  const sig2 = `${tc2.toolName}::${JSON.stringify(tc2.args ?? tc2.input)}`;
                  if (sig1 === sig2) {
                    if (this.debug) {
                      console.log(`[DEBUG] prepareStep: 2 consecutive identical tool calls (${tc1.toolName}), forcing toolChoice=none`);
                      console.log(`[DEBUG]   sig: ${sig1.substring(0, 200)}`);
                    }
                    return { toolChoice: 'none' };
                  }
                }
              }

              // Force text-only response after 3 consecutive tool errors
              // (e.g. workspace deleted mid-run — let the model produce its answer)
              if (steps.length >= 3) {
                const last3 = steps.slice(-3);
                const allErrors = last3.every(s =>
                  s.toolResults?.length > 0 && s.toolResults.every(tr => {
                    const r = typeof tr.result === 'string' ? tr.result : '';
                    return r.includes('<error ') || r.includes('does not exist');
                  })
                );
                if (allErrors) {
                  if (this.debug) {
                    console.log(`[DEBUG] prepareStep: 3 consecutive tool errors, forcing toolChoice=none`);
                  }
                  return { toolChoice: 'none' };
                }
              }

              const lastStep = steps[steps.length - 1];
              const modelJustStopped = lastStep?.finishReason === 'stop'
                && (!lastStep?.toolCalls || lastStep.toolCalls.length === 0);

              if (modelJustStopped) {
                // Task blocking: inject reminder when tasks are incomplete
                if (this.enableTasks && this.taskManager?.hasIncompleteTasks()) {
                  const taskSummary = this.taskManager.getTaskSummary();
                  const blockedMessage = createTaskCompletionBlockedMessage(taskSummary);
                  return {
                    userMessage: blockedMessage
                  };
                }

                // Completion prompt: inject review message on first stop
                if (this.completionPrompt && !options._completionPromptProcessed && !completionPromptInjected) {
                  completionPromptInjected = true;
                  const resultToReview = lastStep.text || preCompletionResult || '';

                  if (this.debug) {
                    console.log('[DEBUG] Injecting completion prompt into main loop via prepareStep...');
                  }

                  if (this.tracer) {
                    this.tracer.recordEvent('completion_prompt.started', {
                      'completion_prompt.original_result_length': resultToReview.length
                    });
                  }

                  const completionPromptMessage = `${this.completionPrompt}

Here is the result to review:
<result>
${resultToReview}
</result>

IMPORTANT: First review ALL completed work in the conversation above before taking any action.
Double-check your response based on the criteria above. If everything looks good, respond with your previous answer exactly as-is. If your text has inaccuracies, fix the text. Only call a tool if you find a genuinely MISSING action — NEVER redo work that was already completed successfully. Respond with the COMPLETE corrected answer.`;

                  return {
                    userMessage: completionPromptMessage,
                    toolChoice: 'none' // Force text-only review — no tool calls
                  };
                }
              }

              return undefined;
            },
            maxTokens: maxResponseTokens,
            temperature: 0.3,
            onStepFinish: (stepResult) => {
              const { toolResults, toolCalls, text, reasoningText, finishReason, usage } = stepResult;
              currentIteration++;
              toolContext.currentIteration = currentIteration;

              // Record telemetry — include model's reasoning and tool call details
              if (this.tracer) {
                const stepEvent = {
                  'iteration': currentIteration,
                  'max_iterations': maxIterations,
                  'finish_reason': finishReason,
                  'has_tool_calls': !!(toolResults && toolResults.length > 0)
                };
                // Model's text output (its monologue explaining why it's calling tools)
                if (text) {
                  stepEvent['ai.text'] = text.substring(0, 10000);
                  stepEvent['ai.text.length'] = text.length;
                }
                // Model's internal reasoning/thinking tokens (if available)
                if (reasoningText) {
                  stepEvent['ai.reasoning'] = reasoningText.substring(0, 10000);
                  stepEvent['ai.reasoning.length'] = reasoningText.length;
                }
                // Tool call names and args for this step
                if (toolCalls && toolCalls.length > 0) {
                  stepEvent['ai.tool_calls'] = toolCalls.map(tc => ({
                    name: tc.toolName,
                    args: JSON.stringify(tc.args || {}).substring(0, 2000)
                  }));
                }
                this.tracer.addEvent('iteration.step', stepEvent);

                // Track graceful timeout wind-down steps
                if (gracefulTimeoutState.triggered) {
                  this.tracer.addEvent('graceful_timeout.wind_down_step', {
                    bonus_step: gracefulTimeoutState.bonusStepsUsed,
                    bonus_max: gracefulTimeoutState.bonusStepsMax
                  });
                }
              }

              // Record token usage
              if (usage) {
                this.tokenCounter.recordUsage(usage);
              }

              // Stream text to callback if present
              if (options.onStream && text) {
                options.onStream(text);
              }

              if (this.debug) {
                const toolSummary = toolCalls?.length
                  ? toolCalls.map(tc => {
                      const args = tc.args ? JSON.stringify(tc.args) : '';
                      return args ? `${tc.toolName}(${debugTruncate(args, 120)})` : tc.toolName;
                    }).join(', ')
                  : 'none';
                console.log(`[DEBUG] Step ${currentIteration}/${maxIterations} finished (reason: ${finishReason}, tools: [${toolSummary}])`);
                if (text) {
                  console.log(`[DEBUG]   model text: ${debugTruncate(text)}`);
                }
                if (reasoningText) {
                  console.log(`[DEBUG]   reasoning: ${debugTruncate(reasoningText)}`);
                }
                debugLogToolResults(toolResults);
              }
            }
          };

          // Native JSON schema output — use model's built-in JSON schema constraint
          // when no tools are active (many providers like Gemini don't support
          // structured output + function calling simultaneously).
          // When tools ARE active, we rely on AJV post-validation + correction loop.
          const hasActiveTools = Object.keys(tools).length > 0;
          if (options.schema && !hasActiveTools) {
            try {
              const parsedSchema = typeof options.schema === 'string' ? JSON.parse(options.schema) : options.schema;
              if (isJsonSchema(options.schema)) {
                streamOptions.output = Output.object({ schema: jsonSchema(parsedSchema) });
                if (this.debug) {
                  console.log(`[DEBUG] Native JSON schema output enabled (no active tools)`);
                }
              }
            } catch (e) {
              if (this.debug) {
                console.log(`[DEBUG] Failed to set native JSON schema output: ${e.message}`);
              }
            }
          }

          // Add native thinking/reasoning providerOptions when thinkingEffort is set
          const providerOpts = this._buildThinkingProviderOptions(maxResponseTokens);
          if (providerOpts) {
            streamOptions.providerOptions = providerOpts;
          }

          const executeAIRequest = async () => {
            const result = await this.streamTextWithRetryAndFallback(streamOptions);

            // Set up graceful timeout timer now that streamText is running.
            // streamText() returns immediately — the actual tool loop runs asynchronously
            // and completes when we await result.steps/result.text below.
            let gracefulTimeoutId = null;
            let hardAbortTimeoutId = null;
            if (this.timeoutBehavior === 'graceful' && gracefulTimeoutState && this.maxOperationTimeout > 0) {
              gracefulTimeoutId = setTimeout(() => {
                gracefulTimeoutState.triggered = true;
                if (this.debug) {
                  console.log(`[DEBUG] Soft timeout after ${this.maxOperationTimeout}ms — entering wind-down mode (${gracefulTimeoutState.bonusStepsMax} bonus steps)`);
                }
                // Safety net: hard abort after 60s if wind-down doesn't complete
                hardAbortTimeoutId = setTimeout(() => {
                  if (this._abortController) {
                    this._abortController.abort();
                  }
                  if (this.debug) {
                    console.log(`[DEBUG] Hard abort — wind-down safety net expired after 60s`);
                  }
                }, 60000);
              }, this.maxOperationTimeout);
            }

            try {
              // Use only the last step's text as the final answer.
              // result.text concatenates ALL steps (including intermediate planning text),
              // but the user should only see the final answer from the last step.
              const steps = await result.steps;
              let finalText;
              if (steps && steps.length > 1) {
                // Multi-step: use last step's text (the actual answer after tool calls)
                const lastStepText = steps[steps.length - 1].text;
                finalText = lastStepText || await result.text;
              } else {
                finalText = await result.text;
              }

              if (this.debug) {
                console.log(`[DEBUG] streamText completed: ${steps?.length || 0} steps, finalText=${finalText?.length || 0} chars`);
              }

              // Record final token usage
              const usage = await result.usage;
              if (usage) {
                this.tokenCounter.recordUsage(usage, result.experimental_providerMetadata);
              }

              return { finalText, result };
            } finally {
              // Clean up graceful timeout timers
              if (gracefulTimeoutId) clearTimeout(gracefulTimeoutId);
              if (hardAbortTimeoutId) clearTimeout(hardAbortTimeoutId);
            }
          };

          let aiResult;
          if (this.tracer) {
            const inputPreview = message.length > 1000
              ? message.substring(0, 1000) + '... [truncated]'
              : message;

            aiResult = await this.tracer.withSpan('ai.request', executeAIRequest, {
              'ai.model': this.model,
              'ai.provider': this.clientApiProvider || 'auto',
              'ai.input': inputPreview,
              'ai.input_length': message.length,
              'max_steps': maxIterations,
              'max_tokens': maxResponseTokens,
              'temperature': 0.3,
              'message_count': currentMessages.length
            });
          } else {
            aiResult = await executeAIRequest();
          }

          // Try native JSON schema output first — Output.object() is set when no tools are active
          if (options.schema && streamOptions.output) {
            try {
              const outputObject = await aiResult.result.output;
              if (outputObject) {
                finalResult = JSON.stringify(outputObject);
              } else if (aiResult.finalText) {
                finalResult = aiResult.finalText;
              }
            } catch (e) {
              // NoObjectGeneratedError — fall back to text-based extraction
              if (this.debug) {
                console.log(`[DEBUG] Native JSON output failed, falling back to text: ${e.message}`);
              }
              if (aiResult.finalText) {
                finalResult = aiResult.finalText;
              }
            }
          } else if (aiResult.finalText) {
            finalResult = aiResult.finalText;
          }

          // Graceful timeout fallback: when wind-down produced empty text,
          // try to collect useful text from the full result or intermediate steps.
          // Some models (e.g., Gemini) return finishReason:'other' with empty text
          // when forced from tool-calling to text-only mode mid-task.
          if (gracefulTimeoutState.triggered && (!finalResult || finalResult === 'I was unable to complete your request due to reaching the maximum number of tool iterations.')) {
            try {
              // Try result.text (concatenation of all step texts)
              const allText = await aiResult.result.text;
              if (allText && allText.trim()) {
                finalResult = allText;
                if (this.debug) {
                  console.log(`[DEBUG] Graceful timeout: using concatenated step text (${allText.length} chars)`);
                }
              } else {
                // Last resort: collect tool result summaries as partial information
                const steps = await aiResult.result.steps;
                const toolSummaries = [];
                for (const step of (steps || [])) {
                  if (step.toolResults?.length > 0) {
                    for (const tr of step.toolResults) {
                      const resultText = typeof tr.result === 'string' ? tr.result : JSON.stringify(tr.result);
                      if (resultText && resultText.length > 0 && resultText.length < 5000) {
                        toolSummaries.push(resultText.substring(0, 2000));
                      }
                    }
                  }
                }
                if (toolSummaries.length > 0) {
                  finalResult = `The operation timed out before a complete answer could be generated. Here is the partial information gathered:\n\n${toolSummaries.join('\n\n---\n\n')}`;
                  if (this.debug) {
                    console.log(`[DEBUG] Graceful timeout: built fallback from ${toolSummaries.length} tool results`);
                  }
                } else {
                  finalResult = 'The operation timed out before enough information could be gathered to provide an answer. Please try again with a simpler query or increase the timeout.';
                }
              }
            } catch (e) {
              if (this.debug) {
                console.log(`[DEBUG] Graceful timeout fallback error: ${e.message}`);
              }
              finalResult = 'The operation timed out before enough information could be gathered to provide an answer. Please try again with a simpler query or increase the timeout.';
            }
          }

          // Update currentMessages from the result for history storage
          // The SDK manages the full message history internally
          const resultMessages = await aiResult.result.response?.messages;
          if (resultMessages) {
            // Append the AI-generated messages to our message list
            for (const msg of resultMessages) {
              currentMessages.push(msg);
            }
          }

          // Post-streamText completionPrompt fallback:
          // The stopWhen/prepareStep mechanism only fires between tool-call steps.
          // If the model answered without tool calls (or its final step had none),
          // stopWhen never gets a chance to force continuation. In that case, run
          // a second streamText pass with the completion prompt injected.
          if (this.completionPrompt && !options._completionPromptProcessed && !completionPromptInjected && finalResult) {
            completionPromptInjected = true;
            preCompletionResult = finalResult;

            if (this.debug) {
              console.log('[DEBUG] Injecting completion prompt as post-streamText follow-up pass...');
            }

            if (this.tracer) {
              this.tracer.recordEvent('completion_prompt.started', {
                'completion_prompt.original_result_length': finalResult.length
              });
            }

            const completionPromptMessage = `${this.completionPrompt}

Here is the result to review:
<result>
${finalResult}
</result>

IMPORTANT: First review ALL completed work in the conversation above before taking any action.
Double-check your response based on the criteria above. If everything looks good, respond with your previous answer exactly as-is. If your text has inaccuracies, fix the text. Only call a tool if you find a genuinely MISSING action — NEVER redo work that was already completed successfully. Respond with the COMPLETE corrected answer.`;

            currentMessages.push({ role: 'user', content: completionPromptMessage });

            const completionStreamOptions = {
              model: this.provider ? this.provider(this.model) : this.model,
              messages: this.prepareMessagesWithImages(currentMessages),
              tools,
              toolChoice: 'none', // Force text-only response — no tool calls during review
              maxTokens: maxResponseTokens,
              temperature: 0.3,
              onStepFinish: ({ toolResults, text, finishReason, usage }) => {
                if (usage) {
                  this.tokenCounter.recordUsage(usage);
                }
                if (options.onStream && text) {
                  options.onStream(text);
                }
                if (this.debug) {
                  console.log(`[DEBUG] Completion prompt step finished (reason: ${finishReason}, tools: ${toolResults?.length || 0})`);
                }
              }
            };

            const providerOpts = this._buildThinkingProviderOptions(maxResponseTokens);
            if (providerOpts) {
              completionStreamOptions.providerOptions = providerOpts;
            }

            try {
              const cpResult = await this.streamTextWithRetryAndFallback(completionStreamOptions);
              const cpFinalText = await cpResult.text;
              const cpUsage = await cpResult.usage;
              if (cpUsage) {
                this.tokenCounter.recordUsage(cpUsage, cpResult.experimental_providerMetadata);
              }

              // Append follow-up messages to conversation history
              const cpMessages = await cpResult.response?.messages;
              if (cpMessages) {
                for (const msg of cpMessages) {
                  currentMessages.push(msg);
                }
              }

              // Use updated result if non-empty, otherwise keep original
              if (cpFinalText && cpFinalText.trim().length > 0) {
                finalResult = cpFinalText;
              }

              if (this.debug) {
                console.log(`[DEBUG] Completion prompt follow-up produced ${cpFinalText?.length || 0} chars (using ${cpFinalText && cpFinalText.trim().length > 0 ? 'updated' : 'original'} result)`);
              }
            } catch (cpError) {
              if (this.debug) {
                console.log(`[DEBUG] Completion prompt follow-up failed: ${cpError.message}, keeping original result`);
              }
              // Keep original result on failure
            }
          }

          break; // Success

        } catch (error) {
          // Handle context-limit error: compact messages and retry (once)
          if (!compactionAttempted && handleContextLimitError) {
            const compactionResult = handleContextLimitError(error, currentMessages, {
              keepLastSegment: true,
              minSegmentsToKeep: 1
            });

            if (compactionResult) {
              const { messages: compactedMessages, stats } = compactionResult;

              if (stats.removed === 0) {
                console.error(`[ERROR] Context window exceeded but no messages can be compacted.`);
                finalResult = `Error: Context window limit exceeded and conversation cannot be compacted further.`;
                throw new Error(finalResult);
              }

              compactionAttempted = true;
              console.log(`[INFO] Context window limit exceeded. Compacting conversation...`);
              console.log(`[INFO] Removed ${stats.removed} messages (${stats.reductionPercent}% reduction)`);

              currentMessages = [...compactedMessages];

              if (this.tracer) {
                this.tracer.addEvent('context.compacted', {
                  'original_count': stats.originalCount,
                  'compacted_count': stats.compactedCount,
                  'reduction_percent': stats.reductionPercent,
                  'tokens_saved': stats.tokensSaved
                });
              }

              continue; // Retry with compacted messages
            }
          }

          console.error(`Error during streamText:`, error);
          finalResult = `Error: Failed to get response from AI model. ${error.message}`;
          throw new Error(finalResult);
        }
      }

      if (currentIteration >= maxIterations) {
        console.warn(`[WARN] Max tool iterations (${maxIterations}) reached for session ${this.sessionId}.`);
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

      // Log completion prompt telemetry if it was injected
      if (completionPromptInjected && this.tracer) {
        this.tracer.recordEvent('completion_prompt.completed', {
          'completion_prompt.final_result_length': finalResult?.length || 0,
          'completion_prompt.used_original': preCompletionResult && finalResult === preCompletionResult
        });
      }

      // Schema handling - validate and fix response according to provided schema
      // Skip if already formatted or in a recursive correction call
      if (options.schema && !options._schemaFormatted && !options._skipValidation) {
        try {
          // Step 1: Validate and fix Mermaid diagrams BEFORE cleaning schema
          if (!this.disableMermaidValidation) {
            if (this.debug) {
              console.log(`[DEBUG] Mermaid validation: Validating result BEFORE schema cleaning...`);
            }

            const mermaidValidation = await validateAndFixMermaidResponse(finalResult, {
              debug: this.debug,
              path: this.workspaceRoot || this.allowedFolders[0],
              provider: this.clientApiProvider,
              model: this.model,
              tracer: this.tracer
            });

            if (mermaidValidation.wasFixed) {
              finalResult = mermaidValidation.fixedResponse;
              if (this.debug) {
                console.log(`[DEBUG] Mermaid validation: Diagrams fixed`);
                if (mermaidValidation.performanceMetrics) {
                  console.log(`[DEBUG] Mermaid validation: Fixed in ${mermaidValidation.performanceMetrics.totalTimeMs}ms`);
                }
              }
            } else if (this.debug) {
              console.log(`[DEBUG] Mermaid validation: Completed (no fixes needed)`);
            }
          } else if (this.debug) {
            console.log(`[DEBUG] Mermaid validation: Skipped due to disableMermaidValidation option`);
          }

          // Step 2: Clean the schema response (remove code blocks, extract JSON)
          finalResult = cleanSchemaResponse(finalResult);

          // Step 3: Validate and potentially correct JSON responses
          if (isJsonSchema(options.schema)) {
            if (this.debug) {
              console.log(`[DEBUG] JSON validation: Starting validation process`);
              console.log(`[DEBUG] JSON validation: Response length: ${finalResult.length} chars`);
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

            // Check if the response is valid JSON but is actually a schema definition
            if (validation.isValid && isJsonSchemaDefinition(finalResult, { debug: this.debug })) {
              if (this.debug) {
                console.log(`[DEBUG] JSON validation: Response is a JSON schema definition instead of data, correcting...`);
              }

              const schemaDefinitionPrompt = createSchemaDefinitionCorrectionPrompt(
                finalResult,
                options.schema,
                0
              );

              finalResult = await this.answer(schemaDefinitionPrompt, [], {
                ...options,
                _schemaFormatted: true,
                _skipValidation: true,
                _disableTools: true,
                _completionPromptProcessed: true,
                _maxIterationsOverride: 3
              });
              finalResult = cleanSchemaResponse(finalResult);
              validation = validateJsonResponse(finalResult, { debug: this.debug, schema: options.schema });
              retryCount = 1;
            }

            // Try auto-wrapping for simple schemas before entering correction loop
            if (!validation.isValid) {
              const autoWrapped = tryAutoWrapForSimpleSchema(finalResult, options.schema, { debug: this.debug });
              if (autoWrapped) {
                if (this.debug) {
                  console.log(`[DEBUG] JSON validation: Auto-wrapped plain text for simple schema`);
                }
                finalResult = autoWrapped;
                validation = validateJsonResponse(finalResult, { debug: this.debug, schema: options.schema });
              }
            }

            // Correction loop
            while (!validation.isValid && retryCount < maxRetries) {
              if (this.debug) {
                console.log(`[DEBUG] JSON validation: Validation failed (attempt ${retryCount + 1}/${maxRetries}):`, validation.error);
              }

              let correctionPrompt;
              try {
                if (isJsonSchemaDefinition(finalResult, { debug: this.debug })) {
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
                _skipValidation: true,
                _disableTools: true,
                _completionPromptProcessed: true,
                _maxIterationsOverride: 3
              });
              finalResult = cleanSchemaResponse(finalResult);

              validation = validateJsonResponse(finalResult, { debug: this.debug, schema: options.schema });
              retryCount++;

              if (this.debug) {
                if (validation.isValid) {
                  console.log(`[DEBUG] JSON validation: Correction successful on attempt ${retryCount}`);
                } else {
                  console.log(`[DEBUG] JSON validation: Correction failed on attempt ${retryCount}: ${validation.error}`);
                }
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

            if (!validation.isValid && this.debug) {
              console.log(`[DEBUG] JSON validation: Failed after ${maxRetries} attempts: ${validation.error}`);
            } else if (validation.isValid && this.debug) {
              console.log(`[DEBUG] JSON validation: Final validation successful`);
            }
          }
        } catch (error) {
          if (this.debug) {
            console.log(`[DEBUG] Schema validation/cleanup failed: ${error.message}`);
          }
        }
      }

      // Final mermaid validation for all responses (regardless of schema)
      if (!this.disableMermaidValidation && !options._schemaFormatted) {
        try {
          if (this.debug) {
            console.log(`[DEBUG] Mermaid validation: Performing final mermaid validation on result...`);
          }
          
          const finalMermaidValidation = await validateAndFixMermaidResponse(finalResult, {
            debug: this.debug,
            path: this.workspaceRoot || this.allowedFolders[0],
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



      // Append DSL output buffer directly to response (bypasses LLM rewriting)
      // Skip during _completionPromptProcessed — only the parent answer() should append the buffer.
      // Combine _outputBuffer (from DSL output() calls) and _extractedRawBlocks (from tool results)
      // Using separate accumulators prevents the cycle described in issue #438.
      const allOutputItems = [
        ...(this._outputBuffer?.items || []),
        ...(this._extractedRawBlocks || [])
      ];
      if (allOutputItems.length > 0 && !options._schemaFormatted && !options._completionPromptProcessed) {
        const outputContent = allOutputItems.join('\n\n');
        if (options.schema) {
          // Schema response — the finalResult is JSON. Wrap output in RAW_OUTPUT
          // delimiters so clients (visor, etc.) can extract and propagate the
          // content separately from the JSON.
          finalResult = (finalResult || '') + '\n<<<RAW_OUTPUT>>>\n' + outputContent + '\n<<<END_RAW_OUTPUT>>>';
        } else {
          finalResult = (finalResult || '') + '\n\n' + outputContent;
        }
        if (options.onStream) {
          options.onStream('\n\n' + outputContent);
        }
        if (this.debug) {
          console.log(`[DEBUG] Appended ${allOutputItems.length} output items (${outputContent.length} chars) to final result${options.schema ? ' (with RAW_OUTPUT delimiters)' : ''}`);
        }
        this._outputBuffer.items = [];
        this._extractedRawBlocks = [];
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
      enableExecutePlan: this.enableExecutePlan,
      architectureFileName: this.architectureFileName,
      // Pass allowedFolders which will recompute workspaceRoot correctly
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

    // Empty attempt_complete reminders (legacy and new format)
    if (content.includes('<attempt_complete></attempt_complete>') &&
        content.includes('reuses your PREVIOUS assistant message')) {
      return true;
    }

    return false;
  }


  /**
   * Clean up resources (including MCP connections)
   */
  async cleanup() {
    // Abort any in-flight operations (delegations, streaming, etc.)
    if (!this._abortController.signal.aborted) {
      this._abortController.abort();
    }

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

    // Clean up delegation manager
    if (this.delegationManager) {
      try {
        this.delegationManager.cleanup();
        if (this.debug) {
          console.log('[DEBUG] Delegation manager cleaned up');
        }
      } catch (error) {
        console.error('Error cleaning up delegation manager:', error);
      }
    }

    // Clear history and other resources
    this.clearHistory();
  }

  /**
   * Cancel the current request and all in-flight delegations.
   * Aborts the internal AbortController so streamText, subagents,
   * and any code checking the signal will stop.
   */
  cancel() {
    this.cancelled = true;
    this._abortController.abort();
    if (this.debug) {
      console.log(`[DEBUG] Agent cancelled for session ${this.sessionId}`);
    }
  }

  /**
   * Get the abort signal for this agent.
   * Delegations and subagents should check this signal.
   * @returns {AbortSignal}
   */
  get abortSignal() {
    return this._abortController.signal;
  }
}
