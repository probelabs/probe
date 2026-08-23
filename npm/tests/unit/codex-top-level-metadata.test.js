import { afterEach, describe, expect, jest, test } from '@jest/globals';
import { mkdtemp, realpath, rm } from 'fs/promises';
import { tmpdir } from 'os';
import { join } from 'path';
import { HOOK_TYPES } from '../../src/agent/hooks/HookManager.js';
import {
  CODEX_MODEL,
  CODEX_REASONING_EFFORT,
  CODEX_SANDBOX,
  CODEX_APPROVAL_POLICY,
  CODEX_PINNED_EXECUTABLE_PATH,
  CODEX_PINNED_EXECUTABLE_SHA256,
  CODEX_PINNED_SERVER_VERSION
} from '../../src/agent/engines/codex.js';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

const temporaryDirectories = new Set();

function engineFrom(chunks) {
  return {
    async *query() {
      yield* chunks;
    }
  };
}

function receiptMetadata(overrides = {}) {
  return {
    sessionId: 'session-1',
    conversationId: 'thread-1',
    messageCount: 1,
    codexEventReceipt: {
      policyVerdict: { verdict: 'allow' },
      cleanup: { status: 'succeeded' },
      ...overrides
    }
  };
}

async function createAgent(chunks) {
  const cwd = await realpath(await mkdtemp(join(tmpdir(), 'probe-r2-codex-cwd-')));
  const codexHome = await realpath(await mkdtemp(join(tmpdir(), 'probe-r2-codex-home-')));
  temporaryDirectories.add(cwd);
  temporaryDirectories.add(codexHome);

  const agent = new ProbeAgent({
    provider: 'codex',
    fallback: false,
    path: cwd,
    cwd,
    codexHome,
    model: CODEX_MODEL,
    thinkingEffort: CODEX_REASONING_EFFORT,
    codexSandbox: CODEX_SANDBOX,
    codexApprovalPolicy: CODEX_APPROVAL_POLICY,
    codexExecutablePath: CODEX_PINNED_EXECUTABLE_PATH,
    codexExpectedExecutablePath: CODEX_PINNED_EXECUTABLE_PATH,
    codexExpectedExecutableSha256: CODEX_PINNED_EXECUTABLE_SHA256,
    codexExpectedServerVersion: CODEX_PINNED_SERVER_VERSION
  });
  jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('Codex test system prompt');
  jest.spyOn(agent, 'getEngine').mockResolvedValue(engineFrom(chunks));
  return agent;
}

function observeRelease(agent) {
  const observed = [];
  const onStream = jest.fn(text => observed.push(`stream:${text}`));
  const onMetadata = jest.fn(async () => observed.push('metadata'));
  const tools = [];
  agent.events.on('toolCall', toolEvent => {
    tools.push(toolEvent);
    observed.push(`tool:${toolEvent.id}`);
  });
  const originalEmit = agent.hooks.emit.bind(agent.hooks);
  const hooksEmit = jest.spyOn(agent.hooks, 'emit').mockImplementation(async (...args) => {
    const [hookName, payload] = args;
    if (hookName === HOOK_TYPES.MESSAGE_USER) {
      observed.push('hook:message:user');
    } else if (payload && typeof payload === 'object' &&
        Object.prototype.hasOwnProperty.call(payload, 'response')) {
      observed.push('hook:response');
    }
    return originalEmit(...args);
  });
  return { observed, onStream, onMetadata, tools, hooksEmit };
}

function assertPreQueryHook(agent, hooksEmit) {
  const expected = [HOOK_TYPES.MESSAGE_USER, { sessionId: agent.sessionId, message: 'question', images: [] }];
  expect(hooksEmit.mock.calls[0]).toEqual(expected);
  expect(hooksEmit.mock.calls.filter(([hookName]) => hookName === HOOK_TYPES.MESSAGE_USER)).toEqual([expected]);
}

function assertNoRelease(agent, observed, onStream, onMetadata, tools, hooksEmit, expectedMetadataCalls = 0) {
  assertPreQueryHook(agent, hooksEmit);
  const responsePayloads = hooksEmit.mock.calls.filter(([, payload]) => payload && typeof payload === 'object' &&
    Object.prototype.hasOwnProperty.call(payload, 'response'));
  expect(responsePayloads).toHaveLength(0);
  expect(onStream).not.toHaveBeenCalled();
  expect(tools).toHaveLength(0);
  expect(agent.history).toHaveLength(0);
  expect(onMetadata).toHaveBeenCalledTimes(expectedMetadataCalls);
  expect(observed.filter(entry => entry.startsWith('stream:') || entry.startsWith('tool:') || entry === 'hook:response'))
    .toHaveLength(0);
}

