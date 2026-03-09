import { describe, test, expect, jest } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('ProbeAgent circuit breaker for repeated tool calls', () => {
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

  function createMockStreamResult(text, steps = [], messages = []) {
    return {
      text: Promise.resolve(text),
      usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
      response: { messages: Promise.resolve(messages) },
      experimental_providerMetadata: undefined,
      steps: Promise.resolve(steps),
    };
  }

  test('stopWhen detects 3 consecutive identical tool calls', async () => {
    const agent = createMockedAgent();

    let capturedOptions = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedOptions = opts;
      return createMockStreamResult('Answer');
    });

    agent.answer('test question').catch(() => {});
    await new Promise(resolve => setTimeout(resolve, 100));

    expect(capturedOptions).not.toBeNull();
    const { stopWhen } = capturedOptions;

    // 3 identical tool calls should trigger circuit breaker
    const steps = [
      { toolCalls: [{ toolName: 'search', args: { query: 'same' } }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'search', args: { query: 'same' } }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'search', args: { query: 'same' } }], finishReason: 'tool-calls' },
    ];
    expect(stopWhen({ steps })).toBe(true);

    jest.restoreAllMocks();
  });

  test('stopWhen does not trigger for different tool calls', async () => {
    const agent = createMockedAgent();

    let capturedOptions = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedOptions = opts;
      return createMockStreamResult('Answer');
    });

    agent.answer('test question').catch(() => {});
    await new Promise(resolve => setTimeout(resolve, 100));

    const { stopWhen } = capturedOptions;

    // Different tool calls should NOT trigger
    const steps = [
      { toolCalls: [{ toolName: 'search', args: { query: 'one' } }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'search', args: { query: 'two' } }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'search', args: { query: 'three' } }], finishReason: 'tool-calls' },
    ];
    expect(stopWhen({ steps })).toBe(false);

    jest.restoreAllMocks();
  });

  test('stopWhen uses tc.input as fallback when tc.args is undefined', async () => {
    const agent = createMockedAgent();

    let capturedOptions = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedOptions = opts;
      return createMockStreamResult('Answer');
    });

    agent.answer('test question').catch(() => {});
    await new Promise(resolve => setTimeout(resolve, 100));

    const { stopWhen } = capturedOptions;

    // tc.args undefined but tc.input present (Vercel AI SDK provider variation)
    const steps = [
      { toolCalls: [{ toolName: 'extract', args: undefined, input: { targets: 'file.js#Foo' } }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'extract', args: undefined, input: { targets: 'file.js#Foo' } }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'extract', args: undefined, input: { targets: 'file.js#Foo' } }], finishReason: 'tool-calls' },
    ];
    expect(stopWhen({ steps })).toBe(true);

    // With args, different calls should NOT match even if input is same
    const steps2 = [
      { toolCalls: [{ toolName: 'extract', args: { targets: 'file.js#Foo' } }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'extract', args: { targets: 'file.js#Bar' } }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'extract', args: { targets: 'file.js#Baz' } }], finishReason: 'tool-calls' },
    ];
    expect(stopWhen({ steps: steps2 })).toBe(false);

    jest.restoreAllMocks();
  });

  test('prepareStep forces toolChoice=none after 3 consecutive tool errors', async () => {
    const agent = createMockedAgent();

    let capturedOptions = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedOptions = opts;
      return createMockStreamResult('Answer');
    });

    agent.answer('test question').catch(() => {});
    await new Promise(resolve => setTimeout(resolve, 100));

    const { prepareStep } = capturedOptions;

    // 3 consecutive steps where all tool results are errors → force text output
    const errorResult = '<error type="path_error" recoverable="true"><message>Path does not exist: /tmp/workspace</message></error>';
    const errorSteps = [
      { toolCalls: [{ toolName: 'search', args: { query: 'from' } }], toolResults: [{ result: errorResult }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'search', args: { query: 'require' } }], toolResults: [{ result: errorResult }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'search', args: { query: 'use' } }], toolResults: [{ result: errorResult }], finishReason: 'tool-calls' },
    ];
    const result = prepareStep({ steps: errorSteps, stepNumber: 4 });
    expect(result).toEqual({ toolChoice: 'none' });

    // Mixed results (some errors, some success) should NOT force text-only
    const mixedSteps = [
      { toolCalls: [{ toolName: 'search', args: { query: 'one' } }], toolResults: [{ result: errorResult }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'search', args: { query: 'two' } }], toolResults: [{ result: 'some results' }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'search', args: { query: 'three' } }], toolResults: [{ result: errorResult }], finishReason: 'tool-calls' },
    ];
    const mixedResult = prepareStep({ steps: mixedSteps, stepNumber: 4 });
    expect(mixedResult?.toolChoice).toBeUndefined();

    jest.restoreAllMocks();
  });

  test('prepareStep forces toolChoice=none after 2 consecutive identical tool calls', async () => {
    const agent = createMockedAgent();

    let capturedOptions = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedOptions = opts;
      return createMockStreamResult('Answer');
    });

    agent.answer('test question').catch(() => {});
    await new Promise(resolve => setTimeout(resolve, 100));

    const { prepareStep } = capturedOptions;

    // 2 identical tool calls → force no tools
    const steps = [
      { toolCalls: [{ toolName: 'search', args: { query: 'same' } }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'search', args: { query: 'same' } }], finishReason: 'tool-calls' },
    ];
    const result = prepareStep({ steps, stepNumber: 3 });
    expect(result).toEqual({ toolChoice: 'none' });

    jest.restoreAllMocks();
  });

  test('prepareStep does not force toolChoice=none for different tool calls', async () => {
    const agent = createMockedAgent();

    let capturedOptions = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedOptions = opts;
      return createMockStreamResult('Answer');
    });

    agent.answer('test question').catch(() => {});
    await new Promise(resolve => setTimeout(resolve, 100));

    const { prepareStep } = capturedOptions;

    // Different tool calls → no override
    const steps = [
      { toolCalls: [{ toolName: 'search', args: { query: 'one' } }], finishReason: 'tool-calls' },
      { toolCalls: [{ toolName: 'search', args: { query: 'two' } }], finishReason: 'tool-calls' },
    ];
    const result = prepareStep({ steps, stepNumber: 3 });
    // Should not return toolChoice: 'none'
    expect(result?.toolChoice).toBeUndefined();

    jest.restoreAllMocks();
  });
});
