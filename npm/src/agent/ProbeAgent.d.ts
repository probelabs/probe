// TypeScript definitions for ProbeAgent class
import { EventEmitter } from 'events';
import type { RetryOptions } from './RetryManager';
import type { FallbackOptions } from './FallbackManager';

/**
 * Configuration options for creating a ProbeAgent instance
 */
export interface ProbeAgentOptions {
  /** Optional session ID for the agent */
  sessionId?: string;
  /** Custom system prompt to replace the default system message */
  customPrompt?: string;
  /** Alias for customPrompt. More intuitive naming for system prompts. */
  systemPrompt?: string;
  /** Predefined prompt type (persona) */
  promptType?: 'code-explorer' | 'engineer' | 'code-review' | 'support' | 'architect';
  /** Allow the use of the 'implement' tool for code editing */
  allowEdit?: boolean;
  /** Enable the delegate tool for task distribution to subagents */
  enableDelegate?: boolean;
  /** Search directory path */
  path?: string;
  /** Force specific AI provider */
  provider?: 'anthropic' | 'openai' | 'google' | 'bedrock';
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
  /** List of allowed tool names. Use ['*'] for all tools (default), [] or null for no tools (raw AI mode), or specific tool names like ['search', 'query', 'extract']. Supports exclusion with '!' prefix (e.g., ['*', '!bash']). */
  allowedTools?: string[] | null;
  /** Convenience flag to disable all tools (equivalent to allowedTools: []). Takes precedence over allowedTools if set. */
  disableTools?: boolean;
  /** Retry configuration for handling transient API failures */
  retry?: RetryOptions;
  /** Fallback configuration for multi-provider support */
  fallback?: FallbackOptions | { auto: boolean };
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
 * Clone options for creating a new agent with shared history
 */
export interface CloneOptions {
  /** Session ID for the cloned agent (defaults to new UUID) */
  sessionId?: string;
  /** Remove internal messages (schema reminders, schema formatting prompts, mermaid fixes, etc.) */
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

  /** AI provider being used */
  readonly clientApiProvider?: string;
  
  /** Current AI model */
  readonly model?: string;

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
 * ProbeAgent Events interface
 */
export interface ProbeAgentEvents {
  on(event: 'toolCall', listener: (event: ToolCallEvent) => void): this;
  emit(event: 'toolCall', event: ToolCallEvent): boolean;
  removeListener(event: 'toolCall', listener: (event: ToolCallEvent) => void): this;
  removeAllListeners(event?: 'toolCall'): this;
}

// Default export
export { ProbeAgent as default };
