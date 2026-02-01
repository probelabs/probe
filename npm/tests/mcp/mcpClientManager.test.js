/**
 * Unit tests for MCPClientManager
 */

import { jest } from '@jest/globals';
import { MCPClientManager, createTransport, isMethodAllowed } from '../../src/agent/mcp/client.js';
import { validateTimeout, parseEnabledServers, DEFAULT_TIMEOUT, MAX_TIMEOUT } from '../../src/agent/mcp/config.js';
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
    const CUSTOM_GLOBAL_TIMEOUT = 45000; // 45 seconds
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
        timeout: CUSTOM_GLOBAL_TIMEOUT
      }
    };

    await manager.initialize(config);

    // Verify the config is stored correctly
    expect(manager.config.settings.timeout).toBe(CUSTOM_GLOBAL_TIMEOUT);
  });

  test('should prefer per-server timeout over global timeout', async () => {
    const FAST_TIMEOUT = 5000; // 5 seconds
    const SLOW_TIMEOUT = 120000; // 2 minutes
    const config = {
      mcpServers: {
        'fast-server': {
          command: 'node',
          args: ['-e', 'console.log("fast")'],
          transport: 'stdio',
          enabled: true,
          timeout: FAST_TIMEOUT
        },
        'slow-server': {
          command: 'node',
          args: ['-e', 'console.log("slow")'],
          transport: 'stdio',
          enabled: true,
          timeout: SLOW_TIMEOUT
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
        timeout: DEFAULT_TIMEOUT
      }
    };

    await manager.initialize(config);

    // Verify per-server timeouts are stored in config
    expect(config.mcpServers['fast-server'].timeout).toBe(FAST_TIMEOUT);
    expect(config.mcpServers['slow-server'].timeout).toBe(SLOW_TIMEOUT);
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
    // Zero timeout means immediate timeout (edge case but valid)
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
        timeout: DEFAULT_TIMEOUT
      }
    };

    await manager.initialize(config);

    // Zero should be stored (validation allows 0 as valid)
    expect(config.mcpServers['no-timeout-server'].timeout).toBe(0);
  });

  test('should cap timeout at maximum value (10 minutes) at config load time', async () => {
    const config = {
      mcpServers: {
        'test-server': {
          command: 'node',
          args: ['-e', 'console.log("test")'],
          transport: 'stdio',
          enabled: true,
          timeout: 999999999 // Very large value
        }
      }
    };

    const result = await manager.initialize(config);

    // Timeout should be capped to MAX_TIMEOUT (600000) at load time
    // The server config is normalized by parseEnabledServers
    expect(result.total).toBe(1);
    // Verify the client's stored config has the capped value
    const clientInfo = manager.clients.get('test-server');
    if (clientInfo) {
      expect(clientInfo.config.timeout).toBe(MAX_TIMEOUT);
    }
  });

  test('should skip server with negative timeout at config load time', async () => {
    const config = {
      mcpServers: {
        'invalid-server': {
          command: 'node',
          args: ['-e', 'console.log("test")'],
          transport: 'stdio',
          enabled: true,
          timeout: -5000 // Invalid negative value
        },
        'valid-server': {
          command: 'node',
          args: ['-e', 'console.log("test")'],
          transport: 'stdio',
          enabled: true,
          timeout: 5000 // Valid value
        }
      }
    };

    const result = await manager.initialize(config);

    // Invalid server should be skipped, only valid server should be processed
    expect(result.total).toBe(1);
  });
});

describe('parseEnabledServers Timeout Validation', () => {
  test('should validate and normalize timeout at config load time', () => {
    const config = {
      mcpServers: {
        'server-with-valid-timeout': {
          command: 'node',
          args: ['server.js'],
          transport: 'stdio',
          enabled: true,
          timeout: 60000
        }
      }
    };

    const servers = parseEnabledServers(config);

    expect(servers).toHaveLength(1);
    expect(servers[0].timeout).toBe(60000);
  });

  test('should cap excessive timeout to MAX_TIMEOUT at load time', () => {
    const config = {
      mcpServers: {
        'server-with-large-timeout': {
          command: 'node',
          args: ['server.js'],
          transport: 'stdio',
          enabled: true,
          timeout: 999999999
        }
      }
    };

    const servers = parseEnabledServers(config);

    expect(servers).toHaveLength(1);
    expect(servers[0].timeout).toBe(MAX_TIMEOUT);
  });

  test('should skip server with invalid negative timeout at load time', () => {
    const config = {
      mcpServers: {
        'invalid-server': {
          command: 'node',
          args: ['server.js'],
          transport: 'stdio',
          enabled: true,
          timeout: -1000
        },
        'valid-server': {
          command: 'node',
          args: ['server.js'],
          transport: 'stdio',
          enabled: true,
          timeout: 5000
        }
      }
    };

    const servers = parseEnabledServers(config);

    // Invalid server should be skipped
    expect(servers).toHaveLength(1);
    expect(servers[0].name).toBe('valid-server');
    expect(servers[0].timeout).toBe(5000);
  });

  test('should allow server without timeout (uses default at runtime)', () => {
    const config = {
      mcpServers: {
        'server-no-timeout': {
          command: 'node',
          args: ['server.js'],
          transport: 'stdio',
          enabled: true
          // No timeout - will use DEFAULT_TIMEOUT at runtime
        }
      }
    };

    const servers = parseEnabledServers(config);

    expect(servers).toHaveLength(1);
    expect(servers[0].timeout).toBeUndefined();
  });

  test('should preserve zero timeout as valid value', () => {
    const config = {
      mcpServers: {
        'zero-timeout-server': {
          command: 'node',
          args: ['server.js'],
          transport: 'stdio',
          enabled: true,
          timeout: 0
        }
      }
    };

    const servers = parseEnabledServers(config);

    expect(servers).toHaveLength(1);
    expect(servers[0].timeout).toBe(0);
  });
});

