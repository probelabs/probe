# Retry and Fallback System

The ProbeAgent SDK includes a comprehensive retry and fallback system to handle API failures gracefully, ensuring maximum reliability for your AI applications.

## Features

- **Exponential Backoff**: Automatic retry with increasing delays
- **Provider Fallback**: Seamlessly switch between AI providers
- **Model Fallback**: Try different models on the same provider
- **Cross-Provider Fallback**: Fall back from Anthropic → OpenAI → Google → Bedrock
- **Configurable Retry Logic**: Custom retry limits, delays, and error patterns
- **Detailed Statistics**: Track retry attempts, successful fallbacks, and failures

## Quick Start

### Basic Retry Configuration

```javascript
import { ProbeAgent } from '@probelabs/probe';

const agent = new ProbeAgent({
  path: '/path/to/code',
  provider: 'anthropic',

  // Configure retry behavior
  retry: {
    maxRetries: 5,              // Retry up to 5 times per provider
    initialDelay: 1000,         // Start with 1 second delay
    maxDelay: 30000,            // Cap delays at 30 seconds
    backoffFactor: 2            // Double the delay each time
  }
});

// API calls will automatically retry on failures
const result = await agent.answer('Explain this code');
```

### Provider Fallback Configuration

```javascript
const agent = new ProbeAgent({
  path: '/path/to/code',

  // Configure fallback providers
  fallback: {
    strategy: 'custom',
    providers: [
      {
        provider: 'anthropic',
        apiKey: process.env.ANTHROPIC_API_KEY,
        baseURL: 'https://api.anthropic.com/v1',
        model: 'claude-sonnet-4-6',
        maxRetries: 5  // Retry 5 times on this provider
      },
      {
        provider: 'bedrock',
        region: 'us-west-2',
        accessKeyId: process.env.AWS_ACCESS_KEY_ID,
        secretAccessKey: process.env.AWS_SECRET_ACCESS_KEY,
        model: 'anthropic.claude-sonnet-4-6',
        maxRetries: 3
      },
      {
        provider: 'openai',
        apiKey: process.env.OPENAI_API_KEY,
        model: 'gpt-5.2',
        maxRetries: 3
      }
    ],
    maxTotalAttempts: 15  // Maximum total attempts across all providers
  }
});
```

## Configuration Options

### Retry Configuration

The `retry` object supports the following options:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `maxRetries` | number | 3 | Maximum retry attempts per provider |
| `initialDelay` | number | 1000 | Initial delay in milliseconds |
| `maxDelay` | number | 30000 | Maximum delay in milliseconds |
| `backoffFactor` | number | 2 | Exponential backoff multiplier |
| `retryableErrors` | string[] | See below | List of retryable error patterns |

**Default Retryable Errors:**
- `Overloaded`, `overloaded`
- `rate_limit`, `rate limit`
- `429`, `500`, `502`, `503`, `504` (HTTP status codes)
- `timeout`, `ECONNRESET`, `ETIMEDOUT`, `ENOTFOUND`
- `api_error`

### Fallback Configuration

The `fallback` object supports the following options:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `strategy` | string | 'any' | Fallback strategy (see below) |
| `providers` | array | [] | List of provider configurations |
| `models` | array | [] | List of models for same-provider fallback |
| `stopOnSuccess` | boolean | true | Stop on first successful response |
| `maxTotalAttempts` | number | 10 | Max attempts across all providers |

**Fallback Strategies:**
- `'same-model'`: Try the same model on different providers
- `'same-provider'`: Try different models on the same provider
- `'any'`: Try any available provider/model
- `'custom'`: Use custom provider list (recommended)

### Provider Configuration

Each provider in the `providers` array can have:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `provider` | string | ✅ | Provider name: 'anthropic', 'openai', 'google', 'bedrock' |
| `model` | string | ❌ | Model name (uses provider default if omitted) |
| `apiKey` | string | ✅* | API key for the provider |
| `baseURL` | string | ❌ | Custom API endpoint |
| `maxRetries` | number | ❌ | Override global retry count for this provider |
| `region` | string | ❌** | AWS region (Bedrock only) |
| `accessKeyId` | string | ❌** | AWS access key ID (Bedrock only) |
| `secretAccessKey` | string | ❌** | AWS secret key (Bedrock only) |
| `sessionToken` | string | ❌ | AWS session token (Bedrock only) |

\* Required for all providers except Bedrock (which can use AWS credentials)
\*\* Required for Bedrock if not using `apiKey`

