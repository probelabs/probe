/**
 * Main tools module
 * @module tools
 */

// Export Vercel AI SDK tool generators
export { searchTool, queryTool, extractTool, delegateTool } from './vercel.js';
export { bashTool } from './bash.js';

// Export LangChain tools
export { createSearchTool, createQueryTool, createExtractTool } from './langchain.js';

// Export common schemas
export {
	searchSchema,
	querySchema,
	extractSchema,
	delegateSchema,
	bashSchema,
	delegateDescription,
	delegateToolDefinition,
	bashDescription,
	bashToolDefinition,
	attemptCompletionSchema,
	attemptCompletionToolDefinition
} from './common.js';

// Export system message
export { DEFAULT_SYSTEM_MESSAGE } from './system-message.js';

// For backward compatibility, create and export pre-configured tools
import { searchTool as searchToolGenerator, queryTool as queryToolGenerator, extractTool as extractToolGenerator, delegateTool as delegateToolGenerator } from './vercel.js';
import { bashTool as bashToolGenerator } from './bash.js';
import { DEFAULT_SYSTEM_MESSAGE } from './system-message.js';

// Create default tool instances (for backward compatibility)
const tools = {
	searchTool: searchToolGenerator(),
	queryTool: queryToolGenerator(),
	extractTool: extractToolGenerator(),
	delegateTool: delegateToolGenerator(),
	bashTool: bashToolGenerator(),
	DEFAULT_SYSTEM_MESSAGE
};

export { tools };