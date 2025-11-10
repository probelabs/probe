/**
 * Tests for Context Compactor
 */

import { jest } from '@jest/globals';
import {
  isContextLimitError,
  identifyMessageSegments,
  compactMessages,
  calculateCompactionStats,
  handleContextLimitError
} from '../src/agent/contextCompactor.js';

describe('Context Compactor', () => {
  describe('isContextLimitError', () => {
    it('should detect Anthropic context length error', () => {
      const error = new Error('prompt is too long: context_length_exceeded');
      expect(isContextLimitError(error)).toBe(true);
    });

    it('should detect OpenAI context window error', () => {
      const error = new Error('This model\'s maximum context length is 8192 tokens. However, your messages resulted in 10000 tokens.');
      expect(isContextLimitError(error)).toBe(true);
    });

    it('should detect Gemini token limit error', () => {
      const error = new Error('The input token count exceeds the maximum limit of 128000 tokens');
      expect(isContextLimitError(error)).toBe(true);
    });

    it('should detect generic "too long" error', () => {
      const error = new Error('The prompt is too long and exceeds the context window');
      expect(isContextLimitError(error)).toBe(true);
    });

    it('should detect "over limit" variations', () => {
      const error = new Error('Total tokens over the limit');
      expect(isContextLimitError(error)).toBe(true);
    });

    it('should detect "maximum tokens" error', () => {
      const error = new Error('Request exceeds maximum tokens allowed');
      expect(isContextLimitError(error)).toBe(true);
    });

    it('should work with string error messages', () => {
      const errorString = 'context window exceeded';
      expect(isContextLimitError(errorString)).toBe(true);
    });

    it('should not match unrelated errors', () => {
      const error = new Error('Network connection failed');
      expect(isContextLimitError(error)).toBe(false);
    });

    it('should not match partial matches without overflow cues', () => {
      const error = new Error('Using context from previous message');
      expect(isContextLimitError(error)).toBe(false);
    });
  });

  describe('identifyMessageSegments', () => {
    it('should identify simple user-assistant-result segment', () => {
      const messages = [
        { role: 'system', content: 'You are an AI assistant' },
        { role: 'user', content: 'Search for function definitions' },
        { role: 'assistant', content: '<thinking>Let me search</thinking>\n<search>function</search>' },
        { role: 'user', content: '<tool_result>Found 10 results</tool_result>' }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(1);
      expect(segments[0]).toEqual({
        userIndex: 1,
        monologueIndices: [2],
        finalIndex: 3
      });
    });

    it('should identify multiple segments', () => {
      const messages = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'First query' },
        { role: 'assistant', content: '<search>test</search>' },
        { role: 'user', content: '<tool_result>Result 1</tool_result>' },
        { role: 'user', content: 'Second query' },
        { role: 'assistant', content: '<extract>file.js</extract>' },
        { role: 'user', content: '<tool_result>Result 2</tool_result>' }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(2);

      expect(segments[0].userIndex).toBe(1);
      expect(segments[0].finalIndex).toBe(3);

      expect(segments[1].userIndex).toBe(4);
      expect(segments[1].finalIndex).toBe(6);
    });

    it('should handle segment with multiple monologue messages', () => {
      const messages = [
        { role: 'user', content: 'Query' },
        { role: 'assistant', content: '<thinking>First thought</thinking>' },
        { role: 'assistant', content: '<thinking>Second thought</thinking>' },
        { role: 'assistant', content: '<search>test</search>' },
        { role: 'user', content: '<tool_result>Result</tool_result>' }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(1);
      expect(segments[0].monologueIndices).toEqual([1, 2, 3]);
    });

    it('should handle attempt_completion as segment end', () => {
      const messages = [
        { role: 'user', content: 'Query' },
        { role: 'assistant', content: '<search>test</search>' },
        { role: 'user', content: '<tool_result>Found results</tool_result>' },
        { role: 'assistant', content: '<attempt_completion>Here is the answer</attempt_completion>' }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(1);

      // Segment includes all messages with final being attempt_completion
      expect(segments[0].userIndex).toBe(0);
      expect(segments[0].finalIndex).toBe(2); // tool_result is the final
    });

    it('should handle incomplete segment (no final answer)', () => {
      const messages = [
        { role: 'user', content: 'Query' },
        { role: 'assistant', content: '<thinking>In progress...</thinking>' }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(1);
      expect(segments[0].finalIndex).toBe(null);
    });

    it('should skip system messages', () => {
      const messages = [
        { role: 'system', content: 'System message 1' },
        { role: 'user', content: 'Query' },
        { role: 'system', content: 'System message 2' },
        { role: 'assistant', content: '<search>test</search>' }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(1);
      expect(segments[0].userIndex).toBe(1); // First user message
    });
  });

  describe('compactMessages', () => {
    it('should remove intermediate monologues from old segments', () => {
      const messages = [
        { role: 'system', content: 'System' },
        // Segment 1 (old - should be compacted)
        { role: 'user', content: 'First query' },
        { role: 'assistant', content: '<thinking>Thought 1</thinking>' },
        { role: 'assistant', content: '<search>test</search>' },
        { role: 'user', content: '<tool_result>Result 1</tool_result>' },
        // Segment 2 (old - should be compacted)
        { role: 'user', content: 'Second query' },
        { role: 'assistant', content: '<thinking>Thought 2</thinking>' },
        { role: 'assistant', content: '<extract>file.js</extract>' },
        { role: 'user', content: '<tool_result>Result 2</tool_result>' },
        // Segment 3 (active - should be preserved)
        { role: 'user', content: 'Third query' },
        { role: 'assistant', content: '<thinking>Active thought</thinking>' }
      ];

      const compacted = compactMessages(messages, {
        keepLastSegment: true,
        minSegmentsToKeep: 1
      });

      // Should be fewer messages than original
      expect(compacted.length).toBeLessThan(messages.length);

      // Check system message preserved
      expect(compacted[0].role).toBe('system');

      // Check segment 1 and 2 compacted (only user and final)
      const hasFirstQuery = compacted.some(m => m.content === 'First query');
      const hasResult1 = compacted.some(m => m.content === '<tool_result>Result 1</tool_result>');
      const hasThought1 = compacted.some(m => m.content && m.content.includes('Thought 1'));

      const hasSecondQuery = compacted.some(m => m.content === 'Second query');
      const hasResult2 = compacted.some(m => m.content === '<tool_result>Result 2</tool_result>');
      const hasThought2 = compacted.some(m => m.content && m.content.includes('Thought 2'));

      expect(hasFirstQuery).toBe(true);
      expect(hasResult1).toBe(true);
      expect(hasThought1).toBe(false); // Thought 1 should be removed

      expect(hasSecondQuery).toBe(true);
      expect(hasResult2).toBe(true);
      expect(hasThought2).toBe(false); // Thought 2 should be removed

      // Check segment 3 is fully preserved (active)
      const hasActiveThought = compacted.some(m => m.content && m.content.includes('Active thought'));
      expect(hasActiveThought).toBe(true);
    });

    it('should keep all system messages', () => {
      const messages = [
        { role: 'system', content: 'System 1' },
        { role: 'user', content: 'Query' },
        { role: 'system', content: 'System 2' },
        { role: 'assistant', content: '<search>test</search>' },
        { role: 'user', content: '<tool_result>Result</tool_result>' }
      ];

      const compacted = compactMessages(messages);

      const systemMessages = compacted.filter(m => m.role === 'system');
      expect(systemMessages).toHaveLength(2);
    });

    it('should handle empty message array', () => {
      const compacted = compactMessages([]);
      expect(compacted).toEqual([]);
    });

    it('should preserve segments when minSegmentsToKeep is high', () => {
      const messages = [
        { role: 'user', content: 'Query 1' },
        { role: 'assistant', content: '<search>test</search>' },
        { role: 'user', content: '<tool_result>Result 1</tool_result>' },
        { role: 'user', content: 'Query 2' },
        { role: 'assistant', content: '<extract>file</extract>' }
      ];

      const compacted = compactMessages(messages, {
        minSegmentsToKeep: 5
      });

      // All segments should be preserved
      expect(compacted).toEqual(messages);
    });

    it('should respect keepLastSegment option', () => {
      const messages = [
        { role: 'user', content: 'Query 1' },
        { role: 'assistant', content: '<thinking>Old thought</thinking>' },
        { role: 'assistant', content: '<search>test</search>' },
        { role: 'user', content: '<tool_result>Result</tool_result>' },
        { role: 'user', content: 'Query 2' },
        { role: 'assistant', content: '<thinking>New thought</thinking>' }
      ];

      // With keepLastSegment=false and minSegmentsToKeep=1
      const compacted = compactMessages(messages, {
        keepLastSegment: false,
        minSegmentsToKeep: 1
      });

      // Should only preserve the second-to-last segment fully
      // Segment 1 should be compacted
      const hasOldThought = compacted.some(m => m.content.includes('Old thought'));
      const hasNewThought = compacted.some(m => m.content.includes('New thought'));

      expect(hasOldThought).toBe(false);
      expect(hasNewThought).toBe(true);
    });

    it('should handle segments without final answers', () => {
      const messages = [
        { role: 'user', content: 'Query 1' },
        { role: 'assistant', content: '<thinking>Thought 1</thinking>' },
        { role: 'user', content: '<tool_result>Result</tool_result>' },
        { role: 'user', content: 'Query 2' },
        { role: 'assistant', content: '<thinking>Thought 2</thinking>' }
        // No final answer for segment 2
      ];

      const compacted = compactMessages(messages, {
        minSegmentsToKeep: 1
      });

      // Should preserve the last incomplete segment
      expect(compacted).toContain(messages[4]);
    });
  });

  describe('calculateCompactionStats', () => {
    it('should calculate correct statistics', () => {
      const original = [
        { role: 'user', content: 'A'.repeat(100) },
        { role: 'assistant', content: 'B'.repeat(200) },
        { role: 'assistant', content: 'C'.repeat(300) },
        { role: 'user', content: 'D'.repeat(400) }
      ];

      const compacted = [
        { role: 'user', content: 'A'.repeat(100) },
        { role: 'user', content: 'D'.repeat(400) }
      ];

      const stats = calculateCompactionStats(original, compacted);

      expect(stats.originalCount).toBe(4);
      expect(stats.compactedCount).toBe(2);
      expect(stats.removed).toBe(2);
      expect(stats.reductionPercent).toBe(50.0);

      // Check token estimation
      expect(stats.originalTokens).toBeGreaterThan(0);
      expect(stats.compactedTokens).toBeLessThan(stats.originalTokens);
      expect(stats.tokensSaved).toBe(stats.originalTokens - stats.compactedTokens);
    });

    it('should handle empty arrays', () => {
      const stats = calculateCompactionStats([], []);

      expect(stats.originalCount).toBe(0);
      expect(stats.compactedCount).toBe(0);
      expect(stats.removed).toBe(0);
      expect(stats.reductionPercent).toBe(0);
    });

    it('should handle no reduction', () => {
      const messages = [
        { role: 'user', content: 'Test' }
      ];

      const stats = calculateCompactionStats(messages, messages);

      expect(stats.removed).toBe(0);
      expect(stats.reductionPercent).toBe(0);
      expect(stats.tokensSaved).toBe(0);
    });

    it('should estimate tokens correctly', () => {
      const messages = [
        { role: 'user', content: 'This is a test message with about 10 words in it' }
      ];

      const stats = calculateCompactionStats(messages, []);

      // Rough estimate: ~50 chars / 4 = ~12-13 tokens
      expect(stats.originalTokens).toBeGreaterThan(10);
      expect(stats.originalTokens).toBeLessThan(20);
    });
  });

  describe('handleContextLimitError', () => {
    it('should compact messages on context limit error', () => {
      const error = new Error('context length exceeded');
      const messages = [
        { role: 'user', content: 'Query 1' },
        { role: 'assistant', content: '<thinking>Thought 1</thinking>' },
        { role: 'assistant', content: '<search>test</search>' },
        { role: 'user', content: '<tool_result>Result 1</tool_result>' },
        { role: 'user', content: 'Query 2' },
        { role: 'assistant', content: '<thinking>Thought 2</thinking>' },
        { role: 'assistant', content: '<extract>file</extract>' }
      ];

      const result = handleContextLimitError(error, messages);

      expect(result).not.toBe(null);
      expect(result.compacted).toBe(true);
      expect(result.messages.length).toBeLessThanOrEqual(messages.length);
      expect(result.stats).toBeDefined();
    });

    it('should return null for non-context errors', () => {
      const error = new Error('Network timeout');
      const messages = [
        { role: 'user', content: 'Test' }
      ];

      const result = handleContextLimitError(error, messages);

      expect(result).toBe(null);
    });

    it('should pass options to compactMessages', () => {
      const error = new Error('tokens exceed maximum');
      const messages = [
        { role: 'user', content: 'Query 1' },
        { role: 'assistant', content: '<search>test</search>' },
        { role: 'user', content: '<tool_result>Result</tool_result>' },
        { role: 'user', content: 'Query 2' },
        { role: 'assistant', content: '<extract>file</extract>' }
      ];

      const result = handleContextLimitError(error, messages, {
        keepLastSegment: true,
        minSegmentsToKeep: 3
      });

      expect(result).not.toBe(null);
      expect(result.messages).toBeDefined();
    });

    it('should handle string error messages', () => {
      const errorString = 'Input token count exceeds limit';
      const messages = [
        { role: 'user', content: 'Test' },
        { role: 'assistant', content: 'Response' }
      ];

      const result = handleContextLimitError(errorString, messages);

      expect(result).not.toBe(null);
      expect(result.compacted).toBe(true);
    });
  });

  describe('Real-world scenario', () => {
    it('should handle complex multi-turn conversation', () => {
      const messages = [
        { role: 'system', content: 'You are a helpful assistant' },

        // Turn 1 - Complete
        { role: 'user', content: 'Search for all function definitions' },
        { role: 'assistant', content: '<thinking>I need to search</thinking>' },
        { role: 'assistant', content: '<search>function</search>' },
        { role: 'user', content: '<tool_result>Found 50 functions</tool_result>' },

        // Turn 2 - Complete
        { role: 'user', content: 'Extract the first function' },
        { role: 'assistant', content: '<thinking>Let me extract</thinking>' },
        { role: 'assistant', content: '<extract>file.js:10</extract>' },
        { role: 'user', content: '<tool_result>function code here</tool_result>' },

        // Turn 3 - Complete
        { role: 'user', content: 'Modify it to add error handling' },
        { role: 'assistant', content: '<thinking>I will implement</thinking>' },
        { role: 'assistant', content: '<implement>...</implement>' },
        { role: 'user', content: '<tool_result>Success</tool_result>' },

        // Turn 4 - Active
        { role: 'user', content: 'Now test it' },
        { role: 'assistant', content: '<thinking>Running tests</thinking>' },
        { role: 'assistant', content: '<bash>npm test</bash>' }
      ];

      const compacted = compactMessages(messages, {
        keepLastSegment: true,
        minSegmentsToKeep: 1
      });

      // Should keep system + Turn 1 (user+final) + Turn 2 (user+final) + Turn 3 (user+final) + Turn 4 (all)
      expect(compacted.length).toBeLessThan(messages.length);

      // System message should be preserved
      expect(compacted[0].role).toBe('system');

      // All user messages should be preserved
      const userMessages = compacted.filter(m => m.role === 'user' && !m.content.includes('tool_result'));
      expect(userMessages.length).toBe(4);

      // Turns 1-3 should have monologues removed
      const hasThinking1 = compacted.some(m => m.content && m.content.includes('I need to search'));
      const hasThinking2 = compacted.some(m => m.content && m.content.includes('Let me extract'));
      const hasThinking3 = compacted.some(m => m.content && m.content.includes('I will implement'));

      expect(hasThinking1).toBe(false);
      expect(hasThinking2).toBe(false);
      expect(hasThinking3).toBe(false);

      // Active segment (Turn 4) should be fully preserved
      const hasThinking4 = compacted.some(m => m.content && m.content.includes('Running tests'));
      const hasBash = compacted.some(m => m.content && m.content.includes('bash'));

      expect(hasThinking4).toBe(true);
      expect(hasBash).toBe(true);
    });

    it('should provide meaningful statistics', () => {
      const messages = Array.from({ length: 50 }, (_, i) => ({
        role: i % 2 === 0 ? 'user' : 'assistant',
        content: `Message ${i} with some content that takes up space`
      }));

      const compacted = compactMessages(messages, {
        keepLastSegment: true,
        minSegmentsToKeep: 5
      });

      const stats = calculateCompactionStats(messages, compacted);

      expect(stats.originalCount).toBe(50);
      expect(stats.compactedCount).toBeLessThan(50);
      expect(stats.reductionPercent).toBeGreaterThan(0);
      expect(stats.tokensSaved).toBeGreaterThan(0);

      // Verify it's actually reducing
      expect(stats.compactedCount).toBeLessThan(stats.originalCount);
    });
  });
});
