# COMMAND-LINE POWER

Direct access to Probe's core functionality through clean, simple CLI commands.

## SEARCH COMMAND

Find code across your entire codebase:

```bash
probe search <QUERY> [PATH] [OPTIONS]
```

### CORE OPTIONS

| Option | Function |
|--------|----------|
| `<QUERY>` | **Required**: What to search for |
| `[PATH]` | Where to search (default: current directory) |
| `--files-only` | List matching files without code blocks |
| `--ignore <PATTERN>` | Additional patterns to ignore |
| `--reranker, -r <TYPE>` | Algorithm: `hybrid`, `bm25`, `tfidf` |
| `--frequency, -s` | Enable smart token matching (default) |
| `--exact` | Literal text matching only |
| `--max-results <N>` | Limit number of results |
| `--max-tokens <N>` | Limit total tokens (for AI) |
| `--allow-tests` | Include test files and code |
| `--any-term` | Match any search term (OR logic) |
| `--no-merge` | Keep code blocks separate |

### COMMAND EXAMPLES

```bash
# BASIC SEARCH - CURRENT DIRECTORY
probe search "authentication flow"

# EXACT MATCH IN SPECIFIC FOLDER
probe search "updateUser" ./src/api --exact

# LIMIT FOR AI CONTEXT WINDOWS
probe search "error handling" --max-tokens 8000

# FIND RAW FILES WITHOUT PARSING
probe search "config" --files-only
```

## EXTRACT COMMAND

Pull complete code blocks from specific files and lines:

```bash
probe extract <FILES> [OPTIONS]
```

### EXTRACT OPTIONS

| Option | Function |
|--------|----------|
| `<FILES>` | Files to extract from (e.g., `main.rs:42` or `main.rs#function_name`) |
| `--allow-tests` | Include test code blocks |
| `-c, --context <N>` | Add N context lines |
| `-f, --format <TYPE>` | Output as: `markdown`, `plain`, `json` |

### EXTRACTION EXAMPLES

```bash
# GET FUNCTION CONTAINING LINE 42
probe extract src/main.rs:42

# EXTRACT MULTIPLE BLOCKS
probe extract src/auth.js:15 src/api.js:27

# EXTRACT BY SYMBOL NAME
probe extract src/main.rs#handle_extract

# OUTPUT AS JSON
probe extract src/handlers.rs:108 --format json

# ADD SURROUNDING CONTEXT
probe extract src/utils.rs:72 --context 5
```

## POWER TECHNIQUES

### FROM COMPILER ERRORS

Feed error output directly to extract relevant code:

```bash
# EXTRACT CODE FROM COMPILER ERRORS
rustc main.rs 2>&1 | probe extract

# PULL CODE FROM TEST FAILURES 
go test ./... | probe extract
```

### UNIX PIPELINE INTEGRATION

Chain with other tools for maximum effect:

```bash
# FIND THEN FILTER 
probe search "database" | grep "connection"

# PROCESS & FORMAT
probe search "api" --format json | jq '.results[0]'
```

## COMMAND COMBINATIONS

Create powerful workflows by combining features:

```bash
# FIND AUTHENTICATION CODE WITHOUT TESTS
probe search "authenticate" --max-results 10 --ignore "test" --no-merge

# EXTRACT SPECIFIC FUNCTIONS WITH CONTEXT
grep -n "handleRequest" ./src/*.js | cut -d':' -f1,2 | probe extract --context 3

# FIND AND EXTRACT ERROR HANDLERS
probe search "error handling" --files-only | xargs -I{} probe extract {} --format markdown
```
