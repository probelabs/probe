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
	console.log(`Cancelling tool executions for session: ${sessionId}`);
	const sessionData = activeToolExecutions.get(sessionId);
	if (sessionData) {
		sessionData.cancelled = true;
		console.log(`Session ${sessionId} marked as cancelled`);
		return true;
	}
	return false;
}

// Function to register a new tool execution
function registerToolExecution(sessionId) {
	if (!sessionId) return;

	if (!activeToolExecutions.has(sessionId)) {
		activeToolExecutions.set(sessionId, { cancelled: false });
	}
}

// Function to clear tool execution data for a session
export function clearToolExecutionData(sessionId) {
	if (!sessionId) return;

	if (activeToolExecutions.has(sessionId)) {
		activeToolExecutions.delete(sessionId);
		console.log(`Cleared tool execution data for session: ${sessionId}`);
	}
}

// Generate a session ID
const sessionId = randomUUID();
// Only log session ID in debug mode
if (process.env.DEBUG_CHAT === '1') {
	console.log(`Generated session ID for search caching: ${sessionId}`);
}

// Create configured tools with the session ID
const configOptions = {
	sessionId,
	debug: process.env.DEBUG_CHAT === '1'
};

// Create the base tools
const baseTools = {
	searchTool: searchTool(configOptions),
	queryTool: queryTool(configOptions),
	extractTool: extractTool(configOptions)
};

// Wrap the tools to emit events when they're called
const wrapToolWithEmitter = (tool, toolName) => {
	return {
		...tool,
		execute: async (params) => {
			const debug = process.env.DEBUG_CHAT === '1';
			if (debug) {
				console.log(`[DEBUG] Executing ${toolName} with params:`, params);
			}

			// Get the session ID from params or use the default
			const toolSessionId = params.sessionId || sessionId;
			if (debug) {
				console.log(`[DEBUG] Using session ID for tool call: ${toolSessionId}`);
			}

			// Register this tool execution
			registerToolExecution(toolSessionId);

			// Check if this session has been cancelled
			if (isSessionCancelled(toolSessionId)) {
				console.log(`Tool execution cancelled for session ${toolSessionId}`);
				throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
			}

			console.log(`Executing ${toolName} with params:`, params);

			try {
				// Emit a tool call start event
				const toolCallStartData = {
					timestamp: new Date().toISOString(),
					name: toolName,
					args: params,
					status: 'started'
				};
				if (debug) {
					console.log(`[DEBUG] Emitting tool call start event for session ${toolSessionId}`);
				}
				toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallStartData);

				// Execute the original tool with periodic cancellation checks
				const executionPromise = tool.execute(params);

				// Create a polling mechanism to check for cancellation
				const cancellationCheckInterval = setInterval(() => {
					if (isSessionCancelled(toolSessionId)) {
						clearInterval(cancellationCheckInterval);
						console.log(`Detected cancellation during tool execution for session ${toolSessionId}`);
						// We can't actually cancel the tool execution once it's started,
						// but we can mark it as cancelled so we don't process the result
					}
				}, 100);

				// Wait for the execution to complete
				const result = await executionPromise;

				// Clear the cancellation check interval
				clearInterval(cancellationCheckInterval);

				// Check again if the session was cancelled during execution
				if (isSessionCancelled(toolSessionId)) {
					console.log(`Tool execution was cancelled for session ${toolSessionId}`);
					throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
				}

				// Emit the tool call event
				const toolCallData = {
					timestamp: new Date().toISOString(),
					name: toolName,
					args: params,
					resultPreview: typeof result === 'string'
						? (result.length > 200 ? result.substring(0, 200) + '... (truncated)' : result)
						: JSON.stringify(result, null, 2).substring(0, 200) + '... (truncated)',
					status: 'completed'
				};
				if (debug) {
					console.log(`[DEBUG] Emitting tool call event for session ${toolSessionId}:`, toolCallData);
				}
				toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallData);

				return result;
			} catch (error) {
				if (debug) {
					console.error(`[DEBUG] Error executing ${toolName}:`, error);
				}

				// Emit a tool call error event
				const toolCallErrorData = {
					timestamp: new Date().toISOString(),
					name: toolName,
					args: params,
					error: error.message,
					status: 'error'
				};
				toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallErrorData);

				throw error;
			}
		}
	};
};

