import 'dotenv/config';
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { generateText } from 'ai';
import { randomUUID } from 'crypto';
import { TokenCounter } from './tokenCounter.js';
import { TokenUsageDisplay } from './tokenUsageDisplay.js';
import { writeFileSync } from 'fs';
import { join } from 'path';
import { existsSync } from 'fs';
// Import the tools that emit events and the listFilesByLevel utility
import { DEFAULT_SYSTEM_MESSAGE, searchTool, queryTool, extractTool, listFilesByLevel } from '@buger/probe';
import { probeTool, searchToolInstance, queryToolInstance, extractToolInstance } from './probeTool.js';

// Maximum number of messages to keep in history
const MAX_HISTORY_MESSAGES = 20;

// Parse and validate allowed folders from environment variable
const allowedFolders = process.env.ALLOWED_FOLDERS
  ? process.env.ALLOWED_FOLDERS.split(',').map(folder => folder.trim()).filter(Boolean)
  : [];

// Validate folders exist on startup
console.log('Configured search folders:');
for (const folder of allowedFolders) {
  const exists = existsSync(folder);
  console.log(`- ${folder} ${exists ? '✓' : '✗ (not found)'}`);
  if (!exists) {
    console.warn(`Warning: Folder "${folder}" does not exist or is not accessible`);
  }
}

if (allowedFolders.length === 0) {
  console.warn('No folders configured. Set ALLOWED_FOLDERS in .env file or the current directory will be used by default.');
}

/**
 * ProbeChat class to handle chat interactions with AI models
 */
export class ProbeChat {
  /**
   * Create a new ProbeChat instance
   * @param {Object} options - Configuration options
   * @param {Function} options.toolCallCallback - Callback function for tool calls (sessionId, toolCallData)
   */
  constructor(options = {}) {
    // Flag to track if a request has been cancelled
    this.cancelled = false;

    // AbortController for cancelling fetch requests
    this.abortController = null;
    // Make allowedFolders accessible as a property of the class
    this.allowedFolders = allowedFolders;

    // Initialize token counter and display
    this.tokenCounter = new TokenCounter();
    this.tokenDisplay = new TokenUsageDisplay({
      maxTokens: 8192 // Will be updated based on model
    });

    // Use provided session ID or generate a unique one
    this.sessionId = options.sessionId || randomUUID();

    // Get debug mode
    this.debug = process.env.DEBUG_CHAT === '1';

    if (this.debug) {
      console.log(`[DEBUG] Generated session ID for chat: ${this.sessionId}`);
    }

    // Configure tools with the session ID
    this.configOptions = {
      sessionId: this.sessionId,
      debug: this.debug
    };

    // Create configured tool instances that emit SSE events
    // We need to ensure the tools use the correct session ID
    this.tools = {
      probe: {
        ...probeTool,
        execute: async (params) => {
          // Ensure the session ID is passed to the tool
          const enhancedParams = {
            ...params,
            sessionId: this.sessionId
          };
          if (this.debug) {
            console.log(`[DEBUG] ProbeChat executing probeTool with sessionId: ${this.sessionId}`);
          }
          return await probeTool.execute(enhancedParams);
        }
      },
      search: {
        ...searchToolInstance,
        execute: async (params) => {
          // Ensure the session ID is passed to the tool
          const enhancedParams = {
            ...params,
            sessionId: this.sessionId
          };
          if (this.debug) {
            console.log(`[DEBUG] ProbeChat executing searchToolInstance with sessionId: ${this.sessionId}`);
          }
          return await searchToolInstance.execute(enhancedParams);
        }
      },
      query: {
        ...queryToolInstance,
        execute: async (params) => {
          // Ensure the session ID is passed to the tool
          const enhancedParams = {
            ...params,
            sessionId: this.sessionId
          };
          if (this.debug) {
            console.log(`[DEBUG] ProbeChat executing queryToolInstance with sessionId: ${this.sessionId}`);
          }
          return await queryToolInstance.execute(enhancedParams);
        }
      },
      extract: {
        ...extractToolInstance,
        execute: async (params) => {
          // Ensure the session ID is passed to the tool
          const enhancedParams = {
            ...params,
            sessionId: this.sessionId
          };
          if (this.debug) {
            console.log(`[DEBUG] ProbeChat executing extractToolInstance with sessionId: ${this.sessionId}`);
          }
          return await extractToolInstance.execute(enhancedParams);
        }
      }
    };

    // Initialize the chat model
    this.initializeModel();

    // Initialize chat history
    this.history = [];
  }

