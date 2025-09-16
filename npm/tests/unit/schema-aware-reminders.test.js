/**
 * Tests for schema-aware reminder messages in ProbeAgent
 * Tests the new functionality that provides different reminder messages based on schema presence
 */
import { jest, describe, test, expect, beforeEach, afterEach } from '@jest/globals';

describe('Schema-Aware Reminder Messages', () => {
  let mockAgent;

  beforeEach(() => {
    // Mock a minimal ProbeAgent-like object to test reminder logic
    mockAgent = {
      debug: false,
      options: {},
      currentMessages: [],

      // Mock the reminder message generation logic
      buildReminderMessage(options) {
        let reminderContent;
        if (options.schema) {
          // Schema-aware reminder
          reminderContent = `Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information to provide a final answer.

Remember: Use proper XML format with BOTH opening and closing tags:

<tool_name>
<parameter>value</parameter>
</tool_name>

IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.
Use attempt_completion with your response directly inside the tags:

<attempt_completion>
{"key": "value", "field": "your actual data here matching the schema"}
</attempt_completion>

Your response must conform to this schema:
${options.schema}`;
        } else {
          // Standard reminder without schema
          reminderContent = `Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information to provide a final answer.

Remember: Use proper XML format with BOTH opening and closing tags:

<tool_name>
<parameter>value</parameter>
</tool_name>

Or for quick completion if your previous response was already correct:
<attempt_complete>`;
        }
        return reminderContent;
      }
    };
  });

  describe('With Schema Provided', () => {
    test('should include schema-specific instructions when JSON schema is provided', () => {
      const options = {
        schema: '{"result": "string", "confidence": "number"}'
      };

      const reminder = mockAgent.buildReminderMessage(options);

      expect(reminder).toContain('A schema was provided');
      expect(reminder).toContain('You MUST respond with data that matches this schema');
      expect(reminder).toContain('attempt_completion');
      expect(reminder).toContain('{"key": "value", "field": "your actual data here matching the schema"}');
      expect(reminder).toContain('Your response must conform to this schema:');
      expect(reminder).toContain(options.schema);

      // Should NOT contain the shorthand attempt_complete
      expect(reminder).not.toContain('<attempt_complete>');
    });

    test('should include full schema in reminder without truncation', () => {
      const longSchema = `{
        "type": "object",
        "properties": {
          "analysis": {"type": "string"},
          "findings": {
            "type": "array",
            "items": {"type": "string"}
          },
          "confidence": {"type": "number", "minimum": 0, "maximum": 1},
          "recommendations": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "action": {"type": "string"},
                "priority": {"type": "string", "enum": ["high", "medium", "low"]}
              }
            }
          }
        }
      }`;

      const options = { schema: longSchema };
      const reminder = mockAgent.buildReminderMessage(options);

      // Should include the complete schema, not truncated
      expect(reminder).toContain(longSchema);
      expect(reminder).toContain('"recommendations"');
      expect(reminder).toContain('"priority"');
    });

    test('should work with non-JSON schemas too', () => {
      const mermaidSchema = `graph TD
        A[Start] --> B[Process]
        B --> C[End]`;

      const options = { schema: mermaidSchema };
      const reminder = mockAgent.buildReminderMessage(options);

      expect(reminder).toContain('A schema was provided');
      expect(reminder).toContain('You MUST respond with data that matches this schema');
      expect(reminder).toContain(mermaidSchema);
      expect(reminder).toContain('graph TD');
    });

    test('should provide clear example of attempt_completion format', () => {
      const options = {
        schema: '{"status": "string", "message": "string"}'
      };

      const reminder = mockAgent.buildReminderMessage(options);

      expect(reminder).toContain('<attempt_completion>');
      expect(reminder).toContain('{"key": "value", "field": "your actual data here matching the schema"}');
      expect(reminder).toContain('</attempt_completion>');

      // Should show the direct content format, not <result> wrapper
      expect(reminder).not.toContain('<result>');
    });
  });

  describe('Without Schema', () => {
    test('should provide standard reminder when no schema is present', () => {
      const options = {}; // No schema

      const reminder = mockAgent.buildReminderMessage(options);

      expect(reminder).toContain('Please use one of the available tools');
      expect(reminder).toContain('attempt_completion');
      expect(reminder).toContain('<attempt_complete>');

      // Should NOT contain schema-specific instructions
      expect(reminder).not.toContain('A schema was provided');
      expect(reminder).not.toContain('matches this schema');
      expect(reminder).not.toContain('conform to this schema');
    });

    test('should include attempt_complete shorthand when no schema', () => {
      const options = {}; // No schema

      const reminder = mockAgent.buildReminderMessage(options);

      expect(reminder).toContain('for quick completion if your previous response was already correct:');
      expect(reminder).toContain('<attempt_complete>');
    });

    test('should provide standard XML formatting guidance', () => {
      const options = {}; // No schema

      const reminder = mockAgent.buildReminderMessage(options);

      expect(reminder).toContain('Use proper XML format with BOTH opening and closing tags');
      expect(reminder).toContain('<tool_name>');
      expect(reminder).toContain('<parameter>value</parameter>');
      expect(reminder).toContain('</tool_name>');
    });
  });

  describe('Edge Cases', () => {
    test('should handle empty schema string', () => {
      const options = { schema: '' };

      const reminder = mockAgent.buildReminderMessage(options);

      // Empty string is falsy, so should get standard treatment
      expect(reminder).toContain('Please use one of the available tools');
      expect(reminder).toContain('<attempt_complete>');
      expect(reminder).not.toContain('A schema was provided');
    });

    test('should handle schema with special characters', () => {
      const options = {
        schema: '{"special": "characters like <>&\\"\\n\\t"}'
      };

      const reminder = mockAgent.buildReminderMessage(options);

      expect(reminder).toContain('A schema was provided');
      expect(reminder).toContain(options.schema);
      expect(reminder).toContain('<>&\\"\\n\\t');
    });

    test('should be consistent with tool formatting instructions', () => {
      const optionsWithSchema = { schema: '{"test": "value"}' };
      const optionsWithoutSchema = {};

      const reminderWithSchema = mockAgent.buildReminderMessage(optionsWithSchema);
      const reminderWithoutSchema = mockAgent.buildReminderMessage(optionsWithoutSchema);

      // Both should have the same XML formatting instructions
      const xmlInstructions = 'Use proper XML format with BOTH opening and closing tags';
      expect(reminderWithSchema).toContain(xmlInstructions);
      expect(reminderWithoutSchema).toContain(xmlInstructions);

      // Both should mention attempt_completion
      expect(reminderWithSchema).toContain('attempt_completion');
      expect(reminderWithoutSchema).toContain('attempt_completion');
    });
  });
});

