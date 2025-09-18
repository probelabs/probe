import { get_encoding } from 'tiktoken';

/**
 * TokenCounter class to track token usage in the agent
 */
export class TokenCounter {
  constructor() {
    // Initialize the tokenizer with cl100k_base encoding (works for both Claude and GPT models)
    try {
      // Initialize tokenizer
      this.tokenizer = get_encoding('cl100k_base');

      // Context window tracking
      this.contextSize = 0; // Current size based on history
      this.history = []; // Store message history for context calculation

      // Token counters
      this.requestTokens = 0; // Total prompt tokens over session
      this.responseTokens = 0; // Total completion tokens over session
      this.currentRequestTokens = 0; // Prompt tokens for the current LLM call
      this.currentResponseTokens = 0; // Completion tokens for the current LLM call

      // Cache token tracking
      this.cacheCreationTokens = 0; // Total Anthropic cache creation tokens
      this.cacheReadTokens = 0; // Total Anthropic cache read tokens
      this.currentCacheCreationTokens = 0; // Anthropic cache creation for current call
      this.currentCacheReadTokens = 0; // Anthropic cache read for current call
      this.cachedPromptTokens = 0; // Total OpenAI cached prompt tokens
      this.currentCachedPromptTokens = 0; // OpenAI cached prompt for current call

    } catch (error) {
      console.error('Error initializing tokenizer:', error);
      // Fallback to a simple token counting method if tiktoken fails
      this.tokenizer = null;
      this.contextSize = 0;
      this.requestTokens = 0;
      this.responseTokens = 0;
      this.currentRequestTokens = 0;
      this.currentResponseTokens = 0;
      this.cacheCreationTokens = 0;
      this.cacheReadTokens = 0;
      this.currentCacheCreationTokens = 0;
      this.currentCacheReadTokens = 0;
      this.cachedPromptTokens = 0;
      this.currentCachedPromptTokens = 0;
      this.history = [];
    }
    this.debug = process.env.DEBUG === '1';
  }

  /**
   * Count tokens in a string using tiktoken or fallback method
   * @param {string} text - The text to count tokens for
   * @returns {number} - The number of tokens
   */
  countTokens(text) {
    if (typeof text !== 'string') {
      text = String(text); // Ensure text is a string
    }

    if (this.tokenizer) {
      try {
        const tokens = this.tokenizer.encode(text);
        return tokens.length;
      } catch (error) {
        // Fallback to a simple approximation (1 token ≈ 4 characters)
        return Math.ceil(text.length / 4);
      }
    } else {
      // Fallback to a simple approximation (1 token ≈ 4 characters)
      return Math.ceil(text.length / 4);
    }
  }

  /**
   * Add to request token count (manual counting, less used now with recordUsage)
   * @param {string|number} input - The text to count tokens for or the token count directly
   */
  addRequestTokens(input) {
    let tokenCount = 0;

    if (typeof input === 'number') {
      tokenCount = input;
    } else if (typeof input === 'string') {
      tokenCount = this.countTokens(input);
    } else {
      console.warn('[WARN] Invalid input type for addRequestTokens:', typeof input);
      return;
    }

    this.requestTokens += tokenCount;
    this.currentRequestTokens = tokenCount;

    if (this.debug) {
      console.log(`[DEBUG] (Manual) Added ${tokenCount} request tokens. Total: ${this.requestTokens}, Current: ${this.currentRequestTokens}`);
    }
  }

  /**
   * Add to response token count (manual counting, less used now with recordUsage)
   * @param {string|number} input - The text to count tokens for or the token count directly
   */
  addResponseTokens(input) {
    let tokenCount = 0;

    if (typeof input === 'number') {
      tokenCount = input;
    } else if (typeof input === 'string') {
      tokenCount = this.countTokens(input);
    } else {
      console.warn('[WARN] Invalid input type for addResponseTokens:', typeof input);
      return;
    }

    this.responseTokens += tokenCount;
    this.currentResponseTokens = tokenCount;

    if (this.debug) {
      console.log(`[DEBUG] (Manual) Added ${tokenCount} response tokens. Total: ${this.responseTokens}, Current: ${this.currentResponseTokens}`);
    }
  }