  /**
   * Initialize the AI model based on available API keys and forced provider setting
   */
  initializeModel() {
    // Get API keys from environment variables
    const anthropicApiKey = process.env.ANTHROPIC_API_KEY;
    const openaiApiKey = process.env.OPENAI_API_KEY;
    const googleApiKey = process.env.GOOGLE_API_KEY;

    // Get custom API URLs if provided
    const anthropicApiUrl = process.env.ANTHROPIC_API_URL || 'https://api.anthropic.com/v1';
    const openaiApiUrl = process.env.OPENAI_API_URL || 'https://api.openai.com/v1';
    const googleApiUrl = process.env.GOOGLE_API_URL || 'https://generativelanguage.googleapis.com/v1beta';

    // Get model override if provided
    const modelName = process.env.MODEL_NAME;

    // Get forced provider if specified
    const forceProvider = process.env.FORCE_PROVIDER ? process.env.FORCE_PROVIDER.toLowerCase() : null;

    if (this.debug) {
      console.log(`[DEBUG] Available API keys: Anthropic=${!!anthropicApiKey}, OpenAI=${!!openaiApiKey}, Google=${!!googleApiKey}`);
      console.log(`[DEBUG] Force provider: ${forceProvider || '(not set)'}`);
    }

    // Check if a specific provider is forced
    if (forceProvider) {
      console.log(`Provider forced to: ${forceProvider}`);

      if (forceProvider === 'anthropic' && anthropicApiKey) {
        this.initializeAnthropicModel(anthropicApiKey, anthropicApiUrl, modelName);
        return;
      } else if (forceProvider === 'openai' && openaiApiKey) {
        this.initializeOpenAIModel(openaiApiKey, openaiApiUrl, modelName);
        return;
      } else if (forceProvider === 'google' && googleApiKey) {
        this.initializeGoogleModel(googleApiKey, googleApiUrl, modelName);
        return;
      }

      console.warn(`WARNING: Forced provider "${forceProvider}" selected but API key is missing!`);
    }

    // If no provider is forced, use the first available API key
    if (anthropicApiKey) {
      this.initializeAnthropicModel(anthropicApiKey, anthropicApiUrl, modelName);
    } else if (openaiApiKey) {
      this.initializeOpenAIModel(openaiApiKey, openaiApiUrl, modelName);
    } else if (googleApiKey) {
      this.initializeGoogleModel(googleApiKey, googleApiUrl, modelName);
    } else {
      console.warn('No API key provided. Please set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY environment variable.');
      // Instead of throwing an error, we'll set a flag indicating we're in no API keys mode
      this.noApiKeysMode = true;
      // Set default values for properties that would normally be set in the initialize methods
      this.model = 'none';
      this.apiType = 'none';
      console.log('Running in NO API KEYS MODE - setup instructions will be shown to users');
      console.log('Note: For debugging, set DEBUG_CHAT=1 environment variable');
    }
  }

  /**
   * Initialize Anthropic model
   * @param {string} apiKey - Anthropic API key
   * @param {string} apiUrl - Anthropic API URL
   * @param {string} modelName - Optional model name override
   */
  initializeAnthropicModel(apiKey, apiUrl, modelName) {
    // Initialize Anthropic provider
    this.provider = createAnthropic({
      apiKey: apiKey,
      baseURL: apiUrl,
      headers: {
        'anthropic-beta': 'token-efficient-tools-2025-02-19'
      }
    });
    this.model = modelName || 'claude-3-7-sonnet-latest';
    this.apiType = 'anthropic';

    console.log(`Using Anthropic API with model: ${this.model}`);

    if (this.debug) {
      console.log(`[DEBUG] Anthropic API URL: ${apiUrl}`);
    }
  }

  /**
   * Initialize OpenAI model
   * @param {string} apiKey - OpenAI API key
   * @param {string} apiUrl - OpenAI API URL
   * @param {string} modelName - Optional model name override
   */
  initializeOpenAIModel(apiKey, apiUrl, modelName) {
    // Initialize OpenAI provider
    this.provider = createOpenAI({
      apiKey: apiKey,
      baseURL: apiUrl,
    });
    this.model = modelName || 'gpt-4o-2024-05-13';
    this.apiType = 'openai';

    console.log(`Using OpenAI API with model: ${this.model}`);

    if (this.debug) {
      console.log(`[DEBUG] OpenAI API URL: ${apiUrl}`);
    }
  }

