import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmod, mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { createCodexEngine } from '../../src/agent/engines/codex.js';
import { attestGovernedCodexSession, buildGovernedCodexInitialToolArgs } from '../../src/agent/engines/governed-codex-profile.js';
import { generateSchemaInstructions } from '../../src/agent/schemaUtils.js';

const TOOLS = ['search', 'extract', 'listFiles'];
const DIGEST_A = `sha256:${'a'.repeat(64)}`;
const DIGEST_B = `sha256:${'b'.repeat(64)}`;
const RESULT_IDENTITY = 'probe.governed-result-identity/v1';
const execFileAsync = promisify(execFile);
const LINEAGE = { subjectId: 'SYS-REQ-048', subjectFingerprint: '636e9230b39a705f4dc1488038718281791f82f31e917296af090e9c0638fef7', role: 'spec-review', coverageKey: 'SYS-REQ-048::spec-review', findings: [{ type: 'ambiguity', severity: 'medium', message: 'timeout semantics are underspecified' }] };
const LINEAGE_SCHEMA = JSON.stringify({ type: 'object', required: ['subjectId', 'subjectFingerprint', 'role', 'coverageKey', 'findings'], additionalProperties: false, properties: { subjectId: { type: 'string' }, subjectFingerprint: { type: 'string' }, role: { type: 'string' }, coverageKey: { type: 'string' }, findings: { type: 'array', items: { type: 'object', required: ['type', 'severity', 'message'], additionalProperties: false, properties: { type: { type: 'string' }, severity: { type: 'string' }, message: { type: 'string' } } } } } });
const fakeCodex = `#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import { createInterface } from 'node:readline';
import { spawn } from 'node:child_process';
const file = process.env.PROBE_EXP0148_STATE + '/' + process.pid + '.json';
const seen = []; const phases = []; let descendantPid = null; const save = () => writeFileSync(file, JSON.stringify({ pid: process.pid, descendantPid, seen, phases }));
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
  if (prompt.includes('[TERMINAL-WITHHELD]')) {
    process.on('SIGTERM', () => { phases.push('shutdown-observed'); save(); });
    const descendant = spawn(process.execPath, ['-e', 'process.on("SIGTERM", () => {}); setInterval(() => {}, 1000)'], { stdio: ['ignore', 1, 2] });
    descendantPid = descendant.pid; save();
    setInterval(() => {}, 1000);
    await new Promise(resolve => setTimeout(resolve, 30));
    phases.push('terminal-evidence'); save();
    send({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: request.id }, msg: { type: 'task_complete' } } });
    return;
  }
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
  if (prompt.includes('[RESPONSE-CLOSE-STALL]')) {
    process.on('SIGTERM', () => { phases.push('shutdown-observed'); save(); });
    const descendant = spawn(process.execPath, ['-e', 'process.on("SIGTERM", () => {}); setInterval(() => {}, 1000)'], { stdio: ['ignore', 1, 2] });
    descendantPid = descendant.pid; save();
    setInterval(() => {}, 1000);
    await new Promise(resolve => setTimeout(resolve, 50));
    phases.push('response-delivered'); save();
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
function dispatchFor(prompt) {
  const promptBytes = Buffer.byteLength(prompt, 'utf8'); const byteLength = Buffer.alloc(8);
  byteLength.writeBigUInt64BE(BigInt(promptBytes));
  const promptDigest = `sha256:${createHash('sha256').update('probe.governed-codex-dispatch/prompt/v1', 'utf8').update(Buffer.from([0])).update(byteLength).update(prompt, 'utf8').digest('hex')}`;
  return { source: 'probe-host-tools-call', tool: 'codex', promptDigest, promptBytes };
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
async function waitForPhase(file, phase, delay = setTimeout) {
  for (let i = 0; i < 100; i++) {
    try { const state = await readState(file); if (state.phases.includes(phase)) return state; }
    catch { /* Child may be replacing its state snapshot. */ }
    await new Promise(resolve => delay(resolve, 5));
  }
  throw new Error(`fake Codex phase timeout: ${phase}`);
}
async function collect(iterator) {
  const output = []; for await (const item of iterator) output.push(item); return output;
}
async function settleWithin(promise, timeoutMs) {
  let timer;
  try { return await Promise.race([promise, new Promise(resolve => { timer = setTimeout(() => resolve({ timeout: true }), timeoutMs); })]); }
  finally { clearTimeout(timer); }
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
async function runGoverned(root, stateDir, marker, options = { schema: LINEAGE_SCHEMA }) {
  const before = await stateFiles(stateDir); const agent = governedAgent(root);
  const settled = await agent.answerGoverned(marker, options).then(value => ({ value }), error => ({ error }));
  const state = await readState(await waitForState(stateDir, before, marker)); const call = state.seen.find(item => item.method === 'tools/call');
  return { settled, state, call, url: Object.values(call.params.arguments.config.mcp_servers)[0].url };
}
function installIdentifiedEngine(agent, receipt, candidate) {
  const state = { queries: 0, closes: 0, prompts: [], options: [] };
  agent.engine = { async *query(prompt, options) { state.queries++; state.prompts.push(prompt); state.options.push(options); yield { type: 'text', content: candidate }; yield { type: 'metadata', data: { attestation: structuredClone(receipt) } }; }, async close() { state.closes++; } };
  return state;
}
function expectedResultIdentity(canonical) {
  const bytes = Buffer.from(canonical, 'utf8'); const byteLength = Buffer.alloc(8); byteLength.writeBigUInt64BE(BigInt(bytes.length));
  return { version: RESULT_IDENTITY, source: 'probe-host-schema-valid-json', resultDigest: `sha256:${createHash('sha256').update('probe.governed-result-identity/data/v1', 'utf8').update(Buffer.from([0])).update(byteLength).update(bytes).digest('hex')}`, canonicalBytes: bytes.length };
}
function assertDeepFrozen(value) {
  if (!value || typeof value !== 'object') return; assert.equal(Object.isFrozen(value), true); for (const child of Object.values(value)) assertDeepFrozen(child);
}
function assertFailureStage(error, answerFailureStage) {
  assert.equal(error?.name, 'GovernedAnswerFailure'); assert.equal(error?.message, '');
  assert.equal(error?.answerFailureStage, answerFailureStage); assert.equal(error?.stack, undefined); assert.equal(Object.hasOwn(error ?? {}, 'cause'), false);
}

test('EXP-0148 governed Codex runtime binding', async t => {
  const root = await mkdtemp(join(tmpdir(), 'probe-exp0148-'));
  const bin = join(root, 'bin'); const stateDir = join(root, 'state');
  await mkdir(bin); await mkdir(stateDir);
  const executable = join(bin, 'codex'); await writeFile(executable, fakeCodex); await chmod(executable, 0o755);
  const originalPath = process.env.PATH; process.env.PATH = `${bin}:${originalPath}`; process.env.PROBE_EXP0148_STATE = stateDir;
  t.after(async () => { process.env.PATH = originalPath; delete process.env.PROBE_EXP0148_STATE; await rm(root, { recursive: true, force: true }); });
  let governedValid; let boundValid;

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
    const run = await runGoverned(root, stateDir, '[WRONG]'); assertFailureStage(run.settled.error, 'schema_result_validation');
    assert.equal(alive(run.state.pid), false); assert.equal(await closed(run.url), true);
  });

  await t.test('O3 malformed JSON fails closed', async () => {
    const run = await runGoverned(root, stateDir, '[BADJSON]'); assert.ok(run.settled.error);
    assert.equal(alive(run.state.pid), false); assert.equal(await closed(run.url), true);
  });

  await t.test('O4 forbidden extra properties fail closed', async () => {
    const run = await runGoverned(root, stateDir, '[EXTRA]'); assertFailureStage(run.settled.error, 'schema_result_validation');
    assert.equal(alive(run.state.pid), false); assert.equal(await closed(run.url), true);
  });

  await t.test('O5 missing or duplicate attestation metadata fails closed', async () => {
    for (const copies of [0, 2]) {
      const agent = governedAgent(root); let closeCount = 0; let queryCount = 0;
      agent.engine = { async *query() { queryCount++; yield { type: 'text', content: JSON.stringify(LINEAGE) }; for (let i = 0; i < copies; i++) yield { type: 'metadata', data: { attestation: governedValid.settled.value.runtimeAttestation } }; }, async close() { closeCount++; } };
      const error = await agent.answerGoverned('[INJECTED]', { schema: LINEAGE_SCHEMA }).then(() => null, caught => caught); assertFailureStage(error, 'unknown');
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

  await t.test('EXP-0171 terminal evidence with a withheld tools/call response times out and leaves no lifecycle survivor', async () => {
    const before = await stateFiles(stateDir); const agent = governedAgent(root); let state; let pending; let url; let resolutions = 0; let rejections = 0;
    const signal = agent._abortController.signal; let abortListeners = 0;
    const add = signal.addEventListener.bind(signal); const remove = signal.removeEventListener.bind(signal);
    signal.addEventListener = (...args) => { if (args[0] === 'abort') abortListeners++; return add(...args); };
    signal.removeEventListener = (...args) => { if (args[0] === 'abort') abortListeners--; return remove(...args); };
    const originalSetTimeout = globalThis.setTimeout; const requestTimers = []; const lifecycleTimers = [];
    globalThis.setTimeout = (callback, delay, ...args) => {
      const mappedDelay = delay === 600000 ? 80 : delay === 5000 ? 30 : delay === 10000 ? 500 : delay;
      const timer = originalSetTimeout(callback, mappedDelay, ...args);
      if (delay === 600000) requestTimers.push(timer); else if (delay === 5000 || delay === 10000) lifecycleTimers.push(timer);
      return timer;
    };
    try {
      pending = agent.answerGoverned('[TERMINAL-WITHHELD]', { schema: LINEAGE_SCHEMA }).then(
        value => { resolutions++; return { value }; },
        error => { rejections++; return { error }; }
      );
      const file = await waitForState(stateDir, before, '[TERMINAL-WITHHELD]');
      state = await readState(file); const call = state.seen.find(item => item.method === 'tools/call'); url = Object.values(call.params.arguments.config.mcp_servers)[0].url;
      state = await waitForPhase(file, 'terminal-evidence', originalSetTimeout);
      assert.deepEqual(state.phases, ['terminal-evidence']);
      const settled = await settleWithin(pending, 500);
      state = await waitForPhase(file, 'shutdown-observed', originalSetTimeout);
      assert.equal(settled.timeout, undefined); assertFailureStage(settled.error, 'provider_engine'); assert.equal('value' in settled, false);
      assert.deepEqual(Object.keys(settled), ['error']); assert.deepEqual([resolutions, rejections], [0, 1]);
      assert.deepEqual(state.phases, ['terminal-evidence', 'shutdown-observed']); assert.equal(state.phases.includes('response-delivered'), false);
      assert.equal(abortListeners, 0); assert.equal(requestTimers.length, 2); assert.equal(requestTimers.every(timer => timer._destroyed), true);
      assert.equal(lifecycleTimers.length, 2); assert.equal(lifecycleTimers.every(timer => timer._destroyed), true);
      assert.equal(alive(state.pid), false); assert.equal(alive(state.descendantPid), false); assert.equal(await closed(url), true);
    } finally {
      globalThis.setTimeout = originalSetTimeout;
      if (state?.pid && alive(state.pid)) {
        try { process.kill(-state.pid, 'SIGKILL'); } catch { process.kill(state.pid, 'SIGKILL'); }
        if (pending) await settleWithin(pending, 500);
      }
    }
  });

  await t.test('EXP-0171 delivered response localizes a TERM-resistant stall to bounded cleanup with no survivor', async () => {
    const before = await stateFiles(stateDir); const agent = governedAgent(root); let state; let url; let settled;
    const originalSetTimeout = globalThis.setTimeout; const lifecycleTimers = [];
    globalThis.setTimeout = (callback, delay, ...args) => {
      const timer = originalSetTimeout(callback, delay === 5000 ? 30 : delay === 10000 ? 500 : delay, ...args);
      if (delay === 5000 || delay === 10000) lifecycleTimers.push(timer); return timer;
    };
    const pending = agent.answerGoverned('[VALID][RESPONSE-CLOSE-STALL]', { schema: LINEAGE_SCHEMA }).then(value => ({ value }), error => ({ error }));
    try {
      const file = await waitForState(stateDir, before, '[RESPONSE-CLOSE-STALL]'); state = await readState(file);
      const call = state.seen.find(item => item.method === 'tools/call'); url = Object.values(call.params.arguments.config.mcp_servers)[0].url;
      state = await waitForPhase(file, 'response-delivered');
      assert.equal(state.phases[0], 'response-delivered');
      state = await waitForPhase(file, 'shutdown-observed');
      assert.deepEqual(state.phases, ['response-delivered', 'shutdown-observed']);
      settled = await settleWithin(pending, 500);
    } finally {
      globalThis.setTimeout = originalSetTimeout;
      if (state?.pid && alive(state.pid)) {
        try { process.kill(-state.pid, 'SIGKILL'); } catch { process.kill(state.pid, 'SIGKILL'); }
        await settleWithin(pending, 500);
      }
    }
    assert.equal(settled.timeout, undefined); assert.ifError(settled.error); assert.deepEqual(settled.value.data, LINEAGE);
    assert.equal(lifecycleTimers.length, 2); assert.equal(lifecycleTimers.every(timer => timer._destroyed), true);
    assert.equal(alive(state.pid), false); assert.equal(alive(state.descendantPid), false); assert.equal(await closed(url), true);
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

  await t.test('EXP-0151 O01 omitted digest preserves exact v1 receipt and result', async () => {
    const run = await runGoverned(root, stateDir, '[VALID][UNBOUND-V1]'); assert.ifError(run.settled.error);
    assert.deepEqual(run.settled.value.runtimeAttestation, governedValid.settled.value.runtimeAttestation);
    assert.deepEqual(Object.keys(run.settled.value), ['data', 'runtimeAttestation']);
    const inherited = Object.assign(Object.create({ invocationDigest: DIGEST_A }), { schema: LINEAGE_SCHEMA });
    const inheritedRun = await runGoverned(root, stateDir, '[VALID][INHERITED-V1]', inherited); assert.ifError(inheritedRun.settled.error);
    assert.equal(inheritedRun.settled.value.runtimeAttestation.version, 'probe.governed-codex-attestation/v1');
  });

  await t.test('EXP-0151 O02 bound digest returns the exact distinct v2 projection', async () => {
    boundValid = await runGoverned(root, stateDir, '[VALID][BOUND]', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A }); assert.ifError(boundValid.settled.error);
    const receipt = boundValid.settled.value.runtimeAttestation;
    assert.deepEqual(Object.keys(boundValid.settled.value), ['data', 'runtimeAttestation']); assert.deepEqual(boundValid.settled.value.data, LINEAGE);
    const server = Object.entries(boundValid.call.params.arguments.config.mcp_servers)[0]; assert.deepEqual(boundValid.call.params.arguments, buildGovernedCodexInitialToolArgs({ profile: profile(root), prompt: boundValid.call.params.arguments.prompt, mcp: { name: server[0], url: server[1].url } }));
    assert.deepEqual(Object.keys(receipt), ['version', 'profileId', 'requested', 'observed', 'executionContext', 'dispatch', 'evidence', 'usage']);
    assert.equal(receipt.version, 'probe.governed-codex-attestation/v2'); assert.equal(receipt.profileId, 'luna-xhigh-readonly-v1');
    assert.deepEqual(receipt.requested, governedValid.settled.value.runtimeAttestation.requested); assert.deepEqual(receipt.observed, governedValid.settled.value.runtimeAttestation.observed);
    assert.deepEqual(receipt.executionContext, { source: 'caller', invocationDigest: DIGEST_A });
    assert.deepEqual(Object.keys(receipt.dispatch), ['source', 'tool', 'promptDigest', 'promptBytes']);
    assert.deepEqual(receipt.evidence, { eventCount: 1 }); assert.deepEqual(receipt.usage, { status: 'unavailable' });
  });

  await t.test('EXP-0151 O03 independently recomputes prompt digest and byte length', () => {
    assert.deepEqual(boundValid.settled.value.runtimeAttestation.dispatch, dispatchFor(boundValid.call.params.arguments.prompt));
  });

  await t.test('EXP-0151 O04 digest is host-only and absent from prompt, tool args, and candidate', () => {
    assert.equal(JSON.stringify(boundValid.call.params.arguments).includes(DIGEST_A), false);
    assert.equal(boundValid.call.params.arguments.prompt.includes(DIGEST_A), false);
    assert.equal(JSON.stringify(boundValid.settled.value.data).includes(DIGEST_A), false);
  });

  await t.test('EXP-0151 O05 changing only caller digest changes only execution context', async () => {
    const run = await runGoverned(root, stateDir, '[VALID][BOUND]', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_B }); assert.ifError(run.settled.error);
    const a = structuredClone(boundValid.settled.value.runtimeAttestation); const b = structuredClone(run.settled.value.runtimeAttestation);
    assert.equal(a.executionContext.invocationDigest, DIGEST_A); assert.equal(b.executionContext.invocationDigest, DIGEST_B);
    delete a.executionContext; delete b.executionContext; assert.deepEqual(a, b);
    assert.equal(boundValid.call.params.arguments.prompt, run.call.params.arguments.prompt);
  });

  await t.test('EXP-0151 O06 changing prompt bytes changes dispatch under the same digest', async () => {
    const run = await runGoverned(root, stateDir, '[VALID][BOUND-CHANGED]', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A }); assert.ifError(run.settled.error);
    assert.notEqual(run.call.params.arguments.prompt, boundValid.call.params.arguments.prompt);
    assert.notEqual(run.settled.value.runtimeAttestation.dispatch.promptDigest, boundValid.settled.value.runtimeAttestation.dispatch.promptDigest);
    assert.equal(run.settled.value.runtimeAttestation.executionContext.invocationDigest, DIGEST_A);
  });

  await t.test('EXP-0151 O07 invalid digests fail before acquisition and direct tools call', async () => {
    const invalid = ['a'.repeat(64), `sha256:${'A'.repeat(64)}`, ` ${DIGEST_A}`, `${DIGEST_A}\n`, 'sha256:ab', 7, null];
    const agent = governedAgent(root); let acquisitions = 0; agent.getEngine = async () => { acquisitions++; throw new Error('must not acquire'); };
    for (const invocationDigest of invalid) await assert.rejects(agent.answerGoverned('x', { schema: LINEAGE_SCHEMA, invocationDigest }), { name: 'TypeError', message: 'answerGoverned invocationDigest must match sha256:<64 lowercase hexadecimal digits>' });
    assert.equal(acquisitions, 0);
    const before = await stateFiles(stateDir); const engine = await createCodexEngine({ agent: host(), governedCodexProfile: profile(root) });
    const output = await collect(engine.query('[DIRECT-INVALID]', { invocationDigest: 'bad' })); const state = await readState(await waitForState(stateDir, before));
    assert.equal(output.filter(item => item.type === 'error').length, 1); assert.equal(state.seen.some(item => item.method === 'tools/call'), false); assert.equal(alive(state.pid), false);
  });

  await t.test('EXP-0151 O08 unmatched v2 metadata fails closed', async () => {
    const receipt = boundValid.settled.value.runtimeAttestation;
    const cases = [[], [receipt, receipt], [{ ...receipt, version: 'probe.governed-codex-attestation/v1' }], [{ ...receipt, executionContext: { source: 'caller', invocationDigest: DIGEST_B } }]];
    for (const receipts of cases) {
      const agent = governedAgent(root); let closeCount = 0;
      agent.engine = { async *query() { yield { type: 'text', content: JSON.stringify(LINEAGE) }; for (const attestation of receipts) yield { type: 'metadata', data: { attestation } }; }, async close() { closeCount++; } };
      const error = await agent.answerGoverned('[INJECTED-V2]', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A }).then(() => null, caught => caught); assertFailureStage(error, 'unknown'); assert.equal(closeCount, 1);
    }
  });

  await t.test('EXP-0151 O09 simultaneous siblings retain their own binding', async () => {
    const [a, b] = await Promise.all([
      runGoverned(root, stateDir, '[VALID][HOLD][BOUND-SIBLING-A]', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A }),
      runGoverned(root, stateDir, '[VALID][BOUND-SIBLING-B]', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_B })
    ]);
    assert.ifError(a.settled.error); assert.ifError(b.settled.error); assert.notEqual(a.state.pid, b.state.pid);
    assert.equal(a.settled.value.runtimeAttestation.executionContext.invocationDigest, DIGEST_A); assert.equal(b.settled.value.runtimeAttestation.executionContext.invocationDigest, DIGEST_B);
    assert.notEqual(a.url, b.url); assert.equal(alive(a.state.pid) || alive(b.state.pid), false);
  });

  await t.test('EXP-0151 O10 bound call has one schema suffix, query, and tools call', () => {
    const suffix = generateSchemaInstructions(LINEAGE_SCHEMA, { debug: false }); const prompt = boundValid.call.params.arguments.prompt;
    assert.equal(prompt.split(suffix).length - 1, 1); assert.equal(boundValid.state.seen.filter(item => item.method === 'tools/call').length, 1);
    const agent = governedAgent(root); let queryCount = 0;
    agent.engine = { async *query() { queryCount++; yield { type: 'text', content: JSON.stringify(LINEAGE) }; yield { type: 'metadata', data: { attestation: boundValid.settled.value.runtimeAttestation } }; }, async close() {} };
    return agent.answerGoverned('[QUERY-COUNT]', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A }).then(() => assert.equal(queryCount, 1));
  });

  await t.test('EXP-0151 O11 bound projection recursively redacts host and model bytes', () => {
    const receipt = boundValid.settled.value.runtimeAttestation; const serialized = JSON.stringify(receipt);
    for (const forbidden of ['prompt', 'schema', 'candidate', 'environment', 'credentials', 'correlation', 'requestId', 'sessionId', 'threadId', 'conversationId', 'cwd', 'path']) assert.equal(recursiveKeys(receipt).includes(forbidden), false, forbidden);
    for (const secret of [root, boundValid.call.params.arguments.prompt, LINEAGE_SCHEMA, JSON.stringify(LINEAGE), process.env.PATH]) assert.equal(serialized.includes(secret), false);
  });

  await t.test('EXP-0151 O12 inherited governed contracts remain live', () => {
    assert.equal(governedValid.settled.value.runtimeAttestation.version, 'probe.governed-codex-attestation/v1');
    assert.deepEqual(boundValid.settled.value.data, LINEAGE); assert.equal(alive(boundValid.state.pid), false);
  });

  await t.test('EXP-0151 O13 bound cancellation emits no admissible result and cleans up', async () => {
    const before = await stateFiles(stateDir); const agent = governedAgent(root);
    const pending = agent.answerGoverned('[WAIT][BOUND-CANCEL]', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A }).then(value => ({ value }), error => ({ error }));
    const state = await readState(await waitForState(stateDir, before, '[BOUND-CANCEL]')); const call = state.seen.find(item => item.method === 'tools/call'); const url = Object.values(call.params.arguments.config.mcp_servers)[0].url;
    agent.cancel(); const settled = await pending; assert.ok(settled.error); assert.equal('value' in settled, false); assert.equal(alive(state.pid), false); assert.equal(await closed(url), true);
  });

  await t.test('EXP-0151 O14 mirrors match and regular answer bytes remain frozen', async () => {
    const sourceDeclaration = await readFile(new URL('../../src/agent/ProbeAgent.d.ts', import.meta.url), 'utf8'); const packageDeclaration = await readFile(new URL('../../index.d.ts', import.meta.url), 'utf8');
    const contract = value => value.slice(value.indexOf('export interface GovernedAnswerOptions'), value.indexOf('/**\n * Clone options', value.indexOf('export interface GovernedAnswerOptions')));
    assert.equal(contract(sourceDeclaration), contract(packageDeclaration));
    for (const declaration of [sourceDeclaration, packageDeclaration]) assert.equal(declaration.includes('answer(message: string, images?: any[], options?: AnswerOptions): Promise<string>;'), true);
    const source = await readFile(new URL('../../src/agent/ProbeAgent.js', import.meta.url), 'utf8'); const answer = source.slice(source.indexOf('  async answer(message'), source.indexOf('  /**\n   * Get token usage information', source.indexOf('  async answer(message')));
    assert.equal(createHash('sha256').update(answer).digest('hex'), '53ee9f207963f5b991aaf89e143211039ad632d9cc7535d91af66bcae95b135f');
  });

  await t.test('EXP-0151 O15 changing own getters are read once at each boundary', async () => {
    let agentReads = 0; const options = { schema: LINEAGE_SCHEMA };
    Object.defineProperty(options, 'invocationDigest', { enumerable: true, get() { agentReads++; return agentReads === 1 ? DIGEST_A : DIGEST_B; } });
    const agent = governedAgent(root); let transported;
    agent.engine = { async *query(_prompt, opts) { transported = opts.invocationDigest; yield { type: 'text', content: JSON.stringify(LINEAGE) }; yield { type: 'metadata', data: { attestation: boundValid.settled.value.runtimeAttestation } }; }, async close() {} };
    const answer = await agent.answerGoverned('[GETTER-AGENT]', options); assert.equal(agentReads, 1); assert.equal(transported, DIGEST_A); assert.equal(answer.runtimeAttestation.executionContext.invocationDigest, DIGEST_A);
    const before = await stateFiles(stateDir); const engine = await createCodexEngine({ agent: host(), governedCodexProfile: profile(root) }); let engineReads = 0; const engineOptions = {};
    Object.defineProperty(engineOptions, 'invocationDigest', { enumerable: true, get() { engineReads++; return engineReads === 1 ? DIGEST_A : DIGEST_B; } });
    const output = await collect(engine.query('[VALID][GETTER-ENGINE]', engineOptions)); const state = await readState(await waitForState(stateDir, before, '[GETTER-ENGINE]'));
    assert.equal(engineReads, 1); assert.equal(output.find(item => item.type === 'metadata').data.attestation.executionContext.invocationDigest, DIGEST_A); assert.equal(alive(state.pid), false);
  });

  await t.test('EXP-0151 O16 caller mutation after start cannot alter snapshot', async () => {
    const before = await stateFiles(stateDir); const agent = governedAgent(root); const options = { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A };
    const pending = agent.answerGoverned('[VALID][DELAY][MUTATION]', options).then(value => ({ value }), error => ({ error })); options.invocationDigest = DIGEST_B;
    const settled = await pending; assert.ifError(settled.error); const state = await readState(await waitForState(stateDir, before, '[MUTATION]')); const call = state.seen.find(item => item.method === 'tools/call');
    assert.equal(settled.value.runtimeAttestation.executionContext.invocationDigest, DIGEST_A);
    assert.equal(JSON.stringify(call.params.arguments).includes(DIGEST_A) || JSON.stringify(call.params.arguments).includes(DIGEST_B), false);
  });

  await t.test('omitting the profile preserves the ungoverned result path', async () => {
    const before = await stateFiles(stateDir); const engine = await createCodexEngine({ agent: host(), model: 'legacy' });
    const output = await collect(engine.query('[MISSING]')); const state = await readState(await waitForState(stateDir, before, '[MISSING]'));
    assert.match(output.find(item => item.type === 'text').content, /^candidate-/); assert.equal(alive(state.pid), true);
    await engine.close(); assert.equal(alive(state.pid), false);
  });

  let identifiedValid;
  await t.test('EXP-0152 T01 omitted and inherited selector preserve v1 and v2 lanes', async () => {
    const v1 = await runGoverned(root, stateDir, '[VALID][EXP0152-V1]'); const v2 = await runGoverned(root, stateDir, '[VALID][EXP0152-V2]', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A });
    assert.deepEqual(Object.keys(v1.settled.value), ['data', 'runtimeAttestation']); assert.deepEqual(Object.keys(v2.settled.value), ['data', 'runtimeAttestation']); assert.equal(Object.isFrozen(v1.settled.value), false); assert.equal(Object.isFrozen(v2.settled.value), false);
    const inherited = Object.assign(Object.create({ resultIdentity: RESULT_IDENTITY }), { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A }); const inheritedRun = await runGoverned(root, stateDir, '[VALID][EXP0152-INHERITED]', inherited);
    assert.ifError(inheritedRun.settled.error); assert.deepEqual(Object.keys(inheritedRun.settled.value), ['data', 'runtimeAttestation']); assert.equal(inheritedRun.settled.value.runtimeAttestation.version, 'probe.governed-codex-attestation/v2');
  });

  await t.test('EXP-0152 T02 public types and overload returns compile with expected errors', async () => {
    const npmRoot = fileURLToPath(new URL('../..', import.meta.url));
    const typescriptBin = join(npmRoot, 'node_modules', 'typescript', 'bin', 'tsc');
    const fixtureDir = await mkdtemp(join(npmRoot, '.exp-0152-types-'));
    const fixturePath = join(fixtureDir, 'overloads.mts');
    const fixtureSource = `import type ProbeAgent from '../index.js';
