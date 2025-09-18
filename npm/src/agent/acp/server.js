// ACP Server - Main server implementation for Agent Client Protocol
import { randomUUID } from 'crypto';
import { ACPConnection } from './connection.js';
import { ProbeAgent } from '../ProbeAgent.js';
import {
  ACP_PROTOCOL_VERSION,
  RequestMethod,
  NotificationMethod,
  ToolCallStatus,
  ToolCallKind,
  ErrorCode,
  SessionMode,
  createTextContent,
  createToolCallProgress
} from './types.js';

/**
 * ACP Session represents a conversation context
 */
class ACPSession {
  constructor(id, mode = SessionMode.NORMAL) {
    this.id = id;
    this.mode = mode;
    this.agent = null;
    this.history = [];
    this.toolCalls = new Map();
    this.createdAt = new Date().toISOString();
    this.updatedAt = this.createdAt;
  }
  
  /**
   * Get or create ProbeAgent for this session
   */
  getAgent(config = {}) {
    if (!this.agent) {
      this.agent = new ProbeAgent({
        sessionId: this.id,
        ...config
      });
    }
    return this.agent;
  }
  
  /**
   * Update session timestamp
   */
  touch() {
    this.updatedAt = new Date().toISOString();
  }
  
  /**
   * Serialize session state
   */
  toJSON() {
    return {
      id: this.id,
      mode: this.mode,
      historyLength: this.history.length,
      toolCallsCount: this.toolCalls.size,
      createdAt: this.createdAt,
      updatedAt: this.updatedAt
    };
  }
}

/**
 * ACP Server - handles Agent Client Protocol communication
 */
export class ACPServer {
  constructor(options = {}) {
    this.options = {
      debug: process.env.DEBUG === '1',
      provider: options.provider || null,
      model: options.model || null,
      path: options.path || process.cwd(),
      allowEdit: options.allowEdit || false,
      ...options
    };
    
    this.connection = null;
    this.sessions = new Map();
    this.capabilities = this.getCapabilities();
    this.initialized = false;
    
    if (this.options.debug) {
      console.error('[ACP] Server created with options:', this.options);
    }
  }
  
  /**
   * Get server capabilities
   */
  getCapabilities() {
    return {
      tools: [
        {
          name: 'search',
          description: 'Search for code patterns and content in the repository',
          kind: ToolCallKind.search
        },
        {
          name: 'query',
          description: 'Perform structural queries using AST patterns',
          kind: ToolCallKind.query
        },
        {
          name: 'extract',
          description: 'Extract specific code blocks from files',
          kind: ToolCallKind.extract
        }
      ],
      sessionManagement: true,
      streaming: true,
      permissions: this.options.allowEdit
    };
  }
  
  /**
   * Start the ACP server
   */
  async start() {
    this.connection = new ACPConnection(process.stdin, process.stdout);
    
    // Set up message handlers
    this.connection.on('request', this.handleRequest.bind(this));
    this.connection.on('notification', this.handleNotification.bind(this));
    this.connection.on('error', this.handleError.bind(this));
    this.connection.on('disconnect', this.handleDisconnect.bind(this));
    
    // Start the connection
    this.connection.start();
    
    if (this.options.debug) {
      console.error('[ACP] Server started and listening for messages');
    }
  }
  
  /**
   * Handle incoming requests
   */
  async handleRequest(message) {
    const { method, params, id } = message;
    
    try {
      let result;
      
      switch (method) {
        case RequestMethod.INITIALIZE:
          result = await this.handleInitialize(params);
          break;
          
        case RequestMethod.NEW_SESSION:
          result = await this.handleNewSession(params);
          break;
          
        case RequestMethod.LOAD_SESSION:
          result = await this.handleLoadSession(params);
          break;
          
        case RequestMethod.SET_SESSION_MODE:
          result = await this.handleSetSessionMode(params);
          break;
          
        case RequestMethod.PROMPT:
          result = await this.handlePrompt(params);
          break;
          
        case RequestMethod.CANCEL:
          result = await this.handleCancel(params);
          break;
          
        default:
          throw new Error(`Unknown method: ${method}`);
      }
      
      this.connection.sendResponse(id, result);
      
    } catch (error) {
      if (this.options.debug) {
        console.error(`[ACP] Error handling request ${method}:`, error);
      }
      
      let errorCode = ErrorCode.INTERNAL_ERROR;
      if (error.message.includes('Unknown method')) {
        errorCode = ErrorCode.METHOD_NOT_FOUND;
      } else if (error.message.includes('Invalid params')) {
        errorCode = ErrorCode.INVALID_PARAMS;
      }
      
      this.connection.sendError(id, errorCode, error.message);
    }
  }
  
  /**
   * Handle notifications
   */
  async handleNotification(message) {
    const { method, params } = message;
    
    if (this.options.debug) {
      console.error(`[ACP] Received notification: ${method}`, params);
    }
    
    // Handle notifications here if needed
    // For now, just log them
  }
  
  /**
   * Handle initialize request
   */
  async handleInitialize(params) {
    if (!params || !params.protocolVersion) {
      throw new Error('Invalid params: protocolVersion required');
    }
    
    if (params.protocolVersion !== ACP_PROTOCOL_VERSION) {
      throw new Error(`Unsupported protocol version: ${params.protocolVersion}`);
    }
    
    this.initialized = true;
    
    if (this.options.debug) {
      console.error('[ACP] Initialized with protocol version:', params.protocolVersion);
    }
    
    return {
      protocolVersion: ACP_PROTOCOL_VERSION,
      serverInfo: {
        name: 'probe-agent-acp',
        version: '1.0.0',
        description: 'Probe AI agent with code search capabilities'
      },
      capabilities: this.capabilities
    };
  }
  
