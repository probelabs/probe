// TypeScript definitions for ProbeAgent SDK
import { EventEmitter } from 'events';

/**
 * Configuration options for creating a ProbeAgent instance
 */
export interface ProbeAgentOptions {
  /** Optional session ID for the agent */
  sessionId?: string;
  /** Custom system prompt to replace the default system message */
  customPrompt?: string;
  /** Predefined prompt type (persona) */
  promptType?: 'code-explorer' | 'engineer' | 'code-review' | 'support' | 'architect';
  /** Allow the use of the 'implement' tool for code editing */
  allowEdit?: boolean;
  /** Search directory path */
  path?: string;
  /** Force specific AI provider */
  provider?: 'anthropic' | 'openai' | 'google';
  /** Override model name */
  model?: string;
  /** Enable debug mode */
  debug?: boolean;
  /** Optional telemetry tracer instance */
  tracer?: any;
  /** Enable MCP (Model Context Protocol) tool integration */
  enableMcp?: boolean;
  /** Path to MCP configuration file */
  mcpConfigPath?: string;
  /** MCP configuration object (overrides mcpConfigPath) */
  mcpConfig?: any;
  /** @deprecated Use mcpConfig instead */
  mcpServers?: any[];
  /** Pluggable storage adapter for conversation history */
  storageAdapter?: StorageAdapter;
  /** Hook callbacks for event-driven integration */
  hooks?: Record<string, (data: any) => void | Promise<void>>;
}

/**
 * Tool execution event data
 */
export interface ToolCallEvent {
  /** Unique tool call identifier */
  id: string;
  /** Name of the tool being called */
  name: string;
  /** Current execution status */
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  /** Tool parameters */
  params?: any;
  /** Tool execution result (when completed) */
  result?: any;
  /** Error information (when failed) */
  error?: string;
  /** Session ID */
  sessionId?: string;
  /** Execution start time */
  startTime?: number;
  /** Execution end time */
  endTime?: number;
  /** Execution duration in milliseconds */
  duration?: number;
}

/**
 * Token usage statistics
 */
export interface TokenUsage {
  /** Size of the context window */
  contextWindow?: number;
  /** Request tokens used */
  request?: number;
  /** Response tokens generated */
  response?: number;
  /** Total tokens (request + response) */
  total?: number;
  /** Cache read tokens */
  cacheRead?: number;
  /** Cache write tokens */
  cacheWrite?: number;
  /** Total request tokens across all calls */
  totalRequest?: number;
  /** Total response tokens across all calls */
  totalResponse?: number;
  /** Total tokens across all calls */
  totalTokens?: number;
  /** Total cache read tokens across all calls */
  totalCacheRead?: number;
  /** Total cache write tokens across all calls */
  totalCacheWrite?: number;
}

/**
 * Chat message structure
 */
export interface ChatMessage {
  /** Message role */
  role: 'user' | 'assistant' | 'system';
  /** Message content */
  content: string;
  /** Optional message metadata */
  metadata?: any;
}

/**
 * Storage adapter base class for pluggable history storage
 */
export declare class StorageAdapter {
  /**
   * Load conversation history for a session
   */
  loadHistory(sessionId: string): Promise<ChatMessage[]>;

  /**
   * Save a message to storage
   */
  saveMessage(sessionId: string, message: ChatMessage): Promise<void>;

  /**
   * Clear history for a session
   */
  clearHistory(sessionId: string): Promise<void>;

  /**
   * Optional: Get session metadata
   */
  getSessionMetadata(sessionId: string): Promise<any | null>;

  /**
   * Optional: Update session activity timestamp
   */
  updateSessionActivity(sessionId: string): Promise<void>;
}

/**
 * Default in-memory storage adapter
 */
export declare class InMemoryStorageAdapter extends StorageAdapter {
  constructor();
}

/**
 * Hook types for ProbeAgent event system
 */
