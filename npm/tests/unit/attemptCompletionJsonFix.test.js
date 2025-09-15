/**
 * Unit tests for attempt_completion JSON schema fix
 * Tests the core functionality without requiring full ProbeAgent integration
 */

import { describe, test, expect } from '@jest/globals';
import { 
  cleanSchemaResponse, 
  validateJsonResponse, 
  isJsonSchemaDefinition,
  createJsonCorrectionPrompt,
  createSchemaDefinitionCorrectionPrompt 
} from '../../src/agent/schemaUtils.js';

describe('attempt_completion JSON schema fix', () => {
  
  describe('Core Schema Processing Logic', () => {
    test('should detect when AI returns schema definition instead of data', () => {
      // Simulate attempt_completion result that contains schema definition
      const schemaDefinitionResponse = `\`\`\`json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "word-count",
  "title": "Word Count Result",
  "description": "Result of word counting operation",
  "type": "object",
  "required": ["message", "count"],
  "properties": {
    "message": {
      "type": "string",
      "description": "A descriptive message about the word count"
    },
    "count": {
      "type": "number",
      "description": "The actual number of words counted"
    }
  }
}
\`\`\``;

      // Clean the response (remove markdown)
      const cleaned = cleanSchemaResponse(schemaDefinitionResponse);
      
      // Should be valid JSON
      const validation = validateJsonResponse(cleaned);
      expect(validation.isValid).toBe(true);
      
      // But should be detected as schema definition, not data
      const isSchemaDefinition = isJsonSchemaDefinition(cleaned);
      expect(isSchemaDefinition).toBe(true);
    });

    test('should not detect actual data as schema definition', () => {
      // Simulate correct attempt_completion result with actual data
      const dataResponse = `\`\`\`json
{
  "message": "There are 3 words in the text Hello world test",
  "count": 3
}
\`\`\``;

      // Clean the response
      const cleaned = cleanSchemaResponse(dataResponse);
      
      // Should be valid JSON
      const validation = validateJsonResponse(cleaned);
      expect(validation.isValid).toBe(true);
      
      // Should NOT be detected as schema definition
      const isSchemaDefinition = isJsonSchemaDefinition(cleaned);
      expect(isSchemaDefinition).toBe(false);
      
      // Parse and verify it's actual data
      const parsed = JSON.parse(cleaned);
      expect(parsed).toHaveProperty('message');
      expect(parsed).toHaveProperty('count');
      expect(typeof parsed.message).toBe('string');
      expect(typeof parsed.count).toBe('number');
    });

    test('should create appropriate correction prompt for schema definition confusion', () => {
      const schemaDefinition = JSON.stringify({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
          "message": {"type": "string"},
          "count": {"type": "number"}
        },
        "required": ["message", "count"]
      });

      const originalSchema = `{
  "type": "object",
  "properties": {
    "message": {"type": "string"},
    "count": {"type": "number"}
  },
  "required": ["message", "count"]
}`;

      // Test first correction attempt
      const correctionPrompt = createSchemaDefinitionCorrectionPrompt(
        schemaDefinition, 
        originalSchema, 
        0
      );

      expect(correctionPrompt).toContain('CRITICAL MISUNDERSTANDING:');
      expect(correctionPrompt).toContain('You returned a JSON schema definition instead of data');
      expect(correctionPrompt).toContain('You must return ACTUAL DATA');
      expect(correctionPrompt).toContain('Instead of: {"type": "object"');
      expect(correctionPrompt).toContain('Return: {"actualData": "value"');
      expect(correctionPrompt).toContain(originalSchema);
    });

    test('should create escalating correction prompts for invalid JSON', () => {
      const invalidResponse = 'Based on the text "Hello world test", I can count that there are 3 words.';
      const schema = `{"type": "object", "properties": {"message": {"type": "string"}, "count": {"type": "number"}}}`;
      const error = 'Unexpected token B in JSON at position 0';

      // Test escalating prompts
      const prompts = [
        createJsonCorrectionPrompt(invalidResponse, schema, error, 0),
        createJsonCorrectionPrompt(invalidResponse, schema, error, 1),
        createJsonCorrectionPrompt(invalidResponse, schema, error, 2)
      ];

      expect(prompts[0]).toContain('CRITICAL JSON ERROR:');
      expect(prompts[1]).toContain('URGENT - JSON PARSING FAILED:');
      expect(prompts[2]).toContain('FINAL ATTEMPT - CRITICAL JSON ERROR:');

      // All should contain the error and schema
      prompts.forEach(prompt => {
        expect(prompt).toContain(error);
        expect(prompt).toContain(schema);
        expect(prompt).toContain(invalidResponse.substring(0, 100));
      });
    });

    test('should properly clean markdown from attempt_completion responses', () => {
      const testCases = [
        {
          input: 'Based on analysis:\n\n```json\n{"result": "success"}\n```\n\nThis completes the task.',
          expected: '{"result": "success"}',
          description: 'should extract JSON from markdown with surrounding text'
        },
        {
          input: '```json\n{"message": "Word count is 3", "count": 3}\n```',
          expected: '{"message": "Word count is 3", "count": 3}',
          description: 'should extract clean JSON from simple markdown block'
        },
        {
          input: 'Plain text response without JSON',
          expected: 'Plain text response without JSON',
          description: 'should return plain text unchanged when no JSON found'
        },
        {
          input: '{"direct": "json", "without": "markdown"}',
          expected: '{"direct": "json", "without": "markdown"}',
          description: 'should handle direct JSON without markdown'
        }
      ];

      testCases.forEach(({ input, expected, description }) => {
        const result = cleanSchemaResponse(input);
        expect(result).toBe(expected);
      });
    });
  });

  describe('JSON Validation Edge Cases', () => {
    test('should handle malformed JSON responses', () => {
      const malformedResponses = [
        '{"incomplete": ',
        '{"missing": "quote}',
        '{"trailing": "comma",}',
        '{invalid: "property"}'
      ];

      malformedResponses.forEach(response => {
        const validation = validateJsonResponse(response);
        expect(validation.isValid).toBe(false);
        expect(validation.error).toBeDefined();
      });
    });

    test('should validate correct JSON responses', () => {
      const validResponses = [
        '{"simple": "object"}',
        '["array", "of", "strings"]',
        '{"nested": {"object": {"with": "values"}}}',
        '{"mixed": ["array", 123, {"nested": true}]}'
      ];

      validResponses.forEach(response => {
        const validation = validateJsonResponse(response);
        expect(validation.isValid).toBe(true);
        expect(validation.parsed).toBeDefined();
      });
    });
  });

  describe('Integration Simulation', () => {
    test('should simulate the complete attempt_completion JSON correction flow', () => {
      // Step 1: Simulate attempt_completion returning markdown
      const attemptCompletionResult = 'Based on my analysis of the text "Hello world test", I can confirm there are 3 words total.';

      // Step 2: Clean any potential markdown
      const cleaned = cleanSchemaResponse(attemptCompletionResult);
      expect(cleaned).toBe(attemptCompletionResult); // No markdown to clean

      // Step 3: Validate as JSON
      const validation = validateJsonResponse(cleaned);
      expect(validation.isValid).toBe(false); // Plain text is not JSON

      // Step 4: Create correction prompt
      const schema = '{"type": "object", "properties": {"message": {"type": "string"}, "count": {"type": "number"}}}';
      const correctionPrompt = createJsonCorrectionPrompt(
        cleaned, 
        schema, 
        validation.error, 
        0
      );

      expect(correctionPrompt).toContain('CRITICAL JSON ERROR:');
      expect(correctionPrompt).toContain('You MUST fix this and return ONLY valid JSON');
      expect(correctionPrompt).toContain(schema);
      expect(correctionPrompt).toContain(attemptCompletionResult.substring(0, 100));

      // Step 5: Simulate corrected response
      const correctedResponse = '{"message": "There are 3 words in the text Hello world test", "count": 3}';
      const finalValidation = validateJsonResponse(correctedResponse);
      expect(finalValidation.isValid).toBe(true);
      expect(finalValidation.parsed.message).toContain('3 words');
      expect(finalValidation.parsed.count).toBe(3);
    });
  });
});