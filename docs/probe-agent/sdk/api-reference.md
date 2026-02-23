# API Reference

Complete API documentation for the ProbeAgent SDK.

---

## TL;DR

```javascript
import { ProbeAgent, search, extract, query, tools } from '@probelabs/probe';

// Direct functions
const results = await search({ path: './src', query: 'authentication' });

// AI Agent
const agent = new ProbeAgent({ path: './src', provider: 'anthropic' });
const response = await agent.answer('How does authentication work?');
```

---

## ProbeAgent Class

The main AI-powered code assistant class.

### Constructor

```javascript
const agent = new ProbeAgent(options?: ProbeAgentOptions);
```

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `sessionId` | `string` | Unique session identifier (readonly) |
| `history` | `ChatMessage[]` | Conversation history (read/write) |
| `events` | `EventEmitter` | Tool execution event emitter (readonly) |
| `allowEdit` | `boolean` | Whether editing is enabled (readonly) |
| `allowedFolders` | `string[]` | Allowed search folders (readonly) |
| `cwd` | `string \| null` | Working directory (readonly) |
| `debug` | `boolean` | Debug mode status (readonly) |
| `cancelled` | `boolean` | Cancellation status (read/write) |
| `clientApiProvider` | `string` | AI provider being used (readonly) |
| `model` | `string` | Current AI model (readonly) |

### Methods

#### initialize()

Initialize the agent asynchronously. Must be called after constructor.

```javascript
await agent.initialize();
```

Handles:
- API key validation
- CLI fallback detection
- MCP initialization
- History loading

---

#### answer()

Get an AI response to a question.

```javascript
const response = await agent.answer(
  message: string,
  images?: any[],
  options?: AnswerOptions
): Promise<string>
```

**Parameters:**

| Name | Type | Description |
|------|------|-------------|
| `message` | `string` | The question or prompt |
| `images` | `any[]` | Optional image attachments |
| `options` | `AnswerOptions` | Additional options |

**AnswerOptions:**

```javascript
{
  schema?: string;       // JSON schema for structured output
  context?: string;      // Additional context
  maxIterations?: number // Max tool iterations
}
```

---

#### getTokenUsage()

Get current token usage statistics.

```javascript
const usage = agent.getTokenUsage(): TokenUsage;
```

**Returns:**

```javascript
{
  contextWindow?: number,
  request?: number,
  response?: number,
  total?: number,
  cacheRead?: number,
  cacheWrite?: number,
  totalRequest?: number,
  totalResponse?: number,
  totalTokens?: number
}
```

---

#### cancel()

Cancel ongoing operations.

```javascript
agent.cancel(): void;
```

---

#### clearHistory()

Clear conversation history.

```javascript
agent.clearHistory(): void;
```

---

#### history (property)

Access and modify conversation history directly.

```javascript
// Read history
const messages = agent.history;

// Replace history
agent.history = newMessages;

// Add message
agent.history.push({ role: 'user', content: '...' });
```

---

#### clone()

Create a new agent with shared history.

```javascript
const cloned = agent.clone(options?: CloneOptions): ProbeAgent;
```

**CloneOptions:**

```javascript
{
  sessionId?: string,
  stripInternalMessages?: boolean,
  keepSystemMessage?: boolean,
  deepCopy?: boolean,
  overrides?: Partial<ProbeAgentOptions>
}
```

---

## ProbeAgentOptions

Configuration for ProbeAgent constructor.

```javascript
interface ProbeAgentOptions {
  // Session
  sessionId?: string;

  // Prompts
  customPrompt?: string;
  systemPrompt?: string;
  promptType?: 'code-explorer' | 'code-searcher' | 'engineer' |
               'code-review' | 'support' | 'architect';
  completionPrompt?: string;
  architectureFileName?: string;

  // Features
  allowEdit?: boolean;
  enableDelegate?: boolean;
  enableBash?: boolean;
  allowSkills?: boolean;
  enableTasks?: boolean;
  searchDelegate?: boolean;

  // Bash Configuration
  // Priority: custom deny > custom allow > default deny > allow list
  // See security guide for details
  bashConfig?: {
    allow?: string[];                // Override default deny (e.g., ['git:push'])
    deny?: string[];                 // Always block (e.g., ['git:push:--force'])
    disableDefaultAllow?: boolean;   // Remove built-in allow list
    disableDefaultDeny?: boolean;    // Remove built-in deny list (not recommended)
    debug?: boolean;
  };

  // Path & Provider
  path?: string;
  provider?: 'anthropic' | 'openai' | 'google' | 'bedrock';
  model?: string;

  // MCP Integration
  enableMcp?: boolean;
  mcpConfigPath?: string;
  mcpConfig?: any;

  // Tools
  allowedTools?: string[] | null;
  disableTools?: boolean;

  // Reliability
  retry?: RetryOptions;
  fallback?: FallbackOptions | { auto: boolean };

  // Validation
  disableMermaidValidation?: boolean;
  disableJsonValidation?: boolean;

  // Skills
  skillDirs?: string[];
  disableSkills?: boolean;

  // Timeouts
  requestTimeout?: number;
  maxOperationTimeout?: number;

  // Debug
  debug?: boolean;
  tracer?: any;
}
```

