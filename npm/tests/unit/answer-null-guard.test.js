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
      maxRetries: 1,
      initialDelay: 0,
      maxDelay: 0,
      backoffFactor: 1,
      jitter: false,
    };

    const streamSpy = jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async () => ({
      text: Promise.resolve(''),
      finishReason: Promise.resolve('stop'),
      usage: Promise.resolve({ promptTokens: 10, completionTokens: 0 }),
      response: { messages: Promise.resolve([]) },
      experimental_providerMetadata: undefined,
      steps: Promise.resolve([]),
      // No output property — simulates no structured output
    }));

    await expect(agent.answer('test question'))
      .rejects
      .toThrow('Failed to get response from AI model. No output generated.');
    expect(streamSpy).toHaveBeenCalledTimes(2);
  });

  test('returns empty string when schema present but outputObject is null', async () => {
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => ({
      text: Promise.resolve(''),
      finishReason: Promise.resolve('stop'),
      usage: Promise.resolve({ promptTokens: 10, completionTokens: 0 }),
      response: { messages: Promise.resolve([]) },
      experimental_providerMetadata: undefined,
      steps: Promise.resolve([]),
      result: {
        output: Promise.resolve(null),
        text: Promise.resolve(''),
        steps: Promise.resolve([]),
      },
    }));

    // Pass a schema to trigger the schema code path
    const { z } = await import('zod');
    const result = await agent.answer('test question', [], {
      schema: z.object({ answer: z.string() }),
    });
    expect(typeof result).toBe('string');
  });

  test('returns empty string when schema output throws and finalText is empty', async () => {
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      // Create the rejection lazily to avoid unhandled-rejection before await
      const outputPromise = new Promise((_, reject) => {
        setTimeout(() => reject(new Error('NoObjectGeneratedError')), 0);
      });
      // Attach a no-op catch so Node doesn't report unhandled rejection
      outputPromise.catch(() => {});

      return {
        text: Promise.resolve(''),
        finishReason: Promise.resolve('stop'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 0 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([]),
        result: {
          output: outputPromise,
          text: Promise.resolve(''),
          steps: Promise.resolve([]),
        },
      };
    });

    const { z } = await import('zod');
    const result = await agent.answer('test question', [], {
      schema: z.object({ answer: z.string() }),
    });
    expect(typeof result).toBe('string');
  });

  test('returns actual text when finalText is available', async () => {
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async () => ({
      text: Promise.resolve('here is my answer'),
      finishReason: Promise.resolve('stop'),
      usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
      response: { messages: Promise.resolve([]) },
      experimental_providerMetadata: undefined,
      steps: Promise.resolve([]),
    }));

    const result = await agent.answer('test question');
    expect(result).toBe('here is my answer');
  });
});
