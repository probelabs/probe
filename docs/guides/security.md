# Security Guide

Security considerations for Probe CLI and Probe Agent.

---

## TL;DR

- Input validation prevents injection attacks
- Path traversal protection built-in
- File size and regex limits prevent DoS
- API keys handled via environment variables
- Bash execution requires explicit opt-in

---

## Input Validation

### Query Validation

Probe enforces strict query syntax:

```rust
// Valid queries require explicit operators
"authentication AND login"    // ✓ Valid
"authentication OR signup"    // ✓ Valid
"authentication login"        // ✗ Rejected (ambiguous)
```

### Pattern Limits

| Limit | Value | Protection |
|-------|-------|------------|
| Max terms | 1,000 | Prevents regex explosion |
| Max patterns | 5,000 | Limits complexity |
| Max regex size | 8MB | Memory protection |
| Max line length | 2,000 chars | Processing limits |

### Regex Escaping

Special characters automatically escaped:

```javascript
// User input is escaped before pattern matching
query: "function()" → "function\\(\\)"
```

---

## Path Security

### Path Traversal Prevention

All file paths are canonicalized:

```rust
// Paths resolved to absolute form
std::fs::canonicalize(path)

// Prevents:
// - "../../../etc/passwd"
// - Symlink attacks
// - Unicode normalization issues
```

### Allowed Folders

Restrict search scope:

```bash
# Environment variable
ALLOWED_FOLDERS=/project1,/project2

# CLI argument
probe search "query" /specific/path
```

### Gitignore Integration

Respects `.gitignore` rules:

- `node_modules/` excluded
- `.git/` excluded
- Build directories excluded
- Custom patterns honored

---

## File Limits

### Size Restrictions

| Limit | Value | Purpose |
|-------|-------|---------|
| Max file size | 1MB | Prevent memory exhaustion |
| Max image size | 20MB | Limit upload size |
| Max line length | 2,000 | Skip binary/minified |

### Timeout Protection

```bash
# Default timeout: 30 seconds
probe search "query" ./path --timeout 60
```

---

## Command Execution

### Bash Tool Security

The bash tool requires explicit opt-in:

```javascript
const agent = new ProbeAgent({
  enableBash: true,
  bashConfig: {
    allow: ['npm test', 'git status'],
    deny: ['rm -rf', 'sudo', 'curl'],
    disableDefaultDeny: false
  }
});
```

### Default Deny Patterns

Dangerous commands blocked by default:

```javascript
const DEFAULT_DENY = [
  'rm -rf',
  'sudo',
  'chmod 777',
  'curl | sh',
  'wget -O- | sh',
  // ... more patterns
];
```

### Safe Subprocess Execution

```rust
// Commands use direct execution, not shell
Command::new("go")
  .arg("list")
  .arg("-m")
  .arg("-json")
  .output()

// No shell interpolation possible
```

---

## API Key Handling

### Environment Variables

API keys stored in environment, not code:

```bash
# Recommended approach
export ANTHROPIC_API_KEY=sk-ant-...
export OPENAI_API_KEY=sk-...
export GOOGLE_API_KEY=...
```

### No Logging

API keys excluded from logs:

```javascript
// Keys masked in debug output
console.log('API Key:', maskKey(apiKey));
// Output: "API Key: sk-ant-...xxxx"
```

### Rotation Support

Keys can be rotated without code changes:

```bash
# Simply update environment
export ANTHROPIC_API_KEY=new-key-value
```

---

## Web Interface Security

### Authentication

Basic HTTP auth for production:

```bash
AUTH_ENABLED=1
AUTH_USERNAME=admin
AUTH_PASSWORD=secure-password
```

### HTTPS Recommendation

Use reverse proxy for HTTPS:

```nginx
server {
    listen 443 ssl;
    server_name probe.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://localhost:8080;
    }
}
```

### CORS Headers

Configurable CORS for API access:

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST
Access-Control-Allow-Headers: Content-Type, Authorization
```

---

## Edit Tool Security

### Explicit Opt-In

Code editing disabled by default:

```javascript
const agent = new ProbeAgent({
  allowEdit: false  // Default
});
```

### Web Server Control

```bash
# Editing disabled by default
probe-chat --web ./project

