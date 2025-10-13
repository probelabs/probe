# ProbeAgent Session Cloning Guide

The `clone()` method provides a native, intelligent way to clone ProbeAgent sessions with automatic filtering of internal messages.

## Migration from Manual Cloning

If you were previously cloning sessions manually, here's how to migrate:

### Before (Manual Approach)
```javascript
// Create new agent
const clonedAgent = new ProbeAgent({
  sessionId: 'clone-id',
  path: './src'
});

await clonedAgent.initialize();

// Manually copy history (includes internal messages!)
clonedAgent.history = [...baseAgent.history];

// Use the clone
await clonedAgent.answer('Continue working...');
```

### After (Native Method)
```javascript
// One line - automatically filters internal messages!
const clonedAgent = baseAgent.clone({
  sessionId: 'clone-id'  // Optional, auto-generates if not provided
});

await clonedAgent.initialize();

// Use the clone with clean history
await clonedAgent.answer('Continue working...');
```

## Why Use the Clone Method?

The native `clone()` method offers several advantages over manual history copying:

1. **Smart Filtering**: Automatically removes internal messages (schema reminders, mermaid fixes, tool use prompts)
2. **Clean Context**: Cloned agents get only the meaningful conversation (system + user + assistant + tool results)
3. **Cache Efficiency**: Preserves the system message for prompt caching
4. **Configuration Copying**: Automatically copies all agent settings
5. **Type Safety**: Returns a properly configured ProbeAgent instance

## Basic Usage

### Example 1: Simple Clone

```javascript
import { ProbeAgent } from '@probelabs/probe';

// Create base agent
const baseAgent = new ProbeAgent({
  sessionId: 'base',
  path: './src',
  debug: true
});

await baseAgent.initialize();

// Have a conversation with schema (creates internal messages)
await baseAgent.answer('What files are in this project?');
await baseAgent.answer(
  'List all functions',
  [],
  {
    schema: JSON.stringify({
      type: 'object',
      properties: {
        functions: { type: 'array' }
      }
    })
  }
);

console.log(`Base agent has ${baseAgent.history.length} messages`);
// Output: Base agent has 10 messages (includes internal schema reminders)

// Clone with automatic filtering
const clonedAgent = baseAgent.clone();

await clonedAgent.initialize();

console.log(`Cloned agent has ${clonedAgent.history.length} messages`);
// Output: Cloned agent has 5 messages (internal messages stripped)

// Use the cloned agent
await clonedAgent.answer('Based on what you learned, suggest improvements');
```

## Clone Options

### Example 2: Custom Session ID

```javascript
const clonedAgent = baseAgent.clone({
  sessionId: 'my-custom-session-id'
});
```

### Example 3: Keep Internal Messages

```javascript
// Keep all messages including internal ones
const clonedAgent = baseAgent.clone({
  stripInternalMessages: false
});

// Useful when you need the exact conversation state
```

### Example 4: Remove System Message

```javascript
// Clone without the system message (use new prompt type)
const clonedAgent = baseAgent.clone({
  keepSystemMessage: false,
  overrides: {
    promptType: 'architect', // Use a different prompt
    customPrompt: 'You are a security auditor...'
  }
});
```

### Example 5: Override Configuration

```javascript
// Clone with different settings
const clonedAgent = baseAgent.clone({
  overrides: {
    debug: false,          // Disable debug in clone
    allowEdit: true,       // Enable edit tool
    maxIterations: 50,     // Increase iteration limit
    promptType: 'code-review'  // Different prompt type
  }
});
```

### Example 6: Shallow Copy (Performance)

```javascript
// Use shallow copy for better performance (be careful with mutations!)
const clonedAgent = baseAgent.clone({
  deepCopy: false
});
```

## Real-World Use Cases

### Use Case 1: Parallel Checks (Visor Pattern)

