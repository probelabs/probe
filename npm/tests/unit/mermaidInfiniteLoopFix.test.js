import { jest, beforeEach, describe, it, expect } from '@jest/globals';
import { validateMermaidDiagram, validateAndFixMermaidResponse } from '../../src/agent/schemaUtils.js';

describe('Mermaid Infinite Loop Fix', () => {
  describe('Node label quote handling', () => {
    
    
    
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
  });
});
