package ranker

import (
	"math"
	"sort"
	"sync"
)

// BM25Ranker implements BM25 ranking algorithm
// Based on probe's implementation from src/ranking.rs
type BM25Ranker struct {
	k1 float64 // Term frequency saturation (default 1.5)
	b  float64 // Document length normalization (default 0.5)
}

// New creates a new BM25 ranker with tuned parameters
// k1=1.5 (slightly higher than standard 1.2) gives more weight to term frequency
// b=0.5 (lower than standard 0.75) reduces penalty for longer documents (better for code)
func New() *BM25Ranker {
	return &BM25Ranker{
		k1: 1.5,
		b:  0.5,
	}
}

// Document represents a searchable document with its tokens
type Document struct {
	ID      string
	Content string
	Tokens  []string
	Data    interface{} // Original data (OpenAPI spec, endpoint, etc)
}

// ScoredResult represents a ranked search result
type ScoredResult struct {
	Document *Document
	Score    float64
	Rank     int
}

// Rank scores documents using BM25 algorithm
// Returns results sorted by score (highest first)
func (r *BM25Ranker) Rank(documents []*Document, queryTokens []string) []*ScoredResult {
	if len(documents) == 0 || len(queryTokens) == 0 {
		return nil
	}

	// 1. Build term frequency (TF) maps for each document
	// 2. Calculate document frequency (DF) for each term
	// 3. Compute average document length
	docTF := make([]map[string]int, len(documents))
	docLengths := make([]int, len(documents))
	termDF := make(map[string]int)

	for i, doc := range documents {
		tf := make(map[string]int)
		for _, token := range doc.Tokens {
			tf[token]++
		}
		docTF[i] = tf
		docLengths[i] = len(doc.Tokens)

		// Track which documents contain each term (for DF)
		seen := make(map[string]bool)
		for token := range tf {
			if !seen[token] {
				termDF[token]++
				seen[token] = true
			}
		}
	}

	// Calculate average document length
	avgdl := r.computeAvgDocLength(docLengths)

	// 4. Precompute IDF for all query terms
	// IDF formula: ln(1 + (N - df + 0.5) / (df + 0.5))
	queryTermSet := make(map[string]bool)
	for _, token := range queryTokens {
		queryTermSet[token] = true
	}

	idf := make(map[string]float64)
	nDocs := float64(len(documents))
	for term := range queryTermSet {
		df := float64(termDF[term])
		idf[term] = math.Log(1.0 + (nDocs-df+0.5)/(df+0.5))
	}

	// 5. Score documents in parallel
	results := make([]*ScoredResult, len(documents))
	var wg sync.WaitGroup

	for i := range documents {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			score := r.scoreBM25(docTF[idx], docLengths[idx], avgdl, queryTokens, idf)
			results[idx] = &ScoredResult{
				Document: documents[idx],
				Score:    score,
			}
		}(i)
	}

	wg.Wait()

	// 6. Sort by score (descending)
	sort.Slice(results, func(i, j int) bool {
		// Primary: higher score first
		if results[i].Score != results[j].Score {
			return results[i].Score > results[j].Score
		}
		// Secondary: stable sort by index for determinism
		return i < j
	})

	// Assign ranks
	for i := range results {
		results[i].Rank = i + 1
	}

	return results
}

// scoreBM25 computes BM25 score for a single document
// Formula: sum over query terms of: IDF(term) * (TF * (k1+1)) / (TF + k1 * (1-b + b*(docLen/avgdl)))
func (r *BM25Ranker) scoreBM25(
	docTF map[string]int,
	docLen int,
	avgdl float64,
	queryTokens []string,
	idf map[string]float64,
) float64 {
	score := 0.0
	docLenNorm := 1.0 - r.b + r.b*(float64(docLen)/avgdl)

	for _, token := range queryTokens {
		tf := float64(docTF[token])
		if tf == 0 {
			continue
		}

		termIDF := idf[token]

		// BM25 TF component: (tf * (k1+1)) / (tf + k1 * docLenNorm)
		tfComponent := (tf * (r.k1 + 1.0)) / (tf + r.k1*docLenNorm)

		score += termIDF * tfComponent
	}

	return score
}

// computeAvgDocLength calculates average document length
func (r *BM25Ranker) computeAvgDocLength(lengths []int) float64 {
	if len(lengths) == 0 {
		return 0.0
	}

	sum := 0
	for _, l := range lengths {
		sum += l
	}

	return float64(sum) / float64(len(lengths))
}
