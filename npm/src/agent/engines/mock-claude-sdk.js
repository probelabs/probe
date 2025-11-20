/**
 * Mock Claude SDK Engine for testing the multi-engine architecture
 * This simulates how the Claude SDK would work in Claude Code environment
 */

export async function createMockClaudeSDKEngine(options = {}) {
  const { agent, systemPrompt, debug } = options;

  // Convert Probe tools to mock format
  const tools = [];
  if (agent.allowedTools.isEnabled('listFiles')) {
    tools.push({ name: 'probe_list_files', description: 'List files' });
  }
  if (agent.allowedTools.isEnabled('search')) {
    tools.push({ name: 'probe_search', description: 'Search code' });
  }

  return {
    /**
     * Mock query implementation that simulates Claude SDK behavior
     */
    async *query(prompt, opts = {}) {
      if (debug) {
        console.log('[DEBUG] Mock Claude SDK engine query called');
        console.log('[DEBUG] Prompt:', prompt);
      }

      // Simulate initial thinking
      yield {
        type: 'text',
        content: 'I\'ll help you ' + prompt.toLowerCase() + '. Let me search for that information.\n\n'
      };

      // Simulate tool call based on prompt
      if (prompt.toLowerCase().includes('list') && prompt.toLowerCase().includes('files')) {
        // Simulate calling listFiles tool
        if (debug) {
          console.log('[DEBUG] Mock: Simulating listFiles tool call');
        }

        // Get the path from the prompt (simple extraction)
        const pathMatch = prompt.match(/(?:in|from)\s+(?:the\s+)?([\/\w\-\.]+)\s+folder/i);
        const path = pathMatch ? pathMatch[1] : 'src/agent/engines';

        // Actually call the real tool if available
        if (agent.toolImplementations?.listFiles) {
          try {
            const result = await agent.toolImplementations.listFiles.execute({
              path: path,
              pattern: '*.js'
            });

            yield {
              type: 'text',
              content: 'Found the following JavaScript files:\n\n'
            };

            // Parse and format the result
            if (typeof result === 'string') {
              const files = result.split('\n').filter(f => f.endsWith('.js'));
              for (const file of files) {
                yield {
                  type: 'text',
                  content: `- ${file}\n`
                };
              }
            }
          } catch (error) {
            yield {
              type: 'text',
              content: `Error listing files: ${error.message}\n`
            };
          }
        } else {
          // Mock response if tool not available
          yield {
            type: 'text',
            content: `Here are the JavaScript files in ${path}:\n\n`
          };
          yield {
            type: 'text',
            content: '- enhanced-claude-sdk.js\n'
          };
          yield {
            type: 'text',
            content: '- enhanced-vercel.js\n'
          };
          yield {
            type: 'text',
            content: '- claude-sdk.js\n'
          };
          yield {
            type: 'text',
            content: '- vercel.js\n'
          };
          yield {
            type: 'text',
            content: '- mock-claude-sdk.js\n'
          };
        }

        yield {
          type: 'text',
          content: '\nThese are the engine implementation files that provide multi-engine support for the Probe AI agent.'
        };
      } else if (prompt.toLowerCase().includes('search')) {
        // Simulate search tool
        yield {
          type: 'text',
          content: 'Searching for code patterns...\n\n[Mock search results would appear here]\n'
        };
      } else {
        // Generic response
        yield {
          type: 'text',
          content: 'This is a mock response from the Claude SDK engine. '
        };
        yield {
          type: 'text',
          content: 'In a real Claude Code environment, this would execute the actual query and use the Probe tools.\n'
        };
      }

      // Simulate completion
      yield {
        type: 'metadata',
        data: { type: 'done' }
      };
    },

    /**
     * Get available tools
     */
    getTools() {
      return tools;
    },

    /**
     * Get system prompt
     */
    getSystemPrompt() {
      return systemPrompt || 'Mock Claude SDK system prompt';
    },

    /**
     * Optional cleanup
     */
    async close() {
      if (debug) {
        console.log('[DEBUG] Mock Claude SDK engine closed');
      }
    }
  };
}