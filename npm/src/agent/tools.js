// Tool creation and schema exports for the probe agent
import {
  searchTool,
  queryTool,
  extractTool,
  delegateTool,
  analyzeAllTool,
  createExecutePlanTool,
  createCleanupExecutePlanTool,
  bashTool,
  editTool,
  createTool,
  multiEditTool,
  DEFAULT_SYSTEM_MESSAGE,
  searchSchema,
  querySchema,
  extractSchema,
  delegateSchema,
  analyzeAllSchema,
  executePlanSchema,
  cleanupExecutePlanSchema,
  bashSchema,
  editSchema,
  createSchema,
  multiEditSchema,
  listFilesSchema,
  searchFilesSchema,
  readImageSchema,
  listSkillsSchema,
  useSkillSchema
} from '../index.js';

// Create configured tool instances
export function createTools(configOptions) {
  const tools = {};

  const isToolAllowed =
    configOptions.isToolAllowed ||
    ((toolName) => {
      if (!configOptions.allowedTools) return true;
      return configOptions.allowedTools.isEnabled(toolName);
    });

  // Core tools
  if (isToolAllowed('search')) {
    tools.searchTool = searchTool(configOptions);
  }
  if (isToolAllowed('query')) {
    tools.queryTool = queryTool(configOptions);
  }
  if (isToolAllowed('extract')) {
    tools.extractTool = extractTool(configOptions);
  }
  if (configOptions.enableDelegate && isToolAllowed('delegate')) {
    tools.delegateTool = delegateTool(configOptions);
  }
  if (configOptions.enableExecutePlan && isToolAllowed('execute_plan')) {
    tools.executePlanTool = createExecutePlanTool(configOptions);
    // cleanup_execute_plan is enabled together with execute_plan
    if (isToolAllowed('cleanup_execute_plan')) {
      tools.cleanupExecutePlanTool = createCleanupExecutePlanTool(configOptions);
    }
  } else if (isToolAllowed('analyze_all')) {
    // analyze_all is fallback when execute_plan is not enabled
    tools.analyzeAllTool = analyzeAllTool(configOptions);
  }

  // Add bash tool if enabled
  if (configOptions.enableBash && isToolAllowed('bash')) {
    tools.bashTool = bashTool(configOptions);
  }

  // Add edit and create tools if enabled
  if (configOptions.allowEdit && isToolAllowed('edit')) {
    tools.editTool = editTool(configOptions);
  }
  if (configOptions.allowEdit && isToolAllowed('create')) {
    tools.createTool = createTool(configOptions);
  }
  if (configOptions.allowEdit && isToolAllowed('multi_edit')) {
    tools.multiEditTool = multiEditTool(configOptions);
  }
  return tools;
}

// Export task tool from tasks module
export {
  taskSchema,
  taskSystemPrompt,
  taskGuidancePrompt,
  createTaskCompletionBlockedMessage,
  createTaskTool,
  TaskManager
} from './tasks/index.js';

export {
  DEFAULT_SYSTEM_MESSAGE,
  searchSchema,
  querySchema,
  extractSchema,
  delegateSchema,
  analyzeAllSchema,
  executePlanSchema,
  cleanupExecutePlanSchema,
  bashSchema,
  editSchema,
  createSchema,
  multiEditSchema,
  listFilesSchema,
  searchFilesSchema,
  readImageSchema,
  listSkillsSchema,
  useSkillSchema
};
