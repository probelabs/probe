// Tool definitions and XML parsing for the probe agent
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
  attemptCompletionSchema,
  attemptCompletionToolDefinition,
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
  searchToolDefinition,
  queryToolDefinition,
  extractToolDefinition,
  delegateToolDefinition,
  analyzeAllToolDefinition,
  getExecutePlanToolDefinition,
  getCleanupExecutePlanToolDefinition,
  bashToolDefinition,
  editToolDefinition,
  createToolDefinition,
  multiEditToolDefinition,
  googleSearchToolDefinition,
  urlContextToolDefinition,
  parseXmlToolCall
} from '../index.js';
import { randomUUID } from 'crypto';
import { processXmlWithThinkingAndRecovery } from './xmlParsingUtils.js';

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

// Export tool definitions and schemas
// Export task tool from tasks module
export {
  taskSchema,
  taskToolDefinition,
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
  attemptCompletionSchema,
  searchToolDefinition,
  queryToolDefinition,
  extractToolDefinition,
  delegateToolDefinition,
  analyzeAllToolDefinition,
  getExecutePlanToolDefinition,
  getCleanupExecutePlanToolDefinition,
  bashToolDefinition,
  editToolDefinition,
  createToolDefinition,
  multiEditToolDefinition,
  attemptCompletionToolDefinition,
  googleSearchToolDefinition,
  urlContextToolDefinition,
  parseXmlToolCall
};

// Define the listFiles tool XML definition
export const listFilesToolDefinition = `
## listFiles
Description: List files and directories in a specified location.

Parameters:
- directory: (optional) The directory path to list files from. Defaults to current directory if not specified.

Usage Example:

<examples>

User: Can you list the files in the src directory?
<listFiles>
<directory>src</directory>
</listFiles>

User: What files are in the current directory?
<listFiles>
</listFiles>

</examples>
`;

// Define the searchFiles tool XML definition
export const searchFilesToolDefinition = `
## searchFiles
Description: Find files with name matching a glob pattern with recursive search capability.

Parameters:
- pattern: (required) The glob pattern to search for (e.g., "**/*.js", "*.md").
- directory: (optional) The directory to search in. Defaults to current directory if not specified.
- recursive: (optional) Whether to search recursively. Defaults to true.

Usage Example:

<examples>

User: Can you find all JavaScript files in the project?
<searchFiles>
<pattern>**/*.js</pattern>
</searchFiles>

User: Find all markdown files in the docs directory, but only at the top level.
<searchFiles>
<pattern>*.md</pattern>
<directory>docs</directory>
<recursive>false</recursive>
</searchFiles>

</examples>
`;

// Define the listSkills tool XML definition
export const listSkillsToolDefinition = `
## listSkills
Description: List available agent skills discovered in the repository.

Parameters:
- filter: (optional) Substring filter to match skill names or descriptions.

Usage Example:

<examples>

User: What skills are available?
<listSkills>
</listSkills>

User: Show me skills related to docs
<listSkills>
<filter>docs</filter>
</listSkills>

</examples>
`;

// Define the useSkill tool XML definition
export const useSkillToolDefinition = `
## useSkill
Description: Load and activate a specific skill's instructions. Use this before following a skill's guidance.

Parameters:
- name: (required) The skill name to activate.

Usage Example:

<examples>

User: Use the onboarding skill
<useSkill>
<name>onboarding</name>
</useSkill>

</examples>
`;

// Define the readImage tool XML definition
export const readImageToolDefinition = `
## readImage
Description: Read and load an image file so it can be viewed by the AI. Use this when you need to analyze, describe, or work with image content. Images from user messages are automatically loaded, but use this tool to explicitly read images mentioned in tool outputs or when you need to examine specific image files.

Parameters:
- path: (required) The path to the image file to read. Supports png, jpg, jpeg, webp, bmp, and svg formats.

Usage Example:

<examples>

User: Can you describe what's in screenshot.png?
<readImage>
<path>screenshot.png</path>
</readImage>

User: Analyze the diagram in docs/architecture.svg
<readImage>
<path>docs/architecture.svg</path>
</readImage>

</examples>
`;

/**
 * Enhanced XML parser that handles thinking tags and attempt_complete shorthand
 * This function removes any <thinking></thinking> tags from the input string
 * before passing it to the original parseXmlToolCall function
 * @param {string} xmlString - The XML string to parse
 * @param {string[]} [validTools] - List of valid tool names to parse (optional)
 * @returns {Object|null} - The parsed tool call or null if no valid tool call found
 */
export function parseXmlToolCallWithThinking(xmlString, validTools) {
  // Use the shared processing logic
  const { cleanedXmlString, recoveryResult, thinkingContent } = processXmlWithThinkingAndRecovery(xmlString, validTools);

  // If recovery found an attempt_complete pattern, return it with thinking content
  if (recoveryResult) {
    return { ...recoveryResult, thinkingContent };
  }

  // Otherwise, use the original parseXmlToolCall function to parse the cleaned XML string
  const toolCall = parseXmlToolCall(cleanedXmlString, validTools);

  // Return tool call with thinking content attached
  return toolCall ? { ...toolCall, thinkingContent } : null;
}
