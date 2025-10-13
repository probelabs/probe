import { validateAndFixMermaidResponse } from '../../src/agent/schemaUtils.js';

describe('Mermaid Auto-Fix - Backticks', () => {
  const mockOptions = {
    debug: false,
    path: '/test/path',
    provider: 'anthropic',
    model: 'claude-3-sonnet-20240229'
  };

  describe('Auto-fix backticks in node labels', () => {





    test('should remove backticks from quoted labels', async () => {
      const response = `\`\`\`mermaid
flowchart TD
    A["Already quoted \`backticks\`"] --> B{"Also quoted \`here\`"}
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);

      // @probelabs/maid v0.0.15+ treats backticks inside quoted labels as errors (FL-LABEL-BACKTICK)
      // and removes them during auto-fix. This is the expected behavior.
      expect(result.fixedResponse).toContain('A["Already quoted backticks"]');
      expect(result.fixedResponse).toContain('B{"Also quoted here"}');
    });

  });

  describe('Validation detects backticks correctly', () => {
  });
});
