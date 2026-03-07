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
      { role: 'assistant', content: 'I need to search', toolInvocations: [{ toolName: 'search', args: { query: 'functions' } }] },
      { role: 'tool', content: 'Found 10 functions', toolName: 'search' },
      { role: 'assistant', content: 'Found functions' },
      // Segment 2
      { role: 'user', content: 'Extract the first one' },
      { role: 'assistant', content: 'Let me extract', toolInvocations: [{ toolName: 'extract', args: { file: 'test.js' } }] },
      { role: 'tool', content: 'function code here', toolName: 'extract' },
      { role: 'assistant', content: 'Here is the code' },
      // Segment 3 (active - incomplete, has tool call)
      { role: 'user', content: 'Modify it' },
      { role: 'assistant', content: 'I will modify', toolInvocations: [{ toolName: 'edit', args: { file: 'test.js' } }] }
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
    const userMessages = agent.history.filter(m => m.role === 'user');
    expect(userMessages.length).toBe(3);

    // Old segment intermediate tool calls should be removed
    const hasOldToolCall = agent.history.some(
      m => m.content && m.content.includes('I need to search') && Array.isArray(m.toolInvocations)
    );
    expect(hasOldToolCall).toBe(false);

    // Active segment messages should be preserved
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
      { role: 'assistant', content: 'Thought 1', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
      { role: 'tool', content: 'Result 1', toolName: 'search' },
      { role: 'assistant', content: 'Answer 1' },
      { role: 'user', content: 'Query 2' },
      { role: 'assistant', content: 'Thought 2', toolInvocations: [{ toolName: 'search', args: { query: 'q2' } }] },
      { role: 'tool', content: 'Result 2', toolName: 'search' },
      { role: 'assistant', content: 'Answer 2' },
      { role: 'user', content: 'Query 3' },
      { role: 'assistant', content: 'Thought 3', toolInvocations: [{ toolName: 'search', args: { query: 'q3' } }] }
    ];

    // Keep last 2 segments fully
    const stats = await agent.compactHistory({
      keepLastSegment: true,
      minSegmentsToKeep: 2
    });

    expect(stats.removed).toBeGreaterThan(0);

    // Segment 1 should be compacted (intermediate tool calls removed)
    const hasThought1 = agent.history.some(
      m => m.content && m.content.includes('Thought 1') && Array.isArray(m.toolInvocations)
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
      { role: 'assistant', content: 'Searching...', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
      { role: 'tool', content: 'Result 1', toolName: 'search' },
      { role: 'assistant', content: 'Done' },
      { role: 'user', content: 'Query 2' },
      { role: 'assistant', content: 'Active thought', toolInvocations: [{ toolName: 'search', args: { query: 'q2' } }] }
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

describe('Proactive history compaction in answer()', () => {
  function createTestAgent(overrides = {}) {
    const agent = new ProbeAgent({
      sessionId: 'test-proactive',
      path: '/tmp/test',
      debug: false,
      ...overrides
    });
    // Mock the provider to avoid real API calls
    agent.provider = (model) => model;
    return agent;
  }

  it('should compact prior turn monologues before sending to model', async () => {
    const agent = createTestAgent();

    // Simulate history from a completed turn 1 (system + user + tool monologue + final answer)
    agent.history = [
      { role: 'system', content: 'You are a helpful assistant' },
      { role: 'user', content: 'What does search do?' },
      { role: 'assistant', content: [
        { type: 'text', text: 'Let me search for that.' },
        { type: 'tool-call', toolCallId: 'tc1', toolName: 'search', input: { query: 'search function' } }
      ]},
      { role: 'tool', content: [
        { type: 'tool-result', toolCallId: 'tc1', toolName: 'search', output: 'Found: search function does X, Y, Z with lots of detail...' }
      ]},
      { role: 'assistant', content: [
        { type: 'text', text: 'Let me look deeper.' },
        { type: 'tool-call', toolCallId: 'tc2', toolName: 'extract', input: { file: 'search.rs' } }
      ]},
      { role: 'tool', content: [
        { type: 'tool-result', toolCallId: 'tc2', toolName: 'extract', output: 'Full source code of search.rs...' }
      ]},
      { role: 'assistant', content: [
        { type: 'text', text: 'The search function does X, Y, Z. Here is a detailed explanation...' }
      ]}
    ];

    // Capture messages sent to streamText
    let capturedMessages = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedMessages = opts.messages;
      return {
        text: Promise.resolve('Mock response'),
        steps: Promise.resolve([{ finishReason: 'stop', toolCalls: [] }]),
        usage: Promise.resolve({ promptTokens: 100, completionTokens: 50 }),
        response: Promise.resolve({ messages: [] })
      };
    });

    try {
      await agent.answer('Now tell me about extract');
    } catch (e) {
      // May throw due to mock, that's fine
    }

    expect(capturedMessages).not.toBeNull();

    // The intermediate tool calls from turn 1 should be stripped
    // We should have: system, user1, final_answer1, user2
    const roles = capturedMessages.map(m => m.role);
    expect(roles[0]).toBe('system');

    // Count: prior turn intermediate tool messages should be removed
    const toolMessages = capturedMessages.filter(m => m.role === 'tool');
    expect(toolMessages.length).toBe(0); // All tool results from turn 1 compacted

    // Both user messages should be present
    const userMessages = capturedMessages.filter(m => m.role === 'user');
    expect(userMessages.length).toBe(2);
    expect(userMessages[0].content).toBe('What does search do?');
    expect(userMessages[1].content).toBe('Now tell me about extract');

    // Final answer from turn 1 should be preserved
    const assistantMessages = capturedMessages.filter(m => m.role === 'assistant');
    expect(assistantMessages.length).toBe(1);
    const finalText = assistantMessages[0].content.find(p => p.type === 'text')?.text || assistantMessages[0].content;
    expect(finalText).toContain('The search function does X, Y, Z');

    jest.restoreAllMocks();
    await agent.cleanup();
  });

  it('should not compact on first turn (no prior history)', async () => {
    const agent = createTestAgent({ sessionId: 'test-first-turn' });

    agent.history = []; // Empty history = first turn

    let capturedMessages = null;
    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      capturedMessages = opts.messages;
      return {
        text: Promise.resolve('Mock response'),
        steps: Promise.resolve([{ finishReason: 'stop', toolCalls: [] }]),
        usage: Promise.resolve({ promptTokens: 100, completionTokens: 50 }),
        response: Promise.resolve({ messages: [] })
      };
    });

    try {
      await agent.answer('First question');
    } catch (e) { /* mock */ }

    expect(capturedMessages).not.toBeNull();
    // Should have system + user (no compaction needed on first turn)
    expect(capturedMessages[0].role).toBe('system');
    const userMsgs = capturedMessages.filter(m => m.role === 'user');
    expect(userMsgs.length).toBe(1);
    expect(userMsgs[0].content).toBe('First question');
    // No tool/assistant messages from prior turns
    expect(capturedMessages.filter(m => m.role === 'tool').length).toBe(0);
    expect(capturedMessages.filter(m => m.role === 'assistant').length).toBe(0);

    jest.restoreAllMocks();
    await agent.cleanup();
  });

  it('should preserve full history in this.history (only compact for context)', async () => {
    const agent = createTestAgent({ sessionId: 'test-preserve' });

    // History with a completed turn
    const fullHistory = [
      { role: 'system', content: 'System prompt' },
      { role: 'user', content: 'Question 1' },
      { role: 'assistant', content: 'Thinking...', toolInvocations: [{ toolName: 'search', args: {} }] },
      { role: 'tool', content: 'Tool result', toolName: 'search' },
      { role: 'assistant', content: 'Answer 1' },
    ];
    agent.history = [...fullHistory];

    jest.spyOn(agent, 'streamTextWithRetryAndFallback').mockImplementation(async (opts) => {
      return {
        text: Promise.resolve('Answer 2'),
        steps: Promise.resolve([{ finishReason: 'stop', toolCalls: [] }]),
        usage: Promise.resolve({ promptTokens: 100, completionTokens: 50 }),
        response: Promise.resolve({ messages: [
          { role: 'assistant', content: [{ type: 'text', text: 'Answer 2' }] }
        ]})
      };
    });

    await agent.answer('Question 2');

    // this.history should contain the compacted history plus the new turn's messages
    // Prior turn intermediate monologue is stripped, but user messages and final answers preserved
    const userMessages = agent.history.filter(m => m.role === 'user');
    expect(userMessages.length).toBe(2); // Both user questions preserved
    expect(userMessages[0].content).toBe('Question 1');
    expect(userMessages[1].content).toBe('Question 2');

    jest.restoreAllMocks();
    await agent.cleanup();
  });
});
