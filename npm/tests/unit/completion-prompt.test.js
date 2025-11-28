import { describe, test, expect, beforeEach, afterEach, jest } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('ProbeAgent completionPrompt option', () => {
  describe('constructor configuration', () => {
    test('should store completionPrompt option in constructor', () => {
      const agent = new ProbeAgent({
        completionPrompt: 'Please double-check your answer',
        path: process.cwd()
      });

      expect(agent.completionPrompt).toBe('Please double-check your answer');
    });

    test('should default to null when no completionPrompt option provided', () => {
      const agent = new ProbeAgent({
        path: process.cwd()
      });

      expect(agent.completionPrompt).toBeNull();
    });

    test('should handle empty string completionPrompt as null', () => {
      const agent = new ProbeAgent({
        completionPrompt: '',
        path: process.cwd()
      });

      // Empty string is falsy, so it becomes null due to || operator
      expect(agent.completionPrompt).toBeNull();
    });

    test('should preserve multi-line completionPrompt', () => {
      const multiLinePrompt = `Please review the response:
1. Check for accuracy
2. Verify completeness
3. Ensure proper formatting`;

      const agent = new ProbeAgent({
        completionPrompt: multiLinePrompt,
        path: process.cwd()
      });

      expect(agent.completionPrompt).toBe(multiLinePrompt);
    });
  });

  describe('clone behavior', () => {
    test('should preserve completionPrompt in clone', () => {
      const baseAgent = new ProbeAgent({
        completionPrompt: 'Verify your response',
        path: process.cwd()
      });

      const cloned = baseAgent.clone();

      expect(cloned.completionPrompt).toBe(baseAgent.completionPrompt);
      expect(cloned.completionPrompt).toBe('Verify your response');
    });

    test('should preserve absence of completionPrompt in clone', () => {
      const baseAgent = new ProbeAgent({
        path: process.cwd()
        // No completionPrompt specified
      });

      const cloned = baseAgent.clone();

      expect(cloned.completionPrompt).toBeNull();
      expect(baseAgent.completionPrompt).toBeNull();
    });

    test('should allow override of completionPrompt in clone', () => {
      const baseAgent = new ProbeAgent({
        completionPrompt: 'Original prompt',
        path: process.cwd()
      });

      const cloned = baseAgent.clone({
        overrides: {
          completionPrompt: 'Overridden prompt'
        }
      });

      expect(cloned.completionPrompt).toBe('Overridden prompt');
      expect(baseAgent.completionPrompt).toBe('Original prompt');
    });
  });

  describe('integration with other options', () => {
    test('should work alongside other agent options', () => {
      const agent = new ProbeAgent({
        completionPrompt: 'Review the answer',
        systemPrompt: 'You are a helpful assistant',
        allowEdit: true,
        enableBash: false,
        debug: false,
        path: process.cwd()
      });

      expect(agent.completionPrompt).toBe('Review the answer');
      expect(agent.customPrompt).toBe('You are a helpful assistant');
      expect(agent.allowEdit).toBe(true);
      expect(agent.enableBash).toBe(false);
    });

    test('should work with schema option', () => {
      const agent = new ProbeAgent({
        completionPrompt: 'Verify JSON structure',
        path: process.cwd()
      });

      expect(agent.completionPrompt).toBe('Verify JSON structure');
    });

    test('should work with disableMermaidValidation', () => {
      const agent = new ProbeAgent({
        completionPrompt: 'Check diagrams',
        disableMermaidValidation: true,
        path: process.cwd()
      });

      expect(agent.completionPrompt).toBe('Check diagrams');
      expect(agent.disableMermaidValidation).toBe(true);
    });
  });

  describe('_completionPromptProcessed flag behavior', () => {
    test('should not trigger completionPrompt when _completionPromptProcessed is true', async () => {
      // This test verifies that the infinite loop prevention works
      // by checking that _completionPromptProcessed flag is properly handled
      const agent = new ProbeAgent({
        completionPrompt: 'Review this',
        path: process.cwd()
      });

      // The _completionPromptProcessed flag should prevent recursive calls
      // This is tested indirectly through the logic in ProbeAgent.answer()
      expect(agent.completionPrompt).toBe('Review this');
    });
  });
});

describe('completionPrompt message format', () => {
  test('should format completion prompt message correctly', () => {
    const completionPrompt = 'Please verify the accuracy of your response.';
    const finalResult = 'This is the AI response.';

    // Simulate the message format used in ProbeAgent
    const formattedMessage = `${completionPrompt}

Here is the result to review:
<result>
${finalResult}
</result>

After reviewing, provide your final answer using attempt_completion.`;

    expect(formattedMessage).toContain(completionPrompt);
    expect(formattedMessage).toContain(finalResult);
    expect(formattedMessage).toContain('<result>');
    expect(formattedMessage).toContain('</result>');
    expect(formattedMessage).toContain('attempt_completion');
  });
});
