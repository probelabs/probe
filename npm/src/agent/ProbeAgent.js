// Core ProbeAgent class adapted from examples/chat/probeChat.js
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { streamText } from 'ai';
import { randomUUID } from 'crypto';
import { EventEmitter } from 'events';
import { TokenCounter } from './tokenCounter.js';
import { 
  createTools,
  searchToolDefinition,
  queryToolDefinition,
  extractToolDefinition,
  listFilesToolDefinition,
  searchFilesToolDefinition,
  attemptCompletionToolDefinition,
  implementToolDefinition,
  attemptCompletionSchema,
  parseXmlToolCallWithThinking
} from './tools.js';
import { 
  createWrappedTools, 
  listFilesToolInstance, 
  searchFilesToolInstance,
  clearToolExecutionData 
} from './probeTool.js';
import { listFilesByLevel } from '../index.js';
import { 
  cleanSchemaResponse,
  isJsonSchema,
  validateJsonResponse,
  createJsonCorrectionPrompt,
  validateAndFixMermaidResponse
} from './schemaUtils.js';

// Maximum tool iterations to prevent infinite loops - configurable via MAX_TOOL_ITERATIONS env var
const MAX_TOOL_ITERATIONS = parseInt(process.env.MAX_TOOL_ITERATIONS || '30', 10);
const MAX_HISTORY_MESSAGES = 100;

/**
 * ProbeAgent class to handle AI interactions with code search capabilities
 */
export class ProbeAgent {
  /**
   * Create a new ProbeAgent instance
   * @param {Object} options - Configuration options
   * @param {string} [options.sessionId] - Optional session ID
   * @param {string} [options.customPrompt] - Custom prompt to replace the default system message
   * @param {string} [options.promptType] - Predefined prompt type (architect, code-review, support)
   * @param {boolean} [options.allowEdit=false] - Allow the use of the 'implement' tool
   * @param {string} [options.path] - Search directory path
   * @param {string} [options.provider] - Force specific AI provider
   * @param {string} [options.model] - Override model name
   * @param {boolean} [options.debug] - Enable debug mode
   */
  constructor(options = {}) {
    // Basic configuration
    this.sessionId = options.sessionId || randomUUID();
    this.customPrompt = options.customPrompt || null;
    this.promptType = options.promptType || 'code-explorer';
    this.allowEdit = !!options.allowEdit;
    this.debug = options.debug || process.env.DEBUG === '1';
    this.cancelled = false;
    this.tracer = options.tracer || null;

    // Search configuration
    this.allowedFolders = options.path ? [options.path] : [process.cwd()];

    // API configuration
    this.clientApiProvider = options.provider || null;
    this.clientApiKey = null; // Will be set from environment
    this.clientApiUrl = null;

    // Initialize token counter
    this.tokenCounter = new TokenCounter();

    if (this.debug) {
      console.log(`[DEBUG] Generated session ID for agent: ${this.sessionId}`);
      console.log(`[DEBUG] Maximum tool iterations configured: ${MAX_TOOL_ITERATIONS}`);
      console.log(`[DEBUG] Allow Edit (implement tool): ${this.allowEdit}`);
    }

    // Initialize tools
    this.initializeTools();

    // Initialize the AI model
    this.initializeModel();

    // Initialize chat history
    this.history = [];
    
    // Initialize event emitter for tool execution updates
    this.events = new EventEmitter();
  }

