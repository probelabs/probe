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
});
