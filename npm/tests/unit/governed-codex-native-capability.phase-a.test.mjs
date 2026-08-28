import assert from 'node:assert/strict';
import test from 'node:test';
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { attestGovernedCodexSession, buildGovernedCodexInitialToolArgs,
  validateGovernedCodexProfile } from '../../src/agent/engines/governed-codex-profile.js';

const TOOLS = ['search', 'extract', 'listFiles'];
const PROFILE_ID = 'luna-xhigh-readonly-native-exec-v1';
const schema = JSON.stringify({ type: 'object', required: ['ok'], additionalProperties: false,
  properties: { ok: { type: 'boolean' } } });
const profile = (cwd) => ({ version: 'probe.governed-codex-profile/v2', profileId: PROFILE_ID, engine: 'codex',
  model: 'gpt-5.6-luna', reasoningEffort: 'xhigh', sandbox: 'read-only', approvalPolicy: 'never', cwd,
  probeMcpTools: [...TOOLS], codexNativeTools: ['exec'], fallback: false, retries: 0 });
const permission = () => ({ type: 'managed', file_system: { type: 'restricted', entries: [
  { access: 'read', path: { type: 'special', value: { kind: 'root' } } }
] }, network: 'restricted' });
const session = (cwd) => ({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'session-safe' }, id: '', msg: {
  type: 'session_configured', session_id: 'session-safe', thread_id: 'session-safe', model: 'gpt-5.6-luna',
  model_provider_id: 'openai', approval_policy: 'never', approvals_reviewer: 'user', permission_profile: permission(),
  reasoning_effort: 'xhigh', rollout_path: `${cwd}/sessions/2026/08/28/rollout-2026-08-28T12-00-00-00000000-0000-4000-8000-000000000001.jsonl`, cwd
} } });
const native = (index = 0, patch = {}) => ({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'session-safe' }, msg: {
  type: 'raw_response_item', item: { type: 'custom_tool_call', id: `raw-secret-id-${index}`, status: 'completed',
    call_id: `raw-secret-call-${index}`, name: 'exec', input: 'SECRET_ARGUMENT_BODY',
    internal_chat_message_metadata_passthrough: { turn_id: 'raw-secret-turn' }, ...patch }
}, id: '2' } });

test('Phase A profile attests only a bounded disjoint native capability aggregate', async () => {
  const cwd = await mkdtemp(join(tmpdir(), 'probe-native-capability-'));
  try {
    const normalized = validateGovernedCodexProfile(profile(cwd));
    assert.deepEqual(normalized.probeMcpTools, TOOLS); assert.deepEqual(normalized.codexNativeTools, ['exec']);
    const args = buildGovernedCodexInitialToolArgs({ profile: normalized, prompt: 'bounded',
      mcp: { name: 'probe_0123456789abcdef', url: 'http://127.0.0.1:12345/mcp' } });
    const server = args.config.mcp_servers.probe_0123456789abcdef;
    assert.deepEqual(server.enabled_tools, ['mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles']);
    assert.equal(JSON.stringify(server).includes('exec'), false);
    const aggregate = { total: 1, tools: [{ name: 'exec', status: 'completed', count: 1 }] };
    const attestation = attestGovernedCodexSession({ profile: normalized, events: [session(cwd), aggregate] });
    assert.equal(attestation.version, 'probe.governed-codex-attestation/v3');
    assert.equal(attestation.profileId, PROFILE_ID);
    assert.deepEqual(attestation.observed.nativeTools, aggregate);
    assert.doesNotThrow(() => attestGovernedCodexSession({ profile: normalized,
      events: [session(cwd), { total: 256, tools: [{ name: 'exec', status: 'completed', count: 256 }] }] }));
    for (const rejected of [
      { total: 257, tools: [{ name: 'exec', status: 'completed', count: 257 }] },
      { total: 1, tools: [{ name: 'bash', status: 'completed', count: 1 }] },
      { total: 1, tools: [{ name: 'exec', status: 'started', count: 1 }] },
      { total: 2, tools: [{ name: 'exec', status: 'completed', count: 1 }] }
    ]) assert.throws(() => attestGovernedCodexSession({ profile: normalized, events: [session(cwd), rejected] }), /Invalid/);
    assert.throws(() => validateGovernedCodexProfile({ ...profile(cwd), codexNativeTools: ['search'] }), /Invalid/);
  } finally { await rm(cwd, { recursive: true, force: true }); }
});

