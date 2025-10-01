/**
 * Unit tests for Mermaid validation functionality using real-world examples from Visor project
 * These tests use actual mermaid diagrams extracted from project documentation
 */

import { describe, test, expect } from '@jest/globals';
import {
  extractMermaidFromMarkdown,
  validateMermaidDiagram,
  validateMermaidResponse
} from '../../src/agent/schemaUtils.js';

// Real-world mermaid diagrams extracted from Visor project documentation
const visorExampleDiagrams = {
  componentInteraction: `
\`\`\`mermaid
graph TD
    subgraph "Before: CLI-based Approach"
        A[AICheckProvider] --> B[AIReviewService]
        B --> C{spawn probe-chat}
        C --> D[AI API]
    end

    subgraph "After: Integrated SDK Approach"
        E[AICheckProvider] --> F[AIReviewService]
        F --> G[ProbeAgent SDK]
        G --> H[AI API]
    end
\`\`\`
`,

  aiCheckSequence: `
\`\`\`mermaid
sequenceDiagram
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
    AICP-->>-CEE: Return ReviewSummary
\`\`\`
`,

  dataFlowChart: `
\`\`\`mermaid
flowchart TD
    A[Start: .visor.yaml] --> B{Read Check Config};
    B --> C["Load Prompt<br/>from file or content"];
    C --> D{"Render Prompt Template<br/>with PR and Dep Context"};
    D --> E[Execute AI Check via ProbeAgent];
    E --> F[Receive Validated JSON Result];
    F --> G["Load Output Template<br/>from config or default"];
    G --> H{"Render Output Template<br/>with JSON Result"};
    H --> I[Post Formatted Comment to GitHub];
    I --> J[End];
\`\`\`
`
};

// Test cases for validation improvements based on real-world usage
const edgeCaseDiagrams = {
  complexFlowchartWithMultilineLabels: `
\`\`\`mermaid
flowchart TD
    A["Complex Process<br/>with multiple lines<br/>and special chars"] --> B{Decision Point?}
    B -->|Yes| C["Process Option A<br/>- Step 1<br/>- Step 2"]
    B -->|No| D["Process Option B<br/>with details"]
    C --> E[Final Result]
    D --> E
    E --> F["End Process<br/>📊 Generate Report"]
\`\`\`
`,

  sequenceWithComplexParticipants: `
\`\`\`mermaid
sequenceDiagram
    participant WebApp as Web Application
    participant API as REST API Gateway
    participant Auth as Authentication Service
    participant DB as Database
    participant Cache as Redis Cache

    WebApp->>+API: POST /api/user/login
    API->>+Auth: validateCredentials(username, password)
    Auth->>+DB: SELECT user WHERE username = ?
    DB-->>-Auth: User record or null
    
    alt User exists and password valid
        Auth-->>API: JWT token + user data
        API->>+Cache: SETEX user_session:123 3600 {...}
        Cache-->>-API: OK
        API-->>WebApp: 200 OK + JWT + user profile
    else Authentication failed
        Auth-->>API: 401 Unauthorized
        API-->>WebApp: 401 Unauthorized + error message
    end
\`\`\`
`,

  ganttChartExample: `
\`\`\`mermaid
gantt
    title Software Development Timeline
    dateFormat YYYY-MM-DD
    section Planning
    Requirements Analysis    :done, req, 2024-01-01, 2024-01-15
    System Design          :done, design, after req, 15d
    section Development
    Backend Development     :active, backend, 2024-02-01, 30d
    Frontend Development    :frontend, after backend, 25d
    section Testing
    Unit Testing           :testing, after frontend, 10d
    Integration Testing    :int-test, after testing, 7d
    section Deployment
    Production Deployment  :deploy, after int-test, 3d
\`\`\`
`
};

