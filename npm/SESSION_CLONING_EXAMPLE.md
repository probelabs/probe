# ProbeAgent Session Cloning Examples

Session cloning allows you to create multiple ProbeAgent instances that share conversation history, enabling efficient reuse across different tasks while preserving AI provider cache efficiency.

## Why Clone Sessions?

1. **Cache Efficiency**: Reusing the same conversation history preserves Claude's prompt caching, reducing costs and improving response times
2. **Context Sharing**: Multiple agents can share the same background knowledge
3. **Parallel Processing**: Run multiple checks/tasks with shared context

## Basic Session Cloning

### Example 1: Simple Clone

```javascript
import { ProbeAgent } from '@probelabs/probe';

// Create the original agent
const originalAgent = new ProbeAgent({
  sessionId: 'my-session',
  path: './src',
  debug: true
});

await originalAgent.initialize();

// Have a conversation
await originalAgent.answer('What files are in this project?');
await originalAgent.answer('Show me the main entry point');

// Clone the session by creating a new agent with the same history
const clonedAgent = new ProbeAgent({
  sessionId: 'my-session-clone',  // Different session ID
  path: './src',
  debug: true
});

await clonedAgent.initialize();

// Copy the history from original to cloned agent
clonedAgent.history = [...originalAgent.history];

// The cloned agent now has all the context from the original
await clonedAgent.answer('Based on what you just learned, suggest improvements');
```

## Advanced: Visor-Style Session Cloning

This is how Visor clones sessions for running multiple checks in parallel:

### Example 2: Multiple Checks with Shared Context

```javascript
import { ProbeAgent } from '@probelabs/probe';

// Base session - learns about the codebase
const baseAgent = new ProbeAgent({
  sessionId: 'base-analysis',
  path: './src',
  debug: true
});

await baseAgent.initialize();

// Build up context
await baseAgent.answer('Analyze the project structure');
await baseAgent.answer('Identify the main components');

console.log(`Base agent has ${baseAgent.history.length} messages in history`);

// Now clone for multiple parallel checks
const checks = [
  'Check for security vulnerabilities',
  'Check for performance issues',
  'Check for code style violations'
];

const checkResults = await Promise.all(
  checks.map(async (checkDescription, index) => {
    // Create a new agent for this check
    const checkAgent = new ProbeAgent({
      sessionId: `check-${index}`,
      path: './src',
      debug: true
    });

    await checkAgent.initialize();

    // Clone the history from base agent
    // This preserves the cache because the conversation prefix is identical
    checkAgent.history = [...baseAgent.history];

    console.log(`Check ${index}: Cloned ${checkAgent.history.length} messages`);

    // Run the specific check with full context
    const result = await checkAgent.answer(checkDescription);

    return {
      check: checkDescription,
      result: result
    };
  })
);

console.log('All checks completed:', checkResults);
```

### Example 3: Incremental Context Building

```javascript
import { ProbeAgent } from '@probelabs/probe';

// Stage 1: Initial exploration
const stage1Agent = new ProbeAgent({
  sessionId: 'stage1',
  path: './src'
});

await stage1Agent.initialize();
await stage1Agent.answer('List all TypeScript files');
await stage1Agent.answer('Find the authentication logic');

// Stage 2: Clone and add security analysis
const stage2Agent = new ProbeAgent({
  sessionId: 'stage2',
  path: './src'
});

await stage2Agent.initialize();

// Clone stage 1 history
stage2Agent.history = [...stage1Agent.history];

// Continue with security-focused questions
await stage2Agent.answer('Analyze authentication for security issues');
await stage2Agent.answer('Check for SQL injection vulnerabilities');

// Stage 3: Clone stage 2 and generate report
const stage3Agent = new ProbeAgent({
  sessionId: 'stage3',
  path: './src'
});

await stage3Agent.initialize();

// Clone all previous context
stage3Agent.history = [...stage2Agent.history];

// Generate comprehensive report with full context
const report = await stage3Agent.answer(
  'Generate a security audit report based on all findings',
  [],
  {
    schema: JSON.stringify({
      type: 'object',
      properties: {
        summary: { type: 'string' },
        vulnerabilities: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              severity: { type: 'string' },
              description: { type: 'string' },
              location: { type: 'string' }
            }
          }
        }
      }
    })
  }
);

console.log('Security Report:', report);
```

## How the Cache Fix Works

Before the fix, cloning would break cache efficiency:

```javascript
// ❌ BEFORE (cache broken)
// Original: [system, user1, assistant1, user2, assistant2]
// Clone creates: [NEW_system, system, user1, assistant1, user2, assistant2, user3]
//                 ^^^^^^^^^^^ Different prefix = cache miss!
```

After the fix:

```javascript
// ✅ AFTER (cache preserved)
// Original: [system, user1, assistant1, user2, assistant2]
// Clone reuses: [system, user1, assistant1, user2, assistant2, user3]
//                ^^^^^^^ Same prefix = cache hit!
```

The fix in `ProbeAgent.js:1154` checks if `history[0]` is already a system message:

```javascript
const hasSystemMessage = this.history.length > 0 && this.history[0].role === 'system';

if (hasSystemMessage) {
  // Reuse existing system message from cloned history
  currentMessages = [
    ...this.history,
    userMessage
  ];
} else {
  // Fresh session - add new system message
  currentMessages = [
    { role: 'system', content: systemMessage },
    ...this.history,
    userMessage
  ];
}
```

