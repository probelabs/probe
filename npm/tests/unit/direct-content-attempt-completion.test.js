/**
 * Tests for direct content attempt_completion tool (no <result> wrapper)
 * Tests the simplified parsing that uses entire inner content as result
 */
import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { parseXmlToolCallWithThinking } from '../../src/agent/tools.js';

describe('Direct Content attempt_completion Parsing', () => {
  test('should parse simple attempt_completion with direct content', () => {
    const xml = `<attempt_completion>
The authentication system has been analyzed successfully. It uses secure JWT tokens.
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.toolName).toBe('attempt_completion');
    expect(parsed.params.result).toBe('The authentication system has been analyzed successfully. It uses secure JWT tokens.');
  });

  test('should handle multiline content with formatting', () => {
    const xml = `<attempt_completion>
Analysis Complete:

**Security Findings:**
- JWT tokens are properly signed
- Password hashing uses bcrypt
- Rate limiting is implemented

**Recommendations:**
1. Add 2FA support
2. Implement session timeouts
3. Add audit logging
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.toolName).toBe('attempt_completion');
    expect(parsed.params.result).toContain('Analysis Complete:');
    expect(parsed.params.result).toContain('**Security Findings:**');
    expect(parsed.params.result).toContain('**Recommendations:**');
  });

  test('should handle JSON content directly', () => {
    const xml = `<attempt_completion>
{
  "analysis": "The authentication system is secure",
  "findings": [
    "JWT tokens properly implemented",
    "Password hashing uses bcrypt",
    "Rate limiting active"
  ],
  "confidence": 0.95,
  "recommendations": [
    {"action": "Add 2FA", "priority": "high"},
    {"action": "Implement session timeouts", "priority": "medium"}
  ]
}
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.toolName).toBe('attempt_completion');
    expect(parsed.params.result).toContain('"analysis": "The authentication system is secure"');
    expect(parsed.params.result).toContain('"confidence": 0.95');
    expect(parsed.params.result).toContain('"recommendations"');

    // Should be valid JSON
    expect(() => JSON.parse(parsed.params.result.trim())).not.toThrow();
  });

  test('should handle code blocks in content', () => {
    const xml = `<attempt_completion>
I found the authentication function:

\`\`\`javascript
function authenticate(token) {
  return jwt.verify(token, secret);
}
\`\`\`

This function validates JWT tokens using the secret key.
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.toolName).toBe('attempt_completion');
    expect(parsed.params.result).toContain('```javascript');
    expect(parsed.params.result).toContain('function authenticate');
  });

  test('should handle XML-like content without confusion', () => {
    const xml = `<attempt_completion>
The config file contains: &lt;database&gt;&lt;host&gt;localhost&lt;/host&gt;&lt;/database&gt;
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.params.result).toContain('&lt;database&gt;&lt;host&gt;localhost&lt;/host&gt;&lt;/database&gt;');
  });

  test('should handle empty content', () => {
    const xml = `<attempt_completion></attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.params.result).toBe('');
  });

  test('should handle content with special characters', () => {
    const xml = `<attempt_completion>
Found 5 files with "special" characters: @#$%^&*()[]{}|\\:";'<>?,./
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.params.result).toContain('special');
    expect(parsed.params.result).toContain('@#$%^&*()[]{}|\\:');
  });

  test('should remove command parameter if parsed by generic logic', () => {
    // Test the legacy compatibility where command might be parsed but should be removed
    const xml = `<attempt_completion>
Task completed successfully.
<command>npm test</command>
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.toolName).toBe('attempt_completion');

    // The entire inner content should be the result (including the command tag if present)
    expect(parsed.params.result).toContain('Task completed successfully.');
    expect(parsed.params.result).toContain('<command>npm test</command>');
  });

  test('should preserve whitespace and formatting', () => {
    const xml = `<attempt_completion>
    Indented analysis:
        - Finding 1
        - Finding 2

    Conclusion: System is secure.
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.params.result).toContain('Indented analysis:');
    expect(parsed.params.result).toContain('        - Finding 1');
    expect(parsed.params.result).toContain('Conclusion: System is secure.');
  });
});

describe('Direct Content vs Legacy <result> Tag', () => {
  test('should NOT parse <result> tags anymore', () => {
    // This test ensures we no longer support the <result> wrapper
    const xml = `<attempt_completion>
<result>This should be treated as part of the content, not as a parameter</result>
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.toolName).toBe('attempt_completion');

    // The <result> tags should be part of the content, not parsed as parameters
    expect(parsed.params.result).toContain('<result>This should be treated as part of the content, not as a parameter</result>');
  });

  test('should treat everything inside attempt_completion as direct content', () => {
    const xml = `<attempt_completion>
<title>Security Analysis Report</title>
<summary>System is secure</summary>
<details>
  <finding>JWT implementation correct</finding>
  <finding>Password hashing proper</finding>
</details>
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.params.result).toContain('<title>Security Analysis Report</title>');
    expect(parsed.params.result).toContain('<summary>System is secure</summary>');
    expect(parsed.params.result).toContain('<finding>JWT implementation correct</finding>');
  });
});

describe('Integration with Schema Response Flow', () => {
  test('should handle JSON schema responses cleanly', () => {
    const jsonResponse = `{
  "status": "complete",
  "analysis": "Authentication system analysis finished",
  "findings": ["Secure JWT implementation", "Proper password hashing"],
  "score": 95
}`;

    const xml = `<attempt_completion>${jsonResponse}</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.params.result.trim()).toBe(jsonResponse);

    // Should be valid JSON that can be parsed
    const jsonData = JSON.parse(parsed.params.result.trim());
    expect(jsonData.status).toBe('complete');
    expect(jsonData.score).toBe(95);
    expect(Array.isArray(jsonData.findings)).toBe(true);
  });

  test('should handle non-JSON responses for non-JSON schemas', () => {
    const mermaidResponse = `graph TD
    A[Authentication] --> B[JWT Validation]
    B --> C[User Access]
    C --> D[Protected Resource]`;

    const xml = `<attempt_completion>${mermaidResponse}</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.params.result.trim()).toBe(mermaidResponse);
    expect(parsed.params.result).toContain('graph TD');
    expect(parsed.params.result).toContain('Authentication');
  });
});