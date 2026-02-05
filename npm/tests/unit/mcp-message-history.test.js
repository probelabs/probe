/**
 * Tests for MCP tool execution message history integrity
 *
 * This test verifies that MCP tool execution correctly adds both:
 * 1. The assistant message containing the tool call
 * 2. The user message containing the tool result
 *
 * Bug reference: https://github.com/probelabs/probe/issues/393
 */

import { jest, describe, test, expect, beforeEach, afterEach } from '@jest/globals';

// Set environment to use mock AI provider
process.env.USE_MOCK_AI = 'true';

import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('MCP Tool Message History', () => {
  let agent;
  let mockMcpBridge;
  let mockCallCount;
  let mockResponses;

  beforeEach(() => {
    // Reset mock call count and responses
    mockCallCount = 0;
    mockResponses = [];

    // Create a mock MCP bridge with all required methods
    mockMcpBridge = {
      isMcpTool: jest.fn((name) => name === 'test_mcp_tool'),
      getToolNames: jest.fn(() => ['test_mcp_tool']),
      getToolDefinitions: jest.fn(() => ({
        test_mcp_tool: {
          description: 'A test MCP tool',
          inputSchema: {
            type: 'object',
            properties: {
              query: { type: 'string', description: 'Test query' }
            }
          }
        }
      })),
      getXmlToolDefinitions: jest.fn(() => `
## test_mcp_tool
Description: A test MCP tool
Parameters:
- query: string (optional) - Test query

Example:
<test_mcp_tool>
<params>
{"query": "example"}
</params>
</test_mcp_tool>
`),
      mcpTools: {
        test_mcp_tool: {
          execute: jest.fn(async () => 'MCP tool result')
        }
      },
      cleanup: jest.fn()
    };

    agent = new ProbeAgent({
      sessionId: 'test-mcp-history',
      path: process.cwd(),
      debug: false
    });

    // Inject the mock MCP bridge
    agent.mcpBridge = mockMcpBridge;

    // Fix the provider to be a callable function (matching real provider interface)
    agent.provider = (modelName) => `mock-${modelName}`;

    // Mock the streamTextWithRetryAndFallback method to return controlled responses
    agent.streamTextWithRetryAndFallback = jest.fn(async () => {
      const response = mockResponses[mockCallCount] || { text: '<attempt_completion>\n<result>Default</result>\n</attempt_completion>' };
      mockCallCount++;

      // Create a mock async iterator for textStream
      const textParts = [response.text];
      let index = 0;

      return {
        textStream: {
          [Symbol.asyncIterator]: () => ({
            next: async () => {
              if (index < textParts.length) {
                const value = textParts[index++];
                return { value, done: false };
              }
              return { value: undefined, done: true };
            }
          })
        },
        text: Promise.resolve(response.text),
        usage: Promise.resolve({ promptTokens: 100, completionTokens: 50 })
      };
    });
  });

  afterEach(async () => {
    if (agent) {
      agent.mcpBridge = null;
    }
  });

  describe('MCP tool success path', () => {
    test('should add assistant message before tool result on successful MCP execution', async () => {
      // Set up mock responses
      mockResponses = [
        { text: '<test_mcp_tool>\n<params>\n{"query": "test"}\n</params>\n</test_mcp_tool>' },
        { text: '<attempt_completion>\n<result>Done</result>\n</attempt_completion>' }
      ];

      // Only set system message - answer() will add the user message
      agent.history = [
        { role: 'system', content: 'You are a helpful assistant.' }
      ];

      // Run the answer loop
      await agent.answer('Use the test tool', [], {
        maxIterations: 3
      });

      // Verify MCP tool was called
      expect(mockMcpBridge.mcpTools.test_mcp_tool.execute).toHaveBeenCalledWith({ query: 'test' });

      // Verify the message history pattern
      const historyAfterToolCall = agent.history;

      // Find the MCP tool call and result in history
      const assistantWithToolCall = historyAfterToolCall.find(
        m => m.role === 'assistant' && m.content && m.content.includes('<test_mcp_tool>')
      );
      const userWithToolResult = historyAfterToolCall.find(
        m => m.role === 'user' && m.content && m.content.includes('<tool_result>')
      );

      // Both messages should exist (this is what issue #393 fixed)
      expect(assistantWithToolCall).toBeDefined();
      expect(userWithToolResult).toBeDefined();

      // Verify the assistant message comes before the tool result
      const assistantIndex = historyAfterToolCall.indexOf(assistantWithToolCall);
      const userIndex = historyAfterToolCall.indexOf(userWithToolResult);
      expect(assistantIndex).toBeLessThan(userIndex);

      // Verify proper alternation: assistant should be immediately followed by user
      expect(historyAfterToolCall[assistantIndex + 1]).toBe(userWithToolResult);
    });

    test('should maintain proper message alternation after multiple MCP tool calls', async () => {
      // Set up mock responses for multiple tool calls
      mockResponses = [
        { text: '<test_mcp_tool>\n<params>\n{"query": "first"}\n</params>\n</test_mcp_tool>' },
        { text: '<test_mcp_tool>\n<params>\n{"query": "second"}\n</params>\n</test_mcp_tool>' },
        { text: '<attempt_completion>\n<result>All done</result>\n</attempt_completion>' }
      ];

      // Only set system message - answer() will add the user message
      agent.history = [
        { role: 'system', content: 'System' }
      ];

      await agent.answer('Run two tools', [], { maxIterations: 5 });

      // Verify two MCP tool calls were made
      expect(mockMcpBridge.mcpTools.test_mcp_tool.execute).toHaveBeenCalledTimes(2);

      // Check for proper alternation pattern - no two consecutive user messages
      const history = agent.history;
      let prevRole = null;
      let foundAlternationError = false;

      for (let i = 1; i < history.length; i++) {
        const msg = history[i];
        // Two consecutive user messages would indicate the bug
        if (prevRole === 'user' && msg.role === 'user') {
          foundAlternationError = true;
          break;
        }
        prevRole = msg.role;
      }

      expect(foundAlternationError).toBe(false);
    });
  });

  describe('MCP tool error path', () => {
    test('should add assistant message before error result on failed MCP execution', async () => {
      // Make the MCP tool throw an error
      mockMcpBridge.mcpTools.test_mcp_tool.execute = jest.fn(async () => {
        throw new Error('MCP tool failed');
      });

      // Set up mock responses
      mockResponses = [
        { text: '<test_mcp_tool>\n<params>\n{"query": "test"}\n</params>\n</test_mcp_tool>' },
        { text: '<attempt_completion>\n<result>Handled error</result>\n</attempt_completion>' }
      ];

      // Only set system message - answer() will add the user message
      agent.history = [
        { role: 'system', content: 'System' }
      ];

      await agent.answer('Use tool that will fail', [], { maxIterations: 3 });

      // Verify the tool was attempted
      expect(mockMcpBridge.mcpTools.test_mcp_tool.execute).toHaveBeenCalled();

      // Verify assistant message exists before error result
      const history = agent.history;

      const assistantWithToolCall = history.find(
        m => m.role === 'assistant' && m.content && m.content.includes('<test_mcp_tool>')
      );
      const userWithError = history.find(
        m => m.role === 'user' && m.content && m.content.includes('<tool_result>')
      );

      // Both messages should exist
      expect(assistantWithToolCall).toBeDefined();
      expect(userWithError).toBeDefined();

      // Verify proper ordering
      const assistantIndex = history.indexOf(assistantWithToolCall);
      const errorIndex = history.indexOf(userWithError);
      expect(assistantIndex).toBeLessThan(errorIndex);

      // Assistant should be immediately followed by error result
      expect(history[assistantIndex + 1]).toBe(userWithError);
    });
  });

  describe('Message pattern consistency', () => {
    test('should never have consecutive user messages in MCP flow', async () => {
      // This is the core test that would catch issue #393
      // Before the fix, MCP tools would produce [user, user] pattern

      mockResponses = [
        { text: '<test_mcp_tool>\n<params>\n{"query": "call1"}\n</params>\n</test_mcp_tool>' },
        { text: '<test_mcp_tool>\n<params>\n{"query": "call2"}\n</params>\n</test_mcp_tool>' },
        { text: '<test_mcp_tool>\n<params>\n{"query": "call3"}\n</params>\n</test_mcp_tool>' },
        { text: '<attempt_completion>\n<result>Done after multiple calls</result>\n</attempt_completion>' }
      ];

      // Only set system message - answer() will add the user message
      agent.history = [
        { role: 'system', content: 'System prompt' }
      ];

      await agent.answer('Execute multiple tools', [], { maxIterations: 10 });

      // The fix ensures proper alternation
      const history = agent.history;

      // Count consecutive user messages (which would indicate the bug)
      let consecutiveUserCount = 0;
      for (let i = 1; i < history.length; i++) {
        if (history[i].role === 'user' && history[i - 1].role === 'user') {
          consecutiveUserCount++;
        }
      }

      // With the fix, there should be no consecutive user messages
      expect(consecutiveUserCount).toBe(0);

      // Verify the pattern is always: assistant -> user for tool calls
      const toolResults = history.filter(m => m.role === 'user' && m.content && m.content.includes('<tool_result>'));
      for (const toolResult of toolResults) {
        const idx = history.indexOf(toolResult);
        // The message before a tool result should always be an assistant message
        expect(history[idx - 1].role).toBe('assistant');
      }
    });
  });
});
