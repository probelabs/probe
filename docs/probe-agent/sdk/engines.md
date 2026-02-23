# Engines & Providers

ProbeAgent supports multiple AI providers and execution engines. This document covers provider configuration, model selection, and multi-provider strategies.

---

## TL;DR

```javascript
// Auto-detect provider from environment
const agent = new ProbeAgent({ path: './src' });

// Explicit provider
const agent = new ProbeAgent({
  path: './src',
  provider: 'anthropic',
  model: 'claude-sonnet-4-6'
});
```

---

## Supported Providers

### API-Based Providers

| Provider | Environment Variable | Default Model |
|----------|---------------------|---------------|
| Anthropic | `ANTHROPIC_API_KEY` | `claude-sonnet-4-6` |
| OpenAI | `OPENAI_API_KEY` | `gpt-5.2` |
| Google | `GOOGLE_GENERATIVE_AI_API_KEY` | `gemini-2.5-flash` |
| AWS Bedrock | AWS credentials | `anthropic.claude-sonnet-4-6` |

### CLI-Based Providers

| Provider | Requirement | Description |
|----------|-------------|-------------|
| Claude Code | `claude` CLI installed | Uses Claude Code's built-in access |
| Codex | `codex` CLI installed | Uses Codex CLI |

---

## Provider Configuration

### Anthropic Claude

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'anthropic',
  model: 'claude-sonnet-4-6'  // Optional
});
```

**Available Models:**
- `claude-sonnet-4-6` (default)
- `claude-opus-4-20250514`
- `claude-3-haiku-20240307`

### OpenAI

```bash
export OPENAI_API_KEY=sk-...
```

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'openai',
  model: 'gpt-5.2'  // Optional
});
```

**Available Models:**
- `gpt-5.2` (default)
- `gpt-5.2-mini`
- `gpt-4-turbo`

### Google Gemini

```bash
export GOOGLE_GENERATIVE_AI_API_KEY=...
```

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'google',
  model: 'gemini-2.5-flash'  // Optional
});
```

**Available Models:**
- `gemini-2.5-flash` (default)
- `gemini-1.5-pro`
- `gemini-1.5-flash`

### AWS Bedrock

```bash
export AWS_ACCESS_KEY_ID=...
export AWS_SECRET_ACCESS_KEY=...
export AWS_REGION=us-east-1
# Optional: export AWS_SESSION_TOKEN=...
```

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'bedrock',
  model: 'anthropic.claude-sonnet-4-6'  // Optional
});
```

### Claude Code (CLI)

Requires `claude` CLI to be installed and authenticated:

```bash
# Verify installation
which claude
claude --version
```

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'claude-code'
});
```

### Codex (CLI)

Requires `codex` CLI to be installed:

```bash
which codex
```

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'codex'
});
```

---

## Auto-Detection

If no provider is specified, ProbeAgent auto-detects based on available credentials:

**Priority Order:**
1. Claude Code CLI (if `claude` command exists)
2. Codex CLI (if `codex` command exists)
3. Anthropic (if `ANTHROPIC_API_KEY` set)
4. OpenAI (if `OPENAI_API_KEY` set)
5. Google (if `GOOGLE_GENERATIVE_AI_API_KEY` set)
6. Bedrock (if AWS credentials set)

```javascript
// Auto-detect best available provider
const agent = new ProbeAgent({ path: './src' });
await agent.initialize();

console.log(`Using: ${agent.clientApiProvider}`);
console.log(`Model: ${agent.model}`);
```

---

## Custom API Endpoints

Override default API endpoints for self-hosted or proxy setups:

```bash
# Generic endpoint (applies to all)
export LLM_BASE_URL=https://your-proxy.com

# Provider-specific (overrides generic)
export ANTHROPIC_API_URL=https://your-anthropic-proxy.com
export OPENAI_API_URL=https://your-openai-proxy.com
export GOOGLE_API_URL=https://your-google-proxy.com
```

---

## Retry Configuration

Configure automatic retry for transient failures:

```javascript
const agent = new ProbeAgent({
  path: './src',
  retry: {
    maxRetries: 3,           // Number of retry attempts
    initialDelay: 1000,      // First retry delay (ms)
    maxDelay: 30000,         // Maximum delay (ms)
    backoffFactor: 2,        // Exponential backoff multiplier
    jitter: true             // Add random jitter
  }
});
```

