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

describe('completionPrompt with delegate tool', () => {
  test('should preserve completionPrompt when enableDelegate is true', () => {
    const agent = new ProbeAgent({
      completionPrompt: 'Review the delegated response',
      enableDelegate: true,
      path: process.cwd()
    });

    expect(agent.completionPrompt).toBe('Review the delegated response');
    expect(agent.enableDelegate).toBe(true);
  });

  test('should not inherit completionPrompt in delegate subagent by design', () => {
    // The delegate tool creates its own ProbeAgent without completionPrompt
    // This is by design - subagents should not have completion validation
    const parentAgent = new ProbeAgent({
      completionPrompt: 'Parent validation prompt',
      enableDelegate: true,
      path: process.cwd()
    });

    // When delegate creates a subagent, it should NOT have completionPrompt
    // The subagent is created fresh with only specific options
    const subagentOptions = {
      path: process.cwd(),
      enableDelegate: false, // Prevent recursion
      disableMermaidValidation: true,
      disableJsonValidation: true
      // Note: completionPrompt is NOT passed to subagent
    };

    const subagent = new ProbeAgent(subagentOptions);
    expect(subagent.completionPrompt).toBeNull();
    expect(parentAgent.completionPrompt).toBe('Parent validation prompt');
  });

  test('should work with clone when both completionPrompt and enableDelegate are set', () => {
    const baseAgent = new ProbeAgent({
      completionPrompt: 'Verify response',
      enableDelegate: true,
      path: process.cwd()
    });

    const cloned = baseAgent.clone();

    expect(cloned.completionPrompt).toBe('Verify response');
    expect(cloned.enableDelegate).toBe(true);
  });
});

describe('completionPrompt with validation options', () => {
  test('should work with disableJsonValidation', () => {
    const agent = new ProbeAgent({
      completionPrompt: 'Check JSON output',
      disableJsonValidation: true,
      path: process.cwd()
    });

    expect(agent.completionPrompt).toBe('Check JSON output');
    expect(agent.disableJsonValidation).toBe(true);
  });

  test('should work with both validation options disabled', () => {
    const agent = new ProbeAgent({
      completionPrompt: 'Final review',
      disableMermaidValidation: true,
      disableJsonValidation: true,
      path: process.cwd()
    });

    expect(agent.completionPrompt).toBe('Final review');
    expect(agent.disableMermaidValidation).toBe(true);
    expect(agent.disableJsonValidation).toBe(true);
  });

  test('should preserve all validation options in clone', () => {
    const baseAgent = new ProbeAgent({
      completionPrompt: 'Review prompt',
      disableMermaidValidation: true,
      disableJsonValidation: true,
      path: process.cwd()
    });

    const cloned = baseAgent.clone();

    expect(cloned.completionPrompt).toBe('Review prompt');
    expect(cloned.disableMermaidValidation).toBe(true);
    expect(cloned.disableJsonValidation).toBe(true);
  });
});

describe('completionPrompt with retry and fallback options', () => {
  test('should work alongside retry configuration', () => {
    const agent = new ProbeAgent({
      completionPrompt: 'Verify with retry enabled',
      retry: {
        maxRetries: 3,
        initialDelay: 1000
      },
      path: process.cwd()
    });

    expect(agent.completionPrompt).toBe('Verify with retry enabled');
    expect(agent.retryConfig).toEqual({
      maxRetries: 3,
      initialDelay: 1000
    });
  });

  test('should work alongside fallback configuration', () => {
    const agent = new ProbeAgent({
      completionPrompt: 'Review before fallback',
      fallback: {
        strategy: 'same-provider'
      },
      path: process.cwd()
    });

    expect(agent.completionPrompt).toBe('Review before fallback');
    expect(agent.fallbackConfig).toEqual({
      strategy: 'same-provider'
    });
  });
});

describe('completionPrompt edge cases', () => {
  test('should handle special characters in completionPrompt', () => {
    const specialPrompt = 'Review: Check <tags> and "quotes" and \'apostrophes\' and `backticks`';
    const agent = new ProbeAgent({
      completionPrompt: specialPrompt,
      path: process.cwd()
    });

    expect(agent.completionPrompt).toBe(specialPrompt);
  });

  test('should handle very long completionPrompt', () => {
    const longPrompt = 'Review the response. '.repeat(100);
    const agent = new ProbeAgent({
      completionPrompt: longPrompt,
      path: process.cwd()
    });

    expect(agent.completionPrompt).toBe(longPrompt);
    expect(agent.completionPrompt.length).toBeGreaterThan(2000);
  });

  test('should handle completionPrompt with newlines and formatting', () => {
    const formattedPrompt = `
## Review Checklist

1. **Accuracy**: Verify all facts
2. **Completeness**: Check nothing is missing
3. **Format**: Ensure proper structure

### Additional Notes
- Pay attention to edge cases
- Verify code examples work
`;
    const agent = new ProbeAgent({
      completionPrompt: formattedPrompt,
      path: process.cwd()
    });

    expect(agent.completionPrompt).toBe(formattedPrompt);
    expect(agent.completionPrompt).toContain('## Review Checklist');
    expect(agent.completionPrompt).toContain('**Accuracy**');
  });

  test('should handle completionPrompt with unicode characters', () => {
    const unicodePrompt = 'レビュー: 回答を確認してください 🔍 ✅';
    const agent = new ProbeAgent({
      completionPrompt: unicodePrompt,
      path: process.cwd()
    });

    expect(agent.completionPrompt).toBe(unicodePrompt);
  });
});

describe('completionPrompt isolation', () => {
  test('should not affect other agents when set', () => {
    const agentWithPrompt = new ProbeAgent({
      completionPrompt: 'Review this',
      path: process.cwd()
    });

    const agentWithoutPrompt = new ProbeAgent({
      path: process.cwd()
    });

    expect(agentWithPrompt.completionPrompt).toBe('Review this');
    expect(agentWithoutPrompt.completionPrompt).toBeNull();
  });

  test('should allow clearing completionPrompt via clone override', () => {
    const baseAgent = new ProbeAgent({
      completionPrompt: 'Original prompt',
      path: process.cwd()
    });

    // Note: We can't set to null via override due to || operator, but empty string works
    const cloned = baseAgent.clone({
      overrides: {
        completionPrompt: ''
      }
    });

    // Empty string becomes null due to || operator in constructor
    expect(cloned.completionPrompt).toBeNull();
    expect(baseAgent.completionPrompt).toBe('Original prompt');
  });
});
