import { afterEach, describe, expect, jest, test } from '@jest/globals';
import { mkdtemp, rm } from 'fs/promises';
import { tmpdir } from 'os';
import { join } from 'path';
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
  const cwd = await mkdtemp(join(tmpdir(), 'probe-r2-codex-cwd-'));
  const codexHome = await mkdtemp(join(tmpdir(), 'probe-r2-codex-home-'));
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
  // The Codex branch emits the completion hook after the transaction is released.
  agent.hooks.on(undefined, () => observed.push('completion'));
  const hooksEmit = jest.spyOn(agent.hooks, 'emit');
  return { observed, onStream, onMetadata, tools, hooksEmit };
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
    expect(onMetadata).toHaveBeenCalledTimes(1);
    expect(onMetadata).toHaveBeenCalledWith(receiptMetadata());
    expect(observed).toEqual([
      'metadata', 'stream:response ', 'stream:text', 'tool:first', 'tool:second', 'completion'
    ]);
    const completionEmits = hooksEmit.mock.calls.filter(([hookName]) => hookName === undefined);
    expect(completionEmits).toHaveLength(1);
    expect(hooksEmit).toHaveBeenLastCalledWith(undefined, {
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
    const { onStream, onMetadata, tools, hooksEmit } = observeRelease(agent);
    const callbackError = new Error('receipt sink failed');
    onMetadata.mockRejectedValue(callbackError);

    await expect(agent.answer('question', [], { onStream, onMetadata })).rejects.toBe(callbackError);
    expect(onMetadata).toHaveBeenCalledTimes(1);
    expect(onStream).not.toHaveBeenCalled();
    expect(tools).toHaveLength(0);
    expect(agent.history).toHaveLength(0);
    expect(hooksEmit.mock.calls.filter(([hookName]) => hookName === undefined)).toHaveLength(0);
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
    ], /duplicate success metadata/]
  ])('rejects %s without releasing any observable effects', async (_name, chunks, error) => {
    const agent = await createAgent(chunks);
    const { onStream, onMetadata, tools, hooksEmit } = observeRelease(agent);

    await expect(agent.answer('question', [], { onStream, onMetadata })).rejects.toThrow(error);
    expect(onMetadata).not.toHaveBeenCalled();
    expect(onStream).not.toHaveBeenCalled();
    expect(tools).toHaveLength(0);
    expect(agent.history).toHaveLength(0);
    expect(agent.hooks.getCallbackCount(undefined)).toBe(1);
    expect(hooksEmit.mock.calls.filter(([hookName]) => hookName === undefined)).toHaveLength(0);
  });
});
