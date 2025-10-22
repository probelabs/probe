# Retry and Fallback Implementation Summary

## Overview

This document summarizes the implementation of comprehensive retry and fallback support for ProbeAgent to handle API failures like "Overloaded" errors gracefully.

## What Was Implemented

### 1. RetryManager (`npm/src/agent/RetryManager.js`)

A robust retry manager with exponential backoff:

**Features:**
- Configurable max retries, delays, and backoff factors
- Smart error detection (Overloaded, 429, 503, timeouts, network errors)
- Exponential backoff with configurable limits
- Detailed statistics tracking
- Custom retryable error patterns

**Default Configuration:**
- Max retries: 3
- Initial delay: 1 second
- Max delay: 30 seconds
- Backoff factor: 2x

**Usage:**
```javascript
const retry = new RetryManager({
  maxRetries: 5,
  initialDelay: 1000,
  maxDelay: 30000,
  backoffFactor: 2
});

const result = await retry.executeWithRetry(
  () => apiCall(),
  { provider: 'anthropic', model: 'claude-3' }
);
```

### 2. FallbackManager (`npm/src/agent/FallbackManager.js`)

A flexible fallback system supporting multiple providers:

**Features:**
- Multiple fallback strategies (same-model, same-provider, custom)
- Support for all providers: Anthropic, OpenAI, Google, AWS Bedrock
- Per-provider configuration (API keys, base URLs, retry limits)
- Cross-cloud failover (Azure → Bedrock → OpenAI)
- Automatic provider instance creation
- Statistics tracking per provider

**Fallback Strategies:**
- `same-model`: Try same model on different providers
- `same-provider`: Try different models on same provider
- `any`: Use any available provider
- `custom`: Define custom fallback chain

**Usage:**
```javascript
const fallback = new FallbackManager({
  strategy: 'custom',
  providers: [
    {
      provider: 'anthropic',
      apiKey: 'xxx',
      model: 'claude-3-7-sonnet-20250219',
      maxRetries: 5
    },
    {
      provider: 'bedrock',
      region: 'us-west-2',
      accessKeyId: 'xxx',
      secretAccessKey: 'xxx',
      model: 'anthropic.claude-sonnet-4-20250514-v1:0'
    }
  ]
});

const result = await fallback.executeWithFallback(
  (provider, model, config) => streamText({ model: provider(model), ... })
);
```

### 3. ProbeAgent Integration

Updated ProbeAgent to support retry and fallback:

**Constructor Options:**
```javascript
const agent = new ProbeAgent({
  path: '/path/to/code',

  // Retry configuration
  retry: {
    maxRetries: 5,
    initialDelay: 1000,
    maxDelay: 30000,
    backoffFactor: 2,
    retryableErrors: ['Overloaded', '429', ...]
  },

  // Fallback configuration
  fallback: {
    strategy: 'custom',
    providers: [...],
    maxTotalAttempts: 15
  }
});
```

**Environment Variable Support:**
- `MAX_RETRIES`, `RETRY_INITIAL_DELAY`, `RETRY_MAX_DELAY`, `RETRY_BACKOFF_FACTOR`
- `FALLBACK_PROVIDERS` (JSON array)
- `FALLBACK_MODELS` (JSON array)
- `AUTO_FALLBACK=1` (use all available providers)
- `DISABLE_FALLBACK=1` (disable fallback)

**Integration Points:**
- `streamTextWithRetryAndFallback()` - Wrapper for streamText with retry/fallback
- `initializeFallbackManager()` - Auto-configure fallback from env vars
- Updated `answer()` method to use retry/fallback wrapper

### 4. Comprehensive Tests

Created extensive test suites:

**RetryManager Tests** (`npm/tests/unit/retryManager.test.js`):
- ✅ Basic retry functionality
- ✅ Exponential backoff verification
- ✅ Retryable vs non-retryable error detection
- ✅ Max retries exhaustion
- ✅ Statistics tracking
- ✅ Custom retryable errors
- ✅ Edge cases (maxRetries=0, null errors, etc.)