---

## Data Types

### ChatMessage

```javascript
interface ChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
  metadata?: any;
}
```

### ToolCallEvent

```javascript
interface ToolCallEvent {
  id: string;
  name: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  params?: any;
  result?: any;
  error?: string;
  sessionId?: string;
  startTime?: number;
  endTime?: number;
  duration?: number;
}
```

---

## Event System

### Listening for Tool Calls

```javascript
agent.events.on('toolCall', (event: ToolCallEvent) => {
  console.log(`Tool: ${event.name}, Status: ${event.status}`);
  if (event.status === 'completed') {
    console.log(`Result: ${event.result}`);
  }
});
```

### Available Events

| Event | Description |
|-------|-------------|
| `toolCall` | Emitted on tool execution updates |

---

## Direct Functions

### search()

Semantic code search.

```javascript
const results = await search(
  query: string,
  path?: string,
  options?: {
    maxResults?: number;
    timeout?: number;
    allowTests?: boolean;
  }
): Promise<SearchResult[]>
```

---

### query()

AST-based pattern matching.

```javascript
const results = await query(
  pattern: string,
  path?: string,
  options?: {
    language?: string;
    maxResults?: number;
  }
): Promise<QueryResult[]>
```

---

### extract()

Extract code blocks from files.

```javascript
const code = await extract(
  files: string[],
  path?: string,
  options?: {
    contextLines?: number;
    format?: 'plain' | 'markdown' | 'json' | 'xml';
  }
): Promise<ExtractResult>
```

---

### grep()

Ripgrep-style search.

```javascript
const results = await grep({
  pattern: string,
  paths: string | string[],
  ignoreCase?: boolean,
  lineNumbers?: boolean,
  context?: number,
  maxCount?: number
}): Promise<string>
```

---

## Tool Definitions

### Creating Tools

```javascript
import { tools } from '@probelabs/probe';

const { searchTool, queryTool, extractTool } = tools;

// For Vercel AI SDK
const toolSet = {
  search: searchTool({ defaultPath: './src' }),
  query: queryTool({ defaultPath: './src' }),
  extract: extractTool({ defaultPath: './src' })
};
```

### Tool Options

```javascript
interface SearchOptions {
  sessionId?: string;
  debug?: boolean;
  defaultPath?: string;
  allowedFolders?: string[];
}
```

---

## Retry Manager

### Configuration

```javascript
interface RetryOptions {
  maxRetries?: number;      // 0-100, default: 3
  initialDelay?: number;    // ms, default: 1000
  maxDelay?: number;        // ms, default: 30000
  backoffFactor?: number;   // 1-10, default: 2
  retryableErrors?: string[];
  debug?: boolean;
  jitter?: boolean;         // default: true
}
```

### Usage

```javascript
const agent = new ProbeAgent({
  retry: {
    maxRetries: 5,
    initialDelay: 1000,
    backoffFactor: 2
  }
});
```

---

## Fallback Manager

### Configuration

```javascript
interface FallbackOptions {
  strategy?: 'same-model' | 'same-provider' | 'any' | 'custom';
  models?: string[];
  providers?: ProviderConfig[];
  stopOnSuccess?: boolean;
  continueOnNonRetryableError?: boolean;
  maxTotalAttempts?: number;  // 1-100
  debug?: boolean;
}
```

### Provider Configuration

```javascript
interface ProviderConfig {
  provider: 'anthropic' | 'openai' | 'google' | 'bedrock';
  model?: string;
  apiKey?: string;
  baseURL?: string;
  maxRetries?: number;
  // AWS Bedrock specific
  region?: string;
  accessKeyId?: string;
  secretAccessKey?: string;
}
```

### Usage

```javascript
const agent = new ProbeAgent({
  fallback: {
    strategy: 'custom',
    providers: [
      { provider: 'anthropic', model: 'claude-sonnet-4-6' },
      { provider: 'openai', model: 'gpt-5.2' }
    ]
  }
});
```

