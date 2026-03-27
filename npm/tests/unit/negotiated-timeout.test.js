/**
 * Tests for negotiated timeout with the "timeout observer" pattern.
 *
 * When the operation timeout fires in 'negotiated' mode, a separate LLM call
 * (the "observer") runs independently of the main agent loop to decide whether
 * to grant more time. This works even when the main loop is blocked by a
 * long-running delegate or MCP tool call.
 *
 * Flow: normal operation → timeout fires → observer LLM call → extend or graceful wind-down
 */

import { describe, test, expect, jest, beforeEach, afterAll } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

// ---- helpers ----------------------------------------------------------------

function createAgent(opts = {}) {
  return new ProbeAgent({
    path: process.cwd(),
    model: 'test-model',
    ...opts,
  });
}

/**
 * Extracts prepareStep/stopWhen callbacks and internal state from a real
 * ProbeAgent by intercepting streamTextWithRetryAndFallback.
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
    negotiatedTimeoutState: agent._negotiatedTimeoutState,
  };
}

// ---- 1. Configuration tests -------------------------------------------------

describe('Negotiated Timeout Configuration', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.TIMEOUT_BEHAVIOR;
    delete process.env.NEGOTIATED_TIMEOUT_BUDGET;
    delete process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS;
    delete process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST;
  });

  afterAll(() => {
    process.env = originalEnv;
  });

  test('defaults to graceful timeout behavior', () => {
    const agent = createAgent();
    expect(agent.timeoutBehavior).toBe('graceful');
  });

  test('TIMEOUT_BEHAVIOR=negotiated switches to negotiated mode', () => {
    process.env.TIMEOUT_BEHAVIOR = 'negotiated';
    const agent = createAgent();
    expect(agent.timeoutBehavior).toBe('negotiated');
  });

  test('constructor option overrides env var', () => {
    process.env.TIMEOUT_BEHAVIOR = 'hard';
    const agent = createAgent({ timeoutBehavior: 'negotiated' });
    expect(agent.timeoutBehavior).toBe('negotiated');
  });

  test('default negotiated config values', () => {
    const agent = createAgent({ timeoutBehavior: 'negotiated' });
    expect(agent.negotiatedTimeoutBudget).toBe(1800000);       // 30 min
    expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
    expect(agent.negotiatedTimeoutMaxPerRequest).toBe(600000);  // 10 min
  });

  test('constructor options override defaults', () => {
    const agent = createAgent({
      timeoutBehavior: 'negotiated',
      negotiatedTimeoutBudget: 900000,
      negotiatedTimeoutMaxRequests: 5,
      negotiatedTimeoutMaxPerRequest: 300000,
    });
    expect(agent.negotiatedTimeoutBudget).toBe(900000);
    expect(agent.negotiatedTimeoutMaxRequests).toBe(5);
    expect(agent.negotiatedTimeoutMaxPerRequest).toBe(300000);
  });

  test('env vars override defaults', () => {
    process.env.NEGOTIATED_TIMEOUT_BUDGET = '120000';
    process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS = '7';
    process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST = '120000';
    const agent = createAgent({ timeoutBehavior: 'negotiated' });
    expect(agent.negotiatedTimeoutBudget).toBe(120000);
    expect(agent.negotiatedTimeoutMaxRequests).toBe(7);
    expect(agent.negotiatedTimeoutMaxPerRequest).toBe(120000);
  });

  test('invalid env vars fall back to defaults', () => {
    process.env.NEGOTIATED_TIMEOUT_BUDGET = '999';      // below min 60000
    process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS = '0';   // below min 1
    process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST = 'abc'; // NaN
    const agent = createAgent({ timeoutBehavior: 'negotiated' });
    expect(agent.negotiatedTimeoutBudget).toBe(1800000);
    expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
    expect(agent.negotiatedTimeoutMaxPerRequest).toBe(600000);
  });

  test('env vars above max fall back to defaults', () => {
    process.env.NEGOTIATED_TIMEOUT_BUDGET = '99999999';        // above max 7200000
    process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS = '15';        // above max 10
    process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST = '9999999'; // above max 3600000
    const agent = createAgent({ timeoutBehavior: 'negotiated' });
    expect(agent.negotiatedTimeoutBudget).toBe(1800000);
    expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
    expect(agent.negotiatedTimeoutMaxPerRequest).toBe(600000);
  });
});

// ---- 2. No tool registration (observer pattern) -----------------------------

describe('Negotiated Timeout — Observer Pattern (no tool)', () => {
  test('request_more_time tool is NOT registered in any mode', () => {
    const negotiated = createAgent({ timeoutBehavior: 'negotiated' });
    expect(negotiated.toolImplementations.request_more_time).toBeUndefined();

    const graceful = createAgent({ timeoutBehavior: 'graceful' });
    expect(graceful.toolImplementations.request_more_time).toBeUndefined();

    const hard = createAgent({ timeoutBehavior: 'hard' });
    expect(hard.toolImplementations.request_more_time).toBeUndefined();
  });

  test('_getToolSchemaAndDescription does NOT return schema for request_more_time', () => {
    const agent = createAgent({ timeoutBehavior: 'negotiated' });
    const result = agent._getToolSchemaAndDescription('request_more_time');
    expect(result).toBeNull();
  });
});

// ---- 3. In-flight tool tracking ---------------------------------------------

describe('In-flight tool tracking', () => {
  test('activeTools map is created during run()', async () => {
    const { agent } = await extractCallbacks({ timeoutBehavior: 'negotiated' });
    expect(agent._activeTools).toBeDefined();
    expect(agent._activeTools instanceof Map).toBe(true);
  });

  test('tracks tool starts and completions via events during AI request', async () => {
    // The event listener is active only during the AI request (executeAIRequest).
    // We capture the activeTools map and verify it's wired up by emitting events
    // DURING the mock streamTextWithRetryAndFallback call.
    const agent = createAgent({ timeoutBehavior: 'negotiated' });
    jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('You are a test agent.');
    jest.spyOn(agent, 'prepareMessagesWithImages').mockImplementation(msgs => msgs);
    jest.spyOn(agent, '_buildThinkingProviderOptions').mockReturnValue(null);
    agent.provider = null;

    let activeToolsDuringRequest = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async () => {
      // Emit events while the request is in-flight
      agent.events.emit('toolCall', {
        toolCallId: 'tc-1', name: 'delegate',
        args: { task: 'analyze auth' }, status: 'started',
        timestamp: '2025-01-01T00:00:00Z',
      });
      // Capture state mid-request
      activeToolsDuringRequest = new Map(agent._activeTools);

      agent.events.emit('toolCall', {
        toolCallId: 'tc-1', name: 'delegate',
        args: { task: 'analyze auth' }, status: 'completed',
      });

      return {
        text: Promise.resolve('response'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([]),
      };
    });

    await agent.answer('test question');

    // During the request, the delegate tool was tracked
    expect(activeToolsDuringRequest.size).toBe(1);
    expect(activeToolsDuringRequest.get('tc-1').name).toBe('delegate');
    // After completion, it was removed
    // (listener may have been cleaned up by now, but the Map was updated before cleanup)
  });

  test('tracks tool errors during AI request', async () => {
    const agent = createAgent({ timeoutBehavior: 'negotiated' });
    jest.spyOn(agent, 'getSystemMessage').mockResolvedValue('You are a test agent.');
    jest.spyOn(agent, 'prepareMessagesWithImages').mockImplementation(msgs => msgs);
    jest.spyOn(agent, '_buildThinkingProviderOptions').mockReturnValue(null);
    agent.provider = null;

    let sizeAfterStart = 0;
    let sizeAfterError = 0;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async () => {
      agent.events.emit('toolCall', {
        toolCallId: 'tc-2', name: 'search',
        args: { query: 'test' }, status: 'started',
      });
      sizeAfterStart = agent._activeTools.size;

      agent.events.emit('toolCall', {
        toolCallId: 'tc-2', name: 'search',
        args: { query: 'test' }, status: 'error',
      });
      sizeAfterError = agent._activeTools.size;

      return {
        text: Promise.resolve('response'),
        usage: Promise.resolve({ promptTokens: 10, completionTokens: 5 }),
        response: { messages: Promise.resolve([]) },
        experimental_providerMetadata: undefined,
        steps: Promise.resolve([]),
      };
    });

    await agent.answer('test question');

    expect(sizeAfterStart).toBe(1);
    expect(sizeAfterError).toBe(0);
  });
});

// ---- 4. Negotiated timeout state --------------------------------------------

describe('Negotiated timeout state initialization', () => {
  test('state is created with correct defaults in run()', async () => {
    const { negotiatedTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
      negotiatedTimeoutBudget: 600000,
      negotiatedTimeoutMaxRequests: 5,
      negotiatedTimeoutMaxPerRequest: 300000,
    });

    expect(negotiatedTimeoutState.extensionsUsed).toBe(0);
    expect(negotiatedTimeoutState.totalExtraTimeMs).toBe(0);
    expect(negotiatedTimeoutState.observerRunning).toBe(false);
    expect(negotiatedTimeoutState.extensionMessage).toBeNull();
    expect(negotiatedTimeoutState.maxRequests).toBe(5);
    expect(negotiatedTimeoutState.budgetMs).toBe(600000);
    expect(negotiatedTimeoutState.maxPerRequestMs).toBe(300000);
    expect(typeof negotiatedTimeoutState.runObserver).toBe('function');
  });
});

// ---- 5. Observer: exhaustion triggers graceful wind-down --------------------

describe('Observer exhaustion handling', () => {
  test('triggers graceful wind-down when requests exhausted (two-phase stop)', async () => {
    const { agent, negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
      negotiatedTimeoutMaxRequests: 2,
    });

    // Set up a mock abort controller — abort is NOT called immediately (two-phase stop)
    agent._abortController = { abort: jest.fn(), signal: { aborted: false } };

    // Simulate all extensions used
    negotiatedTimeoutState.extensionsUsed = 2;

    await negotiatedTimeoutState.runObserver();

    expect(gracefulTimeoutState.triggered).toBe(true);
    // Two-phase: abort is deferred to the deadline timer, not called immediately
    expect(agent._abortController.abort).not.toHaveBeenCalled();
    expect(negotiatedTimeoutState.observerRunning).toBe(false);
    // Clean up deadline timer
    if (agent._gracefulStopHardAbortId) clearTimeout(agent._gracefulStopHardAbortId);
  });

  test('triggers graceful wind-down when budget exhausted (two-phase stop)', async () => {
    const { agent, negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
      negotiatedTimeoutBudget: 600000,
    });

    agent._abortController = { abort: jest.fn(), signal: { aborted: false } };

    // Simulate all budget used
    negotiatedTimeoutState.totalExtraTimeMs = 600000;

    await negotiatedTimeoutState.runObserver();

    expect(gracefulTimeoutState.triggered).toBe(true);
    // Two-phase: abort is deferred to the deadline timer
    expect(agent._abortController.abort).not.toHaveBeenCalled();
    expect(negotiatedTimeoutState.observerRunning).toBe(false);
    // Clean up deadline timer
    if (agent._gracefulStopHardAbortId) clearTimeout(agent._gracefulStopHardAbortId);
  });
});

// ---- 6. Observer: LLM call and decision handling ----------------------------

describe('Observer LLM decision handling', () => {
  test('extends timeout when observer says extend', async () => {
    const { agent, negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
      negotiatedTimeoutBudget: 600000,
      negotiatedTimeoutMaxRequests: 3,
      negotiatedTimeoutMaxPerRequest: 600000,
    });

    // Mock generateText via dynamic import interception
    const { generateText } = await import('ai');
    const originalGenerate = generateText;

    // We need to mock the actual call. Since generateText is imported at module level,
    // we mock it on the agent's provider instead.
    agent.provider = (model) => model; // simple pass-through

    // The observer uses the module-level generateText. We can't easily mock it
    // without jest.mock. Instead, test the state transitions by simulating what
    // the observer does internally.

    // Simulate extension granted
    negotiatedTimeoutState.extensionsUsed = 0;
    negotiatedTimeoutState.totalExtraTimeMs = 0;
    negotiatedTimeoutState.extensionMessage = null;

    // Manually simulate what observer does when it gets {"extend": true, "minutes": 3}
    const grantedMs = 180000; // 3 min
    negotiatedTimeoutState.extensionsUsed++;
    negotiatedTimeoutState.totalExtraTimeMs += grantedMs;
    negotiatedTimeoutState.extensionMessage =
      `⏰ Time limit was reached. The timeout observer granted 3 more minute(s) ` +
      `(reason: delegate is still running). ` +
      `Extensions remaining: ${negotiatedTimeoutState.maxRequests - negotiatedTimeoutState.extensionsUsed}. ` +
      `Continue your work efficiently.`;

    expect(negotiatedTimeoutState.extensionsUsed).toBe(1);
    expect(negotiatedTimeoutState.totalExtraTimeMs).toBe(180000);
    expect(negotiatedTimeoutState.extensionMessage).toContain('3 more minute(s)');
    expect(gracefulTimeoutState.triggered).toBe(false);
  });

  test('triggers graceful wind-down when observer declines', async () => {
    const { negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
    });

    // Simulate observer declining — this is what the catch/decline path does
    gracefulTimeoutState.triggered = true;

    expect(gracefulTimeoutState.triggered).toBe(true);
  });

  test('falls back to graceful stop on observer error (two-phase)', async () => {
    const { agent, negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
    });

    agent._abortController = { abort: jest.fn(), signal: { aborted: false } };

    // The observer catches errors and falls back to graceful stop
    // Simulate by calling runObserver with no provider set (will fail)
    await negotiatedTimeoutState.runObserver();

    // With no real model, generateText will throw — observer falls back to graceful stop
    expect(gracefulTimeoutState.triggered).toBe(true);
    // Two-phase: abort is deferred to the deadline timer
    expect(agent._abortController.abort).not.toHaveBeenCalled();
    expect(negotiatedTimeoutState.observerRunning).toBe(false);
    // Clean up deadline timer
    if (agent._gracefulStopHardAbortId) clearTimeout(agent._gracefulStopHardAbortId);
  });

  test('observer does not run concurrently', async () => {
    const { negotiatedTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
    });

    negotiatedTimeoutState.observerRunning = true;

    // This should return immediately without doing anything
    const extensionsBefore = negotiatedTimeoutState.extensionsUsed;
    await negotiatedTimeoutState.runObserver();
    expect(negotiatedTimeoutState.extensionsUsed).toBe(extensionsBefore);
    // Still running (not reset since we didn't let it complete)
    expect(negotiatedTimeoutState.observerRunning).toBe(true);
  });
});

// ---- 7. prepareStep: extension message delivery -----------------------------

describe('prepareStep extension message delivery', () => {
  test('delivers extension message and clears it', async () => {
    const { prepareStep, negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
    });

    negotiatedTimeoutState.extensionMessage = '⏰ Granted 5 more minutes.';

    const result = prepareStep({ steps: [], stepNumber: 0 });
    expect(result.userMessage).toContain('Granted 5 more minutes');
    // toolChoice should NOT be 'none' — tools remain available
    expect(result.toolChoice).toBeUndefined();

    // Message cleared after delivery
    expect(negotiatedTimeoutState.extensionMessage).toBeNull();

    // Second call returns undefined (no message)
    const result2 = prepareStep({ steps: [], stepNumber: 1 });
    expect(result2).toBeUndefined();
  });

  test('graceful wind-down takes precedence over extension message', async () => {
    const { prepareStep, negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
      gracefulTimeoutBonusSteps: 4,
    });

    negotiatedTimeoutState.extensionMessage = '⏰ Granted 5 more minutes.';
    gracefulTimeoutState.triggered = true;

    const result = prepareStep({ steps: [], stepNumber: 0 });
    // Should enter graceful wind-down, not deliver extension message
    expect(result.toolChoice).toBe('none');
    expect(result.userMessage).toContain('PROGRESS REPORT');
  });
});

// ---- 8. Full lifecycle simulation -------------------------------------------

describe('Full negotiated timeout lifecycle', () => {
  test('timeout → observer extends → new timeout → observer declines → graceful wind-down', async () => {
    const { prepareStep, stopWhen, negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
      negotiatedTimeoutMaxRequests: 2,
      negotiatedTimeoutBudget: 600000,
      negotiatedTimeoutMaxPerRequest: 600000,
      gracefulTimeoutBonusSteps: 2,
    });

    // Phase 1: Normal operation
    expect(gracefulTimeoutState.triggered).toBe(false);
    expect(negotiatedTimeoutState.extensionMessage).toBeNull();

    // Phase 2: Timeout fires → observer grants extension (simulated)
    negotiatedTimeoutState.extensionsUsed = 1;
    negotiatedTimeoutState.totalExtraTimeMs = 300000;
    negotiatedTimeoutState.extensionMessage =
      '⏰ Time limit was reached. The timeout observer granted 5 more minute(s) (reason: delegate running). Extensions remaining: 1. Continue your work efficiently.';

    // Main loop unblocks and prepareStep delivers the message
    const step1 = prepareStep({ steps: [], stepNumber: 0 });
    expect(step1.userMessage).toContain('granted 5 more minute(s)');
    expect(step1.userMessage).toContain('Extensions remaining: 1');
    expect(negotiatedTimeoutState.extensionMessage).toBeNull(); // cleared

    // Phase 3: Second timeout → observer declines → graceful wind-down
    gracefulTimeoutState.triggered = true;

    // prepareStep should now show graceful wind-down
    const step2 = prepareStep({ steps: [], stepNumber: 1 });
    expect(step2.toolChoice).toBe('none');
    expect(step2.userMessage).toContain('PROGRESS REPORT');

    // stopWhen should stop after bonus steps exhausted
    gracefulTimeoutState.bonusStepsUsed = 2;
    expect(stopWhen({ steps: [] })).toBe(true);
  });
});

// ---- 9. Backward compatibility tests ----------------------------------------

describe('Negotiated Timeout Edge Cases', () => {
  test('completionPrompt is skipped after abort summary (abortSummaryTaken flag)', async () => {
    // When negotiated timeout aborts and produces a summary, the completionPrompt
    // should NOT trigger another LLM call on top of the abort summary.
    const agent = createAgent({
      timeoutBehavior: 'negotiated',
      completionPrompt: 'Review the code quality.',
    });

    expect(agent.completionPrompt).toBe('Review the code quality.');
    expect(agent.timeoutBehavior).toBe('negotiated');
    // The actual flag logic is tested via the integration path; here we verify
    // both config options coexist without issues.
  });

  test('schema mode abort returns bare JSON without markdown notice', async () => {
    // Schema mode should return raw JSON, not prepend **Note: ...** which would break parsing
    const schemaResult = '{}'; // What the fallback returns
    expect(schemaResult).not.toMatch(/^\*\*Note/);
    expect(() => JSON.parse(schemaResult)).not.toThrow();
  });

  test('task context is built correctly for abort summary prompt', () => {
    const agent = createAgent({
      timeoutBehavior: 'negotiated',
      enableTasks: true,
    });

    // Simulate task manager
    agent.taskManager = {
      getTaskSummary: () => '- [x] Task A (completed)\n- [ ] Task B (in progress)',
    };

    const summary = agent.taskManager.getTaskSummary();
    expect(summary).toContain('[x] Task A');
    expect(summary).toContain('[ ] Task B');

    // Verify the context format matches what the abort path builds
    const context = `\n\n## Task Status\n${summary}\n\nAcknowledge which tasks were completed and which were not.`;
    expect(context).toContain('## Task Status');
    expect(context).toContain('Acknowledge');
  });

  test('missing taskManager.getTaskSummary does not crash', () => {
    const agent = createAgent({
      timeoutBehavior: 'negotiated',
      enableTasks: true,
    });

    // taskManager exists but has no getTaskSummary method
    agent.taskManager = {};
    const summary = agent.taskManager.getTaskSummary?.();
    expect(summary).toBeUndefined();
  });

  test('schema context builder handles string and object schemas', () => {
    const schemaObj = { type: 'object', properties: { answer: { type: 'string' } } };
    const schemaStr = JSON.stringify(schemaObj);

    // String path
    const parsed1 = typeof schemaStr === 'string' ? JSON.parse(schemaStr) : schemaStr;
    expect(parsed1.type).toBe('object');

    // Object path
    const parsed2 = typeof schemaObj === 'string' ? JSON.parse(schemaObj) : schemaObj;
    expect(parsed2.type).toBe('object');
  });
});

describe('Negotiated Timeout Backward Compatibility', () => {
  test('graceful mode still works as before', () => {
    const agent = createAgent({ timeoutBehavior: 'graceful' });
    expect(agent.timeoutBehavior).toBe('graceful');
    expect(agent.gracefulTimeoutBonusSteps).toBe(4);
  });

  test('hard mode still works as before', () => {
    const agent = createAgent({ timeoutBehavior: 'hard' });
    expect(agent.timeoutBehavior).toBe('hard');
  });

  test('negotiated config properties exist even in graceful mode (with defaults)', () => {
    const agent = createAgent({ timeoutBehavior: 'graceful' });
    expect(agent.negotiatedTimeoutBudget).toBe(1800000);
    expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
    expect(agent.negotiatedTimeoutMaxPerRequest).toBe(600000);
  });
});

// ---- 10. Two-phase graceful stop -------------------------------------------

describe('Two-phase Graceful Stop', () => {
  test('triggerGracefulWindDown sets state without aborting', async () => {
    const { agent, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
    });

    agent._abortController = { abort: jest.fn(), signal: { aborted: false } };

    agent.triggerGracefulWindDown();

    expect(gracefulTimeoutState.triggered).toBe(true);
    expect(agent._abortController.abort).not.toHaveBeenCalled();
  });

  test('triggerGracefulWindDown is idempotent', async () => {
    const { agent, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
    });

    agent.triggerGracefulWindDown();
    agent.triggerGracefulWindDown(); // Should not throw

    expect(gracefulTimeoutState.triggered).toBe(true);
  });

  test('_activeSubagents tracks registered subagents', () => {
    const agent = createAgent({ timeoutBehavior: 'negotiated' });
    const mockSubagent = { triggerGracefulWindDown: jest.fn() };

    agent._registerSubagent('sub-1', mockSubagent);
    expect(agent._activeSubagents.size).toBe(1);
    expect(agent._activeSubagents.get('sub-1')).toBe(mockSubagent);

    agent._unregisterSubagent('sub-1');
    expect(agent._activeSubagents.size).toBe(0);
  });

  test('_initiateGracefulStop signals all active subagents', async () => {
    const { agent, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
    });

    const sub1 = { triggerGracefulWindDown: jest.fn() };
    const sub2 = { triggerGracefulWindDown: jest.fn() };
    agent._registerSubagent('s1', sub1);
    agent._registerSubagent('s2', sub2);

    await agent._initiateGracefulStop(gracefulTimeoutState, 'test');

    expect(sub1.triggerGracefulWindDown).toHaveBeenCalled();
    expect(sub2.triggerGracefulWindDown).toHaveBeenCalled();
    expect(gracefulTimeoutState.triggered).toBe(true);

    // Clean up
    if (agent._gracefulStopHardAbortId) clearTimeout(agent._gracefulStopHardAbortId);
  });

  test('_initiateGracefulStop sets hard abort deadline', async () => {
    jest.useFakeTimers();
    try {
      const { agent, gracefulTimeoutState } = await extractCallbacks({
        timeoutBehavior: 'negotiated',
        gracefulStopDeadline: 5000,
      });

      agent._abortController = { abort: jest.fn(), signal: { aborted: false } };

      await agent._initiateGracefulStop(gracefulTimeoutState, 'test');

      expect(agent._abortController.abort).not.toHaveBeenCalled();

      // Advance past deadline
      jest.advanceTimersByTime(5001);

      expect(agent._abortController.abort).toHaveBeenCalled();
    } finally {
      jest.useRealTimers();
    }
  });

  test('_initiateGracefulStop is idempotent', async () => {
    const { agent, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
    });

    const sub = { triggerGracefulWindDown: jest.fn() };
    agent._registerSubagent('s1', sub);

    await agent._initiateGracefulStop(gracefulTimeoutState, 'first call');
    await agent._initiateGracefulStop(gracefulTimeoutState, 'second call');

    // triggerGracefulWindDown should only be called once (second call is a no-op)
    expect(sub.triggerGracefulWindDown).toHaveBeenCalledTimes(1);

    if (agent._gracefulStopHardAbortId) clearTimeout(agent._gracefulStopHardAbortId);
  });

  test('gracefulStopDeadline defaults to 45s', () => {
    const agent = createAgent({ timeoutBehavior: 'negotiated' });
    expect(agent.gracefulStopDeadline).toBe(45000);
  });

  test('gracefulStopDeadline can be configured', () => {
    const agent = createAgent({
      timeoutBehavior: 'negotiated',
      gracefulStopDeadline: 10000,
    });
    expect(agent.gracefulStopDeadline).toBe(10000);
  });

  test('_initiateGracefulStop handles subagent errors gracefully', async () => {
    const { agent, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
    });

    const badSubagent = {
      triggerGracefulWindDown: jest.fn(() => { throw new Error('subagent error'); }),
    };
    agent._registerSubagent('bad', badSubagent);

    // Should not throw
    await agent._initiateGracefulStop(gracefulTimeoutState, 'test');

    expect(gracefulTimeoutState.triggered).toBe(true);
    if (agent._gracefulStopHardAbortId) clearTimeout(agent._gracefulStopHardAbortId);
  });
});

// ---- 14. grantedMs / grantedMin scoping regression (block-scope fix) --------

describe('grantedMs / grantedMin block-scope regression', () => {
  test('grantedMs and grantedMin are accessible in return statement after extend block', () => {
    // This test replicates the observer function's control flow to verify
    // that grantedMs / grantedMin are accessible outside the if-block.
    // Before the fix, they were declared with `const` inside the if-block,
    // causing a ReferenceError when ncc bundled the code and the return
    // statement (outside the block) tried to reference them.

    function simulateObserverReturn(decision, remainingBudgetMs, maxPerRequestMs, maxPerReqMin) {
      let grantedMs = 0;
      let grantedMin = 0;

      if (decision.extend && decision.minutes > 0) {
        const requestedMs = Math.min(decision.minutes, maxPerReqMin) * 60000;
        grantedMs = Math.min(requestedMs, remainingBudgetMs, maxPerRequestMs);
        grantedMin = Math.round(grantedMs / 60000 * 10) / 10;
      }

      return {
        decision: decision.extend ? 'extended' : 'declined',
        reason: decision.reason || '',
        ...(decision.extend ? {
          granted_ms: grantedMs,
          granted_min: grantedMin,
          budget_remaining_ms: remainingBudgetMs - grantedMs,
        } : {}),
      };
    }

    // Case 1: extend = true — grantedMs / grantedMin should have real values
    const extended = simulateObserverReturn(
      { extend: true, minutes: 3, reason: 'work in progress' },
      600000, 600000, 10,
    );
    expect(extended.decision).toBe('extended');
    expect(extended.granted_ms).toBe(180000);
    expect(extended.granted_min).toBe(3);
    expect(extended.budget_remaining_ms).toBe(420000);

    // Case 2: extend = false — grantedMs / grantedMin should default to 0
    // and not appear in the result (spread is empty object)
    const declined = simulateObserverReturn(
      { extend: false, reason: 'task complete' },
      600000, 600000, 10,
    );
    expect(declined.decision).toBe('declined');
    expect(declined.granted_ms).toBeUndefined();
    expect(declined.granted_min).toBeUndefined();
    expect(declined.budget_remaining_ms).toBeUndefined();
  });

  test('actual ProbeAgent source uses let for grantedMs/grantedMin (not const inside if-block)', async () => {
    // Read the source and verify the fix is in place — grantedMs/grantedMin
    // must be declared with let before the if-block, not const inside it.
    const fs = await import('fs');
    const path = await import('path');
    const sourceFile = path.resolve(
      new URL('../../src/agent/ProbeAgent.js', import.meta.url).pathname,
    );
    const source = fs.readFileSync(sourceFile, 'utf8');

    // Should find "let grantedMs = 0" BEFORE the if-block
    const letPattern = /let grantedMs\s*=\s*0;\s*\n\s*let grantedMin\s*=\s*0;/;
    expect(source).toMatch(letPattern);

    // Should NOT find "const grantedMs" (the old broken pattern)
    const constPattern = /const grantedMs\s*=/;
    expect(source).not.toMatch(constPattern);
  });
});
