# Testing Guide

Comprehensive testing documentation for the OpenAPI search engine.

## Running Tests

### Run all e2e tests

```bash
go test -v -run TestE2E
```

### Run specific test suite

```bash
go test -v -run TestE2E_BasicSearch
go test -v -run TestE2E_CamelCaseSplitting
go test -v -run TestE2E_Stemming
go test -v -run TestE2E_BM25Ranking
```

### Run with coverage

```bash
go test -cover -coverprofile=coverage.out
go tool cover -html=coverage.out
```

## Test Suites

### 1. TestE2E_BasicSearch

Tests fundamental search functionality across multiple OpenAPI specs.

**What it tests:**
- Basic keyword search
- Finding endpoints by common terms (messages, SMS, user)
- Minimum result thresholds
- Result correctness

**Example:**
```go
Query: "message"
Expected: POST /chat.postMessage, POST /chat.update, etc.
```

### 2. TestE2E_CamelCaseSplitting

Tests that camelCase and PascalCase terms are properly tokenized.

**What it tests:**
- `postMessage` → matches `POST /chat.postMessage`
- `post message` → matches same endpoint
- `PaymentIntent` → matches `/payment_intents`

**Why it matters:** API specs often use camelCase for operation IDs and descriptions. Proper splitting ensures both `getUserInfo` and `get user info` match the same endpoint.

### 3. TestE2E_Stemming

Tests that Porter2 stemming works correctly for word variants.

**What it tests:**
- `authenticate`, `authentication`, `authenticating` → all match auth endpoints
- `message`, `messages`, `messaging` → all match message endpoints
- `subscription`, `subscriptions` → both match subscription endpoints

**Why it matters:** Users may search with different word forms. Stemming normalizes these to match the same root concept.

### 4. TestE2E_BM25Ranking

Tests that BM25 algorithm correctly ranks results by relevance.

**What it tests:**
- Multi-term matches score higher than single-term
- Scores are in descending order
- Most relevant result appears first
- Score thresholds are met

**Example:**
```
Query: "refund charge"
Top result: POST /charges/{id}/refund  (score: 4.07)
  ↑ Both "refund" and "charge" matched

Lower result: GET /charges  (score: 1.35)
  ↑ Only "charge" matched
```

### 5. TestE2E_MultiTermQuery

Tests queries with multiple terms and ensures proper matching.

**What it tests:**
- Two-term queries: `user login` → `/user/login`
- Three-term queries: `create payment intent` → `/payment_intents`
- Operation + resource: `delete order` → `DELETE /store/order`
- All required terms appear in matched tokens

### 6. TestE2E_YAMLAndJSONFormats

Tests that both YAML and JSON OpenAPI specs are correctly parsed and indexed.

**What it tests:**
- YAML specs: github-api.yaml, stripe-api.yaml, petstore-api.yaml
- JSON specs: slack-api.json, twilio-api.json
- Both formats produce searchable results

**Why it matters:** OpenAPI specs can be in either format. The engine must handle both.

### 7. TestE2E_SpecificAPIs

Tests domain-specific searches across different API types.

**What it tests:**
- GitHub API: pull requests, repositories, commits
- Stripe API: charges, subscriptions, payment intents
- Slack API: messages, reactions, conversations
- Twilio API: SMS, calls, phone numbers
- Petstore API: pets, orders, users

**Example results:**
```
GitHub - "pull requests"   → GET /repos/{owner}/{repo}/pulls (score: 9.44)
Stripe - "cancel subscription" → POST /subscriptions/{id}/cancel (score: 8.51)
Slack - "add reaction emoji"   → POST /reactions.add (score: 10.72)
```

### 8. TestE2E_EdgeCases

Tests boundary conditions and unusual inputs.

**What it tests:**
- Empty query → no results
- Single character → may or may not match
- Numbers (404) → matches HTTP status codes
- Special characters (`/{id}/`) → matches path parameters
- Non-existent terms → no results
- Max results limit → respects limit

## Test Fixtures

### Location
```
fixtures/
├── github-api.yaml      # YAML - Repository management
├── stripe-api.yaml      # YAML - Payment processing
├── petstore-api.yaml    # YAML - Classic petstore example
├── slack-api.json       # JSON - Messaging API
└── twilio-api.json      # JSON - Communications API
```

### Statistics

**Total endpoints across all fixtures:** ~60

**By API:**
- GitHub: 7 endpoints (repos, issues, pull requests, commits, search)
- Stripe: 9 endpoints (charges, customers, subscriptions, payment intents)
- Petstore: 17 endpoints (pets, store, orders, users)
- Slack: 9 endpoints (chat, conversations, users, files, reactions)
- Twilio: 5 endpoints (messages, calls, phone numbers)

**By HTTP method:**
- GET: ~25 endpoints
- POST: ~20 endpoints
- PUT: ~5 endpoints
- DELETE: ~5 endpoints

### Coverage Matrix

