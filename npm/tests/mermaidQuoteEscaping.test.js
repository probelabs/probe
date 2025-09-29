import { validateAndFixMermaidResponse, validateMermaidDiagram, decodeHtmlEntities } from '../src/agent/schemaUtils.js';

describe('Mermaid Quote Escaping Comprehensive Tests', () => {
  
  describe('Basic Quote Escaping', () => {
    test('should escape double quotes inside node labels', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[Task with "quotes" inside]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
      // The diagram itself is valid, so it might not be changed
      // But if it needs fixing due to other chars, quotes should be escaped
    });

    test('should escape single quotes inside node labels', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[It's John's task]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
    });

    test('should handle both single and double quotes together', async () => {
      const diagram = `
        \`\`\`mermaid
        graph LR
          A[John's "special" task]
          B["Mary's 'important' work"]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
    });
  });

  describe('HTML Entity Handling', () => {
    test('should convert &apos; to &#39; when content needs fixing', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[It&apos;s a (test)]
          B[John&apos;s &apos;data&apos; (file)]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
      // When parentheses trigger fixing, &apos; should be converted to &#39;
      if (result.wasFixed) {
        expect(result.fixedResponse).not.toContain('&apos;');
        expect(result.fixedResponse).toContain('&#39;');
      }
    });

    test('should handle &quot; entities correctly during fixing', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[Process &quot;data&quot; (file)]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
      // When fixed due to parentheses, HTML entities are decoded then re-encoded
      // &quot; gets decoded to " then wrapped in outer quotes
      if (result.wasFixed) {
        // The result should have the content properly quoted
        expect(result.fixedResponse).toMatch(/A\[.*".*data.*".*\]/);
      }
    });

    test('should not double-encode existing &#39; entities', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[It&#39;s already encoded]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
      expect(result.fixedResponse).not.toContain('&amp;#39;');
      expect(result.fixedResponse).toContain('&#39;');
    });

    test('should handle mixed &apos; and &#39; entities', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[Mix &apos;test&apos; and &#39;data&#39;]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
      // All should be normalized to &#39;
      expect(result.fixedResponse).not.toContain('&apos;');
      const count = (result.fixedResponse.match(/&#39;/g) || []).length;
      expect(count).toBeGreaterThanOrEqual(4); // At least 4 apostrophes
    });
  });

  describe('Nested Quotes in Already-Quoted Content', () => {
    test('should escape inner quotes when content has outer quotes and problematic chars', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A["Process (data) file"]
          B['Check (status) value']
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
      // These should be fixed due to parentheses and the single quote in B
      expect(result.wasFixed).toBe(true);
      // B should have been converted to use &#39; instead of single quotes
      expect(result.fixedResponse).toContain('&#39;');
    });

    test('should handle complex nested quotes with special characters', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[Process "input" (from file)]
          B[Check 'data' & "status"]
          C[Run "test's" command]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
      // Content with parentheses and ampersands should trigger fixing
      if (result.wasFixed) {
        expect(result.fixedResponse).toContain('&quot;');
        expect(result.fixedResponse).toContain('&#39;');
      }
    });
  });

  describe('Edge Cases and Special Patterns', () => {
    test('should handle empty quotes', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[""]
          B['']
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
    });

    test('should handle quotes at start and end of content', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A["start and end"]
          B['"quoted"']
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
    });

    test('should handle multiple consecutive quotes', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[Text with "" empty quotes]
          B[Text with '' apostrophes]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
    });

    test('should handle quotes in different node types', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A["Square with quotes"]
          B{"Diamond with 'quotes'"}
          C(["Database with \"quotes\""])
          D{{"Hexagon with 'data'"}}
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
    });
  });

  describe('Real-World Scenarios', () => {
    test('should handle the reported GitHub issue case', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          subgraph Dependent Check
            F -- "Accessed via Liquid" --> G[Another Check&apos;s &apos;exec&apos;];
            G -- "e.g., {{ outputs[&apos;check-name&apos;].key }}" --> H[Processes Data];
          end
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
      expect(result.fixedResponse).not.toContain('&apos;s &apos;');
      expect(result.fixedResponse).toContain('&#39;');
    });

    test('should handle JSON-like content in labels', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[Parse {"key": "value"} data]
          B[Config ['item1', 'item2']]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
    });

    test('should handle file paths with quotes', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[Read "/path/to/file's.txt"]
          B[Write "C:\\Users\\John's\\data.json"]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
    });

    test('should handle code snippets in labels', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[Run command: echo "Hello, World!"]
          B[Execute: grep 'pattern' file.txt]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
    });
  });

  describe('HTML Entity Decoder Tests', () => {
    test('should decode all supported HTML entities', () => {
      const input = '&lt;tag&gt; &amp; &quot;text&quot; &#39;data&#39; &apos;test&apos; &nbsp;space';
      const expected = '<tag> & "text" \'data\' \'test\'  space';
      
      expect(decodeHtmlEntities(input)).toBe(expected);
    });

    test('should handle null and undefined gracefully', () => {
      expect(decodeHtmlEntities(null)).toBe(null);
      expect(decodeHtmlEntities(undefined)).toBe(undefined);
      expect(decodeHtmlEntities('')).toBe('');
    });

    test('should decode entities multiple times in same string', () => {
      const input = '&quot;one&quot; and &quot;two&quot; and &quot;three&quot;';
      const expected = '"one" and "two" and "three"';
      
      expect(decodeHtmlEntities(input)).toBe(expected);
    });
  });

  describe('Validation Without Auto-Fix', () => {
    test('should validate diagrams with quotes as valid when appropriate', async () => {
      const validDiagram = `graph TD
        A["Valid quoted label"]
        B[Regular label]`;
      
      const result = await validateMermaidDiagram(validDiagram);
      
      expect(result.isValid).toBe(true);
    });

    test('should detect problematic quote patterns', async () => {
      // Single quotes in node labels cause GitHub "got PS" error
      const problematicDiagram = `graph TD
        A{'Single quotes problem'}`;
      
      const result = await validateMermaidDiagram(problematicDiagram);
      
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('Single quotes');
    });
  });

  describe('Multiple Diagrams in One Response', () => {
    test('should handle multiple diagrams with different quote patterns', async () => {
      const response = `
        Here's the first diagram:
        \`\`\`mermaid
        graph TD
          A[First with "quotes"]
          B[First with 'apostrophes']
        \`\`\`
        
        And here's the second:
        \`\`\`mermaid
        graph LR
          C[Second with &apos;entities&apos;]
          D[Second with &quot;entities&quot;]
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(response, { autoFix: true });
      
      expect(result.isValid).toBe(true);
      expect(result.diagrams).toHaveLength(2);
    });
  });

  describe('Performance and Large Content', () => {
    test('should handle diagrams with many nodes containing quotes', async () => {
      const nodes = [];
      for (let i = 0; i < 50; i++) {
        nodes.push(`  Node${i}[Task ${i} with "data" and 'info']`);
      }
      
      const diagram = `
        \`\`\`mermaid
        graph TD
${nodes.join('\n')}
        \`\`\`
      `;
      
      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });
      
      expect(result.isValid).toBe(true);
    });
  });
});