export declare const HOOK_TYPES: {
  readonly AGENT_INITIALIZED: 'agent:initialized';
  readonly AGENT_CLEANUP: 'agent:cleanup';
  readonly MESSAGE_USER: 'message:user';
  readonly MESSAGE_ASSISTANT: 'message:assistant';
  readonly MESSAGE_SYSTEM: 'message:system';
  readonly TOOL_START: 'tool:start';
  readonly TOOL_END: 'tool:end';
  readonly TOOL_ERROR: 'tool:error';
  readonly AI_STREAM_START: 'ai:stream:start';
  readonly AI_STREAM_DELTA: 'ai:stream:delta';
  readonly AI_STREAM_END: 'ai:stream:end';
  readonly STORAGE_LOAD: 'storage:load';
  readonly STORAGE_SAVE: 'storage:save';
  readonly STORAGE_CLEAR: 'storage:clear';
  readonly ITERATION_START: 'iteration:start';
  readonly ITERATION_END: 'iteration:end';
};

/**
 * Hook manager for event-driven integration
 */
export declare class HookManager {
  constructor();

  /**
   * Register a hook callback
   */
  on(hookName: string, callback: (data: any) => void | Promise<void>): () => void;

  /**
   * Register a one-time hook callback
   */
  once(hookName: string, callback: (data: any) => void | Promise<void>): void;

  /**
   * Unregister a hook callback
   */
  off(hookName: string, callback: (data: any) => void | Promise<void>): void;

  /**
   * Emit a hook event
   */
  emit(hookName: string, data: any): Promise<void>;

  /**
   * Clear all hooks or hooks for a specific event
   */
  clear(hookName?: string): void;
}

/**
 * Answer options
 */
export interface AnswerOptions {
  /** Response schema for structured output */
  schema?: string;
  /** Additional context or constraints */
  context?: string;
  /** Maximum number of tool iterations */
  maxIterations?: number;
}

/**
 * Clone options for creating a new agent with shared history
 */
export interface CloneOptions {
  /** Session ID for the cloned agent (defaults to new UUID) */
  sessionId?: string;
  /** Remove internal messages (schema reminders, mermaid fixes, etc.) */
  stripInternalMessages?: boolean;
  /** Keep the system message in cloned history */
  keepSystemMessage?: boolean;
  /** Deep copy messages to prevent mutations */
  deepCopy?: boolean;
  /** Override any ProbeAgent constructor options */
  overrides?: Partial<ProbeAgentOptions>;
}

/**
 * ProbeAgent class - AI-powered code exploration and interaction
 */
export declare class ProbeAgent {
  /** Unique session identifier */
  readonly sessionId: string;
  
  /** Current chat history */
  history: ChatMessage[];
  
  /** Event emitter for tool execution updates */
  readonly events: EventEmitter & ProbeAgentEvents;
  
  /** Whether the agent allows code editing */
  readonly allowEdit: boolean;
  
  /** Allowed search folders */
  readonly allowedFolders: string[];
  
  /** Debug mode status */
  readonly debug: boolean;
  
  /** Whether operations have been cancelled */
  cancelled: boolean;

  /**
   * Create a new ProbeAgent instance
   */
  constructor(options?: ProbeAgentOptions);

  /**
   * Answer a question with optional image attachments
   * @param message - The question or prompt
   * @param images - Optional array of image data or paths
   * @param options - Additional options for the response
   * @returns Promise resolving to the AI response
   */
  answer(message: string, images?: any[], options?: AnswerOptions): Promise<string>;

  /**
   * Get token usage statistics
   * @returns Current token usage information
   */
  getTokenUsage(): TokenUsage;

  /**
   * Cancel any ongoing operations
   */
  cancel(): void;

  /**
   * Clear the conversation history
   */
  clearHistory(): void;

  /**
   * Add a message to the conversation history
   * @param message - Message to add
   */
  addMessage(message: ChatMessage): void;

  /**
   * Set the conversation history
   * @param messages - Array of chat messages
   */
  setHistory(messages: ChatMessage[]): void;

  /**
   * Clone this agent's session to create a new agent with shared conversation history
   * @param options - Clone options
   * @returns New agent instance with cloned history
   */
  clone(options?: CloneOptions): ProbeAgent;
}

