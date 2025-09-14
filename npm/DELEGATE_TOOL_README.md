# Delegate Tool

The delegate tool allows you to delegate **big distinct tasks** to specialized probe subagents. When you have large complex tasks, you can separate them into individual jobs and delegate each to a different agent. This is perfect for breaking down multi-part work, parallel processing, or when you need focused expertise on specific components.

## Usage

### As a Vercel AI SDK Tool

```javascript
import { delegateTool } from '@probelabs/probe';

// Create a delegate tool with configuration
const delegate = delegateTool({
  debug: true,        // Enable debug logging
  timeout: 300        // Timeout in seconds (default: 5 minutes)
});

// Use the tool
const result = await delegate.execute({
  task: 'Search through the authentication module and explain how user login validation works, including any security measures'
});

console.log(result);
```

### As a Raw Function

```javascript
import { delegate } from '@probelabs/probe';

// Delegate a task directly
const result = await delegate({
  task: 'Analyze the codebase and create a summary of all API endpoints',
  timeout: 600,       // 10 minutes timeout
  debug: true
});

console.log(result);
```

### In XML Tool Format (for AI agents)

```xml
<delegate>
<task>Search through the authentication module and explain how user login validation works, including any security measures</task>
</delegate>
```

## How It Works

1. **Task Definition**: You provide a complete, self-contained task description
2. **Clean Agent Spawning**: The delegate tool spawns a new probe agent process with:
   - Default 'code-researcher' prompt (not inherited from parent)
   - Schema validation disabled for simpler responses
   - Mermaid validation disabled for faster processing
3. **Independent Execution**: The subagent processes your task in isolation
4. **Response Waiting**: The main agent waits for the subagent to complete and return results
5. **Result Return**: The delegate tool returns the subagent's clean response

## Subagent Environment

Each delegated task runs in a clean environment:
- **Prompt**: Always uses the default `code-researcher` prompt, regardless of the parent agent's prompt
- **Validation**: Schema and Mermaid validation are disabled for faster, simpler responses
- **Isolation**: Each subagent runs independently without inheriting parent context

## Configuration Options

- `task` (required): The specific task to delegate. Should be a complete, self-contained task that can be executed independently.
- `timeout` (optional, default: 300): Maximum time to wait for the subagent in seconds
- `debug` (optional, default: false): Enable debug logging for troubleshooting

## Error Handling

The delegate tool includes comprehensive error handling:
- Process spawn failures
- Timeouts
- Agent execution errors
- Empty responses

## Use Cases

1. **Task Separation**: Break large complex tasks into individual jobs for parallel processing
2. **Complex Analysis**: Delegate comprehensive code analysis tasks to focused agents
3. **Specialized Queries**: Ask domain-specific questions that require deep investigation
4. **Load Distribution**: Distribute work across multiple agents for better performance
5. **Modular Tasks**: Decompose multi-part work into smaller, self-contained pieces

## Task Separation Examples

**Large Task**: "Analyze the entire codebase for issues"

**Separate into distinct tasks**:
```javascript
// Task 1: Security focus
await delegate({
  task: 'Analyze all authentication and authorization code for security vulnerabilities and suggest fixes'
});

// Task 2: Performance focus  
await delegate({
  task: 'Review database queries and API endpoints for performance bottlenecks and optimization opportunities'
});

// Task 3: Code quality focus
await delegate({
  task: 'Examine code structure, patterns, and maintainability issues across all modules'
});
```

## Integration

The delegate tool is fully integrated into:
- Vercel AI SDK tools
- LangChain tools (via Vercel compatibility)
- ACP (Agent Communication Protocol) system
- ProbeAgent class
- XML tool parsing system

## Example Scenarios

```javascript
// Delegate security analysis
await delegate({
  task: 'Analyze the authentication system for security vulnerabilities and suggest improvements'
});

// Delegate documentation generation
await delegate({
  task: 'Generate comprehensive API documentation for all endpoints in the /api folder'
});

// Delegate code review
await delegate({
  task: 'Review the recent changes in the user management module and identify potential issues'
});
```