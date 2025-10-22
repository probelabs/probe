/**
 * Integration tests for Retry + Fallback together
 */

import { describe, test, expect, jest, beforeEach } from '@jest/globals';
import { RetryManager } from '../../src/agent/RetryManager.js';
import { FallbackManager } from '../../src/agent/FallbackManager.js';

describe('Retry and Fallback Integration', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('Combined retry and fallback flow', () => {
    test('should retry on each provider before falling back', async () => {
      const retry = new RetryManager({
        maxRetries: 2,
        initialDelay: 10,
        debug: false
      });

      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1' },
          { provider: 'openai', apiKey: 'key2' },
          { provider: 'google', apiKey: 'key3' }
        ],
        debug: false
      });

      const callLog = [];

      // Mock function that tracks which provider is being used
      const mockFn = jest.fn().mockImplementation((provider, model, config) => {
        callLog.push(config.provider);

        // First provider (anthropic) fails 3 times (1 initial + 2 retries)
        if (config.provider === 'anthropic') {
          return retry.executeWithRetry(() => {
            throw new Error('Overloaded');
          }, { provider: 'anthropic' });
        }

        // Second provider (openai) fails 3 times
        if (config.provider === 'openai') {
          return retry.executeWithRetry(() => {
            throw new Error('Overloaded');
          }, { provider: 'openai' });
        }

        // Third provider (google) succeeds on second retry
        if (config.provider === 'google') {
          let attempts = 0;
          return retry.executeWithRetry(() => {
            attempts++;
            if (attempts < 2) {
              throw new Error('Overloaded');
            }
            return 'success from google';
          }, { provider: 'google' });
        }
      });

      const result = await fallback.executeWithFallback(mockFn);

      expect(result).toBe('success from google');
      expect(callLog).toEqual(['anthropic', 'openai', 'google']);

      // Verify fallback stats
      const fallbackStats = fallback.getStats();
      expect(fallbackStats.totalAttempts).toBe(3);
      expect(fallbackStats.successfulProvider).toContain('google');
      expect(fallbackStats.failedProviders.length).toBe(2);
    });

    test('should handle all providers exhausting retries', async () => {
      const retry = new RetryManager({
        maxRetries: 1,
        initialDelay: 5,
        debug: false
      });

      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1' },
          { provider: 'openai', apiKey: 'key2' }
        ],
        debug: false
      });

      const mockFn = jest.fn().mockImplementation((provider, model, config) => {
        return retry.executeWithRetry(() => {
          throw new Error('Overloaded');
        }, { provider: config.provider });
      });

      await expect(fallback.executeWithFallback(mockFn)).rejects.toThrow();

      // All providers should have been attempted
      expect(mockFn).toHaveBeenCalledTimes(2);

      const fallbackStats = fallback.getStats();
      expect(fallbackStats.totalAttempts).toBe(2);
      expect(fallbackStats.successfulProvider).toBeNull();
      expect(fallbackStats.failedProviders.length).toBe(2);
    });
  });

  describe('Real-world scenarios', () => {
    test('should handle Azure Claude -> Bedrock fallback scenario', async () => {
      const retry = new RetryManager({
        maxRetries: 3,
        initialDelay: 10,
        debug: false
      });

      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          {
            provider: 'anthropic',
            apiKey: 'azure-key',
            baseURL: 'https://azure-endpoint.com',
            model: 'claude-3-7-sonnet-20250219'
          },
          {
            provider: 'bedrock',
            region: 'us-west-2',
            apiKey: 'bedrock-key',
            model: 'anthropic.claude-sonnet-4-20250514-v1:0'
          }
        ],
        debug: false
      });

      let azureAttempts = 0;
      const mockFn = jest.fn().mockImplementation((provider, model, config) => {
        return retry.executeWithRetry(() => {
          // Azure fails with overloaded
          if (config.baseURL?.includes('azure')) {
            azureAttempts++;
            throw new Error('Overloaded');
          }

          // Bedrock succeeds
          return `success with ${model}`;
        }, { provider: config.provider, model });
      });

      const result = await fallback.executeWithFallback(mockFn);

      expect(result).toContain('anthropic.claude-sonnet');
      expect(azureAttempts).toBe(4); // 1 initial + 3 retries

      const stats = fallback.getStats();
      expect(stats.successfulProvider).toContain('bedrock');
    });

    test('should handle mixed retryable and non-retryable errors', async () => {
      const retry = new RetryManager({
        maxRetries: 2,
        initialDelay: 10,
        debug: false
      });

      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1' },
          { provider: 'openai', apiKey: 'key2' }
        ],
        debug: false
      });

      const mockFn = jest.fn().mockImplementation((provider, model, config) => {
        return retry.executeWithRetry(() => {
          // First provider: non-retryable error (invalid API key)
          if (config.provider === 'anthropic') {
            throw new Error('Invalid API key');
          }

          // Second provider: succeeds
          return 'success';
        }, { provider: config.provider });
      });

      const result = await fallback.executeWithFallback(mockFn);

      expect(result).toBe('success');

      // Anthropic should only be attempted once (non-retryable)
      // OpenAI should succeed on first attempt
      expect(mockFn).toHaveBeenCalledTimes(2);
    });

    test('should respect maxTotalAttempts across providers', async () => {
      const retry = new RetryManager({
        maxRetries: 5,
        initialDelay: 5,
        debug: false
      });

      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1' },
          { provider: 'openai', apiKey: 'key2' },
          { provider: 'google', apiKey: 'key3' },
          { provider: 'bedrock', apiKey: 'key4' }
        ],
        maxTotalAttempts: 2,  // Only allow 2 provider attempts total
        debug: false
      });

      const mockFn = jest.fn().mockImplementation((provider, model, config) => {
        return retry.executeWithRetry(() => {
          throw new Error('Overloaded');
        }, { provider: config.provider });
      });

      await expect(fallback.executeWithFallback(mockFn)).rejects.toThrow();

      // Should only attempt 2 providers
      expect(mockFn).toHaveBeenCalledTimes(2);
    });
  });

  describe('Statistics and monitoring', () => {
    test('should track detailed statistics across retry and fallback', async () => {
      const retry = new RetryManager({
        maxRetries: 2,
        initialDelay: 5,
        debug: false
      });

      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1' },
          { provider: 'openai', apiKey: 'key2' }
        ],
        debug: false
      });

      let anthropicAttempts = 0;
      const mockFn = jest.fn().mockImplementation((provider, model, config) => {
        if (config.provider === 'anthropic') {
          return retry.executeWithRetry(() => {
            anthropicAttempts++;
            throw new Error('Overloaded');
          }, { provider: 'anthropic' });
        }

        return retry.executeWithRetry(() => 'success', { provider: 'openai' });
      });

      await fallback.executeWithFallback(mockFn);

      // Check fallback stats
      const fallbackStats = fallback.getStats();
      expect(fallbackStats.totalAttempts).toBe(2);
      expect(fallbackStats.failedProviders.length).toBe(1);
      expect(fallbackStats.successfulProvider).toContain('openai');

      // Anthropic should have 3 total attempts (1 + 2 retries)
      expect(anthropicAttempts).toBe(3);
    });
  });

  describe('Edge cases', () => {
    test('should handle empty provider list gracefully', async () => {
      const fallback = new FallbackManager({
        strategy: 'any',
        providers: []
      });

      await expect(fallback.executeWithFallback(() => {})).rejects.toThrow('No providers configured');
    });

    test('should handle provider creation failures', async () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: '' },  // Empty API key might cause issues
          { provider: 'openai', apiKey: 'valid-key' }
        ],
        debug: false
      });

      const mockFn = jest.fn().mockImplementation((provider, model, config) => {
        if (config.provider === 'openai') {
          return 'success';
        }
        throw new Error('Provider creation failed');
      });

      // Should fallback to second provider
      const result = await fallback.executeWithFallback(mockFn);
      expect(result).toBe('success');
    });

    test('should handle AbortSignal in retry manager', async () => {
      const retry = new RetryManager({
        maxRetries: 10,
        initialDelay: 100,
        debug: false
      });

      const controller = new AbortController();

      // Abort after first attempt
      setTimeout(() => controller.abort(), 50);

      const mockFn = jest.fn().mockImplementation(() => {
        throw new Error('Overloaded');
      });

      await expect(
        retry.executeWithRetry(mockFn, { signal: controller.signal })
      ).rejects.toThrow('Operation aborted');

      // Should not exhaust all retries
      expect(mockFn.mock.calls.length).toBeLessThan(10);
    });
  });

  describe('Performance', () => {
    test('should complete fallback reasonably quickly with fast retries', async () => {
      const retry = new RetryManager({
        maxRetries: 2,
        initialDelay: 1,  // 1ms
        maxDelay: 10,
        debug: false
      });

      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1' },
          { provider: 'openai', apiKey: 'key2' }
        ],
        debug: false
      });

      const mockFn = jest.fn().mockImplementation((provider, model, config) => {
        if (config.provider === 'anthropic') {
          return retry.executeWithRetry(() => {
            throw new Error('Overloaded');
          }, { provider: 'anthropic' });
        }
        return retry.executeWithRetry(() => 'success', { provider: 'openai' });
      });

      const startTime = Date.now();
      await fallback.executeWithFallback(mockFn);
      const duration = Date.now() - startTime;

      // Should complete in less than 500ms (with some margin for test execution)
      // 3 retries * ~2ms delay = ~6ms for anthropic, then openai succeeds immediately
      expect(duration).toBeLessThan(500);
    });
  });
});
