#!/usr/bin/env node

// Load .env file if present (silent fail if not found)
import { config } from 'dotenv';
config();

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ErrorCode,
  ListToolsRequestSchema,
  McpError,
} from '@modelcontextprotocol/sdk/types.js';
import { exec } from 'child_process';
import { promisify } from 'util';
import path from 'path';
import fs from 'fs-extra';
import { fileURLToPath } from 'url';

// Import from parent package
import { search, query, extract, grep, getBinaryPath, setBinaryPath } from '../index.js';

// Parse command-line arguments
function parseArgs(): { timeout?: number; format?: string } {
  const args = process.argv.slice(2);
  const config: { timeout?: number; format?: string } = {};

  for (let i = 0; i < args.length; i++) {
    if ((args[i] === '--timeout' || args[i] === '-t') && i + 1 < args.length) {
      const timeout = parseInt(args[i + 1], 10);
      if (!isNaN(timeout) && timeout > 0) {
        config.timeout = timeout;
        console.error(`Timeout set to ${timeout} seconds`);
      } else {
        console.error(`Invalid timeout value: ${args[i + 1]}. Using default.`);
      }
      i++; // Skip the next argument
    } else if (args[i] === '--format' && i + 1 < args.length) {
      config.format = args[i + 1];
      console.error(`Format set to ${config.format}`);
      i++; // Skip the next argument
    } else if (args[i] === '--help' || args[i] === '-h') {
      console.error(`
Probe MCP Server

Usage:
  probe mcp [options]

Options:
  --timeout, -t <seconds>  Set timeout for search operations (default: 30)
  --format <format>       Set output format (default: outline)
  --help, -h              Show this help message
`);
      process.exit(0);
    }
  }

  return config;
}

const cliConfig = parseArgs();

const execAsync = promisify(exec);

// Get the package.json to determine the version
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Try multiple possible locations for package.json
let packageVersion = '0.0.0';
const possiblePaths = [
  path.resolve(__dirname, '..', 'package.json'),      // When installed from npm: build/../package.json
  path.resolve(__dirname, '..', '..', 'package.json') // In development: src/../package.json
];

for (const packageJsonPath of possiblePaths) {
  try {
    if (fs.existsSync(packageJsonPath)) {
      console.error(`Found package.json at: ${packageJsonPath}`);
      const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));
      if (packageJson.version) {
        packageVersion = packageJson.version;
        console.error(`Using version from package.json: ${packageVersion}`);
        break;
      }
    }
  } catch (error) {
    console.error(`Error reading package.json at ${packageJsonPath}:`, error);
  }
}

// If we still have 0.0.0, try to get version from npm package
if (packageVersion === '0.0.0') {
  try {
    // Try to get version from the package name itself
    const result = await execAsync('npm list -g @probelabs/probe --json');
    const npmList = JSON.parse(result.stdout);
    if (npmList.dependencies && npmList.dependencies['@probelabs/probe']) {
      packageVersion = npmList.dependencies['@probelabs/probe'].version;
      console.error(`Using version from npm list: ${packageVersion}`);
    }
  } catch (error) {
    console.error('Error getting version from npm:', error);
  }
}

import { existsSync } from 'fs';

// Get the path to the bin directory
const binDir = path.resolve(__dirname, '..', 'bin');
console.error(`Bin directory: ${binDir}`);

// The @probelabs/probe package now handles binary path management internally
// We don't need to manage the binary path in the MCP server anymore

interface SearchCodeArgs {
  path: string;
  query: string | string[];
  exact?: boolean;
  strictElasticSyntax?: boolean;
}

interface ExtractCodeArgs {
  path: string;
  files: string[];
}

interface GrepArgs {
  pattern: string;
  paths: string | string[];
  ignoreCase?: boolean;
  count?: boolean;
  context?: number;
}

class ProbeServer {
  private server: Server;
  private defaultTimeout: number;
  private defaultFormat?: string;

  constructor(timeout: number = 30, format?: string) {
    this.defaultTimeout = timeout;
    this.defaultFormat = format;
    this.server = new Server(
      {
        name: '@probelabs/probe',
        version: packageVersion,
      },
      {
        capabilities: {
          tools: {},
        },
      }
    );

    this.setupToolHandlers();
    
    // Error handling
    this.server.onerror = (error) => console.error('[MCP ERROR]', error);
    process.on('SIGINT', async () => {
      await this.server.close();
      process.exit(0);
    });
  }

