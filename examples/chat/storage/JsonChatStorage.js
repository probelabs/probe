import { homedir } from 'os';
import { join } from 'path';
import { existsSync, mkdirSync, writeFileSync, readFileSync, readdirSync, statSync, unlinkSync } from 'fs';

/**
 * JSON file-based storage for chat history
 * Each session is stored as a separate JSON file in ~/.probe/sessions/
 * Uses file modification time for sorting sessions
 */
export class JsonChatStorage {
  constructor(options = {}) {
    this.webMode = options.webMode || false;
    this.verbose = options.verbose || false;
    this.baseDir = this.getChatHistoryDir();
    this.sessionsDir = join(this.baseDir, 'sessions');
    this.fallbackToMemory = false;
    
    // In-memory fallback storage
    this.memorySessions = new Map();
    this.memoryMessages = new Map(); // sessionId -> messages[]
  }
  
  /**
   * Get the appropriate directory for storing chat history
   */
  getChatHistoryDir() {
    if (process.platform === 'win32') {
      // Windows: Use LocalAppData
      const localAppData = process.env.LOCALAPPDATA || join(homedir(), 'AppData', 'Local');
      return join(localAppData, 'probe');
    } else {
      // Mac/Linux: Use ~/.probe
      return join(homedir(), '.probe');
    }
  }
  
  /**
   * Ensure the chat history directory exists
   */
  ensureChatHistoryDir() {
    try {
      if (!existsSync(this.baseDir)) {
        mkdirSync(this.baseDir, { recursive: true });
      }
      if (!existsSync(this.sessionsDir)) {
        mkdirSync(this.sessionsDir, { recursive: true });
      }
      return true;
    } catch (error) {
      console.warn(`Failed to create chat history directory ${this.baseDir}:`, error.message);
      return false;
    }
  }
  
  /**
   * Get the file path for a session
   */
  getSessionFilePath(sessionId) {
    return join(this.sessionsDir, `${sessionId}.json`);
  }
  
  /**
   * Initialize storage - JSON files if in web mode and directory is accessible
   */
  async initialize() {
    if (!this.webMode) {
      this.fallbackToMemory = true;
      if (this.verbose) {
        console.log('Using in-memory storage (CLI mode)');
      }
      return true;
    }
    
    try {
      if (!this.ensureChatHistoryDir()) {
        this.fallbackToMemory = true;
        if (this.verbose) {
          console.log('Cannot create history directory, using in-memory storage');
        }
        return true;
      }
      
      if (this.verbose) {
        console.log(`JSON file storage initialized at: ${this.sessionsDir}`);
      }
      
      return true;
    } catch (error) {
      console.warn('Failed to initialize JSON storage, falling back to memory:', error.message);
      this.fallbackToMemory = true;
      return true;
    }
  }
  
  /**
   * Save or update session data
   */
  async saveSession(sessionData) {
    const { id, createdAt, lastActivity, firstMessagePreview, metadata = {} } = sessionData;
    
    if (this.fallbackToMemory) {
      this.memorySessions.set(id, {
        id,
        created_at: createdAt,
        last_activity: lastActivity,
        first_message_preview: firstMessagePreview,
        metadata
      });
      return true;
    }
    
    try {
      const filePath = this.getSessionFilePath(id);
      
      // Read existing session data if it exists
      let existingData = {
        id,
        created_at: createdAt,
        last_activity: lastActivity,
        first_message_preview: firstMessagePreview,
        metadata,
        messages: []
      };
      
      if (existsSync(filePath)) {
        try {
          const fileContent = readFileSync(filePath, 'utf8');
          const existing = JSON.parse(fileContent);
          existingData = {
            ...existing,
            last_activity: lastActivity,
            first_message_preview: firstMessagePreview || existing.first_message_preview,
            metadata: { ...existing.metadata, ...metadata }
          };
        } catch (error) {
          console.warn(`Failed to read existing session file ${filePath}:`, error.message);
        }
      }
      
      writeFileSync(filePath, JSON.stringify(existingData, null, 2));
      return true;
    } catch (error) {
      console.error('Failed to save session:', error);
      return false;
    }
  }
  
