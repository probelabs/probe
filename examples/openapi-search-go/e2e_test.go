package main

import (
	"openapi-search/search"
	"strings"
	"testing"
)

// BM25 score thresholds for test expectations
const (
	expectedMultiTermScore  = 2.0 // Expected minimum score when multiple query terms match
	expectedSingleTermScore = 1.0 // Expected minimum score for single term matches
	expectedGoodMatchScore  = 1.5 // Expected minimum for good quality matches
)

// TestE2E_BasicSearch tests basic search functionality
func TestE2E_BasicSearch(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	tests := []struct {
		name          string
		query         string
		wantEndpoints []string // Substring matches in endpoint paths/methods
		minResults    int
	}{
		{
			name:  "Search for messages",
			query: "message",
			wantEndpoints: []string{
				"POST /chat.postMessage",
				"POST /chat.update",
				"POST /Accounts/{AccountSid}/Messages.json",
			},
			minResults: 3,
		},
		{
			name:  "Search for SMS",
			query: "SMS",
			wantEndpoints: []string{
				"POST /Accounts/{AccountSid}/Messages.json",
			},
			minResults: 1,
		},
		{
			name:  "Search for user management",
			query: "user",
			wantEndpoints: []string{
				"GET /users.list",
				"GET /users.info",
				"POST /user",
				"GET /user/login",
			},
			minResults: 4,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results := engine.Search(tt.query, 50)

			if len(results) < tt.minResults {
				t.Errorf("Expected at least %d results, got %d", tt.minResults, len(results))
			}

			// Check that expected endpoints are in results
			for _, want := range tt.wantEndpoints {
				found := false
				for _, result := range results {
					resultStr := result.Endpoint.Method + " " + result.Endpoint.Path
					if strings.Contains(resultStr, want) {
						found = true
						break
					}
				}
				if !found {
					t.Errorf("Expected endpoint containing %q in results, but not found", want)
				}
			}
		})
	}
}

// TestE2E_CamelCaseSplitting tests that camelCase terms are properly split
func TestE2E_CamelCaseSplitting(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	tests := []struct {
		name        string
		query       string
		shouldMatch string // Endpoint that should match
	}{
		{
			name:        "CamelCase - postMessage",
			query:       "postMessage",
			shouldMatch: "POST /chat.postMessage",
		},
		{
			name:        "Split parts - post message",
			query:       "post message",
			shouldMatch: "POST /chat.postMessage",
		},
		{
			name:        "CamelCase - PaymentIntent",
			query:       "PaymentIntent",
			shouldMatch: "POST /payment_intents",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results := engine.Search(tt.query, 20)

			if len(results) == 0 {
				t.Fatalf("Expected results for query %q, got none", tt.query)
			}

			found := false
			for _, result := range results {
				resultStr := result.Endpoint.Method + " " + result.Endpoint.Path
				if strings.Contains(resultStr, tt.shouldMatch) {
					found = true
					t.Logf("Found %q with score %.2f", resultStr, result.Score)
					break
				}
			}

			if !found {
				t.Errorf("Expected to find %q in results", tt.shouldMatch)
				t.Logf("Got %d results:", len(results))
				for i, r := range results {
					if i < 5 { // Show first 5 results
						t.Logf("  %d. %s %s (score: %.2f)",
							i+1, r.Endpoint.Method, r.Endpoint.Path, r.Score)
					}
				}
			}
		})
	}
}

// TestE2E_Stemming tests that stemming works correctly
func TestE2E_Stemming(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	tests := []struct {
		name     string
		queries  []string // Different forms that should match similarly
		minScore float64  // Minimum score for top result
	}{
		{
			name:     "Authentication variants",
			queries:  []string{"authenticate", "authentication", "authenticating"},
			minScore: 1.0,
		},
		{
			name:     "Message variants",
			queries:  []string{"message", "messages", "messaging"},
			minScore: 1.0,
		},
		{
			name:     "Subscription variants",
			queries:  []string{"subscription", "subscriptions"},
			minScore: 1.0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var firstResults []search.SearchResult

			for i, query := range tt.queries {
				results := engine.Search(query, 10)
				if len(results) == 0 {
					t.Errorf("Query %q returned no results", query)
					continue
				}

				if results[0].Score < tt.minScore {
					t.Errorf("Query %q top result score %.2f below minimum %.2f",
						query, results[0].Score, tt.minScore)
				}

				// Store first query results for comparison
				if i == 0 {
					firstResults = results
				} else if len(firstResults) > 0 && len(results) > 0 {
					// Different query forms should match similar endpoints
					// (not necessarily identical due to other factors, but should overlap)
					overlap := 0
					maxCheck := min(5, min(len(firstResults), len(results)))
					for _, r1 := range firstResults[:maxCheck] {
						for _, r2 := range results[:maxCheck] {
							if r1.Endpoint.Path == r2.Endpoint.Path &&
								r1.Endpoint.Method == r2.Endpoint.Method {
								overlap++
								break
							}
						}
					}

					if overlap == 0 {
						t.Logf("Warning: No overlap in top %d results between %q and %q",
							maxCheck, tt.queries[0], query)
					}
				}
			}
		})
	}
}

