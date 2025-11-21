/**
 * Tests for FallbackManager
 */

import { describe, test, expect, jest, beforeEach, afterEach } from '@jest/globals';
import { FallbackManager, FALLBACK_STRATEGIES, buildFallbackProvidersFromEnv } from '../../src/agent/FallbackManager.js';

describe('FallbackManager', () => {
  let originalEnv;

  beforeEach(() => {
    jest.clearAllMocks();
    originalEnv = { ...process.env };
  });

  afterEach(() => {
    process.env = originalEnv;
  });

  describe('constructor', () => {
    test('should initialize with default values', () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'test-key-1' },
          { provider: 'openai', apiKey: 'test-key-2' }
        ]
      });

      expect(fallback.strategy).toBe('custom');
      expect(fallback.stopOnSuccess).toBe(true);
      expect(fallback.maxTotalAttempts).toBe(10);
      expect(fallback.providers.length).toBe(2);
    });

    test('should accept custom configuration', () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [{ provider: 'anthropic', apiKey: 'test' }],
        stopOnSuccess: false,
        maxTotalAttempts: 5,
        debug: true
      });

      expect(fallback.stopOnSuccess).toBe(false);
      expect(fallback.maxTotalAttempts).toBe(5);
      expect(fallback.debug).toBe(true);
    });
  });

  describe('validation', () => {
    test('should throw error for same-provider strategy without models', () => {
      expect(() => {
        new FallbackManager({
          strategy: FALLBACK_STRATEGIES.SAME_PROVIDER
        });
      }).toThrow('strategy "same-provider" requires models list');
    });

    test('should throw error for custom strategy without providers', () => {
      expect(() => {
        new FallbackManager({
          strategy: FALLBACK_STRATEGIES.CUSTOM
        });
      }).toThrow('strategy "custom" requires providers list');
    });

    test('should throw error for provider without provider field', () => {
      expect(() => {
        new FallbackManager({
          strategy: 'custom',
          providers: [{ apiKey: 'test' }]
        });
      }).toThrow('must have a "provider" field');
    });

    test('should throw error for invalid provider name', () => {
      expect(() => {
        new FallbackManager({
          strategy: 'custom',
          providers: [{ provider: 'invalid', apiKey: 'test' }]
        });
      }).toThrow('Invalid provider "invalid"');
    });

    test('should throw error for anthropic without apiKey', () => {
      expect(() => {
        new FallbackManager({
          strategy: 'custom',
          providers: [{ provider: 'anthropic' }]
        });
      }).toThrow('Provider "anthropic" requires apiKey');
    });

    test('should throw error for bedrock without credentials', () => {
      expect(() => {
        new FallbackManager({
          strategy: 'custom',
          providers: [{ provider: 'bedrock' }]
        });
      }).toThrow('Bedrock provider requires either');
    });

    test('should accept bedrock with AWS credentials', () => {
      expect(() => {
        new FallbackManager({
          strategy: 'custom',
          providers: [{
            provider: 'bedrock',
            accessKeyId: 'test',
            secretAccessKey: 'test',
            region: 'us-east-1'
          }]
        });
      }).not.toThrow();
    });

    test('should accept bedrock with apiKey', () => {
      expect(() => {
        new FallbackManager({
          strategy: 'custom',
          providers: [{
            provider: 'bedrock',
            apiKey: 'test'
          }]
        });
      }).not.toThrow();
    });
  });

  describe('executeWithFallback', () => {
    test('should succeed on first provider', async () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1' },
          { provider: 'openai', apiKey: 'key2' }
        ],
        debug: false
      });

      const mockFn = jest.fn().mockResolvedValue('success');

      const result = await fallback.executeWithFallback(mockFn);

      expect(result).toBe('success');
      expect(mockFn).toHaveBeenCalledTimes(1);
      expect(fallback.stats.totalAttempts).toBe(1);
      expect(fallback.stats.successfulProvider).toContain('anthropic');
    });

    test('should fallback to second provider on failure', async () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1', model: 'claude-3' },
          { provider: 'openai', apiKey: 'key2', model: 'gpt-4' }
        ],
        debug: false
      });

      let callCount = 0;
      const mockFn = jest.fn().mockImplementation(() => {
        callCount++;
        if (callCount === 1) {
          throw new Error('Provider 1 failed');
        }
        return Promise.resolve('success from provider 2');
      });

      const result = await fallback.executeWithFallback(mockFn);

      expect(result).toBe('success from provider 2');
      expect(mockFn).toHaveBeenCalledTimes(2);
      expect(fallback.stats.totalAttempts).toBe(2);
      expect(fallback.stats.successfulProvider).toContain('openai/gpt-4');
      expect(fallback.stats.failedProviders.length).toBe(1);
    });

    test('should exhaust all providers and fail', async () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1' },
          { provider: 'openai', apiKey: 'key2' },
          { provider: 'google', apiKey: 'key3' }
        ],
        debug: false
      });

      const mockFn = jest.fn().mockRejectedValue(new Error('All providers failed'));

      await expect(fallback.executeWithFallback(mockFn)).rejects.toThrow('All provider fallbacks exhausted');

      expect(mockFn).toHaveBeenCalledTimes(3);
      expect(fallback.stats.totalAttempts).toBe(3);
      expect(fallback.stats.successfulProvider).toBeNull();
      expect(fallback.stats.failedProviders.length).toBe(3);
    });

    test('should respect maxTotalAttempts', async () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1' },
          { provider: 'openai', apiKey: 'key2' },
          { provider: 'google', apiKey: 'key3' },
          { provider: 'bedrock', apiKey: 'key4' }
        ],
        maxTotalAttempts: 2,
        debug: false
      });

      const mockFn = jest.fn().mockRejectedValue(new Error('Failed'));

      await expect(fallback.executeWithFallback(mockFn)).rejects.toThrow();

      // Should only try 2 providers due to maxTotalAttempts
      expect(mockFn).toHaveBeenCalledTimes(2);
      expect(fallback.stats.totalAttempts).toBe(2);
    });

    test('should pass provider, model, and config to function', async () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          {
            provider: 'anthropic',
            apiKey: 'test-key',
            model: 'claude-3',
            baseURL: 'https://custom.url'
          }
        ],
        debug: false
      });

      let receivedProvider, receivedModel, receivedConfig;
      const mockFn = jest.fn().mockImplementation((provider, model, config) => {
        receivedProvider = provider;
        receivedModel = model;
        receivedConfig = config;
        return Promise.resolve('success');
      });

      await fallback.executeWithFallback(mockFn);

      expect(receivedProvider).toBeDefined();
      expect(receivedModel).toBe('claude-3');
      expect(receivedConfig.provider).toBe('anthropic');
      expect(receivedConfig.apiKey).toBe('test-key');
      expect(receivedConfig.baseURL).toBe('https://custom.url');
    });

    test('should use default model if not specified', async () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'test' }
        ],
        debug: false
      });

      let receivedModel;
      const mockFn = jest.fn().mockImplementation((provider, model) => {
        receivedModel = model;
        return Promise.resolve('success');
      });

      await fallback.executeWithFallback(mockFn);

      expect(receivedModel).toBe('claude-sonnet-4-5-20250929');
    });

    test('should throw error when no providers configured', async () => {
      const fallback = new FallbackManager({
        strategy: 'any',
        providers: []
      });

      const mockFn = jest.fn();

      await expect(fallback.executeWithFallback(mockFn)).rejects.toThrow('No providers configured');
    });

    test('should handle errors with full context', async () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1' }
        ],
        debug: false
      });

      const mockFn = jest.fn().mockRejectedValue(new Error('Test error'));

      try {
        await fallback.executeWithFallback(mockFn);
        fail('Should have thrown error');
      } catch (error) {
        expect(error.message).toContain('All provider fallbacks exhausted');
        expect(error.message).toContain('Test error');
        expect(error.stats).toBeDefined();
        expect(error.stats.totalAttempts).toBe(1);
        expect(error.allProvidersFailed).toBe(true);
      }
    });
  });

  describe('statistics', () => {
    test('should track provider attempts', async () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [
          { provider: 'anthropic', apiKey: 'key1', model: 'claude-3' },
          { provider: 'openai', apiKey: 'key2', model: 'gpt-4' }
        ],
        debug: false
      });

      let callCount = 0;
      await fallback.executeWithFallback(jest.fn().mockImplementation(() => {
        callCount++;
        if (callCount === 1) throw new Error('Fail');
        return 'success';
      }));

      const stats = fallback.getStats();
      expect(stats.providerAttempts['anthropic/claude-3']).toBe(1);
      expect(stats.providerAttempts['openai/gpt-4']).toBe(1);
    });

    test('should reset statistics', () => {
      const fallback = new FallbackManager({
        strategy: 'custom',
        providers: [{ provider: 'anthropic', apiKey: 'test' }]
      });

      fallback.stats.totalAttempts = 5;
      fallback.stats.successfulProvider = 'test';

      fallback.resetStats();

      expect(fallback.stats.totalAttempts).toBe(0);
      expect(fallback.stats.successfulProvider).toBeNull();
      expect(fallback.stats.failedProviders.length).toBe(0);
    });
  });

  describe('buildFallbackProvidersFromEnv', () => {
    const envKeys = [
      'ANTHROPIC_API_KEY',
      'ANTHROPIC_API_URL',
      'OPENAI_API_KEY',
      'OPENAI_API_URL',
      'GOOGLE_API_KEY',
      'AWS_ACCESS_KEY_ID',
      'AWS_SECRET_ACCESS_KEY',
      'AWS_REGION',
      'AWS_BEDROCK_API_KEY'
    ];

    beforeEach(() => {
      // Ensure tests in this block start with a predictable environment
      for (const key of envKeys) {
        delete process.env[key];
      }
    });

    test('should build providers from environment variables', () => {
      process.env.ANTHROPIC_API_KEY = 'ant-key';
      process.env.OPENAI_API_KEY = 'openai-key';
      process.env.GOOGLE_API_KEY = 'google-key';

      const providers = buildFallbackProvidersFromEnv({
        primaryProvider: 'anthropic',
        primaryModel: 'claude-3'
      });

      expect(providers.length).toBe(3);
      expect(providers[0].provider).toBe('anthropic');
      expect(providers[0].model).toBe('claude-3');
      expect(providers[1].provider).toBe('openai');
      expect(providers[2].provider).toBe('google');
    });

    test('should include custom URLs', () => {
      process.env.ANTHROPIC_API_KEY = 'test';
      process.env.ANTHROPIC_API_URL = 'https://custom.anthropic.com';

      const providers = buildFallbackProvidersFromEnv({
        primaryProvider: 'anthropic'
      });

      expect(providers[0].baseURL).toBe('https://custom.anthropic.com');
    });

    test('should handle AWS Bedrock credentials', () => {
      process.env.AWS_ACCESS_KEY_ID = 'aws-id';
      process.env.AWS_SECRET_ACCESS_KEY = 'aws-secret';
      process.env.AWS_REGION = 'us-east-1';

      const providers = buildFallbackProvidersFromEnv({
        primaryProvider: 'bedrock'
      });

      expect(providers.length).toBe(1);
      expect(providers[0].provider).toBe('bedrock');
      expect(providers[0].accessKeyId).toBe('aws-id');
      expect(providers[0].secretAccessKey).toBe('aws-secret');
      expect(providers[0].region).toBe('us-east-1');
    });

    test('should handle AWS Bedrock with API key', () => {
      process.env.AWS_BEDROCK_API_KEY = 'bedrock-key';

      const providers = buildFallbackProvidersFromEnv({
        primaryProvider: 'bedrock'
      });

      expect(providers.length).toBe(1);
      expect(providers[0].provider).toBe('bedrock');
      expect(providers[0].apiKey).toBe('bedrock-key');
    });

    test('should prioritize primary provider', () => {
      process.env.ANTHROPIC_API_KEY = 'ant-key';
      process.env.OPENAI_API_KEY = 'openai-key';

      const providers = buildFallbackProvidersFromEnv({
        primaryProvider: 'openai'
      });

      expect(providers[0].provider).toBe('openai');
      expect(providers[1].provider).toBe('anthropic');
    });

    test('should return empty array with no API keys', () => {
      const providers = buildFallbackProvidersFromEnv({});
      expect(providers.length).toBe(0);
    });
  });
});
