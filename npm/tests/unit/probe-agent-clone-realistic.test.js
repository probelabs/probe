import { describe, test, expect, beforeEach } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

/**
 * Realistic integration test for clone() method
 * Simulates a real conversation with schema, mermaid fixes, tool reminders, etc.
 */
describe('ProbeAgent.clone() - Realistic Integration', () => {
  let baseAgent;

  beforeEach(() => {
    baseAgent = new ProbeAgent({
      sessionId: 'realistic-test',
      path: process.cwd(),
      debug: false
    });
  });

  test('should clean a realistic session with all types of internal messages', () => {
    // Simulate a realistic conversation with schema, mermaid, and multiple reminders
    baseAgent.history = [
      // 1. System message (should be kept)
      {
        role: 'system',
        content: 'You are a helpful AI coding assistant with access to code search tools.'
      },

      // 2. User's first question
      {
        role: 'user',
        content: 'Analyze the authentication system and create a diagram'
      },

      // 3. Assistant starts working
      {
        role: 'assistant',
        content: 'I\'ll analyze the authentication system for you.\n\n<search>\n<query>authentication login</query>\n</search>'
      },

      // 4. Tool reminder (should be stripped)
      {
        role: 'user',
        content: 'Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information.\n\nRemember: Use proper XML format with BOTH opening and closing tags:\n\n<tool_name>\n<parameter>value</parameter>\n</tool_name>'
      },

      // 5. Assistant uses tool
      {
        role: 'assistant',
        content: '<search>\n<query>auth</query>\n</search>'
      },

      // 6. User provides tool result
      {
        role: 'user',
        content: 'Found 15 results for "auth":\n\n1. src/auth/login.js - Login handler\n2. src/auth/token.js - JWT token management'
      },

      // 7. Assistant creates a mermaid diagram (with errors)
      {
        role: 'assistant',
        content: 'Here\'s the authentication flow:\n\n```mermaid\ngraph TD\nA -> B\nC -> D\n```'
      },

      // 8. Mermaid fix prompt (should be stripped)
      {
        role: 'user',
        content: 'The mermaid diagram in your response has syntax errors. Please fix the mermaid syntax errors.\n\nHere is the corrected version:\n```mermaid\ngraph TD\nA --> B\nC --> D\n```'
      },

      // 9. Assistant fixes mermaid
      {
        role: 'assistant',
        content: 'Thank you, here\'s the corrected diagram:\n\n```mermaid\ngraph TD\nA --> B\nC --> D\n```'
      },

      // 10. User asks for structured output
      {
        role: 'user',
        content: 'Now provide a security analysis in JSON format'
      },

      // 11. Assistant responds (not matching schema)
      {
        role: 'assistant',
        content: 'Here is my analysis: The authentication system looks good.'
      },

      // 12. Schema reminder (should be stripped)
      {
        role: 'user',
        content: 'IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.\n\nYour response must conform to this schema:\n{\n  "type": "object",\n  "properties": {\n    "vulnerabilities": { "type": "array" },\n    "recommendations": { "type": "array" }\n  }\n}'
      },

      // 13. Assistant provides structured response
      {
        role: 'assistant',
        content: '{\n  "vulnerabilities": ["No password hashing detected"],\n  "recommendations": ["Implement bcrypt for password hashing"]\n}'
      },

      // 14. Another tool reminder (should be stripped)
      {
        role: 'user',
        content: 'Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information.\n\nRemember: Use proper XML format with BOTH opening and closing tags:\n\n<tool_name>\n<parameter>value</parameter>\n</tool_name>\n\nOr for quick completion if your previous response was already correct and complete:\n<attempt_complete>\n\nIMPORTANT: When using <attempt_complete>, this must be the ONLY content in your response.'
      },

      // 15. Assistant completes
      {
        role: 'assistant',
        content: '<attempt_completion>\nI\'ve completed the security analysis of the authentication system.\n</attempt_completion>'
      },

      // 16. User asks another question
      {
        role: 'user',
        content: 'What about the authorization system?'
      },

      // 17. Assistant responds
      {
        role: 'assistant',
        content: 'Let me search for authorization code.\n\n<search>\n<query>authorization permissions</query>\n</search>'
      },

      // 18. JSON correction prompt (should be stripped)
      {
        role: 'user',
        content: 'Your response does not match the expected JSON schema. Please provide a valid JSON response.\n\nSchema validation error: Expected object with "status" field'
      },

      // 19. Assistant fixes JSON
      {
        role: 'assistant',
        content: '{\n  "status": "analyzing",\n  "findings": []\n}'
      }
    ];

    // Clone with default settings (strip internal messages)
    const clonedAgent = baseAgent.clone({
      stripInternalMessages: true
    });

    // Verify the clone has cleaned history
    expect(clonedAgent.history.length).toBeLessThan(baseAgent.history.length);

    // Count message types in original
    const originalUserMessages = baseAgent.history.filter(m => m.role === 'user').length;
    const originalAssistantMessages = baseAgent.history.filter(m => m.role === 'assistant').length;
    const originalSystemMessages = baseAgent.history.filter(m => m.role === 'system').length;

    console.log(`\n📊 Original history: ${baseAgent.history.length} messages`);
    console.log(`   - System: ${originalSystemMessages}`);
    console.log(`   - User: ${originalUserMessages}`);
    console.log(`   - Assistant: ${originalAssistantMessages}`);

    // Count message types in clone
    const clonedUserMessages = clonedAgent.history.filter(m => m.role === 'user').length;
    const clonedAssistantMessages = clonedAgent.history.filter(m => m.role === 'assistant').length;
    const clonedSystemMessages = clonedAgent.history.filter(m => m.role === 'system').length;

    console.log(`\n📊 Cloned history: ${clonedAgent.history.length} messages`);
    console.log(`   - System: ${clonedSystemMessages}`);
    console.log(`   - User: ${clonedUserMessages}`);
    console.log(`   - Assistant: ${clonedAssistantMessages}`);

    // Verify expectations
    // Original: 19 messages (1 system, 9 user, 9 assistant)
    expect(baseAgent.history).toHaveLength(19);
    expect(originalSystemMessages).toBe(1);
    expect(originalUserMessages).toBe(9);
    expect(originalAssistantMessages).toBe(9);

    // Cloned should remove 4 internal user messages:
    // - 2 tool reminders
    // - 1 mermaid fix prompt
    // - 1 schema reminder
    // - 1 JSON correction prompt
    // Total: 19 - 5 = 14 messages (1 system, 4 user, 9 assistant)
    expect(clonedAgent.history).toHaveLength(14);
    expect(clonedSystemMessages).toBe(1);
    expect(clonedUserMessages).toBe(4); // Only real user questions
    expect(clonedAssistantMessages).toBe(9); // All assistant responses kept

    // Verify system message is preserved
    expect(clonedAgent.history[0].role).toBe('system');
    expect(clonedAgent.history[0].content).toContain('helpful AI coding assistant');

    // Verify real user questions are kept
    const userContents = clonedAgent.history
      .filter(m => m.role === 'user')
      .map(m => m.content);

    expect(userContents).toContain('Analyze the authentication system and create a diagram');
    expect(userContents[1]).toContain('Found 15 results'); // Tool result
    expect(userContents[2]).toBe('Now provide a security analysis in JSON format');
    expect(userContents[3]).toBe('What about the authorization system?');

    // Verify internal messages are NOT in clone
    const allContent = clonedAgent.history.map(m => m.content).join('\n');

    expect(allContent).not.toContain('IMPORTANT: A schema was provided');
    expect(allContent).not.toContain('Please use one of the available tools');
    expect(allContent).not.toContain('The mermaid diagram in your response has syntax errors');
    expect(allContent).not.toContain('Your response does not match the expected JSON schema');
    expect(allContent).not.toContain('When using <attempt_complete>, this must be the ONLY content');

    // Verify assistant responses are kept
    expect(allContent).toContain('I\'ll analyze the authentication system');
    expect(allContent).toContain('corrected diagram');
    expect(allContent).toContain('vulnerabilities');
    expect(allContent).toContain('authorization permissions');

    console.log(`\n✅ Successfully removed ${baseAgent.history.length - clonedAgent.history.length} internal messages`);
    console.log('✅ All meaningful conversation content preserved');
  });

  test('should preserve all messages when stripInternalMessages is false', () => {
    // Same realistic history
    baseAgent.history = [
      { role: 'system', content: 'System' },
      { role: 'user', content: 'Real question' },
      { role: 'assistant', content: 'Answer' },
      { role: 'user', content: 'IMPORTANT: A schema was provided. You MUST respond...' },
      { role: 'assistant', content: 'Schema response' },
      { role: 'user', content: 'Please use one of the available tools...' },
      { role: 'assistant', content: 'Tool call' }
    ];

    const clonedAgent = baseAgent.clone({
      stripInternalMessages: false
    });

    // Should keep ALL messages including internal ones
    expect(clonedAgent.history).toHaveLength(baseAgent.history.length);
    expect(clonedAgent.history).toHaveLength(7);

    const allContent = clonedAgent.history.map(m => m.content).join('\n');
    expect(allContent).toContain('IMPORTANT: A schema was provided');
    expect(allContent).toContain('Please use one of the available tools');
  });

  test('should handle complex content structures in realistic scenario', () => {
    baseAgent.history = [
      { role: 'system', content: 'System' },
      // User message with images
      {
        role: 'user',
        content: [
          { type: 'text', text: 'Analyze this screenshot' },
          { type: 'image', image: 'data:image/png;base64,iVBORw0KGg...' }
        ]
      },
      { role: 'assistant', content: 'I can see the screenshot shows...' },
      // Internal reminder (should be stripped)
      {
        role: 'user',
        content: 'IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.'
      },
      // User message with complex content (should be kept)
      {
        role: 'user',
        content: [
          { type: 'text', text: 'Provide analysis in JSON' }
        ]
      },
      { role: 'assistant', content: '{"analysis": "complete"}' }
    ];

    const clonedAgent = baseAgent.clone();

    // Should have 5 messages (removed 1 internal)
    expect(clonedAgent.history).toHaveLength(5);

    // Verify complex content structures are preserved
    const userWithImage = clonedAgent.history.find(m =>
      Array.isArray(m.content) && m.content.some(c => c.type === 'image')
    );
    expect(userWithImage).toBeDefined();
    expect(userWithImage.content).toHaveLength(2);
    expect(userWithImage.content[1].type).toBe('image');

    // Verify internal message was removed
    const schemaReminder = clonedAgent.history.find(m =>
      typeof m.content === 'string' && m.content.includes('IMPORTANT: A schema was provided')
    );
    expect(schemaReminder).toBeUndefined();
  });

  test('should handle session with multiple schema attempts', () => {
    // Realistic scenario: AI fails schema validation multiple times
    baseAgent.history = [
      { role: 'system', content: 'System' },
      { role: 'user', content: 'Generate a report' },
      { role: 'assistant', content: 'Here is my report' },
      // First schema reminder
      {
        role: 'user',
        content: 'IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.\nYour response must conform to this schema: {...}'
      },
      { role: 'assistant', content: '{"incomplete": true}' },
      // Second schema reminder (JSON correction)
      {
        role: 'user',
        content: 'Your response does not match the expected JSON schema. Please provide a valid JSON response.\n\nSchema validation error: Missing required field "status"'
      },
      { role: 'assistant', content: '{"status": "complete", "data": {}}' },
      // Third schema reminder
      {
        role: 'user',
        content: 'IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.'
      },
      { role: 'assistant', content: '{"status": "complete", "data": {}, "metadata": {}}' }
    ];

    const clonedAgent = baseAgent.clone();

    // Should remove all 3 schema reminders
    // Original: 9 messages (1 system, 4 user [1 real + 3 internal], 4 assistant)
    // Cloned: 6 messages (1 system, 1 user, 4 assistant) - removed 3 internal
    expect(baseAgent.history).toHaveLength(9);
    expect(clonedAgent.history).toHaveLength(6);

    expect(clonedAgent.history.filter(m => m.role === 'system')).toHaveLength(1);
    expect(clonedAgent.history.filter(m => m.role === 'user')).toHaveLength(1); // Only the real user question
    expect(clonedAgent.history.filter(m => m.role === 'assistant')).toHaveLength(4); // All 4 attempts

    // Verify no schema reminders remain
    const allContent = clonedAgent.history.map(m => m.content).join('\n');
    expect(allContent).not.toContain('IMPORTANT: A schema was provided');
    expect(allContent).not.toContain('does not match the expected JSON schema');

    // Verify the real user message is kept
    const userMessages = clonedAgent.history.filter(m => m.role === 'user');
    expect(userMessages[0].content).toBe('Generate a report');
  });

  test('should handle session with multiple mermaid fix attempts', () => {
    // Realistic scenario: Multiple mermaid syntax errors
    baseAgent.history = [
      { role: 'system', content: 'System' },
      { role: 'user', content: 'Create a flowchart' },
      { role: 'assistant', content: '```mermaid\ngraph TD\nA -> B\n```' },
      // First mermaid fix
      {
        role: 'user',
        content: 'The mermaid diagram in your response has syntax errors. Please fix the mermaid syntax errors.\n\nHere is the corrected version:\n```mermaid\ngraph TD\nA --> B\n```'
      },
      { role: 'assistant', content: '```mermaid\ngraph TD\nA --> B\nC -> D\n```' },
      // Second mermaid fix
      {
        role: 'user',
        content: 'The mermaid diagram in your response has syntax errors. Please fix the mermaid syntax errors.'
      },
      { role: 'assistant', content: '```mermaid\ngraph TD\nA --> B\nC --> D\n```' }
    ];

    const clonedAgent = baseAgent.clone();

    // Should remove both mermaid fix prompts
    expect(baseAgent.history).toHaveLength(7);
    expect(clonedAgent.history).toHaveLength(5);

    // Verify no mermaid fix prompts remain
    const allContent = clonedAgent.history.map(m => m.content).join('\n');
    expect(allContent).not.toContain('mermaid diagram in your response has syntax errors');
  });

  test('should handle empty and null content in realistic scenario', () => {
    baseAgent.history = [
      { role: 'system', content: 'System' },
      { role: 'user', content: 'Question' },
      { role: 'assistant', content: null }, // Edge case
      { role: 'user', content: 'IMPORTANT: A schema was provided...' }, // Should be stripped
      { role: 'assistant', content: '' }, // Edge case
      { role: 'user', content: undefined }, // Edge case
      { role: 'assistant', content: 'Final answer' }
    ];

    const clonedAgent = baseAgent.clone();

    // Should remove the schema reminder but keep edge cases
    expect(clonedAgent.history).toHaveLength(6);

    // Verify schema reminder was removed
    const schemaReminder = clonedAgent.history.find(m =>
      m.content && m.content.includes('IMPORTANT: A schema was provided')
    );
    expect(schemaReminder).toBeUndefined();
  });

  test('should correctly identify partial matches vs full internal messages', () => {
    baseAgent.history = [
      { role: 'system', content: 'System' },
      // Actual user question that happens to contain similar words (should be kept)
      { role: 'user', content: 'Can you use the available tools to search?' },
      { role: 'assistant', content: 'Yes, I will' },
      // Real internal message (should be stripped)
      {
        role: 'user',
        content: 'Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information.\n\nRemember: Use proper XML format with BOTH opening and closing tags:'
      },
      { role: 'assistant', content: '<search><query>test</query></search>' }
    ];

    const clonedAgent = baseAgent.clone();

    // Should keep user question but strip internal reminder
    expect(clonedAgent.history).toHaveLength(4);

    const userMessages = clonedAgent.history.filter(m => m.role === 'user');
    expect(userMessages).toHaveLength(1);
    expect(userMessages[0].content).toBe('Can you use the available tools to search?');
  });

  test('should provide clean context for parallel task execution', () => {
    // Simulate building context with one agent, then cloning for parallel tasks
    baseAgent.history = [
      { role: 'system', content: 'You are a code reviewer' },
      { role: 'user', content: 'Review the codebase' },
      { role: 'assistant', content: '<search><query>code review</query></search>' },
      // Full tool reminder (internal - should be stripped)
      {
        role: 'user',
        content: 'Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information.\n\nRemember: Use proper XML format with BOTH opening and closing tags:'
      },
      { role: 'assistant', content: '<search><query>code</query></search>' },
      { role: 'user', content: 'Found 100 files' },
      { role: 'assistant', content: 'Analyzed the codebase structure' },
      { role: 'user', content: 'Provide security analysis' },
      { role: 'assistant', content: 'Security looks good' },
      // Schema reminder (internal - should be stripped)
      { role: 'user', content: 'IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.' },
      { role: 'assistant', content: '{"security": "good"}' }
    ];

    // Clone for different parallel tasks
    const securityClone = baseAgent.clone({ sessionId: 'security-check' });
    const performanceClone = baseAgent.clone({ sessionId: 'performance-check' });
    const styleClone = baseAgent.clone({ sessionId: 'style-check' });

    // All clones should have clean history
    [securityClone, performanceClone, styleClone].forEach(clone => {
      // Original has 11 messages (1 system, 5 user [3 real + 2 internal], 5 assistant)
      // Cloned should have 9 (1 system, 3 user, 5 assistant) - removed 2 internal
      expect(clone.history).toHaveLength(9);

      // Should not contain internal messages
      const content = clone.history.map(m => m.content).join('\n');
      expect(content).not.toContain('Please use one of the available tools to help answer');
      expect(content).not.toContain('IMPORTANT: A schema was provided');

      // Should contain real conversation
      expect(content).toContain('Review the codebase');
      expect(content).toContain('Found 100 files');
      expect(content).toContain('Analyzed the codebase structure');
    });

    // Each clone should have unique session ID
    expect(securityClone.sessionId).toBe('security-check');
    expect(performanceClone.sessionId).toBe('performance-check');
    expect(styleClone.sessionId).toBe('style-check');
  });
});
