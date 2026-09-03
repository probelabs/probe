import assert from 'node:assert/strict';
import { access, mkdtemp, readFile, rm } from 'node:fs/promises';
import { PassThrough } from 'node:stream';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import Ajv from 'ajv';
import { validateJsonResponse } from '../../src/agent/schemaUtils.js';

const codex = process.env.CODEX_BIN || 'codex';
const THREAD_ID = 'thread-probe-1';
const TURN_ID = 'turn-probe-1';
const CALL_ID = 'call-probe-1';
const PROMPT = 'Search for the needle and return the bounded result.';
const CWD = '/tmp/probe-subject';
const TOOLS = [
  { type: 'function', name: 'search', description: 'Search files', inputSchema: { type: 'object' } },
  { type: 'function', name: 'extract', description: 'Extract code', inputSchema: { type: 'object' } },
  { type: 'function', name: 'listFiles', description: 'List files', inputSchema: { type: 'object' } },
];
const OUTPUT_SCHEMA = { type: 'object', additionalProperties: false, required: ['items'], properties: {
  items: { type: 'array', maxItems: 12, items: { type: 'object', additionalProperties: false,
    required: ['id'], properties: { id: { type: 'string' } } } },
} };
const FINAL_ITEM = { id: 'item-probe-1', type: 'agentMessage', text: JSON.stringify({ items: [{ id: 'needle' }] }) };

const writeJson = (stream, message) => stream.write(`${JSON.stringify(message)}\n`);

async function protocolSchemas() {
  const root = await mkdtemp(join(tmpdir(), 'probe-app-server-v2-schemas-'));
  const files = {
    clientRequest: 'ClientRequest.json', serverRequest: 'ServerRequest.json', response: 'JSONRPCResponse.json',
    notification: 'JSONRPCNotification.json', initializeParams: 'v1/InitializeParams.json',
    initializeResponse: 'v1/InitializeResponse.json', threadStartParams: 'v2/ThreadStartParams.json',
    threadStartResponse: 'v2/ThreadStartResponse.json', turnStartParams: 'v2/TurnStartParams.json',
    turnStartResponse: 'v2/TurnStartResponse.json', toolParams: 'DynamicToolCallParams.json',
    toolResponse: 'DynamicToolCallResponse.json', itemCompleted: 'v2/ItemCompletedNotification.json',
    turnCompleted: 'v2/TurnCompletedNotification.json',
  };
  try {
    const generated = spawnSync(codex, ['app-server', 'generate-json-schema', '--experimental', '--out', root], { encoding: 'utf8' });
    assert.equal(generated.status, 0, generated.stderr);
    const loaded = {};
    for (const [name, file] of Object.entries(files)) loaded[name] = JSON.parse(await readFile(join(root, file), 'utf8'));
    return loaded;
  } finally {
    await rm(root, { recursive: true, force: true });
    await assert.rejects(() => access(root), { code: 'ENOENT' });
  }
}

class ProtocolValidator {
  constructor(schemas) {
    const ajv = new Ajv({ strict: false, allErrors: true, formats: {
      int64: true, uint64: true, uint32: true, uint16: true, uint: true, int32: true, double: true,
    } });
    this.validators = Object.fromEntries(Object.entries(schemas).map(([name, schema]) => [name, ajv.compile(schema)]));
  }

  check(name, value) {
    const valid = this.validators[name](value);
    assert.equal(valid, true, `${name}: ${JSON.stringify(this.validators[name].errors)}`);
  }
}

class FakeAppServer {
  constructor(input, output, validator) {
    this.input = input; this.output = output; this.validator = validator; this.buffer = '';
    this.pendingToolCalls = new Map(); this.turnRequestId = null;
    input.on('data', (chunk) => this.receive(chunk));
  }

  receive(chunk) {
    this.buffer += chunk;
    const lines = this.buffer.split('\n'); this.buffer = lines.pop();
    for (const line of lines) {
      if (!line.trim()) continue;
      const message = JSON.parse(line);
      if (message.method) this.handleRequest(message);
      else if (this.pendingToolCalls.has(message.id)) this.handleToolReply(message);
    }
  }

