/**
 * Integration tests for the complete validation flow
 * Tests how JSON and Mermaid validation work together
 */

import { describe, test, expect, jest } from '@jest/globals';
import {
  isJsonSchema,
  isMermaidSchema,
  validateJsonResponse,
  validateMermaidDiagram,
  validateMermaidResponse,
  cleanSchemaResponse,
  processSchemaResponse,
  createJsonCorrectionPrompt,
  createMermaidCorrectionPrompt,
  extractMermaidFromMarkdown
} from '../../src/agent/schemaUtils.js';

describe('Validation Flow Integration', () => {
  describe('Schema Detection Priority', () => {
    test('should detect both JSON and Mermaid schemas correctly', () => {
      const jsonOnlySchema = '{"users": [{"name": "string"}]}';
      const mermaidOnlySchema = 'Create a mermaid flowchart diagram';
      const mixedSchema = 'Create a JSON response with a mermaid diagram showing the flow';
      
      expect(isJsonSchema(jsonOnlySchema)).toBe(true);
      expect(isMermaidSchema(jsonOnlySchema)).toBe(false);
      
      expect(isJsonSchema(mermaidOnlySchema)).toBe(false);
      expect(isMermaidSchema(mermaidOnlySchema)).toBe(true);
      
      expect(isJsonSchema(mixedSchema)).toBe(true);
      expect(isMermaidSchema(mixedSchema)).toBe(true);
    });

    test('should prioritize validation order (Mermaid first, then JSON)', async () => {
      const response = `{
        "description": "Here's the process flow",
        "diagram": "\`\`\`mermaid\\ngraph TD\\n    A[Start] --> B[Process]\\n    B --> C[End]\\n\`\`\`"
      }`;

      const schema = 'Return JSON with a mermaid diagram field';

      // Test that we can detect both types
      expect(isMermaidSchema(schema)).toBe(true);
      expect(isJsonSchema(schema)).toBe(true);

      // Test Mermaid validation first - response contains diagram in JSON field, not in markdown blocks
      const mermaidValidation = await validateMermaidResponse(response);
      expect(mermaidValidation.isValid).toBe(false); // No mermaid code blocks found in this JSON structure

      // Test JSON validation second
      const cleanedResponse = cleanSchemaResponse(response);
      const jsonValidation = validateJsonResponse(cleanedResponse);
      expect(jsonValidation.isValid).toBe(true);
    });
  });

  describe('Complex Response Validation', () => {
    test('should handle response with valid JSON and valid Mermaid', async () => {
      const response = `Here's your analysis:

\`\`\`json
{
  "status": "completed",
  "diagram_count": 1
}
\`\`\`

\`\`\`mermaid
graph TD
    A[Analysis] --> B[Results]
    B --> C[Report]
\`\`\``;

      // Clean and validate JSON part
      const cleanedJson = '{\n  "status": "completed",\n  "diagram_count": 1\n}';
      const jsonValidation = validateJsonResponse(cleanedJson);
      expect(jsonValidation.isValid).toBe(true);

      // Validate Mermaid part
      const mermaidValidation = await validateMermaidResponse(response);
      expect(mermaidValidation.isValid).toBe(true);
    });

    test('should handle response with invalid JSON and valid Mermaid', async () => {
      const response = `Here's your analysis:

\`\`\`json
{
  "status": completed,
  "diagram_count": 1
}
\`\`\`

\`\`\`mermaid
graph TD
    A[Analysis] --> B[Results]
    B --> C[Report]
\`\`\``;

      // JSON should be invalid (missing quotes)
      const cleanedJson = '{\n  "status": completed,\n  "diagram_count": 1\n}';
      const jsonValidation = validateJsonResponse(cleanedJson);
      expect(jsonValidation.isValid).toBe(false);

      // Mermaid should be valid
      const mermaidValidation = await validateMermaidResponse(response);
      expect(mermaidValidation.isValid).toBe(true);
    });

    test('should handle response with valid JSON and invalid Mermaid', async () => {
      const response = `Here's your analysis:

\`\`\`json
{
  "status": "completed",
  "diagram_count": 1
}
\`\`\`

\`\`\`mermaid
invalid diagram syntax
\`\`\``;

      // JSON should be valid
      const cleanedJson = '{\n  "status": "completed",\n  "diagram_count": 1\n}';
      const jsonValidation = validateJsonResponse(cleanedJson);
      expect(jsonValidation.isValid).toBe(true);

      // Mermaid should be invalid
      const mermaidValidation = await validateMermaidResponse(response);
      expect(mermaidValidation.isValid).toBe(false);
    });

    test('should handle multiple Mermaid diagrams with mixed validity', async () => {
      const response = `\`\`\`mermaid
graph TD
    A --> B
\`\`\`

\`\`\`mermaid
invalid syntax
\`\`\`

\`\`\`mermaid
sequenceDiagram
    Alice->>Bob: Hello
\`\`\``;

      const validation = await validateMermaidResponse(response);
      expect(validation.isValid).toBe(false);
      expect(validation.diagrams).toHaveLength(3);
      expect(validation.diagrams[0].isValid).toBe(true);
      expect(validation.diagrams[1].isValid).toBe(false);
      expect(validation.diagrams[2].isValid).toBe(true);
    });
  });

  describe('Error Message Quality', () => {
    test('should provide detailed JSON correction prompts', () => {
      const invalidJson = '{"name": John, "age": 25}';
      const schema = '{"name": "string", "age": "number"}';
      const error = 'Unexpected token J in JSON at position 9';
      const detailedError = 'Unexpected token J in JSON at position 9';

      const prompt = createJsonCorrectionPrompt(invalidJson, schema, error, detailedError);

      expect(prompt).toContain('not valid JSON');
      expect(prompt).toContain(invalidJson);
      expect(prompt).toContain('Unexpected token J');
      expect(prompt).toContain('corrected JSON');
    });

    test('should provide detailed Mermaid correction prompts', () => {
      const invalidResponse = `\`\`\`mermaid
graph TD
    A[Start --> B[Missing bracket
\`\`\``;

      const schema = 'Create a mermaid flowchart';
      const errors = ['Diagram 1: Unclosed bracket on line 2'];
      const diagrams = [{
        diagram: 'graph TD\n    A[Start --> B[Missing bracket',
        isValid: false,
        error: 'Unclosed bracket on line 2',
        detailedError: 'Line "A[Start --> B[Missing bracket" contains an unclosed bracket'
      }];

      const prompt = createMermaidCorrectionPrompt(invalidResponse, schema, errors, diagrams);

      expect(prompt).toContain('invalid Mermaid diagrams');
      expect(prompt).toContain('Unclosed bracket');
      expect(prompt).toContain('mermaid code blocks');
      expect(prompt).toContain('correct Mermaid syntax');
    });

    test('should handle complex error scenarios', async () => {
      const response = `Here are the results:

\`\`\`mermaid
sequenceDiagram
    Alice->>Bob Hello world
    Bob-->>Alice: Response
\`\`\`

\`\`\`json
{
  "results": [
    {"id": 1, "status": completed}
  ]
}
\`\`\``;

      // Both should be invalid
      const mermaidValidation = await validateMermaidResponse(response);
      expect(mermaidValidation.isValid).toBe(false);
      expect(mermaidValidation.errors[0]).toContain('Missing colon');

      const jsonPart = '{\n  "results": [\n    {"id": 1, "status": completed}\n  ]\n}';
      const jsonValidation = validateJsonResponse(jsonPart);
      expect(jsonValidation.isValid).toBe(false);
    });
  });

  describe('Edge Cases', () => {
    test('should handle empty responses', async () => {
      const emptyInputs = ['', '   ', '\n\n', null, undefined];

      for (const input of emptyInputs) {
        const mermaidValidation = await validateMermaidResponse(input);
        expect(mermaidValidation.isValid).toBe(false);

        if (input) {
          const jsonValidation = validateJsonResponse(input);
          expect(jsonValidation.isValid).toBe(false);
        }
      }
    });

    test('should handle malformed markdown blocks', async () => {
      const malformedResponse = `\`\`\`mermaid
graph TD
    A --> B
\`\`\`mermaid (extra text)
sequenceDiagram
    Alice->>Bob: Test
\`\`\``;

      const { diagrams } = extractMermaidFromMarkdown(malformedResponse);
      expect(diagrams).toHaveLength(1);
      expect(diagrams[0].content).toContain('graph TD');
    });

    test('should handle nested code blocks', async () => {
      const response = `Here's an example:

\`\`\`markdown
This is how you create a mermaid diagram:

\`\`\`mermaid
graph TD
    A --> B
\`\`\`
\`\`\`

And here's a real diagram:

\`\`\`mermaid
sequenceDiagram
    Alice->>Bob: Hello
\`\`\``;

      const { diagrams } = extractMermaidFromMarkdown(response);
      expect(diagrams).toHaveLength(2); // Both embedded and real diagrams are extracted
      expect(diagrams[0].content).toContain('graph TD');
      expect(diagrams[1].content).toContain('sequenceDiagram');
    });

    test('should handle very large diagrams', async () => {
      const largeDiagram = 'graph TD\n' + Array(1000).fill(0).map((_, i) => 
        `    A${i}[Node ${i}] --> A${i + 1}[Node ${i + 1}]`
      ).join('\n');

      const result = await validateMermaidDiagram(largeDiagram);
      expect(result.isValid).toBe(true);
      expect(result.diagramType).toBe('flowchart');
    });

    test('should handle Unicode characters in diagrams', async () => {
      const unicodeDiagram = `graph TD
    A[开始] --> B[处理]
    B --> C[结束]
    D[🎯 Goal] --> E[✅ Complete]`;

      const result = await validateMermaidDiagram(unicodeDiagram);
      expect(result.isValid).toBe(true);
    });

    test('should handle process schema response with all options', () => {
      const response = `\`\`\`json
{
  "test": "value"
}
\`\`\`

\`\`\`mermaid
graph TD
    A --> B
\`\`\``;

      const result = processSchemaResponse(response, 'mixed schema', {
        validateJson: true,
        validateXml: false,
        debug: true
      });

      expect(result.cleaned).toBeDefined();
      expect(result.debug).toBeDefined();
      expect(result.jsonValidation).toBeDefined();
      expect(result.debug.wasModified).toBe(true);
    });
  });
});