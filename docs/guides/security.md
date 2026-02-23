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
    allow: ['git:push'],           // Custom allow (overrides default deny)
    deny: ['git:push:--force'],    // Custom deny (always wins)
    disableDefaultDeny: false
  }
});
```

### Pattern Syntax

Bash permissions use colon-separated patterns that match commands and arguments:

| Pattern | Matches | Does NOT match |
|---------|---------|----------------|
| `git:push` | `git push`, `git push origin main`, `git push --force` | `git pull`, `git status` |
| `git:push:--force` | `git push --force`, `git push --force origin` | `git push`, `git push origin main` |
| `git:branch:*` | `git branch -a`, `git branch --list` | `git status` |
| `npm:install` | `npm install`, `npm install express` | `npm test` |

A pattern like `git:push` matches the command `git` with first argument `push`, regardless of any additional arguments. The pattern only checks the parts it specifies — extra arguments in the actual command are ignored.

**Note on wildcards:** `git:push` and `git:push:*` are functionally identical. The `*` wildcard matches any argument **or no argument**, so it adds no additional coverage. Prefer the shorter form `git:push` for clarity.

### Permission Resolution Priority

Permissions are resolved using a **strict 4-level priority system**. Higher priority always wins:

```
Priority 1 (highest): Custom deny   — --bash-deny patterns, ALWAYS block
Priority 2:           Custom allow  — --bash-allow patterns, override default deny
Priority 3:           Default deny  — built-in deny list, block by default
Priority 4 (lowest):  Allow list    — built-in + custom allow, permit safe commands
```

**How this works in practice:**

1. **Custom deny always wins.** If a command matches a `--bash-deny` pattern, it is blocked regardless of any allow patterns. This is the user's explicit "never allow this" list.

2. **Custom allow overrides default deny.** If a command matches a `--bash-allow` pattern, it bypasses the built-in default deny list. This lets users selectively allow specific commands (like `git push`) without disabling all default protections.

3. **Default deny blocks by default.** If a command matches the built-in deny list and is NOT in the custom allow list, it is blocked.

4. **Allow list permits.** If a command matches the combined allow list (built-in defaults + custom), it is permitted.

### Example: Allow Push but Block Force Push

```bash
# CLI usage
probe-chat --enable-bash \
  --bash-allow "git:push" \
  --bash-deny "git:push:--force" "git:push:--force-with-lease" \
  ./my-project
```

```javascript
// SDK usage
const agent = new ProbeAgent({
  enableBash: true,
  bashConfig: {
    allow: ['git:push'],
    deny: ['git:push:--force', 'git:push:--force-with-lease']
  }
});
```

| Command | Result | Reason |
|---------|--------|--------|
| `git push origin main` | Allowed | Custom allow `git:push` overrides default deny |
| `git push --tags` | Allowed | Custom allow `git:push` overrides default deny |
| `git push --force` | **Denied** | Custom deny `git:push:--force` wins over custom allow |
| `git push --force-with-lease` | **Denied** | Custom deny wins over custom allow |
| `git reset --hard HEAD` | **Denied** | Still in default deny, not overridden |
| `git status` | Allowed | In default allow list (no conflict) |
| `rm -rf /` | **Denied** | Still in default deny, not overridden |

### Example: CI/CD Pipeline Permissions

```javascript
// Allow git operations needed for deployment
const agent = new ProbeAgent({
  enableBash: true,
  bashConfig: {
    allow: ['git:push', 'git:commit', 'git:add', 'npm:install', 'npm:run'],
    deny: [
      'git:push:--force',              // No force push
      'git:push:--force-with-lease',   // No force push variants
      'npm:run:eject',                 // Don't allow eject
    ]
  }
});
```

### Complex Commands (&&, ||, pipes)

When a command contains `&&`, `||`, or `|`, each component is checked independently using the same priority rules. **All components must be allowed** for the command to execute:

```bash
# Allowed: both components pass
git status && git push origin main   # (with --bash-allow "git:push")

# Denied: git push --force is in custom deny
git status && git push --force       # (with --bash-deny "git:push:--force")

# Denied: rm -rf is in default deny, not overridden
git push && rm -rf /                 # (with --bash-allow "git:push")
```

### Default Deny Patterns

Dangerous commands blocked by default (partial list):

| Category | Patterns |
|----------|----------|
| Destructive file ops | `rm:-rf`, `rm:-r`, `rmdir`, `mkfs` |
| Privilege escalation | `sudo`, `su`, `doas` |
| Git destructive | `git:push`, `git:reset`, `git:clean`, `git:commit`, `git:merge`, `git:rebase` |
| Git branch/tag delete | `git:branch:-d`, `git:branch:-D`, `git:tag:-d` |
| GitHub CLI writes | `gh:issue:create`, `gh:pr:merge`, `gh:repo:delete` |
| Package installs | `npm:install`, `pip:install`, `cargo:install` |
| Network downloads | `curl`, `wget`, `nc` |
| System operations | `shutdown`, `reboot`, `mount`, `crontab` |

### Default Allow Patterns

Safe read-only commands allowed by default (partial list):

| Category | Patterns |
|----------|----------|
| File exploration | `ls`, `cat`, `head`, `tail`, `find`, `grep`, `tree` |
| Git read-only | `git:status`, `git:log`, `git:diff`, `git:show`, `git:branch`, `git:blame` |
| Git plumbing | `git:cat-file`, `git:ls-files`, `git:rev-parse`, `git:merge-base` |
| GitHub CLI reads | `gh:issue:list`, `gh:pr:view`, `gh:repo:list`, `gh:search:*`, `gh:api` |
| Package info | `npm:list`, `pip:list`, `cargo:--version` |
| System info | `whoami`, `pwd`, `uname`, `date`, `env` |

### Nuclear Options (Not Recommended)

```bash
# Disable ALL default deny patterns (dangerous!)
probe-chat --enable-bash --no-default-bash-deny ./my-project

# Disable ALL default allow patterns (locks out all commands)
probe-chat --enable-bash --no-default-bash-allow ./my-project
```

These flags are available for edge cases but are **not recommended**. Use `--bash-allow` to selectively override specific deny patterns instead.

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
      'git:push',          // Override default deny for push
      'npm:run:test',      // Allow npm test
    ],
    deny: [
      'git:push:--force',  // But block force push (custom deny always wins)
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

