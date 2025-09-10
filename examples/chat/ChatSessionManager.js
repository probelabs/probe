// ChatSessionManager - Bridge between web server and ProbeAgent
import { ProbeAgent } from '../../npm/src/agent/ProbeAgent.js';
import { toolCallEmitter } from './probeTool.js';
import { randomUUID } from 'crypto';

/**
 * ChatSessionManager wraps ProbeAgent for web chat usage
 * Handles session management, history persistence, and web API compatibility
 */
export class ChatSessionManager {
  /**
   * Create a new ChatSessionManager instance
   * @param {Object} options - Configuration options
   * @param {string} [options.sessionId] - Session ID (generates new one if not provided)
   * @param {Object} [options.storage] - Storage instance for persistence
   * @param {string} [options.path] - Search directory path
   * @param {string} [options.apiProvider] - AI provider (anthropic, openai, google)
   * @param {string} [options.apiKey] - API key for the provider
   * @param {string} [options.apiUrl] - Custom API URL
   * @param {string} [options.model] - Model name override
   * @param {boolean} [options.allowEdit] - Allow code modification
   * @param {boolean} [options.debug] - Enable debug mode
   */
  constructor(options = {}) {
    // Session management
    this.sessionId = options.sessionId || randomUUID();
    this.storage = options.storage || null;
    this.debug = options.debug || process.env.DEBUG === '1';
    
    // Track creation time for compatibility with web server
    this.createdAt = Date.now();
    this.lastActivity = this.createdAt;
    
    // Create ProbeAgent instance
    this.agent = new ProbeAgent({
      sessionId: this.sessionId,
      path: options.path || process.cwd(),
      provider: options.apiProvider,
      model: options.model,
      allowEdit: !!options.allowEdit,
      debug: this.debug
    });
    
    // Forward ProbeAgent events to global toolCallEmitter for SSE
    this.agent.events.on('toolCall', (toolCallEvent) => {
      // Forward to global emitter with session-specific event name
      toolCallEmitter.emit(`toolCall:${this.sessionId}`, toolCallEvent);
      
      if (this.debug) {
        console.log(`[ChatSessionManager] Forwarded ${toolCallEvent.status} toolCall event for ${toolCallEvent.name} to session ${this.sessionId}`);
      }
    });
    
    // Initialize display history for web UI compatibility
    this.displayHistory = [];
    
    // Mark as not ready until history is loaded
    this._ready = false;
    
    if (this.debug) {
      console.log(`[ChatSessionManager] Created session ${this.sessionId} with ProbeAgent`);
    }
  }
  
  /**
   * Initialize the ChatSessionManager by loading history
   * This must be called before using chat()
   */
  async initialize() {
    if (this._ready) return; // Already initialized
    
    await this.loadHistory();
    this._ready = true;
    
    if (this.debug) {
      console.log(`[ChatSessionManager] Initialized session ${this.sessionId} with ${this.agent.history.length} messages from storage`);
    }
  }
  
  /**
   * Process a chat message - main entry point for web chat
   * @param {string} message - User message
   * @param {Array} [images] - Optional image attachments  
   * @returns {Promise<string>} - Assistant response
   */
  async chat(message, images = []) {
    // Ensure initialization is complete before processing chat
    await this.initialize();
    
    try {
      this.lastActivity = Date.now();
      
      // Save user message to storage
      if (this.storage) {
        await this.storage.saveMessage(this.sessionId, {
          role: 'user',
          content: message,
          timestamp: this.lastActivity,
          displayType: 'user',
          visible: 1,
          images: images || []
        });
      }
      
      // Add to display history for web UI
      this.displayHistory.push({
        role: 'user',
        content: message,
        timestamp: new Date(this.lastActivity).toISOString(),
        displayType: 'user',
        visible: true,
        images: images || []
      });
      
      // Call ProbeAgent to get response (ProbeAgent manages its own history internally)
      const response = await this.agent.answer(message, images);
      
      this.lastActivity = Date.now();
      
      if (this.debug) {
        console.log(`[ChatSessionManager] ProbeAgent history after answer(): ${this.agent.history.length} messages`);
      }
      
      // Save assistant response to storage
      if (this.storage) {
        await this.storage.saveMessage(this.sessionId, {
          role: 'assistant',
          content: response,
          timestamp: this.lastActivity,
          displayType: 'final',
          visible: 1
        });
      }
      
      // Add to display history for web UI
      this.displayHistory.push({
        role: 'assistant',
        content: response,
        timestamp: new Date(this.lastActivity).toISOString(),
        displayType: 'final',
        visible: true
      });
      
      return response;
      
    } catch (error) {
      if (this.debug) {
        console.error(`[ChatSessionManager] Chat error in session ${this.sessionId}:`, error);
      }
      throw error;
    }
  }
  
