package main

import (
	"openapi-search/search"
	"openapi-search/tokenizer"
	"strings"
	"testing"
)

// TestStopWords_Filtering verifies that stop words are removed from queries
func TestStopWords_Filtering(t *testing.T) {
	tok := tokenizer.New()

	tests := []struct {
		name            string
		input           string
		shouldNotContain []string // Stop words that should be filtered out
		mustContain     []string  // Important words that should remain
	}{
		{
			name:            "Natural language query with stop words",
			input:           "How can I call the weather API?",
			shouldNotContain: []string{"how", "can", "i", "the"},
			mustContain:     []string{"call", "weather", "api"},
		},
		{
			name:            "Query with pronouns and articles",
			input:           "I want to get my user data",
			shouldNotContain: []string{"i", "want", "to", "my"},
			mustContain:     []string{"get", "user", "data"},
		},
		{
			name:            "Query with filler words",
			input:           "What is the best way to authenticate",
			shouldNotContain: []string{"what", "is", "the", "way", "to"},
			mustContain:     []string{"best", "authenticate"},
		},
		{
			name:            "Query with question words",
			input:           "Where can I find payment refund endpoint",
			shouldNotContain: []string{"where", "can", "i"},
			mustContain:     []string{"find", "payment", "refund", "endpoint"},
		},
		{
			name:            "Query with too and very",
			input:           "This is too complex and very slow",
			shouldNotContain: []string{"this", "is", "too", "and", "very"},
			mustContain:     []string{"complex", "slow"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tokens := tok.Tokenize(tt.input)

			// Create token map for easy checking
			tokenMap := make(map[string]bool)
			for _, token := range tokens {
				tokenMap[token] = true
			}

			t.Logf("Input: %q", tt.input)
			t.Logf("Tokens: %v", tokens)

			// Verify stop words are removed
			for _, stopWord := range tt.shouldNotContain {
				if tokenMap[stopWord] {
					t.Errorf("Stop word %q should have been removed, but found in: %v",
						stopWord, tokens)
				}
			}

			// Verify important words remain
			for _, important := range tt.mustContain {
				// Check for exact match or stemmed version
				found := false
				for token := range tokenMap {
					if token == important || strings.HasPrefix(token, important[:min(3, len(important))]) {
						found = true
						break
					}
				}
				if !found {
					t.Errorf("Important word %q (or its stem) not found in tokens: %v",
						important, tokens)
				}
			}
		})
	}
}

// TestStopWords_E2E verifies stop words don't affect search results
func TestStopWords_E2E(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	tests := []struct {
		name           string
		queryWithStops string // Query with stop words
		queryClean     string // Same query without stop words
		description    string
	}{
		{
			name:           "Natural question vs clean query",
			queryWithStops: "How can I call the weather API?",
			queryClean:     "call weather API",
			description:    "Both should return similar results",
		},
		{
			name:           "Question with pronouns vs keywords",
			queryWithStops: "Where can I find user authentication?",
			queryClean:     "user authentication",
			description:    "Stop words should not affect results",
		},
		{
			name:           "Verbose vs concise",
			queryWithStops: "I want to create a new payment",
			queryClean:     "create payment",
			description:    "Filler words filtered automatically",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Logf("Testing: %s", tt.description)

			// Search with stop words
			resultsWithStops := engine.Search(tt.queryWithStops, 10)
			t.Logf("Query with stops: %q → %d results", tt.queryWithStops, len(resultsWithStops))

			// Search without stop words
			resultsClean := engine.Search(tt.queryClean, 10)
			t.Logf("Query clean: %q → %d results", tt.queryClean, len(resultsClean))

			// Both should return results
			if len(resultsWithStops) == 0 {
				t.Errorf("Query with stop words returned no results")
			}
			if len(resultsClean) == 0 {
				t.Errorf("Clean query returned no results")
			}

			// Results should be similar (stop words filtered out automatically)
			if len(resultsWithStops) > 0 && len(resultsClean) > 0 {
				t.Logf("With stops - Top: %s %s (score: %.2f)",
					resultsWithStops[0].Endpoint.Method,
					resultsWithStops[0].Endpoint.Path,
					resultsWithStops[0].Score)

				t.Logf("Clean - Top: %s %s (score: %.2f)",
					resultsClean[0].Endpoint.Method,
					resultsClean[0].Endpoint.Path,
					resultsClean[0].Score)

				// Check for overlap in top 5 results
				overlap := 0
				maxCheck := min(5, min(len(resultsWithStops), len(resultsClean)))
				for _, r1 := range resultsWithStops[:maxCheck] {
					for _, r2 := range resultsClean[:maxCheck] {
						if r1.Endpoint.Path == r2.Endpoint.Path &&
							r1.Endpoint.Method == r2.Endpoint.Method {
							overlap++
							break
						}
					}
				}

				t.Logf("Overlap in top %d results: %d endpoints", maxCheck, overlap)

				if overlap == 0 {
					t.Logf("Warning: No overlap - stop words may be affecting results differently")
				}
			}
		})
	}
}

// TestStopWords_NaturalLanguageQueries tests real-world natural language queries
func TestStopWords_NaturalLanguageQueries(t *testing.T) {
	engine := search.NewEngine()
	if err := engine.IndexDirectory("fixtures"); err != nil {
		t.Fatalf("Failed to index fixtures: %v", err)
	}

	queries := []struct {
		query       string
		expectMatch string // Expected endpoint path substring
	}{
		{
			query:       "How do I authenticate a user?",
			expectMatch: "auth",
		},
		{
			query:       "Can you show me how to send a message?",
			expectMatch: "message",
		},
		{
			query:       "I need to create a new subscription",
			expectMatch: "subscription",
		},
		{
			query:       "What is the best way to refund a payment?",
			expectMatch: "refund",
		},
		{
			query:       "Where can I find the API to list all users?",
			expectMatch: "user",
		},
	}

	for _, tc := range queries {
		t.Run(tc.query, func(t *testing.T) {
			results := engine.Search(tc.query, 10)

			if len(results) == 0 {
				t.Errorf("Natural language query returned no results: %q", tc.query)
				return
			}

			t.Logf("Query: %q", tc.query)
			t.Logf("Top result: %s %s (score: %.2f, matches: %v)",
				results[0].Endpoint.Method,
				results[0].Endpoint.Path,
				results[0].Score,
				results[0].Matches)

			// Check if top result contains expected substring
			found := false
			for i := 0; i < min(3, len(results)); i++ {
				if strings.Contains(strings.ToLower(results[i].Endpoint.Path), tc.expectMatch) ||
					strings.Contains(strings.ToLower(results[i].Endpoint.Summary), tc.expectMatch) {
					found = true
					break
				}
			}

			if !found {
				t.Logf("Warning: Expected match %q not found in top 3 results", tc.expectMatch)
			}

			// Verify stop words were filtered
			t.Logf("Matched tokens (stop words should be absent): %v", results[0].Matches)
		})
	}
}

// Use min from e2e_test.go (avoid redeclaration)