  /**
   * Initialize tools with configuration
   */
  initializeTools() {
    const configOptions = {
      sessionId: this.sessionId,
      debug: this.debug,
      defaultPath: this.allowedFolders.length > 0 ? this.allowedFolders[0] : process.cwd(),
      allowedFolders: this.allowedFolders
    };

    // Create base tools
    const baseTools = createTools(configOptions);
    
    // Create wrapped tools with event emission
    const wrappedTools = createWrappedTools(baseTools);

    // Store tool instances for execution
    this.toolImplementations = {
      search: wrappedTools.searchToolInstance,
      query: wrappedTools.queryToolInstance,
      extract: wrappedTools.extractToolInstance,
      listFiles: listFilesToolInstance,
      searchFiles: searchFilesToolInstance,
    };
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
    const llmBaseUrl = process.env.LLM_BASE_URL;
    const anthropicApiUrl = process.env.ANTHROPIC_API_URL || llmBaseUrl;
    const openaiApiUrl = process.env.OPENAI_API_URL || llmBaseUrl;
    const googleApiUrl = process.env.GOOGLE_API_URL || llmBaseUrl;

    // Get model override if provided
    const modelName = process.env.MODEL_NAME;

    // Use client-forced provider or environment variable
    const forceProvider = this.clientApiProvider || (process.env.FORCE_PROVIDER ? process.env.FORCE_PROVIDER.toLowerCase() : null);

    if (this.debug) {
      console.log(`[DEBUG] Available API keys: Anthropic=${!!anthropicApiKey}, OpenAI=${!!openaiApiKey}, Google=${!!googleApiKey}`);
      console.log(`[DEBUG] Force provider: ${forceProvider || '(not set)'}`);
      if (modelName) console.log(`[DEBUG] Model override: ${modelName}`);
    }

    // Check if a specific provider is forced
    if (forceProvider) {
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
      throw new Error('No API key provided. Please set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY environment variable.');
    }
  }

