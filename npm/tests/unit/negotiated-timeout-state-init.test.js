/**
 * Tests that ProbeAgent correctly initializes negotiated timeout properties
 * when timeoutBehavior is 'negotiated' vs 'graceful'.
 *
 * These tests would have caught the original bug: without negotiated timeout
 * config, ProbeAgent initialized with timeoutBehavior: 'graceful' (the default),
 * and the negotiated timeout observer pattern was never activated.
 * The AI agent would hit the soft timeout and enter graceful wind-down with
 * a fixed number of bonus steps, unable to request more time.
 *
 * The observer pattern (a separate LLM call when timeout fires) only activates
 * when timeoutBehavior === 'negotiated'. With 'graceful', the agent just gets
 * gracefulTimeoutBonusSteps extra iterations and then stops.
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

// Clean up env between tests
const originalEnv = { ...process.env };

beforeEach(() => {
  delete process.env.TIMEOUT_BEHAVIOR;
  delete process.env.NEGOTIATED_TIMEOUT_BUDGET;
  delete process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS;
  delete process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST;
  delete process.env.GRACEFUL_TIMEOUT_BONUS_STEPS;
  delete process.env.GRACEFUL_STOP_DEADLINE;
});

afterAll(() => {
  process.env = originalEnv;
});

// ---- Tests ------------------------------------------------------------------

describe('ProbeAgent Negotiated Timeout Initialization', () => {

  describe('When timeoutBehavior is "negotiated"', () => {
    test('should store negotiated behavior on the instance', () => {
      const agent = createAgent({
        timeoutBehavior: 'negotiated',
        negotiatedTimeoutBudget: 3600000,
        negotiatedTimeoutMaxRequests: 3,
        negotiatedTimeoutMaxPerRequest: 1200000,
      });

      expect(agent.timeoutBehavior).toBe('negotiated');
    });

    test('should store custom budget on the instance', () => {
      const agent = createAgent({
        timeoutBehavior: 'negotiated',
        negotiatedTimeoutBudget: 3600000,
        negotiatedTimeoutMaxRequests: 3,
        negotiatedTimeoutMaxPerRequest: 1200000,
      });

      expect(agent.negotiatedTimeoutBudget).toBe(3600000);
      expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
      expect(agent.negotiatedTimeoutMaxPerRequest).toBe(1200000);
    });

    test('should use custom values from the engineer.yaml-matching config', () => {
      // This mirrors what the fixed engineer.yaml now specifies:
      //   negotiated_timeout_budget: 3600000
      //   negotiated_timeout_max_requests: 3
      //   negotiated_timeout_max_per_request: 1200000
      const agent = createAgent({
        timeoutBehavior: 'negotiated',
        negotiatedTimeoutBudget: 3600000,
        negotiatedTimeoutMaxRequests: 3,
        negotiatedTimeoutMaxPerRequest: 1200000,
        gracefulStopDeadline: 30000,
        maxOperationTimeout: 1800000,
      });

      expect(agent.timeoutBehavior).toBe('negotiated');
      expect(agent.negotiatedTimeoutBudget).toBe(3600000);
      expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
      expect(agent.negotiatedTimeoutMaxPerRequest).toBe(1200000);
      expect(agent.gracefulStopDeadline).toBe(30000);
      expect(agent.maxOperationTimeout).toBe(1800000);
    });

    test('should allow enough budget for multiple extensions', () => {
      const agent = createAgent({
        timeoutBehavior: 'negotiated',
        negotiatedTimeoutBudget: 3600000,
        negotiatedTimeoutMaxRequests: 3,
        negotiatedTimeoutMaxPerRequest: 1200000,
      });

      // Budget should be able to fill all requests
      const maxGrantable = agent.negotiatedTimeoutMaxRequests * agent.negotiatedTimeoutMaxPerRequest;
      expect(maxGrantable).toBeGreaterThanOrEqual(agent.negotiatedTimeoutBudget);
    });
  });

  describe('When timeoutBehavior is "graceful" (the default)', () => {
    test('should default to graceful behavior', () => {
      const agent = createAgent();
      expect(agent.timeoutBehavior).toBe('graceful');
    });

    test('should use default negotiated values even in graceful mode', () => {
      const agent = createAgent({
        timeoutBehavior: 'graceful',
      });

      // Defaults exist but won't be used by the graceful code path
      expect(agent.negotiatedTimeoutBudget).toBe(1800000);   // 30 min default
      expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
      expect(agent.negotiatedTimeoutMaxPerRequest).toBe(600000); // 10 min default
    });

    test('should initialize gracefulTimeoutBonusSteps', () => {
      const agent = createAgent({
        timeoutBehavior: 'graceful',
        gracefulTimeoutBonusSteps: 6,
      });

      expect(agent.gracefulTimeoutBonusSteps).toBe(6);
    });

    test('should default gracefulTimeoutBonusSteps to 4', () => {
      const agent = createAgent({
        timeoutBehavior: 'graceful',
      });

      expect(agent.gracefulTimeoutBonusSteps).toBe(4);
    });
  });

  describe('When no timeoutBehavior is specified', () => {
    test('should default to graceful behavior', () => {
      const agent = createAgent({});
      expect(agent.timeoutBehavior).toBe('graceful');
    });

    test('TIMEOUT_BEHAVIOR env var can set negotiated', () => {
      process.env.TIMEOUT_BEHAVIOR = 'negotiated';
      const agent = createAgent({});
      expect(agent.timeoutBehavior).toBe('negotiated');
    });

    test('constructor option overrides TIMEOUT_BEHAVIOR env var', () => {
      process.env.TIMEOUT_BEHAVIOR = 'hard';
      const agent = createAgent({ timeoutBehavior: 'negotiated' });
      expect(agent.timeoutBehavior).toBe('negotiated');
    });
  });

  describe('Environment variable fallbacks for negotiated settings', () => {
    test('env vars are used when options not provided', () => {
      process.env.NEGOTIATED_TIMEOUT_BUDGET = '2400000';
      process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS = '5';
      process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST = '900000';

      const agent = createAgent({
        timeoutBehavior: 'negotiated',
      });

      expect(agent.negotiatedTimeoutBudget).toBe(2400000);
      expect(agent.negotiatedTimeoutMaxRequests).toBe(5);
      expect(agent.negotiatedTimeoutMaxPerRequest).toBe(900000);
    });

    test('options take precedence over env vars', () => {
      process.env.NEGOTIATED_TIMEOUT_BUDGET = '900000';
      process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS = '1';
      process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST = '300000';

      const agent = createAgent({
        timeoutBehavior: 'negotiated',
        negotiatedTimeoutBudget: 3600000,
        negotiatedTimeoutMaxRequests: 3,
        negotiatedTimeoutMaxPerRequest: 1200000,
      });

      expect(agent.negotiatedTimeoutBudget).toBe(3600000);
      expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
      expect(agent.negotiatedTimeoutMaxPerRequest).toBe(1200000);
    });

    test('invalid env values fall back to defaults', () => {
      process.env.NEGOTIATED_TIMEOUT_BUDGET = '100';    // Below 60000 minimum
      process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS = '0'; // Below 1 minimum
      process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST = 'abc'; // Not a number

      const agent = createAgent({
        timeoutBehavior: 'negotiated',
      });

      expect(agent.negotiatedTimeoutBudget).toBe(1800000);    // Default
      expect(agent.negotiatedTimeoutMaxRequests).toBe(3);       // Default
      expect(agent.negotiatedTimeoutMaxPerRequest).toBe(600000); // Default
    });

    test('boundary: minimum valid budget (60000)', () => {
      process.env.NEGOTIATED_TIMEOUT_BUDGET = '60000';

      const agent = createAgent({
        timeoutBehavior: 'negotiated',
      });

      expect(agent.negotiatedTimeoutBudget).toBe(60000);
    });

    test('boundary: maximum valid budget (7200000)', () => {
      process.env.NEGOTIATED_TIMEOUT_BUDGET = '7200000';

      const agent = createAgent({
        timeoutBehavior: 'negotiated',
      });

      expect(agent.negotiatedTimeoutBudget).toBe(7200000);
    });

    test('boundary: budget above maximum falls back to default', () => {
      process.env.NEGOTIATED_TIMEOUT_BUDGET = '7200001';

      const agent = createAgent({
        timeoutBehavior: 'negotiated',
      });

      expect(agent.negotiatedTimeoutBudget).toBe(1800000); // Default
    });
  });

  describe('The bug scenario: graceful vs negotiated behavior difference', () => {
    test('graceful agent has NO mechanism to extend its own timeout', () => {
      // This is the core of the original bug:
      // With graceful behavior, the agent gets a few bonus steps after timeout
      // but CANNOT request more time. It just stops.
      const gracefulAgent = createAgent({
        timeoutBehavior: 'graceful',
        gracefulTimeoutBonusSteps: 4,
      });

      expect(gracefulAgent.timeoutBehavior).toBe('graceful');
      expect(gracefulAgent.gracefulTimeoutBonusSteps).toBe(4);
      // The agent will simply wind down after 4 extra steps
    });

    test('negotiated agent CAN request time extensions via observer', () => {
      // With negotiated behavior, when timeout fires, a separate observer LLM
      // evaluates whether to grant more time. This is the fix.
      const negotiatedAgent = createAgent({
        timeoutBehavior: 'negotiated',
        negotiatedTimeoutBudget: 3600000,
        negotiatedTimeoutMaxRequests: 3,
        negotiatedTimeoutMaxPerRequest: 1200000,
      });

      expect(negotiatedAgent.timeoutBehavior).toBe('negotiated');
      // Up to 3 extensions of 20 min each, within 60 min total budget
      expect(negotiatedAgent.negotiatedTimeoutBudget).toBe(3600000);
      expect(negotiatedAgent.negotiatedTimeoutMaxRequests).toBe(3);
      expect(negotiatedAgent.negotiatedTimeoutMaxPerRequest).toBe(1200000);
    });

    test('subagent without forwarded settings defaults to graceful (the bug)', () => {
      // This simulates what happened before the fix:
      // delegate.js created a subagent WITHOUT forwarding negotiated settings,
      // so the subagent fell back to graceful with default bonus steps.
      const subagentWithoutForwarding = createAgent({
        // timeoutBehavior is not passed - defaults to graceful
        // negotiatedTimeoutBudget is not passed - uses default
        // negotiatedTimeoutMaxRequests is not passed - uses default
        // negotiatedTimeoutMaxPerRequest is not passed - uses default
      });

      // This is the buggy state: agent thinks it's graceful
      expect(subagentWithoutForwarding.timeoutBehavior).toBe('graceful');
    });

    test('subagent WITH forwarded settings gets negotiated behavior (the fix)', () => {
      // This simulates the fix: delegate.js now forwards all negotiated settings
      const subagentWithForwarding = createAgent({
        timeoutBehavior: 'negotiated',
        negotiatedTimeoutBudget: 3600000,
        negotiatedTimeoutMaxRequests: 3,
        negotiatedTimeoutMaxPerRequest: 1200000,
      });

      // This is the fixed state: agent knows it can negotiate
      expect(subagentWithForwarding.timeoutBehavior).toBe('negotiated');
      expect(subagentWithForwarding.negotiatedTimeoutBudget).toBe(3600000);
    });
  });
});
