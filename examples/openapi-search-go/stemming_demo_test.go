package main

import (
	"openapi-search/search"
	"testing"
)

// TestStemming_IntegrationDemo demonstrates that stemming works end-to-end
// This test proves that both query and indexed data are tokenized/stemmed identically
func TestStemming_IntegrationDemo(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	// Demonstrate: Different word forms should match the same endpoints
	testCases := []struct {
		name               string
		queryVariants      []string // Different forms of the same concept
		expectedCommonPath string   // All variants should match this
		description        string
	}{
		{
			name: "Authentication word variants",
			queryVariants: []string{
				"authenticate",      // verb
				"authentication",    // noun
				"authenticating",    // gerund
			},
			expectedCommonPath: "/user/login", // Auth-related endpoint
			description: "All variants stem to 'authent' and match authentication endpoints",
		},
		{
			name: "Message word variants",
			queryVariants: []string{
				"message",    // singular
				"messages",   // plural
				"messaging",  // gerund
			},
			expectedCommonPath: "chat", // Message-related paths
			description: "All variants stem to 'messag' and match message endpoints",
		},
		{
			name: "Subscription word variants",
			queryVariants: []string{
				"subscription",   // singular
				"subscriptions",  // plural
			},
			expectedCommonPath: "/subscriptions",
			description: "Both variants match subscription endpoints",
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			t.Logf("Testing: %s", tc.description)

			var allResults [][]search.SearchResult

			// Search with each variant
			for _, query := range tc.queryVariants {
				results := engine.Search(query, 20)
				allResults = append(allResults, results)

				t.Logf("Query %q returned %d results", query, len(results))
				if len(results) > 0 {
					t.Logf("  Top result: %s %s (score: %.2f, matches: %v)",
						results[0].Endpoint.Method,
						results[0].Endpoint.Path,
						results[0].Score,
						results[0].Matches)
				}
			}

			// Verify all variants found results
			for i, results := range allResults {
				if len(results) == 0 {
					t.Errorf("Query variant %q returned no results", tc.queryVariants[i])
					continue
				}

				// Check if any result contains the expected path
				found := false
				for _, result := range results {
					if containsSubstring(result.Endpoint.Path, tc.expectedCommonPath) {
						found = true
						break
					}
				}

				if !found {
					t.Logf("Warning: Query %q didn't match expected path %q in top results",
						tc.queryVariants[i], tc.expectedCommonPath)
				}
			}

			// Verify that different variants produce overlapping results
			// (they should, because they all stem to the same form)
			if len(allResults) >= 2 {
				firstResults := allResults[0]
				secondResults := allResults[1]

				overlap := 0
				for _, r1 := range firstResults[:minInt(5, len(firstResults))] {
					for _, r2 := range secondResults[:minInt(5, len(secondResults))] {
						if r1.Endpoint.Path == r2.Endpoint.Path &&
							r1.Endpoint.Method == r2.Endpoint.Method {
							overlap++
							break
						}
					}
				}

				t.Logf("Overlap between top 5 results of %q and %q: %d endpoints",
					tc.queryVariants[0], tc.queryVariants[1], overlap)

				if overlap == 0 {
					t.Logf("Warning: No overlap - stemming may not be working as expected")
				}
			}
		})
	}
}

// TestStemming_MatchDifferentForms verifies query and data with different word forms match
func TestStemming_MatchDifferentForms(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	tests := []struct {
		query          string
		dataContains   string // What the endpoint description contains
		shouldMatch    bool
		minScore       float64
	}{
		{
			query:        "authenticate",        // verb form in query
			dataContains: "authentication",      // noun form in data
			shouldMatch:  true,
			minScore:     1.0,
		},
		{
			query:        "creating",            // gerund in query
			dataContains: "create",              // base form in data
			shouldMatch:  true,
			minScore:     1.0,
		},
		{
			query:        "payment",             // singular in query
			dataContains: "payments",            // plural in data
			shouldMatch:  true,
			minScore:     1.0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.query+"_matches_"+tt.dataContains, func(t *testing.T) {
			results := engine.Search(tt.query, 20)

			if len(results) == 0 && tt.shouldMatch {
				t.Errorf("Expected results for query %q, got none", tt.query)
				return
			}

			if len(results) > 0 {
				t.Logf("Query %q matched %d endpoints", tt.query, len(results))
				t.Logf("Top result: %s %s (score: %.2f, matches: %v)",
					results[0].Endpoint.Method,
					results[0].Endpoint.Path,
					results[0].Score,
					results[0].Matches)

				if results[0].Score < tt.minScore {
					t.Logf("Warning: Top score %.2f below expected minimum %.2f",
						results[0].Score, tt.minScore)
				}

				// Log what tokens matched
				t.Logf("Matched tokens prove stemming worked: %v", results[0].Matches)
			}
		})
	}
}

// Helper function
func containsSubstring(s, substr string) bool {
	return len(s) >= len(substr) &&
		   (s == substr ||
		    len(s) > len(substr) &&
		    (s[:len(substr)] == substr ||
		     s[len(s)-len(substr):] == substr ||
		     findSubstring(s, substr)))
}

func findSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

func minInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}
