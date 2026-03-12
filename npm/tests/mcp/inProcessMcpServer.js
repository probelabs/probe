/**
 * In-process MCP Server for testing.
 *
 * Uses InMemoryTransport to create MCP servers that run inside Jest
 * without spawning external processes. Much faster and more deterministic
 * than stdio-based mock servers.
 *
 * Usage:
 *   const helper = new InProcessMcpServer('test-server');
 *   helper.addTool({ name: 'my_tool', ... }, async (args) => 'result');
 *   const { clientTransport } = await helper.start();
 *   // Pass clientTransport to MCPClientManager via transportInstance
 *   await helper.stop();
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import {
  ListToolsRequestSchema,
  CallToolRequestSchema,
} from '@modelcontextprotocol/sdk/types.js';

export class InProcessMcpServer {
  /**
   * @param {string} name - Server name
   * @param {Object} [options]
   * @param {string} [options.version='1.0.0']
   */
  constructor(name, options = {}) {
    this.name = name;
    this.version = options.version || '1.0.0';
    this.tools = new Map(); // name → { definition, handler }
    this.server = null;
    this.clientTransport = null;
    this.serverTransport = null;
  }

  /**
   * Register a tool on this server.
   *
   * @param {Object} definition - MCP tool definition
   * @param {string} definition.name - Tool name
   * @param {string} [definition.description] - Tool description
   * @param {Object} [definition.inputSchema] - JSON Schema for inputs
   * @param {Function} handler - async (args) => string | { content: [...] }
   */
  addTool(definition, handler) {
    this.tools.set(definition.name, { definition, handler });
    return this; // chainable
  }

  /**
   * Start the server and return the client-side transport.
   *
   * @returns {Promise<{ clientTransport: InMemoryTransport, server: Server }>}
   */
  async start() {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    this.clientTransport = clientTransport;
    this.serverTransport = serverTransport;

    this.server = new Server(
      { name: this.name, version: this.version },
      { capabilities: { tools: {} } }
    );

    // List tools handler
    this.server.setRequestHandler(ListToolsRequestSchema, async () => ({
      tools: Array.from(this.tools.values()).map(({ definition }) => ({
        name: definition.name,
        description: definition.description || '',
        inputSchema: definition.inputSchema || { type: 'object', properties: {} },
      })),
    }));

    // Call tool handler
    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      const { name, arguments: args } = request.params;
      const tool = this.tools.get(name);
      if (!tool) {
        return {
          content: [{ type: 'text', text: `Unknown tool: ${name}` }],
          isError: true,
        };
      }

      try {
        const result = await tool.handler(args || {});
        // Normalize result — handler can return a string or full content array
        if (typeof result === 'string') {
          return { content: [{ type: 'text', text: result }] };
        }
        if (result && result.content) {
          return result;
        }
        return { content: [{ type: 'text', text: JSON.stringify(result) }] };
      } catch (err) {
        return {
          content: [{ type: 'text', text: `Error: ${err.message}` }],
          isError: true,
        };
      }
    });

    await this.server.connect(serverTransport);

    return { clientTransport, server: this.server };
  }

  /**
   * Stop the server and clean up transports.
   */
  async stop() {
    try {
      if (this.server) await this.server.close();
    } catch (_) { /* ignore close errors */ }
    try {
      if (this.clientTransport) await this.clientTransport.close();
    } catch (_) { /* ignore */ }
    this.server = null;
    this.clientTransport = null;
    this.serverTransport = null;
  }

  /**
   * Get a server config object that can be passed to MCPClientManager.initialize().
   * Must be called after start().
   *
   * @param {Object} [overrides] - Additional config properties
   * @returns {Object} Config suitable for mcpServers entry
   */
  getClientConfig(overrides = {}) {
    if (!this.clientTransport) {
      throw new Error('Server not started. Call start() first.');
    }
    return {
      name: this.name,
      transport: 'in-memory',
      transportInstance: this.clientTransport,
      enabled: true,
      ...overrides,
    };
  }
}

/**
 * Create an agent-type MCP server that supports graceful_stop.
 *
 * The server runs a long task when `analyze` is called. When `graceful_stop`
 * is called, the analyze task returns its partial results early.
 *
 * @param {string} [name='agent-server'] - Server name
 * @returns {InProcessMcpServer & { getState: () => Object }}
 */
export function createAgentMcpServer(name = 'agent-server') {
  const state = {
    stopRequested: false,
    analyzeRunning: false,
    analyzeResolve: null,
    partialResults: [],
  };

  const server = new InProcessMcpServer(name);

  server.addTool(
    {
      name: 'analyze',
      description: 'Run a long-running analysis task',
      inputSchema: {
        type: 'object',
        properties: {
          query: { type: 'string', description: 'What to analyze' },
        },
        required: ['query'],
      },
    },
    async (args) => {
      state.analyzeRunning = true;
      state.stopRequested = false;
      state.partialResults = [];

      // Simulate work in steps — check stopRequested between steps
      for (let i = 0; i < 10; i++) {
        if (state.stopRequested) {
          state.partialResults.push(`Step ${i}: interrupted by graceful_stop`);
          break;
        }
        state.partialResults.push(`Step ${i}: analyzed "${args.query}" chunk ${i}`);
        // Small delay to simulate work
        await new Promise((resolve) => setTimeout(resolve, 50));
      }

      state.analyzeRunning = false;
      const summary = state.stopRequested
        ? `Partial analysis (stopped at step ${state.partialResults.length}): ${state.partialResults.join('; ')}`
        : `Complete analysis: ${state.partialResults.join('; ')}`;
      return summary;
    }
  );

  server.addTool(
    {
      name: 'graceful_stop',
      description: 'Signal the server to wrap up its current work and return partial results',
      inputSchema: { type: 'object', properties: {} },
    },
    async () => {
      state.stopRequested = true;
      return 'Graceful stop acknowledged. Running tasks will finish and return partial results.';
    }
  );

  server.getState = () => state;
  return server;
}
