# SDK Quick Start

Build your first AI coding assistant with ProbeAgent in under 5 minutes.

---

## TL;DR

```bash
npm install @probelabs/probe
```

```javascript
import { ProbeAgent } from '@probelabs/probe/agent';

const agent = new ProbeAgent({ path: './my-project' });
await agent.initialize();
const response = await agent.answer("How does authentication work?");
console.log(response);
```

---

## Installation

```bash
# Using npm
npm install @probelabs/probe

# Using yarn
yarn add @probelabs/probe

# Using pnpm
pnpm add @probelabs/probe
```

### Prerequisites

- Node.js 18+
- At least one AI provider API key:
  - `ANTHROPIC_API_KEY` (Claude)
  - `OPENAI_API_KEY` (GPT)
  - `GOOGLE_GENERATIVE_AI_API_KEY` (Gemini)
  - Or `claude` / `codex` CLI installed

---

## Basic Usage

### 1. Create an Agent

```javascript
import { ProbeAgent } from '@probelabs/probe/agent';

const agent = new ProbeAgent({
  path: './my-project',           // Directory to search
  provider: 'anthropic',          // AI provider (optional, auto-detected)
});

// Initialize must be called before use
await agent.initialize();
```

### 2. Ask Questions

```javascript
// Simple question
const response = await agent.answer("What does the login function do?");
console.log(response);

// Follow-up questions (maintains context)
const followUp = await agent.answer("How does it handle errors?");
console.log(followUp);
```

### 3. Clean Up

```javascript
// Always clean up when done
await agent.close();
```

---

## Complete Example

```javascript
import { ProbeAgent } from '@probelabs/probe/agent';

async function exploreCodebase() {
  const agent = new ProbeAgent({
    path: process.cwd(),
    provider: 'anthropic',
    debug: true  // Enable debug logging
  });

  try {
    await agent.initialize();

    // Explore the codebase
    console.log(await agent.answer("Give me an overview of this project"));
    console.log(await agent.answer("What are the main entry points?"));
    console.log(await agent.answer("How is error handling implemented?"));

    // Get token usage
    const usage = agent.getTokenUsage();
    console.log(`Total tokens used: ${usage.total.total}`);

  } finally {
    await agent.close();
  }
}

exploreCodebase();
```

---

## Configuration Options

### Essential Options

```javascript
const agent = new ProbeAgent({
  // Required
  path: './src',                    // Directory to search

  // Provider Configuration
  provider: 'anthropic',            // 'anthropic', 'openai', 'google', 'bedrock'
  model: 'claude-sonnet-4-6',  // Override default model

  // Behavior
  debug: false,                     // Enable debug output
  maxIterations: 30,                // Max tool iterations per question
  requestTimeout: 120000,           // Request timeout (ms)
});
```

### Enable Code Editing

```javascript
const agent = new ProbeAgent({
  path: './src',
  allowEdit: true,     // Enable edit/create tools
  enableBash: true,    // Enable bash command execution
  bashConfig: {
    allow: ['npm test', 'npm run build'],  // Allowed commands
    deny: ['rm -rf']                        // Blocked commands
  }
});
```

### Custom System Prompt

```javascript
const agent = new ProbeAgent({
  path: './src',
  customPrompt: `You are a code review assistant focused on security.
    When analyzing code, prioritize:
    1. SQL injection vulnerabilities
    2. XSS attack vectors
    3. Authentication weaknesses`,
  // Or use a preset:
  // promptType: 'code-review'  // 'engineer', 'architect', 'support'
});
```

### Multi-Provider with Fallback

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'anthropic',
  retry: {
    maxRetries: 3,
    initialDelay: 1000,
    backoffFactor: 2
  },
  fallback: {
    strategy: 'any',  // Try any available provider
    // Or custom provider list:
    // providers: [
    //   { provider: 'anthropic', model: 'claude-sonnet-4-6' },
    //   { provider: 'openai', model: 'gpt-5.2' }
    // ]
  }
});
```

---

## Streaming Responses

```javascript
const response = await agent.answer("Explain the architecture", [], {
  onStream: (chunk) => {
    process.stdout.write(chunk);  // Print as it arrives
  }
});
```

---

## Handling Events

```javascript
// Listen for tool calls
agent.events.on('toolCall', (event) => {
  console.log(`Tool: ${event.name}, Status: ${event.status}`);
  if (event.status === 'completed') {
    console.log(`Duration: ${event.duration}ms`);
  }
});

