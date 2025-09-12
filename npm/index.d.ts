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
  files: string[];
  /** Path to search in */
  path?: string;
  /** Number of context lines */
  contextLines?: number;
  /** Output format */
  format?: 'markdown' | 'plain' | 'json';
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

// Default export for ES modules
export { ProbeAgent as default };