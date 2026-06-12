/**
 * Tests for #533: answer() must always return a string, never null/undefined.
 *
 * When both structured output and finalText are empty (e.g., during timeouts
 * or model failures), answer() previously returned null, causing downstream
 * callers like delegate() to throw "Delegate agent returned invalid response
 * (not a string)".
 */

import { describe, test, expect, jest, beforeEach } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

function createAgent(opts = {}) {
  return new ProbeAgent({
    path: process.cwd(),
    model: 'test-model',
    ...opts,
  });
}

describe('answer() null-guard (#533)', () => {
  const EXHAUST_AFTER_ONE_RETRY = 1;
  let agent;

  beforeEach(() => {
    agent = createAgent();
    jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('You are a test agent.');
    jest.spyOn(agent, 'prepareMessagesWithImages').mockImplementation(msgs => msgs);
    jest.spyOn(agent, '_buildThinkingProviderOptions').mockReturnValue(null);
    agent.provider = null;
  });

  test('retries and fails when non-schema stream has no steps and no text', async () => {
    agent.retryConfig = {
      ...agent.retryConfig,
      maxRetries: EXHAUST_AFTER_ONE_RETRY,
      initialDelay: 0,
      maxDelay: 0,
      backoffFactor: 1,
      jitter: false,
    };

    let attempts = 0;
    const retryManager = agent._createRetryManager();
    const streamSpy = jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (_opts, consumeResult) => {
      return await retryManager.executeWithRetry(
        async () => {
          attempts += 1;
          return await consumeResult({
            text: Promise.resolve(''),
            finishReason: Promise.resolve('stop'),
            usage: Promise.resolve({ promptTokens: 10, completionTokens: 0 }),
            response: { messages: Promise.resolve([]) },
            experimental_providerMetadata: undefined,
            steps: Promise.resolve([]),
            // No output property — simulates no structured output
          });
        },
        { provider: 'test', model: 'test-model' }
      );
    });

    await expect(agent.answer('test question'))
      .rejects
      .toThrow('Failed to get response from AI model. No output generated.');
    expect(streamSpy).toHaveBeenCalledTimes(1);
    expect(attempts).toBe(2);
  });

  test('returns empty string when schema present but outputObject is null', async () => {
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (_opts, consumeResult) => {
      const result = {
        text: Promise.resolve(''),
        finishReason: Promise.resolve('stop'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 0 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([]),
        output: Promise.resolve(null),
      };

      return consumeResult ? await consumeResult(result) : result;
    });

    // Pass a schema to trigger the schema code path
    const { z } = await import('zod');
    const result = await agent.answer('test question', [], {
      schema: z.object({ answer: z.string() }),
    });
    expect(typeof result).toBe('string');
  });

  test('returns empty string when schema output throws and finalText is empty', async () => {
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (_opts, consumeResult) => {
      // Create the rejection lazily to avoid unhandled-rejection before await
      const outputPromise = new Promise((_, reject) => {
        setTimeout(() => reject(new Error('NoObjectGeneratedError')), 0);
      });
      // Attach a no-op catch so Node doesn't report unhandled rejection
      outputPromise.catch(() => {});

      const result = {
        text: Promise.resolve(''),
        finishReason: Promise.resolve('stop'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 0 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([]),
        output: outputPromise,
      };

      return consumeResult ? await consumeResult(result) : result;
    });

    const { z } = await import('zod');
    const result = await agent.answer('test question', [], {
      schema: z.object({ answer: z.string() }),
    });
    expect(typeof result).toBe('string');
  });

  test('returns actual text when finalText is available', async () => {
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (_opts, consumeResult) => {
      const result = {
        text: Promise.resolve('here is my answer'),
        finishReason: Promise.resolve('stop'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([]),
      };

      return consumeResult ? await consumeResult(result) : result;
    });

    const result = await agent.answer('test question');
    expect(result).toBe('here is my answer');
  });
});
