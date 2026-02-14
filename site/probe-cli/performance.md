# Performance Tuning

Optimize Probe for large codebases and demanding workloads.

---

## TL;DR

```bash
# Optimized search
PROBE_PARSER_POOL_SIZE=8 probe search "query" ./ \
  --language rust \
  --max-results 50 \
  --session my-search

# Skip warmup for scripts
PROBE_NO_PARSER_WARMUP=1 probe search "query" ./

# Debug performance
DEBUG=1 probe search "query" ./
```

---

## Performance Architecture

Probe uses multiple optimization techniques:

```
┌─────────────────────────────────────────────────────┐
│                    Search Pipeline                   │
├─────────────────────────────────────────────────────┤
│  1. File Scanning (ripgrep)         │ Parallel     │
│  2. File Filtering (.gitignore)     │ O(1) lookup  │
│  3. Content Matching (SIMD)         │ Vectorized   │
│  4. AST Parsing (tree-sitter)       │ Cached       │
│  5. Block Extraction                │ Streamed     │
│  6. Ranking (BM25/SIMD)             │ Vectorized   │
│  7. Result Assembly                 │ Lazy         │
└─────────────────────────────────────────────────────┘
```

---

## Configuration Options

### Parser Pool Size

Control parallel parsing threads:

```bash
# Default: number of CPU cores
PROBE_PARSER_POOL_SIZE=8 probe search "query" ./

# For I/O bound workloads (network drives, SSDs)
PROBE_PARSER_POOL_SIZE=16 probe search "query" ./

# For CPU bound workloads (complex AST patterns)
PROBE_PARSER_POOL_SIZE=4 probe search "query" ./
```

**Recommendation:**
- SSD: 2x CPU cores
- HDD: 1x CPU cores
- Network: 4x CPU cores (hide latency)

### Tree Cache Size

Cache parsed ASTs for repeated queries:

```bash
# Increase cache for large codebases
PROBE_TREE_CACHE_SIZE=1000 probe search "query" ./

# Disable caching (low memory environments)
PROBE_TREE_CACHE_SIZE=0 probe search "query" ./
```

**Default:** Automatic based on available memory

### Parser Warmup

Skip warmup for faster cold starts:

```bash
# Skip warmup (faster first query, slower subsequent)
PROBE_NO_PARSER_WARMUP=1 probe search "query" ./

# Default behavior: warm up parsers on first use
probe search "query" ./
```

**Use cases:**
- CI/CD pipelines (single query)
- Script automation
- Quick one-off searches

---

## SIMD Optimization

Probe uses SIMD (Single Instruction, Multiple Data) for:

1. **Tokenization**: memchr, aho-corasick
2. **Ranking**: simsimd for dot products
3. **Pattern Matching**: Vectorized string operations

### Disabling SIMD

For debugging or compatibility:

```bash
# Disable all SIMD
DISABLE_SIMD_TOKENIZATION=1 \
DISABLE_SIMD_RANKING=1 \
DISABLE_SIMD_PATTERN_MATCHING=1 \
probe search "query" ./

# Disable specific features
DISABLE_SIMD_RANKING=1 probe search "query" ./
```

**Note:** Disabling SIMD reduces performance by 2-10x.

---

## Query Optimization

### Limit Results Early

```bash
# Stop after 20 results (fastest)
probe search "function" ./ --max-results 20

# Limit by token count (for AI)
probe search "function" ./ --max-tokens 8000

# Limit by bytes
probe search "function" ./ --max-bytes 50000
```

### Use Language Filters

```bash
# Search only Rust files (skip parsing others)
probe search "struct" ./ --language rust

# Use search hints
probe search "interface AND lang:typescript" ./

# Filter by extension
probe search "class AND ext:py" ./
```

### Use Sessions for Pagination

```bash
# First batch
probe search "api" ./ --session api-search --max-results 100

# Subsequent batches (deduplicated)
probe search "api" ./ --session api-search --max-results 100
```

Sessions cache:
- Parsed file list
- MD5 hashes for invalidation
- Previously returned blocks

### Skip Block Merging

For maximum speed, disable block merging:

```bash
probe search "function" ./ --no-merge
```

---

## Memory Optimization

### Large Codebases

