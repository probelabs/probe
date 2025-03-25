// Import tool generators from @buger/probe package
import { searchTool, queryTool, extractTool, DEFAULT_SYSTEM_MESSAGE } from '@buger/probe';
import { randomUUID } from 'crypto';

// Generate a session ID
const sessionId = process.env.PROBE_SESSION_ID || randomUUID();
console.log(`Generated session ID for search caching: ${sessionId}`);

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
  extractTool: extractTool(configOptions)
};

// Export individual tools for direct use
export const { searchTool: searchToolInstance, queryTool: queryToolInstance, extractTool: extractToolInstance } = tools;

// For backward compatibility, export the original tool objects
export { searchToolInstance as searchTool, queryToolInstance as queryTool, extractToolInstance as extractTool, DEFAULT_SYSTEM_MESSAGE };