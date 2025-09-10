// ACP Tool Integration - Maps probe tools to ACP tool format
import { randomUUID } from 'crypto';
import { 
  ToolCallStatus, 
  ToolCallKind, 
  createTextContent,
  createToolCallProgress 
} from './types.js';

/**
 * ACP Tool Call represents a tool execution instance
 */
export class ACPToolCall {
  constructor(id, name, kind, params, sessionId) {
    this.id = id;
    this.name = name;
    this.kind = kind;
    this.params = params;
    this.sessionId = sessionId;
    this.status = ToolCallStatus.PENDING;
    this.startTime = Date.now();
    this.endTime = null;
    this.result = null;
    this.error = null;
  }
  
  /**
   * Update tool call status
   */
  updateStatus(status, result = null, error = null) {
    this.status = status;
    this.result = result;
    this.error = error;
    
    if (status === ToolCallStatus.COMPLETED || status === ToolCallStatus.FAILED) {
      this.endTime = Date.now();
    }
  }
  
  /**
   * Get execution duration in ms
   */
  getDuration() {
    const end = this.endTime || Date.now();
    return end - this.startTime;
  }
  
  /**
   * Serialize to JSON
   */
  toJSON() {
    return {
      id: this.id,
      name: this.name,
      kind: this.kind,
      params: this.params,
      sessionId: this.sessionId,
      status: this.status,
      startTime: this.startTime,
      endTime: this.endTime,
      duration: this.getDuration(),
      result: this.result,
      error: this.error
    };
  }
}

/**
 * ACP Tool Manager - manages tool execution and lifecycle
 */
export class ACPToolManager {
  constructor(server, probeAgent) {
    this.server = server;
    this.probeAgent = probeAgent;
    this.activeCalls = new Map();
    this.debug = server.options.debug;
  }
  
  /**
   * Execute a tool with ACP lifecycle tracking
   */
  async executeToolCall(sessionId, toolName, params) {
    const toolCallId = randomUUID();
    const kind = this.getToolKind(toolName);
    
    const toolCall = new ACPToolCall(toolCallId, toolName, kind, params, sessionId);
    this.activeCalls.set(toolCallId, toolCall);
    
    if (this.debug) {
      console.error(`[ACP] Starting tool call: ${toolName} (${toolCallId})`);
    }
    
    // Send pending notification
    this.server.sendToolCallProgress(
      sessionId, 
      toolCallId, 
      ToolCallStatus.PENDING
    );
    
    try {
      // Update to in progress
      toolCall.updateStatus(ToolCallStatus.IN_PROGRESS);
      this.server.sendToolCallProgress(
        sessionId, 
        toolCallId, 
        ToolCallStatus.IN_PROGRESS
      );
      
      // Execute the actual tool
      const result = await this.executeProbeTool(toolName, params);
      
      // Update to completed
      toolCall.updateStatus(ToolCallStatus.COMPLETED, result);
      this.server.sendToolCallProgress(
        sessionId, 
        toolCallId, 
        ToolCallStatus.COMPLETED, 
        result
      );
      
      if (this.debug) {
        console.error(`[ACP] Tool call completed: ${toolName} (${toolCall.getDuration()}ms)`);
      }
      
      return result;
      
    } catch (error) {
      // Update to failed
      toolCall.updateStatus(ToolCallStatus.FAILED, null, error.message);
      this.server.sendToolCallProgress(
        sessionId, 
        toolCallId, 
        ToolCallStatus.FAILED, 
        null, 
        error.message
      );
      
      if (this.debug) {
        console.error(`[ACP] Tool call failed: ${toolName}`, error);
      }
      
      throw error;
      
    } finally {
      // Clean up completed calls after a delay
      setTimeout(() => {
        this.activeCalls.delete(toolCallId);
      }, 30000); // Keep for 30 seconds for status queries
    }
  }
  
  /**
   * Get tool kind based on tool name
   */
  getToolKind(toolName) {
    switch (toolName) {
      case 'search':
        return ToolCallKind.search;
      case 'query':
        return ToolCallKind.query;
      case 'extract':
        return ToolCallKind.extract;
      case 'implement':
        return ToolCallKind.edit;
      default:
        return ToolCallKind.execute;
    }
  }
  
