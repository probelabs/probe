import { describe, expect, jest, test } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

function engineFrom(chunks) {
  return {
    async *query() {
      yield* chunks;
    }
  };
}

function createAgent(chunks) {
  const agent = new ProbeAgent({
    provider: 'codex',
    fallback: false,
    path: process.cwd(),
    model: 'test-model'
  });
  jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('Codex test system prompt');
  jest.spyOn(agent, 'getEngine').mockResolvedValue(engineFrom(chunks));
  return agent;
}

const receiptMetadata = {
  conversationId: 'thread-1',
  codexEventReceipt: { policyVerdict: { verdict: 'allow' } }
};

describe('Codex top-level metadata receipt seam', () => {
  test('calls onMetadata exactly once and preserves response text', async () => {
    const onMetadata = jest.fn();
    const metadata = { ...receiptMetadata };
    const agent = createAgent([
      { type: 'text', content: 'response text' },
      { type: 'metadata', data: metadata }
    ]);

    await expect(agent.answer('question', [], { onMetadata })).resolves.toBe('response text');
    expect(onMetadata).toHaveBeenCalledTimes(1);
    expect(onMetadata).toHaveBeenCalledWith(metadata);
  });

  test('fails closed when the metadata callback rejects', async () => {
    const callbackError = new Error('receipt sink failed');
    const onMetadata = jest.fn().mockRejectedValue(callbackError);
    const agent = createAgent([
      { type: 'text', content: 'response text' },
      { type: 'metadata', data: receiptMetadata }
    ]);

    await expect(agent.answer('question', [], { onMetadata })).rejects.toBe(callbackError);
    expect(onMetadata).toHaveBeenCalledTimes(1);
  });

  test.each([
    ['missing', [{ type: 'text', content: 'response text' }], /required success metadata/],
    ['missing receipt', [{ type: 'text', content: 'response text' }, { type: 'metadata', data: {} }], /invalid success metadata/],
    ['duplicate', [
      { type: 'text', content: 'response text' },
      { type: 'metadata', data: receiptMetadata },
      { type: 'metadata', data: receiptMetadata }
    ], /duplicate success metadata/]
  ])('rejects %s success metadata', async (_name, chunks, error) => {
    const onMetadata = jest.fn();
    const agent = createAgent(chunks);

    await expect(agent.answer('question', [], { onMetadata })).rejects.toThrow(error);
    expect(onMetadata).not.toHaveBeenCalled();
  });
});
