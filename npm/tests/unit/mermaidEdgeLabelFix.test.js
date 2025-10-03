/**
 * Unit tests for Mermaid edge label syntax fixing
 *
 * Tests that MermaidFixingAgent correctly instructs AI to use pipe syntax
 * for edge labels instead of double quotes, preventing validation loops.
 */

import { describe, test, expect, jest } from '@jest/globals';
import { validateMermaidDiagram } from '../../src/agent/schemaUtils.js';

// Dynamically import MermaidFixingAgent to avoid circular dependency
const { MermaidFixingAgent } = await import('../../src/agent/schemaUtils.js');

describe('Mermaid Edge Label Syntax Fixing', () => {
  test('should detect invalid edge labels with double quotes', async () => {
    const invalidDiagrams = [
      'graph TD\n    A -- "Label Text" --> B',
      'graph TD\n    E -- "JSON-RPC Messages ONLY" --> F[stdout]',
      'flowchart LR\n    X -- "Step 1" --> Y\n    Y -- "Step 2" --> Z'
    ];

    for (const diagram of invalidDiagrams) {
      const result = await validateMermaidDiagram(diagram);
      expect(result.isValid).toBe(false);
      expect(result.error).toBeDefined();
    }
  });

  test('should validate edge labels with pipe syntax', async () => {
    const validDiagrams = [
      { desc: 'dashes with pipes', diagram: 'graph TD\n    A --|Label Text|--> B' },
      { desc: 'dashes with spaced pipes', diagram: 'graph TD\n    A -- |Label Text| --> B' },
      { desc: 'arrow with spaced pipes', diagram: 'graph TD\n    A -->|Label Text| B' },
      { desc: 'complex label', diagram: 'graph TD\n    E --|JSON-RPC Messages ONLY|--> F[stdout]' },
      { desc: 'multiple edges', diagram: 'flowchart LR\n    X --|Step 1|--> Y\n    Y --|Step 2|--> Z' }
    ];

    for (const { desc, diagram } of validDiagrams) {
      const result = await validateMermaidDiagram(diagram);
      expect(result.isValid).toBe(true);
      expect(result.diagramType).toBe('flowchart');
    }
  });

  test('MermaidFixingAgent prompt should include edge label syntax rules', async () => {
    const fixer = new MermaidFixingAgent({ debug: false });
    const prompt = fixer.getMermaidFixingPrompt();

    // Verify prompt instructs to use pipe syntax
    expect(prompt).toContain('Edge/Arrow labels');
    expect(prompt).toContain('pipe syntax');
    expect(prompt).toContain('--|');
    expect(prompt).toContain('NEVER use double quotes');
  });

  test('should fix invalid edge labels using mocked AI', async () => {
    // Create mock AI that returns corrected diagram with pipe syntax
    const invalidDiagram = `graph TD
    A[Start]
    A -- "Invalid Label 1" --> B[Process]
    B -- "Invalid Label 2" --> C[End]`;

    const fixedDiagram = `graph TD
    A[Start]
    A -->|Invalid Label 1| B[Process]
    B -->|Invalid Label 2| C[End]`;

    // Mock ProbeAgent.answer to return fixed diagram
    const mockAgent = {
      answer: jest.fn().mockResolvedValue(`\`\`\`mermaid\n${fixedDiagram}\n\`\`\``)
    };

    // Create MermaidFixingAgent and inject mock
    const fixer = new MermaidFixingAgent({ debug: false });
    await fixer.initializeAgent();
    fixer.agent = mockAgent;

    // Verify original diagram is invalid
    const validationBefore = await validateMermaidDiagram(invalidDiagram);
    expect(validationBefore.isValid).toBe(false);

    // Trigger the fixing flow
    const errors = ['line 4:14: Expecting token sequences, but found: "JSON-RPC Messages ONLY"'];
    const result = await fixer.fixMermaidDiagram(invalidDiagram, errors, {});


    // Verify the fix - result should have pipe syntax
    expect(result).toContain('-->|Invalid Label 1|');
    expect(result).toContain('-->|Invalid Label 2|');
    expect(result).not.toContain('"Invalid Label 1"');
    expect(result).not.toContain('"Invalid Label 2"');

    // Verify the AI was called with correct prompt
    expect(mockAgent.answer).toHaveBeenCalled();
    const callArgs = mockAgent.answer.mock.calls[0];
    expect(callArgs[0]).toContain('Analyze and fix');
    expect(callArgs[0]).toContain('Mermaid diagram');

    // Verify no schema parameter (this was causing the loop)
    if (callArgs.length >= 3) {
      expect(callArgs[2]).not.toHaveProperty('schema');
    }

    // Verify fixed diagram is valid
    const validationAfter = await validateMermaidDiagram(result);
    if (!validationAfter.isValid) {
      throw new Error(`Fixed diagram validation failed: ${validationAfter.error}\nResult: ${result}`);
    }
    expect(validationAfter.isValid).toBe(true);
  });

  test('should not fix already valid edge labels', async () => {
    const validDiagram = `graph TD
    A --|Label Text|--> B
    B --|Another Label|--> C`;

    const validation = await validateMermaidDiagram(validDiagram);
    expect(validation.isValid).toBe(true);
    expect(validation.diagramType).toBe('flowchart');
  });

  test('should handle mixed valid and invalid edge labels', async () => {
    const mixedDiagram = `graph TD
    A --|Valid Label|--> B
    B -- "Invalid Label" --> C`;

    const result = await validateMermaidDiagram(mixedDiagram);
    expect(result.isValid).toBe(false);
    expect(result.error).toBeDefined();
  });

  test('MermaidFixingAgent should not loop infinitely', async () => {
    // Mock AI that returns a fixed diagram (this test ensures the flow completes)
    const mockAgent = {
      answer: jest.fn().mockResolvedValue('```mermaid\ngraph TD\n    A -->|Fixed Label| B\n```')
    };

    const fixer = new MermaidFixingAgent({ debug: false });
    await fixer.initializeAgent();
    fixer.agent = mockAgent;

    const invalidDiagram = 'graph TD\n    A -- "Invalid" --> B';

    // This should complete without looping
    const result = await fixer.fixMermaidDiagram(invalidDiagram, ['Invalid syntax'], {});

    // Verify AI was only called once (not in infinite loop)
    expect(mockAgent.answer).toHaveBeenCalledTimes(1);
    expect(result).toContain('-->|Fixed Label|');
  });

  test('MermaidFixingAgent should disable mermaid validation in nested ProbeAgent', async () => {
    // This test verifies the fix for the infinite recursion bug
    // The inner ProbeAgent used by MermaidFixingAgent must have disableMermaidValidation=true
    const fixer = new MermaidFixingAgent({ debug: false });
    await fixer.initializeAgent();

    // Verify the nested ProbeAgent has mermaid validation disabled
    expect(fixer.agent.disableMermaidValidation).toBe(true);
  });
});
