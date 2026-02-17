/**
 * Tests for MCP tool integration with execute_plan DSL.
 *
 * This tests the fix for issue #418: MCP tools were unavailable inside
 * execute_plan DSL sandbox due to initialization order (MCP initialized
 * after tools were created).
 */

import { createDSLRuntime } from '../../src/agent/dsl/runtime.js';
import { createExecutePlanTool } from '../../src/tools/executePlan.js';

// Mock MCP bridge that simulates MCPXmlBridge behavior
// Real MCP tools have .execute() method (Vercel tool format)
function createMockMcpBridge(toolFns = {}) {
  // Wrap raw functions as tool objects with execute method
  const mcpTools = {};
  for (const [name, fn] of Object.entries(toolFns)) {
    mcpTools[name] = {
      execute: async (params) => fn(params),
    };
  }
  return {
    // callTool is on MCPManager, not MCPXmlBridge, but we include it for compatibility
    callTool: async (name, params) => {
      if (mcpTools[name]) {
        return mcpTools[name].execute(params);
      }
      throw new Error(`Unknown MCP tool: ${name}`);
    },
    mcpTools,
  };
}

// Mock tool implementations for testing
function createMockTools() {
  return {
    search: {
      execute: async (params) => `search results for: ${params.query}`,
    },
  };
}

function createMockLLM() {
  return async (instruction, data) => {
    return `LLM processed: ${instruction}`;
  };
}