import type { GovernedAnswerResult, GovernedIdentifiedAnswerResult, GovernedInvocationAnswerResult } from '../index.js';
type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
  (<T>() => T extends B ? 1 : 2) ? true : false;
type Expect<T extends true> = T;
declare const agent: ProbeAgent;
declare const schema: string;
declare const invocationDigest: string;
const identified = agent.answerGoverned('x', { schema, invocationDigest, resultIdentity: 'probe.governed-result-identity/v1' });
const bound = agent.answerGoverned('x', { schema, invocationDigest });
const unbound = agent.answerGoverned('x', { schema });
type Identified = Expect<Equal<typeof identified, Promise<GovernedIdentifiedAnswerResult>>>;
type Bound = Expect<Equal<typeof bound, Promise<GovernedInvocationAnswerResult>>>;
type Unbound = Expect<Equal<typeof unbound, Promise<GovernedAnswerResult>>>;
// @ts-expect-error wrong resultIdentity literal
agent.answerGoverned('x', { schema, invocationDigest, resultIdentity: 'wrong' });
// @ts-expect-error resultIdentity requires invocationDigest
agent.answerGoverned('x', { schema, resultIdentity: 'probe.governed-result-identity/v1' });`;
    try {
      await writeFile(fixturePath, fixtureSource, 'utf8');
      await execFileAsync(process.execPath,[typescriptBin,'--noEmit','--strict','--skipLibCheck','--target','ES2022','--module','NodeNext','--moduleResolution','NodeNext','--types','node',fixturePath],{cwd:npmRoot});
    } finally {
      await rm(fixtureDir, { recursive: true, force: true });
    }
  });

  await t.test('EXP-0152 T03 selector errors are exact, ordered, and pre-acquisition', async () => {
    const agent = governedAgent(root); let acquisitions = 0; agent.getEngine = async () => { acquisitions++; throw new Error('must not acquire'); };
    await assert.rejects(agent.answerGoverned('x', { schema: LINEAGE_SCHEMA, invocationDigest: 'bad', resultIdentity: 'wrong' }), { name: 'TypeError', message: 'answerGoverned invocationDigest must match sha256:<64 lowercase hexadecimal digits>' });
    await assert.rejects(agent.answerGoverned('x', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A, resultIdentity: 'wrong' }), { name: 'TypeError', message: 'answerGoverned resultIdentity must equal probe.governed-result-identity/v1' });
    await assert.rejects(agent.answerGoverned('x', { schema: LINEAGE_SCHEMA, resultIdentity: RESULT_IDENTITY }), { name: 'TypeError', message: 'answerGoverned resultIdentity requires an own invocationDigest' }); assert.equal(acquisitions, 0);
  });

  await t.test('EXP-0152 T04 schema and optional selectors are one-read snapshots before await', async () => {
    const trace = []; let schemaReads = 0; let invocationReads = 0; let resultReads = 0; const options = {};
    Object.defineProperty(options, 'schema', { enumerable: true, get() { trace.push('schema'); schemaReads++; return schemaReads === 1 ? LINEAGE_SCHEMA : JSON.stringify({ type: 'string' }); } });
    Object.defineProperty(options, 'invocationDigest', { enumerable: true, get() { trace.push('invocation'); invocationReads++; return invocationReads === 1 ? DIGEST_A : DIGEST_B; } });
    Object.defineProperty(options, 'resultIdentity', { enumerable: true, get() { trace.push('result'); resultReads++; return resultReads === 1 ? RESULT_IDENTITY : 'wrong'; } });
    const agent = governedAgent(root); const state = installIdentifiedEngine(agent, boundValid.settled.value.runtimeAttestation, JSON.stringify(LINEAGE)); const value = await agent.answerGoverned('[EXP0152-GETTERS]', options);
    assert.deepEqual(trace, ['schema', 'invocation', 'result']); assert.deepEqual([schemaReads, invocationReads, resultReads], [1, 1, 1]); assert.equal(state.prompts[0].endsWith(generateSchemaInstructions(LINEAGE_SCHEMA, { debug: false })), true); assert.deepEqual(value.data, LINEAGE);
    const mutable = { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A, resultIdentity: RESULT_IDENTITY }; const delayed = governedAgent(root); installIdentifiedEngine(delayed, boundValid.settled.value.runtimeAttestation, JSON.stringify(LINEAGE)); const pending = delayed.answerGoverned('[EXP0152-MUTATE]', mutable); mutable.schema = JSON.stringify({ type: 'string' }); assert.deepEqual((await pending).data, LINEAGE);
  });

  await t.test('EXP-0152 T05 independently recomputes canonical bytes and framed digest', async () => {
    const agent = governedAgent(root); const candidate = '{"findings":[{"message":"timeout semantics are underspecified","severity":"medium","type":"ambiguity"}],"coverageKey":"SYS-REQ-048::spec-review","role":"spec-review","subjectFingerprint":"636e9230b39a705f4dc1488038718281791f82f31e917296af090e9c0638fef7","subjectId":"SYS-REQ-048"}';
    installIdentifiedEngine(agent, boundValid.settled.value.runtimeAttestation, candidate); identifiedValid = await agent.answerGoverned('[EXP0152-IDENTIFIED]', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A, resultIdentity: RESULT_IDENTITY });
    const canonical = JSON.stringify({ coverageKey: LINEAGE.coverageKey, findings: [{ message: LINEAGE.findings[0].message, severity: 'medium', type: 'ambiguity' }], role: 'spec-review', subjectFingerprint: LINEAGE.subjectFingerprint, subjectId: 'SYS-REQ-048' });
    assert.deepEqual(identifiedValid.resultIdentity, expectedResultIdentity(canonical)); assert.deepEqual(Object.keys(identifiedValid), ['data', 'runtimeAttestation', 'resultIdentity']);
  });

  await t.test('EXP-0152 T06 equivalence and honest mixed-key ordering include own __proto__', async () => {
    const mixedKeys=['10','2','01','4294967294','4294967295','é','😀','__proto__'];
    const schema=JSON.stringify({type:'object',required:mixedKeys,additionalProperties:false,properties:Object.fromEntries(mixedKeys.map(key=>[key,{type:'string'}])),patternProperties:{'^__proto__$':{type:'string'}}}); const raw = '{"10":"ten","2":"two","01":"leading","4294967294":"max","4294967295":"overflow","é":"accent","😀":"emoji","__proto__":"proto"}';
    const agent = governedAgent(root); installIdentifiedEngine(agent, boundValid.settled.value.runtimeAttestation, raw); const mixed = await agent.answerGoverned('mixed', { schema, invocationDigest: DIGEST_A, resultIdentity: RESULT_IDENTITY });
    const expected = '{"2":"two","10":"ten","4294967294":"max","01":"leading","4294967295":"overflow","__proto__":"proto","é":"accent","😀":"emoji"}';
    assert.deepEqual(Object.keys(mixed.data), ['2', '10', '4294967294', '01', '4294967295', '__proto__', 'é', '😀']); assert.equal(Object.prototype.hasOwnProperty.call(mixed.data, '__proto__'), true); assert.deepEqual(mixed.resultIdentity, expectedResultIdentity(expected));
    const equivalent = governedAgent(root); installIdentifiedEngine(equivalent, boundValid.settled.value.runtimeAttestation, '{"😀":"emoji","é":"accent","__proto__":"proto","4294967295":"overflow","4294967294":"max","01":"leading","2":"two","10":"ten"}'); const reordered = await equivalent.answerGoverned('mixed-2', { schema, invocationDigest: DIGEST_A, resultIdentity: RESULT_IDENTITY }); assert.equal(reordered.resultIdentity.resultDigest, mixed.resultIdentity.resultDigest);
    for (const candidate of ['-0', '0']) { const numberAgent = governedAgent(root); installIdentifiedEngine(numberAgent, boundValid.settled.value.runtimeAttestation, candidate); const result = await numberAgent.answerGoverned('number', { schema: JSON.stringify({ type: 'number' }), invocationDigest: DIGEST_A, resultIdentity: RESULT_IDENTITY }); assert.equal(result.resultIdentity.resultDigest, expectedResultIdentity('0').resultDigest); }
  });

  await t.test('EXP-0152 T07 semantic content and array order change identity', async () => {
    const answer = async candidate => { const agent = governedAgent(root); installIdentifiedEngine(agent, boundValid.settled.value.runtimeAttestation, candidate); return agent.answerGoverned('array', { schema: '{}', invocationDigest: DIGEST_A, resultIdentity: RESULT_IDENTITY }); };
    const [a, b, c] = await Promise.all([answer('[1,2]'), answer('[2,1]'), answer('[1,3]')]); assert.notEqual(a.resultIdentity.resultDigest, b.resultIdentity.resultDigest); assert.notEqual(a.resultIdentity.resultDigest, c.resultIdentity.resultDigest);
  });

  await t.test('EXP-0152 T08 identity is content-only across invocation bindings', async () => {
    const receiptB = structuredClone(boundValid.settled.value.runtimeAttestation); receiptB.executionContext.invocationDigest = DIGEST_B;
    const run = async (digest, receipt) => { const agent = governedAgent(root); installIdentifiedEngine(agent, receipt, JSON.stringify(LINEAGE)); return agent.answerGoverned('same', { schema: LINEAGE_SCHEMA, invocationDigest: digest, resultIdentity: RESULT_IDENTITY }); };
    const [a, b] = await Promise.all([run(DIGEST_A, boundValid.settled.value.runtimeAttestation), run(DIGEST_B, receiptB)]); assert.equal(a.resultIdentity.resultDigest, b.resultIdentity.resultDigest); assert.notEqual(a.runtimeAttestation.executionContext.invocationDigest, b.runtimeAttestation.executionContext.invocationDigest);
  });

  await t.test('EXP-0152 T09 complete identified graph is deeply frozen', () => {
    assertDeepFrozen(identifiedValid); assertDeepFrozen(identifiedValid.data); assertDeepFrozen(identifiedValid.resultIdentity); assertDeepFrozen(identifiedValid.runtimeAttestation);
  });

  await t.test('EXP-0152 T10 normalized data is a new host snapshot', async () => {
    const candidate='{"z":[{"b":2,"a":1}]}';
    const originalJSONParse=JSON.parse;
    const captures=[];
    JSON.parse=function (...args) {
      const parsed=Reflect.apply(originalJSONParse,this,args);
      if (args[0]===candidate && parsed!==null && typeof parsed==='object') captures.push(parsed);
      return parsed;
    };
    let result;
    try {
      const agent = governedAgent(root); installIdentifiedEngine(agent, boundValid.settled.value.runtimeAttestation, candidate); result = await agent.answerGoverned('copy', { schema: '{}', invocationDigest: DIGEST_A, resultIdentity: RESULT_IDENTITY });
    } finally {
      JSON.parse=originalJSONParse;
    }
    assert.equal(captures.length, 2); const validationParsed=captures.at(-1);
    assert.notEqual(result.data, validationParsed); assert.notEqual(result.data.z, validationParsed.z); assert.notEqual(result.data.z[0], validationParsed.z[0]);
    assert.deepEqual(result.data, { z: [{ a: 1, b: 2 }] }); assert.equal(result.data.z[0].a, 1); assert.equal(result.data.z[0].b, 2); assertDeepFrozen(result.data);
  });

  await t.test('EXP-0152 T11 receipt and schema failures emit no identity and clean once', async () => {
    const mismatch = governedAgent(root); const wrong = structuredClone(boundValid.settled.value.runtimeAttestation); wrong.executionContext.invocationDigest = DIGEST_B; const mismatchState = installIdentifiedEngine(mismatch, wrong, '{');
    const mismatchSettled = await mismatch.answerGoverned('mismatch', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A, resultIdentity: RESULT_IDENTITY }).then(value => ({ value }), error => ({ error })); assert.equal('value' in mismatchSettled, false); assertFailureStage(mismatchSettled.error, 'unknown'); assert.deepEqual([mismatchState.queries, mismatchState.closes], [1, 1]);
    const invalid = governedAgent(root); const invalidState = installIdentifiedEngine(invalid, boundValid.settled.value.runtimeAttestation, '{'); const invalidSettled = await invalid.answerGoverned('invalid', { schema: LINEAGE_SCHEMA, invocationDigest: DIGEST_A, resultIdentity: RESULT_IDENTITY }).then(value => ({ value }), error => ({ error })); assert.equal('value' in invalidSettled, false); assertFailureStage(invalidSettled.error, 'schema_result_validation'); assert.deepEqual([invalidState.queries, invalidState.closes], [1, 1]);
  });

  await t.test('EXP-0152 T12 public identity and receipt recursively exclude operational bytes', () => {
    const projection = { runtimeAttestation: identifiedValid.runtimeAttestation, resultIdentity: identifiedValid.resultIdentity }; const serialized = JSON.stringify(projection);
    for (const forbidden of ['prompt', 'schema', 'candidate', 'canonical', 'environment', 'credentials', 'correlation', 'requestId', 'sessionId', 'threadId', 'conversationId', 'cwd', 'path']) assert.equal(recursiveKeys(projection).includes(forbidden), false, forbidden);
    for (const secret of [root, LINEAGE_SCHEMA, JSON.stringify(LINEAGE), process.env.PATH]) assert.equal(serialized.includes(secret), false);
  });

  await t.test('EXP-0152 T13 complete EXP-0151 oracle set remains present', async () => {
    const source = await readFile(new URL(import.meta.url), 'utf8'); for (let i = 1; i <= 16; i++) assert.match(source, new RegExp(`EXP-0151 O${String(i).padStart(2, '0')}`));
  });

  await t.test('EXP-0152 T14 declaration mirrors and overload order are exact', async () => {
    const sourceDeclaration = await readFile(new URL('../../src/agent/ProbeAgent.d.ts', import.meta.url), 'utf8'); const packageDeclaration = await readFile(new URL('../../index.d.ts', import.meta.url), 'utf8');
    const contract = value => value.slice(value.indexOf('export interface GovernedAnswerOptions'), value.indexOf('/**\n * Clone options', value.indexOf('export interface GovernedAnswerOptions'))); assert.equal(contract(sourceDeclaration), contract(packageDeclaration));
    const overloads = value => value.slice(value.indexOf('  answerGoverned(message: string, options: GovernedIdentifiedAnswerOptions'), value.indexOf('\n\n  /**\n   * Get token usage', value.indexOf('  answerGoverned(message: string, options: GovernedIdentifiedAnswerOptions'))); assert.equal(overloads(sourceDeclaration), overloads(packageDeclaration));
    assert.equal(overloads(sourceDeclaration), `  answerGoverned(message: string, options: GovernedIdentifiedAnswerOptions, images?: any[]): Promise<GovernedIdentifiedAnswerResult>;\n  answerGoverned(message: string, options: GovernedInvocationAnswerOptions, images?: any[]): Promise<GovernedInvocationAnswerResult>;\n  answerGoverned(message: string, options: GovernedAnswerOptions, images?: any[]): Promise<GovernedAnswerResult>;\n  previewGovernedAnswerDispatch(message: string, options: GovernedAnswerDispatchOptions): Promise<Readonly<GovernedAnswerDispatch>>;`);
  });

  await t.test('EXP-0152 T15 frozen files and legacy function/declaration regions remain exact', async () => {
    const sha = value => createHash('sha256').update(value).digest('hex'); const read = path => readFile(new URL(path, import.meta.url), 'utf8');
    assert.equal(sha(await read('../../src/agent/engines/codex.js')), '41a4ff48e7f5a51cefe2efc89d7563336c7756377e974a6beb427386f87f5e38'); assert.equal(sha(await read('../../src/agent/schemaUtils.js')), '24332877e019ef29311f03ce9b63e61925c25fd7f83f5dd442b22dc68c60f6e9'); assert.equal(sha(await read('../../src/agent/engines/governed-codex-profile.js')), 'b0824ae50bb26a4c189e8224392cedbc8116b092a16baa199acd97aadcf7ced9');
    const source = await read('../../src/agent/ProbeAgent.js'); const answer = source.slice(source.indexOf('  async answer(message'), source.indexOf('  /**\n   * Get token usage information', source.indexOf('  async answer(message'))); assert.equal(sha(answer), '53ee9f207963f5b991aaf89e143211039ad632d9cc7535d91af66bcae95b135f');
    const governed = source.slice(source.indexOf('  async answerGoverned(message'), source.indexOf('\n  /**\n   * Answer a question', source.indexOf('  async answerGoverned(message'))); assert.equal((governed.match(/_prepareGovernedAnswerPrompt\(/g) || []).length, 1); assert.equal((governed.match(/options\.schema/g) || []).length, 0); assert.equal((governed.match(/options\.resultIdentity/g) || []).length, 1); assert.equal((governed.match(/validateJsonResponse\(/g) || []).length, 1); assert.match(governed, /return \{ data: validation\.parsed, runtimeAttestation \};/);
    const prepared = source.slice(source.indexOf('  _prepareGovernedAnswerPrompt'), source.indexOf('\n  /**\n   * Preview', source.indexOf('  _prepareGovernedAnswerPrompt'))); assert.equal((prepared.match(/options\.schema/g) || []).length, 1); assert.equal((prepared.match(/generateSchemaInstructions\(/g) || []).length, 1);
    const identityHelper = source.slice(source.indexOf('function identifyGovernedResult'), source.indexOf('// Maximum tool iterations')); assert.equal((identityHelper.match(/JSON\.stringify\(/g) || []).length, 1);
  });
});
