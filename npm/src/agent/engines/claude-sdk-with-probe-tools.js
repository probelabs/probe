/**
 * Claude Agent SDK Engine with Probe Tools as Custom MCP Tools
 * This properly registers Probe tools as custom tools using the MCP protocol
 */

import { z } from 'zod';

/**
 * Create custom MCP server with Probe tools
 * @param {Object} agent - The ProbeAgent instance with tool implementations
 * @returns {Object} MCP server configuration
 */
async function createProbeMCPServer(agent) {
  // Dynamic import to avoid hard dependency
  const { tool, createSdkMcpServer } = await import('@anthropic-ai/claude-agent-sdk');

  // Define Probe tools using the tool helper
  const probeSearchTool = tool(
    "search",
    "Search for code patterns across the codebase using Elasticsearch-like query syntax",
    {
      query: z.string().describe("The search query (supports regex and patterns)"),
      path: z.string().optional().describe("Directory to search in"),
      maxResults: z.number().optional().default(10).describe("Maximum number of results")
    },
    async (args) => {
      // Execute the actual Probe search tool
      if (agent.toolImplementations?.search) {
        const result = await agent.toolImplementations.search(args);
        return {
          type: "text",
          text: typeof result === 'string' ? result : JSON.stringify(result, null, 2)
        };
      }
      throw new Error("Search tool not available");
    }
  );

  const probeExtractTool = tool(
    "extract",
    "Extract code blocks from specific file locations with line-level precision",
    {
      files: z.array(z.string()).describe("File paths with optional line numbers (e.g., 'file.js:10-20')")
    },
    async (args) => {
      if (agent.toolImplementations?.extract) {
        const result = await agent.toolImplementations.extract(args);
        return {
          type: "text",
          text: typeof result === 'string' ? result : JSON.stringify(result, null, 2)
        };
      }
      throw new Error("Extract tool not available");
    }
  );

  const probeQueryTool = tool(
    "query",
    "Query code structure using AST patterns for advanced structural search",
    {
      pattern: z.string().describe("Tree-sitter pattern to search for"),
      language: z.string().optional().describe("Programming language"),
      path: z.string().optional().describe("Directory to search in")
    },
    async (args) => {
      if (agent.toolImplementations?.query) {
        const result = await agent.toolImplementations.query(args);
        return {
          type: "text",
          text: typeof result === 'string' ? result : JSON.stringify(result, null, 2)
        };
      }
      throw new Error("Query tool not available");
    }
  );

  const probeListFilesTool = tool(
    "listFiles",
    "List files in a directory with filtering options",
    {
      path: z.string().describe("Directory path to list"),
      pattern: z.string().optional().describe("File pattern filter"),
      recursive: z.boolean().optional().default(true).describe("Search recursively")
    },
    async (args) => {
      if (agent.toolImplementations?.listFiles) {
        const result = await agent.toolImplementations.listFiles(args);
        return {
          type: "text",
          text: typeof result === 'string' ? result : JSON.stringify(result, null, 2)
        };
      }
      throw new Error("ListFiles tool not available");
    }
  );

  const probeSearchFilesTool = tool(
    "searchFiles",
    "Search for files by name or pattern across the codebase",
    {
      pattern: z.string().describe("File name pattern to search for"),
      path: z.string().optional().describe("Directory to search in")
    },
    async (args) => {
      if (agent.toolImplementations?.searchFiles) {
        const result = await agent.toolImplementations.searchFiles(args);
        return {
          type: "text",
          text: typeof result === 'string' ? result : JSON.stringify(result, null, 2)
        };
      }
      throw new Error("SearchFiles tool not available");
    }
  );

  // Create the MCP server with all Probe tools
  const mcpServer = createSdkMcpServer(
    "probe-tools",
    "1.0.0",
    {
      tools: [
        probeSearchTool,
        probeExtractTool,
        probeQueryTool,
        probeListFilesTool,
        probeSearchFilesTool
      ]
    }
  );

  return mcpServer;
}

/**
 * Create a Claude Agent SDK engine with Probe tools as custom MCP tools
 * @param {Object} options - Configuration options
 * @returns {Object} Engine interface
 */
export async function createClaudeSDKEngineWithProbeTools(options) {
  // Dynamic import to avoid hard dependency
  const { query } = await import('@anthropic-ai/claude-agent-sdk');

  // Get reference to ProbeAgent for tool access
  const agent = options.agent;

  // Create MCP server with Probe tools
  const probeMCPServer = await createProbeMCPServer(agent);

  // Create code-search focused system prompt
  const systemPrompt = `You are an AI-powered code analysis assistant with access to Probe code search tools via MCP.

## Your Tools

You have access to powerful code search and analysis tools (via the probe-tools MCP server):

1. **mcp__probe-tools__search**: Search for code patterns across the codebase
2. **mcp__probe-tools__extract**: Extract specific code blocks from files
3. **mcp__probe-tools__query**: Query code structure using AST patterns
4. **mcp__probe-tools__listFiles**: List and explore directory structures
5. **mcp__probe-tools__searchFiles**: Find files by name or pattern

## Working Directory

You are analyzing code in: ${agent.allowedFolders?.[0] || process.cwd()}

## Best Practices

When analyzing code:
1. Start with broad searches using mcp__probe-tools__search
2. Use mcp__probe-tools__extract to get full context of specific implementations
3. Use mcp__probe-tools__query for structural searches
4. Always provide file paths and line numbers in your responses

Remember: All tools are prefixed with mcp__probe-tools__ when calling them.`;

  return {
    /**
     * Query using Claude Agent SDK with Probe tools as MCP tools
     * @param {string} prompt - The prompt to send
     * @param {Object} opts - Additional options
     * @returns {AsyncIterable} Response stream
     */
    async *query(userPrompt, opts = {}) {
      try {
        // Create async generator for streaming input (required for MCP tools)
        async function* createPromptStream() {
          yield {
            type: 'text',
            text: userPrompt
          };
        }

        // Use the query function with MCP servers
        const result = query({
          prompt: createPromptStream(),
          options: {
            mcpServers: {
              "probe-tools": probeMCPServer
            },
            systemPrompt: {
              type: 'text',
              text: systemPrompt
            },
            model: options.model || 'claude-3-5-sonnet-latest',
            temperature: opts.temperature || 0.3,
            maxTokens: opts.maxTokens || agent.maxResponseTokens,
            ...opts
          }
        });

        // Stream the responses
        for await (const message of result) {
          if (message.type === 'text') {
            yield {
              type: 'text',
              content: message.text
            };
          } else if (message.type === 'assistant') {
            yield {
              type: 'assistant',
              message: message.message
            };
          } else if (message.type === 'result') {
            yield {
              type: 'result',
              status: message.subtype,
              usage: message.usage
            };
          } else {
            // Pass through other message types
            yield {
              type: 'metadata',
              data: message
            };
          }
        }
      } catch (error) {
        yield {
          type: 'error',
          error: error.message
        };
      }
    },

    /**
     * Get available tools
     */
    getTools() {
      return [
        'mcp__probe-tools__search',
        'mcp__probe-tools__extract',
        'mcp__probe-tools__query',
        'mcp__probe-tools__listFiles',
        'mcp__probe-tools__searchFiles'
      ];
    },

    /**
     * Get system prompt
     */
    getSystemPrompt() {
      return systemPrompt;
    },

    /**
     * Clean up resources
     */
    async close() {
      // No cleanup needed for MCP servers
    }
  };
}