  /**
   * Record token usage from the AI SDK's result for a single LLM call.
   * This resets 'current' counters and updates totals.
   * @param {Object} usage - The usage object { promptTokens, completionTokens, totalTokens }
   * @param {Object} providerMetadata - Metadata possibly containing cache info
   */
  recordUsage(usage, providerMetadata) {
    if (!usage) {
      console.warn('[WARN] No usage information provided to recordUsage');
      return;
    }

    // --- Reset CURRENT counters for this specific API call ---
    this.currentRequestTokens = 0;
    this.currentResponseTokens = 0;
    this.currentCacheCreationTokens = 0;
    this.currentCacheReadTokens = 0;
    this.currentCachedPromptTokens = 0;

    // --- Process usage data ---
    const promptTokens = Number(usage.promptTokens) || 0;
    const completionTokens = Number(usage.completionTokens) || 0;

    // Update CURRENT tokens for this call
    this.currentRequestTokens = promptTokens;
    this.currentResponseTokens = completionTokens;

    // Update TOTAL tokens accumulated over the session
    this.requestTokens += promptTokens;
    this.responseTokens += completionTokens;

    // --- Process Provider Metadata for Cache Info ---
    if (providerMetadata?.anthropic) {
      const cacheCreation = Number(providerMetadata.anthropic.cacheCreationInputTokens) || 0;
      const cacheRead = Number(providerMetadata.anthropic.cacheReadInputTokens) || 0;

      this.currentCacheCreationTokens = cacheCreation;
      this.currentCacheReadTokens = cacheRead;

      this.cacheCreationTokens += cacheCreation;
      this.cacheReadTokens += cacheRead;

      if (this.debug) {
        console.log(`[DEBUG] Anthropic cache tokens (current): creation=${cacheCreation}, read=${cacheRead}`);
      }
    }

    if (providerMetadata?.openai) {
      const cachedPrompt = Number(providerMetadata.openai.cachedPromptTokens) || 0;

      this.currentCachedPromptTokens = cachedPrompt;
      this.cachedPromptTokens += cachedPrompt;

      if (this.debug) {
        console.log(`[DEBUG] OpenAI cached prompt tokens (current): ${cachedPrompt}`);
      }
    }

    if (this.debug) {
      console.log(
        `[DEBUG] Recorded usage: current(req=${this.currentRequestTokens}, resp=${this.currentResponseTokens}), total(req=${this.requestTokens}, resp=${this.responseTokens})`
      );
      console.log(`[DEBUG] Total cache tokens: Anthropic(create=${this.cacheCreationTokens}, read=${this.cacheReadTokens}), OpenAI(prompt=${this.cachedPromptTokens})`);
    }
  }

  /**
   * Calculate the current context window size based on provided messages or internal history.
   * @param {Array|null} messages - Optional messages array to use for calculation. If null, uses internal this.history.
   * @returns {number} - Total tokens estimated in the context window.
   */
  calculateContextSize(messages = null) {
    const msgsToCount = messages !== null ? messages : this.history;
    let totalTokens = 0;

    if (this.debug && messages === null) {
      console.log(`[DEBUG] Calculating context size from internal history (${this.history.length} messages)`);
    }

    for (const msg of msgsToCount) {
      let messageTokens = 0;
      // Add tokens for role overhead (approximate)
      messageTokens += 4;

      // Content tokens
      if (typeof msg.content === 'string') {
        messageTokens += this.countTokens(msg.content);
      } else if (Array.isArray(msg.content)) {
        // Handle array content (e.g., Vercel AI SDK tool usage format)
        for (const item of msg.content) {
          if (item.type === 'text' && typeof item.text === 'string') {
            messageTokens += this.countTokens(item.text);
          } else {
            // Estimate tokens for other non-text parts (tool calls/results)
            messageTokens += this.countTokens(JSON.stringify(item));
          }
        }
      } else if (msg.content) {
        // Fallback for other content types
        messageTokens += this.countTokens(JSON.stringify(msg.content));
      }

      // --- Add tokens for tool calls/results if present (Vercel SDK format) ---
      if (msg.toolCalls) {
        messageTokens += this.countTokens(JSON.stringify(msg.toolCalls));
        messageTokens += 5; // Approx overhead for tool_calls structure
      }
      // For 'tool' role messages (results)
      if (msg.role === 'tool' && msg.toolCallId) {
        messageTokens += this.countTokens(msg.toolCallId); // Add tokens for the ID
        messageTokens += 5; // Approx overhead for tool role structure
      }
      if (msg.toolCallResults) {
        messageTokens += this.countTokens(JSON.stringify(msg.toolCallResults));
        messageTokens += 5; // Approx overhead
      }

      totalTokens += messageTokens;
    }

    // Update the instance property *only* if calculating based on internal history
    if (messages === null) {
      this.contextSize = totalTokens;
      if (this.debug) {
        console.log(`[DEBUG] Updated internal context size: ${this.contextSize} tokens`);
      }
    }

    return totalTokens;
  }