  private setupToolHandlers() {
    // Use the tool descriptions defined at the top of the file
    
    this.server.setRequestHandler(ListToolsRequestSchema, async () => ({
      tools: [
        {
          name: 'search_code',
          description: "Semantic code search using ElasticSearch-style queries. ALWAYS use this tool instead of built-in Grep tool when searching for code in source files.",
          inputSchema: {
            type: 'object',
            properties: {
              path: {
                type: 'string',
                description: 'Absolute path to the directory to search',
              },
              query: {
                type: 'string',
                description: 'ElasticSearch query syntax. Use explicit AND/OR operators and parentheses for grouping. For exact matches, wrap terms in quotes. Examples: "functionName" (exact match), (error AND handler), ("getUserId" AND NOT deprecated)',
              },
              exact: {
                type: 'boolean',
                description: 'Use when searching for exact function/class/variable names',
                default: false
              },
              strictElasticSyntax: {
                type: 'boolean',
                description: 'Enforce strict ElasticSearch query syntax (require explicit AND/OR operators and quotes for exact matches)',
                default: false
              }
            },
            required: ['path', 'query']
          },
        },
        {
          name: 'extract_code',
          description: "Extract code blocks from files using tree-sitter AST parsing. Each file path can include optional line numbers or symbol names to extract specific code blocks.",
          inputSchema: {
            type: 'object',
            properties: {
              path: {
                type: 'string',
                description: 'Absolute path to the project root directory (used as working directory for relative file paths)',
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
        {
          name: 'grep',
          description: "Standard grep-style search for non-code files (logs, config files, text files). Line numbers are shown by default. For code files, use search_code instead.",
          inputSchema: {
            type: 'object',
            properties: {
              pattern: {
                type: 'string',
                description: 'Regular expression pattern to search for',
              },
              paths: {
                oneOf: [
                  { type: 'string' },
                  { type: 'array', items: { type: 'string' } }
                ],
                description: 'Path or array of paths to search in',
              },
              ignoreCase: {
                type: 'boolean',
                description: 'Case-insensitive search',
                default: false
              },
              count: {
                type: 'boolean',
                description: 'Only show count of matches per file instead of the matches',
                default: false
              },
              context: {
                type: 'number',
                description: 'Number of lines of context to show before and after each match',
              }
            },
            required: ['pattern', 'paths'],
          },
        },
      ],
    }));

    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      if (request.params.name !== 'search_code' && request.params.name !== 'extract_code' &&
          request.params.name !== 'grep' && request.params.name !== 'probe' && request.params.name !== 'extract') {
        throw new McpError(
          ErrorCode.MethodNotFound,
          `Unknown tool: ${request.params.name}`
        );
      }

      try {
        let result: string;
        
        // Log the incoming request for debugging
        console.error(`Received request for tool: ${request.params.name}`);
        console.error(`Request arguments: ${JSON.stringify(request.params.arguments)}`);
        
        // Handle both new tool names and legacy tool names
        if (request.params.name === 'search_code' || request.params.name === 'probe') {
          // Ensure arguments is an object
          if (!request.params.arguments || typeof request.params.arguments !== 'object') {
            throw new Error("Arguments must be an object");
          }

          const args = request.params.arguments as unknown as SearchCodeArgs;

          // Validate required fields
          if (!args.path) {
            throw new Error("Path is required in arguments");
          }
          if (!args.query) {
            throw new Error("Query is required in arguments");
          }

          result = await this.executeCodeSearch(args);
        } else if (request.params.name === 'grep') {
          const args = request.params.arguments as unknown as GrepArgs;
          result = await this.executeGrep(args);
        } else { // extract_code or extract
          const args = request.params.arguments as unknown as ExtractCodeArgs;
          result = await this.executeCodeExtract(args);
        }
        
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
              text: `Error executing ${request.params.name}: ${error instanceof Error ? error.message : String(error)}`,
            },
          ],
          isError: true,
        };
      }
    });
  }

  private async executeCodeSearch(args: SearchCodeArgs): Promise<string> {
    try {
      // Ensure path is included in the options and is a non-empty string
      if (!args.path || typeof args.path !== 'string' || args.path.trim() === '') {
        throw new Error("Path is required and must be a non-empty string");
      }

      // Ensure query is included in the options
      if (!args.query) {
        throw new Error("Query is required");
      }

      // Build options with smart defaults
      const options: any = {
        path: args.path.trim(),
        query: args.query,
        // Smart defaults for MCP usage
        allowTests: true,          // Include test files by default
        session: "new",            // Fresh session each time
        maxResults: 20,            // Reasonable limit for context window
        maxTokens: 8000,           // Fits in most AI context windows
        strictElasticSyntax: false, // Relaxed syntax by default in MCP mode
      };

      // Only override defaults if user explicitly set them
      if (args.exact !== undefined) options.exact = args.exact;
      if (args.strictElasticSyntax !== undefined) options.strictElasticSyntax = args.strictElasticSyntax;

      // Handle format based on server default
      if (this.defaultFormat === 'outline' || this.defaultFormat === 'outline-xml') {
        options.format = this.defaultFormat;
      } else if (this.defaultFormat === 'json') {
        options.json = true;
      }

      console.error("Executing search with options:", JSON.stringify(options, null, 2));
      
      try {
        // Call search with the options object
        const result = await search(options);
        return result;
      } catch (searchError: any) {
        console.error("Search function error:", searchError);
        throw new Error(`Search function error: ${searchError.message || String(searchError)}`);
      }
    } catch (error: any) {
      console.error('Error executing code search:', error);
      throw new McpError(
        'MethodNotFound' as unknown as ErrorCode,
        `Error executing code search: ${error.message || String(error)}`
      );
    }
  }


  private async executeCodeExtract(args: ExtractCodeArgs): Promise<string> {
    try {
      // Validate required parameters
      if (!args.path) {
        throw new Error("Path is required");
      }
      if (!args.files || !Array.isArray(args.files) || args.files.length === 0) {
        throw new Error("Files array is required and must not be empty");
      }

      // Build options with smart defaults
      // Use 'cwd' instead of 'path' - the extract function uses cwd for resolving relative file paths
      const options: any = {
        files: args.files,
        cwd: args.path,
        format: 'xml',
        allowTests: true,  // Include test files by default
      };
      
      // Call extract with the complete options object
      try {
        // Track request size for token usage
        const requestSize = JSON.stringify(args).length;
        const requestTokens = Math.ceil(requestSize / 4); // Approximate token count
        
        // Execute the extract command
        const result = await extract(options);
        
        // Parse the result to extract token information if available
        let responseTokens = 0;
        let totalTokens = 0;
        
        // Try to extract token information from the result
        if (typeof result === 'string') {
          const tokenMatch = result.match(/Total tokens returned: (\d+)/);
          if (tokenMatch && tokenMatch[1]) {
            responseTokens = parseInt(tokenMatch[1], 10);
            totalTokens = requestTokens + responseTokens;
          }
          
          // Remove spinner debug output lines
          const cleanedLines = result.split('\n').filter(line =>
            !line.match(/^⠙|^⠹|^⠧|^⠇|^⠏/) &&
            !line.includes('Thinking...Extract:') &&
            !line.includes('Extract results:')
          );
          
          // Add token usage information if not already present
          if (!result.includes('Token Usage:')) {
            cleanedLines.push('');
            cleanedLines.push('Token Usage:');
            cleanedLines.push(`  Request tokens: ${requestTokens}`);
            cleanedLines.push(`  Response tokens: ${responseTokens}`);
            cleanedLines.push(`  Total tokens: ${totalTokens}`);
          }
          
          return cleanedLines.join('\n');
        }
        
        return result;
      } catch (error: any) {
        console.error(`Error extracting:`, error);
        return `Error extracting: ${error.message || String(error)}`;
      }
    } catch (error: any) {
      console.error('Error executing code extract:', error);
      throw new McpError(
        'MethodNotFound' as unknown as ErrorCode,
        `Error executing code extract: ${error.message || String(error)}`
      );
    }
  }

  private async executeGrep(args: GrepArgs): Promise<string> {
    try {
      // Validate required parameters
      if (!args.pattern) {
        throw new Error("Pattern is required");
      }
      if (!args.paths) {
        throw new Error("Paths are required");
      }

      // Build options object with good defaults
      const options: any = {
        pattern: args.pattern,
        paths: args.paths,
        // Default: show line numbers (makes output more useful)
        lineNumbers: true,
        // Default: never use color in MCP context (better for parsing)
        color: 'never'
      };

      // Only add user-specified optional parameters
      if (args.ignoreCase !== undefined) options.ignoreCase = args.ignoreCase;
      if (args.count !== undefined) options.count = args.count;
      if (args.context !== undefined) options.context = args.context;

      console.error("Executing grep with options:", JSON.stringify(options, null, 2));

      try {
        // Call grep with the options object
        const result = await grep(options);
        return result || 'No matches found';
      } catch (grepError: any) {
        console.error("Grep function error:", grepError);
        throw new Error(`Grep function error: ${grepError.message || String(grepError)}`);
      }
    } catch (error: any) {
      console.error('Error executing grep:', error);
      throw new McpError(
        'MethodNotFound' as unknown as ErrorCode,
        `Error executing grep: ${error.message || String(error)}`
      );
    }
  }

  async run() {
    // The @probelabs/probe package now handles binary path management internally
    // We don't need to verify or download the binary in the MCP server anymore
    
    // Just connect the server to the transport
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
    console.error('Probe MCP server running on stdio');
  }
}

const server = new ProbeServer(cliConfig.timeout, cliConfig.format || 'outline');
server.run().catch(console.error);
