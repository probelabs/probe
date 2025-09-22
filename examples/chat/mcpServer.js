/**
 * MCP Server implementation for Probe Agent
 * Provides MCP tools that integrate with the Probe code search capabilities
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  searchToolInstance,
  queryToolInstance,
  extractToolInstance,
  listFilesToolInstance,
  searchFilesToolInstance
} from './probeTool.js';
import { ProbeChat } from './probeChat.js';

/**
 * Create MCP server for Probe tools
 */
export async function createMCPServer(options = {}) {
  const server = new Server(
    {
      name: 'probe-mcp-server',
      version: '1.0.0',
    },
    {
      capabilities: {
        tools: {},
      },
    }
  );

  // List available tools
  server.setRequestHandler('tools/list', async () => ({
    tools: [
      {
        name: 'probe_search',
        description: 'Search code using keywords or patterns with flexible text search',
        inputSchema: {
          type: 'object',
          properties: {
            query: {
              type: 'string',
              description: 'Search query using elastic search syntax'
            },
            path: {
              type: 'string',
              description: 'Directory to search in (defaults to current working directory)'
            },
            maxTokens: {
              type: 'number',
              description: 'Maximum tokens to return (default: 10000)'
            },
            allowTests: {
              type: 'boolean',
              description: 'Include test files in results (default: false)'
            },
            exact: {
              type: 'boolean',
              description: 'Perform exact search without tokenization'
            },
            language: {
              type: 'string',
              description: 'Limit search to specific programming language'
            }
          },
          required: ['query']
        }
      },
      {
        name: 'probe_query',
        description: 'Query code using structural AST patterns',
        inputSchema: {
          type: 'object',
          properties: {
            pattern: {
              type: 'string',
              description: 'AST pattern to search for'
            },
            path: {
              type: 'string',
              description: 'Directory to search in'
            },
            language: {
              type: 'string',
              description: 'Programming language'
            },
            allowTests: {
              type: 'boolean',
              description: 'Include test files'
            }
          },
          required: ['pattern']
        }
      },
      {
        name: 'probe_extract',
        description: 'Extract code blocks from files',
        inputSchema: {
          type: 'object',
          properties: {
            targets: {
              type: 'array',
              items: { type: 'string' },
              description: 'File paths or file:line specifications to extract'
            },
            contextLines: {
              type: 'number',
              description: 'Number of context lines'
            },
            allowTests: {
              type: 'boolean',
              description: 'Allow test files'
            },
            format: {
              type: 'string',
              enum: ['plain', 'markdown', 'json'],
              description: 'Output format'
            }
          },
          required: ['targets']
        }
      },
      {
        name: 'probe_list_files',
        description: 'List files in a directory',
        inputSchema: {
          type: 'object',
          properties: {
            directory: {
              type: 'string',
              description: 'Directory path to list files from'
            }
          }
        }
      },
      {
        name: 'probe_search_files',
        description: 'Search for files using glob patterns',
        inputSchema: {
          type: 'object',
          properties: {
            pattern: {
              type: 'string',
              description: 'Glob pattern to match files'
            },
            directory: {
              type: 'string',
              description: 'Directory to search in'
            },
            recursive: {
              type: 'boolean',
              description: 'Search recursively'
            }
          },
          required: ['pattern']
        }
      },
      {
        name: 'probe_chat',
        description: 'Ask questions about the codebase using AI agent',
        inputSchema: {
          type: 'object',
          properties: {
            message: {
              type: 'string',
              description: 'Question to ask about the codebase'
            },
            schema: {
              type: 'string',
              description: 'Optional JSON schema for structured output'
            }
          },
          required: ['message']
        }
      }
    ]
  }));

  // Handle tool calls
  server.setRequestHandler('tools/call', async (request) => {
    const { name, arguments: args } = request.params;

    try {
      switch (name) {
        case 'probe_search': {
          const result = await searchToolInstance.execute({
            query: args.query,
            path: args.path,
            allow_tests: args.allowTests,
            exact: args.exact,
            maxTokens: args.maxTokens,
            language: args.language,
            sessionId: options.sessionId || 'mcp-session'
          });

          return {
            content: [
              {
                type: 'text',
                text: typeof result === 'string' ? result : JSON.stringify(result)
              }
            ]
          };
        }

        case 'probe_query': {
          const result = await queryToolInstance.execute({
            pattern: args.pattern,
            path: args.path,
            language: args.language,
            allow_tests: args.allowTests,
            sessionId: options.sessionId || 'mcp-session'
          });

          return {
            content: [
              {
                type: 'text',
                text: typeof result === 'string' ? result : JSON.stringify(result)
              }
            ]
          };
        }

        case 'probe_extract': {
          const result = await extractToolInstance.execute({
            targets: args.targets,
            context_lines: args.contextLines,
            allow_tests: args.allowTests,
            format: args.format,
            sessionId: options.sessionId || 'mcp-session'
          });

          return {
            content: [
              {
                type: 'text',
                text: typeof result === 'string' ? result : JSON.stringify(result)
              }
            ]
          };
        }

        case 'probe_list_files': {
          const result = await listFilesToolInstance.execute({
            directory: args.directory,
            sessionId: options.sessionId || 'mcp-session'
          });

          return {
            content: [
              {
                type: 'text',
                text: typeof result === 'string' ? result : JSON.stringify(result)
              }
            ]
          };
        }

        case 'probe_search_files': {
          const result = await searchFilesToolInstance.execute({
            pattern: args.pattern,
            directory: args.directory,
            recursive: args.recursive,
            sessionId: options.sessionId || 'mcp-session'
          });

          return {
            content: [
              {
                type: 'text',
                text: typeof result === 'string' ? result : JSON.stringify(result)
              }
            ]
          };
        }

        case 'probe_chat': {
          // Create a ProbeChat instance for AI-powered responses
          const chat = new ProbeChat({
            allowEdit: false,
            debug: options.debug
          });

          const result = await chat.chat(args.message, {
            schema: args.schema
          });

          return {
            content: [
              {
                type: 'text',
                text: result
              }
            ]
          };
        }

        default:
          throw new Error(`Unknown tool: ${name}`);
      }
    } catch (error) {
      return {
        content: [
          {
            type: 'text',
            text: `Error executing tool ${name}: ${error.message}`
          }
        ],
        isError: true
      };
    }
  });

  return server;
}

/**
 * Start MCP server with stdio transport
 */
export async function startMCPServer(options = {}) {
  const server = await createMCPServer(options);
  const transport = new StdioServerTransport();

  await server.connect(transport);

  console.error('Probe MCP Server started successfully');

  return server;
}

// If running as standalone script
if (import.meta.url === `file://${process.argv[1]}`) {
  startMCPServer({
    debug: process.env.DEBUG === '1'
  }).catch(error => {
    console.error('Failed to start MCP server:', error);
    process.exit(1);
  });
}