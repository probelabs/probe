import { describe, expect, test } from '@jest/globals';
import { BuiltInMCPServer } from '../../src/agent/mcp/built-in-server.js';
import { markProbeAgentForTests } from '../../src/agent/governance-marker.js';

function agent() {
  const names = ['search', 'extract', 'listFiles'];
  return markProbeAgentForTests({
    allowedTools: { mode: 'whitelist', allowed: names, exclusions: [], isEnabled: name => names.includes(name) },
    toolImplementations: Object.fromEntries(names.map(name => [name, { execute: async args => `${name}:${args?.value || ''}` }]))
  });
}

function meta() {
  return {
    progressToken: 1,
    threadId: 'governed-thread',
    'x-codex-turn-metadata': {
      session_id: 'governed-thread',
      thread_id: 'governed-thread',
      turn_id: '2',
      sandbox: 'seatbelt',
      turn_started_at_unix_ms: 1787445973467,
      model: 'gpt-5.6-luna',
      reasoning_effort: 'xhigh'
    }
  };
}

describe('real governed BuiltInMCPServer surface', () => {
  test('lists exactly Probe search, extract, and listFiles and calls only exact names', async () => {
    const server = new BuiltInMCPServer(agent(), { governed: true, serverName: 'probe_test' });
    expect((await server.handleListTools()).tools.map(tool => tool.name)).toEqual([
      'mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles'
    ]);
    await expect(server.handleCallTool({ _meta: meta(), name: 'mcp__probe__search', arguments: { value: 'ok' } }))
      .resolves.toEqual({ content: [{ type: 'text', text: 'search:ok' }] });
    await expect(server.handleCallTool({ _meta: meta(), name: 'search', arguments: {} })).rejects.toThrow(/exact allowlisted/);
    await expect(server.handleCallTool({ _meta: meta(), name: 'mcp__probe__bash', arguments: {} })).rejects.toThrow(/exact allowlisted/);
    await expect(server.handleCallTool({ _meta: meta(), name: 'mcp__probe__search', server: 'wrong', arguments: {} })).rejects.toThrow(/server identity/);
    expect(server.getAuditSnapshot().toolCalls).toEqual([
      expect.objectContaining({
        name: 'mcp__probe__search',
        arguments: expect.objectContaining({ bytes: 14 }),
        result: expect.objectContaining({ status: 'ok' }),
        metadata: expect.objectContaining({ session_id: 'governed-thread', turn_id: '2' })
      })
    ]);
  });
});
