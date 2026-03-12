/**
 * Tests for in-process MCP server infrastructure and graceful_stop flow.
 *
 * Uses InMemoryTransport to run MCP servers inside Jest without spawning
 * external processes. Validates:
 * - In-process MCP server setup and tool registration
 * - MCPClientManager connecting via InMemoryTransport
 * - graceful_stop tool detection and invocation
 * - Agent-type MCP server graceful shutdown flow
 */

import { describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { MCPClientManager } from '../../src/agent/mcp/client.js';
import {
  InProcessMcpServer,
  createAgentMcpServer,
} from './inProcessMcpServer.js';

describe('InProcessMcpServer', () => {
  let server;

  afterEach(async () => {
    if (server) await server.stop();
  });

  test('creates a server and lists tools', async () => {
    server = new InProcessMcpServer('test-server');
    server.addTool(
      {
        name: 'echo',
        description: 'Echo a message',
        inputSchema: {
          type: 'object',
          properties: { message: { type: 'string' } },
        },
      },
      async (args) => args.message
    );

    const { clientTransport } = await server.start();
    expect(clientTransport).toBeTruthy();

    // Connect MCPClientManager
    const manager = new MCPClientManager();
    await manager.initialize({
      mcpServers: {
        'test-server': server.getClientConfig(),
      },
    });

    expect(manager.tools.size).toBe(1);
    expect(manager.tools.has('test-server_echo')).toBe(true);

    await manager.disconnect();
  });

  test('executes a tool and returns result', async () => {
    server = new InProcessMcpServer('test-server');
    server.addTool(
      {
        name: 'greet',
        description: 'Greet someone',
        inputSchema: {
          type: 'object',
          properties: { name: { type: 'string' } },
        },
      },
      async (args) => `Hello, ${args.name}!`
    );

    await server.start();

    const manager = new MCPClientManager();
    await manager.initialize({
      mcpServers: {
        'test-server': server.getClientConfig(),
      },
    });

    const result = await manager.callTool('test-server_greet', { name: 'World' });
    expect(result.content[0].text).toBe('Hello, World!');

    await manager.disconnect();
  });

  test('handles tool errors', async () => {
    server = new InProcessMcpServer('test-server');
    server.addTool(
      { name: 'fail', description: 'Always fails' },
      async () => { throw new Error('intentional failure'); }
    );

    await server.start();

    const manager = new MCPClientManager();
    await manager.initialize({
      mcpServers: {
        'test-server': server.getClientConfig(),
      },
    });

    const result = await manager.callTool('test-server_fail', {});
    expect(result.content[0].text).toContain('intentional failure');
    expect(result.isError).toBe(true);

    await manager.disconnect();
  });

  test('supports multiple tools on one server', async () => {
    server = new InProcessMcpServer('multi');
    server
      .addTool({ name: 'tool_a', description: 'A' }, async () => 'a')
      .addTool({ name: 'tool_b', description: 'B' }, async () => 'b')
      .addTool({ name: 'tool_c', description: 'C' }, async () => 'c');

    await server.start();

    const manager = new MCPClientManager();
    await manager.initialize({
      mcpServers: { multi: server.getClientConfig() },
    });

    expect(manager.tools.size).toBe(3);

    const resultA = await manager.callTool('multi_tool_a', {});
    expect(resultA.content[0].text).toBe('a');

    const resultC = await manager.callTool('multi_tool_c', {});
    expect(resultC.content[0].text).toBe('c');

    await manager.disconnect();
  });
});

describe('MCPClientManager.callGracefulStopAll', () => {
  let server;

  afterEach(async () => {
    if (server) await server.stop();
  });

  test('calls graceful_stop on servers that expose it', async () => {
    server = createAgentMcpServer('agent');
    await server.start();

    const manager = new MCPClientManager();
    await manager.initialize({
      mcpServers: { agent: server.getClientConfig() },
    });

    // Verify graceful_stop is detected
    expect(manager.tools.has('agent_graceful_stop')).toBe(true);

    const results = await manager.callGracefulStopAll();
    expect(results).toHaveLength(1);
    expect(results[0]).toEqual({ server: 'agent', success: true });

    await manager.disconnect();
  });

  test('skips servers without graceful_stop', async () => {
    server = new InProcessMcpServer('plain');
    server.addTool({ name: 'echo', description: 'Echo' }, async (args) => args.message);
    await server.start();

    const manager = new MCPClientManager();
    await manager.initialize({
      mcpServers: { plain: server.getClientConfig() },
    });

    expect(manager.tools.has('plain_graceful_stop')).toBe(false);

    const results = await manager.callGracefulStopAll();
    expect(results).toHaveLength(0);

    await manager.disconnect();
  });

  test('handles mixed servers (some with graceful_stop, some without)', async () => {
    const agentServer = createAgentMcpServer('agent');
    const plainServer = new InProcessMcpServer('plain');
    plainServer.addTool({ name: 'echo' }, async () => 'echo');

    await agentServer.start();
    await plainServer.start();

    const manager = new MCPClientManager();
    await manager.initialize({
      mcpServers: {
        agent: agentServer.getClientConfig(),
        plain: plainServer.getClientConfig(),
      },
    });

    const results = await manager.callGracefulStopAll();
    expect(results).toHaveLength(1);
    expect(results[0].server).toBe('agent');

    await manager.disconnect();
    await agentServer.stop();
    await plainServer.stop();
    server = null; // prevent double cleanup
  });
});

describe('Agent MCP Server graceful_stop flow', () => {
  let server;

  afterEach(async () => {
    if (server) await server.stop();
  });

  test('analyze completes fully without graceful_stop', async () => {
    server = createAgentMcpServer('agent');
    await server.start();

    const manager = new MCPClientManager();
    await manager.initialize({
      mcpServers: { agent: server.getClientConfig() },
    });

    const result = await manager.callTool('agent_analyze', { query: 'test' });
    expect(result.content[0].text).toContain('Complete analysis');
    expect(result.content[0].text).toContain('chunk 9'); // all 10 steps

    await manager.disconnect();
  });

  test('graceful_stop interrupts running analyze and returns partial results', async () => {
    server = createAgentMcpServer('agent');
    await server.start();

    const manager = new MCPClientManager();
    await manager.initialize({
      mcpServers: { agent: server.getClientConfig() },
    });

    // Start analyze (runs for ~500ms total: 10 steps x 50ms)
    const analyzePromise = manager.callTool('agent_analyze', { query: 'architecture' });

    // Wait a bit for analyze to start, then call graceful_stop
    await new Promise((resolve) => setTimeout(resolve, 120));
    expect(server.getState().analyzeRunning).toBe(true);

    await manager.callGracefulStopAll();
    expect(server.getState().stopRequested).toBe(true);

    // Wait for analyze to finish (it should stop early)
    const result = await analyzePromise;
    expect(result.content[0].text).toContain('Partial analysis');
    expect(result.content[0].text).toContain('interrupted by graceful_stop');
    // Should have fewer than 10 steps
    expect(result.content[0].text).not.toContain('chunk 9');

    await manager.disconnect();
  });

  test('graceful_stop is a no-op when no task is running', async () => {
    server = createAgentMcpServer('agent');
    await server.start();

    const manager = new MCPClientManager();
    await manager.initialize({
      mcpServers: { agent: server.getClientConfig() },
    });

    // Call graceful_stop with nothing running
    const stopResult = await manager.callTool('agent_graceful_stop', {});
    expect(stopResult.content[0].text).toContain('Graceful stop acknowledged');

    // Analyze should still work after stop (stop flag is reset per-run)
    const analyzeResult = await manager.callTool('agent_analyze', { query: 'test' });
    expect(analyzeResult.content[0].text).toContain('Complete analysis');

    await manager.disconnect();
  });
});
