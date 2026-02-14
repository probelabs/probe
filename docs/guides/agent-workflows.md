# Agent Workflows Guide

Common patterns and best practices for building AI coding assistants with Probe Agent.

---

## Basic Patterns

### Simple Q&A Agent

```javascript
import { ProbeAgent } from '@probelabs/probe/agent';

async function simpleQA() {
  const agent = new ProbeAgent({
    path: './my-project',
    provider: 'anthropic'
  });

  await agent.initialize();

  try {
    const answer = await agent.answer("How does authentication work?");
    console.log(answer);
  } finally {
    await agent.close();
  }
}
```

### Interactive Chat

```javascript
import readline from 'readline';
import { ProbeAgent } from '@probelabs/probe/agent';

async function interactiveChat() {
  const agent = new ProbeAgent({
    path: './my-project',
    debug: false
  });

  await agent.initialize();

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
  });

  console.log('Chat started. Type "exit" to quit.\n');

  const prompt = () => {
    rl.question('You: ', async (input) => {
      if (input.toLowerCase() === 'exit') {
        await agent.close();
        rl.close();
        return;
      }

      const response = await agent.answer(input);
      console.log(`\nAssistant: ${response}\n`);
      prompt();
    });
  };

  prompt();
}
```

### Streaming Responses

```javascript
async function streamingChat() {
  const agent = new ProbeAgent({ path: './my-project' });
  await agent.initialize();

  process.stdout.write('Assistant: ');

  await agent.answer("Explain the architecture", [], {
    onStream: (chunk) => {
      process.stdout.write(chunk);
    }
  });

  console.log('\n');
}
```

---

## Code Analysis Patterns

### Code Review Bot

```javascript
async function codeReview(filePath) {
  const agent = new ProbeAgent({
    path: process.cwd(),
    promptType: 'code-review',
    allowEdit: false  // Read-only
  });

  await agent.initialize();

  const review = await agent.answer(`
    Review the code in ${filePath}. Focus on:
    1. Potential bugs
    2. Security issues
    3. Performance concerns
    4. Code style
    5. Suggestions for improvement
  `);

  return review;
}

// Usage
const review = await codeReview('src/auth/login.ts');
console.log(review);
```

### Architecture Analyzer

```javascript
async function analyzeArchitecture() {
  const agent = new ProbeAgent({
    path: './my-project',
    promptType: 'architect'
  });

  await agent.initialize();

  const analysis = await agent.answer(`
    Analyze the architecture of this codebase:
    1. What are the main components?
    2. How do they interact?
    3. What design patterns are used?
    4. What are potential improvements?
  `);

  return analysis;
}
```

### Dependency Mapper

```javascript
async function mapDependencies(modulePath) {
  const agent = new ProbeAgent({ path: './my-project' });
  await agent.initialize();

  const deps = await agent.answer(`
    Analyze ${modulePath} and map its dependencies:
    1. What modules does it import?
    2. What modules depend on it?
    3. Are there circular dependencies?
    4. Suggest ways to reduce coupling.
  `, [], {
    schema: `{
      "imports": ["string"],
      "importedBy": ["string"],
      "circular": ["string"],
      "suggestions": ["string"]
    }`
  });

  return JSON.parse(deps);
}
```

---

## Code Modification Patterns

### Refactoring Assistant

```javascript
async function refactorCode(task) {
  const agent = new ProbeAgent({
    path: './my-project',
    allowEdit: true,
    enableBash: true,
    bashConfig: {
      allow: ['npm test', 'npm run lint'],
      deny: ['rm -rf', 'git push']
    }
  });

  await agent.initialize();

  const result = await agent.answer(`
    ${task}

    After making changes:
    1. Run tests to verify
    2. Run linting
    3. Summarize what was changed
  `);

  return result;
}

// Usage
await refactorCode("Extract the validation logic from src/api/users.ts into a separate module");
```

### Bug Fix Workflow

```javascript
async function fixBug(description) {
  const agent = new ProbeAgent({
    path: './my-project',
    allowEdit: true,
    enableBash: true,
    maxIterations: 20  // Allow more iterations for complex fixes
  });

  await agent.initialize();

  // Hook to track progress
  agent.events.on('toolCall', (event) => {
    if (event.status === 'completed') {
      console.log(`✓ ${event.name}`);
    }
  });

  const result = await agent.answer(`
    Fix this bug: ${description}

    Steps:
    1. Search for related code
    2. Identify the root cause
    3. Implement the fix
    4. Run tests
    5. Document what was fixed
  `);

  return result;
}
```

