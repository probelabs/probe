# Search Command

The `search` command is Probe's primary tool for semantic code search. It combines ripgrep's speed with tree-sitter's AST parsing and intelligent ranking algorithms to find relevant code across your codebase.

---

## TL;DR

```bash
# Basic search
probe search "authentication" ./src

# With Elasticsearch-style operators
probe search "error AND handling" ./src

# Limit by tokens (for AI context windows)
probe search "user login" ./ --max-tokens 8000

# Language-specific search
probe search "interface" ./ --language typescript

# JSON output for machine processing
probe search "api" ./ --format json --max-results 20
```

---

## Basic Syntax

```bash
probe search "PATTERN" [PATH] [OPTIONS]
probe "PATTERN" [PATH] [OPTIONS]  # Shorthand (no subcommand needed)
```

| Argument | Description |
|----------|-------------|
| `PATTERN` | Search query (supports Elasticsearch syntax) |
| `PATH` | Directory to search (default: current directory) |

---

## Query Syntax

Probe supports Elasticsearch-style query syntax for powerful searches:

### Basic Terms

```bash
probe search "function"
probe search "authentication"
```

### Boolean Operators

| Operator | Example | Description |
|----------|---------|-------------|
| `AND` | `"error AND handling"` | Both terms must appear |
| `OR` | `"login OR auth"` | Either term matches |
| `NOT` | `"database NOT sqlite"` | Exclude term |
| `()` | `"(error OR exception) AND handle"` | Grouping |

### Wildcards

```bash
probe search "auth*"           # Matches: auth, authentication, authorize
probe search "connect*ion"     # Matches: connection, conection
```

### Exact Phrases

```bash
probe search "\"exact phrase\""   # Must match exactly
probe search "'user login'"       # Alternative quoting
```

---

## Search Hints (Filters)

Filter results using special hint syntax within your query:

| Hint | Example | Description |
|------|---------|-------------|
| `ext:<ext>` | `"function AND ext:rs"` | Filter by file extension |
| `file:<pattern>` | `"class AND file:src/**/*.py"` | Filter by file path pattern |
| `path:<pattern>` | `"error AND path:tests"` | Alias for file pattern |
| `dir:<pattern>` | `"config AND dir:settings"` | Filter by directory |
| `type:<type>` | `"struct AND type:rust"` | Filter by ripgrep file type |
| `lang:<language>` | `"component AND lang:javascript"` | Filter by programming language |
| `filename:<name>` | `"main AND filename:app.ts"` | Exact filename match |

### Examples

```bash
# Search only in Rust files
probe search "impl AND ext:rs" ./src

# Search in test directories
probe search "assert AND dir:tests" ./

# Search TypeScript React components
probe search "useState AND lang:typescript AND file:**/*.tsx" ./src
```

---

## Command Options

### Result Limiting

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--max-results` | Number | - | Maximum number of results |
| `--max-bytes` | Number | - | Maximum total bytes of code |
| `--max-tokens` | Number | - | Maximum tokens (for AI context) |

```bash
# Limit to 10 results
probe search "api" ./ --max-results 10

# Limit for Claude's context window
probe search "error handling" ./ --max-tokens 10000
```

### Search Behavior

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-s`, `--frequency` | Boolean | true | Use frequency-based search with stemming |
| `-e`, `--exact` | Boolean | false | Exact match (no tokenization) |
| `-f`, `--files-only` | Boolean | false | Output only file paths |
| `-n`, `--exclude-filenames` | Boolean | false | Exclude files matching query terms |
| `--allow-tests` | Boolean | false | Include test files |
| `--no-merge` | Boolean | false | Don't merge adjacent code blocks |
| `--merge-threshold` | Number | 5 | Lines between blocks to merge |

```bash
# Exact case-insensitive match
probe search "getUserById" ./ --exact

# Include test files
probe search "mock" ./ --allow-tests

# Get only file paths
probe search "deprecated" ./ --files-only
```

### Output Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-o`, `--format` | String | "outline" | Output format |
| `--dry-run` | Boolean | false | Output only file:line references |
| `-v`, `--verbose` | Boolean | false | Show timing and debug info |

**Available Formats:**

| Format | Description |
|--------|-------------|
| `terminal` | Plain text terminal output |
| `markdown` | Markdown formatted output |
| `plain` | Plain text without formatting |
| `json` | Structured JSON with metadata |
| `xml` | Structured XML output |
| `color` | Terminal with syntax highlighting |
| `outline` | Hierarchical code outline |
| `outline-xml` | XML-formatted outline |

