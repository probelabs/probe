/**
 * Built-in MCP Server for Probe
 * Runs in the same process as ProbeAgent, eliminating spawn overhead
 */

import { createServer } from 'http';
import { EventEmitter } from 'events';
import { Server as MCPServer } from '@modelcontextprotocol/sdk/server/index.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema
} from '@modelcontextprotocol/sdk/types.js';

/**
 * Built-in MCP Server that runs in-process
 */
export class BuiltInMCPServer extends EventEmitter {
  constructor(agent, options = {}) {
    super();
    this.agent = agent;
    this.port = options.port || 0; // 0 = ephemeral port
    this.host = options.host || '127.0.0.1';
    this.httpServer = null;
    this.mcpServer = null;
    this.connections = new Set();
    this.debug = options.debug || false;
  }

  /**
   * Start the built-in MCP server
   */
  async start() {
    // Create HTTP server for SSE/HTTP transport
    this.httpServer = createServer();

    // Handle SSE connections
    this.httpServer.on('request', (req, res) => {
      this.handleRequest(req, res);
    });

    // Create MCP server
    this.mcpServer = new MCPServer({
      name: 'probe-builtin',
      version: '1.0.0'
    }, {
      capabilities: {
        tools: {}
      }
    });

    // Register MCP handlers
    this.registerHandlers();

    // Start listening on ephemeral port
    return new Promise((resolve, reject) => {
      this.httpServer.listen(this.port, this.host, () => {
        const address = this.httpServer.address();
        this.port = address.port;

        if (this.debug) {
          console.log(`[MCP] Built-in server started at http://${this.host}:${this.port}`);
        }

        this.emit('ready', { host: this.host, port: this.port });
        resolve({ host: this.host, port: this.port });
      });

      this.httpServer.on('error', reject);
    });
  }

