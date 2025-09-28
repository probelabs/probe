import { jest, beforeEach, describe, it, expect } from '@jest/globals';
import { validateMermaidDiagram, validateAndFixMermaidResponse } from '../../src/agent/schemaUtils.js';

describe('Mermaid Infinite Loop Fix', () => {
  describe('Node label quote handling', () => {
    it('should handle double quotes in node labels without creating single quotes', async () => {
      const diagram = `\`\`\`mermaid
graph TD
    A[Process "data" file]
    B[Handle "special" case]
\`\`\``;
      
      // The fixed diagram should use HTML entities instead of single quotes
      const result = await validateAndFixMermaidResponse(
        diagram, 
        '/test/path',
        'google',
        'test-model',
        { maxRetries: 1 }
      );
      
      // Should not contain single quotes that would trigger validation errors
      const fixedResponse = result.fixedResponse || result.originalResponse;
      expect(fixedResponse).not.toContain("'data'");
      expect(fixedResponse).not.toContain("'special'");
      
      // Should contain either HTML entities or properly escaped quotes
      const hasHtmlEntities = fixedResponse.includes('&quot;') || fixedResponse.includes('&#39;');
      const hasEscapedQuotes = fixedResponse.includes('\\"');
      expect(hasHtmlEntities || hasEscapedQuotes).toBe(true);
    });
    
    it('should handle both single and double quotes in content', async () => {
      const diagram = `\`\`\`mermaid
graph TD
    A[Check A: fetch-items(forEach: true)]
    B["Process 'data' file"]
    C[Handle "special" case]
\`\`\``;
      
      const result = await validateAndFixMermaidResponse(
        diagram,
        '/test/path', 
        'google',
        'test-model',
        { maxRetries: 1 }
      );
      
      // Validation should pass or fix without creating conflicting quote patterns
      const fixedResponse = result.fixedResponse || result.originalResponse;
      const validation = await validateMermaidDiagram(fixedResponse.replace(/\`\`\`mermaid\n/, '').replace(/\n\`\`\`/, ''));
      
      // Should either be valid or have errors NOT related to single quotes in node labels
      if (!validation.isValid) {
        expect(validation.error).not.toMatch(/Single quotes in node label/);
      }
    });
    
    it('should not create infinite loops when fixing nested quotes', async () => {
      const problematicDiagram = `\`\`\`mermaid
graph TD
    subgraph "New Raw Array Access Feature"
        A["Check A: fetch-items<br/>(forEach: true)"]
        B["echo 'Item {{ outputs['fetch-items'].id }} of {{ outputs['fetch-items-raw'] | size }}'"]
    end
\`\`\``;
      
      const startTime = Date.now();
      const result = await validateAndFixMermaidResponse(
        problematicDiagram,
        '/test/path',
        'google', 
        'test-model',
        { maxRetries: 2 }  // Limited retries to prevent actual infinite loops in test
      );
      const endTime = Date.now();
      
      // Should complete within reasonable time (not stuck in loop)
      expect(endTime - startTime).toBeLessThan(5000);
      
      // Result should not contain the problematic single quote pattern
      const fixedResponse = result.fixedResponse || result.originalResponse;
      const hasProblematicPattern = /\["[^"]*'[^"]*"\]/.test(fixedResponse);
      expect(hasProblematicPattern).toBe(false);
    });
    
    it('should validate that HTML entities work in Mermaid diagrams', async () => {
      const diagramWithEntities = `graph TD
    A["Process &quot;data&quot; file"]
    B["Handle &#39;special&#39; case"]
    C{"Check &quot;status&quot;"}`;
      
      const validation = await validateMermaidDiagram(diagramWithEntities);
      
      // HTML entities should not trigger single quote validation errors
      if (!validation.isValid) {
        expect(validation.error).not.toMatch(/Single quotes in node label/);
        expect(validation.error).not.toMatch(/got PS/);
      }
    });
  });
  
  describe('Diamond node quote handling', () => {
    it('should handle quotes in diamond nodes without creating conflicts', async () => {
      const diagram = `\`\`\`mermaid
graph TD
    A{Process "data" file}
    B{Handle "special" case}
\`\`\``;
      
      const result = await validateAndFixMermaidResponse(
        diagram,
        '/test/path',
        'google',
        'test-model',
        { maxRetries: 1 }
      );
      
      // Should not contain single quotes that would trigger validation errors
      const fixedResponse = result.fixedResponse || result.originalResponse;
      expect(fixedResponse).not.toContain("{'");
      expect(fixedResponse).not.toContain("'}");
      
      // Should use HTML entities for quotes
      const hasHtmlEntities = fixedResponse.includes('&quot;') || fixedResponse.includes('&#39;');
      expect(hasHtmlEntities).toBe(true);
    });
  });
});