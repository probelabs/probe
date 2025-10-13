import { describe, test, expect, beforeEach } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('ProbeAgent.clone()', () => {
  let baseAgent;

  beforeEach(async () => {
    baseAgent = new ProbeAgent({
      sessionId: 'test-base',
      path: process.cwd(),
      debug: false
    });

    // Don't initialize - we'll manually set history for testing
  });

  describe('Basic Cloning', () => {
    test('should clone agent with same configuration', () => {
      baseAgent.history = [
        { role: 'system', content: 'You are a helpful assistant' },
        { role: 'user', content: 'Hello' },
        { role: 'assistant', content: 'Hi there!' }
      ];

      const cloned = baseAgent.clone();

      expect(cloned).toBeInstanceOf(ProbeAgent);
      expect(cloned.sessionId).not.toBe(baseAgent.sessionId);
      expect(cloned.history).toHaveLength(3);
      expect(cloned.allowedFolders).toEqual(baseAgent.allowedFolders);
    });

    test('should use custom session ID when provided', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' }
      ];

      const cloned = baseAgent.clone({
        sessionId: 'custom-session-id'
      });

      expect(cloned.sessionId).toBe('custom-session-id');
    });

    test('should deep copy history by default', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'Test' }
      ];

      const cloned = baseAgent.clone();

      // Modify cloned history
      cloned.history[1].content = 'Modified';

      // Original should be unchanged
      expect(baseAgent.history[1].content).toBe('Test');
    });

    test('should shallow copy when deepCopy is false', () => {
      baseAgent.history = [
        { role: 'user', content: 'Test' }
      ];

      const cloned = baseAgent.clone({ deepCopy: false });

      // Arrays are different
      expect(cloned.history).not.toBe(baseAgent.history);

      // But they share the same objects
      expect(cloned.history[0]).toBe(baseAgent.history[0]);
    });
  });

  describe('Internal Message Filtering', () => {
    test('should strip schema reminder messages', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'List functions' },
        { role: 'assistant', content: 'Here are the functions...' },
        {
          role: 'user',
          content: 'IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.\nYour response must conform to this schema: {...}'
        },
        { role: 'assistant', content: '{"functions": [...]}' }
      ];

      const cloned = baseAgent.clone({
        stripInternalMessages: true
      });

      // Should remove the schema reminder (index 3)
      expect(cloned.history).toHaveLength(4);
      expect(cloned.history.some(m => m.content.includes('IMPORTANT: A schema was provided'))).toBe(false);
    });

    test('should strip tool use reminder messages', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'Search for files' },
        { role: 'assistant', content: 'I will search...' },
        {
          role: 'user',
          content: 'Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information.\n\nRemember: Use proper XML format with BOTH opening and closing tags:\n\n<tool_name>\n<parameter>value</parameter>\n</tool_name>'
        },
        { role: 'assistant', content: '<search><query>files</query></search>' }
      ];

      const cloned = baseAgent.clone({
        stripInternalMessages: true
      });

      // Should remove the tool reminder (index 3)
      expect(cloned.history).toHaveLength(4);
      expect(cloned.history.some(m => m.content.includes('Please use one of the available tools'))).toBe(false);
    });

    test('should strip mermaid fix prompts', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'Create a diagram' },
        { role: 'assistant', content: 'graph TD\nA -> B' },
        {
          role: 'user',
          content: 'The mermaid diagram in your response has syntax errors. Please fix the mermaid syntax errors.\n\nHere is the corrected version:\n```mermaid\ngraph TD\nA --> B\n```'
        },
        { role: 'assistant', content: 'Fixed diagram' }
      ];

      const cloned = baseAgent.clone({
        stripInternalMessages: true
      });

      // Should remove the mermaid fix prompt (index 3)
      expect(cloned.history).toHaveLength(4);
      expect(cloned.history.some(m => m.content.includes('mermaid diagram in your response has syntax errors'))).toBe(false);
    });

    test('should strip JSON correction prompts', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'Generate JSON' },
        { role: 'assistant', content: 'Invalid JSON' },
        {
          role: 'user',
          content: 'Your response does not match the expected JSON schema. Please provide a valid JSON response.\n\nSchema validation error: ...'
        },
        { role: 'assistant', content: '{"valid": true}' }
      ];

      const cloned = baseAgent.clone({
        stripInternalMessages: true
      });

      // Should remove the JSON correction prompt (index 3)
      expect(cloned.history).toHaveLength(4);
      expect(cloned.history.some(m => m.content.includes('does not match the expected JSON schema'))).toBe(false);
    });

    test('should keep all messages when stripInternalMessages is false', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'Test' },
        { role: 'user', content: 'IMPORTANT: A schema was provided. You MUST respond...' },
        { role: 'assistant', content: 'Response' }
      ];

      const cloned = baseAgent.clone({
        stripInternalMessages: false
      });

      // Should keep all messages including internal ones
      expect(cloned.history).toHaveLength(4);
      expect(cloned.history.some(m => m.content.includes('IMPORTANT: A schema was provided'))).toBe(true);
    });

    test('should only strip user role messages', () => {
      baseAgent.history = [
        { role: 'system', content: 'IMPORTANT: A schema was provided' }, // Not stripped (system)
        { role: 'user', content: 'IMPORTANT: A schema was provided' }, // Stripped (user)
        { role: 'assistant', content: 'IMPORTANT: A schema was provided' } // Not stripped (assistant)
      ];

      const cloned = baseAgent.clone({
        stripInternalMessages: true
      });

      // Should only strip the user message
      expect(cloned.history).toHaveLength(2);
      expect(cloned.history[0].role).toBe('system');
      expect(cloned.history[1].role).toBe('assistant');
    });
  });

  describe('System Message Handling', () => {
    test('should keep system message by default', () => {
      baseAgent.history = [
        { role: 'system', content: 'You are a helpful assistant' },
        { role: 'user', content: 'Hello' }
      ];

      const cloned = baseAgent.clone();

      expect(cloned.history[0].role).toBe('system');
      expect(cloned.history[0].content).toBe('You are a helpful assistant');
    });

    test('should remove system message when keepSystemMessage is false', () => {
      baseAgent.history = [
        { role: 'system', content: 'You are a helpful assistant' },
        { role: 'user', content: 'Hello' },
        { role: 'assistant', content: 'Hi!' }
      ];

      const cloned = baseAgent.clone({
        keepSystemMessage: false
      });

      expect(cloned.history).toHaveLength(2);
      expect(cloned.history[0].role).toBe('user');
    });
  });

  describe('Configuration Overrides', () => {
    test('should override configuration with provided options', () => {
      baseAgent.debug = false;
      baseAgent.allowEdit = false;
      baseAgent.maxIterations = 30;

      const cloned = baseAgent.clone({
        overrides: {
          debug: true,
          allowEdit: true,
          maxIterations: 50
        }
      });

      expect(cloned.debug).toBe(true);
      expect(cloned.allowEdit).toBe(true);
      expect(cloned.maxIterations).toBe(50);
    });

    test('should allow changing promptType via overrides', () => {
      baseAgent.promptType = 'code-explorer';

      const cloned = baseAgent.clone({
        overrides: {
          promptType: 'architect'
        }
      });

      expect(cloned.promptType).toBe('architect');
    });

    test('should preserve non-overridden configuration', () => {
      baseAgent.outline = true;
      baseAgent.maxResponseTokens = 2000;

      const cloned = baseAgent.clone({
        overrides: {
          debug: true
        }
      });

      expect(cloned.outline).toBe(true);
      expect(cloned.maxResponseTokens).toBe(2000);
    });
  });

  describe('Complex History Scenarios', () => {
    test('should handle mixed internal and regular messages', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'Question 1' },
        { role: 'assistant', content: 'Answer 1' },
        { role: 'user', content: 'Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information.\n\nRemember: Use proper XML format with BOTH opening and closing tags:' }, // Internal
        { role: 'assistant', content: 'Tool call' },
        { role: 'user', content: 'Question 2' },
        { role: 'assistant', content: 'Answer 2' },
        { role: 'user', content: 'IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.' }, // Internal
        { role: 'assistant', content: 'Schema response' }
      ];

      const cloned = baseAgent.clone({
        stripInternalMessages: true
      });

      // Should have 7 messages (removed 2 internal)
      expect(cloned.history).toHaveLength(7);
      expect(cloned.history.filter(m => m.role === 'user')).toHaveLength(2);
      expect(cloned.history.filter(m => m.role === 'assistant')).toHaveLength(4);
    });

    test('should handle empty history', () => {
      baseAgent.history = [];

      const cloned = baseAgent.clone();

      expect(cloned.history).toHaveLength(0);
    });

    test('should handle history with only system message', () => {
      baseAgent.history = [
        { role: 'system', content: 'System only' }
      ];

      const cloned = baseAgent.clone();

      expect(cloned.history).toHaveLength(1);
      expect(cloned.history[0].role).toBe('system');
    });

    test('should handle messages with complex content structure', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        {
          role: 'user',
          content: [
            { type: 'text', text: 'Hello' },
            { type: 'image', image: 'base64...' }
          ]
        },
        { role: 'assistant', content: 'Response' }
      ];

      const cloned = baseAgent.clone();

      expect(cloned.history).toHaveLength(3);
      expect(Array.isArray(cloned.history[1].content)).toBe(true);
    });
  });

  describe('Cache Efficiency', () => {
    test('should preserve system message for cache efficiency', () => {
      baseAgent.history = [
        { role: 'system', content: 'Long expensive system prompt...' },
        { role: 'user', content: 'Q1' },
        { role: 'assistant', content: 'A1' },
        { role: 'user', content: 'IMPORTANT: A schema was provided...' }, // Internal
        { role: 'assistant', content: 'A2' }
      ];

      const cloned = baseAgent.clone();

      // System message should be at index 0 for cache efficiency
      expect(cloned.history[0].role).toBe('system');
      expect(cloned.history[0].content).toBe('Long expensive system prompt...');

      // Internal message should be removed
      expect(cloned.history.some(m =>
        m.content && m.content.includes('IMPORTANT: A schema was provided')
      )).toBe(false);
    });
  });

  describe('Edge Cases', () => {
    test('should handle null content', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: null },
        { role: 'assistant', content: 'Response' }
      ];

      const cloned = baseAgent.clone();

      expect(cloned.history).toHaveLength(3);
    });

    test('should handle undefined content', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: undefined },
        { role: 'assistant', content: 'Response' }
      ];

      const cloned = baseAgent.clone();

      expect(cloned.history).toHaveLength(3);
    });

    test('should handle messages with empty content', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: '' },
        { role: 'assistant', content: 'Response' }
      ];

      const cloned = baseAgent.clone();

      expect(cloned.history).toHaveLength(3);
    });
  });

  describe('Multiple Clones', () => {
    test('should allow creating multiple independent clones', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'Test' }
      ];

      const clone1 = baseAgent.clone({ sessionId: 'clone1' });
      const clone2 = baseAgent.clone({ sessionId: 'clone2' });
      const clone3 = baseAgent.clone({ sessionId: 'clone3' });

      // All should have independent histories
      clone1.history.push({ role: 'user', content: 'Clone1' });
      clone2.history.push({ role: 'user', content: 'Clone2' });
      clone3.history.push({ role: 'user', content: 'Clone3' });

      expect(clone1.history).toHaveLength(3);
      expect(clone2.history).toHaveLength(3);
      expect(clone3.history).toHaveLength(3);
      expect(baseAgent.history).toHaveLength(2); // Unchanged

      expect(clone1.history[2].content).toBe('Clone1');
      expect(clone2.history[2].content).toBe('Clone2');
      expect(clone3.history[2].content).toBe('Clone3');
    });

    test('should allow chaining clones', () => {
      baseAgent.history = [
        { role: 'system', content: 'System' },
        { role: 'user', content: 'Q1' },
        { role: 'assistant', content: 'A1' }
      ];

      const clone1 = baseAgent.clone({ sessionId: 'gen1' });
      clone1.history.push({ role: 'user', content: 'Q2' });
      clone1.history.push({ role: 'assistant', content: 'A2' });

      const clone2 = clone1.clone({ sessionId: 'gen2' });
      clone2.history.push({ role: 'user', content: 'Q3' });

      const clone3 = clone2.clone({ sessionId: 'gen3' });

      expect(baseAgent.history).toHaveLength(3);
      expect(clone1.history).toHaveLength(5);
      expect(clone2.history).toHaveLength(6);
      expect(clone3.history).toHaveLength(6);
    });
  });
});
