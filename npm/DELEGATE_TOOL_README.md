# Delegate Tool

The delegate tool is used **automatically by AI agents** within the agentic loop to delegate big distinct tasks to specialized probe subagents. When an AI agent encounters complex multi-part requests, it should automatically identify opportunities for task separation and use delegation without explicit user instruction.

## Agentic Usage Pattern

The delegate tool is designed for **automatic use by AI agents**, not direct developer calls. The AI agent recognizes when a user's request involves multiple large, distinct components and automatically breaks them down into focused, parallel tasks.

### Automatic Task Recognition

When users make complex requests, the AI agent should automatically:

1. **Identify** multiple distinct components in the request
2. **Separate** them into self-contained tasks  
3. **Delegate** each task to a specialized subagent
4. **Combine** results from all subagents

### XML Tool Format (AI Agent Usage)

```xml
<delegate>
<task>Analyze all authentication and authorization code in the codebase for security vulnerabilities and provide specific remediation recommendations</task>
</delegate>

<delegate>
<task>Review database queries and API endpoints for performance bottlenecks and suggest optimization strategies</task>
</delegate>
```

## How It Works

1. **Task Definition**: You provide a complete, self-contained task description
2. **SDK-Based Agent Creation**: The delegate tool creates a new ProbeAgent instance in-process with:
   - Default 'code-researcher' prompt (not inherited from parent)
   - Schema validation disabled for simpler responses
   - Mermaid validation disabled for faster processing
   - Delegation explicitly disabled to prevent recursion
3. **Independent Execution**: The subagent processes your task in isolation within the same process
4. **Response Waiting**: The main agent awaits the subagent's completion (Promise-based)
5. **Result Return**: The delegate tool returns the subagent's clean response

## Subagent Environment

Each delegated task runs in a clean environment with automatic configuration:
- **Prompt**: Automatically uses the default `code-researcher` prompt, regardless of the parent agent's prompt
- **Validation**: Schema and Mermaid validation are automatically disabled for faster, simpler responses
- **Iterations**: Automatically limited to remaining parent iterations to respect global limits
- **Isolation**: Each subagent runs independently without inheriting parent context or history
- **No Recursion**: Delegation is explicitly disabled in subagents (`enableDelegate: false`) to prevent infinite delegation chains
- **Inherited Config**: Path, provider, and model are inherited from parent for consistency
- **In-Process**: Runs within the same Node.js process using the ProbeAgent SDK (no process spawning)

All these settings are applied automatically - no manual configuration needed.

## Security and Resource Management

The delegate tool includes built-in protection against resource exhaustion:

### Concurrent Delegation Limits

- **Global Limit**: Maximum of 3 concurrent delegations by default (configurable via `MAX_CONCURRENT_DELEGATIONS` environment variable)
- **Per-Session Limit**: Maximum of 10 delegations per parent session (configurable via `MAX_DELEGATIONS_PER_SESSION` environment variable)
- **Automatic Cleanup**: Counters are automatically decremented when delegations complete, fail, or timeout

### Recursion Prevention

- Subagents cannot spawn their own subagents - the `delegate` tool is explicitly disabled in delegated sessions
- This prevents infinite delegation chains and ensures bounded resource usage

### Rate Limiting

Delegation requests that exceed limits will fail immediately with clear error messages:
- `Maximum concurrent delegations (N) reached. Please wait for some delegations to complete.`
- `Maximum delegations per session (N) reached for session {id}`

### Environment Variables

```bash
# Set custom limits (optional)
export MAX_CONCURRENT_DELEGATIONS=5
export MAX_DELEGATIONS_PER_SESSION=20
```

## Configuration Options

- `task` (required): The specific task to delegate. Should be a complete, self-contained task that can be executed independently.
- `timeout` (optional, default: 300): Maximum time to wait for the subagent in seconds
- `debug` (optional, default: false): Enable debug logging for troubleshooting
- `parentSessionId` (optional): Parent session ID for tracking per-session limits (automatically provided by ProbeAgent)
- `currentIteration` (optional): Current iteration count (automatically provided by ProbeAgent)
- `maxIterations` (optional): Maximum iterations allowed (automatically provided by ProbeAgent)

## Error Handling

The delegate tool includes comprehensive error handling:
- Process spawn failures
- Timeouts
- Agent execution errors
- Empty responses

## Agentic Use Cases

The AI agent automatically uses delegation for:

1. **Task Separation**: When user requests involve multiple distinct domains
2. **Complex Analysis**: Breaking comprehensive analysis into specialized areas  
3. **Parallel Processing**: Distributing work across multiple focused subagents
4. **Domain Expertise**: Delegating to subagents optimized for specific areas
5. **Large Scope**: Decomposing overwhelming requests into manageable pieces

## Automatic Task Separation Examples

**User Request**: "Analyze my entire codebase for security, performance, and maintainability issues"

**AI Agent automatically separates into**:
```xml
<delegate>
<task>Analyze all authentication, authorization, input validation, and cryptographic code for security vulnerabilities and provide specific remediation recommendations with code examples</task>
</delegate>

<delegate>
<task>Review all database queries, API endpoints, algorithms, and resource usage patterns for performance bottlenecks and suggest concrete optimization strategies</task>
</delegate>

<delegate>  
<task>Examine code structure, design patterns, documentation, and maintainability across all modules and provide refactoring recommendations</task>
</delegate>
```

**User Request**: "Help me understand how the payment system works"

**AI Agent automatically separates into**:
```xml
<delegate>
<task>Analyze the payment processing flow including transaction handling, validation, and error management to explain the complete payment workflow</task>
</delegate>

<delegate>
<task>Examine payment security measures including encryption, authentication, fraud detection, and compliance implementations</task>
</delegate>

<delegate>
<task>Review payment database schema, data models, and storage patterns to explain how payment data is structured and managed</task>
</delegate>
```

## Integration

The delegate tool is integrated into AI agent systems through:
- **ACP (Agent Communication Protocol)**: For advanced agent systems
- **XML Tool Parsing**: For AI agent tool call recognition
- **ProbeAgent Class**: For programmatic agent implementations  
- **Vercel AI SDK**: For AI framework compatibility
- **LangChain Tools**: Via Vercel compatibility layer

## AI Agent Decision Making

The AI agent should automatically use delegation when it recognizes:

- **Multi-domain requests**: User asks about multiple technical areas
- **Large scope tasks**: Requests that would benefit from parallel processing
- **Specialized expertise**: Tasks requiring focused domain knowledge
- **Complex analysis**: Comprehensive reviews that can be divided
- **Performance optimization**: When parallel execution improves response time

The delegate tool operates transparently within the agentic loop - users don't need to know it's being used. They simply get faster, more focused responses to complex requests.