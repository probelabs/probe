# Probe Agent Overview

Probe Agent is an AI-powered code exploration and interaction platform built on Node.js. It combines Probe's semantic code search capabilities with intelligent agent orchestration, enabling you to build sophisticated AI coding assistants.

---

## What is Probe Agent?

Probe Agent provides:

- **AI-Powered Code Understanding**: Leverage LLMs to explore, explain, and modify code
- **Multi-Provider Support**: Works with Anthropic Claude, OpenAI GPT, Google Gemini, AWS Bedrock, Claude Code, and Codex
- **Tool Orchestration**: Automatic tool calling for search, extraction, editing, and more
- **Session Management**: Persistent conversations with history and context compaction
- **Protocol Support**: MCP and ACP for integration with AI editors and agent systems

---

## TL;DR - Quick Example

```javascript
import { ProbeAgent } from '@probelabs/probe/agent';

const agent = new ProbeAgent({
  path: './my-project',
  provider: 'anthropic'
});

await agent.initialize();
const response = await agent.answer("How does authentication work in this codebase?");
console.log(response);
```

---

## Agent Flavors

Probe Agent comes in several forms to suit different use cases:

### 1. ProbeAgent SDK

The core Node.js class for programmatic integration:

```bash
npm install @probelabs/probe
```

```javascript
import { ProbeAgent } from '@probelabs/probe/agent';
```

**Use when**: Building custom AI tools, integrating into existing applications, or creating specialized workflows.

### 2. Probe Chat CLI

Interactive terminal-based chat interface:

```bash
npx -y @probelabs/probe-chat@latest ./my-project
```

**Use when**: Exploring codebases interactively, quick code questions, or developer productivity.

### 3. Probe Chat Web

Browser-based chat interface with syntax highlighting:

```bash
npx -y @probelabs/probe-chat@latest --web ./my-project
```

**Use when**: Team collaboration, sharing code insights, or when you prefer a visual interface.

### 4. MCP Server

Model Context Protocol server for AI editor integration:

```bash
probe mcp
```

**Use when**: Integrating with Cursor, Windsurf, Claude Code, or other MCP-compatible AI editors.

### 5. ACP Server

Agent Communication Protocol for advanced agent orchestration:

```bash
node index.js --acp
```

**Use when**: Building multi-agent systems, complex workflows, or custom AI platforms.

---

## Key Capabilities

### Intelligent Code Search

Probe Agent uses semantic search to find relevant code:

```javascript
const agent = new ProbeAgent({ path: './src' });
await agent.initialize();

// The agent automatically uses search tools
const response = await agent.answer("Find all functions that handle user authentication");
```

### Code Extraction & Context

Extract full function and class definitions with context:

```javascript
const response = await agent.answer("Extract the login function and explain how it works");
```

### Code Editing (Optional)

Enable code modifications with safeguards:

```javascript
const agent = new ProbeAgent({
  path: './src',
  allowEdit: true,
  enableBash: true
});
```

### Multi-Provider Resilience

Automatic retry and fallback across providers:

```javascript
const agent = new ProbeAgent({
  provider: 'anthropic',
  retry: { maxRetries: 3, backoffFactor: 2 },
  fallback: { strategy: 'any' }
});
```

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      Your Application                        │
└─────────────────────────────┬───────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────┐
│                       ProbeAgent SDK                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Engines   │  │    Tools    │  │  Session Manager    │  │
│  │  (Claude,   │  │  (Search,   │  │  (History, Tokens,  │  │
│  │   OpenAI,   │  │   Extract,  │  │   Compaction)       │  │
│  │   Gemini)   │  │   Edit...)  │  │                     │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
└─────────┼────────────────┼────────────────────┼─────────────┘
          │                │                    │
┌─────────▼────────────────▼────────────────────▼─────────────┐
│                      Probe CLI (Rust)                        │
│         Semantic Search • AST Parsing • Code Extraction      │
└─────────────────────────────────────────────────────────────┘
```

---

## Supported Providers

| Provider | Type | Configuration |
|----------|------|---------------|
| **Anthropic Claude** | API | `ANTHROPIC_API_KEY` |
| **OpenAI GPT** | API | `OPENAI_API_KEY` |
| **Google Gemini** | API | `GOOGLE_GENERATIVE_AI_API_KEY` |
| **AWS Bedrock** | API | AWS credentials |
| **Claude Code** | CLI | Requires `claude` command |
| **Codex** | CLI | Requires `codex` command |

---

## Feature Matrix

| Feature | SDK | Chat CLI | Chat Web | MCP | ACP |
|---------|-----|----------|----------|-----|-----|
| Code Search | ✓ | ✓ | ✓ | ✓ | ✓ |
| Code Extraction | ✓ | ✓ | ✓ | ✓ | ✓ |
| AST Queries | ✓ | ✓ | ✓ | ✓ | ✓ |
| Code Editing | ✓ | ✓ | ✓ | - | ✓ |
| Bash Execution | ✓ | ✓ | ✓ | - | - |
| Session Persistence | ✓ | ✓ | ✓ | - | ✓ |
| Multi-Provider | ✓ | ✓ | ✓ | - | ✓ |
| Streaming | ✓ | ✓ | ✓ | - | ✓ |
| Token Tracking | ✓ | ✓ | ✓ | - | ✓ |
| Delegation | ✓ | - | - | - | ✓ |

---

## When to Use What

### Use ProbeAgent SDK when:
- Building custom AI applications
- Integrating into existing Node.js projects
- Need full control over agent behavior
- Creating specialized workflows

### Use Probe Chat when:
- Interactive code exploration
- Quick questions about a codebase
- Developer productivity
- Team code reviews

### Use MCP Server when:
- Working with AI code editors (Cursor, Windsurf)
- Want Probe search in Claude Code
- Need standardized tool integration

### Use ACP Server when:
- Building multi-agent systems
- Need session management and isolation
- Creating custom AI platforms
- Complex orchestration requirements

---

## Next Steps

- [SDK Quick Start](./sdk/getting-started.md) - Build your first agent
- [API Reference](./sdk/api-reference.md) - Complete ProbeAgent documentation
- [Tools Reference](./sdk/tools-reference.md) - Available tools and parameters
- [MCP Protocol](./protocols/mcp.md) - MCP server setup and integration
- [ACP Protocol](./protocols/acp.md) - Agent Communication Protocol guide
