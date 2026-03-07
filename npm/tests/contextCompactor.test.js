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
        { role: 'assistant', content: 'Let me search', toolInvocations: [{ toolName: 'search', args: { query: 'function' } }] },
        { role: 'tool', content: 'Found 10 results', toolName: 'search' },
        { role: 'assistant', content: '', toolInvocations: [{ toolName: 'attempt_completion', args: { result: 'Done' } }] }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(1);
      expect(segments[0]).toEqual({
        userIndex: 1,
        monologueIndices: [2, 3, 4],
        finalIndex: 4
      });
    });

    it('should identify multiple segments', () => {
      const messages = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'First query' },
        { role: 'assistant', content: 'Searching...', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
        { role: 'tool', content: 'Result 1', toolName: 'search' },
        { role: 'assistant', content: 'Answer 1' },
        { role: 'user', content: 'Second query' },
        { role: 'assistant', content: 'Extracting...', toolInvocations: [{ toolName: 'extract', args: { file: 'f1' } }] },
        { role: 'tool', content: 'Result 2', toolName: 'extract' },
        { role: 'assistant', content: 'Answer 2' }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(2);

      expect(segments[0].userIndex).toBe(1);
      expect(segments[0].finalIndex).toBe(4);

      expect(segments[1].userIndex).toBe(5);
      expect(segments[1].finalIndex).toBe(8);
    });

    it('should handle segment with multiple monologue messages', () => {
      const messages = [
        { role: 'user', content: 'Query' },
        { role: 'assistant', content: 'First thought', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
        { role: 'tool', content: 'Partial result', toolName: 'search' },
        { role: 'assistant', content: 'Second thought', toolInvocations: [{ toolName: 'search', args: { query: 'q2' } }] },
        { role: 'tool', content: 'Result', toolName: 'search' },
        { role: 'assistant', content: 'Done' }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(1);
      expect(segments[0].monologueIndices).toEqual([1, 2, 3, 4, 5]);
    });

    it('should handle text-only assistant message as segment end', () => {
      const messages = [
        { role: 'user', content: 'Query' },
        { role: 'assistant', content: 'Searching...', toolInvocations: [{ toolName: 'search', args: { query: 'test' } }] },
        { role: 'tool', content: 'Found results', toolName: 'search' },
        { role: 'assistant', content: 'Here is the answer' }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(1);

      expect(segments[0].userIndex).toBe(0);
      expect(segments[0].finalIndex).toBe(3);
    });

    it('should handle legacy attempt_completion as segment end (backward compat)', () => {
      const messages = [
        { role: 'user', content: 'Query' },
        { role: 'assistant', content: 'Searching...', toolInvocations: [{ toolName: 'search', args: { query: 'test' } }] },
        { role: 'tool', content: 'Found results', toolName: 'search' },
        { role: 'assistant', content: '', toolInvocations: [{ toolName: 'attempt_completion', args: { result: 'Here is the answer' } }] }
      ];

      const segments = identifyMessageSegments(messages);
      expect(segments).toHaveLength(1);

      expect(segments[0].userIndex).toBe(0);
      expect(segments[0].finalIndex).toBe(3);
    });

    it('should handle incomplete segment (no final answer)', () => {
      // An assistant message with a pending tool call is not a completion
      const messages = [
        { role: 'user', content: 'Query' },
        { role: 'assistant', content: 'In progress...', toolInvocations: [{ toolName: 'search', args: { query: 'test' } }] }
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
        { role: 'assistant', content: 'Thought 1', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
        { role: 'tool', content: 'Result 1', toolName: 'search' },
        { role: 'assistant', content: 'Answer 1' },
        // Segment 2 (old - should be compacted)
        { role: 'user', content: 'Second query' },
        { role: 'assistant', content: 'Thought 2', toolInvocations: [{ toolName: 'extract', args: { file: 'f1' } }] },
        { role: 'tool', content: 'Result 2', toolName: 'extract' },
        { role: 'assistant', content: 'Answer 2' },
        // Segment 3 (active - should be preserved, has tool call so not a completion)
        { role: 'user', content: 'Third query' },
        { role: 'assistant', content: 'Active thought', toolInvocations: [{ toolName: 'search', args: { query: 'q3' } }] }
      ];

      const compacted = compactMessages(messages, {
        keepLastSegment: true,
        minSegmentsToKeep: 1
      });

      // Should be fewer messages than original
      expect(compacted.length).toBeLessThan(messages.length);

      // Check system message preserved
      expect(compacted[0].role).toBe('system');

      // Check segment 1 and 2 compacted (intermediate monologues removed)
      const hasFirstQuery = compacted.some(m => m.content === 'First query');
      const hasThought1 = compacted.some(m => m.content && m.content.includes('Thought 1'));

      const hasSecondQuery = compacted.some(m => m.content === 'Second query');
      const hasThought2 = compacted.some(m => m.content && m.content.includes('Thought 2'));

      expect(hasFirstQuery).toBe(true);
      expect(hasThought1).toBe(false); // Thought 1 should be removed

      expect(hasSecondQuery).toBe(true);
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
        { role: 'assistant', content: 'Searching...', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
        { role: 'tool', content: 'Result', toolName: 'search' },
        { role: 'assistant', content: 'Done' }
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
        { role: 'assistant', content: 'Searching...', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
        { role: 'tool', content: 'Result 1', toolName: 'search' },
        { role: 'assistant', content: 'Done' },
        { role: 'user', content: 'Query 2' },
        { role: 'assistant', content: 'Extracting...', toolInvocations: [{ toolName: 'extract', args: { file: 'f1' } }] }
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
        { role: 'assistant', content: 'Old thought', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
        { role: 'tool', content: 'Result', toolName: 'search' },
        { role: 'assistant', content: 'Answer' },
        { role: 'user', content: 'Query 2' },
        { role: 'assistant', content: 'New thought', toolInvocations: [{ toolName: 'search', args: { query: 'q2' } }] }
      ];

      // With keepLastSegment=false and minSegmentsToKeep=1
      const compacted = compactMessages(messages, {
        keepLastSegment: false,
        minSegmentsToKeep: 1
      });

      // Should only preserve the second-to-last segment fully
      // Segment 1 should be compacted
      const hasOldThought = compacted.some(m => m.content && m.content.includes('Old thought'));
      const hasNewThought = compacted.some(m => m.content && m.content.includes('New thought'));

      expect(hasOldThought).toBe(false);
      expect(hasNewThought).toBe(true);
    });

    it('should handle segments without final answers', () => {
      const messages = [
        { role: 'user', content: 'Query 1' },
        { role: 'assistant', content: 'Thought 1', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
        { role: 'tool', content: 'Result', toolName: 'search' },
        { role: 'assistant', content: 'Answer' },
        { role: 'user', content: 'Query 2' },
        { role: 'assistant', content: 'Thought 2', toolInvocations: [{ toolName: 'search', args: { query: 'q2' } }] }
        // No final answer for segment 2 (still has pending tool call)
      ];

      const compacted = compactMessages(messages, {
        minSegmentsToKeep: 1
      });

      // Should preserve the last incomplete segment
      expect(compacted).toContain(messages[5]);
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
        { role: 'assistant', content: 'Thought 1', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
        { role: 'tool', content: 'Result 1', toolName: 'search' },
        { role: 'assistant', content: 'Answer 1' },
        { role: 'user', content: 'Query 2' },
        { role: 'assistant', content: 'Thought 2', toolInvocations: [{ toolName: 'extract', args: { file: 'f1' } }] },
        { role: 'tool', content: 'Extracted', toolName: 'extract' },
        { role: 'assistant', content: 'Extracting...', toolInvocations: [{ toolName: 'extract', args: { file: 'f2' } }] }
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
        { role: 'assistant', content: 'Searching...', toolInvocations: [{ toolName: 'search', args: { query: 'q1' } }] },
        { role: 'tool', content: 'Result', toolName: 'search' },
        { role: 'assistant', content: 'Done' },
        { role: 'user', content: 'Query 2' },
        { role: 'assistant', content: 'Extracting...', toolInvocations: [{ toolName: 'extract', args: { file: 'f1' } }] }
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

        // Turn 1 - Complete (tool call + tool result + completion text)
        { role: 'user', content: 'Search for all function definitions' },
        { role: 'assistant', content: 'I need to search', toolInvocations: [{ toolName: 'search', args: { query: 'function' } }] },
        { role: 'tool', content: 'Found 50 functions', toolName: 'search' },
        { role: 'assistant', content: 'Found 50 function definitions' },

        // Turn 2 - Complete
        { role: 'user', content: 'Extract the first function' },
        { role: 'assistant', content: 'Let me extract', toolInvocations: [{ toolName: 'extract', args: { file: 'file.js:10' } }] },
        { role: 'tool', content: 'function code here', toolName: 'extract' },
        { role: 'assistant', content: 'Here is the extracted function' },

        // Turn 3 - Complete
        { role: 'user', content: 'Modify it to add error handling' },
        { role: 'assistant', content: 'I will implement', toolInvocations: [{ toolName: 'implement', args: { file: 'file.js' } }] },
        { role: 'tool', content: 'Success', toolName: 'implement' },
        { role: 'assistant', content: 'Error handling added' },

        // Turn 4 - Active (has pending tool call)
        { role: 'user', content: 'Now test it' },
        { role: 'assistant', content: 'Running tests', toolInvocations: [{ toolName: 'bash', args: { command: 'npm test' } }] }
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
      const userMessages = compacted.filter(m => m.role === 'user');
      expect(userMessages.length).toBe(4);

      // Turns 1-3 should have intermediate tool calls removed
      const hasToolCall1 = compacted.some(m => m.content && m.content.includes('I need to search') && Array.isArray(m.toolInvocations));
      const hasToolCall2 = compacted.some(m => m.content && m.content.includes('Let me extract') && Array.isArray(m.toolInvocations));
      const hasToolCall3 = compacted.some(m => m.content && m.content.includes('I will implement') && Array.isArray(m.toolInvocations));

      expect(hasToolCall1).toBe(false);
      expect(hasToolCall2).toBe(false);
      expect(hasToolCall3).toBe(false);

      // Active segment (Turn 4) should be fully preserved
      const hasThinking4 = compacted.some(m => m.content && m.content.includes('Running tests'));

      expect(hasThinking4).toBe(true);
    });

    it('should provide meaningful statistics', () => {
      // Create segments with tool calls so intermediate messages can be compacted
      const messages = [];
      for (let i = 0; i < 10; i++) {
        messages.push({ role: 'user', content: `Query ${i} with some content that takes up space` });
        messages.push({ role: 'assistant', content: `Thinking about ${i}`, toolInvocations: [{ toolName: 'search', args: { query: `q${i}` } }] });
        messages.push({ role: 'tool', content: `Result for query ${i} with detailed content`, toolName: 'search' });
        messages.push({ role: 'assistant', content: `Answer ${i} with detailed explanation` });
      }

      const compacted = compactMessages(messages, {
        keepLastSegment: true,
        minSegmentsToKeep: 2
      });

      const stats = calculateCompactionStats(messages, compacted);

      expect(stats.originalCount).toBe(40);
      expect(stats.compactedCount).toBeLessThan(40);
      expect(stats.reductionPercent).toBeGreaterThan(0);
      expect(stats.tokensSaved).toBeGreaterThan(0);

      // Verify it's actually reducing
      expect(stats.compactedCount).toBeLessThan(stats.originalCount);
    });
  });
});
