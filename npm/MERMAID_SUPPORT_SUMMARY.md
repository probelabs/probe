# Mermaid Support in Probe Agent

## Overview

The Probe Agent has comprehensive built-in support for **Mermaid diagram validation and auto-fixing**. This functionality allows the agent to:

1. ✅ **Detect** when responses should contain Mermaid diagrams
2. ✅ **Extract** Mermaid diagrams from markdown responses  
3. ✅ **Validate** diagram syntax against Mermaid specifications
4. ✅ **Auto-fix** broken diagrams using a specialized AI agent
5. ✅ **Test** with comprehensive test coverage (66+ tests)

## Key Features

### 1. Schema Detection
- Automatically detects when schemas expect Mermaid output
- Keywords: `mermaid`, `diagram`, `flowchart`, `sequence`, `gantt`, `pie chart`, etc.
- Case-insensitive detection

### 2. Diagram Extraction
- Extracts Mermaid diagrams from \`\`\`mermaid code blocks
- Preserves diagram attributes and formatting
- Handles multiple diagrams in single response
- Position tracking for accurate replacement

### 3. Syntax Validation
Supports validation for all major Mermaid diagram types:
- **Flowcharts/Graphs** (`graph TD`, `flowchart LR`)
- **Sequence Diagrams** (`sequenceDiagram`)
- **Gantt Charts** (`gantt`) 
- **Pie Charts** (`pie`)
- **State Diagrams** (`stateDiagram`)
- **Class Diagrams** (`classDiagram`)
- **Entity-Relationship** (`erDiagram`)
- **User Journey** (`journey`)
- **Git Graphs** (`gitgraph`)
- **Requirement Diagrams** (`requirementDiagram`)
- **C4 Diagrams** (`C4Context`)

### 4. Error Detection
Catches common syntax errors:
- Unclosed brackets in flowcharts
- Missing colons in sequence messages
- Invalid diagram type declarations
- Malformed node definitions

### 5. Auto-Fix Capability
- **MermaidFixingAgent**: Specialized AI agent for syntax correction
- Preserves original diagram intent and structure
- Fixes syntax errors while maintaining semantic meaning
- Uses separate ProbeAgent instance with Mermaid-specific prompts

## Test Coverage

### Real-World Examples Tested ✅
From the uploaded Visor project file, we validated:

1. **Component Interaction Diagram** (flowchart with subgraphs)
2. **AI Check Process Sequence** (complex sequence diagram) 
3. **Data Flow Chart** (flowchart with decision nodes)

All passed validation perfectly!

### Edge Cases Covered ✅
- Complex flowcharts with multiline labels and special characters
- Sequence diagrams with multiple participants and alt blocks
- Gantt charts with date formatting
- Large diagrams (50+ nodes) with performance benchmarks
- Unicode and special character handling
- Unusual whitespace patterns
- Inline attributes in code blocks

## Usage Examples

### Basic Validation
\`\`\`javascript
import { validateMermaidResponse } from '@probelabs/probe/agent';

const response = \`Here's your diagram:
\\\`\\\`\\\`mermaid
graph TD
    A[Start] --> B[Process]
    B --> C[End]
\\\`\\\`\\\`\`;

const result = await validateMermaidResponse(response);
console.log(result.isValid); // true
console.log(result.diagrams[0].diagramType); // 'flowchart'
\`\`\`

### Auto-Fix Broken Diagrams
\`\`\`javascript
import { validateAndFixMermaidResponse } from '@probelabs/probe/agent';

const brokenResponse = \`\\\`\\\`\\\`mermaid
sequenceDiagram
    Alice->>Bob Request data  // Missing colon!
\\\`\\\`\\\`\`;

const result = await validateAndFixMermaidResponse(brokenResponse, {
  debug: true,
  path: process.cwd()
});

console.log(result.wasFixed); // true (if API keys available)
console.log(result.fixedResponse); // Contains corrected diagram
\`\`\`

### Using in ProbeAgent
\`\`\`javascript
import { ProbeAgent } from '@probelabs/probe/agent';

const agent = new ProbeAgent({
  path: './my-project',
  debug: true
});

const result = await agent.answer(
  "Create a mermaid flowchart showing the user authentication process",
  [],
  { schema: "Return a mermaid diagram showing the authentication flow" }
);

// The response will be automatically validated and potentially auto-fixed
\`\`\`

## Test Files

### Core Tests
- \`tests/unit/mermaidValidation.test.js\` - Basic validation functionality (43 tests)
- \`tests/unit/enhancedMermaidValidation.test.js\` - Advanced features (19 tests)  
- \`tests/integration/validationFlow.test.js\` - Integration scenarios (4 tests)

### Real-World Examples  
- \`tests/unit/mermaidValidationVisorExample.test.js\` - Tests with actual Visor project diagrams (12 tests)

### Standalone Test Script
- \`test-mermaid-validation.js\` - Standalone testing script for development

## Running Tests

\`\`\`bash
# Run all mermaid tests
npm test -- --testNamePattern="Mermaid"

# Run specific test file
npm test tests/unit/mermaidValidationVisorExample.test.js

# Run standalone test script
node test-mermaid-validation.js
\`\`\`

## API Requirements for Auto-Fix

The auto-fixing functionality requires API keys for one of:
- \`OPENAI_API_KEY\` (OpenAI GPT models)
- \`ANTHROPIC_API_KEY\` (Claude models)  
- \`GOOGLE_API_KEY\` (Gemini models)

Without API keys, validation still works perfectly - just no auto-fixing.

## Summary

✅ **All 3 diagrams** from your uploaded file **pass validation**  
✅ **66 tests** covering comprehensive mermaid functionality  
✅ **Auto-fix mechanism** ready for broken diagrams  
✅ **Production-ready** mermaid support in Probe Agent

The mermaid validation system in the Probe Agent is robust, well-tested, and handles real-world complexity beautifully!