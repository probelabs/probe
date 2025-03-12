import 'dotenv/config';
import { createServer } from 'http';
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { streamText, generateText } from 'ai';
import { readFileSync, existsSync } from 'fs';
import { resolve } from 'path';
import { probeTool } from './probeTool.js';
import { withAuth } from './auth.js';

// Check for debug mode
const DEBUG = process.env.DEBUG === 'true' || process.env.DEBUG === '1';

// Get API keys from environment variables
const ANTHROPIC_API_KEY = process.env.ANTHROPIC_API_KEY;
const OPENAI_API_KEY = process.env.OPENAI_API_KEY;

// Authentication configuration
const AUTH_ENABLED = process.env.AUTH_ENABLED === 'true' || process.env.AUTH_ENABLED === '1';
const AUTH_USERNAME = process.env.AUTH_USERNAME || 'admin';
const AUTH_PASSWORD = process.env.AUTH_PASSWORD || 'password';

if (AUTH_ENABLED) {
	console.log(`Authentication enabled (username: ${AUTH_USERNAME})`);
} else {
	console.log('Authentication disabled');
}

// Get custom API URLs if provided
const ANTHROPIC_API_URL = process.env.ANTHROPIC_API_URL || 'https://api.anthropic.com/v1';
const OPENAI_API_URL = process.env.OPENAI_API_URL || 'https://api.openai.com/v1';

// Get model override if provided
const MODEL_NAME = process.env.MODEL_NAME;

// Determine which API to use based on available keys
let apiProvider;
let defaultModel;
let apiType;

if (ANTHROPIC_API_KEY) {
	// Initialize Anthropic provider with API key and custom URL if provided
	apiProvider = createAnthropic({
		apiKey: ANTHROPIC_API_KEY,
		baseURL: ANTHROPIC_API_URL,
	});
	defaultModel = MODEL_NAME || 'claude-3-7-sonnet-latest';
	apiType = 'anthropic';

	if (DEBUG) {
		console.log(`[DEBUG] Using Anthropic API with URL: ${ANTHROPIC_API_URL}`);
		console.log(`[DEBUG] Using model: ${defaultModel}`);
	}
} else if (OPENAI_API_KEY) {
	// Initialize OpenAI provider with API key and custom URL if provided
	apiProvider = createOpenAI({
		apiKey: OPENAI_API_KEY,
		baseURL: OPENAI_API_URL,
	});
	defaultModel = MODEL_NAME || 'gpt-4o';
	apiType = 'openai';

	if (DEBUG) {
		console.log(`[DEBUG] Using OpenAI API with URL: ${OPENAI_API_URL}`);
		console.log(`[DEBUG] Using model: ${defaultModel}`);
	}
} else {
	console.error('No API keys found. Please set either ANTHROPIC_API_KEY or OPENAI_API_KEY environment variable.');
	process.exit(1);
}

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
	console.warn('No folders configured. Set ALLOWED_FOLDERS in .env file.');
}

// Track token usage for monitoring
let totalRequestTokens = 0;
let totalResponseTokens = 0;

// Simple token counter function (very approximate)
function countTokens(text) {
	// Rough approximation: 1 token ≈ 4 characters for English text
	return Math.ceil(text.length / 4);
}

/**
 * Handle non-streaming chat request (returns complete response as JSON)
 */
