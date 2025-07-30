# Probe Chat Implementation Tool - Pluggable Backend System

The Probe Chat Implementation Tool now supports multiple AI-powered code implementation backends through a flexible, pluggable architecture. This allows you to choose between different AI coding assistants based on your needs, API availability, and preferences.

## 🚀 Quick Start

### Using Different Backends

```bash
# Use the default backend (aider)
probe-chat --allow-edit

# Use Claude Code backend
probe-chat --allow-edit --implement-tool-backend claude-code

# Configure fallback backends
probe-chat --allow-edit --implement-tool-backend claude-code --implement-tool-fallbacks aider

# List available backends
probe-chat --implement-tool-list-backends

# Get detailed info about a backend
probe-chat --implement-tool-backend-info claude-code
```

## 📋 Available Backends

### Aider Backend (Default)
- **Description**: AI pair programming in your terminal
- **Strengths**: Battle-tested, supports many models, git integration
- **Requirements**: Python 3.8+, `pip install aider-chat`
- **API Keys**: Requires one of: ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY

### Claude Code Backend
- **Description**: Advanced AI coding assistant powered by Claude
- **Strengths**: Latest Claude models, sophisticated code understanding, MCP tools
- **Requirements**: Node.js 18+, `npm install -g @anthropic-ai/claude-code`
- **API Keys**: Requires ANTHROPIC_API_KEY

## ⚙️ Configuration

### Environment Variables

```bash
# Backend Selection
export IMPLEMENT_TOOL_BACKEND=claude-code              # Choose primary backend
export IMPLEMENT_TOOL_FALLBACKS=aider,claude-code  # Comma-separated fallbacks
export IMPLEMENT_TOOL_TIMEOUT=300000                    # Timeout in milliseconds

# Aider Configuration
export AIDER_MODEL=gpt-4                          # Model for aider
export AIDER_AUTO_COMMIT=false                    # Auto-commit changes
export AIDER_TIMEOUT=300000                       # Aider-specific timeout

# Claude Code Configuration  
export CLAUDE_CODE_MODEL=claude-3-5-sonnet-20241022  # Claude model
export CLAUDE_CODE_MAX_TOKENS=8000                # Max tokens
export CLAUDE_CODE_TEMPERATURE=0.3                # Temperature (0-2)
export CLAUDE_CODE_MAX_TURNS=10                   # Max conversation turns
```

### Configuration File

Create `implement-config.json` in your project root:

```json
{
  "implement": {
    "defaultBackend": "claude-code",
    "fallbackBackends": ["aider"],
    "selectionStrategy": "auto",
    "timeout": 300000
  },
  "backends": {
    "aider": {
      "model": "gpt-4",
      "autoCommit": false,
      "additionalArgs": ["--no-auto-commits"]
    },
    "claude-code": {
      "model": "claude-3-5-sonnet-20241022",
      "maxTokens": 8000,
      "temperature": 0.3,
      "tools": ["edit", "search", "bash"]
    }
  }
}
```

## 🔄 Backend Selection Strategies

The system supports three selection strategies:

1. **auto** (default): Tries backends in order of availability
2. **preference**: Uses only the specified backend, fails if unavailable
3. **capability**: Selects backend based on language and feature support

## 🛠️ CLI Options

```bash
# Backend selection
--implement-tool-backend <name>      # Choose backend (aider, claude-code)
--implement-tool-fallbacks <names>   # Comma-separated fallback backends
--implement-tool-timeout <ms>        # Implementation timeout
--implement-tool-config <path>       # Path to config file

# Information
--implement-tool-list-backends       # List all available backends
--implement-tool-backend-info <name> # Show backend details
```

## 📊 Backend Comparison

| Feature | Aider | Claude Code |
|---------|-------|-------------|
| **Languages** | Python, JS, TS, Go, Rust, Java, C/C++, C#, Ruby, PHP | JS, TS, Python, Rust, Go, Java, C++, C#, Ruby, PHP, Swift |
| **Streaming Output** | ✅ | ✅ |
| **Direct File Edit** | ✅ | ✅ |
| **Test Generation** | ❌ | ✅ |
| **Plan Generation** | ❌ | ✅ |
| **Rollback Support** | ✅ | ❌ |
| **Max Sessions** | 3 | 5 |

## 🔧 Advanced Usage

### Custom Backend Configuration

```javascript
// In your code
const implementTool = createImplementTool({
  enabled: true,
  backendConfig: {
    defaultBackend: 'claude-code',
    fallbackBackends: ['aider'],
    backends: {
      'claude-code': {
        apiKey: process.env.MY_CLAUDE_KEY,
        model: 'claude-3-5-sonnet-20241022',
        systemPrompt: 'You are an expert TypeScript developer...'
      }
    }
  }
});
```

### Programmatic Backend Selection

```javascript
// Execute with specific backend
const result = await implementTool.execute({
  task: 'Refactor this function to use async/await',
  backend: 'claude-code',  // Force specific backend
  generateTests: true,     // Backend-specific option
  sessionId: 'my-session-123'
});
```

## 🚨 Troubleshooting

### Common Issues

1. **Backend not available**
   - Check if required dependencies are installed
   - Verify API keys are set correctly
   - Run `probe-chat --implement-tool-backend-info <name>` for diagnostics

2. **Timeout errors**
   - Increase timeout: `--implement-tool-timeout 600000` (10 minutes)
   - Check network connectivity
   - Consider using a different backend

3. **API key issues**
   - Ensure keys are exported in your environment
   - Check key validity with the provider
   - Verify key permissions

### Debug Mode

Enable debug logging to troubleshoot issues:

```bash
DEBUG_CHAT=1 probe-chat --allow-edit --implement-tool-backend claude-code
```

## 🔐 Security Considerations

- API keys are never logged or exposed
- File access is restricted to the working directory
- All changes are made locally until explicitly committed
- Review all AI-generated changes before committing

## 🤝 Contributing

To add a new backend:

1. Extend the `BaseBackend` class
2. Implement required methods
3. Register in `backends/registry.js`
4. Add configuration schema
5. Update documentation

See `implement/backends/BaseBackend.js` for the interface definition.

## 📝 Migration from Legacy System

The new system is backward compatible. Your existing workflows will continue to work:

```bash
# Old way (still works, uses aider backend)
probe-chat --allow-edit

# New way (explicit backend selection)
probe-chat --allow-edit --implement-tool-backend aider
```

No changes are required to existing scripts or workflows. The system defaults to aider backend for compatibility.

## 🔗 Related Documentation

- [Probe Chat Documentation](../../README.md)
- [Aider Documentation](https://aider.chat/)
- [Claude Code SDK](https://docs.anthropic.com/en/docs/claude-code)

## 📄 License

This pluggable backend system is part of Probe Chat and follows the same Apache-2.0 license.