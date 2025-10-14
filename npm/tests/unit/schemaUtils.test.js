/**
 * Unit tests for schemaUtils module
 * Tests JSON and Mermaid validation functionality
 */

import { describe, test, expect, beforeEach } from '@jest/globals';
import {
  cleanSchemaResponse,
  validateJsonResponse,
  validateXmlResponse,
  processSchemaResponse,
  isJsonSchema,
  createJsonCorrectionPrompt,
  isMermaidSchema,
  extractMermaidFromMarkdown,
  validateMermaidDiagram,
  validateMermaidResponse,
  createMermaidCorrectionPrompt
} from '../../src/agent/schemaUtils.js';

describe('Schema Utilities', () => {
  describe('cleanSchemaResponse', () => {
    test('should handle null/undefined input', () => {
      expect(cleanSchemaResponse(null)).toBeNull();
      expect(cleanSchemaResponse(undefined)).toBeUndefined();
      expect(cleanSchemaResponse('')).toBe('');
    });

    test('should handle non-string input', () => {
      expect(cleanSchemaResponse(123)).toBe(123);
      expect(cleanSchemaResponse({})).toEqual({});
    });

    test('should extract JSON from markdown code blocks when response starts with {', () => {
      const input = '```json\n{"test": "value"}\n```';
      const expected = '{"test": "value"}';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should extract JSON from markdown code blocks when response starts with [', () => {
      const input = '```json\n[{"test": "value"}]\n```';
      const expected = '[{"test": "value"}]';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should extract JSON boundaries correctly with multiple brackets', () => {
      const input = '```json\n{"nested": {"array": [1, 2, 3]}}\n```';
      const expected = '{"nested": {"array": [1, 2, 3]}}';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should return original input when not starting with JSON brackets', () => {
      const input = '```xml\n<root>test</root>\n```';
      expect(cleanSchemaResponse(input)).toBe(input); // Returns unchanged
    });

    test('should return original input for non-JSON backtick content', () => {
      const input = '`some text content`';
      expect(cleanSchemaResponse(input)).toBe(input); // Returns unchanged
    });

    test('should handle JSON with surrounding whitespace and markdown', () => {
      const input = '  ```json\n{"test": "value"}\n```  ';
      const expected = '{"test": "value"}';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should handle direct JSON input without markdown', () => {
      const input = '{"test": "value"}';
      const expected = '{"test": "value"}';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should handle array JSON input without markdown', () => {
      const input = '[1, 2, 3]';
      const expected = '[1, 2, 3]';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should not extract JSON from text with surrounding content', () => {
      const input = 'This is some text with {"json": "inside"}';
      // Should return original since JSON has text before/after it
      // This prevents false positives like extracting {{ pr.title }} from markdown
      expect(cleanSchemaResponse(input)).toBe(input);
    });

    test('should return original for text with too much content before JSON', () => {
      const input = 'Line 1\nLine 2\nLine 3\nLine 4\nMany lines of text that should prevent extraction {"json": "inside"}';
      // Should return original since there are too many lines before the JSON
      expect(cleanSchemaResponse(input)).toBe(input);
    });

    test('should handle empty JSON object', () => {
      const input = '{}';
      const expected = '{}';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should handle empty JSON array', () => {
      const input = '[]';
      const expected = '[]';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    // New tests for enhanced JSON detection after code blocks
    test('should extract JSON from code blocks with various patterns', () => {
      const testCases = [
        {
          input: '```json\n{"test": "value"}\n```',
          expected: '{"test": "value"}',
          description: 'standard json code block'
        },
        {
          input: '```\n{"test": "value"}\n```',
          expected: '{"test": "value"}',
          description: 'code block without language specifier'
        },
        {
          input: '`{"test": "value"}`',
          expected: '{"test": "value"}',
          description: 'single backtick JSON'
        }
      ];

      testCases.forEach(({ input, expected, description }) => {
        expect(cleanSchemaResponse(input)).toBe(expected);
      });
    });

    test('should handle code blocks with immediate JSON start', () => {
      const input = '```json\n{';
      const remaining = '"test": "value", "nested": {"array": [1, 2, 3]}}';
      const fullInput = input + remaining;
      
      expect(cleanSchemaResponse(fullInput)).toBe('{' + remaining);
    });

    test('should handle code blocks with array JSON', () => {
      const input = '```json\n[{"item": 1}, {"item": 2}]```';
      const expected = '[{"item": 1}, {"item": 2}]';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should extract JSON with proper bracket counting', () => {
      const input = '```json\n{"outer": {"inner": {"deep": [1, 2, {"nested": true}]}}}\n```';
      const expected = '{"outer": {"inner": {"deep": [1, 2, {"nested": true}]}}}';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should handle code blocks with whitespace after marker', () => {
      const input = '```json   \n  {"test": "value"}  \n```';
      const expected = '{"test": "value"}';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should handle incomplete code blocks gracefully', () => {
      const input = '```json\n{"test": "incomplete"';
      // Should fall back to boundary detection
      expect(cleanSchemaResponse(input)).toBe(input);
    });

    test('should prioritize code block extraction over boundary detection', () => {
      const input = 'Some text {"not": "this"} ```json\n{"extract": "this"}\n```';
      const expected = '{"extract": "this"}';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should handle mixed bracket types in code blocks', () => {
      const input = '```json\n[{"objects": [1, 2]}, {"more": {"nested": true}}]\n```';
      const expected = '[{"objects": [1, 2]}, {"more": {"nested": true}}]';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should not extract JSON when embedded in surrounding text', () => {
      const input = 'Here is some JSON: {"test": "value"} that should be extracted';
      // Should return original since JSON has text before and after it
      // This prevents extracting fragments like {{ pr.title }} from content
      expect(cleanSchemaResponse(input)).toBe(input);
    });

    test('should not extract JSON when text precedes it', () => {
      const input = 'Result:\n{"test": "value"}';
      // Should return original since there's text before the JSON
      expect(cleanSchemaResponse(input)).toBe(input);
    });

    test('should extract JSON from code block after correction prompt (mermaid-style fix)', () => {
      // This is the exact pattern we see when LLM responds to correction prompts
      // with ```json blocks instead of raw JSON
      const input = '```json\n{\n  "issues": [\n    {\n      "file": "test.js",\n      "line": 1\n    }\n  ]\n}\n```';
      const result = cleanSchemaResponse(input);

      // Should extract the JSON content without the code block markers
      expect(result).not.toContain('```');
      expect(result).toContain('"issues"');

      // Verify it can be parsed
      expect(() => JSON.parse(result)).not.toThrow();
      const parsed = JSON.parse(result);
      expect(parsed.issues).toBeDefined();
      expect(Array.isArray(parsed.issues)).toBe(true);
    });

    test('should extract multiline JSON from ```json blocks', () => {
      const input = '```json\n{\n  "key": "value",\n  "nested": {\n    "array": [1, 2, 3]\n  }\n}\n```';
      const result = cleanSchemaResponse(input);

      expect(result).not.toContain('```');
      const parsed = JSON.parse(result);
      expect(parsed.key).toBe('value');
      expect(parsed.nested.array).toEqual([1, 2, 3]);
    });
  });

  describe('validateJsonResponse', () => {
    test('should validate correct JSON', () => {
      const result = validateJsonResponse('{"test": "value"}');
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual({ test: "value" });
    });

    test('should validate JSON arrays', () => {
      const result = validateJsonResponse('[1, 2, 3]');
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual([1, 2, 3]);
    });

    test('should validate primitive JSON values', () => {
      expect(validateJsonResponse('null').isValid).toBe(true);
      expect(validateJsonResponse('42').isValid).toBe(true);
      expect(validateJsonResponse('"string"').isValid).toBe(true);
      expect(validateJsonResponse('true').isValid).toBe(true);
    });

    test('should reject invalid JSON', () => {
      const result = validateJsonResponse('{"test": value}'); // Missing quotes
      expect(result.isValid).toBe(false);
      expect(result.error).toBeDefined();
    });

    test('should reject incomplete JSON', () => {
      const result = validateJsonResponse('{"test":');
      expect(result.isValid).toBe(false);
      expect(result.error).toBeDefined();
    });

    test('should handle empty input', () => {
      const result = validateJsonResponse('');
      expect(result.isValid).toBe(false);
      expect(result.error).toBeDefined();
    });

    test('should handle complex nested JSON', () => {
      const complex = '{"nested": {"array": [1, {"deep": true}], "null": null}}';
      const result = validateJsonResponse(complex);
      expect(result.isValid).toBe(true);
      expect(result.parsed.nested.array[1].deep).toBe(true);
    });
  });

  describe('validateXmlResponse', () => {
    test('should validate basic XML', () => {
      const result = validateXmlResponse('<root>test</root>');
      expect(result.isValid).toBe(true);
    });

    test('should validate XML with attributes', () => {
      const result = validateXmlResponse('<root attr="value">test</root>');
      expect(result.isValid).toBe(true);
    });

    test('should validate self-closing tags', () => {
      const result = validateXmlResponse('<root><item/></root>');
      expect(result.isValid).toBe(true);
    });

    test('should reject non-XML content', () => {
      const result = validateXmlResponse('just plain text');
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('No XML tags found');
    });

    test('should reject empty input', () => {
      const result = validateXmlResponse('');
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('No XML tags found');
    });
  });

  describe('isJsonSchema', () => {
    test('should detect JSON object schemas', () => {
      expect(isJsonSchema('{"type": "object"}')).toBe(true);
      expect(isJsonSchema('{ "properties": {} }')).toBe(true);
      expect(isJsonSchema('{"test": "value"}')).toBe(true);
    });

    test('should detect JSON array schemas', () => {
      expect(isJsonSchema('[{"name": "string"}]')).toBe(true);
      expect(isJsonSchema('[]')).toBe(true);
    });

    test('should detect JSON content-type indicators', () => {
      expect(isJsonSchema('application/json')).toBe(true);
      expect(isJsonSchema('Response should be JSON format')).toBe(true);
      expect(isJsonSchema('return as json')).toBe(true);
    });

    test('should handle mixed case', () => {
      expect(isJsonSchema('{"Type": "Object"}')).toBe(true);
      expect(isJsonSchema('APPLICATION/JSON')).toBe(true);
    });

    test('should reject non-JSON schemas', () => {
      expect(isJsonSchema('<schema></schema>')).toBe(false);
      expect(isJsonSchema('plain text schema')).toBe(false);
      expect(isJsonSchema('')).toBe(false);
      expect(isJsonSchema(null)).toBe(false);
      expect(isJsonSchema(undefined)).toBe(false);
    });
  });

  describe('createJsonCorrectionPrompt', () => {
    test('should create basic correction prompt for first retry (retryCount 0)', () => {
      const invalidResponse = '{"test": value}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v in JSON';
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error, 0);
      
      expect(prompt).toContain(invalidResponse);
      expect(prompt).toContain(schema);
      expect(prompt).toContain(error);
      expect(prompt).toContain('CRITICAL JSON ERROR:');
      expect(prompt).toContain('Return ONLY the corrected JSON');
    });

    test('should create more urgent prompt for second retry (retryCount 1)', () => {
      const invalidResponse = '{"test": value}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v in JSON';
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error, 1);
      
      expect(prompt).toContain('URGENT - JSON PARSING FAILED:');
      expect(prompt).toContain('second chance');
      expect(prompt).toContain('ABSOLUTELY NO explanatory text');
    });

    test('should create final attempt prompt for third retry (retryCount 2)', () => {
      const invalidResponse = '{"test": value}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v in JSON';
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error, 2);
      
      expect(prompt).toContain('FINAL ATTEMPT - CRITICAL JSON ERROR:');
      expect(prompt).toContain('final retry');
      expect(prompt).toContain('EXAMPLE:');
      expect(prompt).toContain('NOT:');
    });

    test('should cap at highest strength level for retryCount > 2', () => {
      const invalidResponse = '{"test": value}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v in JSON';
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error, 5);
      
      expect(prompt).toContain('FINAL ATTEMPT - CRITICAL JSON ERROR:');
    });

    test('should truncate long invalid responses', () => {
      const longResponse = 'Hello '.repeat(200) + '{"test": value}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v';
      
      const prompt = createJsonCorrectionPrompt(longResponse, schema, error, 0);
      
      expect(prompt).toContain('...');
      expect(prompt.length).toBeLessThan(longResponse.length + 500);
    });

    test('should handle default retryCount parameter', () => {
      const invalidResponse = '{"test": value}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v in JSON';
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error);
      
      expect(prompt).toContain('CRITICAL JSON ERROR:');
    });

    test('should handle multiline responses with truncation', () => {
      const invalidResponse = '{\n  "test": value\n}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v';
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error, 1);
      
      expect(prompt).toContain('URGENT');
      expect(prompt.split('\n').length).toBeGreaterThan(5);
    });
  });

  describe('processSchemaResponse', () => {
    test('should process and clean response', () => {
      const input = '```json\n{"test": "value"}\n```';
      const result = processSchemaResponse(input, '{}');
      
      expect(result.cleaned).toBe('{"test": "value"}');
    });

    test('should include debug information when requested', () => {
      const input = '```json\n{"test": "value"}\n```';
      const result = processSchemaResponse(input, '{}', { debug: true });
      
      expect(result.debug).toBeDefined();
      expect(result.debug.wasModified).toBe(true);
      expect(result.debug.originalLength).toBeGreaterThan(result.debug.cleanedLength);
    });

    test('should validate JSON when requested', () => {
      const input = '{"test": "value"}';
      const result = processSchemaResponse(input, '{}', { validateJson: true });
      
      expect(result.jsonValidation).toBeDefined();
      expect(result.jsonValidation.isValid).toBe(true);
    });

    test('should validate XML when requested', () => {
      const input = '<root>test</root>';
      const result = processSchemaResponse(input, '<schema/>', { validateXml: true });
      
      expect(result.xmlValidation).toBeDefined();
      expect(result.xmlValidation.isValid).toBe(true);
    });
  });
});