async function handleNonStreamingChatRequest(req, res, message) {
	try {
		// Prepare system message with folder context
		let systemMessage = getSystemMessage();

		// Create messages array with user's message
		const messages = [
			{
				role: 'user',
				content: message
			}
		];

		// Track token usage
		const requestTokens = countTokens(systemMessage) + countTokens(message);
		totalRequestTokens += requestTokens;

		if (DEBUG) {
			console.log(`\n[DEBUG] ===== API Chat Request (non-streaming) =====`);
			console.log(`[DEBUG] User message: "${message}"`);
			console.log(`[DEBUG] System message length: ${systemMessage.length} characters`);
			console.log(`[DEBUG] Estimated request tokens: ${requestTokens}`);
		}

		// Configure generateText options
		const generateOptions = {
			model: apiProvider(defaultModel),
			messages: messages,
			system: systemMessage,
			tools: {
				searchCode: probeTool
			},
			maxSteps: 15,
			temperature: 0.1
		};

		// Add API-specific options
		if (apiType === 'anthropic' && defaultModel.includes('3-7')) {
			generateOptions.experimental_thinking = {
				enabled: true,
				budget: 8000
			};
		}

		// Generate complete response
		const result = await generateText(generateOptions);

		// Log tool usage
		if (result.toolCalls && result.toolCalls.length > 0) {
			console.log('Tool was used:', result.toolCalls.length, 'times');
			result.toolCalls.forEach((call, index) => {
				console.log(`Tool call ${index + 1}:`, call.name);
			});
		}

		// Return response as JSON
		res.writeHead(200, { 'Content-Type': 'application/json' });
		res.end(JSON.stringify({
			response: result.text,
			toolCalls: result.toolCalls || [],
			timestamp: new Date().toISOString()
		}));

		console.log('Finished generating non-streaming response');
	} catch (error) {
		console.error('Error generating response:', error);

		// Determine the appropriate status code and error message
		let statusCode = 500;
		let errorMessage = 'Internal server error';

		if (error.status) {
			// Handle API-specific error codes
			statusCode = error.status;

			// Provide more specific error messages based on status code
			if (statusCode === 401) {
				errorMessage = 'Authentication failed: Invalid API key';
			} else if (statusCode === 403) {
				errorMessage = 'Authorization failed: Insufficient permissions';
			} else if (statusCode === 404) {
				errorMessage = 'Resource not found: Check API endpoint URL';
			} else if (statusCode === 429) {
				errorMessage = 'Rate limit exceeded: Too many requests';
			} else if (statusCode >= 500) {
				errorMessage = 'API server error: Service may be unavailable';
			}
		} else if (error.code === 'ENOTFOUND' || error.code === 'ECONNREFUSED') {
			// Handle connection errors
			statusCode = 503;
			errorMessage = 'Connection failed: Unable to reach API server';
		} else if (error.message && error.message.includes('timeout')) {
			statusCode = 504;
			errorMessage = 'Request timeout: API server took too long to respond';
		}

		res.writeHead(statusCode, { 'Content-Type': 'application/json' });
		res.end(JSON.stringify({
			error: errorMessage,
			message: error.message,
			status: statusCode
		}));
	}
}

/**
 * Handle streaming chat request (returns chunks of text)
 */
async function handleStreamingChatRequest(req, res, message) {
	try {
		// Prepare system message with folder context
		let systemMessage = getSystemMessage();

		// Create messages array with user's message
		const messages = [
			{
				role: 'user',
				content: message
			}
		];

		// Track token usage
		const requestTokens = countTokens(systemMessage) + countTokens(message);
		totalRequestTokens += requestTokens;

		if (DEBUG) {
			console.log(`\n[DEBUG] ===== API Chat Request (streaming) =====`);
			console.log(`[DEBUG] User message: "${message}"`);
			console.log(`[DEBUG] System message length: ${systemMessage.length} characters`);
			console.log(`[DEBUG] Estimated request tokens: ${requestTokens}`);
		}

		res.writeHead(200, {
			'Content-Type': 'text/plain',
			'Transfer-Encoding': 'chunked',
			'Cache-Control': 'no-cache',
			'Connection': 'keep-alive'
		});

		// Configure streamText options
		const streamOptions = {
			model: apiProvider(defaultModel),
			messages: messages,
			system: systemMessage,
			tools: {
				searchCode: probeTool
			},
			maxSteps: 15,
			temperature: 0.1
		};

		// Add API-specific options
		if (apiType === 'anthropic' && defaultModel.includes('3-7')) {
			streamOptions.experimental_thinking = {
				enabled: true,
				budget: 8000
			};
		}

		const result = await streamText(streamOptions);

		// Stream the response chunks
		for await (const chunk of result.textStream) {
			res.write(chunk);
		}

		// Handle the final result after streaming completes
		const finalResult = await result;

		// Log tool usage
		if (finalResult.toolCalls && finalResult.toolCalls.length > 0) {
			console.log('Tool was used:', finalResult.toolCalls.length, 'times');
			finalResult.toolCalls.forEach((call, index) => {
				console.log(`Tool call ${index + 1}:`, call.name);
			});
		}

		res.end();
		console.log('Finished streaming response');
	} catch (error) {
		console.error('Error streaming response:', error);

		// Determine the appropriate status code and error message
		let statusCode = 500;
		let errorMessage = 'Internal server error';

		if (error.status) {
			// Handle API-specific error codes
			statusCode = error.status;

			// Provide more specific error messages based on status code
			if (statusCode === 401) {
				errorMessage = 'Authentication failed: Invalid API key';
			} else if (statusCode === 403) {
				errorMessage = 'Authorization failed: Insufficient permissions';
			} else if (statusCode === 404) {
				errorMessage = 'Resource not found: Check API endpoint URL';
			} else if (statusCode === 429) {
				errorMessage = 'Rate limit exceeded: Too many requests';
			} else if (statusCode >= 500) {
				errorMessage = 'API server error: Service may be unavailable';
			}
		} else if (error.code === 'ENOTFOUND' || error.code === 'ECONNREFUSED') {
			// Handle connection errors
			statusCode = 503;
			errorMessage = 'Connection failed: Unable to reach API server';
		} else if (error.message && error.message.includes('timeout')) {
			statusCode = 504;
			errorMessage = 'Request timeout: API server took too long to respond';
		}

		// For streaming responses, we need to send a plain text error
		res.writeHead(statusCode, { 'Content-Type': 'text/plain' });
		res.end(`Error: ${errorMessage} - ${error.message}`);
	}
}