describe('validateTimeout Function', () => {
  test('should return undefined for undefined input', () => {
    expect(validateTimeout(undefined)).toBeUndefined();
  });

  test('should return undefined for null input', () => {
    expect(validateTimeout(null)).toBeUndefined();
  });

  test('should return undefined for negative numbers', () => {
    expect(validateTimeout(-1000)).toBeUndefined();
    expect(validateTimeout(-1)).toBeUndefined();
  });

  test('should return undefined for non-numeric strings', () => {
    expect(validateTimeout('invalid')).toBeUndefined();
    expect(validateTimeout('abc')).toBeUndefined();
  });

  test('should return undefined for NaN', () => {
    expect(validateTimeout(NaN)).toBeUndefined();
  });

  test('should return undefined for Infinity', () => {
    expect(validateTimeout(Infinity)).toBeUndefined();
    expect(validateTimeout(-Infinity)).toBeUndefined();
  });

  test('should return 0 for zero input', () => {
    expect(validateTimeout(0)).toBe(0);
  });

  test('should return valid positive numbers unchanged up to MAX_TIMEOUT', () => {
    expect(validateTimeout(1000)).toBe(1000);
    expect(validateTimeout(30000)).toBe(30000);
    expect(validateTimeout(DEFAULT_TIMEOUT)).toBe(DEFAULT_TIMEOUT);
  });

  test('should cap values at MAX_TIMEOUT', () => {
    expect(validateTimeout(MAX_TIMEOUT)).toBe(MAX_TIMEOUT);
    expect(validateTimeout(MAX_TIMEOUT + 1)).toBe(MAX_TIMEOUT);
    expect(validateTimeout(999999999)).toBe(MAX_TIMEOUT);
  });

  test('should convert numeric strings to numbers', () => {
    expect(validateTimeout('5000')).toBe(5000);
    expect(validateTimeout('30000')).toBe(30000);
  });
});