/**
 * Search tool configuration options
 */
export interface SearchOptions {
  /** Session identifier */
  sessionId?: string;
  /** Debug mode */
  debug?: boolean;
  /** Default search path */
  defaultPath?: string;
  /** Allowed search folders */
  allowedFolders?: string[];
}

/**
 * Search parameters
 */
export interface SearchParams {
  /** Search query */
  query: string;
  /** Path to search in */
  path?: string;
  /** Maximum number of results */
  maxResults?: number;
  /** Search timeout in seconds */
  timeout?: number;
  /** Allow test files in results */
  allowTests?: boolean;
  /** Session ID */
  sessionId?: string;
}

/**
 * Query tool parameters for structural code search
 */
export interface QueryParams {
  /** AST-grep pattern */
  pattern: string;
  /** Path to search in */
  path?: string;
  /** Programming language */
  language?: string;
  /** Maximum number of results */
  maxResults?: number;
  /** Session ID */
  sessionId?: string;
}

/**
 * Extract tool parameters
 */
export interface ExtractParams {
  /** Files and line numbers or symbols to extract */
  files?: string[];
  /** Path to a file containing unstructured text to extract file paths from */
  inputFile?: string;
  /** Content to pipe to stdin (e.g., git diff output). Alternative to inputFile or files. */
  content?: string | Buffer;
  /** Path to search in */
  path?: string;
  /** Number of context lines */
  contextLines?: number;
  /** Output format */
  format?: 'markdown' | 'plain' | 'json' | 'xml' | 'color' | 'outline-xml' | 'outline-diff';
  /** Include test files */
  allowTests?: boolean;
  /** Return results as parsed JSON instead of string */
  json?: boolean;
  /** Session ID */
  sessionId?: string;
}

/**
 * Tool execution result
 */
export interface ToolResult {
  /** Whether the operation was successful */
  success: boolean;
  /** Result data */
  result?: any;
  /** Error message if failed */
  error?: string;
  /** Execution metadata */
  metadata?: any;
}

/**
 * Search tool function type
 */
export type SearchTool = (options?: SearchOptions) => {
  execute(params: SearchParams): Promise<ToolResult>;
};

/**
 * Query tool function type
 */
export type QueryTool = (options?: SearchOptions) => {
  execute(params: QueryParams): Promise<ToolResult>;
};

/**
 * Extract tool function type
 */
export type ExtractTool = (options?: SearchOptions) => {
  execute(params: ExtractParams): Promise<ToolResult>;
};

/**
 * Main probe search function
 */
export declare function search(
  query: string,
  path?: string,
  options?: {
    maxResults?: number;
    timeout?: number;
    allowTests?: boolean;
  }
): Promise<any>;

/**
 * Structural code query using ast-grep
 */
export declare function query(
  pattern: string,
  path?: string,
  options?: {
    language?: string;
    maxResults?: number;
  }
): Promise<any>;

/**
 * Standard grep-style search that works across multiple operating systems
 *
 * Use this for searching non-code files (logs, config files, text files, etc.)
 * that are not supported by probe's semantic search. For code files, prefer
 * using the search() function which provides AST-aware semantic search.
 *
 * @param options - Grep options
 * @param options.pattern - Regular expression pattern to search for
 * @param options.paths - Path or array of paths to search in
 * @param options.ignoreCase - Case-insensitive search (-i flag)
 * @param options.lineNumbers - Show line numbers in output (-n flag)
 * @param options.count - Only show count of matches per file (-c flag)
 * @param options.filesWithMatches - Only show filenames that contain matches (-l flag)
 * @param options.filesWithoutMatches - Only show filenames that do not contain matches (-L flag)
 * @param options.invertMatch - Invert match: show lines that do NOT match (-v flag)
 * @param options.beforeContext - Number of lines of context before match (-B flag)
 * @param options.afterContext - Number of lines of context after match (-A flag)
 * @param options.context - Number of lines of context before and after match (-C flag)
 * @param options.noGitignore - Do not respect .gitignore files (--no-gitignore flag)
 * @param options.color - Colorize output: 'always', 'never', 'auto' (--color flag)
 * @param options.maxCount - Stop reading a file after N matching lines (-m flag)
 * @returns Promise resolving to grep results as string
 */
