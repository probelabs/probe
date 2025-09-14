import { describe, test, expect } from '@jest/globals';
import { 
  isJsonSchemaDefinition, 
  createSchemaDefinitionCorrectionPrompt 
} from '../../src/agent/schemaUtils.js';

describe('Schema definition detection', () => {
  test('should detect JSON schema definitions', () => {
    const schemaDefinition = JSON.stringify({
      "$schema": "http://json-schema.org/draft-07/schema#",
      "$id": "code-review",
      "title": "Code Review",
      "description": "Structured format for code review issues",
      "type": "object",
      "required": ["issues"],
      "properties": {
        "issues": {
          "type": "array",
          "items": {
            "type": "object"
          }
        }
      }
    });

    expect(isJsonSchemaDefinition(schemaDefinition)).toBe(true);
  });

  test('should not detect regular data as schema definition', () => {
    const regularData = JSON.stringify({
      "issues": [
        {
          "file": "test.js",
          "line": 1,
          "message": "This is an actual issue",
          "severity": "error"
        }
      ],
      "summary": "Code review completed"
    });

    expect(isJsonSchemaDefinition(regularData)).toBe(false);
  });

  test('should detect schema definition with minimal properties', () => {
    const minimalSchema = JSON.stringify({
      "type": "object",
      "properties": {
        "name": { "type": "string" }
      },
      "required": ["name"]
    });

    expect(isJsonSchemaDefinition(minimalSchema)).toBe(true);
  });

  test('should not detect objects with only one schema indicator', () => {
    const singleIndicator = JSON.stringify({
      "type": "object",
      "data": "some actual data"
    });

    expect(isJsonSchemaDefinition(singleIndicator)).toBe(false);
  });

  test('should handle invalid JSON gracefully', () => {
    const invalidJson = '{"incomplete": json';
    expect(isJsonSchemaDefinition(invalidJson)).toBe(false);
  });

  test('should handle null and undefined inputs', () => {
    expect(isJsonSchemaDefinition(null)).toBe(false);
    expect(isJsonSchemaDefinition(undefined)).toBe(false);
    expect(isJsonSchemaDefinition('')).toBe(false);
  });

  test('should create appropriate correction prompt for schema definition', () => {
    const schemaDefinition = JSON.stringify({
      "$schema": "http://json-schema.org/draft-07/schema#",
      "type": "object",
      "properties": {
        "name": { "type": "string" }
      }
    });

    const originalSchema = `{
      "type": "object",
      "properties": {
        "name": { "type": "string" },
        "age": { "type": "number" }
      }
    }`;

    const prompt = createSchemaDefinitionCorrectionPrompt(schemaDefinition, originalSchema, 0);

    expect(prompt).toContain('CRITICAL MISUNDERSTANDING');
    expect(prompt).toContain('schema definition instead of data');
    expect(prompt).toContain('ACTUAL DATA that follows the schema');
    expect(prompt).toContain('Instead of:');
    expect(prompt).toContain('Return:');
  });

  test('should escalate correction prompt severity with retry count', () => {
    const schemaDefinition = '{"type": "object"}';
    const originalSchema = '{"type": "object", "properties": {"test": {"type": "string"}}}';

    const prompt1 = createSchemaDefinitionCorrectionPrompt(schemaDefinition, originalSchema, 0);
    const prompt2 = createSchemaDefinitionCorrectionPrompt(schemaDefinition, originalSchema, 1);
    const prompt3 = createSchemaDefinitionCorrectionPrompt(schemaDefinition, originalSchema, 2);

    expect(prompt1).toContain('CRITICAL MISUNDERSTANDING');
    expect(prompt2).toContain('URGENT - WRONG RESPONSE TYPE');
    expect(prompt3).toContain('FINAL ATTEMPT - SCHEMA VS DATA CONFUSION');
  });
});