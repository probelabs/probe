// Import tool generators from @buger/probe package
import { searchTool, queryTool, extractTool, DEFAULT_SYSTEM_MESSAGE, listFilesByLevel } from '@buger/probe';
import { randomUUID } from 'crypto';
import { EventEmitter } from 'events';

// Create an event emitter for tool calls
export const toolCallEmitter = new EventEmitter();

// Map to track active tool executions by session ID
const activeToolExecutions = new Map();

// Function to check if a session has been cancelled
export function isSessionCancelled(sessionId) {
	return activeToolExecutions.get(sessionId)?.cancelled || false;
}

// Function to cancel all tool executions for a session
export function cancelToolExecutions(sessionId) {
	// Only log if not in non-interactive mode or if in debug mode
	if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
		console.log(`Cancelling tool executions for session: ${sessionId}`);
	}
	const sessionData = activeToolExecutions.get(sessionId);
	if (sessionData) {
		sessionData.cancelled = true;
		// Only log if not in non-interactive mode or if in debug mode
		if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
			console.log(`Session ${sessionId} marked as cancelled`);
		}
		return true;
	}
	return false;
}

// Function to register a new tool execution
function registerToolExecution(sessionId) {
	if (!sessionId) return;

	if (!activeToolExecutions.has(sessionId)) {
		activeToolExecutions.set(sessionId, { cancelled: false });
	} else {
		// Reset cancelled flag if session already exists for a new execution
		activeToolExecutions.get(sessionId).cancelled = false;
	}
}

// Function to clear tool execution data for a session
export function clearToolExecutionData(sessionId) {
	if (!sessionId) return;

	if (activeToolExecutions.has(sessionId)) {
		activeToolExecutions.delete(sessionId);
		// Only log if not in non-interactive mode or if in debug mode
		if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
			console.log(`Cleared tool execution data for session: ${sessionId}`);
		}
	}
}

// Generate a default session ID (less relevant now, session is managed per-chat)
const defaultSessionId = randomUUID();
// Only log session ID in debug mode
if (process.env.DEBUG_CHAT === '1') {
	console.log(`Generated default session ID (probeTool.js): ${defaultSessionId}`);
}

// Create configured tools with the session ID
// Note: These configOptions are less critical now as sessionId is passed explicitly
const configOptions = {
	sessionId: defaultSessionId,
	debug: process.env.DEBUG_CHAT === '1'
};

// Create the base tools using the imported generators
const baseSearchTool = searchTool(configOptions);
const baseQueryTool = queryTool(configOptions);
const baseExtractTool = extractTool(configOptions);


// Wrap the tools to emit events and handle cancellation
const wrapToolWithEmitter = (tool, toolName, baseExecute) => {
	return {
		...tool, // Spread schema, description etc.
		execute: async (params) => { // The execute function now receives parsed params
			const debug = process.env.DEBUG_CHAT === '1';
			// Get the session ID from params (passed down from probeChat.js)
			const toolSessionId = params.sessionId || defaultSessionId; // Fallback, but should always have sessionId

			if (debug) {
				console.log(`[DEBUG] probeTool: Executing ${toolName} for session ${toolSessionId}`);
				console.log(`[DEBUG] probeTool: Received params:`, params);
			}

			// Register this tool execution (and reset cancel flag if needed)
			registerToolExecution(toolSessionId);

			// Check if this session has been cancelled *before* execution
			if (isSessionCancelled(toolSessionId)) {
				// Only log if not in non-interactive mode or if in debug mode
				console.error(`Tool execution cancelled BEFORE starting for session ${toolSessionId}`);
				throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
			}
			// Only log if not in non-interactive mode or if in debug mode
			console.error(`Executing ${toolName} for session ${toolSessionId}`); // Simplified log

			// Remove sessionId from params before passing to base tool if it expects only schema params
			const { sessionId, ...toolParams } = params;

			try {
				// Emit a tool call start event
				const toolCallStartData = {
					timestamp: new Date().toISOString(),
					name: toolName,
					args: toolParams, // Log schema params
					status: 'started'
				};
				if (debug) {
					console.log(`[DEBUG] probeTool: Emitting toolCallStart:${toolSessionId}`);
				}
				toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallStartData);

				// Execute the original tool's execute function with schema params
				// Use a promise-based approach with cancellation check
				let result = null;
				let executionError = null;

				const executionPromise = baseExecute(toolParams).catch(err => {
					executionError = err; // Capture error
				});

				const checkInterval = 50; // Check every 50ms
				while (result === null && executionError === null) {
					if (isSessionCancelled(toolSessionId)) {
						console.error(`Tool execution cancelled DURING execution for session ${toolSessionId}`);
						// Attempt to signal cancellation if the underlying tool supports it (future enhancement)
						// For now, just throw the cancellation error
						throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
					}
					// Check if promise is resolved or rejected
					const status = await Promise.race([
						executionPromise.then(() => 'resolved').catch(() => 'rejected'),
						new Promise(resolve => setTimeout(() => resolve('pending'), checkInterval))
					]);

					if (status === 'resolved') {
						result = await executionPromise; // Get the result
					} else if (status === 'rejected') {
						// Error already captured by the catch block on executionPromise
						break;
					}
					// If 'pending', continue loop
				}

				// If loop exited due to error
				if (executionError) {
					throw executionError;
				}

				// If loop exited due to cancellation within the loop
				if (isSessionCancelled(toolSessionId)) {
					// Only log if not in non-interactive mode or if in debug mode
					if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
						console.log(`Tool execution finished but session was cancelled for ${toolSessionId}`);
					}
					throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
				}


				// Emit the tool call completion event
				const toolCallData = {
					timestamp: new Date().toISOString(),
					name: toolName,
					args: toolParams,
					// Safely preview result
					resultPreview: typeof result === 'string'
						? (result.length > 200 ? result.substring(0, 200) + '...' : result)
						: (result ? JSON.stringify(result).substring(0, 200) + '...' : 'No Result'),
					status: 'completed'
				};
				if (debug) {
					console.log(`[DEBUG] probeTool: Emitting toolCall:${toolSessionId} (completed)`);
				}
				toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallData);

				return result;
			} catch (error) {
				// If it's a cancellation error, re-throw it directly
				if (error.message.includes('cancelled for session')) {
					// Only log if not in non-interactive mode or if in debug mode
					if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
						console.log(`Caught cancellation error for ${toolName} in session ${toolSessionId}`);
					}
					// Emit cancellation event? Or let the caller handle it? Let caller handle.
					throw error;
				}

				// Handle other execution errors
				if (debug) {
					console.error(`[DEBUG] probeTool: Error executing ${toolName}:`, error);
				}

				// Emit a tool call error event
				const toolCallErrorData = {
					timestamp: new Date().toISOString(),
					name: toolName,
					args: toolParams,
					error: error.message || 'Unknown error',
					status: 'error'
				};
				if (debug) {
					console.log(`[DEBUG] probeTool: Emitting toolCall:${toolSessionId} (error)`);
				}
				toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallErrorData);

				throw error; // Re-throw the error to be caught by probeChat.js loop
			}
		}
	};
};

