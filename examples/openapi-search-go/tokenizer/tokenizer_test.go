package tokenizer

import (
	"testing"
)

// TestTokenize_Stemming verifies that both original and stemmed forms are included
func TestTokenize_Stemming(t *testing.T) {
	tok := New()

	tests := []struct {
		name          string
		input         string
		mustContain   []string // Must contain these tokens
		shouldContain []string // Should contain (stemmed variants)
	}{
		{
			name:          "Authentication variants",
			input:         "authentication",
			mustContain:   []string{"authentication"},
			shouldContain: []string{"authent"}, // stemmed form
		},
		{
			name:          "Message variants",
			input:         "messages",
			mustContain:   []string{"messages"},
			shouldContain: []string{"messag"}, // stemmed form
		},
		{
			name:          "Create/Creating variants",
			input:         "creating",
			mustContain:   []string{"creating"},
			shouldContain: []string{"creat"}, // stemmed form
		},
		{
			name:        "JWT special case",
			input:       "JWT",
			mustContain: []string{"jwt"},
			// JWT is special case, no stemming
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tokens := tok.Tokenize(tt.input)

			// Create map for easier checking
			tokenMap := make(map[string]bool)
			for _, token := range tokens {
				tokenMap[token] = true
			}

			// Check must-contain tokens
			for _, required := range tt.mustContain {
				if !tokenMap[required] {
					t.Errorf("Expected token %q in results, got: %v", required, tokens)
				}
			}

			// Check should-contain tokens (stemmed)
			for _, expected := range tt.shouldContain {
				if !tokenMap[expected] {
					t.Errorf("Expected stemmed token %q in results, got: %v", expected, tokens)
				}
			}

			t.Logf("Input: %q → Tokens: %v", tt.input, tokens)
		})
	}
}

// TestTokenize_CamelCase verifies camelCase splitting
func TestTokenize_CamelCase(t *testing.T) {
	tok := New()

	tests := []struct {
		name        string
		input       string
		mustContain []string
	}{
		{
			name:        "postMessage",
			input:       "postMessage",
			mustContain: []string{"post", "message"},
		},
		{
			name:        "getUserInfo",
			input:       "getUserInfo",
			mustContain: []string{"get", "user", "info"},
		},
		{
			name:        "PaymentIntent",
			input:       "PaymentIntent",
			mustContain: []string{"payment", "intent"},
		},
		{
			name:        "APIClient",
			input:       "APIClient",
			mustContain: []string{"api", "client"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tokens := tok.Tokenize(tt.input)

			tokenMap := make(map[string]bool)
			for _, token := range tokens {
				tokenMap[token] = true
			}

			for _, required := range tt.mustContain {
				if !tokenMap[required] {
					t.Errorf("Expected token %q in results, got: %v", required, tokens)
				}
			}

			t.Logf("Input: %q → Tokens: %v", tt.input, tokens)
		})
	}
}

// TestTokenize_BothQueryAndData verifies same tokenization for query and data
func TestTokenize_BothQueryAndData(t *testing.T) {
	tok := New()

	// Simulate searching for "authentication" in data containing "authenticate user"
	queryTokens := tok.Tokenize("authentication")
	dataTokens := tok.Tokenize("authenticate user")

	t.Logf("Query tokens: %v", queryTokens)
	t.Logf("Data tokens: %v", dataTokens)

	// Both should contain "authent" (stemmed form), allowing them to match
	queryMap := make(map[string]bool)
	for _, token := range queryTokens {
		queryMap[token] = true
	}

	dataMap := make(map[string]bool)
	for _, token := range dataTokens {
		dataMap[token] = true
	}

	// Check for overlap via stemmed form
	overlap := false
	for token := range queryMap {
		if dataMap[token] {
			overlap = true
			t.Logf("Matched token: %q", token)
		}
	}

	if !overlap {
		t.Errorf("Expected overlap between query and data tokens via stemming")
		t.Errorf("Query: %v", queryTokens)
		t.Errorf("Data: %v", dataTokens)
	}
}

// TestTokenize_StopWords verifies stop word removal
func TestTokenize_StopWords(t *testing.T) {
	tok := New()

	input := "the user is authenticated"
	tokens := tok.Tokenize(input)

	// "the" and "is" should be removed
	for _, token := range tokens {
		if token == "the" || token == "is" {
			t.Errorf("Stop word %q should have been removed from tokens: %v", token, tokens)
		}
	}

	// "user" and "authenticated" should remain
	tokenMap := make(map[string]bool)
	for _, token := range tokens {
		tokenMap[token] = true
	}

	if !tokenMap["user"] {
		t.Errorf("Expected 'user' in tokens, got: %v", tokens)
	}

	// Should contain either "authenticated" or "authent" (stemmed)
	if !tokenMap["authenticated"] && !tokenMap["authent"] {
		t.Errorf("Expected 'authenticated' or 'authent' in tokens, got: %v", tokens)
	}

	t.Logf("Input: %q → Tokens: %v", input, tokens)
}
