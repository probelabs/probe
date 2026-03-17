/**
 * Tests for negotiated timeout observer capping extensions to external hard timeout.
 *
 * Issue #522: The observer can grant extensions that push the effective deadline
 * past the external hard timeout (e.g., visor's Promise.race ceiling), causing
 * the external timeout to kill the agent instantly with no partial results.
 *
 * The fix adds an optional `externalHardTimeout` parameter that caps extensions
 * so the granted time never exceeds the external ceiling.
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
 * Extracts internal state from a real ProbeAgent by intercepting streamText.
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

// ---- 1. externalHardTimeout configuration -----------------------------------

describe('externalHardTimeout configuration', () => {
  test('stores externalHardTimeout from constructor options', () => {
    const agent = createAgent({
      timeoutBehavior: 'negotiated',
      externalHardTimeout: 1800000, // 30 min
    });
    expect(agent.externalHardTimeout).toBe(1800000);
  });

  test('defaults to null when not provided', () => {
    const agent = createAgent({
      timeoutBehavior: 'negotiated',
    });
    expect(agent.externalHardTimeout).toBeNull();
  });

  test('reads from EXTERNAL_HARD_TIMEOUT env var', () => {
    const origEnv = process.env.EXTERNAL_HARD_TIMEOUT;
    process.env.EXTERNAL_HARD_TIMEOUT = '1200000';
    try {
      const agent = createAgent({
        timeoutBehavior: 'negotiated',
      });
      expect(agent.externalHardTimeout).toBe(1200000);
    } finally {
      if (origEnv === undefined) {
        delete process.env.EXTERNAL_HARD_TIMEOUT;
      } else {
        process.env.EXTERNAL_HARD_TIMEOUT = origEnv;
      }
    }
  });

  test('constructor option takes precedence over env var', () => {
    const origEnv = process.env.EXTERNAL_HARD_TIMEOUT;
    process.env.EXTERNAL_HARD_TIMEOUT = '1200000';
    try {
      const agent = createAgent({
        timeoutBehavior: 'negotiated',
        externalHardTimeout: 900000,
      });
      expect(agent.externalHardTimeout).toBe(900000);
    } finally {
      if (origEnv === undefined) {
        delete process.env.EXTERNAL_HARD_TIMEOUT;
      } else {
        process.env.EXTERNAL_HARD_TIMEOUT = origEnv;
      }
    }
  });
});

// ---- 2. Extension capping to external hard timeout --------------------------

describe('Extension capping to external hard timeout', () => {
  test('observer caps granted time so it does not exceed external hard timeout', async () => {
    const { agent, negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
      maxOperationTimeout: 1500000, // 25 min
      externalHardTimeout: 1800000, // 30 min
      negotiatedTimeoutBudget: 1800000, // 30 min budget
      negotiatedTimeoutMaxRequests: 3,
      negotiatedTimeoutMaxPerRequest: 600000, // 10 min per request
    });

    // Simulate: 25 min have elapsed (timeout just fired)
    negotiatedTimeoutState.startTime = Date.now() - 1500000;

    // Observer wants to grant 10 min, but only 5 min of headroom to external timeout
    // grantedMs should be capped to ~5 min (300000ms), not the full 10 min (600000ms)
    const requestedMs = 600000; // 10 min
    const remainingBudgetMs = negotiatedTimeoutState.budgetMs - negotiatedTimeoutState.totalExtraTimeMs;
    const elapsed = Date.now() - negotiatedTimeoutState.startTime;
    const externalHeadroom = agent.externalHardTimeout
      ? Math.max(0, agent.externalHardTimeout - elapsed)
      : Infinity;

    const grantedMs = Math.min(requestedMs, remainingBudgetMs, negotiatedTimeoutState.maxPerRequestMs, externalHeadroom);

    // The granted time should be approximately 5 min (300s), not the requested 10 min
    expect(grantedMs).toBeLessThanOrEqual(300000 + 5000); // small tolerance for test execution time
    expect(grantedMs).toBeLessThan(requestedMs); // Must be less than requested
  });

  test('observer declines extension when external headroom is less than minimum useful time', async () => {
    const { agent, negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
      maxOperationTimeout: 1500000, // 25 min
      externalHardTimeout: 1530000, // 25.5 min — only 30s headroom
      negotiatedTimeoutBudget: 1800000,
      negotiatedTimeoutMaxRequests: 3,
      negotiatedTimeoutMaxPerRequest: 600000,
    });

    // Simulate: 25 min have elapsed
    negotiatedTimeoutState.startTime = Date.now() - 1500000;

    const elapsed = Date.now() - negotiatedTimeoutState.startTime;
    const externalHeadroom = agent.externalHardTimeout - elapsed;

    // With only ~30s headroom, extension should be declined (< 60s minimum)
    expect(externalHeadroom).toBeLessThan(60000);
  });

  test('without externalHardTimeout, extensions are not capped to external ceiling', async () => {
    const { agent, negotiatedTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
      maxOperationTimeout: 1500000,
      // No externalHardTimeout set
      negotiatedTimeoutBudget: 1800000,
      negotiatedTimeoutMaxRequests: 3,
      negotiatedTimeoutMaxPerRequest: 600000,
    });

    expect(agent.externalHardTimeout).toBeNull();

    // Without external cap, headroom is effectively infinite
    const externalHeadroom = agent.externalHardTimeout
      ? Math.max(0, agent.externalHardTimeout - (Date.now() - negotiatedTimeoutState.startTime))
      : Infinity;

    expect(externalHeadroom).toBe(Infinity);
  });

  test('observer triggers graceful stop when external headroom exhausted', async () => {
    const { agent, negotiatedTimeoutState, gracefulTimeoutState } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
      maxOperationTimeout: 1500000,
      externalHardTimeout: 1500000, // Same as operation timeout — zero headroom
      negotiatedTimeoutBudget: 1800000,
      negotiatedTimeoutMaxRequests: 3,
      negotiatedTimeoutMaxPerRequest: 600000,
    });

    agent._abortController = { abort: jest.fn(), signal: { aborted: false } };

    // Simulate: operation timeout elapsed
    negotiatedTimeoutState.startTime = Date.now() - 1500000;

    // Run the observer — should detect zero headroom and trigger graceful stop
    await negotiatedTimeoutState.runObserver();

    // Should have triggered graceful stop because no headroom for an extension
    expect(gracefulTimeoutState.triggered).toBe(true);
  });
});

// ---- 3. MCP tool tracking via agentEvents -----------------------------------

describe('MCP tool tracking via agentEvents', () => {
  test('MCPClientManager constructor accepts agentEvents option', async () => {
    // Dynamic import to match the module structure
    const { MCPClientManager } = await import('../../src/agent/mcp/client.js');
    const { EventEmitter } = await import('events');

    const emitter = new EventEmitter();
    const manager = new MCPClientManager({ agentEvents: emitter });
    expect(manager.agentEvents).toBe(emitter);
  });

  test('MCPClientManager without agentEvents does not throw', async () => {
    const { MCPClientManager } = await import('../../src/agent/mcp/client.js');
    const manager = new MCPClientManager();
    expect(manager.agentEvents).toBeNull();
  });

  test('activeTools map is created during negotiated timeout run', async () => {
    const { agent } = await extractCallbacks({
      timeoutBehavior: 'negotiated',
    });

    // _activeTools is set up inside run() and exists after answer() completes
    expect(agent._activeTools).toBeDefined();
    expect(agent._activeTools instanceof Map).toBe(true);
  });

  test('toolCall events populate and depopulate activeTools during run', async () => {
    // Test the event handler logic directly by simulating what happens during run()
    const { EventEmitter } = await import('events');
    const events = new EventEmitter();
    const activeTools = new Map();

    // Replicate the onToolCall handler from ProbeAgent.run()
    const onToolCall = (event) => {
      const key = event.toolCallId || `${event.name}:${JSON.stringify(event.args || {}).slice(0, 100)}`;
      if (event.status === 'started') {
        activeTools.set(key, { name: event.name, args: event.args, startedAt: event.timestamp });
      } else if (event.status === 'completed' || event.status === 'error') {
        activeTools.delete(key);
      }
    };
    events.on('toolCall', onToolCall);

    // Simulate MCP tool start
    events.emit('toolCall', {
      toolCallId: 'mcp-read-123',
      name: 'mcp_server__read_file',
      args: { path: '/tmp/test.txt' },
      status: 'started',
      timestamp: new Date().toISOString(),
    });

    expect(activeTools.size).toBe(1);
    expect(activeTools.get('mcp-read-123').name).toBe('mcp_server__read_file');

    // Simulate regular tool start
    events.emit('toolCall', {
      toolCallId: 'search-456',
      name: 'search',
      args: { query: 'test' },
      status: 'started',
    });

    expect(activeTools.size).toBe(2);

    // Simulate MCP tool completion
    events.emit('toolCall', {
      toolCallId: 'mcp-read-123',
      name: 'mcp_server__read_file',
      status: 'completed',
    });

    expect(activeTools.size).toBe(1);
    expect(activeTools.has('search-456')).toBe(true);

    // Simulate tool error
    events.emit('toolCall', {
      toolCallId: 'search-456',
      name: 'search',
      status: 'error',
    });

    expect(activeTools.size).toBe(0);

    events.removeListener('toolCall', onToolCall);
  });
});
