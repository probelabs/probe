// AI agent implementation
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { generateText } from 'ai';
import { randomUUID } from 'crypto';
import { get_encoding } from 'tiktoken';
import { searchTool, queryTool, extractTool, DEFAULT_SYSTEM_MESSAGE, listFilesByLevel } from '@buger/probe';
import config from './config.js';

// Initialize tokenizer
let tokenizer;
try {
	tokenizer = get_encoding('cl100k_base');
} catch (error) {
	console.warn('Could not initialize tiktoken, falling back to approximate token counting');
}

// Token counter function
function countTokens(text) {
	if (tokenizer) {
		try {
			return tokenizer.encode(text).length;
		} catch (error) {
			// Fallback to a simple approximation (1 token ≈ 4 characters)
			return Math.ceil(text.length / 4);
		}
	} else {
		// Fallback to a simple approximation (1 token ≈ 4 characters)
		return Math.ceil(text.length / 4);
	}
}

export class ProbeAgent {
	constructor() {
		// Initialize token counters
		this.requestTokens = 0;
		this.responseTokens = 0;
		this.toolTokenUsage = { request: 0, response: 0 };

		// Generate a unique session ID for this agent instance
		this.sessionId = randomUUID();

		if (config.debug) {
			console.error(`[DEBUG] Generated session ID for agent: ${this.sessionId}`);
		}

		// Store base configuration for tools
		this.baseToolConfig = {
			sessionId: this.sessionId,
			debug: config.debug,
			defaultPath: config.allowedFolders.length > 0 ? config.allowedFolders[0] : process.cwd(), // Use first allowed folder or current working directory
			allowedFolders: config.allowedFolders // Pass allowed folders to tools
		};

		// Initialize tools with base config (will be updated with timeout in processQuery)
		this.tools = this.createTools(this.baseToolConfig);

		// Initialize the AI model
		this.initializeModel();

		// Initialize chat history
		this.history = [];
	}

	/**
	 * Create tool instances with given configuration
	 */
	createTools(configOptions) {
		return [
			searchTool(configOptions),
			queryTool(configOptions),
			extractTool(configOptions)
		];
	}

	/**
	 * Initialize the AI model based on available API keys and forced provider setting
	 */
	initializeModel() {
		console.error('Initializing AI model...');
		console.error(`Available API keys: Anthropic=${!!config.anthropicApiKey}, OpenAI=${!!config.openaiApiKey}, Google=${!!config.googleApiKey}`);
		console.error(`Force provider config value: "${config.forceProvider}"`);

		// Check if a specific provider is forced
		if (config.forceProvider) {
			console.error(`Provider forced to: ${config.forceProvider}`);

			if (config.forceProvider === 'anthropic' && config.anthropicApiKey) {
				console.error('Using Anthropic provider as forced');
				this.initializeAnthropicModel();
				return;
			} else if (config.forceProvider === 'openai' && config.openaiApiKey) {
				console.error('Using OpenAI provider as forced');
				this.initializeOpenAIModel();
				return;
			} else if (config.forceProvider === 'google' && config.googleApiKey) {
				console.error('Using Google provider as forced');
				this.initializeGoogleModel();
				return;
			}

			console.error(`WARNING: Forced provider "${config.forceProvider}" selected but API key is missing!`);
			// If we get here, the validation in config.js should have already thrown an error
		}

		// If no provider is forced, use the first available API key
		console.error('No provider forced, selecting based on available API keys');

		if (config.anthropicApiKey) {
			console.error('Using Anthropic provider (API key available)');
			this.initializeAnthropicModel();
		} else if (config.openaiApiKey) {
			console.error('Using OpenAI provider (API key available)');
			this.initializeOpenAIModel();
		} else if (config.googleApiKey) {
			console.error('Using Google provider (API key available)');
			this.initializeGoogleModel();
		} else {
			console.error('ERROR: No API keys available!');
			throw new Error('No API key provided. Please set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY environment variable.');
		}
	}

