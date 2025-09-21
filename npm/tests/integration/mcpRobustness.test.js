/**
 * Robustness tests for MCP integration under stress and edge conditions
 */

import { jest } from '@jest/globals';
import { MCPClientManager } from '../../src/agent/mcp/client.js';
import { MCPXmlBridge } from '../../src/agent/mcp/xmlBridge.js';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { mkdtemp, writeFile, rm } from 'fs/promises';
import { tmpdir } from 'os';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe('MCP Robustness Tests', () => {
  let tempDir;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'mcp-robustness-test-'));
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

  describe('High Load Testing', () => {
    test('should handle rapid tool execution requests', async () => {
      const bridge = new MCPXmlBridge({ debug: false });

      // Mock a fast-responding tool
      bridge.mcpTools = {
        'fast_tool': {
          execute: async (params) => {
            return `Fast response for: ${params.input}`;
          }
        }
      };

      // Execute 50 tool calls rapidly
      const startTime = Date.now();
      const promises = [];

      for (let i = 0; i < 50; i++) {
        promises.push(
          bridge.executeFromXml(`<fast_tool><params>{"input": "request_${i}"}</params></fast_tool>`)
        );
      }

      const results = await Promise.all(promises);
      const endTime = Date.now();

      expect(results).toHaveLength(50);
      results.forEach((result, index) => {
        expect(result.success).toBe(true);
        expect(result.result).toContain(`request_${index}`);
      });

      console.log(`Executed 50 tool calls in ${endTime - startTime}ms`);
      expect(endTime - startTime).toBeLessThan(5000); // Should complete within 5 seconds

      await bridge.cleanup();
    });

    test('should handle mixed tool execution patterns', async () => {
      const bridge = new MCPXmlBridge({ debug: false });

      // Mock tools with different response times
      bridge.mcpTools = {
        'instant_tool': {
          execute: async (params) => `Instant: ${params.id}`
        },
        'slow_tool': {
          execute: async (params) => {
            await new Promise(resolve => setTimeout(resolve, 100));
            return `Slow: ${params.id}`;
          }
        },
        'variable_tool': {
          execute: async (params) => {
            const delay = Math.random() * 50;
            await new Promise(resolve => setTimeout(resolve, delay));
            return `Variable: ${params.id}`;
          }
        }
      };

      // Execute mixed pattern of tools
      const promises = [];
      const tools = ['instant_tool', 'slow_tool', 'variable_tool'];

      for (let i = 0; i < 30; i++) {
        const tool = tools[i % tools.length];
        promises.push(
          bridge.executeFromXml(`<${tool}><params>{"id": ${i}}</params></${tool}>`)
        );
      }

      const results = await Promise.all(promises);

      expect(results).toHaveLength(30);
      results.forEach((result, index) => {
        expect(result.success).toBe(true);
        expect(result.result).toContain(`${index}`);
      });

      await bridge.cleanup();
    });
  });

  describe('Network Resilience', () => {
    test('should handle HTTP transport with network simulation', async () => {
      let fetchCallCount = 0;
      let shouldFail = false;

      // Mock fetch with network simulation
      global.fetch = jest.fn().mockImplementation(async (url, options) => {
        fetchCallCount++;

        if (shouldFail && fetchCallCount % 3 === 0) {
          throw new Error('Network timeout');
        }

        if (url.includes('/initialize')) {
          return {
            ok: true,
            json: () => Promise.resolve({ protocolVersion: '2024-11-05' })
          };
        }

        if (url.includes('/message')) {
          return {
            ok: true,
            json: () => Promise.resolve({ result: `Response ${fetchCallCount}` })
          };
        }

        return { ok: true };
      });

      const { createTransport } = await import('../../src/agent/mcp/client.js');

      const transport = createTransport({
        transport: 'http',
        url: 'http://localhost:3000/mcp'
      });

      // Test successful initialization
      await expect(transport.start()).resolves.toBeDefined();

      // Test successful messages
      await expect(transport.send({ method: 'test1' })).resolves.toBeDefined();
      await expect(transport.send({ method: 'test2' })).resolves.toBeDefined();

      // Enable failures
      shouldFail = true;

      // Test that the transport still works (some may fail due to network simulation)
      try {
        await transport.send({ method: 'test3' });
      } catch (error) {
        expect(error.message).toContain('Network timeout');
      }

      // But others should succeed
      await expect(transport.send({ method: 'test4' })).resolves.toBeDefined();

      await transport.close();

      expect(fetchCallCount).toBeGreaterThan(3);
    });

    test('should handle gradual server degradation', async () => {
      let responseDelay = 0;

      const bridge = new MCPXmlBridge({ debug: false });

      // Mock tool with increasing delay
      bridge.mcpTools = {
        'degrading_tool': {
          execute: async (params) => {
            await new Promise(resolve => setTimeout(resolve, responseDelay));
            responseDelay += 10; // Increase delay each time
            return `Response after ${responseDelay - 10}ms delay`;
          }
        }
      };

      const results = [];
      const promises = [];

      // Execute 10 tool calls with increasing delays
      for (let i = 0; i < 10; i++) {
        promises.push(
          bridge.executeFromXml(`<degrading_tool><params>{"call": ${i}}</params></degrading_tool>`)
        );
      }

      const allResults = await Promise.all(promises);

      expect(allResults).toHaveLength(10);
      allResults.forEach((result) => {
        expect(result.success).toBe(true);
      });

      await bridge.cleanup();
    });
  });

  describe('Memory Management Under Stress', () => {
    test('should handle large payloads', async () => {
      const bridge = new MCPXmlBridge({ debug: false });

      // Mock tool that handles large data
      bridge.mcpTools = {
        'large_data_tool': {
          execute: async (params) => {
            const size = params.size || 1000;
            const data = new Array(size).fill(0).map((_, i) => `item_${i}`);
            return {
              processed_items: data.length,
              sample: data.slice(0, 5),
              total_size: JSON.stringify(data).length
            };
          }
        }
      };

      // Test with increasing payload sizes
      const sizes = [100, 1000, 5000, 10000];
      const results = [];

      for (const size of sizes) {
        const result = await bridge.executeFromXml(
          `<large_data_tool><params>{"size": ${size}}</params></large_data_tool>`
        );
        results.push(result);
      }

      expect(results).toHaveLength(4);
      results.forEach((result, index) => {
        expect(result.success).toBe(true);
        const parsed = typeof result.result === 'string'
          ? JSON.parse(result.result)
          : result.result;
        expect(parsed.processed_items).toBe(sizes[index]);
      });

      await bridge.cleanup();
    });

    test('should handle memory pressure scenarios', async () => {
      const bridge = new MCPXmlBridge({ debug: false });

      // Mock tool that creates temporary large objects
      bridge.mcpTools = {
        'memory_pressure_tool': {
          execute: async (params) => {
            // Create and immediately discard large objects
            for (let i = 0; i < 100; i++) {
              const largeArray = new Array(10000).fill(`data_${i}`);
              // Process the array briefly
              const sum = largeArray.length;
            }
            return `Processed memory pressure test: ${params.iteration}`;
          }
        }
      };

      // Execute multiple memory-intensive operations
      const promises = [];
      for (let i = 0; i < 20; i++) {
        promises.push(
          bridge.executeFromXml(`<memory_pressure_tool><params>{"iteration": ${i}}</params></memory_pressure_tool>`)
        );
      }

      const results = await Promise.all(promises);

      expect(results).toHaveLength(20);
      results.forEach((result) => {
        expect(result.success).toBe(true);
      });

      await bridge.cleanup();
    });
  });

  describe('Configuration Edge Cases', () => {
    test('should handle extremely large configuration files', async () => {
      // Create a configuration with many servers
      const mcpConfig = {
        mcpServers: {}
      };

      // Add 100 mock servers (most disabled)
      for (let i = 0; i < 100; i++) {
        mcpConfig.mcpServers[`server_${i}`] = {
          command: 'echo',
          args: [`server_${i}`],
          transport: 'stdio',
          enabled: i < 5 // Only enable first 5
        };
      }

      const configPath = join(tempDir, 'large-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;

      const manager = new MCPClientManager({ debug: false });
      const result = await manager.initialize();

      // Should only attempt to connect to enabled servers
      expect(result.total).toBe(5);

      await manager.disconnect();
    });

    test('should handle configuration with invalid JSON structures', async () => {
      // Create configuration with edge case JSON
      const edgeCaseConfig = {
        mcpServers: {
          'unicode_server': {
            command: 'echo',
            args: ['🚀', '测试', '🌟'],
            transport: 'stdio',
            enabled: false,
            description: 'Server with unicode characters: émojis 🎉'
          },
          'special_chars': {
            command: 'echo',
            args: ['arg with spaces', 'arg"with"quotes', 'arg\\with\\backslashes'],
            transport: 'stdio',
            enabled: false,
            env: {
              'VAR_WITH_SPACES': 'value with spaces',
              'VAR_WITH_QUOTES': 'value "with" quotes',
              'VAR_WITH_UNICODE': '🌍 global variable'
            }
          }
        }
      };

      const configPath = join(tempDir, 'edge-case-config.json');
      await writeFile(configPath, JSON.stringify(edgeCaseConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;

      const { loadMCPConfiguration, parseEnabledServers } = await import('../../src/agent/mcp/config.js');

      // Should load without throwing
      const config = loadMCPConfiguration();
      expect(config).toBeDefined();
      expect(config.mcpServers.unicode_server).toBeDefined();
      expect(config.mcpServers.special_chars).toBeDefined();

      // Should parse without throwing
      const servers = parseEnabledServers(config);
      expect(servers).toEqual([]); // All servers are disabled
    });
  });

  describe('Long-Running Stability', () => {
    test('should maintain stability over many operations', async () => {
      const bridge = new MCPXmlBridge({ debug: false });

      // Mock a stateful tool
      let callCount = 0;
      bridge.mcpTools = {
        'stateful_tool': {
          execute: async (params) => {
            callCount++;
            return {
              call_number: callCount,
              input: params.input,
              timestamp: Date.now()
            };
          }
        }
      };

      // Execute many operations over time
      const totalOperations = 200;
      const batchSize = 20;
      const results = [];

      for (let batch = 0; batch < totalOperations / batchSize; batch++) {
        const batchPromises = [];

        for (let i = 0; i < batchSize; i++) {
          const opNumber = batch * batchSize + i;
          batchPromises.push(
            bridge.executeFromXml(`<stateful_tool><params>{"input": "op_${opNumber}"}</params></stateful_tool>`)
          );
        }

        const batchResults = await Promise.all(batchPromises);
        results.push(...batchResults);

        // Small delay between batches
        await new Promise(resolve => setTimeout(resolve, 10));
      }

      expect(results).toHaveLength(totalOperations);
      results.forEach((result, index) => {
        expect(result.success).toBe(true);
        const parsed = typeof result.result === 'string'
          ? JSON.parse(result.result)
          : result.result;
        expect(parsed.call_number).toBe(index + 1);
      });

      await bridge.cleanup();
    });

    test('should handle intermittent failures gracefully', async () => {
      const bridge = new MCPXmlBridge({ debug: false });

      // Mock tool that fails intermittently
      let callCount = 0;
      bridge.mcpTools = {
        'flaky_tool': {
          execute: async (params) => {
            callCount++;

            // Fail every 7th call
            if (callCount % 7 === 0) {
              throw new Error(`Intermittent failure on call ${callCount}`);
            }

            return `Success on call ${callCount}`;
          }
        }
      };

      const promises = [];
      for (let i = 0; i < 50; i++) {
        promises.push(
          bridge.executeFromXml(`<flaky_tool><params>{"call": ${i}}</params></flaky_tool>`)
        );
      }

      const results = await Promise.all(promises);

      expect(results).toHaveLength(50);

      let successCount = 0;
      let failureCount = 0;

      results.forEach((result) => {
        if (result.success) {
          successCount++;
        } else {
          failureCount++;
          expect(result.error).toContain('Intermittent failure');
        }
      });

      // Should have approximately 7 failures (every 7th call)
      expect(failureCount).toBeGreaterThan(5);
      expect(failureCount).toBeLessThan(10);
      expect(successCount).toBe(50 - failureCount);

      await bridge.cleanup();
    });
  });

  describe('Integration Stress Tests', () => {
    test('should handle ProbeAgent with mock server under load', async () => {
      process.env.ANTHROPIC_API_KEY = 'test-key';

      // Create configuration for mock server
      const mcpConfig = {
        mcpServers: {
          'stress-test-server': {
            command: 'node',
            args: [join(__dirname, '../mcp/mockMcpServer.js')],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'stress-test-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;

      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      // Wait for initialization
      await new Promise(resolve => setTimeout(resolve, 3000));

      if (agent.mcpBridge && agent.mcpBridge.getToolNames().length > 0) {
        // Test system message generation under rapid calls
        const systemMessagePromises = [];
        for (let i = 0; i < 10; i++) {
          systemMessagePromises.push(agent.getSystemMessage());
        }

        const systemMessages = await Promise.all(systemMessagePromises);
        expect(systemMessages).toHaveLength(10);

        // All messages should be identical and contain MCP tools
        systemMessages.forEach(message => {
          expect(message).toContain('foobar');
          expect(message).toBe(systemMessages[0]);
        });

        console.log('Stress test completed successfully');
      } else {
        console.warn('Mock server not available for stress testing');
      }

      await agent.cleanup();
    }, 20000);

    test('should handle multiple ProbeAgent instances with MCP', async () => {
      process.env.ANTHROPIC_API_KEY = 'test-key';

      const agents = [];

      try {
        // Create multiple agents with MCP enabled
        for (let i = 0; i < 5; i++) {
          const agent = new ProbeAgent({
            enableMcp: true,
            debug: false,
            path: tempDir,
            sessionId: `stress-test-${i}`
          });
          agents.push(agent);
        }

        // Wait for all initializations
        await new Promise(resolve => setTimeout(resolve, 2000));

        // Verify all agents are independent
        for (let i = 0; i < agents.length; i++) {
          const agent = agents[i];
          expect(agent.sessionId).toContain(`stress-test-${i}`);
          expect(agent.enableMcp).toBe(true);
        }

        console.log('Multiple agent test completed successfully');
      } finally {
        // Cleanup all agents
        await Promise.all(agents.map(agent => agent.cleanup()));
      }
    });
  });
});