  /**
   * Handle new session request
   */
  async handleNewSession(params) {
    const sessionId = params?.sessionId || randomUUID();
    const mode = params?.mode || SessionMode.NORMAL;
    
    const session = new ACPSession(sessionId, mode);
    this.sessions.set(sessionId, session);
    
    if (this.options.debug) {
      console.error(`[ACP] Created new session: ${sessionId} (mode: ${mode})`);
    }
    
    return {
      sessionId,
      mode,
      createdAt: session.createdAt
    };
  }
  
  /**
   * Handle load session request
   */
  async handleLoadSession(params) {
    if (!params || !params.sessionId) {
      throw new Error('Invalid params: sessionId required');
    }
    
    const session = this.sessions.get(params.sessionId);
    if (!session) {
      throw new Error(`Session not found: ${params.sessionId}`);
    }
    
    if (this.options.debug) {
      console.error(`[ACP] Loaded session: ${params.sessionId}`);
    }
    
    return session.toJSON();
  }
  
  /**
   * Handle set session mode request
   */
  async handleSetSessionMode(params) {
    if (!params || !params.sessionId || !params.mode) {
      throw new Error('Invalid params: sessionId and mode required');
    }
    
    const session = this.sessions.get(params.sessionId);
    if (!session) {
      throw new Error(`Session not found: ${params.sessionId}`);
    }
    
    session.mode = params.mode;
    session.touch();
    
    if (this.options.debug) {
      console.error(`[ACP] Set session mode: ${params.sessionId} -> ${params.mode}`);
    }
    
    // Notify about session update
    if (this.connection) {
      this.connection.sendNotification(NotificationMethod.SESSION_UPDATED, {
        sessionId: params.sessionId,
        mode: params.mode
      });
    }
    
    return { success: true };
  }
  
  /**
   * Handle prompt request - main AI interaction
   */
  async handlePrompt(params) {
    if (!params || !params.sessionId || !params.message) {
      throw new Error('Invalid params: sessionId and message required');
    }
    
    const session = this.sessions.get(params.sessionId);
    if (!session) {
      throw new Error(`Session not found: ${params.sessionId}`);
    }
    
    session.touch();
    
    // Get or create ProbeAgent for this session
    const agent = session.getAgent({
      path: this.options.path,
      provider: this.options.provider,
      model: this.options.model,
      allowEdit: this.options.allowEdit,
      debug: this.options.debug
    });
    
    if (this.options.debug) {
      console.error(`[ACP] Processing prompt for session ${params.sessionId}:`, params.message.substring(0, 100));
    }
    
    try {
      // Process the message with the ProbeAgent
      const response = await agent.answer(params.message);
      
      // Add to session history
      session.history.push(
        { role: 'user', content: params.message, timestamp: new Date().toISOString() },
        { role: 'assistant', content: response, timestamp: new Date().toISOString() }
      );
      
      // Send the response as content blocks
      return {
        content: [createTextContent(response)],
        sessionId: params.sessionId,
        timestamp: new Date().toISOString()
      };
      
    } catch (error) {
      if (this.options.debug) {
        console.error(`[ACP] Error processing prompt:`, error);
      }
      
      // Return error as content
      return {
        content: [createTextContent(`Error: ${error.message}`)],
        sessionId: params.sessionId,
        timestamp: new Date().toISOString(),
        error: true
      };
    }
  }
  
  /**
   * Handle cancel request
   */
  async handleCancel(params) {
    if (!params || !params.sessionId) {
      throw new Error('Invalid params: sessionId required');
    }
    
    const session = this.sessions.get(params.sessionId);
    if (session && session.agent) {
      session.agent.cancel();
    }
    
    if (this.options.debug) {
      console.error(`[ACP] Cancelled operations for session: ${params.sessionId}`);
    }
    
    return { success: true };
  }
  
  /**
   * Handle connection errors
   */
  handleError(error) {
    if (this.options.debug) {
      console.error('[ACP] Connection error:', error);
    }
  }
  
  /**
   * Handle disconnection
   */
  handleDisconnect() {
    if (this.options.debug) {
      console.error('[ACP] Client disconnected');
    }
    
    // Clean up sessions and resources
    for (const session of this.sessions.values()) {
      if (session.agent) {
        session.agent.cancel();
      }
    }
    
    this.sessions.clear();
  }
  
  /**
   * Send tool call progress notification
   */
  sendToolCallProgress(sessionId, toolCallId, status, result = null, error = null) {
    const progress = createToolCallProgress(toolCallId, status, result, error);
    
    this.connection.sendNotification(NotificationMethod.TOOL_CALL_PROGRESS, {
      sessionId,
      ...progress
    });
  }
  
  /**
   * Send message chunk for streaming
   */
  sendMessageChunk(sessionId, chunk) {
    this.connection.sendNotification(NotificationMethod.MESSAGE_CHUNK, {
      sessionId,
      chunk
    });
  }
  
  /**
   * Get session statistics
   */
  getStats() {
    return {
      sessions: this.sessions.size,
      initialized: this.initialized,
      capabilities: this.capabilities
    };
  }
}