/**
 * Tests for timeout validation and configuration in ProbeAgent
 */

import { describe, test, expect, beforeEach, afterEach, jest } from '@jest/globals';

describe('Timeout Validation', () => {
  let originalEnv;

  beforeEach(() => {
    // Save original environment
    originalEnv = { ...process.env };
  });

  afterEach(() => {
    // Restore original environment
    process.env = originalEnv;
  });

  describe('REQUEST_TIMEOUT environment variable', () => {
    test('should use default (120000ms) when env var not set', async () => {
      delete process.env.REQUEST_TIMEOUT;

      // Import fresh to get new env reading
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.requestTimeout).toBe(120000);
    });

    test('should use valid env var value', async () => {
      process.env.REQUEST_TIMEOUT = '60000';

      // Need to clear module cache to re-read env
      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.requestTimeout).toBe(60000);
    });

    test('should fallback to default for NaN value', async () => {
      process.env.REQUEST_TIMEOUT = 'invalid';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.requestTimeout).toBe(120000);
    });

    test('should fallback to default for value below minimum (1000ms)', async () => {
      process.env.REQUEST_TIMEOUT = '500';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.requestTimeout).toBe(120000);
    });

    test('should fallback to default for value above maximum (3600000ms)', async () => {
      process.env.REQUEST_TIMEOUT = '4000000';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.requestTimeout).toBe(120000);
    });

    test('should fallback to default for negative value', async () => {
      process.env.REQUEST_TIMEOUT = '-1000';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.requestTimeout).toBe(120000);
    });

    test('should accept options.requestTimeout over env var', async () => {
      process.env.REQUEST_TIMEOUT = '60000';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic',
        requestTimeout: 90000
      });

      expect(agent.requestTimeout).toBe(90000);
    });
  });

  describe('MAX_OPERATION_TIMEOUT environment variable', () => {
    test('should use default (300000ms) when env var not set', async () => {
      delete process.env.MAX_OPERATION_TIMEOUT;

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.maxOperationTimeout).toBe(300000);
    });

    test('should use valid env var value', async () => {
      process.env.MAX_OPERATION_TIMEOUT = '600000';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.maxOperationTimeout).toBe(600000);
    });

    test('should fallback to default for NaN value', async () => {
      process.env.MAX_OPERATION_TIMEOUT = 'not-a-number';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.maxOperationTimeout).toBe(300000);
    });

    test('should fallback to default for value below minimum (1000ms)', async () => {
      process.env.MAX_OPERATION_TIMEOUT = '100';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.maxOperationTimeout).toBe(300000);
    });

    test('should fallback to default for value above maximum (7200000ms)', async () => {
      process.env.MAX_OPERATION_TIMEOUT = '8000000';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.maxOperationTimeout).toBe(300000);
    });

    test('should accept options.maxOperationTimeout over env var', async () => {
      process.env.MAX_OPERATION_TIMEOUT = '600000';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic',
        maxOperationTimeout: 400000
      });

      expect(agent.maxOperationTimeout).toBe(400000);
    });
  });

  describe('ENGINE_ACTIVITY_TIMEOUT validation', () => {
    // Note: ENGINE_ACTIVITY_TIMEOUT is validated inline in streamTextWithRetryAndFallback
    // These tests verify the validation logic pattern

    test('should validate activity timeout range (5000-600000ms)', () => {
      // Test the validation logic directly
      const validateActivityTimeout = (envValue) => {
        const parsed = parseInt(envValue, 10);
        return isNaN(parsed) || parsed < 5000 || parsed > 600000 ? 180000 : parsed;
      };

      // Valid values
      expect(validateActivityTimeout('60000')).toBe(60000);
      expect(validateActivityTimeout('180000')).toBe(180000);
      expect(validateActivityTimeout('5000')).toBe(5000);
      expect(validateActivityTimeout('600000')).toBe(600000);

      // Invalid values - should return default (180000)
      expect(validateActivityTimeout('invalid')).toBe(180000);
      expect(validateActivityTimeout('4999')).toBe(180000);
      expect(validateActivityTimeout('600001')).toBe(180000);
      expect(validateActivityTimeout('-1000')).toBe(180000);
      expect(validateActivityTimeout('')).toBe(180000);
    });
  });

  describe('Timeout boundary values', () => {
    test('should accept minimum valid REQUEST_TIMEOUT (1000ms)', async () => {
      process.env.REQUEST_TIMEOUT = '1000';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.requestTimeout).toBe(1000);
    });

    test('should accept maximum valid REQUEST_TIMEOUT (3600000ms)', async () => {
      process.env.REQUEST_TIMEOUT = '3600000';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.requestTimeout).toBe(3600000);
    });

    test('should accept minimum valid MAX_OPERATION_TIMEOUT (1000ms)', async () => {
      process.env.MAX_OPERATION_TIMEOUT = '1000';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.maxOperationTimeout).toBe(1000);
    });

    test('should accept maximum valid MAX_OPERATION_TIMEOUT (7200000ms)', async () => {
      process.env.MAX_OPERATION_TIMEOUT = '7200000';

      jest.resetModules();
      const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');
      const agent = new ProbeAgent({
        apiKey: 'test-key',
        apiType: 'anthropic'
      });

      expect(agent.maxOperationTimeout).toBe(7200000);
    });
  });
});
