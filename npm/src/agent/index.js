import { ProbeAgent } from './ProbeAgent.js';
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ErrorCode,
  ListToolsRequestSchema,
  McpError,
} from '@modelcontextprotocol/sdk/types.js';
import { readFileSync, existsSync } from 'fs';
import { resolve } from 'path';

// Helper function to detect if input is a file path and read it
function readInputContent(input) {
  if (!input) return null;
  
  // Check if the input looks like a file path and exists
  try {
    const resolvedPath = resolve(input);
    if (existsSync(resolvedPath)) {
      return readFileSync(resolvedPath, 'utf-8').trim();
    }
  } catch (error) {
    // If file reading fails, treat as literal string
  }
  
  // Return as literal string if not a valid file
  return input;
}

// Parse command line arguments
function parseArgs() {
  const args = process.argv.slice(2);
  const config = {
    mcp: false,
    question: null,
    path: null,
    prompt: null,
    systemPrompt: null,
    provider: null,
    model: null,
    allowEdit: false,
    verbose: false,
    help: false,
    maxIterations: null
  };
  
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    
    if (arg === '--mcp') {
      config.mcp = true;
    } else if (arg === '--help' || arg === '-h') {
      config.help = true;
    } else if (arg === '--verbose') {
      config.verbose = true;
    } else if (arg === '--allow-edit') {
      config.allowEdit = true;
    } else if (arg === '--path' && i + 1 < args.length) {
      config.path = args[++i];
    } else if (arg === '--prompt' && i + 1 < args.length) {
      config.prompt = args[++i];
    } else if (arg === '--system-prompt' && i + 1 < args.length) {
      config.systemPrompt = args[++i];
    } else if (arg === '--provider' && i + 1 < args.length) {
      config.provider = args[++i];
    } else if (arg === '--model' && i + 1 < args.length) {
      config.model = args[++i];
    } else if (arg === '--max-iterations' && i + 1 < args.length) {
      config.maxIterations = parseInt(args[++i], 10);
    } else if (!arg.startsWith('--') && !config.question) {
      // First non-flag argument is the question
      config.question = arg;
    }
  }
  
  return config;
}

// Show help message
function showHelp() {
  console.log(`
probe agent - AI-powered code exploration tool

Usage:
  probe agent <question>           Answer a question about the codebase
  probe agent <file>               Read question from file
  probe agent --mcp                Start as MCP server

Options:
  --path <dir>                     Search directory (default: current)
  --prompt <type>                  Persona: code-explorer, engineer, code-review, support, architect
  --system-prompt <text|file>      Custom system prompt (text or file path)
  --provider <name>                Force AI provider: anthropic, openai, google
  --model <name>                   Override model name
  --allow-edit                     Enable code modification capabilities
  --verbose                        Enable verbose output
  --mcp                           Run as MCP server
  --max-iterations <number>        Max tool iterations (default: 30)
  --help, -h                      Show this help message

Environment Variables:
  ANTHROPIC_API_KEY               Anthropic Claude API key
  OPENAI_API_KEY                  OpenAI GPT API key
  GOOGLE_API_KEY                  Google Gemini API key
  FORCE_PROVIDER                  Force specific provider (anthropic, openai, google)
  MODEL_NAME                      Override model name
  DEBUG                           Enable verbose mode (set to '1')

Examples:
  probe agent "How does authentication work?"
  probe agent question.txt        # Read question from file
  probe agent "Find all database queries" --path ./src --prompt engineer
  probe agent "Review this code for bugs" --prompt code-review --system-prompt custom-prompt.txt
  probe agent --mcp               # Start MCP server mode

Personas:
  code-explorer    Default. Explores and explains code structure and functionality
  engineer         Senior engineer focused on implementation and architecture
  code-review      Reviews code for bugs, performance, and best practices
  support          Helps troubleshoot issues and solve problems
  architect        Focuses on software architecture and high-level design
`);
}

// MCP Server implementation
class ProbeAgentMcpServer {
  constructor() {
    this.server = new Server(
      {
        name: '@buger/probe-agent',
        version: '1.0.0',
      },
      {
        capabilities: {
          tools: {},
        },
      }
    );

    this.setupToolHandlers();
    this.server.onerror = (error) => console.error('[MCP Error]', error);
    process.on('SIGINT', async () => {
      await this.server.close();
      process.exit(0);
    });
  }