```javascript
import { ProbeAgent } from '@probelabs/probe';

async function runParallelChecks(codebase) {
  // Step 1: Build shared context
  const baseAgent = new ProbeAgent({
    sessionId: 'context-builder',
    path: codebase,
    debug: true
  });

  await baseAgent.initialize();

  // Expensive context building (only done once)
  await baseAgent.answer('Analyze the project structure');
  await baseAgent.answer('List all API endpoints');
  await baseAgent.answer('Identify database models');

  console.log(`Base context: ${baseAgent.history.length} messages`);

  // Step 2: Clone for parallel checks
  const checks = [
    {
      name: 'security',
      prompt: 'Analyze for security vulnerabilities',
      schema: {
        type: 'object',
        properties: {
          vulnerabilities: { type: 'array' },
          severity: { type: 'string' }
        }
      }
    },
    {
      name: 'performance',
      prompt: 'Check for performance issues',
      schema: {
        type: 'object',
        properties: {
          issues: { type: 'array' },
          impact: { type: 'string' }
        }
      }
    },
    {
      name: 'style',
      prompt: 'Check code style and best practices',
      schema: {
        type: 'object',
        properties: {
          violations: { type: 'array' },
          suggestions: { type: 'array' }
        }
      }
    }
  ];

  // Run all checks in parallel with cloned context
  const results = await Promise.all(
    checks.map(async (check) => {
      // Clone base agent for this check
      const checkAgent = baseAgent.clone({
        sessionId: `check-${check.name}`
      });

      await checkAgent.initialize();

      // Run check with structured output
      const result = await checkAgent.answer(
        check.prompt,
        [],
        { schema: JSON.stringify(check.schema) }
      );

      // Clean up
      await checkAgent.cleanup();

      return {
        check: check.name,
        result: JSON.parse(result)
      };
    })
  );

  // Clean up base agent
  await baseAgent.cleanup();

  return results;
}

// Usage
const checkResults = await runParallelChecks('./my-project');
console.log('Security:', checkResults[0]);
console.log('Performance:', checkResults[1]);
console.log('Style:', checkResults[2]);
```

### Use Case 2: Progressive Context Building

```javascript
async function progressiveAnalysis(codebase) {
  // Stage 1: Basic exploration
  const stage1 = new ProbeAgent({
    sessionId: 'stage1-exploration',
    path: codebase
  });

  await stage1.initialize();
  await stage1.answer('What is the main purpose of this codebase?');
  await stage1.answer('What are the key components?');

  // Stage 2: Deep dive (clone stage 1)
  const stage2 = stage1.clone({
    sessionId: 'stage2-deepdive'
  });

  await stage2.initialize();
  await stage2.answer('Analyze the authentication system in detail');
  await stage2.answer('How does the database layer work?');

  // Stage 3: Security audit (clone stage 2)
  const stage3 = stage2.clone({
    sessionId: 'stage3-security',
    overrides: {
      promptType: 'security-audit'
    }
  });

  await stage3.initialize();
  const securityReport = await stage3.answer(
    'Generate a comprehensive security audit report',
    [],
    {
      schema: JSON.stringify({
        type: 'object',
        properties: {
          summary: { type: 'string' },
          critical: { type: 'array' },
          high: { type: 'array' },
          medium: { type: 'array' },
          recommendations: { type: 'array' }
        }
      })
    }
  );

  // Each stage builds on the previous one
  // But internal messages are automatically cleaned
  console.log(`Stage 1: ${stage1.history.length} messages`);
  console.log(`Stage 2: ${stage2.history.length} messages (filtered)`);
  console.log(`Stage 3: ${stage3.history.length} messages (filtered)`);

  return JSON.parse(securityReport);
}
```

### Use Case 3: A/B Testing Different Prompts

```javascript
async function comparePrompts(question, codebase) {
  // Build base context
  const baseAgent = new ProbeAgent({
    sessionId: 'base-context',
    path: codebase
  });

  await baseAgent.initialize();
  await baseAgent.answer('Analyze the codebase structure');

  // Clone with different prompt types
  const architectAgent = baseAgent.clone({
    sessionId: 'architect-test',
    overrides: { promptType: 'architect' }
  });

  const reviewerAgent = baseAgent.clone({
    sessionId: 'reviewer-test',
    overrides: { promptType: 'code-review' }
  });

  const supportAgent = baseAgent.clone({
    sessionId: 'support-test',
    overrides: { promptType: 'support' }
  });

  // Initialize all clones
  await Promise.all([
    architectAgent.initialize(),
    reviewerAgent.initialize(),
    supportAgent.initialize()
  ]);

  // Ask the same question with different prompts
  const [architectResponse, reviewerResponse, supportResponse] = await Promise.all([
    architectAgent.answer(question),
    reviewerAgent.answer(question),
    supportAgent.answer(question)
  ]);

  return {
    architect: architectResponse,
    reviewer: reviewerResponse,
    support: supportResponse
  };
}

// Usage
const responses = await comparePrompts(
  'How should we refactor the authentication module?',
  './src'
);

console.log('Architect says:', responses.architect);
console.log('Reviewer says:', responses.reviewer);
console.log('Support says:', responses.support);
```

### Use Case 4: Stateless API with Session Management

