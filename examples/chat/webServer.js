import 'dotenv/config';
import { createServer } from 'http';
import { streamText } from 'ai';
import { readFileSync, existsSync } from 'fs';
import { resolve, dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { ProbeChat } from './probeChat.js';
import { authMiddleware, withAuth } from './auth.js';
import { probeTool, searchToolInstance, queryToolInstance, extractToolInstance } from './probeTool.js';

// Get the directory name of the current module
const __dirname = dirname(fileURLToPath(import.meta.url));

/**
 * Start the web server
 * @param {string} version - The version of the application
 */
export function startWebServer(version) {
	// Authentication configuration
	const AUTH_ENABLED = process.env.AUTH_ENABLED === 'true' || process.env.AUTH_ENABLED === '1';
	const AUTH_USERNAME = process.env.AUTH_USERNAME || 'admin';
	const AUTH_PASSWORD = process.env.AUTH_PASSWORD || 'password';

	if (AUTH_ENABLED) {
		console.log(`Authentication enabled (username: ${AUTH_USERNAME})`);
	} else {
		console.log('Authentication disabled');
	}

	// Initialize the ProbeChat instance
	let probeChat;
	try {
		probeChat = new ProbeChat();
		console.log(`Session ID: ${probeChat.getSessionId()}`);
	} catch (error) {
		console.error('Error initializing ProbeChat:', error.message);
		process.exit(1);
	}

	// Define the tools available to the AI
	const tools = [probeTool, searchToolInstance, queryToolInstance, extractToolInstance];

	// Track token usage for monitoring
	let totalRequestTokens = 0;
	let totalResponseTokens = 0;

	/**
	 * Handle non-streaming chat request (returns complete response as JSON)
	 */
	async function handleNonStreamingChatRequest(req, res, message) {
		try {
			const DEBUG = process.env.DEBUG === 'true' || process.env.DEBUG === '1';
			if (DEBUG) {
				console.log(`\n[DEBUG] ===== API Chat Request (non-streaming) =====`);
				console.log(`[DEBUG] User message: "${message}"`);
			}

			// Use the ProbeChat instance to get a response
			const responseText = await probeChat.chat(message);

			// Get token usage
			const tokenUsage = probeChat.getTokenUsage();
			totalRequestTokens = tokenUsage.request;
			totalResponseTokens = tokenUsage.response;

			// Return response as JSON
			res.writeHead(200, { 'Content-Type': 'application/json' });
			res.end(JSON.stringify({
				response: responseText,
				tokenUsage: tokenUsage,
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
			const DEBUG = process.env.DEBUG === 'true' || process.env.DEBUG === '1';
			if (DEBUG) {
				console.log(`\n[DEBUG] ===== API Chat Request (streaming) =====`);
				console.log(`[DEBUG] User message: "${message}"`);
			}

			res.writeHead(200, {
				'Content-Type': 'text/plain',
				'Transfer-Encoding': 'chunked',
				'Cache-Control': 'no-cache',
				'Connection': 'keep-alive'
			});

			// Use the ProbeChat instance to get a response
			const responseText = await probeChat.chat(message);

			// Get token usage
			const tokenUsage = probeChat.getTokenUsage();
			totalRequestTokens = tokenUsage.request;
			totalResponseTokens = tokenUsage.response;

			// Write the response as a single chunk
			res.write(responseText);
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

	const server = createServer(async (req, res) => {
		// Define route handlers with authentication
		const routes = {
			// UI Routes
			'GET /': withAuth((req, res) => {
				res.writeHead(200, { 'Content-Type': 'text/html' });
				const html = readFileSync(join(__dirname, 'index.html'), 'utf8');
				res.end(html);
			}),

			'GET /folders': withAuth((req, res) => {
				res.writeHead(200, { 'Content-Type': 'application/json' });
				res.end(JSON.stringify({ folders: probeChat.allowedFolders || [] }));
			}),

			'GET /openapi.yaml': (req, res) => {
				const yamlPath = join(__dirname, 'openapi.yaml');
				if (existsSync(yamlPath)) {
					res.writeHead(200, { 'Content-Type': 'text/yaml' });
					const yaml = readFileSync(yamlPath, 'utf8');
					res.end(yaml);
				} else {
					res.writeHead(404, { 'Content-Type': 'text/plain' });
					res.end('OpenAPI specification not found');
				}
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

						const DEBUG = process.env.DEBUG === 'true' || process.env.DEBUG === '1';
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
								folder: folder || (probeChat.allowedFolders && probeChat.allowedFolders.length > 0 ? probeChat.allowedFolders[0] : '.'),
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

			'POST /api/query': withAuth(async (req, res) => {
				let body = '';
				req.on('data', chunk => body += chunk);
				req.on('end', async () => {
					try {
						const { pattern, path, language, allow_tests } = JSON.parse(body);

						if (!pattern) {
							res.writeHead(400, { 'Content-Type': 'application/json' });
							res.end(JSON.stringify({ error: 'Missing required parameter: pattern' }));
							return;
						}

						const DEBUG = process.env.DEBUG === 'true' || process.env.DEBUG === '1';
						if (DEBUG) {
							console.log(`\n[DEBUG] ===== API Query Request =====`);
							console.log(`[DEBUG] Pattern: "${pattern}"`);
							console.log(`[DEBUG] Path: "${path || 'default'}"`);
							console.log(`[DEBUG] Language: "${language || 'default'}"`);
							console.log(`[DEBUG] Allow tests: ${allow_tests ? 'yes' : 'no'}`);
						}

						try {
							// Execute the query tool
							const result = await queryToolInstance.execute({
								pattern,
								path: path || (probeChat.allowedFolders && probeChat.allowedFolders.length > 0 ? probeChat.allowedFolders[0] : '.'),
								language: language || undefined,
								allow_tests: allow_tests || false
							});

							res.writeHead(200, { 'Content-Type': 'application/json' });
							res.end(JSON.stringify({
								results: result,
								timestamp: new Date().toISOString()
							}));
						} catch (error) {
							console.error('Error executing query command:', error);

							// Determine the appropriate status code and error message
							let statusCode = 500;
							let errorMessage = 'Error executing query command';

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

			'POST /api/extract': withAuth(async (req, res) => {
				let body = '';
				req.on('data', chunk => body += chunk);
				req.on('end', async () => {
					try {
						const { file_path, line, end_line, allow_tests, context_lines, format } = JSON.parse(body);

						if (!file_path) {
							res.writeHead(400, { 'Content-Type': 'application/json' });
							res.end(JSON.stringify({ error: 'Missing required parameter: file_path' }));
							return;
						}

						const DEBUG = process.env.DEBUG === 'true' || process.env.DEBUG === '1';
						if (DEBUG) {
							console.log(`\n[DEBUG] ===== API Extract Request =====`);
							console.log(`[DEBUG] File path: "${file_path}"`);
							console.log(`[DEBUG] Line: ${line || 'not specified'}`);
							console.log(`[DEBUG] End line: ${end_line || 'not specified'}`);
							console.log(`[DEBUG] Allow tests: ${allow_tests ? 'yes' : 'no'}`);
							console.log(`[DEBUG] Context lines: ${context_lines || 'default'}`);
							console.log(`[DEBUG] Format: ${format || 'default'}`);
						}

						try {
							// Execute the extract tool
							const result = await extractToolInstance.execute({
								file_path,
								line,
								end_line,
								allow_tests: allow_tests || false,
								context_lines: context_lines || 10,
								format: format || 'plain'
							});

							res.writeHead(200, { 'Content-Type': 'application/json' });
							res.end(JSON.stringify({
								results: result,
								timestamp: new Date().toISOString()
							}));
						} catch (error) {
							console.error('Error executing extract command:', error);

							// Determine the appropriate status code and error message
							let statusCode = 500;
							let errorMessage = 'Error executing extract command';

							if (error.code === 'ENOENT') {
								statusCode = 404;
								errorMessage = 'File not found or not accessible';
							} else if (error.code === 'EACCES') {
								statusCode = 403;
								errorMessage = 'Permission denied to access file';
							} else if (error.message && error.message.includes('Invalid file')) {
								statusCode = 400;
								errorMessage = 'Invalid file specified';
							} else if (error.message && error.message.includes('timeout')) {
								statusCode = 504;
								errorMessage = 'Extract operation timed out';
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

						const DEBUG = process.env.DEBUG === 'true' || process.env.DEBUG === '1';
						if (DEBUG) {
							console.log(`\n[DEBUG] ===== Chat Request =====`);
							console.log(`[DEBUG] User message: "${message}"`);
						}

						res.writeHead(200, {
							'Content-Type': 'text/plain',
							'Transfer-Encoding': 'chunked',
							'Cache-Control': 'no-cache',
							'Connection': 'keep-alive'
						});

						// Use the ProbeChat instance to get a response
						const responseText = await probeChat.chat(message);

						// Write the response
						res.write(responseText);
						res.end();

						// Get token usage
						const tokenUsage = probeChat.getTokenUsage();
						totalRequestTokens = tokenUsage.request;
						totalResponseTokens = tokenUsage.response;

						console.log('Finished streaming response');
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
		console.log(`Probe Web Interface v${version}`);
		console.log(`Server running on http://localhost:${PORT}`);
		console.log(`Environment: ${process.env.NODE_ENV || 'development'}`);
		console.log('Probe tool is available for AI to use');
		console.log(`Session ID: ${probeChat.getSessionId()}`);
	});
}