  /**
   * Update internal history and recalculate internal context window size.
   * @param {Array} messages - New message history array.
   */
  updateHistory(messages) {
    // Ensure messages is an array
    if (!Array.isArray(messages)) {
      console.warn("[WARN] updateHistory called with non-array:", messages);
      this.history = [];
    } else {
      // Create a shallow copy to avoid external modifications
      this.history = [...messages];
    }
    // Recalculate context size based on the new internal history
    this.calculateContextSize(); // This updates this.contextSize
    if (this.debug) {
      console.log(`[DEBUG] History updated (${this.history.length} messages). Recalculated context size: ${this.contextSize}`);
    }
  }

  /**
   * Clear all counters and internal history. Reset context size.
   */
  clear() {
    // Reset counters
    this.requestTokens = 0;
    this.responseTokens = 0;
    this.currentRequestTokens = 0;
    this.currentResponseTokens = 0;
    this.cacheCreationTokens = 0;
    this.cacheReadTokens = 0;
    this.currentCacheCreationTokens = 0;
    this.currentCacheReadTokens = 0;
    this.cachedPromptTokens = 0;
    this.currentCachedPromptTokens = 0;

    // Clear history and context
    this.history = [];
    this.contextSize = 0; // Reset calculated context size

    if (this.debug) {
      console.log('[DEBUG] TokenCounter cleared: usage, history, and context size reset.');
    }
  }

  /**
   * Start a new conversation turn - reset CURRENT token counters.
   * Calculates context size based on history *before* the new turn.
   */
  startNewTurn() {
    this.currentRequestTokens = 0;
    this.currentResponseTokens = 0;
    this.currentCacheCreationTokens = 0;
    this.currentCacheReadTokens = 0;
    this.currentCachedPromptTokens = 0;

    // Calculate context size based on current history *before* new messages are added
    this.calculateContextSize(); // Updates this.contextSize

    if (this.debug) {
      console.log('[DEBUG] TokenCounter: New turn started. Current counters reset.');
      console.log(`[DEBUG] Context size at start of turn: ${this.contextSize} tokens`);
    }
  }

  /**
   * Get the current token usage state including context size.
   * Recalculates context size from internal history before returning.
   * @returns {Object} - Object containing current turn, total session, and context window usage.
   */
  getTokenUsage() {
    // Always calculate context window size from internal history right before returning usage
    const currentContextSize = this.calculateContextSize(); // Recalculates and updates this.contextSize

    // Consolidate cache info for simpler reporting
    const currentCacheRead = this.currentCacheReadTokens + this.currentCachedPromptTokens;
    const currentCacheWrite = this.currentCacheCreationTokens;
    const totalCacheRead = this.cacheReadTokens + this.cachedPromptTokens;
    const totalCacheWrite = this.cacheCreationTokens;

    const usageData = {
      contextWindow: currentContextSize, // Use the freshly calculated value
      current: { // Usage for the *last* LLM call recorded
        request: this.currentRequestTokens,
        response: this.currentResponseTokens,
        total: this.currentRequestTokens + this.currentResponseTokens,
        cacheRead: currentCacheRead,
        cacheWrite: currentCacheWrite,
        cacheTotal: currentCacheRead + currentCacheWrite,
        // Keep detailed breakdown if needed
        anthropic: {
          cacheCreation: this.currentCacheCreationTokens,
          cacheRead: this.currentCacheReadTokens,
        },
        openai: {
          cachedPrompt: this.currentCachedPromptTokens
        }
      },
      total: { // Accumulated usage over the session
        request: this.requestTokens,
        response: this.responseTokens,
        total: this.requestTokens + this.responseTokens,
        cacheRead: totalCacheRead,
        cacheWrite: totalCacheWrite,
        cacheTotal: totalCacheRead + totalCacheWrite,
        // Keep detailed breakdown if needed
        anthropic: {
          cacheCreation: this.cacheCreationTokens,
          cacheRead: this.cacheReadTokens,
        },
        openai: {
          cachedPrompt: this.cachedPromptTokens
        }
      }
    };

    if (this.debug) {
      // console.log(`[DEBUG] getTokenUsage() called. Returning data:`, JSON.stringify(usageData, null, 2));
    }

    return usageData;
  }
}