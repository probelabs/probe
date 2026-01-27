/**
 * Tests for RetryManager
 */

import { describe, test, expect, jest, beforeEach } from '@jest/globals';
import { RetryManager, isRetryableError, DEFAULT_RETRYABLE_ERRORS } from '../../src/agent/RetryManager.js';

describe('RetryManager', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('constructor', () => {
    test('should initialize with default values', () => {
      const retry = new RetryManager();
      expect(retry.maxRetries).toBe(3);
      expect(retry.initialDelay).toBe(1000);
      expect(retry.maxDelay).toBe(30000);
      expect(retry.backoffFactor).toBe(2);
      expect(retry.debug).toBe(false);
    });

    test('should accept custom configuration', () => {
      const retry = new RetryManager({
        maxRetries: 5,
        initialDelay: 500,
        maxDelay: 10000,
        backoffFactor: 3,
        debug: true
      });

      expect(retry.maxRetries).toBe(5);
      expect(retry.initialDelay).toBe(500);
      expect(retry.maxDelay).toBe(10000);
      expect(retry.backoffFactor).toBe(3);
      expect(retry.debug).toBe(true);
    });
  });

  describe('isRetryableError', () => {
    test('should identify overloaded errors', () => {
      const error = new Error('Overloaded');
      expect(isRetryableError(error)).toBe(true);
    });

    test('should identify rate limit errors', () => {
      const error = new Error('rate_limit exceeded');
      expect(isRetryableError(error)).toBe(true);
    });

    test('should identify 429 status code', () => {
      const error = new Error('Request failed');
      error.statusCode = 429;
      expect(isRetryableError(error)).toBe(true);
    });

    test('should identify 503 status code', () => {
      const error = new Error('Service unavailable');
      error.status = 503;
      expect(isRetryableError(error)).toBe(true);
    });

    test('should identify timeout errors', () => {
      const error = new Error('Request timeout');
      expect(isRetryableError(error)).toBe(true);
    });

    test('should identify network errors', () => {
      const error = new Error('Network failure');
      error.code = 'ECONNRESET';
      expect(isRetryableError(error)).toBe(true);
    });

    test('should not identify non-retryable errors', () => {
      const error = new Error('Invalid API key');
      expect(isRetryableError(error)).toBe(false);
    });

    test('should handle api_error type', () => {
      const error = new Error('API Error');
      error.type = 'api_error';
      expect(isRetryableError(error)).toBe(true);
    });
  });

  describe('executeWithRetry', () => {
    test('should succeed on first attempt', async () => {
      const retry = new RetryManager({ debug: false });
      const mockFn = jest.fn().mockResolvedValue('success');

      const result = await retry.executeWithRetry(mockFn);

      expect(result).toBe('success');
      expect(mockFn).toHaveBeenCalledTimes(1);
      expect(retry.stats.totalAttempts).toBe(1);
      expect(retry.stats.totalRetries).toBe(0);
    });

    test('should retry on retryable error and succeed', async () => {
      const retry = new RetryManager({
        maxRetries: 3,
        initialDelay: 10,
        debug: false
      });

      let attemptCount = 0;
      const mockFn = jest.fn().mockImplementation(() => {
        attemptCount++;
        if (attemptCount < 3) {
          const error = new Error('Overloaded');
          throw error;
        }
        return Promise.resolve('success');
      });

      const result = await retry.executeWithRetry(mockFn, { provider: 'test' });

      expect(result).toBe('success');
      expect(mockFn).toHaveBeenCalledTimes(3);
      expect(retry.stats.totalAttempts).toBe(3);
      expect(retry.stats.totalRetries).toBe(2);
      expect(retry.stats.successfulRetries).toBe(1);
    });

    test('should fail immediately on non-retryable error', async () => {
      const retry = new RetryManager({ debug: false });
      const mockFn = jest.fn().mockRejectedValue(new Error('Invalid API key'));

      await expect(retry.executeWithRetry(mockFn)).rejects.toThrow('Invalid API key');
      expect(mockFn).toHaveBeenCalledTimes(1);
      expect(retry.stats.totalAttempts).toBe(1);
      expect(retry.stats.totalRetries).toBe(0);
    });

    test('should exhaust retries and fail', async () => {
      const retry = new RetryManager({
        maxRetries: 2,
        initialDelay: 10,
        debug: false
      });
      const mockFn = jest.fn().mockRejectedValue(new Error('Overloaded'));

      await expect(retry.executeWithRetry(mockFn)).rejects.toThrow('Overloaded');
      expect(mockFn).toHaveBeenCalledTimes(3); // 1 initial + 2 retries
      expect(retry.stats.totalAttempts).toBe(3);
      expect(retry.stats.failedRetries).toBe(1);
    });

    test('should apply exponential backoff', async () => {
      const retry = new RetryManager({
        maxRetries: 3,
        initialDelay: 100,
        backoffFactor: 2,
        jitter: false,  // Disable jitter for predictable timing
        debug: false
      });

      const timestamps = [];
      const mockFn = jest.fn().mockImplementation(() => {
        timestamps.push(Date.now());
        if (timestamps.length < 3) {
          throw new Error('Overloaded');
        }
        return Promise.resolve('success');
      });

      await retry.executeWithRetry(mockFn);

      // Verify delays are increasing (with some tolerance for timing and test execution)
      const delay1 = timestamps[1] - timestamps[0];
      const delay2 = timestamps[2] - timestamps[1];

      // With jitter disabled: delay1 should be ~100ms, delay2 should be ~200ms
      // Allow more tolerance for CI/test timing variance
      expect(delay1).toBeGreaterThanOrEqual(75);  // Allow more tolerance
      expect(delay2).toBeGreaterThanOrEqual(150); // Allow more tolerance
      expect(delay2).toBeGreaterThan(delay1);     // Exponential increase
    });

    test('should respect maxDelay cap', async () => {
      const retry = new RetryManager({
        maxRetries: 5,
        initialDelay: 1000,
        maxDelay: 2000,
        backoffFactor: 10,
        jitter: false, // Disable jitter for predictable timing
        debug: false
      });

      let attemptCount = 0;
      const timestamps = [];
      const mockFn = jest.fn().mockImplementation(() => {
        timestamps.push(Date.now());
        attemptCount++;
        if (attemptCount < 3) {
          throw new Error('Overloaded');
        }
        return Promise.resolve('success');
      });

      await retry.executeWithRetry(mockFn);

      // With backoffFactor of 10, delay should be capped at maxDelay (2000ms)
      const delay2 = timestamps[2] - timestamps[1];
      // Allow small timing variance in CI/runtime scheduling
      expect(delay2).toBeGreaterThanOrEqual(1950);
      expect(delay2).toBeLessThan(2100); // Should not exceed maxDelay significantly
    });

    test('should track statistics correctly', async () => {
      const retry = new RetryManager({ maxRetries: 3, initialDelay: 10, debug: false });

      // Successful on first attempt
      await retry.executeWithRetry(jest.fn().mockResolvedValue('ok'));

      // Successful after 2 retries
      let count1 = 0;
      await retry.executeWithRetry(jest.fn().mockImplementation(() => {
        count1++;
        if (count1 < 3) throw new Error('Overloaded');
        return 'ok';
      }));

      // Failed after exhausting retries
      try {
        await retry.executeWithRetry(jest.fn().mockRejectedValue(new Error('Overloaded')));
      } catch (e) {
        // Expected to fail
      }

      const stats = retry.getStats();
      expect(stats.totalAttempts).toBe(8); // 1 + 3 + 4 (last one tries 1+maxRetries=4)
      expect(stats.totalRetries).toBe(5); // 0 + 2 + 3
      expect(stats.successfulRetries).toBe(1);
      expect(stats.failedRetries).toBe(1);
    });

    test('should reset statistics', () => {
      const retry = new RetryManager();
      retry.stats.totalAttempts = 10;
      retry.stats.totalRetries = 5;

      retry.resetStats();

      expect(retry.stats.totalAttempts).toBe(0);
      expect(retry.stats.totalRetries).toBe(0);
    });
  });

  describe('custom retryable errors', () => {
    test('should use custom retryable error patterns', async () => {
      const retry = new RetryManager({
        maxRetries: 2,
        initialDelay: 10,
        retryableErrors: ['CustomError', 'SpecialFailure'],
        debug: false
      });

      let attemptCount = 0;
      const mockFn = jest.fn().mockImplementation(() => {
        attemptCount++;
        if (attemptCount < 2) {
          throw new Error('CustomError occurred');
        }
        return Promise.resolve('success');
      });

      const result = await retry.executeWithRetry(mockFn);
      expect(result).toBe('success');
      expect(mockFn).toHaveBeenCalledTimes(2);
    });

    test('should not retry on default errors with custom list', async () => {
      const retry = new RetryManager({
        maxRetries: 2,
        initialDelay: 10,
        retryableErrors: ['CustomError'],
        debug: false
      });

      const mockFn = jest.fn().mockRejectedValue(new Error('Overloaded'));

      await expect(retry.executeWithRetry(mockFn)).rejects.toThrow('Overloaded');
      expect(mockFn).toHaveBeenCalledTimes(1); // Should fail immediately
    });
  });

  describe('edge cases', () => {
    test('should handle maxRetries = 0', async () => {
      const retry = new RetryManager({ maxRetries: 0, debug: false });
      const mockFn = jest.fn().mockRejectedValue(new Error('Overloaded'));

      await expect(retry.executeWithRetry(mockFn)).rejects.toThrow('Overloaded');
      expect(mockFn).toHaveBeenCalledTimes(1);
    });

    test('should handle errors without message', async () => {
      const retry = new RetryManager({
        maxRetries: 3,
        initialDelay: 5,
        debug: false
      });
      const error = new Error();
      error.type = 'api_error';  // This is retryable

      const mockFn = jest.fn().mockRejectedValue(error);

      await expect(retry.executeWithRetry(mockFn)).rejects.toThrow();
      // Error is retryable (api_error), so should retry maxRetries times: 1 initial + 3 retries = 4
      expect(mockFn).toHaveBeenCalledTimes(4);
    });

    test('should handle null/undefined errors', () => {
      expect(isRetryableError(null)).toBe(false);
      expect(isRetryableError(undefined)).toBe(false);
    });
  });
});
