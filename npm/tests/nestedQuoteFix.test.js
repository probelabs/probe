import { validateAndFixMermaidResponse } from '../src/agent/schemaUtils.js';

test('should handle nested quotes with &apos; entities correctly', async () => {
  // Test case from the issue - content with &apos; entities that was causing nested quotes
  const problematicDiagram = `
    \`\`\`mermaid
    graph TD
      subgraph Dependent Check
        F -- "Accessed via Liquid" --> G[Another Check&apos;s &apos;exec&apos;];
        G -- "e.g., {{ outputs[&apos;check-name&apos;].key }}" --> H[Processes Data];
      end
    \`\`\`
  `;

  const result = await validateAndFixMermaidResponse(problematicDiagram, {
    autoFix: true,
    debug: false
  });

  console.log('Validation result:', result);
  console.log('Fixed content:', result.fixedResponse);

  // The diagram should be fixed successfully
  if (!result.isValid) {
    console.error('Validation errors:', result.errors);
  }
  expect(result.isValid).toBe(true);
  
  // Check that the fixed content doesn't have nested quotes
  // It should have properly escaped HTML entities
  expect(result.fixedResponse).not.toContain('&apos;s &apos;');
  expect(result.fixedResponse).toContain('&#39;');
  
  // The fixed diagram should have proper node label quoting
  expect(result.fixedResponse).toMatch(/G\["[^"]*"\]/);
});

test('should handle mixed HTML entities correctly', async () => {
  const mixedEntitiesDiagram = `
    \`\`\`mermaid
    graph LR
      A[Process &quot;data&quot; file]
      B[Check &apos;status&apos; value]
      C[Mixed &quot;quotes&quot; and &apos;apostrophes&apos;]
    \`\`\`
  `;

  const result = await validateAndFixMermaidResponse(mixedEntitiesDiagram, {
    autoFix: true,
    debug: false
  });

  expect(result.isValid).toBe(true);
  
  // Should decode and re-encode properly
  // &apos; should be converted to &#39;
  expect(result.fixedResponse).not.toContain('&apos;');
  expect(result.fixedResponse).toContain('&#39;');
  // &quot; should stay as &quot; (both are valid)
  expect(result.fixedResponse).toContain('&quot;');
});

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