**Retryable Errors:**
- Rate limiting (429)
- Server errors (500, 502, 503, 504)
- Timeout errors
- Connection errors (ECONNRESET, ETIMEDOUT, ENOTFOUND)
- API overload errors

---

## Fallback Configuration

Configure automatic fallback to alternative providers:

### Strategy: Any Available

Try any available provider on failure:

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'anthropic',
  fallback: {
    strategy: 'any'
  }
});
```

### Strategy: Same Model

Try the same model on different providers:

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'anthropic',
  fallback: {
    strategy: 'same-model',
    models: ['claude-sonnet-4-6']
  }
});
```

### Strategy: Same Provider

Try different models on the same provider:

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'anthropic',
  fallback: {
    strategy: 'same-provider',
    models: ['claude-sonnet-4-6', 'claude-3-haiku-20240307']
  }
});
```

### Strategy: Custom

Define exact fallback sequence:

```javascript
const agent = new ProbeAgent({
  path: './src',
  fallback: {
    strategy: 'custom',
    providers: [
      { provider: 'anthropic', model: 'claude-sonnet-4-6' },
      { provider: 'openai', model: 'gpt-5.2' },
      { provider: 'google', model: 'gemini-2.5-flash' }
    ],
    stopOnSuccess: true,
    maxTotalAttempts: 10
  }
});
```

### Fallback Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `strategy` | string | - | `'any'`, `'same-model'`, `'same-provider'`, `'custom'` |
| `models` | string[] | - | Models for same-provider/same-model |
| `providers` | ProviderConfig[] | - | Custom provider list |
| `stopOnSuccess` | boolean | true | Stop after first success |
| `continueOnNonRetryableError` | boolean | false | Try fallback on non-retryable errors |
| `maxTotalAttempts` | number | 10 | Max total attempts across all providers |
| `debug` | boolean | false | Enable debug logging |

---

## Combined Retry + Fallback

Use both for maximum resilience:

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
    strategy: 'any',
    stopOnSuccess: true
  }
});
```

**Execution Order:**
1. Try primary provider
2. Retry up to `maxRetries` on transient errors
3. On persistent failure, try next fallback provider
4. Repeat retry logic for each fallback
5. Fail if all providers exhausted

---

## Timeout Configuration

```javascript
const agent = new ProbeAgent({
  path: './src',
  requestTimeout: 120000,       // Per-request timeout (ms)
  maxOperationTimeout: 300000   // Total operation timeout (ms)
});
```

**Environment Variable:**
```bash
ENGINE_ACTIVITY_TIMEOUT=180000  # 3 minutes (range: 5s - 10min)
```

---

## Engine Statistics

Access retry and fallback statistics:

```javascript
// After operations
const usage = agent.getTokenUsage();
console.log('Provider:', agent.clientApiProvider);
console.log('Model:', agent.model);
console.log('Total tokens:', usage.total.total);
```

---

## Provider Comparison

| Feature | Anthropic | OpenAI | Google | Bedrock |
|---------|-----------|--------|--------|---------|
| Streaming | ✓ | ✓ | ✓ | ✓ |
| Tool Use | ✓ | ✓ | ✓ | ✓ |
| Vision | ✓ | ✓ | ✓ | ✓ |
| Caching | ✓ | ✓ | Varies | ✓ |
| Max Context | 200K | 128K | 1M+ | 200K |

---

## Best Practices

### 1. Use Fallback for Production

```javascript
const agent = new ProbeAgent({
  provider: 'anthropic',
  retry: { maxRetries: 3 },
  fallback: { strategy: 'any' }
});
```

### 2. Match Model to Task

```javascript
// Complex analysis: use capable model
const analyzerAgent = new ProbeAgent({
  provider: 'anthropic',
  model: 'claude-sonnet-4-6'
});

// Simple queries: use fast model
const queryAgent = new ProbeAgent({
  provider: 'anthropic',
  model: 'claude-3-haiku-20240307'
});
```

### 3. Monitor Provider Usage

```javascript
agent.events.on('toolCall', (event) => {
  if (event.name === 'ai_request') {
    console.log(`Provider: ${agent.clientApiProvider}`);
  }
});
```

---

## Related Documentation

- [Retry & Fallback](./retry-fallback.md) - Detailed retry/fallback guide
- [API Reference](./api-reference.md) - Full configuration options
- [Troubleshooting](../../reference/troubleshooting.md) - Provider issues
