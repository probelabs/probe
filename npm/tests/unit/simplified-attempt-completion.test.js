/**
 * Tests for simplified attempt_completion tool - without command parameter and JSON issues
 */
import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { attemptCompletionSchema } from '../../src/tools/common.js';
import { parseXmlToolCallWithThinking } from '../../src/agent/tools.js';

describe('Simplified attempt_completion Schema', () => {
  test('should accept valid plain text result', () => {
    const params = {
      result: 'I have successfully analyzed the authentication system. It uses JWT tokens with RS256 encryption.'
    };
    
    const validation = attemptCompletionSchema.safeParse(params);
    expect(validation.success).toBe(true);
    expect(validation.data.result).toBe(params.result);
  });

  test('should accept multiline text result', () => {
    const params = {
      result: `Analysis complete:

1. The authentication system uses JWT tokens
2. Password hashing is implemented with bcrypt
3. Rate limiting is in place for login attempts

Security recommendations:
- Consider adding 2FA
- Implement session timeout`
    };
    
    const validation = attemptCompletionSchema.safeParse(params);
    expect(validation.success).toBe(true);
    expect(validation.data.result).toBe(params.result);
  });

  test('should accept empty result', () => {
    const params = {
      result: ''
    };
    
    const validation = attemptCompletionSchema.safeParse(params);
    expect(validation.success).toBe(true);
  });

  test('should reject missing result parameter', () => {
    const params = {};
    
    const validation = attemptCompletionSchema.safeParse(params);
    expect(validation.success).toBe(false);
    expect(validation.error.issues[0].code).toBe('invalid_type');
  });

  test('should reject non-string result', () => {
    const params = {
      result: 123
    };
    
    const validation = attemptCompletionSchema.safeParse(params);
    expect(validation.success).toBe(false);
  });

  test('should not have command parameter in schema', () => {
    // Test that the command parameter is no longer accepted
    const params = {
      result: 'Test result',
      command: 'echo "test"'
    };
    
    const validation = attemptCompletionSchema.safeParse(params);
    expect(validation.success).toBe(true);
    
    // Command should be ignored (not present in validated data)
    expect(validation.data).toEqual({ result: 'Test result' });
    expect(validation.data.command).toBeUndefined();
  });
});

describe('Simplified attempt_completion XML Parsing', () => {
  test('should parse simple attempt_completion with direct content (no result wrapper)', () => {
    const xml = `<attempt_completion>
The authentication system has been analyzed successfully. It uses secure JWT tokens.
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.toolName).toBe('attempt_completion');
    expect(parsed.params.result).toBe('The authentication system has been analyzed successfully. It uses secure JWT tokens.');
  });

  test('should parse multiline result with formatting', () => {
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

  test('should handle result with code blocks', () => {
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

  test('should handle result with XML-like content', () => {
    const xml = `<attempt_completion>
The config file contains: &lt;database&gt;&lt;host&gt;localhost&lt;/host&gt;&lt;/database&gt;
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    // XML entities should remain as-is in the result (not decoded)
    expect(parsed.params.result).toContain('&lt;database&gt;&lt;host&gt;localhost&lt;/host&gt;&lt;/database&gt;');
  });

  test('should treat all content as direct content (including old-style tags)', () => {
    // Test that everything inside attempt_completion is treated as content
    const xml = `<attempt_completion>
<result>Analysis complete. The system is secure.</result>
<command>echo "test"</command>
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xml);

    expect(parsed).toBeDefined();
    expect(parsed.toolName).toBe('attempt_completion');
    expect(parsed.params.result).toContain('Analysis complete. The system is secure.');
    expect(parsed.params.result).toContain('<result>');
    expect(parsed.params.result).toContain('<command>echo "test"</command>');
    expect(parsed.params.command).toBeUndefined(); // Command should not be a separate parameter
  });

  test('should handle empty content', () => {
    const xml = `<attempt_completion>
</attempt_completion>`;

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
});

describe('Integration with ProbeAgent (Mocked)', () => {
  test('should handle valid attempt_completion without JSON validation', () => {
    // Mock the schema validation that would happen in ProbeAgent
    const params = {
      result: 'The search functionality has been implemented successfully with BM25 ranking.'
    };
    
    const validation = attemptCompletionSchema.safeParse(params);
    expect(validation.success).toBe(true);
    
    // Simulate what ProbeAgent.js does - just extract the result
    const finalResult = validation.data.result;
    
    expect(finalResult).toBe(params.result);
    expect(typeof finalResult).toBe('string');
  });

  test('should handle attempt_completion with markdown formatting', () => {
    const params = {
      result: `# Authentication Analysis

## Overview
The system implements secure authentication using:
- JWT tokens with RS256 signing
- bcrypt password hashing
- Rate limiting on login attempts

## Security Score: A-

The implementation follows security best practices.`
    };
    
    const validation = attemptCompletionSchema.safeParse(params);
    expect(validation.success).toBe(true);
    
    const finalResult = validation.data.result;
    expect(finalResult).toContain('# Authentication Analysis');
    expect(finalResult).toContain('## Security Score: A-');
  });

  test('should not require JSON cleaning or validation for plain text', () => {
    // This test ensures we don't need the complex JSON cleaning logic anymore
    const plainTextResult = 'Simple analysis result without any JSON formatting.';
    
    // Direct validation - no JSON parsing needed
    const validation = attemptCompletionSchema.safeParse({ result: plainTextResult });
    expect(validation.success).toBe(true);
    
    // No need for cleanSchemaResponse or validateJsonResponse
    expect(validation.data.result).toBe(plainTextResult);
  });
});

describe('Legacy Compatibility', () => {
  test('should maintain backward compatibility with direct content format', () => {
    // Ensure direct content attempt_completion XML works
    const directXml = `<attempt_completion>
Task completed successfully.
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(directXml);

    expect(parsed).toBeDefined();
    expect(parsed.toolName).toBe('attempt_completion');
    expect(parsed.params.result).toBe('Task completed successfully.');
  });

  test('should handle content that includes old-style tags', () => {
    // Content might include old-style tags, which should be treated as content now
    const xmlWithOldTags = `<attempt_completion>
Task completed.
<command>npm test</command>
</attempt_completion>`;

    const parsed = parseXmlToolCallWithThinking(xmlWithOldTags);

    expect(parsed).toBeDefined();
    expect(parsed.toolName).toBe('attempt_completion');
    expect(parsed.params.result).toContain('Task completed.');
    expect(parsed.params.result).toContain('<command>npm test</command>');

    // The result should include everything as content
    const validation = attemptCompletionSchema.safeParse(parsed.params);
    expect(validation.success).toBe(true);
  });
});