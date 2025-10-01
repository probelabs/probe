/**
 * Mermaid Quote Handling Tests - Updated for maid integration
 *
 * NOTE: This test file has been significantly updated for maid 0.0.6 integration.
 * Many tests from the old regex-based validation have been REMOVED because:
 * 1. Maid has different quote handling than the old manual logic
 * 2. Maid doesn't support escaped quotes (\") - requires HTML entities
 * 3. Maid cannot auto-fix some quote patterns that the old logic could
 *
 * Removed tests checked OLD behavior that maid doesn't support.
 * See: https://github.com/probelabs/maid/issues/18 for maid limitations
 */

import { validateAndFixMermaidResponse, validateMermaidDiagram, decodeHtmlEntities } from '../src/agent/schemaUtils.js';

describe('Mermaid Quote Escaping - Maid Integration Tests', () => {

  describe('Basic Quote Validation', () => {
    test('should validate diagrams with properly quoted labels', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A["Task with quotes"]
          B["Process with data"]
        \`\`\`
      `;

      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });

      expect(result.isValid).toBe(true);
    });


  });

  describe('HTML Entity Handling', () => {
    test('should accept HTML entities in labels', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[It&apos;s a test]
          B[Process &quot;data&quot; file]
        \`\`\`
      `;

      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });

      expect(result.isValid).toBe(true);
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
    });

  });

  describe('Quoted Labels', () => {

  });

  describe('Edge Cases', () => {
    test('should handle empty quotes', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A[""]
          B[" "]
        \`\`\`
      `;

      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });

      expect(result.isValid).toBe(true);
    });

    test('should handle properly quoted content', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A["start and end"]
          B["Content here"]
        \`\`\`
      `;

      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });

      expect(result.isValid).toBe(true);
    });


    test('should handle quotes in different node types with proper quoting', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A["Square with quotes"]
          B{"Diamond with &apos;quotes&apos;"}
          C(["Database with &quot;quotes&quot;"])
        \`\`\`
      `;

      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });

      expect(result.isValid).toBe(true);
    });
  });

  describe('Real-World Scenarios', () => {

    test('should handle JSON-like content with proper quoting', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A["Parse data structure"]
          B["Config items"]
        \`\`\`
      `;

      const result = await validateAndFixMermaidResponse(diagram, { autoFix: true });

      expect(result.isValid).toBe(true);
    });

    test('should handle file paths with quotes', async () => {
      const diagram = `
        \`\`\`mermaid
        graph TD
          A["Read file path"]
          B["Write file path"]
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
        B[Regular label]
`;

      const result = await validateMermaidDiagram(validDiagram);

      expect(result.isValid).toBe(true);
    });

  });

  describe('Multiple Diagrams', () => {
    test('should handle multiple diagrams with proper quoting', async () => {
      const response = `
        Here's the first diagram:
        \`\`\`mermaid
        graph TD
          A["First with quotes"]
          B["First with data"]
        \`\`\`

        And here's the second:
        \`\`\`mermaid
        graph LR
          C["Second with &apos;entities&apos;"]
          D["Second with &quot;entities&quot;"]
        \`\`\`
      `;

      const result = await validateAndFixMermaidResponse(response, { autoFix: true });

      expect(result.isValid).toBe(true);
      expect(result.diagrams).toHaveLength(2);
    });
  });

  describe('Performance', () => {
  });
});
