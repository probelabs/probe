import { describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('ProbeAgent model option', () => {
  let originalModelName;

  beforeEach(() => {
    // Save original MODEL_NAME environment variable
    originalModelName = process.env.MODEL_NAME;
  });

  afterEach(() => {
    // Restore original MODEL_NAME
    if (originalModelName === undefined) {
      delete process.env.MODEL_NAME;
    } else {
      process.env.MODEL_NAME = originalModelName;
    }
  });

  test('should store model option in constructor', () => {
    const agent = new ProbeAgent({
      model: 'custom-model-123',
      path: process.cwd()
    });

    expect(agent.clientApiModel).toBe('custom-model-123');
  });

  test('should default to null when no model option provided', () => {
    const agent = new ProbeAgent({
      path: process.cwd()
    });

    expect(agent.clientApiModel).toBeNull();
  });

  test('should use model option over MODEL_NAME environment variable', () => {
    process.env.MODEL_NAME = 'env-model';

    const agent = new ProbeAgent({
      model: 'option-model',
      path: process.cwd()
    });

    expect(agent.clientApiModel).toBe('option-model');
    // The actual model name would be set during initializeModel()
    // which uses clientApiModel || process.env.MODEL_NAME
  });

  test('should fall back to MODEL_NAME when model option not provided', () => {
    process.env.MODEL_NAME = 'env-model';

    const agent = new ProbeAgent({
      path: process.cwd()
    });

    expect(agent.clientApiModel).toBeNull();
    // initializeModel() would use process.env.MODEL_NAME as fallback
  });

  test('should preserve model option in clone', () => {
    const baseAgent = new ProbeAgent({
      model: 'original-model',
      path: process.cwd()
    });

    const cloned = baseAgent.clone();

    // The clone should preserve the user's model preference (clientApiModel)
    expect(cloned.clientApiModel).toBe(baseAgent.clientApiModel);
    expect(cloned.clientApiModel).toBe('original-model');
    // Both should resolve to the same actual model
    expect(cloned.model).toBe('original-model');
  });

  test('should preserve absence of model option in clone', () => {
    const baseAgent = new ProbeAgent({
      path: process.cwd()
      // No model specified
    });

    const cloned = baseAgent.clone();

    // The clone should not have a model preference either
    expect(cloned.clientApiModel).toBeNull();
    expect(baseAgent.clientApiModel).toBeNull();
    // Both should use the default mock model
    expect(cloned.model).toBe('mock-model');
    expect(baseAgent.model).toBe('mock-model');
  });

  test('should allow override of model option in clone', () => {
    const baseAgent = new ProbeAgent({
      model: 'original-model',
      path: process.cwd()
    });

    const cloned = baseAgent.clone({
      overrides: {
        model: 'cloned-model'
      }
    });

    // The clone should have the overridden model stored in clientApiModel
    expect(cloned.clientApiModel).toBe('cloned-model');
  });

  test('should handle empty string model option', () => {
    const agent = new ProbeAgent({
      model: '',
      path: process.cwd()
    });

    // Empty string is falsy, so it becomes null due to || operator
    expect(agent.clientApiModel).toBeNull();
  });
});
