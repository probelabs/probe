import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { EventEmitter } from 'node:events';
import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import MCP, { BuiltInMCPServer } from '@probelabs/probe/agent/mcp';

const call = (server, name, args) => server.handleCallTool({ name: `mcp__probe__${name}`, arguments: args });
const framedDigest = payload => {
  const domain = Buffer.from('reqproof.probe.tool-arguments/v1'), body = Buffer.from(payload);
  const length = value => { const bytes = Buffer.alloc(8); bytes.writeBigUInt64BE(BigInt(value.length)); return bytes; };
  return `sha256:${createHash('sha256').update(length(domain)).update(domain).update(length(body)).update(body).digest('hex')}`;
};

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
  await t.test('public package export is stable for JavaScript and NodeNext TypeScript consumers', () => {
    assert.equal(BuiltInMCPServer, MCP.BuiltInMCPServer);
    const packageRoot = fileURLToPath(new URL('../..', import.meta.url));
    const temp = mkdtempSync(join(tmpdir(), 'probe-exp-0159-types-'));
    try {
      const packageLink = join(temp, 'node_modules', '@probelabs', 'probe');
      mkdirSync(dirname(packageLink), { recursive: true });
      symlinkSync(packageRoot, packageLink, 'dir');
      writeFileSync(join(temp, 'package.json'), JSON.stringify({ type: 'module' }));
      writeFileSync(join(temp, 'consumer.ts'), `
        import MCP, { BuiltInMCPServer, MCPClientManager, MCPXmlBridge } from '@probelabs/probe/agent/mcp';
        import type RootProbeAgent from '@probelabs/probe';
        import type AgentProbeAgent from '@probelabs/probe/agent';
        import type { ProbeAgent } from '@probelabs/probe/agent';
        declare const agent: ProbeAgent; declare const rootAgent: RootProbeAgent; declare const subpathAgent: AgentProbeAgent;
        const signals: AbortSignal[] = [agent.abortSignal, rootAgent.abortSignal, subpathAgent.abortSignal]; void signals;
        declare const event: import('@probelabs/probe').ToolCallEvent; const digest: string | undefined = event.argumentsDigest; void digest;
        const server = new BuiltInMCPServer(agent, { port: 0, debug: false });
        const same: typeof BuiltInMCPServer = MCP.BuiltInMCPServer;
        void same; void server.start(); void server.stop(); void server.handleListTools();
        void server.handleCallTool({ name: 'mcp__probe__search', arguments: { query: 'x' } });
        const count: number = server.getToolCount(); const url: string = server.getConfig().url;
        void count; void url; new MCPClientManager(); new MCPXmlBridge();
      `);
      writeFileSync(join(temp, 'tsconfig.json'), JSON.stringify({ compilerOptions: {
        target: 'ES2022', module: 'NodeNext', moduleResolution: 'NodeNext', strict: true,
        noEmit: true, skipLibCheck: true
      }, files: ['consumer.ts'] }));
      const compile = spawnSync(process.execPath, [join(packageRoot, 'node_modules', 'typescript', 'bin', 'tsc'), '-p', join(temp, 'tsconfig.json')], { encoding: 'utf8' });
      assert.equal(compile.status, 0, `${compile.stdout}${compile.stderr}`);
    } finally {
      rmSync(temp, { recursive: true, force: true });
    }
  });

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
      assert.match(events[i].argumentsDigest, /^sha256:[0-9a-f]{64}$/);
      assert.equal(events[i].argumentsDigest, events[i + 1].argumentsDigest);
      for (const key of ['params', 'args', 'arguments', 'result', 'resultPreview', 'preview', 'body', 'source']) {
        assert.equal(Object.prototype.hasOwnProperty.call(events[i], key), false);
        assert.equal(Object.prototype.hasOwnProperty.call(events[i + 1], key), false);
      }
    }
  });

  await t.test('digest binds canonical validated arguments, defaults, framing, and number rules', async () => {
    const { agent, server } = fixture();
    const events = []; agent.events.on('toolCall', event => events.push(event));
    await call(server, 'extract', { targets: 'http.go http_test.go' });
    await call(server, 'extract', { allow_tests: true, targets: 'http.go http_test.go' });
    await call(server, 'extract', { targets: 'http.go http_test.go', allow_tests: false });
    await call(server, 'search', { maxTokens: -0, query: 'ordered' });
    await call(server, 'search', { query: 'ordered', maxTokens: 0 });
    const digests = events.filter(event => event.status === 'in_progress').map(event => event.argumentsDigest);
    const payload = '{"allow_tests":true,"targets":"http.go http_test.go"}';
    assert.equal(digests[0], framedDigest(payload));
    assert.equal(digests[0], digests[1]);
    assert.notEqual(digests[1], digests[2]);
    assert.equal(digests[3], digests[4]);
    assert.notEqual(digests[0], `sha256:${createHash('sha256').update(Buffer.from(payload)).digest('hex')}`);
    const nullRecord = Object.assign(Object.create(null), { targets: 'http.go http_test.go' });
    await call(server, 'extract', nullRecord);
    assert.equal(events.at(-2).argumentsDigest, digests[0]);
  });

  await t.test('invalid values and containers fail before lifecycle without leaking details', async () => {
    const { agent, server, calls } = fixture();
    const events = []; agent.events.on('toolCall', event => events.push(event));
    const accessor = {}; Object.defineProperty(accessor, 'query', { enumerable: true, get() { throw new Error('ACCESSOR_SECRET'); } });
    const hidden = { query: 'safe' }; Object.defineProperty(hidden, 'HIDDEN_SECRET', { value: true });
    const symbolKey = { query: 'safe', [Symbol('SYMBOL_SECRET')]: true };
    const exotic = Object.assign(Object.create({ EXOTIC_SECRET: true }), { query: 'safe' });
    const hook = { query: 'safe', toJSON() { return 'HOOK_SECRET'; } };
    const trapped = new Proxy({ query: 'safe' }, { ownKeys() { throw new Error('PROXY_SECRET'); } });
    const cycle = { query: 'safe' }; cycle.self = cycle;
    const hole = []; hole.length = 1;
    const extra = ['safe']; extra.extra = true;
    const containerCases = [null, [], accessor, hidden, symbolKey, exotic, hook, trapped, cycle, { query: hole }, { query: extra }];
    for (const args of containerCases) {
      const result = await call(server, 'search', args);
      assert.equal(result.isError, true);
      assert.equal(result.content[0].text, 'Error executing mcp__probe__search: TOOL_ARGUMENT_CONTAINER_INVALID');
    }
    const invalidCases = [undefined, 1n, () => {}, Symbol('VALUE_SECRET'), NaN, Infinity];
    for (const query of invalidCases) {
      const result = await call(server, 'search', { query });
      assert.equal(result.isError, true);
      assert.equal(result.content[0].text, 'Error executing mcp__probe__search: TOOL_ARGUMENT_VALIDATION_FAILED');
    }
    assert.equal(calls.length, 0); assert.deepEqual(events, []);
    const diagnostics = containerCases.length + invalidCases.length;
    assert.equal(diagnostics, 17);
  });

  await t.test('failed lifecycle exposes one stable code and retains its digest', async () => {
    const { agent, server, calls } = fixture();
    agent.toolImplementations.search.execute = async () => { throw new Error('EXECUTION_SECRET'); };
    const events = []; agent.events.on('toolCall', event => events.push(event));
    const result = await call(server, 'search', { query: 'ARGUMENT_SECRET' });
    assert.equal(result.isError, true); assert.equal(calls.length, 0);
    assert.deepEqual(events.map(event => event.status), ['in_progress', 'failed']);
    assert.equal(events[0].argumentsDigest, events[1].argumentsDigest);
    assert.equal(events[1].error, 'TOOL_EXECUTION_FAILED');
    const publicTrace = JSON.stringify(events);
    for (const secret of ['ARGUMENT_SECRET', 'EXECUTION_SECRET']) assert.equal(publicTrace.includes(secret), false);
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
    const controller = new AbortController(), events = new EventEmitter();
    let entered = 0, observedSignal, closes = 0, markEntered;
    const started = new Promise(resolve => { markEntered = resolve; });
    const agent = { allowedTools: { isEnabled: name => name === 'search' }, toolImplementations: { search: {} }, events, sessionId: 'public-facade', cwd: process.cwd(),
      get abortSignal() { return controller.signal; }, cancel() { controller.abort(); }, async close() { closes++; await server.stop(); } };
    const server = new BuiltInMCPServer(agent);
    assert.equal('_abortController' in agent, false);
    // This is cooperative AbortSignal handling; arbitrary binary termination requires external containment.
    agent.toolImplementations.search.execute = ({ abortSignal }) => new Promise((resolve, reject) => {
      entered++; observedSignal = abortSignal; markEntered();
      abortSignal.addEventListener('abort', () => reject(new Error('controlled child cancelled')), { once: true });
    });
    const progress = []; agent.events.on('toolCall', event => progress.push(event));
    const pending = call(server, 'search', { query: 'wait' });
    await started; assert.equal(entered, 1); assert.equal(observedSignal, agent.abortSignal); agent.cancel();
    const result = await pending; await agent.close();
    assert.equal(result.isError, true);
    assert.equal(closes, 1);
    assert.deepEqual(progress.map(event => event.status), ['in_progress', 'failed']);
    assert.equal(progress[0].argumentsDigest, progress[1].argumentsDigest);
    assert.equal(progress[1].error, 'TOOL_EXECUTION_FAILED');
  });
});