  /**
   * Update session activity timestamp
   */
  async updateSessionActivity(sessionId, timestamp = Date.now()) {
    if (this.fallbackToMemory) {
      const session = this.memorySessions.get(sessionId);
      if (session) {
        session.last_activity = timestamp;
      }
      return true;
    }
    
    try {
      const filePath = this.getSessionFilePath(sessionId);
      if (existsSync(filePath)) {
        const fileContent = readFileSync(filePath, 'utf8');
        const sessionData = JSON.parse(fileContent);
        sessionData.last_activity = timestamp;
        writeFileSync(filePath, JSON.stringify(sessionData, null, 2));
      }
      return true;
    } catch (error) {
      console.error('Failed to update session activity:', error);
      return false;
    }
  }
  
  /**
   * Save a message to the session
   */
  async saveMessage(sessionId, messageData) {
    const {
      role,
      content,
      timestamp = Date.now(),
      displayType,
      visible = 1,
      images = [],
      metadata = {}
    } = messageData;
    
    const message = {
      role,
      content,
      timestamp,
      display_type: displayType,
      visible,
      images,
      metadata
    };
    
    if (this.fallbackToMemory) {
      if (!this.memoryMessages.has(sessionId)) {
        this.memoryMessages.set(sessionId, []);
      }
      this.memoryMessages.get(sessionId).push(message);
      return true;
    }
    
    try {
      const filePath = this.getSessionFilePath(sessionId);
      let sessionData = {
        id: sessionId,
        created_at: timestamp,
        last_activity: timestamp,
        first_message_preview: null,
        metadata: {},
        messages: []
      };
      
      // Read existing session data
      if (existsSync(filePath)) {
        try {
          const fileContent = readFileSync(filePath, 'utf8');
          sessionData = JSON.parse(fileContent);
        } catch (error) {
          console.warn(`Failed to read session file ${filePath}:`, error.message);
        }
      }
      
      // Add message to session
      sessionData.messages.push(message);
      sessionData.last_activity = timestamp;
      
      // Update first message preview if this is the first user message
      if (role === 'user' && !sessionData.first_message_preview) {
        const preview = content.length > 100 ? content.substring(0, 100) + '...' : content;
        sessionData.first_message_preview = preview;
      }
      
      writeFileSync(filePath, JSON.stringify(sessionData, null, 2));
      return true;
    } catch (error) {
      console.error('Failed to save message:', error);
      return false;
    }
  }
  
  /**
   * Get session history (display messages only)
   */
  async getSessionHistory(sessionId, limit = 100) {
    if (this.fallbackToMemory) {
      const messages = this.memoryMessages.get(sessionId) || [];
      return messages
        .filter(msg => msg.visible)
        .slice(0, limit);
    }
    
    try {
      const filePath = this.getSessionFilePath(sessionId);
      if (!existsSync(filePath)) {
        return [];
      }
      
      const fileContent = readFileSync(filePath, 'utf8');
      const sessionData = JSON.parse(fileContent);
      
      return (sessionData.messages || [])
        .filter(msg => msg.visible)
        .slice(0, limit);
    } catch (error) {
      console.error('Failed to get session history:', error);
      return [];
    }
  }
  
  /**
   * List recent sessions using file modification dates
   */
  async listSessions(limit = 50, offset = 0) {
    if (this.fallbackToMemory) {
      const sessions = Array.from(this.memorySessions.values())
        .sort((a, b) => b.last_activity - a.last_activity)
        .slice(offset, offset + limit);
      return sessions;
    }
    
    try {
      if (!existsSync(this.sessionsDir)) {
        return [];
      }
      
      // Get all JSON files in sessions directory
      const files = readdirSync(this.sessionsDir)
        .filter(file => file.endsWith('.json'))
        .map(file => {
          const filePath = join(this.sessionsDir, file);
          const stat = statSync(filePath);
          return {
            file,
            filePath,
            mtime: stat.mtime.getTime(),
            sessionId: file.replace('.json', '')
          };
        })
        .sort((a, b) => b.mtime - a.mtime) // Sort by modification time (newest first)
        .slice(offset, offset + limit);
      
      const sessions = [];
      for (const fileInfo of files) {
        try {
          const fileContent = readFileSync(fileInfo.filePath, 'utf8');
          const sessionData = JSON.parse(fileContent);
          sessions.push({
            id: sessionData.id,
            created_at: sessionData.created_at,
            last_activity: sessionData.last_activity || fileInfo.mtime,
            first_message_preview: sessionData.first_message_preview,
            metadata: sessionData.metadata || {}
          });
        } catch (error) {
          console.warn(`Failed to read session file ${fileInfo.filePath}:`, error.message);
        }
      }
      
      return sessions;
    } catch (error) {
      console.error('Failed to list sessions:', error);
      return [];
    }
  }
  
