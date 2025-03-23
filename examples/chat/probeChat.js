import 'dotenv/config';
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { generateText } from 'ai';
import { randomUUID } from 'crypto';
import { TokenCounter } from './tokenCounter.js';
import { existsSync } from 'fs';
import { searchTool, queryTool, extractTool, DEFAULT_SYSTEM_MESSAGE } from '@buger/probe';

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
  constructor() {
    // Make allowedFolders accessible as a property of the class
    this.allowedFolders = allowedFolders;

    // Initialize token counter
    this.tokenCounter = new TokenCounter();

    // Generate a unique session ID for this chat instance
    this.sessionId = randomUUID();

    // Get debug mode
    this.debug = process.env.DEBUG === 'true' || process.env.DEBUG === '1';

    if (this.debug) {
      console.log(`[DEBUG] Generated session ID for chat: ${this.sessionId}`);
    }

    // Configure tools with the session ID
    this.configOptions = {
      sessionId: this.sessionId,
      debug: this.debug
    };

    // Create configured tool instances
    this.tools = [
      searchTool(this.configOptions),
      queryTool(this.configOptions),
      extractTool(this.configOptions)
    ];

    // Initialize the chat model
    this.initializeModel();

    // Initialize chat history
    this.history = [];
  }

  /**
   * Initialize the AI model based on available API keys
   */
  initializeModel() {
    // Get API keys from environment variables
    const anthropicApiKey = process.env.ANTHROPIC_API_KEY;
    const openaiApiKey = process.env.OPENAI_API_KEY;

    // Get custom API URLs if provided
    const anthropicApiUrl = process.env.ANTHROPIC_API_URL || 'https://api.anthropic.com/v1';
    const openaiApiUrl = process.env.OPENAI_API_URL || 'https://api.openai.com/v1';

    // Get model override if provided
    const modelName = process.env.MODEL_NAME;

    // Determine which API to use based on available keys
    if (anthropicApiKey) {
      // Initialize Anthropic provider
      this.provider = createAnthropic({
        apiKey: anthropicApiKey,
        baseURL: anthropicApiUrl,
      });
      this.model = modelName || 'claude-3-7-sonnet-latest';
      this.apiType = 'anthropic';

      if (this.debug) {
        console.log(`[DEBUG] Using Anthropic API with model: ${this.model}`);
      }
    } else if (openaiApiKey) {
      // Initialize OpenAI provider
      this.provider = createOpenAI({
        apiKey: openaiApiKey,
        baseURL: openaiApiUrl,
      });
      this.model = modelName || 'gpt-4o-2024-05-13';
      this.apiType = 'openai';

      if (this.debug) {
        console.log(`[DEBUG] Using OpenAI API with model: ${this.model}`);
      }
    } else {
      throw new Error('No API key provided. Please set ANTHROPIC_API_KEY or OPENAI_API_KEY environment variable.');
    }
  }

  /**
   * Get the system message with instructions for the AI
   * @returns {string} - The system message
   */
  getSystemMessage() {
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

    return systemMessage;
  }

  /**
   * Process a user message and get a response
   * @param {string} message - The user message
   * @returns {Promise<string>} - The AI response
   */
  async chat(message) {
    try {
      if (this.debug) {
        console.log(`[DEBUG] Received user message: ${message}`);
      }

      // Count tokens in the user message
      this.tokenCounter.addRequestTokens(message);

      // Limit history to prevent token overflow
      if (this.history.length > MAX_HISTORY_MESSAGES) {
        const historyStart = this.history.length - MAX_HISTORY_MESSAGES;
        this.history = this.history.slice(historyStart);

        if (this.debug) {
          console.log(`[DEBUG] Trimmed history to ${this.history.length} messages`);
        }
      }

      // Prepare messages array
      const messages = [
        ...this.history,
        { role: 'user', content: message }
      ];

      if (this.debug) {
        console.log(`[DEBUG] Sending ${messages.length} messages to model`);
      }

      // Configure generateText options
      const generateOptions = {
        model: this.provider(this.model),
        messages: messages,
        system: this.getSystemMessage(),
        tools: this.tools,
        maxSteps: 15,
        temperature: 0.7,
        maxTokens: 4000
      };

      // Add API-specific options
      if (this.apiType === 'anthropic' && this.model.includes('3-7')) {
        generateOptions.experimental_thinking = {
          enabled: true,
          budget: 8000
        };
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

      // Log tool usage if available
      if (result.toolCalls && result.toolCalls.length > 0) {
        console.log(`Tool was used: ${result.toolCalls.length} times`);

        if (this.debug) {
          result.toolCalls.forEach((call, index) => {
            console.log(`[DEBUG] Tool call ${index + 1}: ${call.name}`);
            if (call.args) {
              console.log(`[DEBUG] Tool call ${index + 1} args:`, JSON.stringify(call.args, null, 2));
            }
            if (call.result) {
              const preview = typeof call.result === 'string'
                ? (call.result.length > 100
                  ? call.result.substring(0, 100) + '... (truncated)'
                  : call.result)
                : JSON.stringify(call.result, null, 2).substring(0, 100) + '... (truncated)';
              console.log(`[DEBUG] Tool call ${index + 1} result preview: ${preview}`);
            }
          });
        }
      }

      return responseText;
    } catch (error) {
      console.error('Error in chat:', error);
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
    this.history = [];
    this.sessionId = randomUUID();
    this.tokenCounter.clear();

    if (this.debug) {
      console.log(`[DEBUG] Cleared chat history; new session ID: ${this.sessionId}`);
    }

    // Reconfigure the tools with the new session ID
    this.configOptions.sessionId = this.sessionId;
    this.tools = [
      searchTool(this.configOptions),
      queryTool(this.configOptions),
      extractTool(this.configOptions)
    ];
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