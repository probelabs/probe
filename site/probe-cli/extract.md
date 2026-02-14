# Extract Command

The `extract` command retrieves complete code blocks from files with full context. It's designed to extract functions, classes, and other code structures based on file locations, line numbers, or symbol names.

---

## TL;DR

```bash
# Extract function at line 42
probe extract src/auth.rs:42

# Extract by symbol name
probe extract src/auth.rs#login_user

# Extract from git diff
git diff | probe extract --diff

# Extract with LLM prompt template
probe extract src/api.ts:100 --prompt engineer

# Copy to clipboard
probe extract src/utils.py:25 --to-clipboard
```

---

## Basic Syntax

```bash
probe extract <FILE:LINE> [OPTIONS]
probe extract <FILE#SYMBOL> [OPTIONS]
probe extract <FILE:START-END> [OPTIONS]
```

### Extraction Methods

| Method | Syntax | Description |
|--------|--------|-------------|
| Line-based | `file.rs:42` | Extract block containing line 42 |
| Symbol-based | `file.rs#function_name` | Extract named symbol |
| Range-based | `file.rs:10-20` | Extract lines 10-20 |
| Multiple files | `file1.rs:10 file2.ts:20` | Extract from multiple locations |

---

## Input Sources

### From Arguments

```bash
# Single file
probe extract src/main.rs:42

# Multiple files
probe extract src/auth.rs:10 src/user.rs:50 src/api.rs:100

# Mixed methods
probe extract src/auth.rs:42 src/user.rs#User src/api.rs:10-50
```

### From Clipboard

```bash
# Read file:line references from clipboard
probe extract --from-clipboard
```

### From File

```bash
# Read from a file containing references
probe extract --input-file references.txt

# From error logs
cat error.log | grep -E '\w+\.\w+:\d+' | probe extract --input-file -
```

### From Git Diff

```bash
# Extract changed code blocks
git diff | probe extract --diff

# Extract from staged changes
git diff --cached | probe extract --diff

# Extract from specific commit
git show HEAD~1 | probe extract --diff
```

---

## Command Options

### Context Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-c`, `--context` | Number | 0 | Lines of context before/after |
| `-k`, `--keep-input` | Boolean | false | Keep and display original input |

```bash
# Add 5 lines of context
probe extract src/auth.rs:42 --context 5

# Keep original stack trace visible
probe extract --input-file error.log --keep-input
```

### Output Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-o`, `--format` | String | "color" | Output format |
| `--dry-run` | Boolean | false | Output only file:line references |
| `-t`, `--to-clipboard` | Boolean | false | Copy output to clipboard |

**Available Formats:**

| Format | Description |
|--------|-------------|
| `color` | Syntax-highlighted terminal output |
| `markdown` | Markdown with code fences |
| `plain` | Plain text without formatting |
| `json` | Structured JSON output |
| `xml` | Structured XML output |
| `outline-xml` | XML-formatted hierarchical outline |
| `outline-diff` | Diff-style outline format |

```bash
# Markdown for documentation
probe extract src/api.rs:42 --format markdown

# JSON for tooling
probe extract src/auth.rs:100 --format json

# Copy to clipboard
probe extract src/utils.ts:25 --to-clipboard
```

### Input Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-f`, `--from-clipboard` | Boolean | false | Read from clipboard |
| `-F`, `--input-file` | String | - | Read from file |
| `--diff` | Boolean | false | Parse input as git diff |

```bash
# From clipboard (IDE stack trace)
probe extract --from-clipboard

# From error log file
probe extract --input-file crash.log

# From piped git diff
git diff HEAD~3 | probe extract --diff
```

### LLM Prompt Templates

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--prompt` | String | - | System prompt template |
| `--instructions` | String | - | User instructions for LLM |

**Built-in Templates:**

| Template | Description |
|----------|-------------|
| `engineer` | Senior software engineer focus (code changes) |
| `architect` | Software architect focus (design decisions) |
| `code-review` | Code review focus |
| `code-review-template` | Code review with custom rules |
| Custom path | Load template from file |

```bash
# Engineer prompt for code modifications
probe extract src/auth.rs:42 --prompt engineer

# Code review with instructions
probe extract src/api.ts:100 --prompt code-review --instructions "Check for security issues"

# Custom template file
probe extract src/db.rs:50 --prompt ./prompts/database-review.md
```

### Filter Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--allow-tests` | Boolean | false | Include test files |
| `-i`, `--ignore` | String[] | - | Patterns to ignore |
| `--no-gitignore` | Boolean | false | Don't respect .gitignore |