## Environment Variable Configuration

You can configure retry and fallback behavior via environment variables:

### Retry Environment Variables

```bash
# Retry configuration
MAX_RETRIES=5                    # Max retries per provider
RETRY_INITIAL_DELAY=1000        # Initial delay in ms
RETRY_MAX_DELAY=30000           # Maximum delay in ms
RETRY_BACKOFF_FACTOR=2          # Exponential backoff multiplier
```

### Fallback Environment Variables

```bash
# Disable fallback
DISABLE_FALLBACK=1

# Auto-fallback (automatically use all available providers)
AUTO_FALLBACK=1

# Custom fallback providers (JSON array)
FALLBACK_PROVIDERS='[
  {
    "provider": "anthropic",
    "apiKey": "sk-ant-xxx",
    "model": "claude-sonnet-4-6"
  },
  {
    "provider": "bedrock",
    "region": "us-west-2",
    "accessKeyId": "xxx",
    "secretAccessKey": "xxx",
    "model": "anthropic.claude-sonnet-4-6"
  }
]'

# Fallback models (JSON array, for same-provider fallback)
FALLBACK_MODELS='["claude-sonnet-4-6", "claude-sonnet-4-6"]'

# Max total attempts across all providers
FALLBACK_MAX_TOTAL_ATTEMPTS=15
```

## Usage Examples

### Example 1: Azure Claude → Bedrock Claude Fallback

Perfect for when you want to use the same model across different cloud providers:

```javascript
const agent = new ProbeAgent({
  path: '/path/to/code',
  fallback: {
    strategy: 'custom',
    providers: [
      {
        provider: 'anthropic',
        apiKey: process.env.AZURE_CLAUDE_API_KEY,
        baseURL: 'https://your-azure-endpoint.com/v1',
        model: 'claude-sonnet-4-6'
      },
      {
        provider: 'bedrock',
        region: 'us-west-2',
        accessKeyId: process.env.AWS_ACCESS_KEY_ID,
        secretAccessKey: process.env.AWS_SECRET_ACCESS_KEY,
        model: 'anthropic.claude-sonnet-4-6'
      }
    ]
  }
});
```

### Example 2: Model Degradation (Claude 3.7 → Claude 3.5 → GPT-4)

Gracefully degrade to less powerful models if the primary is unavailable:

```javascript
const agent = new ProbeAgent({
  path: '/path/to/code',
  fallback: {
    strategy: 'custom',
    providers: [
      {
        provider: 'anthropic',
        apiKey: process.env.ANTHROPIC_API_KEY,
        model: 'claude-sonnet-4-6'
      },
      {
        provider: 'anthropic',
        apiKey: process.env.ANTHROPIC_API_KEY,
        model: 'claude-sonnet-4-6'
      },
      {
        provider: 'openai',
        apiKey: process.env.OPENAI_API_KEY,
        model: 'gpt-5.2'
      }
    ]
  }
});
```

### Example 3: Auto-Fallback

Automatically use all available providers from environment variables:

```bash
# Set all your API keys
export ANTHROPIC_API_KEY=sk-ant-xxx
export OPENAI_API_KEY=sk-xxx
export GOOGLE_API_KEY=xxx
export AUTO_FALLBACK=1
```

```javascript
const agent = new ProbeAgent({
  path: '/path/to/code',
  provider: 'anthropic',  // Primary provider
  fallback: {
    auto: true  // Automatically fallback to other providers
  }
});
```

### Example 4: Custom Retryable Errors

Only retry on specific error types:

```javascript
const agent = new ProbeAgent({
  path: '/path/to/code',
  retry: {
    maxRetries: 5,
    retryableErrors: [
      'Overloaded',
      'rate_limit',
      '429',
      '503',
      'CustomError'  // Your custom error pattern
    ]
  }
});
```

### Example 5: Different Retry Limits Per Provider

Configure different retry strategies for different providers:

```javascript
const agent = new ProbeAgent({
  path: '/path/to/code',
  retry: {
    maxRetries: 3  // Global default
  },
  fallback: {
    strategy: 'custom',
    providers: [
      {
        provider: 'anthropic',
        apiKey: process.env.ANTHROPIC_API_KEY,
        maxRetries: 10  // Retry Anthropic more aggressively
      },
      {
        provider: 'openai',
        apiKey: process.env.OPENAI_API_KEY,
        maxRetries: 2  // Retry OpenAI less
      }
    ]
  }
});
```

