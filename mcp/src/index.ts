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
const packageJsonPath = path.resolve(__dirname, '..', '..', 'package.json');
let packageVersion = '0.0.0';

try {
  if (fs.existsSync(packageJsonPath)) {
    const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));
    packageVersion = packageJson.version || '0.0.0';
  }
} catch (error) {
  console.error('Error reading package.json:', error);
}

// Path to the probe binary (will be set after download)
let PROBE_PATH = process.env.PROBE_PATH || '';

interface SearchCodeArgs {
  path: string;
  query: string | string[];
  filesOnly?: boolean;
  ignore?: string[];
  includeFilenames?: boolean;
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
          description: 'Search code in a specified directory. Use this tool whenever you need to search the codebase to understand code structure, find implementations, or identify patterns. Queries can be any text (including multi-word phrases like "IP whitelist"), but simple, focused queries typically yield better results. Use the maxResults parameter to limit the number of results when needed. For multi-term queries, all terms must be present in a file by default, but you can use anyTerm=true to match files containing any of the terms.',
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
                description: 'Query patterns to search for (string or array of strings). Can be keywords, phrases, or regex patterns. For multiple terms, provide either a space-separated string ("term1 term2") or an array of strings (["term1", "term2"]). By default, all terms must be present in a file unless anyTerm=true is specified.',
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
              includeFilenames: {
                type: 'boolean',
                description: 'Include files whose names match query words',
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
                description: 'Maximum total tokens in code content to return (for AI usage)',
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
      ],
    }));

    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      if (request.params.name !== 'search_code') {
        throw new McpError(
          ErrorCode.MethodNotFound,
          `Unknown tool: ${request.params.name}`
        );
      }

      const args = request.params.arguments as unknown as SearchCodeArgs;
      
      try {
        const result = await this.executeCodeSearch(args);
        return {
          content: [
            {
              type: 'text',
              text: result,
            },
          ],
        };
      } catch (error) {
        console.error('Error executing code search:', error);
        return {
          content: [
            {
              type: 'text',
              text: `Error executing code search: ${error instanceof Error ? error.message : String(error)}`,
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
    
    // Add query as the first positional argument (can be string or array)
    const queries = Array.isArray(args.query) ? args.query : [args.query];
    // Use the first query as the main pattern (positional argument)
    if (queries.length > 0) {
      // Wrap query in quotes to handle multi-word queries
      cliArgs.push(`"${queries[0]}"`);
    }
    
    // Add path
    cliArgs.push('--paths', args.path);
    
    // Add optional arguments
    if (args.filesOnly) {
      cliArgs.push('--files-only');
    }
    
    if (args.ignore && args.ignore.length > 0) {
      for (const ignorePattern of args.ignore) {
        cliArgs.push('--ignore', ignorePattern);
      }
    }
    
    if (args.includeFilenames) {
      cliArgs.push('--include-filenames');
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
    
    if (args.maxTokens !== undefined) {
      cliArgs.push('--max-tokens', args.maxTokens.toString());
    }
    
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
    
    // Execute the command
    const command = `${PROBE_PATH} ${cliArgs.join(' ')}`;
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

  async run() {
    // Download the probe binary before starting the server
    try {
      console.log(`Downloading probe binary (version: ${packageVersion})...`);
      PROBE_PATH = await downloadProbeBinary(packageVersion);
      console.log(`Using probe binary at: ${PROBE_PATH}`);
    } catch (error) {
      console.error('Error downloading probe binary:', error);
      
      // Provide more detailed error information and suggestions
      if (error instanceof Error) {
        if (error.message.includes('404')) {
          console.error(`Version "${packageVersion}" not found in the repository.`);
          console.error('Expected version format: x.y.z (e.g., 1.2.3)');
          console.error('Suggestions:');
          console.error('1. Check if the version in package.json is correct');
          console.error('2. Verify that a release with tag v${packageVersion} exists in the repository');
          console.error('3. If using a development version, consider using a published release version');
        } else if (error.message.includes('network')) {
          console.error('Network error occurred while downloading the binary.');
          console.error('Suggestions:');
          console.error('1. Check your internet connection');
          console.error('2. Verify that GitHub API is accessible from your network');
          console.error('3. Try again later if GitHub might be experiencing issues');
        } else {
          console.error(`Error details: ${error.message}`);
        }
      }
      
      console.log('Falling back to environment variable PROBE_PATH if available');
      
      if (!PROBE_PATH) {
        console.error('No probe binary available. Please set PROBE_PATH environment variable or fix the version issue.');
        process.exit(1);
      }
    }
    
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
    console.error('Probe MCP server running on stdio');
  }
}

const server = new ProbeServer();
server.run().catch(console.error);
