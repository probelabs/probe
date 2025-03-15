# Implementation Plan: Make examples/chat Call Tools Like examples/web

## Overview

The goal is to update the `examples/chat` implementation to call tools exactly like `examples/web` does. This involves:

1. Updating `probeChat.js` to use SDK-based tools directly from `@buger/probe`
2. Updating `tools.js` for backward compatibility
3. Ensuring consistent tool usage across files

## Detailed Changes

### 1. Update `probeChat.js`

Replace the current implementation that imports tools from `./tools.js` with direct imports from `@buger/probe`:

```javascript
// BEFORE:
import { searchTool, queryTool, extractTool } from './tools.js';
// ...
const generateOptions = {
  // ...
  tools: {
    search: searchTool,
    query: queryTool,
    extract: extractTool
  },
  // ...
};

// AFTER:
import { searchTool, queryTool, extractTool, DEFAULT_SYSTEM_MESSAGE } from '@buger/probe';
// ...
// Generate a session ID
const sessionId = randomUUID();
// Configure tools with the session ID
const configOptions = {
  sessionId,
  debug: process.env.DEBUG === 'true' || process.env.DEBUG === '1'
};
// Create configured tool instances
const tools = [
  searchTool(configOptions),
  queryTool(configOptions),
  extractTool(configOptions)
];
// ...
const generateOptions = {
  // ...
  tools: tools,
  // ...
};
```

### 2. Update `tools.js` for Backward Compatibility

Replace the CLI-based implementation with SDK-based tools from `@buger/probe`:

```javascript
// Import tool generators from @buger/probe package
import { searchTool, queryTool, extractTool, DEFAULT_SYSTEM_MESSAGE } from '@buger/probe';
import { randomUUID } from 'crypto';

// Generate a session ID
const sessionId = process.env.PROBE_SESSION_ID || randomUUID();
console.log(`Generated session ID for search caching: ${sessionId}`);

// Configure tools with the session ID
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
export const { searchTool: searchToolInstance, queryTool: queryToolInstance, extractTool: extractToolInstance } = tools;

// For backward compatibility, export the original tool objects
export { searchToolInstance as searchTool, queryToolInstance as queryTool, extractToolInstance as extractTool };
```

### 3. Ensure Consistent Tool Usage in `index.js`

The `index.js` file already uses the SDK-based tools correctly, but we should ensure it passes them to the AI in the same way as the web example:

```javascript
// BEFORE:
const generateOptions = {
  // ...
  tools: configuredTools,
  // ...
};

// AFTER (if needed):
const generateOptions = {
  // ...
  tools: [configuredTools.search, configuredTools.query, configuredTools.extract],
  // ...
};
```

## Implementation Steps

1. Switch to Code mode
2. Update `tools.js` first to provide backward compatibility
3. Update `probeChat.js` to use the SDK-based tools
4. Update `index.js` if needed to ensure consistent tool usage
5. Test the implementation to ensure it works correctly