export declare function grep(options: {
  pattern: string;
  paths: string | string[];
  ignoreCase?: boolean;
  lineNumbers?: boolean;
  count?: boolean;
  filesWithMatches?: boolean;
  filesWithoutMatches?: boolean;
  invertMatch?: boolean;
  beforeContext?: number;
  afterContext?: number;
  context?: number;
  noGitignore?: boolean;
  color?: 'always' | 'never' | 'auto';
  maxCount?: number;
  binaryOptions?: {
    forceDownload?: boolean;
    version?: string;
  };
}): Promise<string>;

/**
 * Extract code blocks from files
 */
export declare function extract(
  files: string[],
  path?: string,
  options?: {
    contextLines?: number;
    format?: 'markdown' | 'plain' | 'json';
  }
): Promise<any>;

/**
 * Create search tool instance
 */
export declare function searchTool(options?: SearchOptions): ReturnType<SearchTool>;

/**
 * Create query tool instance
 */
export declare function queryTool(options?: SearchOptions): ReturnType<QueryTool>;

/**
 * Create extract tool instance
 */
export declare function extractTool(options?: SearchOptions): ReturnType<ExtractTool>;

/**
 * Get the path to the probe binary
 */
export declare function getBinaryPath(): string;

/**
 * Set the path to the probe binary
 */
export declare function setBinaryPath(path: string): void;

/**
 * List files by directory level
 */
export declare function listFilesByLevel(
  path?: string,
  options?: {
    maxLevel?: number;
    includeHidden?: boolean;
  }
): Promise<any>;

/**
 * Default system message for AI interactions
 */
export declare const DEFAULT_SYSTEM_MESSAGE: string;

/**
 * Schema definitions
 */
export declare const searchSchema: any;
export declare const querySchema: any;
export declare const extractSchema: any;
export declare const attemptCompletionSchema: any;

/**
 * Tool definitions for AI frameworks
 */
export declare const searchToolDefinition: any;
export declare const queryToolDefinition: any;
export declare const extractToolDefinition: any;
export declare const attemptCompletionToolDefinition: any;

/**
 * Parse XML tool calls
 */
export declare function parseXmlToolCall(xmlString: string): any;

/**
 * Legacy tools object (deprecated - use individual tool functions instead)
 * @deprecated Use searchTool, queryTool, extractTool functions instead
 */
export declare const tools: {
  search: ReturnType<SearchTool>;
  query: ReturnType<QueryTool>;
  extract: ReturnType<ExtractTool>;
};

/**
 * ProbeAgent Events interface
 */
export interface ProbeAgentEvents {
  on(event: 'toolCall', listener: (event: ToolCallEvent) => void): this;
  emit(event: 'toolCall', event: ToolCallEvent): boolean;
  removeListener(event: 'toolCall', listener: (event: ToolCallEvent) => void): this;
  removeAllListeners(event?: 'toolCall'): this;
}

/**
 * Simple telemetry configuration (no OpenTelemetry dependencies)
 */
export interface SimpleTelemetryOptions {
  /** Enable console logging */
  enableConsole?: boolean;
  /** Enable file logging */
  enableFile?: boolean;
  /** File path for logs */
  filePath?: string;
}

/**
 * Simple telemetry class for basic tracing without OpenTelemetry
 */
export declare class SimpleTelemetry {
  constructor(options?: SimpleTelemetryOptions);
  log(message: string, data?: any): void;
  flush(): Promise<void>;
  shutdown(): Promise<void>;
}

/**
 * Simple application tracer for basic operations
 */
export declare class SimpleAppTracer {
  constructor(telemetry?: SimpleTelemetry, sessionId?: string);
  isEnabled(): boolean;
  log(operation: string, data?: any): void;
  flush(): Promise<void>;
  shutdown(): Promise<void>;
}

