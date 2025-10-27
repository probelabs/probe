package main

import (
	"flag"
	"fmt"
	"openapi-search/search"
	"os"
	"strings"
)

func main() {
	// Parse command line flags
	specsDir := flag.String("specs", "specs", "Directory containing OpenAPI specs")
	query := flag.String("query", "", "Search query")
	maxResults := flag.Int("max", 10, "Maximum number of results")
	flag.Parse()

	// If query not provided via flag, use remaining args
	if *query == "" && len(flag.Args()) > 0 {
		*query = strings.Join(flag.Args(), " ")
	}

	if *query == "" {
		fmt.Println("Usage: openapi-search -query \"your search query\" [-specs dir] [-max 10]")
		fmt.Println("   or: openapi-search \"your search query\"")
		os.Exit(1)
	}

	// Create search engine
	engine := search.NewEngine()

	// Index OpenAPI specs
	fmt.Printf("Indexing OpenAPI specs from: %s\n", *specsDir)
	if err := engine.IndexDirectory(*specsDir); err != nil {
		fmt.Fprintf(os.Stderr, "Error indexing specs: %v\n", err)
		os.Exit(1)
	}

	fmt.Println(engine.Stats())
	fmt.Println()

	// Perform search
	fmt.Printf("Searching for: \"%s\"\n", *query)
	fmt.Println(strings.Repeat("=", 80))

	results := engine.Search(*query, *maxResults)

	if len(results) == 0 {
		fmt.Println("No results found.")
		return
	}

	// Display results
	for i, result := range results {
		fmt.Printf("\n%d. [Score: %.2f] %s\n", i+1, result.Score, result.Endpoint.String())

		if result.Endpoint.Description != "" {
			fmt.Printf("   Description: %s\n", truncate(result.Endpoint.Description, 100))
		}

		if len(result.Matches) > 0 {
			fmt.Printf("   Matched terms: %s\n", strings.Join(result.Matches, ", "))
		}

		// Show parameters if any
		if len(result.Endpoint.Parameters) > 0 {
			fmt.Printf("   Parameters:\n")
			for _, param := range result.Endpoint.Parameters {
				required := ""
				if param.Required {
					required = " (required)"
				}
				fmt.Printf("     - %s (%s)%s: %s\n",
					param.Name,
					param.In,
					required,
					truncate(param.Description, 60))
			}
		}
	}

	fmt.Printf("\n%s\n", strings.Repeat("=", 80))
	fmt.Printf("Found %d results\n", len(results))
}

func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "..."
}
