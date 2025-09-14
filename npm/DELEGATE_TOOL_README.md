# Delegate Tool

The delegate tool allows you to delegate specific tasks to a specialized probe subagent. This is useful when you need to perform complex tasks that require specialized knowledge or when you want to offload work to another agent.

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

1. **Task Definition**: You provide a clear, specific task description
2. **Agent Spawning**: The delegate tool spawns a new probe agent process
3. **Task Execution**: The subagent processes your task independently 
4. **Response Waiting**: The main agent waits for the subagent to complete and return results
5. **Result Return**: The delegate tool returns the subagent's response

## Configuration Options

- `task` (required): The specific task to delegate. Be clear and detailed about what needs to be accomplished.
- `timeout` (optional, default: 300): Maximum time to wait for the subagent in seconds
- `debug` (optional, default: false): Enable debug logging for troubleshooting

## Error Handling

The delegate tool includes comprehensive error handling:
- Process spawn failures
- Timeouts
- Agent execution errors
- Empty responses

## Use Cases

1. **Complex Analysis**: Delegate comprehensive code analysis tasks
2. **Specialized Queries**: Ask domain-specific questions that require deep investigation
3. **Load Distribution**: Distribute work across multiple agents
4. **Modular Tasks**: Break down large tasks into smaller, delegatable pieces

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