  setupToolHandlers() {
    this.server.setRequestHandler(ListToolsRequestSchema, async () => ({
      tools: [
        {
          name: 'search_code',
          description: "Search code and answer questions about the codebase using an AI agent. This tool provides intelligent responses based on code analysis.",
          inputSchema: {
            type: 'object',
            properties: {
              query: {
                type: 'string',
                description: 'The question or request about the codebase.',
              },
              path: {
                type: 'string',
                description: 'Optional path to the directory to search in. Defaults to current directory.',
              },
              prompt: {
                type: 'string',
                description: 'Optional persona type: code-explorer, engineer, code-review, support, architect.',
              },
              system_prompt: {
                type: 'string',
                description: 'Optional custom system prompt (text or file path).',
              },
              provider: {
                type: 'string',
                description: 'Optional AI provider to force: anthropic, openai, google.',
              },
              model: {
                type: 'string',
                description: 'Optional model name override.',
              },
              allow_edit: {
                type: 'boolean',
                description: 'Enable code modification capabilities.',
              },
              max_iterations: {
                type: 'number',
                description: 'Maximum number of tool iterations (default: 30).',
              }
            },
            required: ['query']
          },
        },
      ],
    }));

    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      if (request.params.name !== 'search_code') {
        throw new McpError(
          ErrorCode.MethodNotFound,
          `Unknown tool: ${request.params.name}`
        );
      }

      try {
        const args = request.params.arguments;

        // Validate required fields
        if (!args.query) {
          throw new Error("Query is required");
        }

        // Set MAX_TOOL_ITERATIONS if provided
        if (args.max_iterations) {
          process.env.MAX_TOOL_ITERATIONS = args.max_iterations.toString();
        }

        // Process system prompt if provided (could be file or literal string)
        let systemPrompt = null;
        if (args.system_prompt) {
          systemPrompt = readInputContent(args.system_prompt);
          if (!systemPrompt) {
            throw new Error('System prompt could not be read');
          }
        }

        // Process query input (could be file or literal string)
        const query = readInputContent(args.query);
        if (!query) {
          throw new Error('Query is required and could not be read');
        }

        // Create agent with configuration
        const agentConfig = {
          path: args.path || process.cwd(),
          promptType: args.prompt || 'code-explorer',
          customPrompt: systemPrompt,
          provider: args.provider,
          model: args.model,
          allowEdit: !!args.allow_edit,
          debug: process.env.DEBUG === '1'
        };

        const agent = new ProbeAgent(agentConfig);
        const result = await agent.answer(query);

        // Get token usage for debugging
        const tokenUsage = agent.getTokenUsage();
        console.error(`Token usage: ${JSON.stringify(tokenUsage)}`);

        return {
          content: [
            {
              type: 'text',
              text: result,
            },
          ],
        };
      } catch (error) {
        console.error(`Error executing search_code:`, error);
        return {
          content: [
            {
              type: 'text',
              text: `Error: ${error.message}`,
            },
          ],
          isError: true,
        };
      }
    });
  }

  async run() {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
    console.error('Probe Agent MCP server running on stdio');
  }
}

// Main function
async function main() {
  const config = parseArgs();

  if (config.help) {
    showHelp();
    return;
  }

  if (config.mcp) {
    // Start as MCP server
    const server = new ProbeAgentMcpServer();
    await server.run();
    return;
  }

  if (!config.question) {
    showHelp();
    process.exit(1);
  }

  try {
    // Set environment variables if provided via flags
    if (config.verbose) {
      process.env.DEBUG = '1';
    }
    if (config.provider) {
      process.env.FORCE_PROVIDER = config.provider;
    }
    if (config.model) {
      process.env.MODEL_NAME = config.model;
    }
    if (config.maxIterations) {
      process.env.MAX_TOOL_ITERATIONS = config.maxIterations.toString();
    }

    // Process question input (could be file or literal string)
    const question = readInputContent(config.question);
    if (!question) {
      console.error('Error: Question is required and could not be read');
      process.exit(1);
    }

    // Process system prompt if provided (could be file or literal string)
    let systemPrompt = null;
    if (config.systemPrompt) {
      systemPrompt = readInputContent(config.systemPrompt);
      if (!systemPrompt) {
        console.error('Error: System prompt could not be read');
        process.exit(1);
      }
    }

    // Create and configure agent
    const agentConfig = {
      path: config.path,
      promptType: config.prompt,
      customPrompt: systemPrompt,
      allowEdit: config.allowEdit,
      debug: config.verbose
    };

    const agent = new ProbeAgent(agentConfig);
    const result = await agent.answer(question);

    // Output the result
    console.log(result);

    // Show token usage in verbose mode
    if (config.verbose) {
      const tokenUsage = agent.getTokenUsage();
      console.error(`\n[DEBUG] Token usage: ${JSON.stringify(tokenUsage, null, 2)}`);
    }

  } catch (error) {
    console.error(`Error: ${error.message}`);
    if (config.verbose) {
      console.error(error.stack);
    }
    process.exit(1);
  }
}

// Handle uncaught exceptions
process.on('uncaughtException', (error) => {
  console.error('Uncaught Exception:', error);
  process.exit(1);
});

process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled Rejection at:', promise, 'reason:', reason);
  process.exit(1);
});

// Run main function
main().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});