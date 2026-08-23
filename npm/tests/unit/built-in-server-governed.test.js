import { describe, expect, jest, test } from '@jest/globals';
import { BuiltInMCPServer } from '../../src/agent/mcp/built-in-server.js';
import { markProbeAgentForTests } from '../../src/agent/governance-marker.js';

const TOOL_NAMES = ['search', 'extract', 'listFiles'];

function governedAgent(overrides = {}) {
  const counts = { search: 0, extract: 0, listFiles: 0 };
  const implementations = Object.fromEntries(TOOL_NAMES.map(name => [name, {
    execute: async args => {
      counts[name]++;
      return `${name}:${args?.value || ''}`;
    }
  }]));
  for (const [name, implementation] of Object.entries(overrides)) {
    implementations[name] = {
      ...implementation,
      execute: async args => {
        counts[name]++;
        return implementation.execute(args);
      }
    };
  }
  return {
    agent: markProbeAgentForTests({
      allowedTools: {
        mode: 'whitelist',
        allowed: TOOL_NAMES,
        exclusions: [],
        isEnabled: name => TOOL_NAMES.includes(name)
      },
      toolImplementations: implementations
    }),
    counts
  };
}

function metadata(progressToken = 1, extension = {}) {
  return {
    progressToken,
    threadId: 'governed-thread',
    'x-codex-turn-metadata': {
      session_id: 'governed-thread',
      thread_id: 'governed-thread',
      turn_id: '2',
      sandbox: 'seatbelt',
      turn_started_at_unix_ms: 1787445973467,
      model: 'gpt-5.6-luna',
      reasoning_effort: 'xhigh',
      ...extension
    }
  };
}

function callParams(progressToken = 1, name = 'mcp__probe__search', args = { value: 'ok' }, extension = {}) {
  return { _meta: metadata(progressToken, extension), name, arguments: args };
}

function expectLedgerClear(server) {
  expect(server.audit.inFlight).toBe(0);
  expect(server.audit.reservedBytes).toBe(0);
}

function expectAccounting(server, executionCounts, toolCallCount = 0) {
  const snapshot = server.getAuditSnapshot();
  expect(snapshot.toolCalls).toHaveLength(toolCallCount);
  expect(snapshot.executionCounts).toEqual(executionCounts);
  expectLedgerClear(server);
  return snapshot;
}

function newServer(overrides = {}) {
  const { agent, counts } = governedAgent(overrides);
  return {
    server: new BuiltInMCPServer(agent, { governed: true, serverName: 'probe_test' }),
    counts
  };
}

