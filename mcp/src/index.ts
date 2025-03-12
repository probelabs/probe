#!/usr/bin/env node
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
import { downloadProbeBinary } from './downloader.js';
import fs from 'fs-extra';
import { fileURLToPath } from 'url';

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
      console.log(`Found package.json at: ${packageJsonPath}`);
      const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));
      if (packageJson.version) {
        packageVersion = packageJson.version;
        console.log(`Using version from package.json: ${packageVersion}`);
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
    const result = await execAsync('npm list -g @buger/probe-mcp --json');
    const npmList = JSON.parse(result.stdout);
    if (npmList.dependencies && npmList.dependencies['@buger/probe-mcp']) {
      packageVersion = npmList.dependencies['@buger/probe-mcp'].version;
      console.log(`Using version from npm list: ${packageVersion}`);
    }
  } catch (error) {
    console.error('Error getting version from npm:', error);
  }
}

import { existsSync } from 'fs';

// Get the path to the bin directory
const binDir = path.resolve(__dirname, '..', 'bin');
console.log(`Bin directory: ${binDir}`);

// Path to the probe binary (will be set after download)
let PROBE_PATH = process.env.PROBE_PATH || '';

// Check if the binary exists at the environment variable path
if (PROBE_PATH && !existsSync(PROBE_PATH)) {
  console.warn(`Warning: PROBE_PATH environment variable set to ${PROBE_PATH}, but no binary found at that location.`);
  PROBE_PATH = '';
}

// Ensure the bin directory exists
try {
  fs.ensureDirSync(binDir);
} catch (error) {
  console.error(`Error creating bin directory: ${error}`);
}

interface SearchCodeArgs {
  path: string;
  query: string | string[];
  filesOnly?: boolean;
  ignore?: string[];
  excludeFilenames?: boolean;
  reranker?: 'hybrid' | 'hybrid2' | 'bm25' | 'tfidf';
  frequencySearch?: boolean;
  exact?: boolean;
  maxResults?: number;
  maxBytes?: number;
  maxTokens?: number;
  allowTests?: boolean;
  anyTerm?: boolean;
  noMerge?: boolean;
  mergeThreshold?: number;
}

interface ExtractCodeArgs {
  files: string[];
  allowTests?: boolean;
  contextLines?: number;
  format?: 'markdown' | 'plain' | 'json';
}

class ProbeServer {
  private server: Server;

