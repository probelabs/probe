/**
 * Main tools module
 * @module tools
 */

// Export Vercel AI SDK tool generators
export { searchTool, queryTool, extractTool, delegateTool } from './vercel.js';
export { bashTool } from './bash.js';
export { editTool, createTool, multiEditTool } from './edit.js';

// Export LangChain tools
export { createSearchTool, createQueryTool, createExtractTool } from './langchain.js';

// Export execute_plan and cleanup_execute_plan tools
export { createExecutePlanTool, createCleanupExecutePlanTool } from './executePlan.js';

// Export common schemas and utilities
export {
	searchSchema,
	querySchema,
	extractSchema,
	delegateSchema,
	bashSchema,
	executePlanSchema,
	cleanupExecutePlanSchema,
	delegateDescription,
	bashDescription,
	attemptCompletionSchema,
	parseAndResolvePaths,
	resolveTargetPath,
	listFilesSchema,
	searchFilesSchema,
	readImageSchema,
	listSkillsSchema,
	useSkillSchema
} from './common.js';

// Export edit and create schemas
export {
	editSchema,
	createSchema,
	multiEditSchema,
	editDescription,
	createDescription,
	multiEditDescription
} from './edit.js';

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
