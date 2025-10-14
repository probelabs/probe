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

    // New truncation behavior: Find FIRST schema message (index 11) and truncate from there
    // Messages before index 11:
    // 0: system, 1: user, 2: assistant, 3: user (tool reminder - REMOVED), 4: assistant,
    // 5: user (tool result), 6: assistant, 7: user (mermaid fix - REMOVED), 8: assistant,
    // 9: user, 10: assistant
    // 11: FIRST SCHEMA MESSAGE → truncate from here
    // After truncation and removing non-schema internal messages (tool reminder at 3, mermaid at 7):
    // System + 2 real user messages + 1 tool result + 5 assistant responses = 9 total
    expect(clonedAgent.history).toHaveLength(9);
    expect(clonedSystemMessages).toBe(1);
    expect(clonedUserMessages).toBe(3); // 2 real questions + 1 tool result
    expect(clonedAssistantMessages).toBe(5); // Assistant responses before schema

    // Verify system message is preserved
    expect(clonedAgent.history[0].role).toBe('system');
    expect(clonedAgent.history[0].content).toContain('helpful AI coding assistant');

    // Verify real user questions are kept (before schema truncation)
    const userContents = clonedAgent.history
      .filter(m => m.role === 'user')
      .map(m => m.content);

    expect(userContents).toContain('Analyze the authentication system and create a diagram');
    expect(userContents.some(c => c.includes('Found 15 results'))).toBe(true); // Tool result
    expect(userContents).toContain('Now provide a security analysis in JSON format');

    // Verify internal messages are NOT in clone
    const allContent = clonedAgent.history.map(m => m.content).join('\n');

    expect(allContent).not.toContain('IMPORTANT: A schema was provided');
    expect(allContent).not.toContain('Please use one of the available tools');
    expect(allContent).not.toContain('The mermaid diagram in your response has syntax errors');
    expect(allContent).not.toContain('Your response does not match the expected JSON schema');
    expect(allContent).not.toContain('When using <attempt_complete>, this must be the ONLY content');

    // Verify assistant responses before schema are kept
    expect(allContent).toContain('I\'ll analyze the authentication system');
    expect(allContent).toContain('corrected diagram');
    // These are AFTER the schema message, so should NOT be in clone:
    expect(allContent).not.toContain('vulnerabilities'); // After schema at index 12
    expect(allContent).not.toContain('authorization permissions'); // After schema at index 16

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

    // Truncates at schema message (index 3)
    // Keeps: 0 (system), 1 (user with image), 2 (assistant)
    // Removes: 3 (schema), 4 (user), 5 (assistant)
    expect(clonedAgent.history).toHaveLength(3);

    // Verify complex content structures are preserved
    const userWithImage = clonedAgent.history.find(m =>
      Array.isArray(m.content) && m.content.some(c => c.type === 'image')
    );
    expect(userWithImage).toBeDefined();
    expect(userWithImage.content).toHaveLength(2);
    expect(userWithImage.content[1].type).toBe('image');

    // Verify internal message was removed (via truncation)
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

    // Truncates at first schema message (index 3)
    // Original: 9 messages (0-8)
    // Keeps: 0 (system), 1 (user), 2 (assistant)
    // Removes: 3 (schema), 4 (assistant), 5 (schema), 6 (assistant), 7 (schema), 8 (assistant)
    expect(baseAgent.history).toHaveLength(9);
    expect(clonedAgent.history).toHaveLength(3);

    expect(clonedAgent.history.filter(m => m.role === 'system')).toHaveLength(1);
    expect(clonedAgent.history.filter(m => m.role === 'user')).toHaveLength(1); // Only the real user question
    expect(clonedAgent.history.filter(m => m.role === 'assistant')).toHaveLength(1); // Only first attempt before schema

    // Verify no schema reminders remain
    const allContent = clonedAgent.history.map(m => m.content).join('\n');
    expect(allContent).not.toContain('IMPORTANT: A schema was provided');
    expect(allContent).not.toContain('does not match the expected JSON schema');
    expect(allContent).not.toContain('{"incomplete": true}'); // After schema
    expect(allContent).not.toContain('{"status": "complete"'); // After schema

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

    // Truncates at schema message (index 3)
    // Keeps: 0 (system), 1 (user), 2 (assistant with null)
    // Removes: 3 (schema), 4 (assistant), 5 (user), 6 (assistant)
    expect(clonedAgent.history).toHaveLength(3);

    // Verify schema reminder was removed (via truncation)
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
      // Original: 11 messages (0-10)
      // Truncates at schema (index 9), keeps 0-8
      // Then filters tool reminder at index 3
      // Result: 8 messages (1 system + 2 user + 5 assistant)
      expect(clone.history).toHaveLength(8);

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

  test('should strip CRITICAL schema formatting messages (recursive answer call)', () => {
    // Simulate a realistic scenario where schema causes a recursive answer() call
    // with CRITICAL formatting prompt
    baseAgent.history = [
      { role: 'system', content: 'You are a helpful AI assistant' },
      { role: 'user', content: 'Provide an overview of the codebase' },
      { role: 'assistant', content: 'Here is my analysis of the codebase...' },
      // First schema reminder (should be stripped)
      {
        role: 'user',
        content: 'IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.\nUse attempt_completion with your response directly inside the tags:\n\n<attempt_completion>\n[Your response content matching the provided schema format]\n</attempt_completion>\n\nYour response must conform to this schema:\n{"type": "object", "properties": {"overview": {"type": "string"}}}'
      },
      { role: 'assistant', content: '<attempt_completion>\nOverview content\n</attempt_completion>' },
      // CRITICAL schema formatting prompt from recursive answer() call (should be stripped)
      {
        role: 'user',
        content: 'CRITICAL: You MUST respond with ONLY valid JSON DATA that conforms to this schema structure. DO NOT return the schema definition itself.\n\nSchema to follow (this is just the structure - provide ACTUAL DATA):\n{"type": "object", "properties": {"overview": {"type": "string"}, "summary": {"type": "string"}}}\n\nREQUIREMENTS:\n- Return ONLY the JSON object/array with REAL DATA that matches the schema structure\n- DO NOT return the schema definition itself (no "$schema", "$id", "type", "properties", etc.)\n- NO additional text, explanations, or markdown formatting'
      },
      { role: 'assistant', content: '{"overview": "The codebase analysis", "summary": "Summary text"}' }
    ];

    const clonedAgent = baseAgent.clone({
      stripInternalMessages: true
    });

    // Truncation behavior: First schema message at index 3 (IMPORTANT), truncate from there
    // Original: 7 messages (0: system, 1: user, 2: assistant, 3: IMPORTANT, 4: assistant, 5: CRITICAL, 6: assistant)
    // Cloned: 3 messages (0: system, 1: user, 2: assistant) - everything from index 3 removed
    expect(baseAgent.history).toHaveLength(7);
    expect(clonedAgent.history).toHaveLength(3);

    expect(clonedAgent.history.filter(m => m.role === 'system')).toHaveLength(1);
    expect(clonedAgent.history.filter(m => m.role === 'user')).toHaveLength(1); // Only real question
    expect(clonedAgent.history.filter(m => m.role === 'assistant')).toHaveLength(1); // Only first assistant response

    // Verify internal messages are NOT in clone
    const allContent = clonedAgent.history.map(m => m.content).join('\n');
    expect(allContent).not.toContain('IMPORTANT: A schema was provided');
    expect(allContent).not.toContain('CRITICAL: You MUST respond with ONLY valid JSON DATA');
    expect(allContent).not.toContain('Schema to follow (this is just the structure');
    expect(allContent).not.toContain('DO NOT return the schema definition itself');
    expect(allContent).not.toContain('Overview content'); // This was after schema, so removed
    expect(allContent).not.toContain('"overview": "The codebase analysis"'); // This was after schema, so removed

    // Verify content before schema is kept
    expect(allContent).toContain('Provide an overview of the codebase');
    expect(allContent).toContain('Here is my analysis');
  });

  test('should handle visor-style session cloning scenario (overview → code-review)', () => {
    // Simulate the exact scenario from visor where:
    // 1. Overview check runs with schema="overview"
    // 2. Session is cloned for code-review with schema="code-review"
    // 3. The CRITICAL schema message from overview should NOT leak into code-review
    baseAgent.history = [
      { role: 'system', content: 'You are a code review assistant' },

      // Overview check conversation
      { role: 'user', content: 'Analyze this PR and provide an overview' },
      { role: 'assistant', content: '<search><query>PR changes</query></search>' },
      { role: 'user', content: '<tool_result>\nFound 5 files changed\n</tool_result>' },
      { role: 'assistant', content: 'Based on the changes, here is my overview...' },

      // Schema reminder for overview (should be stripped)
      {
        role: 'user',
        content: 'IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.\nYour response must conform to this schema:\n{"$id": "overview", "type": "object", "properties": {"summary": {"type": "string"}}}'
      },
      { role: 'assistant', content: '<attempt_completion>\n{"summary": "Overview of changes"}\n</attempt_completion>' },

      // CRITICAL schema formatting for overview (should be stripped)
      {
        role: 'user',
        content: 'CRITICAL: You MUST respond with ONLY valid JSON DATA that conforms to this schema structure. DO NOT return the schema definition itself.\n\nSchema to follow (this is just the structure - provide ACTUAL DATA):\n{"$id": "overview", "type": "object", "properties": {"summary": {"type": "string"}}}\n\nREQUIREMENTS:\n- Return ONLY the JSON object/array with REAL DATA'
      },
      { role: 'assistant', content: '{"summary": "Overview of changes"}' }
    ];

    // Clone for code-review (simulating visor's session cloning)
    const codeReviewClone = baseAgent.clone({
      sessionId: 'code-review-session',
      stripInternalMessages: true,
      keepSystemMessage: true,
      deepCopy: true
    });

    // Truncation at first schema message (index 5)
    // Original: 9 messages (0-8)
    // Cloned: 5 messages (0-4) - truncated at index 5 where IMPORTANT appears
    expect(codeReviewClone.history).toHaveLength(5);
    expect(codeReviewClone.sessionId).toBe('code-review-session');

    // Verify NO schema messages remain
    const cloneContent = codeReviewClone.history.map(m => m.content).join('\n');
    expect(cloneContent).not.toContain('IMPORTANT: A schema was provided');
    expect(cloneContent).not.toContain('CRITICAL: You MUST respond');
    expect(cloneContent).not.toContain('Schema to follow');
    expect(cloneContent).not.toContain('"$id": "overview"');
    expect(cloneContent).not.toContain('attempt_completion'); // This was after schema

    // Verify real conversation before schema is kept
    expect(cloneContent).toContain('Analyze this PR');
    expect(cloneContent).toContain('Found 5 files changed');
    expect(cloneContent).toContain('here is my overview');

    // Verify assistant's responses before schema are kept
    const assistantMessages = codeReviewClone.history.filter(m => m.role === 'assistant');
    expect(assistantMessages).toHaveLength(2); // Only 2 assistant messages before schema
    expect(assistantMessages[0].content).toContain('<search>');
    expect(assistantMessages[1].content).toContain('here is my overview');
  });

  test('should strip multiple CRITICAL messages from multiple schema iterations', () => {
    // Test scenario where schema validation fails multiple times,
    // generating multiple CRITICAL messages
    baseAgent.history = [
      { role: 'system', content: 'System' },
      { role: 'user', content: 'Generate report' },
      { role: 'assistant', content: 'Report content' },

      // First schema attempt
      {
        role: 'user',
        content: 'IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.'
      },
      { role: 'assistant', content: '<attempt_completion>\nReport\n</attempt_completion>' },

      // First CRITICAL formatting
      {
        role: 'user',
        content: 'CRITICAL: You MUST respond with ONLY valid JSON DATA that conforms to this schema structure.\n\nSchema to follow (this is just the structure - provide ACTUAL DATA):\n{"type": "object"}'
      },
      { role: 'assistant', content: '{"invalid": true}' },

      // JSON correction
      {
        role: 'user',
        content: 'Your response does not match the expected JSON schema. Please provide a valid JSON response.'
      },

      // Second CRITICAL formatting
      {
        role: 'user',
        content: 'CRITICAL: You MUST respond with ONLY valid JSON DATA that conforms to this schema structure.\n\nSchema to follow (this is just the structure - provide ACTUAL DATA):\n{"type": "object", "properties": {"report": {"type": "string"}}}'
      },
      { role: 'assistant', content: '{"report": "Final report"}' }
    ];

    const cloned = baseAgent.clone();

    // Truncation at first schema message (index 3 - IMPORTANT)
    // Original: 10 messages (0-9)
    // Cloned: 3 messages (0-2) - truncated at index 3
    expect(baseAgent.history).toHaveLength(10);
    expect(cloned.history).toHaveLength(3); // system + user + assistant (before schema)

    const content = cloned.history.map(m => m.content).join('\n');
    expect(content).not.toContain('IMPORTANT: A schema was provided');
    expect(content).not.toContain('CRITICAL: You MUST respond');
    expect(content).not.toContain('Schema to follow');
    expect(content).not.toContain('does not match the expected JSON schema');
    expect(content).not.toContain('attempt_completion'); // After schema
    expect(content).not.toContain('{"invalid": true}'); // After schema
    expect(content).not.toContain('Final report'); // After schema

    // Only content before schema remains
    expect(content).toContain('Generate report');
    expect(content).toContain('Report content');
  });
});