afterEach(async () => {
  await Promise.all([...temporaryDirectories].map(directory => rm(directory, { recursive: true, force: true })));
  temporaryDirectories.clear();
});

describe('Codex top-level metadata receipt seam', () => {
  test('validates one allow/succeeded receipt and releases transactionally', async () => {
    const agent = await createAgent([
      { type: 'text', content: 'response ' },
      { type: 'toolBatch', tools: [{ id: 'first' }] },
      { type: 'text', content: 'text' },
      { type: 'toolBatch', tools: [{ id: 'second' }] },
      { type: 'metadata', data: receiptMetadata() }
    ]);
    const { observed, onStream, onMetadata, tools, hooksEmit } = observeRelease(agent);

    await expect(agent.answer('question', [], { onStream, onMetadata })).resolves.toBe('response text');
    assertPreQueryHook(agent, hooksEmit);
    expect(onMetadata).toHaveBeenCalledTimes(1);
    expect(onMetadata).toHaveBeenCalledWith(receiptMetadata());
    expect(observed).toEqual([
      'hook:message:user', 'metadata', 'stream:response ', 'stream:text', 'tool:first', 'tool:second', 'hook:response'
    ]);
    const responsePayloads = hooksEmit.mock.calls.filter(([, payload]) => payload && typeof payload === 'object' &&
      Object.prototype.hasOwnProperty.call(payload, 'response'));
    expect(responsePayloads).toHaveLength(1);
    expect(responsePayloads[0][1]).toEqual({
      sessionId: agent.sessionId,
      prompt: 'question',
      response: 'response text'
    });
    expect(onStream).toHaveBeenCalledTimes(2);
    expect(tools).toHaveLength(2);
    expect(agent.history).toEqual([
      { role: 'user', content: 'question' },
      { role: 'assistant', content: 'response text' }
    ]);
  });

  test('fails closed when the metadata callback rejects', async () => {
    const agent = await createAgent([
      { type: 'text', content: 'response text' },
      { type: 'metadata', data: receiptMetadata() }
    ]);
    const { observed, onStream, onMetadata, tools, hooksEmit } = observeRelease(agent);
    const callbackError = new Error('receipt sink failed');
    onMetadata.mockRejectedValue(callbackError);

    await expect(agent.answer('question', [], { onStream, onMetadata })).rejects.toBe(callbackError);
    assertNoRelease(agent, observed, onStream, onMetadata, tools, hooksEmit, 1);
  });

  test.each([
    ['missing', [{ type: 'text', content: 'response text' }], /required success metadata/],
    ['malformed', [
      { type: 'text', content: 'response text' },
      { type: 'metadata', data: { codexEventReceipt: { policyVerdict: { verdict: 'allow' } } } }
    ], /invalid success metadata/],
    ['deny verdict', [
      { type: 'text', content: 'response text' },
      { type: 'metadata', data: receiptMetadata({ policyVerdict: { verdict: 'deny' } }) }
    ], /invalid success metadata/],
    ['failed cleanup', [
      { type: 'text', content: 'response text' },
      { type: 'metadata', data: receiptMetadata({ cleanup: { status: 'failed' } }) }
    ], /invalid success metadata/],
    ['duplicate', [
      { type: 'text', content: 'response text' },
      { type: 'metadata', data: receiptMetadata() },
      { type: 'metadata', data: receiptMetadata() }
    ], /duplicate success metadata/],
    ['unknown before metadata', [{ type: 'unknown', value: 'nope' }], /unknown chunk type/],
    ['metadata then text', [
      { type: 'metadata', data: receiptMetadata() },
      { type: 'text', content: 'late response' }
    ], /after success metadata/],
    ['malformed text', [{ type: 'text' }], /malformed text chunk/],
    ['malformed toolBatch', [{ type: 'toolBatch', tools: {} }], /malformed toolBatch chunk/],
    ['malformed error', [{ type: 'error', error: 'not an Error' }], /malformed error chunk/],
    ['invalid envelope', [[]], /invalid chunk/]
  ])('rejects %s without releasing any observable effects', async (_name, chunks, error) => {
    const agent = await createAgent(chunks);
    const { observed, onStream, onMetadata, tools, hooksEmit } = observeRelease(agent);

    await expect(agent.answer('question', [], { onStream, onMetadata })).rejects.toThrow(error);
    assertNoRelease(agent, observed, onStream, onMetadata, tools, hooksEmit);
  });
});
