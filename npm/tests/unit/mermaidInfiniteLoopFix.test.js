import { jest, beforeEach, describe, it, expect } from '@jest/globals';
import { validateMermaidDiagram, validateAndFixMermaidResponse } from '../../src/agent/schemaUtils.js';

describe('Mermaid Infinite Loop Fix', () => {
  describe('Node label quote handling', () => {
    it('should handle double quotes in node labels without creating single quotes', async () => {
      // Use a diagram with parentheses that will trigger auto-fixing
      const diagram = `\`\`\`mermaid
graph TD
    A[Process (data) file]
    B[Handle "special" case with (parentheses)]
\`\`\``;
      
      // The fixed diagram should use HTML entities instead of single quotes
      const result = await validateAndFixMermaidResponse(
        diagram, 
        '/test/path',
        'google',
        'test-model',
        { maxRetries: 1 }
      );
      
      // Should fix the diagram
      expect(result.wasFixed).toBe(true);
      
      // Should not contain single quotes that would trigger validation errors
      const fixedResponse = result.fixedResponse || result.originalResponse;
      expect(fixedResponse).not.toContain("'data'");
      expect(fixedResponse).not.toContain("'special'");
      
      // Should contain HTML entities for quotes or be properly wrapped
      const hasProperQuoting = fixedResponse.includes('["') && fixedResponse.includes('"]');
      expect(hasProperQuoting).toBe(true);
    });
    
    it('should handle both single and double quotes in content', async () => {
      // Use parentheses to trigger auto-fixing which will handle quotes properly
      const diagram = `\`\`\`mermaid
graph TD
    A[Check A: fetch-items(forEach: true)]
    B[Process 'data' file with (parentheses)]
    C[Handle "special" case]
\`\`\``;
      
      const result = await validateAndFixMermaidResponse(
        diagram,
        '/test/path', 
        'google',
        'test-model',
        { maxRetries: 1 }
      );
      
      // Should fix the parentheses issue
      expect(result.wasFixed).toBe(true);
      
      // Fixed response should properly handle quotes
      const fixedResponse = result.fixedResponse || result.originalResponse;
      
      // Should have wrapped labels with quotes
      expect(fixedResponse).toContain('["');
      
      // Should use HTML entities for any internal quotes
      if (fixedResponse.includes("'")) {
        expect(fixedResponse).toContain('&#39;');
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
      // Use a diagram with parentheses to trigger auto-fixing
      const diagram = `\`\`\`mermaid
graph TD
    A{Process (data) file}
    B{Handle "special" case with (parentheses)}
\`\`\``;
      
      const result = await validateAndFixMermaidResponse(
        diagram,
        '/test/path',
        'google',
        'test-model',
        { maxRetries: 1 }
      );
      
      // Should fix the diagram
      expect(result.wasFixed).toBe(true);
      
      // Should not contain single quotes that would trigger validation errors
      const fixedResponse = result.fixedResponse || result.originalResponse;
      expect(fixedResponse).not.toContain("{'");
      expect(fixedResponse).not.toContain("'}");
      
      // Should have proper diamond node syntax with quotes
      const hasProperDiamondQuoting = fixedResponse.includes('{"') && fixedResponse.includes('"}');
      expect(hasProperDiamondQuoting).toBe(true);
    });
  });
});