```bash
# JSON for machine processing
probe search "function" ./ --format json

# Markdown for documentation
probe search "api" ./ --format markdown > api-docs.md
```

### Ranking Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-r`, `--reranker` | String | "bm25" | Ranking algorithm |
| `--question` | String | - | Natural language question (for BERT) |

**Available Rerankers:**

| Reranker | Description |
|----------|-------------|
| `bm25` | Default, proven information retrieval |
| `tfidf` | Term frequency-inverse document frequency |
| `hybrid` | BM25 + TF-IDF combination |
| `hybrid2` | Advanced hybrid with better metrics |
| `ms-marco-tinybert` | BERT-based (smallest, requires feature) |
| `ms-marco-minilm-l6` | BERT-based (medium) |
| `ms-marco-minilm-l12` | BERT-based (largest) |

```bash
# Use hybrid ranking
probe search "user management" ./ --reranker hybrid

# BERT reranking with question (requires --features bert-reranker)
probe search "api" ./ --reranker ms-marco-tinybert --question "How is the REST API structured?"
```

### Language Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-l`, `--language` | String | auto | Limit to programming language |

**Supported Languages:**
`rust`, `javascript`, `typescript`, `python`, `go`, `c`, `cpp`, `java`, `ruby`, `php`, `swift`, `csharp`, `yaml`, `html`, `markdown`

```bash
# Search only Python files
probe search "class" ./ --language python

# Search only TypeScript
probe search "interface" ./ -l typescript
```

### Session & Caching

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--session` | String | - | Session ID for pagination |
| `--timeout` | Number | 30 | Timeout in seconds |

```bash
# Paginated search with session
probe search "database" ./ --session my-search --max-results 50

# Get next page (automatic deduplication)
probe search "database" ./ --session my-search --max-results 50
```

### Ignore Patterns

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-i`, `--ignore` | String[] | - | Additional patterns to ignore |
| `--no-gitignore` | Boolean | false | Don't respect .gitignore |

```bash
# Ignore vendor and generated files
probe search "config" ./ --ignore "vendor/*" --ignore "*.generated.ts"
```

---

## Output Examples

### JSON Output

```bash
probe search "login" ./ --format json --max-results 2
```

```json
{
  "results": [
    {
      "file": "src/auth/login.ts",
      "lines": { "start": 15, "end": 42 },
      "code": "export async function login(email: string, password: string) {...}",
      "score": 0.892
    }
  ],
  "metadata": {
    "query": "login",
    "total_matches": 15,
    "returned": 2
  }
}
```

### Outline Output (Default)

```
src/auth/login.ts
├─ login (function) [15-42]
│  Handles user authentication with email/password
└─ validateCredentials (function) [44-58]
   Validates user credentials against database
```

---

## Common Patterns

### AI Context Optimization

```bash
# Search with token limit for Claude
probe search "authentication flow" ./ --max-tokens 8000 --format json

# Include test examples
probe search "user service" ./ --allow-tests --max-tokens 10000
```

### Codebase Exploration

```bash
# Find all API endpoints
probe search "router OR endpoint OR handler" ./ --files-only

# Find error handling patterns
probe search "(catch OR error OR exception) AND handle" ./src
```

### Code Review

```bash
# Find TODOs and FIXMEs
probe search "TODO OR FIXME OR HACK" ./ --format markdown

# Find deprecated code
probe search "deprecated OR obsolete" ./src
```

### Integration with AI Tools

```bash
# Pipe to clipboard for ChatGPT
probe search "database connection" ./ --format markdown | pbcopy

# Use with LLM CLI tools
probe search "api handler" ./ --format json | llm "explain this code"
```

---

## Performance Tips

1. **Use language filters** when you know the target language
2. **Set max-results** to avoid processing unnecessary matches
3. **Use --session** for paginated large result sets
4. **Enable DEBUG=1** to see timing information
5. **Use --files-only** for quick file enumeration

```bash
# Optimized search
DEBUG=1 probe search "config" ./ --language rust --max-results 20
```

---

## Related Documentation

- [Output Formats](../output-formats.md) - Detailed format specifications
- [Performance Tuning](./performance.md) - Optimization strategies
- [Extract Command](./extract.md) - Extract code blocks
- [Query Command](./query.md) - AST-based patterns
