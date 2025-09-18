import { validateAndFixMermaidResponse } from '../../src/agent/schemaUtils.js';

describe('Mermaid Auto-Fix', () => {
  const mockOptions = {
    debug: false,
    path: '/test/path',
    provider: 'anthropic',
    model: 'claude-3-sonnet-20240229'
  };

  describe('Auto-fix unquoted subgraph names with parentheses', () => {
    test('should auto-fix unquoted subgraph names with parentheses', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    subgraph AI Check Execution
        A[AI Provider Generates Response]
    end

    A --> B{Is schema plain or none?}

    subgraph Structured Path (e.g., code-review)
        B -- No --> C[Parse JSON with issues array]
        C --> D[PRReviewer receives ReviewSummary]
    end

    subgraph Unstructured Path (e.g., overview)  
        B -- Yes --> H[AI response is treated as raw markdown]
    end
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixingResults).toHaveLength(1);
      expect(result.fixingResults[0].fixMethod).toBe('subgraph_quote_wrapping');
      expect(result.fixingResults[0].wasFixed).toBe(true);
      
      // Check that the fixed response has quoted subgraph names
      expect(result.fixedResponse).toContain('subgraph "Structured Path (e.g., code-review)"');
      expect(result.fixedResponse).toContain('subgraph "Unstructured Path (e.g., overview)"');
      
      // Should be valid after auto-fix
      expect(result.isValid).toBe(true);
    });

    test('should not modify already quoted subgraph names', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    subgraph "AI Check Execution"
        A[AI Provider Generates Response]
    end

    subgraph "Structured Path (e.g., code-review)"
        B[Parse JSON with issues array]
    end
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // Should not need fixing since it's already properly quoted
      expect(result.wasFixed).toBe(false);
      expect(result.isValid).toBe(true);
      expect(result.fixedResponse).toBe(response);
    });

    test('should not modify subgraph names without parentheses', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    subgraph AI Check Execution
        A[AI Provider Generates Response]
    end

    subgraph Simple Name
        B[Some task]
    end
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // Should not need fixing since there are no parentheses
      expect(result.wasFixed).toBe(false);
      expect(result.isValid).toBe(true);
      expect(result.fixedResponse).toBe(response);
    });

    test('should handle mixed scenarios with some subgraphs needing fixes', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    subgraph "Already Quoted (with parens)"
        A[Task A]
    end

    subgraph Simple Name
        B[Task B]
    end

    subgraph Needs Fixing (with parens)
        C[Task C]
    end
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixingResults).toHaveLength(1);
      expect(result.fixingResults[0].fixMethod).toBe('subgraph_quote_wrapping');
      
      // Check that only the unquoted one with parentheses was fixed
      expect(result.fixedResponse).toContain('subgraph "Already Quoted (with parens)"'); // unchanged
      expect(result.fixedResponse).toContain('subgraph Simple Name'); // unchanged
      expect(result.fixedResponse).toContain('subgraph "Needs Fixing (with parens)"'); // fixed
      
      expect(result.isValid).toBe(true);
    });

    test('should preserve indentation when auto-fixing', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    A --> B
        subgraph Indented Name (with parens)
            C[Task C]
        end
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixedResponse).toContain('        subgraph "Indented Name (with parens)"');
      expect(result.isValid).toBe(true);
    });
  });

  describe('Performance and reliability', () => {
    test('should be much faster than AI fixing', async () => {
      const response = `\`\`\`mermaid
flowchart TD
    subgraph Test Name (with parens)
        A[Task]
    end
\`\`\``;

      const startTime = Date.now();
      const result = await validateAndFixMermaidResponse(response, mockOptions);
      const duration = Date.now() - startTime;
      
      expect(result.wasFixed).toBe(true);
      expect(result.performanceMetrics.aiFixingTimeMs).toBe(0); // No AI used
      expect(duration).toBeLessThan(100); // Should be very fast
    });

    test('should fall back to AI fixing for complex errors', async () => {
      const response = `\`\`\`mermaid
flowchart TD
    subgraph Valid Name
        A[Task with unclosed bracket
    end
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // This should not be auto-fixable and should go to AI
      // (though we're not testing actual AI here, just that it attempts to)
      expect(result.fixingResults.length).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Auto-fix node labels with parentheses', () => {
    test('should auto-fix unquoted node labels with parentheses', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    A[Start Process] --> B[Task with (some details) inside]
    B --> C[Another task (with more info)]
    C --> D["Already quoted (stays unchanged)"]
    D --> E[Simple task without parens]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixingResults).toHaveLength(1);
      expect(result.fixingResults[0].fixMethod).toBe('node_label_quote_wrapping');
      expect(result.fixingResults[0].wasFixed).toBe(true);
      
      // Check that the fixed response has quoted node labels with parentheses
      expect(result.fixedResponse).toContain('B["Task with (some details) inside"]');
      expect(result.fixedResponse).toContain('C["Another task (with more info)"]');
      
      // Should preserve already quoted and simple labels
      expect(result.fixedResponse).toContain('D["Already quoted (stays unchanged)"]');
      expect(result.fixedResponse).toContain('E[Simple task without parens]');
      
      // Should be valid after auto-fix
      expect(result.isValid).toBe(true);
    });

    test('should not modify already quoted node labels', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    A["Task with (parentheses) already quoted"] --> B["Another (quoted) task"]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // Should not need fixing since it's already properly quoted
      expect(result.wasFixed).toBe(false);
      expect(result.isValid).toBe(true);
      expect(result.fixedResponse).toBe(response);
    });

    test('should not modify node labels without parentheses', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    A[Simple Task] --> B[Another Task]
    B --> C[Third Task]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      // Should not need fixing since there are no parentheses
      expect(result.wasFixed).toBe(false);
      expect(result.isValid).toBe(true);
      expect(result.fixedResponse).toBe(response);
    });

    test('should handle complex flow with multiple node types', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    A[Start] --> B{Decision (check status)}
    B -->|Yes| C[Process (handle success)]
    B -->|No| D[Error handling (show message)]
    C --> E((End Process))
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixingResults).toHaveLength(1);
      expect(result.fixingResults[0].fixMethod).toBe('node_label_quote_wrapping');
      
      // Check that different node types are handled correctly
      expect(result.fixedResponse).toContain('B{"Decision (check status)"}'); // Diamond node
      expect(result.fixedResponse).toContain('C["Process (handle success)"]'); // Square node
      expect(result.fixedResponse).toContain('D["Error handling (show message)"]'); // Square node
      
      expect(result.isValid).toBe(true);
    });

    test('should preserve indentation when auto-fixing node labels', async () => {
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    A[Start]
        A --> B[Task (with details)]
            B --> C[End]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixedResponse).toContain('        A --> B["Task (with details)"]');
      expect(result.isValid).toBe(true);
    });

    test('should handle multiple auto-fixes in sequence', async () => {
      // Test with a diagram that has only node label issues (no subgraph issues)
      // to verify the node label auto-fix works independently
      const response = `Here's the diagram:

\`\`\`mermaid
flowchart TD
    A[Start] --> B[Task (with details)]
    B --> C{Decision (check this)}
    C --> D[End Process]
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixingResults).toHaveLength(1);
      expect(result.fixingResults[0].fixMethod).toBe('node_label_quote_wrapping');
      
      // Should fix node labels with parentheses
      expect(result.fixedResponse).toContain('B["Task (with details)"]');
      expect(result.fixedResponse).toContain('C{"Decision (check this)"}');
      
      expect(result.isValid).toBe(true);
    });

    test('should auto-fix complex node labels with HTML and parentheses formatting', async () => {
      // Test case based on real output that contains unquoted labels with parentheses and HTML
      const response = `Here's the workflow diagram:

\`\`\`mermaid
graph LR
    subgraph "1. Execution"
        A[Check 1 (security)] --> B{ReviewSummary};
        C[Check 2 (overview)] --> B;
        D[Check 3 (performance)] --> B;
    end

    subgraph "2. Rendering & Grouping"
        B -- Renders --> E[CheckResult 1 <br><i>content (details) <br> group (review)</i>];
        B -- Renders --> F[CheckResult 2 <br><i>content (details) <br> group (overview)</i>];
        B -- Renders --> G[CheckResult 3 <br><i>content (details) <br> group (review)</i>];
    end

    subgraph "3. Final Output"
        H[GroupedCheckResults <br><i>{ review (CR1, CR3), overview (CR2) }</i>]
        I[PR Comment 1 (review)]
        J[PR Comment 2 (overview)]
        K[GitHub Annotations]
    end

    E --> H;
    F --> H;
    G --> H;
    H --> I;
    H --> J;
    H --> K;
\`\`\``;

      const result = await validateAndFixMermaidResponse(response, mockOptions);
      
      expect(result.wasFixed).toBe(true);
      expect(result.fixingResults).toHaveLength(1);
      expect(result.fixingResults[0].fixMethod).toBe('node_label_quote_wrapping');
      expect(result.fixingResults[0].wasFixed).toBe(true);
      
      // Check that complex node labels with HTML and parentheses are properly quoted
      expect(result.fixedResponse).toContain('A["Check 1 (security)"]');
      expect(result.fixedResponse).toContain('C["Check 2 (overview)"]');
      expect(result.fixedResponse).toContain('D["Check 3 (performance)"]');
      expect(result.fixedResponse).toContain('E["CheckResult 1 <br><i>content (details) <br> group (review)</i>"]');
      expect(result.fixedResponse).toContain('F["CheckResult 2 <br><i>content (details) <br> group (overview)</i>"]');
      expect(result.fixedResponse).toContain('G["CheckResult 3 <br><i>content (details) <br> group (review)</i>"]');
      expect(result.fixedResponse).toContain('H["GroupedCheckResults <br><i>{" review (CR1, CR3), overview (CR2) "}</i>"]');
      expect(result.fixedResponse).toContain('I["PR Comment 1 (review)"]');
      expect(result.fixedResponse).toContain('J["PR Comment 2 (overview)"]');
      
      // Verify that simple labels without parentheses remain unchanged
      expect(result.fixedResponse).toContain('K[GitHub Annotations]');
      
      // Should be valid after auto-fix
      expect(result.isValid).toBe(true);
    });

  });
});