**FallbackManager Tests** (`npm/tests/unit/fallbackManager.test.js`):
- ✅ Provider fallback flow
- ✅ Configuration validation
- ✅ Provider instance creation
- ✅ Statistics tracking
- ✅ Environment variable parsing
- ✅ AWS Bedrock support (credentials + API key)
- ✅ Cross-provider fallback
- ✅ Max total attempts limit

### 5. Documentation

**Comprehensive Documentation** (`npm/docs/RETRY_AND_FALLBACK.md`):
- Complete feature overview
- Configuration reference
- Environment variable guide
- 7+ usage examples:
  1. Basic retry configuration
  2. Provider fallback
  3. Cross-cloud fallback (Azure → Bedrock)
  4. Auto-fallback
  5. Environment variable configuration
  6. Custom retryable errors
  7. Statistics and monitoring
- Best practices
- Troubleshooting guide
- API reference
- Migration guide

**Example Code** (`npm/examples/retry-fallback-example.js`):
- 7 working examples demonstrating all features
- Comments and explanations
- Ready to run and test

**Updated README** (`npm/README.md`):
- Added retry and fallback section
- Quick start examples
- Feature highlights
- Link to detailed documentation

## How It Works

### Complete Flow

```
1. User calls: agent.answer("question")
   ↓
2. ProbeAgent.answer() → streamTextWithRetryAndFallback()
   ↓
3. Check if FallbackManager configured?
   ├─ NO → Use RetryManager with current provider
   │   ↓
   │   Retry with exponential backoff
   │   ├─ Success → Return result
   │   └─ Fail after max retries → Throw error
   │
   └─ YES → Use FallbackManager
       ↓
       For each provider in list:
       ├─ Create provider instance
       ├─ Use RetryManager for this provider
       │   ├─ Attempt 1
       │   ├─ Retry 1 (wait 1s)
       │   ├─ Retry 2 (wait 2s)
       │   ├─ Retry 3 (wait 4s)
       │   └─ Max retries → Move to next provider
       │
       ├─ Success → Return result
       └─ All providers exhausted → Throw error with stats
```

### Example Execution

```
Primary: Anthropic Claude 3.7 (Azure)
  ├─ Attempt 1: ❌ Overloaded (wait 1s)
  ├─ Attempt 2: ❌ Overloaded (wait 2s)
  ├─ Attempt 3: ❌ Overloaded (wait 4s)
  ├─ Attempt 4: ❌ Overloaded (wait 8s)
  └─ Attempt 5: ❌ Overloaded → Fallback

Fallback 1: Bedrock Claude Sonnet 4
  ├─ Attempt 1: ❌ 503 Service Unavailable (wait 1s)
  ├─ Attempt 2: ❌ 503 (wait 2s)
  └─ Attempt 3: ❌ 503 → Fallback

Fallback 2: OpenAI GPT-4
  └─ Attempt 1: ✅ Success!

Total attempts: 9
Total providers tried: 3
Successful provider: openai/gpt-4o
```

## Key Features

### 1. Vercel AI SDK Compatibility

- ✅ Works seamlessly with Vercel AI SDK's `streamText`
- ✅ Preserves all SDK features (streaming, tool calls, etc.)
- ✅ No breaking changes to existing API

### 2. Flexible Configuration

- ✅ Programmatic API (JavaScript objects)
- ✅ Environment variables (JSON strings)
- ✅ Auto-fallback mode (use all available providers)
- ✅ Per-provider retry configuration

### 3. Provider Support

- ✅ Anthropic Claude (direct API + custom endpoints)
- ✅ AWS Bedrock (credentials + API key)
- ✅ OpenAI GPT
- ✅ Google Gemini
- ✅ Custom base URLs for all providers

### 4. Error Handling

- ✅ Smart error detection (Overloaded, 429, 503, timeouts)
- ✅ Custom retryable error patterns
- ✅ Non-retryable error fast-fail
- ✅ Detailed error context with statistics

### 5. Monitoring

- ✅ Debug logging for all retry/fallback attempts
- ✅ Statistics tracking (attempts, retries, providers used)
- ✅ Success/failure tracking per provider
- ✅ Error details for each failed attempt

## Configuration Examples

### Example 1: Simple Retry