## How It Works

### Retry Flow

When an API call fails:

```
1. Check if error is retryable (matches error patterns)
2. If not retryable → fail immediately
3. If retryable:
   a. Wait initialDelay milliseconds
   b. Retry the request
   c. If fails again, increase delay by backoffFactor
   d. Repeat until maxRetries exhausted
4. If all retries fail → proceed to fallback (if configured)
```

### Fallback Flow

When a provider exhausts all retries:

```
1. Mark current provider as failed
2. Move to next provider in the list
3. Apply retry logic to the new provider
4. Repeat until:
   - A provider succeeds → return result
   - maxTotalAttempts reached → fail
   - All providers exhausted → fail
```

### Complete Flow Example

```
Primary: Anthropic Claude 3.7 (Azure)
  ├─ Attempt 1: ❌ Overloaded (wait 1s)
  ├─ Attempt 2: ❌ Overloaded (wait 2s)
  ├─ Attempt 3: ❌ Overloaded (wait 4s)
  └─ Exhausted → Fallback

Fallback 1: Bedrock Claude Sonnet 4
  ├─ Attempt 1: ❌ 503 Service Unavailable (wait 1s)
  ├─ Attempt 2: ❌ 503 (wait 2s)
  └─ Exhausted → Fallback

Fallback 2: OpenAI GPT-4
  ├─ Attempt 1: ✅ Success!
  └─ Return result
```

## Monitoring and Statistics

### Getting Statistics

Both `RetryManager` and `FallbackManager` track detailed statistics:

```javascript
const agent = new ProbeAgent({
  // ... configuration
  debug: true  // Enable debug logging
});

// Make a request
await agent.answer('Explain this code');

// Access internal managers (if needed for debugging)
if (agent.retryManager) {
  const retryStats = agent.retryManager.getStats();
  console.log('Retry Stats:', retryStats);
  // {
  //   totalAttempts: 7,
  //   totalRetries: 4,
  //   successfulRetries: 1,
  //   failedRetries: 0
  // }
}

if (agent.fallbackManager) {
  const fallbackStats = agent.fallbackManager.getStats();
  console.log('Fallback Stats:', fallbackStats);
  // {
  //   totalAttempts: 3,
  //   providerAttempts: {
  //     'anthropic/claude-sonnet-4-6': 1,
  //     'bedrock/anthropic.claude-sonnet-4-6': 1,
  //     'openai/gpt-5.2': 1
  //   },
  //   successfulProvider: 'openai/gpt-5.2',
  //   failedProviders: [
  //     { provider: 'anthropic/...', error: {...} },
  //     { provider: 'bedrock/...', error: {...} }
  //   ]
  // }
}
```

### Debug Logging

Enable debug mode to see detailed logs:

```javascript
const agent = new ProbeAgent({
  debug: true,
  retry: { maxRetries: 3 },
  fallback: { /* ... */ }
});

// Logs will show:
// [RetryManager] Retry attempt 1/3 { provider: 'anthropic', model: 'claude-3-7-...' }
// [RetryManager] Waiting 1000ms before retry...
// [FallbackManager] Attempting provider: anthropic/claude-3-7-... (attempt 1/10)
// [FallbackManager] ❌ Failed with provider: anthropic/claude-3-7-...
// [FallbackManager] Trying next provider (2 remaining)...
// [FallbackManager] ✅ Success with provider: openai/gpt-5.2
```

## Best Practices

### 1. Start Conservative, Scale Up

```javascript
// Development: Low retries, quick failures
const devAgent = new ProbeAgent({
  retry: { maxRetries: 1 },
  fallback: false
});

// Production: Aggressive retries, full fallback
const prodAgent = new ProbeAgent({
  retry: { maxRetries: 5 },
  fallback: {
    strategy: 'custom',
    providers: [/* multiple providers */],
    maxTotalAttempts: 20
  }
});
```

### 2. Use Provider-Specific Retry Limits

```javascript
{
  providers: [
    {
      provider: 'anthropic',
      maxRetries: 10  // More retries for primary
    },
    {
      provider: 'openai',
      maxRetries: 3   // Fewer for fallback
    }
  ]
}
```

### 3. Configure Delays Based on Your Use Case

```javascript
// User-facing: Shorter delays for faster response
{
  retry: {
    initialDelay: 500,    // 0.5s
    maxDelay: 5000        // 5s
  }
}

// Background processing: Longer delays, more retries
{
  retry: {
    initialDelay: 2000,   // 2s
    maxDelay: 60000,      // 60s
    maxRetries: 10
  }
}
```

