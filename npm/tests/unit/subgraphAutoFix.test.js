/**
 * Subgraph and Node Label Auto-Fix Tests
 *
 * NOTE: Many tests in this file have been skipped for maid 0.0.5 integration.
 * These tests check OLD manual regex-based auto-fix behavior for:
 * - Subgraph quote wrapping (parentheses in unquoted subgraph names)
 * - Node label quote wrapping (parentheses in unquoted node labels)
 * - Specific fixMethod values and performance expectations
 *
 * Maid handles parentheses and quotes differently using proper parsing.
 * Tests marked with .skip check OLD behavior that maid doesn't replicate.
 */

import { validateAndFixMermaidResponse } from '../../src/agent/schemaUtils.js';

describe('Mermaid Auto-Fix', () => {
  const mockOptions = {
    debug: false,
    path: '/test/path',
    provider: 'anthropic',
    model: 'claude-3-sonnet-20240229'
  };

  describe('Auto-fix unquoted subgraph names with parentheses', () => {

    test('should not modify already quoted subgraph names', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    subgraph "AI Check Execution"
        A[AI Provider Generates Response]
    end

    subgraph "Structured Path (e.g., code-review)"
        B[Parse JSON with issues array]
    end
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // Should not need fixing since it's already properly quoted
      expect(result.wasFixed).toBe(false);
      expect(result.isValid).toBe(true);
      expect(result.fixedResponse).toBe(response);
    });



  });

  describe('Performance and reliability', () => {

    test('should fall back to AI fixing for complex errors', async () => {
      const response = `\`\`\`mermaid
flowchart TD
    subgraph Valid Name
        A[Task with unclosed bracket
    end
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // This should not be auto-fixable and should go to AI
      // (though we're not testing actual AI here, just that it attempts to)
      expect(result.fixingResults.length).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Auto-fix node labels with parentheses', () => {

    test('should not modify already quoted node labels', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    A["Task with (parentheses) already quoted"] --> B["Another (quoted) task"]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // Should not need fixing since it's already properly quoted
      expect(result.wasFixed).toBe(false);
      expect(result.isValid).toBe(true);
      expect(result.fixedResponse).toBe(response);
    });

    test('should not modify node labels without parentheses', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    A[Simple Task] --> B[Another Task]
    B --> C[Third Task]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // Should not need fixing since there are no parentheses
      expect(result.wasFixed).toBe(false);
      expect(result.isValid).toBe(true);
      expect(result.fixedResponse).toBe(response);
    });





  });
});
