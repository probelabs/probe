// Import tool generators from @buger/probe package
import { searchTool, queryTool, extractTool, DEFAULT_SYSTEM_MESSAGE } from '@buger/probe';
import { randomUUID } from 'crypto';
import { EventEmitter } from 'events';

// Create an event emitter for tool calls
export const toolCallEmitter = new EventEmitter();

// Generate a session ID
const sessionId = randomUUID();
// Only log session ID in debug mode
if (process.env.DEBUG === '1') {
	console.log(`Generated session ID for search caching: ${sessionId}`);
}

// Create configured tools with the session ID
const configOptions = {
	sessionId,
	debug: process.env.DEBUG === '1'
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
			const debug = process.env.DEBUG === '1';
			if (debug) {
				console.log(`[DEBUG] Executing ${toolName} with params:`, params);
			}

			// Get the session ID from params or use the default
			const toolSessionId = params.sessionId || sessionId;
			if (debug) {
				console.log(`[DEBUG] Using session ID for tool call: ${toolSessionId}`);
			}

			try {
				// Execute the original tool
				const result = await tool.execute(params);

				// Emit the tool call event
				const toolCallData = {
					timestamp: new Date().toISOString(),
					name: toolName,
					args: params,
					resultPreview: typeof result === 'string'
						? (result.length > 200 ? result.substring(0, 200) + '... (truncated)' : result)
						: JSON.stringify(result, null, 2).substring(0, 200) + '... (truncated)'
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
				throw error;
			}
		}
	};
};

// Export the wrapped tools
export const tools = {
	searchTool: wrapToolWithEmitter(baseTools.searchTool, 'searchCode'),
	queryTool: wrapToolWithEmitter(baseTools.queryTool, 'queryCode'),
	extractTool: wrapToolWithEmitter(baseTools.extractTool, 'extractCode')
};

// Export individual tools for direct use
export { DEFAULT_SYSTEM_MESSAGE };
export const { searchTool: searchToolInstance, queryTool: queryToolInstance, extractTool: extractToolInstance } = tools;

// Export the tool generators for direct use
export { searchTool, queryTool, extractTool };

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
		const debug = process.env.DEBUG === '1';
		if (debug) {
			console.log(`[DEBUG] Executing probeTool with params:`, params);
		}

		// Get the session ID from params or use the default
		const toolSessionId = params.sessionId || sessionId;
		if (debug) {
			console.log(`[DEBUG] Using session ID for probeTool call: ${toolSessionId}`);
		}

		try {
			// Map the old parameter names to the new ones
			const { keywords, folder, ...rest } = params;
			const result = await searchToolInstance.execute({
				query: keywords,
				path: folder || '.',  // Default to current directory if folder is not specified
				...rest
			});

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
				resultPreview: JSON.stringify(formattedResult).substring(0, 200) + '... (truncated)'
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
			throw error;
		}
	}
};