// TestE2E_BM25Ranking tests that BM25 ranking prioritizes better matches
func TestE2E_BM25Ranking(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	tests := []struct {
		name          string
		query         string
		topResult     string // Expected top result substring
		checkRanking  bool   // If true, verify scores are descending
		minTopScore   float64
		maxBottomRank int // Check that low scores are ranked lower
	}{
		{
			name:         "Specific match - refund charge",
			query:        "refund charge",
			topResult:    "POST /charges/{id}/refund",
			checkRanking: true,
			minTopScore:  expectedMultiTermScore, // Multiple term match should score higher
		},
		{
			name:         "Multiple term match - create subscription",
			query:        "create subscription",
			topResult:    "POST /subscriptions",
			checkRanking: true,
			minTopScore:  expectedGoodMatchScore,
		},
		{
			name:         "Exact operation - list repositories",
			query:        "list repositories",
			topResult:    "/repos", // Any repo endpoint should match
			checkRanking: true,
			minTopScore:  expectedSingleTermScore,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results := engine.Search(tt.query, 20)

			if len(results) == 0 {
				t.Fatalf("Expected results for query %q", tt.query)
			}

			// Check top result
			topResultStr := results[0].Endpoint.Method + " " + results[0].Endpoint.Path
			if !strings.Contains(topResultStr, tt.topResult) {
				t.Errorf("Expected top result to contain %q, got %q (score: %.2f)",
					tt.topResult, topResultStr, results[0].Score)

				t.Logf("Top 5 results:")
				for i := 0; i < min(5, len(results)); i++ {
					t.Logf("  %d. %s %s (score: %.2f, matches: %v)",
						i+1,
						results[i].Endpoint.Method,
						results[i].Endpoint.Path,
						results[i].Score,
						results[i].Matches)
				}
			}

			// Check minimum score
			if results[0].Score < tt.minTopScore {
				t.Errorf("Top result score %.2f below minimum %.2f",
					results[0].Score, tt.minTopScore)
			}

			// Check that scores are descending
			if tt.checkRanking {
				for i := 1; i < len(results); i++ {
					if results[i].Score > results[i-1].Score {
						t.Errorf("Results not properly ranked: result %d (score %.2f) > result %d (score %.2f)",
							i+1, results[i].Score, i, results[i-1].Score)
					}
				}
			}
		})
	}
}

// TestE2E_MultiTermQuery tests queries with multiple terms
func TestE2E_MultiTermQuery(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	tests := []struct {
		name            string
		query           string
		mustMatchAll    []string // All these terms must appear in matched tokens
		topResultShould string   // Top result should contain this
	}{
		{
			name:            "Two terms - user login",
			query:           "user login",
			mustMatchAll:    []string{"user", "login"},
			topResultShould: "/user/login",
		},
		{
			name:            "Three terms - create payment intent",
			query:           "create payment intent",
			mustMatchAll:    []string{"payment", "intent"},
			topResultShould: "/payment_intents",
		},
		{
			name:            "Operation + resource - delete order",
			query:           "delete order",
			mustMatchAll:    []string{"delete", "order"},
			topResultShould: "DELETE",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results := engine.Search(tt.query, 20)

			if len(results) == 0 {
				t.Fatalf("Expected results for query %q", tt.query)
			}

			// Check top result contains expected substring
			topResultStr := results[0].Endpoint.Method + " " + results[0].Endpoint.Path
			if !strings.Contains(topResultStr, tt.topResultShould) {
				t.Errorf("Expected top result to contain %q, got %q",
					tt.topResultShould, topResultStr)
			}

			// Check that matched terms include required terms
			matchedTermsMap := make(map[string]bool)
			for _, match := range results[0].Matches {
				matchedTermsMap[match] = true
			}

			for _, required := range tt.mustMatchAll {
				found := false
				// Check for exact match or stemmed match
				for matched := range matchedTermsMap {
					if matched == strings.ToLower(required) ||
						strings.HasPrefix(matched, strings.ToLower(required)[:min(len(required), 4)]) {
						found = true
						break
					}
				}

				if !found {
					t.Logf("Warning: Required term %q not found in matches %v for top result",
						required, results[0].Matches)
				}
			}

			t.Logf("Top result: %s %s (score: %.2f, matches: %v)",
				results[0].Endpoint.Method,
				results[0].Endpoint.Path,
				results[0].Score,
				results[0].Matches)
		})
	}
}

