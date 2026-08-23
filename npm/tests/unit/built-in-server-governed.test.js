import { describe, expect, test } from '@jest/globals';
import { BuiltInMCPServer } from '../../src/agent/mcp/built-in-server.js';

function agent() {
  const names = ['search', 'extract', 'listFiles'];
  return {
    allowedTools: { isEnabled: name => names.includes(name) },
    toolImplementations: Object.fromEntries(names.map(name => [name, { execute: async args => `${name}:${args?.value || ''}` }]))
  };
}

describe('real governed BuiltInMCPServer surface', () => {
  test('lists exactly Probe search, extract, and listFiles and calls only exact names', async () => {
    const server = new BuiltInMCPServer(agent(), { governed: true, serverName: 'probe_test' });
    expect((await server.handleListTools()).tools.map(tool => tool.name)).toEqual([
      'mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles'
    ]);
    await expect(server.handleCallTool({ name: 'mcp__probe__search', arguments: { value: 'ok' } }))
      .resolves.toEqual({ content: [{ type: 'text', text: 'search:ok' }] });
    await expect(server.handleCallTool({ name: 'search', arguments: {} })).rejects.toThrow(/exact allowlisted/);
    await expect(server.handleCallTool({ name: 'mcp__probe__bash', arguments: {} })).rejects.toThrow(/exact allowlisted/);
    await expect(server.handleCallTool({ name: 'mcp__probe__search', server: 'wrong', arguments: {} })).rejects.toThrow(/server identity/);
  });
});
