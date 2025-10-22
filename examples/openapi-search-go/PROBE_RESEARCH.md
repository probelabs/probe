# Probe Search Architecture Research

Comprehensive research on how probe's search system works.

## Quick Summary

Probe uses a sophisticated multi-stage search pipeline:

1. **Query parsing**: Elasticsearch-style boolean queries (`AND`, `OR`, `+required`, `-excluded`)
2. **Tokenization**: CamelCase splitting, compound word decomposition, Porter2 stemming
3. **Pattern generation**: Convert query to regex patterns for ripgrep
4. **File searching**: SIMD-accelerated pattern matching
5. **Early filtering**: AST-based boolean query evaluation per file
6. **Early ranking**: BM25 scoring to prioritize files
7. **Batch processing**: Process top-ranked files incrementally
8. **Full extraction**: Parse AST, extract code blocks
9. **Final ranking**: BM25 with optional BERT reranking
10. **Caching**: Multi-tier caching (compound words, AST eval, session results)

## Core Components

### 1. Tokenization (`src/search/tokenization.rs`)

**Location**: Lines 2698-2820

**Flow**:
```
Input: "handleJWTAuthentication"
  ↓
Whitespace split: ["handleJWTAuthentication"]
  ↓
Non-alphanumeric split: ["handleJWTAuthentication"]
  ↓
CamelCase split: ["handle", "JWT", "Authentication"]
  ↓
Lowercase: ["handle", "jwt", "authentication"]
  ↓
Compound split: (if applicable)
  ↓
Stop word filter: ["handle", "jwt", "authentication"] (all pass)
  ↓
Stemming: ["handl", "jwt", "authent"]
  ↓
Add original: ["handl", "jwt", "authent", "authentication"]
  ↓
Dedupe: ["handl", "jwt", "authent", "authentication"]
```

**Key Functions**:

- `tokenize(text)` - Main entry point (line 2698)
- `split_camel_case(s)` - CamelCase/PascalCase splitter (lines 1908-2051)
  - Handles: `APIClient` → `["API", "Client"]`
  - Handles: `parseJSON` → `["parse", "JSON"]`
  - Handles: `OAuth2` → `["OAuth", "2"]`
- `split_compound_word(s)` - Dictionary-based decomposition (lines 2087-2149)
  - Uses decompound library + vocabulary validation
  - 3-tier cache: precomputed, runtime LRU (1000), library
  - Example: `database` → `["data", "base"]`
- `is_stop_word(s)` - English + programming stop words
- `get_stemmer()` - Porter2 stemmer singleton (in `ranking.rs:37-40`)

**Special Cases**:
- `oauth2` → `["oauth", "2"]`
- `jwt` → `["jwt"]` (no stemming)
- `html5` → `["html", "5"]`
- `openapi` → `["openapi", "open", "api"]`

### 2. BM25 Ranking (`src/ranking.rs`)

**Location**: Lines 184-428

**Formula**:
```
BM25(D, Q) = Σ(term ∈ Q) IDF(term) × TF_component(term, D)

where:
  IDF(term) = ln(1 + (N - DF(term) + 0.5) / (DF(term) + 0.5))

  TF_component = (TF × (k1 + 1)) / (TF + k1 × doc_length_norm)

  doc_length_norm = 1 - b + b × (doc_length / avg_doc_length)
```

**Parameters**:
- `k1 = 1.5` (term frequency saturation) - Higher than standard 1.2
- `b = 0.5` (length normalization) - Lower than standard 0.75
- Lower `b` reduces penalty for longer documents (better for code)

**Key Functions**:

- `rank_documents(docs, query, query_ast)` - Main ranking function (lines 279-428)
  1. Parse query into terms
  2. Create token map (`HashMap<String, u8>`) for efficient indexing
  3. Compute TF per document: `Vec<HashMap<u8, usize>>`
  4. Compute DF per term: `HashMap<String, usize>`
  5. Calculate average doc length
  6. Precompute IDF for all query terms
  7. Score documents in parallel using Rayon
  8. Sort by score (descending), then index (ascending) for determinism

