# Query Patterns Guide

Best practices and common patterns for effective code search with Probe.

---

## Search Strategy

### Start Broad, Then Narrow

```bash
# 1. Start with broad search
probe search "authentication" ./

# 2. Narrow by context
probe search "authentication AND middleware" ./

# 3. Filter by location
probe search "authentication AND dir:src/auth" ./

# 4. Use specific patterns
probe query "fn authenticate($$$)" ./src/auth -l rust
```

### Combine Search Types

| Need | Use | Example |
|------|-----|---------|
| Find by concept | `search` | `probe search "error handling"` |
| Find by structure | `query` | `probe query "try { $$$ } catch"` |
| Find exact text | `grep` | `probe grep "TODO: fix"` |
| Extract context | `extract` | `probe extract file.rs:42` |

---

## Effective Search Queries

### Boolean Operators

```bash
# AND: both terms required
probe search "database AND connection" ./

# OR: either term matches
probe search "login OR authenticate OR auth" ./

# NOT: exclude matches
probe search "error NOT test" ./

# Grouping: complex conditions
probe search "(error OR exception) AND (handler OR process)" ./
```

### Search Hints

```bash
# By file extension
probe search "class AND ext:ts,tsx" ./

# By language
probe search "struct AND lang:rust" ./

# By directory
probe search "config AND dir:settings" ./

# By file path
probe search "api AND file:**/routes/*" ./

# Combine hints
probe search "interface AND lang:typescript AND dir:types" ./
```

### Wildcards

```bash
# Prefix matching
probe search "auth*" ./        # auth, authenticate, authorization

# Suffix matching
probe search "*Handler" ./     # errorHandler, requestHandler

# Pattern matching
probe search "get*User*" ./    # getUserById, getCurrentUser
```

---

## Common Search Patterns

### Finding API Endpoints

```bash
# REST endpoints
probe search "router OR endpoint OR handler AND api" ./

# Express.js routes
probe query "app.$METHOD('$PATH', $$$)" ./src -l javascript

# FastAPI routes
probe query "@app.$METHOD('$PATH')" ./src -l python
```

### Finding Database Operations

```bash
# SQL queries
probe search "SELECT OR INSERT OR UPDATE OR DELETE" ./

# ORM operations
probe search "query OR find OR create AND database" ./

# Transaction handling
probe query "transaction { $$$BODY }" ./src -l rust
```

### Finding Error Handling

```bash
# Try-catch patterns
probe query "try { $$$TRY } catch ($E) { $$$CATCH }" ./src -l javascript

# Rust error handling
probe query "match $EXPR { Ok($OK) => $$$, Err($ERR) => $$$ }" ./src -l rust

# Python exceptions
probe search "except AND handle" ./
probe query "except $EXCEPTION as $E: $$$" ./src -l python
```

### Finding Configuration

```bash
# Config files
probe search "config AND settings" ./ --files-only

# Environment variables
probe search "process.env OR getenv OR env::" ./

# Feature flags
probe search "feature AND flag OR toggle" ./
```

### Finding Tests

```bash
# Test functions
probe search "test OR spec" ./ --allow-tests

# Test assertions
probe query "expect($VALUE).toBe($EXPECTED)" ./tests -l javascript

# Rust tests
probe query "#[test] fn $NAME() { $$$BODY }" ./src -l rust
```

### Finding TODOs and Technical Debt

```bash
# Comment markers
probe search "TODO OR FIXME OR HACK OR XXX" ./

# Deprecated code
probe search "deprecated OR obsolete" ./

# Temporary code
probe search "temporary OR workaround" ./
```

---

## AST Query Patterns

### Function Definitions

**Rust:**
```bash
# All functions
probe query "fn $NAME($$$PARAMS) { $$$BODY }" ./src -l rust

# Async functions
probe query "async fn $NAME($$$PARAMS) { $$$BODY }" ./src -l rust

# Functions returning Result
probe query "fn $NAME($$$) -> Result<$OK, $ERR> { $$$ }" ./src -l rust

# Public functions
probe query "pub fn $NAME($$$) { $$$ }" ./src -l rust
```

**JavaScript/TypeScript:**
```bash
# Function declarations
probe query "function $NAME($$$PARAMS) { $$$BODY }" ./src -l javascript

# Arrow functions
probe query "const $NAME = ($$$PARAMS) => $BODY" ./src -l typescript

# Async functions
probe query "async function $NAME($$$) { $$$ }" ./src -l javascript

# Class methods
probe query "$NAME($$$PARAMS) { $$$BODY }" ./src -l javascript
```

