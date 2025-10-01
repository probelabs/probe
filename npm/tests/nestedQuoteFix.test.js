/**
 * Nested Quote Fix Tests
 *
 * NOTE: Most tests in this file have been skipped for maid 0.0.5 integration.
 * These tests check OLD regex-based HTML entity handling and quote fixing behavior:
 * - Converting &apos; to &#39;
 * - Automatic quote wrapping with escaped inner quotes
 * - Specific HTML entity normalization
 *
 * Maid handles HTML entities and quotes differently using proper parsing.
 * Tests marked with .skip check OLD behavior that maid doesn't replicate.
 */

import { validateAndFixMermaidResponse } from '../src/agent/schemaUtils.js';



test('should not double-encode already encoded entities', async () => {
  const preEncodedDiagram = `
    \`\`\`mermaid
    graph TD
      A[Text with &#39;single&#39; quotes]
      B[Text with &quot;double&quot; quotes]
    \`\`\`
  `;

  const result = await validateAndFixMermaidResponse(preEncodedDiagram, {
    autoFix: true,
    debug: false
  });

  expect(result.isValid).toBe(true);
  
  // Should not double-encode
  expect(result.fixedResponse).not.toContain('&amp;#39;');
  expect(result.fixedResponse).not.toContain('&amp;quot;');
});



