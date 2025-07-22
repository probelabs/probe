import 'dotenv/config';
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { streamText } from 'ai'; // Removed 'tool' import as it's not used directly here
import { randomUUID } from 'crypto';
import { TokenCounter } from './tokenCounter.js';
import { TokenUsageDisplay } from './tokenUsageDisplay.js';
import { writeFileSync, existsSync } from 'fs';
import { join } from 'path';
import { TelemetryConfig } from './telemetry.js';
import { trace } from '@opentelemetry/api';
import { appTracer } from './appTracer.js';
// Import the tools that emit events and the listFilesByLevel utility
import { listFilesByLevel } from '@buger/probe';
// Import schemas and parser from common (assuming tools.js)
import {
  searchSchema, querySchema, extractSchema, attemptCompletionSchema,
  searchToolDefinition, queryToolDefinition, extractToolDefinition, attemptCompletionToolDefinition, implementToolDefinition,
  listFilesToolDefinition, searchFilesToolDefinition,
  parseXmlToolCallWithThinking
} from './tools.js'; // Assuming common.js is moved to tools/
// Import tool *instances* for execution
import { searchToolInstance, queryToolInstance, extractToolInstance, implementToolInstance, listFilesToolInstance, searchFilesToolInstance } from './probeTool.js'; // Added new tool instances

// Maximum number of messages to keep in history
const MAX_HISTORY_MESSAGES = 100;
// Maximum iterations for the tool loop - configurable via MAX_TOOL_ITERATIONS env var
const MAX_TOOL_ITERATIONS = parseInt(process.env.MAX_TOOL_ITERATIONS || '30', 10);

// Parse and validate allowed folders from environment variable
const allowedFolders = process.env.ALLOWED_FOLDERS
  ? process.env.ALLOWED_FOLDERS.split(',').map(folder => folder.trim()).filter(Boolean)
  : [];

// Validate folders exist on startup - will be handled by index.js in non-interactive mode
// This is kept for backward compatibility with direct ProbeChat usage
const validateFolders = () => {
  if (allowedFolders.length > 0) {
    for (const folder of allowedFolders) {
      const exists = existsSync(folder);
      // Only log if not in non-interactive mode or if in debug mode
      if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
        console.log(`- ${folder} ${exists ? '✓' : '✗ (not found)'}`);
        if (!exists) {
          console.warn(`Warning: Folder "${folder}" does not exist or is not accessible`);
        }
      }
    }
  } else {
    // Only log if not in non-interactive mode or if in debug mode
    if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
      console.warn('No folders configured via ALLOWED_FOLDERS. Tools might default to current directory or require explicit paths.');
    }
  }
};

// Only validate folders on startup if not in non-interactive mode
if (typeof process !== 'undefined' && !process.env.PROBE_CHAT_SKIP_FOLDER_VALIDATION) {
  validateFolders();
}


/**
 * Extract image URLs from message text
 * @param {string} message - The message text to analyze
 * @param {boolean} debug - Whether to log debug information
 * @returns {Array} Array of { url: string, cleanedMessage: string }
 */
function extractImageUrls(message, debug = false) {
  // This function should be called within the session context, so it will inherit the trace ID
  const tracer = trace.getTracer('probe-chat', '1.0.0');
  return tracer.startActiveSpan('content.image.extract', (span) => {
    try {
      // Pattern to match image URLs:
      // 1. GitHub private-user-images URLs (always images, regardless of extension)
      // 2. GitHub user-attachments/assets URLs (always images, regardless of extension)
      // 3. URLs with common image extensions (PNG, JPG, JPEG, WebP, GIF)
      // Updated to stop at quotes, spaces, or common HTML/XML delimiters
      const imageUrlPattern = /https?:\/\/(?:(?:private-user-images\.githubusercontent\.com|github\.com\/user-attachments\/assets)\/[^\s"'<>]+|[^\s"'<>]+\.(?:png|jpg|jpeg|webp|gif)(?:\?[^\s"'<>]*)?)/gi;
      
      span.setAttributes({
        'message.length': message.length,
        'debug.enabled': debug
      });
      
      if (debug) {
        console.log(`[DEBUG] Scanning message for image URLs. Message length: ${message.length}`);
        console.log(`[DEBUG] Image URL pattern: ${imageUrlPattern.toString()}`);
      }
      
      const urls = [];
      let match;
      
      while ((match = imageUrlPattern.exec(message)) !== null) {
        urls.push(match[0]);
        if (debug) {
          console.log(`[DEBUG] Found image URL: ${match[0]}`);
        }
      }
      
      // Remove image URLs from message text
      const cleanedMessage = message.replace(imageUrlPattern, '').trim();
      
      span.setAttributes({
        'images.found': urls.length,
        'message.cleaned_length': cleanedMessage.length
      });
      
      if (debug) {
        console.log(`[DEBUG] Total image URLs found: ${urls.length}`);
        if (urls.length > 0) {
          console.log(`[DEBUG] Original message length: ${message.length}, cleaned message length: ${cleanedMessage.length}`);
        }
      }
      
      const result = {
        imageUrls: urls,
        cleanedMessage: cleanedMessage
      };
      
      span.setStatus({ code: 1 }); // SUCCESS
      return result;
    } catch (error) {
      span.recordException(error);
      span.setStatus({ code: 2, message: error.message }); // ERROR
      throw error;
    } finally {
      span.end();
    }
  });
}

/**
 * Validate image URLs by checking if they're accessible, handling redirects
 * @param {string[]} imageUrls - Array of image URLs to validate
 * @param {boolean} debug - Whether to log debug messages
 * @returns {Promise<string[]>} Array of valid final image URLs (after redirects)
 */
async function validateImageUrls(imageUrls, debug = false) {
  const validUrls = [];
  
  for (const url of imageUrls) {
    try {
      // Always use GET request with Range header to validate and get content type
      // This works better than HEAD for GitHub URLs and other services
      const response = await fetch(url, {
        method: 'GET',
        headers: {
          'Range': 'bytes=0-1023' // Only fetch first 1KB to check content type and minimize data transfer
        },
        timeout: 10000, // 10 second timeout for GitHub URLs which can be slower
        redirect: 'follow'
      });
      
      if (response.ok || response.status === 206) { // 206 = Partial Content (from Range header)
        // Check if the response has image content type
        const contentType = response.headers.get('content-type');
        if (contentType && contentType.startsWith('image/')) {
          // Use the final URL after following redirects
          const finalUrl = response.url;
          validUrls.push(finalUrl);
          if (debug) {
            if (finalUrl !== url) {
              console.log(`[DEBUG] Valid image URL after redirect: ${url} -> ${finalUrl} (${contentType})`);
            } else {
              console.log(`[DEBUG] Valid image URL: ${finalUrl} (${contentType})`);
            }
          }
        } else {
          if (debug) {
            console.log(`[DEBUG] URL not an image: ${url} (${contentType || 'unknown type'})`);
          }
        }
      } else {
        if (debug) {
          console.log(`[DEBUG] URL not accessible: ${url} (status: ${response.status})`);
        }
      }
    } catch (error) {
      if (debug) {
        console.log(`[DEBUG] Error validating image URL ${url}: ${error.message}`);
      }
    }
  }
  
  return validUrls;
}

/**
 * ProbeChat class to handle chat interactions with AI models
 */