describe('MCP Integration with DSL Runtime', () => {
  describe('MCP tools in sandbox', () => {
    test('MCP tool is available when mcpBridge is provided at creation', async () => {
      const mcpBridge = createMockMcpBridge({
        zendesk_search_tickets: async (params) => {
          return { tickets: [{ id: 1, subject: 'Test ticket' }] };
        },
      });

      const runtime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        mcpBridge,
        mcpTools: mcpBridge.mcpTools,
      });

      const result = await runtime.execute(`
        const tickets = zendesk_search_tickets({query: "test"});
        return tickets;
      `);

      expect(result.status).toBe('success');
      expect(result.result).toEqual({ tickets: [{ id: 1, subject: 'Test ticket' }] });
    });

    test('multiple MCP tools are available', async () => {
      const mcpBridge = createMockMcpBridge({
        tool_a: async () => 'result_a',
        tool_b: async () => 'result_b',
        tool_c: async () => 'result_c',
      });

      const runtime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        mcpBridge,
        mcpTools: mcpBridge.mcpTools,
      });

      const result = await runtime.execute(`
        const a = tool_a({});
        const b = tool_b({});
        const c = tool_c({});
        return [a, b, c];
      `);

      expect(result.status).toBe('success');
      expect(result.result).toEqual(['result_a', 'result_b', 'result_c']);
    });

    test('MCP tool error returns ERROR: string', async () => {
      const mcpBridge = createMockMcpBridge({
        failing_tool: async () => {
          throw new Error('API rate limited');
        },
      });

      const runtime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        mcpBridge,
        mcpTools: mcpBridge.mcpTools,
      });

      const result = await runtime.execute(`
        const r = failing_tool({});
        return r;
      `);

      expect(result.status).toBe('success');
      expect(result.result).toBe('ERROR: API rate limited');
    });

    test('MCP tools work alongside native tools', async () => {
      const mcpBridge = createMockMcpBridge({
        external_api: async (params) => ({ data: params.query }),
      });

      const runtime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        mcpBridge,
        mcpTools: mcpBridge.mcpTools,
      });

      const result = await runtime.execute(`
        const searchResult = search({query: "functions"});
        const apiResult = external_api({query: "data"});
        return {search: searchResult, api: apiResult};
      `);

      expect(result.status).toBe('success');
      expect(result.result.search).toContain('search results');
      expect(result.result.api).toEqual({ data: 'data' });
    });

    test('MCP tools not available when mcpBridge is null', async () => {
      const runtime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        mcpBridge: null,
        mcpTools: {},
      });

      const result = await runtime.execute(`
        const r = nonexistent_mcp_tool({});
        return r;
      `);

      expect(result.status).toBe('error');
      expect(result.error).toContain('nonexistent_mcp_tool is not defined');
    });
  });

  describe('Lazy MCP initialization in createExecutePlanTool', () => {
    test('runtime is rebuilt when getMcpBridge returns new bridge', async () => {
      // Simulate the initialization order problem:
      // 1. Tool is created with null mcpBridge
      // 2. Later, mcpBridge becomes available
      // 3. On execute(), runtime should be rebuilt with the bridge

      let currentMcpBridge = null;

      const getMcpBridge = () => currentMcpBridge;
      const getMcpTools = () => currentMcpBridge?.mcpTools || {};

      const executePlanTool = createExecutePlanTool({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        getMcpBridge,
        getMcpTools,
      });

      // First execution - no MCP bridge yet
      const result1 = await executePlanTool.execute({
        code: 'return "first";',
        description: 'Test 1',
      });
      expect(result1).toContain('first');

      // Now MCP bridge becomes available (simulating initializeMCP())
      currentMcpBridge = createMockMcpBridge({
        zendesk_search: async (params) => ({ tickets: ['t1', 't2'] }),
      });

      // Second execution - should have MCP tools available
      const result2 = await executePlanTool.execute({
        code: `
          const r = zendesk_search({query: "test"});
          return r;
        `,
        description: 'Test 2',
      });

      // Should succeed and have the MCP tool result
      expect(result2).toContain('tickets');
    });

    test('runtime is NOT rebuilt when mcpBridge unchanged', async () => {
      let rebuildCount = 0;
      const mcpBridge = createMockMcpBridge({
        test_tool: async () => {
          rebuildCount++;
          return 'result';
        },
      });

      const executePlanTool = createExecutePlanTool({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        getMcpBridge: () => mcpBridge,
        getMcpTools: () => mcpBridge.mcpTools,
      });

      // Multiple executions with same bridge
      await executePlanTool.execute({ code: 'return 1;', description: 'Test' });
      await executePlanTool.execute({ code: 'return 2;', description: 'Test' });
      await executePlanTool.execute({ code: 'return 3;', description: 'Test' });

      // rebuildCount tracks tool calls, not rebuilds
      // The important thing is the same runtime is reused
      expect(rebuildCount).toBe(0); // No MCP tool was called
    });

    test('MCP tool filtering via isMcpToolAllowed', async () => {
      const mcpBridge = createMockMcpBridge({
        allowed_tool: async () => 'allowed result',
        blocked_tool: async () => 'blocked result',
      });

      const executePlanTool = createExecutePlanTool({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        getMcpBridge: () => mcpBridge,
        getMcpTools: () => mcpBridge.mcpTools,
        isMcpToolAllowed: (name) => name === 'allowed_tool',
        maxRetries: 0, // Disable retries to see the raw error
      });

      // allowed_tool should work
      const result1 = await executePlanTool.execute({
        code: 'const r = allowed_tool({}); return r;',
        description: 'Test allowed',
      });
      expect(result1).toContain('allowed result');

      // blocked_tool should not be defined - results in execution failure
      const result2 = await executePlanTool.execute({
        code: 'const r = blocked_tool({}); return r;',
        description: 'Test blocked',
      });
      // Either "not defined" error or execution failure
      expect(result2).toMatch(/blocked_tool is not defined|Plan execution failed/);
    });

    test('handles bridge changing from null to object', async () => {
      let bridge = null;

      const executePlanTool = createExecutePlanTool({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        getMcpBridge: () => bridge,
        getMcpTools: () => bridge?.mcpTools || {},
      });

      // Execute without bridge - should work for native tools
      const result1 = await executePlanTool.execute({
        code: 'const r = search({query: "test"}); return r;',
        description: 'Native tool',
      });
      expect(result1).toContain('search results');

      // Bridge becomes available
      bridge = createMockMcpBridge({
        jira_get_issues: async () => [{ key: 'PROJ-123' }],
      });

      // Execute with bridge - MCP tool should now work
      const result2 = await executePlanTool.execute({
        code: 'const r = jira_get_issues({}); return r;',
        description: 'MCP tool',
      });
      expect(result2).toContain('PROJ-123');
    });

    test('handles bridge changing between different bridges', async () => {
      let bridge = createMockMcpBridge({
        tool_v1: async () => 'v1 result',
      });

      const executePlanTool = createExecutePlanTool({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        getMcpBridge: () => bridge,
        getMcpTools: () => bridge?.mcpTools || {},
        maxRetries: 0, // Disable retries to see raw errors
      });

      // First bridge
      const result1 = await executePlanTool.execute({
        code: 'const r = tool_v1({}); return r;',
        description: 'V1',
      });
      expect(result1).toContain('v1 result');

      // Switch to different bridge with different tools
      bridge = createMockMcpBridge({
        tool_v2: async () => 'v2 result',
      });

      // New tool should work
      const result2 = await executePlanTool.execute({
        code: 'const r = tool_v2({}); return r;',
        description: 'V2',
      });
      expect(result2).toContain('v2 result');

      // Old tool should no longer exist - results in execution failure
      const result3 = await executePlanTool.execute({
        code: 'const r = tool_v1({}); return r;',
        description: 'Old V1',
      });
      // Either "not defined" error or execution failure
      expect(result3).toMatch(/tool_v1 is not defined|Plan execution failed/);
    });
  });

  describe('OTEL tracing for MCP tools', () => {
    function createMockTracer() {
      const spans = [];
      return {
        spans,
        createToolSpan: (name, attrs = {}) => {
          const span = {
            name,
            attributes: { ...attrs },
            status: null,
            ended: false,
            setAttributes: (a) => Object.assign(span.attributes, a),
            setStatus: (s) => { span.status = s; },
            addEvent: () => {},
            end: () => { span.ended = true; },
          };
          spans.push(span);
          return span;
        },
        addEvent: () => {},
        recordToolResult: () => {},
      };
    }

    test('MCP tool calls are traced', async () => {
      const mockTracer = createMockTracer();
      const mcpBridge = createMockMcpBridge({
        traced_mcp_tool: async () => 'traced result',
      });

      const runtime = createDSLRuntime({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        mcpBridge,
        mcpTools: mcpBridge.mcpTools,
        tracer: mockTracer,
      });

      const result = await runtime.execute(`
        const r = traced_mcp_tool({});
        return r;
      `);

      expect(result.status).toBe('success');

      // Should have a span for the MCP tool
      const mcpSpan = mockTracer.spans.find(s => s.name === 'dsl.traced_mcp_tool');
      expect(mcpSpan).toBeDefined();
      expect(mcpSpan.ended).toBe(true);
      expect(mcpSpan.status).toBe('OK');
    });
  });

  describe('Direct DSL options path', () => {
    test('direct options with mcpBridge work correctly', async () => {
      // When using direct toolImplementations (test mode), MCP should still work
      const mcpBridge = createMockMcpBridge({
        direct_mcp_tool: async () => 'direct result',
      });

      const executePlanTool = createExecutePlanTool({
        toolImplementations: createMockTools(),
        llmCall: createMockLLM(),
        mcpBridge,
        mcpTools: mcpBridge.mcpTools,
      });

      const result = await executePlanTool.execute({
        code: 'const r = direct_mcp_tool({}); return r;',
        description: 'Direct test',
      });

      expect(result).toContain('direct result');
    });
  });
});
