/**
 * Tests for ProbeAgent.compactHistory() method
 */

import { jest } from '@jest/globals';
import { ProbeAgent } from '../src/agent/ProbeAgent.js';

describe('ProbeAgent.compactHistory()', () => {
  let agent;

  beforeEach(() => {
    // Create agent with mock provider to avoid actual API calls
    agent = new ProbeAgent({
      sessionId: 'test-session',
      path: '/tmp/test',
      provider: 'mock',
      debug: false
    });
  });

  afterEach(async () => {
    if (agent) {
      await agent.cleanup();
    }
  });

  it('should compact history and return statistics', async () => {
    // Populate history with multiple segments (native tool calling format)
    agent.history = [
      { role: 'system', content: 'You are a helpful assistant' },
      // Segment 1
      { role: 'user', content: 'Search for functions' },
      { role: 'assistant', content: 'I need to search' },
      { role: 'assistant', content: 'Searching...' },
      { role: 'tool', content: 'Found 10 functions', toolName: 'search' },
      { role: 'assistant', content: '', toolInvocations: [{ toolName: 'attempt_completion', args: { result: 'Found functions' } }] },
      // Segment 2
      { role: 'user', content: 'Extract the first one' },
      { role: 'assistant', content: 'Let me extract' },
      { role: 'assistant', content: 'Extracting...' },
      { role: 'tool', content: 'function code here', toolName: 'extract' },
      { role: 'assistant', content: '', toolInvocations: [{ toolName: 'attempt_completion', args: { result: 'Here is the code' } }] },
      // Segment 3 (active)
      { role: 'user', content: 'Modify it' },
      { role: 'assistant', content: 'I will modify' }
    ];

    const stats = await agent.compactHistory();

    // Check stats
    expect(stats.originalCount).toBe(13);
    expect(stats.compactedCount).toBeLessThan(13);
    expect(stats.removed).toBeGreaterThan(0);
    expect(stats.reductionPercent).toBeGreaterThan(0);
    expect(stats.tokensSaved).toBeGreaterThan(0);

    // Check history was actually compacted
    expect(agent.history.length).toBeLessThan(13);

    // System message should be preserved
    expect(agent.history[0].role).toBe('system');

    // All user messages should be preserved
    const userMessages = agent.history.filter(m => m.role === 'user');
    expect(userMessages.length).toBe(3);

    // Old segment thinking should be removed
    const hasOldThinking = agent.history.some(
      m => m.content && m.content.includes('I need to search')
    );
    expect(hasOldThinking).toBe(false);

    // Active segment thinking should be preserved
    const hasActiveThinking = agent.history.some(
      m => m.content && m.content.includes('I will modify')
    );
    expect(hasActiveThinking).toBe(true);
  });

  it('should handle empty history gracefully', async () => {
    agent.history = [];

    const stats = await agent.compactHistory();

    expect(stats.originalCount).toBe(0);
    expect(stats.compactedCount).toBe(0);
    expect(stats.removed).toBe(0);
    expect(stats.reductionPercent).toBe(0);
    expect(stats.tokensSaved).toBe(0);
  });

  it('should respect custom options', async () => {
    agent.history = [
      { role: 'user', content: 'Query 1' },
      { role: 'assistant', content: 'Thought 1' },
      { role: 'tool', content: 'Result 1', toolName: 'search' },
      { role: 'assistant', content: '', toolInvocations: [{ toolName: 'attempt_completion', args: { result: 'Answer 1' } }] },
      { role: 'user', content: 'Query 2' },
      { role: 'assistant', content: 'Thought 2' },
      { role: 'tool', content: 'Result 2', toolName: 'search' },
      { role: 'assistant', content: '', toolInvocations: [{ toolName: 'attempt_completion', args: { result: 'Answer 2' } }] },
      { role: 'user', content: 'Query 3' },
      { role: 'assistant', content: 'Thought 3' }
    ];

    // Keep last 2 segments fully
    const stats = await agent.compactHistory({
      keepLastSegment: true,
      minSegmentsToKeep: 2
    });

    expect(stats.removed).toBeGreaterThan(0);

    // Segment 1 should be compacted
    const hasThought1 = agent.history.some(
      m => m.content && m.content.includes('Thought 1')
    );
    expect(hasThought1).toBe(false);

    // Segments 2 and 3 should be preserved
    const hasThought2 = agent.history.some(
      m => m.content && m.content.includes('Thought 2')
    );
    const hasThought3 = agent.history.some(
      m => m.content && m.content.includes('Thought 3')
    );
    expect(hasThought2).toBe(true);
    expect(hasThought3).toBe(true);
  });

  it('should handle history with only system message', async () => {
    agent.history = [
      { role: 'system', content: 'You are a helpful assistant' }
    ];

    const stats = await agent.compactHistory();

    expect(stats.removed).toBe(0);
    expect(agent.history.length).toBe(1);
    expect(agent.history[0].role).toBe('system');
  });

  it('should handle history with incomplete segments', async () => {
    agent.history = [
      { role: 'user', content: 'Query' },
      { role: 'assistant', content: 'Processing...' }
    ];

    const stats = await agent.compactHistory();

    // Should not crash, incomplete segment should be preserved
    expect(agent.history.length).toBeGreaterThan(0);
    expect(stats).toBeDefined();
  });

  it('should preserve message order', async () => {
    agent.history = [
      { role: 'system', content: 'System' },
      { role: 'user', content: 'Query 1' },
      { role: 'assistant', content: 'Searching...' },
      { role: 'tool', content: 'Result 1', toolName: 'search' },
      { role: 'assistant', content: '', toolInvocations: [{ toolName: 'attempt_completion', args: { result: 'Done' } }] },
      { role: 'user', content: 'Query 2' },
      { role: 'assistant', content: 'Active thought' }
    ];

    await agent.compactHistory();

    // First message should still be system
    expect(agent.history[0].role).toBe('system');

    // User messages should maintain their relative order
    const userMessages = agent.history.filter(m => m.role === 'user');
    expect(userMessages[0].content).toBe('Query 1');
    expect(userMessages[1].content).toBe('Query 2');
  });
});
