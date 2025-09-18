/**
 * Unit tests for GitHub-specific Mermaid diagram compatibility
 * These tests ensure our validator catches the same errors that cause GitHub to fail with "got 'PS'" errors
 */

import { describe, test, expect } from '@jest/globals';
import { validateMermaidDiagram } from '../../src/agent/schemaUtils.js';

describe('GitHub Mermaid Compatibility Validation', () => {
  describe('GitHub-incompatible patterns that cause "got PS" errors', () => {
    test('should reject single quotes in node labels', async () => {
      const diagramWithSingleQuotes = `graph TD
    A[Start] --> B[Process]
    B --> C{spawn('npx probe-chat')}
    C --> D[End]`;

      const result = await validateMermaidDiagram(diagramWithSingleQuotes);
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('Single quotes in node label');
      expect(result.error).toContain('GitHub incompatible');
      expect(result.detailedError).toContain('got PS');
    });

    test('should reject parentheses in square bracket node labels', async () => {
      const diagramWithParensInBrackets = `flowchart TD
    A[Start] --> B{Read Check Config}
    B --> C[Load Prompt<br/>(file or content)]
    C --> D[End]`;

      const result = await validateMermaidDiagram(diagramWithParensInBrackets);
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('Parentheses in node label');
      expect(result.error).toContain('GitHub incompatible');
      expect(result.detailedError).toContain('got PS');
    });

    test('should reject complex expressions in diamond nodes', async () => {
      const diagramWithComplexDiamond = `flowchart TD
    A[Start] --> B{process<complex>}
    B --> C[End]`;

      const result = await validateMermaidDiagram(diagramWithComplexDiamond);
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('Complex expression in diamond node');
      expect(result.error).toContain('GitHub incompatible');
    });

    test('should reject multiple problematic patterns in single diagram', async () => {
      const diagramWithMultipleIssues = `graph TD
    A[Start] --> B{spawn('command')}
    B --> C[Process (with details)]
    C --> D{check('<>')}
    D --> E[End]`;

      const result = await validateMermaidDiagram(diagramWithMultipleIssues);
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('GitHub incompatible');
    });
  });

  describe('GitHub-compatible patterns', () => {
    test('should accept node labels with double quotes', async () => {
      const compatibleDiagram = `graph TD
    A[Start] --> B["Process with (details)"]
    B --> C{spawn command}
    C --> D[End]`;

      const result = await validateMermaidDiagram(compatibleDiagram);
      expect(result.isValid).toBe(true);
      expect(result.diagramType).toBe('flowchart');
    });

    test('should accept simplified node labels', async () => {
      const simplifiedDiagram = `graph TD
    A[Start] --> B[Process]
    B --> C{spawn npx probe-chat}
    C --> D[End]`;

      const result = await validateMermaidDiagram(simplifiedDiagram);
      expect(result.isValid).toBe(true);
      expect(result.diagramType).toBe('flowchart');
    });

    test('should accept alternative syntax for complex labels', async () => {
      const alternativeSyntax = `flowchart TD
    A["Start: .visor.yaml"] --> B{Read Check Config}
    B --> C["Load Prompt<br/>from file or content"]
    C --> D{Render Template}
    D --> E[End]`;

      const result = await validateMermaidDiagram(alternativeSyntax);
      expect(result.isValid).toBe(true);
      expect(result.diagramType).toBe('flowchart');
    });

    test('should accept HTML breaks in labels', async () => {
      const diagramWithBreaks = `flowchart TD
    A[Start] --> B["Load Template<br/>Process Data<br/>Generate Output"]
    B --> C[End]`;

      const result = await validateMermaidDiagram(diagramWithBreaks);
      expect(result.isValid).toBe(true);
      expect(result.diagramType).toBe('flowchart');
    });

    test('should accept subgraph structures', async () => {
      const subgraphDiagram = `graph TD
    subgraph "Before: CLI-based"
        A[Provider] --> B[Service]
        B --> C{spawn command}
    end
    subgraph "After: SDK-based"  
        D[Provider] --> E[Service]
        E --> F[SDK]
    end`;

      const result = await validateMermaidDiagram(subgraphDiagram);
      expect(result.isValid).toBe(true);
      expect(result.diagramType).toBe('flowchart');
    });
  });

  describe('Real-world GitHub failure cases', () => {
    test('should identify Visor component interaction diagram issues', async () => {
      // This is the actual diagram that fails on GitHub
      const visorComponentDiagram = `graph TD
    subgraph "Before: CLI-based Approach"
        A[AICheckProvider] --> B[AIReviewService]
        B --> C{spawn('npx probe-chat')}
        C --> D[AI API]
    end

    subgraph "After: Integrated SDK Approach"
        E[AICheckProvider] --> F[AIReviewService]
        F --> G[ProbeAgent SDK]
        G --> H[AI API]
    end`;

      const result = await validateMermaidDiagram(visorComponentDiagram);
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('Single quotes in node label');
      expect(result.detailedError).toContain("spawn('npx probe-chat')");
    });

    test('should identify Visor data flow chart issues', async () => {
      // This is the actual diagram that fails on GitHub
      const visorDataFlowDiagram = `flowchart TD
    A[Start: .visor.yaml] --> B{Read Check Config};
    B --> C[Load Prompt<br/>(file or content)];
    C --> D{Render Prompt Template<br/>with PR & Dep Context};
    D --> E[Execute AI Check via ProbeAgent];
    E --> F[Receive Validated JSON Result];
    F --> G[Load Output Template<br/>(from config or default)];
    G --> H{Render Output Template<br/>with JSON Result};
    H --> I[Post Formatted Comment to GitHub];
    I --> J[End];`;

      const result = await validateMermaidDiagram(visorDataFlowDiagram);
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('Parentheses in node label');
      expect(result.detailedError).toContain('(file or content)');
    });

    test('should validate Visor sequence diagram as GitHub-compatible', async () => {
      // This diagram works fine on GitHub
      const visorSequenceDiagram = `sequenceDiagram
    participant CEE as CheckExecutionEngine
    participant AICP as AICheckProvider
    participant ARS as AIReviewService
    participant PA as ProbeAgent
    participant AI as AI API

    CEE->>+AICP: execute(checkConfig, prInfo, dependencies)
    AICP->>AICP: Load prompt (from file or content)
    AICP->>AICP: Render Liquid template with context (pr, files, outputs)
    AICP->>+ARS: executeReview(processedPrompt, schema)
    ARS->>+PA: new ProbeAgent(options)
    ARS->>+PA: answer(prompt, { schema })
    PA->>+AI: Send API Request
    AI-->>-PA: Return JSON Response
    PA->>PA: Validate response against schema
    PA-->>-ARS: Return validated JSON
    ARS-->>-AICP: Return ReviewSummary
    AICP-->>-CEE: Return ReviewSummary`;

      const result = await validateMermaidDiagram(visorSequenceDiagram);
      expect(result.isValid).toBe(true);
      expect(result.diagramType).toBe('sequence');
    });
  });

  describe('Error message quality', () => {
    test('should provide specific fix suggestions', async () => {
      const problematicDiagram = `graph TD
    A[Start] --> B{spawn('cmd')}
    B --> C[Process]`;

      const result = await validateMermaidDiagram(problematicDiagram);
      expect(result.isValid).toBe(false);
      expect(result.detailedError).toContain('Use double quotes or escape characters instead');
      expect(result.detailedError).toContain('got PS');
    });

    test('should identify exact problematic line', async () => {
      const multiLineDiagram = `flowchart TD
    A[Start] --> B[Good Label]
    B --> C[Bad (parens)]
    C --> D[End]`;

      const result = await validateMermaidDiagram(multiLineDiagram);
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('line 3');
      expect(result.detailedError).toContain('Bad (parens)');
    });

    test('should explain GitHub-specific nature of the error', async () => {
      const githubIncompatibleDiagram = `graph TD
    A --> B{complex('expression')}`;

      const result = await validateMermaidDiagram(githubIncompatibleDiagram);
      expect(result.isValid).toBe(false);
      expect(result.error).toContain('GitHub incompatible');
      expect(result.detailedError).toContain('GitHub mermaid renderer fails');
    });
  });

  describe('Edge cases for GitHub compatibility', () => {
    test('should handle escaped characters properly', async () => {
      const diagramWithEscaping = `graph TD
    A[Start] --> B["Process with \\"quotes\\""]
    B --> C[End]`;

      const result = await validateMermaidDiagram(diagramWithEscaping);
      expect(result.isValid).toBe(true);
    });

    test('should allow parentheses outside of node labels', async () => {
      const diagramWithParensOutside = `graph TD
    A[Start] --> B[Process]
    B --> C[End]
    
    A -.-> (external system)`;

      const result = await validateMermaidDiagram(diagramWithParensOutside);
      expect(result.isValid).toBe(true);
    });

    test('should handle complex but valid patterns', async () => {
      const complexValidDiagram = `flowchart TD
    A["System Start"] --> B{"Decision Point"}
    B -->|Yes| C["Process A"]
    B -->|No| D["Process B"]
    C --> E["Output"]
    D --> E`;

      const result = await validateMermaidDiagram(complexValidDiagram);
      expect(result.isValid).toBe(true);
    });
  });
});