# Explicit flag required
probe-chat --web --allow-edit ./project
```

### File Validation

- File must exist before editing
- Backup created before modification
- Atomic write operations

---

## Skill Security

### Path Validation

Skills must be inside repository root:

```javascript
// Validated paths
.claude/skills/code-review/SKILL.md  // ✓ Valid

// Rejected paths
../../../etc/SKILL.md                 // ✗ Traversal blocked
```

### Symlink Detection

Symlinked skill files are skipped:

```javascript
// Symlinks not followed for security
if (isSymlink(skillPath)) {
  console.warn('Skipping symlinked skill:', skillPath);
  continue;
}
```

### Safe YAML Parsing

YAML parsed with failsafe schema:

```javascript
// No code execution in YAML
yaml.load(content, { schema: 'failsafe' });
```

---

## Output Escaping

### XML Escaping

All output properly escaped:

```rust
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&apos;")
}
```

### JSON Serialization

Using standard libraries:

```rust
// Safe serialization via serde
serde_json::to_string(&result)
```

---

## Error Handling

### No Sensitive Data in Errors

```rust
// Good: Generic error message
Err(anyhow!("Failed to read file"))

// Bad: Exposes system details
Err(anyhow!("Failed to read /etc/passwd: permission denied"))
```

### Context Without Exposure

```rust
fs::read_to_string(path)
    .context("Failed to read configuration")?
```

---

## Resource Limits

### Prevent DoS

| Resource | Limit | Protection |
|----------|-------|------------|
| File size | 1MB | Memory |
| Regex size | 8MB | CPU |
| Query terms | 1,000 | Processing |
| Timeout | 30s default | Runaway operations |
| Max results | Configurable | Response size |
| Max tokens | Configurable | Cost control |

### Token Limits

```bash
# Limit response size
probe search "query" ./path --max-tokens 10000

# Limit result count
probe search "query" ./path --max-results 50
```

---

## Best Practices

### 1. Production Deployment

```bash
# Enable authentication
AUTH_ENABLED=1
AUTH_USERNAME=admin
AUTH_PASSWORD=$(openssl rand -base64 32)

# Use HTTPS (via reverse proxy)
# Restrict allowed folders
ALLOWED_FOLDERS=/app/src

# Disable dangerous features
# Don't use --allow-edit or --enable-bash unless needed
```

### 2. API Key Management

```bash
# Use environment variables
export ANTHROPIC_API_KEY=...

# Or secret management
source /path/to/secrets.env

# Never commit keys
echo "*.env" >> .gitignore
```

### 3. Restrict Bash Execution

```javascript
const agent = new ProbeAgent({
  enableBash: true,
  bashConfig: {
    allow: [
      'npm test',
      'npm run lint',
      'git status',
      'git log'
    ],
    deny: [
      'rm',
      'sudo',
      'curl',
      'wget'
    ]
  }
});
```

### 4. Limit Search Scope

```javascript
const agent = new ProbeAgent({
  path: './src',
  allowedFolders: ['./src', './lib']
});
```

### 5. Monitor Usage

```javascript
// Track token usage
const usage = agent.getTokenUsage();
if (usage.totalTokens > 1000000) {
  console.warn('High token usage detected');
}

// Enable tracing
probe-chat --trace-file ./audit.jsonl
```

---

## Security Checklist

### Before Production

- [ ] Authentication enabled
- [ ] HTTPS configured (reverse proxy)
- [ ] API keys in environment/secrets
- [ ] Allowed folders restricted
- [ ] Edit tool disabled (unless needed)
- [ ] Bash tool disabled (unless needed)
- [ ] Tracing/logging enabled
- [ ] Timeout limits configured
- [ ] Token limits configured

### Ongoing

- [ ] Rotate API keys regularly
- [ ] Review access logs
- [ ] Update dependencies
- [ ] Monitor token usage
- [ ] Audit bash command patterns

---

## Related Documentation

- [Environment Variables](../reference/environment-variables.md) - Configuration
- [Limits](../reference/limits.md) - System limits
- [Troubleshooting](../reference/troubleshooting.md) - Common issues