## Best Practices

### 1. Session ID Uniqueness

```javascript
// ✅ Good: Each clone gets a unique session ID
const clone1 = new ProbeAgent({ sessionId: 'base-clone-1' });
const clone2 = new ProbeAgent({ sessionId: 'base-clone-2' });

// ❌ Bad: Reusing same session ID can cause cache conflicts
const clone1 = new ProbeAgent({ sessionId: 'same-id' });
const clone2 = new ProbeAgent({ sessionId: 'same-id' });
```

### 2. Deep Copy History

```javascript
// ✅ Good: Deep copy to avoid mutations
clonedAgent.history = JSON.parse(JSON.stringify(originalAgent.history));

// ✅ Also good: Spread operator for shallow copy (sufficient for most cases)
clonedAgent.history = [...originalAgent.history];

// ❌ Bad: Direct reference (mutations affect both)
clonedAgent.history = originalAgent.history;
```

### 3. Initialize Before Cloning

```javascript
// ✅ Good: Initialize then clone
const agent = new ProbeAgent({ sessionId: 'new' });
await agent.initialize();
agent.history = [...baseAgent.history];

// ❌ Bad: Clone before initialization
const agent = new ProbeAgent({ sessionId: 'new' });
agent.history = [...baseAgent.history];  // May be overwritten
await agent.initialize();
```

### 4. Monitor Cache Hits

```javascript
const agent = new ProbeAgent({
  sessionId: 'my-session',
  debug: true  // Shows cache-related debug logs
});

await agent.initialize();

// Look for: "[DEBUG] Reusing existing system message from history for cache efficiency"
```

## Real-World Use Case: Visor

Visor uses session cloning to run multiple code checks in parallel:

```javascript
// Simplified Visor workflow
class Visor {
  async runChecks(codebase) {
    // 1. Create base agent and build context
    const baseAgent = new ProbeAgent({
      sessionId: 'visor-base',
      path: codebase
    });

    await baseAgent.initialize();
    await baseAgent.answer('Analyze the codebase structure');

    // 2. Define checks
    const checks = [
      { name: 'security', prompt: 'Check for security issues' },
      { name: 'performance', prompt: 'Check for performance issues' },
      { name: 'style', prompt: 'Check code style' },
      { name: 'tests', prompt: 'Check test coverage' }
    ];

    // 3. Run all checks in parallel with cloned context
    const results = await Promise.all(
      checks.map(async (check) => {
        const checkAgent = new ProbeAgent({
          sessionId: `visor-${check.name}`,
          path: codebase
        });

        await checkAgent.initialize();

        // Clone base context for cache efficiency
        checkAgent.history = [...baseAgent.history];

        // Run check with schema for structured output
        return await checkAgent.answer(check.prompt, [], {
          schema: JSON.stringify({
            type: 'object',
            properties: {
              status: { type: 'string', enum: ['pass', 'fail', 'warning'] },
              issues: { type: 'array' },
              suggestions: { type: 'array' }
            }
          })
        });
      })
    );

    return results;
  }
}
```

## Token Usage and Cost Savings

With proper session cloning and cache reuse:

```javascript
// Example token usage
// Base session: 10,000 tokens (system + context)
// Clone 1: 500 tokens (new user message only - base cached)
// Clone 2: 500 tokens (new user message only - base cached)
// Clone 3: 500 tokens (new user message only - base cached)

// Total: 11,500 tokens with caching

// Without caching (before fix):
// Base: 10,000 tokens
// Clone 1: 10,500 tokens (full context repeated)
// Clone 2: 10,500 tokens (full context repeated)
// Clone 3: 10,500 tokens (full context repeated)

// Total: 41,500 tokens without caching

// Savings: 72% reduction in tokens!
```

## Debugging Session Clones

Enable debug mode to see what's happening:

```javascript
const agent = new ProbeAgent({
  sessionId: 'debug-session',
  debug: true
});

await agent.initialize();

// You'll see logs like:
// [DEBUG] Generated session ID for agent: debug-session
// [DEBUG] Reusing existing system message from history for cache efficiency
// [DEBUG] Trimmed stored history from 150 to 100 messages
```

## TypeScript Support

```typescript
import { ProbeAgent, ChatMessage } from '@probelabs/probe';

const baseAgent = new ProbeAgent({
  sessionId: 'typed-base',
  path: './src'
});

await baseAgent.initialize();
await baseAgent.answer('Analyze the code');

// Type-safe history cloning
const clonedAgent = new ProbeAgent({
  sessionId: 'typed-clone',
  path: './src'
});

await clonedAgent.initialize();

// history is typed as ChatMessage[]
const clonedHistory: ChatMessage[] = [...baseAgent.history];
clonedAgent.history = clonedHistory;
```

## Summary

Session cloning is powerful for:
- ✅ Running parallel checks with shared context
- ✅ Preserving AI provider cache for cost savings
- ✅ Building incremental analysis pipelines
- ✅ Reusing expensive context across multiple queries

The cache fix ensures that cloned sessions preserve prompt cache efficiency, making session cloning practical for production use cases like Visor.