### 4. Monitor Failures

```javascript
const agent = new ProbeAgent({
  retry: { maxRetries: 5 },
  fallback: { /* ... */ }
});

try {
  const result = await agent.answer('Question');
} catch (error) {
  if (error.allProvidersFailed) {
    // Log to monitoring service
    console.error('All providers failed:', error.stats);
    // Send alert
  }
}
```

### 5. Use Environment Variables for Secrets

```javascript
// Good: Use environment variables
{
  provider: 'anthropic',
  apiKey: process.env.ANTHROPIC_API_KEY
}

// Bad: Hardcode API keys
{
  provider: 'anthropic',
  apiKey: 'sk-ant-hardcoded-key'  // Don't do this!
}
```

## Troubleshooting

### Issue: Retries Not Working

**Symptoms:** API calls fail immediately without retrying

**Solutions:**
1. Check if error is in retryable error list
2. Verify `maxRetries` is set > 0
3. Enable debug logging to see retry attempts
4. Check if fallback is disabled unintentionally

```javascript
// Debug retryable errors
import { isRetryableError } from '@probelabs/probe/agent/RetryManager.js';
console.log(isRetryableError(yourError));
```

### Issue: Fallback Not Triggering

**Symptoms:** Falls back to same provider or doesn't fallback at all

**Solutions:**
1. Ensure fallback providers are properly configured
2. Check API keys are valid
3. Verify provider names are correct ('anthropic', 'openai', 'google', 'bedrock')
4. Enable debug logging

### Issue: Too Many Retries

**Symptoms:** Requests take too long to fail

**Solutions:**
1. Reduce `maxRetries`
2. Reduce `maxTotalAttempts`
3. Shorten `maxDelay`
4. Remove problematic providers from fallback list

### Issue: Rate Limits Still Occurring

**Symptoms:** Getting rate limited despite retries

**Solutions:**
1. Increase `initialDelay` and `maxDelay`
2. Increase `backoffFactor` (3 or 4 instead of 2)
3. Add '429' and 'rate_limit' to `retryableErrors`
4. Consider implementing request queuing

## API Reference

### RetryManager

```javascript
import { RetryManager } from '@probelabs/probe/agent/RetryManager.js';

const retry = new RetryManager({
  maxRetries: 3,
  initialDelay: 1000,
  maxDelay: 30000,
  backoffFactor: 2,
  retryableErrors: ['Overloaded', '429'],
  debug: false
});

// Execute with retry
const result = await retry.executeWithRetry(
  async () => {
    return await someAsyncFunction();
  },
  { provider: 'anthropic', model: 'claude-3' }
);

// Get statistics
const stats = retry.getStats();

// Reset statistics
retry.resetStats();
```

### FallbackManager

```javascript
import { FallbackManager } from '@probelabs/probe/agent/FallbackManager.js';

const fallback = new FallbackManager({
  strategy: 'custom',
  providers: [/* ... */],
  maxTotalAttempts: 10,
  debug: false
});

// Execute with fallback
const result = await fallback.executeWithFallback(
  async (provider, model, config) => {
    // provider: AI SDK provider instance
    // model: Model name
    // config: Full provider configuration
    return await streamText({ model: provider(model), /* ... */ });
  }
);

// Get statistics
const stats = fallback.getStats();

// Reset statistics
fallback.resetStats();
```

## Migration Guide

### From Simple Configuration

**Before:**
```javascript
const agent = new ProbeAgent({
  provider: 'anthropic'
});
```

**After (with retry and fallback):**
```javascript
const agent = new ProbeAgent({
  provider: 'anthropic',
  retry: {
    maxRetries: 5
  },
  fallback: {
    auto: true  // Use all available providers
  }
});
```

### From Environment-Based Fallback

**Before:**
```bash
export ANTHROPIC_API_KEY=xxx
export OPENAI_API_KEY=xxx
```

**After:**
```bash
export ANTHROPIC_API_KEY=xxx
export OPENAI_API_KEY=xxx
export AUTO_FALLBACK=1
export MAX_RETRIES=5
```

No code changes required! The agent will automatically use available providers.

## See Also

- [ProbeAgent API Reference](./PROBEAGENT.md)
- [Error Handling Guide](./ERROR_HANDLING.md)
- [Environment Variables](./ENVIRONMENT_VARIABLES.md)
