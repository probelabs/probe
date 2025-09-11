/**
 * Test fixtures for Mermaid diagrams and JSON responses
 * Used across multiple test files for consistency
 */

export const validMermaidDiagrams = {
  flowchart: {
    simple: `graph TD
    A[Start] --> B[Process]
    B --> C[End]`,
    
    complex: `graph TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Process A]
    B -->|No| D[Process B]
    C --> E[End]
    D --> E`,
    
    withStyling: `graph TD
    A[Start] --> B[Process]
    B --> C[End]
    style A fill:#f9f,stroke:#333,stroke-width:4px`
  },

  sequence: {
    simple: `sequenceDiagram
    Alice->>Bob: Hello Bob, how are you?
    Bob-->>Alice: Great!`,
    
    complex: `sequenceDiagram
    participant Alice
    participant Bob
    participant Carol
    
    Alice->>Bob: Hello Bob
    Bob->>Carol: Hello Carol
    Carol-->>Bob: Hi Bob
    Bob-->>Alice: Hi Alice`,
    
    withActivation: `sequenceDiagram
    Alice->>+Bob: Request
    Bob-->>-Alice: Response`
  },

  gantt: `gantt
    title A Gantt Diagram
    dateFormat  YYYY-MM-DD
    section Section
    A task           :a1, 2014-01-01, 30d
    Another task     :after a1, 20d`,

  pie: `pie title Pets adopted by volunteers
    "Dogs" : 386
    "Cats" : 85.9
    "Rats" : 15`,

  state: `stateDiagram-v2
    [*] --> Still
    Still --> [*]
    Still --> Moving
    Moving --> Still
    Moving --> Crash
    Crash --> [*]`,

  class: `classDiagram
    Animal <|-- Duck
    Animal <|-- Fish
    Animal <|-- Zebra
    Animal : +int age
    Animal : +String gender
    Animal: +isMammal()
    Animal: +mate()
    class Duck{
        +String beakColor
        +swim()
        +quack()
    }`
};

export const invalidMermaidDiagrams = {
  unknownType: 'unknownDiagram\n    some content',
  
  missingBracket: `graph TD
    A[Start --> B[Process]
    B --> C[End]`,
    
  missingColon: `sequenceDiagram
    Alice->>Bob Hello world
    Bob-->>Alice: Response`,
    
  malformedSyntax: 'this is not a valid mermaid diagram',
  
  withCodeBlocks: `\`\`\`mermaid
graph TD
    A --> B
\`\`\``,
  
  empty: '',
  whitespaceOnly: '   \n\t   '
};

export const validJsonResponses = {
  simple: '{"test": "value"}',
  
  complex: `{
    "users": [
      {"id": 1, "name": "Alice", "active": true},
      {"id": 2, "name": "Bob", "active": false}
    ],
    "total": 2,
    "metadata": {
      "timestamp": "2023-01-01T00:00:00Z",
      "version": "1.0"
    }
  }`,
  
  array: '[1, 2, 3, {"nested": true}]',
  primitives: {
    null: 'null',
    boolean: 'true',
    number: '42',
    string: '"hello world"'
  }
};

export const invalidJsonResponses = {
  missingQuotes: '{"test": value}',
  trailingComma: '{"test": "value",}',
  unquotedKeys: '{test: "value"}',
  incomplete: '{"test":',
  malformed: '{"test":: "value"}',
  empty: '',
  whitespace: '   \n\t   '
};

export const mixedResponses = {
  validBoth: `Here's the analysis:

\`\`\`json
{
  "status": "completed",
  "diagram_count": 1
}
\`\`\`

\`\`\`mermaid
graph TD
    A[Analysis] --> B[Results]
    B --> C[Complete]
\`\`\``,

  validJsonInvalidMermaid: `Here's the analysis:

\`\`\`json
{
  "status": "completed",
  "errors": []
}
\`\`\`

\`\`\`mermaid
invalid diagram syntax
\`\`\``,

  invalidJsonValidMermaid: `Here's the analysis:

\`\`\`json
{
  "status": completed,
  "errors": []
}
\`\`\`

\`\`\`mermaid
graph TD
    A[Start] --> B[End]
\`\`\``,

  invalidBoth: `Here's the analysis:

\`\`\`json
{
  "status": completed
  "errors": []
}
\`\`\`

\`\`\`mermaid
unknown diagram type
    invalid syntax
\`\`\``,

  multipleDiagrams: `Multiple diagrams:

\`\`\`mermaid
graph TD
    A --> B
\`\`\`

\`\`\`mermaid
sequenceDiagram
    Alice->>Bob: Hello
\`\`\`

\`\`\`mermaid
pie title Test
    "A" : 50
    "B" : 50
\`\`\``
};

export const schemas = {
  jsonOnly: '{"type": "object", "properties": {"name": {"type": "string"}}}',
  mermaidOnly: 'Create a mermaid flowchart showing the process',
  mixed: 'Return JSON response with embedded mermaid diagram',
  neither: 'Return plain text response',
  
  specific: {
    flowchart: 'Generate a mermaid flowchart diagram',
    sequence: 'Create a sequence diagram using mermaid',
    gantt: 'Show a gantt chart in mermaid format',
    pie: 'Create a pie chart diagram',
    json: 'Response must be valid JSON format'
  }
};

export const errorScenarios = {
  json: {
    unexpectedToken: {
      response: '{"test": value}',
      error: 'Unexpected token v in JSON at position 9',
      detailedError: 'Unexpected token v in JSON at position 9'
    },
    
    unexpectedEnd: {
      response: '{"test":',
      error: 'Unexpected end of JSON input',
      detailedError: 'Unexpected end of JSON input'
    },
    
    duplicateKeys: {
      response: '{"test": 1, "test": 2}',
      error: 'Duplicate key "test"',
      detailedError: 'JSON contains duplicate key "test"'
    }
  },
  
  mermaid: {
    unclosedBracket: {
      diagram: 'graph TD\n    A[Start --> B[Process]',
      error: 'Unclosed bracket on line 2',
      detailedError: 'Line "A[Start --> B[Process]" contains an unclosed bracket'
    },
    
    missingColon: {
      diagram: 'sequenceDiagram\n    Alice->>Bob Hello',
      error: 'Missing colon in sequence message on line 2',
      detailedError: 'Line "Alice->>Bob Hello" appears to be a sequence message but is missing a colon'
    },
    
    unknownType: {
      diagram: 'invalidDiagram\n    content',
      error: 'Diagram does not match any known Mermaid diagram pattern',
      detailedError: 'The diagram must start with a valid Mermaid diagram type (graph, sequenceDiagram, gantt, pie, etc.)'
    }
  }
};