// Export the wrapped tool instances
export const searchToolInstance = wrapToolWithEmitter(baseSearchTool, 'search', baseSearchTool.execute);
export const queryToolInstance = wrapToolWithEmitter(baseQueryTool, 'query', baseQueryTool.execute);
export const extractToolInstance = wrapToolWithEmitter(baseExtractTool, 'extract', baseExtractTool.execute);

// --- Backward Compatibility Layer (probeTool mapping to searchToolInstance) ---
// This might be less relevant if the AI is strictly using the new XML format,
// but keep it for potential direct API calls or older UI elements.
export const probeTool = {
	...searchToolInstance, // Inherit schema description etc. from the wrapped search tool
	name: "search", // Explicitly set name
	description: 'DEPRECATED: Use <search> tool instead. Search code using keywords.',
	// parameters: searchSchema, // Use the imported schema
	execute: async (params) => { // Expects { keywords, folder, ..., sessionId }
		const debug = process.env.DEBUG_CHAT === '1';
		if (debug) {
			console.log(`[DEBUG] probeTool (Compatibility Layer) executing for session ${params.sessionId}`);
		}

		// Map old params ('keywords', 'folder') to new ones ('query', 'path')
		const { keywords, folder, sessionId, ...rest } = params;
		const mappedParams = {
			query: keywords,
			path: folder || '.', // Default path if folder is missing
			sessionId: sessionId, // Pass session ID through
			...rest // Pass other params like allow_tests, maxResults etc.
		};

		if (debug) {
			console.log("[DEBUG] probeTool mapped params: ", mappedParams);
		}

		// Call the *wrapped* searchToolInstance execute function
		// It will handle cancellation checks and event emitting internally
		try {
			// Note: The name emitted by searchToolInstance will be 'search', not 'probeTool' or 'searchCode'
			const result = await searchToolInstance.execute(mappedParams);

			// Format the result for backward compatibility if needed by caller
			// The raw result from searchToolInstance is likely just the search results array/string
			const formattedResult = {
				results: result, // Assuming result is the direct data
				command: `probe search --query "${keywords}" --path "${folder || '.'}"`, // Reconstruct approx command
				timestamp: new Date().toISOString()
			};
			if (debug) {
				console.log("[DEBUG] probeTool compatibility layer returning formatted result.");
			}
			return formattedResult;

		} catch (error) {
			if (debug) {
				console.error(`[DEBUG] Error in probeTool compatibility layer:`, error);
			}
			// Error is already emitted by the wrapped searchToolInstance, just re-throw
			throw error;
		}
	}
};

// Export necessary items
export { DEFAULT_SYSTEM_MESSAGE, listFilesByLevel };
// Export the tool generator functions if needed elsewhere
export { searchTool, queryTool, extractTool };