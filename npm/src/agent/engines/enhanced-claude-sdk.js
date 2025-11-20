/**
 * Enhanced Claude Agent SDK Engine with Probe tool integration
 * Provides native Claude capabilities with MCP support and Probe tools
 */

/**
 * Convert Probe tool definitions to Claude SDK format
 */
function convertProbeToolsToClaudeFormat(agent) {
  const tools = [];

  // Core Probe tools
  if (agent.allowedTools.isEnabled('search')) {
    tools.push({
      name: 'probe_search',
      description: 'Search for code patterns across the codebase using probe search',
      input_schema: {
        type: 'object',
        properties: {
          query: { type: 'string', description: 'Search query (supports regex and patterns)' },
          path: { type: 'string', description: 'Path to search in (optional)' },
          maxResults: { type: 'number', description: 'Maximum number of results (default: 10)' }
        },
        required: ['query']
      }
    });
  }

  if (agent.allowedTools.isEnabled('extract')) {
    tools.push({
      name: 'probe_extract',
      description: 'Extract code blocks from specific file locations',
      input_schema: {
        type: 'object',
        properties: {
          files: {
            type: 'array',
            items: { type: 'string' },
            description: 'File paths with optional line numbers (e.g., "file.js:10-20")'
          }
        },
        required: ['files']
      }
    });
  }

  if (agent.allowedTools.isEnabled('query')) {
    tools.push({
      name: 'probe_query',
      description: 'Query code structure using AST patterns',
      input_schema: {
        type: 'object',
        properties: {
          pattern: { type: 'string', description: 'Tree-sitter pattern to search for' },
          language: { type: 'string', description: 'Programming language' },
          path: { type: 'string', description: 'Path to search in (optional)' }
        },
        required: ['pattern']
      }
    });
  }

  if (agent.allowedTools.isEnabled('listFiles')) {
    tools.push({
      name: 'probe_list_files',
      description: 'List files in a directory with filtering options',
      input_schema: {
        type: 'object',
        properties: {
          path: { type: 'string', description: 'Directory path' },
          pattern: { type: 'string', description: 'File pattern filter (optional)' },
          recursive: { type: 'boolean', description: 'Search recursively (default: true)' }
        },
        required: ['path']
      }
    });
  }

  if (agent.allowedTools.isEnabled('searchFiles')) {
    tools.push({
      name: 'probe_search_files',
      description: 'Search for files by name or pattern',
      input_schema: {
        type: 'object',
        properties: {
          pattern: { type: 'string', description: 'File name pattern to search for' },
          path: { type: 'string', description: 'Directory to search in (optional)' }
        },
        required: ['pattern']
      }
    });
  }

  return tools;
}

/**
 * Create a code-search focused system prompt
 */
function createCodeSearchSystemPrompt(agent) {
  const baseDirectory = agent.allowedFolders?.[0] || process.cwd();

  return `You are an AI-powered code analysis assistant with access to the Probe code search tool.

## Your Capabilities

You have access to powerful code search and analysis tools:

1. **probe_search**: Search for code patterns, functions, classes, or any text across the codebase
2. **probe_extract**: Extract specific code blocks from files with line-level precision
3. **probe_query**: Query code structure using AST patterns (advanced structural search)
4. **probe_list_files**: List and explore directory structures
5. **probe_search_files**: Find files by name or pattern

## Working Directory

You are analyzing code in: ${baseDirectory}

${agent.allowedFolders?.length > 1 ? `Additional allowed directories: ${agent.allowedFolders.slice(1).join(', ')}` : ''}

## Best Practices

When analyzing code:
1. Start with broad searches to understand the codebase structure
2. Use probe_search to find relevant code patterns
3. Use probe_extract to get full context of specific implementations
4. Use probe_query for structural searches (finding all functions with specific signatures, etc.)
5. Always provide file paths and line numbers in your responses for easy navigation

## Response Format

When discussing code:
- Include file paths with line numbers (e.g., src/main.js:42-58)
- Show relevant code snippets in code blocks
- Explain the code's purpose and how it works
- Suggest improvements or identify issues when relevant

Remember: You have direct access to the entire codebase. Use the tools liberally to provide comprehensive, accurate answers based on the actual code.`;
}

/**
 * Create an enhanced Claude Agent SDK engine with Probe tools
 * @param {Object} options - Configuration options
 * @returns {Object} Engine interface
 */
