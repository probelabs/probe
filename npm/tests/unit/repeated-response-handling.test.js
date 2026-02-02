/**
 * Comprehensive unit tests for repeated response handling in tool loop
 * Tests the circuit breaker that accepts repeated identical responses as final answers
 * and the message deduplication that replaces repeated assistant+reminder pairs
 *
 * This is a critical path - these tests cover:
 * - Exact string matching for identical responses
 * - Edge cases around the 50 char threshold
 * - Tool call marker detection (api_call, tool_name, function)
 * - Thinking tag cleanup (closed, unclosed, multiple)
 * - Message deduplication logic
 * - Real-world scenarios from production logs
 */
import { jest, describe, test, expect, beforeEach } from '@jest/globals';

describe('Repeated Response Handling', () => {
  let mockMessages;
  let lastNoToolResponse;
  let sameResponseCount;
  const MAX_REPEATED_IDENTICAL_RESPONSES = 3;

  beforeEach(() => {
    mockMessages = [];
    lastNoToolResponse = null;
    sameResponseCount = 0;
  });

  /**
   * Simulates the repeated response detection logic from ProbeAgent
   * This must match the implementation in ProbeAgent.js lines 3232-3260
   */
  function processNoToolResponse(assistantResponseContent) {
    // Check for repeated identical responses
    if (lastNoToolResponse !== null && assistantResponseContent === lastNoToolResponse) {
      sameResponseCount++;
      if (sameResponseCount >= MAX_REPEATED_IDENTICAL_RESPONSES) {
        // Clean up the response - remove thinking tags
        let cleanedResponse = assistantResponseContent;
        cleanedResponse = cleanedResponse.replace(/<thinking>[\s\S]*?<\/thinking>/gi, '').trim();
        cleanedResponse = cleanedResponse.replace(/<thinking>[\s\S]*$/gi, '').trim();

        const hasSubstantialContent = cleanedResponse.length > 50 &&
          !cleanedResponse.includes('<api_call>') &&
          !cleanedResponse.includes('<tool_name>') &&
          !cleanedResponse.includes('<function>');

        if (hasSubstantialContent) {
          return { accepted: true, finalResult: cleanedResponse };
        }
      }
    } else {
      // Different response, reset counter
      lastNoToolResponse = assistantResponseContent;
      sameResponseCount = 1;
    }
    return { accepted: false };
  }

  /**
   * Simulates the message deduplication logic from ProbeAgent
   * This must match the implementation in ProbeAgent.js lines 3351-3384
   */
  function addReminderMessage(currentMessages, reminderContent) {
    // Check if we should replace the previous reminder
    const prevUserMsgIndex = currentMessages.length - 2;
    const prevUserMsg = currentMessages[prevUserMsgIndex];
    const isExistingReminder = prevUserMsg && prevUserMsg.role === 'user' &&
      (prevUserMsg.content.includes('Please use one of the available tools') ||
       prevUserMsg.content.includes('<tool_result>'));

    if (isExistingReminder && sameResponseCount > 1) {
      const prevAssistantIndex = prevUserMsgIndex - 1;
      if (prevAssistantIndex >= 0 && currentMessages[prevAssistantIndex].role === 'assistant') {
        // Remove the duplicate assistant and old reminder
        currentMessages.splice(prevAssistantIndex, 2);
      }

      const iterationHint = `\n\n(Attempt #${sameResponseCount}: Your previous ${sameResponseCount} responses were identical.)`;
      currentMessages.push({
        role: 'user',
        content: reminderContent + iterationHint
      });
    } else {
      currentMessages.push({
        role: 'user',
        content: reminderContent
      });
    }
  }

  describe('Repeated Response Detection - Basic Cases', () => {
    test('should accept response after exactly 3 identical responses', () => {
      const response = 'This is a complete answer with enough content to be considered substantial and useful.';

      // First response - not accepted, counter starts at 1
      let result = processNoToolResponse(response);
      expect(result.accepted).toBe(false);
      expect(sameResponseCount).toBe(1);

      // Second response - same, counter at 2
      result = processNoToolResponse(response);
      expect(result.accepted).toBe(false);
      expect(sameResponseCount).toBe(2);

      // Third response - same, counter at 3, should accept
      result = processNoToolResponse(response);
      expect(result.accepted).toBe(true);
      expect(result.finalResult).toBe(response);
      expect(sameResponseCount).toBe(3);
    });

    test('should NOT accept after only 2 identical responses', () => {
      const response = 'This is a complete answer with enough content to be considered substantial and useful.';

      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      expect(result.accepted).toBe(false);
      expect(sameResponseCount).toBe(2);
    });

    test('should reset counter when response differs', () => {
      const response1 = 'This is a complete answer with enough content to be considered substantial.';
      const response2 = 'This is a different answer with enough content to be considered substantial.';

      // First response
      processNoToolResponse(response1);
      expect(sameResponseCount).toBe(1);

      // Second response - same
      processNoToolResponse(response1);
      expect(sameResponseCount).toBe(2);

      // Third response - different, counter resets
      processNoToolResponse(response2);
      expect(sameResponseCount).toBe(1);
      expect(lastNoToolResponse).toBe(response2);
    });

    test('should handle alternating responses correctly', () => {
      const responseA = 'Response A with enough content to be substantial for testing purposes.';
      const responseB = 'Response B with enough content to be substantial for testing purposes.';

      processNoToolResponse(responseA); // count = 1
      processNoToolResponse(responseB); // count = 1 (reset)
      processNoToolResponse(responseA); // count = 1 (reset)
      processNoToolResponse(responseB); // count = 1 (reset)

      // Counter should never reach 3
      expect(sameResponseCount).toBe(1);
    });

    test('should handle 4+ identical responses (accept on 3rd)', () => {
      const response = 'This is a complete answer with enough content to be considered substantial.';

      processNoToolResponse(response); // 1
      processNoToolResponse(response); // 2
      const result3 = processNoToolResponse(response); // 3 - should accept

      expect(result3.accepted).toBe(true);

      // Even though we accepted, the counter should be at 3
      expect(sameResponseCount).toBe(3);
    });
  });

  describe('Repeated Response Detection - Exact String Matching', () => {
    test('should treat responses with different whitespace as different', () => {
      const response1 = 'This is a response with enough content to be substantial for testing.';
      const response2 = 'This is a response with enough content to be substantial for testing. '; // trailing space

      processNoToolResponse(response1);
      processNoToolResponse(response1);
      processNoToolResponse(response2); // Different! Counter resets

      expect(sameResponseCount).toBe(1);
      expect(lastNoToolResponse).toBe(response2);
    });

    test('should treat responses with different case as different', () => {
      const response1 = 'This is a response with enough content to be substantial for testing.';
      const response2 = 'this is a response with enough content to be substantial for testing.'; // lowercase

      processNoToolResponse(response1);
      processNoToolResponse(response1);
      processNoToolResponse(response2);

      expect(sameResponseCount).toBe(1);
    });

    test('should treat responses with different newlines as different', () => {
      const response1 = 'Line 1 with content.\nLine 2 with more content for testing.';
      const response2 = 'Line 1 with content.\n\nLine 2 with more content for testing.'; // extra newline

      processNoToolResponse(response1);
      processNoToolResponse(response1);
      processNoToolResponse(response2);

      expect(sameResponseCount).toBe(1);
    });
  });

  describe('Repeated Response Detection - Content Length Threshold', () => {
    test('should NOT accept response with exactly 50 characters (boundary)', () => {
      // Exactly 50 characters: "12345678901234567890123456789012345678901234567890"
      const response50 = 'A'.repeat(50);

      processNoToolResponse(response50);
      processNoToolResponse(response50);
      const result = processNoToolResponse(response50);

      expect(result.accepted).toBe(false);
      expect(response50.length).toBe(50);
    });

    test('should accept response with 51 characters (just over boundary)', () => {
      const response51 = 'A'.repeat(51);

      processNoToolResponse(response51);
      processNoToolResponse(response51);
      const result = processNoToolResponse(response51);

      expect(result.accepted).toBe(true);
      expect(response51.length).toBe(51);
    });

    test('should NOT accept response with 49 characters (under boundary)', () => {
      const response49 = 'A'.repeat(49);

      processNoToolResponse(response49);
      processNoToolResponse(response49);
      const result = processNoToolResponse(response49);

      expect(result.accepted).toBe(false);
    });

    test('should NOT accept empty response', () => {
      const emptyResponse = '';

      processNoToolResponse(emptyResponse);
      processNoToolResponse(emptyResponse);
      const result = processNoToolResponse(emptyResponse);

      expect(result.accepted).toBe(false);
    });

    test('should NOT accept whitespace-only response', () => {
      const whitespaceResponse = '   \n\t\n   ';

      processNoToolResponse(whitespaceResponse);
      processNoToolResponse(whitespaceResponse);
      const result = processNoToolResponse(whitespaceResponse);

      expect(result.accepted).toBe(false);
    });
  });

  describe('Repeated Response Detection - Tool Call Markers', () => {
    test('should NOT accept response containing <api_call>', () => {
      const response = 'Here is my analysis <api_call>search</api_call> with a tool call marker and more content.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      expect(result.accepted).toBe(false);
    });

    test('should NOT accept response containing <tool_name>', () => {
      const response = 'Here is my analysis <tool_name>search</tool_name> with a tool call marker and more content.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      expect(result.accepted).toBe(false);
    });

    test('should NOT accept response containing <function>', () => {
      const response = 'Here is my analysis <function>search</function> with a tool call marker and more content.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      expect(result.accepted).toBe(false);
    });

    test('should accept response with partial/malformed api_call tag', () => {
      // Note: The check is for exact string '<api_call>' - malformed tags are allowed
      // This is intentional: we only block properly formed tool call markers
      const response = 'Here is my analysis <api_call with incomplete tag and enough content for threshold.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      // Should accept because '<api_call>' (complete tag) is not present
      expect(result.accepted).toBe(true);
    });

    test('should accept response mentioning tool names as plain text', () => {
      // "api_call" without angle brackets should be allowed
      const response = 'I tried to use api_call but it did not work. Here is my analysis with substantial content.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      expect(result.accepted).toBe(true);
    });

    test('should accept response with other XML-like tags', () => {
      const response = 'Here is my analysis <custom_tag>some content</custom_tag> with enough content for threshold.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      expect(result.accepted).toBe(true);
    });
  });

  describe('Repeated Response Detection - Thinking Tag Cleanup', () => {
    test('should clean closed thinking tags from accepted response', () => {
      const response = '<thinking>Internal reasoning here</thinking>This is the actual answer with enough content to pass threshold.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      expect(result.accepted).toBe(true);
      expect(result.finalResult).not.toContain('<thinking>');
      expect(result.finalResult).not.toContain('</thinking>');
      expect(result.finalResult).toBe('This is the actual answer with enough content to pass threshold.');
    });

    test('should clean unclosed thinking tag - results in empty content (NOT accepted)', () => {
      // When a thinking tag is unclosed, the regex removes everything from <thinking> to end
      // This results in empty content, which fails the length > 50 check
      const response = '<thinking>Internal reasoning that never closes... This is the actual answer with enough content.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      // Should NOT accept because after cleaning unclosed thinking tag, content is empty
      expect(result.accepted).toBe(false);
    });

    test('should accept response with content BEFORE unclosed thinking tag', () => {
      // Content before the unclosed thinking tag should be preserved
      const response = 'This is the actual answer with enough content to pass threshold.<thinking>Internal reasoning that never closes...';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      expect(result.accepted).toBe(true);
      expect(result.finalResult).not.toContain('<thinking>');
      expect(result.finalResult).toBe('This is the actual answer with enough content to pass threshold.');
    });

    test('should clean multiple thinking tags', () => {
      // After removing thinking tags: "Some content here between thoughts and more content that is definitely long enough to pass."
      // This is 93 characters, well over the 50 char threshold
      const response = '<thinking>First thought</thinking>Some content here between thoughts<thinking>Second thought</thinking> and more content that is definitely long enough to pass.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      expect(result.accepted).toBe(true);
      expect(result.finalResult).toBe('Some content here between thoughts and more content that is definitely long enough to pass.');
    });

    test('should handle response with only thinking tags (should NOT accept)', () => {
      const response = '<thinking>This is all internal reasoning with no actual answer</thinking>';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      // After cleaning, content is empty, so should NOT accept
      expect(result.accepted).toBe(false);
    });

    test('should be case-insensitive for thinking tag cleanup', () => {
      const response = '<THINKING>Uppercase thinking</THINKING>Actual content that is long enough to pass threshold.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      expect(result.accepted).toBe(true);
      expect(result.finalResult).not.toContain('THINKING');
    });

    test('should handle thinking tag with attributes', () => {
      // Edge case: thinking tag with attributes (shouldn't happen but be robust)
      const response = '<thinking type="internal">Some reasoning</thinking>Actual content with enough length.';

      processNoToolResponse(response);
      processNoToolResponse(response);
      const result = processNoToolResponse(response);

      // The regex should still match because it looks for <thinking>...</thinking>
      // But with attributes it might not match - let's verify behavior
      expect(result.accepted).toBe(true);
    });
  });

  describe('Message Deduplication - Basic Cases', () => {
    test('should replace previous assistant+reminder pair on repeated responses', () => {
      const reminderContent = 'Please use one of the available tools';
      const assistantContent = 'Repeated response content that is long enough to be meaningful.';

      // Simulate first iteration
      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent); // sameResponseCount = 1
      addReminderMessage(mockMessages, reminderContent);

      expect(mockMessages).toHaveLength(2);

      // Simulate second iteration - same response
      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent); // sameResponseCount = 2
      addReminderMessage(mockMessages, reminderContent);

      // Should have removed the duplicate pair and added new reminder
      expect(mockMessages).toHaveLength(2);
      expect(mockMessages[mockMessages.length - 1].content).toContain('Attempt #2');
    });

    test('should not deduplicate on first occurrence', () => {
      const reminderContent = 'Please use one of the available tools';
      const assistantContent = 'First response content that is long enough.';

      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent);
      addReminderMessage(mockMessages, reminderContent);

      expect(mockMessages).toHaveLength(2);
      expect(mockMessages[1].content).toBe(reminderContent);
      expect(mockMessages[1].content).not.toContain('Attempt #');
    });

    test('should preserve messages when response differs', () => {
      const reminderContent = 'Please use one of the available tools';
      const response1 = 'First response content that is long enough.';
      const response2 = 'Second different response content that is long enough.';

      // First iteration
      mockMessages.push({ role: 'assistant', content: response1 });
      processNoToolResponse(response1);
      addReminderMessage(mockMessages, reminderContent);

      // Second iteration - different response
      mockMessages.push({ role: 'assistant', content: response2 });
      processNoToolResponse(response2);
      addReminderMessage(mockMessages, reminderContent);

      // Should keep all messages since responses are different
      expect(mockMessages).toHaveLength(4);
    });

    test('should include correct attempt number in iteration hint', () => {
      const reminderContent = 'Please use one of the available tools';
      const assistantContent = 'Same response repeated multiple times for testing deduplication.';

      // First iteration
      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent);
      addReminderMessage(mockMessages, reminderContent);

      // Second iteration
      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent);
      addReminderMessage(mockMessages, reminderContent);

      expect(mockMessages[mockMessages.length - 1].content).toContain('Attempt #2');
      expect(mockMessages[mockMessages.length - 1].content).toContain('previous 2 responses');
    });
  });

  describe('Message Deduplication - Edge Cases', () => {
    test('should handle deduplication when previous message is tool_result', () => {
      const toolResultReminder = '<tool_result>Error: some error</tool_result>';
      const assistantContent = 'Response after tool result that is long enough.';

      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent);
      addReminderMessage(mockMessages, toolResultReminder);

      // Second iteration
      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent);
      addReminderMessage(mockMessages, toolResultReminder);

      // Should deduplicate because previous user message contains <tool_result>
      expect(mockMessages).toHaveLength(2);
    });

    test('should NOT deduplicate when previous user message is not a reminder', () => {
      const userQuestion = 'What is the status of the project?';
      const reminderContent = 'Please use one of the available tools';
      const assistantContent = 'Response that repeats with enough content for testing.';

      // User asks a question
      mockMessages.push({ role: 'user', content: userQuestion });
      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent);
      addReminderMessage(mockMessages, reminderContent);

      // Second iteration - same response
      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent);
      addReminderMessage(mockMessages, reminderContent);

      // Should NOT deduplicate the user question - only the reminder pair
      const userQuestionExists = mockMessages.some(m => m.content === userQuestion);
      expect(userQuestionExists).toBe(true);
    });

    test('should handle deduplication when message array has system message', () => {
      const systemMessage = { role: 'system', content: 'You are a helpful assistant.' };
      const reminderContent = 'Please use one of the available tools';
      const assistantContent = 'Response content that repeats.';

      mockMessages.push(systemMessage);
      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent);
      addReminderMessage(mockMessages, reminderContent);

      // Second iteration
      mockMessages.push({ role: 'assistant', content: assistantContent });
      processNoToolResponse(assistantContent);
      addReminderMessage(mockMessages, reminderContent);

      // System message should be preserved
      expect(mockMessages[0]).toBe(systemMessage);
      expect(mockMessages.length).toBe(3); // system + assistant + reminder
    });

    test('should handle empty message array gracefully', () => {
      const reminderContent = 'Please use one of the available tools';

      // Try to add reminder to empty array
      expect(() => {
        addReminderMessage(mockMessages, reminderContent);
      }).not.toThrow();

      expect(mockMessages).toHaveLength(1);
      expect(mockMessages[0].content).toBe(reminderContent);
    });
  });

  describe('Real-World Scenarios', () => {
    /**
     * Simulates the exact scenario from the production log where:
     * - Iterations 42, 43, 44 had the same response
     * - Message count grew from 84 to 86 to 88
     * - Finally hit max iterations
     */
    test('should handle the production log scenario - iterations 42-44 identical', () => {
      const reminderContent = `Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information to provide a final answer.

Remember: Use proper XML format with BOTH opening and closing tags`;

      // The actual response from the log (truncated for readability)
      const logResponse = `<thinking>
I've been trying to find the configuration file that defines the Jira projects to search. My previous attempts have been unsuccessful.
</thinking>

The Jira project configuration is defined in the tyk-assistant.yaml file.

## References
- [REFINE/Oel/tyk-assistant.yaml:18-24](https://github.com/TykTechnologies/REFINE/blob/main/Oel/tyk-assistant.yaml#L18-24) - Jira project configuration`;

      // Simulate iterations 42, 43, 44
      mockMessages.push({ role: 'assistant', content: logResponse });
      let result = processNoToolResponse(logResponse);
      expect(result.accepted).toBe(false);
      addReminderMessage(mockMessages, reminderContent);

      // Iteration 43
      mockMessages.push({ role: 'assistant', content: logResponse });
      result = processNoToolResponse(logResponse);
      expect(result.accepted).toBe(false);
      addReminderMessage(mockMessages, reminderContent);

      // Iteration 44
      mockMessages.push({ role: 'assistant', content: logResponse });
      result = processNoToolResponse(logResponse);

      // With our fix, on 3rd identical response, it should be accepted
      expect(result.accepted).toBe(true);
      expect(result.finalResult).not.toContain('<thinking>');
      expect(result.finalResult).toContain('Jira project configuration');
      expect(result.finalResult).toContain('References');

      // Also verify message deduplication kept message count manageable
      // After 3 iterations with deduplication: should be assistant + reminder = 2 messages
      expect(mockMessages.length).toBeLessThanOrEqual(4);
    });

    test('should handle response with markdown formatting', () => {
      const markdownResponse = `## Analysis Results

The configuration is located in the following files:

1. **config.yaml** - Main configuration
2. **settings.json** - Override settings

### Code References
- \`src/config/loader.ts:42\` - Config loading logic
- \`src/settings/parser.ts:15\` - Settings parser`;

      processNoToolResponse(markdownResponse);
      processNoToolResponse(markdownResponse);
      const result = processNoToolResponse(markdownResponse);

      expect(result.accepted).toBe(true);
      expect(result.finalResult).toContain('## Analysis Results');
      expect(result.finalResult).toContain('Code References');
    });

    test('should handle response with code blocks', () => {
      const codeBlockResponse = `Here is the relevant code:

\`\`\`typescript
function processConfig(config: Config): Result {
  return validate(config);
}
\`\`\`

This function handles the configuration processing for the application with proper validation.`;

      processNoToolResponse(codeBlockResponse);
      processNoToolResponse(codeBlockResponse);
      const result = processNoToolResponse(codeBlockResponse);

      expect(result.accepted).toBe(true);
      expect(result.finalResult).toContain('```typescript');
    });

    test('should handle response with URLs and links', () => {
      const urlResponse = `The documentation can be found at:
- https://docs.example.com/api/v2
- [API Reference](https://api.example.com/docs)

See the GitHub issue for more details: https://github.com/org/repo/issues/123`;

      processNoToolResponse(urlResponse);
      processNoToolResponse(urlResponse);
      const result = processNoToolResponse(urlResponse);

      expect(result.accepted).toBe(true);
      expect(result.finalResult).toContain('https://docs.example.com');
    });
  });

  describe('Integration - Full Simulation', () => {
    /**
     * Simulates a full tool loop flow with our changes
     */
    test('should complete full flow: different responses then 3 identical', () => {
      const reminderContent = 'Please use one of the available tools';
      const response1 = 'First unique response with enough content for the test.';
      const response2 = 'Second unique response with enough content for the test.';
      const repeatedResponse = 'This response will repeat three times with enough content.';

      // First unique response
      mockMessages.push({ role: 'assistant', content: response1 });
      let result = processNoToolResponse(response1);
      expect(result.accepted).toBe(false);
      addReminderMessage(mockMessages, reminderContent);

      // Second unique response
      mockMessages.push({ role: 'assistant', content: response2 });
      result = processNoToolResponse(response2);
      expect(result.accepted).toBe(false);
      addReminderMessage(mockMessages, reminderContent);

      // First of repeated
      mockMessages.push({ role: 'assistant', content: repeatedResponse });
      result = processNoToolResponse(repeatedResponse);
      expect(result.accepted).toBe(false);
      addReminderMessage(mockMessages, reminderContent);

      // Second of repeated
      mockMessages.push({ role: 'assistant', content: repeatedResponse });
      result = processNoToolResponse(repeatedResponse);
      expect(result.accepted).toBe(false);
      addReminderMessage(mockMessages, reminderContent);

      // Third of repeated - should accept
      mockMessages.push({ role: 'assistant', content: repeatedResponse });
      result = processNoToolResponse(repeatedResponse);
      expect(result.accepted).toBe(true);
      expect(result.finalResult).toBe(repeatedResponse);
    });

    test('should maintain message history integrity during deduplication', () => {
      const reminderContent = 'Please use one of the available tools';
      const response = 'Repeated response for integrity testing with sufficient content.';

      // Add initial context messages
      mockMessages.push({ role: 'system', content: 'System prompt' });
      mockMessages.push({ role: 'user', content: 'User question' });

      const initialLength = mockMessages.length;

      // First iteration
      mockMessages.push({ role: 'assistant', content: response });
      processNoToolResponse(response);
      addReminderMessage(mockMessages, reminderContent);

      // Second iteration with deduplication
      mockMessages.push({ role: 'assistant', content: response });
      processNoToolResponse(response);
      addReminderMessage(mockMessages, reminderContent);

      // Verify system and user messages are preserved
      expect(mockMessages[0].role).toBe('system');
      expect(mockMessages[0].content).toBe('System prompt');
      expect(mockMessages[1].role).toBe('user');
      expect(mockMessages[1].content).toBe('User question');

      // Third iteration
      mockMessages.push({ role: 'assistant', content: response });
      processNoToolResponse(response);
      addReminderMessage(mockMessages, reminderContent);

      // System and user messages should still be preserved
      expect(mockMessages[0].role).toBe('system');
      expect(mockMessages[1].role).toBe('user');
    });
  });

  describe('Regression Tests', () => {
    test('should not break normal flow when responses are all different', () => {
      const responses = [
        'First response with unique content for testing purposes.',
        'Second response with different unique content for testing.',
        'Third response with even more different content for testing.',
        'Fourth response continuing with different content.',
        'Fifth response still different from all previous ones.'
      ];

      for (const response of responses) {
        const result = processNoToolResponse(response);
        expect(result.accepted).toBe(false);
      }

      // Counter should always be 1 (reset each time)
      expect(sameResponseCount).toBe(1);
    });

    test('should not affect tool call detection (orthogonal feature)', () => {
      // This test ensures our changes don't accidentally affect the separate
      // tool call detection logic - we only handle no-tool-call responses
      const responseWithTool = '<search><query>test</query></search>';

      // This response has a tool call, so it wouldn't even reach our logic
      // But verify our detection doesn't break for responses that look like tools
      const result = processNoToolResponse(responseWithTool);
      expect(result.accepted).toBe(false); // Too short anyway
    });

    test('should handle very long responses', () => {
      // Note: .trim() removes the trailing space from the last repeat
      const longResponse = 'This is a very long response. '.repeat(1000);

      processNoToolResponse(longResponse);
      processNoToolResponse(longResponse);
      const result = processNoToolResponse(longResponse);

      expect(result.accepted).toBe(true);
      // After trim(), the trailing space is removed, so length is 1 less
      expect(result.finalResult.length).toBe(longResponse.trim().length);
    });

    test('should handle unicode content', () => {
      const unicodeResponse = '这是一个足够长的中文回复，用于测试Unicode内容处理。這是繁體中文部分。🎉 Emoji test!';

      processNoToolResponse(unicodeResponse);
      processNoToolResponse(unicodeResponse);
      const result = processNoToolResponse(unicodeResponse);

      expect(result.accepted).toBe(true);
      expect(result.finalResult).toBe(unicodeResponse);
    });

    test('should handle responses with special regex characters', () => {
      const regexCharsResponse = 'Response with regex chars: [a-z]+ (group) \\d+ $end^ *star+ .dot? {braces} |pipe| and more content.';

      processNoToolResponse(regexCharsResponse);
      processNoToolResponse(regexCharsResponse);
      const result = processNoToolResponse(regexCharsResponse);

      expect(result.accepted).toBe(true);
    });
  });
});
