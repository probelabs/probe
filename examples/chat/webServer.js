import 'dotenv/config';
import { createServer } from 'http';
import { streamText } from 'ai';
import { readFileSync, existsSync } from 'fs';
import { resolve, dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { randomUUID } from 'crypto';
import { ProbeChat } from './probeChat.js';
import { TokenUsageDisplay } from './tokenUsageDisplay.js';
import { authMiddleware, withAuth } from './auth.js';
import {
	probeTool,
	searchToolInstance,
	queryToolInstance,
	extractToolInstance,
	toolCallEmitter,
	cancelToolExecutions,
	clearToolExecutionData,
	isSessionCancelled
} from './probeTool.js';
import { registerRequest, cancelRequest, clearRequest, isRequestActive } from './cancelRequest.js';

// Get the directory name of the current module
const __dirname = dirname(fileURLToPath(import.meta.url));


// Map to store chat instances by session ID
const chatSessions = new Map();

/**
 * Retrieve or create a ProbeChat instance keyed by sessionId.
 */
function getOrCreateChat(sessionId) {
	if (!sessionId) {
		// Safety fallback: generate a random ID if missing
		sessionId = crypto.randomUUID();
	}
	if (chatSessions.has(sessionId)) {
		return chatSessions.get(sessionId);
	}
	const newChat = new ProbeChat({ sessionId });
	chatSessions.set(sessionId, newChat);
	return newChat;
}

/**
 * Start the web server
 * @param {string} version - The version of the application
 * @param {boolean} hasApiKeys - Whether any API keys are configured
 */
export function startWebServer(version, hasApiKeys = true) {
	// Authentication configuration
	const AUTH_ENABLED = process.env.AUTH_ENABLED === '1';
	const AUTH_USERNAME = process.env.AUTH_USERNAME || 'admin';
	const AUTH_PASSWORD = process.env.AUTH_PASSWORD || 'password';

	if (AUTH_ENABLED) {
		console.log(`Authentication enabled (username: ${AUTH_USERNAME})`);
	} else {
		console.log('Authentication disabled');
	}

	// Map to store SSE clients by session ID
	const sseClients = new Map();

	// Initialize the ProbeChat instance if API keys are available
	let probeChat;
	let noApiKeysMode = false;

	if (hasApiKeys) {
		try {
			// Generate a default session ID for the server
			const defaultSessionId = randomUUID();
			console.log(`Generated default session ID: ${defaultSessionId}`);

			probeChat = new ProbeChat({
				sessionId: defaultSessionId // Use the default session ID
			});
			console.log(`Server initialized with session ID: ${probeChat.getSessionId()}`);
		} catch (error) {
			console.error('Error initializing ProbeChat:', error.message);
			noApiKeysMode = true;
			console.log('Running in No API Keys mode - will show setup instructions to users');
		}
	} else {
		noApiKeysMode = true;
		console.log('Running in No API Keys mode - will show setup instructions to users');
	}

	// Define the tools available to the AI (only if we have API keys)
	const tools = noApiKeysMode ? [] : [probeTool, searchToolInstance, queryToolInstance, extractToolInstance];

	/**
	 * Handle non-streaming chat request (returns complete response as JSON)
	 */
	async function handleNonStreamingChatRequest(req, res, message, sessionId) {
		try {
			const DEBUG = process.env.DEBUG_CHAT === '1';
			if (DEBUG) {
				console.log(`\n[DEBUG] ===== API Chat Request (non-streaming) =====`);
				console.log(`[DEBUG] User message: "${message}"`);
				console.log(`[DEBUG] Session ID: ${sessionId}`);
			}

			// Check if we have a chat instance for this session
			let chatInstance = chatSessions.get(sessionId);

			if (DEBUG) {
				if (chatInstance) {
					console.log(`[DEBUG] Found existing chat instance for session: ${sessionId} with history length: ${chatInstance.history.length}`);
				} else {
					console.log(`[DEBUG] No existing chat instance found for session: ${sessionId}, creating new one`);
				}
			}

			// If no chat instance exists for this session, create one
			if (!chatInstance) {
				chatInstance = new ProbeChat({ sessionId });
				chatSessions.set(sessionId, chatInstance);

				if (DEBUG) {
					console.log(`[DEBUG] Created new chat instance for session: ${sessionId}`);
					console.log(`[DEBUG] Total chat sessions in memory: ${chatSessions.size}`);

					// Log all active sessions
					console.log(`[DEBUG] Active sessions:`);
					for (const [sid, chat] of chatSessions.entries()) {
						console.log(`[DEBUG]   - Session ${sid}: ${chat.history.length} messages in history`);
					}
				}
			} else if (DEBUG) {
				console.log(`[DEBUG] Using existing chat instance with ${chatInstance.history.length} messages in history`);
			}

			// Check if this is a special clear history message
			if (message === '__clear_history__' || JSON.parse(body).clearHistory) {
				console.log(`Clearing chat history for session: ${sessionId}`);
				// Clear the history for this chat instance
				if (typeof chatInstance.clearHistory === 'function') {
					chatInstance.clearHistory();
					res.writeHead(200, { 'Content-Type': 'application/json' });
					res.end(JSON.stringify({
						response: 'Chat history cleared',
						timestamp: new Date().toISOString()
					}));
					return;
				}
			}

			// Use the chat instance to get a response
			const responseText = await chatInstance.chat(message);

			// Get token usage (now includes both current and total)
			const tokenUsage = chatInstance.getTokenUsage();

			// Include token usage in response headers
			const tokenUsageHeader = JSON.stringify(tokenUsage);

			// Debug log for token usage
			console.log(`[DEBUG] Including token usage in response headers: ${tokenUsageHeader}`);

			// Log Anthropic cache token usage if available
			if (tokenUsage.total.anthropic && tokenUsage.total.anthropic.cacheTotal > 0) {
				console.log(`[DEBUG] Anthropic cache token usage: Creation=${tokenUsage.total.anthropic.cacheCreation}, Read=${tokenUsage.total.anthropic.cacheRead}, Total=${tokenUsage.total.anthropic.cacheTotal}`);
			}

			// Log OpenAI cached prompt tokens if available
			if (tokenUsage.total.openai && tokenUsage.total.openai.cachedPrompt > 0) {
				console.log(`[DEBUG] OpenAI cached prompt tokens: ${tokenUsage.total.openai.cachedPrompt}`);
			}

			// Return response as JSON with CORS headers
			res.writeHead(200, {
				'Content-Type': 'application/json',
				'Access-Control-Allow-Origin': '*',
				'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
				'Access-Control-Allow-Headers': 'Content-Type',
				'Access-Control-Expose-Headers': 'X-Token-Usage'
			});
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
	async function handleStreamingChatRequest(req, res, message, sessionId) {
		try {
			const DEBUG = process.env.DEBUG_CHAT === '1';
			if (DEBUG) {
				console.log(`\n[DEBUG] ===== API Chat Request (streaming) =====`);
				console.log(`[DEBUG] User message: "${message}"`);
				console.log(`[DEBUG] Session ID: ${sessionId}`);
			}

			res.writeHead(200, {
				'Content-Type': 'text/plain',
				'Transfer-Encoding': 'chunked',
				'Cache-Control': 'no-cache',
				'Connection': 'keep-alive'
			});

			// Check if we have a chat instance for this session
			let chatInstance = chatSessions.get(sessionId);

			if (DEBUG) {
				if (chatInstance) {
					console.log(`[DEBUG] Found existing chat instance for session: ${sessionId} with history length: ${chatInstance.history.length}`);
				} else {
					console.log(`[DEBUG] No existing chat instance found for session: ${sessionId}, creating new one`);
				}
			}

			// If no chat instance exists for this session, create one
			if (!chatInstance) {
				chatInstance = new ProbeChat({ sessionId });
				chatSessions.set(sessionId, chatInstance);

				if (DEBUG) {
					console.log(`[DEBUG] Created new chat instance for session: ${sessionId}`);
					console.log(`[DEBUG] Total chat sessions in memory: ${chatSessions.size}`);

					// Log all active sessions
					console.log(`[DEBUG] Active sessions:`);
					for (const [sid, chat] of chatSessions.entries()) {
						console.log(`[DEBUG]   - Session ${sid}: ${chat.history.length} messages in history`);
					}
				}
			} else if (DEBUG) {
				console.log(`[DEBUG] Using existing chat instance with ${chatInstance.history.length} messages in history`);
			}

			// Check if this is a special clear history message
			if (message === '__clear_history__' || JSON.parse(body).clearHistory) {
				console.log(`Clearing chat history for session: ${sessionId}`);
				// Clear the history for this chat instance
				if (typeof chatInstance.clearHistory === 'function') {
					chatInstance.clearHistory();
					res.write('Chat history cleared');
					res.end();
					return;
				}
			}

			// Use the chat instance to get a response
			const responseText = await chatInstance.chat(message);

			// Get token usage (now includes both current and total)
			const tokenUsage = chatInstance.getTokenUsage();

			// Include token usage in response headers
			const display = new TokenUsageDisplay();
			const formattedUsage = display.format(tokenUsage);
			const tokenUsageHeader = JSON.stringify(formattedUsage);

			// Debug log for token usage in streaming response
			console.log(`[DEBUG] Including token usage in streaming response headers: ${tokenUsageHeader}`);

			// Log Anthropic cache token usage if available
			if (tokenUsage.total.anthropic && tokenUsage.total.anthropic.cacheTotal > 0) {
				console.log(`[DEBUG] Anthropic cache token usage: Creation=${tokenUsage.total.anthropic.cacheCreation}, Read=${tokenUsage.total.anthropic.cacheRead}, Total=${tokenUsage.total.anthropic.cacheTotal}`);
			}

			// Log OpenAI cached prompt tokens if available
			if (tokenUsage.total.openai && tokenUsage.total.openai.cachedPrompt > 0) {
				console.log(`[DEBUG] OpenAI cached prompt tokens: ${tokenUsage.total.openai.cachedPrompt}`);
			}

			// Set token usage and CORS headers
			res.setHeader('X-Token-Usage', tokenUsageHeader);
			res.setHeader('Access-Control-Allow-Origin', '*');
			res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
			res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
			res.setHeader('Access-Control-Expose-Headers', 'X-Token-Usage');

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

	// Helper function to send SSE data
	function sendSSEData(res, data, eventType = 'message') {
		const DEBUG = process.env.DEBUG_CHAT === '1';
		try {
			if (DEBUG) {
				console.log(`[DEBUG] Sending SSE data, event type: ${eventType}`);
			}
			res.write(`event: ${eventType}\n`);
			res.write(`data: ${JSON.stringify(data)}\n\n`);
			if (DEBUG) {
				console.log(`[DEBUG] SSE data sent successfully for event: ${eventType}`);
				console.log(`[DEBUG] SSE data content: ${JSON.stringify(data).substring(0, 200)}${JSON.stringify(data).length > 200 ? '...' : ''}`);
			}
		} catch (error) {
			console.error(`[DEBUG] Error sending SSE data:`, error);
		}
	}

	// Map to store active chat instances by session ID for cancellation purposes
	const activeChatInstances = new Map();

	const server = createServer(async (req, res) => {
		// Apply authentication middleware to all requests first
		const processRequest = (routeHandler) => {
			// First apply authentication middleware
			authMiddleware(req, res, () => {
				// Then process the route if authentication passes
				routeHandler(req, res);
			});
		};

		// Define route handlers
		const routes = {
			// Handle OPTIONS requests for CORS preflight
			'OPTIONS /api/token-usage': (req, res) => {
				res.writeHead(200, {
					'Access-Control-Allow-Origin': '*',
					'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
					'Access-Control-Allow-Headers': 'Content-Type',
					'Access-Control-Max-Age': '86400' // 24 hours
				});
				res.end();
			},

			// Handle OPTIONS requests for chat endpoint
			'OPTIONS /chat': (req, res) => {
				res.writeHead(200, {
					'Access-Control-Allow-Origin': '*',
					'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
					'Access-Control-Allow-Headers': 'Content-Type',
					'Access-Control-Max-Age': '86400' // 24 hours
				});
				res.end();
			},
			// Token usage API endpoint
			'GET /api/token-usage': (req, res) => {
				// Parse session ID from query parameter
				let sessionId;
				try {
					const url = new URL(req.url, `http://${req.headers.host}`);
					sessionId = url.searchParams.get('sessionId');
				} catch (error) {
					// Fallback to manual parsing
					const queryString = req.url.split('?')[1] || '';
					const params = new URLSearchParams(queryString);
					sessionId = params.get('sessionId');
				}

				if (!sessionId) {
					res.writeHead(400, { 'Content-Type': 'application/json' });
					res.end(JSON.stringify({ error: 'Missing sessionId parameter' }));
					return;
				}

				// Get the chat instance for this session
				const chatInstance = chatSessions.get(sessionId);
				if (!chatInstance) {
					res.writeHead(404, { 'Content-Type': 'application/json' });
					res.end(JSON.stringify({ error: 'Session not found' }));
					return;
				}

				// Get token usage
				const tokenUsage = chatInstance.getTokenUsage();

				// Format the usage data for UI
				const display = new TokenUsageDisplay();
				const formattedUsage = display.format(tokenUsage);

				// Return formatted usage as JSON with CORS headers
				res.writeHead(200, {
					'Content-Type': 'application/json',
					'Cache-Control': 'no-cache',
					'Access-Control-Allow-Origin': '*',
					'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
					'Access-Control-Allow-Headers': 'Content-Type'
				});
				res.end(JSON.stringify(formattedUsage));
			},
			// Static file routes
			'GET /logo.png': (req, res) => {
				const logoPath = join(__dirname, 'logo.png');
				if (existsSync(logoPath)) {
					res.writeHead(200, { 'Content-Type': 'image/png' });
					const logoData = readFileSync(logoPath);
					res.end(logoData);
				} else {
					res.writeHead(404, { 'Content-Type': 'text/plain' });
					res.end('Logo not found');
				}
			},
			// UI Routes
			'GET /': (req, res) => {
				res.writeHead(200, { 'Content-Type': 'text/html' });
				const html = readFileSync(join(__dirname, 'index.html'), 'utf8');

				// If we're in no API keys mode, add a flag to the HTML
				if (noApiKeysMode) {
					const modifiedHtml = html.replace('<body>', '<body data-no-api-keys="true">');
					res.end(modifiedHtml);
				} else {
					res.end(html);
				}
			},

			'GET /folders': (req, res) => {
				// Get the current working directory
				const currentWorkingDir = process.cwd();

				// Use the allowed folders if available, or default to the current working directory
				const folders = probeChat && probeChat.allowedFolders && probeChat.allowedFolders.length > 0
					? probeChat.allowedFolders
					: [currentWorkingDir];

				console.log(`Current working directory: ${currentWorkingDir}`);
				console.log(`Returning folders: ${JSON.stringify(folders)}`);

				res.writeHead(200, { 'Content-Type': 'application/json' });
				res.end(JSON.stringify({
					folders: folders,
					currentDir: currentWorkingDir,
					noApiKeysMode: noApiKeysMode
				}));
			},

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

			// SSE endpoint for tool calls - no authentication for easier testing
			'GET /api/tool-events': (req, res) => {
				// Parse session ID from query parameter
				let sessionId;
				try {
					const url = new URL(req.url, `http://${req.headers.host}`);
					sessionId = url.searchParams.get('sessionId');
					const DEBUG = process.env.DEBUG_CHAT === '1';
					if (DEBUG) {
						console.log(`[DEBUG] Parsed URL: ${url.toString()}, sessionId: ${sessionId}`);
					}
				} catch (error) {
					const DEBUG = process.env.DEBUG_CHAT === '1';
					if (DEBUG) {
						console.error(`[DEBUG] Error parsing URL: ${error.message}`);
					}
					// Fallback to manual parsing
					const queryString = req.url.split('?')[1] || '';
					const params = new URLSearchParams(queryString);
					sessionId = params.get('sessionId');
					if (DEBUG) {
						console.log(`[DEBUG] Manually parsed sessionId: ${sessionId}`);
					}
				}

				if (!sessionId) {
					if (process.env.DEBUG_CHAT === '1') {
						console.error(`[DEBUG] No sessionId found in request URL: ${req.url}`);
					}
					res.writeHead(400, { 'Content-Type': 'application/json' });
					res.end(JSON.stringify({ error: 'Missing sessionId parameter' }));
					return;
				}

				const DEBUG = process.env.DEBUG_CHAT === '1';
				if (DEBUG) {
					console.log(`[DEBUG] Setting up SSE connection for session: ${sessionId}`);
				}

				// Set headers for SSE
				res.writeHead(200, {
					'Content-Type': 'text/event-stream',
					'Cache-Control': 'no-cache',
					'Connection': 'keep-alive',
					'Access-Control-Allow-Origin': '*'
				});

				if (DEBUG) {
					console.log(`[DEBUG] SSE headers set for session: ${sessionId}`);
					console.log(`[DEBUG] Headers:`, {
						'Content-Type': 'text/event-stream',
						'Cache-Control': 'no-cache',
						'Connection': 'keep-alive',
						'Access-Control-Allow-Origin': '*'
					});
				}

				// Send initial connection established event
				const connectionData = {
					type: 'connection',
					message: 'Connection established',
					sessionId,
					timestamp: new Date().toISOString()
				};

				if (DEBUG) {
					console.log(`[DEBUG] Sending initial connection event for session: ${sessionId}`);
					console.log(`[DEBUG] Connection data:`, connectionData);
				}

				sendSSEData(res, connectionData, 'connection');

				// Send a test event to verify the connection is working
				setTimeout(() => {
					const DEBUG = process.env.DEBUG_CHAT === '1';

					if (DEBUG) {
						console.log(`[DEBUG] Sending test event to session: ${sessionId}`);
					}

					const testData = {
						type: 'test',
						message: 'SSE connection test event',
						timestamp: new Date().toISOString(),
						sessionId,
						status: 'active',
						connectionInfo: {
							clientCount: sseClients.size,
							serverTime: new Date().toISOString(),
							testId: Math.random().toString(36).substring(2, 15)
						}
					};

					if (DEBUG) {
						console.log(`[DEBUG] Test event data:`, testData);
					}

					sendSSEData(res, testData, 'test');

					// Send a second test event after a short delay to verify continuous connection
					setTimeout(() => {
						if (DEBUG) {
							console.log(`[DEBUG] Sending follow-up test event to session: ${sessionId}`);
						}

						const followUpTestData = {
							type: 'test',
							message: 'SSE connection follow-up test event',
							timestamp: new Date().toISOString(),
							sessionId,
							status: 'confirmed',
							sequence: 2
						};

						sendSSEData(res, followUpTestData, 'test');
					}, 2000);
				}, 1000);

				// Function to handle tool call events for this session
				const handleToolCall = (toolCall) => {
					const DEBUG = process.env.DEBUG_CHAT === '1';

					if (DEBUG) {
						console.log(`[DEBUG] Handling tool call for session ${sessionId}:`);
						console.log(`[DEBUG] Tool call name: ${toolCall.name}`);
						console.log(`[DEBUG] Tool call timestamp: ${toolCall.timestamp}`);
						console.log(`[DEBUG] Tool call args:`, toolCall.args);

						// Only log a preview of the result to avoid flooding the console
						if (toolCall.resultPreview) {
							const preview = toolCall.resultPreview.substring(0, 100) +
								(toolCall.resultPreview.length > 100 ? '... (truncated)' : '');
							console.log(`[DEBUG] Tool call result preview: ${preview}`);
						}
					}

					// Add a flag to indicate this is being sent via SSE
					const enhancedToolCall = {
						...toolCall,
						_via_sse: true,
						_sent_at: new Date().toISOString()
					};

					// Send the tool call data via SSE
					sendSSEData(res, enhancedToolCall, 'toolCall');

					if (DEBUG) {
						console.log(`[DEBUG] Tool call event sent via SSE for session ${sessionId}`);
					}
				};

				// Register event listener for this session
				const eventName = `toolCall:${sessionId}`;
				if (DEBUG) {
					console.log(`[DEBUG] Registering event listener for: ${eventName}`);
				}

				// Remove any existing listeners for this session to avoid duplicates
				toolCallEmitter.removeAllListeners(eventName);

				// Add the new listener
				toolCallEmitter.on(eventName, handleToolCall);
				if (DEBUG) {
					console.log(`[DEBUG] Registered event listener for session ${sessionId}`);
				}

				// Log the number of listeners
				if (process.env.DEBUG_CHAT === '1') {
					const listenerCount = toolCallEmitter.listenerCount(eventName);
					console.log(`[DEBUG] Current listener count for ${eventName}: ${listenerCount}`);
				}

				// Add client to the map
				sseClients.set(sessionId, res);
				if (DEBUG) {
					console.log(`[DEBUG] Added SSE client for session ${sessionId}, total clients: ${sseClients.size}`);
				}

				// Handle client disconnect
				req.on('close', () => {
					if (DEBUG) {
						console.log(`[DEBUG] SSE client disconnecting: ${sessionId}`);
					}
					toolCallEmitter.removeListener(eventName, handleToolCall);
					sseClients.delete(sessionId);
					if (DEBUG) {
						console.log(`[DEBUG] SSE client disconnected: ${sessionId}, remaining clients: ${sseClients.size}`);
					}
				});
			},

			// Cancellation endpoint
			'POST /cancel-request': async (req, res) => {
				let body = '';
				req.on('data', chunk => body += chunk);
				req.on('end', async () => {
					try {
						const { sessionId } = JSON.parse(body);

						if (!sessionId) {
							res.writeHead(400, { 'Content-Type': 'application/json' });
							res.end(JSON.stringify({ error: 'Missing required parameter: sessionId' }));
							return;
						}

						const DEBUG = process.env.DEBUG_CHAT === '1';
						if (DEBUG) {
							console.log(`\n[DEBUG] ===== Cancel Request =====`);
							console.log(`[DEBUG] Session ID: ${sessionId}`);
						}

						// Cancel any active tool executions for this session
						const toolExecutionsCancelled = cancelToolExecutions(sessionId);

						// Cancel the request in the request tracker
						const requestCancelled = cancelRequest(sessionId);

						// Get the chat instance for this session
						const chatInstance = activeChatInstances.get(sessionId);
						let chatInstanceAborted = false;

						if (chatInstance) {
							// Signal to the chat instance to abort
							if (typeof chatInstance.abort === 'function') {
								try {
									chatInstance.abort();
									chatInstanceAborted = true;
									if (DEBUG) {
										console.log(`[DEBUG] Aborted chat instance for session: ${sessionId}`);
									}
								} catch (error) {
									console.error(`Error aborting chat instance for session ${sessionId}:`, error);
								}
							}

							// Remove the chat instance
							activeChatInstances.delete(sessionId);
						}

						// Log the cancellation status
						console.log(`Cancellation status for session ${sessionId}:`);
						console.log(`- Tool executions cancelled: ${toolExecutionsCancelled}`);
						console.log(`- Request cancelled: ${requestCancelled}`);
						console.log(`- Chat instance aborted: ${chatInstanceAborted}`);

						res.writeHead(200, { 'Content-Type': 'application/json' });
						res.end(JSON.stringify({
							success: true,
							message: 'Cancellation request processed',
							details: {
								toolExecutionsCancelled,
								requestCancelled,
								chatInstanceAborted
							},
							timestamp: new Date().toISOString()
						}));
					} catch (error) {
						console.error('Error parsing request body:', error);
						res.writeHead(400, { 'Content-Type': 'application/json' });
						res.end(JSON.stringify({ error: 'Invalid JSON in request body' }));
					}
				});
			},

			// API Routes
			'POST /api/search': async (req, res) => {
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

						const DEBUG = process.env.DEBUG_CHAT === '1';
						if (DEBUG) {
							console.log(`\n[DEBUG] ===== API Search Request =====`);
							console.log(`[DEBUG] Keywords: "${keywords}"`);
							console.log(`[DEBUG] Folder: "${folder || 'default'}"`);
							console.log(`[DEBUG] Exact match: ${exact ? 'yes' : 'no'}`);
							console.log(`[DEBUG] Allow tests: ${allow_tests ? 'yes' : 'no'}`);
						}

						try {
							// Get session ID from request if available
							const requestSessionId = JSON.parse(body).sessionId;
							const sessionId = requestSessionId || probeChat.getSessionId();
							if (DEBUG) {
								console.log(`[DEBUG] Using session ID for direct tool call: ${sessionId}`);
							}

							// Execute the probe tool directly
							const result = await probeTool.execute({
								keywords,
								folder: folder || (probeChat.allowedFolders && probeChat.allowedFolders.length > 0 ? probeChat.allowedFolders[0] : '.'),
								exact: exact || false,
								allow_tests: allow_tests || false
							});

							// Emit tool call event
							const toolCallData = {
								timestamp: new Date().toISOString(),
								name: 'searchCode',
								args: {
									keywords,
									folder: folder || '.',
									exact: exact || false,
									allow_tests: allow_tests || false
								},
								resultPreview: JSON.stringify(result).substring(0, 200) + '... (truncated)'
							};

							if (DEBUG) {
								console.log(`[DEBUG] Emitting direct tool call event for session ${sessionId}`);
							}
							// Add a unique ID to the tool call data to help with deduplication
							toolCallData.id = `${toolCallData.name}-${Date.now()}-${Math.random().toString(36).substring(2, 7)}`;
							toolCallEmitter.emit(`toolCall:${sessionId}`, toolCallData);

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
			},

			'POST /api/query': async (req, res) => {
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

						const DEBUG = process.env.DEBUG_CHAT === '1';
						if (DEBUG) {
							console.log(`\n[DEBUG] ===== API Query Request =====`);
							console.log(`[DEBUG] Pattern: "${pattern}"`);
							console.log(`[DEBUG] Path: "${path || 'default'}"`);
							console.log(`[DEBUG] Language: "${language || 'default'}"`);
							console.log(`[DEBUG] Allow tests: ${allow_tests ? 'yes' : 'no'}`);
						}

						try {
							// Get session ID from request if available
							const requestSessionId = JSON.parse(body).sessionId;
							const sessionId = requestSessionId || probeChat.getSessionId();
							if (DEBUG) {
								console.log(`[DEBUG] Using session ID for direct tool call: ${sessionId}`);
							}

							// Execute the query tool
							const result = await queryToolInstance.execute({
								pattern,
								path: path || (probeChat.allowedFolders && probeChat.allowedFolders.length > 0 ? probeChat.allowedFolders[0] : '.'),
								language: language || undefined,
								allow_tests: allow_tests || false
							});

							// Emit tool call event
							const toolCallData = {
								timestamp: new Date().toISOString(),
								name: 'queryCode',
								args: {
									pattern,
									path: path || '.',
									language: language || undefined,
									allow_tests: allow_tests || false
								},
								resultPreview: JSON.stringify(result).substring(0, 200) + '... (truncated)'
							};

							if (DEBUG) {
								console.log(`[DEBUG] Emitting direct tool call event for session ${sessionId}`);
							}
							// Add a unique ID to the tool call data to help with deduplication
							toolCallData.id = `${toolCallData.name}-${Date.now()}-${Math.random().toString(36).substring(2, 7)}`;
							toolCallEmitter.emit(`toolCall:${sessionId}`, toolCallData);

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
			},

			'POST /api/extract': async (req, res) => {
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

						const DEBUG = process.env.DEBUG_CHAT === '1';
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
							// Get session ID from request if available
							const requestSessionId = JSON.parse(body).sessionId;
							const sessionId = requestSessionId || probeChat.getSessionId();
							if (DEBUG) {
								console.log(`[DEBUG] Using session ID for direct tool call: ${sessionId}`);
							}

							// Execute the extract tool
							const result = await extractToolInstance.execute({
								file_path,
								line,
								end_line,
								allow_tests: allow_tests || false,
								context_lines: context_lines || 10,
								format: format || 'plain'
							});

							// Emit tool call event
							const toolCallData = {
								timestamp: new Date().toISOString(),
								name: 'extractCode',
								args: {
									file_path,
									line,
									end_line,
									allow_tests: allow_tests || false,
									context_lines: context_lines || 10,
									format: format || 'plain'
								},
								resultPreview: JSON.stringify(result).substring(0, 200) + '... (truncated)'
							};

							if (DEBUG) {
								console.log(`[DEBUG] Emitting direct tool call event for session ${sessionId}`);
							}
							// Add a unique ID to the tool call data to help with deduplication
							toolCallData.id = `${toolCallData.name}-${Date.now()}-${Math.random().toString(36).substring(2, 7)}`;
							toolCallEmitter.emit(`toolCall:${sessionId}`, toolCallData);

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
			},

			'POST /chat': (req, res) => { // This is the route used by the frontend UI
				let body = '';
				req.on('data', chunk => body += chunk);
				req.on('end', async () => {
					try {
						const requestData = JSON.parse(body);
						const { message, sessionId, apiProvider, apiKey, apiUrl, clearHistory } = requestData; // Added clearHistory

						const DEBUG = process.env.DEBUG_CHAT === '1';
						if (DEBUG) {
							console.log(`\n[DEBUG] ===== Chat Request =====`);
							const safeRequestData = { ...requestData, apiKey: apiKey ? '******' : undefined };
							console.log(`[DEBUG] Full request data:`, safeRequestData);
							console.log(`[DEBUG] User message: "${message}"`);
							console.log(`[DEBUG] Session ID from request: ${sessionId || 'not provided'}`);
							if (apiKey) console.log(`[DEBUG] API key provided for provider: ${apiProvider}`);
							if (clearHistory) console.log(`[DEBUG] Clear history flag set.`);
						}

						// --- Session and Instance Management (REVISED) ---
						const chatSessionId = sessionId || randomUUID(); // Ensure we always have a session ID
						if (DEBUG) console.log(`[DEBUG] Using session ID for chat: ${chatSessionId}`);

						// 1. Try to get the existing instance
						let chatInstance = chatSessions.get(chatSessionId);

						// 2. If no instance exists, create a NEW one
						if (!chatInstance) {
							if (DEBUG) console.log(`[DEBUG] No existing chat instance found for session: ${chatSessionId}, creating new one.`);

							// Temporarily set env vars IF apiKey is provided in the request for instance creation
							const originalEnv = {};
							let restoreEnv = false;
							if (apiKey) {
								restoreEnv = true;
								// Store originals
								originalEnv.ANTHROPIC_API_KEY = process.env.ANTHROPIC_API_KEY;
								originalEnv.OPENAI_API_KEY = process.env.OPENAI_API_KEY;
								originalEnv.GOOGLE_API_KEY = process.env.GOOGLE_API_KEY;
								originalEnv.ANTHROPIC_API_URL = process.env.ANTHROPIC_API_URL;
								originalEnv.OPENAI_API_URL = process.env.OPENAI_API_URL;
								originalEnv.GOOGLE_API_URL = process.env.GOOGLE_API_URL;
								originalEnv.FORCE_PROVIDER = process.env.FORCE_PROVIDER;

								// Clear server keys and set from request
								process.env.ANTHROPIC_API_KEY = ''; process.env.OPENAI_API_KEY = ''; process.env.GOOGLE_API_KEY = '';
								if (apiProvider === 'anthropic') { process.env.ANTHROPIC_API_KEY = apiKey; if (apiUrl) process.env.ANTHROPIC_API_URL = apiUrl; process.env.FORCE_PROVIDER = 'anthropic'; }
								else if (apiProvider === 'openai') { process.env.OPENAI_API_KEY = apiKey; if (apiUrl) process.env.OPENAI_API_URL = apiUrl; process.env.FORCE_PROVIDER = 'openai'; }
								else if (apiProvider === 'google') { process.env.GOOGLE_API_KEY = apiKey; if (apiUrl) process.env.GOOGLE_API_URL = apiUrl; process.env.FORCE_PROVIDER = 'google'; }
								if (DEBUG) console.log(`[DEBUG] Temporarily set API key for provider: ${apiProvider} to create new instance.`);
							} else if (DEBUG) {
								console.log(`[DEBUG] Creating new instance with default/server-configured API key.`);
							}

							// Create the instance
							try {
								chatInstance = new ProbeChat({ sessionId: chatSessionId });
								chatSessions.set(chatSessionId, chatInstance); // Store the new instance
								if (DEBUG) console.log(`[DEBUG] Created and stored new chat instance for session: ${chatSessionId}. Total sessions: ${chatSessions.size}`);
							} catch (error) {
								console.error('Error creating new ProbeChat instance:', error);
								// Restore env vars on error too
								if (restoreEnv) {
									process.env.ANTHROPIC_API_KEY = originalEnv.ANTHROPIC_API_KEY; process.env.OPENAI_API_KEY = originalEnv.OPENAI_API_KEY; process.env.GOOGLE_API_KEY = originalEnv.GOOGLE_API_KEY;
									process.env.ANTHROPIC_API_URL = originalEnv.ANTHROPIC_API_URL; process.env.OPENAI_API_URL = originalEnv.OPENAI_API_URL; process.env.GOOGLE_API_URL = originalEnv.GOOGLE_API_URL;
									process.env.FORCE_PROVIDER = originalEnv.FORCE_PROVIDER;
								}
								if (!res.headersSent) res.writeHead(500, { 'Content-Type': 'text/plain' });
								res.end(`Error: Failed to initialize chat session - ${error.message}`);
								return;
							}

							// Restore original environment variables if they were temporarily changed
							if (restoreEnv) {
								process.env.ANTHROPIC_API_KEY = originalEnv.ANTHROPIC_API_KEY; process.env.OPENAI_API_KEY = originalEnv.OPENAI_API_KEY; process.env.GOOGLE_API_KEY = originalEnv.GOOGLE_API_KEY;
								process.env.ANTHROPIC_API_URL = originalEnv.ANTHROPIC_API_URL; process.env.OPENAI_API_URL = originalEnv.OPENAI_API_URL; process.env.GOOGLE_API_URL = originalEnv.GOOGLE_API_URL;
								process.env.FORCE_PROVIDER = originalEnv.FORCE_PROVIDER;
								if (DEBUG) console.log(`[DEBUG] Restored original environment variables after instance creation.`);
							}
						}
						// 3. If an instance WAS found, USE IT. Do NOT recreate.
						else {
							if (DEBUG) {
								console.log(`[DEBUG] Using existing chat instance for session: ${chatSessionId}. History length: ${chatInstance.history.length}`);
								// Optionally log history summary here if needed
								if (chatInstance.history.length > 0) {
									console.log(`[DEBUG] History summary for session ${chatSessionId}:`);
									chatInstance.history.forEach((msg, idx) => {
										const contentString = (typeof msg.content === 'string') ? msg.content : JSON.stringify(msg.content);
										const preview = contentString.length > 30 ? `${contentString.substring(0, 30)}...` : contentString;
										console.log(`[DEBUG]   ${idx + 1}. ${msg.role}: ${preview}`);
									});
								}
							}
							// NOTE: We explicitly DO NOTHING here regarding API keys from the request.
							// The existing session uses the configuration it was created with.
							// This prioritizes persistent history over per-request API key overrides.
						}
						// --- End Session and Instance Management (REVISED) ---

						// Register this request as active for cancellation
						registerRequest(chatSessionId, {
							abort: () => {
								if (chatInstance && typeof chatInstance.abort === 'function') {
									chatInstance.abort();
								}
								console.log(`Abort triggered for request associated with session: ${chatSessionId}`);
							}
						});
						if (DEBUG) console.log(`[DEBUG] Registered cancellable request for session: ${chatSessionId}`);

						// Set streaming headers
						res.writeHead(200, {
							'Content-Type': 'text/plain',
							'Transfer-Encoding': 'chunked',
							'Cache-Control': 'no-cache',
							'Connection': 'keep-alive'
						});

						// Store the instance reference for potential cancellation during the request
						activeChatInstances.set(chatSessionId, chatInstance);

						try {
							// Handle clear history command
							if (message === '__clear_history__' || clearHistory) { // Check flag from requestData
								console.log(`Clearing chat history for session: ${chatSessionId}`);
								if (chatInstance && typeof chatInstance.clearHistory === 'function') {
									chatInstance.clearHistory();
									res.write('Chat history cleared');
								} else {
									res.write('Error: Could not clear history.');
									console.error(`[ERROR] Failed to clear history for session ${chatSessionId}. Instance: ${!!chatInstance}`);
								}
								// End response and perform cleanup
								if (res.writable && !res.writableEnded) res.end();
								clearRequest(chatSessionId);
								activeChatInstances.delete(chatSessionId);
								clearToolExecutionData(chatSessionId);
								return; // Stop further processing
							}

							// Process the actual chat message using the correct instance
							const responseText = await chatInstance.chat(message, chatSessionId); // Pass session ID for logging consistency

							// Check if cancelled *during* the chat call
							if (isSessionCancelled(chatSessionId)) {
								throw new Error('Request was cancelled by the user');
							}

							// Write the final response if not cancelled
							res.write(responseText);
							console.log('Finished streaming response for UI');

						} catch (error) {
							// Handle errors, including cancellation
							if (error.message && error.message.includes('cancelled')) {
								console.log(`Chat request processing was cancelled for session: ${chatSessionId}`);
								if (res.writable && !res.writableEnded) {
									res.write('\n\n*Request was cancelled by the user.*');
								}
							} else {
								console.error(`Error processing chat for session ${chatSessionId}:`, error);
								if (res.writable && !res.writableEnded) {
									res.write(`\n\n*Error: ${error.message}*`);
								}
							}
						} finally {
							// Cleanup regardless of success, error, or cancellation
							if (res.writable && !res.writableEnded) res.end();
							clearRequest(chatSessionId);
							activeChatInstances.delete(chatSessionId);
							clearToolExecutionData(chatSessionId);
							if (DEBUG) console.log(`[DEBUG] Cleaned up resources post-chat for session: ${chatSessionId}`);
						}

					} catch (error) { // Catch outer errors (JSON parsing, initial setup)
						console.error('Outer error processing chat request:', error);
						let statusCode = 500; errorMessage = 'Internal Server Error';
						if (error instanceof SyntaxError) { statusCode = 400; errorMessage = 'Invalid JSON in request body'; }
						if (!res.headersSent) res.writeHead(statusCode, { 'Content-Type': 'text/plain' });
						if (res.writable && !res.writableEnded) res.end(`${errorMessage}: ${error.message}`);
						else console.error("[ERROR] Cannot send error response, stream already closed.");
					}
				});
			}
		};

		// Route handling logic
		const method = req.method;
		const url = req.url;
		const routeKey = `${method} ${url}`;
		// Check if we have an exact route match
		if (routes[routeKey]) {
			// Skip authentication for public routes
			if (routeKey === 'GET /openapi.yaml' || routeKey === 'GET /api/tool-events') {
				return routes[routeKey](req, res);
			}
			// Apply authentication for protected routes
			return processRequest(routes[routeKey]);
		}
		// Check for partial matches (e.g., /api/chat?param=value should match 'POST /api/chat')
		const baseUrl = url.split('?')[0];
		const baseRouteKey = `${method} ${baseUrl}`;

		if (routes[baseRouteKey]) {
			// Skip authentication for public routes
			if (baseRouteKey === 'GET /openapi.yaml' || baseRouteKey === 'GET /api/tool-events') {
				return routes[baseRouteKey](req, res);
			}
			// Apply authentication for protected routes
			return processRequest(routes[baseRouteKey]);
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

		if (noApiKeysMode) {
			console.log('Running in NO API KEYS MODE - setup instructions will be shown to users');
		} else {
			console.log('Probe tool is available for AI to use');
			console.log(`Session ID: ${probeChat.getSessionId()}`);
		}
	});
}