describe('real governed BuiltInMCPServer surface', () => {
  test('latches exact metadata identity and exposes only exact Probe tools', async () => {
    const { server, counts } = newServer();

    expect((await server.handleListTools()).tools.map(tool => tool.name)).toEqual([
      'mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles'
    ]);
    await expect(server.handleCallTool(callParams())).resolves.toEqual({
      content: [{ type: 'text', text: 'search:ok' }]
    });
    expect(server.governedIdentity).toEqual({
      session_id: 'governed-thread',
      thread_id: 'governed-thread',
      turn_id: '2',
      sandbox: 'seatbelt',
      turn_started_at_unix_ms: 1787445973467,
      model: 'gpt-5.6-luna',
      reasoning_effort: 'xhigh',
      threadId: 'governed-thread'
    });

    await expect(server.handleCallTool(callParams(2, 'mcp__probe__search', {}, { turn_id: '3' })))
      .rejects.toThrow(/identity does not match/);
    await expect(server.handleCallTool({ ...callParams(), name: 'search' }))
      .rejects.toThrow(/exact allowlisted/);
    await expect(server.handleCallTool({ ...callParams(), name: 'mcp__probe__bash' }))
      .rejects.toThrow(/exact allowlisted/);
    await expect(server.handleCallTool({ ...callParams(), server: 'wrong' }))
      .rejects.toThrow(/server identity/);

    const snapshot = expectAccounting(server, { search: 1, extract: 0, listFiles: 0 }, 1);
    expect(counts.search).toBe(1);
    expect(snapshot.toolCalls[0]).toEqual(expect.objectContaining({
      name: 'mcp__probe__search',
      arguments: { sha256: expect.any(String), bytes: 14 },
      result: expect.objectContaining({ status: 'ok' }),
      metadata: expect.objectContaining({ session_id: 'governed-thread', turn_id: '2' })
    }));
  });

  test('rejects duplicate and out-of-order progress tokens before execute', async () => {
    const { server, counts } = newServer();

    await expect(server.handleCallTool(callParams(1))).resolves.toEqual({
      content: [{ type: 'text', text: 'search:ok' }]
    });
    await expect(server.handleCallTool(callParams(1))).rejects.toThrow(/duplicate or out of order/);
    await expect(server.handleCallTool(callParams(3))).resolves.toEqual({
      content: [{ type: 'text', text: 'search:ok' }]
    });
    await expect(server.handleCallTool(callParams(2))).rejects.toThrow(/duplicate or out of order/);

    expect(counts.search).toBe(2);
    expectAccounting(server, { search: 2, extract: 0, listFiles: 0 }, 2);
  });

  test('rejects a concurrent duplicate token before execute and records one side effect', async () => {
    let executionStarted;
    let releaseExecution;
    const started = new Promise(resolve => { executionStarted = resolve; });
    const release = new Promise(resolve => { releaseExecution = resolve; });
    const execute = jest.fn(async () => {
      executionStarted();
      await release;
      return 'held';
    });
    const { server } = newServer({ search: { execute } });

    const firstCall = server.handleCallTool(callParams(1));
    await started;
    await expect(server.handleCallTool(callParams(1))).rejects.toThrow(/duplicate or out of order/);
    releaseExecution();
    await expect(firstCall).resolves.toEqual({ content: [{ type: 'text', text: 'held' }] });

    expect(execute).toHaveBeenCalledTimes(1);
    expectAccounting(server, { search: 1, extract: 0, listFiles: 0 }, 1);
  });

  test('persists bounded failed terminal audit records for throws and oversized results', async () => {
    const { server, counts } = newServer({
      search: { execute: async () => { throw new Error('x'.repeat(2 * 1024 * 1024)); } },
      extract: { execute: async () => 'x'.repeat(262145) }
    });

    await expect(server.handleCallTool(callParams(1))).resolves.toMatchObject({ isError: true });
    await expect(server.handleCallTool(callParams(2, 'mcp__probe__extract'))).resolves.toMatchObject({ isError: true });

    const snapshot = expectAccounting(server, { search: 1, extract: 1, listFiles: 0 }, 2);
    expect(counts.search).toBe(1);
    expect(counts.extract).toBe(1);
    expect(snapshot.toolCalls).toEqual(expect.arrayContaining([
      expect.objectContaining({ name: 'mcp__probe__search', result: expect.objectContaining({ status: 'failed' }) }),
      expect.objectContaining({ name: 'mcp__probe__extract', result: expect.objectContaining({ status: 'failed' }) })
    ]));
    for (const record of snapshot.toolCalls) {
      expect(record.result.bytes).toBeLessThanOrEqual(1048576);
    }
    expect(Buffer.byteLength(JSON.stringify(snapshot), 'utf8')).toBeLessThanOrEqual(1048576);
  });

  test('rejects oversized args and invalid or extra extension metadata before latching identity', async () => {
    const { server, counts } = newServer();
    const oversizedArgs = { value: 'x'.repeat(1048577) };

    await expect(server.handleCallTool(callParams(1, 'mcp__probe__search', oversizedArgs)))
      .rejects.toThrow(/serialized-byte bound/);
    await expect(server.handleCallTool({
      ...callParams(),
      _meta: { ...metadata(), 'x-codex-turn-metadata': { ...metadata()['x-codex-turn-metadata'], extra: true } }
    })).rejects.toThrow(/keys are not exact/);
    await expect(server.handleCallTool({
      ...callParams(),
      _meta: { ...metadata(), progressToken: 0 }
    })).rejects.toThrow(/progressToken is invalid/);

    expect(server.governedIdentity).toBeNull();
    expect(counts.search).toBe(0);
    expectAccounting(server, { search: 0, extract: 0, listFiles: 0 }, 0);
  });

  test('accepts 64 compact calls and rejects the 65th before execute', async () => {
    const { server, counts } = newServer();

    for (let token = 1; token <= 64; token++) {
      await expect(server.handleCallTool(callParams(token, 'mcp__probe__search', {})))
        .resolves.toEqual({ content: [{ type: 'text', text: 'search:' }] });
    }
    await expect(server.handleCallTool(callParams(65, 'mcp__probe__search', {})))
      .rejects.toThrow(/audit bound exceeded/);

    const snapshot = expectAccounting(server, { search: 64, extract: 0, listFiles: 0 }, 64);
    expect(counts.search).toBe(64);
    expect(snapshot.toolCalls.at(-1).ordinal).toBe(64);
  });
});
