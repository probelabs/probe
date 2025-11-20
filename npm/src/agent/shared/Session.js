/**
 * Base Session class for AI provider engines
 * Manages conversation state and message counting
 */
export class Session {
  constructor(id, debug = false) {
    this.id = id;
    this.conversationId = null;  // Provider-specific conversation/thread ID for resumption
    this.messageCount = 0;
    this.debug = debug;
  }

  /**
   * Set the conversation ID for session resumption
   * @param {string} conversationId - Provider's conversation/thread ID
   */
  setConversationId(conversationId) {
    this.conversationId = conversationId;
    if (this.debug) {
      console.log(`[Session ${this.id}] Conversation ID: ${conversationId}`);
    }
  }

  /**
   * Increment the message count
   */
  incrementMessageCount() {
    this.messageCount++;
  }

  /**
   * Get session info as plain object
   * @returns {Object} Session information
   */
  getInfo() {
    return {
      id: this.id,
      conversationId: this.conversationId,
      messageCount: this.messageCount
    };
  }

  /**
   * Get resume arguments for CLI commands (used by Claude Code)
   * @returns {Array<string>} CLI arguments for resuming conversation
   */
  getResumeArgs() {
    if (this.conversationId && this.messageCount > 0) {
      return ['--resume', this.conversationId];
    }
    return [];
  }
}
