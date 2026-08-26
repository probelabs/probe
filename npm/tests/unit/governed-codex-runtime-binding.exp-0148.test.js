import test from 'node:test';
import assert from 'node:assert/strict';
import { chmod, mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { createCodexEngine } from '../../src/agent/engines/codex.js';
import { attestGovernedCodexSession, buildGovernedCodexInitialToolArgs } from '../../src/agent/engines/governed-codex-profile.js';
import { generateSchemaInstructions } from '../../src/agent/schemaUtils.js';

const TOOLS = ['search', 'extract', 'listFiles'];
const LINEAGE = { subjectId: 'SYS-REQ-048', subjectFingerprint: '636e9230b39a705f4dc1488038718281791f82f31e917296af090e9c0638fef7', role: 'spec-review', coverageKey: 'SYS-REQ-048::spec-review', findings: [{ type: 'ambiguity', severity: 'medium', message: 'timeout semantics are underspecified' }] };
const LINEAGE_SCHEMA = JSON.stringify({ type: 'object', required: ['subjectId', 'subjectFingerprint', 'role', 'coverageKey', 'findings'], additionalProperties: false, properties: { subjectId: { type: 'string' }, subjectFingerprint: { type: 'string' }, role: { type: 'string' }, coverageKey: { type: 'string' }, findings: { type: 'array', items: { type: 'object', required: ['type', 'severity', 'message'], additionalProperties: false, properties: { type: { type: 'string' }, severity: { type: 'string' }, message: { type: 'string' } } } } } });
const fakeCodex = `#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import { createInterface } from 'node:readline';
const file = process.env.PROBE_EXP0148_STATE + '/' + process.pid + '.json';
const seen = []; const save = () => writeFileSync(file, JSON.stringify({ pid: process.pid, seen }));
const send = (value) => process.stdout.write(JSON.stringify(value) + '\\n');
const configured = (requestId, args, patch = {}, identity = process.pid) => ({ jsonrpc: '2.0', method: 'codex/event', params: {
  _meta: { requestId, threadId: 'session-' + identity }, id: '', msg: {
    type: 'session_configured', session_id: 'session-' + identity, thread_id: 'session-' + identity,
    model: 'gpt-5.6-luna', model_provider_id: 'openai', approval_policy: 'never', approvals_reviewer: 'user',
    permission_profile: { type: 'managed', file_system: { type: 'restricted', entries: [{ access: 'read', path: { type: 'special', value: { kind: 'root' } } }] }, network: 'restricted' },
    reasoning_effort: 'xhigh', rollout_path: args.cwd + '/sessions/2026/08/26/rollout-2026-08-26T12-00-00-00000000-0000-4000-8000-' + String(identity).padStart(12, '0') + '.jsonl', cwd: args.cwd, ...patch
  }
}});
createInterface({ input: process.stdin }).on('line', async line => {
  const request = JSON.parse(line); seen.push(request); save();
  if (request.method === 'initialize') return send({ jsonrpc: '2.0', id: request.id, result: {} });
  const args = request.params.arguments; const prompt = args.prompt;
  if (prompt.includes('[WAIT]')) return;
  if (prompt.includes('[EVENT]')) send({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: request.id }, msg: { type: 'raw_response_item', item: { role: 'assistant', content: [{ type: 'text', text: 'event-candidate' }] } } } });
  if (prompt.includes('[DELAY]')) await new Promise(resolve => setTimeout(resolve, 80));
  if (prompt.includes('[HOLD]')) await new Promise(resolve => setTimeout(resolve, 200));
  let events = [configured(request.id, args)];
  if (prompt.includes('[MISSING]')) events = [];
  if (prompt.includes('[DUPLICATE]')) events.push(events[0]);
  if (prompt.includes('[MALFORMED]')) events = [{ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: request.id }, id: '', msg: { type: 'session_configured' } } }];
  if (prompt.includes('[UNCORRELATED]')) events = [configured(999, args)];
  if (prompt.includes('[MISMATCHED]')) events = [configured(request.id, args, { model: 'wrong' })];
  const foreign = /\\[FOREIGN:(\\d+):([^\\]]+)\\]/.exec(prompt);
  if (foreign) { const cwd = decodeURIComponent(foreign[2]); events = [configured(request.id, { ...args, cwd }, {}, Number(foreign[1]))]; }
  for (const event of events) send(event);
  let text = 'candidate-' + process.pid;
  const lineage = { subjectId: 'SYS-REQ-048', subjectFingerprint: '636e9230b39a705f4dc1488038718281791f82f31e917296af090e9c0638fef7', role: 'spec-review', coverageKey: 'SYS-REQ-048::spec-review', findings: [{ type: 'ambiguity', severity: 'medium', message: 'timeout semantics are underspecified' }] };
  if (prompt.includes('[VALID]')) text = JSON.stringify(lineage);
  if (prompt.includes('[WRONG]')) text = JSON.stringify({ subjectId: lineage.subjectId });
  if (prompt.includes('[BADJSON]')) text = '{';
  if (prompt.includes('[EXTRA]')) text = JSON.stringify({ ...lineage, extra: true });
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
function configuredEvidence(pid, requestId, cwd) {
  const identity = `session-${pid}`;
  return { jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId, threadId: identity }, id: '', msg: {
    type: 'session_configured', session_id: identity, thread_id: identity, model: 'gpt-5.6-luna', model_provider_id: 'openai', approval_policy: 'never', approvals_reviewer: 'user',
    permission_profile: { type: 'managed', file_system: { type: 'restricted', entries: [{ access: 'read', path: { type: 'special', value: { kind: 'root' } } }] }, network: 'restricted' }, reasoning_effort: 'xhigh',
    rollout_path: `${cwd}/sessions/2026/08/26/rollout-2026-08-26T12-00-00-00000000-0000-4000-8000-${String(pid).padStart(12, '0')}.jsonl`, cwd
  } } };
}
function recursiveKeys(value, found = []) {
  if (value && typeof value === 'object') for (const [key, child] of Object.entries(value)) { found.push(key); recursiveKeys(child, found); } return found;
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
function governedAgent(root) {
  return new ProbeAgent({ provider: 'codex', path: root, cwd: root, allowedTools: [...TOOLS], governedCodexProfile: profile(root), disableMermaidValidation: true });
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
async function runGoverned(root, stateDir, marker) {
  const before = await stateFiles(stateDir); const agent = governedAgent(root);
  const settled = await agent.answerGoverned(marker, { schema: LINEAGE_SCHEMA }).then(value => ({ value }), error => ({ error }));
  const state = await readState(await waitForState(stateDir, before, marker)); const call = state.seen.find(item => item.method === 'tools/call');
  return { settled, state, call, url: Object.values(call.params.arguments.config.mcp_servers)[0].url };
}

test('EXP-0148 governed Codex runtime binding', async t => {
  const root = await mkdtemp(join(tmpdir(), 'probe-exp0148-'));
  const bin = join(root, 'bin'); const stateDir = join(root, 'state');
  await mkdir(bin); await mkdir(stateDir);
  const executable = join(bin, 'codex'); await writeFile(executable, fakeCodex); await chmod(executable, 0o755);
  const originalPath = process.env.PATH; process.env.PATH = `${bin}:${originalPath}`; process.env.PROBE_EXP0148_STATE = stateDir;
  t.after(async () => { process.env.PATH = originalPath; delete process.env.PROBE_EXP0148_STATE; await rm(root, { recursive: true, force: true }); });
  let governedValid;

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
    const internal = attestGovernedCodexSession({ profile: profile(root), events: [configuredEvidence(state.pid, 2, root)] });
    assert.deepEqual(receipt, {
      version: 'probe.governed-codex-attestation/v1', profileId: 'luna-xhigh-readonly-v1',
      requested: { profileDigest: internal.requested.profileDigest, cwdDigest: internal.requested.cwdDigest, probeToolsDigest: internal.requested.probeToolsDigest, model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'read-only', approvalPolicy: 'never' },
      observed: { source: 'session_configured', model: 'gpt-5.6-luna', modelProviderId: 'openai', reasoningEffort: 'xhigh', approvalPolicy: 'never', cwdDigest: internal.observed.cwdDigest, permissionProfileDigest: internal.observed.permissionProfileDigest, filesystem: 'restricted-read-root', network: 'restricted' },
      evidence: { eventCount: 1 }, usage: { status: 'unavailable' }
    });
    assert.deepEqual(Object.keys(receipt), ['version', 'profileId', 'requested', 'observed', 'evidence', 'usage']);
    assert.deepEqual(Object.keys(receipt.requested), ['profileDigest', 'cwdDigest', 'probeToolsDigest', 'model', 'reasoningEffort', 'sandbox', 'approvalPolicy']);
    assert.deepEqual(Object.keys(receipt.observed), ['source', 'model', 'modelProviderId', 'reasoningEffort', 'approvalPolicy', 'cwdDigest', 'permissionProfileDigest', 'filesystem', 'network']);
    for (const forbidden of ['correlation', 'requestId', 'sessionId', 'threadId', 'conversationId', 'rolloutPath', 'cwd', 'prompt', 'candidate', 'environment']) assert.equal(recursiveKeys(receipt).includes(forbidden), false);
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
    const cwdA = join(root, 'cwd-a'); const cwdB = join(root, 'cwd-b'); await mkdir(cwdA); await mkdir(cwdB);
    const before = await stateFiles(stateDir); const pendingA = runEngine(cwdA, stateDir, '[A][HOLD]', true);
    const liveA = await readState(await waitForState(stateDir, before, '[A]'));
    const [a, b] = await Promise.all([pendingA, runEngine(cwdB, stateDir, `[FOREIGN:${liveA.pid}:${encodeURIComponent(cwdA)}]`, true)]);
    assert.notEqual(a.state.pid, b.state.pid); assert.notEqual(a.url, b.url);
    assert.notEqual(Object.keys(a.call.params.arguments.config.mcp_servers)[0], Object.keys(b.call.params.arguments.config.mcp_servers)[0]);
    assert.equal(a.output.filter(item => item.type === 'text').length, 1); assert.equal(a.output.some(item => item.type === 'error'), false);
    assert.equal(b.output.some(item => item.type === 'text'), false); assert.equal(b.output.filter(item => item.type === 'error').length, 1);
    assert.equal(alive(a.state.pid) || alive(b.state.pid), false); assert.equal(await closed(a.url), true); assert.equal(await closed(b.url), true);
  });

  await t.test('public cancel, cleanup, and explicit close await child and listener cleanup', async () => {
    for (const action of ['cancel', 'cleanup']) {
      const before = await stateFiles(stateDir); const released = [];
      const agent = new ProbeAgent({ provider: 'codex', path: root, cwd: root, allowedTools: [...TOOLS], governedCodexProfile: profile(root), disableMermaidValidation: true });
      const pending = agent.answer('[WAIT]', [], { onStream: text => released.push(text) }).then(value => ({ value }), error => ({ error }));
      const state = await readState(await waitForState(stateDir, before, '[WAIT]')); const call = state.seen.find(item => item.method === 'tools/call');
      const url = Object.values(call.params.arguments.config.mcp_servers)[0].url;
      if (action === 'cancel') assert.equal(agent.cancel(), undefined); else { await agent.cleanup(); assert.equal(alive(state.pid), false); assert.equal(await closed(url), true); }
      const settled = await pending; assert.ok(settled.error); assert.deepEqual(released, []);
      assert.equal(alive(state.pid), false); assert.equal(await closed(url), true);
    }
    const before = await stateFiles(stateDir); const engine = await createCodexEngine({ agent: host(), governedCodexProfile: profile(root) });
    const pending = collect(engine.query('[WAIT]')); const state = await readState(await waitForState(stateDir, before, '[WAIT]'));
    const url = Object.values(state.seen.find(item => item.method === 'tools/call').params.arguments.config.mcp_servers)[0].url;
    await engine.close(); const output = await pending; assert.equal(output.some(item => item.type === 'text'), false);
    assert.equal(alive(state.pid), false); assert.equal(await closed(url), true);
  });

  await t.test('O1 governed answer returns parsed data and the exact attestation', async () => {
    governedValid = await runGoverned(root, stateDir, '[VALID]'); assert.ifError(governedValid.settled.error);
    assert.deepEqual(Object.keys(governedValid.settled.value), ['data', 'runtimeAttestation']);
    assert.deepEqual(governedValid.settled.value.data, LINEAGE);
    const receipt = governedValid.settled.value.runtimeAttestation; const internal = attestGovernedCodexSession({ profile: profile(root), events: [configuredEvidence(governedValid.state.pid, 2, root)] });
    assert.deepEqual(receipt, { version: 'probe.governed-codex-attestation/v1', profileId: 'luna-xhigh-readonly-v1', requested: { profileDigest: internal.requested.profileDigest, cwdDigest: internal.requested.cwdDigest, probeToolsDigest: internal.requested.probeToolsDigest, model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'read-only', approvalPolicy: 'never' }, observed: { source: 'session_configured', model: 'gpt-5.6-luna', modelProviderId: 'openai', reasoningEffort: 'xhigh', approvalPolicy: 'never', cwdDigest: internal.observed.cwdDigest, permissionProfileDigest: internal.observed.permissionProfileDigest, filesystem: 'restricted-read-root', network: 'restricted' }, evidence: { eventCount: 1 }, usage: { status: 'unavailable' } });
    assert.equal(alive(governedValid.state.pid), false); assert.equal(await closed(governedValid.url), true);
  });

  await t.test('O2 preflight rejects before acquisition and wrong-shaped JSON fails closed', async () => {
    const preflight = governedAgent(root); let acquisitions = 0; preflight.getEngine = async () => { acquisitions++; throw new Error('must not acquire'); };
    for (const call of [() => preflight.answerGoverned('x'), () => preflight.answerGoverned('x', { schema: '' }), () => preflight.answerGoverned('x', { schema: 'plain text' }), () => preflight.answerGoverned('x', { schema: LINEAGE_SCHEMA }, {}), () => preflight.answerGoverned('x', { schema: LINEAGE_SCHEMA }, ['image'])]) await assert.rejects(call());
    const ungoverned = new ProbeAgent({ provider: 'codex', path: root, allowedTools: [...TOOLS] }); ungoverned.getEngine = preflight.getEngine;
    await assert.rejects(ungoverned.answerGoverned('x', { schema: LINEAGE_SCHEMA }), /governedCodexProfile/); assert.equal(acquisitions, 0);
    const run = await runGoverned(root, stateDir, '[WRONG]'); assert.match(run.settled.error.message, /Schema validation failed/);
    assert.equal(alive(run.state.pid), false); assert.equal(await closed(run.url), true);
  });

  await t.test('O3 malformed JSON fails closed', async () => {
    const run = await runGoverned(root, stateDir, '[BADJSON]'); assert.ok(run.settled.error);
    assert.equal(alive(run.state.pid), false); assert.equal(await closed(run.url), true);
  });

  await t.test('O4 forbidden extra properties fail closed', async () => {
    const run = await runGoverned(root, stateDir, '[EXTRA]'); assert.match(run.settled.error.message, /Schema validation failed/);
    assert.equal(alive(run.state.pid), false); assert.equal(await closed(run.url), true);
  });

  await t.test('O5 missing or duplicate attestation metadata fails closed', async () => {
    for (const copies of [0, 2]) {
      const agent = governedAgent(root); let closeCount = 0; let queryCount = 0;
      agent.engine = { async *query() { queryCount++; yield { type: 'text', content: JSON.stringify(LINEAGE) }; for (let i = 0; i < copies; i++) yield { type: 'metadata', data: { attestation: governedValid.settled.value.runtimeAttestation } }; }, async close() { closeCount++; } };
      await assert.rejects(agent.answerGoverned('[INJECTED]', { schema: LINEAGE_SCHEMA }), /exactly one/);
      assert.equal(queryCount, 1); assert.equal(closeCount, 1);
    }
  });

  await t.test('O6 the sole tools/call contains exactly one exact schema suffix', () => {
    const suffix = generateSchemaInstructions(LINEAGE_SCHEMA, { debug: false }); const prompt = governedValid.call.params.arguments.prompt;
    assert.equal(prompt.endsWith('[VALID]' + suffix), true); assert.equal(prompt.split(suffix).length - 1, 1);
    assert.equal(governedValid.state.seen.filter(item => item.method === 'tools/call').length, 1);
  });

  await t.test('O7 cancellation returns no result and awaits cleanup', async () => {
    const before = await stateFiles(stateDir); const agent = governedAgent(root);
    const pending = agent.answerGoverned('[WAIT]', { schema: LINEAGE_SCHEMA }).then(value => ({ value }), error => ({ error }));
    const state = await readState(await waitForState(stateDir, before, '[WAIT]')); const call = state.seen.find(item => item.method === 'tools/call'); const url = Object.values(call.params.arguments.config.mcp_servers)[0].url;
    agent.cancel(); const settled = await pending; assert.ok(settled.error); assert.equal('value' in settled, false);
    assert.equal(alive(state.pid), false); assert.equal(await closed(url), true);
  });

  await t.test('O8 answer keeps its string-returning behavior', async () => {
    const before = await stateFiles(stateDir); const agent = governedAgent(root); const value = await agent.answer('[STRING]');
    const state = await readState(await waitForState(stateDir, before, '[STRING]')); const call = state.seen.find(item => item.method === 'tools/call'); const url = Object.values(call.params.arguments.config.mcp_servers)[0].url;
    assert.equal(typeof value, 'string'); assert.match(value, /^candidate-/); assert.equal(alive(state.pid), false); assert.equal(await closed(url), true);
  });

  await t.test('O9 frozen Proof-admission lineage is preserved without interpretation', () => {
    const data = governedValid.settled.value.data;
    assert.equal(data.subjectId, 'SYS-REQ-048'); assert.equal(data.subjectFingerprint, '636e9230b39a705f4dc1488038718281791f82f31e917296af090e9c0638fef7');
    assert.equal(data.role, 'spec-review'); assert.equal(data.coverageKey, 'SYS-REQ-048::spec-review'); assert.deepEqual(data.findings, LINEAGE.findings);
    assert.equal('runtimeAttestation' in data, false);
  });

  await t.test('omitting the profile preserves the ungoverned result path', async () => {
    const before = await stateFiles(stateDir); const engine = await createCodexEngine({ agent: host(), model: 'legacy' });
    const output = await collect(engine.query('[MISSING]')); const state = await readState(await waitForState(stateDir, before, '[MISSING]'));
    assert.match(output.find(item => item.type === 'text').content, /^candidate-/); assert.equal(alive(state.pid), true);
    await engine.close(); assert.equal(alive(state.pid), false);
  });
});
