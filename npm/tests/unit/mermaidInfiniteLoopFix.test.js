import { jest, beforeEach, describe, it, expect } from '@jest/globals';
import { validateMermaidDiagram, validateAndFixMermaidResponse, MermaidFixingAgent } from '../../src/agent/schemaUtils.js';

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

  describe('MermaidFixingAgent should not pass schema to avoid infinite loops', () => {
    it('should call agent.answer without schema parameter', async () => {
      // Create a mock ProbeAgent
      const mockAgent = {
        answer: jest.fn().mockResolvedValue('```mermaid\ngraph TD\n    A --> B\n```')
      };

      // Create MermaidFixingAgent and inject mock
      const fixer = new MermaidFixingAgent({ debug: false });
      await fixer.initializeAgent();
      fixer.agent = mockAgent;

      // Call fixMermaidDiagram
      const brokenDiagram = 'graph TD\n    A["broken (syntax"]';
      await fixer.fixMermaidDiagram(brokenDiagram, ['line 1: unclosed bracket'], {});

      // Verify that answer was called without schema
      expect(mockAgent.answer).toHaveBeenCalled();
      const callArgs = mockAgent.answer.mock.calls[0];
      expect(callArgs[0]).toContain('Analyze and fix'); // prompt
      expect(callArgs[1]).toEqual([]); // messages array

      // Critical: verify no schema in options (either no 3rd arg or 3rd arg has no schema)
      if (callArgs.length >= 3) {
        expect(callArgs[2]).not.toHaveProperty('schema');
      }
    });

    it('should initialize ProbeAgent with maxIterations set to 2', async () => {
      // Create MermaidFixingAgent
      const fixer = new MermaidFixingAgent({ debug: false });

      // Initialize the agent
      const agent = await fixer.initializeAgent();

      // Verify maxIterations is set to 2
      expect(agent.maxIterations).toBe(2);
    });
  });
});
