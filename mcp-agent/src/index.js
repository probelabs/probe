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
	const config = {};
	
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
			i++; // Skip the next argument
		}
		// Handle --timeout flag
		else if ((args[i] === '--timeout' || args[i] === '-t') && i + 1 < args.length) {
			const timeout = parseInt(args[i + 1], 10);
			if (!isNaN(timeout) && timeout > 0) {
				config.timeout = timeout;
				console.error(`Timeout set to ${timeout} seconds`);
			} else {
				console.error(`Invalid timeout value: ${args[i + 1]}. Using default.`);
			}
			i++; // Skip the next argument
		}
		// Handle --help flag
		else if (args[i] === '--help' || args[i] === '-h') {
			console.log(`\nProbe MCP Agent Server\n\nUsage:\n  probe-mcp-agent [options]\n\nOptions:\n  --provider <name>        Force a specific AI provider (anthropic, openai, google)\n  --anthropic             Shorthand for --provider anthropic\n  --openai                Shorthand for --provider openai\n  --google                Shorthand for --provider google\n  --timeout, -t <seconds> Set timeout for search operations (default: 120)\n  --help, -h              Show this help message\n`);
			process.exit(0);
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
	
	return config;
}

// Parse command line arguments before initializing
const cliConfig = parseArgs();

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
	constructor(timeout = 300) { // Increased from 120 to 300 seconds (5 minutes)
		// Store timeout configuration
		this.defaultTimeout = timeout;
		
		// Don't initialize AI agent on startup - lazy initialize when needed
		this.agent = null;

		// Initialize the MCP server
		this.server = new Server(
			{
				name: '@probelabs/probe-mcp-agent',
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
		const toolsResponse = {
			tools: [
				{
					name: 'question',
					description: "Ask a question about the codebase. This tool uses AI to analyze code and provide relevant answers.",
					inputSchema: {
						type: 'object',
						properties: {
							query: {
								type: 'string',
								description: 'The question or request about the codebase.',
							},
							path: {
								type: 'string',
								description: 'Optional: Absolute path to search in. If not provided, uses allowed folders.',
							},
							timeout: {
								type: 'number',
								description: 'Timeout for the operation in seconds (default: 300)',
							}
						},
						required: ['query']
					},
				},
			],
		};
		
		console.error(`[DEBUG] Registering ${toolsResponse.tools.length} tools: ${toolsResponse.tools.map(t => t.name).join(', ')}`);
		
		this.server.setRequestHandler(ListToolsRequestSchema, async () => {
			console.error(`[DEBUG] Tools requested - returning ${toolsResponse.tools.length} tools`);
			return toolsResponse;
		});

		this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
			if (request.params.name !== 'question') {
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

				// Lazy initialize AI agent only when tool is called
				if (!this.agent) {
					console.error(`[DEBUG] Initializing AI agent on first tool call`);
					const { ProbeAgent } = await import('./agent.js');
					this.agent = new ProbeAgent();
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

				// Pass timeout from args or use default
				const timeout = args.timeout || this.defaultTimeout;
				console.error(`[DEBUG] Using timeout: ${timeout} seconds for query processing`);
				const result = await this.agent.processQuery(args.query, searchPath, { timeout });

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
			console.error(`[DEBUG] Default timeout: ${this.defaultTimeout}s`);
			console.error(`[DEBUG] Process PID: ${process.pid}`);
			console.error(`[DEBUG] Node.js version: ${process.version}`);
			
			// AI agent will be initialized lazily when first tool is called
			console.error(`[DEBUG] AI agent: Lazy initialization (will load on first tool call)`);

			// Connect the server to the transport
			const transport = new StdioServerTransport();
			await this.server.connect(transport);
			console.error('Probe Agent MCP server running on stdio');
			console.error(`[DEBUG] Server successfully connected and ready to receive requests`);
		} catch (error) {
			console.error('Error starting server:', error);
			console.error(`[DEBUG] Error details: ${error.stack || error.message}`);
			process.exit(1);
		}
	}
}

const server = new ProbeAgentServer(cliConfig.timeout);
server.run().catch(console.error);