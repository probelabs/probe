// ACP Connection - handles JSON-RPC 2.0 communication over stdio
import { EventEmitter } from 'events';
import { validateMessage, createResponse, createError, ErrorCode } from './types.js';

/**
 * ACP Connection class for handling bidirectional JSON-RPC communication
 */
export class ACPConnection extends EventEmitter {
  constructor(inputStream = process.stdin, outputStream = process.stdout) {
    super();
    
    this.inputStream = inputStream;
    this.outputStream = outputStream;
    this.buffer = '';
    this.messageId = 1;
    this.pendingRequests = new Map();
    this.isConnected = false;
    this.debug = process.env.DEBUG === '1';
    
    this.setupStreams();
  }
  
  /**
   * Setup input/output streams
   */
  setupStreams() {
    this.inputStream.setEncoding('utf8');
    this.inputStream.on('data', this.handleData.bind(this));
    this.inputStream.on('end', () => {
      this.isConnected = false;
      this.emit('disconnect');
    });
    
    this.inputStream.on('error', (error) => {
      if (this.debug) {
        console.error('[ACP] Input stream error:', error);
      }
      this.emit('error', error);
    });
    
    this.outputStream.on('error', (error) => {
      if (this.debug) {
        console.error('[ACP] Output stream error:', error);
      }
      this.emit('error', error);
    });
  }
  
  /**
   * Start the connection
   */
  start() {
    this.isConnected = true;
    this.emit('connect');
    
    if (this.debug) {
      console.error('[ACP] Connection started');
    }
  }
  
  /**
   * Handle incoming data
   */
  handleData(chunk) {
    this.buffer += chunk;
    
    // Process complete messages (separated by newlines)
    const lines = this.buffer.split('\n');
    this.buffer = lines.pop() || ''; // Keep incomplete line in buffer
    
    for (const line of lines) {
      if (line.trim()) {
        try {
          const message = JSON.parse(line);
          this.handleMessage(message);
        } catch (error) {
          if (this.debug) {
            console.error('[ACP] Failed to parse message:', line, error);
          }
          this.sendError(null, ErrorCode.PARSE_ERROR, 'Parse error');
        }
      }
    }
  }
  
  /**
   * Handle a parsed JSON-RPC message
   */
  handleMessage(message) {
    const validation = validateMessage(message);
    if (!validation.valid) {
      if (this.debug) {
        console.error('[ACP] Invalid message:', validation.error, message);
      }
      this.sendError(message.id || null, ErrorCode.INVALID_REQUEST, validation.error);
      return;
    }
    
    if (this.debug) {
      console.error('[ACP] Received message:', JSON.stringify(message));
    }
    
    // Handle response to our request
    if (message.id && (message.result !== undefined || message.error !== undefined)) {
      this.handleResponse(message);
      return;
    }
    
    // Handle request or notification
    if (message.method) {
      if (message.id !== undefined) {
        // Request - needs response
        this.emit('request', message);
      } else {
        // Notification - no response needed
        this.emit('notification', message);
      }
    }
  }
  
  /**
   * Handle response to our request
   */
  handleResponse(message) {
    const { id, result, error } = message;
    const pendingRequest = this.pendingRequests.get(id);
    
    if (pendingRequest) {
      this.pendingRequests.delete(id);
      
      if (error) {
        pendingRequest.reject(new Error(`RPC Error ${error.code}: ${error.message}`));
      } else {
        pendingRequest.resolve(result);
      }
    } else if (this.debug) {
      console.error('[ACP] Received response for unknown request ID:', id);
    }
  }
  
  /**
   * Send a message
   */
  sendMessage(message) {
    if (!this.isConnected) {
      throw new Error('Connection not established');
    }
    
    const json = JSON.stringify(message);
    
    if (this.debug) {
      console.error('[ACP] Sending message:', json);
    }
    
    this.outputStream.write(json + '\n');
  }
  
  /**
   * Send a request and wait for response
   */
  async sendRequest(method, params = null) {
    const id = this.messageId++;
    const message = {
      jsonrpc: '2.0',
      method,
      id
    };
    
    if (params !== null) {
      message.params = params;
    }
    
    return new Promise((resolve, reject) => {
      // Store the pending request
      this.pendingRequests.set(id, { resolve, reject });
      
      // Send the message
      this.sendMessage(message);
      
      // Set timeout
      setTimeout(() => {
        if (this.pendingRequests.has(id)) {
          this.pendingRequests.delete(id);
          reject(new Error('Request timeout'));
        }
      }, 30000); // 30 second timeout
    });
  }
  
  /**
   * Send a notification (no response expected)
   */
  sendNotification(method, params = null) {
    const message = {
      jsonrpc: '2.0',
      method
    };
    
    if (params !== null) {
      message.params = params;
    }
    
    this.sendMessage(message);
  }
  
  /**
   * Send a response to a request
   */
  sendResponse(id, result) {
    const response = createResponse(id, result);
    this.sendMessage(response);
  }
  
  /**
   * Send an error response
   */
  sendError(id, code, message, data = null) {
    const error = createError(code, message, data);
    const response = createResponse(id, null, error);
    this.sendMessage(response);
  }
  
  /**
   * Close the connection
   */
  close() {
    this.isConnected = false;
    
    // Reject all pending requests
    for (const [id, pendingRequest] of this.pendingRequests) {
      pendingRequest.reject(new Error('Connection closed'));
    }
    this.pendingRequests.clear();
    
    this.emit('disconnect');
  }
}