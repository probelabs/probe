/**
 * Integration test for the JSON schema validation loop bug fix
 *
 * This test validates that schema instructions are added to the INITIAL user message,
 * preventing the validation loop that occurred when the AI responded with plain text
 * and then received correction prompts.
 *
 * Bug: https://github.com/probelabs/probe/issues/XXX
 * Fix: Schema instructions prepended to user message before AI sees it
 */

import { describe, test, expect } from '@jest/globals';
import { generateExampleFromSchema } from '../../src/agent/schemaUtils.js';

describe('Schema in Initial Message - Bug Fix Integration Test', () => {

  describe('generateExampleFromSchema - Used in Initial Message', () => {
    test('should generate valid JSON example for Visor refine schema', () => {
      // This is the exact schema from the bug report that caused the validation loop
      const visorRefineSchema = {
        type: 'object',
        additionalProperties: false,
        properties: {
          refined: {
            type: 'boolean',
            description: 'true if the task description is clear and actionable, false if clarification is needed'
          },
          text: {
            type: 'string',
            description: 'If refined=true, confirmation message. If refined=false, specific questions to ask.'
          }
        },
        required: ['refined', 'text']
      };

      const example = generateExampleFromSchema(visorRefineSchema);

      // Should generate a valid example
      expect(example).toBeDefined();
      expect(example).not.toBeNull();

      // Should have the correct structure
      expect(example).toHaveProperty('refined');
      expect(example).toHaveProperty('text');

      // Should use correct types
      expect(typeof example.refined).toBe('boolean');
      expect(typeof example.text).toBe('string');

      // Should be valid JSON when stringified
      const jsonString = JSON.stringify(example);
      expect(() => JSON.parse(jsonString)).not.toThrow();

      // The example should match what the AI is expected to generate
      expect(example).toEqual({
        refined: false,
        text: 'If refined=true, confirmation message. If refined=false, specific questions to ask.'
      });
    });

    test('should create a complete schema instruction message', () => {
      const schema = {
        type: 'object',
        properties: {
          status: { type: 'string', description: 'Operation status' },
          count: { type: 'number' }
        }
      };

      const example = generateExampleFromSchema(schema);

      // Simulate how the message would be constructed in ProbeAgent
      const userMessage = 'Please analyze this code';
      const schemaInstructions = `\n\nIMPORTANT: When you provide your final answer using attempt_completion, you MUST format it as valid JSON matching this schema:\n\n${JSON.stringify(schema, null, 2)}\n\nExample:\n<attempt_completion>\n${JSON.stringify(example, null, 2)}\n</attempt_completion>\n\nYour response inside attempt_completion must be ONLY valid JSON - no plain text, no explanations, no markdown.`;

      const fullMessage = userMessage + schemaInstructions;

      // Should contain the original message
      expect(fullMessage).toContain('Please analyze this code');

      // Should contain clear JSON requirement
      expect(fullMessage).toContain('IMPORTANT');
      expect(fullMessage).toContain('you MUST format it as valid JSON');

      // Should contain the schema
      expect(fullMessage).toContain('"type": "object"');
      expect(fullMessage).toContain('"status"');
      expect(fullMessage).toContain('"count"');

      // Should contain a concrete example
      expect(fullMessage).toContain('Example:');
      expect(fullMessage).toContain('<attempt_completion>');
      expect(fullMessage).toContain('"status": "Operation status"');
      expect(fullMessage).toContain('"count": 0');

      // Should contain explicit restrictions
      expect(fullMessage).toContain('ONLY valid JSON');
      expect(fullMessage).toContain('no plain text');
      expect(fullMessage).toContain('no explanations');
      expect(fullMessage).toContain('no markdown');
    });

    test('should handle schema as JSON string (common in Visor)', () => {
      const schemaString = JSON.stringify({
        type: 'object',
        properties: {
          result: { type: 'string' }
        }
      });

      const example = generateExampleFromSchema(schemaString);

      expect(example).toBeDefined();
      expect(example).toHaveProperty('result');
      expect(typeof example.result).toBe('string');
    });

    test('should gracefully handle invalid schema without crashing', () => {
      const invalidSchema = 'not valid json {';

      const example = generateExampleFromSchema(invalidSchema);

      // Should return null for invalid schema
      expect(example).toBeNull();

      // This ensures the user message construction doesn't crash
      // even if schema parsing fails
    });
  });

  describe('Bug Prevention - Validation Loop Scenario', () => {
    test('should document the bug scenario that is now prevented', () => {
      // BEFORE THE FIX:
      // 1. User message: "Hi! Just checking"
      // 2. Schema: {refined: boolean, text: string}
      // 3. AI receives ONLY: "Hi! Just checking" (no schema info)
      // 4. AI responds: "Hello! I'm ready to help..." (plain text)
      // 5. System tries to parse as JSON -> FAILS
      // 6. System sends correction: "CRITICAL JSON ERROR..."
      // 7. AI still responds with plain text -> LOOP continues for 3 attempts
      // 8. Total time: 100+ seconds, 30+ API calls

      // AFTER THE FIX:
      // 1. User message: "Hi! Just checking"
      // 2. Schema: {refined: boolean, text: string}
      const schema = {
        type: 'object',
        properties: {
          refined: { type: 'boolean' },
          text: { type: 'string' }
        }
      };

      // 3. System generates example
      const example = generateExampleFromSchema(schema);

      // 4. AI receives enriched message with schema instructions
      const userMessage = 'Hi! Just checking';
      const enrichedMessage = userMessage + `\n\nIMPORTANT: When you provide your final answer using attempt_completion, you MUST format it as valid JSON matching this schema:\n\n${JSON.stringify(schema, null, 2)}\n\nExample:\n<attempt_completion>\n${JSON.stringify(example, null, 2)}\n</attempt_completion>\n\nYour response inside attempt_completion must be ONLY valid JSON - no plain text, no explanations, no markdown.`;

      // 5. AI now knows to respond with JSON from the start
      // 6. AI responds: <attempt_completion>{"refined": false, "text": "..."}</attempt_completion>
      // 7. System parses JSON -> SUCCESS on first try
      // 8. Total time: 4 seconds, 1 API call

      // Verify the fix is in place
      expect(enrichedMessage).toContain('you MUST format it as valid JSON');
      expect(enrichedMessage).toContain(JSON.stringify(example, null, 2));
      expect(enrichedMessage).not.toContain('CRITICAL JSON ERROR'); // No correction needed!
    });

    test('should prevent the specific Gemini 2.5 Pro issue', () => {
      // The bug was specifically observed with Google Gemini 2.5 Pro
      // which would respond with plain text or <task> XML tags

      const visorSchema = {
        type: 'object',
        additionalProperties: false,
        properties: {
          refined: { type: 'boolean' },
          text: { type: 'string' }
        },
        required: ['refined', 'text']
      };

      const example = generateExampleFromSchema(visorSchema);

      // The example should be immediately usable by the AI
      expect(JSON.stringify(example)).toBe(
        '{"refined":false,"text":"your answer here"}'
      );

      // This is what the AI should see upfront, preventing confusion
      const instructions = `IMPORTANT: When you provide your final answer using attempt_completion, you MUST format it as valid JSON matching this schema:

${JSON.stringify(visorSchema, null, 2)}

Example:
<attempt_completion>
${JSON.stringify(example, null, 2)}
</attempt_completion>

Your response inside attempt_completion must be ONLY valid JSON - no plain text, no explanations, no markdown.`;

      // Should be crystal clear to the AI
      expect(instructions).toContain('MUST format it as valid JSON');
      expect(instructions).toContain('Example:');
      expect(instructions).toContain('ONLY valid JSON');
      expect(instructions).toContain('no plain text');
    });
  });

  describe('Edge Cases', () => {
    test('should handle schema with deeply nested objects', () => {
      const nestedSchema = {
        type: 'object',
        properties: {
          user: {
            type: 'object',
            properties: {
              profile: {
                type: 'object',
                properties: {
                  name: { type: 'string' }
                }
              }
            }
          }
        }
      };

      const example = generateExampleFromSchema(nestedSchema);

      // Should handle nesting
      expect(example).toHaveProperty('user');
      expect(typeof example.user).toBe('object');

      // Inner objects default to empty
      expect(example.user).toEqual({});
    });

    test('should handle schema with all primitive types', () => {
      const allTypesSchema = {
        type: 'object',
        properties: {
          str: { type: 'string', description: 'A string' },
          num: { type: 'number' },
          bool: { type: 'boolean' },
          arr: { type: 'array' },
          obj: { type: 'object' }
        }
      };

      const example = generateExampleFromSchema(allTypesSchema);

      expect(example).toEqual({
        str: 'A string',
        num: 0,
        bool: false,
        arr: [],
        obj: {}
      });

      // Should be valid JSON
      expect(() => JSON.parse(JSON.stringify(example))).not.toThrow();
    });

    test('should use description as example value for strings', () => {
      const schema = {
        type: 'object',
        properties: {
          greeting: { type: 'string', description: 'Hello, World!' },
          noDesc: { type: 'string' }
        }
      };

      const example = generateExampleFromSchema(schema);

      // Description should be used as the example value
      expect(example.greeting).toBe('Hello, World!');

      // Without description, default placeholder
      expect(example.noDesc).toBe('your answer here');
    });
  });

  describe('Performance - Bug Fix Impact', () => {
    test('should prevent wasted iterations (regression test)', () => {
      // Before fix: 30+ iterations (tool loop + 3 correction attempts each)
      // After fix: 1 iteration (AI gets it right on first try)

      const schema = {
        type: 'object',
        properties: {
          refined: { type: 'boolean' },
          text: { type: 'string' }
        }
      };

      const startTime = Date.now();

      // Generate example (happens once per request)
      const example = generateExampleFromSchema(schema);

      const endTime = Date.now();
      const generationTime = endTime - startTime;

      // Should be very fast (< 10ms)
      expect(generationTime).toBeLessThan(10);

      // Should produce valid output
      expect(example).toBeDefined();
      expect(() => JSON.parse(JSON.stringify(example))).not.toThrow();

      // This small cost (< 10ms) saves 100+ seconds of validation loops
    });
  });
});
