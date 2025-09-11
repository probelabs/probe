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

    test('should remove markdown code blocks', () => {
      const input = '```json\n{"test": "value"}\n```';
      const expected = '{"test": "value"}';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should remove multiple code blocks', () => {
      const input = '```json\n{"test": "value"}\n```\n\n```\nmore code\n```';
      const expected = '{"test": "value"}\n\nmore code';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should remove code blocks with different languages', () => {
      const input = '```xml\n<root>test</root>\n```';
      const expected = '<root>test</root>';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should remove inline backticks', () => {
      const input = '`{"test": "value"}`';
      const expected = '{"test": "value"}';
      expect(cleanSchemaResponse(input)).toBe(expected);
    });

    test('should handle mixed formatting', () => {
      const input = '  ```json\n{"test": "value"}\n```  ';
      const expected = '{"test": "value"}';
      expect(cleanSchemaResponse(input)).toBe(expected);
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
    test('should create basic correction prompt', () => {
      const invalidResponse = '{"test": value}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v in JSON';
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error);
      
      expect(prompt).toContain(invalidResponse);
      expect(prompt).toContain(schema);
      expect(prompt).toContain(error);
      expect(prompt).toContain('not valid JSON');
      expect(prompt).toContain('Return ONLY the corrected JSON');
    });

    test('should include detailed errors when provided', () => {
      const invalidResponse = '{"test": value}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v';
      const detailedError = 'Unexpected token v in JSON at position 9';
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error, detailedError);
      
      expect(prompt).toContain(error);
      expect(prompt).toContain(detailedError);
      expect(prompt).toContain('Error:');
      expect(prompt).toContain('Detailed Error:');
    });

    test('should not duplicate identical errors', () => {
      const invalidResponse = '{"test":}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected end of JSON input';
      const detailedError = 'Unexpected end of JSON input'; // Same as error
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error, detailedError);
      
      const errorOccurrences = (prompt.match(/Unexpected end of JSON input/g) || []).length;
      expect(errorOccurrences).toBe(1);
    });

    test('should handle multiline responses', () => {
      const invalidResponse = '{\n  "test": value\n}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v';
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error);
      
      expect(prompt).toContain(invalidResponse);
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