/**
 * Get system message with folder context
 */
function getSystemMessage() {
	let systemMessage = `## Revised Prompt/Guidelines

**Role:**
You are a helpful assistant that can search a codebase using a specialized tool called 'searchCode'. You must:
1. **Always** use the 'searchCode' tool before providing an answer that references the code.
2. Base your code-related answers **only** on what is found using the 'searchCode' tool.
3. Provide detailed answers, including relevant file names and line numbers.

---

### 1. Use the 'searchCode' Tool First
Whenever a user’s question involves code (e.g., wanting to see how something is implemented, or referencing functions/classes/configuration), **immediately** make one or more calls to the 'searchCode' tool. 

- **Examples** of usage:
  - If the user asks: “How is request size and response size handled in an analytics plugin?”  
    Use 'searchCode' with queries like:
    '(analytics OR plugin) AND (requestSize OR responseSize)'
    or more refined approaches:
    '+analytics +request +response'
    or:
    'plugin AND "request size" AND "response size"'
    adjusting as needed to narrow or broaden results.

### 2. Elasticsearch-Like Query Syntax
The 'searchCode' tool supports an Elasticsearch-like syntax. Use it to refine searches:
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
After you have search results from the 'searchCode' tool, **use them directly** to form your answer:
1. **Summarize** what the code does, referencing relevant lines.
2. Include **file names** and **line numbers** for clarity.
3. If the code does not answer the question directly or more context is needed, state that clearly.

### 4. If No Relevant Results
- If you cannot find the requested information after **multiple** refined searches, ask for further context:
  - Example: “I couldn’t find references to the 'AnalyticsPlugin' handling both request and response sizes in the codebase. Could you clarify which repository or module you’re referring to?”

### 5. General Flow for Each User Query
1. **Read** the user’s question.
2. **Determine** relevant keywords and combine them using the syntax guidelines.
3. **Call** the 'searchCode' tool with your best guess.
4. If needed, **refine** or **broaden** your search in one or two more attempts.
5. **Provide** an answer referencing the search results (file name, line number, code snippet).
6. If no results, ask the user to clarify.

---

## Example Interaction Flow

**User:** “Would an analytics plugin provide both request and response sizes?”

1. **searchCode** call:
   '(analytics OR plugin) AND (requestSize OR responseSize)'
2. Suppose the search results show a file 'plugins/AnalyticsPlugin.java' lines 45-60 referencing request size, and lines 61-70 referencing response size.

3. **Answer** (example):
   - Summarize: “Yes, the 'AnalyticsPlugin' in 'plugins/AnalyticsPlugin.java' calculates both request and response sizes.”
   - Show code references:
     - Lines 45-60 for request size
     - Lines 61-70 for response size

This flow ensures you always rely on real code references and provide a structured, detailed explanation.`;

	if (allowedFolders.length > 0) {
		const folderList = allowedFolders.map(f => `"${f}"`).join(', ');
		systemMessage += ` The following folders are configured for code search: ${folderList}. When using searchCode, specify one of these folders in the folder argument.`;
	}

	return systemMessage;
}