| Feature | Fixture Coverage |
|---------|-----------------|
| Path parameters | ✓ All APIs (e.g., `/users/{userId}`) |
| Query parameters | ✓ All APIs |
| Multiple tags | ✓ GitHub, Petstore |
| Nested paths | ✓ Stripe, GitHub |
| CamelCase operations | ✓ Slack (`postMessage`) |
| Underscores | ✓ Stripe (`payment_intents`) |
| Hyphens | ✓ GitHub (`pull-requests`) |
| Descriptions | ✓ All APIs |

## Writing New Tests

### Basic Test Template

```go
func TestE2E_YourFeature(t *testing.T) {
    engine := search.NewEngine()
    if err := engine.IndexDirectory("fixtures"); err != nil {
        t.Fatalf("Failed to index fixtures: %v", err)
    }

    tests := []struct {
        name        string
        query       string
        wantResults int
        checkScore  float64
    }{
        {
            name:        "Your test case",
            query:       "test query",
            wantResults: 5,
            checkScore:  2.0,
        },
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            results := engine.Search(tt.query, 20)

            if len(results) < tt.wantResults {
                t.Errorf("Expected at least %d results, got %d",
                    tt.wantResults, len(results))
            }

            if len(results) > 0 && results[0].Score < tt.checkScore {
                t.Errorf("Top score %.2f below minimum %.2f",
                    results[0].Score, tt.checkScore)
            }
        })
    }
}
```

### Adding New Fixtures

1. **Create the spec file:**
   ```bash
   touch fixtures/your-api.yaml
   ```

2. **Add OpenAPI 3.0 content:**
   ```yaml
   openapi: 3.0.0
   info:
     title: Your API
     version: 1.0.0
   paths:
     /your/endpoint:
       get:
         summary: Your endpoint
         description: Detailed description
         operationId: yourOperation
         tags:
           - your-tag
   ```

3. **Add test case:**
   ```go
   {
       name:  "Your API test",
       query: "your specific search",
       wantEndpoints: []string{"GET /your/endpoint"},
       minResults: 1,
   }
   ```

## Expected Test Behavior

### Score Ranges

Based on current test data:

| Score Range | Meaning | Example |
|-------------|---------|---------|
| 8.0+ | Excellent match (3+ terms) | "create payment intent" → 8.97 |
| 4.0-8.0 | Good match (2+ terms) | "user login" → 4.77 |
| 1.0-4.0 | Partial match (1-2 terms) | "weather" → 1.40 |
| 0.0-1.0 | Weak match (stemmed/partial) | "get" → 0.81 |

### Ranking Behavior

**Multi-term queries favor:**
1. Endpoints matching ALL terms highest
2. Endpoints matching MOST terms next
3. Endpoints matching ANY term last

**BM25 considers:**
- Term frequency (TF) in document
- Inverse document frequency (IDF) - rarer terms score higher
- Document length normalization - shorter docs slightly favored

## Debugging Failed Tests

### Test fails with "expected endpoint not found"

```bash
# Run with verbose output
go test -v -run TestE2E_YourTest

# Check what results were returned
# Tests should log top results on failure
```

### Test fails with low score

```go
// Add logging to see matched terms
t.Logf("Matched terms: %v", results[0].Matches)
t.Logf("Score: %.2f", results[0].Score)
```

### Test fails inconsistently

- Check for floating-point comparison issues
- Ensure deterministic sorting (BM25 ranker has secondary sort by index)
- Verify fixture data hasn't changed

## Continuous Integration

### GitHub Actions Example

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-go@v4
        with:
          go-version: '1.21'
      - run: go test -v -race -coverprofile=coverage.out
      - run: go tool cover -func=coverage.out
```

## Performance Benchmarks

Run benchmarks to measure search performance:

```bash
go test -bench=. -benchmem
```

Example benchmark:

```go
func BenchmarkSearch(b *testing.B) {
    engine := search.NewEngine()
    engine.IndexDirectory("fixtures")

    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        engine.Search("user authentication", 10)
    }
}
```

## Coverage Goals

Current coverage: ~85%

**Well-covered:**
- ✓ Tokenization logic
- ✓ BM25 ranking
- ✓ Search pipeline
- ✓ Result formatting

**Could improve:**
- ⚠ Error handling edge cases
- ⚠ OpenAPI parsing edge cases
- ⚠ Very large result sets

## Common Issues

### Issue: Test passes locally, fails in CI

**Cause:** Fixture files not committed to git

**Fix:**
```bash
git add fixtures/*.yaml fixtures/*.json
git commit -m "Add test fixtures"
```

### Issue: Scores vary slightly between runs

**Cause:** Floating-point arithmetic differences

**Fix:** Use score ranges instead of exact values:
```go
if score < 2.0 || score > 3.0 {
    t.Errorf("Score out of expected range")
}
```

### Issue: New fixture not being indexed

**Cause:** File extension not .yaml or .json

**Fix:** Rename file to use correct extension

## Resources

- **Go testing package:** https://pkg.go.dev/testing
- **Table-driven tests:** https://dave.cheney.net/2019/05/07/prefer-table-driven-tests
- **BM25 algorithm:** https://en.wikipedia.org/wiki/Okapi_BM25
