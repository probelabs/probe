/**
 * Tests for MCP tool integration with native Vercel AI SDK tools
 *
 * After the migration from XML-based tool calling to native Vercel AI SDK tools,
 * message history is managed automatically by the SDK's streamText/maxSteps.
 * These tests verify that MCP tools are properly included in the native tools
 * passed to the AI SDK, and that the MCP bridge is correctly wired up.
 */

import { jest, describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { z } from 'zod';

// Set environment to use mock AI provider
process.env.USE_MOCK_AI = 'true';

import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('MCP Tool Native Integration', () => {
  let agent;
  let mockMcpBridge;

  beforeEach(() => {
    // Create a mock MCP bridge with the current API surface
    mockMcpBridge = {
      isMcpTool: jest.fn((name) => name === 'test_mcp_tool'),
      getToolNames: jest.fn(() => ['test_mcp_tool']),
      getVercelTools: jest.fn((filterNames) => {
        const tools = {
          test_mcp_tool: {
            description: 'A test MCP tool',
            parameters: z.object({
              query: z.string().optional().describe('Test query')
            }),
            execute: jest.fn(async (params) => `MCP tool result for: ${params.query}`)
          }
        };
        if (filterNames) {
          const filtered = {};
          for (const name of filterNames) {
            if (tools[name]) filtered[name] = tools[name];
          }
          return filtered;
        }
        return tools;
      }),
      mcpTools: {
        test_mcp_tool: {
          description: 'A test MCP tool',
          parameters: z.object({
            query: z.string().optional().describe('Test query')
          }),
          execute: jest.fn(async (params) => `MCP tool result for: ${params.query}`)
        }
      },
      cleanup: jest.fn()
    };

    agent = new ProbeAgent({
      sessionId: 'test-mcp-native',
      path: process.cwd(),
      debug: false
    });

    // Inject the mock MCP bridge
    agent.mcpBridge = mockMcpBridge;

    // Fix the provider to be a callable function (matching real provider interface)
    agent.provider = (modelName) => `mock-${modelName}`;
  });

  afterEach(async () => {
    if (agent) {
      agent.mcpBridge = null;
    }
  });

  describe('MCP tools in _buildNativeTools', () => {
    test('should include MCP tools in the native tools object', () => {
      let completionCalled = false;
      const onComplete = (result) => { completionCalled = true; };

      const tools = agent._buildNativeTools({}, onComplete);

      // The tools object should contain the MCP tool
      expect(tools).toHaveProperty('test_mcp_tool');
      // It should also contain standard tools like attempt_completion
      expect(tools).toHaveProperty('attempt_completion');
    });

    test('should not include MCP tools when _disableTools is set', () => {
      let completionCalled = false;
      const onComplete = (result) => { completionCalled = true; };

      const tools = agent._buildNativeTools({ _disableTools: true }, onComplete);

      // With _disableTools, only attempt_completion should be present
      expect(tools).toHaveProperty('attempt_completion');
      expect(tools).not.toHaveProperty('test_mcp_tool');
    });

    test('should call getVercelTools on the bridge', () => {
      const onComplete = () => {};
      agent._buildNativeTools({}, onComplete);

      // getVercelTools should have been called to retrieve MCP tools
      expect(mockMcpBridge.getVercelTools).toHaveBeenCalled();
    });

    test('should not include MCP tools when mcpBridge is null', () => {
      agent.mcpBridge = null;

      const onComplete = () => {};
      const tools = agent._buildNativeTools({}, onComplete);

      // Should still have standard tools but no MCP tools
      expect(tools).toHaveProperty('attempt_completion');
      expect(tools).not.toHaveProperty('test_mcp_tool');
    });

    test('should not crash when MCP tools are in toolImplementations (issue #469)', () => {
      // Reproduce the exact scenario from the bug:
      // After initializeMCP(), MCP tools are merged into toolImplementations.
      // _buildNativeTools iterates ALL toolImplementations, including MCP tools.
      // _getToolSchemaAndDescription returns null for MCP tools, causing a
      // TypeError when destructuring: Cannot destructure property 'schema' of null.
      const mcpToolName = '__tools___slack-send-dm';
      agent.toolImplementations[mcpToolName] = {
        execute: jest.fn(async () => 'MCP result')
      };

      // This should NOT throw "Cannot destructure property 'schema' of null"
      const onComplete = () => {};
      expect(() => {
        agent._buildNativeTools({}, onComplete);
      }).not.toThrow();

      const tools = agent._buildNativeTools({}, onComplete);

      // MCP tools in toolImplementations should be skipped (no schema known)
      // They get included via mcpBridge.getVercelTools() instead
      expect(tools).not.toHaveProperty(mcpToolName);
      // MCP tool from the bridge should still be present
      expect(tools).toHaveProperty('test_mcp_tool');
      expect(tools).toHaveProperty('attempt_completion');
    });

    test('should handle multiple MCP tools in toolImplementations without crash', () => {
      // Simulate what initializeMCP does: merge all MCP tools into toolImplementations
      const mcpTools = {
        '__tools___slack-send-dm': { execute: jest.fn() },
        '__tools___slack-search': { execute: jest.fn() },
        '__tools___slack-read-thread': { execute: jest.fn() },
        '__tools___slack-download-file': { execute: jest.fn() },
      };
      for (const [name, impl] of Object.entries(mcpTools)) {
        agent.toolImplementations[name] = impl;
      }

      const onComplete = () => {};
      expect(() => {
        agent._buildNativeTools({}, onComplete);
      }).not.toThrow();

      const tools = agent._buildNativeTools({}, onComplete);
      // None of the MCP tools from toolImplementations should be in native tools
      for (const name of Object.keys(mcpTools)) {
        expect(tools).not.toHaveProperty(name);
      }
      // But the bridge-provided MCP tool should be there
      expect(tools).toHaveProperty('test_mcp_tool');
    });
  });

  describe('MCP bridge API surface', () => {
    test('isMcpTool should correctly identify MCP tools', () => {
      expect(agent.mcpBridge.isMcpTool('test_mcp_tool')).toBe(true);
      expect(agent.mcpBridge.isMcpTool('search')).toBe(false);
      expect(agent.mcpBridge.isMcpTool('nonexistent')).toBe(false);
    });

    test('getToolNames should return all MCP tool names', () => {
      const names = agent.mcpBridge.getToolNames();
      expect(names).toEqual(['test_mcp_tool']);
    });

    test('getVercelTools should return tool objects with execute functions', () => {
      const tools = agent.mcpBridge.getVercelTools();
      expect(tools).toHaveProperty('test_mcp_tool');
      expect(tools.test_mcp_tool).toHaveProperty('execute');
      expect(typeof tools.test_mcp_tool.execute).toBe('function');
    });

    test('getVercelTools with filter should return only matching tools', () => {
      const filtered = agent.mcpBridge.getVercelTools(['test_mcp_tool']);
      expect(filtered).toHaveProperty('test_mcp_tool');

      const empty = agent.mcpBridge.getVercelTools(['nonexistent_tool']);
      expect(Object.keys(empty)).toHaveLength(0);
    });
  });

  describe('MCP tool execution via bridge', () => {
    test('should execute MCP tool directly via the bridge tool object', async () => {
      const tools = agent.mcpBridge.getVercelTools();
      const result = await tools.test_mcp_tool.execute({ query: 'test' });
      expect(result).toBe('MCP tool result for: test');
    });

    test('should handle MCP tool execution errors', async () => {
      // Replace the execute function with one that throws
      mockMcpBridge.mcpTools.test_mcp_tool.execute = jest.fn(async () => {
        throw new Error('MCP tool failed');
      });
      mockMcpBridge.getVercelTools = jest.fn(() => ({
        test_mcp_tool: {
          description: 'A test MCP tool',
          parameters: z.object({ query: z.string().optional() }),
          execute: mockMcpBridge.mcpTools.test_mcp_tool.execute
        }
      }));

      const tools = agent.mcpBridge.getVercelTools();
      await expect(tools.test_mcp_tool.execute({ query: 'test' }))
        .rejects.toThrow('MCP tool failed');
    });
  });

  describe('Multiple MCP tools', () => {
    test('should handle multiple MCP tools in _buildNativeTools', () => {
      // Set up bridge with multiple tools
      mockMcpBridge.getToolNames = jest.fn(() => ['tool_a', 'tool_b', 'tool_c']);
      mockMcpBridge.getVercelTools = jest.fn(() => ({
        tool_a: {
          description: 'Tool A',
          parameters: z.object({}),
          execute: jest.fn(async () => 'A result')
        },
        tool_b: {
          description: 'Tool B',
          parameters: z.object({}),
          execute: jest.fn(async () => 'B result')
        },
        tool_c: {
          description: 'Tool C',
          parameters: z.object({}),
          execute: jest.fn(async () => 'C result')
        }
      }));

      const onComplete = () => {};
      const tools = agent._buildNativeTools({}, onComplete);

      expect(tools).toHaveProperty('tool_a');
      expect(tools).toHaveProperty('tool_b');
      expect(tools).toHaveProperty('tool_c');
      expect(tools).toHaveProperty('attempt_completion');
    });
  });

  describe('MCP tools with raw JSON Schema (issue #472)', () => {
    test('should wrap raw JSON Schema inputSchema with jsonSchema() without crashing', () => {
      // Real MCP tools return raw JSON Schema objects, not Zod schemas.
      // Without wrapping, Vercel AI SDK's asSchema() misidentifies them as Zod
      // and crashes with: TypeError: Cannot read properties of undefined (reading 'typeName')
      mockMcpBridge.getToolNames = jest.fn(() => ['slack_send_dm']);
      mockMcpBridge.getVercelTools = jest.fn(() => ({
        slack_send_dm: {
          description: 'Send a direct message via Slack',
          inputSchema: {
            type: 'object',
            properties: {
              channel: { type: 'string', description: 'Channel ID' },
              message: { type: 'string', description: 'Message text' }
            },
            required: ['channel', 'message']
          },
          execute: jest.fn(async (params) => `Sent to ${params.channel}`)
        }
      }));

      const onComplete = () => {};
      expect(() => {
        agent._buildNativeTools({}, onComplete);
      }).not.toThrow();

      const tools = agent._buildNativeTools({}, onComplete);
      expect(tools).toHaveProperty('slack_send_dm');
      expect(tools).toHaveProperty('attempt_completion');
    });

    test('should handle MCP tools with empty inputSchema', () => {
      mockMcpBridge.getToolNames = jest.fn(() => ['simple_tool']);
      mockMcpBridge.getVercelTools = jest.fn(() => ({
        simple_tool: {
          description: 'A tool with no parameters',
          inputSchema: { type: 'object', properties: {} },
          execute: jest.fn(async () => 'done')
        }
      }));

      const onComplete = () => {};
      expect(() => {
        agent._buildNativeTools({}, onComplete);
      }).not.toThrow();

      const tools = agent._buildNativeTools({}, onComplete);
      expect(tools).toHaveProperty('simple_tool');
    });

    test('should handle MCP tools with no inputSchema at all', () => {
      mockMcpBridge.getToolNames = jest.fn(() => ['no_schema_tool']);
      mockMcpBridge.getVercelTools = jest.fn(() => ({
        no_schema_tool: {
          description: 'A tool with no schema',
          execute: jest.fn(async () => 'result')
        }
      }));

      const onComplete = () => {};
      expect(() => {
        agent._buildNativeTools({}, onComplete);
      }).not.toThrow();

      const tools = agent._buildNativeTools({}, onComplete);
      expect(tools).toHaveProperty('no_schema_tool');
    });

    test('should handle MCP tools with no description', () => {
      mockMcpBridge.getToolNames = jest.fn(() => ['no_desc_tool']);
      mockMcpBridge.getVercelTools = jest.fn(() => ({
        no_desc_tool: {
          inputSchema: { type: 'object', properties: { q: { type: 'string' } } },
          execute: jest.fn(async () => 'result')
        }
      }));

      const onComplete = () => {};
      expect(() => {
        agent._buildNativeTools({}, onComplete);
      }).not.toThrow();

      const tools = agent._buildNativeTools({}, onComplete);
      expect(tools).toHaveProperty('no_desc_tool');
    });

    test('should handle multiple MCP tools with raw JSON Schema', () => {
      // Simulate a real MCP server with multiple tools using raw JSON Schema
      mockMcpBridge.getToolNames = jest.fn(() => [
        '__tools___slack-send-dm',
        '__tools___slack-search',
        '__tools___slack-read-thread',
        '__tools___slack-download-file'
      ]);
      mockMcpBridge.getVercelTools = jest.fn(() => ({
        '__tools___slack-send-dm': {
          description: 'Send a DM',
          inputSchema: {
            type: 'object',
            properties: {
              user_id: { type: 'string' },
              text: { type: 'string' }
            },
            required: ['user_id', 'text']
          },
          execute: jest.fn(async () => 'sent')
        },
        '__tools___slack-search': {
          description: 'Search messages',
          inputSchema: {
            type: 'object',
            properties: {
              query: { type: 'string' },
              limit: { type: 'number' }
            },
            required: ['query']
          },
          execute: jest.fn(async () => 'results')
        },
        '__tools___slack-read-thread': {
          description: 'Read a thread',
          inputSchema: {
            type: 'object',
            properties: {
              thread_ts: { type: 'string' },
              channel: { type: 'string' }
            },
            required: ['thread_ts', 'channel']
          },
          execute: jest.fn(async () => 'thread')
        },
        '__tools___slack-download-file': {
          description: 'Download a file',
          inputSchema: {
            type: 'object',
            properties: {
              file_id: { type: 'string' }
            },
            required: ['file_id']
          },
          execute: jest.fn(async () => 'file')
        }
      }));

      const onComplete = () => {};
      expect(() => {
        agent._buildNativeTools({}, onComplete);
      }).not.toThrow();

      const tools = agent._buildNativeTools({}, onComplete);
      expect(tools).toHaveProperty('__tools___slack-send-dm');
      expect(tools).toHaveProperty('__tools___slack-search');
      expect(tools).toHaveProperty('__tools___slack-read-thread');
      expect(tools).toHaveProperty('__tools___slack-download-file');
      expect(tools).toHaveProperty('attempt_completion');
    });

    test('should preserve MCP tool execute functions after wrapping', async () => {
      const mockExecute = jest.fn(async (params) => `Result: ${params.query}`);
      mockMcpBridge.getToolNames = jest.fn(() => ['search_tool']);
      mockMcpBridge.getVercelTools = jest.fn(() => ({
        search_tool: {
          description: 'Search',
          inputSchema: {
            type: 'object',
            properties: { query: { type: 'string' } },
            required: ['query']
          },
          execute: mockExecute
        }
      }));

      const onComplete = () => {};
      const tools = agent._buildNativeTools({}, onComplete);

      // The wrapped tool should still call the original execute function
      expect(tools).toHaveProperty('search_tool');
      const result = await tools.search_tool.execute({ query: 'hello' });
      expect(mockExecute).toHaveBeenCalledWith({ query: 'hello' });
      expect(result).toBe('Result: hello');
    });

    test('should handle mix of Zod and raw JSON Schema MCP tools', () => {
      // Some MCP implementations might return Zod schemas, others raw JSON Schema
      mockMcpBridge.getToolNames = jest.fn(() => ['zod_tool', 'json_tool']);
      mockMcpBridge.getVercelTools = jest.fn(() => ({
        zod_tool: {
          description: 'Tool with Zod schema',
          parameters: z.object({ name: z.string() }),
          execute: jest.fn(async () => 'zod result')
        },
        json_tool: {
          description: 'Tool with raw JSON Schema',
          inputSchema: {
            type: 'object',
            properties: { name: { type: 'string' } },
            required: ['name']
          },
          execute: jest.fn(async () => 'json result')
        }
      }));

      const onComplete = () => {};
      expect(() => {
        agent._buildNativeTools({}, onComplete);
      }).not.toThrow();

      const tools = agent._buildNativeTools({}, onComplete);
      expect(tools).toHaveProperty('zod_tool');
      expect(tools).toHaveProperty('json_tool');
    });

    test('should handle MCP tool with complex nested JSON Schema', () => {
      mockMcpBridge.getToolNames = jest.fn(() => ['complex_tool']);
      mockMcpBridge.getVercelTools = jest.fn(() => ({
        complex_tool: {
          description: 'Tool with nested schema',
          inputSchema: {
            type: 'object',
            properties: {
              config: {
                type: 'object',
                properties: {
                  enabled: { type: 'boolean' },
                  options: {
                    type: 'array',
                    items: { type: 'string' }
                  }
                }
              },
              tags: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    key: { type: 'string' },
                    value: { type: 'string' }
                  }
                }
              }
            }
          },
          execute: jest.fn(async () => 'complex result')
        }
      }));

      const onComplete = () => {};
      expect(() => {
        agent._buildNativeTools({}, onComplete);
      }).not.toThrow();

      const tools = agent._buildNativeTools({}, onComplete);
      expect(tools).toHaveProperty('complex_tool');
    });
  });
});