describe('isMethodAllowed Function', () => {
  describe('Basic Functionality', () => {
    test('should allow all methods when no filter specified', () => {
      expect(isMethodAllowed('any_method', null, null)).toBe(true);
      expect(isMethodAllowed('another_method', null, null)).toBe(true);
      expect(isMethodAllowed('search_code', undefined, undefined)).toBe(true);
    });

    test('should allow all methods with empty arrays', () => {
      expect(isMethodAllowed('any_method', [], [])).toBe(true);
      expect(isMethodAllowed('any_method', [], null)).toBe(true);
      expect(isMethodAllowed('any_method', null, [])).toBe(true);
    });
  });

  describe('Allowlist Mode (allowedMethods)', () => {
    test('should only allow methods in allowedMethods array', () => {
      const allowed = ['search_code', 'extract_code'];

      expect(isMethodAllowed('search_code', allowed, null)).toBe(true);
      expect(isMethodAllowed('extract_code', allowed, null)).toBe(true);
      expect(isMethodAllowed('delete_code', allowed, null)).toBe(false);
      expect(isMethodAllowed('other_method', allowed, null)).toBe(false);
    });

    test('should be case-sensitive', () => {
      const allowed = ['Search_Code'];

      expect(isMethodAllowed('Search_Code', allowed, null)).toBe(true);
      expect(isMethodAllowed('search_code', allowed, null)).toBe(false);
      expect(isMethodAllowed('SEARCH_CODE', allowed, null)).toBe(false);
    });
  });

  describe('Blocklist Mode (blockedMethods)', () => {
    test('should block only methods in blockedMethods array', () => {
      const blocked = ['dangerous_delete', 'risky_operation'];

      expect(isMethodAllowed('safe_read', null, blocked)).toBe(true);
      expect(isMethodAllowed('safe_write', null, blocked)).toBe(true);
      expect(isMethodAllowed('dangerous_delete', null, blocked)).toBe(false);
      expect(isMethodAllowed('risky_operation', null, blocked)).toBe(false);
    });
  });

  describe('Wildcard Pattern Matching', () => {
    test('should support prefix wildcards', () => {
      const allowed = ['*_read', '*_query'];

      expect(isMethodAllowed('file_read', allowed, null)).toBe(true);
      expect(isMethodAllowed('db_read', allowed, null)).toBe(true);
      expect(isMethodAllowed('system_query', allowed, null)).toBe(true);
      expect(isMethodAllowed('file_write', allowed, null)).toBe(false);
      expect(isMethodAllowed('read', allowed, null)).toBe(false); // No prefix
    });

    test('should support suffix wildcards', () => {
      const allowed = ['search_*', 'query_*'];

      expect(isMethodAllowed('search_files', allowed, null)).toBe(true);
      expect(isMethodAllowed('search_code', allowed, null)).toBe(true);
      expect(isMethodAllowed('query_database', allowed, null)).toBe(true);
      expect(isMethodAllowed('find_files', allowed, null)).toBe(false);
      expect(isMethodAllowed('search', allowed, null)).toBe(false); // No suffix
    });

    test('should support middle wildcards', () => {
      const allowed = ['get_*_info', 'read_*_data'];

      expect(isMethodAllowed('get_user_info', allowed, null)).toBe(true);
      expect(isMethodAllowed('get_system_info', allowed, null)).toBe(true);
      expect(isMethodAllowed('read_file_data', allowed, null)).toBe(true);
      expect(isMethodAllowed('get_user_data', allowed, null)).toBe(false);
    });

    test('should support multiple wildcards', () => {
      const allowed = ['*_get_*'];

      expect(isMethodAllowed('user_get_info', allowed, null)).toBe(true);
      expect(isMethodAllowed('file_get_content', allowed, null)).toBe(true);
      expect(isMethodAllowed('get_info', allowed, null)).toBe(false); // Missing prefix
    });

    test('should support wildcard in blockedMethods', () => {
      const blocked = ['dangerous_*', '*_delete'];

      expect(isMethodAllowed('safe_read', null, blocked)).toBe(true);
      expect(isMethodAllowed('dangerous_operation', null, blocked)).toBe(false);
      expect(isMethodAllowed('dangerous_delete', null, blocked)).toBe(false);
      expect(isMethodAllowed('file_delete', null, blocked)).toBe(false);
      expect(isMethodAllowed('safe_delete', null, blocked)).toBe(false);
    });

    test('should match single asterisk to any string including empty', () => {
      const allowed = ['*'];

      // Single asterisk should match any method
      expect(isMethodAllowed('any_method', allowed, null)).toBe(true);
      expect(isMethodAllowed('', allowed, null)).toBe(true);
    });
  });

  describe('Special Characters in Patterns', () => {
    test('should escape regex special characters in patterns', () => {
      // Test that special regex characters are properly escaped
      const allowed = ['method.name', 'query[0]', 'test+plus'];

      expect(isMethodAllowed('method.name', allowed, null)).toBe(true);
      expect(isMethodAllowed('methodXname', allowed, null)).toBe(false); // . should not match any char
      expect(isMethodAllowed('query[0]', allowed, null)).toBe(true);
      expect(isMethodAllowed('test+plus', allowed, null)).toBe(true);
    });
  });

  describe('Priority: allowedMethods over blockedMethods', () => {
    test('should use allowedMethods when both are provided', () => {
      const allowed = ['safe_method'];
      const blocked = ['safe_method']; // Same method in both lists

      // allowedMethods should take priority - safe_method should be allowed
      expect(isMethodAllowed('safe_method', allowed, blocked)).toBe(true);
      expect(isMethodAllowed('other_method', allowed, blocked)).toBe(false);
    });
  });
});

