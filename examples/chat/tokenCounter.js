import { get_encoding } from 'tiktoken';

/**
 * TokenCounter class to track token usage in the chat
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
      this.requestTokens = 0;
      this.responseTokens = 0;
      this.currentRequestTokens = 0;
      this.currentResponseTokens = 0;

      // Cache token tracking
      this.cacheCreationTokens = 0;
      this.cacheReadTokens = 0;
      this.currentCacheCreationTokens = 0;
      this.currentCacheReadTokens = 0;
      this.cachedPromptTokens = 0;
      this.currentCachedPromptTokens = 0;
    } catch (error) {
      console.error('Error initializing tokenizer:', error);
      // Fallback to a simple token counting method if tiktoken fails
      this.tokenizer = null;
      this.contextSize = 0;
      // Note: maxContextSize was previously defined here in fallback, but seems better managed elsewhere.
      // Consider adding it back if needed, or confirming it's handled by probeChat.js
      // this.maxContextSize = 8192;
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
        // Ensure text is a string before encoding
        const textToEncode = typeof text === 'string' ? text : String(text);
        const tokens = this.tokenizer.encode(textToEncode);
        return tokens.length;
      } catch (error) {
        console.warn('Error counting tokens, using fallback method:', error);
        // Fallback to a simple approximation (1 token ≈ 4 characters)
        const textLength = typeof text === 'string' ? text.length : String(text).length;
        return Math.ceil(textLength / 4);
      }
    } else {
      // Fallback to a simple approximation (1 token ≈ 4 characters)
      const textLength = typeof text === 'string' ? text.length : String(text).length;
      return Math.ceil(textLength / 4);
    }
  }

  /**
   * Add to request token count
   * @param {string|number} input - The text to count tokens for or the token count directly
   */
  addRequestTokens(input) {
    let tokenCount;

    if (typeof input === 'number') {
      tokenCount = input;
    } else if (typeof input === 'string') {
      tokenCount = this.countTokens(input);
    } else {
      console.warn('[WARN] Invalid input type for addRequestTokens:', typeof input);
      return;
    }

    this.requestTokens += tokenCount;
    this.currentRequestTokens = tokenCount; // Set current request tokens

    const debug = process.env.DEBUG_CHAT === '1';
    if (debug) {
      if (typeof input === 'string') {
        console.log(
          `[DEBUG] Added ${tokenCount} request tokens for text of length ${input.length}`
        );
      } else {
        console.log(`[DEBUG] Added ${tokenCount} request tokens directly`);
      }
    }
  }

  /**
   * Add to response token count
   * @param {string|number} input - The text to count tokens for or the token count directly
   */
  addResponseTokens(input) {
    let tokenCount;

    if (typeof input === 'number') {
      tokenCount = input;
    } else if (typeof input === 'string') {
      tokenCount = this.countTokens(input);
    } else {
      console.warn('[WARN] Invalid input type for addResponseTokens:', typeof input);
      return;
    }

    this.responseTokens += tokenCount;
    this.currentResponseTokens = tokenCount; // Set current response tokens

    const debug = process.env.DEBUG_CHAT === '1';
    if (debug) {
      if (typeof input === 'string') {
        console.log(
          `[DEBUG] Added ${tokenCount} response tokens for text of length ${input.length}`
        );
      } else {
        console.log(`[DEBUG] Added ${tokenCount} response tokens directly`);
      }
    }
  }

  /**
   * Record token usage from the AI SDK's result
   * @param {Object} usage - The usage object from the AI SDK's result
   * @param {Object} providerMetadata - The provider metadata from the AI SDK's result
   */
  recordUsage(usage, providerMetadata) {
    if (!usage) {
      console.warn('[WARN] No usage information provided to recordUsage');
      return;
    }

    // Reset current tokens before recording new usage
    this.currentRequestTokens = 0;
    this.currentResponseTokens = 0;
    this.currentCacheCreationTokens = 0;
    this.currentCacheReadTokens = 0;
    this.currentCachedPromptTokens = 0;

    // Check if debug mode is enabled
    const isDebugMode = process.env.DEBUG_CHAT === '1';

    // Convert to numbers and add to totals
    const promptTokens = Number(usage.promptTokens) || 0;
    const completionTokens = Number(usage.completionTokens) || 0;

    this.requestTokens += promptTokens;
    this.currentRequestTokens = promptTokens;

    this.responseTokens += completionTokens;
    this.currentResponseTokens = completionTokens;

    // Record Anthropic cache tokens if available in provider metadata
    if (providerMetadata?.anthropic) {
      const cacheCreation = Number(providerMetadata.anthropic.cacheCreationInputTokens) || 0;
      const cacheRead = Number(providerMetadata.anthropic.cacheReadInputTokens) || 0;

      this.cacheCreationTokens += cacheCreation;
      this.currentCacheCreationTokens = cacheCreation;

      this.cacheReadTokens += cacheRead;
      this.currentCacheReadTokens = cacheRead;

      if (isDebugMode) {
        console.log(`[DEBUG] Anthropic cache tokens: creation=${cacheCreation}, read=${cacheRead}`);
      }
    }

    // Record OpenAI cached prompt tokens if available
    if (providerMetadata?.openai) {
      const cachedPrompt = Number(providerMetadata.openai.cachedPromptTokens) || 0;

      this.cachedPromptTokens += cachedPrompt;
      this.currentCachedPromptTokens = cachedPrompt;

      if (isDebugMode) {
        console.log(`[DEBUG] OpenAI cached prompt tokens: ${cachedPrompt}`);
      }
    }

    // Force a context window calculation after recording usage
    this.calculateContextSize();

    if (isDebugMode) {
      console.log(
        `[DEBUG] Recorded usage: prompt=${usage.promptTokens || 0}, completion=${usage.completionTokens || 0}, total=${usage.totalTokens || 0}`
      );
      console.log(
        `[DEBUG] Current tokens: request=${this.currentRequestTokens}, response=${this.currentResponseTokens}`
      );
      console.log(
        `[DEBUG] Total tokens: request=${this.requestTokens}, response=${this.responseTokens}`
      );

      // Log Anthropic cache tokens if available
      if (providerMetadata?.anthropic) {
        console.log(
          `[DEBUG] Anthropic cache tokens: creation=${this.currentCacheCreationTokens}, read=${this.currentCacheReadTokens}, total=${this.currentCacheCreationTokens + this.currentCacheReadTokens}`
        );
      }

      // Log OpenAI cached prompt tokens if available
      if (providerMetadata?.openai) {
        console.log(
          `[DEBUG] OpenAI cached prompt tokens: ${this.currentCachedPromptTokens}`
        );
      }
      // Log calculated context size
      console.log(`[DEBUG] Context size after usage record: ${this.contextSize}`);
    }
  }

  /**
   * Calculate the current context window size based on history
   * @param {Array} messages - Optional messages to use instead of stored history
   * @returns {number} - Total tokens in context window
   */
  calculateContextSize(messages = null) {
    const msgsToCount = messages || this.history;
    let totalTokens = 0;

    // Removed: Early return for empty history. Loop handles this.
    // if (msgsToCount.length === 0) {
    //   this.contextSize = 100;
    //   return 100;
    // }

    // Check if debug mode is enabled
    const isDebugMode = process.env.DEBUG_CHAT === '1';

    for (const msg of msgsToCount) {
      // Count tokens in the message content
      if (typeof msg.content === 'string') {
        const contentTokens = this.countTokens(msg.content);
        totalTokens += contentTokens;
        if (isDebugMode) {
          console.log(`[DEBUG] Message content tokens: ${contentTokens} for ${msg.role}`);
        }
      } else if (Array.isArray(msg.content)) {
        // Handle Vercel AI SDK tool usage where content is an array
        for (const contentItem of msg.content) {
          if (contentItem.type === 'text') {
            const textTokens = this.countTokens(contentItem.text);
            totalTokens += textTokens;
            if (isDebugMode) {
              console.log(`[DEBUG] Multi-content text tokens: ${textTokens} for ${msg.role}`);
            }
          } else if (contentItem.type === 'tool_use') {
            // Approx tokens for tool use block structure itself, plus content
            const toolUseTokens = this.countTokens(JSON.stringify(contentItem));
            totalTokens += toolUseTokens;
            if (isDebugMode) {
              console.log(`[DEBUG] Multi-content tool use tokens: ${toolUseTokens} for ${msg.role}`);
            }
          }
          // Add other content types here if needed
        }
      } else if (msg.content) {
        // Handle structured content by converting to string (fallback)
        const structuredTokens = this.countTokens(JSON.stringify(msg.content));
        totalTokens += structuredTokens;
        if (isDebugMode) {
          console.log(`[DEBUG] Structured content tokens: ${structuredTokens} for ${msg.role}`);
        }
      }

      // Count tokens in role (approximately 4 tokens)
      // Only add role tokens if there's content or tool activity
      if (msg.content || msg.toolCalls || msg.toolCallResults || msg.tool_use_id) {
        totalTokens += 4;
      }


      // Count tokens in tool calls and results (Vercel AI SDK structure)
      if (msg.toolCalls) {
        const toolCallTokens = this.countTokens(JSON.stringify(msg.toolCalls));
        totalTokens += toolCallTokens;
        if (isDebugMode) {
          console.log(`[DEBUG] Tool call tokens: ${toolCallTokens}`);
        }
      }

      // Count tokens for tool results (now typically role: 'tool')
      if (msg.role === 'tool') {
        // Add tokens for tool_call_id, tool_use_id structure (approx)
        totalTokens += 5; // Rough estimate for {"role":"tool", "tool_call_id": "...", "content": ...} overhead
        // Content is handled by the main content checks above
        if (isDebugMode) {
          console.log(`[DEBUG] Added ~5 tokens for tool role structure`);
        }
      }

      // Deprecated? Keep for compatibility if old history format exists
      if (msg.toolCallResults) {
        const resultTokens = this.countTokens(JSON.stringify(msg.toolCallResults));
        totalTokens += resultTokens;
        if (isDebugMode) {
          console.log(`[DEBUG] (Deprecated?) Tool result tokens: ${resultTokens}`);
        }
      }
    }

    // Removed: Ensure we never return 0 by using a minimum of 100 tokens
    // totalTokens = Math.max(totalTokens, 100);
    this.contextSize = totalTokens; // Update the instance property

    if (isDebugMode) {
      console.log(`[DEBUG] Calculated context size: ${totalTokens} tokens from ${msgsToCount.length} messages`);
    }

    return totalTokens; // Return the actual calculated size
  }

  /**
   * Update history and recalculate context window size
   * @param {Array} messages - New message history
   */
  updateHistory(messages) {
    this.history = messages;
    this.calculateContextSize(); // Recalculate context size based on new history
  }

  /**
   * Clear all counters and history
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

    const isDebugMode = process.env.DEBUG_CHAT === '1';
    if (isDebugMode) {
      console.log('[DEBUG] Token usage, history and context window reset');
    }
  }

  /**
   * Start a new conversation turn - reset current token counters
   */
  startNewTurn() {
    this.currentRequestTokens = 0;
    this.currentResponseTokens = 0;
    this.currentCacheCreationTokens = 0;
    this.currentCacheReadTokens = 0;
    this.currentCachedPromptTokens = 0;

    // Calculate context size based on current history before the new turn starts
    this.calculateContextSize();

    const isDebugMode = process.env.DEBUG_CHAT === '1';
    if (isDebugMode) {
      console.log('[DEBUG] Current token counters reset for new turn');
      // Note: maxContextSize is not defined in this class, removing the reference
      // console.log(`[DEBUG] Context window size at start of turn: ${this.contextSize} tokens`);
      console.log(`[DEBUG] Context window size at start of turn: ${this.contextSize} tokens`);
    }
  }

  /**
   * Update the context window size (DEPRECATED? Prefer calculateContextSize)
   * This method seems redundant if calculateContextSize is always used.
   * Kept for potential compatibility, but marked as possibly deprecated.
   * @param {number} size - Current context size in tokens
   * @param {number} [maxSize] - Maximum context size (optional, not used internally)
   */
  updateContextWindow(size, maxSize) {
    console.warn('[WARN] updateContextWindow called. Prefer relying on calculateContextSize from history.');
    // If size is 0, calculate it from history instead
    if (size === 0 && this.history.length > 0) {
      this.calculateContextSize();
    } else {
      // Directly set the context size, overriding calculation
      this.contextSize = size;
    }

    // Note: maxContextSize is not stored/managed in this class.
    // if (maxSize) {
    //   this.maxContextSize = maxSize;
    // }

    const isDebugMode = process.env.DEBUG_CHAT === '1';
    if (isDebugMode) {
      console.log(
        `[DEBUG] Context window explicitly updated: ${this.contextSize} tokens`
        // ` (${Math.round((size / this.maxContextSize) * 100)}%)` - removed as maxContextSize is not tracked here
      );
    }
  }

  /**
   * Get the current token usage
   * @returns {Object} - Object containing current and total token counts
   */
  getTokenUsage() {
    // Always calculate context window size from history right before returning usage
    const currentContextSize = this.calculateContextSize();

    // Consolidate cache information
    const currentCacheRead = this.currentCacheReadTokens + this.currentCachedPromptTokens;
    const currentCacheWrite = this.currentCacheCreationTokens;
    const totalCacheRead = this.cacheReadTokens + this.cachedPromptTokens;
    const totalCacheWrite = this.cacheCreationTokens;

    const isDebugMode = process.env.DEBUG_CHAT === '1';
    if (isDebugMode) {
      console.log(`[DEBUG] Getting token usage. Context size: ${currentContextSize}, Current Cache read: ${currentCacheRead}, Current Cache write: ${currentCacheWrite}`);
    }

    const usageData = {
      contextWindow: currentContextSize, // Use the freshly calculated value
      current: {
        request: this.currentRequestTokens,
        response: this.currentResponseTokens,
        anthropic: {
          cacheCreation: this.currentCacheCreationTokens,
          cacheRead: this.currentCacheReadTokens,
          cacheTotal: this.currentCacheCreationTokens + this.currentCacheReadTokens
        },
        openai: {
          cachedPrompt: this.currentCachedPromptTokens
        },
        cacheRead: currentCacheRead,
        cacheWrite: currentCacheWrite,
        cacheTotal: currentCacheRead + currentCacheWrite,
        total: this.currentRequestTokens + this.currentResponseTokens
      },
      total: {
        request: this.requestTokens,
        response: this.responseTokens,
        anthropic: {
          cacheCreation: this.cacheCreationTokens,
          cacheRead: this.cacheReadTokens,
          cacheTotal: this.cacheCreationTokens + this.cacheReadTokens
        },
        openai: {
          cachedPrompt: this.cachedPromptTokens
        },
        cacheRead: totalCacheRead,
        cacheWrite: totalCacheWrite,
        cacheTotal: totalCacheRead + totalCacheWrite,
        total: this.requestTokens + this.responseTokens
      }
    };

    if (isDebugMode) {
      console.log(`[DEBUG] Token usage data prepared:`, JSON.stringify(usageData, null, 2));
    }

    return usageData;
  }
}