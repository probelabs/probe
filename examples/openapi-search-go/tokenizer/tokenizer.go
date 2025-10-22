package tokenizer

import (
	"regexp"
	"strings"
	"unicode"

	"github.com/kljensen/snowball"
)

// Tokenizer handles text tokenization with camelCase splitting and stemming
// Based on probe's tokenization logic from src/search/tokenization.rs
type Tokenizer struct {
	stemmer      string
	stopWords    map[string]bool
	specialCases map[string][]string
}

// New creates a new tokenizer with English stemming
func New() *Tokenizer {
	return &Tokenizer{
		stemmer:      "english",
		stopWords:    buildStopWords(),
		specialCases: buildSpecialCases(),
	}
}

// Tokenize converts text into normalized tokens
// Flow: split whitespace → split non-alphanumeric → camelCase → stem → dedupe
func (t *Tokenizer) Tokenize(text string) []string {
	// 1. Split on whitespace
	words := strings.Fields(text)

	seen := make(map[string]bool)
	var tokens []string

	for _, word := range words {
		// 2. Split on non-alphanumeric characters
		parts := t.splitNonAlphanumeric(word)

		for _, part := range parts {
			if part == "" {
				continue
			}

			// 3. Handle special cases (OAuth2, JWT, etc)
			if special, ok := t.specialCases[strings.ToLower(part)]; ok {
				for _, sp := range special {
					lower := strings.ToLower(sp)
					if !seen[lower] && !t.stopWords[lower] {
						tokens = append(tokens, lower)
						seen[lower] = true
					}
				}
				continue
			}

			// 4. Split camelCase/PascalCase
			camelParts := t.splitCamelCase(part)

			for _, camelPart := range camelParts {
				lower := strings.ToLower(camelPart)

				// Skip stop words
				if t.stopWords[lower] {
					continue
				}

				// Add original form
				if !seen[lower] {
					tokens = append(tokens, lower)
					seen[lower] = true
				}

				// 5. Stem the token
				if len(lower) >= 3 {
					stemmed, err := snowball.Stem(lower, t.stemmer, true)
					if err == nil && stemmed != lower && !seen[stemmed] {
						tokens = append(tokens, stemmed)
						seen[stemmed] = true
					}
				}
			}
		}
	}

	return tokens
}

// splitNonAlphanumeric splits text on non-alphanumeric characters
func (t *Tokenizer) splitNonAlphanumeric(s string) []string {
	re := regexp.MustCompile(`[^a-zA-Z0-9]+`)
	return re.Split(s, -1)
}

// splitCamelCase splits camelCase and PascalCase into separate words
// Based on probe's logic from src/search/tokenization.rs:1908-2051
// Examples:
//   camelCase → [camel, Case]
//   parseJSONToHTML5 → [parse, JSON, To, HTML, 5]
//   APIClient → [API, Client]
func (t *Tokenizer) splitCamelCase(s string) []string {
	if len(s) == 0 {
		return nil
	}

	var result []string
	var current strings.Builder

	runes := []rune(s)

	for i := 0; i < len(runes); i++ {
		r := runes[i]

		// Start new word on uppercase if:
		// 1. Current buffer has content and last char is lowercase
		// 2. Current buffer has content and next char is lowercase (acronym boundary)
		if unicode.IsUpper(r) {
			if current.Len() > 0 {
				// Check if this is end of acronym (e.g., "JSON" in "parseJSONTo")
				if i+1 < len(runes) && unicode.IsLower(runes[i+1]) &&
				   i > 0 && unicode.IsUpper(runes[i-1]) {
					// Split before this char
					result = append(result, current.String())
					current.Reset()
				} else if i > 0 && unicode.IsLower(runes[i-1]) {
					// Regular camelCase boundary
					result = append(result, current.String())
					current.Reset()
				}
			}
		}

		// Start new word on digit boundary
		if unicode.IsDigit(r) && current.Len() > 0 && !unicode.IsDigit(runes[i-1]) {
			result = append(result, current.String())
			current.Reset()
		}

		current.WriteRune(r)
	}

	if current.Len() > 0 {
		result = append(result, current.String())
	}

	// Filter empty strings
	filtered := make([]string, 0, len(result))
	for _, part := range result {
		if part != "" {
			filtered = append(filtered, part)
		}
	}

	return filtered
}

// buildStopWords creates a map of common stop words to exclude
func buildStopWords() map[string]bool {
	words := []string{
		// Common English stop words (articles, pronouns, conjunctions)
		"the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
		"of", "with", "by", "from", "as", "is", "was", "are", "be", "have",
		"has", "had", "do", "does", "did", "will", "would", "could", "should",
		"i", "me", "my", "we", "us", "our", "you", "your", "he", "him", "his",
		"she", "her", "it", "its", "they", "them", "their",

		// Question words and auxiliary verbs
		"how", "what", "when", "where", "who", "why", "which", "can", "may",
		"must", "shall", "might", "am", "been", "being",

		// Common filler words
		"very", "too", "also", "just", "only", "so", "than", "such", "both",
		"some", "any", "all", "each", "every", "either", "neither", "much",
		"more", "most", "other", "another", "same", "own", "into", "through",
		"during", "before", "after", "above", "below", "up", "down", "out",
		"off", "over", "under", "again", "further", "then", "once", "want",
		"need", "make", "show", "give", "take", "see", "know",
		"way", "thing", "things", "something", "anything", "everything",
		"nothing", "somewhere", "anywhere", "everywhere", "nowhere",

		// Programming stop words
		"var", "let", "const", "if", "else", "for", "while", "do", "return",
		"function", "class", "new", "this", "that", "import", "export",
	}

	m := make(map[string]bool)
	for _, w := range words {
		m[w] = true
	}
	return m
}

// buildSpecialCases handles special programming terms that shouldn't be split
func buildSpecialCases() map[string][]string {
	return map[string][]string{
		"oauth2":  {"oauth", "2"},
		"jwt":     {"jwt"},
		"http2":   {"http", "2"},
		"ipv4":    {"ipv", "4"},
		"ipv6":    {"ipv", "6"},
		"html5":   {"html", "5"},
		"base64":  {"base", "64"},
		"sha256":  {"sha", "256"},
		"md5":     {"md", "5"},
		"utf8":    {"utf", "8"},
		"openapi": {"openapi", "open", "api"},
	}
}
