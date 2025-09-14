/**
 * Unit tests for Mermaid validation functionality
 */

import { describe, test, expect } from '@jest/globals';
import {
  isMermaidSchema,
  extractMermaidFromMarkdown,
  validateMermaidDiagram,
  validateMermaidResponse,
  createMermaidCorrectionPrompt,
  decodeHtmlEntities
} from '../../src/agent/schemaUtils.js';

describe('Mermaid Validation', () => {
  describe('isMermaidSchema', () => {
    test('should detect mermaid keywords', () => {
      const mermaidSchemas = [
        'Generate a mermaid flowchart',
        'Create a sequence diagram',
        'Show me a gantt chart',
        'Draw a state diagram',
        'Create a class diagram',
        'Entity relationship diagram',
        'User journey diagram',
        'Git graph visualization',
        'Requirement diagram',
        'C4 context diagram',
        'Return in mermaid format',
        'DIAGRAM should be mermaid',
        'Create a pie chart visualization'
      ];

      const nonMermaidSchemas = [
        'Return JSON format',
        'Generate plain text',
        'Create a table',
        'Show code example',
        'Data visualization in Python',
        '',
        null,
        undefined
      ];

      mermaidSchemas.forEach(schema => {
        expect(isMermaidSchema(schema)).toBe(true);
      });

      nonMermaidSchemas.forEach(schema => {
        expect(isMermaidSchema(schema)).toBe(false);
      });
    });

    test('should handle case insensitivity', () => {
      expect(isMermaidSchema('MERMAID FLOWCHART')).toBe(true);
      expect(isMermaidSchema('sequence DIAGRAM')).toBe(true);
      expect(isMermaidSchema('Gantt Chart')).toBe(true);
    });
  });

  describe('extractMermaidFromMarkdown', () => {
    test('should extract single diagram', () => {
      const response = `Here's a flowchart:

\`\`\`mermaid
graph TD
    A[Start] --> B[Process]
    B --> C[End]
\`\`\`

That's the diagram.`;

      const result = extractMermaidFromMarkdown(response);
      expect(result.diagrams).toHaveLength(1);
      expect(result.diagrams[0].content).toContain('graph TD');
      expect(result.diagrams[0].content).toContain('A[Start] --> B[Process]');
    });

    test('should extract multiple diagrams', () => {
      const response = `Here are two diagrams:

\`\`\`mermaid
graph TD
    A --> B
\`\`\`

And another one:

\`\`\`mermaid
sequenceDiagram
    Alice->>Bob: Hello
    Bob-->>Alice: Hi
\`\`\``;

      const result = extractMermaidFromMarkdown(response);
      expect(result.diagrams).toHaveLength(2);
      expect(result.diagrams[0].content).toContain('graph TD');
      expect(result.diagrams[1].content).toContain('sequenceDiagram');
    });

    test('should handle diagrams with extra whitespace', () => {
      const response = `\`\`\`mermaid


graph TD
    A --> B


\`\`\``;

      const result = extractMermaidFromMarkdown(response);
      expect(result.diagrams).toHaveLength(1);
      expect(result.diagrams[0].content).toBe('graph TD\n    A --> B');
    });

    test('should return empty array for no diagrams', () => {
      const response = 'No mermaid diagrams here, just text.';
      const result = extractMermaidFromMarkdown(response);
      expect(result.diagrams).toHaveLength(0);
    });

    test('should handle malformed markdown blocks', () => {
      const response = `\`\`\`mermaid
graph TD
    A --> B
\`\`\`

\`\`\`mermaid
sequenceDiagram
    Alice->>Bob: Test
\`\`\``;

      const result = extractMermaidFromMarkdown(response);
      expect(result.diagrams).toHaveLength(2);
      expect(result.diagrams[0].content).toContain('graph TD');
      expect(result.diagrams[1].content).toContain('sequenceDiagram');
    });

    test('should handle null/undefined input', () => {
      expect(extractMermaidFromMarkdown(null).diagrams).toHaveLength(0);
      expect(extractMermaidFromMarkdown(undefined).diagrams).toHaveLength(0);
      expect(extractMermaidFromMarkdown('').diagrams).toHaveLength(0);
    });
  });

  describe('validateMermaidDiagram', () => {
    test('should validate flowchart diagrams', async () => {
      const validFlowcharts = [
        'graph TD\n    A[Start] --> B[Process]\n    B --> C[End]',
        'graph LR\n    A --> B\n    B --> C',
        'flowchart TD\n    A --> B'
      ];

      for (const diagram of validFlowcharts) {
        const result = await validateMermaidDiagram(diagram);
        expect(result.isValid).toBe(true);
        expect(result.diagramType).toBe('flowchart');
      }
    });

    test('should validate sequence diagrams', async () => {
      const validSequence = 'sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi';
      const result = await validateMermaidDiagram(validSequence);
      expect(result.isValid).toBe(true);
      expect(result.diagramType).toBe('sequence');
    });

    test('should validate different diagram types', async () => {
      const diagramTypes = [
        { code: 'gantt\n    title A Gantt Diagram', type: 'gantt' },
        { code: 'pie title Test\n    "A" : 30\n    "B" : 70', type: 'pie' },
        { code: 'stateDiagram\n    [*] --> Still', type: 'state' },
        { code: 'classDiagram\n    Animal <|-- Duck', type: 'class' },
        { code: 'erDiagram\n    CUSTOMER ||--o{ ORDER : places', type: 'er' },
        { code: 'journey\n    title My working day', type: 'journey' },
        { code: 'gitgraph\n    commit', type: 'gitgraph' }
      ];

      for (const { code, type } of diagramTypes) {
        const result = await validateMermaidDiagram(code);
        expect(result.isValid).toBe(true);
        expect(result.diagramType).toBe(type);
      }
    });

    test('should reject unknown diagram types', async () => {
      const result = await validateMermaidDiagram('unknownDiagram\n    some content');
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('does not match any known Mermaid diagram pattern');
    });

    test('should reject diagrams with markdown markers', async () => {
      const result = await validateMermaidDiagram('```mermaid\ngraph TD\n    A --> B\n```');
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('markdown code block markers');
    });

    test('should detect syntax errors in flowcharts', async () => {
      const invalidFlowchart = 'graph TD\n    A[Start --> B[Missing bracket';
      const result = await validateMermaidDiagram(invalidFlowchart);
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('Unclosed bracket');
    });

    test('should detect syntax errors in sequence diagrams', async () => {
      const invalidSequence = 'sequenceDiagram\n    Alice->>Bob Hello missing colon';
      const result = await validateMermaidDiagram(invalidSequence);
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('Missing colon in sequence message');
    });

    test('should handle empty input', async () => {
      const results = await Promise.all([
        validateMermaidDiagram(''),
        validateMermaidDiagram(null),
        validateMermaidDiagram(undefined)
      ]);

      results.forEach(result => {
        expect(result.isValid).toBe(false);
        expect(result.error).toContain('Empty or invalid diagram input');
      });
    });

    test('should handle whitespace-only input', async () => {
      const result = await validateMermaidDiagram('   \n\t   ');
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('does not match any known Mermaid diagram pattern');
    });
  });

  describe('validateMermaidResponse', () => {
    test('should validate response with single valid diagram', async () => {
      const response = `Here's your diagram:

\`\`\`mermaid
graph TD
    A[Start] --> B[Process]
    B --> C[End]
\`\`\``;

      const result = await validateMermaidResponse(response);
      expect(result.isValid).toBe(true);
      expect(result.diagrams).toHaveLength(1);
      expect(result.diagrams[0].isValid).toBe(true);
      expect(result.diagrams[0].diagramType).toBe('flowchart');
    });

    test('should reject response with invalid diagram', async () => {
      const response = `Here's your diagram:

\`\`\`mermaid
invalid syntax here
\`\`\``;

      const result = await validateMermaidResponse(response);
      expect(result.isValid).toBe(false);
      expect(result.errors).toBeDefined();
      expect(result.errors.length).toBeGreaterThan(0);
    });

    test('should handle mixed valid and invalid diagrams', async () => {
      const response = `Here are your diagrams:

\`\`\`mermaid
graph TD
    A --> B
\`\`\`

\`\`\`mermaid
invalid syntax
\`\`\`

\`\`\`mermaid
pie title Test
    "A" : 50
    "B" : 50
\`\`\``;

      const result = await validateMermaidResponse(response);
      expect(result.isValid).toBe(false);
      expect(result.diagrams).toHaveLength(3);
      expect(result.errors).toBeDefined();
      expect(result.errors.length).toBeGreaterThan(0);
      
      // Check individual diagram results
      expect(result.diagrams[0].isValid).toBe(true);  // First diagram valid
      expect(result.diagrams[1].isValid).toBe(false); // Second diagram invalid
      expect(result.diagrams[2].isValid).toBe(true);  // Third diagram valid
    });

    test('should reject response with no diagrams', async () => {
      const response = 'This response contains no mermaid diagrams.';
      const result = await validateMermaidResponse(response);
      expect(result.isValid).toBe(false);
      expect(result.errors).toContain('No mermaid diagrams found in response');
    });

    test('should handle multiple valid diagrams', async () => {
      const response = `\`\`\`mermaid
graph TD
    A --> B
\`\`\`

\`\`\`mermaid
sequenceDiagram
    Alice->>Bob: Hello
\`\`\``;

      const result = await validateMermaidResponse(response);
      expect(result.isValid).toBe(true);
      expect(result.diagrams).toHaveLength(2);
      expect(result.diagrams.every(d => d.isValid)).toBe(true);
    });
  });

  describe('createMermaidCorrectionPrompt', () => {
    test('should create comprehensive correction prompt', () => {
      const invalidResponse = `\`\`\`mermaid
invalid syntax
\`\`\``;

      const schema = 'Create a mermaid flowchart';
      const errors = ['Diagram 1: Parse error at line 1'];
      const diagrams = [{
        diagram: 'invalid syntax',
        isValid: false,
        error: 'Parse error at line 1',
        detailedError: 'Unexpected token at position 0'
      }];

      const prompt = createMermaidCorrectionPrompt(invalidResponse, schema, errors, diagrams);

      expect(prompt).toContain(invalidResponse);
      expect(prompt).toContain(schema);
      expect(prompt).toContain('Parse error at line 1');
      expect(prompt).toContain('invalid syntax');
      expect(prompt).toContain('mermaid code blocks');
      expect(prompt).toContain('Unexpected token at position 0');
      expect(prompt).toContain('Validation Errors:');
      expect(prompt).toContain('Diagram Details:');
    });

    test('should handle multiple errors', () => {
      const errors = [
        'Diagram 1: Syntax error',
        'Diagram 2: Missing closing bracket'
      ];
      const diagrams = [
        { diagram: 'bad syntax 1', isValid: false, error: 'Syntax error' },
        { diagram: 'bad syntax 2', isValid: false, error: 'Missing closing bracket' }
      ];

      const prompt = createMermaidCorrectionPrompt('response', 'schema', errors, diagrams);

      expect(prompt).toContain('1. Diagram 1: Syntax error');
      expect(prompt).toContain('2. Diagram 2: Missing closing bracket');
    });

    test('should truncate long diagram content', () => {
      const longDiagram = 'graph TD\n' + 'A --> B\n'.repeat(50);
      const diagrams = [{
        diagram: longDiagram,
        isValid: false,
        error: 'Test error'
      }];

      const prompt = createMermaidCorrectionPrompt('response', 'schema', ['Error'], diagrams);
      
      // Should truncate and add ellipsis
      expect(prompt).toContain('...');
      expect(prompt.indexOf(longDiagram)).toBe(-1); // Full diagram should not be present
    });

    test('should handle diagrams without detailed errors', () => {
      const diagrams = [{
        diagram: 'invalid',
        isValid: false,
        error: 'Basic error'
        // No detailedError
      }];

      const prompt = createMermaidCorrectionPrompt('response', 'schema', ['Error'], diagrams);
      
      expect(prompt).toContain('Basic error');
      // The word "Details:" appears in "Diagram Details:" section header, so we check for the specific pattern
      expect(prompt).not.toContain('Details: Basic error');
    });
  });

  describe('decodeHtmlEntities', () => {
    test('should decode common HTML entities', () => {
      const testCases = [
        { input: '&lt;br&gt;', expected: '<br>' },
        { input: '&amp;', expected: '&' },
        { input: '&quot;test&quot;', expected: '"test"' },
        { input: '&#39;test&#39;', expected: "'test'" },
        { input: '&nbsp;', expected: ' ' },
        { input: 'normal text', expected: 'normal text' },
        { input: '', expected: '' }
      ];

      testCases.forEach(({ input, expected }) => {
        expect(decodeHtmlEntities(input)).toBe(expected);
      });
    });

    test('should decode multiple entities in same string', () => {
      const input = 'A[Start] --&gt; B{&quot;Decision&lt;br&gt;Point&quot;}';
      const expected = 'A[Start] --> B{"Decision<br>Point"}';
      expect(decodeHtmlEntities(input)).toBe(expected);
    });

    test('should handle repeated entities', () => {
      const input = '&lt;&lt;interface&gt;&gt;';
      const expected = '<<interface>>';
      expect(decodeHtmlEntities(input)).toBe(expected);
    });

    test('should handle null and undefined inputs', () => {
      expect(decodeHtmlEntities(null)).toBe(null);
      expect(decodeHtmlEntities(undefined)).toBe(undefined);
      expect(decodeHtmlEntities('')).toBe('');
    });

    test('should handle non-string inputs', () => {
      expect(decodeHtmlEntities(123)).toBe(123);
      expect(decodeHtmlEntities({})).toEqual({});
      expect(decodeHtmlEntities([])).toEqual([]);
    });

    test('should decode entities in mermaid diagram contexts', () => {
      const mermaidWithEntities = `graph TD
    A[Start] --&gt; B{&quot;Is it working?&quot;}
    B --&gt; C[Yes &amp; Good]
    B --&gt; D[No&lt;br&gt;Fix it]`;

      const expectedMermaid = `graph TD
    A[Start] --> B{"Is it working?"}
    B --> C[Yes & Good]
    B --> D[No<br>Fix it]`;

      expect(decodeHtmlEntities(mermaidWithEntities)).toBe(expectedMermaid);
    });

    test('should not affect valid HTML tags that are not encoded', () => {
      const input = 'A[Start] --> B{Decision<br>Point}';
      const expected = 'A[Start] --> B{Decision<br>Point}';
      expect(decodeHtmlEntities(input)).toBe(expected);
    });
  });
});