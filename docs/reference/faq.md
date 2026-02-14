# Frequently Asked Questions

Common questions about Probe CLI and Probe Agent.

---

## General

### What is Probe?

Probe is a semantic code search and AI agent platform consisting of two products:

1. **Probe CLI** - A Rust-based tool for fast, intelligent code search
2. **Probe Agent** - A Node.js SDK for building AI coding assistants

### How is Probe different from grep/ripgrep?

| Feature | grep/ripgrep | Probe |
|---------|--------------|-------|
| Speed | Fast | Fast (uses ripgrep internally) |
| AST awareness | No | Yes (tree-sitter) |
| Semantic search | No | Yes (BM25, TF-IDF, BERT) |
| Code block extraction | No | Yes (full functions/classes) |
| AI integration | No | Yes (MCP, SDK) |
| Token limits | No | Yes (--max-tokens) |

### Is Probe free?

Yes, Probe is open source under the Apache 2.0 license.

### What languages does Probe support?

Probe supports 15+ languages including Rust, JavaScript, TypeScript, Python, Go, C, C++, Java, Ruby, PHP, Swift, C#, HTML, Markdown, and YAML. See [Supported Languages](../supported-languages.md).

---

## Installation

### How do I install Probe?

```bash
# Via npm (recommended)
npm install -g @probelabs/probe

# Via cargo
cargo install probe-search

# Via Homebrew (macOS)
brew install probelabs/tap/probe

# Direct download
# See https://github.com/probelabs/probe/releases
```

### Do I need Rust to use Probe?

No. The npm package includes pre-built binaries for all major platforms. You only need Rust if building from source.

### Probe isn't finding my files. Why?

Check these common issues:

1. **.gitignore**: Probe respects .gitignore by default. Use `--no-gitignore` to include ignored files.
2. **Language support**: Ensure your language is supported.
3. **Path**: Verify you're searching the correct directory.
4. **Permissions**: Ensure read access to files.

```bash
# Debug: show what files Probe sees
probe search "test" ./ --files-only -v
```

---

## Search

### How do I search for exact phrases?

Use quotes:

```bash
probe search "\"exact phrase\""
probe search "'exact phrase'"
```

Or use the `--exact` flag:

```bash
probe search "myFunction" ./ --exact
```

### How do I exclude certain files?

Use the `--ignore` flag or search hints:

```bash
# Ignore patterns
probe search "config" ./ --ignore "*.test.ts" --ignore "node_modules/*"

# Using search hints
probe search "config NOT dir:tests NOT ext:spec.ts" ./
```

### Why are my results in a different order than expected?

Probe uses ranking algorithms (BM25 by default) to order results by relevance. Factors include:

- Term frequency
- Document length
- Field matches

To change ranking:

```bash
probe search "api" ./ --reranker tfidf
probe search "api" ./ --reranker hybrid
```

### How do I search only in specific file types?

```bash
# By extension
probe search "function AND ext:ts,tsx" ./

# By language
probe search "function" ./ --language typescript

# By file type (ripgrep types)
probe search "function AND type:rust" ./
```

### Can I use regex in searches?

Probe uses Elasticsearch-style syntax, not regex. For regex, use the `grep` subcommand:

```bash
probe grep "user[A-Z]\w+" ./src
```

---

## AI Integration

### Which AI providers does Probe support?

| Provider | API Key Variable |
|----------|------------------|
| Anthropic Claude | `ANTHROPIC_API_KEY` |
| OpenAI | `OPENAI_API_KEY` |
| Google Gemini | `GOOGLE_GENERATIVE_AI_API_KEY` |
| AWS Bedrock | AWS credentials |
| Claude Code | `claude` CLI installed |
| Codex | `codex` CLI installed |

### How do I use Probe with Claude Code?

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "probe": {
      "command": "probe",
      "args": ["mcp"]
    }
  }
}
```

### How do I limit token usage?

```bash
# CLI
probe search "auth" ./ --max-tokens 8000

# SDK
const agent = new ProbeAgent({
  path: './src',
  maxResponseTokens: 4096
});
```

### Can Probe edit my code?

Yes, with explicit permission:

```javascript
const agent = new ProbeAgent({
  path: './src',
  allowEdit: true,  // Enable edit/create tools
  enableBash: true  // Enable command execution
});
```

---

## Performance

### How can I make Probe faster?

1. **Use language filters**: `--language rust`
2. **Set result limits**: `--max-results 20`
3. **Use session pagination**: `--session my-search`
4. **Increase parser pool**: `PROBE_PARSER_POOL_SIZE=8`

### Why is the first search slow?

Probe warms up tree-sitter parsers on first use. Subsequent searches are faster. To skip warmup:

```bash
PROBE_NO_PARSER_WARMUP=1 probe search "query" ./
```

### How do I profile Probe performance?

```bash
DEBUG=1 probe search "query" ./
```

This shows timing for each phase (scanning, parsing, ranking).

---

## Probe Agent SDK

### How do I persist conversation history?

Implement a custom StorageAdapter:

```javascript
class MyDatabaseAdapter extends StorageAdapter {
  async loadHistory(sessionId) {
    return await db.messages.findAll({ sessionId });
  }
  async saveMessage(sessionId, message) {
    await db.messages.create({ sessionId, ...message });
  }
  async clearHistory(sessionId) {
    await db.messages.destroy({ sessionId });
  }
}

const agent = new ProbeAgent({
  storageAdapter: new MyDatabaseAdapter()
});
```

### How do I handle API rate limits?

Enable retry with backoff:

```javascript
const agent = new ProbeAgent({
  retry: {
    maxRetries: 3,
    initialDelay: 1000,
    backoffFactor: 2
  },
  fallback: {
    strategy: 'any'  // Try other providers
  }
});
```

### Can I stream responses?

Yes:

```javascript
await agent.answer("Explain this code", [], {
  onStream: (chunk) => {
    process.stdout.write(chunk);
  }
});
```

### How do I add custom tools?

Use MCP to expose custom tools:

```javascript
const agent = new ProbeAgent({
  enableMcp: true,
  mcpConfig: {
    mcpServers: {
      'my-tools': {
        command: 'node',
        args: ['my-mcp-server.js'],
        transport: 'stdio'
      }
    }
  }
});
```

---

## Troubleshooting

### "No API key found" error

Set at least one API key:

```bash
export ANTHROPIC_API_KEY=sk-...
# or
export OPENAI_API_KEY=sk-...
```

Or install Claude Code CLI:

```bash
# If claude command is available, it will be used automatically
which claude
```

### "ENOENT" or "File not found" errors

Check that:
1. The file path is correct
2. You have read permissions
3. The file isn't in .gitignore (or use `--no-gitignore`)

### MCP server not connecting

1. Check the MCP config path
2. Verify the command exists
3. Enable debug logging: `DEBUG_MCP=1`

### Memory issues with large codebases

1. Use `--max-results` to limit results
2. Use `--session` for pagination
3. Increase Node.js memory: `NODE_OPTIONS=--max-old-space-size=8192`

---

## Related Documentation

- [Troubleshooting](./troubleshooting.md) - Detailed solutions
- [Glossary](./glossary.md) - Technical terms
- [Environment Variables](./environment-variables.md) - Configuration