  /**
   * Handle HTTP requests (SSE and JSON-RPC)
   */
  handleRequest(req, res) {
    const { method, url } = req;

    // CORS headers for local development
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

    if (method === 'OPTIONS') {
      res.writeHead(204);
      res.end();
      return;
    }

    // Handle SSE endpoint
    if (url === '/sse' && method === 'GET') {
      this.handleSSE(req, res);
      return;
    }

    // Handle JSON-RPC endpoint
    if (url === '/rpc' && method === 'POST') {
      this.handleJSONRPC(req, res);
      return;
    }

    // Handle stdio-like protocol over HTTP
    if (url === '/mcp' && method === 'POST') {
      this.handleMCPProtocol(req, res);
      return;
    }

    // Health check
    if (url === '/health') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        status: 'ok',
        server: 'probe-builtin-mcp',
        tools: this.getToolCount()
      }));
      return;
    }

    // 404 for unknown endpoints
    res.writeHead(404);
    res.end('Not Found');
  }

  /**
   * Handle Server-Sent Events connection
   */
  handleSSE(req, res) {
    res.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive'
    });

    // Send initial connection event
    res.write('event: connected\n');
    res.write(`data: ${JSON.stringify({ type: 'connected', server: 'probe-builtin' })}\n\n`);

    // Store connection
    this.connections.add(res);

    // Clean up on close
    req.on('close', () => {
      this.connections.delete(res);
    });
  }

  /**
   * Handle JSON-RPC requests
   */
  async handleJSONRPC(req, res) {
    let body = '';

    req.on('data', chunk => {
      body += chunk.toString();
    });

    req.on('end', async () => {
      try {
        const request = JSON.parse(body);
        const response = await this.processRequest(request);

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify(response));
      } catch (error) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          jsonrpc: '2.0',
          error: {
            code: -32700,
            message: 'Parse error',
            data: error.message
          },
          id: null
        }));
      }
    });
  }

  /**
   * Handle MCP protocol messages
   */
  async handleMCPProtocol(req, res) {
    let body = '';

    req.on('data', chunk => {
      body += chunk.toString();
    });

    req.on('end', async () => {
      try {
        const message = JSON.parse(body);

        // Process through MCP server handlers
        let response;

        if (message.method === 'tools/list') {
          response = await this.handleListTools();
        } else if (message.method === 'tools/call') {
          response = await this.handleCallTool(message.params);
        } else {
          response = {
            error: {
              code: -32601,
              message: 'Method not found'
            }
          };
        }

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify(response));
      } catch (error) {
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          error: {
            code: -32603,
            message: 'Internal error',
            data: error.message
          }
        }));
      }
    });
  }

  /**
   * Process JSON-RPC request
   */
  async processRequest(request) {
    const { jsonrpc, method, params, id } = request;

    try {
      let result;

      switch (method) {
        case 'tools/list':
          result = await this.handleListTools();
          break;

        case 'tools/call':
          result = await this.handleCallTool(params);
          break;

        default:
          return {
            jsonrpc: '2.0',
            error: {
              code: -32601,
              message: 'Method not found'
            },
            id
          };
      }

      return {
        jsonrpc: '2.0',
        result,
        id
      };
    } catch (error) {
      return {
        jsonrpc: '2.0',
        error: {
          code: -32603,
          message: 'Internal error',
          data: error.message
        },
        id
      };
    }
  }

  /**
   * Register MCP protocol handlers
   */
  registerHandlers() {
    // Handle list tools request
    this.mcpServer.setRequestHandler(ListToolsRequestSchema, async () => {
      return this.handleListTools();
    });

    // Handle tool execution
    this.mcpServer.setRequestHandler(CallToolRequestSchema, async (request) => {
      return this.handleCallTool(request.params);
    });
  }

  /**
   * Handle list tools request
   */
  async handleListTools() {
    const tools = [];

    // Get tools from agent
    if (this.agent && this.agent.allowedTools) {
      const toolDefs = {
        search: {
          description: 'Search for code patterns using semantic search',
          inputSchema: {
            type: 'object',
            properties: {
              query: { type: 'string', description: 'Search query' },
              path: { type: 'string', description: 'Directory to search', default: '.' },
              maxResults: { type: 'integer', default: 10 }
            },
            required: ['query']
          }
        },
        extract: {
          description: 'Extract code from specific file location',
          inputSchema: {
            type: 'object',
            properties: {
              path: { type: 'string', description: 'File path with optional line number' }
            },
            required: ['path']
          }
        },
        listFiles: {
          description: 'List files in a directory',
          inputSchema: {
            type: 'object',
            properties: {
              path: { type: 'string', description: 'Directory path' },
              pattern: { type: 'string', description: 'File pattern' }
            },
            required: ['path']
          }
        },
        searchFiles: {
          description: 'Search for files by name pattern',
          inputSchema: {
            type: 'object',
            properties: {
              pattern: { type: 'string', description: 'File name pattern' },
              path: { type: 'string', description: 'Directory to search' }
            },
            required: ['pattern']
          }
        },
        query: {
          description: 'Query code using AST patterns',
          inputSchema: {
            type: 'object',
            properties: {
              query: { type: 'string', description: 'AST query' },
              path: { type: 'string', description: 'Directory to search' }
            },
            required: ['query']
          }
        }
      };

      for (const [name, def] of Object.entries(toolDefs)) {
        if (this.agent.allowedTools.isEnabled(name)) {
          tools.push({
            name: `mcp__probe__${name}`,
            description: def.description,
            inputSchema: def.inputSchema
          });
        }
      }
    }

    return { tools };
  }

  /**
   * Handle tool execution
   */
  async handleCallTool(params) {
    const { name, arguments: args } = params;

    // Extract tool name from MCP format
    const toolName = name.replace('mcp__probe__', '');

    // Check if tool is enabled
    if (!this.agent.allowedTools.isEnabled(toolName)) {
      throw new Error(`Tool ${name} is not enabled`);
    }

    // Get tool implementation
    const tool = this.agent.toolImplementations[toolName];
    if (!tool) {
      throw new Error(`Tool ${name} not found`);
    }

    try {
      // Execute tool directly (no spawning!)
      const result = await tool.execute(args);

      return {
        content: [{
          type: 'text',
          text: typeof result === 'string' ? result : JSON.stringify(result, null, 2)
        }]
      };
    } catch (error) {
      return {
        content: [{
          type: 'text',
          text: `Error executing ${name}: ${error.message}`
        }],
        isError: true
      };
    }
  }

  /**
   * Get the number of available tools
   */
  getToolCount() {
    if (!this.agent || !this.agent.allowedTools) {
      return 0;
    }

    const tools = ['search', 'extract', 'listFiles', 'searchFiles', 'query'];
    return tools.filter(name => this.agent.allowedTools.isEnabled(name)).length;
  }

  /**
   * Broadcast message to all SSE connections
   */
  broadcast(event, data) {
    const message = `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;

    for (const connection of this.connections) {
      connection.write(message);
    }
  }

  /**
   * Stop the server
   */
  async stop() {
    // Close all SSE connections
    for (const connection of this.connections) {
      connection.end();
    }
    this.connections.clear();

    // Close HTTP server
    if (this.httpServer) {
      return new Promise((resolve) => {
        this.httpServer.close(() => {
          if (this.debug) {
            console.log('[MCP] Built-in server stopped');
          }
          resolve();
        });
      });
    }
  }

  /**
   * Get server configuration for MCP clients
   */
  getConfig() {
    return {
      transport: 'http',
      url: `http://${this.host}:${this.port}/mcp`,
      // Alternative transports:
      // sse: `http://${this.host}:${this.port}/sse`,
      // rpc: `http://${this.host}:${this.port}/rpc`
    };
  }
}