  /**
   * Initialize Anthropic model
   */
  initializeAnthropicModel(apiKey, apiUrl, modelName) {
    this.provider = createAnthropic({
      apiKey: apiKey,
      ...(apiUrl && { baseURL: apiUrl }),
    });
    this.model = modelName || 'claude-opus-4-1-20250805';
    this.apiType = 'anthropic';
    
    if (this.debug) {
      console.log(`Using Anthropic API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl})` : ''}`);
    }
  }

  /**
   * Initialize OpenAI model
   */
  initializeOpenAIModel(apiKey, apiUrl, modelName) {
    this.provider = createOpenAI({
      compatibility: 'strict',
      apiKey: apiKey,
      ...(apiUrl && { baseURL: apiUrl }),
    });
    this.model = modelName || 'gpt-5-thinking';
    this.apiType = 'openai';
    
    if (this.debug) {
      console.log(`Using OpenAI API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl})` : ''}`);
    }
  }

  /**
   * Initialize Google model
   */
  initializeGoogleModel(apiKey, apiUrl, modelName) {
    this.provider = createGoogleGenerativeAI({
      apiKey: apiKey,
      ...(apiUrl && { baseURL: apiUrl }),
    });
    this.model = modelName || 'gemini-2.5-pro';
    this.apiType = 'google';
    
    if (this.debug) {
      console.log(`Using Google API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl})` : ''}`);
    }
  }

  /**
   * Get the system message with instructions for the AI (XML Tool Format)
   */
  async getSystemMessage() {
    // Build tool definitions
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

    // Build XML tool guidelines
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

Before using a tool, analyze the situation within <thinking></thinking> tags. This helps you organize your thoughts and make better decisions.

Example:
<thinking>
I need to find code related to error handling in the search module. The most appropriate tool for this is the search tool, which requires a query parameter and a path parameter. I have both the query ("error handling") and the path ("src/search"), so I can proceed with the search.
</thinking>

# Tool Use Guidelines

1. Think step-by-step about how to achieve the user's goal.
2. Use <thinking></thinking> tags to analyze the situation and determine the appropriate tool.
3. Choose **one** tool that helps achieve the current step.
4. Format the tool call using the specified XML format. Ensure all required parameters are included.
5. **You MUST respond with exactly one tool call in the specified XML format in each turn.**
6. Wait for the tool execution result, which will be provided in the next message (within a <tool_result> block).
7. Analyze the tool result and decide the next step. If more tool calls are needed, repeat steps 2-6.
8. If the task is fully complete and all previous steps were successful, use the \`<attempt_completion>\` tool to provide the final answer. This is the ONLY way to finish the task.
9. If you cannot proceed (e.g., missing information, invalid request), explain the issue clearly before using \`<attempt_completion>\` with an appropriate message in the \`<result>\` tag.

Available Tools:
- search: Search code using keyword queries.
- query: Search code using structural AST patterns.
- extract: Extract specific code blocks or lines from files.
- listFiles: List files and directories in a specified location.
- searchFiles: Find files matching a glob pattern with recursive search capability.
${this.allowEdit ? '- implement: Implement a feature or fix a bug using aider.\n' : ''}
- attempt_completion: Finalize the task and provide the result to the user.
`;

    // Common instructions
    const commonInstructions = `<instructions>
Follow these instructions carefully:
1. Analyze the user's request.
2. Use <thinking></thinking> tags to analyze the situation and determine the appropriate tool for each step.
3. Use the available tools step-by-step to fulfill the request.
4. You should always prefer the \`search\` tool for code-related questions. Read full files only if really necessary.
5. Ensure to get really deep and understand the full picture before answering.
6. You MUST respond with exactly ONE tool call per message, using the specified XML format, until the task is complete.
7. Wait for the tool execution result (provided in the next user message in a <tool_result> block) before proceeding to the next step.
8. Once the task is fully completed, use the '<attempt_completion>' tool to provide the final result. This is the ONLY way to signal completion.
9. Prefer concise and focused search queries. Use specific keywords and phrases to narrow down results.
</instructions>
`;

    // Define predefined prompts (without the common instructions)
    const predefinedPrompts = {
      'code-explorer': `You are ProbeChat Code Explorer, a specialized AI assistant focused on helping developers, product managers, and QAs understand and navigate codebases. Your primary function is to answer questions based on code, explain how systems work, and provide insights into code functionality using the provided code analysis tools.

When exploring code:
- Provide clear, concise explanations based on user request
- Find and highlight the most relevant code snippets, if required
- Trace function calls and data flow through the system
- Try to understand the user's intent and provide relevant information
- Understand high level picture
- Balance detail with clarity in your explanations`,

      'architect': `You are ProbeChat Architect, a specialized AI assistant focused on software architecture and design. Your primary function is to help users understand, analyze, and design software systems using the provided code analysis tools.

When analyzing code:
- Focus on high-level design patterns and system organization
- Identify architectural patterns and component relationships
- Evaluate system structure and suggest architectural improvements
- Consider scalability, maintainability, and extensibility in your analysis`,

      'code-review': `You are ProbeChat Code Reviewer, a specialized AI assistant focused on code quality and best practices. Your primary function is to help users identify issues, suggest improvements, and ensure code follows best practices using the provided code analysis tools.

When reviewing code:
- Look for bugs, edge cases, and potential issues
- Identify performance bottlenecks and optimization opportunities
- Check for security vulnerabilities and best practices
- Evaluate code style and consistency
- Provide specific, actionable suggestions with code examples where appropriate`,

      'code-review-template': `You are going to perform code review according to provided user rules. Ensure to review only code provided in diff and latest commit, if provided. However you still need to fully understand how modified code works, and read dependencies if something is not clear.`,

      'engineer': `You are senior engineer focused on software architecture and design.
Before jumping on the task you first, in details analyse user request, and try to provide elegant and concise solution.
If solution is clear, you can jump to implementation right away, if not, you can ask user a clarification question, by calling attempt_completion tool, with required details.

Before jumping to implementation:
- Focus on high-level design patterns and system organization
- Identify architectural patterns and component relationships
- Evaluate system structure and suggest architectural improvements
- Focus on backward compatibility.
- Consider scalability, maintainability, and extensibility in your analysis

During the implementation:
- Avoid implementing special cases
- Do not forget to add the tests`,

      'support': `You are ProbeChat Support, a specialized AI assistant focused on helping developers troubleshoot issues and solve problems. Your primary function is to help users diagnose errors, understand unexpected behaviors, and find solutions using the provided code analysis tools.

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

    // Add folder information
    const searchDirectory = this.allowedFolders.length > 0 ? this.allowedFolders[0] : process.cwd();
    if (this.debug) {
      console.log(`[DEBUG] Generating file list for base directory: ${searchDirectory}...`);
    }

    try {
      const files = await listFilesByLevel({
        directory: searchDirectory,
        maxFiles: 100,
        respectGitignore: !process.env.PROBE_NO_GITIGNORE || process.env.PROBE_NO_GITIGNORE === '',
        cwd: process.cwd()
      });

      systemMessage += `\n# Repository Structure\n\nYou are working with a repository located at: ${searchDirectory}\n\nHere's an overview of the repository structure (showing up to 100 most relevant files):\n\n\`\`\`\n${files}\n\`\`\`\n\n`;
    } catch (error) {
      if (this.debug) {
        console.log(`[DEBUG] Could not generate file list: ${error.message}`);
      }
      systemMessage += `\n# Repository Structure\n\nYou are working with a repository located at: ${searchDirectory}\n\n`;
    }

    if (this.allowedFolders.length > 0) {
      systemMessage += `\n**Important**: For security reasons, you can only search within these allowed folders: ${this.allowedFolders.join(', ')}\n\n`;
    }

    return systemMessage;
  }

  /**
   * Answer a question using the agentic flow
   * @param {string} message - The user's question
   * @param {Array} [images] - Optional array of image data (base64 strings or URLs)
   * @param {Object|string} [schemaOrOptions] - Can be either:
   *   - A string: JSON schema for structured output (backwards compatible)
   *   - An object: Options object with schema and other options
   * @param {string} [schemaOrOptions.schema] - JSON schema string for structured output
   * @returns {Promise<string>} - The final answer
   */
  async answer(message, images = [], schemaOrOptions = {}) {
    if (!message || typeof message !== 'string' || message.trim().length === 0) {
      throw new Error('Message is required and must be a non-empty string');
    }

    // Handle backwards compatibility - if third argument is a string, treat it as schema
    let options = {};
    if (typeof schemaOrOptions === 'string') {
      options = { schema: schemaOrOptions };
    } else {
      options = schemaOrOptions || {};
    }

    try {
      // Generate system message
      const systemMessage = await this.getSystemMessage();

      // Create user message with optional image support
      let userMessage = { role: 'user', content: message.trim() };
      
      // If images are provided, use multi-modal message format
      if (images && images.length > 0) {
        userMessage.content = [
          { type: 'text', text: message.trim() },
          ...images.map(image => ({
            type: 'image',
            image: image
          }))
        ];
      }

      // Initialize conversation with existing history + new user message
      let currentMessages = [
        { role: 'system', content: systemMessage },
        ...this.history, // Include previous conversation history
        userMessage
      ];

      let currentIteration = 0;
      let completionAttempted = false;
      let finalResult = 'I was unable to complete your request due to reaching the maximum number of tool iterations.';

      // Adjust max iterations if schema is provided
      // +1 for schema formatting
      // +2 for potential Mermaid validation retries (can be multiple diagrams)
      // +1 for potential JSON correction
      const maxIterations = options.schema ? MAX_TOOL_ITERATIONS + 4 : MAX_TOOL_ITERATIONS;

      if (this.debug) {
        console.log(`[DEBUG] Starting agentic flow for question: ${message.substring(0, 100)}...`);
        if (options.schema) {
          console.log(`[DEBUG] Schema provided, using extended iteration limit: ${maxIterations} (base: ${MAX_TOOL_ITERATIONS})`);
        }
      }

      // Tool iteration loop
      while (currentIteration < maxIterations && !completionAttempted) {
        currentIteration++;
        if (this.cancelled) throw new Error('Request was cancelled by the user');

        if (this.debug) {
          console.log(`\n[DEBUG] --- Tool Loop Iteration ${currentIteration}/${maxIterations} ---`);
          console.log(`[DEBUG] Current messages count for AI call: ${currentMessages.length}`);
        }

        // Add iteration tracing event
        if (this.tracer) {
          this.tracer.addEvent('iteration.start', {
            'iteration': currentIteration,
            'max_iterations': maxIterations,
            'message_count': currentMessages.length
          });
        }

        // Add warning message when reaching the last iteration
        if (currentIteration === maxIterations) {
          const warningMessage = `⚠️ WARNING: You have reached the maximum tool iterations limit (${maxIterations}). This is your final message. Please respond with the data you have so far. If something was not completed, honestly state what was not done and provide any partial results or recommendations you can offer.`;
          
          currentMessages.push({
            role: 'user',
            content: warningMessage
          });
          
          if (this.debug) {
            console.log(`[DEBUG] Added max iterations warning message at iteration ${currentIteration}`);
          }
        }

        // Calculate context size
        this.tokenCounter.calculateContextSize(currentMessages);
        if (this.debug) {
          console.log(`[DEBUG] Estimated context tokens BEFORE LLM call (Iter ${currentIteration}): ${this.tokenCounter.contextSize}`);
        }

        let maxResponseTokens = 4000;
        if (this.model.includes('claude-3-opus') || this.model.startsWith('gpt-4-')) {
          maxResponseTokens = 4096;
        } else if (this.model.includes('claude-3-5-sonnet') || this.model.startsWith('gpt-4o')) {
          maxResponseTokens = 8192;
        }

        // Make AI request
        let assistantResponseContent = '';
        try {
          // Wrap AI request with tracing if available
          const executeAIRequest = async () => {
            const result = await streamText({
              model: this.provider(this.model),
              messages: currentMessages,
              maxTokens: maxResponseTokens,
              temperature: 0.3,
            });

            // Collect the streamed response
            for await (const delta of result.textStream) {
              assistantResponseContent += delta;
            }

            // Record token usage
            const usage = await result.usage;
            if (usage) {
              this.tokenCounter.recordUsage(usage, result.experimental_providerMetadata);
            }

            return result;
          };

          if (this.tracer) {
            await this.tracer.withSpan('ai.request', executeAIRequest, {
              'ai.model': this.model,
              'ai.provider': this.clientApiProvider || 'auto',
              'iteration': currentIteration,
              'max_tokens': maxResponseTokens,
              'temperature': 0.3,
              'message_count': currentMessages.length
            });
          } else {
            await executeAIRequest();
          }

        } catch (error) {
          console.error(`Error during streamText (Iter ${currentIteration}):`, error);
          finalResult = `Error: Failed to get response from AI model during iteration ${currentIteration}. ${error.message}`;
          throw new Error(finalResult);
        }

        // Parse tool call from response
        const parsedTool = parseXmlToolCallWithThinking(assistantResponseContent);
        if (parsedTool) {
          const { toolName, params } = parsedTool;
          if (this.debug) console.log(`[DEBUG] Parsed tool call: ${toolName} with params:`, params);

          if (toolName === 'attempt_completion') {
            completionAttempted = true;
            const validation = attemptCompletionSchema.safeParse(params);
            if (validation.success) {
              finalResult = validation.data.result;
              if (this.debug) console.log(`[DEBUG] Task completed successfully with result: ${finalResult.substring(0, 100)}...`);
            } else {
              console.error(`[ERROR] Invalid attempt_completion parameters:`, validation.error);
              finalResult = 'Error: Invalid completion attempt. The task could not be completed properly.';
            }
            break;
          } else {
            // Execute the tool
            if (this.toolImplementations[toolName]) {
              try {
                // Add sessionId to params for tool execution
                const toolParams = { ...params, sessionId: this.sessionId };
                
                // Emit tool start event
                this.events.emit('toolCall', {
                  timestamp: new Date().toISOString(),
                  name: toolName,
                  args: toolParams,
                  status: 'started'
                });
                
                // Execute tool with tracing if available
                const executeToolCall = async () => {
                  return await this.toolImplementations[toolName].execute(toolParams);
                };

                let toolResult;
                try {
                  if (this.tracer) {
                    toolResult = await this.tracer.withSpan('tool.call', executeToolCall, {
                      'tool.name': toolName,
                      'tool.params': JSON.stringify(toolParams).substring(0, 500),
                      'iteration': currentIteration
                    });
                  } else {
                    toolResult = await executeToolCall();
                  }
                  
                  // Emit tool success event
                  this.events.emit('toolCall', {
                    timestamp: new Date().toISOString(),
                    name: toolName,
                    args: toolParams,
                    resultPreview: typeof toolResult === 'string'
                      ? (toolResult.length > 200 ? toolResult.substring(0, 200) + '...' : toolResult)
                      : (toolResult ? JSON.stringify(toolResult).substring(0, 200) + '...' : 'No Result'),
                    status: 'completed'
                  });
                  
                } catch (toolError) {
                  // Emit tool error event
                  this.events.emit('toolCall', {
                    timestamp: new Date().toISOString(),
                    name: toolName,
                    args: toolParams,
                    error: toolError.message || 'Unknown error',
                    status: 'error'
                  });
                  throw toolError; // Re-throw to be handled by outer catch
                }
                
                // Add assistant response and tool result to conversation
                currentMessages.push({ role: 'assistant', content: assistantResponseContent });
                currentMessages.push({
                  role: 'user',
                  content: `<tool_result>\n${typeof toolResult === 'string' ? toolResult : JSON.stringify(toolResult, null, 2)}\n</tool_result>`
                });

                if (this.debug) {
                  console.log(`[DEBUG] Tool ${toolName} executed successfully. Result length: ${typeof toolResult === 'string' ? toolResult.length : JSON.stringify(toolResult).length}`);
                }
              } catch (error) {
                console.error(`[ERROR] Tool execution failed for ${toolName}:`, error);
                currentMessages.push({ role: 'assistant', content: assistantResponseContent });
                currentMessages.push({
                  role: 'user', 
                  content: `<tool_result>\nError: ${error.message}\n</tool_result>`
                });
              }
            } else {
              console.error(`[ERROR] Unknown tool: ${toolName}`);
              currentMessages.push({ role: 'assistant', content: assistantResponseContent });
              currentMessages.push({
                role: 'user',
                content: `<tool_result>\nError: Unknown tool '${toolName}'. Available tools: ${Object.keys(this.toolImplementations).join(', ')}\n</tool_result>`
              });
            }
          }
        } else {
          // No tool call found, add assistant response and ask for tool usage
          currentMessages.push({ role: 'assistant', content: assistantResponseContent });
          currentMessages.push({
            role: 'user',
            content: 'Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information to provide a final answer.'
          });
          if (this.debug) {
            console.log(`[DEBUG] No tool call detected in assistant response. Prompting for tool use.`);
          }
        }

        // Keep message history manageable
        if (currentMessages.length > MAX_HISTORY_MESSAGES) {
          const messagesBefore = currentMessages.length;
          const systemMsg = currentMessages[0]; // Keep system message
          const recentMessages = currentMessages.slice(-MAX_HISTORY_MESSAGES + 1);
          currentMessages = [systemMsg, ...recentMessages];
          
          if (this.debug) {
            console.log(`[DEBUG] Trimmed message history from ${messagesBefore} to ${currentMessages.length} messages`);
          }
        }
      }

      if (currentIteration >= maxIterations && !completionAttempted) {
        console.warn(`[WARN] Max tool iterations (${maxIterations}) reached for session ${this.sessionId}. Returning current error state.`);
      }

      // Store final history
      this.history = currentMessages.map(msg => ({ ...msg }));
      if (this.history.length > MAX_HISTORY_MESSAGES) {
        const messagesBefore = this.history.length;
        this.history = this.history.slice(-MAX_HISTORY_MESSAGES);
        if (this.debug) {
          console.log(`[DEBUG] Trimmed stored history from ${messagesBefore} to ${this.history.length} messages`);
        }
      }

      // Update token counter with final history
      this.tokenCounter.updateHistory(this.history);

      // Schema handling - format response according to provided schema
      if (options.schema && !options._schemaFormatted) {
        if (this.debug) {
          console.log('[DEBUG] Schema provided, applying automatic formatting...');
        }
        
        try {
          // Step 1: Make a follow-up call to format according to schema
          const schemaPrompt = `Now you need to respond according to this schema:\n\n${options.schema}\n\nPlease reformat your previous response to match this schema exactly. Only return the formatted response, no additional text.`;
          
          // Call answer recursively with _schemaFormatted flag to prevent infinite loop
          finalResult = await this.answer(schemaPrompt, [], { 
            ...options, 
            _schemaFormatted: true 
          });
          
          // Step 2: Clean the response (remove code blocks)
          finalResult = cleanSchemaResponse(finalResult);
          
          // Step 3: Validate and fix Mermaid diagrams if present
          try {
            const mermaidValidation = await validateAndFixMermaidResponse(finalResult, {
              debug: this.debug,
              path: this.allowedFolders[0],
              provider: this.clientApiProvider,
              model: this.model
            });
            
            if (mermaidValidation.wasFixed) {
              finalResult = mermaidValidation.fixedResponse;
              if (this.debug) {
                console.log(`[DEBUG] Mermaid diagrams fixed`);
                if (mermaidValidation.fixingResults) {
                  mermaidValidation.fixingResults.forEach((fixResult, index) => {
                    if (fixResult.wasFixed) {
                      console.log(`[DEBUG] Fixed diagram ${index + 1}: ${fixResult.originalError}`);
                    }
                  });
                }
              }
            }
          } catch (error) {
            if (this.debug) {
              console.log(`[DEBUG] Mermaid validation failed: ${error.message}`);
            }
          }
          
          // Step 4: Validate and potentially correct JSON responses
          if (isJsonSchema(options.schema)) {
            const validation = validateJsonResponse(finalResult);
            
            if (!validation.isValid) {
              if (this.debug) {
                console.log('[DEBUG] JSON validation failed:', validation.error);
              }
              
              // Attempt correction once
              const correctionPrompt = createJsonCorrectionPrompt(
                finalResult, 
                options.schema, 
                validation.error
              );
              
              finalResult = await this.answer(correctionPrompt, [], { 
                ...options, 
                _schemaFormatted: true 
              });
              finalResult = cleanSchemaResponse(finalResult);
              
              // Final validation
              const finalValidation = validateJsonResponse(finalResult);
              if (!finalValidation.isValid && this.debug) {
                console.log('[DEBUG] JSON still invalid after correction:', finalValidation.error);
              }
            }
          }
        } catch (error) {
          console.error('[ERROR] Schema formatting failed:', error);
          // Return the original result if schema formatting fails
        }
      }

      return finalResult;

    } catch (error) {
      console.error(`[ERROR] ProbeAgent.answer failed:`, error);
      
      // Clean up tool execution data
      clearToolExecutionData(this.sessionId);
      
      throw error;
    }
  }

  /**
   * Get token usage information
   * @returns {Object} Token usage data
   */
  getTokenUsage() {
    return this.tokenCounter.getTokenUsage();
  }

  /**
   * Clear conversation history and reset counters
   */
  clearHistory() {
    this.history = [];
    this.tokenCounter.clear();
    clearToolExecutionData(this.sessionId);
    
    if (this.debug) {
      console.log(`[DEBUG] Cleared conversation history and reset counters for session ${this.sessionId}`);
    }
  }

  /**
   * Cancel the current request
   */
  cancel() {
    this.cancelled = true;
    if (this.debug) {
      console.log(`[DEBUG] Agent cancelled for session ${this.sessionId}`);
    }
  }
}