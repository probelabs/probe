# Architecture: Probe → OpenAPI Search (Go)

This document maps the probe search architecture to this Go implementation.

## Component Mapping

### 1. Tokenization

| Probe (Rust) | This Implementation (Go) |
|--------------|--------------------------|
| `src/search/tokenization.rs:2698-2820` | `tokenizer/tokenizer.go:Tokenize()` |
| `split_camel_case()` (lines 1908-2051) | `splitCamelCase()` |
| `split_compound_word()` (lines 2087-2149) | Not implemented (less critical for API specs) |
| `rust-stemmers` (Porter2) | `github.com/kljensen/snowball` |
| `STOP_WORDS` set | `buildStopWords()` map |
| `SPECIAL_CASE_WORDS` | `buildSpecialCases()` map |

**Key Differences:**
- Go version omits compound word splitting (database → data+base) as it's less relevant for OpenAPI specs
- Uses Porter2 stemmer via snowball package instead of rust-stemmers
- Simpler caching strategy (no LRU cache for compound words)

### 2. BM25 Ranking

| Probe (Rust) | This Implementation (Go) |
|--------------|--------------------------|
| `src/ranking.rs:184-208` | `ranker/bm25.go:scoreBM25()` |
| `rank_documents()` (lines 279-428) | `Rank()` |
| `precompute_idfs()` (lines 115-144) | Inlined in `Rank()` |
| `compute_avgdl()` (lines 64-72) | `computeAvgDocLength()` |
| Rayon parallel scoring | Goroutines with sync.WaitGroup |
| `HashMap<u8, usize>` for TF | `map[string]int` |

**Parameters:**
- Both use `k1 = 1.5` (vs standard 1.2)
- Both use `b = 0.5` (vs standard 0.75)
- Lower `b` reduces penalty for longer documents (better for code/specs)

**Key Differences:**
- Go uses string keys instead of u8 indices (no 256 term limit)
- Go uses goroutines instead of Rayon for parallelism
- No SIMD optimization (probe has `src/simd_ranking.rs`)

### 3. Query Processing

| Probe (Rust) | This Implementation (Go) |
|--------------|--------------------------|
| `src/search/elastic_query.rs` | Not implemented (simplified) |
| Boolean query AST | Not needed for basic search |
| `evaluate_with_cache()` | Not implemented |
| LRU cache (1000 entries) | Not implemented |

**Simplified Approach:**
- This implementation treats all query terms as optional (OR semantics)
- No support for `+required`, `-excluded`, `AND`, `OR` operators yet
- Could be added by porting `elastic_query.rs` AST structure

### 4. Search Pipeline

| Probe (Rust) | This Implementation (Go) |
|--------------|--------------------------|
| `src/search/search_runner.rs:362-1598` | `search/engine.go:Search()` |
| File searching with ripgrep | Direct iteration (small dataset) |
| Tree-sitter AST parsing | OpenAPI YAML/JSON parsing |
| Code block extraction | Endpoint extraction |
| Early ranking + batch processing | Single-pass ranking |
| Session caching | Not implemented |

**Simplified Pipeline:**
```
Probe:
Query → Parse → Pattern Gen → File Search → Early Rank →
Batch Process → AST Parse → Extract → BM25 → Merge → Cache

This Implementation:
Query → Tokenize → Index Endpoints → BM25 Rank → Return
```

**Key Differences:**
- No incremental/batch processing (all endpoints ranked at once)
- No caching layer (suitable for small datasets)
- No early filtering (AST evaluation not needed)
- No pattern generation or regex matching

### 5. Data Structures

| Probe (Rust) | This Implementation (Go) |
|--------------|--------------------------|
| `SearchResult` struct | `SearchResult` struct |
| `Document` (implicit) | `ranker.Document` |
| Tree-sitter `Node` | OpenAPI endpoint struct |
| `QueryPlan` | Not needed (no complex queries) |
| `HashMap<PathBuf, HashMap<usize, HashSet<usize>>>` | Direct endpoint iteration |

