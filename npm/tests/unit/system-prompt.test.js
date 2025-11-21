import { describe, test, expect } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('ProbeAgent systemPrompt alias', () => {
  test('uses systemPrompt when provided', () => {
    const agent = new ProbeAgent({
      path: process.cwd(),
      systemPrompt: 'system-level prompt'
    });

    expect(agent.customPrompt).toBe('system-level prompt');
  });

  test('systemPrompt takes precedence over customPrompt', () => {
    const agent = new ProbeAgent({
      path: process.cwd(),
      systemPrompt: 'primary system prompt',
      customPrompt: 'secondary custom prompt'
    });

    expect(agent.customPrompt).toBe('primary system prompt');
  });

  test('falls back to customPrompt when systemPrompt is absent', () => {
    const agent = new ProbeAgent({
      path: process.cwd(),
      customPrompt: 'custom prompt only'
    });

    expect(agent.customPrompt).toBe('custom prompt only');
  });
});
