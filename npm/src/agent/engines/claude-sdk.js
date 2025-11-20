/**
 * Claude Agent SDK Engine
 * Provides native Claude capabilities with MCP support
 * Package must be installed separately: npm install @anthropic-ai/claude-agent-sdk
 */

/**
 * Create a Claude Agent SDK engine
 * @param {Object} options - Configuration options
 * @returns {Object} Engine interface
 */
export async function createClaudeSDKEngine(options) {
  // Dynamic import to avoid hard dependency
  let ClaudeAgent;
  try {
    const sdk = await import('@anthropic-ai/claude-agent-sdk');
    ClaudeAgent = sdk.ClaudeAgent || sdk.default?.ClaudeAgent;

    if (!ClaudeAgent) {
      throw new Error('ClaudeAgent not found in @anthropic-ai/claude-agent-sdk');
    }
  } catch (error) {
    throw new Error(
      'Claude Agent SDK not installed. Please run:\n' +
      'npm install @anthropic-ai/claude-agent-sdk\n\n' +
      'Original error: ' + error.message
    );
  }

  // Initialize the Claude agent
  const agent = new ClaudeAgent({
    apiKey: options.apiKey || process.env.ANTHROPIC_API_KEY,
    model: options.model || 'claude-3-5-sonnet-20241022',
    systemPrompt: options.systemPrompt,
    settingSources: options.settingSources,
    ...options.customSettings
  });

  return {
    /**
     * Query using Claude Agent SDK
     * @param {string} prompt - The prompt to send
     * @param {Object} options - Additional options
     * @returns {AsyncIterable} Response stream
     */
    async *query(prompt, opts = {}) {
      try {
        // Build query options
        const queryOptions = {
          prompt,
          options: {
            mcpServers: opts.mcpServers,
            allowedTools: opts.allowedTools,
            temperature: opts.temperature,
            maxTokens: opts.maxTokens,
            ...opts
          }
        };

        // Stream from Claude Agent SDK
        const stream = agent.query(queryOptions);

        // Map responses to common format
        for await (const message of stream) {
          if (message.type === 'text' || message.type === 'text_delta') {
            yield {
              type: 'text',
              content: message.content || message.text || message.delta
            };
          } else if (message.type === 'tool_use' || message.type === 'tool_call') {
            yield {
              type: 'tool_call',
              tool: {
                id: message.id,
                name: message.name || message.tool_name,
                arguments: message.input || message.arguments
              }
            };
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
     * Register MCP server (Claude SDK specific feature)
     */
    async registerMCPServer(name, config) {
      if (agent.registerMCPServer) {
        return agent.registerMCPServer(name, config);
      }
      throw new Error('MCP registration not supported in this version of Claude SDK');
    },

    /**
     * Clean up resources
     */
    async close() {
      if (agent.dispose) {
        await agent.dispose();
      } else if (agent.close) {
        await agent.close();
      }
    }
  };
}