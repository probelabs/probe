import { describe, expect, beforeEach, afterEach, jest, test } from '@jest/globals';
import { EventEmitter } from 'events';
import { createHash } from 'crypto';
import { mkdtempSync, readFileSync, symlinkSync, writeFileSync, chmodSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';

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
  createCodexEngine,
  preflightCodexHome,
  validateCodexBindings,
  buildCodexEnvironment,
  CODEX_MODEL,
  CODEX_REASONING_EFFORT,
  CODEX_STDERR_MAX_BYTES
} = await import('../../src/agent/engines/codex.js');

const cwd = process.cwd();
const executablePath = process.execPath;
const executableSha256 = createHash('sha256').update(readFileSync(executablePath)).digest('hex');
const fixturePath = new URL('../fixtures/codex-attempt004-golden.jsonl', import.meta.url);
const fixture = readFileSync(fixturePath, 'utf8').trim().split('\n').map(line => JSON.parse(line));

let processMock;
let readerMock;
let serverMock;
let codexHome;
let authTarget;
let engines;

function governedAgent(overrides = {}) {
  const allowed = ['search', 'extract', 'listFiles'];
  return {
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
    toolImplementations: { search: {}, extract: {}, listFiles: {} },
    allowedTools: { mode: 'whitelist', allowed, exclusions: [], isEnabled: name => allowed.includes(name) },
    ...overrides
  };
}

function makeProcess() {
  const child = new EventEmitter();
  child.stdin = { write: jest.fn() };
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  child.killed = false;
  child.kill = jest.fn(signal => {
    child.killed = true;
    child.emit('exit', 0, signal);
    child.emit('close', 0, signal);
  });
  return child;
}

function substitute(value, engine) {
  const text = JSON.stringify(value)
    .replaceAll('CWD', cwd)
    .replaceAll('SERVER_NAME', `probe_${engine.sessionId}`)
    .replaceAll('THREAD_ID', 'thread-1')
    .replaceAll('TURN_ID', 'turn-1');
  return JSON.parse(text);
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
    codexExecutablePath: executablePath,
    codexExpectedExecutablePath: executablePath,
    codexExpectedExecutableSha256: executableSha256,
    ...overrides
  };
}

async function startEngine(overrides = {}) {
  const creating = createCodexEngine(options(overrides));
  await flush();
  emit(fixture[0]);
  const engine = await creating;
  engines.push(engine);
  return engine;
}

async function collect(engine) {
  const chunks = [];
  for await (const chunk of engine.query('BASELINE_OK')) chunks.push(chunk);
  return chunks;
}

async function replayGolden(engine, mutation = message => message) {
  const pending = collect(engine);
  await flush();
  const messages = fixture.slice(1).map(message => substitute(message, engine)).map(mutation);
  for (const message of messages) emit(message);
  jest.advanceTimersByTime(1500);
  await flush();
  return pending;
}

