// This file might become obsolete or significantly simplified if tools
// are only defined/described in the system prompt and not passed to Vercel AI SDK.

// Let's comment it out for now, assuming it's not directly used in the new flow.
// If specific exports are needed elsewhere (like DEFAULT_SYSTEM_MESSAGE), they
// should be moved or imported directly from @buger/probe.


// Import tool generators and schemas from @buger/probe package
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
  // Add the implement tool definition import if it exists in @buger/probe
  // If not, we define it here. Assuming it's not in the package yet:
} from '@buger/probe';
import { randomUUID } from 'crypto';

// Generate a session ID
const sessionId = process.env.PROBE_SESSION_ID || randomUUID();
console.error(`Generated session ID for search caching: ${sessionId}`);

// Debug mode
const debug = process.env.DEBUG_CHAT === '1';

// Configure tools with the session ID
const configOptions = {
  sessionId,
  debug
};

// Create configured tool instances
export const tools = {
  searchTool: searchTool(configOptions),
  queryTool: queryTool(configOptions),
  extractTool: extractTool(configOptions),
  // Note: The actual implement tool *instance* comes from probeTool.js
  // This file primarily deals with definitions for the system prompt.
};

// Export individual tools for direct use
export const { searchTool: searchToolInstance, queryTool: queryToolInstance, extractTool: extractToolInstance } = tools;

// For backward compatibility, export the original tool objects
export {
  searchToolInstance as searchTool,
  queryToolInstance as queryTool,
  extractToolInstance as extractTool,
  DEFAULT_SYSTEM_MESSAGE,
  // Export schemas
  searchSchema,
  querySchema,
  extractSchema,
  attemptCompletionSchema,
  // Export tool definitions
  searchToolDefinition,
  queryToolDefinition,
  extractToolDefinition,
  attemptCompletionToolDefinition,
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


// Import the XML parser function from @buger/probe
import { parseXmlToolCall } from '@buger/probe';

// Re-export the original parseXmlToolCall
export { parseXmlToolCall };

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
  if (process.env.DEBUG_CHAT === '1' && thinkingContent) {
    console.log(`[DEBUG] AI Thinking Process:\n${thinkingContent}`);
  }

  return parsedTool;
}

// If tool instances are needed directly (e.g., for API endpoints bypassing the LLM loop),
// they are now created and exported from probeTool.js.
// We should ensure those are imported where needed.