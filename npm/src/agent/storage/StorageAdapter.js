/**
 * Base class for storage adapters
 * Implement this interface to provide custom storage backends for ProbeAgent history
 */
export class StorageAdapter {
  /**
   * Load conversation history for a session
   * @param {string} sessionId - Session identifier
   * @returns {Promise<Array<Object>>} Array of message objects with {role, content, ...}
   */
  async loadHistory(sessionId) {
    throw new Error('StorageAdapter.loadHistory() must be implemented by subclass');
  }

  /**
   * Save a message to storage
   * @param {string} sessionId - Session identifier
   * @param {Object} message - Message object { role, content, ... }
   * @returns {Promise<void>}
   */
  async saveMessage(sessionId, message) {
    throw new Error('StorageAdapter.saveMessage() must be implemented by subclass');
  }

  /**
   * Clear history for a session
   * @param {string} sessionId - Session identifier
   * @returns {Promise<void>}
   */
  async clearHistory(sessionId) {
    throw new Error('StorageAdapter.clearHistory() must be implemented by subclass');
  }

  /**
   * Get session metadata (optional)
   * @param {string} sessionId - Session identifier
   * @returns {Promise<Object|null>} Session metadata or null
   */
  async getSessionMetadata(sessionId) {
    return null;
  }

  /**
   * Update session activity timestamp (optional)
   * @param {string} sessionId - Session identifier
   * @returns {Promise<void>}
   */
  async updateSessionActivity(sessionId) {
    // Optional - implement if you want to track session activity
  }
}
