import assert from 'node:assert/strict';
import test from 'node:test';
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { attestGovernedCodexSession, buildGovernedCodexInitialToolArgs,
  validateGovernedCodexProfile } from '../../src/agent/engines/governed-codex-profile.js';
import { governedAnswerFailure,
  normalizeGovernedAnswerFailure } from '../../src/agent/engines/governed-answer-failure.js';

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
const session = (cwd, patch = {}) => ({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'session-safe' }, id: '', msg: {
  type: 'session_configured', session_id: 'session-safe', thread_id: 'session-safe', model: 'gpt-5.6-luna',
  model_provider_id: 'openai', approval_policy: 'never', approvals_reviewer: 'user', permission_profile: permission(),
  reasoning_effort: 'xhigh', rollout_path: `${cwd}/sessions/2026/08/28/rollout-2026-08-28T12-00-00-00000000-0000-4000-8000-000000000001.jsonl`, cwd, ...patch
} } });
const native = (index = 0, patch = {}) => ({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'session-safe' }, msg: {
  type: 'raw_response_item', item: { type: 'custom_tool_call', id: `raw-secret-id-${index}`, status: 'completed',
    call_id: `raw-secret-call-${index}`, name: 'exec', input: 'SECRET_ARGUMENT_BODY',
    internal_chat_message_metadata_passthrough: { turn_id: 'raw-secret-turn' }, ...patch }
}, id: '2' } });
function assertFailure(result, stage, boundary = null, subreason = null, correlationOperand = null,
  attestationPredicate = null, schemaSubreason = null) {
  assert.equal(result.result, undefined);
  assert.equal(result.error?.name, 'GovernedAnswerFailure');
  assert.equal(result.error?.message, '');
  assert.equal(result.error?.answerFailureStage, stage);
  if (stage === 'native_event_grammar') {
    assert.equal(result.error?.nativeEventFailureBoundary, boundary);
    if (boundary === 'live_envelope_session') {
      assert.equal(Object.hasOwn(result.error ?? {}, 'nativeEventFailureSubreason'), true);
      assert.equal(result.error?.nativeEventFailureSubreason, subreason);
    } else assert.equal(Object.hasOwn(result.error ?? {}, 'nativeEventFailureSubreason'), false);
  } else {
    assert.equal(Object.hasOwn(result.error ?? {}, 'nativeEventFailureBoundary'), false);
    assert.equal(Object.hasOwn(result.error ?? {}, 'nativeEventFailureSubreason'), false);
  }
  if (stage === 'native_event_grammar' && boundary === 'live_envelope_session' && subreason === 'correlation') {
    assert.equal(Object.hasOwn(result.error ?? {}, 'nativeEventFailureCorrelationOperand'), true);
    assert.equal(result.error?.nativeEventFailureCorrelationOperand, correlationOperand);
  } else assert.equal(Object.hasOwn(result.error ?? {}, 'nativeEventFailureCorrelationOperand'), false);
  if (stage === 'native_event_grammar' && boundary === 'live_envelope_session' && subreason === 'attestation') {
    assert.equal(Object.hasOwn(result.error ?? {}, 'nativeEventFailureAttestationPredicate'), true);
    assert.equal(result.error?.nativeEventFailureAttestationPredicate, attestationPredicate);
  } else assert.equal(Object.hasOwn(result.error ?? {}, 'nativeEventFailureAttestationPredicate'), false);
  if (stage === 'schema_result_validation') {
    assert.equal(Object.hasOwn(result.error ?? {}, 'schemaResultValidationSubreason'), true);
    assert.equal(result.error?.schemaResultValidationSubreason, schemaSubreason);
  } else assert.equal(Object.hasOwn(result.error ?? {}, 'schemaResultValidationSubreason'), false);
  assert.equal(result.error?.stack, undefined);
  assert.equal(Object.hasOwn(result.error ?? {}, 'cause'), false);
  const serialized = JSON.stringify(result.error);
  for (const forbidden of ['SECRET_', 'raw-secret', 'Error:', 'at file:']) assert.equal(serialized.includes(forbidden), false);
}

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
      events: [session(cwd), aggregate] }), 'historical 11-key session shape');
    assert.doesNotThrow(() => attestGovernedCodexSession({ profile: normalized,
      events: [session(cwd, { service_tier: 'default' }), aggregate] }), 'current 12-key session shape');
    for (const service_tier of ['default', 'priority', 'flex'])
      assert.doesNotThrow(() => attestGovernedCodexSession({ profile: normalized,
        events: [session(cwd, { service_tier }), aggregate] }), `service tier ${service_tier}`);
    for (const service_tier of [null, '', 'batch', 1, true, [], {}])
      assert.throws(() => attestGovernedCodexSession({ profile: normalized,
        events: [session(cwd, { service_tier }), aggregate] }), /Invalid event\.msg/);
    for (const optional of ['forked_from_id', 'parent_thread_id', 'thread_source', 'thread_name',
      'active_permission_profile', 'initial_messages', 'network_proxy'])
      assert.throws(() => attestGovernedCodexSession({ profile: normalized,
        events: [session(cwd, { [optional]: 'SECRET_OPTIONAL_FIELD' }), aggregate] }), /Invalid event\.msg/);
    assert.throws(() => attestGovernedCodexSession({ profile: normalized,
      events: [session(cwd, { unknown_extra: 'SECRET_UNKNOWN_FIELD' }), aggregate] }), /Invalid event\.msg/);
    const missingRequired = session(cwd, { service_tier: 'default' }); delete missingRequired.params.msg.model;
    assert.throws(() => attestGovernedCodexSession({ profile: normalized,
      events: [missingRequired, aggregate] }), /Invalid event\.msg/);
    assert.doesNotThrow(() => attestGovernedCodexSession({ profile: normalized,
      events: [session(cwd), { total: 256, tools: [{ name: 'exec', status: 'completed', count: 256 }] }] }));
    for (const rejected of [
      { total: 257, tools: [{ name: 'exec', status: 'completed', count: 257 }] },
      { total: 1, tools: [{ name: 'bash', status: 'completed', count: 1 }] },
      { total: 1, tools: [{ name: 'exec', status: 'started', count: 1 }] },
      { total: 2, tools: [{ name: 'exec', status: 'completed', count: 1 }] }
    ]) assert.throws(() => attestGovernedCodexSession({ profile: normalized, events: [session(cwd), rejected] }), /Invalid/);
    assert.throws(() => validateGovernedCodexProfile({ ...profile(cwd), codexNativeTools: ['search'] }), /Invalid/);
    const source = await readFile(new URL('../../src/agent/engines/codex.js', import.meta.url), 'utf8');
    const collectorStart = source.indexOf('function createGovernedNativeCollector(profile)');
    const collectorEnd = source.indexOf('\nfunction externalReceipt(', collectorStart);
    const collectorSource = source.slice(collectorStart, collectorEnd);
    const typeExtraction = collectorSource.indexOf('const type = event?.params?.msg?.type;');
    const rawTypeFilter = collectorSource.indexOf("type !== 'raw_response_item') return;");
    const msgValidation = collectorSource.indexOf("const msg = governedExactObject(params.msg, ['type', 'item']");
    const redundantMsgTypeGuard = collectorSource.indexOf("if (msg.type !== 'raw_response_item')");
    assert.ok(typeExtraction >= 0 && typeExtraction < rawTypeFilter && rawTypeFilter < msgValidation &&
      msgValidation < redundantMsgTypeGuard);
    assert.equal((collectorSource.match(/if \(profile\.version !== 'probe\.governed-codex-profile\/v2' \|\| type !== 'raw_response_item'\) return;/g) ?? []).length, 1);
    assert.equal((collectorSource.match(/msg\.type !== 'raw_response_item'/g) ?? []).length, 1);
    const routingStart = source.indexOf('// Handle notifications (codex/event)');
    const routingEnd = source.indexOf('\n    } catch (e)', routingStart);
    const routingSource = source.slice(routingStart, routingEnd);
    const methodRoutingGate = routingSource.indexOf("message.method === 'codex/event'");
    const requestIdExtraction = routingSource.indexOf('const requestId = message.params._meta?.requestId;');
    const governedHas = routingSource.indexOf('governedEvidenceHandlers.has(requestId)');
    const governedGet = routingSource.indexOf('governedEvidenceHandlers.get(requestId)(message)');
    assert.ok(methodRoutingGate >= 0 && methodRoutingGate < requestIdExtraction &&
      requestIdExtraction < governedHas && governedHas < governedGet);
    assert.match(source, /governedEvidenceHandlers\.set\(reqId,[\s\S]*?collector\.observe\(event\)/);
    const requestCorrelationGuard = collectorSource.indexOf('if (meta.requestId !== requestId)');
    const threadCorrelationGuard = collectorSource.indexOf("if (meta.threadId !== threadId) governedLiveEnvelopeInvalid('correlation', 'thread_id');");
    const responseEnvelopeShapeGuard = collectorSource.indexOf("if (typeof params.id !== 'string') governedLiveEnvelopeInvalid('envelope_shape');");
    assert.ok(requestCorrelationGuard >= 0 && requestCorrelationGuard < threadCorrelationGuard &&
      threadCorrelationGuard < responseEnvelopeShapeGuard && responseEnvelopeShapeGuard < msgValidation);
    assert.equal(collectorSource.includes('params.id !== String(requestId)'), false);
    assert.match(collectorSource,
      /event\.jsonrpc !== '2\.0' \|\| event\.method !== 'codex\/event'/);
    assert.equal((source.match(/governedLiveEnvelopeInvalid\('session_sequence'\)/g) ?? []).length, 3);
    assert.equal((source.match(/governedLiveEnvelopeInvalid\('envelope_shape'\)/g) ?? []).length, 7);
    assert.equal((source.match(/governedLiveEnvelopeInvalid\('correlation'\)/g) ?? []).length, 1);
    assert.equal((source.match(/governedLiveEnvelopeInvalid\('correlation', 'thread_id'\)/g) ?? []).length, 1);
    assert.equal((source.match(/governedLiveEnvelopeInvalid\('correlation', 'response_id'\)/g) ?? []).length, 0);
    assert.equal((source.match(/'attestation'/g) ?? []).length, 1);
    assert.equal((source.match(/governedLiveEnvelopeInvalid\(\)/g) ?? []).length, 0);
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
const currentPassthrough = { turn_id: 'raw-secret-turn', create_time: 7.25 };
const currentMessagePassthrough = { turn_id: 'raw-secret-turn', create_time: 7.25, content_item_kinds: ['input_text'] };
createInterface({ input: process.stdin }).on('line', async line => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') return send({ jsonrpc: '2.0', id: request.id, result: {} });
  const args = request.params.arguments, cwd = args.cwd, prompt = args.prompt;
  writeFileSync(process.env.PROBE_NATIVE_ARGS_FILE, JSON.stringify(args));
  const configured = (session)(cwd);
  const foreignSession = (foreignRequestId = 99) => {
    const event = (session)(cwd);
    event.params._meta = { requestId: foreignRequestId, threadId: 'SECRET_FOREIGN_THREAD' };
    event.params.msg.session_id = 'SECRET_FOREIGN_SESSION';
    event.params.msg.thread_id = 'SECRET_FOREIGN_SESSION';
    return event;
  };
  const foreignRaw = () => send({ jsonrpc: '2.0', method: 'codex/event', params: {
    _meta: { requestId: 99, threadId: 'SECRET_FOREIGN_THREAD' }, id: '99', msg: {
      type: 'raw_response_item', item: { type: 'SECRET_FOREIGN_ITEM', payload: 'SECRET_FOREIGN_BODY' }
    }
  } });
  if (prompt.includes('[LIVE-SESSION]')) configured.params.msg.model = 'SECRET_INVALID_MODEL';
  if (prompt.includes('[SERVICE-TIER-CURRENT]') || prompt.includes('[SERVICE-TIER-DEFAULT]')) configured.params.msg.service_tier = 'default';
  if (prompt.includes('[SERVICE-TIER-PRIORITY]')) configured.params.msg.service_tier = 'priority';
  if (prompt.includes('[SERVICE-TIER-FLEX]')) configured.params.msg.service_tier = 'flex';
  if (prompt.includes('[SERVICE-TIER-INVALID]')) configured.params.msg.service_tier = 'SECRET_INVALID_TIER';
  for (const optional of ['forked_from_id', 'parent_thread_id', 'thread_source', 'thread_name',
    'active_permission_profile', 'initial_messages', 'network_proxy'])
    if (prompt.includes('[SESSION-OPTIONAL-' + optional + ']')) configured.params.msg[optional] = 'SECRET_OPTIONAL_FIELD';
  if (prompt.includes('[SESSION-UNKNOWN-EXTRA]')) configured.params.msg.unknown_extra = 'SECRET_UNKNOWN_FIELD';
  if (prompt.includes('[ATTEST-RESPONSE-ID]')) configured.params.id = 'opaque-session-response-id';
  if (prompt.includes('[ATTEST-SESSION-SHAPE]')) delete configured.params.msg.model;
  if (prompt.includes('[ATTEST-IDENTITY]')) configured.params.msg.session_id = 'SECRET_INVALID_IDENTITY';
  if (prompt.includes('[ATTEST-PERMISSION]')) configured.params.msg.permission_profile.network = 'enabled';
  if (prompt.includes('[ATTEST-PERMISSION-TYPE]')) configured.params.msg.permission_profile.type = 'SECRET_TYPE';
  if (prompt.includes('[ATTEST-ROLLOUT]')) configured.params.msg.rollout_path += '/SECRET_INVALID_ROLLOUT';
  if (prompt.includes('[ATTEST-CWD]')) configured.params.msg.cwd = '/SECRET_INVALID_CWD';
  if (prompt.includes('[FOREIGN-SESSION-BEFORE]') || prompt.includes('[FOREIGN-DUPLICATE-SESSION]') ||
    prompt.includes('[CONCURRENT-FOREIGN]')) send(foreignSession());
  if (prompt.includes('[MISSING-REQUEST-ID]')) {
    const event = foreignSession(); delete event.params._meta.requestId; send(event);
  }
  if (prompt.includes('[MALFORMED-REQUEST-ID]')) send(foreignSession('2'));
  if (prompt.includes('[METHOD-MISMATCH]')) {
    const event = (native)(0); event.method = 'other'; send(event);
  }
  if (!prompt.includes('[LIVE-ORDER]') && !prompt.includes('[LIVE-MISSING-SESSION]')) send(configured);
  if (prompt.includes('[FOREIGN-RAW]') || prompt.includes('[CONCURRENT-FOREIGN]')) foreignRaw();
  if (prompt.includes('[FOREIGN-DUPLICATE-SESSION]') || prompt.includes('[CONCURRENT-FOREIGN]')) send(foreignSession());
  const raw = item => send({ jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'session-safe' }, id: '2', msg: { type: 'raw_response_item', item } } });
  const liveEnvelope = mutate => { const event = (native)(0); mutate(event); send(event); };
  if (prompt.includes('[LIVE-RAW-OBJECT]')) liveEnvelope(event => { event.extra = 'SECRET_ENVELOPE_EXTRA'; });
  if (prompt.includes('[LIVE-JSONRPC]')) liveEnvelope(event => { event.jsonrpc = '1.0'; });
  if (prompt.includes('[LIVE-PARAMS]')) liveEnvelope(event => { event.params.extra = 'SECRET_PARAMS_EXTRA'; });
  if (prompt.includes('[LIVE-META]')) liveEnvelope(event => { event.params._meta.extra = 'SECRET_META_EXTRA'; });
  if (prompt.includes('[LIVE-MSG]')) liveEnvelope(event => { event.params.msg.extra = 'SECRET_MSG_EXTRA'; });
  if (prompt.includes('[LIVE-RESPONSE-ID-OPAQUE]')) liveEnvelope(event => { event.params.id = 'opaque-safe-response-id'; });
  if (prompt.includes('[LIVE-RESPONSE-ID-EMPTY]')) liveEnvelope(event => { event.params.id = ''; });
  if (prompt.includes('[LIVE-RESPONSE-ID-MISSING]')) liveEnvelope(event => { delete event.params.id; });
  if (prompt.includes('[LIVE-RESPONSE-ID-EXTRA]')) liveEnvelope(event => { event.params.responseId = 'extra-safe-id'; });
  if (prompt.includes('[LIVE-RESPONSE-ID-NUMBER]')) liveEnvelope(event => { event.params.id = 2; });
  if (prompt.includes('[LIVE-RESPONSE-ID-NULL]')) liveEnvelope(event => { event.params.id = null; });
  if (prompt.includes('[LIVE-RESPONSE-ID-BOOLEAN]')) liveEnvelope(event => { event.params.id = true; });
  if (prompt.includes('[LIVE-RESPONSE-ID-ARRAY]')) liveEnvelope(event => { event.params.id = []; });
  if (prompt.includes('[LIVE-RESPONSE-ID-OBJECT]')) liveEnvelope(event => { event.params.id = {}; });
  const message = (id, role, phase, metadata = currentMessagePassthrough) => ({ type: 'message', id, role,
    content: [{ type: role === 'assistant' ? 'output_text' : 'input_text', text: 'SECRET_MESSAGE_BODY' }],
    ...(role === 'assistant' ? { phase } : {}), internal_chat_message_metadata_passthrough: metadata });
  if (prompt.includes('[ATTEMPT7]')) {
    raw(message('developer-safe', 'developer'));
    raw(message('user-safe', 'user'));
    raw(message('commentary-safe', 'assistant', 'commentary', { ...currentMessagePassthrough, content_item_kinds: ['output_text'] }));
    raw({ type: 'custom_tool_call', id: 'call-item-safe', status: 'completed', call_id: 'call-safe', name: 'exec',
      input: 'SECRET_ARGUMENT_BODY', internal_chat_message_metadata_passthrough: currentPassthrough });
    raw({ type: 'custom_tool_call_output', id: 'output-item-safe', call_id: 'call-safe',
      output: [{ type: 'input_text', text: 'SECRET_RESULT_BODY' }], internal_chat_message_metadata_passthrough: currentPassthrough });
    raw(message('final-safe', 'assistant', 'final_answer', { ...currentMessagePassthrough, content_item_kinds: ['output_text'] }));
  }
  if (prompt.includes('[ATTEMPT8-SAFE]')) {
    const emittedOutputProjection = [];
    const emitOutput = item => { raw(item); emittedOutputProjection.push({
      type: item.type, outputPartTypes: item.output.map(part => part.type)
    }); };
    raw(message('attempt8-developer', 'developer'));
    raw(message('attempt8-user-1', 'user'));
    raw(message('attempt8-user-2', 'user'));
    raw({ type: 'reasoning', id: 'attempt8-reasoning-1', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw(message('attempt8-commentary-1', 'assistant', 'commentary', { ...currentMessagePassthrough, content_item_kinds: ['output_text'] }));
    raw({ type: 'custom_tool_call', id: 'attempt8-call-item-2', status: 'completed', call_id: 'attempt8-call-2', name: 'exec',
      input: 'SECRET_ARGUMENT_BODY', internal_chat_message_metadata_passthrough: currentPassthrough });
    emitOutput({ type: 'custom_tool_call_output', id: 'attempt8-output-item-2', call_id: 'attempt8-call-2', output: [
      { type: 'input_text', text: 'SECRET_RESULT_BODY_2_1' },
      { type: 'input_text', text: 'SECRET_RESULT_BODY_2_2' }
    ], internal_chat_message_metadata_passthrough: currentPassthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-2', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw({ type: 'custom_tool_call', id: 'attempt8-call-item-5', status: 'completed', call_id: 'attempt8-call-5', name: 'exec',
      input: 'SECRET_ARGUMENT_BODY', internal_chat_message_metadata_passthrough: currentPassthrough });
    emitOutput({ type: 'custom_tool_call_output', id: 'attempt8-output-item-5', call_id: 'attempt8-call-5', output: [
      { type: 'input_text', text: 'SECRET_RESULT_BODY_5_1' },
      { type: 'input_text', text: 'SECRET_RESULT_BODY_5_2' },
      { type: 'input_text', text: 'SECRET_RESULT_BODY_5_3' },
      { type: 'input_text', text: 'SECRET_RESULT_BODY_5_4' },
      { type: 'input_text', text: 'SECRET_RESULT_BODY_5_5' }
    ], internal_chat_message_metadata_passthrough: currentPassthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-3', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-4', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-5', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw({ type: 'custom_tool_call', id: 'attempt8-call-item-3', status: 'completed', call_id: 'attempt8-call-3', name: 'exec',
      input: 'SECRET_ARGUMENT_BODY', internal_chat_message_metadata_passthrough: currentPassthrough });
    emitOutput({ type: 'custom_tool_call_output', id: 'attempt8-output-item-3', call_id: 'attempt8-call-3', output: [
      { type: 'input_text', text: 'SECRET_RESULT_BODY_3_1' },
      { type: 'input_text', text: 'SECRET_RESULT_BODY_3_2' },
      { type: 'input_text', text: 'SECRET_RESULT_BODY_3_3' }
    ], internal_chat_message_metadata_passthrough: currentPassthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-6', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-7', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-8', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw(message('attempt8-commentary-2', 'assistant', 'commentary', { ...currentMessagePassthrough, content_item_kinds: ['output_text'] }));
    raw({ type: 'custom_tool_call', id: 'attempt8-call-item-4', status: 'completed', call_id: 'attempt8-call-4', name: 'exec',
      input: 'SECRET_ARGUMENT_BODY', internal_chat_message_metadata_passthrough: currentPassthrough });
    emitOutput({ type: 'custom_tool_call_output', id: 'attempt8-output-item-4', call_id: 'attempt8-call-4', output: [
      { type: 'input_text', text: 'SECRET_RESULT_BODY_4_1' },
      { type: 'input_text', text: 'SECRET_RESULT_BODY_4_2' },
      { type: 'input_text', text: 'SECRET_RESULT_BODY_4_3' },
      { type: 'input_text', text: 'SECRET_RESULT_BODY_4_4' }
    ], internal_chat_message_metadata_passthrough: currentPassthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-9', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-10', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-11', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-12', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw({ type: 'reasoning', id: 'attempt8-reasoning-13', summary: [], encrypted_content: 'SECRET_REASONING_BODY',
      internal_chat_message_metadata_passthrough: passthrough });
    raw(message('attempt8-final', 'assistant', 'final_answer', { ...currentMessagePassthrough, content_item_kinds: ['output_text'] }));
    writeFileSync(process.env.PROBE_NATIVE_PROJECTION_FILE, JSON.stringify(emittedOutputProjection));
  }
  if (prompt.includes('[LIVE-ORDER]')) raw(message('live-order-secret', 'user'));
  if (prompt.includes('[LIVE-DUPLICATE-SESSION]')) send((session)(cwd));
  if (prompt.includes('[BOUNDS]')) raw(message('a'.repeat(128), 'user', null, {
    turn_id: 'raw-secret-turn', create_time: 9007199254740991, content_item_kinds: Array(16).fill('a'.repeat(64)) }));
  if (prompt.includes('[DELTA-CREATE-NEGATIVE]')) raw(message('delta-safe', 'user', null, { ...currentMessagePassthrough, create_time: -1 }));
  if (prompt.includes('[DELTA-CREATE-NONFINITE]')) raw(message('delta-safe', 'user', null, { ...currentMessagePassthrough, create_time: NaN }));
  if (prompt.includes('[DELTA-CREATE-UNSAFE]')) raw(message('delta-safe', 'user', null, { ...currentMessagePassthrough, create_time: 9007199254740992 }));
  if (prompt.includes('[DELTA-KINDS-OVERFLOW]')) raw(message('delta-safe', 'user', null, { ...currentMessagePassthrough, content_item_kinds: Array(17).fill('input_text') }));
  if (prompt.includes('[DELTA-KINDS-WRONG]')) raw(message('delta-safe', 'user', null, { ...currentMessagePassthrough, content_item_kinds: 'input_text' }));
  if (prompt.includes('[DELTA-KIND-UNSAFE]')) raw(message('delta-safe', 'user', null, { ...currentMessagePassthrough, content_item_kinds: ['unsafe/kind'] }));
  if (prompt.includes('[DELTA-KIND-OVERSIZED]')) raw(message('delta-safe', 'user', null, { ...currentMessagePassthrough, content_item_kinds: ['a'.repeat(65)] }));
  if (prompt.includes('[DELTA-PARTIAL]')) raw(message('delta-safe', 'user', null, { turn_id: 'raw-secret-turn', create_time: 7 }));
  if (prompt.includes('[DELTA-EXTRA]')) raw(message('delta-safe', 'user', null, { ...currentMessagePassthrough, extra: true }));
  if (prompt.includes('[DELTA-ID-UNSAFE]')) raw(message('unsafe id', 'user'));
  if (prompt.includes('[DELTA-ID-OVERSIZED]')) raw(message('a'.repeat(129), 'user'));
  if (prompt.includes('[DELTA-DUP-ID]')) { raw(message('duplicate-safe', 'user')); raw(message('duplicate-safe', 'developer')); }
  if (prompt.includes('[DELTA-OUTPUT-DUP-ID]')) {
    raw({ type: 'custom_tool_call', id: 'duplicate-safe', status: 'completed', call_id: 'call-safe', name: 'exec', input: '', internal_chat_message_metadata_passthrough: currentPassthrough });
    raw({ type: 'custom_tool_call_output', id: 'duplicate-safe', call_id: 'call-safe', output: [], internal_chat_message_metadata_passthrough: currentPassthrough });
  }
  if (prompt.includes('[COMMENTARY-ONLY]')) raw(message('commentary-safe', 'assistant', 'commentary', { ...currentMessagePassthrough, content_item_kinds: ['output_text'] }));
  if (prompt.includes('[DOUBLE-FINAL]')) { raw(message('final-one', 'assistant', 'final_answer', { ...currentMessagePassthrough, content_item_kinds: ['output_text'] })); raw(message('final-two', 'assistant', 'final_answer', { ...currentMessagePassthrough, content_item_kinds: ['output_text'] })); }
  if (prompt.includes('[PHASE-UNKNOWN]')) raw(message('phase-safe', 'assistant', 'future_phase', { ...currentMessagePassthrough, content_item_kinds: ['output_text'] }));
  if (prompt.includes('[OVERFLOW-MESSAGES]')) for (let index = 0; index < 257; index++) raw(message('message-' + index, 'user'));
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
  if (prompt.includes('[AMBIGUOUS]')) { const event = (native)(0); delete event.params.msg.item.status; send(event); send({ jsonrpc: '2.0', id: request.id, error: { message: 'SECRET_PROVIDER_ERROR' } }); return; }
  if (prompt.includes('[PROVIDER-ERROR]')) { send({ jsonrpc: '2.0', id: request.id, error: { message: 'SECRET_PROVIDER_ERROR' } }); return; }
  const text = prompt.includes('[BADJSON]') ? 'not-json'
    : prompt.includes('[SCHEMA-REQUIRED]') ? '{}'
    : prompt.includes('[SCHEMA-EXTRA]') ? '{"ok":true,"extra":true}'
    : prompt.includes('[BADSCHEMA]') || prompt.includes('[SCHEMA-ENUM]') ? '{"ok":"wrong"}'
    : prompt.includes('[NONCANONICAL]') ? '{"ok":1e309}' : '{"ok":true}';
  send({ jsonrpc: '2.0', id: request.id, result: { content: [{ type: 'text', text }] } });
});
`;
  const executable = join(bin, 'codex'), priorPath = process.env.PATH;
  const priorPid = process.env.PROBE_NATIVE_PID_FILE, priorArgs = process.env.PROBE_NATIVE_ARGS_FILE;
  const priorProjection = process.env.PROBE_NATIVE_PROJECTION_FILE;
  await writeFile(executable, fake); await chmod(executable, 0o755); process.env.PATH = `${bin}:${priorPath}`;
  let runIndex = 0;
  async function run(marker, options = { schema, invocationDigest: `sha256:${'0'.repeat(64)}` }) {
    const index = runIndex++, pidFile = join(root, `pid-${index}`), argsFile = join(root, `args-${index}`);
    const projectionFile = join(root, `projection-${index}`);
    process.env.PROBE_NATIVE_PID_FILE = pidFile; process.env.PROBE_NATIVE_ARGS_FILE = argsFile;
    process.env.PROBE_NATIVE_PROJECTION_FILE = projectionFile;
    const agent = new ProbeAgent({ provider: 'codex', path: root, cwd: root, allowedTools: [...TOOLS],
      governedCodexProfile: profile(root), searchDelegate: false, disableMermaidValidation: true });
    const events = []; agent.events.on('toolCall', (event) => events.push(event));
    let result, error;
    try { result = await agent.answerGoverned(marker, options); }
    catch (caught) { error = caught; }
    finally { await agent.close().catch(() => {}); }
    const pid = Number(await readFile(pidFile, 'utf8'));
    assert.throws(() => process.kill(pid, 0), { code: 'ESRCH' });
    const projection = await readFile(projectionFile, 'utf8').then(JSON.parse).catch(() => null);
    return { result, error, events, args: JSON.parse(await readFile(argsFile, 'utf8')), projection };
  }
  try {
    const zero = await run('[ZERO]'); assert.ifError(zero.error);
    assert.deepEqual(zero.result.runtimeAttestation.observed.nativeTools, { total: 0, tools: [] });
    assert.deepEqual(zero.result.runtimeAttestation.evidence,
      { sessionEventCount: 1, nativeCallCount: 0, probeMcpCallCount: 0 });
    assert.deepEqual(zero.events, []);

    for (const marker of ['[FOREIGN-SESSION-BEFORE]', '[FOREIGN-RAW]', '[FOREIGN-DUPLICATE-SESSION]',
      '[MISSING-REQUEST-ID]', '[MALFORMED-REQUEST-ID]', '[METHOD-MISMATCH]', '[CONCURRENT-FOREIGN]']) {
      const routed = await run(marker); assert.ifError(routed.error);
      assert.deepEqual(routed.result.runtimeAttestation.observed.nativeTools, { total: 0, tools: [] });
      assert.deepEqual(routed.result.runtimeAttestation.evidence,
        { sessionEventCount: 1, nativeCallCount: 0, probeMcpCallCount: 0 });
      assert.deepEqual(routed.events, []);
      const serialized = JSON.stringify({ result: routed.result, events: routed.events });
      for (const secret of ['SECRET_FOREIGN', 'SECRET_ARGUMENT_BODY', 'raw-secret'])
        assert.equal(serialized.includes(secret), false);
    }

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

    const attempt7 = await run('[ATTEMPT7]'); assert.ifError(attempt7.error);
    assert.deepEqual(attempt7.result.runtimeAttestation.observed.nativeTools,
      { total: 1, tools: [{ name: 'exec', status: 'completed', count: 1 }] });
    assert.deepEqual(attempt7.result.runtimeAttestation.evidence,
      { sessionEventCount: 1, nativeCallCount: 1, probeMcpCallCount: 0 });
    const attempt7Serialized = JSON.stringify({ result: attempt7.result, events: attempt7.events });
    for (const secret of ['SECRET_', 'raw-secret', 'developer-safe', 'user-safe', 'commentary-safe', 'final-safe',
      'call-item-safe', 'call-safe', 'output-item-safe', 'create_time', 'content_item_kinds'])
      assert.equal(attempt7Serialized.includes(secret), false);
    const attempt8 = await run('[ATTEMPT8-SAFE]'); assert.ifError(attempt8.error);
    assert.deepEqual(attempt8.result.runtimeAttestation.observed.nativeTools,
      { total: 4, tools: [{ name: 'exec', status: 'completed', count: 4 }] });
    assert.deepEqual(attempt8.result.runtimeAttestation.evidence,
      { sessionEventCount: 1, nativeCallCount: 4, probeMcpCallCount: 0 });
    assert.deepEqual(attempt8.events, [{ name: 'exec', status: 'completed', count: 4 }]);
    assert.deepEqual(attempt8.projection.map(item => item.type), Array(4).fill('custom_tool_call_output'));
    const attempt8PartCounts = attempt8.projection.map(item => item.outputPartTypes.length);
    assert.deepEqual(attempt8PartCounts, [2, 5, 3, 4]);
    assert.equal(attempt8PartCounts.reduce((total, count) => total + count, 0), 14);
    assert.deepEqual(attempt8.projection.flatMap(item => item.outputPartTypes), Array(14).fill('input_text'));
    const attempt8Serialized = JSON.stringify({ result: attempt8.result, events: attempt8.events });
    for (const secret of ['SECRET_', 'raw-secret', 'attempt8-', 'create_time', 'content_item_kinds'])
      assert.equal(attempt8Serialized.includes(secret), false);
    const bounds = await run('[BOUNDS]'); assert.ifError(bounds.error);
    assert.deepEqual(bounds.result.runtimeAttestation.observed.nativeTools, { total: 0, tools: [] });

    for (const marker of ['[SERVICE-TIER-CURRENT]', '[SERVICE-TIER-DEFAULT]', '[SERVICE-TIER-PRIORITY]',
      '[SERVICE-TIER-FLEX]']) {
      const compatible = await run(marker); assert.ifError(compatible.error);
      assert.deepEqual(compatible.result.runtimeAttestation.observed.nativeTools, { total: 0, tools: [] });
      assert.equal(JSON.stringify(compatible.result).includes('service_tier'), false);
    }

    for (const marker of ['[UNKNOWN-MCP]', '[UNDECLARED]', '[MALFORMED]', '[UNKNOWN]', '[DUPLICATE]', '[OVERFLOW]']) {
      const rejected = await run(marker); assertFailure(rejected, 'native_event_grammar', 'raw_item_predicate');
      assert.deepEqual(rejected.events, []);
    }

    for (const marker of ['[LIVE-MISSING-SESSION]', '[LIVE-ORDER]', '[LIVE-DUPLICATE-SESSION]'])
      assertFailure(await run(marker), 'native_event_grammar', 'live_envelope_session', 'session_sequence');

    for (const marker of ['[LIVE-RAW-OBJECT]', '[LIVE-JSONRPC]', '[LIVE-PARAMS]', '[LIVE-META]', '[LIVE-MSG]',
      '[LIVE-RESPONSE-ID-MISSING]', '[LIVE-RESPONSE-ID-EXTRA]', '[LIVE-RESPONSE-ID-NUMBER]',
      '[LIVE-RESPONSE-ID-NULL]', '[LIVE-RESPONSE-ID-BOOLEAN]', '[LIVE-RESPONSE-ID-ARRAY]',
      '[LIVE-RESPONSE-ID-OBJECT]'])
      assertFailure(await run(marker), 'native_event_grammar', 'live_envelope_session', 'envelope_shape');

    assertFailure(await run('[CROSS]'), 'native_event_grammar', 'live_envelope_session', 'correlation', 'thread_id');
    for (const marker of ['[LIVE-RESPONSE-ID-OPAQUE]', '[LIVE-RESPONSE-ID-EMPTY]']) {
      const compatible = await run(marker); assert.ifError(compatible.error);
      assert.deepEqual(compatible.result.runtimeAttestation.observed.nativeTools,
        { total: 1, tools: [{ name: 'exec', status: 'completed', count: 1 }] });
      assert.deepEqual(compatible.result.runtimeAttestation.evidence,
        { sessionEventCount: 1, nativeCallCount: 1, probeMcpCallCount: 0 });
      assert.deepEqual(compatible.events, [{ name: 'exec', status: 'completed', count: 1 }]);
      const serialized = JSON.stringify({ result: compatible.result, events: compatible.events });
      for (const forbidden of ['opaque-safe-response-id', 'raw-secret', 'SECRET_'])
        assert.equal(serialized.includes(forbidden), false);
    }

    for (const [marker, predicate] of [
      ['[ATTEST-RESPONSE-ID]', 'response_id'], ['[LIVE-SESSION]', 'model'],
      ['[ATTEST-PERMISSION-TYPE]', 'permission_type'],
      ['[ATTEST-SESSION-SHAPE]', 'session_shape'], ['[SERVICE-TIER-INVALID]', 'session_shape'],
      ['[SESSION-UNKNOWN-EXTRA]', 'session_shape'], ['[ATTEST-IDENTITY]', 'session_identity'],
      ['[ATTEST-PERMISSION]', 'network'], ['[ATTEST-ROLLOUT]', 'rollout_path'], ['[ATTEST-CWD]', 'cwd']
    ]) assertFailure(await run(marker), 'native_event_grammar', 'live_envelope_session', 'attestation', null, predicate);
    for (const optional of ['forked_from_id', 'parent_thread_id', 'thread_source', 'thread_name',
      'active_permission_profile', 'initial_messages', 'network_proxy'])
      assertFailure(await run('[SESSION-OPTIONAL-' + optional + ']'), 'native_event_grammar',
        'live_envelope_session', 'attestation', null, 'session_shape');

    for (const marker of ['[DELTA-CREATE-NEGATIVE]', '[DELTA-CREATE-NONFINITE]', '[DELTA-CREATE-UNSAFE]',
      '[DELTA-KINDS-OVERFLOW]', '[DELTA-KINDS-WRONG]', '[DELTA-KIND-UNSAFE]', '[DELTA-KIND-OVERSIZED]',
      '[DELTA-PARTIAL]', '[DELTA-EXTRA]', '[DELTA-ID-UNSAFE]', '[DELTA-ID-OVERSIZED]', '[DELTA-DUP-ID]',
      '[DELTA-OUTPUT-DUP-ID]', '[COMMENTARY-ONLY]', '[DOUBLE-FINAL]', '[PHASE-UNKNOWN]', '[OVERFLOW-MESSAGES]'])
      assertFailure(await run(marker), 'native_event_grammar', 'raw_item_predicate');

    assertFailure(await run('[PROVIDER-ERROR]'), 'provider_engine');
    assertFailure(await run('[AMBIGUOUS]'), 'unknown');
    assert.equal(governedAnswerFailure('native_event_grammar').nativeEventFailureBoundary, null);
    assert.equal(governedAnswerFailure('native_event_grammar', ['raw_item_predicate', 'live_envelope_session'])
      .nativeEventFailureBoundary, null);
    assert.equal(governedAnswerFailure('native_event_grammar', 'raw_item_predicate|live_envelope_session')
      .nativeEventFailureBoundary, null);
    assert.equal(governedAnswerFailure('native_event_grammar', 'future_boundary').nativeEventFailureBoundary, null);
    assert.equal(JSON.stringify(governedAnswerFailure('native_event_grammar', 'SECRET_raw_item_predicate'))
      .includes('SECRET_'), false);
    for (const invalid of [['session_sequence', 'envelope_shape'], 'session_sequence|envelope_shape',
      'future_subreason', 'SECRET_session_sequence']) {
      const failure = governedAnswerFailure('native_event_grammar', 'live_envelope_session', invalid);
      assert.equal(failure.nativeEventFailureSubreason, null);
      assert.deepEqual(Object.keys(failure),
        ['answerFailureStage', 'nativeEventFailureBoundary', 'nativeEventFailureSubreason']);
      assert.equal(JSON.stringify(failure).includes('SECRET_'), false);
    }
    for (const subreason of ['session_sequence', 'envelope_shape', 'correlation', 'attestation'])
      assert.equal(governedAnswerFailure('native_event_grammar', 'live_envelope_session', subreason)
        .nativeEventFailureSubreason, subreason);
    for (const operand of ['thread_id', 'response_id']) {
      const failure = governedAnswerFailure('native_event_grammar', 'live_envelope_session', 'correlation', operand);
      assert.equal(failure.nativeEventFailureCorrelationOperand, operand);
      assert.deepEqual(Object.keys(failure), ['answerFailureStage', 'nativeEventFailureBoundary',
        'nativeEventFailureSubreason', 'nativeEventFailureCorrelationOperand']);
      assert.equal(Object.isFrozen(failure), true);
      assert.deepEqual(Object.getOwnPropertySymbols(failure), []);
      assert.equal(failure.message, ''); assert.equal(failure.stack, undefined);
      assert.equal(Object.hasOwn(failure, 'cause'), false);
    }
    for (const invalid of [['thread_id'], { operand: 'thread_id' }, 'thread_id|response_id', 'future_operand',
      'x'.repeat(129), 'SECRET_thread_id']) {
      const failure = governedAnswerFailure('native_event_grammar', 'live_envelope_session', 'correlation', invalid);
      assert.equal(Object.hasOwn(failure, 'nativeEventFailureCorrelationOperand'), true);
      assert.equal(failure.nativeEventFailureCorrelationOperand, null);
      assert.equal(JSON.stringify(failure).includes('SECRET_'), false);
    }
    assert.equal(governedAnswerFailure('native_event_grammar', 'live_envelope_session', 'correlation')
      .nativeEventFailureCorrelationOperand, null);
    assert.equal(Object.hasOwn(governedAnswerFailure('native_event_grammar', 'raw_item_predicate', 'correlation'),
      'nativeEventFailureSubreason'), false);
    assert.equal(Object.hasOwn(governedAnswerFailure('provider_engine', 'live_envelope_session', 'attestation'),
      'nativeEventFailureSubreason'), false);
    for (const failure of [
      governedAnswerFailure('native_event_grammar', 'raw_item_predicate', 'correlation', 'thread_id'),
      governedAnswerFailure('native_event_grammar', 'live_envelope_session', 'attestation', 'response_id'),
      governedAnswerFailure('provider_engine', 'live_envelope_session', 'correlation', 'thread_id')
    ]) assert.equal(Object.hasOwn(failure, 'nativeEventFailureCorrelationOperand'), false);
    const classified = governedAnswerFailure('native_event_grammar', 'live_envelope_session', 'session_sequence');
    assert.equal(Object.hasOwn(classified, 'nativeEventFailureCorrelationOperand'), false);
    assert.equal(normalizeGovernedAnswerFailure(classified, 'native_event_grammar',
      'live_envelope_session', 'attestation'), classified);
    const sanitized = normalizeGovernedAnswerFailure(new Error('SECRET_ATTESTATION_BODY'),
      'native_event_grammar', 'live_envelope_session', 'attestation');
    assert.equal(sanitized.nativeEventFailureSubreason, 'attestation');
    assert.equal(sanitized.nativeEventFailureAttestationPredicate, null);
    assert.equal(JSON.stringify(sanitized).includes('SECRET_'), false);
    const predicates = ['event_shape', 'jsonrpc', 'params_shape', 'response_id', 'meta_shape', 'session_shape',
      'session_identity', 'model', 'model_provider', 'approval_policy', 'approvals_reviewer',
      'reasoning_effort', 'rollout_path', 'cwd', 'permission_shape', 'session_type', 'permission_type',
      'network', 'filesystem_shape',
      'filesystem_type', 'entries', 'entry', 'access', 'path_shape', 'path_type', 'value_shape', 'kind',
      'native_tool_evidence', 'internal_contract'];
    for (const predicate of predicates) {
      const failure = governedAnswerFailure('native_event_grammar', 'live_envelope_session', 'attestation', null,
        predicate);
      assert.equal(failure.nativeEventFailureAttestationPredicate, predicate);
      assert.deepEqual(Object.keys(failure), ['answerFailureStage', 'nativeEventFailureBoundary',
        'nativeEventFailureSubreason', 'nativeEventFailureAttestationPredicate']);
      assert.equal(failure.message, ''); assert.equal(failure.stack, undefined);
      assert.equal(Object.hasOwn(failure, 'cause'), false);
    }
    for (const unknown of [new TypeError('Invalid future predicate'), new Error('Invalid msg.model'),
      new TypeError('SECRET_Invalid msg.model'), { message: 'Invalid msg.model' }]) {
      const failure = normalizeGovernedAnswerFailure(unknown, 'native_event_grammar',
        'live_envelope_session', 'attestation');
      assert.equal(failure.nativeEventFailureAttestationPredicate, null);
      assert.equal(JSON.stringify(failure).includes('SECRET_'), false);
    }
    const exactMappings = [
      ['event', 'event_shape'], ['event.method', 'event_shape'], ['event.jsonrpc', 'jsonrpc'],
      ['event.params', 'params_shape'], ['event.params.id', 'response_id'], ['event._meta', 'meta_shape'],
      ['requestId', 'meta_shape'], ['event.msg', 'session_shape'], ['session identity', 'session_identity'],
      ['msg.model', 'model'], ['msg.model_provider_id', 'model_provider'],
      ['msg.approval_policy', 'approval_policy'], ['msg.approvals_reviewer', 'approvals_reviewer'],
      ['msg.reasoning_effort', 'reasoning_effort'], ['rollout_path', 'rollout_path'], ['msg.cwd', 'cwd'],
      ['cwd', 'cwd'], ['permission_profile', 'permission_shape'],
      ['permission_profile.type', 'permission_type'], ['permission_profile.network', 'network'],
      ['file_system', 'filesystem_shape'], ['file_system.type', 'filesystem_type'],
      ['file_system.entries', 'entries'], ['file_system entry', 'entry'],
      ['file_system entry access', 'access'], ['permission path', 'path_shape'],
      ['permission path type', 'path_type'], ['permission path value', 'value_shape'],
      ['permission path kind', 'kind'], ['msg.type', 'session_type'],
      ['native tool evidence', 'native_tool_evidence'], ['native tool total', 'native_tool_evidence'],
      ['native tool aggregates', 'native_tool_evidence'], ['native tool aggregate', 'native_tool_evidence'],
      ['undeclared native tool evidence', 'native_tool_evidence'],
      ['native tool status', 'native_tool_evidence'], ['native tool count', 'native_tool_evidence'],
      ['attester input', 'internal_contract'], ['events', 'internal_contract'],
      ['canonical JSON value', 'internal_contract'], ['profile', 'internal_contract'],
      ['profile.version', 'internal_contract'], ['profile.profileId', 'internal_contract'],
      ['profile.engine', 'internal_contract'], ['profile.model', 'internal_contract'],
      ['profile.reasoningEffort', 'internal_contract'], ['profile.sandbox', 'internal_contract'],
      ['profile.approvalPolicy', 'internal_contract'], ['profile.fallback', 'internal_contract'],
      ['profile.retries', 'internal_contract'], ['profile.probeTools', 'internal_contract'],
      ['profile.probeTools[0]', 'internal_contract'], ['profile.probeTools[1]', 'internal_contract'],
      ['profile.probeTools[2]', 'internal_contract'], ['profile.probeMcpTools', 'internal_contract'],
      ['profile.probeMcpTools[0]', 'internal_contract'], ['profile.probeMcpTools[1]', 'internal_contract'],
      ['profile.probeMcpTools[2]', 'internal_contract'], ['profile.codexNativeTools', 'internal_contract'],
      ['profile.codexNativeTools[0]', 'internal_contract'], ['profile capability overlap', 'internal_contract']
    ];
    assert.equal(exactMappings.length, 61);
    assert.equal(new Set(exactMappings.map(([label]) => label)).size, 61);
    for (const [label, predicate] of exactMappings) {
      const classified = normalizeGovernedAnswerFailure(new TypeError(`Invalid ${label}`),
        'native_event_grammar', 'live_envelope_session', 'attestation');
      assert.equal(classified.nativeEventFailureAttestationPredicate, predicate);
    }
    const failureSource = await readFile(new URL('../../src/agent/engines/governed-answer-failure.js',
      import.meta.url), 'utf8');
    assert.equal((failureSource.match(/Object\.defineProperty\(this, 'nativeEventFailureAttestationPredicate'/g)
      ?? []).length, 1);
    const mapStart = failureSource.indexOf('const GOVERNED_ATTESTATION_ERROR_PREDICATES = new Map([');
    const mapEnd = failureSource.indexOf('\n]);', mapStart);
    assert.ok(mapStart >= 0 && mapEnd > mapStart);
    const productionLabels = [...failureSource.slice(mapStart, mapEnd)
      .matchAll(/\['Invalid ([^']+)', '[^']+'\]/g)].map((match) => match[1]);
    assert.equal(productionLabels.length, 61);
    assert.deepEqual(productionLabels, exactMappings.map(([label]) => label));
    const sanitizedCorrelation = normalizeGovernedAnswerFailure(new Error('SECRET_CORRELATION_BODY'),
      'native_event_grammar', 'live_envelope_session', 'correlation', 'response_id');
    assert.equal(sanitizedCorrelation.nativeEventFailureCorrelationOperand, 'response_id');
    assert.equal(JSON.stringify(sanitizedCorrelation).includes('SECRET_'), false);

    const invalidAnswer = await run('[BADJSON]');
    assertFailure(invalidAnswer, 'schema_result_validation', null, null, null, null, 'response_json');
    assert.deepEqual(invalidAnswer.events, [{ name: 'exec', status: 'completed', count: 1 }]);
    assertFailure(await run('[BADSCHEMA]'), 'schema_result_validation', null, null, null, null, 'schema_mismatch');
    assertFailure(await run('[SCHEMA-REQUIRED]'), 'schema_result_validation', null, null, null, null,
      'schema_mismatch');
    assertFailure(await run('[SCHEMA-EXTRA]'), 'schema_result_validation', null, null, null, null,
      'schema_mismatch');
    const enumSchema = JSON.stringify({ type: 'object', required: ['ok'], additionalProperties: false,
      properties: { ok: { enum: [true] } } });
    assertFailure(await run('[SCHEMA-ENUM]', { schema: enumSchema, invocationDigest: `sha256:${'0'.repeat(64)}` }),
      'schema_result_validation', null, null, null, null, 'schema_mismatch');
    assertFailure(await run('[ZERO]', { schema: '{invalid}', invocationDigest: `sha256:${'0'.repeat(64)}` }),
      'schema_result_validation', null, null, null, null, 'schema_definition');
    const invalidDefinition = JSON.stringify({ type: 'future-secret-type' });
    assertFailure(await run('[ZERO]', { schema: invalidDefinition, invocationDigest: `sha256:${'0'.repeat(64)}` }),
      'schema_result_validation', null, null, null, null, 'schema_definition');
    const numericSchema = JSON.stringify({ type: 'object', required: ['ok'], additionalProperties: false,
      properties: { ok: { type: 'number' } } });
    assertFailure(await run('[NONCANONICAL]', { schema: numericSchema, invocationDigest: `sha256:${'0'.repeat(64)}`,
      resultIdentity: 'probe.governed-result-identity/v1' }), 'schema_result_validation', null, null, null, null,
      'result_identity');

    for (const value of [null, 'future-secret-value']) {
      const sanitized = governedAnswerFailure('schema_result_validation', null, null, null, null, value);
      assert.equal(sanitized.schemaResultValidationSubreason, null);
      assert.equal(JSON.stringify(sanitized).includes('secret'), false);
    }
    for (const stage of ['native_event_grammar', 'provider_engine', 'unknown'])
      assert.equal(Object.hasOwn(governedAnswerFailure(stage, null, null, null, null, 'response_json'),
        'schemaResultValidationSubreason'), false);
    const schemaFailureSource = await readFile(new URL('../../src/agent/engines/governed-answer-failure.js',
      import.meta.url), 'utf8');
    assert.equal((schemaFailureSource.match(/Object\.defineProperty\(this, 'schemaResultValidationSubreason'/g) ?? []).length, 1);
    const subreasonSetStart = schemaFailureSource.indexOf('const GOVERNED_SCHEMA_RESULT_VALIDATION_SUBREASONS');
    const subreasonSetEnd = schemaFailureSource.indexOf('\n]);', subreasonSetStart);
    const productionSubreasons = [...schemaFailureSource.slice(subreasonSetStart, subreasonSetEnd)
      .matchAll(/'([a-z_]+)'/g)].map((match) => match[1]);
    assert.deepEqual(productionSubreasons,
      ['response_json', 'schema_definition', 'schema_mismatch', 'result_identity']);
    assert.equal(normalizeGovernedAnswerFailure(new Error('SECRET_NORMALIZE_BODY'),
      'schema_result_validation').schemaResultValidationSubreason, null);
    const agentSource = await readFile(new URL('../../src/agent/ProbeAgent.js', import.meta.url), 'utf8');
    const governedStart = agentSource.indexOf('  async answerGoverned(message');
    const governedEnd = agentSource.indexOf('\n  /**\n   * Answer a question', governedStart);
    const governedSource = agentSource.slice(governedStart, governedEnd);
    assert.equal((governedSource.match(/governedSchemaResultValidationFailure\(validation\)/g) ?? []).length, 1);
    assert.equal((governedSource.match(/'result_identity'/g) ?? []).length, 1);
    assert.equal((governedSource.match(/await engine\.close\(\)/g) ?? []).length, 1);
    for (const forbidden of ['schemaErrors', 'formattedErrors', 'errorSummary', 'schemaError'])
      assert.equal(governedSource.includes(forbidden), false);
  } finally {
    process.env.PATH = priorPath;
    if (priorPid === undefined) delete process.env.PROBE_NATIVE_PID_FILE; else process.env.PROBE_NATIVE_PID_FILE = priorPid;
    if (priorArgs === undefined) delete process.env.PROBE_NATIVE_ARGS_FILE; else process.env.PROBE_NATIVE_ARGS_FILE = priorArgs;
    if (priorProjection === undefined) delete process.env.PROBE_NATIVE_PROJECTION_FILE;
    else process.env.PROBE_NATIVE_PROJECTION_FILE = priorProjection;
    await rm(root, { recursive: true, force: true });
  }
});
