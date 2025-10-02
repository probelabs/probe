import { validateAndFixMermaidResponse } from '../../src/agent/schemaUtils.js';

describe('Mermaid Auto-Fix - Backticks', () => {
  const mockOptions = {
    debug: false,
    path: '/test/path',
    provider: 'anthropic',
    model: 'claude-3-sonnet-20240229'
  };

  describe('Auto-fix backticks in node labels', () => {





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

  });

  describe('Validation detects backticks correctly', () => {
  });
});