// Export the wrapped tools
export const tools = {
	searchTool: wrapToolWithEmitter(baseTools.searchTool, 'search'),
	queryTool: wrapToolWithEmitter(baseTools.queryTool, 'query'),
	extractTool: wrapToolWithEmitter(baseTools.extractTool, 'extract')
};

// Export individual tools for direct use
export { DEFAULT_SYSTEM_MESSAGE };
export const { searchTool: searchToolInstance, queryTool: queryToolInstance, extractTool: extractToolInstance } = tools;
// Export the tool generators for direct use
export { searchTool, queryTool, extractTool, listFilesByLevel };

// For backward compatibility, export the probeTool that maps to searchTool
export const probeTool = {
	...searchToolInstance,
	parameters: {
		...searchToolInstance.parameters,
		// Map the old parameter names to the new ones
		parse: (params) => {
			const { keywords, folder, ...rest } = params;
			return {
				query: keywords,
				path: folder,
				...rest
			};
		}
	},
	execute: async (params) => {
		const debug = process.env.DEBUG_CHAT === '1';
		if (debug) {
			console.log(`[DEBUG] Executing probeTool with params:`, params);
		}

		// Get the session ID from params or use the default
		const toolSessionId = params.sessionId || sessionId;
		if (debug) {
			console.log(`[DEBUG] Using session ID for probeTool call: ${toolSessionId}`);
		}

		// Register this tool execution
		registerToolExecution(toolSessionId);

		// Check if this session has been cancelled
		if (isSessionCancelled(toolSessionId)) {
			console.log(`Tool execution cancelled for session ${toolSessionId}`);
			throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
		}

		try {
			// Emit a tool call start event
			const toolCallStartData = {
				timestamp: new Date().toISOString(),
				name: 'searchCode',
				args: params,
				status: 'started'
			};
			if (debug) {
				console.log(`[DEBUG] Emitting probeTool call start event for session ${toolSessionId}`);
			}
			toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallStartData);

			// Map the old parameter names to the new ones
			const { keywords, folder, ...rest } = params;

			// Create a polling mechanism to check for cancellation
			const cancellationCheckInterval = setInterval(() => {
				if (isSessionCancelled(toolSessionId)) {
					clearInterval(cancellationCheckInterval);
					console.log(`Detected cancellation during probeTool execution for session ${toolSessionId}`);
				}
			}, 100);

			// Execute the search
			const result = await searchToolInstance.execute({
				query: keywords,
				path: folder || '.',  // Default to current directory if folder is not specified
				...rest
			});

			// Clear the cancellation check interval
			clearInterval(cancellationCheckInterval);

			// Check again if the session was cancelled during execution
			if (isSessionCancelled(toolSessionId)) {
				console.log(`ProbeTool execution was cancelled for session ${toolSessionId}`);
				throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
			}

			// Format the result to match the old format
			const formattedResult = {
				results: result,
				command: `probe ${keywords} ${folder || '.'}`,
				timestamp: new Date().toISOString()
			};

			// Emit the tool call event
			const toolCallData = {
				timestamp: new Date().toISOString(),
				name: 'searchCode',
				args: params,
				resultPreview: JSON.stringify(formattedResult).substring(0, 200) + '... (truncated)',
				status: 'completed'
			};
			if (debug) {
				console.log(`[DEBUG] Emitting probeTool call event for session ${toolSessionId}:`, toolCallData);
			}
			toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallData);

			return formattedResult;
		} catch (error) {
			if (debug) {
				console.error(`[DEBUG] Error executing probeTool:`, error);
			}

			// Emit a tool call error event
			const toolCallErrorData = {
				timestamp: new Date().toISOString(),
				name: 'searchCode',
				args: params,
				error: error.message,
				status: 'error'
			};
			toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallErrorData);

			throw error;
		}
	}
};