### Feature Implementation

```javascript
async function implementFeature(spec) {
  const agent = new ProbeAgent({
    path: './my-project',
    allowEdit: true,
    enableBash: true,
    enableDelegate: true,  // For complex tasks
    enableTasks: true      // Track subtasks
  });

  await agent.initialize();

  const result = await agent.answer(`
    Implement this feature: ${spec}

    Requirements:
    1. Follow existing code patterns
    2. Add appropriate tests
    3. Update documentation if needed
    4. Ensure all tests pass
  `);

  return result;
}
```

---

## Multi-Provider Patterns

### Resilient Agent

```javascript
async function resilientAgent() {
  const agent = new ProbeAgent({
    path: './my-project',
    provider: 'anthropic',
    retry: {
      maxRetries: 3,
      initialDelay: 1000,
      backoffFactor: 2
    },
    fallback: {
      strategy: 'any',
      stopOnSuccess: true
    }
  });

  await agent.initialize();

  // Will retry on errors, then try other providers
  const response = await agent.answer("Analyze this code");
  return response;
}
```

### Provider-Specific Routing

```javascript
async function routedAgent(task, complexity) {
  const config = {
    path: './my-project'
  };

  // Route to appropriate model based on complexity
  if (complexity === 'simple') {
    config.provider = 'anthropic';
    config.model = 'claude-3-haiku-20240307';  // Fast, cost-effective
  } else if (complexity === 'complex') {
    config.provider = 'anthropic';
    config.model = 'claude-sonnet-4-5-20250929';
  }

  const agent = new ProbeAgent(config);
  await agent.initialize();

  return await agent.answer(task);
}
```

---

## Session Management Patterns

### Persistent Conversations

```javascript
import fs from 'fs/promises';

class FileStorageAdapter {
  constructor(dir) {
    this.dir = dir;
  }

  async loadHistory(sessionId) {
    try {
      const data = await fs.readFile(`${this.dir}/${sessionId}.json`, 'utf-8');
      return JSON.parse(data);
    } catch {
      return [];
    }
  }

  async saveMessage(sessionId, message) {
    const history = await this.loadHistory(sessionId);
    history.push(message);
    await fs.writeFile(
      `${this.dir}/${sessionId}.json`,
      JSON.stringify(history, null, 2)
    );
  }

  async clearHistory(sessionId) {
    await fs.unlink(`${this.dir}/${sessionId}.json`).catch(() => {});
  }
}

// Usage
const agent = new ProbeAgent({
  path: './my-project',
  sessionId: 'user-123-session',
  storageAdapter: new FileStorageAdapter('./sessions')
});
```

### Context Management

```javascript
async function managedContext() {
  const agent = new ProbeAgent({ path: './my-project' });
  await agent.initialize();

  // Have a conversation
  await agent.answer("What is this project about?");
  await agent.answer("How does authentication work?");
  await agent.answer("Show me the login function");

  // Check token usage
  const usage = agent.getTokenUsage();
  console.log(`Context: ${usage.contextWindow} tokens`);

  // Compact if needed
  if (usage.contextWindow > 50000) {
    const stats = await agent.compactHistory();
    console.log(`Saved ${stats.tokensSaved} tokens`);
  }

  // Continue conversation
  await agent.answer("Now explain the logout process");
}
```

---

## Event-Driven Patterns

### Progress Tracking

```javascript
async function trackedExecution() {
  const agent = new ProbeAgent({
    path: './my-project',
    allowEdit: true
  });

  // Track tool calls
  const toolStats = new Map();

  agent.events.on('toolCall', (event) => {
    if (!toolStats.has(event.name)) {
      toolStats.set(event.name, { count: 0, totalTime: 0 });
    }

    if (event.status === 'completed') {
      const stats = toolStats.get(event.name);
      stats.count++;
      stats.totalTime += event.duration;
    }
  });

  await agent.initialize();
  await agent.answer("Refactor the user service");

  // Print stats
  console.log('\nTool Usage:');
  for (const [tool, stats] of toolStats) {
    console.log(`${tool}: ${stats.count} calls, ${stats.totalTime}ms total`);
  }
}
```

