/**
 * Test enhanced JSON validation error messages with context snippets
 */

import { describe, test, expect } from '@jest/globals';
import { validateJsonResponse, createJsonCorrectionPrompt, cleanSchemaResponse } from '../../src/agent/schemaUtils.js';

describe('Enhanced JSON Validation Error Messages', () => {
  describe('validateJsonResponse with error context', () => {
    test('should provide error context snippet for invalid JSON', () => {
      const invalidJson = '{"name": "test", "value": invalid, "id": 123}';
      const result = validateJsonResponse(invalidJson, { debug: false });

      expect(result.isValid).toBe(false);
      expect(result.error).toBeDefined();
      expect(result.enhancedError).toBeDefined();
      expect(result.errorContext).toBeDefined();

      // Check error context structure
      expect(result.errorContext.position).toBeGreaterThanOrEqual(0);
      expect(result.errorContext.snippet).toContain('invalid');
      expect(result.errorContext.pointer).toBeDefined();
    });

    test('should show visual pointer at error location', () => {
      const invalidJson = '{"key": "value", "broken": ]';
      const result = validateJsonResponse(invalidJson, { debug: false });

      expect(result.isValid).toBe(false);
      expect(result.errorContext).toBeDefined();
      expect(result.errorContext.pointer).toMatch(/\s*\^/); // Arrow pointer
      expect(result.enhancedError).toContain('^ here');
    });

    test('should handle errors at the beginning of string', () => {
      const invalidJson = ']{"key": "value"}';
      const result = validateJsonResponse(invalidJson, { debug: false });

      expect(result.isValid).toBe(false);
      expect(result.errorContext).toBeDefined();
      expect(result.errorContext.position).toBe(0);
    });

    test('should handle errors at the end of string', () => {
      const invalidJson = '{"key": "value"';
      const result = validateJsonResponse(invalidJson, { debug: false });

      expect(result.isValid).toBe(false);
      expect(result.errorContext).toBeDefined();
    });

    test('should handle multi-line JSON with error context', () => {
      const invalidJson = `{
  "name": "test",
  "value": invalid,
  "id": 123
}`;
      const result = validateJsonResponse(invalidJson, { debug: false });

      expect(result.isValid).toBe(false);
      expect(result.errorContext).toBeDefined();
      expect(result.errorContext.snippet).toBeDefined();
      // Snippet should preserve newlines
      expect(result.errorContext.snippet.includes('\n')).toBe(true);
    });

    test('should return valid result for correct JSON', () => {
      const validJson = '{"name": "test", "value": 42}';
      const result = validateJsonResponse(validJson, { debug: false });

      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual({ name: 'test', value: 42 });
      expect(result.errorContext).toBeUndefined();
      expect(result.enhancedError).toBeUndefined();
    });

    test('should handle very long JSON with truncated context', () => {
      // Create a very long JSON string
      const longValue = 'x'.repeat(200);
      const invalidJson = `{"key": "${longValue}", "broken": invalid}`;
      const result = validateJsonResponse(invalidJson, { debug: false });

      expect(result.isValid).toBe(false);
      expect(result.errorContext).toBeDefined();
      // Context snippet should be limited to 100 chars (50 before + 50 after)
      expect(result.errorContext.snippet.length).toBeLessThanOrEqual(101);
    });
  });

  describe('createJsonCorrectionPrompt with enhanced errors', () => {
    test('should accept validation result object and include enhanced error', () => {
      const invalidJson = '{"key": invalid}';
      const validation = validateJsonResponse(invalidJson, { debug: false });
      const schema = '{"type": "object"}';

      const prompt = createJsonCorrectionPrompt(invalidJson, schema, validation, 0);

      expect(prompt).toContain('CRITICAL JSON ERROR');
      expect(prompt).toContain('Error location');
      expect(prompt).toContain('^ here');
    });

    test('should still work with plain error string (backwards compatibility)', () => {
      const invalidJson = '{"key": invalid}';
      const errorString = 'Unexpected token i in JSON at position 8';
      const schema = '{"type": "object"}';

      const prompt = createJsonCorrectionPrompt(invalidJson, schema, errorString, 0);

      expect(prompt).toContain('CRITICAL JSON ERROR');
      expect(prompt).toContain(errorString);
    });

    test('should include error context in correction prompt', () => {
      const invalidJson = '{"name": "test", "value": broken, "id": 123}';
      const validation = validateJsonResponse(invalidJson, { debug: false });
      const schema = '{"type": "object"}';

      const prompt = createJsonCorrectionPrompt(invalidJson, schema, validation, 0);

      // Should include the enhanced error with visual pointer
      expect(prompt).toContain('Error:');
      if (validation.enhancedError) {
        expect(prompt).toContain('Error location');
      }
    });
  });

  describe('cleanSchemaResponse before validation', () => {
    test('should extract JSON from markdown code blocks', () => {
      const markdownResponse = '```json\n{"key": "value"}\n```';
      const cleaned = cleanSchemaResponse(markdownResponse);

      expect(cleaned).toBe('{"key": "value"}');

      // Now validate the cleaned version
      const result = validateJsonResponse(cleaned, { debug: false });
      expect(result.isValid).toBe(true);
    });

    test('should not extract JSON when text precedes it', () => {
      const responseWithText = 'Here is your data:\n{"key": "value"}';
      const cleaned = cleanSchemaResponse(responseWithText);

      // Should return original since text precedes the JSON
      expect(cleaned).toBe(responseWithText);

      // Original text is not valid JSON
      const result = validateJsonResponse(cleaned, { debug: false });
      expect(result.isValid).toBe(false);
    });

    test('should handle JSON in generic code blocks', () => {
      const genericCodeBlock = '```\n{"key": "value"}\n```';
      const cleaned = cleanSchemaResponse(genericCodeBlock);

      expect(cleaned).toBe('{"key": "value"}');

      const result = validateJsonResponse(cleaned, { debug: false });
      expect(result.isValid).toBe(true);
    });

    test('should preserve raw JSON if no cleaning needed', () => {
      const rawJson = '{"key": "value"}';
      const cleaned = cleanSchemaResponse(rawJson);

      expect(cleaned).toBe(rawJson);
    });
  });

  describe('Integration: Clean then validate workflow', () => {
    test('should clean then validate successfully', () => {
      const response = '```json\n{"name": "test", "id": 42}\n```';

      // Step 1: Clean
      const cleaned = cleanSchemaResponse(response);
      expect(cleaned).toBe('{"name": "test", "id": 42}');

      // Step 2: Validate
      const validation = validateJsonResponse(cleaned, { debug: false });
      expect(validation.isValid).toBe(true);
      expect(validation.parsed).toEqual({ name: 'test', id: 42 });
    });

    test('should clean then show enhanced error if still invalid', () => {
      const response = '```json\n{"name": "test", "value": invalid}\n```';

      // Step 1: Clean
      const cleaned = cleanSchemaResponse(response);
      expect(cleaned).toBe('{"name": "test", "value": invalid}');

      // Step 2: Validate
      const validation = validateJsonResponse(cleaned, { debug: false });
      expect(validation.isValid).toBe(false);
      expect(validation.enhancedError).toContain('Error location');
      expect(validation.enhancedError).toContain('^ here');
    });
  });
});
