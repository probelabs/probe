/**
 * Tests for graceful timeout wind-down mechanism.
 *
 * When the operation timeout fires in 'graceful' mode, instead of hard-aborting
 * the agent gets N bonus steps with reminders to wrap up and provide its answer.
 */

import { describe, test, expect, jest, beforeEach, afterAll } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

// ---- helpers ----------------------------------------------------------------

function createAgent(opts = {}) {
  const agent = new ProbeAgent({
    path: process.cwd(),
    model: 'test-model',
    ...opts,
  });
  return agent;
}

// ---- Configuration tests ----------------------------------------------------

describe('Graceful Timeout Configuration', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.TIMEOUT_BEHAVIOR;
    delete process.env.GRACEFUL_TIMEOUT_BONUS_STEPS;
  });

  afterAll(() => {
    process.env = originalEnv;
  });

  test('defaults to graceful behavior with 4 bonus steps', () => {
    const agent = createAgent();
    expect(agent.timeoutBehavior).toBe('graceful');
    expect(agent.gracefulTimeoutBonusSteps).toBe(4);
  });

  test('TIMEOUT_BEHAVIOR=hard switches to hard mode', () => {
    process.env.TIMEOUT_BEHAVIOR = 'hard';
    const agent = createAgent();
    expect(agent.timeoutBehavior).toBe('hard');
  });

  test('GRACEFUL_TIMEOUT_BONUS_STEPS env var overrides default', () => {
    process.env.GRACEFUL_TIMEOUT_BONUS_STEPS = '6';
    const agent = createAgent();
    expect(agent.gracefulTimeoutBonusSteps).toBe(6);
  });

  test('invalid GRACEFUL_TIMEOUT_BONUS_STEPS falls back to default', () => {
    for (const val of ['-1', '0', '25', 'abc']) {
      process.env.GRACEFUL_TIMEOUT_BONUS_STEPS = val;
      const agent = createAgent();
      expect(agent.gracefulTimeoutBonusSteps).toBe(4);
    }
  });

  test('constructor options override env vars', () => {
    process.env.TIMEOUT_BEHAVIOR = 'hard';
    process.env.GRACEFUL_TIMEOUT_BONUS_STEPS = '10';
    const agent = createAgent({ timeoutBehavior: 'graceful', gracefulTimeoutBonusSteps: 2 });
    expect(agent.timeoutBehavior).toBe('graceful');
    expect(agent.gracefulTimeoutBonusSteps).toBe(2);
  });
});

// ---- prepareStep wind-down tests --------------------------------------------

describe('prepareStep wind-down behavior', () => {
  /**
   * Extracts the prepareStep callback from a real ProbeAgent by
   * intercepting streamTextWithRetryAndFallback.
   */
  async function extractCallbacks(agentOpts = {}) {
    const agent = createAgent(agentOpts);
    jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('You are a test agent.');
    jest.spyOn(agent, 'prepareMessagesWithImages').mockImplementation(msgs => msgs);
    jest.spyOn(agent, '_buildThinkingProviderOptions').mockReturnValue(null);
    agent.provider = null;

    let capturedOptions = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedOptions = opts;
      return {
        text: Promise.resolve('test response'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([]),
      };
    });

    await agent.answer('test question');
    return {
      agent,
      stopWhen: capturedOptions.stopWhen,
      prepareStep: capturedOptions.prepareStep,
      gracefulTimeoutState: agent._gracefulTimeoutState,
    };
  }

  test('prepareStep returns undefined when not triggered', async () => {
    const { prepareStep, gracefulTimeoutState } = await extractCallbacks();
    expect(gracefulTimeoutState.triggered).toBe(false);
    const result = prepareStep({ steps: [], stepNumber: 0 });
    expect(result).toBeUndefined();
  });

  test('first wind-down step injects userMessage with toolChoice none', async () => {
    const { prepareStep, gracefulTimeoutState } = await extractCallbacks({ gracefulTimeoutBonusSteps: 4 });

    // Trigger timeout
    gracefulTimeoutState.triggered = true;

    const result = prepareStep({ steps: [], stepNumber: 0 });
    expect(result.toolChoice).toBe('none');
    expect(result.userMessage).toContain('TIME LIMIT REACHED');
    expect(result.userMessage).toContain('3 step(s) remaining');
    expect(result.userMessage).toContain('Do NOT call any more tools');
    expect(gracefulTimeoutState.bonusStepsUsed).toBe(1);
  });

  test('subsequent wind-down steps return toolChoice none without message', async () => {
    const { prepareStep, gracefulTimeoutState } = await extractCallbacks({ gracefulTimeoutBonusSteps: 4 });

    gracefulTimeoutState.triggered = true;

    // First step (has message)
    prepareStep({ steps: [], stepNumber: 0 });
    expect(gracefulTimeoutState.bonusStepsUsed).toBe(1);

    // Second step (no message)
    const result = prepareStep({ steps: [], stepNumber: 1 });
    expect(result.toolChoice).toBe('none');
    expect(result.userMessage).toBeUndefined();
    expect(gracefulTimeoutState.bonusStepsUsed).toBe(2);
  });

  test('bonus steps counter increments correctly across calls', async () => {
    const { prepareStep, gracefulTimeoutState } = await extractCallbacks({ gracefulTimeoutBonusSteps: 3 });

    gracefulTimeoutState.triggered = true;

    for (let i = 0; i < 3; i++) {
      prepareStep({ steps: [], stepNumber: i });
    }
    expect(gracefulTimeoutState.bonusStepsUsed).toBe(3);
  });
});