export class ProbeChat {
  /**
   * Create a new ProbeChat instance
   * @param {Object} options - Configuration options
   * @param {string} [options.sessionId] - Optional session ID
   * @param {boolean} [options.isNonInteractive=false] - Suppress internal logs if true
   * @param {Function} [options.toolCallCallback] - Callback function for tool calls (sessionId, toolCallData) - *Note: Callback may need adjustment for XML flow*
   * @param {string} [options.customPrompt] - Custom prompt to replace the default system message
   * @param {string} [options.promptType] - Predefined prompt type (architect, code-review, support)
   * @param {boolean} [options.allowEdit=false] - Allow the use of the 'implement' tool
   */
  constructor(options = {}) {
    // Suppress internal logs if in non-interactive mode
    this.isNonInteractive = !!options.isNonInteractive;
    // Flag to track if a request has been cancelled
    this.cancelled = false;

    // AbortController for cancelling fetch requests
    this.abortController = null;
    // Make allowedFolders accessible as a property of the class
    this.allowedFolders = allowedFolders;

    // Store custom prompt or prompt type if provided
    this.customPrompt = options.customPrompt || process.env.CUSTOM_PROMPT || null;
    this.promptType = options.promptType || process.env.PROMPT_TYPE || null;

    // Store allowEdit flag - enable if allow_edit is set or if allow_suggestions is set via environment
    // Note: ALLOW_SUGGESTIONS also enables allowEdit because the implement tool is needed to generate
    // code changes that reviewdog can then convert into PR review suggestions
    this.allowEdit = !!options.allowEdit || process.env.ALLOW_EDIT === '1' || process.env.ALLOW_SUGGESTIONS === '1';

    // Store client-provided API credentials if available
    this.clientApiProvider = options.apiProvider;
    this.clientApiKey = options.apiKey;
    this.clientApiUrl = options.apiUrl;

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
      console.log(`[DEBUG] Maximum tool iterations configured: ${MAX_TOOL_ITERATIONS}`);
      console.log(`[DEBUG] Allow Edit (implement tool): ${this.allowEdit}`);
    }

    // Store tool instances for execution
    // These are the actual functions/objects that perform the actions
    this.toolImplementations = {
      search: searchToolInstance,
      query: queryToolInstance,
      extract: extractToolInstance,
      listFiles: listFilesToolInstance,
      searchFiles: searchFilesToolInstance,
      // attempt_completion is handled specially in the loop, no direct implementation needed here
    };

    // Conditionally add the implement tool if allowed
    if (this.allowEdit) {
      this.toolImplementations.implement = implementToolInstance;
    }

    // Initialize the chat model
    this.initializeModel();

    // Initialize telemetry
    this.initializeTelemetry();