**Python:**
```bash
# Function definitions
probe query "def $NAME($$$PARAMS): $$$BODY" ./src -l python

# Async functions
probe query "async def $NAME($$$): $$$" ./src -l python

# Decorated functions
probe query "@$DEC def $NAME($$$): $$$" ./src -l python

# Class methods
probe query "def $NAME(self, $$$): $$$" ./src -l python
```

### Class Definitions

```bash
# TypeScript classes
probe query "class $NAME extends $BASE { $$$BODY }" ./src -l typescript

# Python classes
probe query "class $NAME($BASES): $$$BODY" ./src -l python

# Rust structs with impl
probe query "impl $TRAIT for $STRUCT { $$$METHODS }" ./src -l rust
```

### Import Patterns

```bash
# ES6 named imports
probe query "import { $$$NAMES } from '$MODULE'" ./src -l javascript

# Default imports
probe query "import $NAME from '$MODULE'" ./src -l javascript

# Python imports
probe query "from $MODULE import $$$NAMES" ./src -l python

# Rust use statements
probe query "use $$$PATH;" ./src -l rust
```

### React Patterns

```bash
# Component definitions
probe query "function $NAME($PROPS) { return ($$$JSX) }" ./src -l javascript

# useState hooks
probe query "const [$STATE, $SET_STATE] = useState($INITIAL)" ./src -l javascript

# useEffect hooks
probe query "useEffect(() => { $$$BODY }, [$$$DEPS])" ./src -l javascript

# Custom hooks
probe query "function use$NAME($$$) { $$$BODY }" ./src -l javascript
```

---

## Performance Patterns

### Efficient Large Codebase Search

```bash
# Limit results early
probe search "api" ./ --max-results 50

# Use pagination
probe search "api" ./ --session api-search --max-results 50

# Filter by language
probe search "interface" ./ --language typescript

# Target specific directories
probe search "model" ./src/models ./src/types
```

### AI-Optimized Queries

```bash
# Token-limited for context windows
probe search "authentication flow" ./ --max-tokens 8000 --format json

# Extract with prompt
probe extract src/auth.rs:42 --prompt engineer

# Outline format for structure
probe search "api handler" ./ --format outline
```

---

## Combining Tools

### Search → Extract Workflow

```bash
# 1. Find relevant files
FILES=$(probe search "user authentication" ./ --files-only)

# 2. Extract specific code
for file in $FILES; do
  probe extract "$file:1" --format markdown
done
```

### Search → Query Workflow

```bash
# 1. Find files with concept
probe search "middleware" ./src --files-only

# 2. Query for specific structure
probe query "function $NAME(req, res, next) { $$$ }" ./src/middleware -l javascript
```

### Interactive Exploration

```bash
# 1. Overview search
probe search "authentication" ./ --max-results 10

# 2. Drill into specific file
probe extract src/auth/login.ts:15

# 3. Find related code
probe search "login AND session" ./src/auth

# 4. Structural analysis
probe query "class $NAME extends AuthService { $$$ }" ./src -l typescript
```

---

## Tips

### 1. Use Files-Only for Discovery

```bash
# See what files contain a concept
probe search "database" ./ --files-only
```

### 2. Combine Text and Structure

```bash
# Find files by concept, query by structure
probe search "validation AND input" ./src --files-only
probe query "function validate$NAME($$$) { $$$ }" ./src -l javascript
```

### 3. Iterate on Patterns

```bash
# Start simple
probe query "function $NAME() {}" ./src -l javascript

# Add constraints
probe query "async function $NAME($$$) { $$$ }" ./src -l javascript

# Add return types (TypeScript)
probe query "async function $NAME($$$): Promise<$T> { $$$ }" ./src -l typescript
```

### 4. Use Session for Exploration

```bash
# Paginate through results
probe search "api" ./ --session explore --max-results 20
probe search "api" ./ --session explore --max-results 20  # Next page
probe search "api" ./ --session explore --max-results 20  # And next...
```

---

## Related Documentation

- [Search Command](../probe-cli/search.md) - Search reference
- [Query Command](../probe-cli/query.md) - Query reference
- [Extract Command](../probe-cli/extract.md) - Extract reference
- [Performance Tuning](../probe-cli/performance.md) - Optimization
