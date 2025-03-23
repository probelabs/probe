import { get_encoding } from 'tiktoken';

/**
 * TokenCounter class to track token usage in the chat
 */
export class TokenCounter {
  constructor() {
    // Initialize the tokenizer with cl100k_base encoding (works for both Claude and GPT models)
    try {
      this.tokenizer = get_encoding('cl100k_base');
      this.requestTokens = 0;
      this.responseTokens = 0;
    } catch (error) {
      console.error('Error initializing tokenizer:', error);
      // Fallback to a simple token counting method if tiktoken fails
      this.tokenizer = null;
      this.requestTokens = 0;
      this.responseTokens = 0;
    }
  }

  /**
   * Count tokens in a string using tiktoken or fallback method
   * @param {string} text - The text to count tokens for
   * @returns {number} - The number of tokens
   */
  countTokens(text) {
    if (this.tokenizer) {
      try {
        const tokens = this.tokenizer.encode(text);
        return tokens.length;
      } catch (error) {
        console.warn('Error counting tokens, using fallback method:', error);
        // Fallback to a simple approximation (1 token ≈ 4 characters)
        return Math.ceil(text.length / 4);
      }
    } else {
      // Fallback to a simple approximation (1 token ≈ 4 characters)
      return Math.ceil(text.length / 4);
    }
  }

  /**
   * Add to request token count
   * @param {string} text - The text to count tokens for
   */
  addRequestTokens(text) {
    const tokenCount = this.countTokens(text);
    this.requestTokens += tokenCount;

    const debug = process.env.DEBUG === 'true' || process.env.DEBUG === '1';
    if (debug) {
      console.log(
        `[DEBUG] Added ${tokenCount} request tokens for text of length ${text.length}`
      );
    }
  }

  /**
   * Add to response token count
   * @param {string} text - The text to count tokens for
   */
  addResponseTokens(text) {
    const tokenCount = this.countTokens(text);
    this.responseTokens += tokenCount;

    const debug = process.env.DEBUG === 'true' || process.env.DEBUG === '1';
    if (debug) {
      console.log(
        `[DEBUG] Added ${tokenCount} response tokens for text of length ${text.length}`
      );
    }
  }

  /**
   * Reset counters back to zero
   */
  clear() {
    this.requestTokens = 0;
    this.responseTokens = 0;

    const debug = process.env.DEBUG === 'true' || process.env.DEBUG === '1';
    if (debug) {
      console.log(`[DEBUG] Token usage reset to 0`);
    }
  }

  /**
   * Get the current token usage
   * @returns {Object} - Object containing request, response, and total token counts
   */
  getTokenUsage() {
    return {
      request: this.requestTokens,
      response: this.responseTokens,
      total: this.requestTokens + this.responseTokens
    };
  }
}