```bash
# Include test utilities
probe extract tests/helpers.rs:10 --allow-tests

# Ignore generated files
probe extract src/generated.ts:50 --ignore "*.generated.*"
```

---

## Output Examples

### Color Output (Default)

```bash
probe extract src/auth/login.ts:15
```

```
─────────────────────────────────────────
📄 src/auth/login.ts:15-42
─────────────────────────────────────────
export async function login(
  email: string,
  password: string
): Promise<User> {
  const user = await db.findUserByEmail(email);
  if (!user) {
    throw new AuthError('User not found');
  }

  const valid = await bcrypt.compare(password, user.passwordHash);
  if (!valid) {
    throw new AuthError('Invalid password');
  }

  return generateSession(user);
}
─────────────────────────────────────────
```

### JSON Output

```bash
probe extract src/auth/login.ts:15 --format json
```

```json
{
  "blocks": [
    {
      "file": "src/auth/login.ts",
      "lines": { "start": 15, "end": 42 },
      "symbol": "login",
      "type": "function",
      "code": "export async function login(...) {...}",
      "language": "typescript"
    }
  ],
  "metadata": {
    "total_blocks": 1,
    "total_lines": 28
  }
}
```

### Markdown Output

```bash
probe extract src/auth/login.ts:15 --format markdown
```

````markdown
## src/auth/login.ts

### `login` (lines 15-42)

```typescript
export async function login(
  email: string,
  password: string
): Promise<User> {
  // ... implementation
}
```
````

---

## Common Patterns

### Debugging Workflows

```bash
# Extract from error stack trace
cat error.log | probe extract --input-file -

# Extract from clipboard (copy stack trace from IDE)
probe extract --from-clipboard --format markdown

# Extract with context for debugging
probe extract src/api.ts:142 --context 10
```

### Code Review

```bash
# Extract changed functions for review
git diff main | probe extract --diff --prompt code-review

# Review specific file changes
git diff HEAD~1 -- src/auth.ts | probe extract --diff --prompt code-review \
  --instructions "Focus on authentication security"
```

### Documentation

```bash
# Generate markdown documentation
probe extract src/api/endpoints.ts:* --format markdown > docs/api.md

# Extract public API for docs
probe extract src/index.ts#exportedFunction --format markdown
```

### AI Integration

```bash
# Prepare code for AI analysis
probe extract src/complex.rs:100 --prompt architect | llm "Review this design"

# Extract and copy for ChatGPT
probe extract src/auth.rs:42 --prompt engineer --to-clipboard

# JSON for programmatic AI tools
probe extract src/api.ts:50 --format json | ai-tool analyze
```

### Multiple Extractions

```bash
# Extract related code blocks
probe extract \
  src/models/user.ts:15 \
  src/services/auth.ts:42 \
  src/api/login.ts:100 \
  --format markdown

# From a list of references
cat important-functions.txt | xargs probe extract --format json
```

---

## Symbol Extraction

Extract code by symbol name rather than line number:

```bash
# Extract a function
probe extract src/auth.rs#authenticate_user

# Extract a class
probe extract src/models/user.py#UserModel

# Extract a method
probe extract src/api/handler.ts#handleRequest

# Extract an interface
probe extract src/types.ts#ApiResponse
```

**Note**: Symbol extraction uses tree-sitter to find the named symbol in the AST.

---

## Git Diff Integration

The `--diff` flag parses git diff output and extracts the changed code blocks:

```bash
# Current uncommitted changes
git diff | probe extract --diff

# Staged changes
git diff --cached | probe extract --diff

# Changes in a PR
git diff main...feature-branch | probe extract --diff

# Changes in last N commits
git diff HEAD~5 | probe extract --diff

# Specific file changes
git diff HEAD~1 -- src/auth.ts | probe extract --diff
```

The extractor identifies:
- Modified functions/methods
- Added code blocks
- Changed class definitions
- Updated configuration blocks

---

## Performance Tips

1. **Use symbol extraction** when you know the function name
2. **Use --dry-run** to preview what will be extracted
3. **Batch extractions** with multiple file arguments
4. **Use --context 0** for minimal output (default)

```bash
# Preview extraction targets
probe extract src/large-file.ts:100 --dry-run

# Batch extraction (faster than multiple commands)
probe extract file1.ts:10 file2.ts:20 file3.ts:30
```

---

## Related Documentation

- [Search Command](./search.md) - Find code to extract
- [Query Command](./query.md) - AST-based code queries
- [Output Formats](../output-formats.md) - Format specifications
- [AI Integration](../ai-integration.md) - Using extract with AI tools