	/**
	 * Initialize Anthropic model
	 */
	initializeAnthropicModel() {
		// Initialize Anthropic provider
		this.provider = createAnthropic({
			apiKey: config.anthropicApiKey,
			baseURL: config.anthropicApiUrl,
		});
		this.model = config.modelName || config.defaultAnthropicModel;
		this.apiType = 'anthropic';

		// Always log when using Anthropic API, regardless of debug mode
		console.error(`Using Anthropic API with model: ${this.model}`);

		if (config.debug) {
			console.error(`[DEBUG] Anthropic API Key: ${config.anthropicApiKey ? '✓ Present' : '✗ Missing'}`);
			console.error(`[DEBUG] Anthropic API URL: ${config.anthropicApiUrl}`);
		}
	}

	/**
	 * Initialize OpenAI model
	 */
	initializeOpenAIModel() {
		// Initialize OpenAI provider
		this.provider = createOpenAI({
			apiKey: config.openaiApiKey,
			baseURL: config.openaiApiUrl,
		});
		this.model = config.modelName || config.defaultOpenAIModel;
		this.apiType = 'openai';

		// Always log when using OpenAI API, regardless of debug mode
		console.error(`Using OpenAI API with model: ${this.model}`);

		if (config.debug) {
			console.error(`[DEBUG] OpenAI API Key: ${config.openaiApiKey ? '✓ Present' : '✗ Missing'}`);
			console.error(`[DEBUG] OpenAI API URL: ${config.openaiApiUrl}`);
		}
	}

	/**
	 * Initialize Google model
	 */
	initializeGoogleModel() {
		// Initialize Google provider
		this.provider = createGoogleGenerativeAI({
			apiKey: config.googleApiKey,
			baseURL: config.googleApiUrl,
		});
		this.model = config.modelName || config.defaultGoogleModel;
		this.apiType = 'google';

		// Always log when using Google API, regardless of debug mode
		console.error(`Using Google API with model: ${this.model}`);

		if (config.debug) {
			console.error(`[DEBUG] Google API Key: ${config.googleApiKey ? '✓ Present' : '✗ Missing'}`);
			console.error(`[DEBUG] Google API URL: ${config.googleApiUrl}`);
		}
	}

	/**
	 * Get the system message with instructions for the AI
	 */
	async getSystemMessage() {
		// Use the default system message from the probe package as a base
		let systemMessage = DEFAULT_SYSTEM_MESSAGE || `You are a helpful AI assistant that can search and analyze code repositories using the Probe tool.
You have access to a code search tool that can help you find relevant code snippets.
Always use the search tool first before attempting to answer questions about the codebase.
When responding to questions about code, make sure to include relevant code snippets and explain them clearly.
If you don't know the answer or can't find relevant information, be honest about it.`;

		// Add folder information with clear security instructions
		if (config.allowedFolders.length > 0) {
			const folderList = config.allowedFolders.map(f => `"${f}"`).join(', ');
			systemMessage += `\n\nIMPORTANT: For security reasons, code search is restricted to the following allowed folders: ${folderList}.
You MUST specify one of these folders in the path argument when using the search_code tool.".`;
		} else {
			systemMessage += `\n\nNo specific folders are configured for code search, so the current working directory (${process.cwd()}) will be used by default. You can omit the path parameter in your search calls, or use '.' to explicitly search in the current directory.`;
		}

		systemMessage += `\n\nREQUIRMENT: At the end of message respond with all the files and dependencies, even small ones, which were required to answer this question, for example: file#symbol or file:start-end. If symbol, like function or struct is known, use symbol syntax, when range fits better, respond with line range. Be very detailed, and prefer symbol syntax. Use absolute paths.

Examples:
- /src/utils/parser.js#parseConfig - for a specific function
- /src/models/User.js#User.authenticate - for a specific method
- /src/controllers/auth.js:15-42 - for a specific code range
- /package.json - for configuration files
- /src/components/Button.tsx#ButtonProps - for TypeScript interfaces/types
- /src/database/migrations/20230101_create_users.sql:5-10 - for database queries`;

		console.error(systemMessage);

		// Add file list information if available
		try {
			const searchDirectory = config.allowedFolders.length > 0 ? config.allowedFolders[0] : process.cwd();
			console.error(`Generating file list for ${searchDirectory}...`);

			const files = await listFilesByLevel({
				directory: searchDirectory,
				maxFiles: 100,
				respectGitignore: true,
				cwd: process.cwd() // Explicitly set the current working directory
			});

			if (files.length > 0) {
				systemMessage += `\n\nHere is a list of up to 100 files in the codebase (organized by directory depth):\n\n`;
				systemMessage += files.map(file => `- ${file}`).join('\n');
			}

			console.error(`Added ${files.length} files to system message`);
		} catch (error) {
			console.warn(`Warning: Could not generate file list: ${error.message}`);
		}

		return systemMessage;
	}

