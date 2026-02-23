# Troubleshooting

Solutions to common issues with Probe CLI and Probe Agent.

---

## Installation Issues

### npm install fails with "EACCES permission denied"

**Solution:** Use npx or fix npm permissions.

```bash
# Option 1: Use npx (recommended)
npx -y @probelabs/probe@latest search "query" ./

# Option 2: Fix npm permissions
mkdir ~/.npm-global
npm config set prefix '~/.npm-global'
echo 'export PATH=~/.npm-global/bin:$PATH' >> ~/.bashrc
source ~/.bashrc
npm install -g @probelabs/probe
```

### Binary not found after installation

**Solution:** Add to PATH or use npx.

```bash
# Check installation location
npm list -g @probelabs/probe

# Add npm bin to PATH
export PATH=$(npm bin -g):$PATH

# Or use npx
npx probe search "query" ./
```

### "Unsupported platform" error

**Solution:** Build from source or use a supported platform.

Supported platforms:
- Linux x86_64, aarch64
- macOS x86_64, aarch64 (Apple Silicon)
- Windows x86_64

```bash
# Build from source (requires Rust)
cargo install probe-search
```

---

## Search Issues

### No results found

**Possible causes and solutions:**

1. **Files ignored by .gitignore**
   ```bash
   probe search "query" ./ --no-gitignore
   ```

2. **Wrong directory**
   ```bash
   # Verify path
   ls ./src
   probe search "query" ./src
   ```

3. **Language not supported**
   ```bash
   # Check supported languages
   probe search "query" ./ -v
   ```

4. **Query too specific**
   ```bash
   # Try broader search
   probe search "auth" ./  # Instead of "authenticateUserWithCredentials"
   ```

### Results missing expected code

**Solution:** Adjust search parameters.

```bash
# Include test files
probe search "mock" ./ --allow-tests

# Disable block merging
probe search "function" ./ --no-merge

# Increase merge threshold
probe search "function" ./ --merge-threshold 10
```

### Search is slow

**Solutions:**

1. **Limit results**
   ```bash
   probe search "query" ./ --max-results 20
   ```

2. **Use language filter**
   ```bash
   probe search "query" ./ --language rust
   ```

3. **Increase parser pool**
   ```bash
   PROBE_PARSER_POOL_SIZE=8 probe search "query" ./
   ```

4. **Skip parser warmup (cold start)**
   ```bash
   PROBE_NO_PARSER_WARMUP=1 probe search "query" ./
   ```

### "Session expired" or pagination issues

**Solution:** Use consistent session IDs and queries.

```bash
# First page
probe search "api" ./ --session my-search --max-results 50

# Same query for next page
probe search "api" ./ --session my-search --max-results 50
```

---

## Extract Issues

### "Failed to parse file" error

**Cause:** File may have syntax errors or unsupported encoding.

**Solutions:**

```bash
# Check file encoding
file src/broken.ts

# Use plain text fallback
probe extract src/file.txt:10 --format plain
```

### Wrong code block extracted

**Solution:** Use symbol extraction or adjust line numbers.

```bash
# Extract by symbol name (more precise)
probe extract src/auth.ts#loginUser

# Add context to see surrounding code
probe extract src/auth.ts:42 --context 10
```

### Git diff extraction not working

**Solution:** Ensure proper diff format.

```bash
# Standard git diff
git diff | probe extract --diff

# With proper context
git diff -U5 | probe extract --diff
```

---

## AI Provider Issues

### "No API key found" error

**Solution:** Set the appropriate environment variable.

```bash
# Anthropic
export ANTHROPIC_API_KEY=sk-ant-...

# OpenAI
export OPENAI_API_KEY=sk-...

# Google
export GOOGLE_GENERATIVE_AI_API_KEY=...

# Or use Claude Code CLI
which claude  # Verify installed
```

### Rate limit errors

**Solution:** Enable retry with backoff.

```javascript
const agent = new ProbeAgent({
  retry: {
    maxRetries: 3,
    initialDelay: 2000,
    backoffFactor: 2
  }
});
```

### "Model not found" error

**Solution:** Check model name and provider.