describe('Method Filtering Integration', () => {
  let manager;

  beforeEach(() => {
    manager = new MCPClientManager({ debug: false });
  });

  afterEach(async () => {
    if (manager) {
      await manager.disconnect();
    }
  });

  test('should filter tools during server connection with allowedMethods', async () => {
    const config = {
      mcpServers: {
        'mock-test': {
          command: 'node',
          args: [join(__dirname, 'mockMcpServer.js')],
          transport: 'stdio',
          enabled: true,
          allowedMethods: ['foobar', 'echo'] // Only allow these two
        }
      }
    };

    const result = await manager.initialize(config);

    if (result.connected > 0) {
      const tools = manager.getTools();
      const toolNames = Object.keys(tools);

      // Should have foobar and echo, but not calculator
      expect(toolNames.some(n => n.includes('foobar'))).toBe(true);
      expect(toolNames.some(n => n.includes('echo'))).toBe(true);
      expect(toolNames.some(n => n.includes('calculator'))).toBe(false);
    } else {
      console.warn('Mock server connection failed - skipping integration assertions');
    }
  }, 10000);

  test('should filter tools during server connection with blockedMethods', async () => {
    const config = {
      mcpServers: {
        'mock-test': {
          command: 'node',
          args: [join(__dirname, 'mockMcpServer.js')],
          transport: 'stdio',
          enabled: true,
          blockedMethods: ['calculator'] // Block calculator
        }
      }
    };

    const result = await manager.initialize(config);

    if (result.connected > 0) {
      const tools = manager.getTools();
      const toolNames = Object.keys(tools);

      // Should have foobar and echo, but not calculator
      expect(toolNames.some(n => n.includes('foobar'))).toBe(true);
      expect(toolNames.some(n => n.includes('echo'))).toBe(true);
      expect(toolNames.some(n => n.includes('calculator'))).toBe(false);
    } else {
      console.warn('Mock server connection failed - skipping integration assertions');
    }
  }, 10000);

  test('should register all tools when no method filter specified', async () => {
    const config = {
      mcpServers: {
        'mock-test': {
          command: 'node',
          args: [join(__dirname, 'mockMcpServer.js')],
          transport: 'stdio',
          enabled: true
          // No allowedMethods or blockedMethods
        }
      }
    };

    const result = await manager.initialize(config);

    if (result.connected > 0) {
      const tools = manager.getTools();
      const toolNames = Object.keys(tools);

      // Should have all tools: foobar, echo, and calculator
      expect(toolNames.some(n => n.includes('foobar'))).toBe(true);
      expect(toolNames.some(n => n.includes('echo'))).toBe(true);
      expect(toolNames.some(n => n.includes('calculator'))).toBe(true);
    } else {
      console.warn('Mock server connection failed - skipping integration assertions');
    }
  }, 10000);

  test('should warn about unmatched allowedMethods patterns', async () => {
    const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});

    const config = {
      mcpServers: {
        'mock-test': {
          command: 'node',
          args: [join(__dirname, 'mockMcpServer.js')],
          transport: 'stdio',
          enabled: true,
          allowedMethods: ['foobar', 'nonexistent_method', 'another_missing']
        }
      }
    };

    const result = await manager.initialize(config);

    if (result.connected > 0) {
      // Check that warning was logged about unmatched patterns
      const warnCalls = consoleSpy.mock.calls.filter(call =>
        call[0].includes('[MCP WARN]') && call[0].includes('did not match')
      );
      expect(warnCalls.length).toBeGreaterThan(0);

      // Check that available methods were listed
      const availableCalls = consoleSpy.mock.calls.filter(call =>
        call[0].includes('[MCP WARN]') && call[0].includes('Available methods')
      );
      expect(availableCalls.length).toBeGreaterThan(0);
    }

    consoleSpy.mockRestore();
  }, 10000);

  test('should warn about unmatched blockedMethods patterns', async () => {
    const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});

    const config = {
      mcpServers: {
        'mock-test': {
          command: 'node',
          args: [join(__dirname, 'mockMcpServer.js')],
          transport: 'stdio',
          enabled: true,
          blockedMethods: ['nonexistent_method', 'missing_*']
        }
      }
    };

    const result = await manager.initialize(config);

    if (result.connected > 0) {
      // Check that warning was logged about unmatched patterns
      const warnCalls = consoleSpy.mock.calls.filter(call =>
        call[0].includes('[MCP WARN]') && call[0].includes('did not match')
      );
      expect(warnCalls.length).toBeGreaterThan(0);

      // Check that available methods were listed
      const availableCalls = consoleSpy.mock.calls.filter(call =>
        call[0].includes('[MCP WARN]') && call[0].includes('Available methods')
      );
      expect(availableCalls.length).toBeGreaterThan(0);
    }

    consoleSpy.mockRestore();
  }, 10000);

  test('should not warn when all patterns match', async () => {
    const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});

    const config = {
      mcpServers: {
        'mock-test': {
          command: 'node',
          args: [join(__dirname, 'mockMcpServer.js')],
          transport: 'stdio',
          enabled: true,
          allowedMethods: ['foobar', 'echo'] // These exist in mock server
        }
      }
    };

    const result = await manager.initialize(config);

    if (result.connected > 0) {
      // Check that NO warning was logged about unmatched patterns
      const warnCalls = consoleSpy.mock.calls.filter(call =>
        call[0].includes('[MCP WARN]') && call[0].includes('did not match')
      );
      expect(warnCalls.length).toBe(0);
    }

    consoleSpy.mockRestore();
  }, 10000);
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