    // Initialize chat history
    this.history = [];
  }

  /**
   * Initialize the AI model based on available API keys and forced provider setting
   */
  initializeModel() {
    // Get API keys from environment variables or client-provided values
    const anthropicApiKey = this.clientApiKey && this.clientApiProvider === 'anthropic' ?
      this.clientApiKey : process.env.ANTHROPIC_API_KEY;
    const openaiApiKey = this.clientApiKey && this.clientApiProvider === 'openai' ?
      this.clientApiKey : process.env.OPENAI_API_KEY;
    const googleApiKey = this.clientApiKey && this.clientApiProvider === 'google' ?
      this.clientApiKey : process.env.GOOGLE_API_KEY;

    // Get custom API URLs if provided (client URL takes precedence over environment variables)
    const llmBaseUrl = process.env.LLM_BASE_URL;           // Generic base URL for all providers

    // For each provider, use client URL if available and matches the provider
    const anthropicApiUrl = (this.clientApiUrl && this.clientApiProvider === 'anthropic') ?
      this.clientApiUrl : (process.env.ANTHROPIC_API_URL || llmBaseUrl);

    const openaiApiUrl = (this.clientApiUrl && this.clientApiProvider === 'openai') ?
      this.clientApiUrl : (process.env.OPENAI_API_URL || llmBaseUrl);

    const googleApiUrl = (this.clientApiUrl && this.clientApiProvider === 'google') ?
      this.clientApiUrl : (process.env.GOOGLE_API_URL || llmBaseUrl);

    // Get model override if provided
    const modelName = process.env.MODEL_NAME;

    // Check if client has specified a provider that should be forced
    const clientForceProvider = this.clientApiProvider && this.clientApiKey ? this.clientApiProvider : null;

    // Use client-forced provider or environment variable
    const forceProvider = clientForceProvider || (process.env.FORCE_PROVIDER ? process.env.FORCE_PROVIDER.toLowerCase() : null);

    if (this.debug) {
      console.log(`[DEBUG] Available API keys: Anthropic=${!!anthropicApiKey}, OpenAI=${!!openaiApiKey}, Google=${!!googleApiKey}`);
      console.log(`[DEBUG] Force provider: ${forceProvider || '(not set)'}`);
      if (llmBaseUrl) console.log(`[DEBUG] Generic LLM Base URL: ${llmBaseUrl}`);
      if (process.env.ANTHROPIC_API_URL) console.log(`[DEBUG] Custom Anthropic URL: ${anthropicApiUrl}`);
      if (process.env.OPENAI_API_URL) console.log(`[DEBUG] Custom OpenAI URL: ${openaiApiUrl}`);
      if (process.env.GOOGLE_API_URL) console.log(`[DEBUG] Custom Google URL: ${googleApiUrl}`);
      if (modelName) console.log(`[DEBUG] Model override: ${modelName}`);
    }

    // Check if a specific provider is forced

    if (forceProvider) {
      if (!this.isNonInteractive || this.debug) {
        console.log(`Provider forced to: ${forceProvider}`);
      }

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

      console.warn(`WARNING: Forced provider "${forceProvider}" selected but required API key is missing or invalid! Falling back to auto-detection.`);
    }

    // If no provider is forced or forced provider failed, use the first available API key
    if (anthropicApiKey) {
      this.initializeAnthropicModel(anthropicApiKey, anthropicApiUrl, modelName);
    } else if (openaiApiKey) {
      this.initializeOpenAIModel(openaiApiKey, openaiApiUrl, modelName);
    } else if (googleApiKey) {
      this.initializeGoogleModel(googleApiKey, googleApiUrl, modelName);
    } else {
      console.error('FATAL: No API key provided. Please set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY environment variable.');
      this.noApiKeysMode = true; // Use flag for potential UI handling
      this.model = 'none';
      this.apiType = 'none';
      console.log('ProbeChat cannot function without an API key.');
      // Consider throwing an error here in a real application to prevent execution
      // throw new Error('No API key configured for AI provider.');
    }
  }

  /**
   * Initialize Anthropic model
   * @param {string} apiKey - Anthropic API key
   * @param {string} [apiUrl] - Optional Anthropic API URL override
   * @param {string} [modelName] - Optional model name override
   */
  initializeAnthropicModel(apiKey, apiUrl, modelName) {
    this.provider = createAnthropic({
      apiKey: apiKey,
      ...(apiUrl && { baseURL: apiUrl }), // Conditionally add baseURL
    });
    this.model = modelName || 'claude-3-7-sonnet-20250219';
    this.apiType = 'anthropic';
    if (!this.isNonInteractive || this.debug) {
      const urlSource = process.env.ANTHROPIC_API_URL ? 'ANTHROPIC_API_URL' :
        (process.env.LLM_BASE_URL ? 'LLM_BASE_URL' : 'default');
      console.log(`Using Anthropic API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl}, from: ${urlSource})` : ''}`);
    }
  }

  /**
   * Initialize OpenAI model
   * @param {string} apiKey - OpenAI API key
   * @param {string} [apiUrl] - Optional OpenAI API URL override
   * @param {string} [modelName] - Optional model name override
   */
  initializeOpenAIModel(apiKey, apiUrl, modelName) {
    this.provider = createOpenAI({
      compatibility: 'strict',
      apiKey: apiKey,
      ...(apiUrl && { baseURL: apiUrl }), // Conditionally add baseURL
    });
    this.model = modelName || 'gpt-4o';
    this.apiType = 'openai';
    if (!this.isNonInteractive || this.debug) {
      const urlSource = process.env.OPENAI_API_URL ? 'OPENAI_API_URL' :
        (process.env.LLM_BASE_URL ? 'LLM_BASE_URL' : 'default');
      console.log(`Using OpenAI API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl}, from: ${urlSource})` : ''}`);
    }
  }

  /**
   * Initialize Google model
   * @param {string} apiKey - Google API key
   * @param {string} [apiUrl] - Optional Google API URL override
   * @param {string} [modelName] - Optional model name override
   */
  initializeGoogleModel(apiKey, apiUrl, modelName) {
    this.provider = createGoogleGenerativeAI({
      apiKey: apiKey,
      ...(apiUrl && { baseURL: apiUrl }), // Conditionally add baseURL
    });
    this.model = modelName || 'gemini-2.0-flash';
    this.apiType = 'google';
    if (!this.isNonInteractive || this.debug) {
      const urlSource = process.env.GOOGLE_API_URL ? 'GOOGLE_API_URL' :
        (process.env.LLM_BASE_URL ? 'LLM_BASE_URL' : 'default');
      console.log(`Using Google API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl}, from: ${urlSource})` : ''}`);
    }
    // Note: Google's tool support might differ. Ensure XML approach works reliably.
  }

  /**
   * Initialize telemetry configuration
   */
  initializeTelemetry() {
    try {
      // Check if telemetry is enabled via environment variables
      const fileEnabled = process.env.OTEL_ENABLE_FILE === 'true';
      const remoteEnabled = process.env.OTEL_ENABLE_REMOTE === 'true';
      const consoleEnabled = process.env.OTEL_ENABLE_CONSOLE === 'true';
      
      if (fileEnabled || remoteEnabled || consoleEnabled) {
        this.telemetryConfig = new TelemetryConfig({
          enableFile: fileEnabled,
          enableRemote: remoteEnabled,
          enableConsole: consoleEnabled,
          filePath: process.env.OTEL_FILE_PATH || './traces.jsonl',
          remoteEndpoint: process.env.OTEL_EXPORTER_OTLP_TRACES_ENDPOINT || 'http://localhost:4318/v1/traces'
        });
        
        this.telemetryConfig.initialize();
        
        if (this.debug) {
          console.log('[DEBUG] Telemetry initialized successfully');
        }
      } else {
        if (this.debug) {
          console.log('[DEBUG] Telemetry disabled - no exporters configured');
        }
      }
    } catch (error) {
      console.error('Failed to initialize telemetry:', error.message);
      this.telemetryConfig = null;
    }
  }

  /**
    * Get the system message with instructions for the AI (XML Tool Format)
    * @returns {Promise<string>} - The system message
    */
  async getSystemMessage() {
    // --- Dynamically build Tool Definitions ---
    let toolDefinitions = `
${searchToolDefinition}
${queryToolDefinition}
${extractToolDefinition}
${listFilesToolDefinition}
${searchFilesToolDefinition}
${attemptCompletionToolDefinition}
`;
    if (this.allowEdit) {
      toolDefinitions += `${implementToolDefinition}\n`;
    }

    // --- Dynamically build Tool Guidelines ---
    let xmlToolGuidelines = `
# Tool Use Formatting

Tool use MUST be formatted using XML-style tags. The tool name is enclosed in opening and closing tags, and each parameter is similarly enclosed within its own set of tags. You MUST use exactly ONE tool call per message until you are ready to complete the task.

Structure:
<tool_name>
<parameter1_name>value1</parameter1_name>
<parameter2_name>value2</parameter2_name>
...
</tool_name>

Example:
<search>
<query>error handling</query>
<path>src/search</path>
</search>

# Thinking Process

Before using a tool, analyze the situation within <thinking></thinking> tags. This helps you organize your thoughts and make better decisions. Your thinking process should include:

1. Analyze what information you already have and what information you need to proceed with the task.
2. Determine which of the available tools would be most effective for gathering this information or accomplishing the current step.
3. Check if all required parameters for the tool are available or can be inferred from the context.
4. If all parameters are available, proceed with the tool use.
5. If parameters are missing, explain what's missing and why it's needed.

Example:
<thinking>
I need to find code related to error handling in the search module. The most appropriate tool for this is the search tool, which requires a query parameter and a path parameter. I have both the query ("error handling") and the path ("src/search"), so I can proceed with the search.
</thinking>

# Tool Use Guidelines

1.  Think step-by-step about how to achieve the user's goal.
2.  Use <thinking></thinking> tags to analyze the situation and determine the appropriate tool.
3.  Choose **one** tool that helps achieve the current step.
4.  Format the tool call using the specified XML format. Ensure all required parameters are included.
5.  **You MUST respond with exactly one tool call in the specified XML format in each turn.**
6.  Wait for the tool execution result, which will be provided in the next message (within a <tool_result> block).
7.  Analyze the tool result and decide the next step. If more tool calls are needed, repeat steps 2-6.
8.  If the task is fully complete and all previous steps were successful, use the \`<attempt_completion>\` tool to provide the final answer. This is the ONLY way to finish the task.
9.  If you cannot proceed (e.g., missing information, invalid request), explain the issue clearly before using \`<attempt_completion>\` with an appropriate message in the \`<result>\` tag.
10. Do not be lazy and dig to the topic as deep as possible, until you see full picture.

Available Tools:
- search: Search code using keyword queries.
- query: Search code using structural AST patterns.
- extract: Extract specific code blocks or lines from files.
- listFiles: List files and directories in a specified location.
- searchFiles: Find files matching a glob pattern with recursive search capability.
${this.allowEdit ? '- implement: Implement a feature or fix a bug using aider.\n' : ''}
- attempt_completion: Finalize the task and provide the result to the user.
`;
    // Common instructions that will be added to all prompts
    const commonInstructions = `<instructions>
Follow these instructions carefully:
1.  Analyze the user's request.
2.  Use <thinking></thinking> tags to analyze the situation and determine the appropriate tool for each step.
3.  Use the available tools step-by-step to fulfill the request.
4.  You should always prefer the \`search\` tool for code-related questions. Read full files only if really necessary.
4.  Ensure to get really deep and understand the full picture before answering. Ensure to check dependencies where required.
5.  You MUST respond with exactly ONE tool call per message, using the specified XML format, until the task is complete.
6.  Wait for the tool execution result (provided in the next user message in a <tool_result> block) before proceeding to the next step.
7.  Once the task is fully completed, and you have confirmed the success of all steps, use the '<attempt_completion>' tool to provide the final result. This is the ONLY way to signal completion.
8.  Prefer concise and focused search queries. Use specific keywords and phrases to narrow down results. Avoid reading files in full, only when absolutely necessary.
9.  Show mermaid diagrams to illustrate complex code structures or workflows. In diagrams, content inside ["..."] always should be in quotes.</instructions>
`;

    // Define predefined prompts (without the common instructions)
    const predefinedPrompts = {
      'code-explorer': `You are ProbeChat Code Explorer, a specialized AI assistant focused on helping developers, product managers, and QAs understand and navigate codebases. Your primary function is to answer questions based on code, explain how systems work, and provide insights into code functionality using the provided code analysis tools.

When exploring code:
- Provide clear, concise explanations based on user request
- Find and highlight the most relevant code snippets, if required
- Trace function calls and data flow through the system
- Use diagrams to illustrate code structure and relationships when helpful
- Try to understand the user's intent and provide relevant information
- Understand high level picture
- Balance detail with clarity in your explanations`,

      'architect': `You are ProbeChat Architect, a specialized AI assistant focused on software architecture and design. Your primary function is to help users understand, analyze, and design software systems using the provided code analysis tools. You excel at identifying architectural patterns, suggesting improvements, and creating high-level design documentation. You provide detailed and accurate responses to user queries about system architecture, component relationships, and code organization.

When analyzing code:
- Focus on high-level design patterns and system organization
- Identify architectural patterns and component relationships
- Evaluate system structure and suggest architectural improvements
- Create diagrams to illustrate system architecture and workflows
- Consider scalability, maintainability, and extensibility in your analysis`,

      'code-review': `You are ProbeChat Code Reviewer, a specialized AI assistant focused on code quality and best practices. Your primary function is to help users identify issues, suggest improvements, and ensure code follows best practices using the provided code analysis tools. You excel at spotting bugs, performance issues, security vulnerabilities, and style inconsistencies. You provide detailed and constructive feedback on code quality.

When reviewing code:
- Look for bugs, edge cases, and potential issues
- Identify performance bottlenecks and optimization opportunities
- Check for security vulnerabilities and best practices
- Evaluate code style and consistency
- Is the backward compatibility can be broken?
- Organize feedback by severity (critical, major, minor) and type (bug, performance, security, style)
- Provide specific, actionable suggestions with code examples where appropriate

## Failure Detection

If you detect critical issues that should prevent the code from being merged, include <fail> in your response:
- Security vulnerabilities that could be exploited
- Breaking changes without proper documentation or migration path
- Critical bugs that would cause system failures
- Severe violations of project standards that must be addressed

The <fail> tag will cause the GitHub check to fail, drawing immediate attention to these critical issues.`,

      'engineer': `You are senior engineer focused on software architecture and design.
Before jumping on the task you first, in details analyse user request, and try to provide elegant and concise solution.
If solution is clear, you can jump to implementation right away, if not, you can ask user a clarification question, by calling attempt_completion tool, with required details.
You are allowed to use search tool with allow_tests argument, in order to find the tests.

Before jumping to implementation:
- Focus on high-level design patterns and system organization
- Identify architectural patterns and component relationships
- Evaluate system structure and suggest architectural improvements
- Focus on backward compatibility.
- Respond with diagrams to illustrate system architecture and workflows, if required.
- Consider scalability, maintainability, and extensibility in your analysis

During the implementation:
- Avoid implementing special cases
- Do not forget to add the tests`,

      'support': `You are ProbeChat Support, a specialized AI assistant focused on helping developers troubleshoot issues and solve problems. Your primary function is to help users diagnose errors, understand unexpected behaviors, and find solutions using the provided code analysis tools. You excel at debugging, explaining complex concepts, and providing step-by-step guidance. You provide detailed and patient support to help users overcome technical challenges.

When troubleshooting:
- Focus on finding root causes, not just symptoms
- Explain concepts clearly with appropriate context
- Provide step-by-step guidance to solve problems
- Suggest diagnostic steps to verify solutions
- Consider edge cases and potential complications
- Be empathetic and patient in your explanations`
    };

    let systemMessage = '';

    // Use custom prompt if provided
    if (this.customPrompt) {
      // For custom prompts, use the entire content as is
      systemMessage = "<role>" + this.customPrompt + "</role>";
      if (this.debug) {
        console.log(`[DEBUG] Using custom prompt`);
      }
    }
    // Use predefined prompt if specified
    else if (this.promptType && predefinedPrompts[this.promptType]) {
      systemMessage = "<role>" + predefinedPrompts[this.promptType] + "</role>";
      if (this.debug) {
        console.log(`[DEBUG] Using predefined prompt: ${this.promptType}`);
      }
      // Add common instructions to predefined prompts
      systemMessage += commonInstructions;
    } else {
      // Use the default prompt (code explorer) if no prompt type is specified
      systemMessage = "<role>" + predefinedPrompts['code-explorer'] + "</role>";
      if (this.debug) {
        console.log(`[DEBUG] Using default prompt: code explorer`);
      }
      // Add common instructions to the default prompt
      systemMessage += commonInstructions;
    }
    // Add XML Tool Guidelines
    systemMessage += `\n${xmlToolGuidelines}\n`;

    // Add Tool Definitions
    systemMessage += `\n# Tools Available\n${toolDefinitions}\n`;


    const searchDirectory = this.allowedFolders.length > 0 ? this.allowedFolders[0] : process.cwd();
    if (this.debug) {
      console.log(`[DEBUG] Generating file list for base directory: ${searchDirectory}...`);
    }

    // Add folder information
    if (this.allowedFolders.length > 0) {
      const folderList = this.allowedFolders.map(f => `"${f}"`).join(', ');
      systemMessage += `\n\nYou are configured to primarily operate within these folders: ${folderList}. When using tools like 'search' or 'query', the 'path' parameter should generally refer to these folders or subpaths within them. The root for relative paths is considered the project base.`;
    } else {
      systemMessage += `\n\nCurrent path: ${searchDirectory}. When using tools, specify paths like '.' for the current directory, 'src/utils', etc., within the 'path' parameter. Dependencies are located in /dep folder: "/dep/go/github.com/user/repo", "/dep/js/<package>", "/dep/rust/crate_name".`;
    }

    // Add Rules/Capabilities section
    systemMessage += `\n\n# Capabilities & Rules\n- Search given folder using keywords (\`search\`) or structural patterns (\`query\`).\n- Extract specific code blocks or full files using (\`extract\`).\n- File paths are relative to the project base unless using dependency syntax.\n- Always wait for tool results (\`<tool_result>...\`) before proceeding.\n- Use \`attempt_completion\` ONLY when the entire task is finished.\n- Be direct and technical. Use exactly ONE tool call per response in the specified XML format. Prefer using search tool.\n`;

    if (this.debug) {
      console.log(`[DEBUG] Base system message length (pre-file list): ${systemMessage.length}`);
    }

    // Add file list information if available
    try {
      let files = await listFilesByLevel({
        directory: searchDirectory, // Use the determined search directory
        maxFiles: 100, // Keep it reasonable
        respectGitignore: true
      });

      // Exclude debug file(s) and common large directories
      files = files.filter((file) => {
        const lower = file.toLowerCase();
        return !lower.includes('probe-debug.txt') && !lower.includes('node_modules') && !lower.includes('/.git/');
      });

      if (files.length > 0) {
        const fileListHeader = `\n\n# Project Files (Sample of up to ${files.length} files in ${searchDirectory}):\n`;
        const fileListContent = files.map(file => `- ${file}`).join('\n');
        systemMessage += fileListHeader + fileListContent;
        if (this.debug) {
          console.log(`[DEBUG] Added ${files.length} files to system message. Total length: ${systemMessage.length}`);
        }
      } else {
        if (this.debug) {
          console.log(`[DEBUG] No files found or listed for the project directory: ${searchDirectory}.`);
        }
        systemMessage += `\n\n# Project Files\nNo files listed for the primary directory (${searchDirectory}). You may need to use tools like 'search' or 'query' with broad paths initially if the user's request requires file exploration.`;
      }
    } catch (error) {
      console.warn(`Warning: Could not generate file list for directory "${searchDirectory}": ${error.message}`);
      systemMessage += `\n\n# Project Files\nCould not retrieve file listing. Proceed based on user instructions and tool capabilities.`;
    }

    if (this.debug) {
      console.log(`[DEBUG] Final system message length: ${systemMessage.length}`);
      // Log first/last parts for verification
      const debugFilePath = join(process.cwd(), 'probe-debug-system-prompt.txt');
      try {
        writeFileSync(debugFilePath, systemMessage);
        console.log(`[DEBUG] Full system prompt saved to ${debugFilePath}`);
      } catch (e) {
        console.error(`[DEBUG] Failed to write full system prompt: ${e.message}`);
        console.log(`[DEBUG] System message START:\n${systemMessage.substring(0, 300)}...`);
        console.log(`[DEBUG] System message END:\n...${systemMessage.substring(systemMessage.length - 300)}`);
      }
    }

    return systemMessage;
  }

  /**
   * Abort the current chat request
   */
  abort() {
    if (!this.isNonInteractive || this.debug) {
      console.log(`Aborting chat for session: ${this.sessionId}`);
    }
    this.cancelled = true;

    // Abort any fetch requests
    if (this.abortController) {
      try {
        this.abortController.abort('User cancelled request'); // Pass reason
      } catch (error) {
        // Ignore errors if already aborted or controller is in an unexpected state
        if (error.name !== 'AbortError') {
          console.error('Error aborting fetch request:', error);
        }
      }
    }
  }

  /**
   * Process a user message and get a response
   * @param {string} message - The user message
   * @param {string} [sessionId] - Optional session ID to use for this chat (overrides the default)
   * @returns {Promise<string>} - The AI response
   */
  async chat(message, sessionId, apiCredentials = null) {
    // Use our custom app tracer for granular tracing
    const effectiveSessionId = sessionId || this.sessionId;
    
    // Start the chat session span first, then execute the entire chat flow within the session context
    const chatSessionSpan = appTracer.startChatSession(effectiveSessionId, message, this.apiType, this.model);
    
    // Execute the entire chat flow within the session context
    return await appTracer.withSessionContext(effectiveSessionId, async () => {
    
    try {

        // Update client credentials if provided in this call
        if (apiCredentials) {
          this.clientApiProvider = apiCredentials.apiProvider || this.clientApiProvider;
          this.clientApiKey = apiCredentials.apiKey || this.clientApiKey;
          this.clientApiUrl = apiCredentials.apiUrl || this.clientApiUrl;

          // Re-initialize the model with the new credentials
          if (apiCredentials.apiKey && apiCredentials.apiProvider) {
            this.initializeModel();
          }
        }

        // Handle no API keys mode gracefully
        if (this.noApiKeysMode) {
          console.error("Cannot process chat: No API keys configured.");
          appTracer.endChatSession(effectiveSessionId, false, 0);
          // Return structured response even for API key errors
          return {
            response: "Error: ProbeChat is not configured with an AI provider API key. Please set the appropriate environment variable (e.g., ANTHROPIC_API_KEY, OPENAI_API_KEY) or provide an API key in the browser.",
            tokenUsage: { contextWindow: 0, current: {}, total: {} }
          };
        }

        // Reset cancelled flag for the new request
        this.cancelled = false;

        // Create a new AbortController for this specific request
        // This ensures previous cancellations don't affect new requests
        this.abortController = new AbortController();

        // If a session ID is provided and it's different from the current one, update it
        if (sessionId && sessionId !== this.sessionId) {
          if (this.debug) {
            console.log(`[DEBUG] Switching session ID from ${this.sessionId} to ${sessionId}`);
          }
          // Update the session ID for this instance
          this.sessionId = sessionId;
          // NOTE: History is NOT cleared automatically when session ID changes this way.
          // Call clearHistory() explicitly if a new session should start fresh.
        }

        // Process the message using the potentially updated session ID
        const result = await this._processChat(message, effectiveSessionId);
        
        appTracer.endChatSession(effectiveSessionId, true, result.tokenUsage?.total?.total || 0);
        
        // CRITICAL FIX: Ensure all spans are properly exported before returning
        if (this.telemetryConfig) {
          try {
            // First, ensure the session span is ended within its context
            await appTracer.withSessionContext(effectiveSessionId, async () => {
              // Small delay to ensure all child spans are ended
              await new Promise(resolve => setTimeout(resolve, 50));
            });
            
            // Give BatchSpanProcessor time to process the ended spans
            // BatchSpanProcessor has a scheduledDelayMillis of 500ms (reduced from default 5000ms)
            await new Promise(resolve => setTimeout(resolve, 600));
            
            // Force flush all pending spans
            await this.telemetryConfig.forceFlush();
            
            // Additional delay to ensure file writes complete
            await new Promise(resolve => setTimeout(resolve, 100));
          } catch (flushError) {
            if (this.debug) console.log('[DEBUG] Telemetry flush warning:', flushError.message);
          }
        }
        
        return result;
      } catch (error) {
        appTracer.endChatSession(effectiveSessionId, false, 0);
        
        // CRITICAL FIX: Ensure all spans are properly exported even on error
        if (this.telemetryConfig) {
          try {
            // First, ensure the session span is ended within its context
            await appTracer.withSessionContext(effectiveSessionId, async () => {
              // Small delay to ensure all child spans are ended
              await new Promise(resolve => setTimeout(resolve, 50));
            });
            
            // Give BatchSpanProcessor time to process the ended spans
            // BatchSpanProcessor has a scheduledDelayMillis of 500ms (reduced from default 5000ms)
            await new Promise(resolve => setTimeout(resolve, 600));
            
            // Force flush all pending spans
            await this.telemetryConfig.forceFlush();
            
            // Additional delay to ensure file writes complete
            await new Promise(resolve => setTimeout(resolve, 100));
          } catch (flushError) {
            if (this.debug) console.log('[DEBUG] Telemetry flush warning:', flushError.message);
          }
        }
        
        throw error;
      }
    }); // End withSessionContext
  }

  /**
   * Internal method to process a chat message using the XML tool loop
   * @param {string} message - The user message
   * @param {string} sessionId - The session ID for tracing
   * @returns {Promise<string>} - The final AI response after loop completion
   * @private
   */
  async _processChat(message, sessionId) {
    let currentIteration = 0;
    let completionAttempted = false;
    let finalResult = `Error: Max tool iterations (${MAX_TOOL_ITERATIONS}) reached without completion. You can increase this limit using the MAX_TOOL_ITERATIONS environment variable or --max-iterations flag.`; // Default error

    this.abortController = new AbortController();
    const debugFilePath = join(process.cwd(), 'probe-debug.txt');

    try {
      if (this.debug) {
        console.log(`[DEBUG] ===== Starting XML Tool Chat Loop (Session: ${this.sessionId}) =====`);
        console.log(`[DEBUG] Received user message: ${message}`);
        console.log(`[DEBUG] Initial history length: ${this.history.length}`);
      }

      this.tokenCounter.startNewTurn();
      this.tokenCounter.addRequestTokens(this.tokenCounter.countTokens(message));

      if (this.history.length > MAX_HISTORY_MESSAGES) {
        const removedCount = this.history.length - MAX_HISTORY_MESSAGES;
        this.history = this.history.slice(removedCount);
        if (this.debug) console.log(`[DEBUG] Trimmed history to ${this.history.length} messages (removed ${removedCount}).`);
      }

      const isFirstMessage = this.history.length === 0;
      
      // Start user message processing trace
      const messageId = `msg_${Date.now()}`;
      appTracer.startUserMessageProcessing(sessionId, messageId, message);
      
      // Extract image URLs from the message within the processing context
      const { imageUrls, cleanedMessage } = appTracer.withUserProcessingContext(sessionId, () => 
        extractImageUrls(message, this.debug)
      );
      
      // Start image processing trace if images are found
      if (imageUrls.length > 0) {
        appTracer.startImageProcessing(sessionId, messageId, imageUrls, cleanedMessage.length);
        if (this.debug) console.log(`[DEBUG] Found ${imageUrls.length} image URLs in message`);
      }
      
      // Log image detection only in interactive mode or debug mode
      if (imageUrls.length > 0) {
        if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
          console.log(`Detected ${imageUrls.length} image URL(s) in message.`);
        }
        if (this.debug) {
          console.log(`[DEBUG] Extracted image URLs:`, imageUrls);
        }
      }
      
      // Validate image URLs and filter out broken ones
      let validImageUrls = [];
      let validationResults = null;
      
      if (imageUrls.length > 0) {
        const validationStartTime = Date.now();
        validImageUrls = await validateImageUrls(imageUrls, this.debug);
        const validationEndTime = Date.now();
        
        // Record validation results in trace
        validationResults = {
          totalUrls: imageUrls.length,
          validUrls: validImageUrls.length,
          invalidUrls: imageUrls.length - validImageUrls.length,
          redirectedUrls: 0, // TODO: capture from validateImageUrls if needed
          timeoutUrls: 0, // TODO: capture from validateImageUrls if needed  
          networkErrors: 0, // TODO: capture from validateImageUrls if needed
          durationMs: validationEndTime - validationStartTime
        };
        
        appTracer.recordImageValidation(sessionId, validationResults);
        appTracer.endImageProcessing(sessionId, validImageUrls.length > 0, validImageUrls.length);
      } else {
        validImageUrls = await validateImageUrls(imageUrls, this.debug);
      }
      
      // Start the agent loop trace within user processing context
      appTracer.withUserProcessingContext(sessionId, () => {
        appTracer.startAgentLoop(sessionId, MAX_TOOL_ITERATIONS);
      });
      
      // Log validation results only in interactive mode or debug mode
      if (imageUrls.length > 0) {
        const invalidCount = imageUrls.length - validImageUrls.length;
        if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
          if (validImageUrls.length > 0) {
            console.log(`Image validation: ${validImageUrls.length} valid, ${invalidCount} invalid/inaccessible.`);
          } else {
            console.log(`Image validation: All ${imageUrls.length} image URLs failed validation.`);
          }
        }
        
        if (this.debug && validImageUrls.length > 0) {
          console.log(`[DEBUG] Valid image URLs:`, validImageUrls);
        }
      }
      
      const wrappedMessage = isFirstMessage ? `<task>\n${cleanedMessage}\n</task>` : cleanedMessage;

      // Create the user message with potential image attachments
      const userMessage = { role: 'user', content: wrappedMessage };
      
      // Add image attachments if any valid URLs were found
      if (validImageUrls.length > 0) {
        userMessage.content = [
          { type: 'text', text: wrappedMessage },
          ...validImageUrls.map(url => ({
            type: 'image',
            image: url
          }))
        ];
      }

      let currentMessages = [
        ...this.history,
        userMessage
      ];

      const promptGenerationStart = Date.now();
      const systemPrompt = await this.getSystemMessage();
      const promptGenerationEnd = Date.now();
      
      if (this.debug) {
        const systemTokens = this.tokenCounter.countTokens(systemPrompt);
        this.tokenCounter.addRequestTokens(systemTokens);
        console.log(`[DEBUG] System prompt estimated tokens: ${systemTokens}`);
        
        // Record system prompt generation metrics
        appTracer.recordSystemPromptGeneration(sessionId, {
          baseLength: 11747, // Approximate base system message length
          finalLength: systemPrompt.length,
          filesAdded: this.history.length > 0 ? 35 : 36, // Approximate from logs
          generationDurationMs: promptGenerationEnd - promptGenerationStart,
          promptType: this.promptType || 'default',
          estimatedTokens: systemTokens
        });
      }

      while (currentIteration < MAX_TOOL_ITERATIONS && !completionAttempted) {
        currentIteration++;
        if (this.cancelled) throw new Error('Request was cancelled by the user');

        // Start iteration trace within agent loop context
        appTracer.withAgentLoopContext(sessionId, () => {
          appTracer.startAgentIteration(sessionId, currentIteration, currentMessages.length, this.tokenCounter.contextSize || 0);
        });

        if (this.debug) {
          console.log(`\n[DEBUG] --- Tool Loop Iteration ${currentIteration}/${MAX_TOOL_ITERATIONS} ---`);
          console.log(`[DEBUG] Current messages count for AI call: ${currentMessages.length}`);
          currentMessages.slice(-3).forEach((msg, idx) => {
            const contentPreview = (typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content)).substring(0, 80).replace(/\n/g, ' ');
            console.log(`[DEBUG]   Msg[${currentMessages.length - 3 + idx}]: ${msg.role}: ${contentPreview}...`);
          });
        }

        this.tokenCounter.calculateContextSize(currentMessages);
        if (this.debug) console.log(`[DEBUG] Estimated context tokens BEFORE LLM call (Iter ${currentIteration}): ${this.tokenCounter.contextSize}`);

        let maxResponseTokens = 4000;
        if (this.model.includes('claude-3-opus') || this.model.startsWith('gpt-4-')) {
          maxResponseTokens = 4096;
        } else if (this.model.includes('claude-3-5-sonnet') || this.model.startsWith('gpt-4o')) {
          maxResponseTokens = 8000;
        } else if (this.model.includes('gemini-2.5')) {
          maxResponseTokens = 60000;
        } else if (this.model.startsWith('gemini')) {
          maxResponseTokens = 8000;
        }
        this.tokenDisplay = new TokenUsageDisplay({ maxTokens: maxResponseTokens });

        const userMsgIndices = currentMessages.reduce(
          (acc, msg, index) => (msg.role === 'user' ? [...acc, index] : acc),
          []
        );
        const lastUserMsgIndex = userMsgIndices[userMsgIndices.length - 1] ?? -1;
        const secondLastUserMsgIndex = userMsgIndices[userMsgIndices.length - 2] ?? -1;

        let transformedMessages = currentMessages;
        if (this.apiType === 'anthropic') {
          transformedMessages = currentMessages.map((message, index) => {
            if (message.role === 'user' && (index === lastUserMsgIndex || index === secondLastUserMsgIndex)) {
              return {
                ...message,
                content: typeof message.content === 'string'
                  ? [{ type: "text", text: message.content, providerOptions: { anthropic: { cacheControl: { type: 'ephemeral' } } } }]
                  : message.content.map((content, contentIndex) => {
                    // Only apply cache_control to the text part, not images
                    if (content.type === 'text' && contentIndex === 0) {
                      return {
                        ...content,
                        providerOptions: { anthropic: { cacheControl: { type: 'ephemeral' } } }
                      };
                    }
                    return content;
                  })
              };
            }
            return message;
          });
        }

        let streamError;

        const generateOptions = {
          model: this.provider(this.model),
          messages: transformedMessages,
          system: systemPrompt,
          temperature: 0.3,
          maxTokens: maxResponseTokens,
          signal: this.abortController.signal,
          onError({ error }) {
            streamError = error;
            console.error(error); // your error logging logic here
          },
          providerOptions: {
            openai: {
              streamOptions: {
                include_usage: true
              }
            }
          },
          experimental_telemetry: {
            isEnabled: false, // Disable built-in telemetry in favor of our custom tracing
            functionId: this.sessionId,
            metadata: {
              sessionId: this.sessionId,
              iteration: currentIteration,
              model: this.model,
              apiType: this.apiType,
              allowEdit: this.allowEdit,
              promptType: this.promptType || 'default'
            }
          }
        };

        // Start AI generation request trace within iteration context
        const aiRequestSpan = appTracer.withIterationContext(sessionId, currentIteration, () => {
          return appTracer.startAiGenerationRequest(sessionId, currentIteration, this.model, this.apiType, {
          temperature: 0.3,
          maxTokens: maxResponseTokens,
          maxRetries: 2
          });
        });

        // **Streaming Response Handling**
        let assistantResponseContent = '';
        let startTime = Date.now();
        let firstChunkTime = null;
        try {
          if (this.debug) console.log(`[DEBUG] Calling streamText with model ${this.model}...`);

          if (streamError) {
            throw streamError
          }

          const { textStream } = streamText(generateOptions);
          for await (const chunk of textStream) {
            if (this.cancelled) throw new Error('Request was cancelled by the user');
            if (firstChunkTime === null) {
              firstChunkTime = Date.now();
            }
            assistantResponseContent += chunk;
          }

          if (this.debug) {
            console.log(`[DEBUG] Streamed AI response (Iter ${currentIteration}). Length: ${assistantResponseContent.length}`);
          }
          if (assistantResponseContent.length == 0) {
            console.warn(`[WARN] Empty response from AI model (Iter ${currentIteration}).`);
            throw new Error('Empty response from AI model');
          }

          currentMessages.push({ role: 'assistant', content: assistantResponseContent });

          const responseTokenCount = this.tokenCounter.countTokens(assistantResponseContent);
          if (this.debug) console.log(`[DEBUG] Estimated response tokens (Iter ${currentIteration}): ${responseTokenCount}`);
          this.tokenCounter.addResponseTokens(responseTokenCount);
          this.tokenCounter.calculateContextSize(currentMessages);
          if (this.debug) console.log(`[DEBUG] Context size AFTER LLM response (Iter ${currentIteration}): ${this.tokenCounter.contextSize}`);

          // Record AI response in trace
          const endTime = Date.now();
          appTracer.recordAiResponse(sessionId, currentIteration, {
            response: assistantResponseContent, // Include actual response content
            responseLength: assistantResponseContent.length,
            completionTokens: responseTokenCount,
            promptTokens: this.tokenCounter.contextSize || 0,
            finishReason: 'stop',
            timeToFirstChunk: firstChunkTime ? (firstChunkTime - startTime) : 0,
            timeToFinish: endTime - startTime
          });

          appTracer.endAiRequest(sessionId, currentIteration, true);

        } catch (error) {
          // Classify and record the AI model error
          let errorCategory = 'unknown';
          if (this.cancelled || error.name === 'AbortError' || (error.message && error.message.includes('cancelled'))) {
            errorCategory = 'cancellation';
          } else if (error.message?.includes('timeout')) {
            errorCategory = 'timeout';
          } else if (error.message?.includes('rate limit') || error.message?.includes('quota')) {
            errorCategory = 'api_limit';
          } else if (error.message?.includes('network') || error.message?.includes('fetch')) {
            errorCategory = 'network';
          } else if (error.status >= 400 && error.status < 500) {
            errorCategory = 'client_error';
          } else if (error.status >= 500) {
            errorCategory = 'server_error';
          }
          
          appTracer.recordAiModelError(sessionId, currentIteration, {
            category: errorCategory,
            message: error.message,
            model: this.model,
            provider: this.apiType,
            statusCode: error.status || 0,
            retryAttempt: 0
          });
          
          appTracer.endAiRequest(sessionId, currentIteration, false);
          
          if (this.cancelled || error.name === 'AbortError' || (error.message && error.message.includes('cancelled'))) {
            console.log(`Chat request cancelled during LLM call (Iter ${currentIteration})`);
            this.cancelled = true;
            appTracer.recordSessionCancellation(sessionId, 'ai_request_cancelled', {
              currentIteration,
              activeTool: 'ai_generation'
            });
            throw new Error('Request was cancelled by the user');
          }
          console.error(`Error during streamText (Iter ${currentIteration}):`, error);
          finalResult = `Error: Failed to get response from AI model during iteration ${currentIteration}. ${error.message}`;
          throw new Error(finalResult);
        }

        const parsedTool = parseXmlToolCallWithThinking(assistantResponseContent);
        if (parsedTool) {
          const { toolName, params } = parsedTool;
          if (this.debug) console.log(`[DEBUG] Parsed tool call: ${toolName} with params:`, params);
          
          // Record tool call parsing in trace
          appTracer.recordToolCallParsed(sessionId, currentIteration, toolName, params);

          if (toolName === 'attempt_completion') {
            completionAttempted = true;
            const validation = attemptCompletionSchema.safeParse(params);
            if (!validation.success) {
              finalResult = `Error: AI attempted completion with invalid parameters: ${JSON.stringify(validation.error.issues)}`;
              console.warn(`[WARN] Invalid attempt_completion parameters:`, validation.error.issues);
              appTracer.recordCompletionAttempt(sessionId, false);
            } else {
              finalResult = validation.data.result;
              appTracer.recordCompletionAttempt(sessionId, true, finalResult);
              if (this.debug) {
                console.log(`[DEBUG] Completion attempted successfully. Final Result captured.`);

                try {
                  const systemPrompt = await this.getSystemMessage();
                  let debugContent = `system: ${systemPrompt}\n\n`;
                  for (const msg of currentMessages) {
                    if (msg.role === 'user' || msg.role === 'assistant') {
                      debugContent += `${msg.role}: ${msg.content}\n\n`;
                    }
                  }
                  debugContent += `assistant (final result): ${finalResult}\n\n`;
                  writeFileSync(debugFilePath, debugContent, { flag: 'w' });
                  if (this.debug) console.log(`[DEBUG] Wrote complete chat history to ${debugFilePath}`);
                } catch (error) {
                  console.error(`Error writing chat history to debug file: ${error.message}`);
                }
              }
            }
            break;

          } else if (this.toolImplementations[toolName]) {
            const toolInstance = this.toolImplementations[toolName];
            let toolResultContent = '';
            
            // Start tool execution trace within iteration context
            appTracer.withIterationContext(sessionId, currentIteration, () => {
              appTracer.startToolExecution(sessionId, currentIteration, toolName, params);
            });
            
            try {
              const enhancedParams = { ...params, sessionId: this.sessionId };
              if (this.debug) console.log(`[DEBUG] Executing tool '${toolName}' with params:`, enhancedParams);
              const executionResult = await toolInstance.execute(enhancedParams);
              toolResultContent = typeof executionResult === 'string' ? executionResult : JSON.stringify(executionResult, null, 2);
              if (this.debug) {
                const preview = toolResultContent.substring(0, 200).replace(/\n/g, ' ') + (toolResultContent.length > 200 ? '...' : '');
                console.log(`[DEBUG] Tool '${toolName}' executed successfully. Result preview: ${preview}`);
              }
              
              // End tool execution trace with success
              appTracer.endToolExecution(sessionId, currentIteration, true, toolResultContent.length, null, toolResultContent);
            } catch (error) {
              console.error(`Error executing tool ${toolName}:`, error);
              toolResultContent = `Error executing tool ${toolName}: ${error.message}`;
              if (this.debug) console.log(`[DEBUG] Tool '${toolName}' execution FAILED.`);
              
              // Classify and record tool execution error
              let errorCategory = 'execution';
              if (error.message?.includes('validation')) {
                errorCategory = 'validation';
              } else if (error.message?.includes('permission') || error.message?.includes('access')) {
                errorCategory = 'filesystem';
              } else if (error.message?.includes('network') || error.message?.includes('fetch')) {
                errorCategory = 'network';
              } else if (error.message?.includes('timeout')) {
                errorCategory = 'timeout';
              }
              
              appTracer.recordToolError(sessionId, currentIteration, toolName, {
                category: errorCategory,
                message: error.message,
                exitCode: error.code || 0,
                signal: error.signal || '',
                params: enhancedParams
              });
              
              // End tool execution trace with failure
              appTracer.endToolExecution(sessionId, currentIteration, false, 0, error.message, toolResultContent);
            }

            const toolResultMessage = `<tool_result>\n${toolResultContent}\n</tool_result>`;
            currentMessages.push({ role: 'user', content: toolResultMessage });
            this.tokenCounter.calculateContextSize(currentMessages);
            if (this.debug) console.log(`[DEBUG] Context size after adding tool result for '${toolName}': ${this.tokenCounter.contextSize}`);

          } else {
            if (this.debug) console.log(`[DEBUG] Assistant used invalid tool name: ${toolName}`);
            const errorContent = `<tool_result>\nError: Invalid tool name specified: '${toolName}'. Please use one of: search, query, extract, attempt_completion.\n</tool_result>`;
            currentMessages.push({ role: 'user', content: errorContent });
            this.tokenCounter.calculateContextSize(currentMessages);
          }

        } else {
          if (this.debug) console.log(`[DEBUG] Assistant response did not contain a valid XML tool call.`);
          const forceToolContent = `Your response did not contain a valid tool call in the required XML format. You MUST respond with exactly one tool call (e.g., <search>...</search> or <attempt_completion>...</attempt_completion>) based on the previous steps and the user's goal. Analyze the situation and choose the appropriate next tool.`;
          currentMessages.push({ role: 'user', content: forceToolContent });
          this.tokenCounter.calculateContextSize(currentMessages);
        }

        if (currentMessages.length > MAX_HISTORY_MESSAGES + 3) {
          const messagesBefore = currentMessages.length;
          const removeCount = currentMessages.length - MAX_HISTORY_MESSAGES;
          currentMessages = currentMessages.slice(removeCount);
          
          // Record in-loop history management
          appTracer.recordHistoryOperation(sessionId, 'trim', {
            messagesBefore,
            messagesAfter: currentMessages.length,
            messagesRemoved: removeCount,
            reason: 'loop_memory_limit'
          });
          
          if (this.debug) console.log(`[DEBUG] Trimmed 'currentMessages' within loop to ${currentMessages.length} (removed ${removeCount}).`);
          this.tokenCounter.calculateContextSize(currentMessages);
        }
        
        // End iteration trace
        appTracer.endIteration(sessionId, currentIteration, true, completionAttempted ? 'completion_attempted' : 'tool_executed');
      }

      if (currentIteration >= MAX_TOOL_ITERATIONS && !completionAttempted) {
        console.warn(`[WARN] Max tool iterations (${MAX_TOOL_ITERATIONS}) reached for session ${this.sessionId}. Returning current error state.`);
      }
      
      // End agent loop trace
      appTracer.endAgentLoop(sessionId, currentIteration, completionAttempted, completionAttempted ? 'completion' : 'max_iterations');

      this.history = currentMessages.map(msg => ({ ...msg }));
      if (this.history.length > MAX_HISTORY_MESSAGES) {
        const messagesBefore = this.history.length;
        const finalRemoveCount = this.history.length - MAX_HISTORY_MESSAGES;
        this.history = this.history.slice(finalRemoveCount);
        
        // Record history management operation
        appTracer.recordHistoryOperation(sessionId, 'trim', {
          messagesBefore,
          messagesAfter: this.history.length,
          messagesRemoved: finalRemoveCount,
          reason: 'max_length'
        });
        
        if (this.debug) console.log(`[DEBUG] Final history trim applied. Length: ${this.history.length} (removed ${finalRemoveCount})`);
      }

      this.tokenCounter.updateHistory(this.history);
      
      // Record token metrics
      const tokenUsage = this.tokenCounter.getTokenUsage();
      appTracer.recordTokenMetrics(sessionId, {
        contextWindow: tokenUsage.contextWindow || 0,
        currentTotal: tokenUsage.current?.total || 0,
        requestTokens: tokenUsage.current?.request || 0,
        responseTokens: tokenUsage.current?.response || 0,
        cacheRead: tokenUsage.current?.cacheRead || 0,
        cacheWrite: tokenUsage.current?.cacheWrite || 0
      });
      
      // End user message processing trace
      appTracer.endUserMessageProcessing(sessionId, completionAttempted);
      
      if (this.debug) {
        console.log(`[DEBUG] Updated tokenCounter history with ${this.history.length} messages`);
        console.log(`[DEBUG] Context size after history update: ${this.tokenCounter.contextSize}`);
        console.log(`[DEBUG] ===== Ending XML Tool Chat Loop =====`);
        console.log(`[DEBUG] Loop finished after ${currentIteration} iterations.`);
        console.log(`[DEBUG] Completion attempted: ${completionAttempted}`);
        console.log(`[DEBUG] Final history length: ${this.history.length}`);
        const resultPreview = (typeof finalResult === 'string' ? finalResult : JSON.stringify(finalResult)).substring(0, 200).replace(/\n/g, ' ');
        console.log(`[DEBUG] Returning final result: "${resultPreview}..."`);
      }

      this.tokenCounter.calculateContextSize(this.history);
      const updatedTokenUsage = this.tokenCounter.getTokenUsage();
      if (this.debug) {
        console.log(`[DEBUG] Final context window size: ${updatedTokenUsage.contextWindow}`);
        console.log(`[DEBUG] Cache metrics - Read: ${updatedTokenUsage.current.cacheRead}, Write: ${updatedTokenUsage.current.cacheWrite}`);
      }

      return {
        response: finalResult,
        tokenUsage: updatedTokenUsage
      };

    } catch (error) {
      // Record the top-level processing error
      if (this.cancelled || (error.message && error.message.includes('cancelled'))) {
        appTracer.recordSessionCancellation(sessionId, 'processing_cancelled', {
          currentIteration,
          errorMessage: error.message
        });
      } else {
        // Record as a general processing error
        appTracer.recordAiModelError(sessionId, currentIteration || 0, {
          category: 'processing_error',
          message: error.message,
          model: this.model,
          provider: this.apiType,
          statusCode: 0,
          retryAttempt: 0
        });
      }
      
      // End chat session before cleanup to ensure span is properly captured
      appTracer.endChatSession(sessionId, false, 0);
      
      // Clean up any remaining spans for this session (but session span is already ended)
      appTracer.cleanup(sessionId);
      
      console.error('Error in chat processing loop:', error);
      if (this.debug) console.error('Error in chat processing loop:', error);

      this.tokenCounter.updateHistory(this.history);
      if (this.debug) console.log(`[DEBUG] Error case - Updated tokenCounter history with ${this.history.length} messages`);

      this.tokenCounter.calculateContextSize(this.history);
      const updatedTokenUsage = this.tokenCounter.getTokenUsage();
      if (this.debug) {
        console.log(`[DEBUG] Error case - Final context window size: ${updatedTokenUsage.contextWindow}`);
        console.log(`[DEBUG] Error case - Cache metrics - Read: ${updatedTokenUsage.current.cacheRead}, Write: ${updatedTokenUsage.current.cacheWrite}`);
      }

      if (this.cancelled || (error.message && error.message.includes('cancelled'))) {
        return { response: "Request cancelled.", tokenUsage: updatedTokenUsage };
      }
      return {
        response: `Error during chat processing: ${error.message || 'An unexpected error occurred.'}`,
        tokenUsage: updatedTokenUsage
      };
    } finally {
      this.abortController = null;
    }
  }


  /**
   * Get the current token usage summary
   * @returns {Object} - Raw token usage data for UI display
   */
  getTokenUsage() {
    // Get raw token usage from the counter
    const usage = this.tokenCounter.getTokenUsage();

    // Return the raw usage data directly
    // This allows the web interface to format it as needed
    return usage;
  }

  /**
   * Clear the entire history and reset session/token usage
   * @returns {string} - The new session ID
   */
  clearHistory() {
    const oldHistoryLength = this.history.length;
    const oldSessionId = this.sessionId;

    this.history = [];
    this.sessionId = randomUUID(); // Generate a new session ID

    // Clear the tokenCounter - this resets all counters and the internal history
    this.tokenCounter.clear();

    // Double-check that the tokenCounter's history is empty
    if (this.tokenCounter.history && this.tokenCounter.history.length > 0) {
      this.tokenCounter.history = [];
      if (this.debug) {
        console.log(`[DEBUG] Explicitly cleared tokenCounter history after clear() call`);
      }
    }

    this.cancelled = false; // Reset cancellation flag
    if (this.abortController) {
      // Ensure any lingering abort signal is cleared (though should be handled by `chat`)
      try { this.abortController.abort('History cleared'); } catch (e) { /* ignore */ }
      this.abortController = null;
    }


    if (this.debug) {
      console.log(`[DEBUG] ===== CLEARING CHAT HISTORY & STATE =====`);
      console.log(`[DEBUG] Cleared ${oldHistoryLength} messages from history`);
      console.log(`[DEBUG] Old session ID: ${oldSessionId}`);
      console.log(`[DEBUG] New session ID: ${this.sessionId}`);
      console.log(`[DEBUG] Token counter reset.`);
      console.log(`[DEBUG] Cancellation flag reset.`);
    }

    // Tool implementations are instance properties, they persist. Session ID is passed during execution.

    return this.sessionId; // Return the newly generated session ID
  }

  /**
   * Get the session ID for this chat instance
   * @returns {string} - The session ID
   */
  getSessionId() {
    return this.sessionId;
  }
}

// Export the extractImageUrls function for testing
export { extractImageUrls };