```bash
# Limit results to prevent memory issues
probe search "query" ./ --max-results 100

# Use pagination
probe search "query" ./ --session my-search --max-results 50

# Process files sequentially
PROBE_PARSER_POOL_SIZE=1 probe search "query" ./
```

### Monitoring Memory

```bash
# On Linux
/usr/bin/time -v probe search "query" ./

# On macOS
/usr/bin/time -l probe search "query" ./
```

---

## Benchmarking

### Built-in Benchmarks

```bash
# Run all benchmarks
probe benchmark

# Run specific benchmark
probe benchmark --bench search

# Quick benchmarks only
probe benchmark --fast

# Save results
probe benchmark --format json --output results.json

# Compare with baseline
probe benchmark --compare --baseline previous.json
```

### Manual Timing

```bash
# Verbose output with timing
DEBUG=1 probe search "query" ./

# Time multiple runs
for i in {1..5}; do
  time probe search "query" ./ --max-results 10 2>&1 | tail -1
done
```

### Profiling

```bash
# With cargo flamegraph (requires installation)
cargo flamegraph -- search "query" ./path

# With perf (Linux)
perf record probe search "query" ./
perf report
```

---

## Best Practices

### 1. Use Appropriate Reranker

| Reranker | Speed | Quality | Use Case |
|----------|-------|---------|----------|
| `bm25` | Fast | Good | Default, most queries |
| `tfidf` | Fast | Good | Exact term matching |
| `hybrid` | Medium | Better | Balanced |
| `hybrid2` | Medium | Better | File-aware ranking |
| `ms-marco-*` | Slow | Best | When quality matters |

```bash
# Default (fast)
probe search "auth" ./

# High quality (slower)
probe search "auth" ./ --reranker ms-marco-tinybert
```

### 2. Directory Scope

```bash
# Bad: search everything
probe search "function" ./

# Good: search specific directories
probe search "function" ./src

# Better: multiple specific paths
probe search "function" ./src/core ./src/api
```

### 3. Ignore Patterns

```bash
# Exclude large directories
probe search "config" ./ \
  --ignore "node_modules/*" \
  --ignore "vendor/*" \
  --ignore "*.min.js"
```

### 4. Output Format

| Format | Speed | Use Case |
|--------|-------|----------|
| `outline` | Fastest | Interactive use |
| `plain` | Fast | Piping to tools |
| `json` | Medium | Machine processing |
| `color` | Medium | Terminal display |
| `markdown` | Slow | Documentation |

```bash
# Fast output for scripts
probe search "query" ./ --format plain

# Machine readable
probe search "query" ./ --format json
```

---

## Performance Comparison

Typical performance on a medium codebase (100k lines):

| Operation | Cold Start | Warm |
|-----------|------------|------|
| Simple search | 200ms | 50ms |
| With AST parsing | 500ms | 100ms |
| Full ranking | 800ms | 200ms |
| BERT reranking | 2000ms | 1500ms |

### vs Other Tools

| Tool | 100k files | Memory |
|------|------------|--------|
| grep | 2s | 10MB |
| ripgrep | 0.5s | 50MB |
| Probe (search) | 0.8s | 100MB |
| Probe (query) | 1.5s | 200MB |

---

## Debugging Performance

### Enable Debug Output

```bash
DEBUG=1 probe search "query" ./
```

Output shows:
- File scanning time
- Parsing time per file type
- Ranking time
- Total result count
- Memory usage

### Identify Bottlenecks

```
[DEBUG] Scan: 150ms (1234 files)
[DEBUG] Filter: 20ms (890 files)
[DEBUG] Parse: 400ms (890 files)  ← Parser pool too small?
[DEBUG] Rank: 50ms (234 blocks)
[DEBUG] Output: 30ms
[DEBUG] Total: 650ms
```

### Common Bottlenecks

| Bottleneck | Symptom | Solution |
|------------|---------|----------|
| File I/O | Slow scan phase | Use SSD, increase pool |
| Parsing | Slow parse phase | Increase PARSER_POOL_SIZE |
| Memory | OOM errors | Limit results, use sessions |
| Network | Very slow scan | Cache locally, increase pool |

---

## Related Documentation

- [Search Command](./search.md) - Search options
- [Query Command](./query.md) - Query optimization
- [Environment Variables](../reference/environment-variables.md) - All variables
- [Troubleshooting](../reference/troubleshooting.md) - Common issues
