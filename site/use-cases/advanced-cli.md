# Advanced CLI: Speed, Token Limits, and Large Repos

This guide covers advanced techniques for using Probe's command-line interface effectively, especially when working with large codebases and integrating with AI tools.

## Overview

Probe's CLI is designed for performance and flexibility, but working with large repositories or preparing results for AI models requires some additional techniques. This guide will show you how to optimize your workflow for speed, manage token limits, and effectively handle large codebases.

## Merging Code Blocks and Session-Based Caching

### Controlling Block Merging

By default, Probe merges adjacent code blocks to provide better context. You can control this behavior:

```bash
# Disable merging completely
probe search "authentication" --no-merge

# Adjust the threshold for merging (default is 5 lines)
probe search "authentication" --merge-threshold 10
```

When to use:
- Use `--no-merge` when you need precise, separate results
- Increase `--merge-threshold` when you want more context between related blocks
- Decrease `--merge-threshold` for more focused results

### Session-Based Caching

When performing multiple related searches, use session-based caching to avoid seeing the same code blocks repeatedly:

```bash
# First search - generates a session ID
probe search "authentication" --session ""
# Session: a1b2c3d4 (example output)

# Subsequent searches - reuse the session ID
probe search "login" --session "a1b2c3d4"
# Will skip code blocks already shown in the previous search
```

This is particularly useful when:
- Exploring a topic with multiple related searches
- Working with AI assistants to avoid repetitive results
- Building up a comprehensive understanding of a feature

## Dealing with Huge Monorepos

Large monorepos present unique challenges. Here are strategies to handle them effectively:

### Targeted Directory Searches

Instead of searching the entire repository, target specific directories:

```bash
# Search only in the authentication module
probe search "password reset" ./src/auth

# Search multiple specific directories
probe search "user profile" ./src/users ./src/profiles ./src/auth
```

### Custom Ignore Patterns

Use custom ignore patterns to exclude irrelevant directories:

```bash
# Exclude generated code, tests, and third-party libraries
probe search "api" --ignore "node_modules,dist,build,vendor,test,__tests__"
```

### Two-Phase Search Approach

For very large repositories, use a two-phase approach:

```bash
# Phase 1: Find relevant files
probe search "authentication" --files-only > auth-files.txt

# Phase 2: Search only in those files
cat auth-files.txt | xargs probe search "password reset"
```

### Parallel Processing

For extremely large codebases, split the search across multiple processes:

```bash
# Split the search across multiple directories
find ./src -type d -maxdepth 1 | xargs -P 4 -I {} probe search "error handling" {} --format json
```

## Scripting with grep, xargs, etc.

Probe integrates well with standard Unix tools for powerful workflows:

### Piping and Filtering

```bash
# Find code, then filter with grep
probe search "database" | grep "connection"

# Process results with awk
probe search "function" --format plain | awk '/export/ {print $0}'

# Count occurrences
probe search "TODO" --format plain | grep -c "TODO"
```

### Advanced xargs Usage

```bash
# Extract code from all files matching a pattern
probe search "class" --files-only | xargs -I {} probe extract {}

# Process multiple search terms
echo -e "auth\nuser\nprofile" | xargs -I {} probe search "{}" ./src
```

### Creating Custom Reports

```bash
# Generate a markdown report of all TODO comments
probe search "TODO" --format markdown > todo-report.md

# Create a JSON report of error handling patterns
probe search "try|catch|error" --format json > error-handling.json
```

## Token-Limiting for AI Context Windows

When using Probe with AI models, managing token count is crucial:

### Limiting Output Size

```bash
# Limit by token count (for AI context windows)
probe search "authentication flow" --max-tokens 8000

# Limit by byte size
probe search "authentication flow" --max-bytes 16000

# Limit by result count
probe search "authentication flow" --max-results 5
```

### Format Selection for AI

Different AI models work better with different formats:

```bash
# Markdown format for most AI models
probe search "authentication" --format markdown

# Plain text for simpler models
probe search "authentication" --format plain

# JSON for programmatic processing
probe search "authentication" --format json
```

### Optimizing for Token Efficiency

```bash
# Exclude filenames to save tokens
probe search "authentication" --exclude-filenames

# Use exact matching to reduce noise
probe search "authentication" --exact

# Focus on specific file types
probe search "authentication" ./src --ignore "*.test.js,*.spec.js,*.css,*.html"
```

## Performance Optimization Techniques

### Search Speed Optimization

```bash
# Use frequency-based search (default)
probe search "authentication" --frequency

# For exact literal matching (faster but less flexible)
probe search "exactString" --exact

# Search only filenames first, then content
probe search "config" --files-only
```

### Result Ranking Control

```bash
# Use different ranking algorithms
probe search "authentication" --reranker hybrid  # Default
probe search "authentication" --reranker bm25    # Better for longer documents
probe search "authentication" --reranker tfidf   # Classic algorithm
```

### Memory Usage Optimization

For very large repositories or limited memory environments:

```bash
# Process files in smaller batches
find ./src -name "*.js" | split -l 100 - batch_
for batch in batch_*; do
  probe search "memory leak" $(cat $batch)
  rm $batch
done
```

## Practical Examples

### Finding Security Issues

```bash
# Search for common security vulnerabilities
probe search "password|token|secret|api_key" --format markdown > security-audit.md

# Look for SQL injection vulnerabilities
probe search "exec|eval|SELECT.*FROM.*WHERE" --format json > sql-injection-check.json
```

### Code Quality Analysis

```bash
# Find TODO comments
probe search "TODO|FIXME|HACK" > todos.txt

# Look for large functions
probe query "function $NAME($$$PARAMS) $$$BODY" ./src --language javascript > large-functions.txt

# Find deprecated API usage
probe search "deprecated" --format markdown > deprecated-usage.md
```

### Documentation Generation

```bash
# Extract all exported functions
probe query "export function $NAME($$$PARAMS) $$$BODY" ./src --language typescript > exported-functions.txt

# Generate API documentation
probe query "export function $NAME($$$PARAMS) $$$BODY" ./src --language typescript | \
  probe extract > api-docs.md
```

## Next Steps

- For AI integration, see [Integrating Probe into AI Code Editors](/use-cases/integrating-probe-into-ai-code-editors)
- For team collaboration, check out [Deploying the Probe Web Interface](/use-cases/deploying-probe-web-interface)
- For programmatic access, explore [Building AI Tools on Probe](/use-cases/building-ai-tools)
- For complete command reference, see the [CLI Reference (Commands & Flags)](/cli-mode)