  /**
   * Load conversation history from storage
   */
  async loadHistory() {
    if (!this.storage) return;
    
    try {
      const history = await this.storage.getSessionHistory(this.sessionId);
      if (history && history.length > 0) {
        // Convert storage format to agent format for ProbeAgent.history
        this.agent.history = history
          .filter(msg => msg.visible) // Only include visible messages
          .map(msg => ({
            role: msg.role,
            content: msg.content
          }));
        
        // Convert storage format to display format for web UI
        this.displayHistory = history.map(msg => ({
          role: msg.role,
          content: msg.content,
          timestamp: new Date(msg.timestamp).toISOString(),
          displayType: msg.display_type || (msg.role === 'user' ? 'user' : 'final'),
          visible: !!msg.visible,
          images: msg.images || []
        }));
        
        if (this.debug) {
          console.log(`[ChatSessionManager] Loaded ${history.length} messages from storage for session ${this.sessionId}`);
        }
      }
    } catch (error) {
      console.error(`[ChatSessionManager] Failed to load history:`, error);
    }
  }
  
  /**
   * Clear conversation history and create new session
   * @returns {string} - New session ID
   */
  clearHistory() {
    const oldSessionId = this.sessionId;
    const newSessionId = randomUUID();
    
    if (this.debug) {
      console.log(`[ChatSessionManager] Clearing history: ${oldSessionId} -> ${newSessionId}`);
    }
    
    // Update session ID
    this.sessionId = newSessionId;
    
    // Clear histories
    this.displayHistory = [];
    
    // Create new ProbeAgent instance with fresh session
    this.agent = new ProbeAgent({
      sessionId: newSessionId,
      path: this.agent.allowedFolders[0] || process.cwd(),
      provider: this.agent.clientApiProvider,
      model: this.agent.model,
      allowEdit: this.agent.allowEdit,
      debug: this.debug
    });
    
    // Forward ProbeAgent events to global toolCallEmitter for SSE
    this.agent.events.on('toolCall', (toolCallEvent) => {
      // Forward to global emitter with session-specific event name
      toolCallEmitter.emit(`toolCall:${this.sessionId}`, toolCallEvent);
      
      if (this.debug) {
        console.log(`[ChatSessionManager] Forwarded ${toolCallEvent.status} toolCall event for ${toolCallEvent.name} to session ${this.sessionId}`);
      }
    });
    
    // Reset timestamps
    this.createdAt = Date.now();
    this.lastActivity = this.createdAt;
    
    return newSessionId;
  }
  
  /**
   * Get token usage statistics
   * @returns {Object} - Token usage data compatible with web UI
   */
  getTokenUsage() {
    // Note: getTokenUsage doesn't need initialization as it doesn't depend on history
    const agentUsage = this.agent.getTokenUsage();
    
    // Convert ProbeAgent format to web UI format
    return {
      contextWindow: agentUsage.contextWindow || 0,
      current: {
        request: agentUsage.request || 0,
        response: agentUsage.response || 0,
        total: agentUsage.total || 0,
        cacheRead: agentUsage.cacheRead || 0,
        cacheWrite: agentUsage.cacheWrite || 0,
        cacheTotal: (agentUsage.cacheRead || 0) + (agentUsage.cacheWrite || 0)
      },
      total: {
        request: agentUsage.totalRequest || agentUsage.request || 0,
        response: agentUsage.totalResponse || agentUsage.response || 0,
        total: agentUsage.totalTokens || agentUsage.total || 0,
        cacheRead: agentUsage.totalCacheRead || agentUsage.cacheRead || 0,
        cacheWrite: agentUsage.totalCacheWrite || agentUsage.cacheWrite || 0,
        cacheTotal: (agentUsage.totalCacheRead || agentUsage.cacheRead || 0) + (agentUsage.totalCacheWrite || agentUsage.cacheWrite || 0)
      }
    };
  }
  
  /**
   * Cancel any ongoing operations
   */
  cancel() {
    if (this.agent && typeof this.agent.cancel === 'function') {
      this.agent.cancel();
    }
  }
  
  /**
   * Get session ID
   * @returns {string}
   */
  getSessionId() {
    return this.sessionId;
  }
  
  /**
   * Get conversation history (alias for compatibility)
   */
  get history() {
    return this.agent.history;
  }
  
  /**
   * Set conversation history (alias for compatibility)
   */
  set history(newHistory) {
    this.agent.history = newHistory;
  }
}