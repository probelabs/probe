use std::path::Path;

use crate::models::SearchResult;
use crate::search::search_tokens::count_tokens;

/// Function to format and print search results according to the specified format
pub fn format_and_print_search_results(results: &[SearchResult]) {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    for result in results {
        let file_path = Path::new(&result.file);
        let extension = file_path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let is_full_file = result.node_type == "file";

        if is_full_file {
            println!("File: {}", result.file);
            println!("```{}", extension);
            println!("{}", result.code);
            println!("```");
        } else {
            println!("File: {}", result.file);
            println!("Lines: {}-{}", result.lines.0, result.lines.1);
            println!("```{}", extension);
            println!("{}", result.code);
            println!("```");
        }

        if debug_mode {
            if let Some(rank) = result.rank {
                // Add a display order field to show the actual ordering of results
                println!("Display Order: {}", results.iter().position(|r| r.file == result.file && r.lines == result.lines).unwrap_or(0) + 1);
                
                println!("Rank: {}", rank);

                if let Some(score) = result.score {
                    println!("Combined Score: {:.4}", score);
                }

                // Display the combined score rank if available, otherwise calculate it
                if let Some(combined_rank) = result.combined_score_rank {
                    println!("Combined Score Rank: {}", combined_rank);
                } else {
                    // Fall back to the old behavior if the field isn't set
                    println!("Combined Score Rank: {}", rank);
                }

                if let Some(tfidf_score) = result.tfidf_score {
                    println!("TF-IDF Score: {:.4}", tfidf_score);
                }

                if let Some(tfidf_rank) = result.tfidf_rank {
                    println!("TF-IDF Rank: {}", tfidf_rank);
                }

                if let Some(bm25_score) = result.bm25_score {
                    println!("BM25 Score: {:.4}", bm25_score);
                }

                if let Some(bm25_rank) = result.bm25_rank {
                    println!("BM25 Rank: {}", bm25_rank);
                }
                
                // Display Hybrid 2 score and rank with more prominence
                if let Some(new_score) = result.new_score {
                    println!("Hybrid 2 Score: {:.4}", new_score);
                }
                
                if let Some(hybrid2_rank) = result.hybrid2_rank {
                    println!("Hybrid 2 Rank: {}", hybrid2_rank);
                } else if result.new_score.is_some() {
                    println!("Hybrid 2 Rank: N/A");
                }

                if let Some(file_unique_terms) = result.file_unique_terms {
                    println!("File Unique Terms: {}", file_unique_terms);
                }

                if let Some(file_total_matches) = result.file_total_matches {
                    println!("File Total Matches: {}", file_total_matches);
                }

                if let Some(file_match_rank) = result.file_match_rank {
                    println!("File Match Rank: {}", file_match_rank);
                }

                if let Some(block_unique_terms) = result.block_unique_terms {
                    println!("Block Unique Terms: {}", block_unique_terms);
                }

                if let Some(block_total_matches) = result.block_total_matches {
                    println!("Block Total Matches: {}", block_total_matches);
                }

                println!("Type: {}", result.node_type);
            }
        }

        println!("\n");
    }

    println!("Found {} search results", results.len());

    let total_bytes: usize = results.iter().map(|r| r.code.len()).sum();
    let total_tokens: usize = results.iter().map(|r| count_tokens(&r.code)).sum();
    println!("Total bytes returned: {}", total_bytes);
    println!("Total tokens returned: {}", total_tokens);
}
