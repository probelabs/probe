import 'dotenv/config';
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { generateText } from 'ai';
import { randomUUID } from 'crypto';
import { TokenCounter } from './tokenCounter.js';
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

    // Initialize token counter
    this.tokenCounter = new TokenCounter();

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
    let systemMessage = DEFAULT_SYSTEM_MESSAGE || `You are a helpful AI assistant that can search and analyze code repositories using the Probe tool.
You have access to a code search tool that can help you find relevant code snippets.
Always use the search tool first before attempting to answer questions about the codebase.
When responding to questions about code, make sure to include relevant code snippets and explain them clearly.
If you don't know the answer or can't find relevant information, be honest about it.`;

    // Add folder information
    if (allowedFolders.length > 0) {
      const folderList = allowedFolders.map(f => `"${f}"`).join(', ');
      systemMessage += ` The following folders are configured for code search: ${folderList}. When using searchCode, specify one of these folders in the folder argument.`;
    } else {
      systemMessage += ` No specific folders are configured for code search, so the current directory will be used by default. You can omit the path parameter in your search calls, or use '.' to explicitly search in the current directory.`;
    }

    systemMessage += '\n\nWhen appropriate add mermaid diagrams - inside the [] blocks inside diagram wrap to quotes "]';

    // Add file list information if available
    try {
      const searchDirectory = allowedFolders.length > 0 ? allowedFolders[0] : process.cwd();
      if (this.debug) {
        console.log(`[DEBUG] Generating file list for ${searchDirectory}...`);
      }

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
        { role: 'user', content: message }
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

      if (this.debug) {
        console.log(`[DEBUG] Using max tokens: ${maxTokens} for model: ${this.model}`);
      }

      // Configure generateText options
      const generateOptions = {
        model: this.provider(this.model),
        messages: messages,
        system: await this.getSystemMessage(),
        tools: this.tools,
        maxSteps: 10, // Reduced from 15 to help prevent token limit issues
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

          // Write to probe-debug.txt in the current directory
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
            { flag: 'w' } // 'w' flag overwrites the file each time
          );

          console.log(`[DEBUG] Wrote latest AI request to ${debugFilePath} (Est. tokens: ~${totalEstimatedTokens})`);
        } catch (error) {
          console.error(`[DEBUG] Error writing debug file:`, error);
        }
      }

      // console.log("Tools:", JSON.stringify(this.tools, null, 2));

      // Add API-specific options
      if (this.apiType === 'anthropic' && this.model.includes('3-7')) {
        generateOptions.experimental_thinking = {
          enabled: true,
          budget: 8000
        };
      }

      try {
        // Check if the request has been cancelled before making the API call
        if (this.cancelled) {
          throw new Error('Request was cancelled by the user');
        }

        // Generate response using AI model with tools
        const result = await generateText(generateOptions);

        // Extract the text content from the response
        const responseText = result.text;

        // Add the message and response to history
        this.history.push({ role: 'user', content: message });
        this.history.push({ role: 'assistant', content: responseText });

        // Count tokens in the response
        this.tokenCounter.addResponseTokens(responseText);

        // Append the AI response to the debug file
        if (this.debug) {
          try {
            const debugFilePath = join(process.cwd(), 'probe-debug.txt');
            writeFileSync(
              debugFilePath,
              `\n=== AI RESPONSE ===\n${responseText}\n\n` +
              `=== TOKEN USAGE ===\n` +
              `Request tokens: ${this.tokenCounter.requestTokens}\n` +
              `Response tokens: ${this.tokenCounter.responseTokens}\n` +
              `Total tokens: ${this.tokenCounter.requestTokens + this.tokenCounter.responseTokens
              }\n`,
              { flag: 'a' } // 'a' flag appends to the file
            );

            // Also log final "raw message" that we're sending back to the UI (only if DEBUG_CHAT=1)
            writeFileSync(
              debugFilePath,
              `\n=== SENT TO UI (FINAL) ===\n${responseText}\n`,
              { flag: 'a' }
            );

            console.log(`[DEBUG] Appended AI response to ${debugFilePath}`);
          } catch (error) {
            console.error(`[DEBUG] Error appending to debug file:`, error);
          }
        }

        // Log tool usage if available
        if (result.toolCalls && result.toolCalls.length > 0) {
          console.log(`Tool was used: ${result.toolCalls.length} times`);

          // Process each tool call
          result.toolCalls.forEach((call, index) => {
            if (this.debug) {
              console.log(`[DEBUG] Tool call ${index + 1}: ${call.name}`);
              if (call.args) {
                console.log(`[DEBUG] Tool call ${index + 1} args:`, JSON.stringify(call.args, null, 2));
              }

              // Calculate result size for debugging token limits
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

              // Append tool call information to the debug file
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

                // After each tool call, also write the current conversation state to help debug token growth
                const currentMessages = [
                  ...this.history,
                  { role: 'user', content: message }
                ];

                // Estimate total tokens in conversation after this tool call
                let totalEstimatedTokens = 0;
                currentMessages.forEach(m => {
                  totalEstimatedTokens += Math.ceil(m.content.length / 4);
                });

                // Add estimated tokens from tool calls
                if (result.toolCalls) {
                  result.toolCalls.forEach((tc, i) => {
                    if (i <= index) { // Only count up to current tool call
                      const tcResult = typeof tc.result === 'string' ? tc.result : JSON.stringify(tc.result, null, 2);
                      totalEstimatedTokens += Math.ceil(tcResult.length / 4);

                      // Add args tokens
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
            // Note: We no longer need to emit events here as they're emitted directly from the tools
            if (this.debug) {
              console.log(`[DEBUG] Tool call completed: ${call.name}`);
            }
          });
        }

        return responseText;
      } catch (error) {
        // Check if the error is due to cancellation
        if (error.name === 'AbortError' || (error.message && error.message.includes('cancelled'))) {
          console.log('Chat request was cancelled');
          this.cancelled = true;
          throw new Error('Request was cancelled by the user');
        }

        // Re-throw other errors
        throw error;
      }
    } catch (error) {
      console.error('Error in chat:', error);

      // If the error is due to cancellation, propagate it
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
    return this.tokenCounter.getTokenUsage();
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

    // Create configured tool instances that emit SSE events
    // We need to ensure the tools use the correct session ID
    this.tools = {
      // probe: {
      //   ...probeTool,
      //   name: "searchTool",
      //   execute: async (params) => {
      //     // Ensure the session ID is passed to the tool
      //     const enhancedParams = {
      //       ...params,
      //       sessionId: this.sessionId
      //     };
      //     if (this.debug) {
      //       console.log(`[DEBUG] ProbeChat executing probeTool with sessionId: ${this.sessionId}`);
      //     }
      //     return await probeTool.execute(enhancedParams);
      //   }
      // },
      search: {
        ...searchToolInstance,
        name: "search",
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
        name: "query",
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
        name: "extract",
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