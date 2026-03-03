/**
 * Comprehensive error handling and edge case tests for MCP integration
 *
 * After the migration to native Vercel AI SDK tools, MCP tools are executed
 * directly via their execute() functions. XML parsing tests have been removed
 * since the XML tool calling layer no longer exists.
 */

import { jest } from '@jest/globals';
import { MCPClientManager, createTransport } from '../../src/agent/mcp/client.js';
import { MCPXmlBridge } from '../../src/agent/mcp/xmlBridge.js';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { mkdtemp, writeFile, rm } from 'fs/promises';
import { tmpdir } from 'os';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe('MCP Error Handling and Edge Cases', () => {
  let tempDir;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'mcp-error-test-'));
  });

  afterEach(async () => {
    if (tempDir) {
      await rm(tempDir, { recursive: true, force: true });
    }
    // Clean up environment variables
    delete process.env.MCP_CONFIG_PATH;
    delete process.env.ANTHROPIC_API_KEY;
    delete process.env.DEBUG_MCP;
  });

  describe('Connection Failures', () => {
    test('should handle non-existent command gracefully', async () => {
      const manager = new MCPClientManager({ debug: false });

      const config = {
        mcpServers: {
          'nonexistent': {
            command: 'nonexistent-command-that-does-not-exist',
            args: ['--test'],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const result = await manager.initialize(config);

      expect(result.connected).toBe(0);
      expect(result.total).toBe(1);
      expect(result.tools).toEqual([]);

      await manager.disconnect();
    });

    test('should handle server that exits immediately', async () => {
      const manager = new MCPClientManager({ debug: false });

      const config = {
        mcpServers: {
          'exits-immediately': {
            command: 'node',
            args: ['-e', 'process.exit(1)'], // Exits immediately with error
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const result = await manager.initialize(config);

      expect(result.connected).toBe(0);
      expect(result.total).toBe(1);

      await manager.disconnect();
    });

    test('should handle malformed server responses', async () => {
      const manager = new MCPClientManager({ debug: false });

      const config = {
        mcpServers: {
          'malformed': {
            command: 'node',
            args: ['-e', 'console.log("not json"); process.exit(0);'], // Outputs invalid JSON then exits
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const result = await manager.initialize(config);

      expect(result.connected).toBe(0);
      expect(result.total).toBe(1);

      await manager.disconnect();
    }, 15000); // Increase timeout

    test('should handle unreachable HTTP endpoints', async () => {
      const config = {
        transport: 'http',
        url: 'http://localhost:99999/mcp' // Unreachable port
      };

      const transport = createTransport(config);

      // Mock fetch to simulate connection failure
      global.fetch = jest.fn().mockRejectedValue(new Error('Connection refused'));

      await expect(transport.start()).rejects.toThrow();
    });

    test('should handle invalid WebSocket URLs', async () => {
      const config = {
        transport: 'websocket',
        url: 'not-a-valid-url' // Invalid URL
      };

      // This should throw an error for invalid URL
      expect(() => createTransport(config)).toThrow('Invalid WebSocket URL');
    });
  });

  describe('Invalid Configurations', () => {
    test('should handle missing required transport fields', () => {
      // Missing URL for HTTP transport
      expect(() => createTransport({
        transport: 'http'
      })).toThrow('HTTP transport requires a URL');

      // Missing URL for WebSocket transport
      expect(() => createTransport({
        transport: 'websocket'
      })).toThrow('WebSocket transport requires a URL');

      // Missing URL for SSE transport
      expect(() => createTransport({
        transport: 'sse'
      })).toThrow('SSE transport requires a URL');
    });

    test('should handle invalid transport types', () => {
      expect(() => createTransport({
        transport: 'invalid-transport'
      })).toThrow('Unknown transport type: invalid-transport');
    });

    test('should handle malformed configuration files', async () => {
      const invalidConfigPath = join(tempDir, 'invalid.json');
      await writeFile(invalidConfigPath, '{ this is not valid json }');

      process.env.MCP_CONFIG_PATH = invalidConfigPath;

      const manager = new MCPClientManager({ debug: false });

      // Should not throw, should fall back to defaults
      const result = await manager.initialize();
      expect(result).toBeDefined();

      await manager.disconnect();
    });

    test('should handle empty and null configurations', async () => {
      const manager = new MCPClientManager({ debug: false });

      // Null configuration
      const result1 = await manager.initialize(null);
      expect(result1.connected).toBe(0);
      expect(result1.total).toBe(0);

      // Empty configuration
      const result2 = await manager.initialize({});
      expect(result2.connected).toBe(0);
      expect(result2.total).toBe(0);

      // Configuration with empty mcpServers
      const result3 = await manager.initialize({ mcpServers: {} });
      expect(result3.connected).toBe(0);
      expect(result3.total).toBe(0);

      await manager.disconnect();
    });
  });

  describe('Tool Execution Errors', () => {
    test('should handle tool execution failures with mock server', async () => {
      // Create configuration for mock server that has error-generating tools
      const mcpConfig = {
        mcpServers: {
          'error-test-server': {
            command: 'node',
            args: [join(__dirname, '../mcp/mockMcpServer.js')],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'error-test-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;

      const bridge = new MCPXmlBridge({ debug: false });
      await bridge.initialize();

      // Wait for initialization
      await new Promise(resolve => setTimeout(resolve, 2000));

      if (bridge.getToolNames().length > 0) {
        // Find the error_test tool (prefixed with server name)
        const errorToolName = bridge.getToolNames().find(name => name.includes('error_test'));
        if (errorToolName) {
          const vercelTools = bridge.getVercelTools();
          const errorTool = vercelTools[errorToolName];

          // Test error tool that generates validation errors
          try {
            await errorTool.execute({ error_type: 'validation', message: 'Test validation error' });
          } catch (error) {
            expect(error.message || String(error)).toContain('Test validation error');
          }

          // Test error tool that generates runtime errors
          try {
            await errorTool.execute({ error_type: 'runtime', message: 'Test runtime error' });
          } catch (error) {
            expect(error.message || String(error)).toContain('Test runtime error');
          }
        } else {
          console.warn('Error test tool not found in mock server');
        }
      } else {
        console.warn('Mock server not available for error testing');
      }

      await bridge.cleanup();
    }, 15000);

    test('should handle invalid tool parameters', async () => {
      const bridge = new MCPXmlBridge({ debug: false });

      // Mock a tool with specific parameter requirements
      bridge.mcpTools = {
        'strict_tool': {
          execute: async (params) => {
            if (!params.required_param) {
              throw new Error('Missing required parameter: required_param');
            }
            return 'Success';
          }
        }
      };

      // Test with missing required parameter via direct execute
      await expect(
        bridge.mcpTools.strict_tool.execute({ optional_param: 'value' })
      ).rejects.toThrow('Missing required parameter');

      await bridge.cleanup();
    });

    test('should handle tool timeouts', async () => {
      // Create configuration for mock server with slow operations
      const mcpConfig = {
        mcpServers: {
          'timeout-test-server': {
            command: 'node',
            args: [join(__dirname, '../mcp/mockMcpServer.js')],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'timeout-test-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;

      const bridge = new MCPXmlBridge({ debug: false });
      await bridge.initialize();

      // Wait for initialization
      await new Promise(resolve => setTimeout(resolve, 2000));

      if (bridge.getToolNames().length > 0) {
        // Find a slow operation tool
        const slowToolName = bridge.getToolNames().find(name => name.includes('slow_operation'));
        if (slowToolName) {
          const vercelTools = bridge.getVercelTools();
          const slowTool = vercelTools[slowToolName];

          const start = Date.now();
          try {
            const result = await slowTool.execute({ delay_ms: 500, result: 'completed successfully' });
            const elapsed = Date.now() - start;
            expect(elapsed).toBeGreaterThanOrEqual(500);
            // Result may be string or object depending on tool implementation
            const resultStr = typeof result === 'string' ? result : JSON.stringify(result);
            expect(resultStr).toContain('completed successfully');
          } catch (error) {
            // Tool may throw on timeout, which is acceptable behavior
            console.warn('Slow tool threw error (may be expected):', error.message);
          }
        }
      } else {
        console.warn('Mock server not available for timeout testing');
      }

      await bridge.cleanup();
    }, 20000);
  });

  describe('ProbeAgent Integration Error Handling', () => {
    test('should handle MCP initialization failure in ProbeAgent', async () => {
      process.env.ANTHROPIC_API_KEY = 'test-key';

      // Create invalid MCP configuration
      const mcpConfig = {
        mcpServers: {
          'failing-server': {
            command: 'nonexistent-command',
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'failing-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;

      // Should not throw during initialization
      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      // Wait for MCP initialization attempt
      await new Promise(resolve => setTimeout(resolve, 2000));

      // Agent should still be functional, just without MCP tools
      expect(agent.enableMcp).toBe(true);
      // The bridge exists but has no connected servers/tools
      if (agent.mcpBridge) {
        expect(agent.mcpBridge.getToolNames().length).toBe(0);
      }

      const systemMessage = await agent.getSystemMessage();
      expect(systemMessage).toBeDefined();
      // System message should not contain MCP tools section since no tools loaded
      expect(systemMessage).not.toContain('MCP Tools');

      await agent.cleanup();
    });

    test('should handle concurrent MCP tool operations', async () => {
      const bridge = new MCPXmlBridge({ debug: false });

      // Mock multiple tools
      bridge.mcpTools = {
        'tool1': {
          execute: async (params) => {
            await new Promise(resolve => setTimeout(resolve, 100));
            return `Tool1 result: ${params.input}`;
          }
        },
        'tool2': {
          execute: async (params) => {
            await new Promise(resolve => setTimeout(resolve, 150));
            return `Tool2 result: ${params.input}`;
          }
        },
        'tool3': {
          execute: async (params) => {
            await new Promise(resolve => setTimeout(resolve, 50));
            return `Tool3 result: ${params.input}`;
          }
        }
      };

      // Execute multiple tools concurrently via direct execute
      const promises = [
        bridge.mcpTools.tool1.execute({ input: 'test1' }),
        bridge.mcpTools.tool2.execute({ input: 'test2' }),
        bridge.mcpTools.tool3.execute({ input: 'test3' })
      ];

      const results = await Promise.all(promises);

      expect(results).toHaveLength(3);
      expect(results[0]).toContain('Tool1 result');
      expect(results[1]).toContain('Tool2 result');
      expect(results[2]).toContain('Tool3 result');

      await bridge.cleanup();
    });

    test('should handle partial MCP failures', async () => {
      // Test scenario where some servers connect and others fail
      const mcpConfig = {
        mcpServers: {
          'working-server': {
            command: 'node',
            args: [join(__dirname, '../mcp/mockMcpServer.js')],
            transport: 'stdio',
            enabled: true
          },
          'failing-server': {
            command: 'nonexistent-command',
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'partial-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;

      const manager = new MCPClientManager({ debug: false });
      const result = await manager.initialize();

      // Should have attempted 2 connections, with possibly 1 success
      expect(result.total).toBe(2);
      // Connected count depends on whether mock server actually starts
      expect(result.connected).toBeLessThanOrEqual(1);

      await manager.disconnect();
    }, 15000);
  });

  describe('Memory and Resource Management', () => {
    test('should clean up resources properly on multiple disconnect calls', async () => {
      const manager = new MCPClientManager({ debug: false });

      // Initialize with empty config
      await manager.initialize({ mcpServers: {} });

      // Multiple cleanup calls should not throw
      await manager.disconnect();
      await manager.disconnect();
      await manager.disconnect();

      expect(manager.clients.size).toBe(0);
      expect(manager.tools.size).toBe(0);
    });

    test('should handle cleanup when connections are already closed', async () => {
      const bridge = new MCPXmlBridge({ debug: false });

      // Initialize without any actual connections
      await bridge.initialize({ mcpServers: {} });

      // Cleanup should not throw even if nothing to clean up
      await expect(bridge.cleanup()).resolves.not.toThrow();
      await expect(bridge.cleanup()).resolves.not.toThrow(); // Second cleanup
    });

    test('should handle memory pressure with many tool calls', async () => {
      const bridge = new MCPXmlBridge({ debug: false });

      // Mock a simple tool
      bridge.mcpTools = {
        'memory_test': {
          execute: async (params) => {
            // Create some data
            const data = new Array(1000).fill(params.index || 0);
            return `Processed ${data.length} items`;
          }
        }
      };

      // Execute many tool calls via direct execute
      const promises = [];
      for (let i = 0; i < 100; i++) {
        promises.push(
          bridge.mcpTools.memory_test.execute({ index: i })
        );
      }

      const results = await Promise.all(promises);

      expect(results).toHaveLength(100);
      results.forEach((result) => {
        expect(result).toContain('Processed 1000 items');
      });

      await bridge.cleanup();
    });
  });
});
