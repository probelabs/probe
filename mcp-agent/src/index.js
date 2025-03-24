#!/usr/bin/env node
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
	CallToolRequestSchema,
	ErrorCode,
	ListToolsRequestSchema,
	McpError,
} from '@modelcontextprotocol/sdk/types.js';
import path from 'path';
import fs from 'fs-extra';
import { fileURLToPath } from 'url';
import { ProbeAgent } from './agent.js';
import config from './config.js';

// Parse command line arguments
function parseArgs() {
	const args = process.argv.slice(2);
	for (let i = 0; i < args.length; i++) {
		// Handle --provider flag
		if (args[i] === '--provider' && i + 1 < args.length) {
			const provider = args[i + 1].toLowerCase();
			if (['anthropic', 'openai', 'google'].includes(provider)) {
				process.env.FORCE_PROVIDER = provider;
				console.error(`Forcing provider: ${provider}`);
			} else {
				process.exit(1);
			}
		}
		// Handle shorthand flags
		else if (args[i] === '--google') {
			process.env.FORCE_PROVIDER = 'google';
			console.error('Forcing provider: google');
		}
		else if (args[i] === '--openai') {
			process.env.FORCE_PROVIDER = 'openai';
			console.error('Forcing provider: openai');
		}
		else if (args[i] === '--anthropic') {
			process.env.FORCE_PROVIDER = 'anthropic';
			console.error('Forcing provider: anthropic');
		}
	}
}

// Parse command line arguments before initializing
parseArgs();

// Get the package.json to determine the version
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Try to find package.json
let packageVersion = '1.0.0'; // Default version
const packageJsonPath = path.resolve(__dirname, '..', 'package.json');

try {
	if (fs.existsSync(packageJsonPath)) {
		console.error(`Found package.json at: ${packageJsonPath}`);
		const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));
		if (packageJson.version) {
			packageVersion = packageJson.version;
			console.error(`Using version from package.json: ${packageVersion}`);
		}
	}
} catch (error) {
	console.error(`Error reading package.json:`, error);
}

class ProbeAgentServer {
	constructor() {
		// Initialize the AI agent
		this.agent = new ProbeAgent();

		// Initialize the MCP server
		this.server = new Server(
			{
				name: '@buger/probe-mcp-agent',
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

	setupToolHandlers() {
		this.server.setRequestHandler(ListToolsRequestSchema, async () => ({
			tools: [
				{
					name: 'search_code',
					description: "Search code and answer questions about the codebase. This tool uses AI to analyze code and provide relevant code blocks and explanations.",
					inputSchema: {
						type: 'object',
						properties: {
							query: {
								type: 'string',
								description: 'The question or request about the codebase.',
							},
							path: {
								type: 'string',
								description: 'Absolute path to one of the allowed directories to search in. For security reasons, only allowed folders can be searched.',
							},
							context: {
								type: 'string',
								description: 'Additional context to help the AI understand the request better.',
							},
							max_tokens: {
								type: 'number',
								description: 'Maximum number of tokens to return in the response.',
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
				// Log the incoming request for debugging
				console.error(`Received request for tool: ${request.params.name}`);
				console.error(`Request arguments: ${JSON.stringify(request.params.arguments)}`);

				// Ensure arguments is an object
				if (!request.params.arguments || typeof request.params.arguments !== 'object') {
					throw new Error("Arguments must be an object");
				}

				const args = request.params.arguments;

				// Validate required fields
				if (!args.query) {
					throw new Error("Query is required in arguments");
				}

				// Process the query using the AI agent
				// If path is not provided, use the first allowed folder or current working directory
				let searchPath = args.path || (config.allowedFolders.length > 0 ? config.allowedFolders[0] : process.cwd());

				// If path is "." or "./", use the default path if available
				const defaultPath = config.allowedFolders.length > 0 ? config.allowedFolders[0] : process.cwd();
				if ((searchPath === "." || searchPath === "./") && defaultPath) {
					console.error(`Using default path "${defaultPath}" instead of "${searchPath}"`);
					searchPath = defaultPath;
				}

				// Create a copy of the allowed folders for potential modification
				let updatedAllowedFolders = [...config.allowedFolders];

				// Validate that the search path is within allowed folders
				if (config.allowedFolders.length > 0) {
					const isAllowed = config.allowedFolders.some(folder =>
						searchPath === folder || searchPath.startsWith(`${folder}/`)
					);

					if (!isAllowed) {
						throw new Error(`Path "${searchPath}" is not within allowed folders. Allowed folders: ${config.allowedFolders.join(', ')}`);
					}
				}

				// Add searchPath to allowedFolders if it's not already included
				if (searchPath && !updatedAllowedFolders.includes(searchPath) &&
					!updatedAllowedFolders.some(folder => searchPath.startsWith(`${folder}/`))) {
					updatedAllowedFolders.push(searchPath);
					console.error(`Added search path "${searchPath}" to allowed folders`);

					// Update the global config for this request
					config.allowedFolders = updatedAllowedFolders;
				}

				// Log the search path for debugging
				console.error(`Using search path: ${searchPath}`);
				console.error(`Current working directory: ${process.cwd()}`);

				const result = await this.agent.processQuery(args.query, searchPath);

				// Get token usage
				const tokenUsage = this.agent.getTokenUsage();
				console.error(`Token usage: ${JSON.stringify(tokenUsage)}`);

				// Return the result as a single text content
				// The result may already contain <code> tags if code was extracted
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

	async run() {
		try {
			console.error("Starting Probe Agent MCP server...");

			// Connect the server to the transport
			const transport = new StdioServerTransport();
			await this.server.connect(transport);
			console.error('Probe Agent MCP server running on stdio');
		} catch (error) {
			console.error('Error starting server:', error);
			process.exit(1);
		}
	}
}

const server = new ProbeAgentServer();
server.run().catch(console.error);