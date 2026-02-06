/**
 * Tests for AbortSignal handling in timeout functionality
 */

import { describe, test, expect, jest, beforeEach } from '@jest/globals';

describe('AbortSignal Handling', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('AbortController behavior', () => {
    test('should create AbortController with working signal', () => {
      const controller = new AbortController();
      expect(controller.signal.aborted).toBe(false);

      controller.abort();
      expect(controller.signal.aborted).toBe(true);
    });

    test('should abort with custom reason', () => {
      const controller = new AbortController();
      const reason = new Error('Operation timed out');

      controller.abort(reason);
      expect(controller.signal.aborted).toBe(true);
      expect(controller.signal.reason).toBe(reason);
    });

    test('should trigger abort event listener', (done) => {
      const controller = new AbortController();

      controller.signal.addEventListener('abort', () => {
        expect(controller.signal.aborted).toBe(true);
        done();
      });

      controller.abort();
    });
  });

  describe('Timeout with AbortController pattern', () => {
    test('should abort after timeout', async () => {
      const controller = new AbortController();
      const timeoutMs = 50;

      const timeoutId = setTimeout(() => {
        controller.abort();
      }, timeoutMs);

      // Wait for timeout
      await new Promise(resolve => setTimeout(resolve, timeoutMs + 10));

      expect(controller.signal.aborted).toBe(true);
      clearTimeout(timeoutId);
    });

    test('should not abort if cleared before timeout', async () => {
      const controller = new AbortController();
      const timeoutMs = 100;

      const timeoutId = setTimeout(() => {
        controller.abort();
      }, timeoutMs);

      // Clear before timeout
      clearTimeout(timeoutId);

      // Wait past the original timeout
      await new Promise(resolve => setTimeout(resolve, timeoutMs + 10));

      expect(controller.signal.aborted).toBe(false);
    });

    test('should race promise with abort signal', async () => {
      const controller = new AbortController();

      // Simulate slow operation
      const slowOperation = new Promise((resolve) => {
        setTimeout(() => resolve('completed'), 200);
      });

      // Abort after 50ms
      setTimeout(() => controller.abort(), 50);

      // Create abort promise
      const abortPromise = new Promise((_, reject) => {
        controller.signal.addEventListener('abort', () => {
          reject(new Error('Operation aborted'));
        });
      });

      // Race
      await expect(Promise.race([slowOperation, abortPromise]))
        .rejects.toThrow('Operation aborted');
    });
  });

  describe('Generator with abort signal', () => {
    test('should stop generator when signal is aborted', async () => {
      const controller = new AbortController();
      const results = [];

      async function* generateWithAbort(signal) {
        for (let i = 0; i < 10; i++) {
          if (signal.aborted) {
            throw new Error('Operation aborted');
          }
          yield i;
          // Simulate async work
          await new Promise(resolve => setTimeout(resolve, 10));
        }
      }

      // Abort after 35ms (should allow ~3 iterations)
      setTimeout(() => controller.abort(), 35);

      try {
        for await (const value of generateWithAbort(controller.signal)) {
          results.push(value);
        }
      } catch (error) {
        expect(error.message).toBe('Operation aborted');
      }

      // Should have stopped before completing all 10
      expect(results.length).toBeLessThan(10);
      expect(results.length).toBeGreaterThan(0);
    });

    test('should complete generator if not aborted', async () => {
      const controller = new AbortController();
      const results = [];

      async function* generateWithAbort(signal) {
        for (let i = 0; i < 5; i++) {
          if (signal.aborted) {
            throw new Error('Operation aborted');
          }
          yield i;
        }
      }

      for await (const value of generateWithAbort(controller.signal)) {
        results.push(value);
      }

      expect(results).toEqual([0, 1, 2, 3, 4]);
    });
  });

  describe('Activity timeout simulation', () => {
    test('should detect stalled stream (no activity)', async () => {
      const activityTimeoutMs = 50;
      let lastActivity = Date.now();

      async function* simulateStalledStream() {
        yield 'first';
        // Simulate stall - wait longer than activity timeout
        await new Promise(resolve => setTimeout(resolve, activityTimeoutMs + 20));

        const now = Date.now();
        if (now - lastActivity > activityTimeoutMs) {
          throw new Error(`Stream timeout - no activity for ${activityTimeoutMs}ms`);
        }
        lastActivity = now;
        yield 'second'; // Should not reach here
      }

      const results = [];

      await expect(async () => {
        for await (const value of simulateStalledStream()) {
          results.push(value);
          lastActivity = Date.now();
        }
      }).rejects.toThrow(/no activity/);

      expect(results).toEqual(['first']);
    });

    test('should allow slow but active stream', async () => {
      const activityTimeoutMs = 100;
      let lastActivity = Date.now();

      async function* simulateSlowStream() {
        for (let i = 0; i < 3; i++) {
          const now = Date.now();
          if (now - lastActivity > activityTimeoutMs) {
            throw new Error(`Stream timeout - no activity for ${activityTimeoutMs}ms`);
          }
          lastActivity = now;
          yield i;
          // Wait less than activity timeout
          await new Promise(resolve => setTimeout(resolve, 30));
        }
      }

      const results = [];
      for await (const value of simulateSlowStream()) {
        results.push(value);
        lastActivity = Date.now();
      }

      expect(results).toEqual([0, 1, 2]);
    });
  });

  describe('Request timeout simulation', () => {
    test('should timeout after total time exceeded', async () => {
      const requestTimeoutMs = 100;
      const startTime = Date.now();

      async function* simulateLongStream() {
        for (let i = 0; i < 10; i++) {
          const elapsed = Date.now() - startTime;
          if (elapsed > requestTimeoutMs) {
            throw new Error(`Request timeout - exceeded ${requestTimeoutMs}ms`);
          }
          yield i;
          await new Promise(resolve => setTimeout(resolve, 30));
        }
      }

      const results = [];

      await expect(async () => {
        for await (const value of simulateLongStream()) {
          results.push(value);
        }
      }).rejects.toThrow(/Request timeout/);

      // Should have gotten some results before timeout
      expect(results.length).toBeGreaterThan(0);
      expect(results.length).toBeLessThan(10);
    });
  });
});
