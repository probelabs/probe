import { validateAndFixMermaidResponse } from '../../src/agent/schemaUtils.js';

describe('Mermaid Auto-Fix - Backticks', () => {
  const mockOptions = {
    debug: false,
    path: '/test/path',
    provider: 'anthropic',
    model: 'claude-3-sonnet-20240229'
  };

  describe('Auto-fix backticks in node labels', () => {
    test('should auto-fix single backtick in square bracket node', async () => {
      const response = `\`\`\`mermaid
flowchart TD
    A[Start] --> B[Apply defaults \`type: ai\`]
    B --> C[End]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixingResults.length).toBeGreaterThanOrEqual(1);
      expect(result.fixingResults[0].fixMethod).toBe('node_label_quote_wrapping');
      
      // Check that the backtick was properly quoted
      expect(result.fixedResponse).toContain('B["Apply defaults `type: ai`"]');
      
      // Verify no unquoted backticks remain
      const unquotedBackticks = result.fixedResponse.match(/\[[^"]*`[^"]*\]/g);
      expect(unquotedBackticks).toBeNull();
    });

    test('should auto-fix multiple backticks in same line', async () => {
      const response = `\`\`\`mermaid
flowchart TD
    A[Config with \`type: ai\` and \`on: manual\`] --> B[End]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixedResponse).toContain('A["Config with `type: ai` and `on: manual`"]');
    });

    test('should auto-fix backticks in diamond nodes', async () => {
      const response = `\`\`\`mermaid
flowchart TD
    A[Start] --> B{Check \`extends\` keyword?}
    B --> C[End]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixedResponse).toContain('B{"Check `extends` keyword?"}');
    });

    test('should auto-fix mixed problematic characters including backticks', async () => {
      const response = `\`\`\`mermaid
flowchart TD
    A[File (config.yaml)] --> B{Has 'extends' key?}
    B --> C[Process \`type: ai\` default] --> D[Output <br>result]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      
      // Check all patterns are fixed with HTML entities
      expect(result.fixedResponse).toContain('A["File (config.yaml)"]'); // parentheses
      expect(result.fixedResponse).toContain('B{"Has &#39;extends&#39; key?"}'); // single quotes converted to HTML entity
      expect(result.fixedResponse).toContain('C["Process `type: ai` default"]'); // backticks
      expect(result.fixedResponse).toContain('D["Output <br>result"]'); // HTML tags
    });

    test('should handle multiple diagrams with backticks', async () => {
      const response = `First diagram:
\`\`\`mermaid
flowchart TD
    A[Config with \`extends\`] --> B[End]
\`\`\`

Second diagram:
\`\`\`mermaid
graph LR
    X{Check \`validation\`} --> Y[Apply \`defaults\`]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.diagrams).toHaveLength(2);
      
      // Check both diagrams are fixed
      expect(result.fixedResponse).toContain('A["Config with `extends`"]');
      expect(result.fixedResponse).toContain('X{"Check `validation`"}');
      expect(result.fixedResponse).toContain('Y["Apply `defaults`"]');
    });

    test('should not modify already quoted backticks', async () => {
      const response = `\`\`\`mermaid
flowchart TD
    A["Already quoted \`backticks\`"] --> B{"Also quoted \`here\`"}
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // Even if already quoted, it might go through validation but should remain the same
      // The key is that the output should have proper quotes
      expect(result.fixedResponse).toContain('A["Already quoted `backticks`"]');
      expect(result.fixedResponse).toContain('B{"Also quoted `here`"}');
    });

    test('should preserve other content when fixing backticks', async () => {
      const response = `Here's a configuration diagram:

\`\`\`mermaid
flowchart TD
    A[Load config \`defaults\`] --> B[Process]
    B --> C[Output]
\`\`\`

This shows the config loading process.`;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixedResponse).toContain('Here\'s a configuration diagram:');
      expect(result.fixedResponse).toContain('This shows the config loading process.');
      expect(result.fixedResponse).toContain('A["Load config `defaults`"]');
    });
  });

  describe('Validation detects backticks correctly', () => {
    test('should detect backticks as invalid in validation', async () => {
      const response = `\`\`\`mermaid
flowchart TD
    A[Task with \`backticks\`] --> B[End]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // Should detect as invalid initially
      expect(result.diagrams[0].error).toContain('Backticks in node label');
      expect(result.wasFixed).toBe(true);
    });
  });
});