/**
 * Initialize simple telemetry from options
 */
export declare function initializeSimpleTelemetryFromOptions(options: any): SimpleTelemetry;

/**
 * Full OpenTelemetry configuration options
 */
export interface TelemetryConfigOptions {
  /** Service name for tracing */
  serviceName?: string;
  /** Service version */
  serviceVersion?: string;
  /** Enable file export */
  enableFile?: boolean;
  /** Enable remote OTLP export */
  enableRemote?: boolean;
  /** Enable console export */
  enableConsole?: boolean;
  /** File path for trace export */
  filePath?: string;
  /** Remote OTLP endpoint URL */
  remoteEndpoint?: string;
}

/**
 * Full OpenTelemetry configuration class
 */
export declare class TelemetryConfig {
  constructor(options?: TelemetryConfigOptions);

  /** Initialize the OpenTelemetry SDK */
  initialize(): void;

  /** Get the tracer instance */
  getTracer(): any;

  /** Create a span with attributes */
  createSpan(name: string, attributes?: Record<string, any>): any;

  /** Wrap a function with automatic span creation */
  wrapFunction(name: string, fn: Function, attributes?: Record<string, any>): Function;

  /** Force flush all pending spans */
  forceFlush(): Promise<void>;

  /** Shutdown telemetry */
  shutdown(): Promise<void>;
}

/**
 * Application-specific tracing layer for AI operations
 */
export declare class AppTracer {
  constructor(telemetryConfig?: TelemetryConfig, sessionId?: string);

  /** Check if tracing is enabled */
  isEnabled(): boolean;

  /** Create a root span for the agent session */
  createSessionSpan(attributes?: Record<string, any>): any;

  /** Create a span for AI model requests */
  createAISpan(modelName: string, provider: string, attributes?: Record<string, any>): any;

  /** Create a span for tool calls */
  createToolSpan(toolName: string, attributes?: Record<string, any>): any;

  /** Create a span for code search operations */
  createSearchSpan(query: string, attributes?: Record<string, any>): any;

  /** Create a span for code extraction operations */
  createExtractSpan(files: string | string[], attributes?: Record<string, any>): any;

  /** Create a span for agent iterations */
  createIterationSpan(iteration: number, attributes?: Record<string, any>): any;

  /** Create a span for delegation operations */
  createDelegationSpan(task: string, attributes?: Record<string, any>): any;

  /** Create a span for JSON validation operations */
  createJsonValidationSpan(responseLength: number, attributes?: Record<string, any>): any;

  /** Create a span for Mermaid validation operations */
  createMermaidValidationSpan(diagramCount: number, attributes?: Record<string, any>): any;

  /** Create a span for schema processing operations */
  createSchemaProcessingSpan(schemaType: string, attributes?: Record<string, any>): any;

  /** Record delegation events */
  recordDelegationEvent(eventType: string, data?: Record<string, any>): void;

  /** Record JSON validation events */
  recordJsonValidationEvent(eventType: string, data?: Record<string, any>): void;

  /** Record Mermaid validation events */
  recordMermaidValidationEvent(eventType: string, data?: Record<string, any>): void;

  /** Add an event to the current span */
  addEvent(name: string, attributes?: Record<string, any>): void;

  /** Set attributes on the current span */
  setAttributes(attributes: Record<string, any>): void;

  /** Wrap a function with automatic span creation */
  wrapFunction(spanName: string, fn: Function, attributes?: Record<string, any>): Function;

  /** Execute a function within a span context */
  withSpan(spanName: string, fn: Function, attributes?: Record<string, any>): Promise<any>;

  /** Force flush all pending spans */
  flush(): Promise<void>;

  /** Shutdown tracing */
  shutdown(): Promise<void>;
}

/**
 * Initialize full OpenTelemetry telemetry from options
 */
export declare function initializeTelemetryFromOptions(options: any): TelemetryConfig;

// Default export for ES modules
export { ProbeAgent as default };