```javascript
// Correct model names
const agent = new ProbeAgent({
  provider: 'anthropic',
  model: 'claude-sonnet-4-6'  // Not 'claude-3-sonnet'
});
```

**Current default models:**
- Anthropic: `claude-sonnet-4-6`
- OpenAI: `gpt-5.2`
- Google: `gemini-2.5-flash`

### Token limit exceeded

**Solutions:**

1. **Limit search results**
   ```bash
   probe search "query" ./ --max-tokens 8000
   ```

2. **Compact conversation history**
   ```javascript
   await agent.compactHistory();
   ```

3. **Clear history**
   ```javascript
   await agent.clearHistory();
   ```

---

## MCP Issues

### MCP server not starting

**Debug steps:**

```bash
# Enable debug logging
DEBUG_MCP=1 probe mcp

# Check command exists
which probe
probe --version

# Test MCP server manually
probe mcp
```

### "Method not found" in MCP

**Cause:** Tool name may be prefixed or filtered.

**Solutions:**

1. Check tool names:
   ```javascript
   const tools = bridge.getTools();
   console.log(tools.map(t => t.name));
   ```

2. Check method filtering in config:
   ```json
   {
     "mcpServers": {
       "server": {
         "allowedMethods": ["*"],
         "blockedMethods": []
       }
     }
   }
   ```

### MCP timeout errors

**Solution:** Increase timeout.

```json
{
  "mcpServers": {
    "slow-server": {
      "timeout": 60000
    }
  }
}
```

Or via environment:
```bash
MCP_MAX_TIMEOUT=120000 probe mcp
```

---

## ProbeAgent SDK Issues

### "agent.initialize() must be called first"

**Solution:** Always await initialize() before using the agent.

```javascript
const agent = new ProbeAgent({ path: './src' });
await agent.initialize();  // Required!
const response = await agent.answer("question");
```

### Memory leak with multiple agents

**Solution:** Always close agents when done.

```javascript
const agent = new ProbeAgent({ path: './src' });
try {
  await agent.initialize();
  // ... use agent
} finally {
  await agent.close();  // Clean up
}
```

### Tool execution hanging

**Solutions:**

1. **Set timeouts**
   ```javascript
   const agent = new ProbeAgent({
     requestTimeout: 60000,
     maxOperationTimeout: 300000
   });
   ```

2. **Cancel stuck operations**
   ```javascript
   agent.cancel();
   ```

3. **Limit iterations**
   ```javascript
   const agent = new ProbeAgent({
     maxIterations: 10
   });
   ```

### History not persisting

**Solution:** Use a persistent storage adapter.

```javascript
import fs from 'fs/promises';

class FileStorageAdapter extends StorageAdapter {
  constructor(dir) {
    super();
    this.dir = dir;
  }

  async loadHistory(sessionId) {
    try {
      const data = await fs.readFile(`${this.dir}/${sessionId}.json`);
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
      JSON.stringify(history)
    );
  }

  async clearHistory(sessionId) {
    await fs.unlink(`${this.dir}/${sessionId}.json`).catch(() => {});
  }
}
```

---

## Debugging

### Enable verbose output

```bash
# CLI
DEBUG=1 probe search "query" ./
probe search "query" ./ -v

# Node.js SDK
const agent = new ProbeAgent({
  debug: true
});
```

### Check Probe version

```bash
probe --version
npm list @probelabs/probe
```

### Get stack traces

```bash
RUST_BACKTRACE=1 probe search "query" ./
```

### Log all tool calls

```javascript
agent.events.on('toolCall', (event) => {
  console.log(JSON.stringify(event, null, 2));
});
```

---

## Getting Help

1. **Search existing issues**: [GitHub Issues](https://github.com/probelabs/probe/issues)
2. **Join Discord**: [Discord Server](https://discord.gg/hBN4UsTZ)
3. **Report a bug**: [Create Issue](https://github.com/probelabs/probe/issues/new)

When reporting issues, include:
- Probe version (`probe --version`)
- Operating system and version
- Node.js version (if using SDK)
- Minimal reproduction steps
- Error messages and logs

---

## Related Documentation

- [FAQ](./faq.md) - Common questions
- [Glossary](./glossary.md) - Technical terms
- [Environment Variables](./environment-variables.md) - Configuration