// TestE2E_YAMLAndJSONFormats tests that both YAML and JSON specs are indexed
func TestE2E_YAMLAndJSONFormats(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	// Check that we have endpoints from both YAML and JSON files
	yamlTests := []string{
		"github-api.yaml",  // Should have GitHub endpoints
		"stripe-api.yaml",  // Should have Stripe endpoints
		"petstore-api.yaml", // Should have Petstore endpoints
	}

	jsonTests := []string{
		"slack-api.json",  // Should have Slack endpoints
		"twilio-api.json", // Should have Twilio endpoints
	}

	// Test YAML specs
	for _, specFile := range yamlTests {
		t.Run("YAML_"+specFile, func(t *testing.T) {
			// Search for something unique to each spec
			var query string
			switch specFile {
			case "github-api.yaml":
				query = "repository issues"
			case "stripe-api.yaml":
				query = "charge refund"
			case "petstore-api.yaml":
				query = "pet status"
			}

			results := engine.Search(query, 10)
			if len(results) == 0 {
				t.Errorf("No results found for %s, query: %q", specFile, query)
			} else {
				t.Logf("Found %d results from %s", len(results), specFile)
			}
		})
	}

	// Test JSON specs
	for _, specFile := range jsonTests {
		t.Run("JSON_"+specFile, func(t *testing.T) {
			var query string
			switch specFile {
			case "slack-api.json":
				query = "post message"
			case "twilio-api.json":
				query = "send SMS"
			}

			results := engine.Search(query, 10)
			if len(results) == 0 {
				t.Errorf("No results found for %s, query: %q", specFile, query)
			} else {
				t.Logf("Found %d results from %s", len(results), specFile)
			}
		})
	}
}

// TestE2E_SpecificAPIs tests domain-specific searches
func TestE2E_SpecificAPIs(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	tests := []struct {
		name            string
		query           string
		expectedAPI     string // Which API spec it should come from
		expectedInPath  string
		minScore        float64
	}{
		{
			name:           "GitHub - pull requests",
			query:          "pull requests",
			expectedAPI:    "GitHub",
			expectedInPath: "/pulls",
			minScore:       1.5,
		},
		{
			name:           "Stripe - subscriptions",
			query:          "cancel subscription",
			expectedAPI:    "Stripe",
			expectedInPath: "/subscriptions",
			minScore:       2.0,
		},
		{
			name:           "Slack - reactions",
			query:          "add reaction emoji",
			expectedAPI:    "Slack",
			expectedInPath: "/reactions.add",
			minScore:       1.0,
		},
		{
			name:           "Twilio - voice calls",
			query:          "make call voice",
			expectedAPI:    "Twilio",
			expectedInPath: "/Calls",
			minScore:       1.0,
		},
		{
			name:           "Petstore - find by tags",
			query:          "find pet tags",
			expectedAPI:    "Petstore",
			expectedInPath: "/pet/findByTags",
			minScore:       1.5,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results := engine.Search(tt.query, 10)

			if len(results) == 0 {
				t.Fatalf("No results for %s query: %q", tt.expectedAPI, tt.query)
			}

			topResult := results[0]
			if !strings.Contains(topResult.Endpoint.Path, tt.expectedInPath) {
				t.Errorf("Expected path to contain %q, got %q",
					tt.expectedInPath, topResult.Endpoint.Path)
			}

			if topResult.Score < tt.minScore {
				t.Errorf("Expected score >= %.2f, got %.2f",
					tt.minScore, topResult.Score)
			}

			t.Logf("%s: %s %s (score: %.2f)",
				tt.expectedAPI,
				topResult.Endpoint.Method,
				topResult.Endpoint.Path,
				topResult.Score)
		})
	}
}

// TestE2E_EdgeCases tests edge cases and boundary conditions
func TestE2E_EdgeCases(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	tests := []struct {
		name        string
		query       string
		expectEmpty bool
		maxResults  int
	}{
		{
			name:        "Empty query",
			query:       "",
			expectEmpty: true,
		},
		{
			name:        "Single character",
			query:       "a",
			expectEmpty: false, // Should match some results with 'a'
		},
		{
			name:        "Numbers",
			query:       "404",
			expectEmpty: false, // Should match HTTP status codes
		},
		{
			name:        "Special characters",
			query:       "/{id}/",
			expectEmpty: false, // Should match path parameters
		},
		{
			name:        "Very specific non-existent",
			query:       "xyzabc123nonexistent",
			expectEmpty: true,
		},
		{
			name:        "Max results limit",
			query:       "get",
			expectEmpty: false,
			maxResults:  3,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			maxRes := 50
			if tt.maxResults > 0 {
				maxRes = tt.maxResults
			}

			results := engine.Search(tt.query, maxRes)

			if tt.expectEmpty && len(results) > 0 {
				t.Errorf("Expected empty results for query %q, got %d results",
					tt.query, len(results))
			}

			if !tt.expectEmpty && len(results) == 0 {
				t.Logf("Warning: Expected results for query %q, got none", tt.query)
			}

			if tt.maxResults > 0 && len(results) > tt.maxResults {
				t.Errorf("Expected max %d results, got %d", tt.maxResults, len(results))
			}

			if len(results) > 0 {
				t.Logf("Query %q returned %d results, top score: %.2f",
					tt.query, len(results), results[0].Score)
			}
		})
	}
}

// Helper function (Go 1.21+)
func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
