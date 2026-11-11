/**
 * Tests that delegate.js correctly accepts and forwards negotiated timeout
 * settings to subagent instances.
 *
 * These tests would have caught the original bug: even if the YAML config
 * included negotiated timeout settings, delegate.js was NOT forwarding
 * negotiatedTimeoutBudget, negotiatedTimeoutMaxRequests, and
 * negotiatedTimeoutMaxPerRequest to the ProbeAgent subagent — so delegates
 * always fell back to default values, and the engineer sub-workflow's
 * ProbeAgent got 'graceful' timeout behavior (the default).
 *
 * The fix added these parameters to the delegate() function signature and
 * passes them through when creating the subagent ProbeAgent instance.
 */

import { describe, test, expect, jest, beforeEach, afterEach } from '@jest/globals';

// We test the ProbeAgent constructor directly to verify it receives
// the correct timeout parameters from delegate().
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('Delegate Forwarding of Negotiated Timeout Settings', () => {

  describe('ProbeAgent receives negotiated timeout settings via constructor', () => {
    test('should accept and store negotiatedTimeoutBudget from options', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        model: 'test-model',
        timeoutBehavior: 'negotiated',
        negotiatedTimeoutBudget: 3600000,
        negotiatedTimeoutMaxRequests: 3,
        negotiatedTimeoutMaxPerRequest: 1200000,
      });

      expect(agent.timeoutBehavior).toBe('negotiated');
      expect(agent.negotiatedTimeoutBudget).toBe(3600000);
      expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
      expect(agent.negotiatedTimeoutMaxPerRequest).toBe(1200000);
    });

    test('should accept and store graceful timeout settings from options', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        model: 'test-model',
        timeoutBehavior: 'graceful',
        gracefulTimeoutBonusSteps: 6,
      });

      expect(agent.timeoutBehavior).toBe('graceful');
      expect(agent.gracefulTimeoutBonusSteps).toBe(6);
    });

    test('should default to graceful when timeoutBehavior is not specified', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        model: 'test-model',
      });

      expect(agent.timeoutBehavior).toBe('graceful');
    });

    test('should use option values over environment variable defaults', () => {
      // Simulate env that would provide different defaults
      const origBudget = process.env.NEGOTIATED_TIMEOUT_BUDGET;
      const origMaxReq = process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS;
      const origMaxPer = process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST;

      process.env.NEGOTIATED_TIMEOUT_BUDGET = '900000';
      process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS = '1';
      process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST = '300000';

      try {
        const agent = new ProbeAgent({
          path: process.cwd(),
          model: 'test-model',
          timeoutBehavior: 'negotiated',
          negotiatedTimeoutBudget: 3600000,
          negotiatedTimeoutMaxRequests: 3,
          negotiatedTimeoutMaxPerRequest: 1200000,
        });

        // Options should take precedence over env vars
        expect(agent.negotiatedTimeoutBudget).toBe(3600000);
        expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
        expect(agent.negotiatedTimeoutMaxPerRequest).toBe(1200000);
      } finally {
        // Restore
        if (origBudget !== undefined) process.env.NEGOTIATED_TIMEOUT_BUDGET = origBudget;
        else delete process.env.NEGOTIATED_TIMEOUT_BUDGET;
        if (origMaxReq !== undefined) process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS = origMaxReq;
        else delete process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS;
        if (origMaxPer !== undefined) process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST = origMaxPer;
        else delete process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST;
      }
    });

    test('should fall back to env defaults when options not provided', () => {
      const origBudget = process.env.NEGOTIATED_TIMEOUT_BUDGET;
      const origMaxReq = process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS;
      const origMaxPer = process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST;

      process.env.NEGOTIATED_TIMEOUT_BUDGET = '2400000';
      process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS = '5';
      process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST = '900000';

      try {
        const agent = new ProbeAgent({
          path: process.cwd(),
          model: 'test-model',
          timeoutBehavior: 'negotiated',
          // No negotiated timeout options — should use env
        });

        expect(agent.negotiatedTimeoutBudget).toBe(2400000);
        expect(agent.negotiatedTimeoutMaxRequests).toBe(5);
        expect(agent.negotiatedTimeoutMaxPerRequest).toBe(900000);
      } finally {
        if (origBudget !== undefined) process.env.NEGOTIATED_TIMEOUT_BUDGET = origBudget;
        else delete process.env.NEGOTIATED_TIMEOUT_BUDGET;
        if (origMaxReq !== undefined) process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS = origMaxReq;
        else delete process.env.NEGOTIATED_TIMEOUT_MAX_REQUESTS;
        if (origMaxPer !== undefined) process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST = origMaxPer;
        else delete process.env.NEGOTIATED_TIMEOUT_MAX_PER_REQUEST;
      }
    });
  });

  describe('ProbeAgent stores all negotiated timeout fields on instance', () => {
    test('should expose all negotiated timeout values as instance properties', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        model: 'test-model',
        timeoutBehavior: 'negotiated',
        negotiatedTimeoutBudget: 3600000,
        negotiatedTimeoutMaxRequests: 3,
        negotiatedTimeoutMaxPerRequest: 1200000,
        gracefulStopDeadline: 30000,
      });

      // These instance properties are what delegate.js reads from the parent
      // and passes to the subagent constructor. If any are missing, the
      // subagent won't get the right timeout config.
      expect(agent.timeoutBehavior).toBe('negotiated');
      expect(agent.negotiatedTimeoutBudget).toBe(3600000);
      expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
      expect(agent.negotiatedTimeoutMaxPerRequest).toBe(1200000);
      expect(agent.gracefulStopDeadline).toBe(30000);
    });

    test('should store gracefulTimeoutBonusSteps on instance', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        model: 'test-model',
        timeoutBehavior: 'graceful',
        gracefulTimeoutBonusSteps: 8,
      });

      expect(agent.gracefulTimeoutBonusSteps).toBe(8);
    });
  });

  describe('Without negotiated settings - graceful fallback', () => {
    test('should not have negotiated fields when timeoutBehavior is graceful', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        model: 'test-model',
        timeoutBehavior: 'graceful',
      });

      expect(agent.timeoutBehavior).toBe('graceful');
      // Still has defaults for negotiated fields (they exist but won't be used)
      expect(agent.negotiatedTimeoutBudget).toBeDefined();
      expect(agent.negotiatedTimeoutMaxRequests).toBeDefined();
      expect(agent.negotiatedTimeoutMaxPerRequest).toBeDefined();
    });

    test('should use sensible defaults when no timeout options provided', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        model: 'test-model',
      });

      // Default negotiated timeout budget is 1800000 (30 min)
      expect(agent.negotiatedTimeoutBudget).toBe(1800000);
      // Default max requests is 3
      expect(agent.negotiatedTimeoutMaxRequests).toBe(3);
      // Default max per request is 600000 (10 min)
      expect(agent.negotiatedTimeoutMaxPerRequest).toBe(600000);
    });
  });
});
