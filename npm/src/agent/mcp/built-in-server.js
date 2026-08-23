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
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  isInitializeRequest
} from '@modelcontextprotocol/sdk/types.js';
import { isAuthenticProbeAgent } from '../governance-marker.js';

const GOVERNED_TOOL_PREFIX = 'mcp__probe__';
const GOVERNED_TOOL_NAMES = Object.freeze(['search', 'extract', 'listFiles']);
const GOVERNED_MODEL = 'gpt-5.6-luna';
const GOVERNED_REASONING_EFFORT = 'xhigh';
const GOVERNED_SANDBOX = 'seatbelt';
const MAX_AUDIT_BYTES = 1048576;
const MAX_AUDIT_ID = 512;
const MAX_AUDIT_RECORDS = 64;
const MAX_LIST_AUDIT_RECORDS = 8;
const MAX_TOOL_RESULT_BYTES = 262144;
const MAX_AUDIT_RECORD_RESERVATION_BYTES = 4096;

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function exactKeys(value, keys) {
  return isObject(value) && Object.keys(value).length === keys.length &&
    Object.keys(value).every(key => keys.includes(key));
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (isObject(value)) {
    return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

function boundedJson(value, field) {
  const serialized = canonicalJson(value);
  if (typeof serialized !== 'string' || Buffer.byteLength(serialized, 'utf8') > MAX_AUDIT_BYTES) {
    throw new Error(`Governed MCP ${field} exceeds the serialized-byte bound`);
  }
  return serialized;
}

function cloneBoundedJson(value, field, maxBytes = MAX_AUDIT_BYTES) {
  let serialized;
  try {
    serialized = JSON.stringify(value);
  } catch {
    throw new Error(`Governed MCP ${field} is not JSON-serializable`);
  }
  if (typeof serialized !== 'string' || Buffer.byteLength(serialized, 'utf8') > maxBytes) {
    throw new Error(`Governed MCP ${field} exceeds the serialized-byte bound`);
  }
  try {
    const clone = JSON.parse(serialized);
    // Check the canonical representation too.  This rejects values such as
    // undefined that JSON.stringify would otherwise silently discard.
    boundedJson(clone, field);
    return clone;
  } catch (error) {
    if (error instanceof Error && error.message.startsWith('Governed MCP ')) throw error;
    throw new Error(`Governed MCP ${field} is not JSON-serializable`);
  }
}

function digest(value, field) {
  const serialized = boundedJson(value, field);
  return { sha256: createHash('sha256').update(serialized).digest('hex'), bytes: Buffer.byteLength(serialized, 'utf8') };
}

function requireBoundedString(value, field) {
  if (typeof value !== 'string' || value.length === 0 || value.length > MAX_AUDIT_ID) {
    throw new Error(`Governed MCP ${field} is invalid`);
  }
  return value;
}

function validateGovernedMetadata(meta) {
  if (!exactKeys(meta, ['progressToken', 'threadId', 'x-codex-turn-metadata'])) {
    throw new Error('Governed MCP _meta keys are not exact');
  }
  if (!Number.isInteger(meta.progressToken) || meta.progressToken < 0 || meta.progressToken > Number.MAX_SAFE_INTEGER) {
    throw new Error('Governed MCP progressToken is invalid');
  }
  const threadId = requireBoundedString(meta.threadId, '_meta.threadId');
  const extension = meta['x-codex-turn-metadata'];
  if (!exactKeys(extension, ['session_id', 'thread_id', 'turn_id', 'sandbox', 'turn_started_at_unix_ms', 'model', 'reasoning_effort'])) {
    throw new Error('Governed MCP extension metadata keys are not exact');
  }
  const sessionId = requireBoundedString(extension.session_id, 'extension.session_id');
  const extensionThreadId = requireBoundedString(extension.thread_id, 'extension.thread_id');
  const turnId = requireBoundedString(extension.turn_id, 'extension.turn_id');
  if (sessionId !== extensionThreadId || sessionId !== threadId || extension.sandbox !== GOVERNED_SANDBOX ||
      extension.model !== GOVERNED_MODEL || extension.reasoning_effort !== GOVERNED_REASONING_EFFORT ||
      !Number.isSafeInteger(extension.turn_started_at_unix_ms) || extension.turn_started_at_unix_ms <= 0) {
    throw new Error('Governed MCP extension metadata identity is invalid');
  }
  return {
    session_id: sessionId,
    thread_id: extensionThreadId,
    turn_id: turnId,
    sandbox: extension.sandbox,
    turn_started_at_unix_ms: extension.turn_started_at_unix_ms,
    model: extension.model,
    reasoning_effort: extension.reasoning_effort,
    threadId,
    progressToken: meta.progressToken
  };
}

function governedAgentShape(agent) {
  if (!isAuthenticProbeAgent(agent) || !agent.allowedTools || agent.allowedTools.mode !== 'whitelist' ||
      !Array.isArray(agent.allowedTools.allowed) || agent.allowedTools.allowed.length !== GOVERNED_TOOL_NAMES.length ||
      new Set(agent.allowedTools.allowed).size !== GOVERNED_TOOL_NAMES.length ||
      GOVERNED_TOOL_NAMES.some(name => !agent.allowedTools.allowed.includes(name)) ||
      !isObject(agent.toolImplementations) ||
      Object.keys(agent.toolImplementations).sort().join(',') !== GOVERNED_TOOL_NAMES.slice().sort().join(',') ||
      GOVERNED_TOOL_NAMES.some(name => !isObject(agent.toolImplementations[name]) || typeof agent.toolImplementations[name].execute !== 'function')) {
    throw new Error('Governed Probe MCP requires an authentic exact three-tool ProbeAgent');
  }
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
    this.governed = options.governed === true;
    this.serverName = options.serverName || null;
    if (this.governed && (!this.serverName || typeof this.serverName !== 'string')) {
      throw new Error('Governed Probe MCP requires its derived server name');
    }
    if (this.governed) governedAgentShape(agent);
    this.audit = {
      starts: [],
      listCalls: [],
      toolCalls: [],
      executionCounts: { search: 0, extract: 0, listFiles: 0 },
      nextOrdinal: 1,
      inFlight: 0
    };
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
        if (this.governed) {
          this.audit.starts.push({ host: this.host, port: this.port, url_path: '/mcp' });
        }

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

      const names = this.governed ? GOVERNED_TOOL_NAMES : Object.keys(toolDefs);
      for (const name of names) {
        const def = toolDefs[name];
        if (def && this.agent.allowedTools.isEnabled(name) && this.agent.toolImplementations?.[name]) {
          tools.push({
            name: `${GOVERNED_TOOL_PREFIX}${name}`,
            description: def.description,
            inputSchema: def.inputSchema
          });
        }
      }
    }

    const result = { tools };
    if (this.governed) {
      if (this.audit.listCalls.length >= MAX_LIST_AUDIT_RECORDS || this.audit.nextOrdinal > MAX_AUDIT_RECORDS + MAX_LIST_AUDIT_RECORDS) {
        throw new Error('Governed MCP list audit bound exceeded');
      }
      const resultDigest = digest(result, 'list result');
      const record = {
        ordinal: this.audit.nextOrdinal++,
        tool_names: tools.map(tool => tool.name),
        result: resultDigest
      };
      this.audit.listCalls.push(record);
      try {
        this.assertAuditWithinBounds();
      } catch (error) {
        this.audit.listCalls.pop();
        this.audit.nextOrdinal--;
        throw error;
      }
    }
    return result;
  }

  /**
   * Handle tool execution
   */
  async handleCallTool(params) {
    let metadata = null;
    if (this.governed) {
      if (!params || typeof params !== 'object' || Array.isArray(params) ||
          !Object.prototype.hasOwnProperty.call(params, '_meta') ||
          !Object.prototype.hasOwnProperty.call(params, 'name') ||
          !Object.prototype.hasOwnProperty.call(params, 'arguments') ||
          Object.keys(params).some(key => !['_meta', 'name', 'arguments', 'server'].includes(key)) ||
          typeof params.name !== 'string' || !GOVERNED_TOOL_NAMES.some(name => params.name === `${GOVERNED_TOOL_PREFIX}${name}`)) {
        throw new Error('Governed Probe MCP tool name is not an exact allowlisted identity');
      }
      if (params.server !== undefined && params.server !== this.serverName) {
        throw new Error('Governed Probe MCP server identity is not exact');
      }
      metadata = validateGovernedMetadata(params._meta);
    }
    const { name, arguments: rawArgs = {} } = params;

    if (!isObject(rawArgs)) throw new Error('Governed MCP arguments must be an object');

    // Extract tool name from MCP format
    const toolName = this.governed ? name.slice(GOVERNED_TOOL_PREFIX.length) : name.replace(GOVERNED_TOOL_PREFIX, '');

    // Check if tool is enabled
    if (!this.agent.allowedTools.isEnabled(toolName)) {
      throw new Error(`Tool ${name} is not enabled`);
    }

    // Get tool implementation
    const tool = this.agent.toolImplementations[toolName];
    if (!tool) {
      throw new Error(`Tool ${name} not found`);
    }

    let args = rawArgs;
    let admitted = false;
    let argsDigest = null;
    if (this.governed) {
      // Canonicalize and bound the exact value passed to the implementation.
      // This admission happens before execute(), so a malformed or oversized
      // request cannot cause a tool side effect.
      args = cloneBoundedJson(rawArgs, 'arguments');
      argsDigest = digest(args, 'arguments');
      this.reserveToolAudit(argsDigest);
      admitted = true;
    }

    try {
      // Execute tool directly (no spawning!)
      const result = await tool.execute(args);
      let response;
      try {
        const text = typeof result === 'string' ? result : JSON.stringify(result, null, 2);
        if (typeof text !== 'string' || Buffer.byteLength(text, 'utf8') > MAX_TOOL_RESULT_BYTES) {
          throw new Error('tool result exceeds the serialized-byte bound');
        }
        if (typeof result !== 'string') cloneBoundedJson(result, 'tool result', MAX_TOOL_RESULT_BYTES);
        response = { content: [{ type: 'text', text }] };
      } catch (error) {
        response = {
          content: [{
            type: 'text',
            text: `Error executing ${name}: ${String(error?.message || error).slice(0, MAX_AUDIT_ID)}`
          }],
          isError: true
        };
        if (this.governed) this.recordToolAudit(name, args, response, 'failed', metadata, argsDigest);
        return response;
      }
      if (this.governed) this.recordToolAudit(name, args, response, 'ok', metadata, argsDigest);
      return response;
    } catch (error) {
      const response = {
        content: [{
          type: 'text',
          text: `Error executing ${name}: ${error.message}`
        }],
        isError: true
      };
      if (this.governed) this.recordToolAudit(name, args, response, 'failed', metadata, argsDigest);
      return response;
    } finally {
      if (admitted) this.audit.inFlight--;
    }
  }

  reserveToolAudit(argsDigest) {
    if (this.audit.toolCalls.length + this.audit.inFlight >= MAX_AUDIT_RECORDS ||
        this.audit.nextOrdinal > MAX_AUDIT_RECORDS + MAX_LIST_AUDIT_RECORDS ||
        !argsDigest || argsDigest.bytes > MAX_AUDIT_BYTES) {
      throw new Error('Governed MCP tool audit bound exceeded');
    }
    // The audit stores digests and bounded metadata, never result content.
    // Reserve enough ledger space before execute() for the terminal record.
    const current = this.getAuditSnapshot();
    const currentBytes = Buffer.byteLength(JSON.stringify(current), 'utf8');
    if (currentBytes + MAX_AUDIT_RECORD_RESERVATION_BYTES > MAX_AUDIT_BYTES) {
      throw new Error('Governed MCP cumulative audit-byte bound exceeded');
    }
    this.audit.inFlight++;
  }

  recordToolAudit(name, args, result, status, metadata, knownArgsDigest = null) {
    const bareName = name.slice(GOVERNED_TOOL_PREFIX.length);
    const argsDigest = knownArgsDigest || digest(args, 'arguments');
    const resultDigest = digest({ content: result.content }, 'result');
    const record = {
      ordinal: this.audit.nextOrdinal++,
      name,
      arguments: argsDigest,
      metadata: {
        session_id: metadata.session_id,
        thread_id: metadata.thread_id,
        turn_id: metadata.turn_id,
        sandbox: metadata.sandbox,
        turn_started_at_unix_ms: metadata.turn_started_at_unix_ms,
        model: metadata.model,
        reasoning_effort: metadata.reasoning_effort,
        threadId: metadata.threadId,
        progressToken: metadata.progressToken
      },
      result: { ...resultDigest, status }
    };
    this.audit.toolCalls.push(record);
    try {
      this.assertAuditWithinBounds();
    } catch (error) {
      this.audit.toolCalls.pop();
      this.audit.nextOrdinal--;
      throw error;
    }
    this.audit.executionCounts[bareName]++;
  }

  assertAuditWithinBounds() {
    if (this.audit.toolCalls.length > MAX_AUDIT_RECORDS || this.audit.listCalls.length > MAX_LIST_AUDIT_RECORDS) {
      throw new Error('Governed MCP audit record bound exceeded');
    }
    const snapshot = {
      starts: this.audit.starts,
      listCalls: this.audit.listCalls,
      toolCalls: this.audit.toolCalls,
      executionCounts: this.audit.executionCounts
    };
    if (Buffer.byteLength(boundedJson(snapshot, 'audit snapshot'), 'utf8') > MAX_AUDIT_BYTES) {
      throw new Error('Governed MCP cumulative audit-byte bound exceeded');
    }
  }

  getAuditSnapshot() {
    const snapshot = {
      starts: this.audit.starts.map(start => ({ ...start })),
      listCalls: this.audit.listCalls.map(call => ({ ...call, result: { ...call.result } })),
      toolCalls: this.audit.toolCalls.map(call => ({
        ordinal: call.ordinal,
        name: call.name,
        arguments: { ...call.arguments },
        metadata: { ...call.metadata },
        result: { ...call.result }
      })),
      executionCounts: { ...this.audit.executionCounts }
    };
    boundedJson(snapshot, 'audit snapshot');
    return snapshot;
  }

  getGovernedAuditSnapshot() {
    return this.getAuditSnapshot();
  }

  /**
   * Get the number of available tools
   */
  getToolCount() {
    if (!this.agent || !this.agent.allowedTools) {
      return 0;
    }

    const tools = this.governed ? GOVERNED_TOOL_NAMES : ['search', 'extract', 'listFiles', 'searchFiles', 'query'];
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