	/**
	 * Process a user query and get a response
	 */
	async processQuery(query, path, options = {}) {
		try {
			// If path is not provided, use the first allowed folder or current working directory
			let searchPath = path || (config.allowedFolders.length > 0 ? config.allowedFolders[0] : process.cwd());

			// If path is "." or "./", use the default path if available
			const defaultPath = config.allowedFolders.length > 0 ? config.allowedFolders[0] : process.cwd();
			if ((searchPath === "." || searchPath === "./") && defaultPath) {
				console.error(`Using default path "${defaultPath}" instead of "${searchPath}"`);
				searchPath = defaultPath;
			}

			// Validate that the search path is within allowed folders
			if (config.allowedFolders.length > 0) {
				const isAllowed = config.allowedFolders.some(folder =>
					searchPath === folder || searchPath.startsWith(`${folder}/`)
				);

				if (!isAllowed) {
					throw new Error(`Path "${searchPath}" is not within allowed folders. Allowed folders: ${config.allowedFolders.join(', ')}`);
				}
			}

			if (config.debug) {
				console.error(`[DEBUG] Received user query: ${query}`);
				console.error(`[DEBUG] Path context: ${searchPath}`);
			}

			// Count tokens in the user query
			const queryTokens = countTokens(query);
			this.requestTokens += queryTokens;

			// Limit history to prevent token overflow
			if (this.history.length > config.maxHistoryMessages) {
				const historyStart = this.history.length - config.maxHistoryMessages;
				this.history = this.history.slice(historyStart);

				if (config.debug) {
					console.error(`[DEBUG] Trimmed history to ${this.history.length} messages`);
				}
			}

			// Prepare messages array
			const messages = [
				...this.history,
				{ role: 'user', content: query }
			];

			if (config.debug) {
				console.error(`[DEBUG] Sending ${messages.length} messages to model`);
			}

			// Update tools with timeout if provided
			const toolConfig = { ...this.baseToolConfig };
			if (options.timeout) {
				toolConfig.timeout = options.timeout;
				console.error(`Using timeout: ${options.timeout} seconds`);
			}
			const tools = this.createTools(toolConfig);

			// Configure generateText options
			const generateOptions = {
				model: this.provider(this.model),
				messages: messages,
				system: await this.getSystemMessage(),
				tools: tools.map(tool => {
					// Clone the tool and add the search path to its configuration
					// For search tools, the path is treated as an allowed folder, not just a default
					const updatedAllowedFolders = [...config.allowedFolders];

					// Add searchPath to allowedFolders if it's not already included
					if (searchPath && !updatedAllowedFolders.includes(searchPath) &&
						!updatedAllowedFolders.some(folder => searchPath.startsWith(`${folder}/`))) {
						updatedAllowedFolders.push(searchPath);
					}

					const toolConfig = {
						...tool.config,
						defaultPath: searchPath,
						allowedFolders: updatedAllowedFolders // Pass updated allowed folders to all tools
					};
					return { ...tool, config: toolConfig };
				}),
				maxSteps: 15,
				temperature: 0.7,
				maxTokens: config.maxTokens
			};

			// Add API-specific options
			if (this.apiType === 'anthropic' && this.model.includes('3-7')) {
				generateOptions.experimental_thinking = {
					enabled: true,
					budget: 8000
				};
			} else if (this.apiType === 'google' && this.model.includes('gemini')) {
				// Add any Google-specific options here if needed
				generateOptions.temperature = 0.5; // Google models may need different temperature settings
			}

			// Generate response using AI model with tools
			const result = await generateText(generateOptions);

			// Extract the text content from the response
			const responseText = result.text;

			// Add the message and response to history
			this.history.push({ role: 'user', content: query });
			this.history.push({ role: 'assistant', content: responseText });

			// Count tokens in the response
			const responseTokens = countTokens(responseText);
			this.responseTokens += responseTokens;

			// Log tool usage if available
			if (result.toolCalls && result.toolCalls.length > 0) {
				console.error(`Tool was used: ${result.toolCalls.length} times`);

				if (config.debug) {
					result.toolCalls.forEach((call, index) => {
						console.error(`[DEBUG] Tool call ${index + 1}: ${call.name}`);
						if (call.args) {
							console.error(`[DEBUG] Tool call ${index + 1} args:`, JSON.stringify(call.args, null, 2));
						}
						if (call.result) {
							const resultPreview = typeof call.result === 'string'
								? (call.result.length > 100 ? call.result.substring(0, 100) + '... (truncated)' : call.result)
								: JSON.stringify(call.result, null, 2).substring(0, 100) + '... (truncated)';
							console.error(`[DEBUG] Tool call ${index + 1} result preview: ${resultPreview}`);
						}
					});
				}
			}

			// Add token usage information
			const tokenUsage = {
				request: this.requestTokens + this.toolTokenUsage.request,
				response: this.responseTokens + this.toolTokenUsage.response,
				total: this.requestTokens + this.responseTokens + this.toolTokenUsage.request + this.toolTokenUsage.response
			};

			// Format the response with token usage
			const formattedResponse = `${responseText}\n\n---\nToken Usage: ${tokenUsage.total.toLocaleString()} tokens`;

			// Use the extract command to extract code from the AI response
			let extractedCode = '';
			try {
				// Import required modules
				const { tmpdir } = await import('os');
				const { join } = await import('path');
				const { writeFileSync, unlinkSync } = await import('fs');

				// Create a temporary file with the AI response
				const tempFilePath = join(tmpdir(), `ai-response-${this.sessionId}.txt`);
				writeFileSync(tempFilePath, responseText);

				if (config.debug) {
					console.error(`Created temporary file for AI response: ${tempFilePath}`);
				}

				// Use the extract tool from the probe package with the input-file option
				try {
					// Use the extract function from the probe package
					const { extract } = await import('@buger/probe');

					// Call the extract function with the input file
					extractedCode = await extract({
						inputFile: tempFilePath,
						allowTests: true,
						contextLines: 10,
						format: 'plain'
					});

					if (config.debug) {
						console.error(`Extract command output length: ${extractedCode.length} bytes`);
					}
				} catch (extractError) {
					console.error('Error using extract function:', extractError);
					extractedCode = `Error extracting code: ${extractError.message}`;
				}

				// Clean up the temporary file
				try {
					unlinkSync(tempFilePath);
					if (config.debug) {
						console.error(`Removed temporary file: ${tempFilePath}`);
					}
				} catch (cleanupError) {
					console.error(`Warning: Failed to remove temporary file: ${cleanupError.message}`);
				}
			} catch (fileError) {
				console.error('Error handling temporary file:', fileError);
				extractedCode = `Error handling temporary file: ${fileError.message}`;
			}

			// Prepend the extracted code to the response
			const finalResponse = `<code>${extractedCode}</code>\n\n${formattedResponse}`;

			return finalResponse;
		} catch (error) {
			console.error('Error in processQuery:', error);
			return `Error: ${error.message}`;
		}
	}