test('Phase A fake engine covers native exec admission, separation, rejection, observability, and cleanup', async () => {
  const root = await mkdtemp(join(tmpdir(), 'probe-native-engine-')), bin = join(root, 'bin'); await mkdir(bin);
  const fake = `#!/usr/bin/env node
import { writeFileSync } from 'node:fs';
import { createInterface } from 'node:readline';
writeFileSync(process.env.PROBE_NATIVE_PID_FILE, String(process.pid));
const send = value => process.stdout.write(JSON.stringify(value) + '\\n');
const permission = ${permission.toString()};
const session = ${session.toString()};
const native = ${native.toString()};
const passthrough = { turn_id: 'raw-secret-turn' };
createInterface({ input: process.stdin }).on('line', async line => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') return send({ jsonrpc: '2.0', id: request.id, result: {} });
  const args = request.params.arguments, cwd = args.cwd, prompt = args.prompt;
  writeFileSync(process.env.PROBE_NATIVE_ARGS_FILE, JSON.stringify(args));
  send((session)(cwd));
  if (prompt.includes('[NONTOOL]')) {
    send({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'session-safe' }, id: '2', msg: { type: 'raw_response_item', item: { type: 'message', role: 'user', content: [{ type: 'input_text', text: 'content is not retained' }], internal_chat_message_metadata_passthrough: passthrough } } } });
    send({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'session-safe' }, id: '2', msg: { type: 'raw_response_item', item: { type: 'reasoning', id: 'reasoning-safe', summary: [], encrypted_content: 'opaque', internal_chat_message_metadata_passthrough: passthrough } } } });
  }
  const emitCall = (index, patch = {}) => send((native)(index, patch));
  if (prompt.includes('[EXEC]') || prompt.includes('[BADJSON]') || prompt.includes('[NONTOOL]')) {
    emitCall(0);
    send({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'session-safe' }, id: '2', msg: { type: 'raw_response_item', item: { type: 'custom_tool_call_output', call_id: 'raw-secret-call-0', output: [{ type: 'input_text', text: 'SECRET_RESULT_BODY' }], internal_chat_message_metadata_passthrough: passthrough } } } });
  } else if (prompt.includes('[MCP]')) {
    emitCall(0, { name: 'mcp__probe__listFiles', input: 'SECRET_MCP_ARGUMENT_BODY' });
    const url = Object.values(args.config.mcp_servers)[0].url.replace('/mcp', '/rpc');
    const response = await fetch(url, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: 'mcp__probe__listFiles', arguments: { directory: '.' } } }) });
    const body = await response.text();
    send({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'session-safe' }, id: '2', msg: { type: 'raw_response_item', item: { type: 'custom_tool_call_output', call_id: 'raw-secret-call-0', output: [{ type: 'input_text', text: body }], internal_chat_message_metadata_passthrough: passthrough } } } });
  } else if (prompt.includes('[UNKNOWN-MCP]')) emitCall(0, { name: 'mcp__probe__bash' });
  else if (prompt.includes('[UNDECLARED]')) emitCall(0, { name: 'bash' });
  else if (prompt.includes('[MALFORMED]')) { const event = (native)(0); delete event.params.msg.item.status; send(event); }
  else if (prompt.includes('[UNKNOWN]')) emitCall(0, { type: 'future_tool_call' });
  else if (prompt.includes('[DUPLICATE]')) { emitCall(0); emitCall(0); }
  else if (prompt.includes('[CROSS]')) { const event = (native)(0); event.params._meta.threadId = 'other-session'; send(event); }
  else if (prompt.includes('[OVERFLOW]')) for (let index = 0; index < 257; index++) emitCall(index);
  send({ jsonrpc: '2.0', id: request.id, result: { content: [{ type: 'text', text: prompt.includes('[BADJSON]') ? 'not-json' : '{"ok":true}' }] } });
});
`;
  const executable = join(bin, 'codex'), priorPath = process.env.PATH;
  const priorPid = process.env.PROBE_NATIVE_PID_FILE, priorArgs = process.env.PROBE_NATIVE_ARGS_FILE;
  await writeFile(executable, fake); await chmod(executable, 0o755); process.env.PATH = `${bin}:${priorPath}`;
  let runIndex = 0;
  async function run(marker) {
    const index = runIndex++, pidFile = join(root, `pid-${index}`), argsFile = join(root, `args-${index}`);
    process.env.PROBE_NATIVE_PID_FILE = pidFile; process.env.PROBE_NATIVE_ARGS_FILE = argsFile;
    const agent = new ProbeAgent({ provider: 'codex', path: root, cwd: root, allowedTools: [...TOOLS],
      governedCodexProfile: profile(root), searchDelegate: false, disableMermaidValidation: true });
    const events = []; agent.events.on('toolCall', (event) => events.push(event));
    let result, error;
    try { result = await agent.answerGoverned(marker, { schema, invocationDigest: `sha256:${'0'.repeat(64)}` }); }
    catch (caught) { error = caught; }
    finally { await agent.close().catch(() => {}); }
    const pid = Number(await readFile(pidFile, 'utf8'));
    assert.throws(() => process.kill(pid, 0), { code: 'ESRCH' });
    return { result, error, events, args: JSON.parse(await readFile(argsFile, 'utf8')) };
  }
  try {
    const zero = await run('[ZERO]'); assert.ifError(zero.error);
    assert.deepEqual(zero.result.runtimeAttestation.observed.nativeTools, { total: 0, tools: [] });
    assert.deepEqual(zero.result.runtimeAttestation.evidence,
      { sessionEventCount: 1, nativeCallCount: 0, probeMcpCallCount: 0 });
    assert.deepEqual(zero.events, []);

    const mcp = await run('[MCP]'); assert.ifError(mcp.error);
    assert.deepEqual(mcp.result.runtimeAttestation.observed.nativeTools, { total: 0, tools: [] });
    assert.deepEqual(mcp.result.runtimeAttestation.evidence,
      { sessionEventCount: 1, nativeCallCount: 0, probeMcpCallCount: 1 });
    assert.deepEqual(mcp.events.map(({ name, status }) => ({ name, status })),
      [{ name: 'listFiles', status: 'in_progress' }, { name: 'listFiles', status: 'completed' }]);
    const mcpSerialized = JSON.stringify({ result: mcp.result, events: mcp.events });
    for (const secret of ['SECRET_MCP_ARGUMENT_BODY', 'raw-secret-id', 'raw-secret-call', 'raw-secret-turn'])
      assert.equal(mcpSerialized.includes(secret), false);

    for (const marker of ['[EXEC]', '[NONTOOL]']) {
      const valid = await run(marker); assert.ifError(valid.error);
      const aggregate = { name: 'exec', status: 'completed', count: 1 };
      assert.deepEqual(valid.result.runtimeAttestation.observed.nativeTools, { total: 1, tools: [aggregate] });
      assert.deepEqual(valid.result.runtimeAttestation.evidence,
        { sessionEventCount: 1, nativeCallCount: 1, probeMcpCallCount: 0 });
      assert.deepEqual(valid.events, [aggregate]);
      assert.deepEqual(valid.args.config.mcp_servers[Object.keys(valid.args.config.mcp_servers)[0]].enabled_tools,
        ['mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles']);
      assert.equal(JSON.stringify(valid.args.config.mcp_servers).includes('exec'), false);
      const serialized = JSON.stringify({ result: valid.result, events: valid.events });
      for (const secret of ['SECRET_ARGUMENT_BODY', 'SECRET_RESULT_BODY', 'raw-secret-id', 'raw-secret-call', 'raw-secret-turn']) assert.equal(serialized.includes(secret), false);
    }

    for (const marker of ['[UNKNOWN-MCP]', '[UNDECLARED]', '[MALFORMED]', '[UNKNOWN]', '[DUPLICATE]', '[CROSS]', '[OVERFLOW]']) {
      const rejected = await run(marker); assert.match(rejected.error?.message || '', /Invalid governed Codex native event evidence/);
      assert.deepEqual(rejected.events, []);
    }

    const invalidAnswer = await run('[BADJSON]');
    assert.match(invalidAnswer.error?.message || '', /schema|JSON/i);
    assert.deepEqual(invalidAnswer.events, [{ name: 'exec', status: 'completed', count: 1 }]);
  } finally {
    process.env.PATH = priorPath;
    if (priorPid === undefined) delete process.env.PROBE_NATIVE_PID_FILE; else process.env.PROBE_NATIVE_PID_FILE = priorPid;
    if (priorArgs === undefined) delete process.env.PROBE_NATIVE_ARGS_FILE; else process.env.PROBE_NATIVE_ARGS_FILE = priorArgs;
    await rm(root, { recursive: true, force: true });
  }
});
