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
import { initializeSimpleTelemetryFromOptions, SimpleAppTracer } from './simpleTelemetry.js';
import { 
  cleanSchemaResponse, 
  processSchemaResponse, 
  isJsonSchema, 
  validateJsonResponse, 
  createJsonCorrectionPrompt,
  isMermaidSchema,
  validateMermaidResponse,
  createMermaidCorrectionPrompt,
  validateAndFixMermaidResponse
} from './schemaUtils.js';
import { ACPServer } from './acp/index.js';

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

// Function to check if stdin has data available
function isStdinAvailable() {
  // Check if stdin is not a TTY (indicates piped input)
  // Also ensure we're not in an interactive terminal session
  return !process.stdin.isTTY && process.stdin.readable;
}

// Function to read from stdin with timeout detection for interactive vs piped usage
function readFromStdin() {
  return new Promise((resolve, reject) => {
    let data = '';
    let hasReceivedData = false;
    let dataChunks = [];
    
    // Short timeout to detect if this is interactive usage (no immediate data)
    const timeout = setTimeout(() => {
      if (!hasReceivedData) {
        reject(new Error('INTERACTIVE_MODE'));
      }
    }, 100); // Very short timeout - piped input should arrive immediately
    
    process.stdin.setEncoding('utf8');
    
    // Try to read immediately to see if data is available
    process.stdin.on('readable', () => {
      let chunk;
      while ((chunk = process.stdin.read()) !== null) {
        hasReceivedData = true;
        clearTimeout(timeout);
        dataChunks.push(chunk);
        data += chunk;
      }
    });
    
    process.stdin.on('end', () => {
      clearTimeout(timeout);
      const trimmed = data.trim();
      if (!trimmed && dataChunks.length === 0) {
        reject(new Error('No input received from stdin'));
      } else {
        resolve(trimmed);
      }
    });
    
    process.stdin.on('error', (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    
    // Force a read attempt to trigger readable event if data is available
    process.nextTick(() => {
      const chunk = process.stdin.read();
      if (chunk !== null) {
        hasReceivedData = true;
        clearTimeout(timeout);
        data += chunk;
        dataChunks.push(chunk);
      }
    });
  });
}

// Parse command line arguments
function parseArgs() {
  const args = process.argv.slice(2);
  const config = {
    mcp: false,
    acp: false,
    question: null,
    path: null,
    prompt: null,
    systemPrompt: null,
    schema: null,
    provider: null,
    model: null,
    allowEdit: false,
    verbose: false,
    help: false,
    maxIterations: null,
    traceFile: undefined,
    traceRemote: undefined,
    traceConsole: false,
    useStdin: false // New flag to indicate stdin should be used
  };
  
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    
    if (arg === '--mcp') {
      config.mcp = true;
    } else if (arg === '--acp') {
      config.acp = true;
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
    } else if (arg === '--schema' && i + 1 < args.length) {
      config.schema = args[++i];
    } else if (arg === '--provider' && i + 1 < args.length) {
      config.provider = args[++i];
    } else if (arg === '--model' && i + 1 < args.length) {
      config.model = args[++i];
    } else if (arg === '--max-iterations' && i + 1 < args.length) {
      config.maxIterations = parseInt(args[++i], 10);
    } else if (arg === '--trace-file' && i + 1 < args.length) {
      config.traceFile = args[++i];
    } else if (arg === '--trace-remote' && i + 1 < args.length) {
      config.traceRemote = args[++i];
    } else if (arg === '--trace-console') {
      config.traceConsole = true;
    } else if (!arg.startsWith('--') && !config.question) {
      // First non-flag argument is the question
      config.question = arg;
    }
  }
  
  // Auto-detect stdin usage if no question provided and stdin appears to be piped
  // For simplicity, let's use a more practical approach:
  // If user provides no arguments at all, we try to read from stdin with a short timeout
  // This works better across different environments
  if (!config.question && !config.mcp && !config.acp && !config.help) {
    // We'll check for stdin in the main function with a timeout approach
    config.useStdin = true;
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
  echo "question" | probe agent    Read question from stdin (pipe input)
  probe agent --mcp                Start as MCP server
  probe agent --acp                Start as ACP server

Options:
  --path <dir>                     Search directory (default: current)
  --prompt <type>                  Persona: code-explorer, engineer, code-review, support, architect
  --system-prompt <text|file>      Custom system prompt (text or file path)
  --schema <schema|file>           Output schema (JSON, XML, any format - text or file path)
  --provider <name>                Force AI provider: anthropic, openai, google
  --model <name>                   Override model name
  --allow-edit                     Enable code modification capabilities
  --verbose                        Enable verbose output
  --mcp                           Run as MCP server
  --acp                           Run as ACP server (Agent Client Protocol)
  --max-iterations <number>        Max tool iterations (default: 30)
  --trace-file <path>              Enable tracing to file (JSONL format)
  --trace-remote <endpoint>        Enable tracing to remote OTLP endpoint
  --trace-console                  Enable tracing to console output
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
  echo "How does the search algorithm work?" | probe agent  # Read from stdin
  cat requirements.txt | probe agent --prompt architect     # Pipe file content
  probe agent "Find all database queries" --path ./src --prompt engineer
  probe agent "Review this code for bugs" --prompt code-review --system-prompt custom-prompt.txt
  probe agent "List all functions" --schema '{"functions": [{"name": "string", "file": "string"}]}'
  probe agent "Analyze codebase" --schema schema.json  # Schema from file
  probe agent "Debug issue" --trace-file ./debug.jsonl --verbose
  probe agent "Analyze code" --trace-remote http://localhost:4318/v1/traces
  probe agent --mcp               # Start MCP server mode
  probe agent --acp               # Start ACP server mode

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
              },
              schema: {
                type: 'string',
                description: 'Optional output schema (JSON, XML, or any format - text or file path).',
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

        // Process schema if provided (could be file or literal string)
        let schema = null;
        if (args.schema) {
          schema = readInputContent(args.schema);
          if (!schema) {
            throw new Error('Schema could not be read');
          }
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
        let result = await agent.answer(query, [], { schema });

        // If schema is provided, make a follow-up request to format the output
        if (schema) {
          const schemaPrompt = `Now you need to respond according to this schema:\n\n${schema}\n\nPlease reformat your previous response to match this schema exactly. Only return the formatted response, no additional text.`;
          
          try {
            result = await agent.answer(schemaPrompt, [], { schema });
            // Clean the schema response to remove code blocks and formatting
            result = cleanSchemaResponse(result);

            // Check for mermaid diagrams in response and validate/fix them regardless of schema
            try {
              const mermaidValidation = await validateAndFixMermaidResponse(result, {
                debug: args.debug,
                path: agentConfig.path,
                provider: args.provider,
                model: args.model
              });

              if (mermaidValidation.wasFixed) {
                result = mermaidValidation.fixedResponse;
                if (args.debug) {
                  console.error(`[DEBUG] Mermaid diagrams fixed using specialized agent`);
                  mermaidValidation.fixingResults.forEach((fixResult, index) => {
                    if (fixResult.wasFixed) {
                      console.error(`[DEBUG] Fixed diagram ${index + 1}: ${fixResult.originalError}`);
                    }
                  });
                }
              } else if (!mermaidValidation.isValid && mermaidValidation.diagrams && mermaidValidation.diagrams.length > 0 && args.debug) {
                console.error(`[DEBUG] Mermaid validation failed: ${mermaidValidation.errors?.join(', ')}`);
              }
            } catch (error) {
              if (args.debug) {
                console.error(`[DEBUG] Enhanced mermaid validation failed: ${error.message}`);
              }
            }

            // Then, if schema expects JSON, validate and retry if invalid
            if (isJsonSchema(schema)) {
              const validation = validateJsonResponse(result);
              if (!validation.isValid) {
                // Retry once with correction prompt
                const correctionPrompt = createJsonCorrectionPrompt(result, schema, validation.error);
                try {
                  result = await agent.answer(correctionPrompt, [], { schema });
                  result = cleanSchemaResponse(result);
                  
                  // Validate again after correction
                  const finalValidation = validateJsonResponse(result);
                  if (!finalValidation.isValid && args.debug) {
                    console.error(`[DEBUG] JSON validation failed after retry: ${finalValidation.error}`);
                  }
                } catch (retryError) {
                  // If retry fails, keep the original result
                  if (args.debug) {
                    console.error(`[DEBUG] JSON correction retry failed: ${retryError.message}`);
                  }
                }
              }
            }
          } catch (error) {
            // If schema formatting fails, use original result
          }
        }

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

  if (config.acp) {
    // Start as ACP server
    const server = new ACPServer({
      provider: config.provider,
      model: config.model,
      path: config.path,
      allowEdit: config.allowEdit,
      debug: config.verbose
    });
    await server.start();
    return;
  }

  // Handle stdin input if detected
  if (config.useStdin) {
    try {
      if (config.verbose) {
        console.error('[DEBUG] Reading question from stdin...');
      }
      config.question = await readFromStdin();
      if (!config.question) {
        console.error('Error: No input received from stdin');
        process.exit(1);
      }
    } catch (error) {
      // If this is interactive mode (no piped input), show help
      if (error.message === 'INTERACTIVE_MODE') {
        showHelp();
        process.exit(0);
      } else {
        console.error(`Error reading from stdin: ${error.message}`);
        process.exit(1);
      }
    }
  }

  if (!config.question) {
    showHelp();
    process.exit(1);
  }

  try {
    // Initialize tracing if any tracing options are provided
    let telemetryConfig = null;
    let appTracer = null;
    if (config.traceFile !== undefined || config.traceRemote !== undefined || config.traceConsole) {
      try {
        telemetryConfig = initializeSimpleTelemetryFromOptions(config);
        appTracer = new SimpleAppTracer(telemetryConfig);
        if (config.verbose) {
          console.error('[DEBUG] Simple tracing initialized');
        }
      } catch (error) {
        if (config.verbose) {
          console.error(`[DEBUG] Failed to initialize tracing: ${error.message}`);
        }
      }
    }

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

    // Process schema if provided (could be file or literal string)
    let schema = null;
    if (config.schema) {
      schema = readInputContent(config.schema);
      if (!schema) {
        console.error('Error: Schema could not be read');
        process.exit(1);
      }
    }

    // Create and configure agent
    const agentConfig = {
      path: config.path,
      promptType: config.prompt,
      customPrompt: systemPrompt,
      allowEdit: config.allowEdit,
      debug: config.verbose,
      tracer: appTracer
    };

    const agent = new ProbeAgent(agentConfig);
    
    // Execute with tracing if available
    let result;
    if (appTracer) {
      const sessionSpan = appTracer.createSessionSpan({
        'question': question.substring(0, 100) + (question.length > 100 ? '...' : ''),
        'path': config.path || process.cwd(),
        'prompt_type': config.prompt || 'code-explorer'
      });
      
      try {
        result = await appTracer.withSpan('agent.answer', 
          () => agent.answer(question, [], { schema }),
          { 'question.length': question.length }
        );
      } finally {
        if (sessionSpan) {
          sessionSpan.end();
        }
      }
    } else {
      result = await agent.answer(question, [], { schema });
    }

    // If schema is provided, make a follow-up request to format the output
    if (schema) {
      if (config.verbose) {
        console.error('[DEBUG] Schema provided, making follow-up request to format output...');
      }
      
      const schemaPrompt = `Now you need to respond according to this schema:\n\n${schema}\n\nPlease reformat your previous response to match this schema exactly. Only return the formatted response, no additional text.`;
      
      try {
        if (appTracer) {
          result = await appTracer.withSpan('agent.schema_formatting',
            () => agent.answer(schemaPrompt, [], { schema }),
            { 'schema.length': schema.length }
          );
        } else {
          result = await agent.answer(schemaPrompt, [], { schema });
        }
        
        // Clean the schema response to remove code blocks and formatting
        const cleaningResult = processSchemaResponse(result, schema, { 
          debug: config.verbose 
        });
        result = cleaningResult.cleaned;
        
        if (config.verbose && cleaningResult.debug && cleaningResult.debug.wasModified) {
          console.error('[DEBUG] Schema response was cleaned:');
          console.error(`  Original length: ${cleaningResult.debug.originalLength}`);
          console.error(`  Cleaned length: ${cleaningResult.debug.cleanedLength}`);
        }

        // Check for mermaid diagrams in response and validate/fix them regardless of schema
        try {
          const mermaidValidationResult = await validateAndFixMermaidResponse(result, {
            debug: config.verbose,
            path: config.path,
            provider: config.provider,
            model: config.model,
            tracer: appTracer
          });

          if (mermaidValidationResult.wasFixed) {
            result = mermaidValidationResult.fixedResponse;
            if (config.verbose) {
              console.error(`[DEBUG] Mermaid diagrams fixed using specialized agent`);
              mermaidValidationResult.fixingResults.forEach((fixResult, index) => {
                if (fixResult.wasFixed) {
                  console.error(`[DEBUG] Fixed diagram ${index + 1}: ${fixResult.originalError}`);
                }
              });
            }
          } else if (!mermaidValidationResult.isValid && mermaidValidationResult.diagrams && mermaidValidationResult.diagrams.length > 0 && config.verbose) {
            console.error(`[DEBUG] Mermaid validation failed: ${mermaidValidationResult.errors?.join(', ')}`);
          }
        } catch (error) {
          if (config.verbose) {
            console.error(`[DEBUG] Enhanced mermaid validation failed: ${error.message}`);
          }
        }

        // Then, if schema expects JSON, validate and retry if invalid
        if (isJsonSchema(schema)) {
          const validation = validateJsonResponse(result);
          if (!validation.isValid) {
            if (config.verbose) {
              console.error(`[DEBUG] JSON validation failed: ${validation.error}`);
              console.error('[DEBUG] Attempting to correct JSON...');
            }
            
            // Retry once with correction prompt
            const correctionPrompt = createJsonCorrectionPrompt(result, schema, validation.error);
            try {
              if (appTracer) {
                result = await appTracer.withSpan('agent.json_correction',
                  () => agent.answer(correctionPrompt, [], { schema }),
                  { 'original_error': validation.error }
                );
              } else {
                result = await agent.answer(correctionPrompt, [], { schema });
              }
              result = cleanSchemaResponse(result);
              
              // Validate again after correction
              const finalValidation = validateJsonResponse(result);
              if (config.verbose) {
                if (finalValidation.isValid) {
                  console.error('[DEBUG] JSON correction successful');
                } else {
                  console.error(`[DEBUG] JSON validation failed after retry: ${finalValidation.error}`);
                }
              }
            } catch (retryError) {
              // If retry fails, keep the original result
              if (config.verbose) {
                console.error(`[DEBUG] JSON correction retry failed: ${retryError.message}`);
              }
            }
          } else if (config.verbose) {
            console.error('[DEBUG] JSON validation passed');
          }
        }
      } catch (error) {
        if (config.verbose) {
          console.error('[DEBUG] Schema formatting failed, using original result');
        }
        // If schema formatting fails, use original result
      }
    }

    // Output the result
    console.log(result);

    // Show token usage in verbose mode
    if (config.verbose) {
      const tokenUsage = agent.getTokenUsage();
      console.error(`\n[DEBUG] Token usage: ${JSON.stringify(tokenUsage, null, 2)}`);
    }

    // Flush and shutdown tracing
    if (appTracer) {
      try {
        await appTracer.flush();
        if (config.verbose) {
          console.error('[DEBUG] Tracing flushed');
        }
      } catch (error) {
        if (config.verbose) {
          console.error(`[DEBUG] Failed to flush tracing: ${error.message}`);
        }
      }
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