// Import tool generators from @buger/probe package
import { searchTool, queryTool, extractTool, DEFAULT_SYSTEM_MESSAGE } from '@buger/probe';
import { randomUUID } from 'crypto';

// Generate a session ID
const sessionId = randomUUID();
console.log(`Generated session ID for search caching: ${sessionId}`);

// Create configured tools with the session ID
const configOptions = {
	sessionId,
	debug: process.env.DEBUG === 'true' || process.env.DEBUG === '1'
};

// Export the configured tools
export const tools = {
	searchTool: searchTool(configOptions),
	queryTool: queryTool(configOptions),
	extractTool: extractTool(configOptions)
};

// Export individual tools for direct use
export { DEFAULT_SYSTEM_MESSAGE };
export const { searchTool: searchToolInstance, queryTool: queryToolInstance, extractTool: extractToolInstance } = tools;

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
		// Map the old parameter names to the new ones
		const { keywords, folder, ...rest } = params;
		const result = await searchToolInstance.execute({
			query: keywords,
			path: folder || '.',  // Default to current directory if folder is not specified
			...rest
		});

		// Format the result to match the old format
		return {
			results: result,
			command: `probe ${keywords} ${folder || '.'}`,
			timestamp: new Date().toISOString()
		};
	}
};