// Use hooks for more control
const agent = new ProbeAgent({
  path: './src',
  hooks: {
    'tool:start': (data) => console.log(`Starting: ${data.name}`),
    'tool:end': (data) => console.log(`Finished: ${data.name}`),
    'message:assistant': (data) => console.log(`AI: ${data.content}`)
  }
});
```

---

## Session Persistence

```javascript
import { ProbeAgent, InMemoryStorageAdapter } from '@probelabs/probe/agent';

// Default: in-memory storage
const agent = new ProbeAgent({
  path: './src',
  sessionId: 'my-session-123'  // Reuse for conversation continuity
});

// Custom storage adapter (e.g., database)
class MyDatabaseAdapter extends StorageAdapter {
  async loadHistory(sessionId) {
    return await db.messages.findAll({ where: { sessionId } });
  }
  async saveMessage(sessionId, message) {
    await db.messages.create({ sessionId, ...message });
  }
  async clearHistory(sessionId) {
    await db.messages.destroy({ where: { sessionId } });
  }
}

const agent = new ProbeAgent({
  path: './src',
  storageAdapter: new MyDatabaseAdapter()
});
```

---

## Token Management

```javascript
// Get current usage
const usage = agent.getTokenUsage();
console.log(`Context window: ${usage.contextWindow}`);
console.log(`Current request: ${usage.current.request}`);
console.log(`Total usage: ${usage.total.total}`);
console.log(`Cache hits: ${usage.total.cacheRead}`);

// Compact history to save tokens
const stats = await agent.compactHistory();
console.log(`Saved ${stats.tokensSaved} tokens`);

// Clear history entirely
await agent.clearHistory();
```

---

## Error Handling

```javascript
import { ProbeAgent } from '@probelabs/probe/agent';

try {
  const agent = new ProbeAgent({ path: './src' });
  await agent.initialize();

  const response = await agent.answer("Analyze this code");

} catch (error) {
  if (error.message.includes('API key')) {
    console.error('Missing API key. Set ANTHROPIC_API_KEY or OPENAI_API_KEY');
  } else if (error.message.includes('rate limit')) {
    console.error('Rate limited. Wait and retry.');
  } else {
    console.error('Error:', error.message);
  }
}
```

---

## TypeScript Support

ProbeAgent includes full TypeScript definitions:

```typescript
import {
  ProbeAgent,
  ProbeAgentOptions,
  TokenUsage,
  ChatMessage,
  ToolCallEvent
} from '@probelabs/probe/agent';

const options: ProbeAgentOptions = {
  path: './src',
  provider: 'anthropic',
  debug: true
};

const agent = new ProbeAgent(options);
await agent.initialize();

const usage: TokenUsage = agent.getTokenUsage();
```

---

## Common Patterns

### Code Review Bot

```javascript
const agent = new ProbeAgent({
  path: './src',
  promptType: 'code-review',
  allowEdit: false  // Read-only for safety
});

await agent.initialize();
const review = await agent.answer(`
  Review the recent changes in src/auth.ts.
  Focus on security issues and best practices.
`);
```

### Documentation Generator

```javascript
const agent = new ProbeAgent({
  path: './src',
  customPrompt: 'You are a technical writer. Generate clear documentation.'
});

await agent.initialize();
const docs = await agent.answer(`
  Generate API documentation for the UserService class.
  Include method signatures, parameters, and examples.
`);
```

### Interactive CLI Tool

```javascript
import readline from 'readline';

const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout
});

const agent = new ProbeAgent({ path: './src' });
await agent.initialize();

function prompt() {
  rl.question('You: ', async (input) => {
    if (input === 'exit') {
      await agent.close();
      rl.close();
      return;
    }
    const response = await agent.answer(input);
    console.log(`Assistant: ${response}\n`);
    prompt();
  });
}

prompt();
```

---

## Next Steps

- [API Reference](./api-reference.md) - Complete ProbeAgent documentation
- [Tools Reference](./tools-reference.md) - Available tools and parameters
- [Engines & Providers](./engines.md) - Configure AI providers
- [Hooks System](./hooks.md) - Event hooks for customization
- [Retry & Fallback](./retry-fallback.md) - Error recovery strategies