- `bm25_single_token_optimized(token, params)` - Score one term (lines 184-208)
  - Uses precomputed IDF values
  - Uses u8 term indices (max 256 unique terms)
  - Optimized for repeated calls

- `score_expr_bm25_optimized(expr, params)` - Boolean query eval (lines 226-274)
  - Recursively evaluates AST
  - Returns `Option<f64>`: `None` = excluded, `Some(score)` = match
  - Handles: Term (required/excluded/optional), AND, OR

**Boolean Query Logic**:
```rust
Term(required=true):
  All keywords present? Some(score) : None

Term(excluded=true):
  Any keyword present? None : Some(0.0)

Term(optional):
  has_required_elsewhere? Some(score_if_match) : All_present? Some(score) : None

AND(left, right):
  left? && right? : Some(left_score + right_score) : None

OR(left, right):
  left? || right? : Some(sum_of_matched) : None
```

### 3. SIMD Ranking (`src/simd_ranking.rs`)

**Location**: Lines 1-313

**Purpose**: Accelerate BM25 for large document sets using SIMD vector operations

**Data Structures**:

- `SparseVector` (lines 7-172)
  - `indices: Vec<u8>` - Sorted term indices
  - `values: Vec<f32>` - Corresponding frequencies/weights
  - Methods: `dot_product()`, `intersect_with_values()`

- `SparseDocumentMatrix` (lines 182-313)
  - Precomputed sparse vectors for all docs
  - Query sparse vector
  - IDF values indexed by u8
  - BM25 parameters

**Key Operations**:

- `dot_product(&self, other)` (lines 68-91)
  - Uses `simsimd` crate for SIMD acceleration
  - Two-pointer intersection for sparse vectors
  - Falls back to manual computation if SIMD unavailable

- `compute_bm25_score(doc_idx)` (lines 238-288)
  1. Find intersecting terms (query ∩ doc)
  2. Apply BM25 TF normalization
  3. Element-wise multiply with IDF (SIMD)
  4. Dot product with query weights (SIMD)

**Performance**: ~2-3x faster than scalar BM25 for 100+ documents

### 4. Query Parsing (`src/search/elastic_query.rs`)

**Location**: Lines 17-428

**AST Structure**:
```rust
pub enum Expr {
    Term {
        keywords: Vec<String>,           // Original terms
        lowercase_keywords: Vec<String>, // Pre-lowercase
        field: Option<String>,           // field:value
        required: bool,                  // +term
        excluded: bool,                  // -term
        exact: bool,                     // "phrase"
    },
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}
```

**Syntax Examples**:
```
error AND handler           → And(Term("error"), Term("handler"))
+required optional          → And(Term("required", req=true), Term("optional"))
-excluded included          → And(Term("included"), Term("excluded", excl=true))
(error OR warn) AND log     → And(Or(Term("error"), Term("warn")), Term("log"))
field:value                 → Term("value", field="field")
"exact phrase"              → Term("exact phrase", exact=true)
```

**Key Functions**:

- `parse_query(query_str)` - Main parser (lines 43-148)
  - Tokenizes query string
  - Builds AST recursively
  - Handles operator precedence: `+/-` > `AND` > `OR`

- `evaluate_with_has_required(expr, matched_terms)` (lines 150-297)
  - Evaluates AST against set of matched terms
  - Returns `true` if document satisfies query
  - Key insight: Check required terms FIRST (global constraint)

- `evaluate_with_cache(expr, matched_terms)` (lines 320-365)
  - LRU cache wrapper (1000 entries)
  - Key = hash of matched term set
  - Bypasses full AST traversal for repeated patterns

**Optimization Strategies**:
1. **Fast path**: Single-term queries, empty queries
2. **Required term pre-check**: Fail fast if missing
3. **Caching**: Avoid re-evaluating same matched term sets

