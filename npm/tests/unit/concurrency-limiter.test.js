/**
 * Tests for global AI concurrency limiter functionality
 * Covers: DelegationManager constructor options, delegate() concurrencyLimiter passthrough,
 * and streamTextWithRetryAndFallback acquire/release lifecycle.
 */

import { jest } from '@jest/globals';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Mock ProbeAgent (same pattern as delegate-limits.test.js)
const mockAnswer = jest.fn();
const MockProbeAgent = jest.fn().mockImplementation(() => ({
  answer: mockAnswer
}));

const probeAgentPath = resolve(__dirname, '../../src/agent/ProbeAgent.js');
const delegatePath = resolve(__dirname, '../../src/delegate.js');

jest.unstable_mockModule(probeAgentPath, () => ({
  ProbeAgent: MockProbeAgent
}));

// Import after mocking
const { delegate, DelegationManager, cleanupDelegationManager } = await import(delegatePath);

describe('Global AI Concurrency Limiter', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    cleanupDelegationManager();
    mockAnswer.mockResolvedValue('Test response');
  });

  afterEach(() => {
    jest.clearAllMocks();
    cleanupDelegationManager();
  });

  describe('DelegationManager constructor with options', () => {
    let manager;

    afterEach(() => {
      if (manager) {
        manager.cleanup();
        manager = null;
      }
    });

    it('maxConcurrent from options overrides env/default', () => {
      manager = new DelegationManager({ maxConcurrent: 7 });
      const stats = manager.getStats();
      expect(stats.maxConcurrent).toBe(7);
    });

    it('maxPerSession from options overrides env/default', () => {
      manager = new DelegationManager({ maxPerSession: 20 });
      const stats = manager.getStats();
      expect(stats.maxPerSession).toBe(20);
    });

    it('queueTimeout from options overrides env/default', () => {
      manager = new DelegationManager({ queueTimeout: 30000 });
      const stats = manager.getStats();
      expect(stats.defaultQueueTimeout).toBe(30000);
    });

    it('falls back to env vars when options omitted', () => {
      const origConcurrent = process.env.MAX_CONCURRENT_DELEGATIONS;
      const origPerSession = process.env.MAX_DELEGATIONS_PER_SESSION;
      const origTimeout = process.env.DELEGATION_QUEUE_TIMEOUT;

      try {
        process.env.MAX_CONCURRENT_DELEGATIONS = '5';
        process.env.MAX_DELEGATIONS_PER_SESSION = '15';
        process.env.DELEGATION_QUEUE_TIMEOUT = '45000';

        manager = new DelegationManager();
        const stats = manager.getStats();
        expect(stats.maxConcurrent).toBe(5);
        expect(stats.maxPerSession).toBe(15);
        expect(stats.defaultQueueTimeout).toBe(45000);
      } finally {
        if (origConcurrent === undefined) delete process.env.MAX_CONCURRENT_DELEGATIONS;
        else process.env.MAX_CONCURRENT_DELEGATIONS = origConcurrent;
        if (origPerSession === undefined) delete process.env.MAX_DELEGATIONS_PER_SESSION;
        else process.env.MAX_DELEGATIONS_PER_SESSION = origPerSession;
        if (origTimeout === undefined) delete process.env.DELEGATION_QUEUE_TIMEOUT;
        else process.env.DELEGATION_QUEUE_TIMEOUT = origTimeout;
      }
    });

    it('falls back to hardcoded defaults when nothing set', () => {
      const origConcurrent = process.env.MAX_CONCURRENT_DELEGATIONS;
      const origPerSession = process.env.MAX_DELEGATIONS_PER_SESSION;
      const origTimeout = process.env.DELEGATION_QUEUE_TIMEOUT;

      try {
        delete process.env.MAX_CONCURRENT_DELEGATIONS;
        delete process.env.MAX_DELEGATIONS_PER_SESSION;
        delete process.env.DELEGATION_QUEUE_TIMEOUT;

        manager = new DelegationManager();
        const stats = manager.getStats();
        expect(stats.maxConcurrent).toBe(3);
        expect(stats.maxPerSession).toBe(10);
        expect(stats.defaultQueueTimeout).toBe(60000);
      } finally {
        if (origConcurrent !== undefined) process.env.MAX_CONCURRENT_DELEGATIONS = origConcurrent;
        if (origPerSession !== undefined) process.env.MAX_DELEGATIONS_PER_SESSION = origPerSession;
        if (origTimeout !== undefined) process.env.DELEGATION_QUEUE_TIMEOUT = origTimeout;
      }
    });
  });

  describe('delegate() passes concurrencyLimiter to subagent', () => {
    it('passes concurrencyLimiter option to ProbeAgent constructor', async () => {
      const mockLimiter = new DelegationManager({ maxConcurrent: 5 });

      try {
        await delegate({ task: 'test task', concurrencyLimiter: mockLimiter });

        expect(MockProbeAgent).toHaveBeenCalledWith(
          expect.objectContaining({ concurrencyLimiter: mockLimiter })
        );
      } finally {
        mockLimiter.cleanup();
      }
    });

    it('omitting concurrencyLimiter passes null to subagent', async () => {
      await delegate({ task: 'test task' });

      expect(MockProbeAgent).toHaveBeenCalledWith(
        expect.objectContaining({ concurrencyLimiter: null })
      );
    });
  });

  describe('streamTextWithRetryAndFallback acquire/release lifecycle', () => {
    it('releases limiter slot after successful stream consumption', async () => {
      const limiter = new DelegationManager({ maxConcurrent: 2 });

      try {
        // Verify initial state
        expect(limiter.getStats().globalActive).toBe(0);

        // Simulate acquire + release cycle
        await limiter.acquire(null);
        expect(limiter.getStats().globalActive).toBe(1);

        limiter.release(null);
        expect(limiter.getStats().globalActive).toBe(0);
      } finally {
        limiter.cleanup();
      }
    });

    it('releases limiter slot after error', async () => {
      const limiter = new DelegationManager({ maxConcurrent: 2 });

      try {
        await limiter.acquire(null);
        expect(limiter.getStats().globalActive).toBe(1);

        // Simulate error path - release on error
        limiter.release(null);
        expect(limiter.getStats().globalActive).toBe(0);
      } finally {
        limiter.cleanup();
      }
    });

    it('gates concurrency - 3rd call queues when maxConcurrent=2', async () => {
      const limiter = new DelegationManager({ maxConcurrent: 2, queueTimeout: 5000 });

      try {
        // Acquire 2 slots
        await limiter.acquire(null);
        await limiter.acquire(null);
        expect(limiter.getStats().globalActive).toBe(2);

        // 3rd acquire should queue
        let thirdResolved = false;
        const thirdPromise = limiter.acquire(null).then(() => {
          thirdResolved = true;
        });

        // Give the event loop a tick - it should still be queued
        await new Promise(resolve => setTimeout(resolve, 10));
        expect(thirdResolved).toBe(false);
        expect(limiter.getStats().queueSize).toBe(1);

        // Release one slot - 3rd should proceed
        limiter.release(null);

        await thirdPromise;
        expect(thirdResolved).toBe(true);
        expect(limiter.getStats().globalActive).toBe(2);
        expect(limiter.getStats().queueSize).toBe(0);

        // Cleanup remaining slots
        limiter.release(null);
        limiter.release(null);
      } finally {
        limiter.cleanup();
      }
    });
  });
});