  handleRequest(message) {
    this.validator.check('clientRequest', message);
    if (message.method === 'initialize') {
      this.validator.check('initializeParams', message.params);
      const result = { userAgent: 'fake-probe/1', codexHome: '/tmp/probe-home', platformFamily: 'unix', platformOs: 'test' };
      this.validator.check('initializeResponse', result);
      writeJson(this.output, { id: message.id, result });
      return;
    }
    if (message.method === 'thread/start') {
      this.validator.check('threadStartParams', message.params);
      assert.equal(Object.hasOwn(message.params, 'reasoningEffort'), false);
      assert.deepEqual(message.params.dynamicTools, TOOLS);
      const thread = { id: THREAD_ID, extra: null, sessionId: THREAD_ID, forkedFromId: null, parentThreadId: null,
        preview: '', ephemeral: true, section: null, sectionEnteredAt: null, projectId: null, historyMode: 'legacy',
        modelProvider: 'openai', createdAt: 1700000000, updatedAt: 1700000000, recencyAt: 1700000000,
        status: { type: 'idle' }, path: null, cwd: CWD, cliVersion: '0.150.1', source: 'appServer',
        canAcceptDirectInput: true, threadSource: null, agentNickname: null, agentRole: null, gitInfo: null,
        name: null, turns: [] };
      const result = { thread, model: 'gpt-5.6-luna', modelProvider: 'openai', serviceTier: null, cwd: CWD,
        runtimeWorkspaceRoots: [CWD], instructionSources: [], approvalPolicy: 'never', approvalsReviewer: 'user',
        sandbox: { type: 'readOnly', networkAccess: false }, activePermissionProfile: null, reasoningEffort: 'xhigh' };
      this.validator.check('threadStartResponse', result);
      writeJson(this.output, { id: message.id, result });
      return;
    }
    assert.equal(message.method, 'turn/start');
    this.validator.check('turnStartParams', message.params);
    assert.deepEqual(message.params, { threadId: THREAD_ID, input: [{ type: 'text', text: PROMPT }], outputSchema: OUTPUT_SCHEMA });
    this.turnRequestId = message.id;
    const result = { turn: { id: TURN_ID, items: [], status: 'inProgress' } };
    this.validator.check('turnStartResponse', result);
    writeJson(this.output, { id: message.id, result });
    const request = { id: 'server-request-1', method: 'item/tool/call', params: {
      threadId: THREAD_ID, turnId: TURN_ID, callId: CALL_ID, tool: 'search', arguments: { query: 'needle' },
    } };
    this.validator.check('serverRequest', request); this.validator.check('toolParams', request.params);
    this.pendingToolCalls.set(request.id, request.params); writeJson(this.output, request);
  }

  handleToolReply(message) {
    const expected = this.pendingToolCalls.get(message.id); this.pendingToolCalls.delete(message.id);
    this.validator.check('response', message); this.validator.check('toolResponse', message.result);
    assert.deepEqual(message.result, { success: true, contentItems: [{ type: 'inputText', text: 'search-result' }] });
    const itemNotification = { method: 'item/completed', params: {
      completedAtMs: 1700000000123, item: FINAL_ITEM, threadId: expected.threadId, turnId: expected.turnId,
    } };
    const turnNotification = { method: 'turn/completed', params: {
      threadId: expected.threadId, turn: { id: expected.turnId, items: [FINAL_ITEM], status: 'completed' },
    } };
    this.validator.check('notification', itemNotification); this.validator.check('itemCompleted', itemNotification.params);
    this.validator.check('notification', turnNotification); this.validator.check('turnCompleted', turnNotification.params);
    writeJson(this.output, itemNotification); writeJson(this.output, turnNotification);
  }

  close() { this.input.destroy(); this.output.destroy(); }
}

class ProbeClient {
  constructor(input, output, validator) {
    this.input = input; this.output = output; this.validator = validator; this.buffer = ''; this.nextId = 0;
    this.pending = new Map(); this.callbacks = new Map(); this.threadId = null; this.turnId = null;
    this.declaredTools = new Set(); this.finalText = null;
    this.completed = new Promise((resolve) => { this.complete = resolve; });
    output.on('data', (chunk) => this.receive(chunk));
  }

  request(method, params) {
    const id = `client-request-${++this.nextId}`;
    const result = new Promise((resolve, reject) => this.pending.set(id, { method, resolve, reject }));
    const request = { id, method, params }; this.validator.check('clientRequest', request);
    if (method === 'initialize') this.validator.check('initializeParams', params);
    if (method === 'thread/start') this.validator.check('threadStartParams', params);
    if (method === 'turn/start') this.validator.check('turnStartParams', params);
    writeJson(this.input, request); return result;
  }