// ---- stopWhen wind-down tests -----------------------------------------------

describe('stopWhen wind-down behavior', () => {
  async function extractStopWhen(agentOpts = {}) {
    const agent = createAgent(agentOpts);
    jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('You are a test agent.');
    jest.spyOn(agent, 'prepareMessagesWithImages').mockImplementation(msgs => msgs);
    jest.spyOn(agent, '_buildThinkingProviderOptions').mockReturnValue(null);
    agent.provider = null;

    let capturedOptions = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedOptions = opts;
      return {
        text: Promise.resolve('test response'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([]),
      };
    });

    await agent.answer('test question');
    return {
      stopWhen: capturedOptions.stopWhen,
      gracefulTimeoutState: agent._gracefulTimeoutState,
    };
  }

  test('allows steps during wind-down when bonus steps remain', async () => {
    const { stopWhen, gracefulTimeoutState } = await extractStopWhen();
    gracefulTimeoutState.triggered = true;
    gracefulTimeoutState.bonusStepsUsed = 1;
    gracefulTimeoutState.bonusStepsMax = 4;

    // Even past maxIterations (30), should not stop
    const mockSteps = new Array(50).fill({ finishReason: 'stop', toolCalls: [] });
    expect(stopWhen({ steps: mockSteps })).toBe(false);
  });

  test('stops when bonus steps exhausted', async () => {
    const { stopWhen, gracefulTimeoutState } = await extractStopWhen({ gracefulTimeoutBonusSteps: 4 });
    gracefulTimeoutState.triggered = true;
    gracefulTimeoutState.bonusStepsUsed = 4;

    expect(stopWhen({ steps: [] })).toBe(true);
  });

  test('overrides maxIterations hard limit during wind-down', async () => {
    const { stopWhen, gracefulTimeoutState } = await extractStopWhen();
    gracefulTimeoutState.triggered = true;
    gracefulTimeoutState.bonusStepsUsed = 0;

    // steps.length >= maxIterations would normally stop
    const mockSteps = new Array(50).fill({ finishReason: 'stop', toolCalls: [] });
    expect(stopWhen({ steps: mockSteps })).toBe(false);
  });

  test('normal maxIterations applies when not triggered', async () => {
    const { stopWhen, gracefulTimeoutState } = await extractStopWhen();
    expect(gracefulTimeoutState.triggered).toBe(false);

    // Over 30 (default maxIterations)
    const mockSteps = new Array(35).fill({ finishReason: 'tool_calls', toolCalls: [{ toolName: 'search' }] });
    expect(stopWhen({ steps: mockSteps })).toBe(true);
  });
});

// ---- Two-phase timeout tests ------------------------------------------------