const server = createServer(async (req, res) => {
	// Define route handlers with authentication
	const routes = {
		// UI Routes
		'GET /': withAuth((req, res) => {
			res.writeHead(200, { 'Content-Type': 'text/html' });
			const html = readFileSync('./index.html', 'utf8');
			res.end(html);
		}),

		'GET /folders': withAuth((req, res) => {
			res.writeHead(200, { 'Content-Type': 'application/json' });
			res.end(JSON.stringify({ folders: allowedFolders }));
		}),

		'GET /openapi.yaml': (req, res) => {
			res.writeHead(200, { 'Content-Type': 'text/yaml' });
			const yaml = readFileSync('./openapi.yaml', 'utf8');
			res.end(yaml);
		},

		// API Routes
		'POST /api/search': withAuth(async (req, res) => {
			let body = '';
			req.on('data', chunk => body += chunk);
			req.on('end', async () => {
				try {
					const { keywords, folder, exact, allow_tests } = JSON.parse(body);

					if (!keywords) {
						res.writeHead(400, { 'Content-Type': 'application/json' });
						res.end(JSON.stringify({ error: 'Missing required parameter: keywords' }));
						return;
					}

					if (DEBUG) {
						console.log(`\n[DEBUG] ===== API Search Request =====`);
						console.log(`[DEBUG] Keywords: "${keywords}"`);
						console.log(`[DEBUG] Folder: "${folder || 'default'}"`);
						console.log(`[DEBUG] Exact match: ${exact ? 'yes' : 'no'}`);
						console.log(`[DEBUG] Allow tests: ${allow_tests ? 'yes' : 'no'}`);
					}

					try {
						// Execute the probe tool directly
						const result = await probeTool.execute({
							keywords,
							folder: folder || (allowedFolders.length > 0 ? allowedFolders[0] : undefined),
							exact: exact || false,
							allow_tests: allow_tests || false
						});

						res.writeHead(200, { 'Content-Type': 'application/json' });
						res.end(JSON.stringify(result));
					} catch (error) {
						console.error('Error executing probe command:', error);

						// Determine the appropriate status code and error message
						let statusCode = 500;
						let errorMessage = 'Error executing probe command';

						if (error.code === 'ENOENT') {
							statusCode = 404;
							errorMessage = 'Folder not found or not accessible';
						} else if (error.code === 'EACCES') {
							statusCode = 403;
							errorMessage = 'Permission denied to access folder';
						} else if (error.message && error.message.includes('Invalid folder')) {
							statusCode = 400;
							errorMessage = 'Invalid folder specified';
						} else if (error.message && error.message.includes('timeout')) {
							statusCode = 504;
							errorMessage = 'Search operation timed out';
						}

						res.writeHead(statusCode, { 'Content-Type': 'application/json' });
						res.end(JSON.stringify({
							error: errorMessage,
							message: error.message,
							status: statusCode
						}));
					}
				} catch (error) {
					console.error('Error parsing request body:', error);
					res.writeHead(400, { 'Content-Type': 'application/json' });
					res.end(JSON.stringify({ error: 'Invalid JSON in request body' }));
				}
			});
		}),

		'POST /api/chat': withAuth(async (req, res) => {
			let body = '';
			req.on('data', chunk => body += chunk);
			req.on('end', async () => {
				try {
					const { message, stream } = JSON.parse(body);

					if (!message) {
						res.writeHead(400, { 'Content-Type': 'application/json' });
						res.end(JSON.stringify({ error: 'Missing required parameter: message' }));
						return;
					}

					// Handle streaming vs non-streaming response
					const shouldStream = stream !== false; // Default to streaming

					if (!shouldStream) {
						// Non-streaming response (complete response as JSON)
						await handleNonStreamingChatRequest(req, res, message);
					} else {
						// Streaming response (chunks of text)
						await handleStreamingChatRequest(req, res, message);
					}
				} catch (error) {
					console.error('Error parsing request body:', error);
					res.writeHead(400, { 'Content-Type': 'application/json' });
					res.end(JSON.stringify({ error: 'Invalid JSON in request body' }));
				}
			});
		}),

		'POST /chat': withAuth((req, res) => {
			let body = '';
			req.on('data', chunk => body += chunk);
			req.on('end', async () => {
				try {
					const { message } = JSON.parse(body);

					if (DEBUG) {
						console.log(`\n[DEBUG] ===== Chat Request =====`);
						console.log(`[DEBUG] User message: "${message}"`);
					}

					// Use the shared system message
					let systemMessage = getSystemMessage();

					// Create messages array with user's message
					const messages = [
						{
							role: 'user',
							content: message
						}
					];

					// Track token usage
					const requestTokens = countTokens(systemMessage) + countTokens(message);
					totalRequestTokens += requestTokens;

					if (DEBUG) {
						console.log(`[DEBUG] System message length: ${systemMessage.length} characters`);
						console.log(`[DEBUG] Estimated request tokens: ${requestTokens}`);
						console.log(`[DEBUG] Sending message to ${apiType.charAt(0).toUpperCase() + apiType.slice(1)} with tool support`);
					} else {
						console.log(`Sending message to ${apiType.charAt(0).toUpperCase() + apiType.slice(1)} with tool support`);
					}

					res.writeHead(200, {
						'Content-Type': 'text/plain',
						'Transfer-Encoding': 'chunked',
						'Cache-Control': 'no-cache',
						'Connection': 'keep-alive'
					});

					// Use streamText with tools support
					try {
						// Log which API we're using
						if (DEBUG) {
							console.log(`[DEBUG] Using ${apiType} API with model: ${defaultModel}`);
						} else {
							console.log(`Using ${apiType.charAt(0).toUpperCase() + apiType.slice(1)} API with model: ${defaultModel}`);
						}

						// Configure streamText options based on API type
						const streamOptions = {
							model: apiProvider(defaultModel),
							messages: messages,
							system: systemMessage,
							tools: {
								searchCode: probeTool
							},
							maxSteps: 30,                // Allow up to 15 tool calls
							temperature: 0.1             // Low temperature for more deterministic responses
						};

						// Add API-specific options
						if (apiType === 'anthropic' && defaultModel.includes('3-7')) {
							streamOptions.experimental_thinking = {
								enabled: true,           // Enable thinking mode for Anthropic
								budget: 8000             // Increased thinking budget to match chat.rs max_tokens
							};
						}

						const result = await streamText(streamOptions);

						// Stream the response chunks
						for await (const chunk of result.textStream) {
							if (DEBUG) {
								console.log('[DEBUG] Received chunk:', chunk);
							}
							res.write(chunk);
						}

						// Handle the final result after streaming completes
						const finalResult = await result;

						// Log tool usage
						if (finalResult.toolCalls && finalResult.toolCalls.length > 0) {
							console.log('Tool was used:', finalResult.toolCalls.length, 'times');
							finalResult.toolCalls.forEach((call, index) => {
								console.log(`Tool call ${index + 1}:`, call.name);
							});
						}

						res.end();
						console.log('Finished streaming response');
					} catch (error) {
						console.error('Error streaming response:', error);

						// Determine the appropriate status code and error message
						let statusCode = 500;
						let errorMessage = 'Internal server error';

						if (error.status) {
							// Handle API-specific error codes
							statusCode = error.status;

							// Provide more specific error messages based on status code
							if (statusCode === 401) {
								errorMessage = 'Authentication failed: Invalid API key';
							} else if (statusCode === 403) {
								errorMessage = 'Authorization failed: Insufficient permissions';
							} else if (statusCode === 404) {
								errorMessage = 'Resource not found: Check API endpoint URL';
							} else if (statusCode === 429) {
								errorMessage = 'Rate limit exceeded: Too many requests';
							} else if (statusCode >= 500) {
								errorMessage = 'API server error: Service may be unavailable';
							}
						} else if (error.code === 'ENOTFOUND' || error.code === 'ECONNREFUSED') {
							// Handle connection errors
							statusCode = 503;
							errorMessage = 'Connection failed: Unable to reach API server';
						} else if (error.message && error.message.includes('timeout')) {
							statusCode = 504;
							errorMessage = 'Request timeout: API server took too long to respond';
						}

						// For streaming responses, we need to send a plain text error
						res.writeHead(statusCode, { 'Content-Type': 'text/plain' });
						res.end(`Error: ${errorMessage} - ${error.message}`);
					}
				} catch (error) {
					console.error('Error processing chat request:', error);

					// Determine the appropriate status code and error message
					let statusCode = 500;
					let errorMessage = 'Internal Server Error';

					if (error instanceof SyntaxError) {
						statusCode = 400;
						errorMessage = 'Invalid JSON in request body';
					} else if (error.code === 'EACCES') {
						statusCode = 403;
						errorMessage = 'Permission denied';
					} else if (error.code === 'ENOENT') {
						statusCode = 404;
						errorMessage = 'Resource not found';
					}

					res.writeHead(statusCode, { 'Content-Type': 'text/plain' });
					res.end(`${errorMessage}: ${error.message}`);
				}
			});
		})

	};

	// Route handling logic
	const method = req.method;
	const url = req.url;
	const routeKey = `${method} ${url}`;

	// Check if we have an exact route match
	if (routes[routeKey]) {
		return routes[routeKey](req, res);
	}

	// Check for partial matches (e.g., /api/chat?param=value should match 'POST /api/chat')
	const baseUrl = url.split('?')[0];
	const baseRouteKey = `${method} ${baseUrl}`;

	if (routes[baseRouteKey]) {
		return routes[baseRouteKey](req, res);
	}

	// No route match, return 404
	res.writeHead(404, { 'Content-Type': 'text/plain' });
	res.end('Not Found');
});

// Start the server
const PORT = process.env.PORT || 8080;
server.listen(PORT, () => {
	console.log(`Server running on http://localhost:${PORT}`);
	console.log(`Environment: ${process.env.NODE_ENV || 'development'}`);
	console.log('Probe tool is available for AI to use');
});