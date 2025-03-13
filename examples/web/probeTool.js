// Import tools from @buger/probe package
import { tools } from '@buger/probe';

// Export the tools for use in the application
export const { searchTool, queryTool, extractTool } = tools;

// Export the default system message
export const DEFAULT_SYSTEM_MESSAGE = tools.DEFAULT_SYSTEM_MESSAGE;

// For backward compatibility, export the probeTool that maps to searchTool
export const probeTool = {
	...searchTool,
	parameters: {
		...searchTool.parameters,
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
		const result = await searchTool.execute({
			query: keywords,
			path: folder,
			...rest
		});
		
		// Format the result to match the old format
		return {
			results: result,
			command: `probe ${keywords} ${folder || ''}`,
			timestamp: new Date().toISOString()
		};
	}
};