  /**
   * Delete a session and its file
   */
  async deleteSession(sessionId) {
    if (this.fallbackToMemory) {
      this.memorySessions.delete(sessionId);
      this.memoryMessages.delete(sessionId);
      return true;
    }
    
    try {
      const filePath = this.getSessionFilePath(sessionId);
      if (existsSync(filePath)) {
        unlinkSync(filePath);
      }
      return true;
    } catch (error) {
      console.error('Failed to delete session:', error);
      return false;
    }
  }
  
  /**
   * Prune old sessions (older than specified days)
   */
  async pruneOldSessions(olderThanDays = 30) {
    const cutoffTime = Date.now() - (olderThanDays * 24 * 60 * 60 * 1000);
    
    if (this.fallbackToMemory) {
      let pruned = 0;
      for (const [sessionId, session] of this.memorySessions.entries()) {
        if (session.last_activity < cutoffTime) {
          this.memorySessions.delete(sessionId);
          this.memoryMessages.delete(sessionId);
          pruned++;
        }
      }
      return pruned;
    }
    
    try {
      if (!existsSync(this.sessionsDir)) {
        return 0;
      }
      
      const files = readdirSync(this.sessionsDir).filter(file => file.endsWith('.json'));
      let pruned = 0;
      
      for (const file of files) {
        const filePath = join(this.sessionsDir, file);
        const stat = statSync(filePath);
        
        if (stat.mtime.getTime() < cutoffTime) {
          unlinkSync(filePath);
          pruned++;
        }
      }
      
      return pruned;
    } catch (error) {
      console.error('Failed to prune old sessions:', error);
      return 0;
    }
  }
  
  /**
   * Get storage statistics
   */
  async getStats() {
    if (this.fallbackToMemory) {
      let messageCount = 0;
      let visibleMessageCount = 0;
      
      for (const messages of this.memoryMessages.values()) {
        messageCount += messages.length;
        visibleMessageCount += messages.filter(msg => msg.visible).length;
      }
      
      return {
        session_count: this.memorySessions.size,
        message_count: messageCount,
        visible_message_count: visibleMessageCount,
        storage_type: 'memory'
      };
    }
    
    try {
      if (!existsSync(this.sessionsDir)) {
        return {
          session_count: 0,
          message_count: 0,
          visible_message_count: 0,
          storage_type: 'json_files'
        };
      }
      
      const files = readdirSync(this.sessionsDir).filter(file => file.endsWith('.json'));
      let messageCount = 0;
      let visibleMessageCount = 0;
      
      for (const file of files) {
        try {
          const filePath = join(this.sessionsDir, file);
          const fileContent = readFileSync(filePath, 'utf8');
          const sessionData = JSON.parse(fileContent);
          
          if (sessionData.messages) {
            messageCount += sessionData.messages.length;
            visibleMessageCount += sessionData.messages.filter(msg => msg.visible).length;
          }
        } catch (error) {
          // Skip corrupted files
        }
      }
      
      return {
        session_count: files.length,
        message_count: messageCount,
        visible_message_count: visibleMessageCount,
        storage_type: 'json_files'
      };
    } catch (error) {
      console.error('Failed to get storage stats:', error);
      return {
        session_count: 0,
        message_count: 0,
        visible_message_count: 0,
        storage_type: 'error'
      };
    }
  }
  
  /**
   * Check if using persistent storage
   */
  isPersistent() {
    return !this.fallbackToMemory;
  }
  
  /**
   * Close storage (no-op for JSON files)
   */
  async close() {
    // No cleanup needed for JSON files
  }
}