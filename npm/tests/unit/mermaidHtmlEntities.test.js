import { jest, describe, it, expect } from '@jest/globals';
import { validateMermaidDiagram } from '../../src/agent/schemaUtils.js';

describe('Mermaid HTML Entities Support', () => {
  it('should accept HTML entities in node labels as valid', async () => {
    // Test case based on Mermaid documentation best practices
    const diagramWithEntities = `graph TD
    A["Process &quot;data&quot; file"]
    B["Node with &#39;single quotes&#39;"]
    C{"Check &quot;status&quot; value"}
    D["Mixed &quot;double&quot; and &#39;single&#39; quotes"]`;
    
    const validation = await validateMermaidDiagram(diagramWithEntities);
    
    // HTML entities should be valid according to Mermaid specs
    expect(validation.isValid).toBe(true);
    if (!validation.isValid) {
      console.log('Validation error:', validation.error);
    }
  });
  
  it('should accept numeric HTML entities', async () => {
    const diagramWithNumericEntities = `graph TD
    A["Quote: &#34; and apostrophe: &#39;"]
    B["Hash: &#35; and ampersand: &#38;"]`;
    
    const validation = await validateMermaidDiagram(diagramWithNumericEntities);
    expect(validation.isValid).toBe(true);
  });
  
  it('should accept mixed HTML entities and regular text', async () => {
    const diagram = `flowchart LR
    A["Starting point"]
    B["Process &quot;important&quot; data"]
    C["Check if value &#61; &quot;expected&quot;"]
    D["Output: &#39;success&#39; or &#39;failure&#39;"]`;
    
    const validation = await validateMermaidDiagram(diagram);
    expect(validation.isValid).toBe(true);
  });
  
  it('should not flag HTML entities as single quotes error', async () => {
    const diagram = `graph TD
    A["Text with &#39; entity"]`;
    
    const validation = await validateMermaidDiagram(diagram);
    
    // Should not trigger the single quote validation error
    if (!validation.isValid) {
      expect(validation.error).not.toContain('Single quotes in node label');
      expect(validation.error).not.toContain('got PS');
    }
  });
  
  describe('Real-world examples from Mermaid docs', () => {
    it('should handle example from Mermaid documentation', async () => {
      // Example adapted from Mermaid official docs
      const diagram = `flowchart LR
    A["A double quote:&quot;"]
    B["A dec char:&#9829;"]
    C["A hash:&#35;"]`;
      
      const validation = await validateMermaidDiagram(diagram);
      expect(validation.isValid).toBe(true);
    });
    
    it('should handle complex escaping example', async () => {
      // Complex example from StackOverflow Mermaid discussion
      const diagram = `flowchart LR
    B["&quot;&lt;&lt;&gt;&gt;&amp;&#189;&#35;189;&quot;"]`;
      
      const validation = await validateMermaidDiagram(diagram);
      expect(validation.isValid).toBe(true);
    });
  });
});