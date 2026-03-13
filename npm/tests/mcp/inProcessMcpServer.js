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

/**
 * Create a standard mock MCP server with all 7 tools from mockMcpServer.js.
 * Drop-in replacement for the stdio-based mock server.
 *
 * Tools: foobar, calculator, echo, filesystem, weather, error_test, slow_operation
 *
 * @param {string} [name='mock-test'] - Server name
 * @returns {InProcessMcpServer}
 */
export function createStandardMockServer(name = 'mock-test') {
  const dataStore = new Map();
  dataStore.set('test_key', 'test_value');
  dataStore.set('count', '42');

  const server = new InProcessMcpServer(name);

  // 1. foobar — key-value store
  server.addTool(
    {
      name: 'foobar',
      description: 'A simple key-value store tool for testing basic MCP functionality',
      inputSchema: {
        type: 'object',
        properties: {
          action: { type: 'string', enum: ['get', 'set', 'list'], default: 'get' },
          key: { type: 'string' },
          value: { type: 'string' },
        },
      },
    },
    async (args) => {
      const action = args.action || 'get';
      switch (action) {
        case 'get':
          if (!args.key) throw new Error('Key is required for get operation');
          const val = dataStore.get(args.key);
          return val !== undefined ? `Value for key "${args.key}": ${val}` : `Key "${args.key}" not found`;
        case 'set':
          if (!args.key || args.value === undefined) throw new Error('Both key and value are required for set operation');
          dataStore.set(args.key, args.value);
          return `Successfully set "${args.key}" = "${args.value}"`;
        case 'list':
          const keys = Array.from(dataStore.keys());
          return keys.length > 0 ? `Stored keys: ${keys.join(', ')}` : 'No keys stored';
        default:
          throw new Error(`Unknown action: ${action}`);
      }
    }
  );

  // 2. calculator
  server.addTool(
    {
      name: 'calculator',
      description: 'Performs basic mathematical operations',
      inputSchema: {
        type: 'object',
        properties: {
          operation: { type: 'string', enum: ['add', 'subtract', 'multiply', 'divide'] },
          a: { type: 'number' },
          b: { type: 'number' },
        },
        required: ['operation', 'a', 'b'],
      },
    },
    async (args) => {
      const { operation, a, b } = args;
      let result;
      switch (operation) {
        case 'add': result = a + b; break;
        case 'subtract': result = a - b; break;
        case 'multiply': result = a * b; break;
        case 'divide':
          if (b === 0) throw new Error('Division by zero is not allowed');
          result = a / b; break;
        default: throw new Error(`Unknown operation: ${operation}`);
      }
      return `${a} ${operation} ${b} = ${result}`;
    }
  );

  // 3. echo
  server.addTool(
    {
      name: 'echo',
      description: 'Echoes back the provided message',
      inputSchema: {
        type: 'object',
        properties: { message: { type: 'string' } },
        required: ['message'],
      },
    },
    async (args) => `Echo: ${args.message}`
  );

  // 4. filesystem
  const mockFiles = {
    '/test.txt': 'This is test content',
    '/config.json': '{"setting": "value"}',
    '/empty.txt': '',
  };
  server.addTool(
    {
      name: 'filesystem',
      description: 'Mock filesystem operations for testing',
      inputSchema: {
        type: 'object',
        properties: {
          action: { type: 'string', enum: ['read', 'write', 'list'] },
          path: { type: 'string' },
          content: { type: 'string' },
        },
        required: ['action', 'path'],
      },
    },
    async (args) => {
      switch (args.action) {
        case 'read':
          if (mockFiles[args.path] !== undefined) return `File content of ${args.path}:\n${mockFiles[args.path]}`;
          throw new Error(`File not found: ${args.path}`);
        case 'write':
          if (args.content === undefined) throw new Error('Content is required for write operation');
          mockFiles[args.path] = args.content;
          return `Successfully wrote ${args.content.length} characters to ${args.path}`;
        case 'list':
          return `Files in mock filesystem:\n${Object.keys(mockFiles).join('\n')}`;
        default:
          throw new Error(`Unknown filesystem action: ${args.action}`);
      }
    }
  );

  // 5. weather
  server.addTool(
    {
      name: 'weather',
      description: 'Mock weather service for testing external API simulation',
      inputSchema: {
        type: 'object',
        properties: {
          location: { type: 'string' },
          units: { type: 'string', enum: ['celsius', 'fahrenheit'], default: 'celsius' },
        },
        required: ['location'],
      },
    },
    async (args) => {
      const weatherData = {
        'new york': { celsius: 15, fahrenheit: 59, condition: 'Cloudy' },
        'london': { celsius: 12, fahrenheit: 54, condition: 'Rainy' },
        'tokyo': { celsius: 22, fahrenheit: 72, condition: 'Sunny' },
        'default': { celsius: 20, fahrenheit: 68, condition: 'Clear' },
      };
      const units = args.units || 'celsius';
      const key = args.location.toLowerCase();
      const w = weatherData[key] || weatherData.default;
      return `Weather in ${args.location}: ${w[units]}°${units === 'celsius' ? 'C' : 'F'}, ${w.condition}`;
    }
  );

  // 6. error_test
  server.addTool(
    {
      name: 'error_test',
      description: 'Tool that generates various types of errors for testing error handling',
      inputSchema: {
        type: 'object',
        properties: {
          error_type: { type: 'string', enum: ['validation', 'runtime', 'timeout'], default: 'runtime' },
          message: { type: 'string' },
        },
      },
    },
    async (args) => {
      const errorType = args.error_type || 'runtime';
      switch (errorType) {
        case 'validation':
          throw new Error(args.message || 'This is a validation error for testing');
        case 'runtime':
          throw new Error(args.message || 'This is a runtime error for testing');
        case 'timeout':
          await new Promise(resolve => setTimeout(resolve, 30000));
          return 'This should never be reached due to timeout';
        default:
          throw new Error(`Unknown error type: ${errorType}`);
      }
    }
  );

  // 7. slow_operation
  server.addTool(
    {
      name: 'slow_operation',
      description: 'Tool that simulates slow operations for testing timeouts',
      inputSchema: {
        type: 'object',
        properties: {
          delay_ms: { type: 'number', minimum: 0, maximum: 10000, default: 1000 },
          result: { type: 'string', default: 'completed' },
        },
      },
    },
    async (args) => {
      const delay = args.delay_ms ?? 1000;
      const result = args.result || 'completed';
      await new Promise(resolve => setTimeout(resolve, delay));
      return `Slow operation ${result} after ${delay}ms`;
    }
  );

  server.getDataStore = () => dataStore;
  return server;
}