  receive(chunk) {
    this.buffer += chunk;
    const lines = this.buffer.split('\n'); this.buffer = lines.pop();
    for (const line of lines) {
      if (!line.trim()) continue;
      const message = JSON.parse(line);
      if (message.method === 'item/tool/call') this.acceptToolCall(message);
      else if (message.method === 'item/completed') {
        this.validator.check('notification', message); this.validator.check('itemCompleted', message.params);
        this.finalText = message.params.item.text;
      } else if (message.method === 'turn/completed') {
        this.validator.check('notification', message); this.validator.check('turnCompleted', message.params);
        assert.equal(message.params.threadId, this.threadId); assert.equal(message.params.turn.id, this.turnId);
        this.complete(message.params.turn);
      } else if (this.pending.has(message.id)) {
        this.validator.check('response', message);
        const callback = this.pending.get(message.id); this.pending.delete(message.id);
        if (callback.method === 'initialize') this.validator.check('initializeResponse', message.result);
        if (callback.method === 'thread/start') this.validator.check('threadStartResponse', message.result);
        if (callback.method === 'turn/start') this.validator.check('turnStartResponse', message.result);
        if (callback.method === 'turn/start') this.turnId = message.result.turn.id;
        callback.resolve(message.result);
      }
    }
  }

  acceptToolCall(message) {
    this.validator.check('serverRequest', message); this.validator.check('toolParams', message.params);
    const { threadId, turnId, callId, tool, arguments: args } = message.params;
    assert.equal(threadId, this.threadId); assert.equal(turnId, this.turnId); assert.equal(callId, CALL_ID);
    assert.equal(this.declaredTools.has(tool), true); assert.equal(tool, 'search'); assert.deepEqual(args, { query: 'needle' });
    const result = { success: true, contentItems: [{ type: 'inputText', text: 'search-result' }] };
    const response = { id: message.id, result }; this.callbacks.set(message.id, true);
    this.validator.check('response', response); this.validator.check('toolResponse', result); writeJson(this.input, response);
    this.callbacks.delete(message.id);
  }

  abort() { for (const { reject } of this.pending.values()) reject(new Error('probe client aborted')); this.pending.clear(); this.callbacks.clear(); this.input.end(); }
  close() { this.input.destroy(); this.output.destroy(); }
}

test('fake app-server correlates one schema-constrained turn and tool call', async () => {
  const schemas = new ProtocolValidator(await protocolSchemas());
  const clientToServer = new PassThrough(); const serverToClient = new PassThrough();
  const fake = new FakeAppServer(clientToServer, serverToClient, schemas);
  const client = new ProbeClient(clientToServer, serverToClient, schemas);
  try {
    await client.request('initialize', { clientInfo: { name: 'probe', version: '0.0.1' }, capabilities: { experimentalApi: true } });
    const thread = await client.request('thread/start', { model: 'gpt-5.6-luna', config: { model_reasoning_effort: 'xhigh' },
      cwd: CWD, approvalPolicy: 'never', sandbox: 'read-only', ephemeral: true, dynamicTools: TOOLS });
    assert.equal(thread.model, 'gpt-5.6-luna'); assert.equal(thread.modelProvider, 'openai');
    assert.equal(thread.reasoningEffort, 'xhigh'); assert.equal(thread.approvalsReviewer, 'user');
    client.threadId = thread.thread.id; client.declaredTools = new Set(TOOLS.map(({ name }) => name));
    const response = await client.request('turn/start', { threadId: THREAD_ID, input: [{ type: 'text', text: PROMPT }], outputSchema: OUTPUT_SCHEMA });
    assert.deepEqual(response.turn, { id: TURN_ID, items: [], status: 'inProgress' });
    const completed = await client.completed; assert.equal(completed.id, TURN_ID); assert.equal(client.finalText, FINAL_ITEM.text);
    assert.equal(validateJsonResponse(client.finalText, { schema: OUTPUT_SCHEMA }).isValid, true);
    assert.equal(validateJsonResponse('{"items":[', { schema: OUTPUT_SCHEMA }).isValid, false);
    const tooMany = JSON.stringify({ items: Array.from({ length: 13 }, (_, index) => ({ id: String(index) })) });
    const rejected = validateJsonResponse(tooMany, { schema: OUTPUT_SCHEMA });
    assert.equal(rejected.isValid, false); assert.equal(rejected.schemaErrors.some(({ keyword }) => keyword === 'maxItems'), true);
  } finally {
    client.abort(); fake.close(); client.close();
  }
  assert.equal(client.pending.size, 0); assert.equal(client.callbacks.size, 0); assert.equal(fake.pendingToolCalls.size, 0);
  assert.equal(clientToServer.destroyed, true); assert.equal(serverToClient.destroyed, true);
});