### 5. Search Pipeline (`src/search/search_runner.rs`)

**Location**: Lines 362-1598 (function `perform_probe`)

**Full Pipeline**:

```
1. Query Preprocessing (lines 362-412)
   Parse query → Extract filters → Create QueryPlan

2. Pattern Generation (lines 422-446)
   QueryPlan → Regex patterns (combined + individual)

3. File Searching (lines 448-505)
   SIMD/ripgrep → HashMap<PathBuf, HashMap<term_idx, HashSet<line_num>>>

4. Filename Matching (lines 510-666)
   If enabled: Search file paths for terms

5. Early AST Filtering (lines 674-721)
   Evaluate AST per file → Filter non-matching files

6. Early Caching Check (lines 781-833)
   If session: Skip previously cached results

7. Early Ranking (lines 835-889)
   BM25 rank all matched files (before parsing)

8. Batch Processing (lines 892-1231)
   Process top-ranked files in batches of 100:
   - Read file content
   - Parse AST (tree-sitter)
   - Extract code blocks
   - Stop when estimated files needed reached

9. Result Ranking (lines 1342-1399)
   Full BM25 ranking (+ optional BERT reranking)

10. Limit Application (lines 1405-1438)
    Apply max_results, max_bytes, max_tokens

11. Final Caching & Merging (lines 1441-1577)
    Cache results, merge adjacent blocks
```

**Key Optimizations**:

1. **Early filtering**: AST evaluation before file processing
2. **Early ranking**: Sort files by relevance before parsing
3. **Batch processing**: Process incrementally, stop early
4. **Session caching**: Skip previously seen results
5. **Parallel file processing**: Rayon for concurrent parsing

### 6. Pattern Generation (`src/search/query.rs`)

**Location**: Lines 394-738 (function `create_structured_patterns`)

**Strategy**:

1. **Combined Pattern** (lines 419-433)
   - Single regex: `(?i)(term1|term2|...|termN)`
   - Matches if ANY term present
   - Most efficient for small term sets

2. **Individual Patterns** (lines 439-544)
   - One pattern per term
   - Tokenizes each term
   - Creates pattern per token
   - Maps pattern → term indices

3. **Compound Patterns** (lines 546-625)
   - For camelCase parts
   - For compound word parts
   - Only if part ≥ 3 chars

4. **Deduplication** (lines 631-696)
   - Group by matched term indices
   - Keep 2 most specific patterns per group
   - Sort by length (longer first)

5. **Limit** (lines 711-725)
   - Cap at 5000 patterns
   - Prevents regex explosion

**Example**:
```
Query: "JWTAuthentication"
Patterns:
  (?i)(jwtauthentication)          [matches term 0]
  (?i)(jwt)                        [matches term 0]
  (?i)(authentication)             [matches term 0]
  (?i)(authent)                    [matches term 0] (stemmed)
```

### 7. File Searching (`src/search/file_search.rs`)

**Two Strategies**:

1. **SIMD Pattern Matching** (for simple patterns)
   - Uses `memchr` crate
   - Fastest for literal string matching
   - Limited to simple patterns

2. **Ripgrep** (for complex patterns)
   - Compiled regex patterns
   - Multi-pattern matching
   - Respects gitignore rules
   - Returns: `HashMap<PathBuf, HashMap<term_idx, HashSet<line_num>>>`

**Output Structure**:
```rust
HashMap<PathBuf, HashMap<usize, HashSet<usize>>>
     file path → term index → line numbers
```

Example:
```rust
{
  "src/main.rs": {
    0: {10, 25, 42},  // term 0 on lines 10, 25, 42
    1: {10, 30}       // term 1 on lines 10, 30
  }
}
```

## Performance Characteristics

### Time Complexity

