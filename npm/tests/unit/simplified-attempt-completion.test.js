/**
 * Tests for simplified attempt_completion tool - without command parameter and JSON issues
 */
import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { attemptCompletionSchema } from '../../src/tools/common.js';

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