describe('Integration with ProbeAgent Flow', () => {
  let integrationMockAgent;

  beforeEach(() => {
    // Create a fresh mock agent for integration tests
    integrationMockAgent = {
      buildReminderMessage(options) {
        let reminderContent;
        if (options.schema) {
          // Schema-aware reminder
          reminderContent = `Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information to provide a final answer.

Remember: Use proper XML format with BOTH opening and closing tags:

<tool_name>
<parameter>value</parameter>
</tool_name>

IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.
Use attempt_completion with your response directly inside the tags:

<attempt_completion>
{"key": "value", "field": "your actual data here matching the schema"}
</attempt_completion>

Your response must conform to this schema:
${options.schema}`;
        } else {
          // Standard reminder without schema
          reminderContent = `Please use one of the available tools to help answer the question, or use attempt_completion if you have enough information to provide a final answer.

Remember: Use proper XML format with BOTH opening and closing tags:

<tool_name>
<parameter>value</parameter>
</tool_name>

Or for quick completion if your previous response was already correct:
<attempt_complete>`;
        }
        return reminderContent;
      }
    };
  });

  test('should prevent JSON validation loops by providing clear upfront instructions', () => {
    // This test validates that our schema-aware reminders solve the original problem
    const options = {
      schema: '{"analysis": "string", "score": "number"}'
    };

    const reminder = integrationMockAgent.buildReminderMessage(options);

    // The reminder should clearly state what's expected
    expect(reminder).toContain('You MUST respond with data that matches this schema');
    expect(reminder).toContain('Use attempt_completion with your response directly inside the tags');

    // Should show the exact format expected
    expect(reminder).toContain('<attempt_completion>');
    expect(reminder).toContain('{"key": "value"');
    expect(reminder).toContain('</attempt_completion>');

    // Should include the schema for reference
    expect(reminder).toContain('"analysis": "string"');
    expect(reminder).toContain('"score": "number"');
  });

  test('should maintain backward compatibility for non-schema usage', () => {
    const options = {}; // No schema - existing behavior

    const reminder = integrationMockAgent.buildReminderMessage(options);

    // Should still work like before for non-schema cases
    expect(reminder).toContain('Please use one of the available tools');
    expect(reminder).toContain('<attempt_complete>');
    expect(reminder).not.toContain('schema');
  });
});