- **Tokenization**: O(n × k) where n = chars, k = avg camelCase splits
- **BM25 scoring**: O(d × t) where d = docs, t = query terms
- **AST evaluation**: O(t) per document (cached)
- **File search**: O(f × l) where f = files, l = avg lines
- **Early ranking**: O(d log d) for sorting

### Space Complexity

- **Token indices**: O(t) where t ≤ 256 (u8 limit)
- **TF maps**: O(d × u) where d = docs, u = unique terms
- **IDF map**: O(t) for query terms
- **Sparse vectors**: O(d × u) for SIMD ranking
- **Caches**: O(1000) for LRU caches

### Optimizations Applied

1. **u8 term indices**: Max 256 unique terms, reduces memory
2. **Sparse vectors**: Only store non-zero values
3. **SIMD operations**: 2-3x faster vector math
4. **Rayon parallelism**: Utilize all CPU cores
5. **LRU caching**: Compound words, AST eval, query results
6. **Early termination**: Batch processing stops early
7. **Lazy evaluation**: Parse only matched files
8. **Pre-computation**: IDF, lowercase, stem once

## Key Insights for Porting to Go

### What You Need

1. **Tokenizer**:
   - CamelCase splitter (important!)
   - Porter2 stemmer (`github.com/kljensen/snowball`)
   - Stop word filter
   - Compound word splitter (optional)

2. **BM25 Ranker**:
   - TF-IDF computation
   - Document length normalization
   - Parallel scoring (goroutines)
   - Boolean query support (optional but powerful)

3. **Query Parser** (optional but recommended):
   - AST structure (Term, And, Or)
   - Operator parsing (+, -, AND, OR)
   - Evaluation logic

4. **Caching** (for performance):
   - LRU cache for query results
   - Pre-computed stemming/compound splits

### What You Can Skip

1. **SIMD operations**: Go doesn't have good SIMD support, use concurrency instead
2. **Tree-sitter AST parsing**: Not needed for OpenAPI specs
3. **Complex pattern generation**: Direct text search sufficient
4. **Batch processing**: Simpler to rank all at once for <10K docs
5. **Session caching**: Unless building interactive tool

### Go Equivalents

| Probe (Rust) | Go Equivalent |
|--------------|---------------|
| Rayon parallel iterator | Goroutines + sync.WaitGroup |
| `HashMap<K, V>` | `map[K]V` |
| `Vec<T>` | `[]T` |
| `Option<T>` | Pointer or sentinel value |
| rust-stemmers | github.com/kljensen/snowball |
| tree-sitter | gopkg.in/yaml.v3 (for OpenAPI) |
| simsimd SIMD | Use concurrent processing |
| LRU cache | github.com/hashicorp/golang-lru |

### Recommended Go Architecture

```
package main
├── tokenizer/
│   └── tokenizer.go       // CamelCase, stemming, stop words
├── ranker/
│   └── bm25.go            // BM25 implementation
├── query/
│   └── parser.go          // Boolean query AST (optional)
├── search/
│   ├── engine.go          // Main search engine
│   └── openapi.go         // OpenAPI-specific logic
└── main.go                // CLI interface
```

## References

### Probe Source Files

- `src/search/tokenization.rs` - Tokenization logic
- `src/ranking.rs` - BM25 ranking
- `src/simd_ranking.rs` - SIMD-optimized BM25
- `src/search/elastic_query.rs` - Query parsing
- `src/search/query.rs` - Query plan creation
- `src/search/search_runner.rs` - Main search pipeline
- `src/search/file_search.rs` - File searching

### Academic Papers

- Robertson & Zaragoza (2009) - "The Probabilistic Relevance Framework: BM25 and Beyond"
- Porter (2001) - "Snowball: A language for stemming algorithms"

### Libraries Used

- `rust-stemmers` - Porter2 stemmer
- `decompound` - Compound word splitting
- `tree-sitter` - AST parsing
- `ripgrep` - Fast file searching
- `simsimd` - SIMD vector operations
- `rayon` - Data parallelism
