# OpenAPI Search Engine

A semantic search engine for OpenAPI specifications, inspired by [probe](https://github.com/probelabs/probe)'s architecture. This implementation demonstrates how to build a search system with **tokenization**, **stemming**, and **BM25 ranking** in Go.

## Architecture Overview

This search engine is based on probe's core search components:

### 1. **Tokenizer** (`tokenizer/tokenizer.go`)
- Splits text on whitespace and non-alphanumeric characters
- **CamelCase splitting**: `JWTAuthentication` → `["jwt", "authentication"]`
- **Stemming**: Uses Porter2 stemmer via `github.com/kljensen/snowball`
- **Stop word removal**: Filters ~120 common words ("how", "can", "i", "the", "a", etc.)
- **Natural language support**: Handles full questions like "How do I authenticate a user?"
- Based on probe's `src/search/tokenization.rs`

### 2. **BM25 Ranker** (`ranker/bm25.go`)
- Implements BM25 (Best Matching 25) ranking algorithm
- **Formula**: `IDF(term) × (TF × (k1+1)) / (TF + k1 × (1-b + b×(docLen/avgdl)))`
- **Parameters**:
  - `k1 = 1.5` (term frequency saturation)
  - `b = 0.5` (document length normalization)
- **Parallel scoring**: Uses goroutines for document scoring
- Based on probe's `src/ranking.rs`

### 3. **OpenAPI Parser** (`search/openapi.go`)
- Loads OpenAPI 3.0 specs from YAML or JSON
- Extracts endpoints with metadata (path, method, description, parameters)
- Creates searchable text from all endpoint fields

### 4. **Search Engine** (`search/engine.go`)
- Indexes OpenAPI specs and extracts endpoints
- Tokenizes queries and documents
- Ranks results using BM25
- Returns top-k results with scores and matched terms

## How It Works

### Search Pipeline

```
User Query → Tokenize → BM25 Ranking → Sorted Results
                ↓
        [weather, api]
                ↓
    Compare with indexed endpoints
                ↓
        Calculate relevance scores
                ↓
    Return top matches with context
```

### Example: "How I can call weather API?"

1. **Query tokenization** (same process for both query and indexed data):
   ```
   "How I can call weather API?"
   → ["call", "weather", "api", "weath"]  // includes stemmed forms
   ```

2. **Document tokenization** (OpenAPI endpoint description):
   ```
   "Returns current weather conditions for a specified location"
   → ["returns", "return", "current", "weather", "weath", "conditions", ...]
                                      ^^^^^^  ^^^^^
                                      Matches via both original and stemmed!
   ```

3. **BM25 matching**:
   - Compares query tokens with document tokens
   - Both "weather" (exact match) and "weath" (stemmed match) contribute to score
   - Calculates relevance based on:
     - Term frequency (TF) in document
     - Inverse document frequency (IDF)
     - Document length normalization

4. **Ranking**:
   ```
   GET /weather/current       [Score: 8.45]  ← Best match (both terms matched)
   GET /weather/forecast      [Score: 7.32]  ← Good match (weather matched)
   POST /payments            [Score: 0.00]  ← No match (filtered out)
   ```

**Key insight:** Both query and data go through identical tokenization (including stemming), so different word forms match:
- "authenticate" matches "authentication" (both stem to "authent")
- "message" matches "messages" (both stem to "messag")
- "create" matches "creating" (both stem to "creat")

## Installation

```bash
cd examples/openapi-search-go
go mod download
```

## Testing

Comprehensive e2e tests are included to verify all functionality:

```bash
# Run all tests
go test -v

# Run specific test suite
go test -v -run TestE2E_BasicSearch
go test -v -run TestE2E_CamelCaseSplitting
go test -v -run TestE2E_Stemming

# Run with coverage
go test -cover
```

**Test coverage:**
- ✓ Basic search functionality
- ✓ CamelCase tokenization (`postMessage` → `post`, `message`)
- ✓ Stemming (`authentication`, `authenticate`, `authenticating`)
- ✓ BM25 ranking correctness
- ✓ Multi-term queries
- ✓ YAML and JSON spec parsing
- ✓ Edge cases and boundary conditions

See [TEST_GUIDE.md](TEST_GUIDE.md) for detailed testing documentation.

**Test fixtures:** 5 real-world API specs (GitHub, Stripe, Petstore, Slack, Twilio) with ~60 total endpoints in `fixtures/` directory.

## Usage

### Run the example

```bash
# Search for weather-related endpoints
go run main.go "weather API"

# Search for authentication endpoints
go run main.go "JWT token authentication"

# Search for payment refunds
go run main.go "refund payment"

# Specify custom specs directory
go run main.go -specs ./my-specs -query "user login"

# Limit results
go run main.go -max 5 "create user"
```

### Example Output

```
$ go run main.go "weather forecast"

Indexing OpenAPI specs from: specs
Indexed specs: 3
Total endpoints: 14

Endpoints by method:
  GET: 8
  POST: 5
  PUT: 1
  DELETE: 1

Searching for: "weather forecast"
================================================================================

1. [Score: 12.34] GET /weather/forecast [weather, forecast]
   Returns weather forecast for the next 7 days
   Matched terms: weather, forecast, weath
   Parameters:
     - city (query) (required): City name
     - days (query): Number of days (1-7)

2. [Score: 8.45] GET /weather/current [weather]
   Returns current weather conditions for a specified location
   Matched terms: weather, weath
   Parameters:
     - city (query) (required): City name (e.g., "London", "New York")
     - units (query): Temperature units (metric or imperial)

================================================================================
Found 2 results
```

## Key Algorithms

### Tokenization Algorithm

```go
Input: "handleJWTAuthentication"
│
├─> Split whitespace
├─> Split non-alphanumeric
├─> Split camelCase → ["handle", "JWT", "Authentication"]
│   └─> Lowercase: ["handle", "jwt", "authentication"]
├─> Remove stop words
├─> Stem → ["handl", "jwt", "authent"]
└─> Deduplicate → ["handl", "jwt", "authent", "authentication"]
```

### BM25 Scoring

```go
For each document:
  1. Tokenize document → TF map
  2. For each query term in document:
     a. Get term frequency (TF)
     b. Compute TF component: (TF × (k1+1)) / (TF + k1 × docLenNorm)
     c. Get IDF: ln(1 + (N - DF + 0.5) / (DF + 0.5))
     d. Score += IDF × TF_component
  3. Return final score
```

## Probe Architecture Reference

This implementation is based on the following probe components:

| Component | Probe Source | This Implementation |
|-----------|--------------|---------------------|
| Tokenization | `src/search/tokenization.rs:2698-2820` | `tokenizer/tokenizer.go` |
| CamelCase Splitting | `src/search/tokenization.rs:1908-2051` | `tokenizer.splitCamelCase()` |
| BM25 Ranking | `src/ranking.rs:184-428` | `ranker/bm25.go` |
| Search Pipeline | `src/search/search_runner.rs:225-1598` | `search/engine.go` |
| Query Parsing | `src/search/elastic_query.rs` | (Simplified - no boolean queries) |

### Key Differences from Probe

1. **No AST parsing**: OpenAPI specs are structured JSON/YAML, not code
2. **Simpler query parsing**: No Elasticsearch-style boolean queries (yet)
3. **No SIMD**: Go doesn't have low-level SIMD - uses goroutines instead
4. **Smaller scope**: Focused on OpenAPI specs, not general code search

### Potential Extensions

To make this more like probe, you could add:

1. **Boolean query parsing** (`AND`, `OR`, `+required`, `-excluded`)
2. **Field-specific search** (`method:GET`, `tag:authentication`)
3. **Caching** (LRU cache for query results)
4. **Batch processing** (process top-ranked specs first)
5. **BERT reranking** (neural semantic similarity)
6. **Compound word splitting** (using dictionary-based decomposition)

## Dependencies

- `github.com/kljensen/snowball` - Porter2 stemmer for English
- `gopkg.in/yaml.v3` - YAML parsing for OpenAPI specs

## Learn More

- **Probe documentation**: https://probe.rs
- **BM25 algorithm**: https://en.wikipedia.org/wiki/Okapi_BM25
- **Porter2 stemmer**: https://snowballstem.org/algorithms/english/stemmer.html
- **OpenAPI specification**: https://swagger.io/specification/

## License

This example code is provided for educational purposes to demonstrate probe's search architecture in Go.