  constructor() {
    this.server = new Server(
      {
        name: '@buger/probe-mcp',
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
    this.server.onerror = (error) => console.error('[MCP Error]', error);
    process.on('SIGINT', async () => {
      await this.server.close();
      process.exit(0);
    });
  }

  private setupToolHandlers() {
    this.server.setRequestHandler(ListToolsRequestSchema, async () => ({
      tools: [
        {
          name: 'search_code',
          description: 'Search code in a specified directory using Elasticsearch-like query syntax. \n\nThe search tool supports Elasticsearch-like query syntax with the following features:\n- Basic term searching: "config" or "search"\n- Field-specific searching: "field:value" (e.g., "function:parse")\n- Required terms with + prefix: "+required"\n- Excluded terms with - prefix: "-excluded"\n- Logical operators: "term1 AND term2", "term1 OR term2"\n- Grouping with parentheses: "(term1 OR term2) AND term3"\n\nExamples:\n- Simple search: "config"\n- Required and excluded terms: "+parse -test"\n- Field-specific: "function:evaluate"\n- Complex query: "(parse OR tokenize) AND query"\n\nWhen using search tool:\n- Try simpler queries (e.g. use \'rpc\' instead of \'rpc layer implementation\')\n- This tool knows how to do the stemming by itself, put only unique keywords to query\n- Focus on keywords that would appear in code\n- Split distinct terms into separate searches, unless they should be search together, e.g. how they connect\n- Use multiple probe tool calls if needed\n- If you can\'t find what you want after multiple attempts, ask the user for more context\n- While doing multiple calls, do not repeat the same queries\n\nElasticsearch-like Query Syntax Details:\n- Terms are case-insensitive and automatically stemmed (e.g., "parsing" matches "parse")\n- Use quotes for exact phrases: "white list" (matches the exact phrase)\n- Use + for required terms: +config (must be present)\n- Use - for excluded terms: -test (must not be present)\n- Use field specifiers: function:parse (search in specific code elements)\n- Combine with AND/OR: config AND (parse OR tokenize)\n- Group with parentheses for complex expressions\n\nQueries can be any text (including multi-word phrases like "IP whitelist"), but simple, focused queries typically yield better results. Use the maxResults parameter to limit the number of results when needed. For multi-term queries, all terms must be present in a file by default, but you can use anyTerm=true to match files containing any of the terms.',
          inputSchema: {
            type: 'object',
            properties: {
              path: {
                type: 'string',
                description: 'Absolute path to the directory to search in (e.g., "/Users/username/projects/myproject"). Using absolute paths ensures reliable search results regardless of the current working directory.',
              },
              query: {
                oneOf: [
                  { type: 'string' },
                  { type: 'array', items: { type: 'string' } }
                ],
                description: 'Query patterns to search for with Elasticsearch-like syntax support. Supports logical operators (AND, OR), required (+) and excluded (-) terms, and grouping with parentheses. Examples: "config", "+required -excluded", "(term1 OR term2) AND term3". For multiple terms, provide either a space-separated string ("term1 term2") or an array of strings (["term1", "term2"]). By default, all terms must be present in a file unless anyTerm=true is specified.',
              },
              filesOnly: {
                type: 'boolean',
                description: 'Skip AST parsing and just output unique files',
              },
              ignore: {
                type: 'array',
                items: { type: 'string' },
                description: 'Custom patterns to ignore (in addition to .gitignore and common patterns)',
              },
              excludeFilenames: {
                type: 'boolean',
                description: 'Exclude filenames from being used for matching (filename matching is enabled by default and adds filename tokens during tokenization)',
              },
              reranker: {
                type: 'string',
                enum: ['hybrid', 'hybrid2', 'bm25', 'tfidf'],
                description: 'Reranking method to use for search results',
              },
              frequencySearch: {
                type: 'boolean',
                description: 'Use frequency-based search with stemming and stopword removal (enabled by default)',
              },
              exact: {
                type: 'boolean',
                description: 'Use exact matching without stemming or stopword removal (overrides frequencySearch)',
              },
              maxResults: {
                type: 'number',
                description: 'Maximum number of results to return',
              },
              maxBytes: {
                type: 'number',
                description: 'Maximum total bytes of code content to return',
              },
              maxTokens: {
                type: 'number',
                description: 'Maximum total tokens in code content to return (for AI usage). Default: 40000',
                default: 40000
              },
              allowTests: {
                type: 'boolean',
                description: 'Allow test files and test code blocks in search results (disabled by default)',
              },
              anyTerm: {
                type: 'boolean',
                description: 'Match files that contain any of the search terms (by default, files must contain all terms)',
              },
              noMerge: {
                type: 'boolean',
                description: 'Disable merging of adjacent code blocks after ranking (merging enabled by default)',
              },
              mergeThreshold: {
                type: 'number',
                description: 'Maximum number of lines between code blocks to consider them adjacent for merging (default: 5)',
              },
            },
            required: ['path', 'query'],
          },
        },
        {
          name: 'extract_code',
          description: 'Extract code blocks from files based on file paths and optional line numbers. \n\nThis tool uses tree-sitter to find the closest suitable parent node (function, struct, class, etc.) for a specified line. When a line number is provided, it extracts the entire code block containing that line. If no line number is specified, it extracts the entire file.\n\nUse this tool when you need to:\n- Extract a specific function, class, or method from a file\n- Get the full context around a particular line of code\n- Understand the structure and implementation of a specific code element\n- Extract an entire file when you need its complete content\n\nThe extracted code maintains proper syntax highlighting based on the file extension and includes information about the type of code block (function, class, method, etc.).\n\nExamples:\n- Extract a function at line 42: "/path/to/file.rs:42"\n- Extract an entire file: "/path/to/file.rs"\n- Extract with context lines: "/path/to/file.rs:42" with contextLines=5',
          inputSchema: {
            type: 'object',
            properties: {
              files: {
                type: 'array',
                items: { type: 'string' },
                description: 'Files to extract from (can include line numbers with colon, e.g., "/path/to/file.rs:10"). Each entry should be an absolute path to ensure reliable extraction.',
              },
              allowTests: {
                type: 'boolean',
                description: 'Allow test files and test code blocks in results (disabled by default)',
              },
              contextLines: {
                type: 'number',
                description: 'Number of context lines to include before and after the extracted block when AST parsing fails to find a suitable node',
                default: 0
              },
              format: {
                type: 'string',
                enum: ['markdown', 'plain', 'json'],
                description: 'Output format for the extracted code',
                default: 'markdown'
              },
            },
            required: ['files'],
          },
        },
      ],
    }));

    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      if (request.params.name !== 'probe' && request.params.name !== 'extract') {
        throw new McpError(
          ErrorCode.MethodNotFound,
          `Unknown tool: ${request.params.name}`
        );
      }

      try {
        let result: string;
        
        if (request.params.name === 'probe') {
          const args = request.params.arguments as unknown as SearchCodeArgs;
          result = await this.executeCodeSearch(args);
        } else { // extract
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
    // Build the command arguments
    const cliArgs: string[] = [];
 
    // Add optional arguments
    if (args.filesOnly) {
      cliArgs.push('--files-only');
    }
    
    if (args.ignore && args.ignore.length > 0) {
      for (const ignorePattern of args.ignore) {
        cliArgs.push('--ignore', ignorePattern);
      }
    }
    
    if (args.excludeFilenames) {
      cliArgs.push('--exclude-filenames');
    }
    
    if (args.reranker) {
      cliArgs.push('--reranker', args.reranker);
    }
    
    if (args.frequencySearch) {
      cliArgs.push('--frequency');
    }
    
    if (args.exact) {
      cliArgs.push('--exact');
    }
    
    if (args.maxResults !== undefined) {
      cliArgs.push('--max-results', args.maxResults.toString());
    }
    
    if (args.maxBytes !== undefined) {
      cliArgs.push('--max-bytes', args.maxBytes.toString());
    }
    
    // Always set max-tokens to 40000 by default if not specified
    const maxTokens = args.maxTokens !== undefined ? args.maxTokens : 40000;
    cliArgs.push('--max-tokens', maxTokens.toString());
    
    if (args.allowTests) {
      cliArgs.push('--allow-tests');
    }
    
    if (args.anyTerm) {
      cliArgs.push('--any-term');
    }
    
    // Add new options
    if (args.noMerge) {
      cliArgs.push('--no-merge');
    }
    
    if (args.mergeThreshold !== undefined) {
      cliArgs.push('--merge-threshold', args.mergeThreshold.toString());
    }
    
   // Add query as the first positional argument (can be string or array)
   const queries = Array.isArray(args.query) ? args.query : [args.query];
   // Use the first query as the main pattern (positional argument)
   if (queries.length > 0) {
     // Wrap query in quotes to handle multi-word queries
     cliArgs.push(`"${queries[0]}"`);
   }
   
   // Add path as the second positional argument (not with --paths flag)
   cliArgs.push(`"${args.path}"`);
   
    // Execute the command
    const command = `${PROBE_PATH} search ${cliArgs.join(' ')}`;
    console.log(`Executing command: ${command}`);
    
    try {
      const { stdout, stderr } = await execAsync(command);
      
      if (stderr) {
        console.error(`stderr: ${stderr}`);
      }
      
      return stdout;
    } catch (error) {
      console.error('Error executing probe CLI:', error);
      throw error;
    }
  }

  private async executeCodeExtract(args: ExtractCodeArgs): Promise<string> {
    // Build the command arguments
    const cliArgs: string[] = [];
    
    // Add optional arguments
    if (args.allowTests) {
      cliArgs.push('--allow-tests');
    }
    
    if (args.contextLines !== undefined) {
      cliArgs.push('--context', args.contextLines.toString());
    }
    
    if (args.format) {
      cliArgs.push('--format', args.format);
    }
    
    // Add files as positional arguments
    for (const file of args.files) {
      cliArgs.push(`"${file}"`);
    }
    
    // Execute the command
    const command = `${PROBE_PATH} extract ${cliArgs.join(' ')}`;
    console.log(`Executing command: ${command}`);
    
    try {
      const { stdout, stderr } = await execAsync(command);
      
      if (stderr) {
        console.error(`stderr: ${stderr}`);
      }
      
      return stdout;
    } catch (error) {
      console.error('Error executing probe extract:', error);
      throw error;
    }
  }

  async run() {
    // Download the probe binary before starting the server
    if (!PROBE_PATH) {
      try {
        console.log(`Downloading probe binary (version: ${packageVersion})...`);
        
        // Download the binary
        PROBE_PATH = await downloadProbeBinary(packageVersion);
        console.log(`Successfully downloaded probe binary to: ${PROBE_PATH}`);
      } catch (error) {
        console.error('Error downloading probe binary:', error);
        
        // Provide more detailed error information and suggestions
        if (error instanceof Error) {
          if (error.message.includes('404')) {
            console.error(`Version "${packageVersion}" not found in the repository.`);
            console.error('Expected version format: x.y.z (e.g., 1.2.3)');
            console.error('Suggestions:');
            console.error('1. Check if the version in package.json is correct');
            console.error(`2. Verify that a release with tag v${packageVersion} exists in the repository`);
          } else if (error.message.includes('network')) {
            console.error('Network error occurred while downloading the binary.');
            console.error('Suggestions:');
            console.error('1. Check your internet connection');
            console.error('2. Verify that GitHub API is accessible from your network');
          } else if (error.message.includes('permission') || error.message.includes('EACCES')) {
            console.error('Permission error occurred while downloading or extracting the binary.');
            console.error('Suggestions:');
            console.error('1. Check if you have write permissions to the bin directory');
            console.error('2. Try running the command with elevated privileges');
          } else if (error.message.includes('not found in the archive')) {
            console.error('Binary extraction failed - could not find the binary in the downloaded archive.');
            console.error('Suggestions:');
            console.error('1. Check if the release archive contains the binary in the expected format');
            console.error('2. Try downloading a different version');
          } else {
            console.error(`Error details: ${error.message}`);
          }
        }
        
        // Look for existing binary in the bin directory as a fallback
        const isWindows = process.platform === 'win32';
        const binaryName = isWindows ? 'probe.exe' : 'probe';
        const localBinaryPath = path.join(binDir, binaryName);
        
        if (fs.existsSync(localBinaryPath)) {
          console.log(`Found existing binary in bin directory: ${localBinaryPath}`);
          PROBE_PATH = localBinaryPath;
        }
        // Check if PROBE_PATH environment variable is set as a fallback
        else if (process.env.PROBE_PATH) {
          console.log(`Falling back to environment variable PROBE_PATH: ${process.env.PROBE_PATH}`);
          PROBE_PATH = process.env.PROBE_PATH;
          
          // Verify the binary exists
          if (!fs.existsSync(PROBE_PATH)) {
            console.error(`Error: Binary not found at ${PROBE_PATH}`);
            process.exit(1);
          }
        } else {
          console.error('No probe binary available. Please set PROBE_PATH environment variable or fix the download issue.');
          process.exit(1);
        }
      }
    } else {
      console.log(`Using probe binary from environment variable: ${PROBE_PATH}`);
    }
    
    // Verify the binary is executable
    try {
      // Make sure the binary is executable (on non-Windows platforms)
      if (process.platform !== 'win32') {
        try {
          await fs.chmod(PROBE_PATH, 0o755);
          console.log(`Made binary executable: ${PROBE_PATH}`);
        } catch (err) {
          console.warn(`Warning: Could not set executable permissions on binary: ${err}`);
        }
      }
      
      // Test the binary
      const { stdout } = await execAsync(`${PROBE_PATH} --version`);
      console.log(`Probe binary version: ${stdout.trim()}`);
    } catch (error) {
      console.error(`Error executing probe binary: ${error instanceof Error ? error.message : String(error)}`);
      console.error('Please ensure the binary is executable and valid.');
      process.exit(1);
    }
    
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
    console.error('Probe MCP server running on stdio');
  }
}

const server = new ProbeServer();
server.run().catch(console.error);
