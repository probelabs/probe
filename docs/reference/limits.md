# Limits & Constraints

System limits and constraints for Probe CLI and Probe Agent.

---

## Probe CLI Limits

### Search Limits

| Limit | Default | Configuration |
|-------|---------|---------------|
| Max results | Unlimited | `--max-results N` |
| Max bytes | Unlimited | `--max-bytes N` |
| Max tokens | Unlimited | `--max-tokens N` |
| Timeout | 30 seconds | `--timeout N` |

### Merge Threshold

| Limit | Default | Configuration |
|-------|---------|---------------|
| Block merge distance | 5 lines | `--merge-threshold N` |

### Session Caching

| Limit | Default | Description |
|-------|---------|-------------|
| Cache size | Automatic | Based on available memory |
| Session lifetime | Until invalidation | Invalidated on file changes |

---

## Probe Agent Limits

### Tool Iterations

| Limit | Default | Configuration |
|-------|---------|---------------|
| Max iterations | 30 | `maxIterations` option |

### Timeouts

| Limit | Default | Configuration |
|-------|---------|---------------|
| Request timeout | 120,000 ms | `requestTimeout` option |
| Operation timeout | 300,000 ms | `maxOperationTimeout` option |
| Engine activity | 180,000 ms | `ENGINE_ACTIVITY_TIMEOUT` env |

**Engine Activity Timeout Range:**
- Minimum: 5,000 ms (5 seconds)
- Maximum: 600,000 ms (10 minutes)
- Default: 180,000 ms (3 minutes)

### Delegation Limits

| Limit | Default | Configuration |
|-------|---------|---------------|
| Concurrent global | 3 | `MAX_CONCURRENT_DELEGATIONS` |
| Per session | 10 | `MAX_DELEGATIONS_PER_SESSION` |
| Queue timeout | 60,000 ms | `DELEGATION_QUEUE_TIMEOUT` |
| Execution timeout | 300 seconds | `DELEGATION_TIMEOUT` |

### Retry Limits

| Limit | Default | Configuration |
|-------|---------|---------------|
| Max retries | 3 | `retry.maxRetries` |
| Initial delay | 1,000 ms | `retry.initialDelay` |
| Max delay | 30,000 ms | `retry.maxDelay` |
| Backoff factor | 2 | `retry.backoffFactor` |

**Retry Limits Range:**
- Max retries: 0-100
- Initial delay: 0-60,000 ms
- Max delay: 0-300,000 ms
- Backoff factor: 1-10

### Fallback Limits

| Limit | Default | Configuration |
|-------|---------|---------------|
| Max total attempts | 10 | `fallback.maxTotalAttempts` |

**Range:** 1-100

---

## MCP Limits

### Timeouts

| Limit | Default | Configuration |
|-------|---------|---------------|
| Request timeout | 30,000 ms | `settings.timeout` |
| Max timeout | 1,800,000 ms | `MCP_MAX_TIMEOUT` |

**Max Timeout Range:**
- Minimum: 30,000 ms (30 seconds)
- Maximum: 7,200,000 ms (2 hours)
- Default: 1,800,000 ms (30 minutes)

---

## Provider Context Windows

| Provider | Model | Context Window |
|----------|-------|----------------|
| Anthropic | Claude Sonnet | 200,000 tokens |
| Anthropic | Claude Haiku | 200,000 tokens |
| OpenAI | GPT-4o | 128,000 tokens |
| OpenAI | GPT-4o-mini | 128,000 tokens |
| Google | Gemini 1.5 Pro | 1,000,000+ tokens |
| Google | Gemini 2.0 Flash | 1,000,000+ tokens |

---

## Skill Limits

| Limit | Value | Description |
|-------|-------|-------------|
| Skill name length | 64 characters | Maximum skill name |
| Description length | 400 characters | Auto-truncated |
| Skill name pattern | `[a-z0-9-]+` | Lowercase alphanumeric + hyphens |

---

## Task Limits

| Limit | Description |
|-------|-------------|
| Task ID format | `task-1`, `task-2`, ... |
| Dependencies | Must reference existing task IDs |
| Circular dependencies | Not allowed |

---

## Image Limits

| Limit | Value | Description |
|-------|-------|-------------|
| Max file size | 20 MB | Per image |
| Supported formats | PNG, JPG, JPEG, WebP, BMP, SVG | |

---

## Bash Execution Limits

### Probe Chat

| Limit | Default | Configuration |
|-------|---------|---------------|
| Command timeout | 120,000 ms | `--bash-timeout` |

---

## Memory Considerations

### Large Codebases

For codebases with 100,000+ files:

1. **Use result limits**: `--max-results 100`
2. **Use session pagination**: `--session my-search`
3. **Use language filters**: `--language rust`
4. **Increase Node.js memory**: `NODE_OPTIONS=--max-old-space-size=8192`

### Parser Pool

| Setting | Default | Configuration |
|---------|---------|---------------|
| Pool size | CPU cores | `PROBE_PARSER_POOL_SIZE` |
| Tree cache | Automatic | `PROBE_TREE_CACHE_SIZE` |

---

## Rate Limiting

### Provider Rate Limits

Rate limits are provider-specific. Common responses:

| Error Code | Description | Solution |
|------------|-------------|----------|
| 429 | Rate limit exceeded | Enable retry with backoff |
| 503 | Service overloaded | Enable fallback to alternative |

### Retry Configuration

```javascript
const agent = new ProbeAgent({
  retry: {
    maxRetries: 3,
    initialDelay: 1000,
    backoffFactor: 2
  }
});
```

---

## Best Practices

### 1. Set Appropriate Limits

```bash
# For AI context windows
probe search "query" ./ --max-tokens 10000

# For large results
probe search "query" ./ --max-results 50 --session my-search
```

### 2. Use Pagination

```bash
# First batch
probe search "api" ./ --session api-search --max-results 100

# Subsequent batches
probe search "api" ./ --session api-search --max-results 100
```

### 3. Configure Timeouts

```javascript
const agent = new ProbeAgent({
  requestTimeout: 120000,      // 2 minutes per request
  maxOperationTimeout: 600000  // 10 minutes total
});
```

### 4. Handle Resource Constraints

```javascript
// Monitor token usage
const usage = agent.getTokenUsage();
if (usage.contextWindow > 150000) {
  await agent.compactHistory();
}
```

---

## Related Documentation

- [Performance Tuning](../probe-cli/performance.md) - Optimization guide
- [Environment Variables](./environment-variables.md) - Configuration
- [Troubleshooting](./troubleshooting.md) - Common issues