## Algorithm Implementations

### Tokenization Flow

**Probe:**
```
text → whitespace split → non-alnum split → camelCase split →
compound split → stop word filter → stem → dedupe
```

**This Implementation:**
```
text → whitespace split → non-alnum split → special case →
camelCase split → stop word filter → stem → dedupe
```

### BM25 Formula (Identical)

```
score = Σ IDF(term) × (TF × (k1+1)) / (TF + k1 × (1-b + b×(docLen/avgdl)))

where:
  IDF(term) = ln(1 + (N - DF + 0.5) / (DF + 0.5))
  TF = term frequency in document
  DF = document frequency (num docs containing term)
  N = total number of documents
  docLen = number of tokens in document
  avgdl = average document length
```

## Performance Characteristics

| Aspect | Probe | This Implementation |
|--------|-------|---------------------|
| **Parallelism** | Rayon work-stealing | Goroutines (one per doc) |
| **SIMD** | Yes (`simsimd` for dot products) | No (Go limitation) |
| **Caching** | Multi-tier (compound, eval, session) | None |
| **Lazy Eval** | Yes (batch processing) | No (all-at-once) |
| **Regex** | Compiled patterns, ripgrep | Not used |

**Scalability:**
- Probe: Optimized for 100K+ files
- This: Suitable for 100-1000 endpoints

## Extension Opportunities

To make this more like probe:

### 1. Boolean Query Parsing
```go
type QueryExpr interface {
    Evaluate(matchedTerms map[string]bool) bool
}

type TermExpr struct {
    Keywords []string
    Required bool  // +term
    Excluded bool  // -term
}

type AndExpr struct {
    Left, Right QueryExpr
}

type OrExpr struct {
    Left, Right QueryExpr
}
```

### 2. Field-Specific Search
```go
// Support: method:GET tag:authentication path:/users
type SearchFilter struct {
    Method string
    Tag    string
    PathPattern string
}
```

### 3. Caching Layer
```go
import "github.com/hashicorp/golang-lru"

type Engine struct {
    queryCache *lru.Cache  // query → results
}
```

### 4. Batch Processing
```go
func (e *Engine) Search(query string, maxResults int) []SearchResult {
    // 1. Quick rank all endpoints
    scores := e.quickRank(query)

    // 2. Process only top N
    topN := scores[:min(100, len(scores))]

    // 3. Full analysis on top N
    return e.fullAnalysis(topN, maxResults)
}
```

### 5. SIMD Alternative
```go
// Use concurrent processing as Go's "SIMD"
func parallelDotProduct(a, b []float64) float64 {
    // Split into chunks, process in parallel
    // Aggregate results
}
```

## Lessons Learned

### What Translates Well to Go

1. **BM25 algorithm**: Direct mathematical formula, easy to port
2. **Tokenization logic**: String manipulation works similarly
3. **Parallel scoring**: Goroutines are great for this
4. **Modular architecture**: Package structure maps well

### What's Harder in Go

1. **SIMD operations**: No direct equivalent, must use concurrency
2. **Zero-copy strings**: Go always copies, Rust can use `&str`
3. **Algebraic types**: Rust enums > Go interfaces for AST
4. **Compile-time optimizations**: Rust's const fn, inline, etc.

### What's Better in Go

1. **Simpler concurrency**: Goroutines vs Rayon setup
2. **JSON/YAML parsing**: Excellent stdlib + libraries
3. **HTTP servers**: Easy to wrap this in a REST API
4. **Deployment**: Single binary, no dynamic libs

## References

- **Probe source**: `/src/search/`, `/src/ranking.rs`
- **BM25 paper**: Robertson & Zaragoza (2009)
- **Porter2 stemmer**: https://snowballstem.org/algorithms/english/stemmer.html
- **OpenAPI spec**: https://swagger.io/specification/
