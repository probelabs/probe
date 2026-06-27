# Query Command

The `query` command performs AST-based structural code search using tree-sitter patterns. Unlike semantic search, query finds code by its syntactic structure, making it ideal for finding specific code patterns regardless of naming.

---

## TL;DR

```bash
# Find all function definitions in Rust
probe query "fn $NAME($$$PARAMS) { $$$BODY }" ./src --language rust

# Find all React useState hooks
probe query "useState($INITIAL)" ./src --language javascript

# Find class definitions in Python
probe query "class $NAME: $$$BODY" ./src --language python

# Search line-oriented text files when no parser exists
probe query "reqproof:documents" ./docs --format json
```

---

## Basic Syntax

```bash
probe query "PATTERN" [PATH] [OPTIONS]
```

| Argument | Description |
|----------|-------------|
| `PATTERN` | AST-grep pattern with placeholders |
| `PATH` | Directory to search (default: current directory) |

---

## Pattern Syntax

Query uses [ast-grep](https://ast-grep.github.io/) pattern syntax with metavariables:

### Metavariables

| Syntax | Description | Example |
|--------|-------------|---------|
| `$NAME` | Match single node | `fn $NAME()` matches `fn foo()` |
| `$$$BODY` | Match multiple nodes | `{ $$$BODY }` matches function body |
| `$_` | Match any single node (anonymous) | `if $_ { }` |

### Examples by Language

**Rust:**
```bash
# Function definitions
probe query "fn $NAME($$$PARAMS) -> $RET { $$$BODY }" ./src -l rust

# Struct definitions
probe query "struct $NAME { $$$FIELDS }" ./src -l rust

# Impl blocks
probe query "impl $TRAIT for $TYPE { $$$METHODS }" ./src -l rust

# Match expressions
probe query "match $EXPR { $$$ARMS }" ./src -l rust
```

**JavaScript/TypeScript:**
```bash
# Function declarations
probe query "function $NAME($$$PARAMS) { $$$BODY }" ./src -l javascript

# Arrow functions
probe query "const $NAME = ($$$PARAMS) => $BODY" ./src -l typescript

# React hooks
probe query "useState($INITIAL)" ./src -l javascript
probe query "useEffect(() => { $$$BODY }, [$$$DEPS])" ./src -l javascript

# Class methods
probe query "class $NAME { $$$BODY }" ./src -l typescript
```

**Python:**
```bash
# Function definitions
probe query "def $NAME($$$PARAMS): $$$BODY" ./src -l python

# Class definitions
probe query "class $NAME: $$$BODY" ./src -l python

# Decorators
probe query "@$DECORATOR def $NAME($$$PARAMS): $$$BODY" ./src -l python

# With statements
probe query "with $CONTEXT as $VAR: $$$BODY" ./src -l python
```

**Go:**
```bash
# Function definitions
probe query "func $NAME($$$PARAMS) $RET { $$$BODY }" ./src -l go

# Struct definitions
probe query "type $NAME struct { $$$FIELDS }" ./src -l go

# Interface definitions
probe query "type $NAME interface { $$$METHODS }" ./src -l go

# Goroutines
probe query "go $FUNC($$$ARGS)" ./src -l go
```

---

## Command Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-l`, `--language` | String | auto | Programming language |
| `--max-results` | Number | - | Maximum results to return |
| `-o`, `--format` | String | "color" | Output format |
| `--allow-tests` | Boolean | false | Include test files |
| `-i`, `--ignore` | String[] | - | Patterns to ignore |
| `--no-gitignore` | Boolean | false | Don't respect .gitignore |
| `--with-context`, `--owner-context` | Boolean | false | Include owning source-block context in JSON output |
| `--strict` | Boolean | false | Disable plain-text fallback for unsupported extensions |
| `--text-extension` | String[] | - | Treat an extension as plain text (repeatable, with or without `.`) |

### Plain-Text Fallback

When query encounters a file without a supported parser and no explicit language is requested, it falls back to line-oriented text search. The pattern is treated as a literal substring, and each matching line is returned as a `node_type: "text"` result.

This allows documentation and config-style files such as `.1`, `.5`, `.txt`, `.conf`, `.tex`, `.sh`, and `.json` to be searched without a separate code path:

```bash
probe query "reqproof:documents" ./docs --format json
```

Use `--strict` for AST-only behavior, or `--text-extension EXT` to force additional suffixes into plain-text mode:

```bash
probe query "reqproof:documents" ./docs --text-extension req --format json
```

### Language Options

Required for accurate parsing:

| Language | Flag Value | File Extensions |
|----------|------------|-----------------|
| Rust | `rust` | .rs |
| JavaScript | `javascript`, `js` | .js, .jsx |
| TypeScript | `typescript`, `ts` | .ts, .tsx |
| Python | `python`, `py` | .py |
| Go | `go` | .go |
| C | `c` | .c, .h |
| C++ | `cpp`, `c++` | .cpp, .hpp, .cc |
| Java | `java` | .java |
| Ruby | `ruby`, `rb` | .rb |
| PHP | `php` | .php |
| Swift | `swift` | .swift |
| Solidity | `solidity`, `sol` | .sol |
| C# | `csharp`, `cs` | .cs |

```bash
probe query "fn $NAME()" ./src --language rust
probe query "def $NAME():" ./src -l python
```

### Output Formats

```bash
# Colored terminal output (default)
probe query "fn $NAME()" ./src --format color

# Markdown
probe query "fn $NAME()" ./src --format markdown

# JSON for tooling
probe query "fn $NAME()" ./src --format json

# JSON with owning source-block context
probe query "fetch($$$ARGS)" ./src --language typescript --format json --with-context

# Plain text
probe query "fn $NAME()" ./src --format plain
```

### Owner Context JSON

Use `--with-context` when a structural match is not enough by itself and the caller also needs the source block a human would inspect. This keeps `query` precise while adding neutral source facts such as the owning function, method, class, attached comments, and enclosing calls.

```bash
probe query "fetch($$$ARGS)" ./src --language typescript --format json --with-context
```

The default JSON fields remain available. With context enabled, each result can also include:

```json
{
  "schema_version": "probe.query.context.v1",
  "results": [
    {
      "file": "src/api.ts",
      "lines": [4, 4],
      "node_type": "match",
      "content": "fetch(url)",
      "column_start": 10,
      "column_end": 20,
      "language": "typescript",
      "pattern": {
        "source": "fetch($$$ARGS)",
        "id": null
      },
      "match": {
        "node_type": "call_expression",
        "content": "fetch(url)",
        "lines": [4, 4],
        "columns": [10, 20]
      },
      "owner": {
        "symbol": "loadProfile",
        "qualified_symbol": "loadProfile",
        "node_type": "function_declaration",
        "scope": "function",
        "lines": [2, 5],
        "columns": [1, 2],
        "comments": [
          {
            "kind": "leading",
            "start_line": 1,
            "end_line": 1,
            "text": "// Network boundary: user profile API client."
          }
        ]
      }
    }
  ]
}
```

Probe does not interpret comment contents or apply domain policy. Downstream tools decide whether a comment is a requirement, security annotation, checklist marker, or ordinary text.

---

## Common Patterns

### Finding Function Calls

```bash
# Find all calls to a specific function
probe query "log($$$ARGS)" ./src -l javascript

# Find method calls on objects
probe query "$OBJ.map($$$ARGS)" ./src -l javascript

# Find API calls
probe query "fetch($URL, $$$OPTS)" ./src -l javascript
```

### Finding Assignments

```bash
# Variable assignments
probe query "let $NAME = $VALUE" ./src -l javascript
probe query "const $NAME = $VALUE" ./src -l typescript
probe query "$NAME = $VALUE" ./src -l python
```

### Finding Control Flow

```bash
# If statements
probe query "if $COND { $$$BODY }" ./src -l rust

# Try-catch blocks
probe query "try { $$$TRY } catch ($ERR) { $$$CATCH }" ./src -l javascript

# For loops
probe query "for $VAR in $ITER { $$$BODY }" ./src -l rust
```

### Finding Imports/Exports

```bash
# ES6 imports
probe query "import { $$$NAMES } from '$MODULE'" ./src -l javascript

# Require statements
probe query "const $NAME = require('$MODULE')" ./src -l javascript

# Python imports
probe query "from $MODULE import $$$NAMES" ./src -l python

# Rust use statements
probe query "use $$$PATH;" ./src -l rust
```

### Finding Error Handling

```bash
# Rust Result handling
probe query "$EXPR?" ./src -l rust
probe query "match $EXPR { Ok($OK) => $$$, Err($ERR) => $$$ }" ./src -l rust

# JavaScript error handling
probe query "throw new $ERROR($$$ARGS)" ./src -l javascript

# Python exceptions
probe query "raise $EXCEPTION($$$ARGS)" ./src -l python
```

---

## Query vs Search

| Feature | Query | Search |
|---------|-------|--------|
| Pattern type | AST structure | Text/semantic |
| Use case | Exact code patterns | Finding related code |
| Naming dependence | No | Yes |
| Performance | Slower (full parse) | Faster |
| Wildcards | Metavariables | Text wildcards |

**Use Query when:**
- You know the exact code structure
- Variable names don't matter
- Finding structural patterns (all functions, all classes)
- Refactoring tasks

**Use Search when:**
- Looking for concepts
- Variable names are clues
- Exploring unfamiliar code
- Text-based patterns

---

## Output Examples

### Color Output (Default)

```
src/auth/login.rs:15
──────────────────────────────────────
fn login(email: String, password: String) -> Result<User, AuthError> {
    let user = find_user(&email)?;
    verify_password(&password, &user.hash)?;
    Ok(user)
}
──────────────────────────────────────

src/auth/register.rs:22
──────────────────────────────────────
fn register(email: String, password: String) -> Result<User, AuthError> {
    validate_email(&email)?;
    let hash = hash_password(&password)?;
    create_user(email, hash)
}
──────────────────────────────────────
```

### JSON Output

```json
{
  "results": [
    {
      "file": "src/auth/login.rs",
      "lines": { "start": 15, "end": 20 },
      "pattern": "fn $NAME($$$PARAMS) -> $RET { $$$BODY }",
      "matches": {
        "NAME": "login",
        "PARAMS": "email: String, password: String",
        "RET": "Result<User, AuthError>"
      },
      "code": "fn login(...) { ... }"
    }
  ]
}
```

---

## Tips and Best Practices

### Start Simple

```bash
# Too specific (might miss variations)
probe query "fn process_user_data(user: &User, data: &Data) -> Result<(), Error>" ./

# Better: use metavariables
probe query "fn $NAME($$$PARAMS) -> Result<$OK, $ERR>" ./
```

### Use Language Flag

Always specify language for accurate parsing:

```bash
# May fail or give wrong results
probe query "function $NAME() {}" ./

# Correct
probe query "function $NAME() {}" ./ --language javascript
```

### Combine with Search

Use search to find relevant files, then query for structure:

```bash
# Find files with authentication
probe search "auth" ./ --files-only

# Then query for specific patterns
probe query "fn authenticate($$$)" ./src/auth -l rust
```

### Test Patterns

Start with dry-run to see what matches:

```bash
probe query "fn $NAME()" ./src -l rust --max-results 5
```

---

## Troubleshooting

### "No matches found"

1. **Check language flag**: Pattern syntax varies by language
2. **Simplify pattern**: Start with broader patterns
3. **Check file types**: Use `--allow-tests` if needed

### "Pattern parse error"

1. **Check syntax**: Metavariables start with `$`
2. **Match language syntax**: Patterns must be valid code structure
3. **Escape special chars**: Some characters need escaping

### Performance issues

1. **Limit results**: Use `--max-results`
2. **Narrow scope**: Search specific directories
3. **Use language filter**: Avoid parsing all file types

---

## Related Documentation

- [Search Command](./search.md) - Semantic code search
- [Extract Command](./extract.md) - Code extraction
- [Search Syntax](./search-syntax.md) - Query syntax reference
- [Language Support](../supported-languages.md) - Supported languages