describe('Two-phase timeout mechanism', () => {
  test('soft timeout sets triggered flag without aborting', () => {
    jest.useFakeTimers();
    try {
      const controller = new AbortController();
      const gts = { triggered: false, bonusStepsUsed: 0, bonusStepsMax: 4 };
      const timeoutState = { timeoutId: null, hardAbortId: null };

      timeoutState.timeoutId = setTimeout(() => {
        gts.triggered = true;
        timeoutState.hardAbortId = setTimeout(() => {
          controller.abort();
        }, 60000);
      }, 5000);

      jest.advanceTimersByTime(5000);
      expect(gts.triggered).toBe(true);
      expect(controller.signal.aborted).toBe(false);

      clearTimeout(timeoutState.timeoutId);
      clearTimeout(timeoutState.hardAbortId);
    } finally {
      jest.useRealTimers();
    }
  });

  test('hard abort fires after safety net delay', () => {
    jest.useFakeTimers();
    try {
      const controller = new AbortController();
      const gts = { triggered: false, bonusStepsUsed: 0, bonusStepsMax: 4 };
      const timeoutState = { timeoutId: null, hardAbortId: null };

      timeoutState.timeoutId = setTimeout(() => {
        gts.triggered = true;
        timeoutState.hardAbortId = setTimeout(() => {
          controller.abort();
        }, 60000);
      }, 5000);

      jest.advanceTimersByTime(5000);
      expect(controller.signal.aborted).toBe(false);

      jest.advanceTimersByTime(60000);
      expect(controller.signal.aborted).toBe(true);

      clearTimeout(timeoutState.timeoutId);
      clearTimeout(timeoutState.hardAbortId);
    } finally {
      jest.useRealTimers();
    }
  });

  test('hard mode aborts immediately without wind-down', () => {
    jest.useFakeTimers();
    try {
      const controller = new AbortController();
      const timeoutState = { timeoutId: null };

      timeoutState.timeoutId = setTimeout(() => {
        controller.abort();
      }, 5000);

      jest.advanceTimersByTime(5000);
      expect(controller.signal.aborted).toBe(true);

      clearTimeout(timeoutState.timeoutId);
    } finally {
      jest.useRealTimers();
    }
  });

  test('timer cleanup clears both soft and hard abort timers', () => {
    jest.useFakeTimers();
    try {
      const controller = new AbortController();
      const gts = { triggered: false, bonusStepsUsed: 0, bonusStepsMax: 4 };
      const timeoutState = { timeoutId: null, hardAbortId: null };

      timeoutState.timeoutId = setTimeout(() => {
        gts.triggered = true;
        timeoutState.hardAbortId = setTimeout(() => {
          controller.abort();
        }, 60000);
      }, 5000);

      // Trigger soft timeout
      jest.advanceTimersByTime(5000);
      expect(gts.triggered).toBe(true);

      // Clean up (simulating finally block)
      clearTimeout(timeoutState.timeoutId);
      timeoutState.timeoutId = null;
      clearTimeout(timeoutState.hardAbortId);
      timeoutState.hardAbortId = null;

      // Hard abort should NOT fire
      jest.advanceTimersByTime(60000);
      expect(controller.signal.aborted).toBe(false);
    } finally {
      jest.useRealTimers();
    }
  });
});

// ---- Integration: full wind-down cycle --------------------------------------

describe('Full wind-down cycle simulation', () => {
  test('complete graceful timeout lifecycle with real callbacks', async () => {
    const agent = createAgent({ gracefulTimeoutBonusSteps: 3 });
    jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('You are a test agent.');
    jest.spyOn(agent, 'prepareMessagesWithImages').mockImplementation(msgs => msgs);
    jest.spyOn(agent, '_buildThinkingProviderOptions').mockReturnValue(null);
    agent.provider = null;

    let capturedOptions = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedOptions = opts;
      return {
        text: Promise.resolve('test response'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([]),
      };
    });

    await agent.answer('test question');
    const { stopWhen, prepareStep } = capturedOptions;
    const gts = agent._gracefulTimeoutState;

    // Phase 1: Normal — not triggered
    expect(stopWhen({ steps: [{ finishReason: 'tool_calls', toolCalls: [{}] }] })).toBe(false);
    expect(prepareStep({ steps: [], stepNumber: 5 })).toBeUndefined();

    // Phase 2: Trigger timeout
    gts.triggered = true;

    // Phase 3: Wind-down steps
    // Step 1: gets wrap-up message
    const step1 = prepareStep({ steps: [], stepNumber: 0 });
    expect(step1.toolChoice).toBe('none');
    expect(step1.userMessage).toContain('TIME LIMIT REACHED');
    expect(step1.userMessage).toContain('2 step(s) remaining');
    expect(stopWhen({ steps: [] })).toBe(false); // 1 < 3

    // Step 2: no message
    const step2 = prepareStep({ steps: [], stepNumber: 1 });
    expect(step2.toolChoice).toBe('none');
    expect(step2.userMessage).toBeUndefined();
    expect(stopWhen({ steps: [] })).toBe(false); // 2 < 3

    // Step 3: should trigger stop
    prepareStep({ steps: [], stepNumber: 2 });
    expect(stopWhen({ steps: [] })).toBe(true); // 3 >= 3
  });

  test('wind-down works even when already past maxIterations', async () => {
    const agent = createAgent({ gracefulTimeoutBonusSteps: 2 });
    jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('You are a test agent.');
    jest.spyOn(agent, 'prepareMessagesWithImages').mockImplementation(msgs => msgs);
    jest.spyOn(agent, '_buildThinkingProviderOptions').mockReturnValue(null);
    agent.provider = null;

    let capturedOptions = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedOptions = opts;
      return {
        text: Promise.resolve('test response'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([]),
      };
    });

    await agent.answer('test question');
    const { stopWhen } = capturedOptions;
    const gts = agent._gracefulTimeoutState;

    // Trigger timeout
    gts.triggered = true;

    // Past maxIterations (30) — but wind-down overrides, should still allow
    const manySteps = new Array(40).fill({ finishReason: 'stop', toolCalls: [] });
    expect(stopWhen({ steps: manySteps })).toBe(false);

    // Exhaust bonus steps
    gts.bonusStepsUsed = 2;
    expect(stopWhen({ steps: manySteps })).toBe(true);
  });
});

