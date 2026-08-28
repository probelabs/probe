/**
 * Built-in MCP Server for Probe
 * Runs in the same process as ProbeAgent, eliminating spawn overhead
 */

import { createServer } from 'http';
import { EventEmitter } from 'events';
import { createHash, randomUUID } from 'crypto';
import { Server as MCPServer } from '@modelcontextprotocol/sdk/server/index.js';
import { SSEServerTransport } from '@modelcontextprotocol/sdk/server/sse.js';
import { StreamableHTTPServerTransport } from '@modelcontextprotocol/sdk/server/streamableHttp.js';
import { asSchema } from 'ai';
import { searchSchema, extractSchema, listFilesSchema } from '../../tools/common.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  isInitializeRequest
} from '@modelcontextprotocol/sdk/types.js';

function descriptor(description, schema) {
  const canonical = asSchema(schema);
  return Object.freeze({ description, inputSchema: canonical.jsonSchema, validate: canonical.validate });
}

const TOOL_DESCRIPTORS = Object.freeze({
  search: descriptor('Search for code patterns using semantic search', searchSchema),
  extract: descriptor('Extract code from files or symbols', extractSchema),
  listFiles: descriptor('List files in a directory', listFilesSchema)
});

const ARGUMENT_DOMAIN = Buffer.from('reqproof.probe.tool-arguments/v1', 'utf8');

function ownData(value, arrays, stack = new Set()) {
  if (value === null || typeof value !== 'object') return;
  if (stack.has(value)) throw new TypeError('cycle');
  stack.add(value);
  const proto = Object.getPrototypeOf(value);
  const isArray = Array.isArray(value);
  if (isArray ? proto !== Array.prototype : proto !== Object.prototype && proto !== null) throw new TypeError('prototype');
  const descriptors = Object.getOwnPropertyDescriptors(value);
  const keys = Reflect.ownKeys(descriptors);
  if (keys.some(key => typeof key === 'symbol')) throw new TypeError('symbol');
  if (isArray) {
    if (!arrays) throw new TypeError('array');
    if (keys.some(key => key !== 'length' && (!/^(0|[1-9]\d*)$/.test(key) || Number(key) >= value.length))) throw new TypeError('array key');
    for (let i = 0; i < value.length; i++) if (!Object.prototype.hasOwnProperty.call(descriptors, i)) throw new TypeError('array hole');
  }
  for (const key of keys) {
    if (isArray && key === 'length') continue;
    const property = descriptors[key];
    if (!property.enumerable || !Object.prototype.hasOwnProperty.call(property, 'value') || key === 'toJSON') throw new TypeError('property');
    ownData(property.value, arrays, stack);
  }
  stack.delete(value);
}