### Logging Middleware

```javascript
async function loggedAgent() {
  const agent = new ProbeAgent({
    path: './my-project'
  });

  // Use events API for tool call monitoring
  agent.events.on('toolCall', (event) => {
    if (event.status === 'in_progress') {
      console.log(`[TOOL] Starting: ${event.name}`);
    } else if (event.status === 'completed') {
      console.log(`[TOOL] Completed: ${event.name} (${event.duration}ms)`);
    } else if (event.status === 'failed') {
      console.error(`[ERROR] ${event.name}: ${event.error}`);
    }
  });

  await agent.initialize();
  return agent;
}
```

---

## Integration Patterns

### MCP Tool Extension

```javascript
async function extendedAgent() {
  const agent = new ProbeAgent({
    path: './my-project',
    enableMcp: true,
    mcpConfig: {
      mcpServers: {
        'github': {
          command: 'npx',
          args: ['-y', '@modelcontextprotocol/server-github'],
          transport: 'stdio',
          enabled: true,
          env: { GITHUB_TOKEN: process.env.GITHUB_TOKEN }
        },
        'database': {
          command: 'npx',
          args: ['-y', '@modelcontextprotocol/server-postgres'],
          transport: 'stdio',
          enabled: true,
          env: { DATABASE_URL: process.env.DATABASE_URL }
        }
      }
    }
  });

  await agent.initialize();

  // Agent now has access to GitHub and database tools
  const result = await agent.answer(`
    Find all TODO comments in the codebase and
    create a GitHub issue for each one.
  `);

  return result;
}
```

### API Integration

```javascript
import express from 'express';
import { ProbeAgent } from '@probelabs/probe/agent';

const app = express();
app.use(express.json());

// Agent pool for concurrent requests
const agents = new Map();

app.post('/api/chat', async (req, res) => {
  const { sessionId, message, projectPath } = req.body;

  // Get or create agent for session
  let agent = agents.get(sessionId);
  if (!agent) {
    agent = new ProbeAgent({
      path: projectPath,
      sessionId
    });
    await agent.initialize();
    agents.set(sessionId, agent);
  }

  try {
    const response = await agent.answer(message);
    res.json({
      response,
      usage: agent.getTokenUsage()
    });
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Cleanup endpoint
app.delete('/api/session/:sessionId', async (req, res) => {
  const agent = agents.get(req.params.sessionId);
  if (agent) {
    await agent.close();
    agents.delete(req.params.sessionId);
  }
  res.json({ success: true });
});

app.listen(3000);
```

---

## Best Practices

### 1. Always Clean Up

```javascript
const agent = new ProbeAgent({ path: './src' });
try {
  await agent.initialize();
  // ... use agent
} finally {
  await agent.close();  // Always clean up
}
```

### 2. Handle Errors Gracefully

```javascript
try {
  const response = await agent.answer(question);
} catch (error) {
  if (error.message.includes('rate limit')) {
    // Wait and retry
    await sleep(5000);
    return await agent.answer(question);
  }
  throw error;
}
```

### 3. Monitor Token Usage

```javascript
const usage = agent.getTokenUsage();
if (usage.contextWindow > MAX_CONTEXT) {
  await agent.compactHistory();
}
```

### 4. Use Appropriate Tool Permissions

```javascript
// Read-only for analysis
const analyzerAgent = new ProbeAgent({
  allowEdit: false,
  enableBash: false
});

// Full access for modifications
const editorAgent = new ProbeAgent({
  allowEdit: true,
  enableBash: true,
  bashConfig: {
    allow: ['npm test', 'npm run build'],
    deny: ['rm -rf', 'sudo']
  }
});
```

---

## Related Documentation

- [SDK Getting Started](../probe-agent/sdk/getting-started.md) - Quick start
- [API Reference](../probe-agent/sdk/api-reference.md) - Full API docs
- [Tools Reference](../probe-agent/sdk/tools-reference.md) - Available tools
- [Hooks System](../probe-agent/sdk/hooks.md) - Event handling