  /**
   * Initialize Google model
   * @param {string} apiKey - Google API key
   * @param {string} apiUrl - Google API URL
   * @param {string} modelName - Optional model name override
   */
  initializeGoogleModel(apiKey, apiUrl, modelName) {
    // Initialize Google provider
    this.provider = createGoogleGenerativeAI({
      apiKey: apiKey,
      baseURL: apiUrl,
    });
    this.model = modelName || 'gemini-2.0-flash';
    this.apiType = 'google';

    console.log(`Using Google API with model: ${this.model}`);

    if (this.debug) {
      console.log(`[DEBUG] Google API URL: ${apiUrl}`);
    }
  }

  /**
   * Get the system message with instructions for the AI
   * @returns {Promise<string>} - The system message
   */
  async getSystemMessage() {
    // Use the default system message from the probe package as a base
    let systemMessage = DEFAULT_SYSTEM_MESSAGE;

    const searchDirectory = allowedFolders.length > 0 ? allowedFolders[0] : process.cwd();
    if (this.debug) {
      console.log(`[DEBUG] Generating file list for ${searchDirectory}...`);
    }

    // Add folder information
    if (allowedFolders.length > 0) {
      const folderList = allowedFolders.map(f => `"${f}"`).join(', ');
      systemMessage += `\n\nThe following folders are configured for code search: ${folderList}. When using searchCode, specify one of these folders in the folder argument.`;
    } else {
      systemMessage += `\n\nCurrent folder: ${searchDirectory}. You should specify it as path, or subpaths inside it. If you need to search inside the dependecies code, you should use special syntax for path: "go:github.com/user/repo" or "js:user/repo" or "rust:crate_name|.`;
    }

    systemMessage += '\n\nWhen appropriate add mermaid diagrams - inside the [] blocks inside diagram wrap to quotes "]';

    console.log(`[DEBUG] System message: ${systemMessage}`);

    // Add file list information if available
    try {
      let files = await listFilesByLevel({
        directory: searchDirectory,
        maxFiles: 100,
        respectGitignore: true
      });

      // Exclude debug file(s) and node_modules in debug mode to prevent clutter or accidental large listing
      files = files.filter((file) => {
        const lower = file.toLowerCase();
        return !lower.includes('probe-debug.txt') && !lower.includes('node_modules');
      });

      if (files.length > 0) {
        systemMessage += `\n\nHere is a list of up to ${files.length} files in the codebase (organized by directory depth):\n\n`;
        systemMessage += files.map(file => `- ${file}`).join('\n');
      }
    } catch (error) {
      console.warn(`Warning: Could not generate file list: ${error.message}`);
    }

    return systemMessage;
  }

  /**
   * Abort the current chat request
   */
  abort() {
    console.log(`Aborting chat for session: ${this.sessionId}`);
    this.cancelled = true;

    // Abort any fetch requests
    if (this.abortController) {
      try {
        this.abortController.abort();
      } catch (error) {
        console.error('Error aborting fetch request:', error);
      }
    }
  }

  /**
   * Process a user message and get a response
   * @param {string} message - The user message
   * @param {string} [sessionId] - Optional session ID to use for this chat (overrides the default)
   * @returns {Promise<string>} - The AI response
   */
  async chat(message, sessionId) {
    // Reset cancelled flag
    this.cancelled = false;

    // Create a new AbortController
    this.abortController = new AbortController();
    // If a session ID is provided and it's different from the current one, update it
    if (sessionId && sessionId !== this.sessionId) {
      if (this.debug) {
        console.log(`[DEBUG] Using provided session ID: ${sessionId} (instead of ${this.sessionId})`);
      }
      // Update the session ID permanently
      this.sessionId = sessionId;
      // Update tool configurations with the new session ID
      this.configOptions.sessionId = sessionId;

      // Only recreate tools if the session ID has changed
      // Create configured tool instances that emit SSE events
      // We need to ensure the tools use the correct session ID
      this.tools = {
        search: {
          ...searchToolInstance,
          execute: async (params) => {
            // Ensure the session ID is passed to the tool
            const enhancedParams = {
              ...params,
              sessionId: this.sessionId
            };
            if (this.debug) {
              console.log(`[DEBUG] ProbeChat executing searchToolInstance with sessionId: ${this.sessionId}`);
            }
            return await searchToolInstance.execute(enhancedParams);
          }
        },
        query: {
          ...queryToolInstance,
          execute: async (params) => {
            // Ensure the session ID is passed to the tool
            const enhancedParams = {
              ...params,
              sessionId: this.sessionId
            };
            if (this.debug) {
              console.log(`[DEBUG] ProbeChat executing queryToolInstance with sessionId: ${this.sessionId}`);
            }
            return await queryToolInstance.execute(enhancedParams);
          }
        },
        extract: {
          ...extractToolInstance,
          execute: async (params) => {
            // Ensure the session ID is passed to the tool
            const enhancedParams = {
              ...params,
              sessionId: this.sessionId
            };
            if (this.debug) {
              console.log(`[DEBUG] ProbeChat executing extractToolInstance with sessionId: ${this.sessionId}`);
            }
            return await extractToolInstance.execute(enhancedParams);
          }
        }
      };

      if (this.debug) {
        console.log(`[DEBUG] Recreated tools with new session ID: ${this.sessionId}`);
      }

      // Process the message with the new session ID
      return await this._processChat(message);
    } else {
      // Use the default session ID
      return await this._processChat(message);
    }
  }

