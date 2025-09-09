// Tool definitions and XML parsing for the probe agent
import {
  searchTool,
  queryTool,
  extractTool,
  DEFAULT_SYSTEM_MESSAGE,
  attemptCompletionSchema,
  attemptCompletionToolDefinition,
  searchSchema,
  querySchema,
  extractSchema,
  searchToolDefinition,
  queryToolDefinition,
  extractToolDefinition,
  parseXmlToolCall
} from '../index.js';
import { randomUUID } from 'crypto';

// Create configured tool instances
export function createTools(configOptions) {
  return {
    searchTool: searchTool(configOptions),
    queryTool: queryTool(configOptions),
    extractTool: extractTool(configOptions)
  };
}

// Export tool definitions and schemas
export {
  DEFAULT_SYSTEM_MESSAGE,
  searchSchema,
  querySchema,
  extractSchema,
  attemptCompletionSchema,
  searchToolDefinition,
  queryToolDefinition,
  extractToolDefinition,
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

/**
 * Enhanced XML parser that handles thinking tags
 * This function removes any <thinking></thinking> tags from the input string
 * before passing it to the original parseXmlToolCall function
 * @param {string} xmlString - The XML string to parse
 * @returns {Object|null} - The parsed tool call or null if no valid tool call found
 */
export function parseXmlToolCallWithThinking(xmlString) {
  // Extract thinking content if present (for potential logging or analysis)
  const thinkingMatch = xmlString.match(/<thinking>([\s\S]*?)<\/thinking>/);
  const thinkingContent = thinkingMatch ? thinkingMatch[1].trim() : null;

  // Remove thinking tags and their content from the XML string
  const cleanedXmlString = xmlString.replace(/<thinking>[\s\S]*?<\/thinking>/g, '').trim();

  // Use the original parseXmlToolCall function to parse the cleaned XML string
  const parsedTool = parseXmlToolCall(cleanedXmlString);

  // If debugging is enabled, log the thinking content
  if (process.env.DEBUG === '1' && thinkingContent) {
    console.log(`[DEBUG] AI Thinking Process:\n${thinkingContent}`);
  }

  return parsedTool;
}