package search

import (
	"fmt"
	"openapi-search/ranker"
	"openapi-search/tokenizer"
	"path/filepath"
	"strings"
)

// Engine performs semantic search over OpenAPI specifications
type Engine struct {
	specs     []*OpenAPISpec
	endpoints []Endpoint
	documents []*ranker.Document // Pre-created documents for efficient search
	tokenizer *tokenizer.Tokenizer
	ranker    *ranker.BM25Ranker
}

// NewEngine creates a new search engine
func NewEngine() *Engine {
	return &Engine{
		tokenizer: tokenizer.New(),
		ranker:    ranker.New(),
	}
}

// IndexSpec loads and indexes an OpenAPI spec file
func (e *Engine) IndexSpec(path string) error {
	spec, err := LoadSpec(path)
	if err != nil {
		return fmt.Errorf("failed to load spec %s: %w", path, err)
	}

	e.specs = append(e.specs, spec)

	// Extract and index endpoints with pre-tokenization
	endpoints := spec.ExtractEndpoints()

	// Pre-tokenize all endpoints and create documents once
	startIdx := len(e.endpoints)
	for i := range endpoints {
		text := endpoints[i].GetSearchableText()
		endpoints[i].Tokens = e.tokenizer.Tokenize(text)

		// Pre-compute term frequency map
		tf := make(map[string]int)
		for _, token := range endpoints[i].Tokens {
			tf[token]++
		}

		// Create document once during indexing with pre-computed TF
		doc := &ranker.Document{
			ID:      fmt.Sprintf("%s:%s", endpoints[i].Method, endpoints[i].Path),
			Content: text,
			Tokens:  endpoints[i].Tokens,
			TF:      tf,
			Data:    nil, // Will set after appending to e.endpoints
		}
		e.documents = append(e.documents, doc)
	}

	e.endpoints = append(e.endpoints, endpoints...)

	// Fix document Data pointers to point to actual endpoints slice
	for i := range endpoints {
		e.documents[startIdx+i].Data = &e.endpoints[startIdx+i]
	}

	return nil
}

// IndexDirectory loads and indexes all OpenAPI specs in a directory
func (e *Engine) IndexDirectory(dir string) error {
	files, err := filepath.Glob(filepath.Join(dir, "*.yaml"))
	if err != nil {
		return err
	}

	jsonFiles, err := filepath.Glob(filepath.Join(dir, "*.json"))
	if err != nil {
		return err
	}

	files = append(files, jsonFiles...)

	for _, file := range files {
		if err := e.IndexSpec(file); err != nil {
			// Log error but continue indexing other files
			fmt.Printf("Warning: failed to index %s: %v\n", file, err)
		}
	}

	return nil
}

// SearchResult represents a search result with context
type SearchResult struct {
	Endpoint Endpoint
	Score    float64
	Rank     int
	Matches  []string // Matched query terms
}

// Search performs semantic search over indexed endpoints
// Returns results ranked by BM25 relevance score
func (e *Engine) Search(query string, maxResults int) []SearchResult {
	if len(e.endpoints) == 0 {
		return nil
	}

	// 1. Tokenize query
	queryTokens := e.tokenizer.Tokenize(query)
	if len(queryTokens) == 0 {
		return nil
	}

	// 2. Use pre-created documents (no allocation overhead)
	documents := e.documents

	// 3. Rank with BM25
	scored := e.ranker.Rank(documents, queryTokens)

	// 4. Convert to search results
	results := make([]SearchResult, 0, len(scored))
	queryTokenSet := make(map[string]bool)
	for _, token := range queryTokens {
		queryTokenSet[token] = true
	}

	for _, s := range scored {
		if s.Score == 0 {
			continue // Skip zero-score results
		}

		endpoint := s.Document.Data.(*Endpoint)

		// Find which query tokens matched
		var matches []string
		seen := make(map[string]bool)
		for _, token := range s.Document.Tokens {
			if queryTokenSet[token] && !seen[token] {
				matches = append(matches, token)
				seen[token] = true
			}
		}

		results = append(results, SearchResult{
			Endpoint: *endpoint,
			Score:    s.Score,
			Rank:     s.Rank,
			Matches:  matches,
		})

		if maxResults > 0 && len(results) >= maxResults {
			break
		}
	}

	return results
}

// Stats returns statistics about indexed data
func (e *Engine) Stats() string {
	var sb strings.Builder

	sb.WriteString(fmt.Sprintf("Indexed specs: %d\n", len(e.specs)))
	sb.WriteString(fmt.Sprintf("Total endpoints: %d\n", len(e.endpoints)))

	// Count endpoints by method
	methodCount := make(map[string]int)
	for _, ep := range e.endpoints {
		methodCount[ep.Method]++
	}

	sb.WriteString("\nEndpoints by method:\n")
	for method, count := range methodCount {
		sb.WriteString(fmt.Sprintf("  %s: %d\n", method, count))
	}

	return sb.String()
}