  /**
   * Internal method to process a chat message
   * @param {string} message - The user message
   * @returns {Promise<string>} - The AI response
   * @private
   */
  async _processChat(message) {
    try {
      if (this.debug) {
        console.log(`[DEBUG] Received user message: ${message}`);
        console.log(`[DEBUG] Current history length before adding new message: ${this.history.length}`);

        // Log the current history content
        if (this.history.length > 0) {
          console.log(`[DEBUG] Current history content:`);
          this.history.forEach((msg, index) => {
            const preview = msg.content.length > 50 ?
              `${msg.content.substring(0, 50)}...` : msg.content;
            console.log(`[DEBUG]   ${index + 1}. ${msg.role}: ${preview}`);
          });
        } else {
          console.log(`[DEBUG] No previous history found for this session`);
        }
      }
      // Reset current token counters for new turn
      this.tokenCounter.startNewTurn();

      // Count tokens in the user message
      this.tokenCounter.addRequestTokens(message);

      // Limit history to prevent token overflow when DEBUG_CHAT=1
      if (this.history.length > MAX_HISTORY_MESSAGES) {
        const historyStart = this.history.length - MAX_HISTORY_MESSAGES;
        this.history = this.history.slice(historyStart);
      }

      // Prepare messages array
      const messages = [
        ...this.history,
        {
          role: 'user', content: message, providerOptions: {
            anthropic: { cacheControl: { type: 'ephemeral' } }
          }
        }
      ];

      if (this.debug) {
        console.log(`[DEBUG] Sending ${messages.length} messages to model`);
        console.log(`[DEBUG] Message breakdown:`);
        console.log(`[DEBUG]   - ${this.history.length} messages from history`);
        console.log(`[DEBUG]   - 1 new user message`);

        // Calculate approximate token count for the conversation
        let totalTokens = 0;
        messages.forEach((msg) => {
          // Rough estimate: 1 token per 4 characters
          const estimatedTokens = Math.ceil(msg.content.length / 4);
          totalTokens += estimatedTokens;
        });

        console.log(`[DEBUG] Estimated total tokens for conversation: ~${totalTokens}`);
        console.log(`[DEBUG] Messages being sent to model:`);

        messages.forEach((msg, index) => {
          const preview = msg.content.length > 50 ?
            `${msg.content.substring(0, 50)}...` : msg.content;
          console.log(`[DEBUG]   ${index + 1}. ${msg.role}: ${preview}`);
        });
      }

      // Check if the request has been cancelled
      if (this.cancelled) {
        throw new Error('Request was cancelled by the user');
      }

      // Determine max tokens based on model name
      let maxTokens = 4096; // Default value

      // If model starts with gpt-4o, set to 4096
      if (this.model.startsWith('gpt-4o')) {
        maxTokens = 4096;
      }
      // If model is claude-3-5, claude-3-7, gemini, or o3-mini, set to 8000
      else if (this.model.includes('claude-3-5') || this.model.includes('claude-3-7') ||
        this.model.includes('gemini') || this.model.includes('o3-mini')) {
        maxTokens = 8000;
      }

      // Update token display with max tokens
      this.tokenDisplay = new TokenUsageDisplay({ maxTokens });

      if (this.debug) {
        console.log(`[DEBUG] Using max tokens: ${maxTokens} for model: ${this.model}`);
      }

      // Configure generateOptions
      const generateOptions = {
        model: this.provider(this.model),
        messages: messages,
        system: await this.getSystemMessage(),
        tools: this.tools,
        maxSteps: 20,
        temperature: 0.7,
        maxTokens: maxTokens,
        signal: this.abortController.signal
      };

      // Write debug information to file if in debug mode (DEBUG_CHAT=1)
      if (this.debug) {
        try {
          const systemMessage = await this.getSystemMessage();

          // Estimate token counts for better debugging
          const systemTokens = Math.ceil(systemMessage.length / 4);
          let messagesTokens = 0;
          messages.forEach(m => {
            messagesTokens += Math.ceil(m.content.length / 4);
          });

          const totalEstimatedTokens = systemTokens + messagesTokens;

          // Write to probe-debug.txt
          const debugFilePath = join(process.cwd(), 'probe-debug.txt');
          writeFileSync(
            debugFilePath,
            `=== LATEST AI REQUEST (${new Date().toISOString()}) ===\n\n` +
            `Session ID: ${this.sessionId}\n` +
            `Model: ${this.model} (${this.apiType})\n` +
            `History Length: ${this.history.length} messages\n` +
            `Estimated Tokens: ~${totalEstimatedTokens} (System: ~${systemTokens}, Messages: ~${messagesTokens})\n\n` +
            `=== SYSTEM MESSAGE ===\n${systemMessage}\n\n` +
            `=== MESSAGES SENT TO AI ===\n${JSON.stringify(messages, null, 2)}\n\n`,
            { flag: 'w' }
          );

          console.log(`[DEBUG] Wrote latest AI request to ${debugFilePath} (Est. tokens: ~${totalEstimatedTokens})`);
        } catch (error) {
          console.error(`[DEBUG] Error writing debug file:`, error);
        }
      }

      // Add API-specific options
      if (this.apiType === 'anthropic' && this.model.includes('3-7')) {
        generateOptions.experimental_thinking = {
          enabled: true,
          budget: 8000
        };
      }

      try {
        if (this.cancelled) {
          throw new Error('Request was cancelled by the user');
        }

        // Retry wrapper function for generateText with exponential backoff
        const retryGenerateText = async (options, maxRetries = 3) => {
          let lastError;
          for (let attempt = 1; attempt <= maxRetries; attempt++) {
            try {
              if (this.debug) {
                console.log(`[DEBUG] generateText attempt ${attempt}/${maxRetries}`);
              }
              return await generateText(options);
            } catch (error) {
              lastError = error;
              console.error(`Error in generateText (attempt ${attempt}/${maxRetries}):`, error.message);

              if (attempt < maxRetries) {
                // Wait for 1 second before retrying (could be made exponential if needed)
                const delayMs = 1000;
                if (this.debug) {
                  console.log(`[DEBUG] Retrying in ${delayMs}ms...`);
                }
                await new Promise(resolve => setTimeout(resolve, delayMs));
              }
            }
          }
          // If we've exhausted all retries, throw the last error
          throw lastError;
        };

        const result = await retryGenerateText(generateOptions);

        // Update token counter's history with complete message array
        if (result.messages && Array.isArray(result.messages)) {
          this.tokenCounter.updateHistory(result.messages);
        }

        // Extract the text content from the response
        const responseText = result.text;

        // Update ProbeChat's own history
        if (result.messages && Array.isArray(result.messages)) {
          if (this.debug) {
            console.log(`[DEBUG] Updating history with complete message array from result.messages`);
            console.log(`[DEBUG] Messages array length: ${result.messages.length}`);
          }

          // Replace the current history with the complete message array
          this.history = result.messages.map(msg => {
            // Add ephemeral cache control if missing
            if (!msg.providerOptions?.anthropic?.cacheControl) {
              return {
                ...msg,
                providerOptions: {
                  ...(msg.providerOptions || {}),
                  anthropic: {
                    ...(msg.providerOptions?.anthropic || {}),
                    cacheControl: { type: 'ephemeral' }
                  }
                }
              };
            }
            return msg;
          });

          // Ensure the history does not exceed the maximum length
          if (this.history.length > MAX_HISTORY_MESSAGES) {
            if (this.debug) {
              console.log(`[DEBUG] History length (${this.history.length}) exceeds max (${MAX_HISTORY_MESSAGES}). Trimming...`);
            }
            this.history = this.history.slice(this.history.length - MAX_HISTORY_MESSAGES);
            if (this.debug) {
              console.log(`[DEBUG] History trimmed to ${this.history.length} messages.`);
            }
          }

          // Log the structure of the last few messages for verification
          if (this.debug) {
            const historyTail = this.history.slice(-3);
            console.log(`[DEBUG] Last ${historyTail.length} history messages:`, JSON.stringify(historyTail, null, 2));
          }
        } else {
          // Fallback to the old method if result.messages is not available
          if (this.debug) {
            console.log(`[DEBUG] result.messages not available, falling back to manual history update`);
          }
          // Add user message
          this.history.push({
            role: 'user',
            content: message,
            providerOptions: {
              anthropic: { cacheControl: { type: 'ephemeral' } }
            }
          });
          // Add assistant message
          this.history.push({
            role: 'assistant',
            content: responseText,
            providerOptions: {
              anthropic: { cacheControl: { type: 'ephemeral' } }
            }
          });

          // IMPORTANT FIX: Update tokenCounter's history so calculateContextSize won't remain 100
          this.tokenCounter.updateHistory(this.history);
        }

        // Use the token usage information from the result if available
        if (result.usage) {
          if (this.debug) {
            console.log(`[DEBUG] Provider metadata:`, result.providerMetadata?.anthropic);
            console.log(`[DEBUG] Usage:`, result.usage);
          }

          // Record usage with provider metadata
          this.tokenCounter.recordUsage(result.usage, result.providerMetadata);

          // Force context window calculation
          this.tokenCounter.calculateContextSize();

          // Ensure cache info is properly recorded
          const cacheRead = (result.providerMetadata?.anthropic?.cacheReadInputTokens || 0) +
            (result.providerMetadata?.openai?.cachedPromptTokens || 0);
          const cacheWrite = result.providerMetadata?.anthropic?.cacheCreationInputTokens || 0;

          if (this.debug) {
            console.log(`[DEBUG] Token usage from result: Prompt=${result.usage.promptTokens}, Completion=${result.usage.completionTokens}, Total=${result.usage.totalTokens}`);
            console.log(`[DEBUG] Accumulated usage: Request=${this.tokenCounter.requestTokens}, Response=${this.tokenCounter.responseTokens}`);
            console.log(`[DEBUG] Context window size: ${this.tokenCounter.contextSize}`);
            console.log(`[DEBUG] Cache token usage: Read=${cacheRead}, Write=${cacheWrite}, Total=${cacheRead + cacheWrite}`);
          }

          if (result.providerMetadata?.openai) {
            const cachedPrompt = result.providerMetadata.openai.cachedPromptTokens || 0;
            console.log(`[DEBUG] OpenAI cached prompt tokens: ${cachedPrompt}`);
          }
        } else {
          // Fallback if result.usage is not available
          if (this.debug) {
            console.log(`[DEBUG] result.usage not available, falling back to manual token counting`);
          }

          // Force context window calculation
          this.tokenCounter.calculateContextSize();

          if (this.debug) {
            console.log(`[DEBUG] Context window size (manual calculation): ${this.tokenCounter.contextSize}`);
          }

          const responseTokenCount = this.tokenCounter.countTokens(responseText);
          this.tokenCounter.addResponseTokens(responseTokenCount);

          if (this.debug) {
            console.log(`[DEBUG] Estimated response tokens using tiktoken: ${responseTokenCount}`);
            console.log(`[DEBUG] Context window size: ${this.tokenCounter.contextSize}`);
          }
        }

        // Append final results to debug file
        if (this.debug) {
          try {
            const debugFilePath = join(process.cwd(), 'probe-debug.txt');
            const finalResponseText = result.text || "[No final text response]";

            let tokenInfo = "Token usage information not available.";
            if (result.usage) {
              tokenInfo =
                `Prompt tokens: ${result.usage.promptTokens}\n` +
                `Completion tokens: ${result.usage.completionTokens}\n` +
                `Total tokens: ${result.usage.totalTokens}\n\n` +
                `--- Accumulated Usage ---\n` +
                `Request tokens: ${this.tokenCounter.requestTokens}\n` +
                `Response tokens: ${this.tokenCounter.responseTokens}\n` +
                `Total tokens: ${this.tokenCounter.requestTokens + this.tokenCounter.responseTokens}`;

              if (result.providerMetadata?.anthropic) {
                const cacheCreation = result.providerMetadata.anthropic.cacheCreationInputTokens || 0;
                const cacheRead = result.providerMetadata.anthropic.cacheReadInputTokens || 0;

                tokenInfo += `\n\n--- Anthropic Cache Token Usage ---\n` +
                  `Cache creation tokens: ${cacheCreation}\n` +
                  `Cache read tokens: ${cacheRead}\n` +
                  `Total cache tokens: ${cacheCreation + cacheRead}`;
              }
            } else {
              tokenInfo =
                `Request tokens: ${this.tokenCounter.requestTokens}\n` +
                `Response tokens: ${this.tokenCounter.responseTokens}\n` +
                `Total tokens: ${this.tokenCounter.requestTokens + this.tokenCounter.responseTokens}`;

              if (this.tokenCounter.cacheCreationTokens > 0 || this.tokenCounter.cacheReadTokens > 0) {
                tokenInfo += `\n\n--- Anthropic Cache Token Usage ---\n` +
                  `Cache creation tokens: ${this.tokenCounter.cacheCreationTokens}\n` +
                  `Cache read tokens: ${this.tokenCounter.cacheReadTokens}\n` +
                  `Total cache tokens: ${this.tokenCounter.cacheCreationTokens + this.tokenCounter.cacheReadTokens}`;
              }

              if (this.tokenCounter.cachedPromptTokens > 0) {
                tokenInfo += `\n\n--- OpenAI Cache Token Usage ---\n` +
                  `Cached prompt tokens: ${this.tokenCounter.cachedPromptTokens}`;
              }
            }

            const toolRelatedMessages = result.messages ?
              result.messages.filter(m => m.role === 'assistant' || m.role === 'tool') :
              [];

            writeFileSync(
              debugFilePath,
              `\n=== AI RESPONSE (Final Text) ===\n${finalResponseText}\n\n` +
              `=== RAW TOOL CALLS/RESULTS (From result.messages) ===\n` +
              `${JSON.stringify(toolRelatedMessages, null, 2)}\n\n` +
              `=== TOKEN USAGE (This Turn & Accumulated) ===\n` +
              `${tokenInfo}\n`,
              { flag: 'a' }
            );

            writeFileSync(
              debugFilePath,
              `\n=== SENT TO UI (FINAL) ===\n${responseText}\n`,
              { flag: 'a' }
            );

            writeFileSync(
              debugFilePath,
              `\n=== CURRENT HISTORY STATE ===\n` +
              `History length: ${this.history.length} messages\n` +
              `First few messages: ${JSON.stringify(this.history.slice(0, 2), null, 2)}\n` +
              `Last few messages: ${JSON.stringify(this.history.slice(-2), null, 2)}\n`,
              { flag: 'a' }
            );

            console.log(`[DEBUG] Appended AI response and detailed information to ${debugFilePath}`);
          } catch (error) {
            console.error(`[DEBUG] Error appending to debug file:`, error);
          }
        }

        if (result.toolCalls && result.toolCalls.length > 0) {
          console.log(`Tool was used: ${result.toolCalls.length} times`);

          result.toolCalls.forEach((call, index) => {
            if (this.debug) {
              console.log(`[DEBUG] Tool call ${index + 1}: ${call.name}`);
              if (call.args) {
                console.log(`[DEBUG] Tool call ${index + 1} args:`, JSON.stringify(call.args, null, 2));
              }

              let resultSize = 0;
              let resultPreview = '';
              if (call.result) {
                const resultStr = typeof call.result === 'string'
                  ? call.result
                  : JSON.stringify(call.result, null, 2);
                resultSize = resultStr.length;
                resultPreview = resultStr.length > 100
                  ? resultStr.substring(0, 100) + '... (truncated)'
                  : resultStr;
                console.log(`[DEBUG] Tool call ${index + 1} result size: ~${Math.ceil(resultSize / 4)} tokens (${resultSize} chars)`);
                console.log(`[DEBUG] Tool call ${index + 1} result preview: ${resultPreview}`);
              }

              try {
                const debugFilePath = join(process.cwd(), 'probe-debug.txt');
                const toolCallInfo =
                  `\n=== TOOL CALL ${index + 1} ===\n` +
                  `Name: ${call.name}\n` +
                  `Args: ${JSON.stringify(call.args, null, 2)}\n\n` +
                  `Result Size: ~${Math.ceil(resultSize / 4)} tokens (${resultSize} chars)\n` +
                  `Result: ${typeof call.result === 'string'
                    ? call.result
                    : JSON.stringify(call.result, null, 2)}\n`;

                writeFileSync(debugFilePath, toolCallInfo, { flag: 'a' });

                const currentMessages = [
                  ...this.history,
                  { role: 'user', content: message }
                ];

                let totalEstimatedTokens = 0;
                currentMessages.forEach(m => {
                  totalEstimatedTokens += Math.ceil(m.content.length / 4);
                });

                if (result.toolCalls) {
                  result.toolCalls.forEach((tc, i) => {
                    if (i <= index) {
                      const tcResult = typeof tc.result === 'string'
                        ? tc.result
                        : JSON.stringify(tc.result, null, 2);
                      totalEstimatedTokens += Math.ceil(tcResult.length / 4);

                      const tcArgs = JSON.stringify(tc.args, null, 2);
                      totalEstimatedTokens += Math.ceil(tcArgs.length / 4);
                    }
                  });
                }

                writeFileSync(
                  debugFilePath,
                  `\n=== CONVERSATION STATE AFTER TOOL CALL ${index + 1} ===\n` +
                  `Current estimated tokens: ~${totalEstimatedTokens}\n` +
                  `History messages: ${this.history.length}\n` +
                  `Tool calls so far: ${index + 1} of ${result.toolCalls.length}\n`,
                  { flag: 'a' }
                );

                console.log(`[DEBUG] Appended tool call ${index + 1} and conversation state to ${debugFilePath}`);
              } catch (error) {
                console.error(`[DEBUG] Error appending tool call to debug file:`, error);
              }
            }
            if (this.debug) {
              console.log(`[DEBUG] Tool call completed: ${call.name}`);
            }
          });
        }

        return responseText;
      } catch (error) {
        if (error.name === 'AbortError' || (error.message && error.message.includes('cancelled'))) {
          console.log('Chat request was cancelled');
          this.cancelled = true;
          throw new Error('Request was cancelled by the user');
        }
        throw error;
      }
    } catch (error) {
      console.error('Error in chat:', error);

      if (error.message && error.message.includes('cancelled')) {
        throw error;
      }

      return `Error: ${error.message}`;
    }
  }

