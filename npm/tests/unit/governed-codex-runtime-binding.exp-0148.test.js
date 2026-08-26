import test from 'node:test';
import assert from 'node:assert/strict';
import { chmod, mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { createCodexEngine } from '../../src/agent/engines/codex.js';
import { buildGovernedCodexInitialToolArgs } from '../../src/agent/engines/governed-codex-profile.js';

const TOOLS = ['search', 'extract', 'listFiles'];
const fakeCodex = `#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import { createInterface } from 'node:readline';
const file = process.env.PROBE_EXP0148_STATE + '/' + process.pid + '.json';
const seen = []; const save = () => writeFileSync(file, JSON.stringify({ pid: process.pid, seen }));
const send = (value) => process.stdout.write(JSON.stringify(value) + '\\n');
const configured = (requestId, args, patch = {}) => ({ jsonrpc: '2.0', method: 'codex/event', params: {
  _meta: { requestId, threadId: 'session-' + process.pid }, id: '', msg: {
    type: 'session_configured', session_id: 'session-' + process.pid, thread_id: 'session-' + process.pid,
    model: 'gpt-5.6-luna', model_provider_id: 'openai', approval_policy: 'never', approvals_reviewer: 'user',
    permission_profile: { type: 'managed', file_system: { type: 'restricted', entries: [{ access: 'read', path: { type: 'special', value: { kind: 'root' } } }] }, network: 'restricted' },
    reasoning_effort: 'xhigh', rollout_path: args.cwd + '/sessions/2026/08/26/rollout-2026-08-26T12-00-00-00000000-0000-4000-8000-' + String(process.pid).padStart(12, '0') + '.jsonl', cwd: args.cwd, ...patch
  }
}});
createInterface({ input: process.stdin }).on('line', async line => {
  const request = JSON.parse(line); seen.push(request); save();
  if (request.method === 'initialize') return send({ jsonrpc: '2.0', id: request.id, result: {} });
  const args = request.params.arguments; const prompt = args.prompt;
  if (prompt.includes('[WAIT]')) return;
  if (prompt.includes('[EVENT]')) send({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: request.id }, msg: { type: 'raw_response_item', item: { role: 'assistant', content: [{ type: 'text', text: 'event-candidate' }] } } } });
  if (prompt.includes('[DELAY]')) await new Promise(resolve => setTimeout(resolve, 80));
  let events = [configured(request.id, args)];
  if (prompt.includes('[MISSING]')) events = [];
  if (prompt.includes('[DUPLICATE]')) events.push(events[0]);
  if (prompt.includes('[MALFORMED]')) events = [{ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: request.id }, id: '', msg: { type: 'session_configured' } } }];
  if (prompt.includes('[UNCORRELATED]')) events = [configured(999, args)];
  if (prompt.includes('[MISMATCHED]')) events = [configured(request.id, args, { model: 'wrong' })];
  for (const event of events) send(event);
  let text = 'candidate-' + process.pid;
  if (prompt.includes('[FORBIDDEN]')) {
    const url = Object.values(args.config.mcp_servers)[0].url.replace('/mcp', '/rpc');
    const response = await fetch(url, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: 'mcp__probe__bash', arguments: {} } }) });
    text = (await response.json()).error ? 'host-denied' : 'host-allowed';
  }
  send({ jsonrpc: '2.0', id: request.id, result: { content: [{ type: 'text', text }] } });
});
`;

function profile(cwd) {
  return { version: 'probe.governed-codex-profile/v1', profileId: 'luna-xhigh-readonly-v1', engine: 'codex', model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'read-only', approvalPolicy: 'never', cwd, probeTools: [...TOOLS], fallback: false, retries: 0 };
}
async function stateFiles(dir) {
  return (await readdir(dir)).filter(name => name.endsWith('.json'));
}
async function waitForState(dir, before = [], marker = null) {
  for (let i = 0; i < 100; i++) {
    const files = (await stateFiles(dir)).filter(name => !before.includes(name));
    for (const name of files) {
      const file = join(dir, name);
      try {
        const state = await readState(file);
        if (!marker || state.seen.some(item => item.params?.arguments?.prompt?.includes(marker))) return file;
      } catch { /* Child may be replacing its state snapshot. */ }
    }
    await new Promise(resolve => setTimeout(resolve, 10));
  }
  throw new Error('fake Codex state timeout');
}
async function readState(file) {
  return JSON.parse(await readFile(file, 'utf8'));
}
async function collect(iterator) {
  const output = []; for await (const item of iterator) output.push(item); return output;
}
function alive(pid) {
  try { process.kill(pid, 0); return true; } catch { return false; }
}
async function closed(url) {
  try { await fetch(url.replace('/mcp', '/health'), { signal: AbortSignal.timeout(250) }); return false; } catch { return true; }
}
function host() {
  return { allowedTools: { isEnabled: name => TOOLS.includes(name) }, toolImplementations: Object.fromEntries(TOOLS.map(name => [name, { execute: async () => name }])) };
}
async function runEngine(root, stateDir, marker, useProbe = false) {
  const before = await stateFiles(stateDir); let engine;
  if (useProbe) {
    const agent = new ProbeAgent({ provider: 'codex', path: root, cwd: root, allowedTools: [...TOOLS], governedCodexProfile: profile(root), disableMermaidValidation: true });
    engine = await agent.getEngine();
  } else engine = await createCodexEngine({ agent: host(), governedCodexProfile: profile(root) });
  const output = await collect(engine.query(marker));
  const state = await readState(await waitForState(stateDir, before, marker));
  const call = state.seen.find(item => item.method === 'tools/call');
  const url = Object.values(call.params.arguments.config.mcp_servers)[0].url;
  return { engine, output, state, call, url };
}

test('EXP-0148 governed Codex runtime binding', async t => {
  const root = await mkdtemp(join(tmpdir(), 'probe-exp0148-'));
  const bin = join(root, 'bin'); const stateDir = join(root, 'state');
  await mkdir(bin); await mkdir(stateDir);
  const executable = join(bin, 'codex'); await writeFile(executable, fakeCodex); await chmod(executable, 0o755);
  const originalPath = process.env.PATH; process.env.PATH = `${bin}:${originalPath}`; process.env.PROBE_EXP0148_STATE = stateDir;
  t.after(async () => { process.env.PATH = originalPath; delete process.env.PROBE_EXP0148_STATE; await rm(root, { recursive: true, force: true }); });

  await t.test('rejects non-exact public bindings before spawn', () => {
    const count = () => stateFiles(stateDir);
    assert.throws(() => new ProbeAgent({ provider: 'openai', allowedTools: [...TOOLS], governedCodexProfile: profile(root) }), /provider codex/);
    for (const allowedTools of [undefined, ['*'], ['search', 'listFiles', 'extract'], ['search', 'extract', 'listFiles', 'bash'], ['search', 'extract', 'extract']]) {
      assert.throws(() => new ProbeAgent({ provider: 'codex', allowedTools, governedCodexProfile: profile(root) }), /exactly match/);
    }
    return count().then(files => assert.equal(files.length, 0));
  });

  await t.test('uses exact C5a request, buffers output, sanitizes receipt, and closes', async () => {
    const before = await stateFiles(stateDir);
    const agent = new ProbeAgent({ provider: 'codex', path: root, cwd: root, allowedTools: [...TOOLS], governedCodexProfile: profile(root), disableMermaidValidation: true });
    const engine = await agent.getEngine(); const iterator = engine.query('[EVENT][DELAY]');
    const first = iterator.next(); assert.equal(await Promise.race([first.then(() => 'released'), new Promise(resolve => setTimeout(() => resolve('buffered'), 30))]), 'buffered');
    const output = [(await first).value]; for await (const item of iterator) output.push(item);
    const state = await readState(await waitForState(stateDir, before, '[EVENT]')); const call = state.seen.find(item => item.method === 'tools/call');
    assert.deepEqual(state.seen[0], { jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2024-11-05', capabilities: { tools: {} }, clientInfo: { name: 'probe-codex-client', version: '1.0.0' } } });
    assert.equal(call.id, 2); assert.equal(call.params.name, 'codex');
    const server = Object.entries(call.params.arguments.config.mcp_servers)[0];
    assert.deepEqual(call.params.arguments, buildGovernedCodexInitialToolArgs({ profile: profile(root), prompt: call.params.arguments.prompt, mcp: { name: server[0], url: server[1].url } }));
    assert.equal(output.filter(item => item.type === 'text').length, 1); assert.match(output[0].content, /^candidate-/);
    const receipt = output.find(item => item.type === 'metadata').data.attestation; const serialized = JSON.stringify(receipt);
    for (const secret of [root, call.params.arguments.prompt, String(state.pid), 'event-candidate', process.env.PATH]) assert.equal(serialized.includes(secret), false);
    assert.equal(receipt.observed.network, 'restricted'); assert.equal(alive(state.pid), false); assert.equal(await closed(server[1].url), true);
  });

  await t.test('host rejects a non-allowlisted tool', async () => {
    const run = await runEngine(root, stateDir, '[FORBIDDEN]');
    assert.equal(run.output.find(item => item.type === 'text').content, 'host-denied');
    assert.equal(alive(run.state.pid), false); assert.equal(await closed(run.url), true);
  });

  await t.test('all bad evidence fails closed with zero candidate bytes', async () => {
    for (const marker of ['[MISSING]', '[DUPLICATE]', '[MALFORMED]', '[UNCORRELATED]', '[MISMATCHED]']) {
      const run = await runEngine(root, stateDir, marker);
      assert.equal(run.output.some(item => item.type === 'text'), false, marker);
      assert.equal(run.output.filter(item => item.type === 'error').length, 1, marker);
      assert.equal(alive(run.state.pid), false, marker); assert.equal(await closed(run.url), true, marker);
    }
  });

  await t.test('simultaneous agents keep child, session, and MCP state isolated', async () => {
    const [a, b] = await Promise.all([runEngine(root, stateDir, '[A]', true), runEngine(root, stateDir, '[B]', true)]);
    assert.notEqual(a.state.pid, b.state.pid); assert.notEqual(a.url, b.url);
    assert.notEqual(Object.keys(a.call.params.arguments.config.mcp_servers)[0], Object.keys(b.call.params.arguments.config.mcp_servers)[0]);
    const ra = a.output.find(item => item.type === 'metadata').data.attestation;
    const rb = b.output.find(item => item.type === 'metadata').data.attestation;
    assert.deepEqual(ra.requested, rb.requested); assert.equal(alive(a.state.pid) || alive(b.state.pid), false);
  });

  await t.test('cancellation and explicit close await child and listener cleanup', async () => {
    for (const action of ['abort', 'close']) {
      const before = await stateFiles(stateDir); const engine = await createCodexEngine({ agent: host(), governedCodexProfile: profile(root) });
      try {
        const controller = new AbortController(); const iterator = engine.query('[WAIT]', { abortSignal: controller.signal }); const pending = collect(iterator);
        const file = await waitForState(stateDir, before, '[WAIT]'); const state = await readState(file); const call = state.seen.find(item => item.method === 'tools/call');
        const url = Object.values(call.params.arguments.config.mcp_servers)[0].url;
        if (action === 'abort') controller.abort(); else await engine.close();
        const output = await pending; assert.equal(output.some(item => item.type === 'text'), false);
        assert.equal(alive(state.pid), false); assert.equal(await closed(url), true);
      } finally { await engine.close(); }
    }
  });

  await t.test('omitting the profile preserves the ungoverned result path', async () => {
    const before = await stateFiles(stateDir); const engine = await createCodexEngine({ agent: host(), model: 'legacy' });
    const output = await collect(engine.query('[MISSING]')); const state = await readState(await waitForState(stateDir, before, '[MISSING]'));
    assert.match(output.find(item => item.type === 'text').content, /^candidate-/); assert.equal(alive(state.pid), true);
    await engine.close(); assert.equal(alive(state.pid), false);
  });
});
