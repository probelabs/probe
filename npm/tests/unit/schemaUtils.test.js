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
  createMermaidCorrectionPrompt,
  generateExampleFromSchema,
  isSimpleTextWrapperSchema,
  tryAutoWrapForSimpleSchema
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

    // Tests for <result> tag stripping (fixes GitHub issue #336)
    describe('<result> tag stripping', () => {
      test('should strip <result> wrapper from JSON content', () => {
        const input = '<result>{"intent": "code_help", "topic": "How does rate limiting work?"}</result>';
        const result = cleanSchemaResponse(input);

        expect(result).not.toContain('<result>');
        expect(result).not.toContain('</result>');
        expect(() => JSON.parse(result)).not.toThrow();

        const parsed = JSON.parse(result);
        expect(parsed.intent).toBe('code_help');
        expect(parsed.topic).toBe('How does rate limiting work?');
      });

      test('should strip <result> wrapper with whitespace around JSON', () => {
        const input = '<result>\n  {"key": "value"}\n</result>';
        const result = cleanSchemaResponse(input);

        expect(result).toBe('{"key": "value"}');
        expect(() => JSON.parse(result)).not.toThrow();
      });

      test('should strip <result> wrapper with multiline JSON', () => {
        const input = `<result>
{
  "intent": "code_help",
  "topic": "Rate limiting",
  "details": ["Redis", "Gateway"]
}
</result>`;
        const result = cleanSchemaResponse(input);

        expect(result).not.toContain('<result>');
        expect(result).not.toContain('</result>');
        expect(() => JSON.parse(result)).not.toThrow();

        const parsed = JSON.parse(result);
        expect(parsed.intent).toBe('code_help');
        expect(parsed.details).toEqual(['Redis', 'Gateway']);
      });

      test('should handle <result> wrapper containing code block', () => {
        // Some models might wrap a code block in <result> tags
        const input = '<result>\n```json\n{"test": "value"}\n```\n</result>';
        const result = cleanSchemaResponse(input);

        // Should strip both <result> and code block
        expect(result).not.toContain('<result>');
        expect(result).not.toContain('</result>');
        expect(result).not.toContain('```');
        expect(() => JSON.parse(result)).not.toThrow();
      });

      test('should not strip partial <result> tags', () => {
        // Only opening tag - should not be stripped
        const input1 = '<result>{"test": "value"}';
        expect(cleanSchemaResponse(input1)).toBe(input1);

        // Only closing tag - should not be stripped
        const input2 = '{"test": "value"}</result>';
        expect(cleanSchemaResponse(input2)).toBe(input2);
      });

      test('should not strip <result> when embedded in other content', () => {
        const input = 'Here is the result: <result>{"test": "value"}</result> as requested';
        expect(cleanSchemaResponse(input)).toBe(input);
      });

      test('should handle nested <result> tags correctly', () => {
        // The regex should match the outermost <result> tags
        const input = '<result><result>{"nested": true}</result></result>';
        const result = cleanSchemaResponse(input);

        // After stripping outer tags, inner content should remain
        // Then recursive call strips inner tags
        expect(result).toBe('{"nested": true}');
      });

      test('should strip <result> wrapper from array JSON', () => {
        const input = '<result>[{"id": 1}, {"id": 2}]</result>';
        const result = cleanSchemaResponse(input);

        expect(result).toBe('[{"id": 1}, {"id": 2}]');
        expect(() => JSON.parse(result)).not.toThrow();
      });

      test('should preserve <result> inside JSON string values', () => {
        // <result> appearing as content inside a JSON value should be preserved
        const input = '{"message": "The <result> tag was found"}';
        const result = cleanSchemaResponse(input);

        expect(result).toBe('{"message": "The <result> tag was found"}');
      });
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

    // Schema validation tests
    test('should validate JSON against schema with required fields', () => {
      const schema = {
        type: 'object',
        properties: {
          name: { type: 'string' },
          age: { type: 'number' }
        },
        required: ['name', 'age']
      };

      const validJson = '{"name": "John", "age": 30}';
      const result = validateJsonResponse(validJson, { schema });
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual({ name: 'John', age: 30 });
    });

    test('should reject JSON missing required fields', () => {
      const schema = {
        type: 'object',
        properties: {
          name: { type: 'string' },
          age: { type: 'number' }
        },
        required: ['name', 'age']
      };

      const invalidJson = '{"name": "John"}';
      const result = validateJsonResponse(invalidJson, { schema });
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('Schema validation failed');
      expect(result.schemaErrors).toBeDefined();
      expect(result.formattedErrors).toBeDefined();
      expect(result.formattedErrors.some(e => e.includes('age'))).toBe(true);
    });

    test('should reject JSON with wrong field types', () => {
      const schema = {
        type: 'object',
        properties: {
          name: { type: 'string' },
          age: { type: 'number' }
        }
      };

      const invalidJson = '{"name": "John", "age": "thirty"}';
      const result = validateJsonResponse(invalidJson, { schema });
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('Schema validation failed');
      expect(result.formattedErrors.some(e => e.includes('number') || e.includes('age'))).toBe(true);
    });

    test('should reject JSON with additional properties when not allowed', () => {
      const schema = {
        type: 'object',
        properties: {
          name: { type: 'string' }
        },
        additionalProperties: false
      };

      const invalidJson = '{"name": "John", "extra": "field"}';
      const result = validateJsonResponse(invalidJson, { schema });
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('Schema validation failed');
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    test('should allow JSON with additional properties when allowed', () => {
      const schema = {
        type: 'object',
        properties: {
          name: { type: 'string' }
        },
        additionalProperties: true
      };

      const validJson = '{"name": "John", "extra": "field"}';
      const result = validateJsonResponse(validJson, { schema });
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual({ name: 'John', extra: 'field' });
    });

    test('should validate arrays against schema', () => {
      const schema = {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            id: { type: 'number' },
            name: { type: 'string' }
          },
          required: ['id']
        }
      };

      const validJson = '[{"id": 1, "name": "Item 1"}, {"id": 2}]';
      const result = validateJsonResponse(validJson, { schema });
      expect(result.isValid).toBe(true);
    });

    test('should reject invalid array items', () => {
      const schema = {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            id: { type: 'number' }
          },
          required: ['id']
        }
      };

      const invalidJson = '[{"id": 1}, {"name": "missing id"}]';
      const result = validateJsonResponse(invalidJson, { schema });
      expect(result.isValid).toBe(false);
      expect(result.schemaErrors).toBeDefined();
    });

    test('should accept schema as string', () => {
      const schemaString = '{"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}';
      const validJson = '{"name": "John"}';
      const result = validateJsonResponse(validJson, { schema: schemaString });
      expect(result.isValid).toBe(true);
    });

    test('should handle invalid schema gracefully', () => {
      const invalidSchema = 'not valid json';
      const validJson = '{"name": "John"}';
      const result = validateJsonResponse(validJson, { schema: invalidSchema });
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('Invalid schema provided');
      expect(result.schemaError).toBeDefined();
    });

    test('should validate nested object schemas', () => {
      const schema = {
        type: 'object',
        properties: {
          user: {
            type: 'object',
            properties: {
              name: { type: 'string' },
              email: { type: 'string' }
            },
            required: ['name'],
            additionalProperties: false
          }
        },
        required: ['user']
      };

      const validJson = '{"user": {"name": "John", "email": "john@example.com"}}';
      const result = validateJsonResponse(validJson, { schema });
      expect(result.isValid).toBe(true);
    });

    test('should provide detailed error messages for nested violations', () => {
      const schema = {
        type: 'object',
        properties: {
          user: {
            type: 'object',
            properties: {
              name: { type: 'string' }
            },
            required: ['name'],
            additionalProperties: false
          }
        }
      };

      const invalidJson = '{"user": {"age": 30}}';
      const result = validateJsonResponse(invalidJson, { schema });
      expect(result.isValid).toBe(false);
      expect(result.formattedErrors).toBeDefined();
      expect(result.errorSummary).toContain('Schema validation failed');
    });

    test('should work without schema (backward compatibility)', () => {
      const validJson = '{"name": "John", "extra": "field"}';
      const result = validateJsonResponse(validJson);
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual({ name: 'John', extra: 'field' });
    });

    // Strict schema mode tests (automatic additionalProperties enforcement)
    test('should automatically enforce additionalProperties on nested objects in strict mode', () => {
      const schema = {
        type: 'object',
        properties: {
          user: {
            type: 'object',
            properties: {
              name: { type: 'string' }
            }
            // No additionalProperties specified - should be auto-added
          }
        }
        // No additionalProperties on root either - should be auto-added
      };

      const invalidJson = '{"user": {"name": "John", "age": 30}}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      expect(result.isValid).toBe(false);
      expect(result.error).toBe('Schema validation failed');
      // Should reject the extra 'age' field in nested object
      expect(result.formattedErrors.some(e => e.includes('age'))).toBe(true);
    });

    test('should allow disabling strict mode to permit additional properties', () => {
      const schema = {
        type: 'object',
        properties: {
          user: {
            type: 'object',
            properties: {
              name: { type: 'string' }
            }
          }
        }
      };

      const jsonWithExtra = '{"user": {"name": "John", "age": 30}}';
      const result = validateJsonResponse(jsonWithExtra, { schema, strictSchema: false });

      // Should allow extra fields when strictSchema is disabled
      expect(result.isValid).toBe(true);
      expect(result.parsed.user.age).toBe(30);
    });

    test('should respect explicit additionalProperties: true even in strict mode', () => {
      const schema = {
        type: 'object',
        properties: {
          user: {
            type: 'object',
            properties: {
              name: { type: 'string' }
            },
            additionalProperties: true  // Explicitly allow
          }
        }
      };

      const jsonWithExtra = '{"user": {"name": "John", "age": 30}}';
      const result = validateJsonResponse(jsonWithExtra, { schema, strictSchema: true });

      // Should respect explicit additionalProperties: true
      expect(result.isValid).toBe(true);
      expect(result.parsed.user.age).toBe(30);
    });

    test('should enforce strict mode on deeply nested objects', () => {
      const schema = {
        type: 'object',
        properties: {
          level1: {
            type: 'object',
            properties: {
              level2: {
                type: 'object',
                properties: {
                  level3: {
                    type: 'object',
                    properties: {
                      name: { type: 'string' }
                    }
                  }
                }
              }
            }
          }
        }
      };

      const invalidJson = '{"level1": {"level2": {"level3": {"name": "test", "extra": "field"}}}}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    test('should enforce strict mode on array items', () => {
      const schema = {
        type: 'object',
        properties: {
          items: {
            type: 'array',
            items: {
              type: 'object',
              properties: {
                id: { type: 'number' }
              }
              // No additionalProperties - should be enforced
            }
          }
        }
      };

      const invalidJson = '{"items": [{"id": 1, "extra": "field"}]}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    test('strict mode should be enabled by default', () => {
      const schema = {
        type: 'object',
        properties: {
          name: { type: 'string' }
        }
      };

      const invalidJson = '{"name": "John", "extra": "field"}';
      // Don't specify strictSchema - should default to true
      const result = validateJsonResponse(invalidJson, { schema });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    test('should enforce strict mode with oneOf schemas', () => {
      const schema = {
        type: 'object',
        properties: {
          data: {
            oneOf: [
              {
                type: 'object',
                properties: {
                  type: { type: 'string', const: 'user' },
                  name: { type: 'string' }
                },
                required: ['type', 'name']
              },
              {
                type: 'object',
                properties: {
                  type: { type: 'string', const: 'product' },
                  price: { type: 'number' }
                },
                required: ['type', 'price']
              }
            ]
          }
        }
      };

      const invalidJson = '{"data": {"type": "user", "name": "John", "extra": "field"}}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    test('should enforce strict mode with anyOf schemas', () => {
      const schema = {
        type: 'object',
        properties: {
          value: {
            anyOf: [
              {
                type: 'object',
                properties: {
                  num: { type: 'number' }
                }
              },
              {
                type: 'object',
                properties: {
                  str: { type: 'string' }
                }
              }
            ]
          }
        }
      };

      const invalidJson = '{"value": {"num": 42, "extra": "field"}}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    test('should enforce strict mode with allOf schemas', () => {
      const schema = {
        type: 'object',
        properties: {
          entity: {
            allOf: [
              {
                type: 'object',
                properties: {
                  id: { type: 'number' }
                }
              },
              {
                type: 'object',
                properties: {
                  name: { type: 'string' }
                }
              }
            ]
          }
        }
      };

      const invalidJson = '{"entity": {"id": 1, "name": "Test", "extra": "field"}}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    test('should enforce strict mode with schema definitions', () => {
      const schema = {
        type: 'object',
        definitions: {
          address: {
            type: 'object',
            properties: {
              street: { type: 'string' },
              city: { type: 'string' }
            }
          }
        },
        properties: {
          home: { $ref: '#/definitions/address' }
        }
      };

      const invalidJson = '{"home": {"street": "123 Main St", "city": "NYC", "extra": "field"}}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    test('should enforce strict mode with $defs (JSON Schema 2019-09)', () => {
      const schema = {
        type: 'object',
        $defs: {
          person: {
            type: 'object',
            properties: {
              name: { type: 'string' },
              age: { type: 'number' }
            }
          }
        },
        properties: {
          employee: { $ref: '#/$defs/person' }
        }
      };

      const invalidJson = '{"employee": {"name": "John", "age": 30, "extra": "field"}}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    test('should enforce strict mode on array items with tuple validation', () => {
      const schema = {
        type: 'object',
        properties: {
          coordinates: {
            type: 'array',
            items: [
              {
                type: 'object',
                properties: {
                  x: { type: 'number' }
                }
              },
              {
                type: 'object',
                properties: {
                  y: { type: 'number' }
                }
              }
            ]
          }
        }
      };

      const invalidJson = '{"coordinates": [{"x": 10, "extra": "bad"}, {"y": 20}]}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    test('should handle schemas without type specified', () => {
      const schema = {
        properties: {
          user: {
            properties: {
              name: { type: 'string' }
            }
            // No type: 'object' specified, but has properties
          }
        }
      };

      const invalidJson = '{"user": {"name": "John", "extra": "field"}}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      // Without explicit type: 'object', strict mode won't add additionalProperties
      // This is expected behavior - schemas should be well-formed
      expect(result.isValid).toBe(true);
    });

    test('should not modify schemas with explicit additionalProperties: false', () => {
      const schema = {
        type: 'object',
        properties: {
          data: {
            type: 'object',
            properties: {
              id: { type: 'number' }
            },
            additionalProperties: false  // Already set
          }
        },
        additionalProperties: false  // Already set
      };

      const invalidJson = '{"data": {"id": 1, "extra": "field"}}';
      const result = validateJsonResponse(invalidJson, { schema, strictSchema: true });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors.some(e => e.includes('extra'))).toBe(true);
    });

    // Enhanced error message tests
    test('should provide crisp error messages with dot notation paths', () => {
      const schema = {
        type: 'object',
        properties: {
          user: {
            type: 'object',
            properties: {
              profile: {
                type: 'object',
                properties: {
                  name: { type: 'string' }
                },
                required: ['name']
              }
            }
          }
        }
      };

      const invalidJson = '{"user": {"profile": {}}}';
      const result = validateJsonResponse(invalidJson, { schema });

      expect(result.isValid).toBe(false);
      // Should use dot notation, not slashes
      expect(result.formattedErrors[0]).toContain("at 'user.profile'");
      expect(result.formattedErrors[0]).not.toContain('/user/profile');
      // Should have actionable suggestion
      expect(result.formattedErrors[0]).toContain("Missing required field 'name'");
      expect(result.formattedErrors[0]).toContain("Add 'name' to this object");
    });

    test('should show actual values in type errors', () => {
      const schema = {
        type: 'object',
        properties: {
          age: { type: 'number' }
        }
      };

      const invalidJson = '{"age": "thirty"}';
      const result = validateJsonResponse(invalidJson, { schema });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors[0]).toContain('Wrong type');
      expect(result.formattedErrors[0]).toContain('expected number');
      expect(result.formattedErrors[0]).toContain('got string');
      expect(result.formattedErrors[0]).toContain('value: "thirty"');
      expect(result.formattedErrors[0]).toContain('Change value to number type');
    });

    test('should provide actionable suggestions for additional properties', () => {
      const schema = {
        type: 'object',
        properties: {
          name: { type: 'string' }
        },
        additionalProperties: false
      };

      const invalidJson = '{"name": "John", "extra": "field"}';
      const result = validateJsonResponse(invalidJson, { schema });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors[0]).toContain("Extra field 'extra' is not allowed");
      expect(result.formattedErrors[0]).toContain("Remove 'extra' or add it to the schema");
    });

    test('should show allowed values for enum violations', () => {
      const schema = {
        type: 'object',
        properties: {
          role: { type: 'string', enum: ['admin', 'user', 'guest'] }
        }
      };

      const invalidJson = '{"role": "superadmin"}';
      const result = validateJsonResponse(invalidJson, { schema });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors[0]).toContain('Invalid value "superadmin"');
      expect(result.formattedErrors[0]).toContain('Allowed: "admin", "user", "guest"');
      expect(result.formattedErrors[0]).toContain('Use one of the allowed values');
    });

    test('should show constraint details for range violations', () => {
      const schema = {
        type: 'object',
        properties: {
          age: { type: 'number', minimum: 0, maximum: 150 }
        }
      };

      const invalidJson = '{"age": 200}';
      const result = validateJsonResponse(invalidJson, { schema });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors[0]).toContain('at \'age\'');
      expect(result.formattedErrors[0]).toContain('Value 200');
      expect(result.formattedErrors[0]).toContain('150');
      expect(result.formattedErrors[0]).toContain('Adjust value to meet constraint');
    });

    test('should show current length for string length violations', () => {
      const schema = {
        type: 'object',
        properties: {
          username: { type: 'string', minLength: 3, maxLength: 20 }
        }
      };

      const invalidJson = '{"username": "ab"}';
      const result = validateJsonResponse(invalidJson, { schema });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors[0]).toContain('String length');
      expect(result.formattedErrors[0]).toContain('current: 2');
      expect(result.formattedErrors[0]).toContain('minLength: 3');
      expect(result.formattedErrors[0]).toContain('Adjust string length');
    });

    test('should handle root-level errors clearly', () => {
      const schema = {
        type: 'object',
        properties: {
          name: { type: 'string' }
        },
        additionalProperties: false
      };

      const invalidJson = '{"extra": "field"}';
      const result = validateJsonResponse(invalidJson, { schema });

      expect(result.isValid).toBe(false);
      expect(result.formattedErrors[0]).toContain('at \'<root>\'');
      expect(result.formattedErrors[0]).toContain("Extra field 'extra' is not allowed");
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
      expect(prompt).toContain('attempt_completion with ONLY valid JSON');
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
      expect(prompt).toContain('CORRECT:');
      expect(prompt).toContain('WRONG:');
    });

    test('should cap at highest strength level for retryCount > 2', () => {
      const invalidResponse = '{"test": value}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v in JSON';
      
      const prompt = createJsonCorrectionPrompt(invalidResponse, schema, error, 5);
      
      expect(prompt).toContain('FINAL ATTEMPT - CRITICAL JSON ERROR:');
    });

    test('should include full response without truncation', () => {
      // Full response is needed for AI to properly fix the issue
      const longResponse = 'Hello '.repeat(200) + '{"test": value}';
      const schema = '{"test": "string"}';
      const error = 'Unexpected token v';

      const prompt = createJsonCorrectionPrompt(longResponse, schema, error, 0);

      // Should include the full response, not truncated
      expect(prompt).toContain(longResponse);
      expect(prompt).not.toContain('...');
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

  describe('generateExampleFromSchema', () => {
    test('should generate example from object schema with basic types', () => {
      const schema = {
        type: 'object',
        properties: {
          name: { type: 'string', description: 'User name' },
          age: { type: 'number' },
          active: { type: 'boolean' },
          tags: { type: 'array' }
        }
      };

      const result = generateExampleFromSchema(schema);

      expect(result).toEqual({
        name: 'User name',
        age: 0,
        active: false,
        tags: []
      });
    });

    test('should use description for string fields', () => {
      const schema = {
        type: 'object',
        properties: {
          message: { type: 'string', description: 'Hello world' },
          noDescription: { type: 'string' }
        }
      };

      const result = generateExampleFromSchema(schema);

      expect(result.message).toBe('Hello world');
      expect(result.noDescription).toBe('your answer here');
    });

    test('should handle nested objects', () => {
      const schema = {
        type: 'object',
        properties: {
          user: { type: 'object' },
          settings: { type: 'object' }
        }
      };

      const result = generateExampleFromSchema(schema);

      expect(result).toEqual({
        user: {},
        settings: {}
      });
    });

    test('should parse schema from JSON string', () => {
      const schema = JSON.stringify({
        type: 'object',
        properties: {
          refined: { type: 'boolean' },
          text: { type: 'string' }
        }
      });

      const result = generateExampleFromSchema(schema);

      expect(result).toEqual({
        refined: false,
        text: 'your answer here'
      });
    });

    test('should return null for non-object schema', () => {
      const schema = {
        type: 'string'
      };

      const result = generateExampleFromSchema(schema);

      expect(result).toBeNull();
    });

    test('should return null for schema without properties', () => {
      const schema = {
        type: 'object'
      };

      const result = generateExampleFromSchema(schema);

      expect(result).toBeNull();
    });

    test('should return null for invalid schema', () => {
      const schema = 'not valid json {';

      const result = generateExampleFromSchema(schema);

      expect(result).toBeNull();
    });

    test('should handle schema with multiple field types', () => {
      const schema = {
        type: 'object',
        properties: {
          id: { type: 'number' },
          name: { type: 'string', description: 'Product name' },
          inStock: { type: 'boolean' },
          categories: { type: 'array' },
          metadata: { type: 'object' }
        }
      };

      const result = generateExampleFromSchema(schema);

      expect(result).toEqual({
        id: 0,
        name: 'Product name',
        inStock: false,
        categories: [],
        metadata: {}
      });
    });

    test('should match the exact schema used in Visor refine check', () => {
      // This is the actual schema from the bug report
      const schema = {
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

      const result = generateExampleFromSchema(schema);

      expect(result).toEqual({
        refined: false,
        text: 'If refined=true, confirmation message. If refined=false, specific questions to ask.'
      });

      // Verify it's valid JSON when stringified
      expect(() => JSON.parse(JSON.stringify(result))).not.toThrow();
    });
  });

  describe('isSimpleTextWrapperSchema', () => {
    test('should detect simple {text: string} schema', () => {
      const result = isSimpleTextWrapperSchema('{text: string}');
      expect(result).toEqual({ fieldName: 'text' });
    });

    test('should detect {response: string} schema', () => {
      const result = isSimpleTextWrapperSchema('{response: string}');
      expect(result).toEqual({ fieldName: 'response' });
    });

    test('should detect {"text": "string"} with quotes', () => {
      const result = isSimpleTextWrapperSchema('{"text": "string"}');
      expect(result).toEqual({ fieldName: 'text' });
    });

    test('should detect schema with whitespace', () => {
      const result = isSimpleTextWrapperSchema('{ text : string }');
      expect(result).toEqual({ fieldName: 'text' });
    });

    test('should return null for complex schemas', () => {
      const complexSchema = '{"type": "object", "properties": {"text": {"type": "string"}, "count": {"type": "number"}}}';
      expect(isSimpleTextWrapperSchema(complexSchema)).toBeNull();
    });

    test('should return null for multi-field schemas', () => {
      const multiField = '{text: string, count: number}';
      expect(isSimpleTextWrapperSchema(multiField)).toBeNull();
    });

    test('should return null for null/undefined', () => {
      expect(isSimpleTextWrapperSchema(null)).toBeNull();
      expect(isSimpleTextWrapperSchema(undefined)).toBeNull();
    });

    test('should return null for non-string input', () => {
      expect(isSimpleTextWrapperSchema(123)).toBeNull();
      expect(isSimpleTextWrapperSchema({})).toBeNull();
    });

    test('should handle single quotes', () => {
      const result = isSimpleTextWrapperSchema("{'text': 'string'}");
      expect(result).toEqual({ fieldName: 'text' });
    });

    test('should detect full JSON Schema with required and description', () => {
      // This is the schema format that was failing before the fix (issue #423)
      const schema = '{"type":"object","properties":{"text":{"type":"string","description":"The response to the user"}},"required":["text"]}';
      const result = isSimpleTextWrapperSchema(schema);
      expect(result).toEqual({ fieldName: 'text' });
    });

    test('should detect full JSON Schema with additionalProperties', () => {
      const schema = '{"type":"object","properties":{"response":{"type":"string"}},"additionalProperties":false}';
      const result = isSimpleTextWrapperSchema(schema);
      expect(result).toEqual({ fieldName: 'response' });
    });

    test('should return null for JSON Schema with multiple string properties', () => {
      const schema = '{"type":"object","properties":{"text":{"type":"string"},"summary":{"type":"string"}}}';
      const result = isSimpleTextWrapperSchema(schema);
      expect(result).toBeNull();
    });

    test('should return null for JSON Schema with non-string property', () => {
      const schema = '{"type":"object","properties":{"count":{"type":"number"}}}';
      const result = isSimpleTextWrapperSchema(schema);
      expect(result).toBeNull();
    });
  });

  describe('tryAutoWrapForSimpleSchema', () => {
    test('should wrap plain text for {text: string} schema', () => {
      const response = 'This is a plain text response about the code analysis.';
      const schema = '{text: string}';

      const result = tryAutoWrapForSimpleSchema(response, schema);

      expect(result).toBe('{"text":"This is a plain text response about the code analysis."}');
      expect(() => JSON.parse(result)).not.toThrow();
    });

    test('should wrap plain text for {response: string} schema', () => {
      const response = 'Here is the analysis result.';
      const schema = '{response: string}';

      const result = tryAutoWrapForSimpleSchema(response, schema);

      expect(result).toBe('{"response":"Here is the analysis result."}');
    });

    test('should return null if response is already valid JSON', () => {
      const response = '{"text": "already valid"}';
      const schema = '{text: string}';

      const result = tryAutoWrapForSimpleSchema(response, schema);

      expect(result).toBeNull();
    });

    test('should return null for complex schemas', () => {
      const response = 'Plain text';
      const schema = '{"text": "string", "count": "number"}';

      const result = tryAutoWrapForSimpleSchema(response, schema);

      expect(result).toBeNull();
    });

    test('should handle long responses without truncation', () => {
      const longResponse = 'This is a very long response. '.repeat(100);
      const schema = '{text: string}';

      const result = tryAutoWrapForSimpleSchema(longResponse, schema);

      expect(result).not.toBeNull();
      const parsed = JSON.parse(result);
      expect(parsed.text).toBe(longResponse);
      expect(parsed.text.length).toBe(longResponse.length);
    });

    test('should handle responses with special characters', () => {
      const response = 'Response with "quotes" and\nnewlines and \ttabs';
      const schema = '{text: string}';

      const result = tryAutoWrapForSimpleSchema(response, schema);

      expect(result).not.toBeNull();
      expect(() => JSON.parse(result)).not.toThrow();
      const parsed = JSON.parse(result);
      expect(parsed.text).toBe(response);
    });

    test('should handle responses with unicode', () => {
      const response = 'Unicode: 你好 👋 émojis';
      const schema = '{text: string}';

      const result = tryAutoWrapForSimpleSchema(response, schema);

      expect(result).not.toBeNull();
      const parsed = JSON.parse(result);
      expect(parsed.text).toBe(response);
    });
  });
});