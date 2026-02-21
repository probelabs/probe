// Load .env file if present (silent fail if not found)
import dotenv from 'dotenv';
dotenv.config();

import { ProbeAgent } from './ProbeAgent.js';
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ErrorCode,
  InitializeRequestSchema,
  ListToolsRequestSchema,
  McpError,
} from '@modelcontextprotocol/sdk/types.js';
import { readFileSync, existsSync } from 'fs';
import { resolve } from 'path';
import { extract } from '../index.js';
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
    allowedFolders: null,
    prompt: null,
    systemPrompt: null,
    architectureFileName: null,
    schema: null,
    provider: null,
    model: null,
    allowEdit: process.env.ALLOW_EDIT === '1' || false,
    enableDelegate: false,
    verbose: false,
    help: false,
    maxIterations: null,
    maxResponseTokens: null,
    traceFile: undefined,
    traceRemote: undefined,
    traceConsole: false,
    useStdin: false, // New flag to indicate stdin should be used
    outline: false, // New flag to enable outline format
    noMermaidValidation: false, // New flag to disable mermaid validation
    allowedTools: null, // Tool filtering: ['*'] = all, [] = none, ['tool1', 'tool2'] = specific
    disableTools: false, // Convenience flag to disable all tools
    allowSkills: false, // Enable skill discovery and activation (disabled by default)
    skillDirs: null, // Comma-separated list of repo-relative skill directories
    // Task management
    enableTasks: false, // Enable task tracking for progress management
    // Execute plan DSL tool
    enableExecutePlan: false,
    // Bash tool configuration
    enableBash: false,
    bashAllow: null,
    bashDeny: null,
    bashTimeout: null,
    bashWorkingDir: null,
    disableDefaultBashAllow: false,
    disableDefaultBashDeny: false
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
    } else if (arg === '--enable-delegate') {
      config.enableDelegate = true;
    } else if (arg === '--no-delegate') {
      config.enableDelegate = false; // Explicitly disable delegation (used by subagents)
    } else if (arg === '--path' && i + 1 < args.length) {
      config.path = args[++i];
    } else if (arg === '--allowed-folders' && i + 1 < args.length) {
      config.allowedFolders = args[++i].split(',').map(dir => dir.trim());
    } else if (arg === '--prompt' && i + 1 < args.length) {
      config.prompt = args[++i];
    } else if (arg === '--system-prompt' && i + 1 < args.length) {
      config.systemPrompt = args[++i];
    } else if (arg === '--architecture-file' && i + 1 < args.length) {
      config.architectureFileName = args[++i];
    } else if (arg === '--schema' && i + 1 < args.length) {
      config.schema = args[++i];
    } else if (arg === '--provider' && i + 1 < args.length) {
      config.provider = args[++i];
    } else if (arg === '--model' && i + 1 < args.length) {
      config.model = args[++i];
    } else if (arg === '--max-iterations' && i + 1 < args.length) {
      config.maxIterations = parseInt(args[++i], 10);
    } else if (arg === '--max-response-tokens' && i + 1 < args.length) {
      config.maxResponseTokens = parseInt(args[++i], 10);
    } else if (arg === '--trace-file' && i + 1 < args.length) {
      config.traceFile = args[++i];
    } else if (arg === '--trace-remote' && i + 1 < args.length) {
      config.traceRemote = args[++i];
    } else if (arg === '--trace-console') {
      config.traceConsole = true;
    } else if (arg === '--outline') {
      config.outline = true;
    } else if (arg === '--no-mermaid-validation') {
      config.noMermaidValidation = true;
    } else if (arg === '--allowed-tools' && i + 1 < args.length) {
      // Parse allowed tools: comma-separated list or special values
      const toolsArg = args[++i];
      if (toolsArg === '*' || toolsArg === 'all') {
        config.allowedTools = ['*'];
      } else if (toolsArg === 'none' || toolsArg === '') {
        config.allowedTools = [];
      } else {
        config.allowedTools = toolsArg.split(',').map(t => t.trim()).filter(t => t.length > 0);
      }
    } else if (arg === '--disable-tools') {
      // Convenience flag to disable all tools (raw AI mode)
      config.disableTools = true;
    } else if (arg === '--allow-skills') {
      config.allowSkills = true;
    } else if (arg === '--skills-dir' && i + 1 < args.length) {
      config.skillDirs = args[++i].split(',').map(dir => dir.trim()).filter(Boolean);
    } else if (arg === '--allow-tasks') {
      config.enableTasks = true;
    } else if (arg === '--enable-execute-plan') {
      config.enableExecutePlan = true;
    } else if (arg === '--enable-bash') {
      config.enableBash = true;
    } else if (arg === '--bash-allow' && i + 1 < args.length) {
      config.bashAllow = args[++i];
    } else if (arg === '--bash-deny' && i + 1 < args.length) {
      config.bashDeny = args[++i];
    } else if (arg === '--bash-timeout' && i + 1 < args.length) {
      config.bashTimeout = args[++i];
    } else if (arg === '--bash-working-dir' && i + 1 < args.length) {
      config.bashWorkingDir = args[++i];
    } else if (arg === '--no-default-bash-allow') {
      config.disableDefaultBashAllow = true;
    } else if (arg === '--no-default-bash-deny') {
      config.disableDefaultBashDeny = true;
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
  console.error(`
probe agent - AI-powered code exploration tool

Usage:
  probe agent <question>           Answer a question about the codebase
  probe agent <file>               Read question from file
  echo "question" | probe agent    Read question from stdin (pipe input)
  probe agent --mcp                Start as MCP server
  probe agent --acp                Start as ACP server

Options:
  --path <dir>                     Search directory (default: current)
  --allowed-folders <dirs>         Comma-separated list of allowed directories for file operations
  --prompt <type>                  Persona: code-explorer, engineer, code-review, support, architect
  --system-prompt <text|file>      Custom system prompt (text or file path)
  --architecture-file <name>       Architecture context filename in repo root (defaults to AGENTS.md with CLAUDE.md fallback; ARCHITECTURE.md is always included when present)
  --schema <schema|file>           Output schema (JSON, XML, any format - text or file path)
  --provider <name>                Force AI provider: anthropic, openai, google
  --model <name>                   Override model name
  --allow-edit                     Enable code modification capabilities (edit + create tools)
  --enable-delegate                Enable delegate tool for task distribution to subagents
  --allowed-tools <tools>          Filter available tools (comma-separated list)
                                   Use '*' or 'all' for all tools (default)
                                   Use 'none' or '' for no tools (raw AI mode)
                                   Specific tools: search,query,extract,edit,create,listFiles,searchFiles,listSkills,useSkill
                                   Supports exclusion: '*,!bash' (all except bash)
  --disable-tools                  Disable all tools (raw AI mode, no code analysis)
                                   Convenience flag equivalent to --allowed-tools none
  --allow-skills                   Enable skill discovery and activation (disabled by default)
  --skills-dir <dirs>              Comma-separated list of repo-relative skill directories to scan
  --allow-tasks                    Enable task management for tracking multi-step progress
  --verbose                        Enable verbose output
  --outline                        Use outline-xml format for code search results
  --mcp                           Run as MCP server
  --acp                           Run as ACP server (Agent Client Protocol)
  --max-iterations <number>        Max tool iterations (default: 30)
  --max-response-tokens <number>   Max tokens for AI response (overrides model defaults)
  --trace-file <path>              Enable tracing to file (JSONL format)
  --trace-remote <endpoint>        Enable tracing to remote OTLP endpoint
  --trace-console                  Enable tracing to console output
  --no-mermaid-validation          Disable automatic mermaid diagram validation and fixing
  --help, -h                      Show this help message

DSL Orchestration:
  --enable-execute-plan            Enable execute_plan DSL tool for programmatic orchestration

Bash Tool Options:
  --enable-bash                    Enable bash command execution for system exploration
  --bash-allow <patterns>          Additional bash command patterns to allow (comma-separated)
  --bash-deny <patterns>           Additional bash command patterns to deny (comma-separated)
  --no-default-bash-allow          Disable default bash allow list (use only custom patterns)
  --no-default-bash-deny           Disable default bash deny list (use only custom patterns)  
  --bash-timeout <ms>              Bash command timeout in milliseconds (default: 120000)
  --bash-working-dir <path>        Default working directory for bash commands

Environment Variables:
  ANTHROPIC_API_KEY               Anthropic Claude API key
  OPENAI_API_KEY                  OpenAI GPT API key
  GOOGLE_API_KEY                  Google Gemini API key
  FORCE_PROVIDER                  Force specific provider (anthropic, openai, google)
  MODEL_NAME                      Override model name
  MAX_RESPONSE_TOKENS             Maximum tokens for AI response
  ALLOW_EDIT                      Enable code modification (set to '1')
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
  probe agent "Explain this code" --allowed-tools search,extract  # Only search and extract
  probe agent "What is this project about?" --allowed-tools none  # Raw AI mode (no tools)
  probe agent "Tell me about this project" --disable-tools        # Raw AI mode (convenience flag)
  probe agent "Fix the off-by-one error" --allow-edit --path ./src  # Enable code editing
  ALLOW_EDIT=1 probe agent "Refactor the login flow"                # Edit via env var
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
        name: '@probelabs/probe agent',
        version: '1.0.0',
      },
      {
        capabilities: {
          tools: {},
        },
      }
    );

    // Don't initialize AI agent on startup - lazy initialize when needed
    this.agent = null;

    this.setupToolHandlers();
    this.server.onerror = (error) => console.error('[MCP ERROR]', error);
    process.on('SIGINT', async () => {
      await this.server.close();
      process.exit(0);
    });
  }

  setupToolHandlers() {
    // Handle MCP initialize request
    this.server.setRequestHandler(InitializeRequestSchema, async (request) => {
      return {
        protocolVersion: '2024-11-05',
        capabilities: {
          tools: {},
        },
        serverInfo: {
          name: '@probelabs/probe agent',
          version: '1.0.0',
        },
      };
    });

    this.server.setRequestHandler(ListToolsRequestSchema, async () => ({
      tools: [
        {
          name: 'search_code',
          description: "AI agent that answers free-form questions about codebases. Ask detailed questions in natural language.",
          inputSchema: {
            type: 'object',
            properties: {
              query: {
                type: 'string',
                description: 'A detailed, free-form question about the codebase in natural language. Be specific and descriptive. Example: "How does the authentication system work and where is user session management implemented?"',
              },
              path: {
                type: 'string',
                description: 'Absolute path to the directory to search in (e.g., "/Users/username/projects/myproject").',
              },
              allowed_folders: {
                type: 'array',
                items: { type: 'string' },
                description: 'Optional list of allowed directories for file operations. Defaults to current directory if not specified.',
              },
              prompt: {
                type: 'string',
                description: 'Optional persona type: code-explorer, engineer, code-review, support, architect.',
              },
              system_prompt: {
                type: 'string',
                description: 'Optional custom system prompt (text or file path).',
              },
              architecture_file: {
                type: 'string',
                description: 'Optional architecture context filename in repo root (defaults to AGENTS.md with CLAUDE.md fallback; ARCHITECTURE.md is always included when present).',
              },
              enable_tasks: {
                type: 'boolean',
                description: 'Optional: Enable task management for tracking multi-step progress. When enabled, the agent can create, track, and complete tasks.',
                default: false
              }
            },
            required: ['query']
          },
        },
        {
          name: 'extract_code',
          description: "Extract full code blocks from files using tree-sitter AST parsing. Use this to get complete code content based on file paths and symbols returned by search_code. Each file path can include optional line numbers or symbol names to extract specific code blocks.",
          inputSchema: {
            type: 'object',
            properties: {
              path: {
                type: 'string',
                description: 'Absolute path to the project root directory (used as working directory for relative file paths).',
              },
              files: {
                type: 'array',
                items: { type: 'string' },
                description: 'Array of file paths to extract from. Formats: "file.js" (entire file), "file.js:42" (code block at line 42), "file.js:10-20" (lines 10-20), "file.js#funcName" (specific symbol). Line numbers and symbols are part of the path string, not separate parameters. Paths can be absolute or relative to the project directory.',
              }
            },
            required: ['path', 'files'],
          },
        },
      ],
    }));

    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      if (request.params.name !== 'search_code' && request.params.name !== 'extract_code') {
        throw new McpError(
          ErrorCode.MethodNotFound,
          `Unknown tool: ${request.params.name}`
        );
      }

      try {
        const args = request.params.arguments;

        // Handle extract_code tool
        if (request.params.name === 'extract_code') {
          // Validate required parameters
          if (!args.path) {
            throw new Error("Path is required");
          }
          if (!args.files || !Array.isArray(args.files) || args.files.length === 0) {
            throw new Error("Files array is required and must not be empty");
          }

          // Build options with smart defaults
          // Use 'cwd' instead of 'path' - the extract function uses cwd for resolving relative file paths
          const options = {
            files: args.files,
            cwd: args.path,
            format: 'xml',
            allowTests: true,  // Include test files by default
          };

          if (process.env.DEBUG === '1') {
            console.error('[DEBUG] Executing extract_code with options:', JSON.stringify(options, null, 2));
          }

          // Execute the extract command
          const result = await extract(options);

          return {
            content: [
              {
                type: 'text',
                text: result,
              },
            ],
          };
        }

        // Handle search_code tool
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

        // Lazy initialize AI agent only when tool is called
        if (!this.agent) {
          if (process.env.DEBUG === '1') {
            console.error('[DEBUG] Initializing AI agent on first MCP tool call');
          }
          
          // Create agent with configuration
          const agentConfig = {
            path: args.path || (args.allowed_folders && args.allowed_folders[0]) || process.cwd(),
            promptType: args.prompt || 'code-explorer',
            customPrompt: systemPrompt,
            architectureFileName: args.architecture_file,
            provider: args.provider,
            model: args.model,
            allowEdit: !!args.allow_edit,
            debug: process.env.DEBUG === '1',
            maxResponseTokens: args.max_response_tokens,
            disableMermaidValidation: !!args.no_mermaid_validation,
            allowedTools: args.allowed_tools,
            disableTools: args.disable_tools,
            enableTasks: !!args.enable_tasks
          };

          this.agent = new ProbeAgent(agentConfig);
          // Initialize MCP if enabled
          await this.agent.initialize();
        }

        const agent = this.agent;
        let result = await agent.answer(query, [], { schema });

        // If schema is provided, make a follow-up request to format the output
        if (schema) {
          const schemaPrompt = `Now you need to respond according to this schema:\n\n${schema}\n\nPlease reformat your previous response to match this schema exactly. Only return the formatted response, no additional text.`;
          
          try {
            result = await agent.answer(schemaPrompt, [], { schema });
            // Clean the schema response to remove code blocks and formatting
            result = cleanSchemaResponse(result);

            // Check for mermaid diagrams in response and validate/fix them regardless of schema
            if (!args.no_mermaid_validation) {
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
            } else if (args.debug) {
              console.error(`[DEBUG] Mermaid validation skipped due to --no-mermaid-validation flag`);
            }

            // Then, if schema expects JSON, validate and retry if invalid
            if (isJsonSchema(schema)) {
              const validation = validateJsonResponse(result);
              if (!validation.isValid) {
                // Retry once with correction prompt
                const correctionPrompt = createJsonCorrectionPrompt(result, schema, validation.error);
                try {
                  result = await agent.answer(correctionPrompt, [], { schema, _schemaFormatted: true, _disableTools: true });
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
        const tokenUsage = this.agent.getTokenUsage();
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
        console.error(`Error executing ${request.params.name}:`, error);
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
      enableDelegate: config.enableDelegate,
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

    // Process bash configuration
    let bashConfig = null;
    if (config.enableBash) {
      bashConfig = {};
      
      // Parse allow patterns
      if (config.bashAllow) {
        bashConfig.allow = config.bashAllow.split(',').map(p => p.trim()).filter(p => p.length > 0);
      }
      
      // Parse deny patterns
      if (config.bashDeny) {
        bashConfig.deny = config.bashDeny.split(',').map(p => p.trim()).filter(p => p.length > 0);
      }
      
      // Handle default list flags
      if (config.disableDefaultBashAllow) {
        bashConfig.disableDefaultAllow = true;
      }
      
      if (config.disableDefaultBashDeny) {
        bashConfig.disableDefaultDeny = true;
      }
      
      // Parse timeout
      if (config.bashTimeout) {
        const timeout = parseInt(config.bashTimeout, 10);
        if (isNaN(timeout) || timeout < 1000) {
          console.error('Error: Bash timeout must be a number >= 1000 milliseconds');
          process.exit(1);
        }
        bashConfig.timeout = timeout;
      }
      
      // Set working directory
      if (config.bashWorkingDir) {
        if (!existsSync(config.bashWorkingDir)) {
          console.error(`Error: Bash working directory does not exist: ${config.bashWorkingDir}`);
          process.exit(1);
        }
        bashConfig.workingDirectory = config.bashWorkingDir;
      }
      
      if (config.verbose) {
        console.error('Bash command execution enabled');
      }
    }

    // Create and configure agent
    const agentConfig = {
      path: config.path,
      allowedFolders: config.allowedFolders,
      promptType: config.prompt,
      customPrompt: systemPrompt,
      architectureFileName: config.architectureFileName,
      allowEdit: config.allowEdit,
      enableDelegate: config.enableDelegate,
      debug: config.verbose,
      tracer: appTracer,
      outline: config.outline,
      maxResponseTokens: config.maxResponseTokens,
      disableMermaidValidation: config.noMermaidValidation,
      allowedTools: config.allowedTools,
      disableTools: config.disableTools,
      allowSkills: config.allowSkills,
      skillDirs: config.skillDirs,
      enableExecutePlan: config.enableExecutePlan,
      enableBash: config.enableBash,
      bashConfig: bashConfig,
      enableTasks: config.enableTasks
    };

    const agent = new ProbeAgent(agentConfig);
    // Initialize MCP if enabled
    await agent.initialize();

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
        if (!config.noMermaidValidation) {
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
        } else if (config.verbose) {
          console.error(`[DEBUG] Mermaid validation skipped due to --no-mermaid-validation flag`);
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
                  () => agent.answer(correctionPrompt, [], { schema, _schemaFormatted: true, _disableTools: true }),
                  { 'original_error': validation.error }
                );
              } else {
                result = await agent.answer(correctionPrompt, [], { schema, _schemaFormatted: true, _disableTools: true });
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

    // Output the result (strip <result> tags if present for cleaner CLI output)
    const resultMatch = result.match(/<result>([\s\S]*?)<\/result>/);
    if (resultMatch) {
      console.log(resultMatch[1].trim());
    } else {
      console.log(result);
    }

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
