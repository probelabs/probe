#!/usr/bin/env node
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { CallToolRequestSchema, ErrorCode, ListToolsRequestSchema, McpError, } from '@modelcontextprotocol/sdk/types.js';
import { exec } from 'child_process';
import { promisify } from 'util';
const execAsync = promisify(exec);
// Path to the code-search binary
const CODE_SEARCH_PATH = process.env.CODE_SEARCH_PATH || '/Users/leonidbugaev/go/src/code-search/target/release/code-search';
class CodeSearchServer {
    constructor() {
        this.server = new Server({
            name: 'code-search-mcp',
            version: '0.1.0',
        }, {
            capabilities: {
                tools: {},
            },
        });
        this.setupToolHandlers();
        // Error handling
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
                    description: 'Search code in a specified directory. This tool should be used every time you need to search the codebase for understanding code structure, finding implementations, or identifying patterns. Queries can be any text (including multi-word phrases like "IP whitelist"), but prefer simple, focused queries for better results. Use maxResults parameter to limit the number of results when needed.',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            path: {
                                type: 'string',
                                description: 'Path to search in',
                            },
                            query: {
                                oneOf: [
                                    { type: 'string' },
                                    { type: 'array', items: { type: 'string' } }
                                ],
                                description: 'Query patterns to search for (string or array of strings)',
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
                                enum: ['hybrid', 'bm25', 'tfidf'],
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
                        },
                        required: ['path', 'query'],
                    },
                },
            ],
        }));
        this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
            if (request.params.name !== 'search_code') {
                throw new McpError(ErrorCode.MethodNotFound, `Unknown tool: ${request.params.name}`);
            }
            const args = request.params.arguments;
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
            }
            catch (error) {
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
    async executeCodeSearch(args) {
        // Build the command arguments
        const cliArgs = ['cli'];
        // Add path
        cliArgs.push('--path', args.path);
        // Add query (can be string or array)
        const queries = Array.isArray(args.query) ? args.query : [args.query];
        for (const q of queries) {
            // Wrap query in quotes to handle multi-word queries
            cliArgs.push('--query', `"${q}"`);
        }
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
        // Execute the command
        const command = `${CODE_SEARCH_PATH} ${cliArgs.join(' ')}`;
        console.log(`Executing command: ${command}`);
        try {
            const { stdout, stderr } = await execAsync(command);
            if (stderr) {
                console.error(`stderr: ${stderr}`);
            }
            return stdout;
        }
        catch (error) {
            console.error('Error executing code-search CLI:', error);
            throw error;
        }
    }
    async run() {
        const transport = new StdioServerTransport();
        await this.server.connect(transport);
        console.error('Code Search MCP server running on stdio');
    }
}
const server = new CodeSearchServer();
server.run().catch(console.error);