export async function createEnhancedClaudeSDKEngine(options) {
  // Dynamic import to avoid hard dependency
  let claudeSDK;

  try {
    claudeSDK = await import('@anthropic-ai/claude-agent-sdk');

    if (!claudeSDK.query) {
      throw new Error('query function not found in @anthropic-ai/claude-agent-sdk');
    }

    if (options.debug) {
      console.log('[DEBUG] Using real Claude Agent SDK');
    }
  } catch (error) {
    // Only use mock if explicitly requested for testing
    if (process.env.FORCE_MOCK_CLAUDE_SDK === 'true') {
      if (options.debug) {
        console.log('[DEBUG] Using mock implementation (FORCE_MOCK_CLAUDE_SDK=true)');
      }
      const { createMockClaudeSDKEngine } = await import('./mock-claude-sdk.js');
      return createMockClaudeSDKEngine(options);
    }

    // Otherwise, this is a real error - the SDK should be available
    throw new Error(
      'Claude Agent SDK not installed or incompatible. Please run:\n' +
      'npm install @anthropic-ai/claude-agent-sdk\n\n' +
      'Original error: ' + error.message
    );
  }

  // Get reference to ProbeAgent for tool access
  const agent = options.agent;

  // Create code-search focused system prompt
  const systemPrompt = options.systemPrompt || createCodeSearchSystemPrompt(agent);

  // Convert Probe tools to Claude SDK format
  const probeTools = convertProbeToolsToClaudeFormat(agent);

  // The Claude Agent SDK uses a different approach - it works directly with Claude Code
  // The query function is the main interface

  // Create tool execution wrapper
  const executeProbeeTool = async (toolName, args) => {
    // Map Claude tool names back to Probe tool names
    const toolMap = {
      'probe_search': 'search',
      'probe_extract': 'extract',
      'probe_query': 'query',
      'probe_list_files': 'listFiles',
      'probe_search_files': 'searchFiles'
    };

    const probeToolName = toolMap[toolName] || toolName;

    // Execute the tool using ProbeAgent's implementation
    if (agent.toolImplementations?.[probeToolName]) {
      return await agent.toolImplementations[probeToolName](args);
    }

    throw new Error(`Tool ${toolName} not found in ProbeAgent implementations`);
  };

  return {
    /**
     * Query using Claude Agent SDK with Probe tools
     * @param {string} prompt - The prompt to send
     * @param {Object} opts - Additional options
     * @returns {AsyncIterable} Response stream
     */
    async *query(prompt, opts = {}) {
      try {
        // Build query options with MCP servers if available
        const queryOptions = {
          prompt,
          options: {
            mcpServers: opts.mcpServers || agent.mcpServers,
            allowedTools: opts.allowedTools || probeTools.map(t => t.name),
            temperature: opts.temperature || 0.3,
            maxTokens: opts.maxTokens || agent.maxResponseTokens,
            ...opts
          }
        };

        // Stream from Claude Agent SDK
        const stream = claudeSDK.query(queryOptions);

        // Map responses to common format
        for await (const message of stream) {
          if (message.type === 'text' || message.type === 'text_delta') {
            yield {
              type: 'text',
              content: message.content || message.text || message.delta
            };
          } else if (message.type === 'tool_use' || message.type === 'tool_call') {
            // Execute the Probe tool
            try {
              const result = await executeProbeeTool(
                message.name || message.tool_name,
                message.input || message.arguments
              );

              yield {
                type: 'tool_result',
                tool: {
                  id: message.id,
                  name: message.name || message.tool_name,
                  result
                }
              };
            } catch (error) {
              yield {
                type: 'tool_error',
                tool: {
                  id: message.id,
                  name: message.name || message.tool_name,
                  error: error.message
                }
              };
            }
          } else if (message.type === 'error') {
            yield {
              type: 'error',
              error: message.error || message.message
            };
          } else if (message.type === 'result' && message.subtype === 'success') {
            // Handle final result
            yield {
              type: 'text',
              content: message.result
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
        // Handle errors gracefully
        yield {
          type: 'error',
          error: error.message
        };
      }
    },

    /**
     * Get available tools for this engine
     */
    getTools() {
      return probeTools;
    },

    /**
     * Get system prompt for this engine
     */
    getSystemPrompt() {
      return systemPrompt;
    },

    /**
     * Register MCP server (Claude SDK specific feature)
     */
    async registerMCPServer(name, config) {
      if (claudeSDK.createSdkMcpServer) {
        // Use Claude SDK's MCP server creation
        return claudeSDK.createSdkMcpServer(name, '1.0.0', config);
      }
      throw new Error('MCP registration not supported in this version of Claude SDK');
    },

    /**
     * Clean up resources
     */
    async close() {
      if (claudeAgent.dispose) {
        await claudeAgent.dispose();
      } else if (claudeAgent.close) {
        await claudeAgent.close();
      }
    }
  };
}