```javascript
const agent = new ProbeAgent({
  retry: { maxRetries: 5 }
});
```

### Example 2: Azure → Bedrock Fallback

```javascript
const agent = new ProbeAgent({
  fallback: {
    strategy: 'custom',
    providers: [
      {
        provider: 'anthropic',
        apiKey: process.env.AZURE_CLAUDE_KEY,
        baseURL: 'https://your-azure-endpoint.com',
        model: 'claude-3-7-sonnet-20250219'
      },
      {
        provider: 'bedrock',
        region: 'us-west-2',
        accessKeyId: process.env.AWS_ACCESS_KEY_ID,
        secretAccessKey: process.env.AWS_SECRET_ACCESS_KEY,
        model: 'anthropic.claude-sonnet-4-20250514-v1:0'
      }
    ]
  }
});
```

### Example 3: Environment Variables

```bash
export MAX_RETRIES=5
export FALLBACK_PROVIDERS='[
  {"provider":"anthropic","apiKey":"xxx","model":"claude-3-7-sonnet-20250219"},
  {"provider":"openai","apiKey":"xxx","model":"gpt-4o"}
]'
```

```javascript
const agent = new ProbeAgent({ path: '.' });
// Auto-configured from environment!
```

### Example 4: Auto-Fallback

```bash
export ANTHROPIC_API_KEY=xxx
export OPENAI_API_KEY=xxx
export GOOGLE_API_KEY=xxx
export AUTO_FALLBACK=1
```

```javascript
const agent = new ProbeAgent({
  fallback: { auto: true }
});
// Uses all available providers automatically
```

## Files Created/Modified

### New Files

1. `npm/src/agent/RetryManager.js` - Retry logic with exponential backoff
2. `npm/src/agent/FallbackManager.js` - Provider fallback management
3. `npm/tests/unit/retryManager.test.js` - RetryManager tests
4. `npm/tests/unit/fallbackManager.test.js` - FallbackManager tests
5. `npm/docs/RETRY_AND_FALLBACK.md` - Complete documentation
6. `npm/examples/retry-fallback-example.js` - Usage examples

### Modified Files

1. `npm/src/agent/ProbeAgent.js`:
   - Added retry/fallback imports
   - Added retry/fallback configuration options
   - Added `streamTextWithRetryAndFallback()` method
   - Added `initializeFallbackManager()` method
   - Updated `answer()` to use retry/fallback wrapper

2. `npm/README.md`:
   - Added retry and fallback feature section
   - Added quick start examples
   - Added environment variable examples

## Backward Compatibility

✅ **100% Backward Compatible**

- Existing code works without any changes
- Retry and fallback are opt-in features
- Default behavior unchanged (no retry, no fallback)
- No breaking API changes

## Testing

All tests are comprehensive and cover:

- ✅ Happy path scenarios
- ✅ Error scenarios
- ✅ Edge cases
- ✅ Configuration validation
- ✅ Statistics tracking
- ✅ Environment variable parsing
- ✅ Provider-specific logic (AWS credentials, etc.)

## Next Steps

Potential future enhancements:

1. **Circuit Breaker Pattern**: Temporarily skip failing providers
2. **Request Queueing**: Rate limit protection at application level
3. **Metrics Export**: Prometheus/OpenTelemetry integration
4. **Provider Health Checks**: Pre-flight checks before using providers
5. **Cost Optimization**: Route to cheaper providers first
6. **Smart Fallback**: Learn which providers work best for specific queries

## Summary

This implementation provides a production-ready retry and fallback system for ProbeAgent that:

- ✅ Handles "Overloaded" and other API failures gracefully
- ✅ Supports multi-provider fallback (Azure Claude → Bedrock Claude → OpenAI)
- ✅ Provides exponential backoff with configurable delays
- ✅ Maintains full backward compatibility
- ✅ Includes comprehensive tests and documentation
- ✅ Supports both programmatic and environment variable configuration
- ✅ Tracks detailed statistics for monitoring
- ✅ Works seamlessly with Vercel AI SDK

The system is flexible enough to handle various use cases from simple retry to complex multi-cloud failover scenarios, while being easy to configure and use.
