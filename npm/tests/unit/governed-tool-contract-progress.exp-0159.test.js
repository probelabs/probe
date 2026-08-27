import test from 'node:test';
import assert from 'node:assert/strict';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { BuiltInMCPServer } from '../../src/agent/mcp/built-in-server.js';

const call = (server, name, args) => server.handleCallTool({ name: `mcp__probe__${name}`, arguments: args });

function fixture() {
  const agent = new ProbeAgent({ allowedTools: ['search', 'extract', 'listFiles'], sessionId: 'exp-0159-session', cwd: process.cwd() });
  const server = new BuiltInMCPServer(agent);
  const calls = [];
  const lifecycle = { closes: 0 };
  for (const name of ['search', 'extract', 'listFiles']) {
    agent.toolImplementations[name] = { execute: async params => { calls.push({ name, params }); return `${name}-ok`; } };
  }
  agent.engine = { close: async () => { lifecycle.closes++; await server.stop(); } };
  return { agent, server, calls, lifecycle };
}

test('EXP-0159 canonical governed tool contracts and progress', async t => {
  await t.test('tools/list describes the inputs direct implementations accept', async () => {
    const { server } = fixture();
    const listed = await server.handleListTools();
    const schemas = Object.fromEntries(listed.tools.map(tool => [tool.name, tool.inputSchema]));
    assert.deepEqual(Object.keys(schemas.mcp__probe__search.properties), ['query', 'path', 'exact', 'maxTokens', 'session', 'nextPage']);
    assert.deepEqual(schemas.mcp__probe__search.required, ['query']);
    assert.deepEqual(Object.keys(schemas.mcp__probe__extract.properties), ['targets', 'input_content', 'allow_tests']);
    assert.deepEqual(Object.keys(schemas.mcp__probe__listFiles.properties), ['directory']);
    assert.equal(schemas.mcp__probe__listFiles.required, undefined);
  });

  await t.test('canonical calls emit paired public lifecycle without result bodies', async () => {
    const { agent, server, calls } = fixture();
    const events = [];
    agent.events.on('toolCall', event => events.push(event));
    for (const [name, args] of [['search', { query: 'governed', maxTokens: 50 }], ['extract', { targets: 'src/main.rs' }], ['listFiles', { directory: '.' }]]) {
      const result = await call(server, name, args);
      assert.equal(result.isError, undefined);
    }
    assert.equal(calls.length, 3);
    assert.deepEqual(events.map(({ name, status }) => [name, status]), [
      ['search', 'in_progress'], ['search', 'completed'],
      ['extract', 'in_progress'], ['extract', 'completed'],
      ['listFiles', 'in_progress'], ['listFiles', 'completed']
    ]);
    for (let i = 0; i < events.length; i += 2) {
      assert.equal(events[i].id, events[i + 1].id);
      assert.equal(events[i].sessionId, 'exp-0159-session');
      assert.equal(events[i + 1].sessionId, 'exp-0159-session');
      assert.equal(typeof events[i + 1].duration, 'number');
      for (const key of ['params', 'args', 'result', 'resultPreview', 'preview', 'body']) {
        assert.equal(Object.prototype.hasOwnProperty.call(events[i], key), false);
        assert.equal(Object.prototype.hasOwnProperty.call(events[i + 1], key), false);
      }
    }
  });

  await t.test('malformed and unknown shapes fail before execution', async () => {
    const { server, calls } = fixture();
    assert.equal((await call(server, 'search', { query: 7 })).isError, true);
    assert.equal((await call(server, 'extract', { path: 'wrong-shape' })).isError, true);
    assert.equal((await call(server, 'listFiles', { directory: '.', surprise: true })).isError, true);
    assert.equal((await call(server, 'search', { query: 'x', constructor: 'owned' })).isError, true);
    assert.equal((await call(server, 'search', { query: 'x', toString: 'owned' })).isError, true);
    await assert.rejects(call(server, 'unknown', {}), /not enabled|not found/);
    assert.equal(calls.length, 0);
  });

  await t.test('listener errors fail closed before tool execution', async () => {
    const { agent, server, calls } = fixture();
    const observed = [];
    agent.events.on('toolCall', event => { if (event.status === 'in_progress') throw new Error('observer refused call'); });
    agent.events.on('toolCall', event => observed.push(event.status));
    const result = await call(server, 'search', { query: 'blocked' });
    assert.equal(result.isError, true);
    assert.match(result.content[0].text, /observer refused call/);
    assert.deepEqual(observed, ['failed']);
    assert.equal(calls.length, 0);
  });

  await t.test('completion observer failure cannot reclassify successful execution', async () => {
    const { agent, server, calls } = fixture();
    agent.events.on('toolCall', event => { if (event.status === 'completed') throw new Error('late observer failure'); });
    const result = await call(server, 'search', { query: 'done' });
    assert.equal(result.isError, undefined);
    assert.equal(result.content[0].text, 'search-ok');
    assert.equal(calls.length, 1);
  });

  await t.test('cooperative cancellation settles the call and close bookkeeping', async () => {
    const { agent, server, lifecycle } = fixture();
    let active = false;
    let entered;
    const started = new Promise(resolve => { entered = resolve; });
    // This is cooperative AbortSignal handling; arbitrary binary termination requires external containment.
    agent.toolImplementations.search.execute = () => new Promise((resolve, reject) => {
      active = true;
      entered();
      agent._abortController.signal.addEventListener('abort', () => { active = false; reject(new Error('controlled child cancelled')); }, { once: true });
    });
    const events = [];
    agent.events.on('toolCall', event => events.push(event));
    const pending = call(server, 'search', { query: 'wait' });
    await started;
    agent.cancel();
    const result = await pending;
    await agent.close();
    assert.equal(result.isError, true);
    assert.equal(active, false);
    assert.equal(lifecycle.closes, 1);
    assert.deepEqual(events.map(event => event.status), ['in_progress', 'failed']);
  });
});
