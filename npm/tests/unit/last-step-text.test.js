import { describe, test, expect, jest, beforeEach } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('ProbeAgent finalText uses last step text (no planning preamble)', () => {
  function createMockedAgent(options = {}) {
    const agent = new ProbeAgent({
      path: process.cwd(),
      model: 'test-model',
      ...options,
    });
    jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('You are a test agent.');
    jest.spyOn(agent, 'prepareMessagesWithImages').mockImplementation(msgs => msgs);
    jest.spyOn(agent, '_buildThinkingProviderOptions').mockReturnValue(null);
    agent.provider = null;
    return agent;
  }

  function createMockStreamResult(fullText, steps, messages = []) {
    return {
      text: Promise.resolve(fullText),
      usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
      response: { messages: Promise.resolve(messages) },
      experimental_providerMetadata: undefined,
      steps: Promise.resolve(steps),
    };
  }

  test('single-step response uses result.text as-is', async () => {
    const agent = createMockedAgent();
    const singleStep = [{ text: 'The final answer.', toolCalls: [], finishReason: 'stop' }];

    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockResolvedValue(
      createMockStreamResult('The final answer.', singleStep)
    );

    const result = await agent.answer('What is this?');
    expect(result).toBe('The final answer.');
    jest.restoreAllMocks();
  });

  test('multi-step response returns only last step text, not planning preamble', async () => {
    const agent = createMockedAgent();
    const steps = [
      { text: 'Let me search for relevant code...', toolCalls: [{ toolName: 'search' }], finishReason: 'tool-calls' },
      { text: "I've gathered sufficient data. Now analyzing...", toolCalls: [{ toolName: 'extract' }], finishReason: 'tool-calls' },
      { text: 'Rate limiting in Tyk is implemented via the RateLimitMiddleware in middleware/rate_limit.go.', toolCalls: [], finishReason: 'stop' },
    ];
    // result.text concatenates all steps
    const fullText = steps.map(s => s.text).join('');

    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockResolvedValue(
      createMockStreamResult(fullText, steps)
    );

    const result = await agent.answer('How is rate limiting implemented?');
    // Should only contain the last step's text, not the planning preamble
    expect(result).toBe('Rate limiting in Tyk is implemented via the RateLimitMiddleware in middleware/rate_limit.go.');
    expect(result).not.toContain('Let me search');
    expect(result).not.toContain('gathered sufficient data');
    jest.restoreAllMocks();
  });

  test('multi-step with empty last step text falls back to result.text', async () => {
    const agent = createMockedAgent();
    const steps = [
      { text: 'Searching...', toolCalls: [{ toolName: 'search' }], finishReason: 'tool-calls' },
      { text: '', toolCalls: [], finishReason: 'stop' },
    ];

    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockResolvedValue(
      createMockStreamResult('Searching...', steps)
    );

    const result = await agent.answer('Find something');
    // Falls back to full result.text when last step is empty
    expect(result).toBe('Searching...');
    jest.restoreAllMocks();
  });

  test('empty steps array uses result.text', async () => {
    const agent = createMockedAgent();

    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockResolvedValue(
      createMockStreamResult('Direct answer', [])
    );

    const result = await agent.answer('Simple question');
    expect(result).toBe('Direct answer');
    jest.restoreAllMocks();
  });
});
