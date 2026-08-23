import { describe, expect, test, beforeEach, afterEach, jest } from '@jest/globals';
import { EventEmitter } from 'events';
import { createHash } from 'crypto';
import { mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { markProbeAgentForTests } from '../../src/agent/governance-marker.js';

const spawnMock = jest.fn();
const createInterfaceMock = jest.fn();
const BuiltInMCPServerMock = jest.fn();
const builtInServerModule = new URL('../../src/agent/mcp/built-in-server.js', import.meta.url).pathname;

jest.unstable_mockModule('child_process', async () => ({
  ...(await import('node:child_process')),
  spawn: spawnMock
}));
jest.unstable_mockModule('readline', () => ({ createInterface: createInterfaceMock }));
jest.unstable_mockModule(builtInServerModule, () => ({ BuiltInMCPServer: BuiltInMCPServerMock }));

const {
  buildCodexEnvironment,
  CODEX_EXTENSION_SANDBOX,
  CODEX_MODEL,
  CODEX_PINNED_EXECUTABLE_PATH,
  CODEX_PINNED_EXECUTABLE_SHA256,
  CODEX_REASONING_EFFORT,
  CODEX_STDERR_MAX_BYTES,
  createCodexEngine,
  preflightCodexHome,
  validateCodexBindings
} = await import('../../src/agent/engines/codex.js');

const fixturePath = new URL('../fixtures/codex-attempt004-golden.jsonl', import.meta.url);
const fixture = readFileSync(fixturePath, 'utf8').trim().split('\n').map(line => JSON.parse(line));

let cwd;
let codexHome;
let authTarget;
let processMock;
let readerMock;
let serverMock;
let engines;

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

function digest(value) {
  const serialized = canonicalJson(value);
  return { sha256: createHash('sha256').update(serialized).digest('hex'), bytes: Buffer.byteLength(serialized, 'utf8') };
}

function governedAgent(overrides = {}) {
  const allowed = ['search', 'extract', 'listFiles'];
  const agent = {
    fallbackConfig: false,
    enableDelegate: false,
    enableExecutePlan: false,
    searchDelegate: false,
    allowEdit: false,
    enableBash: false,
    enableSkills: false,
    enableTasks: false,
    enableMcp: false,
    disableMermaidValidation: true,
    disableJsonValidation: true,
    completionPrompt: null,
    mcpConfig: null,
    mcpConfigPath: null,
    mcpServers: null,
    mcpBridge: null,
    maxIterations: null,
    allowedTools: { mode: 'whitelist', allowed, exclusions: [], isEnabled: name => allowed.includes(name) },
    toolImplementations: {
      search: { execute: async args => ({ tool: 'search', args }) },
      extract: { execute: async args => ({ tool: 'extract', args }) },
      listFiles: { execute: async args => ({ tool: 'listFiles', args }) }
    },
    ...overrides
  };
  return markProbeAgentForTests(agent);
}

function makeProcess() {
  const child = new EventEmitter();
  child.stdin = { write: jest.fn() };
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  child.kill = jest.fn(signal => {
    child.emit('exit', 0, signal);
    child.emit('close', 0, signal);
  });
  return child;
}

function substitute(value, engine) {
  const text = JSON.stringify(value)
    .replaceAll('CWD', cwd)
    .replaceAll('CODEX_HOME', codexHome)
    .replaceAll('SERVER_NAME', `probe_${engine.sessionId}`)
    .replaceAll('THREAD_ID', 'thread-1')
    .replaceAll('TURN_ID', 'turn-1')
    .replaceAll('USER_ITEM_ID', 'user-item-1')
    .replaceAll('REASONING_ITEM_ID_1', 'reasoning-item-1')
    .replaceAll('REASONING_ITEM_ID_2', 'reasoning-item-2')
    .replaceAll('REASONING_ITEM_ID_3', 'reasoning-item-3')
    .replaceAll('DISCOVERY_OUTER_ITEM_ID', 'discovery-outer-item-1')
    .replaceAll('TOOL_OUTER_ITEM_ID', 'tool-outer-item-1')
    .replaceAll('DISCOVERY_OUTER_CALL_ID', 'discovery-outer-call-1')
    .replaceAll('TOOL_OUTER_CALL_ID', 'tool-outer-call-1')
    .replaceAll('NESTED_CALL_ID', 'nested-call-1')
    .replaceAll('AGENT_ITEM_ID', 'agent-item-1');
  return JSON.parse(text);
}

function makeAudit() {
  const result = { content: [{ type: 'text', text: '[\n  "fixture-alpha.txt",\n  "fixture-beta.txt"\n]' }] };
  return {
    starts: [{ host: '127.0.0.1', port: 43123, url_path: '/mcp' }],
    listCalls: [{
      ordinal: 1,
      tool_names: ['mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles'],
      result: { sha256: 'list-result', bytes: 1 }
    }],
    toolCalls: [{
      ordinal: 2,
      name: 'mcp__probe__listFiles',
      arguments: digest({ path: '.' }),
      metadata: {
        session_id: 'thread-1',
        thread_id: 'thread-1',
        turn_id: 'turn-1',
        sandbox: CODEX_EXTENSION_SANDBOX,
        turn_started_at_unix_ms: 1787445973467,
        model: CODEX_MODEL,
        reasoning_effort: CODEX_REASONING_EFFORT,
        threadId: 'thread-1',
        progressToken: 1
      },
      result: { ...digest(result), status: 'ok' }
    }],
    executionCounts: { search: 0, extract: 0, listFiles: 1 }
  };
}

function emit(message) {
  readerMock.emit('line', JSON.stringify(message));
}

async function flush() {
  await Promise.resolve();
  await Promise.resolve();
}

function options(overrides = {}) {
  return {
    agent: governedAgent(),
    model: CODEX_MODEL,
    thinkingEffort: CODEX_REASONING_EFFORT,
    cwd,
    sandbox: 'read-only',
    approvalPolicy: 'never',
    codexMcpTimeout: 1000,
    codexHome,
    codexExecutablePath: CODEX_PINNED_EXECUTABLE_PATH,
    codexExpectedExecutablePath: CODEX_PINNED_EXECUTABLE_PATH,
    codexExpectedExecutableSha256: CODEX_PINNED_EXECUTABLE_SHA256,
    sessionId: 'session-1',
    ...overrides
  };
}

async function startEngine(overrides = {}) {
  console.log('DEBUG start create');
  const creating = createCodexEngine(options(overrides));
  await flush();
  console.log('DEBUG start emit');
  emit(fixture[0]);
  console.log('DEBUG start awaiting');
  const engine = await creating;
  console.log('DEBUG start done');
  engines.push(engine);
  return engine;
}

async function collect(engine) {
  const chunks = [];
  for await (const chunk of engine.query('TOOL_PROMPT')) chunks.push(chunk);
  return chunks;
}

async function replayGolden(engine, mutation = message => message) {
  console.log('DEBUG replay start');
  const pending = collect(engine);
  await flush();
  console.log('DEBUG query started');
  const messages = fixture.slice(1).map(message => substitute(message, engine)).flatMap(message => {
    const mutated = mutation(message);
    return Array.isArray(mutated) ? mutated : [mutated];
  });
  for (const message of messages) emit(message);
  console.log('DEBUG emitted');
  await flush();
  console.log('DEBUG flushed', engine.getTransportState());
  jest.advanceTimersByTime(1500);
  await flush();
  console.log('DEBUG done replay');
  return pending;
}

beforeEach(() => {
  jest.useFakeTimers();
  engines = [];
  cwd = mkdtempSync(join(tmpdir(), 'probe-codex-cwd-'));
  codexHome = mkdtempSync(join(tmpdir(), 'probe-codex-home-'));
  authTarget = join(tmpdir(), `probe-auth-${process.pid}-${Math.random().toString(16).slice(2)}`);
  writeFileSync(authTarget, '{}');
  symlinkSync(authTarget, join(codexHome, 'auth.json'));
  processMock = makeProcess();
  readerMock = new EventEmitter();
  readerMock.close = jest.fn();
  serverMock = {
    audit: makeAudit(),
    start: jest.fn(async () => ({ host: '127.0.0.1', port: 43123 })),
    stop: jest.fn(async () => {}),
    getGovernedAuditSnapshot: jest.fn(() => serverMock.audit)
  };
  spawnMock.mockReset();
  spawnMock.mockReturnValue(processMock);
  createInterfaceMock.mockReset();
  createInterfaceMock.mockReturnValue(readerMock);
  BuiltInMCPServerMock.mockReset();
  BuiltInMCPServerMock.mockImplementation(() => serverMock);
});

afterEach(async () => {
  for (const engine of engines) {
    try { await engine.close(); } catch {}
  }
  jest.clearAllTimers();
  jest.useRealTimers();
  for (const path of [cwd, codexHome, authTarget]) {
    try { rmSync(path, { recursive: true, force: true }); } catch {}
  }
});

describe('Codex 0.144.1 capture-governed attempt004 transport', () => {
  test('pins bindings, private home, and isolated child environment', () => {
    expect(() => validateCodexBindings({ model: 'other', thinkingEffort: CODEX_REASONING_EFFORT, cwd })).toThrow(/model/);
    expect(() => validateCodexBindings({ model: CODEX_MODEL, thinkingEffort: CODEX_REASONING_EFFORT, cwd, codexMcpTimeout: 0 })).toThrow(/timeout/);
    expect(buildCodexEnvironment('/private/codex', { PATH: '/bin', OPENAI_API_KEY: 'secret' })).toEqual({
      HOME: '/private/codex', CODEX_HOME: '/private/codex', PATH: '/bin'
    });
    expect(preflightCodexHome(codexHome, cwd)).toMatchObject({
      entries: ['auth.json'], auth: { type: 'symlink', targetValidated: true }, projectConfig: { present: false }
    });
  });

  test('replays the authoritative golden wire capture and gates success on cleanup', async () => {
    const engine = await startEngine();
    const chunks = await replayGolden(engine);
    expect(chunks.map(chunk => chunk.type)).toEqual(['text', 'metadata']);
    expect(chunks[0].content).toBe('PROBE_TOOL_OK');
    const receipt = chunks[1].data.codexEventReceipt;
    expect(receipt.effective).toEqual(expect.objectContaining({ thread_id: 'thread-1', model: CODEX_MODEL }));
    expect(receipt.cleanup).toEqual(expect.objectContaining({ status: 'succeeded' }));
    expect(receipt.policyVerdict).toEqual({ verdict: 'allow' });
    expect(receipt.execBridge[1]).toEqual(expect.objectContaining({
      outerCallId: 'tool-outer-call-1', outerItemId: 'tool-outer-item-1', nestedCallId: 'nested-call-1', status: 'completed'
    }));
    expect(receipt.nestedMcp[0]).toEqual(expect.objectContaining({
      nestedCallId: 'nested-call-1', callId: 'nested-call-1', outerCallId: 'tool-outer-call-1',
      duration: { secs: 0, nanos: 2468667 }, directAudit: { ordinal: 2 }
    }));
    expect(receipt.execBridge[1].outerCallId).not.toBe(receipt.nestedMcp[0].nestedCallId);
    expect(readerMock.close).toHaveBeenCalledTimes(1);
    expect(serverMock.stop).toHaveBeenCalledTimes(1);
    expect(processMock.kill).toHaveBeenCalledWith('SIGTERM');
  });

  test('sends exactly initialize then tools/call and queries TOOL_PROMPT', async () => {
    const engine = await startEngine();
    const pending = replayGolden(engine);
    const writes = processMock.stdin.write.mock.calls.map(([line]) => JSON.parse(line));
    expect(writes).toHaveLength(2);
    expect(writes[0]).toEqual({
      jsonrpc: '2.0', id: 1, method: 'initialize',
      params: { protocolVersion: '2024-11-05', capabilities: { tools: {} }, clientInfo: { name: 'protocol-capture-r4', version: '1.0.0' } }
    });
    expect(writes[1]).toMatchObject({ jsonrpc: '2.0', id: 2, method: 'tools/call', params: { name: 'codex' } });
    expect(writes[1].params.arguments.prompt).toBe('TOOL_PROMPT');
    expect(writes[1].params.arguments.config.features.shell_tool).toBe(false);
    await pending;
  });

  test.each([
    ['turn mismatch', message => message.params?.msg?.type === 'task_started' ? { ...message, params: { ...message.params, id: 'wrong' } } : message],
    ['thread mismatch', message => message.params?.msg?.type === 'agent_message_content_delta' ? { ...message, params: { ...message.params, _meta: { ...message.params._meta, threadId: 'wrong' } } } : message],
    ['unknown method', message => message.params?.msg?.type === 'task_started' ? { ...message, method: 'codex/unknown' } : message],
    ['unknown item variant', message => message.params?.msg?.type === 'raw_response_item' && message.params.msg.item.role === 'developer'
      ? { ...message, params: { ...message.params, msg: { ...message.params.msg, item: { ...message.params.msg.item, role: 'system' } } } } : message],
    ['startup arrays', message => message.params?.msg?.type === 'mcp_startup_complete'
      ? { ...message, params: { ...message.params, msg: { ...message.params.msg, ready: [] } } } : message],
    ['result before task complete', message => message.params?.msg?.type === 'task_complete'
      ? { jsonrpc: '2.0', id: 2, result: { content: [{ type: 'text', text: 'PROBE_TOOL_OK' }], structuredContent: { threadId: 'thread-1', content: 'PROBE_TOOL_OK' } } } : message],
    ['invalid active permission profile', message => message.params?.msg?.type === 'session_configured'
      ? { ...message, params: { ...message.params, msg: { ...message.params.msg, active_permission_profile: { id: ':read-only' } } } } : message]
  ])('fails closed for %s', async (_name, mutation) => {
    const engine = await startEngine();
    await expect(replayGolden(engine, mutation)).rejects.toThrow(/Codex/);
    expect(engine.getTransportState().poisoned).toBe(true);
  });

  test.each([
    ['duration', message => message.params?.msg?.type === 'mcp_tool_call_end'
      ? { ...message, params: { ...message.params, msg: { ...message.params.msg, duration: { secs: 0, nanos: 2468668 } } } } : message],
    ['result', message => message.params?.msg?.type === 'item_completed' && message.params.msg.item?.type === 'McpToolCall'
      ? { ...message, params: { ...message.params, msg: { ...message.params.msg, item: { ...message.params.msg.item, result: { content: [{ type: 'text', text: '[mutated]' }] } } } } } : message],
    ['outer call ID', message => message.params?.msg?.item?.type === 'custom_tool_call' && message.params.msg.item.call_id === 'tool-outer-call-1'
      ? { ...message, params: { ...message.params, msg: { ...message.params.msg, item: { ...message.params.msg.item, call_id: 'wrong-outer-call' } } } } : message],
    ['outer call ID reuse', message => message.params?.msg?.item?.type === 'custom_tool_call' && message.params.msg.item.call_id === 'tool-outer-call-1'
      ? { ...message, params: { ...message.params, msg: { ...message.params.msg, item: { ...message.params.msg.item, call_id: 'discovery-outer-call-1' } } } } : message]
  ])('rejects mutated canonical/legacy %s', async (_name, mutation) => {
    const engine = await startEngine();
    await expect(replayGolden(engine, mutation)).rejects.toThrow(/Codex/);
  });

  test('rejects unrelated interleaving while an outer bridge is open', async () => {
    const engine = await startEngine();
    const interleaving = message => {
      if (message.params?.msg?.type !== 'raw_response_item' || message.params.msg.item?.type !== 'custom_tool_call' ||
          message.params.msg.item.call_id !== 'tool-outer-call-1') return message;
      return [message, substitute(fixture.find(candidate => candidate.params?.msg?.type === 'token_count'), engine)];
    };
    await expect(replayGolden(engine, interleaving)).rejects.toThrow(/Codex/);
  });

  test('rejects extra or unconsumed direct audits and execution count mismatches', async () => {
    const extra = await startEngine();
    extra.serverAudit = serverMock.audit;
    serverMock.audit.toolCalls.push({ ...serverMock.audit.toolCalls[0], ordinal: 3 });
    await expect(replayGolden(extra)).rejects.toThrow(/direct audit/);

    const mismatch = await startEngine();
    serverMock.audit.executionCounts.listFiles = 2;
    await expect(replayGolden(mismatch)).rejects.toThrow(/executionCounts/);
  });

  test('rejects direct audit metadata from another session', async () => {
    const engine = await startEngine();
    serverMock.audit.toolCalls[0].metadata.session_id = 'other-session';
    await expect(replayGolden(engine)).rejects.toThrow(/direct audit/);
  });

  test('keeps stderr bounded and rejects a second query', async () => {
    const engine = await startEngine();
    processMock.stderr.emit('data', Buffer.alloc(CODEX_STDERR_MAX_BYTES + 100, 0x61));
    expect(engine.getTransportState().stderrBytes).toBe(CODEX_STDERR_MAX_BYTES);
    const pending = replayGolden(engine);
    await expect(engine.query('SECOND').next()).rejects.toThrow(/exactly one query/);
    await pending;
    expect(processMock.stdin.write).toHaveBeenCalledTimes(2);
  });
});
