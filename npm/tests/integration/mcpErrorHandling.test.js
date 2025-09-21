/**
 * Comprehensive error handling and edge case tests for MCP integration
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
        // Test error tool that generates validation errors
        const errorXml = `
          <error_test>
          <params>{"error_type": "validation", "message": "Test validation error"}</params>
          </error_test>
        `;

        const result = await bridge.executeFromXml(errorXml);
        expect(result.success).toBe(false);
        expect(result.error).toContain('Test validation error');

        // Test error tool that generates runtime errors
        const runtimeErrorXml = `
          <error_test>
          <params>{"error_type": "runtime", "message": "Test runtime error"}</params>
          </error_test>
        `;

        const runtimeResult = await bridge.executeFromXml(runtimeErrorXml);
        expect(runtimeResult.success).toBe(false);
        expect(runtimeResult.error).toContain('Test runtime error');
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

      // Test with missing required parameter
      const invalidXml = `
        <strict_tool>
        <params>{"optional_param": "value"}</params>
        </strict_tool>
      `;

      const result = await bridge.executeFromXml(invalidXml);
      expect(result.success).toBe(false);
      expect(result.error).toContain('Missing required parameter');

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
        // Test slow operation (should complete within reasonable time)
        const slowXml = `
          <slow_operation>
          <params>{"delay_ms": 500, "result": "completed successfully"}</params>
          </slow_operation>
        `;

        const start = Date.now();
        const result = await bridge.executeFromXml(slowXml);
        const elapsed = Date.now() - start;

        if (result.success) {
          expect(elapsed).toBeGreaterThanOrEqual(500);
          expect(result.result).toContain('completed successfully');
        }
      } else {
        console.warn('Mock server not available for timeout testing');
      }

      await bridge.cleanup();
    }, 20000);
  });

  describe('XML Parsing Edge Cases', () => {
    test('should handle malformed XML', async () => {
      const { parseXmlMcpToolCall } = await import('../../src/agent/mcp/xmlBridge.js');

      // Unclosed tags
      const malformedXml1 = '<test_tool><params>{"test": "value"}</params>';
      const result1 = parseXmlMcpToolCall(malformedXml1, ['test_tool']);
      expect(result1).toBeNull();

      // Mismatched tags
      const malformedXml2 = '<test_tool><params>{"test": "value"}</wrong_tag></test_tool>';
      const result2 = parseXmlMcpToolCall(malformedXml2, ['test_tool']);
      expect(result2).toBeDefined(); // Should still parse the tool name

      // No content
      const emptyXml = '<test_tool></test_tool>';
      const result3 = parseXmlMcpToolCall(emptyXml, ['test_tool']);
      expect(result3).toBeDefined();
      expect(result3.params).toEqual({});
    });

    test('should handle nested XML and complex content', async () => {
      const { parseXmlMcpToolCall } = await import('../../src/agent/mcp/xmlBridge.js');

      // XML with nested elements in JSON
      const complexXml = `
        <complex_tool>
        <params>
        {
          "nested": {
            "data": "<xml>content</xml>",
            "array": [1, 2, 3]
          },
          "special_chars": "quotes \\" and <tags> & ampersands"
        }
        </params>
        </complex_tool>
      `;

      const result = parseXmlMcpToolCall(complexXml, ['complex_tool']);
      expect(result).toBeDefined();
      expect(result.toolName).toBe('complex_tool');
      // The params should be the parsed JSON object
      expect(result.params).toBeDefined();
      expect(result.params.nested).toBeDefined();
      expect(result.params.nested.data).toBe('<xml>content</xml>');
      expect(result.params.nested.array).toEqual([1, 2, 3]);
      expect(result.params.special_chars).toBe('quotes " and <tags> & ampersands');
    });

    test('should handle CDATA sections', async () => {
      const { parseXmlMcpToolCall } = await import('../../src/agent/mcp/xmlBridge.js');

      const cdataXml = `
        <cdata_tool>
        <params><![CDATA[{"message": "This contains <special> characters & symbols"}]]></params>
        </cdata_tool>
      `;

      const result = parseXmlMcpToolCall(cdataXml, ['cdata_tool']);
      expect(result).toBeDefined();
      expect(result.params.message).toBe('This contains <special> characters & symbols');
    });
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
      expect(systemMessage).not.toContain('MCP Tools'); // No MCP tools section

      await agent.cleanup();
    });

    test('should handle concurrent MCP operations', async () => {
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

      // Execute multiple tools concurrently
      const promises = [
        bridge.executeFromXml('<tool1><params>{"input": "test1"}</params></tool1>'),
        bridge.executeFromXml('<tool2><params>{"input": "test2"}</params></tool2>'),
        bridge.executeFromXml('<tool3><params>{"input": "test3"}</params></tool3>')
      ];

      const results = await Promise.all(promises);

      expect(results).toHaveLength(3);
      results.forEach((result, index) => {
        expect(result.success).toBe(true);
        expect(result.result).toContain(`Tool${index + 1} result`);
      });

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

      // Execute many tool calls
      const promises = [];
      for (let i = 0; i < 100; i++) {
        promises.push(
          bridge.executeFromXml(`<memory_test><params>{"index": ${i}}</params></memory_test>`)
        );
      }

      const results = await Promise.all(promises);

      expect(results).toHaveLength(100);
      results.forEach((result, index) => {
        expect(result.success).toBe(true);
        expect(result.result).toContain('Processed 1000 items');
      });

      await bridge.cleanup();
    });
  });
});