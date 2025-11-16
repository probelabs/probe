// Tool definitions and XML parsing for the probe agent
import {
  searchTool,
  queryTool,
  extractTool,
  delegateTool,
  bashTool,
  editTool,
  createTool,
  DEFAULT_SYSTEM_MESSAGE,
  attemptCompletionSchema,
  attemptCompletionToolDefinition,
  searchSchema,
  querySchema,
  extractSchema,
  delegateSchema,
  bashSchema,
  editSchema,
  createSchema,
  searchToolDefinition,
  queryToolDefinition,
  extractToolDefinition,
  delegateToolDefinition,
  bashToolDefinition,
  editToolDefinition,
  createToolDefinition,
  parseXmlToolCall
} from '../index.js';
import { randomUUID } from 'crypto';
import { processXmlWithThinkingAndRecovery } from './xmlParsingUtils.js';

// Create configured tool instances
export function createTools(configOptions) {
  const tools = {
    searchTool: searchTool(configOptions),
    queryTool: queryTool(configOptions),
    extractTool: extractTool(configOptions),
    delegateTool: delegateTool(configOptions)
  };

  // Add bash tool if enabled
  if (configOptions.enableBash) {
    tools.bashTool = bashTool(configOptions);
  }

  // Add edit and create tools if enabled
  if (configOptions.allowEdit) {
    tools.editTool = editTool(configOptions);
    tools.createTool = createTool(configOptions);
  }

  return tools;
}

// Export tool definitions and schemas
export {
  DEFAULT_SYSTEM_MESSAGE,
  searchSchema,
  querySchema,
  extractSchema,
  delegateSchema,
  bashSchema,
  editSchema,
  createSchema,
  attemptCompletionSchema,
  searchToolDefinition,
  queryToolDefinition,
  extractToolDefinition,
  delegateToolDefinition,
  bashToolDefinition,
  editToolDefinition,
  createToolDefinition,
  attemptCompletionToolDefinition,
  parseXmlToolCall
};

// Define the implement tool XML definition
export const implementToolDefinition = `
## implement
Description: Implement a given task. Can modify files. Can be used ONLY if task explicitly stated that something requires modification or implementation.

Parameters:
- task: (required) The task description. Should be as detailed as possible, ideally pointing to exact files which needs be modified or created.
- autoCommits: (optional) Whether to enable auto-commits in aider. Default is false.

Usage Example:

<examples>

User: Can you implement a function to calculate Fibonacci numbers in main.js?
<implement>
<task>Implement a recursive function to calculate the nth Fibonacci number in main.js</task>
</implement>

User: Can you implement a function to calculate Fibonacci numbers in main.js with auto-commits?
<implement>
<task>Implement a recursive function to calculate the nth Fibonacci number in main.js</task>
<autoCommits>true</autoCommits>
</implement>

</examples>
`;

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
  const { cleanedXmlString, recoveryResult } = processXmlWithThinkingAndRecovery(xmlString, validTools);
  
  // If recovery found an attempt_complete pattern, return it
  if (recoveryResult) {
    return recoveryResult;
  }

  // Otherwise, use the original parseXmlToolCall function to parse the cleaned XML string
  return parseXmlToolCall(cleanedXmlString, validTools);
}

