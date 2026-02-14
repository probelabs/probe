# Retry & Fallback

Configure automatic retries and provider fallbacks for maximum reliability.

---

## TL;DR

```javascript
const agent = new ProbeAgent({
  retry: {
    maxRetries: 3,
    initialDelay: 1000,
    backoffFactor: 2
  },
  fallback: {
    strategy: 'any',
    maxTotalAttempts: 10
  }
});
```

---

## Retry System

### How It Works

The RetryManager handles transient failures with exponential backoff:

```
Attempt 1 → Fail → Wait 1000ms
Attempt 2 → Fail → Wait 2000ms
Attempt 3 → Fail → Wait 4000ms (capped at maxDelay)
Attempt 4 → Success ✓
```

### Configuration

```javascript
const agent = new ProbeAgent({
  retry: {
    maxRetries: 3,           // Number of retries (0-100)
    initialDelay: 1000,      // First delay in ms
    maxDelay: 30000,         // Maximum delay cap
    backoffFactor: 2,        // Multiplier per retry
    jitter: true,            // Add random variance (±25%)
    retryableErrors: [],     // Custom error patterns
    debug: false             // Enable logging
  }
});
```

### Default Retryable Errors

These error patterns trigger automatic retries:

| Pattern | Description |
|---------|-------------|
| `Overloaded` | API overload |
| `rate_limit` | Rate limiting (429) |
| `500`, `502`, `503`, `504` | Server errors |
| `ECONNRESET`, `ETIMEDOUT` | Connection errors |
| `timeout` | Timeout errors |
| `api_error` | Generic API errors |

### Environment Variables

```bash
MAX_RETRIES=3
RETRY_INITIAL_DELAY=1000
RETRY_MAX_DELAY=30000
RETRY_BACKOFF_FACTOR=2
RETRY_JITTER=true
```

### Jitter

Jitter prevents the "thundering herd" problem by adding random variance:

```
Base delay: 2000ms
With jitter (±25%): 1500-2500ms
```

---

## Fallback System

### How It Works

When the primary provider fails, FallbackManager tries alternatives:

```
Anthropic (primary) → Fail
OpenAI (fallback 1) → Fail
Google (fallback 2) → Success ✓
```

### Strategies

| Strategy | Description |
|----------|-------------|
| `same-model` | Try same model across providers (e.g., Claude on Anthropic → Bedrock) |
| `same-provider` | Try different models on same provider (e.g., Claude 4.5 → 3.5 → Haiku) |
| `any` | Try any available provider/model |
| `custom` | Use explicit provider list |

### Configuration

```javascript
const agent = new ProbeAgent({
  fallback: {
    strategy: 'custom',
    providers: [
      { provider: 'anthropic', model: 'claude-sonnet-4-5-20250929' },
      { provider: 'openai', model: 'gpt-4o' },
      { provider: 'google', model: 'gemini-2.0-flash' }
    ],
    stopOnSuccess: true,
    maxTotalAttempts: 10,
    debug: false
  }
});
```

### Auto-Detection

With `fallback: { auto: true }`, providers are detected from environment:

```bash
# Providers tried in order based on available API keys:
ANTHROPIC_API_KEY=...  # → anthropic (1st)
OPENAI_API_KEY=...     # → openai (2nd)
GOOGLE_API_KEY=...     # → google (3rd)
```

### Provider Configuration

```javascript
{
  provider: 'anthropic',
  model: 'claude-sonnet-4-5-20250929',
  apiKey: 'sk-ant-...',           // Override env var
  baseURL: 'https://proxy.com',   // Custom endpoint
  maxRetries: 5                   // Provider-specific retries
}
```

### AWS Bedrock

```javascript
{
  provider: 'bedrock',
  model: 'anthropic.claude-sonnet-4-20250514-v1:0',
  region: 'us-east-1',
  accessKeyId: 'AKIA...',
  secretAccessKey: '...',
  sessionToken: '...'  // Optional
}
```

### Environment Variables

```bash
# Custom provider list (JSON)
FALLBACK_PROVIDERS='[
  {"provider":"anthropic","apiKey":"..."},
  {"provider":"openai","apiKey":"..."}
]'

# Models for same-provider strategy
FALLBACK_MODELS='["claude-3.7-sonnet","claude-3.5-sonnet"]'

# Maximum attempts
FALLBACK_MAX_TOTAL_ATTEMPTS=10
```

---

## Combined Flow

Retry and fallback work together:

```
┌─────────────────────────────────────────┐
│          Anthropic Provider             │
│  ┌──────────────────────────────────┐   │
│  │ Attempt 1 → Fail                 │   │
│  │ Wait 1000ms                      │   │
│  │ Attempt 2 → Fail                 │   │
│  │ Wait 2000ms                      │   │
│  │ Attempt 3 → Fail (max retries)   │   │
│  └──────────────────────────────────┘   │
└──────────────────┬──────────────────────┘
                   │ Fallback
                   ▼
┌─────────────────────────────────────────┐
│           OpenAI Provider               │
│  ┌──────────────────────────────────┐   │
│  │ Attempt 1 → Success ✓            │   │
│  └──────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

---

## Statistics

### Retry Statistics

```javascript
const agent = new ProbeAgent({ retry: { ... } });
// After operations...

const stats = agent.retryManager.getStats();
console.log(stats);
// {
//   totalAttempts: 8,
//   totalRetries: 5,
//   successfulRetries: 2,
//   failedRetries: 3
// }

// Reset stats
agent.retryManager.resetStats();
```

### Fallback Statistics

```javascript
const stats = agent.fallbackManager.getStats();
console.log(stats);
// {
//   totalAttempts: 4,
//   providerAttempts: {
//     anthropic: 2,
//     openai: 2
//   },
//   successfulProvider: 'openai',
//   failedProviders: [
//     { provider: 'anthropic', error: 'rate_limit' }
//   ]
// }
```

---

## Examples

### Basic Retry

```javascript
const agent = new ProbeAgent({
  retry: {
    maxRetries: 5,
    initialDelay: 500,
    maxDelay: 15000
  }
});
```

### Multi-Provider Fallback

```javascript
const agent = new ProbeAgent({
  fallback: {
    strategy: 'custom',
    providers: [
      { provider: 'anthropic', model: 'claude-sonnet-4-5-20250929' },
      { provider: 'openai', model: 'gpt-4o' },
      { provider: 'google', model: 'gemini-2.0-flash' }
    ]
  }
});
```

### Same-Provider Fallback

```javascript
const agent = new ProbeAgent({
  fallback: {
    strategy: 'same-provider',
    models: [
      'claude-sonnet-4-5-20250929',
      'claude-3-5-sonnet-20241022',
      'claude-3-haiku-20240307'
    ]
  }
});
```

### Production Configuration

```javascript
const agent = new ProbeAgent({
  retry: {
    maxRetries: 3,
    initialDelay: 1000,
    backoffFactor: 2,
    jitter: true
  },
  fallback: {
    strategy: 'any',
    maxTotalAttempts: 10,
    continueOnNonRetryableError: false
  }
});
```

---

## Error Handling

### Non-Retryable Errors

These errors skip retry/fallback:

- Authentication failures
- Invalid API keys
- Permission denied
- Validation errors
- User cancellation

### Custom Retryable Errors

```javascript
const agent = new ProbeAgent({
  retry: {
    retryableErrors: [
      'custom_error_pattern',
      'temporary_failure',
      ...defaultRetryableErrors
    ]
  }
});
```

---

## Best Practices

### 1. Balance Resilience and Speed

```javascript
// More aggressive (faster, less reliable)
{ maxRetries: 1, initialDelay: 500 }

// More conservative (slower, more reliable)
{ maxRetries: 5, initialDelay: 2000 }
```

### 2. Order Providers by Preference

```javascript
fallback: {
  providers: [
    { provider: 'anthropic' },  // Primary
    { provider: 'openai' },     // Secondary
    { provider: 'google' }      // Tertiary
  ]
}
```

### 3. Monitor Statistics

```javascript
setInterval(() => {
  const retryStats = agent.retryManager.getStats();
  const fallbackStats = agent.fallbackManager.getStats();

  if (retryStats.failedRetries > 10) {
    console.warn('High retry failure rate');
  }

  if (fallbackStats.failedProviders.length > 2) {
    console.warn('Multiple provider failures');
  }
}, 60000);
```

### 4. Environment-Based Configuration

```javascript
const isProd = process.env.NODE_ENV === 'production';

const agent = new ProbeAgent({
  retry: {
    maxRetries: isProd ? 5 : 2,
    debug: !isProd
  }
});
```

---

## Related Documentation

- [API Reference](./api-reference.md) - Full API docs
- [Engines](./engines.md) - Provider configuration
- [Getting Started](./getting-started.md) - Quick start