  /**
   * Execute a probe tool
   */
  async executeProbeTool(toolName, params) {
    // Get the tool from the probe agent
    const tools = this.probeAgent.wrappedTools;
    
    switch (toolName) {
      case 'search':
        if (!tools.searchToolInstance) {
          throw new Error('Search tool not available');
        }
        return await tools.searchToolInstance.execute({
          ...params,
          sessionId: this.probeAgent.sessionId
        });
        
      case 'query':
        if (!tools.queryToolInstance) {
          throw new Error('Query tool not available');
        }
        return await tools.queryToolInstance.execute({
          ...params,
          sessionId: this.probeAgent.sessionId
        });
        
      case 'extract':
        if (!tools.extractToolInstance) {
          throw new Error('Extract tool not available');
        }
        return await tools.extractToolInstance.execute({
          ...params,
          sessionId: this.probeAgent.sessionId
        });
        
      default:
        throw new Error(`Unknown tool: ${toolName}`);
    }
  }
  
  /**
   * Get tool call status
   */
  getToolCallStatus(toolCallId) {
    const toolCall = this.activeCalls.get(toolCallId);
    return toolCall ? toolCall.toJSON() : null;
  }
  
  /**
   * Get all active tool calls for a session
   */
  getActiveToolCalls(sessionId) {
    const calls = [];
    for (const toolCall of this.activeCalls.values()) {
      if (toolCall.sessionId === sessionId) {
        calls.push(toolCall.toJSON());
      }
    }
    return calls;
  }
  
  /**
   * Cancel all tool calls for a session
   */
  cancelSessionToolCalls(sessionId) {
    for (const [id, toolCall] of this.activeCalls) {
      if (toolCall.sessionId === sessionId && 
          (toolCall.status === ToolCallStatus.PENDING || 
           toolCall.status === ToolCallStatus.IN_PROGRESS)) {
        
        toolCall.updateStatus(ToolCallStatus.FAILED, null, 'Cancelled');
        this.server.sendToolCallProgress(
          sessionId, 
          id, 
          ToolCallStatus.FAILED, 
          null, 
          'Cancelled'
        );
      }
    }
  }
  
  /**
   * Get tool definitions for capabilities
   */
  static getToolDefinitions() {
    return [
      {
        name: 'search',
        description: 'Search for code patterns and content using flexible text search with stemming and stopword removal. Supports regex patterns and elastic search query syntax.',
        kind: ToolCallKind.search,
        parameters: {
          type: 'object',
          properties: {
            query: {
              type: 'string',
              description: 'Search query using elastic search syntax. Supports logical operators (AND, OR, NOT), quotes for exact matches, field specifiers, and regex patterns.'
            },
            path: {
              type: 'string',
              description: 'Directory to search in (defaults to current working directory)'
            },
            max_results: {
              type: 'number',
              description: 'Maximum number of results to return (default: 10)'
            },
            allow_tests: {
              type: 'boolean',
              description: 'Include test files in results (default: false)'
            }
          },
          required: ['query']
        }
      },
      {
        name: 'query',
        description: 'Perform structural queries using AST patterns to find specific code structures like functions, classes, or methods.',
        kind: ToolCallKind.query,
        parameters: {
          type: 'object',
          properties: {
            pattern: {
              type: 'string',
              description: 'AST-grep pattern to search for. Examples: "fn $NAME($$$PARAMS) $$$BODY" for Rust functions, "def $NAME($$$PARAMS): $$$BODY" for Python functions.'
            },
            path: {
              type: 'string',
              description: 'Directory to search in (defaults to current working directory)'
            },
            language: {
              type: 'string',
              description: 'Programming language to search in (rust, javascript, python, go, etc.)'
            },
            max_results: {
              type: 'number',
              description: 'Maximum number of results to return (default: 10)'
            }
          },
          required: ['pattern']
        }
      },
      {
        name: 'extract',
        description: 'Extract specific code blocks from files based on file paths and optional line numbers.',
        kind: ToolCallKind.extract,
        parameters: {
          type: 'object',
          properties: {
            files: {
              type: 'array',
              items: { type: 'string' },
              description: 'Array of file paths or file:line specifications to extract from'
            },
            context_lines: {
              type: 'number',
              description: 'Number of context lines to include before and after (default: 0)'
            },
            allow_tests: {
              type: 'boolean',
              description: 'Allow test files in results (default: false)'
            },
            format: {
              type: 'string',
              enum: ['plain', 'markdown', 'json'],
              description: 'Output format (default: markdown)'
            }
          },
          required: ['files']
        }
      }
    ];
  }
}