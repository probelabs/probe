use std::path::Path;
use crate::models::SearchResult;
use colored::*;

/// Function to format and print search results according to the specified format
pub fn format_and_print_search_results(results: &[SearchResult]) {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
    
    // Check if colors should be disabled
    let no_color = std::env::var("NO_COLOR").is_ok();
    
    if !results.is_empty() {
        println!("{}", "╭─────────────────────────────────────────────────╮".cyan());
        println!("{} {} {}", "│".cyan(), format!("Found {} results", results.len()).bold(), "│".cyan());
        println!("{}", "╰─────────────────────────────────────────────────╯".cyan());
        println!();
    }

    for (index, result) in results.iter().enumerate() {
        // Get file extension
        let file_path = Path::new(&result.file);
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Check if this is a full file or partial file
        let is_full_file = result.node_type == "file";

        // Print result number
        println!("{} {}", "Result".bold().blue(), format!("#{}", index + 1).bold().blue());
        
        // Print the file path and node info with color
        if is_full_file {
            println!("{} {}", "File:".bold().green(), result.file.yellow());
        } else {
            println!("{} {} ({})", "File:".bold().green(), result.file.yellow(), result.node_type.cyan());
            if !result.node_path.is_empty() {
                println!("{} {}", "Node:".bold().green(), result.node_path.cyan());
            }
        }

        // Print the score if available and in debug mode
        if debug_mode {
            if let Some(score) = result.score {
                println!("{} {:.6}", "Score:".dimmed(), score);
            }
            if let Some(tfidf_score) = result.tfidf_score {
                println!("{} {:.6}", "TF-IDF Score:".dimmed(), tfidf_score);
            }
            if let Some(bm25_score) = result.bm25_score {
                println!("{} {:.6}", "BM25 Score:".dimmed(), bm25_score);
            }
            if let Some(content_matches) = &result.content_matches {
                println!("{} {}", "Content matches:".dimmed(), content_matches.join(", "));
            }
            if let Some(filename_matches) = &result.filename_matches {
                println!("{} {}", "Filename matches:".dimmed(), filename_matches.join(", "));
            }
            if let Some(unique_terms) = result.unique_terms {
                println!("{} {}", "Unique terms matched:".dimmed(), unique_terms);
            }
            if let Some(total_matches) = result.total_matches {
                println!("{} {}", "Total term matches:".dimmed(), total_matches);
            }
            
            // Display block-level match statistics in debug mode
            if let Some(block_unique_terms) = result.block_unique_terms {
                println!("{} {}", "Block unique terms matched:".dimmed(), block_unique_terms);
            }
            if let Some(block_total_matches) = result.block_total_matches {
                println!("{} {}", "Block total term matches:".dimmed(), block_total_matches);
            }
        }

        // Print the content with syntax highlighting if available
        if !result.content.is_empty() {
            // Determine the language for syntax highlighting
            let language = match extension {
                "rs" => "rust",
                "py" => "python",
                "js" => "javascript",
                "ts" => "typescript",
                "go" => "go",
                "c" | "h" => "c",
                "cpp" | "cc" | "cxx" | "hpp" => "cpp",
                "java" => "java",
                "rb" => "ruby",
                "php" => "php",
                "sh" => "bash",
                "md" => "markdown",
                "json" => "json",
                "yaml" | "yml" => "yaml",
                "html" => "html",
                "css" => "css",
                "sql" => "sql",
                "kt" | "kts" => "kotlin",
                "swift" => "swift",
                "scala" => "scala",
                "dart" => "dart",
                "ex" | "exs" => "elixir",
                "hs" => "haskell",
                "clj" => "clojure",
                "lua" => "lua",
                "r" => "r",
                "pl" | "pm" => "perl",
                "proto" => "protobuf",
                _ => "",
            };

            println!("{}", "Code:".bold().magenta());
            
            // Print the content with or without syntax highlighting
            if !language.is_empty() {
                println!("{}", format!("```{}", language).cyan());
                println!("{}", result.content);
                println!("{}", "```".cyan());
            } else {
                println!("{}", "```".cyan());
                println!("{}", result.content);
                println!("{}", "```".cyan());
            }
        }

        // Print a separator between results
        if index < results.len() - 1 {
            println!();
            println!("{}", "─".repeat(50).cyan());
            println!();
        }
    }
}