// ---- Empty-text fallback tests ----------------------------------------------

describe('Graceful timeout empty-text fallback', () => {
  function createMockedAgent(opts = {}) {
    const agent = createAgent(opts);
    jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('You are a test agent.');
    jest.spyOn(agent, 'prepareMessagesWithImages').mockImplementation(msgs => msgs);
    jest.spyOn(agent, '_buildThinkingProviderOptions').mockReturnValue(null);
    agent.provider = null;
    return agent;
  }

  test('uses concatenated step text when wind-down produces empty finalText', async () => {
    const agent = createMockedAgent({ maxOperationTimeout: 100, gracefulTimeoutBonusSteps: 2 });

    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async () => {
      agent._gracefulTimeoutState.triggered = true;
      return {
        text: Promise.resolve('Some useful intermediate text from step 1'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([
          { text: 'Some useful intermediate text from step 1', finishReason: 'tool-calls', toolCalls: [{ toolName: 'search' }] },
          { text: '', finishReason: 'other', toolCalls: [] },
        ]),
      };
    });

    const result = await agent.answer('test question');
    expect(result).toContain('Some useful intermediate text');
    expect(result).toContain('time constraint');
    expect(result).toContain('may be incomplete');
  });

  test('builds fallback from tool results when all step texts are empty', async () => {
    const agent = createMockedAgent({ maxOperationTimeout: 100, gracefulTimeoutBonusSteps: 2 });

    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async () => {
      agent._gracefulTimeoutState.triggered = true;
      return {
        text: Promise.resolve(''),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([
          {
            text: '',
            finishReason: 'tool-calls',
            toolCalls: [{ toolName: 'search' }],
            toolResults: [{ result: 'BM25 is a ranking algorithm used in src/ranking.rs' }],
          },
          { text: '', finishReason: 'other', toolCalls: [], toolResults: [] },
        ]),
      };
    });

    const result = await agent.answer('test question');
    expect(result).toContain('time constraint');
    expect(result).toContain('timed out');
    expect(result).toContain('partial information');
    expect(result).toContain('BM25 is a ranking algorithm');
  });

  test('uses generic timeout message when no tool results available', async () => {
    const agent = createMockedAgent({ maxOperationTimeout: 100, gracefulTimeoutBonusSteps: 2 });

    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async () => {
      agent._gracefulTimeoutState.triggered = true;
      return {
        text: Promise.resolve(''),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([
          { text: '', finishReason: 'other', toolCalls: [], toolResults: [] },
        ]),
      };
    });

    const result = await agent.answer('test question');
    expect(result).toContain('timed out');
    expect(result).toContain('try again');
  });

  test('does not trigger fallback when timeout did not fire', async () => {
    const agent = createMockedAgent({ maxOperationTimeout: 100, gracefulTimeoutBonusSteps: 2 });

    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async () => {
      return {
        text: Promise.resolve('Normal response text'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([
          { text: 'Normal response text', finishReason: 'stop', toolCalls: [] },
        ]),
      };
    });

    const result = await agent.answer('test question');
    expect(result).toBe('Normal response text');
    expect(result).not.toContain('time constraint');
  });

  test('prepends timeout notice when wind-down produces non-empty text', async () => {
    const agent = createMockedAgent({ maxOperationTimeout: 100, gracefulTimeoutBonusSteps: 2 });

    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async () => {
      agent._gracefulTimeoutState.triggered = true;
      return {
        text: Promise.resolve('Model successfully wrapped up with partial results'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([
          { text: '', finishReason: 'tool-calls', toolCalls: [{ toolName: 'search' }] },
          { text: 'Model successfully wrapped up with partial results', finishReason: 'stop', toolCalls: [] },
        ]),
      };
    });

    const result = await agent.answer('test question');
    // Should have BOTH the timeout notice AND the model's answer
    expect(result).toContain('time constraint');
    expect(result).toContain('may be incomplete');
    expect(result).toContain('Model successfully wrapped up with partial results');
  });
});