describe('Visor Project Mermaid Examples', () => {
  describe('Real-world diagram validation', () => {
    test('should validate component interaction diagram', async () => {
      const { diagrams } = extractMermaidFromMarkdown(visorExampleDiagrams.componentInteraction);
      expect(diagrams).toHaveLength(1);
      
      const validation = await validateMermaidDiagram(diagrams[0].content);
      expect(validation.isValid).toBe(true);
      expect(validation.diagramType).toBe('flowchart');
    });

    test('should validate AI check sequence diagram', async () => {
      const { diagrams } = extractMermaidFromMarkdown(visorExampleDiagrams.aiCheckSequence);
      expect(diagrams).toHaveLength(1);
      
      const validation = await validateMermaidDiagram(diagrams[0].content);
      expect(validation.isValid).toBe(true);
      expect(validation.diagramType).toBe('sequence');
    });

    test('should validate data flow chart', async () => {
      const { diagrams } = extractMermaidFromMarkdown(visorExampleDiagrams.dataFlowChart);
      expect(diagrams).toHaveLength(1);
      
      const validation = await validateMermaidDiagram(diagrams[0].content);
      expect(validation.isValid).toBe(true);
      expect(validation.diagramType).toBe('flowchart');
    });

    test('should validate all Visor diagrams in batch', async () => {
      const combinedResponse = Object.values(visorExampleDiagrams).join('\\n\\n');
      const result = await validateMermaidResponse(combinedResponse);
      
      expect(result.isValid).toBe(true);
      expect(result.diagrams).toHaveLength(3);
      expect(result.diagrams.every(d => d.isValid)).toBe(true);
      
      // Check diagram types
      const diagramTypes = result.diagrams.map(d => d.diagramType);
      expect(diagramTypes).toContain('flowchart');
      expect(diagramTypes).toContain('sequence');
    });
  });

  describe('Complex real-world patterns', () => {
    test('should validate flowchart with multiline labels and special characters', async () => {
      const { diagrams } = extractMermaidFromMarkdown(edgeCaseDiagrams.complexFlowchartWithMultilineLabels);
      expect(diagrams).toHaveLength(1);
      
      const validation = await validateMermaidDiagram(diagrams[0].content);
      expect(validation.isValid).toBe(true);
      expect(validation.diagramType).toBe('flowchart');
    });

    test('should validate sequence diagram with complex participants and alt blocks', async () => {
      const { diagrams } = extractMermaidFromMarkdown(edgeCaseDiagrams.sequenceWithComplexParticipants);
      expect(diagrams).toHaveLength(1);
      
      const validation = await validateMermaidDiagram(diagrams[0].content);
      expect(validation.isValid).toBe(true);
      expect(validation.diagramType).toBe('sequence');
    });

    test('should validate gantt chart with date formats', async () => {
      const { diagrams } = extractMermaidFromMarkdown(edgeCaseDiagrams.ganttChartExample);
      expect(diagrams).toHaveLength(1);

      const validation = await validateMermaidDiagram(diagrams[0].content);
      expect(validation.isValid).toBe(true);
      // Maid 0.0.4 doesn't detect gantt type, returns 'unknown'
      expect(validation.diagramType).toBe('unknown');
    });
  });

  describe('Extraction accuracy', () => {
    test('should preserve diagram structure and formatting', () => {
      const { diagrams } = extractMermaidFromMarkdown(visorExampleDiagrams.aiCheckSequence);
      const diagram = diagrams[0];
      
      // Check that participant aliases are preserved
      expect(diagram.content).toContain('participant CEE as CheckExecutionEngine');
      expect(diagram.content).toContain('participant AICP as AICheckProvider');
      
      // Check that sequence arrows are preserved
      expect(diagram.content).toContain('CEE->>+AICP:');
      expect(diagram.content).toContain('AI-->>-PA:');
      
      // Check that nested calls are preserved
      expect(diagram.content).toContain('AICP->>AICP:');
    });

    test('should handle subgraph structures correctly', () => {
      const { diagrams } = extractMermaidFromMarkdown(visorExampleDiagrams.componentInteraction);
      const diagram = diagrams[0];
      
      // Check that subgraph definitions are preserved
      expect(diagram.content).toContain('subgraph "Before: CLI-based Approach"');
      expect(diagram.content).toContain('subgraph "After: Integrated SDK Approach"');
      
      // Check that connections between subgraphs work
      expect(diagram.content).toContain('A[AICheckProvider] --> B[AIReviewService]');
      expect(diagram.content).toContain('E[AICheckProvider] --> F[AIReviewService]');
    });

    test('should preserve decision node formatting', () => {
      const { diagrams } = extractMermaidFromMarkdown(visorExampleDiagrams.dataFlowChart);
      const diagram = diagrams[0];

      // Check that decision nodes (diamond shapes) are preserved
      // Note: labels with <br/> now use quotes for maid compatibility
      expect(diagram.content).toContain('{Read Check Config}');
      expect(diagram.content).toContain('{"Render Prompt Template');
      expect(diagram.content).toContain('{"Render Output Template');

      // Check that multiline labels with HTML breaks are preserved (now quoted)
      expect(diagram.content).toContain('["Load Prompt<br/>from file or content"]');
    });
  });

  describe('Performance and edge cases', () => {
    test('should handle large diagrams efficiently', async () => {
      // Create a large flowchart with 50 nodes
      const largeFlowchart = `\`\`\`mermaid
flowchart TD
${Array.from({length: 50}, (_, i) => `    N${i}[Node ${i}] --> N${i + 1}[Node ${i + 1}]`).join('\n')}
\`\`\``;

      const startTime = Date.now();
      const result = await validateMermaidResponse(largeFlowchart);
      const duration = Date.now() - startTime;

      expect(result.isValid).toBe(true);
      expect(duration).toBeLessThan(1000); // Should complete within 1 second
    });

    test('should handle diagrams with unusual whitespace', () => {
      const diagramWithWeirdWhitespace = `\`\`\`mermaid


graph TD


    A[Start]    -->    B[Middle]



    B --> C[End]


\`\`\``;

      const { diagrams } = extractMermaidFromMarkdown(diagramWithWeirdWhitespace);
      expect(diagrams).toHaveLength(1);

      // Content preserves whitespace for maid compatibility (no trim)
      const content = diagrams[0].content;
      expect(content.trim().startsWith('graph TD')).toBe(true);
      expect(content.trim().endsWith('B --> C[End]')).toBe(true);
      expect(content).toContain('A[Start]    -->    B[Middle]'); // Internal spacing preserved
    });

    test('should extract diagrams with inline attributes', () => {
      const diagramWithAttributes = `\`\`\`mermaid title="System Architecture"
graph TD
    A --> B
\`\`\``;

      const { diagrams } = extractMermaidFromMarkdown(diagramWithAttributes);
      expect(diagrams).toHaveLength(1);
      expect(diagrams[0].attributes).toBe('title="System Architecture"');
      // Content now preserves trailing newline for maid compatibility
      expect(diagrams[0].content).toBe('graph TD\n    A --> B\n');
    });
  });
});

// Export the test diagrams for use in other test files or demonstration scripts
export { visorExampleDiagrams, edgeCaseDiagrams };