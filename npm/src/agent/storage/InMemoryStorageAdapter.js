import { StorageAdapter } from './StorageAdapter.js';

/**
 * Default in-memory storage adapter
 * This is the default behavior - stores history in a Map in memory
 */
export class InMemoryStorageAdapter extends StorageAdapter {
  constructor() {
    super();
    this.sessions = new Map(); // sessionId -> {messages: [], metadata: {}}
  }

  async loadHistory(sessionId) {
    const session = this.sessions.get(sessionId);
    return session ? session.messages : [];
  }

  async saveMessage(sessionId, message) {
    if (!this.sessions.has(sessionId)) {
      this.sessions.set(sessionId, {
        messages: [],
        metadata: {
          createdAt: new Date().toISOString(),
          lastActivity: new Date().toISOString()
        }
      });
    }

    const session = this.sessions.get(sessionId);
    session.messages.push(message);
    session.metadata.lastActivity = new Date().toISOString();
  }

  async clearHistory(sessionId) {
    this.sessions.delete(sessionId);
  }

  async getSessionMetadata(sessionId) {
    const session = this.sessions.get(sessionId);
    return session ? session.metadata : null;
  }

  async updateSessionActivity(sessionId) {
    const session = this.sessions.get(sessionId);
    if (session) {
      session.metadata.lastActivity = new Date().toISOString();
    }
  }
}