  /**
   * Get the current token usage
   * @returns {Object} - Object containing request, response, and total token counts
   */
  getTokenUsage() {
    // Get token usage from the counter
    const usage = this.tokenCounter.getTokenUsage();

    // Use the context size from the tokenCounter
    const formattedUsage = this.tokenDisplay.format(usage);
    return formattedUsage;
  }

  /**
   * Clear the entire history and reset session/token usage
   * @returns {string} - The new session ID
   */
  clearHistory() {
    const oldHistoryLength = this.history.length;
    const oldSessionId = this.sessionId;

    this.history = [];
    this.sessionId = randomUUID();
    this.tokenCounter.clear();

    if (this.debug) {
      console.log(`[DEBUG] ===== CLEARING CHAT HISTORY =====`);
      console.log(`[DEBUG] Cleared ${oldHistoryLength} messages from history`);
      console.log(`[DEBUG] Old session ID: ${oldSessionId}`);
      console.log(`[DEBUG] New session ID: ${this.sessionId}`);
      console.log(`[DEBUG] Token counter reset to zero`);
    }

    // Update the session ID in the config options
    this.configOptions.sessionId = this.sessionId;

    // Recreate tools with the new session ID
    this.tools = {
      search: {
        ...searchToolInstance,
        name: "search",
        execute: async (params) => {
          const enhancedParams = {
            ...params,
            sessionId: this.sessionId
          };
          if (this.debug) {
            console.log(`[DEBUG] ProbeChat executing searchToolInstance with sessionId: ${this.sessionId}`);
          }
          return await searchToolInstance.execute(enhancedParams);
        }
      },
      query: {
        ...queryToolInstance,
        name: "query",
        execute: async (params) => {
          const enhancedParams = {
            ...params,
            sessionId: this.sessionId
          };
          if (this.debug) {
            console.log(`[DEBUG] ProbeChat executing queryToolInstance with sessionId: ${this.sessionId}`);
          }
          return await queryToolInstance.execute(enhancedParams);
        }
      },
      extract: {
        ...extractToolInstance,
        name: "extract",
        execute: async (params) => {
          const enhancedParams = {
            ...params,
            sessionId: this.sessionId
          };
          if (this.debug) {
            console.log(`[DEBUG] ProbeChat executing extractToolInstance with sessionId: ${this.sessionId}`);
          }
          return await extractToolInstance.execute(enhancedParams);
        }
      }
    };

    if (this.debug) {
      console.log(`[DEBUG] Recreated tools with new session ID: ${this.sessionId}`);
    }

    return this.sessionId;
  }

  /**
   * Get the session ID for this chat instance
   * @returns {string} - The session ID
   */
  getSessionId() {
    return this.sessionId;
  }
}