	/**
	 * Get the current token usage
	 */
	getTokenUsage() {
		return {
			request: this.requestTokens + this.toolTokenUsage.request,
			response: this.responseTokens + this.toolTokenUsage.response,
			total: this.requestTokens + this.responseTokens + this.toolTokenUsage.request + this.toolTokenUsage.response
		};
	}

	/**
	 * Get the session ID for this agent instance
	 */
	getSessionId() {
		return this.sessionId;
	}

	/**
	 * Reset the agent's history and token counters
	 */
	reset() {
		this.history = [];
		this.requestTokens = 0;
		this.responseTokens = 0;
		this.toolTokenUsage = { request: 0, response: 0 };
		this.sessionId = randomUUID();

		// Reconfigure tools with the new session ID
		const configOptions = {
			sessionId: this.sessionId,
			debug: config.debug,
			defaultPath: config.allowedFolders.length > 0 ? config.allowedFolders[0] : process.cwd(), // Use first allowed folder or current working directory
			allowedFolders: config.allowedFolders // Pass allowed folders to tools
		};

		// Create new configured tool instances
		this.tools = [
			searchTool(configOptions),
			queryTool(configOptions),
			extractTool(configOptions)
		];

		if (config.debug) {
			console.error(`[DEBUG] Agent reset with new session ID: ${this.sessionId}`);
		}
	}
}