---

## Telemetry

### TelemetryConfig

```javascript
const telemetry = new TelemetryConfig({
  enableFile?: boolean,
  filePath?: string,
  enableRemote?: boolean,
  remoteEndpoint?: string,
  enableConsole?: boolean,
  serviceName?: string,
  serviceVersion?: string
});

telemetry.initialize();
const tracer = telemetry.getTracer();
```

### AppTracer

```javascript
const tracer = new AppTracer(telemetryConfig, sessionId);

// Create spans
tracer.createSessionSpan({ sessionId });
tracer.createAISpan(modelName, provider);
tracer.createToolSpan(toolName);
tracer.createSearchSpan(query);
tracer.createExtractSpan(files);
tracer.createDelegationSpan(task);

// Record events
tracer.addEvent('event_name', { key: value });

// Flush and shutdown
await tracer.flush();
await tracer.shutdown();
```

---

## Hook Manager

### Usage

```javascript
const hooks = new HookManager();

// Register hook
const unsubscribe = hooks.on('tool:start', async (data) => {
  console.log('Tool starting:', data.name);
});

// One-time hook
hooks.once('agent:initialized', (data) => {
  console.log('Agent ready');
});

// Remove hook
hooks.off('tool:start', callback);

// Clear hooks
hooks.clear('tool:start');  // Specific hook
hooks.clear();              // All hooks
```

### Hook Types

```javascript
const HOOK_TYPES = {
  AGENT_INITIALIZED: 'agent:initialized',
  AGENT_CLEANUP: 'agent:cleanup',
  MESSAGE_USER: 'message:user',
  MESSAGE_ASSISTANT: 'message:assistant',
  MESSAGE_SYSTEM: 'message:system',
  TOOL_START: 'tool:start',
  TOOL_END: 'tool:end',
  TOOL_ERROR: 'tool:error',
  AI_STREAM_START: 'ai:stream:start',
  AI_STREAM_DELTA: 'ai:stream:delta',
  AI_STREAM_END: 'ai:stream:end',
  STORAGE_LOAD: 'storage:load',
  STORAGE_SAVE: 'storage:save',
  STORAGE_CLEAR: 'storage:clear',
  ITERATION_START: 'iteration:start',
  ITERATION_END: 'iteration:end'
};
```

---

## Error Types

```javascript
class ProbeError extends Error {}
class PathError extends ProbeError {}
class ParameterError extends ProbeError {}
class TimeoutError extends ProbeError {}
class ApiError extends ProbeError {}
class DelegationError extends ProbeError {}
```

---

## Constants

### Timeouts

```javascript
const ENGINE_ACTIVITY_TIMEOUT_DEFAULT = 180000;  // 3 minutes
const ENGINE_ACTIVITY_TIMEOUT_MIN = 5000;        // 5 seconds
const ENGINE_ACTIVITY_TIMEOUT_MAX = 600000;      // 10 minutes
```

### Limits

```javascript
const MAX_TOOL_ITERATIONS = 30;
const MAX_HISTORY_MESSAGES = 20;
```

---

## Utility Functions

### Binary Management

```javascript
import { getBinaryPath, setBinaryPath } from '@probelabs/probe';

const path = getBinaryPath();
setBinaryPath('/custom/path/to/probe');
```

### File Listing

```javascript
import { listFilesByLevel } from '@probelabs/probe';

const files = await listFilesByLevel('./src', {
  maxLevel: 3,
  includeHidden: false
});
```

### Error Utilities

```javascript
import { isRetryableError, extractErrorInfo } from '@probelabs/probe';

if (isRetryableError(error)) {
  // Retry logic
}

const info = extractErrorInfo(error);
console.log(info.message, info.code, info.retryable);
```

---

## Environment Variables

### API Keys

```bash
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
GOOGLE_API_KEY=...
```

### Configuration

```bash
FORCE_PROVIDER=anthropic
MODEL_NAME=claude-sonnet-4-6
REQUEST_TIMEOUT=120000
MAX_OPERATION_TIMEOUT=600000
DEBUG_CHAT=1
ALLOWED_FOLDERS=/path1,/path2
```

---

## Related Documentation

- [Getting Started](./getting-started.md) - Quick start guide
- [Tools Reference](./tools-reference.md) - Available tools
- [Retry & Fallback](./retry-fallback.md) - Reliability configuration
- [Hooks](./hooks.md) - Event hooks

