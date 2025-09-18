/**
 * Integration tests for schema validation retry logic
 * Tests the new 3-attempt retry mechanism with escalating prompts
 */

import { describe, test, expect, jest, beforeEach, afterEach } from '@jest/globals';
import * as schemaUtils from '../../src/agent/schemaUtils.js';

describe('Schema Validation Retry Logic Integration Tests', () => {
  let mockAnswer;
  
  beforeEach(() => {
    // Create a mock answer function that simulates the ProbeAgent.answer method
    mockAnswer = jest.fn();
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  test('should create escalating correction prompts for retry attempts', () => {
    const invalidResponse = 'Hello, this is not JSON';
    const schema = '{"result": "string"}';
    const error = 'Unexpected token H in JSON at position 0';
    
    // Test the first retry (retryCount 0)
    const prompt0 = schemaUtils.createJsonCorrectionPrompt(invalidResponse, schema, error, 0);
    expect(prompt0).toContain('CRITICAL JSON ERROR:');
    expect(prompt0).toContain('Return ONLY the corrected JSON');
    
    // Test the second retry (retryCount 1)  
    const prompt1 = schemaUtils.createJsonCorrectionPrompt(invalidResponse, schema, error, 1);
    expect(prompt1).toContain('URGENT - JSON PARSING FAILED:');
    expect(prompt1).toContain('second chance');
    expect(prompt1).toContain('ABSOLUTELY NO explanatory text');
    
    // Test the third retry (retryCount 2)
    const prompt2 = schemaUtils.createJsonCorrectionPrompt(invalidResponse, schema, error, 2);
    expect(prompt2).toContain('FINAL ATTEMPT - CRITICAL JSON ERROR:');
    expect(prompt2).toContain('final retry');
    expect(prompt2).toContain('EXAMPLE:');
  });

  test('should simulate retry logic behavior with real functions', () => {
    const invalidResponses = [
      'Hello, this is not JSON',
      '```json\n{"incomplete":',
      'Another invalid response'
    ];
    const schema = '{"result": "string"}';
    
    // Test that each retry creates the appropriate correction prompt
    invalidResponses.forEach((response, index) => {
      const prompt = schemaUtils.createJsonCorrectionPrompt(response, schema, 'Invalid JSON', index);
      
      if (index === 0) {
        expect(prompt).toContain('CRITICAL JSON ERROR:');
      } else if (index === 1) {
        expect(prompt).toContain('URGENT - JSON PARSING FAILED:');
      } else if (index === 2) {
        expect(prompt).toContain('FINAL ATTEMPT - CRITICAL JSON ERROR:');
      }
      
      expect(prompt).toContain(response.substring(0, 50)); // Should contain truncated response
      expect(prompt).toContain(schema);
      expect(prompt).toContain('Invalid JSON');
    });
  });

  test('should test enhanced JSON extraction patterns', () => {
    const testCases = [
      {
        input: 'AI: Here is your JSON: ```json\n{"result": "success"}\n```',
        expected: '{"result": "success"}',
        description: 'should extract from code blocks with text before'
      },
      {
        input: 'Response:\n{"data": "value"}',
        expected: '{"data": "value"}',
        description: 'should extract from minimal text prefix'
      },
      {
        input: '```\n[{"item": 1}, {"item": 2}]\n```',
        expected: '[{"item": 1}, {"item": 2}]',
        description: 'should extract array from unmarked code block'
      }
    ];

    testCases.forEach(({ input, expected, description }) => {
      const result = schemaUtils.cleanSchemaResponse(input);
      expect(result).toBe(expected);
    });
  });
});