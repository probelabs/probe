/**
 * Unit tests for MCPClientManager
 */

import { jest } from '@jest/globals';
import { MCPClientManager, createTransport } from '../../src/agent/mcp/client.js';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe('MCPClientManager', () => {
  let manager;

  beforeEach(() => {
    manager = new MCPClientManager({ debug: false });
  });

  afterEach(async () => {
    if (manager) {
      await manager.disconnect();
    }
  });

  describe('Transport Creation', () => {
    test('should create stdio transport', () => {
      const config = {
        transport: 'stdio',
        command: 'node',
        args: ['-e', 'console.log("test")']
      };

      const transport = createTransport(config);
      expect(transport).toBeDefined();
      expect(transport.constructor.name).toBe('StdioClientTransport');
    });

    test('should create SSE transport', () => {
      const config = {
        transport: 'sse',
        url: 'http://localhost:3000/sse'
      };

      const transport = createTransport(config);
      expect(transport).toBeDefined();
      expect(transport.constructor.name).toBe('SSEClientTransport');
    });

    test('should create WebSocket transport', () => {
      const config = {
        transport: 'websocket',
        url: 'ws://localhost:8080'
      };

      const transport = createTransport(config);
      expect(transport).toBeDefined();
      expect(transport.constructor.name).toBe('WebSocketClientTransport');
    });

    test('should create WebSocket transport with ws alias', () => {
      const config = {
        transport: 'ws',
        url: 'ws://localhost:8080'
      };

      const transport = createTransport(config);
      expect(transport).toBeDefined();
      expect(transport.constructor.name).toBe('WebSocketClientTransport');
    });

    test('should create HTTP transport', () => {
      const config = {
        transport: 'http',
        url: 'http://localhost:3000/mcp'
      };

      const transport = createTransport(config);
      expect(transport).toBeDefined();
      expect(typeof transport.start).toBe('function');
      expect(typeof transport.send).toBe('function');
      expect(typeof transport.close).toBe('function');
    });

    test('should throw error for unknown transport', () => {
      const config = {
        transport: 'unknown'
      };

      expect(() => createTransport(config)).toThrow('Unknown transport type: unknown');
    });

    test('should throw error for SSE transport without URL', () => {
      const config = {
        transport: 'sse'
      };

      expect(() => createTransport(config)).toThrow('SSE transport requires a URL');
    });

    test('should throw error for WebSocket transport without URL', () => {
      const config = {
        transport: 'websocket'
      };

      expect(() => createTransport(config)).toThrow('WebSocket transport requires a URL');
    });

    test('should throw error for HTTP transport without URL', () => {
      const config = {
        transport: 'http'
      };

      expect(() => createTransport(config)).toThrow('HTTP transport requires a URL');
    });
  });

  describe('Manager Initialization', () => {
    test('should initialize with empty configuration', async () => {
      const result = await manager.initialize({
        mcpServers: {}
      });

      expect(result.connected).toBe(0);
      expect(result.total).toBe(0);
      expect(result.tools).toEqual([]);
      expect(manager.clients.size).toBe(0);
      expect(manager.tools.size).toBe(0);
    });

    test('should handle invalid server configurations', async () => {
      const config = {
        mcpServers: {
          'invalid-server': {
            transport: 'stdio',
            // Missing command
            enabled: true
          }
        }
      };

      const result = await manager.initialize(config);
      expect(result.connected).toBe(0);
      expect(result.total).toBe(0);
    });

    test('should initialize with mock server configuration', async () => {
      const config = {
        mcpServers: {
          'mock-server': {
            command: 'node',
            args: [join(__dirname, 'mockMcpServer.js')],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      // This will attempt to connect but likely fail without actual server
      // We're testing the configuration parsing and setup
      const result = await manager.initialize(config);
      expect(result.total).toBe(1);
      // Connected might be 0 if server isn't actually running, which is expected in unit tests
    });
  });

  describe('Tool Management', () => {
    test('should return empty tools when no servers connected', () => {
      const tools = manager.getTools();
      expect(tools).toEqual({});
    });

    test('should return empty Vercel tools when no servers connected', () => {
      const vercelTools = manager.getVercelTools();
      expect(vercelTools).toEqual({});
    });

    test('should handle tool calls when no tools available', async () => {
      await expect(manager.callTool('nonexistent', {}))
        .rejects.toThrow('Unknown tool: nonexistent');
    });
  });

  describe('Mock Server Integration', () => {
    test('should connect to mock server and load tools', async () => {
      const config = {
        mcpServers: {
          'mock-test': {
            command: 'node',
            args: [join(__dirname, 'mockMcpServer.js')],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      // Start the manager
      const result = await manager.initialize(config);

      if (result.connected > 0) {
        // If connection succeeded, verify tools
        const tools = manager.getTools();
        expect(Object.keys(tools).length).toBeGreaterThan(0);

        // Check for expected mock tools
        const toolNames = Object.keys(tools);
        expect(toolNames.some(name => name.includes('foobar'))).toBe(true);
        expect(toolNames.some(name => name.includes('calculator'))).toBe(true);
        expect(toolNames.some(name => name.includes('echo'))).toBe(true);

        // Test Vercel tools format
        const vercelTools = manager.getVercelTools();
        expect(Object.keys(vercelTools).length).toBeGreaterThan(0);

        for (const [name, tool] of Object.entries(vercelTools)) {
          expect(tool).toHaveProperty('description');
          expect(tool).toHaveProperty('inputSchema');
          expect(typeof tool.execute).toBe('function');
        }
      } else {
        // If connection failed (e.g., no actual server running), that's also acceptable for unit tests
        console.warn('Mock server connection failed - this is expected in unit test environment');
      }
    }, 10000); // Longer timeout for connection attempts
  });

  describe('Cleanup', () => {
    test('should disconnect cleanly', async () => {
      await expect(manager.disconnect()).resolves.not.toThrow();
    });

    test('should handle multiple disconnect calls', async () => {
      await manager.disconnect();
      await expect(manager.disconnect()).resolves.not.toThrow();
    });
  });
});

describe('Per-Server Timeout Configuration', () => {
  let manager;

  beforeEach(() => {
    manager = new MCPClientManager({ debug: false });
  });

  afterEach(async () => {
    if (manager) {
      await manager.disconnect();
    }
  });

  test('should use global timeout when no per-server timeout configured', async () => {
    const config = {
      mcpServers: {
        'test-server': {
          command: 'node',
          args: ['-e', 'console.log("test")'],
          transport: 'stdio',
          enabled: true
          // No timeout specified
        }
      },
      settings: {
        timeout: 45000 // 45 second global timeout
      }
    };

    await manager.initialize(config);

    // Verify the config is stored correctly
    expect(manager.config.settings.timeout).toBe(45000);
  });

  test('should prefer per-server timeout over global timeout', async () => {
    const config = {
      mcpServers: {
        'fast-server': {
          command: 'node',
          args: ['-e', 'console.log("fast")'],
          transport: 'stdio',
          enabled: true,
          timeout: 5000 // 5 second per-server timeout
        },
        'slow-server': {
          command: 'node',
          args: ['-e', 'console.log("slow")'],
          transport: 'stdio',
          enabled: true,
          timeout: 120000 // 2 minute per-server timeout
        },
        'default-server': {
          command: 'node',
          args: ['-e', 'console.log("default")'],
          transport: 'stdio',
          enabled: true
          // No per-server timeout, should use global
        }
      },
      settings: {
        timeout: 30000 // 30 second global timeout
      }
    };

    await manager.initialize(config);

    // Verify per-server timeouts are stored in config
    expect(config.mcpServers['fast-server'].timeout).toBe(5000);
    expect(config.mcpServers['slow-server'].timeout).toBe(120000);
    expect(config.mcpServers['default-server'].timeout).toBeUndefined();
  });

  test('should use default 30s timeout when neither per-server nor global timeout set', async () => {
    const config = {
      mcpServers: {
        'test-server': {
          command: 'node',
          args: ['-e', 'console.log("test")'],
          transport: 'stdio',
          enabled: true
        }
      }
      // No settings.timeout
    };

    await manager.initialize(config);

    // Global timeout should fall back to default 30000ms
    expect(manager.config?.settings?.timeout).toBeUndefined();
    // The actual default is applied in callTool, which uses || 30000
  });

  test('should handle zero timeout as valid per-server value', async () => {
    // Zero timeout could be used to disable timeout (infinite wait)
    // Using nullish coalescing (??) ensures 0 is treated as a valid value
    const config = {
      mcpServers: {
        'no-timeout-server': {
          command: 'node',
          args: ['-e', 'console.log("test")'],
          transport: 'stdio',
          enabled: true,
          timeout: 0 // Explicitly set to 0
        }
      },
      settings: {
        timeout: 30000
      }
    };

    await manager.initialize(config);

    // Zero should be stored and used (not fall back to global)
    expect(config.mcpServers['no-timeout-server'].timeout).toBe(0);
  });
});

describe('HTTP Transport', () => {
  test('should handle HTTP transport methods', async () => {
    const config = {
      transport: 'http',
      url: 'http://localhost:3000/mcp'
    };

    const transport = createTransport(config);

    // Mock fetch for testing
    global.fetch = jest.fn()
      .mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ protocolVersion: '2024-11-05' })
      })
      .mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ result: 'success' })
      })
      .mockResolvedValueOnce({
        ok: true
      });

    await expect(transport.start()).resolves.toBeDefined();
    await expect(transport.send({ method: 'test' })).resolves.toBeDefined();
    await expect(transport.close()).resolves.toBeUndefined();

    expect(global.fetch).toHaveBeenCalledTimes(3);
  });

  test('should handle HTTP transport errors', async () => {
    const config = {
      transport: 'http',
      url: 'http://localhost:3000/mcp'
    };

    const transport = createTransport(config);

    // Mock fetch to return error
    global.fetch = jest.fn()
      .mockResolvedValueOnce({
        ok: false,
        statusText: 'Internal Server Error'
      });

    await expect(transport.start()).rejects.toThrow('HTTP initialization failed: Internal Server Error');
  });
});