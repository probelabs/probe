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
    // Populate history with multiple segments
    agent.history = [
      { role: 'system', content: 'You are a helpful assistant' },
      // Segment 1
      { role: 'user', content: 'Search for functions' },
      { role: 'assistant', content: '<thinking>I need to search</thinking>' },
      { role: 'assistant', content: '<search>function</search>' },
      { role: 'user', content: '<tool_result>Found 10 functions</tool_result>' },
      // Segment 2
      { role: 'user', content: 'Extract the first one' },
      { role: 'assistant', content: '<thinking>Let me extract</thinking>' },
      { role: 'assistant', content: '<extract>file.js:10</extract>' },
      { role: 'user', content: '<tool_result>function code here</tool_result>' },
      // Segment 3 (active)
      { role: 'user', content: 'Modify it' },
      { role: 'assistant', content: '<thinking>I will modify</thinking>' }
    ];

    const stats = await agent.compactHistory();

    // Check stats
    expect(stats.originalCount).toBe(11);
    expect(stats.compactedCount).toBeLessThan(11);
    expect(stats.removed).toBeGreaterThan(0);
    expect(stats.reductionPercent).toBeGreaterThan(0);
    expect(stats.tokensSaved).toBeGreaterThan(0);

    // Check history was actually compacted
    expect(agent.history.length).toBeLessThan(11);

    // System message should be preserved
    expect(agent.history[0].role).toBe('system');

    // All user messages should be preserved
    const userMessages = agent.history.filter(
      m => m.role === 'user' && !m.content.includes('tool_result')
    );
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
      { role: 'assistant', content: '<thinking>Thought 1</thinking>' },
      { role: 'user', content: '<tool_result>Result 1</tool_result>' },
      { role: 'user', content: 'Query 2' },
      { role: 'assistant', content: '<thinking>Thought 2</thinking>' },
      { role: 'user', content: '<tool_result>Result 2</tool_result>' },
      { role: 'user', content: 'Query 3' },
      { role: 'assistant', content: '<thinking>Thought 3</thinking>' }
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
      { role: 'assistant', content: '<thinking>Processing...</thinking>' }
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
      { role: 'assistant', content: '<search>test</search>' },
      { role: 'user', content: '<tool_result>Result 1</tool_result>' },
      { role: 'user', content: 'Query 2' },
      { role: 'assistant', content: '<thinking>Active</thinking>' }
    ];

    await agent.compactHistory();

    // First message should still be system
    expect(agent.history[0].role).toBe('system');

    // User messages should maintain their relative order
    const userMessages = agent.history.filter(m => m.role === 'user');
    expect(userMessages[0].content).toBe('Query 1');
    expect(userMessages[2].content).toBe('Query 2');
  });
});