```javascript
import express from 'express';
import { ProbeAgent } from '@probelabs/probe';

const app = express();
const sessions = new Map(); // Store base agents

// Endpoint to create a session
app.post('/api/sessions', async (req, res) => {
  const { codebase } = req.body;

  const agent = new ProbeAgent({
    sessionId: `session-${Date.now()}`,
    path: codebase
  });

  await agent.initialize();

  // Build initial context
  await agent.answer('Analyze the project structure');

  sessions.set(agent.sessionId, agent);

  res.json({ sessionId: agent.sessionId });
});

// Endpoint to run a query (clones the session each time)
app.post('/api/query', async (req, res) => {
  const { sessionId, question } = req.body;

  const baseAgent = sessions.get(sessionId);
  if (!baseAgent) {
    return res.status(404).json({ error: 'Session not found' });
  }

  // Clone for this query (keeps base agent clean)
  const queryAgent = baseAgent.clone({
    sessionId: `${sessionId}-query-${Date.now()}`
  });

  await queryAgent.initialize();

  try {
    const answer = await queryAgent.answer(question);
    res.json({ answer });
  } finally {
    // Clean up query agent
    await queryAgent.cleanup();
  }
});

// Endpoint to run structured analysis
app.post('/api/analyze', async (req, res) => {
  const { sessionId, analysisType, schema } = req.body;

  const baseAgent = sessions.get(sessionId);
  if (!baseAgent) {
    return res.status(404).json({ error: 'Session not found' });
  }

  // Clone with custom settings for analysis
  const analysisAgent = baseAgent.clone({
    sessionId: `${sessionId}-analysis-${Date.now()}`,
    overrides: {
      promptType: analysisType, // 'architect', 'code-review', etc.
      maxIterations: 50
    }
  });

  await analysisAgent.initialize();

  try {
    const result = await analysisAgent.answer(
      `Perform ${analysisType} analysis`,
      [],
      { schema: JSON.stringify(schema) }
    );
    res.json({ result: JSON.parse(result) });
  } finally {
    await analysisAgent.cleanup();
  }
});

// Clean up sessions periodically
setInterval(() => {
  const now = Date.now();
  for (const [sessionId, agent] of sessions.entries()) {
    // Remove sessions older than 1 hour
    if (now - agent.createdAt > 3600000) {
      agent.cleanup();
      sessions.delete(sessionId);
    }
  }
}, 300000); // Every 5 minutes

app.listen(3000, () => {
  console.log('API server listening on port 3000');
});
```

## What Gets Filtered?

The `stripInternalMessages` option removes:

### Schema Reminders
```
IMPORTANT: A schema was provided. You MUST respond with data that matches this schema.
Your response must conform to this schema: {...}
```

### Tool Use Prompts
```
Please use one of the available tools to help answer the question...
Remember: Use proper XML format with BOTH opening and closing tags...
```

### Mermaid Fix Prompts
```
The mermaid diagram in your response has syntax errors.
Please fix the mermaid syntax errors...
Here is the corrected version:
```

### JSON Correction Prompts
```
Your response does not match the expected JSON schema.
Please provide a valid JSON response.
Schema validation error: ...
```

### Empty Completion Reminders
```
When using <attempt_complete>, this must be the ONLY content in your response...
```

## What Gets Kept?

- ✅ System message (configurable)
- ✅ User questions
- ✅ Assistant responses
- ✅ Tool calls and results
- ✅ Images and attachments
- ✅ All meaningful conversation content

## Performance Considerations

### Deep Copy vs Shallow Copy

```javascript
// Deep copy (default) - Safe but slower
const clone1 = baseAgent.clone({
  deepCopy: true  // Default
});

// Shallow copy - Faster but be careful with mutations
const clone2 = baseAgent.clone({
  deepCopy: false
});

// If you modify clone2.history, it may affect the original!
```

### Memory Management

```javascript
// Clean up clones when done
async function runManyChecks(baseAgent) {
  for (let i = 0; i < 100; i++) {
    const clone = baseAgent.clone();
    await clone.initialize();
    await clone.answer(`Check ${i}`);
    await clone.cleanup(); // Important: free resources!
  }
}
```

## Debug Output

With `debug: true`, you'll see:

```
[DEBUG] Cloned session base-session -> clone-abc123
[DEBUG] Cloned 5 messages (stripInternal: true)
[DEBUG] Stripping internal message at index 3: user
[DEBUG] Stripping internal message at index 7: user
[DEBUG] Reusing existing system message from history for cache efficiency
```

## TypeScript Support

```typescript
import { ProbeAgent, CloneOptions } from '@probelabs/probe';

const baseAgent = new ProbeAgent({ path: './src' });
await baseAgent.initialize();

const options: CloneOptions = {
  sessionId: 'my-clone',
  stripInternalMessages: true,
  keepSystemMessage: true,
  deepCopy: true,
  overrides: {
    debug: false,
    maxIterations: 30
  }
};

const clonedAgent: ProbeAgent = baseAgent.clone(options);
```

## Summary

The native `clone()` method provides:

- 🧹 **Automatic cleaning** of internal messages
- 🚀 **Cache efficiency** by preserving system message
- ⚙️ **Configuration inheritance** with override support
- 🎯 **Type safety** with proper ProbeAgent instances
- 🐛 **Debug visibility** into what's being filtered
- 🔧 **Flexible options** for different use cases

Perfect for production use cases like Visor, API servers, and parallel processing pipelines!
