/**
 * Tests for Enhanced Mermaid validation functionality with specialized fixing agent
 */

import { describe, test, expect, beforeEach, afterEach, jest } from '@jest/globals';
import {
  extractMermaidFromMarkdown,
  replaceMermaidDiagramsInMarkdown,
  MermaidFixingAgent,
  validateAndFixMermaidResponse
} from '../../src/agent/schemaUtils.js';

// Mock ProbeAgent to avoid actual API calls in tests
const mockProbeAgent = {
  answer: jest.fn(),
  getTokenUsage: jest.fn(() => ({ totalTokens: 100, inputTokens: 50, outputTokens: 50 })),
  cancel: jest.fn()
};

// Mock the dynamic import
jest.mock('../../src/agent/ProbeAgent.js', () => ({
  ProbeAgent: jest.fn(() => mockProbeAgent)
}), { virtual: true });

describe('Enhanced Mermaid Validation', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('Enhanced extractMermaidFromMarkdown', () => {
    test('should extract mermaid with position tracking and attributes', () => {
      const response = `Generate a mermaid diagram showing the relationships between the modified components:

\`\`\`mermaid
graph TD
  A[Component A] --> B[Component B]
  B --> C[Database]
  C --> D[API Endpoints]
\`\`\`

Some other text here.

\`\`\`mermaid title="System Flow"
sequenceDiagram
  participant U as User
  participant S as System
  U->>S: Request
  S->>U: Response
\`\`\``;

      const result = extractMermaidFromMarkdown(response);

      expect(result.diagrams).toHaveLength(2);
      
      // First diagram
      expect(result.diagrams[0].content).toContain('graph TD');
      expect(result.diagrams[0].content).toContain('A[Component A] --> B[Component B]');
      expect(result.diagrams[0].attributes).toBe('');
      expect(result.diagrams[0].startIndex).toBeGreaterThan(0);
      expect(result.diagrams[0].endIndex).toBeGreaterThan(result.diagrams[0].startIndex);
      expect(result.diagrams[0].fullMatch).toContain('```mermaid');

      // Second diagram with attributes
      expect(result.diagrams[1].content).toContain('sequenceDiagram');
      expect(result.diagrams[1].content).toContain('participant U as User');
      expect(result.diagrams[1].attributes).toBe('title="System Flow"');
      expect(result.diagrams[1].startIndex).toBeGreaterThan(result.diagrams[0].endIndex);
    });

    test('should handle nested markdown structures', () => {
      const response = `
> Here's a quote with a diagram:
> 
> \`\`\`mermaid
> graph LR
>   A --> B
> \`\`\`
> 
> End of quote.

And a list:
1. Item 1
2. Item 2 with diagram:
   \`\`\`mermaid
   pie title Pie Chart
     "A" : 50
     "B" : 30
     "C" : 20
   \`\`\`
3. Item 3`;

      const result = extractMermaidFromMarkdown(response);

      expect(result.diagrams).toHaveLength(2);
      expect(result.diagrams[0].content).toContain('graph LR');
      expect(result.diagrams[1].content).toContain('pie title Pie Chart');
    });

    test('should preserve whitespace and indentation context', () => {
      const response = `    \`\`\`mermaid
    graph TD
        A[Start] --> B{Decision}
        B -->|Yes| C[Action 1]
        B -->|No| D[Action 2]
    \`\`\``;

      const result = extractMermaidFromMarkdown(response);

      expect(result.diagrams).toHaveLength(1);
      expect(result.diagrams[0].content).toContain('graph TD');
      expect(result.diagrams[0].content).toContain('A[Start] --> B{Decision}');
    });

    test('should handle mixed code block types', () => {
      const response = `
\`\`\`javascript
console.log("Hello");
\`\`\`

\`\`\`mermaid
graph TD
  A --> B
\`\`\`

\`\`\`json
{"key": "value"}
\`\`\`

\`\`\`mermaid class="custom"
stateDiagram-v2
  [*] --> State1
  State1 --> [*]
\`\`\``;

      const result = extractMermaidFromMarkdown(response);

      expect(result.diagrams).toHaveLength(2);
      expect(result.diagrams[0].content).toContain('graph TD');
      expect(result.diagrams[0].attributes).toBe('');
      expect(result.diagrams[1].content).toContain('stateDiagram-v2');
      expect(result.diagrams[1].attributes).toBe('class="custom"');
    });
  });

  describe('replaceMermaidDiagramsInMarkdown', () => {
    test('should replace diagrams while preserving original format', () => {
      const original = `Here's a diagram:

\`\`\`mermaid
graph TD
  A --> B[Bad Syntax
  B --> C
\`\`\`

Some text.`;

      const { diagrams } = extractMermaidFromMarkdown(original);
      
      // Simulate corrected diagram
      const correctedDiagrams = [{
        ...diagrams[0],
        content: 'graph TD\n  A --> B[Fixed Syntax]\n  B --> C'
      }];

      const result = replaceMermaidDiagramsInMarkdown(original, correctedDiagrams);

      expect(result).toContain('B[Fixed Syntax]');
      expect(result).toContain('```mermaid');
      expect(result).toContain('Some text.');
      expect(result).not.toContain('B[Bad Syntax');
    });

    test('should preserve attributes when replacing', () => {
      const original = `\`\`\`mermaid title="My Diagram" class="custom"
graph TD
  A --> B[Broken
\`\`\``;

      const { diagrams } = extractMermaidFromMarkdown(original);
      const correctedDiagrams = [{
        ...diagrams[0],
        content: 'graph TD\n  A --> B[Fixed]'
      }];

      const result = replaceMermaidDiagramsInMarkdown(original, correctedDiagrams);

      expect(result).toContain('```mermaid title="My Diagram" class="custom"');
      expect(result).toContain('A --> B[Fixed]');
    });

    test('should handle multiple diagram replacements', () => {
      const original = `First:
\`\`\`mermaid
graph TD
  A --> B[Error1
\`\`\`

Second:
\`\`\`mermaid
pie title Bad
  "A" 50
\`\`\`

Third:
\`\`\`mermaid title="Test"
sequenceDiagram
  A->>B: Missing colon
\`\`\``;

      const { diagrams } = extractMermaidFromMarkdown(original);
      const correctedDiagrams = diagrams.map((diagram, index) => ({
        ...diagram,
        content: `corrected_diagram_${index}`
      }));

      const result = replaceMermaidDiagramsInMarkdown(original, correctedDiagrams);

      expect(result).toContain('corrected_diagram_0');
      expect(result).toContain('corrected_diagram_1');
      expect(result).toContain('corrected_diagram_2');
      expect(result).toContain('```mermaid title="Test"');
      expect(result).toContain('First:');
      expect(result).toContain('Second:');
      expect(result).toContain('Third:');
    });
  });

  describe('MermaidFixingAgent', () => {
    test('should initialize with correct options', () => {
      const agent = new MermaidFixingAgent({
        path: '/test/path',
        provider: 'anthropic',
        model: 'claude-3',
        debug: true
      });

      expect(agent.options.path).toBe('/test/path');
      expect(agent.options.provider).toBe('anthropic');
      expect(agent.options.model).toBe('claude-3');
      expect(agent.options.debug).toBe(true);
      expect(agent.options.allowEdit).toBe(false);
    });

    test('should extract corrected diagram from response', () => {
      const agent = new MermaidFixingAgent();

      // Test mermaid code block extraction
      const response1 = `Here's the corrected diagram:

\`\`\`mermaid
graph TD
  A --> B[Fixed]
  B --> C
\`\`\``;

      expect(agent.extractCorrectedDiagram(response1)).toBe('graph TD\n  A --> B[Fixed]\n  B --> C');

      // Test fallback to any code block
      const response2 = `\`\`\`
graph TD
  A --> B[Fixed]
\`\`\``;

      expect(agent.extractCorrectedDiagram(response2)).toBe('graph TD\n  A --> B[Fixed]');

      // Test cleanup without code blocks
      const response3 = '```mermaid\ngraph TD\n  A --> B\n```';
      expect(agent.extractCorrectedDiagram(response3)).toBe('graph TD\n  A --> B');
    });

    test('should call ProbeAgent with correct prompt', async () => {
      const agent = new MermaidFixingAgent({ debug: true });
      
      mockProbeAgent.answer.mockResolvedValue(`\`\`\`mermaid
graph TD
  A --> B[Fixed]
  B --> C
\`\`\``);

      const result = await agent.fixMermaidDiagram(
        'graph TD\n  A --> B[Broken\n  B --> C',
        ['Unclosed bracket'],
        { diagramType: 'flowchart' }
      );

      expect(mockProbeAgent.answer).toHaveBeenCalledWith(
        expect.stringContaining('Analyze and fix the following Mermaid diagram'),
        [],
        { schema: 'Return only valid Mermaid diagram code within ```mermaid code block' }
      );
      
      expect(result).toBe('graph TD\n  A --> B[Fixed]\n  B --> C');
    });

    test('should handle fixing errors gracefully', async () => {
      const agent = new MermaidFixingAgent({ debug: true });
      
      mockProbeAgent.answer.mockRejectedValue(new Error('API Error'));

      await expect(
        agent.fixMermaidDiagram('invalid diagram')
      ).rejects.toThrow('Failed to fix Mermaid diagram: API Error');
    });
  });

  describe('validateAndFixMermaidResponse', () => {
    test('should return original response if all diagrams are valid', async () => {
      const validResponse = `Here's a valid diagram:

\`\`\`mermaid
graph TD
  A --> B[Valid]
  B --> C
\`\`\``;

      const result = await validateAndFixMermaidResponse(validResponse);

      expect(result.isValid).toBe(true);
      expect(result.wasFixed).toBe(false);
      expect(result.originalResponse).toBe(validResponse);
      expect(result.fixedResponse).toBe(validResponse);
    });

    test('should fix invalid diagrams using specialized agent', async () => {
      const invalidResponse = `Generate a mermaid diagram showing the relationships between the modified components:

\`\`\`mermaid
graph TD
  A[Component A] --> B[Component B
  B --> C[Database]
  C --> D[API Endpoints]
\`\`\`

Some other text here.`;

      mockProbeAgent.answer.mockResolvedValue(`\`\`\`mermaid
graph TD
  A[Component A] --> B[Component B]
  B --> C[Database]
  C --> D[API Endpoints]
\`\`\``);

      const result = await validateAndFixMermaidResponse(invalidResponse, {
        schema: 'Create mermaid diagram',
        debug: true,
        path: '/test',
        provider: 'anthropic'
      });

      expect(result.wasFixed).toBe(true);
      expect(result.originalResponse).toBe(invalidResponse);
      expect(result.fixedResponse).toContain('B[Component B]'); // Fixed bracket
      expect(result.fixingResults).toHaveLength(1);
      expect(result.fixingResults[0].wasFixed).toBe(true);
      expect(result.tokenUsage).toBeDefined();
    });

    test('should handle multiple invalid diagrams', async () => {
      const invalidResponse = `\`\`\`mermaid
graph TD
  A --> B[Error1
\`\`\`

\`\`\`mermaid
pie title Bad
  "A" 50
\`\`\``;

      let callCount = 0;
      mockProbeAgent.answer.mockImplementation(() => {
        callCount++;
        return Promise.resolve(`\`\`\`mermaid
corrected_diagram_${callCount}
\`\`\``);
      });

      const result = await validateAndFixMermaidResponse(invalidResponse, {
        debug: true
      });

      expect(result.wasFixed).toBe(true);
      expect(result.fixingResults).toHaveLength(2);
      expect(result.fixedResponse).toContain('corrected_diagram_1');
      expect(result.fixedResponse).toContain('corrected_diagram_2');
    });

    test('should handle agent initialization failures gracefully', async () => {
      const invalidResponse = `\`\`\`mermaid
graph TD
  A --> B[Error
\`\`\``;

      // Mock the dynamic import to fail
      jest.doMock('../../src/agent/ProbeAgent.js', () => {
        throw new Error('Import failed');
      }, { virtual: true });

      const result = await validateAndFixMermaidResponse(invalidResponse, {
        debug: true
      });

      expect(result.wasFixed).toBe(false);
      expect(result.fixingError).toBeDefined();
    });

    test('should preserve markdown formatting in fixed response', async () => {
      const response = `# Title

Here's the analysis with a diagram:

\`\`\`mermaid title="Architecture"
graph TD
  A --> B[Broken
  B --> C
\`\`\`

## Conclusion

Some final text.`;

      mockProbeAgent.answer.mockResolvedValue(`\`\`\`mermaid
graph TD
  A --> B[Fixed]
  B --> C
\`\`\``);

      const result = await validateAndFixMermaidResponse(response);

      expect(result.wasFixed).toBe(true);
      expect(result.fixedResponse).toContain('# Title');
      expect(result.fixedResponse).toContain('## Conclusion');
      expect(result.fixedResponse).toContain('```mermaid title="Architecture"');
      expect(result.fixedResponse).toContain('B[Fixed]');
      expect(result.fixedResponse).toContain('Some final text.');
    });
  });

  describe('Edge Cases and Error Handling', () => {
    test('should handle empty responses', async () => {
      const result = await validateAndFixMermaidResponse('');
      expect(result.isValid).toBe(false);
      expect(result.wasFixed).toBe(false);
    });

    test('should handle responses with no mermaid diagrams', async () => {
      const result = await validateAndFixMermaidResponse('Just plain text with no diagrams');
      expect(result.isValid).toBe(false);
      expect(result.wasFixed).toBe(false);
    });

    test('should handle malformed markdown', async () => {
      const malformedResponse = '```mermaid\ngraph TD\n  A --> B\n```incomplete';
      const result = await validateAndFixMermaidResponse(malformedResponse);
      
      // Should still extract and process what it can
      expect(result.diagrams).toBeDefined();
    });

    test('should handle unicode and special characters', async () => {
      const unicodeResponse = `\`\`\`mermaid
graph TD
  A[Ñoñó] --> B[测试]
  B --> C[🚀 Rocket
\`\`\``;

      mockProbeAgent.answer.mockResolvedValue(`\`\`\`mermaid
graph TD
  A[Ñoñó] --> B[测试]
  B --> C["🚀 Rocket"]
\`\`\``);

      const result = await validateAndFixMermaidResponse(unicodeResponse);
      
      expect(result.fixedResponse).toContain('Ñoñó');
      expect(result.fixedResponse).toContain('测试');
      expect(result.fixedResponse).toContain('🚀 Rocket');
    });
  });
});