function canonicalJSON(value, stack = new Set()) {
  if (value === null || typeof value === 'boolean' || typeof value === 'string') return JSON.stringify(value);
  if (typeof value === 'number') { if (!Number.isFinite(value)) throw new TypeError('number'); return Object.is(value, -0) ? '0' : JSON.stringify(value); }
  if (typeof value !== 'object' || stack.has(value)) throw new TypeError('value');
  ownData(value, true); stack.add(value);
  let encoded;
  if (Array.isArray(value)) encoded = `[${value.map(item => canonicalJSON(item, stack)).join(',')}]`;
  else encoded = `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonicalJSON(value[key], stack)}`).join(',')}}`;
  stack.delete(value); return encoded;
}

function argumentDigest(value) {
  const payload = Buffer.from(canonicalJSON(value), 'utf8');
  const length = bytes => { const out = Buffer.alloc(8); out.writeBigUInt64BE(BigInt(bytes.length)); return out; };
  return `sha256:${createHash('sha256').update(length(ARGUMENT_DOMAIN)).update(ARGUMENT_DOMAIN).update(length(payload)).update(payload).digest('hex')}`;
}

async function validateToolArguments(name, args) {
  const contract = TOOL_DESCRIPTORS[name];
  if (!contract) return { value: args };
  try { if (!args || Array.isArray(args) || typeof args !== 'object') throw new TypeError(); ownData(args, false); }
  catch { throw new TypeError('TOOL_ARGUMENT_CONTAINER_INVALID'); }
  try {
    for (const key of Object.keys(args)) if (!Object.prototype.hasOwnProperty.call(contract.inputSchema.properties, key)) throw new TypeError();
    const validated = await contract.validate(args);
    if (!validated.success) throw new TypeError();
    return { value: validated.value, digest: argumentDigest(validated.value) };
  } catch { throw new TypeError('TOOL_ARGUMENT_VALIDATION_FAILED'); }
}

function emitToolCall(agent, event) {
  agent.events?.emit('toolCall', Object.freeze(event));
}

function abortable(promise, signal) {
  if (!signal) return promise;
  if (signal.aborted) return Promise.reject(new Error('Tool execution cancelled'));
  return new Promise((resolve, reject) => {
    const aborted = () => reject(new Error('Tool execution cancelled'));
    signal.addEventListener('abort', aborted, { once: true });
    promise.then(value => { signal.removeEventListener('abort', aborted); resolve(value); }, error => { signal.removeEventListener('abort', aborted); reject(error); });
  });
}

/**
 * Simple in-memory event store for resumability
 */
class InMemoryEventStore {
  constructor() {
    this.events = new Map();
  }

  generateEventId(streamId) {
    return `${streamId}_${Date.now()}_${Math.random().toString(36).substring(2, 10)}`;
  }

  getStreamIdFromEventId(eventId) {
    const parts = eventId.split('_');
    return parts.length > 0 ? parts[0] : '';
  }

  async storeEvent(streamId, message) {
    const eventId = this.generateEventId(streamId);
    this.events.set(eventId, { streamId, message });
    return eventId;
  }

  async replayEventsAfter(lastEventId, { send }) {
    if (!lastEventId || !this.events.has(lastEventId)) {
      return '';
    }

    const streamId = this.getStreamIdFromEventId(lastEventId);
    if (!streamId) {
      return '';
    }

    let foundLastEvent = false;
    const sortedEvents = [...this.events.entries()].sort((a, b) => a[0].localeCompare(b[0]));

    for (const [eventId, { streamId: eventStreamId, message }] of sortedEvents) {
      if (eventStreamId !== streamId) {
        continue;
      }

      if (eventId === lastEventId) {
        foundLastEvent = true;
        continue;
      }

      if (foundLastEvent) {
        await send(eventId, message);
      }
    }

    return streamId;
  }
}

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
    this.sseTransports = new Map();  // Map of sessionId -> SSEServerTransport (deprecated)
    this.streamableTransports = new Map();  // Map of sessionId -> StreamableHTTPServerTransport
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
      this.httpServer.listen(this.port, this.host, async () => {
        const address = this.httpServer.address();
        this.port = address.port;

        if (this.debug) {
          console.log(`[MCP] Built-in server started at http://${this.host}:${this.port}`);
          console.log(`[MCP] SSE endpoint: http://${this.host}:${this.port}/sse`);
          console.log(`[MCP] Messages endpoint: http://${this.host}:${this.port}/messages`);
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

    if (this.debug) {
      console.log(`[MCP] Request: ${method} ${url}`);
    }

    // CORS headers for local development
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

    if (method === 'OPTIONS') {
      res.writeHead(204);
      res.end();
      return;
    }

    // Handle SSE endpoint (GET) - create new transport per connection
    if (url === '/sse' && method === 'GET') {
      if (this.debug) {
        console.log('[MCP] Routing to handleSSEConnection');
      }
      this.handleSSEConnection(req, res);
      return;
    }

    // Handle /messages endpoint (POST) - route to existing transport
    if (url.startsWith('/messages') && method === 'POST') {
      this.handleSSEMessage(req, res);
      return;
    }

    // Handle JSON-RPC endpoint
    if (url === '/rpc' && method === 'POST') {
      this.handleJSONRPC(req, res);
      return;
    }

    // Handle Streamable HTTP protocol (GET/POST/DELETE on /mcp)
    if (url === '/mcp') {
      this.handleStreamableHTTP(req, res);
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
   * Handle SSE connection (GET /sse) - creates new transport
   */
  async handleSSEConnection(req, res) {
    if (this.debug) {
      console.log('[MCP] New SSE connection request');
    }

    // Create new SSEServerTransport for this connection
    const transport = new SSEServerTransport('/messages', res);

    // Store transport by sessionId
    this.sseTransports.set(transport.sessionId, transport);

    // Clean up on connection close
    res.on('close', () => {
      if (this.debug) {
        console.log('[MCP] SSE connection closed, sessionId:', transport.sessionId);
      }
      this.sseTransports.delete(transport.sessionId);
    });

    // Connect MCP server to this transport
    try {
      await this.mcpServer.connect(transport);
      if (this.debug) {
        console.log('[MCP] MCP server connected to SSE transport, sessionId:', transport.sessionId);
      }
    } catch (error) {
      if (this.debug) {
        console.error('[MCP] Error connecting MCP server to transport:', error);
      }
      this.sseTransports.delete(transport.sessionId);
    }
  }

  /**
   * Handle SSE message (POST /messages?sessionId=...) - routes to existing transport
   */
  async handleSSEMessage(req, res) {
    // Parse URL to get sessionId from query parameter
    const url = new URL(req.url, `http://${req.headers.host}`);
    const sessionId = url.searchParams.get('sessionId');

    if (!sessionId) {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        jsonrpc: '2.0',
        error: {
          code: -32000,
          message: 'Bad Request: sessionId query parameter is required'
        },
        id: null
      }));
      return;
    }

    // Find transport for this session
    const transport = this.sseTransports.get(sessionId);
    if (!transport) {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        jsonrpc: '2.0',
        error: {
          code: -32000,
          message: `Bad Request: No transport found for sessionId: ${sessionId}`
        },
        id: null
      }));
      return;
    }

    // Read request body
    let body = '';
    req.on('data', chunk => {
      body += chunk.toString();
    });

    req.on('end', async () => {
      try {
        const message = JSON.parse(body);
        await transport.handlePostMessage(req, res, message);
      } catch (error) {
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          jsonrpc: '2.0',
          error: {
            code: -32603,
            message: 'Internal error',
            data: error.message
          },
          id: null
        }));
      }
    });
  }

  /**
   * Handle Streamable HTTP protocol (GET/POST/DELETE on /mcp)
   */
  async handleStreamableHTTP(req, res) {
    const { method } = req;

    if (this.debug) {
      console.log(`[MCP] Streamable HTTP ${method} request`);
    }

    try {
      // Parse request body for POST requests
      let body = null;
      if (method === 'POST') {
        body = await this.parseRequestBody(req);
      }

      // Check for existing session ID in header
      const sessionId = req.headers['mcp-session-id'];
      let transport;

      if (sessionId && this.streamableTransports.has(sessionId)) {
        // Reuse existing transport
        transport = this.streamableTransports.get(sessionId);
        if (this.debug) {
          console.log(`[MCP] Reusing existing transport for session: ${sessionId}`);
        }
      } else if (!sessionId && method === 'POST' && body && isInitializeRequest(body)) {
        // New session - create transport for initialization request
        if (this.debug) {
          console.log('[MCP] Creating new Streamable HTTP transport for initialization');
        }

        const eventStore = new InMemoryEventStore();
        transport = new StreamableHTTPServerTransport({
          sessionIdGenerator: () => randomUUID(),
          eventStore, // Enable resumability
          onsessioninitialized: (newSessionId) => {
            // Store the transport by session ID
            if (this.debug) {
              console.log(`[MCP] Streamable HTTP session initialized: ${newSessionId}`);
            }
            this.streamableTransports.set(newSessionId, transport);
          },
          onsessionclosed: (closedSessionId) => {
            // Remove transport when session is closed
            if (this.debug) {
              console.log(`[MCP] Streamable HTTP session closed: ${closedSessionId}`);
            }
            this.streamableTransports.delete(closedSessionId);
          }
        });

        // Set up onclose handler
        transport.onclose = () => {
          const sid = transport.sessionId;
          if (sid && this.streamableTransports.has(sid)) {
            if (this.debug) {
              console.log(`[MCP] Transport closed for session ${sid}`);
            }
            this.streamableTransports.delete(sid);
          }
        };

        // Connect the transport to the MCP server
        await this.mcpServer.connect(transport);
      } else {
        // Invalid request - no session ID or not an initialization request
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          jsonrpc: '2.0',
          error: {
            code: -32000,
            message: 'Bad Request: No valid session ID provided or not an initialization request'
          },
          id: null
        }));
        return;
      }

      // Handle the request with the transport
      await transport.handleRequest(req, res, body);
    } catch (error) {
      if (this.debug) {
        console.error('[MCP] Error handling Streamable HTTP request:', error);
      }

      if (!res.headersSent) {
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          jsonrpc: '2.0',
          error: {
            code: -32603,
            message: 'Internal server error',
            data: error.message
          },
          id: null
        }));
      }
    }
  }

  /**
   * Parse request body as JSON
   */
  async parseRequestBody(req) {
    return new Promise((resolve, reject) => {
      let body = '';
      req.on('data', chunk => {
        body += chunk.toString();
      });
      req.on('end', () => {
        try {
          const parsed = body ? JSON.parse(body) : null;
          resolve(parsed);
        } catch (error) {
          reject(error);
        }
      });
      req.on('error', reject);
    });
  }

  /**
   * Handle Server-Sent Events connection (DEPRECATED - use handleSSEConnection instead)
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
      const toolDefs = { ...TOOL_DESCRIPTORS,
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
    const { name, arguments: rawArgs } = params;

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

    let validated;
    try { validated = await validateToolArguments(toolName, rawArgs); } catch (error) {
      return { content: [{ type: 'text', text: `Error executing ${name}: ${error.message}` }], isError: true };
    }
    const { value: args, digest: argumentsDigest } = validated;

    const id = randomUUID();
    const startTime = Date.now();
    const baseEvent = { id, name: toolName, sessionId: this.agent.sessionId, startTime, ...(argumentsDigest && { argumentsDigest }) };
    const fail = error => {
      const endTime = Date.now();
      try { emitToolCall(this.agent, { ...baseEvent, status: 'failed', error: 'TOOL_EXECUTION_FAILED', endTime, duration: endTime - startTime }); }
      catch (listenerError) { error = listenerError; }
      return { content: [{ type: 'text', text: `Error executing ${name}: ${error.message}` }], isError: true };
    };

    try { emitToolCall(this.agent, { ...baseEvent, status: 'in_progress' }); }
    catch (error) { return fail(error); }

    try {
      // Execute tool directly (no spawning!)
      const signal = this.agent.abortSignal;
      const execution = Promise.resolve().then(() => tool.execute({ ...args, sessionId: this.agent.sessionId, workingDirectory: this.agent.workspaceRoot || this.agent.cwd || process.cwd(), abortSignal: signal }));
      const result = await abortable(execution, signal);

      const endTime = Date.now();
      try { emitToolCall(this.agent, { ...baseEvent, status: 'completed', endTime, duration: endTime - startTime }); }
      catch { /* Execution already succeeded; observer failure is non-authoritative. */ }

      return {
        content: [{
          type: 'text',
          text: typeof result === 'string' ? result : JSON.stringify(result, null, 2)
        }]
      };
    } catch (error) {
      return fail(error);
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
    // Close all Streamable HTTP transports
    for (const [sessionId, transport] of this.streamableTransports.entries()) {
      try {
        await transport.close();
        if (this.debug) {
          console.log(`[MCP] Closed Streamable HTTP transport for session: ${sessionId}`);
        }
      } catch (error) {
        if (this.debug) {
          console.error(`[MCP] Error closing Streamable HTTP transport ${sessionId}:`, error);
        }
      }
    }
    this.streamableTransports.clear();

    // Close all SSE transports
    for (const [sessionId, transport] of this.sseTransports.entries()) {
      try {
        await transport.close();
        if (this.debug) {
          console.log(`[MCP] Closed SSE transport for session: ${sessionId}`);
        }
      } catch (error) {
        if (this.debug) {
          console.error(`[MCP] Error closing SSE transport ${sessionId}:`, error);
        }
      }
    }
    this.sseTransports.clear();

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