beforeEach(() => {
  jest.useFakeTimers();
  engines = [];
  codexHome = mkdtempSync(join(tmpdir(), 'probe-codex-home-'));
  authTarget = join(tmpdir(), `probe-auth-${process.pid}-${Math.random().toString(16).slice(2)}`);
  writeFileSync(authTarget, '{}');
  symlinkSync(authTarget, join(codexHome, 'auth.json'));
  processMock = makeProcess();
  readerMock = new EventEmitter();
  readerMock.close = jest.fn();
  serverMock = {
    start: jest.fn(async () => ({ host: '127.0.0.1', port: 43123 })),
    stop: jest.fn(async () => {})
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
  try { rmSync(codexHome, { recursive: true, force: true }); } catch {}
  try { rmSync(authTarget, { force: true }); } catch {}
});

describe('Codex 0.144.1 capture-governed attempt004 transport', () => {
  test('pins bindings, private home, and minimal child environment', () => {
    expect(() => validateCodexBindings({ model: 'other', thinkingEffort: CODEX_REASONING_EFFORT, cwd })).toThrow(/model/);
    expect(() => validateCodexBindings({ model: CODEX_MODEL, thinkingEffort: CODEX_REASONING_EFFORT, cwd, codexMcpTimeout: 0 })).toThrow(/timeout/);
    expect(buildCodexEnvironment('/private/codex', { PATH: '/bin', OPENAI_API_KEY: 'secret' })).toEqual({
      HOME: '/private/codex', CODEX_HOME: '/private/codex', PATH: '/bin'
    });
    expect(preflightCodexHome(codexHome, cwd)).toMatchObject({
      entries: ['auth.json'], auth: { type: 'symlink', targetValidated: true }, projectConfig: { present: false }
    });
  });

  test('replays the checked-in golden wire capture and gates success on cleanup', async () => {
    const engine = await startEngine();
    const pending = replayGolden(engine);
    const chunks = await pending;
    expect(chunks.map(chunk => chunk.type)).toEqual(['text', 'metadata']);
    expect(chunks[0].content).toBe('BASELINE_OK');
    expect(chunks[1].data.codexEventReceipt).toMatchObject({
      effective: expect.objectContaining({ thread_id: 'thread-1', model: CODEX_MODEL, approvals_reviewer: 'user' }),
      cleanup: expect.objectContaining({ status: 'succeeded' }),
      policyVerdict: { verdict: 'allow' }
    });
    expect(readerMock.close).toHaveBeenCalledTimes(1);
    expect(serverMock.stop).toHaveBeenCalledTimes(1);
    expect(processMock.kill).toHaveBeenCalledWith('SIGTERM');
  });

  test('sends exactly initialize then tools/call and no notification', async () => {
    const engine = await startEngine();
    const pending = replayGolden(engine);
    const writes = processMock.stdin.write.mock.calls.map(([line]) => JSON.parse(line));
    expect(writes).toHaveLength(2);
    expect(writes[0]).toEqual({
      jsonrpc: '2.0', id: 1, method: 'initialize',
      params: { protocolVersion: '2024-11-05', capabilities: { tools: {} }, clientInfo: { name: 'protocol-capture-r4', version: '1.0.0' } }
    });
    expect(writes[1]).toMatchObject({ jsonrpc: '2.0', id: 2, method: 'tools/call', params: { name: 'codex' } });
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
    ['result before task complete', message => message.id === 2 ? message : message.params?.msg?.type === 'task_complete'
      ? { jsonrpc: '2.0', id: 2, result: { content: [{ type: 'text', text: 'BASELINE_OK' }], structuredContent: { threadId: 'thread-1', content: 'BASELINE_OK' } } } : message]
  ])('fails closed for %s', async (_name, mutation) => {
    const engine = await startEngine();
    await expect(replayGolden(engine, mutation)).rejects.toThrow(/Codex/);
    expect(engine.getTransportState().poisoned).toBe(true);
  });

  test('accepts optional exact active_permission_profile and rejects extensions', async () => {
    const good = await startEngine();
    const goodPending = replayGolden(good, message => {
      if (message.params?.msg?.type !== 'session_configured') return message;
      return { ...message, params: { ...message.params, msg: { ...message.params.msg, active_permission_profile: { id: ':read-only' } } } };
    });
    await expect(goodPending).resolves.toEqual(expect.arrayContaining([expect.objectContaining({ type: 'text' })]));

    const bad = await startEngine();
    await expect(replayGolden(bad, message => message.params?.msg?.type === 'session_configured'
      ? { ...message, params: { ...message.params, msg: { ...message.params.msg, active_permission_profile: { id: ':read-only', extends: ':workspace' } } } } : message)).rejects.toThrow(/active_permission_profile/);
  });

  test('requires matching MCP begin/end, duration, terminal Ok, and canonical arguments', async () => {
    const engine = await startEngine();
    const invocation = { server: `probe_${engine.sessionId}`, tool: 'mcp__probe__search', arguments: { query: 'governed' } };
    const begin = { jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'thread-1' }, id: 'turn-1', msg: { type: 'mcp_tool_call_begin', call_id: 'call-1', invocation } } };
    const end = { jsonrpc: '2.0', method: 'codex/event', params: { _meta: { requestId: 2, threadId: 'thread-1' }, id: 'turn-1', msg: { type: 'mcp_tool_call_end', call_id: 'call-1', invocation: { ...invocation, arguments: { query: 'governed' } }, duration_ms: 4, result: { Ok: { content: [{ type: 'text', text: 'result' }] } } } } };
    const pending = collect(engine);
    await flush();
    const messages = fixture.slice(1).map(message => substitute(message, engine));
    for (const message of messages) {
      emit(message);
      if (message.params?.msg?.type === 'task_started') { emit(begin); emit(end); }
    }
    jest.advanceTimersByTime(1500);
    await expect(pending).resolves.toEqual(expect.arrayContaining([expect.objectContaining({ type: 'text', content: 'BASELINE_OK' })]));
  });

  test('isolates configuration before BuiltInMCPServer or spawn side effects', async () => {
    writeFileSync(join(codexHome, 'config.toml'), 'server = "ambient"');
    await expect(createCodexEngine(options())).rejects.toThrow(/codexHome|fresh/);
    expect(BuiltInMCPServerMock).not.toHaveBeenCalled();
    expect(spawnMock).not.toHaveBeenCalled();
  });

  test('keeps stderr bounded and second query cannot write', async () => {
    const engine = await startEngine();
    processMock.stderr.emit('data', Buffer.alloc(CODEX_STDERR_MAX_BYTES + 100, 0x61));
    expect(engine.getTransportState().stderrBytes).toBe(CODEX_STDERR_MAX_BYTES);
    const pending = replayGolden(engine);
    await expect(engine.query('SECOND').next()).rejects.toThrow(/exactly one query/);
    await pending;
    expect(processMock.stdin.write).toHaveBeenCalledTimes(2);
  });
});
