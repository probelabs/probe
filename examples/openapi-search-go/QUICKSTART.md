# Quick Start Guide

Get started with the OpenAPI search engine in 5 minutes.

## Installation

```bash
cd examples/openapi-search-go
go mod download
```

## Basic Usage

### 1. Search the example specs

```bash
go run main.go "weather API"
```

Expected output:
```
Searching for: "weather API"
================================================================================

1. [Score: 1.40] GET /alerts [weather, alerts]
   Description: Returns active weather alerts for a location
   Matched terms: weather

2. [Score: 1.37] GET /weather/forecast [weather, forecast]
   Description: Returns weather forecast for the next 7 days
   Matched terms: weather
   ...
```

### 2. Try different queries

```bash
# Authentication-related endpoints
go run main.go "JWT authentication"

# Payment operations
go run main.go "refund payment"

# User management
go run main.go "create user"

# Search with limit
go run main.go -max 3 "weather"
```

### 3. Add your own OpenAPI specs

```bash
# Add your spec files to the specs/ directory
cp /path/to/your/api.yaml specs/

# Run the search
go run main.go "your search query"
```

## How It Works

### The Search Process

1. **Query Tokenization**
   ```
   "weather API" → ["weather", "api", "weath"]
                    (original + stemmed)
   ```

2. **Document Tokenization**
   - Each endpoint is tokenized
   - Includes: path, method, summary, description, parameters
   - Example: `GET /weather/current` → ["get", "weather", "current", "weath", ...]

3. **BM25 Ranking**
   - Compares query tokens with document tokens
   - Calculates relevance score
   - Higher score = better match

4. **Results**
   - Sorted by score (highest first)
   - Shows matched terms
   - Includes parameter details

### Understanding Scores

- **High score (>3.0)**: Multiple query terms matched
- **Medium score (1.0-3.0)**: One or two terms matched
- **Low score (<1.0)**: Partial or stemmed match

Example:
```
Query: "user login"

POST /auth/login        Score: 3.55  ← Both "user" and "login" matched
POST /users             Score: 1.00  ← Only "user" matched
GET /payments           Score: 0.00  ← No match (filtered out)
```

## Advanced Features

### CamelCase Splitting

The tokenizer automatically splits camelCase and PascalCase:

```
JWTAuthentication → ["jwt", "authentication"]
getUserById → ["get", "user", "by", "id"]
APIClient → ["api", "client"]
```

Try it:
```bash
go run main.go "getUserById"  # Matches endpoints with "get" and "user"
```

### Stemming

Query and document tokens are stemmed for better matching:

```
"authentication" → "authent"
"authenticate" → "authent"
"authenticating" → "authent"
```

All these variations will match:
```bash
go run main.go "authentication"
go run main.go "authenticate"
go run main.go "authenticating"
```

## Command-Line Options

```bash
go run main.go [options] "query"

Options:
  -specs string
        Directory containing OpenAPI specs (default "specs")
  -query string
        Search query
  -max int
        Maximum number of results (default 10)

Examples:
  go run main.go "search query"
  go run main.go -max 5 "search query"
  go run main.go -specs ./my-specs -query "search query"
```

## Build and Install

### Build executable

```bash
go build -o openapi-search
```

### Run the binary

```bash
./openapi-search "weather API"
```

### Install globally

```bash
go install
openapi-search "weather API"
```

## Run the Demo

See all features in action:

```bash
./demo.sh
```

This will run multiple example searches demonstrating different features.

## Troubleshooting

### No results found

- Check that spec files are in the `specs/` directory
- Verify specs are valid YAML/JSON
- Try simpler queries (e.g., "user" instead of "user management")

### Error parsing specs

```
Warning: failed to index specs/my-api.yaml: ...
```

- Check YAML/JSON syntax
- Ensure it's OpenAPI 3.0 format
- Check that `paths` section exists

### Too many/few results

```bash
# Limit results
go run main.go -max 5 "query"

# Show all results (use large number)
go run main.go -max 100 "query"
```

## Next Steps

1. **Read the architecture**: See `ARCHITECTURE.md` for implementation details
2. **Learn about probe**: See `PROBE_RESEARCH.md` for probe's search architecture
3. **Extend the code**: Add boolean queries, field-specific search, caching
4. **Build an API**: Wrap the search engine in a REST API

## Example: Building a REST API

```go
package main

import (
    "encoding/json"
    "net/http"
    "openapi-search/search"
)

func main() {
    engine := search.NewEngine()
    engine.IndexDirectory("specs")

    http.HandleFunc("/search", func(w http.ResponseWriter, r *http.Request) {
        query := r.URL.Query().Get("q")
        results := engine.Search(query, 10)
        json.NewEncoder(w).Encode(results)
    })

    http.ListenAndServe(":8080", nil)
}
```

Run it:
```bash
go run server.go
curl "http://localhost:8080/search?q=weather+API"
```

## Resources

- **Probe**: https://probe.rs
- **OpenAPI Spec**: https://swagger.io/specification/
- **BM25 Algorithm**: https://en.wikipedia.org/wiki/Okapi_BM25
- **Porter Stemmer**: https://snowballstem.org/
