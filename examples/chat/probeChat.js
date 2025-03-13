import 'dotenv/config';
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { generateText } from 'ai';
import { randomUUID } from 'crypto';
import { TokenCounter } from './tokenCounter.js';
import { searchTool, queryTool, extractTool } from './tools.js';
import { existsSync } from 'fs';

// Maximum number of messages to keep in history
const MAX_HISTORY_MESSAGES = 20;

// Maximum length for tool results
const MAX_TOOL_RESULT_LENGTH = 100000;

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
    // Initialize token counter
    this.tokenCounter = new TokenCounter();
    
    // Generate a unique session ID for this chat instance
    this.sessionId = randomUUID();
    
    // Get debug mode
    this.debug = process.env.DEBUG === 'true' || process.env.DEBUG === '1';
    
    if (this.debug) {
      console.log(`[DEBUG] Generated session ID for chat: ${this.sessionId}`);
    }
    
    // Store the session ID in an environment variable for tools to access
    process.env.PROBE_SESSION_ID = this.sessionId;
    
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
      // Initialize Anthropic provider with API key and custom URL if provided
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
      // Initialize OpenAI provider with API key and custom URL if provided
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
    let systemMessage = `## Revised Prompt/Guidelines

**Role:**
You are a helpful assistant that can search a codebase using a specialized tool called 'search'. You must:
1. **Always** use the 'search' tool before providing an answer that references the code.
2. Base your code-related answers **only** on what is found using the 'search' tool.
3. Provide detailed answers, including relevant file names and line numbers.

---

### 1. Use the 'search' Tool First
Whenever a user's question involves code (e.g., wanting to see how something is implemented, or referencing functions/classes/configuration), **immediately** make one or more calls to the 'search' tool. 

- **Examples** of usage:
  - If the user asks: "How is request size and response size handled in an analytics plugin?"  
    Use 'search' with queries like:
    '(analytics OR plugin) AND (requestSize OR responseSize)'
    or more refined approaches:
    '+analytics +request +response'
    or:
    'plugin AND "request size" AND "response size"'
    adjusting as needed to narrow or broaden results.

### 2. Elasticsearch-Like Query Syntax
The 'search' tool supports an Elasticsearch-like syntax. Use it to refine searches:
- **Basic Term Search:** 'config'
- **Required Terms:** '+parse +request'
- **Excluded Terms:** 'rpc -test'
- **Logical Operators:** 
  - 'term1 AND term2'
  - 'term1 OR term2'
- **Field-Specific Searching:** 
  - 'function:parse'
  - 'file:PluginAnalytics'
- **Grouping:** '(analytics OR plugin) AND (requestSize OR responseSize)'

**Tips**:
- Start with the most **relevant** keywords.
- Incorporate additional terms (like +, -, AND, OR) only as needed.
- If nothing is found or results are insufficient, **broaden** or **change** your search in subsequent calls.
- Keep queries concise, focusing on key terms that are likely present in the code.

### 3. Provide a Detailed Response
After you have search results from the 'search' tool, **use them directly** to form your answer:
1. **Summarize** what the code does, referencing relevant lines.
2. Include **file names** and **line numbers** for clarity.
3. If the code does not answer the question directly or more context is needed, state that clearly.

### 4. If No Relevant Results
- If you cannot find the requested information after **multiple** refined searches, ask for further context:
  - Example: "I couldn't find references to the 'AnalyticsPlugin' handling both request and response sizes in the codebase. Could you clarify which repository or module you're referring to?"

### 5. General Flow for Each User Query
1. **Read** the user's question.
2. **Determine** relevant keywords and combine them using the syntax guidelines.
3. **Call** the 'search' tool with your best guess.
4. If needed, **refine** or **broaden** your search in one or two more attempts.
5. **Provide** an answer referencing the search results (file name, line number, code snippet).
6. If no results, ask the user to clarify.

---

## Example Interaction Flow

**User:** "Would an analytics plugin provide both request and response sizes?"

1. **search** call:
   '(analytics OR plugin) AND (requestSize OR responseSize)'
2. Suppose the search results show a file 'plugins/AnalyticsPlugin.java' lines 45-60 referencing request size, and lines 61-70 referencing response size.

3. **Answer** (example):
   - Summarize: "Yes, the 'AnalyticsPlugin' in 'plugins/AnalyticsPlugin.java' calculates both request and response sizes."
   - Show code references:
     - Lines 45-60 for request size
     - Lines 61-70 for response size

This flow ensures you always rely on real code references and provide a structured, detailed explanation.`;

    if (allowedFolders.length > 0) {
      const folderList = allowedFolders.map(f => `"${f}"`).join(', ');
      systemMessage += ` The following folders are configured for code search: ${folderList}. When using search, specify one of these folders in the path argument.`;
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
        tools: {
          search: searchTool,
          query: queryTool,
          extract: extractTool
        },
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
              const resultPreview = typeof call.result === 'string' 
                ? (call.result.length > 100 ? call.result.substring(0, 100) + '... (truncated)' : call.result)
                : JSON.stringify(call.result, null, 2).substring(0, 100) + '... (truncated)';
              console.log(`[DEBUG] Tool call ${index + 1} result preview: ${resultPreview}`);
            }
          });
        }
      } else {
        // Check if this is likely a code-related question that should have used tools
        const isCodeQuestion = message.includes('code') || 
                              message.includes('function') || 
                              message.includes('how does') ||
                              message.includes('what is') ||
                              message.includes('where is') ||
                              message.includes('find') ||
                              message.includes('show me');
        
        if (isCodeQuestion) {
          console.log(`[WARNING] AI did not call tools for a likely code-related question: "${message}"`);
          
          if (this.debug) {
            console.log(`[DEBUG] Response without tool call: ${responseText}`);
          }
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
   * Get the session ID for this chat instance
   * @returns {string} - The session ID
   */
  getSessionId() {
    return this.sessionId;
  }
} 
