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

test('should escape inner quotes when they would cause issues', async () => {
  // Test with content that has quotes and special chars that trigger fixing
  const diagramWithProblematicQuotes = `
    \`\`\`mermaid
    graph TD
      A[Process "data" (from file)]
      B[Check 'status' and "result" values]
      C[Handle "error" in production's mode]
    \`\`\`
  `;

  const result = await validateAndFixMermaidResponse(diagramWithProblematicQuotes, {
    autoFix: true,
    debug: false
  });

  expect(result.isValid).toBe(true);
  
  // When content has problematic chars like parentheses or apostrophes,
  // it should be wrapped in quotes with inner quotes escaped
  expect(result.fixedResponse).toContain('&quot;');
  expect(result.fixedResponse).toContain('&#39;');
  
  // Verify no nested unescaped quotes in the fixed output
  const lines = result.fixedResponse.split('\n');
  lines.forEach(line => {
    // Check for patterns like ["content"] where content might have quotes
    const nodeMatch = line.match(/\["([^"]*)"\]/);
    if (nodeMatch) {
      const innerContent = nodeMatch[1];
      // Inner content should have escaped quotes, not raw quotes
      expect(innerContent).not.toMatch(/(?<!&quot;)"/);
    }
  });
});

test('should handle complex mixed quotes and apostrophes correctly', async () => {
  const complexDiagram = `
    \`\`\`mermaid
    graph LR
      A[John's "special" task]
      B[Process 'data' and "status" values]
      C[It's a "test" of 'nested' quotes]
    \`\`\`
  `;

  const result = await validateAndFixMermaidResponse(complexDiagram, {
    autoFix: true,
    debug: false
  });

  expect(result.isValid).toBe(true);
  
  // Should have proper escaping for both single and double quotes
  expect(result.fixedResponse).toContain('&#39;'); // escaped apostrophes
  expect(result.fixedResponse).toContain('&quot;'); // escaped double quotes
  
  // Verify the structure: outer quotes with escaped inner quotes
  expect(result.fixedResponse).toMatch(/A\["John&#39;s &quot;special&quot; task"\]/);
  expect(result.fixedResponse).toMatch(/B\["Process &#39;data&#39; and &quot;status&quot; values"\]/);
  expect(result.fixedResponse).toMatch(/C\["It&#39;s a &quot;test&quot; of &#39;nested&#39; quotes"\]/);
});

test('should handle the original issue case correctly', async () => {
  // This is the actual original issue with &apos; entities
  const originalIssueDiagram = `
    \`\`\`mermaid
    graph TD
      subgraph Dependent Check
        F -- "Accessed via Liquid" --> G[Another Check&apos;s &apos;exec&apos;];
        G -- "e.g., {{ outputs[&apos;check-name&apos;].key }}" --> H[Processes Data];
      end
    \`\`\`
  `;

  const result = await validateAndFixMermaidResponse(originalIssueDiagram, {
    autoFix: true,
    debug: false
  });

  expect(result.isValid).toBe(true);
  
  // Should convert &apos; to &#39; and properly wrap content
  expect(result.fixedResponse).toMatch(/G\["Another Check&#39;s &#39;exec&#39;"\]/);
  
  // The problematic &apos; entities should be converted to &#39;
  expect(result.fixedResponse).not.toContain('&apos;');
  expect(result.fixedResponse).toContain('&#39;');
});