import { describe, expect, test, beforeEach, afterEach, jest } from '@jest/globals';
import { EventEmitter } from 'events';
import { createHash } from 'crypto';
import { mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'fs';
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
  CODEX_QUIET_WINDOW_MS,
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
let harnesses;

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

function governedToolListResult() {
  return {
    tools: [
      {
        name: 'mcp__probe__search',
        description: 'Search for code patterns using semantic search',
        inputSchema: {
          type: 'object',
          properties: {
            query: { type: 'string', description: 'Search query' },
            path: { type: 'string', description: 'Directory to search', default: '.' },
            maxResults: { type: 'integer', default: 10 }
          },
          required: ['query']
        }
      },
      {
        name: 'mcp__probe__extract',
        description: 'Extract code from specific file location',
        inputSchema: {
          type: 'object',
          properties: { path: { type: 'string', description: 'File path with optional line number' } },
          required: ['path']
        }
      },
      {
        name: 'mcp__probe__listFiles',
        description: 'List files in a directory',
        inputSchema: {
          type: 'object',
          properties: {
            path: { type: 'string', description: 'Directory path' },
            pattern: { type: 'string', description: 'File pattern' }
          },
          required: ['path']
        }
      }
    ]
  };
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

function makeHarness() {
  let initializeWrittenResolve;
  let queryWrittenResolve;
  let readerListenerResolve;
  const initializeWritten = new Promise(resolve => { initializeWrittenResolve = resolve; });
  const queryWritten = new Promise(resolve => { queryWrittenResolve = resolve; });
  const readerListenerInstalled = new Promise(resolve => { readerListenerResolve = resolve; });
  const child = makeProcess();
  child.stdin.write.mockImplementation(line => {
    const message = JSON.parse(line);
    if (message.id === 1) initializeWrittenResolve();
    if (message.id === 2) queryWrittenResolve();
  });
  const reader = new EventEmitter();
  reader.close = jest.fn();
  const originalOn = reader.on.bind(reader);
  reader.on = jest.fn((event, listener) => {
    const result = originalOn(event, listener);
    if (event === 'line') readerListenerResolve();
    return result;
  });
  const server = {
    audit: makeAudit(),
    start: jest.fn(async () => ({ host: '127.0.0.1', port: 43123 })),
    stop: jest.fn(async () => {}),
    getGovernedAuditSnapshot: jest.fn(() => server.audit)
  };
  const harness = { child, reader, server, initializeWritten, queryWritten, readerListenerInstalled };
  processMock = child;
  readerMock = reader;
  serverMock = server;
  spawnMock.mockReturnValue(child);
  createInterfaceMock.mockReturnValue(reader);
  BuiltInMCPServerMock.mockImplementation(() => server);
  return harness;
}

async function waitForQuietWindow(engine) {
  const maxPolls = 100;
  for (let poll = 0; poll < maxPolls; poll++) {
    const transport = engine.getTransportState();
    if (transport.quietWindowArmed) return;
    if (transport.poisoned) throw new Error('Codex transport poisoned while waiting for quiet window');
    await Promise.resolve();
  }
  throw new Error(`Codex quiet window did not arm after ${maxPolls} polls`);
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

function replaceMessage(message, msg) {
  return { ...message, params: { ...message.params, msg } };
}

function syntheticSecondToolLifecycle(engine) {
  const firstToolOuterCall = fixture.find(message => message.params?.msg?.type === 'raw_response_item' &&
    message.params.msg.item?.type === 'custom_tool_call' && message.params.msg.item.call_id === 'TOOL_OUTER_CALL_ID');
  const firstToolNestedStart = fixture.find(message => message.params?.msg?.type === 'item_started' &&
    message.params.msg.item?.type === 'McpToolCall' && message.params.msg.item.id === 'NESTED_CALL_ID');
  const firstToolBegin = fixture.find(message => message.params?.msg?.type === 'mcp_tool_call_begin' &&
    message.params.msg.call_id === 'NESTED_CALL_ID');
  const firstToolCompletion = fixture.find(message => message.params?.msg?.type === 'item_completed' &&
    message.params.msg.item?.type === 'McpToolCall' && message.params.msg.item.id === 'NESTED_CALL_ID');
  const firstToolEnd = fixture.find(message => message.params?.msg?.type === 'mcp_tool_call_end' &&
    message.params.msg.call_id === 'NESTED_CALL_ID');
  const firstToolOuterOutput = fixture.find(message => message.params?.msg?.type === 'raw_response_item' &&
    message.params.msg.item?.type === 'custom_tool_call_output' && message.params.msg.item.call_id === 'TOOL_OUTER_CALL_ID');
  const source = [firstToolOuterCall, firstToolNestedStart, firstToolBegin, firstToolCompletion, firstToolEnd, firstToolOuterOutput];
  if (source.some(message => !message)) throw new Error('Golden first-tool lifecycle is incomplete');

  const [outerCall, nestedStart, begin, completion, end, outerOutput] = source.map(message => substitute(message, engine));
  const argumentsValue = { query: 'handler', path: '.' };
  const resultValue = { content: [{ type: 'text', text: 'http.go:10: handler' }] };
  const duration = { secs: 0, nanos: 3000000 };

  return [
    replaceMessage(outerCall, {
      ...outerCall.params.msg.item,
      id: 'tool-outer-item-2',
      call_id: 'tool-outer-call-2',
      input: 'synthetic-bridge-input-2'
    }),
    replaceMessage(nestedStart, {
      ...nestedStart.params.msg.item,
      id: 'nested-call-2',
      tool: 'mcp__probe__search',
      arguments: argumentsValue
    }),
    replaceMessage(begin, {
      ...begin.params.msg,
      call_id: 'nested-call-2',
      invocation: { ...begin.params.msg.invocation, tool: 'mcp__probe__search', arguments: argumentsValue }
    }),
    replaceMessage(completion, {
      ...completion.params.msg,
      item: {
        ...completion.params.msg.item,
        id: 'nested-call-2',
        tool: 'mcp__probe__search',
        arguments: argumentsValue,
        result: resultValue,
        duration
      }
    }),
    replaceMessage(end, {
      ...end.params.msg,
      call_id: 'nested-call-2',
      invocation: { ...end.params.msg.invocation, tool: 'mcp__probe__search', arguments: argumentsValue },
      duration,
      result: { ...end.params.msg.result, Ok: resultValue }
    }),
    replaceMessage(outerOutput, {
      ...outerOutput.params.msg.item,
      call_id: 'tool-outer-call-2',
      output: [
        { type: 'input_text', text: 'synthetic-bridge-output-2-a' },
        { type: 'input_text', text: 'synthetic-bridge-output-2-b' }
      ]
    })
  ];
}

function makeAudit() {
  const result = { content: [{ type: 'text', text: '[\n  "fixture-alpha.txt",\n  "fixture-beta.txt"\n]' }] };
  return {
    starts: [{ host: '127.0.0.1', port: 43123, url_path: '/mcp' }],
    listCalls: [{
      ordinal: 1,
      tool_names: ['mcp__probe__search', 'mcp__probe__extract', 'mcp__probe__listFiles'],
      result: digest(governedToolListResult())
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

function emit(harness, message) {
  harness.reader.emit('line', JSON.stringify(message));
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
  const harness = makeHarness();
  const creating = createCodexEngine(options(overrides));
  await Promise.all([harness.initializeWritten, harness.readerListenerInstalled]);
  emit(harness, fixture[0]);
  const engine = await creating;
  harnesses.set(engine, harness);
  engines.push(engine);
  return engine;
}

async function collect(engine) {
  const chunks = [];
  for await (const chunk of engine.query('TOOL_PROMPT')) chunks.push(chunk);
  return chunks;
}

function createSecondAuditBeforeEmit() {
  let canonicalMcpCompletionCount = 0;
  return (message, harness) => {
    const item = message.params?.msg?.item;
    if (message.params?.msg?.type !== 'item_completed' || item?.type !== 'McpToolCall') return;
    canonicalMcpCompletionCount++;
    if (canonicalMcpCompletionCount !== 2) return;

    const argumentsValue = { query: 'handler', path: '.' };
    const resultValue = { content: [{ type: 'text', text: 'http.go:10: handler' }] };
    const firstRecord = harness.server.audit.toolCalls[0];
    harness.server.audit.toolCalls.push({
      ordinal: 3,
      name: 'mcp__probe__search',
      arguments: digest(argumentsValue),
      metadata: { ...firstRecord.metadata, progressToken: 2 },
      result: { ...digest(resultValue), status: 'ok' }
    });
    harness.server.audit.executionCounts = { search: 1, extract: 0, listFiles: 1 };
  };
}

async function replayGolden(engine, mutation = message => message, beforeEmit = createSecondAuditBeforeEmit()) {
  const harness = harnesses.get(engine);
  const pending = collect(engine);
  await harness.queryWritten;
  const goldenMessages = fixture.slice(1).map(message => substitute(message, engine));
  const firstToolOutputIndex = goldenMessages.findIndex(message => message.params?.msg?.type === 'raw_response_item' &&
    message.params.msg.item?.type === 'custom_tool_call_output' && message.params.msg.item.call_id === 'tool-outer-call-1');
  if (firstToolOutputIndex < 0) throw new Error('Golden first-tool output is missing');
  goldenMessages.splice(firstToolOutputIndex + 1, 0, ...syntheticSecondToolLifecycle(engine));
  const messages = goldenMessages.flatMap(message => {
    const mutated = mutation(message);
    return Array.isArray(mutated) ? mutated : [mutated];
  });
  for (const message of messages) {
    await beforeEmit(message, harness);
    emit(harness, message);
  }
  if (!engine.getTransportState().poisoned) {
    await waitForQuietWindow(engine);
    await jest.advanceTimersByTimeAsync(CODEX_QUIET_WINDOW_MS);
  }
  return pending;
}

beforeEach(() => {
  jest.useFakeTimers();
  engines = [];
  harnesses = new WeakMap();
  cwd = realpathSync(mkdtempSync(join(tmpdir(), 'probe-codex-cwd-')));
  codexHome = realpathSync(mkdtempSync(join(tmpdir(), 'probe-codex-home-')));
  authTarget = join(tmpdir(), `probe-auth-${process.pid}-${Math.random().toString(16).slice(2)}`);
  writeFileSync(authTarget, '{}');
  symlinkSync(authTarget, join(codexHome, 'auth.json'));
  spawnMock.mockReset();
  createInterfaceMock.mockReset();
  BuiltInMCPServerMock.mockReset();
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

  test('exposes successful late MCP cleanup after startup timeout and late resolve', async () => {
    const harness = makeHarness();
    let resolveStart;
    const lateStart = new Promise(resolve => { resolveStart = resolve; });
    harness.server.start.mockReturnValue(lateStart);

    const creating = createCodexEngine(options({ codexMcpTimeout: 25 }));
    const creatingError = creating.catch(failure => failure);
    await jest.advanceTimersByTimeAsync(25);
    const error = await creatingError;

    expect(error).toBeInstanceOf(Error);
    expect(error.message).toMatch(/MCP startup timeout/);
    expect(harness.server.stop).toHaveBeenCalledTimes(1);

    resolveStart({ host: '127.0.0.1', port: 43123 });
    await expect(error.codexMcpLateCleanup).resolves.toEqual(expect.objectContaining({ status: 'succeeded' }));
    expect(harness.server.stop).toHaveBeenCalledTimes(2);
  });

  test('replays the authoritative golden wire capture and gates success on cleanup', async () => {
    const engine = await startEngine();
    const harness = harnesses.get(engine);
    const chunks = await replayGolden(engine);
    expect(chunks.map(chunk => chunk.type)).toEqual(['text', 'metadata']);
    expect(chunks[0].content).toBe('PROBE_TOOL_OK');
    const receipt = chunks[1].data.codexEventReceipt;
    expect(receipt.effective).toEqual(expect.objectContaining({ thread_id: 'thread-1', model: CODEX_MODEL }));
    expect(receipt.cleanup).toEqual(expect.objectContaining({ status: 'succeeded' }));
    expect(receipt.policyVerdict).toEqual({ verdict: 'allow' });
    expect(receipt.counts).toEqual(expect.objectContaining({ bridges: 3, nestedMcp: 2, directAudits: 2 }));
    expect(receipt.execBridge.map(({ outerCallId, outerItemId, nestedCallId, nestedItemId, status }) => ({
      outerCallId, outerItemId, nestedCallId, nestedItemId, status
    }))).toEqual([
      {
        outerCallId: 'discovery-outer-call-1', outerItemId: 'discovery-outer-item-1',
        nestedCallId: null, nestedItemId: null, status: 'completed'
      },
      {
        outerCallId: 'tool-outer-call-1', outerItemId: 'tool-outer-item-1',
        nestedCallId: 'nested-call-1', nestedItemId: 'nested-call-1', status: 'completed'
      },
      {
        outerCallId: 'tool-outer-call-2', outerItemId: 'tool-outer-item-2',
        nestedCallId: 'nested-call-2', nestedItemId: 'nested-call-2', status: 'completed'
      }
    ]);
    expect(receipt.nestedMcp.map(({ nestedCallId, callId, outerCallId, tool, duration }) => ({
      nestedCallId, callId, outerCallId, tool, duration
    }))).toEqual([
      {
        nestedCallId: 'nested-call-1', callId: 'nested-call-1', outerCallId: 'tool-outer-call-1',
        tool: 'mcp__probe__listFiles', duration: { secs: 0, nanos: 2468667 }
      },
      {
        nestedCallId: 'nested-call-2', callId: 'nested-call-2', outerCallId: 'tool-outer-call-2',
        tool: 'mcp__probe__search', duration: { secs: 0, nanos: 3000000 }
      }
    ]);
    expect(receipt.directServerAudit.records.map(record => record.ordinal)).toEqual([2, 3]);
    expect(receipt.directServerAudit.records.map(record => record.name)).toEqual([
      'mcp__probe__listFiles', 'mcp__probe__search'
    ]);
    expect(receipt.directServerAudit.records.map(record => record.metadata.progressToken)).toEqual([1, 2]);
    expect(receipt.nestedMcp.map(record => record.directAudit.metadata.progressToken)).toEqual([1, 2]);
    expect(receipt.eventCounts).toEqual({
      session_configured: 1,
      mcp_startup_update: 2,
      task_started: 1,
      mcp_startup_complete: 1,
      raw_response_item: 13,
      item_started: 7,
      item_completed: 7,
      user_message: 1,
      token_count: 3,
      mcp_tool_call_begin: 2,
      mcp_tool_call_end: 2,
      agent_message_content_delta: 4,
      agent_message: 1,
      task_complete: 1,
      result: 1
    });
    const bridgeIdentities = receipt.execBridge.flatMap(({ outerCallId, outerItemId, nestedCallId }) =>
      [outerCallId, outerItemId, nestedCallId].filter(Boolean));
    expect(new Set(bridgeIdentities).size).toBe(bridgeIdentities.length);
    expect(receipt.directServerAudit.records[1].arguments).toEqual(digest({ query: 'handler', path: '.' }));
    expect(receipt.directServerAudit.records[1].result).toEqual({
      ...digest({ content: [{ type: 'text', text: 'http.go:10: handler' }] }), status: 'ok'
    });
    expect(harness.reader.close).toHaveBeenCalledTimes(1);
    expect(harness.server.stop).toHaveBeenCalledTimes(1);
    expect(harness.child.kill).toHaveBeenCalledWith('SIGTERM');
  });

  test('sends exactly initialize then tools/call and queries TOOL_PROMPT', async () => {
    const engine = await startEngine();
    const harness = harnesses.get(engine);
    const pending = replayGolden(engine);
    const writes = harness.child.stdin.write.mock.calls.map(([line]) => JSON.parse(line));
    expect(writes).toHaveLength(2);
    expect(writes[0]).toEqual({
      jsonrpc: '2.0', id: 1, method: 'initialize',
      params: { protocolVersion: '2024-11-05', capabilities: { tools: {} }, clientInfo: { name: 'protocol-capture-r4-tool', version: '1.0.0' } }
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
    const extraHarness = harnesses.get(extra);
    extra.serverAudit = extraHarness.server.audit;
    extraHarness.server.audit.toolCalls.push({ ...extraHarness.server.audit.toolCalls[0], ordinal: 3 });
    await expect(replayGolden(extra)).rejects.toThrow(/direct audit/);

    const mismatch = await startEngine();
    const mismatchHarness = harnesses.get(mismatch);
    mismatchHarness.server.audit.executionCounts.listFiles = 2;
    await expect(replayGolden(mismatch)).rejects.toThrow(/executionCounts/);
  });

  test('rejects direct audit metadata from another session', async () => {
    const engine = await startEngine();
    harnesses.get(engine).server.audit.toolCalls[0].metadata.session_id = 'other-session';
    await expect(replayGolden(engine)).rejects.toThrow(/direct audit/);
  });

  test('keeps stderr bounded and rejects a second query', async () => {
    const engine = await startEngine();
    const harness = harnesses.get(engine);
    harness.child.stderr.emit('data', Buffer.alloc(CODEX_STDERR_MAX_BYTES + 100, 0x61));
    expect(engine.getTransportState().stderrBytes).toBe(CODEX_STDERR_MAX_BYTES);
    const pending = replayGolden(engine);
    await expect(engine.query('SECOND').next()).rejects.toThrow(/exactly one query/);
    await pending;
    expect(harness.child.stdin.write).toHaveBeenCalledTimes(2);
  });
});
