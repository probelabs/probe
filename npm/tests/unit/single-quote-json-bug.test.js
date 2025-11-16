/**
 * Test for single-quote JSON bug
 *
 * Bug Description:
 * When AI responses contain JavaScript array syntax with single quotes like ['*']
 * instead of valid JSON with double quotes ["*"], the JSON parser fails with:
 * "Unexpected token ''', "['*']" is not valid JSON"
 *
 * This test replicates the bug seen in the debug logs where:
 * 1. AI returns response with JavaScript array syntax: ['*', '!bash']
 * 2. cleanSchemaResponse extracts it as-is (no syntax normalization)
 * 3. JSON.parse fails because single quotes are invalid in JSON
 */

import { describe, test, expect } from '@jest/globals';
import { cleanSchemaResponse, validateJsonResponse } from '../../src/agent/schemaUtils.js';

describe('Single-quote JSON Bug', () => {
  describe('JavaScript array syntax with single quotes', () => {
    test('should fail to parse JavaScript array syntax with single quotes', () => {
      // This is what the AI returns - JavaScript syntax, not JSON
      const invalidJson = "['*']";

      const result = validateJsonResponse(invalidJson);

      expect(result.isValid).toBe(false);
      expect(result.error).toBeDefined();
      // Should contain error about single quote
      expect(result.error.toLowerCase()).toMatch(/unexpected token|unexpected character|'|quote/);
    });

    test('should fail to parse array with single-quoted strings', () => {
      // More complex example from the bug report
      const invalidJson = "['*', '!bash']";

      const result = validateJsonResponse(invalidJson);

      expect(result.isValid).toBe(false);
      expect(result.error).toBeDefined();
      expect(result.errorContext).toBeDefined();
      expect(result.errorContext.position).toBe(1); // Points to the first single quote
    });

    test('should succeed with double-quoted JSON array syntax', () => {
      // This is what SHOULD be returned - valid JSON
      const validJson = '["*"]';

      const result = validateJsonResponse(validJson);

      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(['*']);
    });

    test('should succeed with double-quoted JSON array with exclusions', () => {
      // Valid JSON version of the bug example
      const validJson = '["*", "!bash"]';

      const result = validateJsonResponse(validJson);

      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(['*', '!bash']);
    });
  });

  describe('cleanSchemaResponse normalizes quote syntax (FIX)', () => {
    test('should not extract from javascript code blocks (only json/generic blocks)', () => {
      // AI response with JavaScript array syntax in ```javascript block
      const input = "```javascript\n['*', '!bash']\n```";

      const cleaned = cleanSchemaResponse(input);

      // cleanSchemaResponse doesn't extract from ```javascript blocks, only ```json and ```
      // So it returns the original input unchanged
      expect(cleaned).toBe(input);

      // The uncleaned version with code block markers is not valid JSON
      const result = validateJsonResponse(cleaned);
      expect(result.isValid).toBe(false);
    });

    test('should extract JavaScript array from json code block AND normalize quotes', () => {
      // Even in a ```json block, the AI might use single quotes
      const input = "```json\n['*']\n```";

      const cleaned = cleanSchemaResponse(input);

      // FIX: Extraction happens AND quote normalization (single -> double quotes)
      expect(cleaned).toBe('["*"]');

      // Now it's valid JSON!
      const result = validateJsonResponse(cleaned);
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(['*']);
    });

    test('should not modify valid JSON arrays with double quotes', () => {
      const input = '```json\n["*", "!bash"]\n```';

      const cleaned = cleanSchemaResponse(input);

      expect(cleaned).toBe('["*", "!bash"]');

      // This is valid JSON
      const result = validateJsonResponse(cleaned);
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(['*', '!bash']);
    });

    test('should normalize single quotes in complex arrays', () => {
      const input = "```json\n['*', '!bash', '!docker']\n```";

      const cleaned = cleanSchemaResponse(input);

      // Should normalize all single quotes to double quotes
      expect(cleaned).toBe('["*", "!bash", "!docker"]');

      const result = validateJsonResponse(cleaned);
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(['*', '!bash', '!docker']);
    });

    test('should normalize single quotes in objects', () => {
      const input = "```json\n{'key': 'value', 'num': 42}\n```";

      const cleaned = cleanSchemaResponse(input);

      // Should normalize object keys and string values
      expect(cleaned).toBe('{"key": "value", "num": 42}');

      const result = validateJsonResponse(cleaned);
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual({ key: 'value', num: 42 });
    });
  });

  describe('Real-world examples from bug report', () => {
    test('should replicate exact error from debug log line 9', () => {
      // From log line 9: [DEBUG] JSON validation: Preview: ['*', '!bash']
      const buggyResponse = "['*', '!bash']";

      const result = validateJsonResponse(buggyResponse);

      // From log line 10: Parse failed with error: Unexpected token ''', "['*', '!bash']" is not valid JSON
      expect(result.isValid).toBe(false);
      expect(result.error).toContain("Unexpected token");
    });

    test('should replicate exact error from debug log line 48', () => {
      // From log line 48: [DEBUG] JSON validation: Preview: ['*']
      const buggyResponse = "['*']";

      const result = validateJsonResponse(buggyResponse);

      // From log line 49: Parse failed with error: Unexpected token ''', "['*']" is not valid JSON
      expect(result.isValid).toBe(false);
      expect(result.error).toContain("Unexpected token");
    });

    test('should show error context pointing to single quote', () => {
      const buggyResponse = "['*']";

      const result = validateJsonResponse(buggyResponse);

      expect(result.isValid).toBe(false);
      expect(result.errorContext).toBeDefined();
      // Error should point to position 1 (the first single quote after '[')
      expect(result.errorContext.position).toBe(1);
      expect(result.errorContext.snippet).toContain("['*']");
    });
  });

  describe('Edge cases with mixed quotes', () => {
    test('should fail with mixed single and double quotes', () => {
      const invalidJson = '["foo", \'bar\']';

      const result = validateJsonResponse(invalidJson);

      expect(result.isValid).toBe(false);
    });

    test('should fail with object using single quotes', () => {
      const invalidJson = "{'key': 'value'}";

      const result = validateJsonResponse(invalidJson);

      expect(result.isValid).toBe(false);
    });

    test('should succeed with properly escaped single quotes inside double quotes', () => {
      const validJson = '["It\'s valid"]';

      const result = validateJsonResponse(validJson);

      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(["It's valid"]);
    });
  });

  describe('Attempt completion context', () => {
    test('should succeed when attempt_completion result contains JavaScript array syntax (FIX)', () => {
      // This simulates the exact scenario from the bug report where
      // attempt_completion's result field contains ['*'] instead of ["*"]
      const attemptCompletionResult = "['*']";

      // First, cleanSchemaResponse is called (now it DOES normalize quotes - FIX!)
      const cleaned = cleanSchemaResponse(attemptCompletionResult);
      expect(cleaned).toBe('["*"]'); // Single quotes normalized to double quotes

      // Then validateJsonResponse is called without schema
      const result = validateJsonResponse(cleaned);

      // With the fix, this should now succeed!
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(['*']);
    });

    test('should handle multi-line response with single-quote arrays', () => {
      // AI might return explanation followed by array
      const response = `Looking at the _parseAllowedTools method, when allowedTools is undefined,
the system returns ['*'] which enables all tools.`;

      // cleanSchemaResponse won't extract this (text before JSON)
      const cleaned = cleanSchemaResponse(response);
      expect(cleaned).toBe(response);

      // Validation will fail because it's not valid JSON
      const result = validateJsonResponse(cleaned);
      expect(result.isValid).toBe(false);
    });
  });
});
