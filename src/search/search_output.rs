use anyhow::Result;
use regex::Regex;
use std::path::Path;

use probe_code::models::SearchResult;
use probe_code::search::query::QueryPlan;
use probe_code::search::search_tokens::sum_tokens_with_deduplication;

/// Function to format and print search results according to the specified format
pub fn format_and_print_search_results(
    results: &[SearchResult],
    dry_run: bool,
    format: &str,
    query_plan: Option<&QueryPlan>,
    symbols: bool,
) {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Count valid results (with non-empty file names)
    let valid_results: Vec<&SearchResult> = results.iter().filter(|r| !r.file.is_empty()).collect();

    // Check if terminal supports colors and if output is being piped
    let use_color = match format {
        "color" => colored::control::SHOULD_COLORIZE.should_colorize(),
        _ => false,
    };

    // Handle different output formats
    match format {
        "color" if use_color => {
            format_and_print_color_results(&valid_results, dry_run, query_plan, debug_mode, symbols);
        }
        "json" => {
            if let Err(e) = format_and_print_json_results(&valid_results, symbols) {
                eprintln!("Error formatting JSON: {e}");
            }
            return; // Skip the summary output at the end
        }
        "xml" => {
            if let Err(e) = format_and_print_xml_results(&valid_results, symbols) {
                eprintln!("Error formatting XML: {e}");
            }
            return; // Skip the summary output at the end
        }
        "outline" => {
            format_and_print_outline_results(&valid_results, dry_run, symbols);
            return; // Skip the duplicate summary output at the end
        }
        _ => {
            // Default format (terminal)
            for result in &valid_results {
                let file_path = Path::new(&result.file);
                let extension = file_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("");
                let is_full_file = result.node_type == "file";

                if dry_run {
                    // In dry-run mode, only print file names and line numbers
                    if is_full_file {
                        println!("File: {}", result.file);
                    } else {
                        println!(
                            "File: {}, Lines: {}-{}",
                            result.file, result.lines.0, result.lines.1
                        );
                    }
                } else {
                    // Normal mode with full content or symbol display
                    if is_full_file {
                        println!("File: {}", result.file);
                        if symbols && result.symbol_signature.is_some() {
                            println!("Symbol: {}", result.symbol_signature.as_ref().unwrap());
                        } else if symbols {
                            println!("Symbol: <not available>");
                        } else {
                            println!("```{extension}");
                            println!("{}", result.code);
                            println!("```");
                        }
                    } else {
                        println!("File: {}", result.file);
                        println!(
                            "Lines: {start}-{end}",
                            start = result.lines.0,
                            end = result.lines.1
                        );
                        if symbols && result.symbol_signature.is_some() {
                            println!("Symbol: {}", result.symbol_signature.as_ref().unwrap());
                        } else if symbols {
                            println!("Symbol: <not available>");
                        } else {
                            println!("```{extension}");
                            println!("{code}", code = result.code);
                            println!("```");
                        }
                    }
                }
                if debug_mode {
                    if let Some(rank) = result.rank {
                        // Add a display order field to show the actual ordering of results
                        println!(
                            "Display Order: {}",
                            results
                                .iter()
                                .position(|r| r.file == result.file && r.lines == result.lines)
                                .unwrap_or(0)
                                + 1
                        );

                        println!("Rank: {rank}");

                        if let Some(score) = result.score {
                            println!("Combined Score: {score:.4}");
                        }

                        // Display the combined score rank if available, otherwise calculate it
                        if let Some(combined_rank) = result.combined_score_rank {
                            println!("Combined Score Rank: {combined_rank}");
                        } else {
                            // Fall back to the old behavior if the field isn't set
                            println!("Combined Score Rank: {rank}");
                        }

                        if let Some(tfidf_score) = result.tfidf_score {
                            println!("TF-IDF Score: {tfidf_score:.4}");
                        }

                        if let Some(tfidf_rank) = result.tfidf_rank {
                            println!("TF-IDF Rank: {tfidf_rank}");
                        }

                        if let Some(bm25_score) = result.bm25_score {
                            // Check if this is actually a BERT score by looking at the rank field
                            // When BERT reranking is used, both score and bm25_score are set to BERT score
                            let is_bert_score =
                                result.score == result.bm25_score && result.score.is_some();
                            if is_bert_score {
                                println!("BERT Score: {bm25_score:.4}");
                            } else {
                                println!("BM25 Score: {bm25_score:.4}");
                            }
                        }

                        if let Some(bm25_rank) = result.bm25_rank {
                            println!("BM25 Rank: {bm25_rank}");
                        }

                        // Display Hybrid 2 score and rank with more prominence
                        if let Some(new_score) = result.new_score {
                            println!("Hybrid 2 Score: {new_score:.4}");
                        }

                        if let Some(hybrid2_rank) = result.hybrid2_rank {
                            println!("Hybrid 2 Rank: {hybrid2_rank}");
                        } else if result.new_score.is_some() {
                            println!("Hybrid 2 Rank: N/A");
                        }

                        if let Some(file_unique_terms) = result.file_unique_terms {
                            println!("File Unique Terms: {file_unique_terms}");
                        }

                        if let Some(file_total_matches) = result.file_total_matches {
                            println!("File Total Matches: {file_total_matches}");
                        }

                        if let Some(file_match_rank) = result.file_match_rank {
                            println!("File Match Rank: {file_match_rank}");
                        }

                        if let Some(block_unique_terms) = result.block_unique_terms {
                            println!("Block Unique Terms: {block_unique_terms}");
                        }

                        if let Some(block_total_matches) = result.block_total_matches {
                            println!("Block Total Matches: {block_total_matches}");
                        }

                        println!("Type: {}", result.node_type);
                    }
                }
            }
        }
    }

    println!("Found {count} search results", count = valid_results.len());

    let total_bytes: usize = if symbols {
        // In symbols mode, count bytes from symbol signatures instead of full code
        valid_results.iter().map(|r| {
            r.symbol_signature.as_ref().map(|s| s.len()).unwrap_or(0)
        }).sum()
    } else {
        valid_results.iter().map(|r| r.code.len()).sum()
    };

    // BATCH TOKENIZATION WITH DEDUPLICATION OPTIMIZATION:
    // Use batch processing with content deduplication for improved performance
    // when multiple identical code blocks need tokenization (common in search results)
    let total_tokens: usize = if symbols {
        // In symbols mode, count tokens from symbol signatures instead of full code
        let symbol_blocks: Vec<&str> = valid_results.iter()
            .filter_map(|r| r.symbol_signature.as_ref().map(|s| s.as_str()))
            .collect();
        sum_tokens_with_deduplication(&symbol_blocks)
    } else {
        let code_blocks: Vec<&str> = valid_results.iter().map(|r| r.code.as_str()).collect();
        sum_tokens_with_deduplication(&code_blocks)
    };
    println!("Total bytes returned: {total_bytes}");
    println!("Total tokens returned: {total_tokens}");
}

/// Format and print search results with color highlighting for matching words
fn format_and_print_color_results(
    results: &[&SearchResult],
    dry_run: bool,
    query_plan: Option<&QueryPlan>,
    debug_mode: bool,
    symbols: bool,
) {
    use colored::*;
    use regex::Regex;

    if results.is_empty() {
        println!("No results found.");
        return;
    }

    // Print a header with the number of results
    println!("{}", format!("Found {} results", results.len()).bold());
    println!();

    // Print the results
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
        println!(
            "{} {}",
            "Result".bold().blue(),
            format!("#{}", index + 1).bold().blue()
        );

        // Print the file path and node info with color
        if is_full_file {
            println!(
                "{label} {file}",
                label = "File:".bold().green().yellow(),
                file = result.file
            );
        } else {
            println!(
                "{} {} ({})",
                "File:".bold().green(),
                result.file.yellow(),
                result.node_type.cyan()
            );
            println!(
                "{} {}-{}",
                "Lines:".bold().green(),
                result.lines.0,
                result.lines.1
            );
        }

        // Print additional debug information if in debug mode
        if debug_mode {
            // Print the same debug info that would be shown in standard mode
            if let Some(keywords) = &result.matched_keywords {
                println!("{} {keywords:?}", "Matched Keywords:".bold().green());
            }
            if let Some(score) = result.score {
                println!("{} {score:.4}", "Score:".bold().green());
            }
            if let Some(query_plan) = query_plan {
                println!("{} {query_plan:?}", "Query Plan:".bold().green());
            }
        }

        if dry_run {
            // In dry-run mode, don't print the content
            continue;
        }

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

        // Check if we should display symbols instead of code
        if symbols {
            if let Some(symbol_signature) = &result.symbol_signature {
                println!("{label} {signature}", 
                    label = "Symbol:".bold().magenta(),
                    signature = symbol_signature.bright_cyan());
            } else {
                println!("{label} {not_available}", 
                    label = "Symbol:".bold().magenta(),
                    not_available = "<not available>".dimmed());
            }
        } else {
            println!("{label}", label = "Code:".bold().magenta());

            // Print the code with syntax highlighting
            if !language.is_empty() {
                println!("{code_block}", code_block = format!("```{language}").cyan());
            } else {
                println!("{code_block}", code_block = "```".cyan());
            }
        }

        // Only print code with highlighting if not in symbols mode
        if !symbols {
            // Generate patterns from the matched keywords in the search result
            let mut patterns = Vec::new();

            // Use the matched keywords from the search result if available
            if let Some(keywords) = &result.matched_keywords {
                for keyword in keywords {
                    // Create a case-insensitive regex for the keyword with word boundaries
                    if let Ok(regex) = Regex::new(&format!(r"(?i){}", regex::escape(keyword))) {
                        patterns.push(regex);
                    }

                    // Also try to match camelCase/PascalCase variations
                    if let Ok(regex) = Regex::new(&format!(r"(?i){}", regex::escape(keyword))) {
                        patterns.push(regex);
                    }
                }
            }

            // If no patterns were generated, add some default patterns
            if patterns.is_empty() {
                // Use lazily initialized static regexes to avoid recompilation
                lazy_static::lazy_static! {
                    static ref STRUCT_REGEX: Regex = Regex::new(r"(?i)struct").unwrap();
                    static ref SEARCH_REGEX: Regex = Regex::new(r"(?i)search").unwrap();
                    static ref RESULT_REGEX: Regex = Regex::new(r"(?i)result").unwrap();
                }

                patterns.push(STRUCT_REGEX.clone());
                patterns.push(SEARCH_REGEX.clone());
                patterns.push(RESULT_REGEX.clone());
            }

            // Process the code line by line with inline highlighting
            for line in result.code.lines() {
                let mut output_line = String::new();
                let mut last_end = 0;
                let mut matches = Vec::new();

                // Collect all matches from all patterns
                for pattern in &patterns {
                    for mat in pattern.find_iter(line) {
                        matches.push((mat.start(), mat.end()));
                    }
                }

                // Sort matches by start position
                matches.sort_by_key(|&(start, _)| start);

                // Merge overlapping matches
                let mut merged_matches = Vec::new();
                for (start, end) in matches {
                    if let Some((_, prev_end)) = merged_matches.last() {
                        if start <= *prev_end {
                            // Overlapping match, extend the previous one
                            let last_idx = merged_matches.len() - 1;
                            merged_matches[last_idx].1 = end.max(*prev_end);
                            continue;
                        }
                    }
                    merged_matches.push((start, end));
                }

                // Build the highlighted line
                for &(start, end) in &merged_matches {
                    // Add text before the match
                    if start > last_end {
                        output_line.push_str(&line[last_end..start]);
                    }

                    // Add the highlighted match
                    let matched_text = &line[start..end];
                    output_line.push_str(&matched_text.yellow().bold().to_string());

                    last_end = end;
                }

                // Add any remaining text
                if last_end < line.len() {
                    output_line.push_str(&line[last_end..]);
                }

                // Print the line (highlighted or original if no matches)
                if !merged_matches.is_empty() {
                    println!("{output_line}");
                } else {
                    println!("{line}");
                }
            }
        }

        println!();

        // Print a separator between results
        if index < results.len() - 1 {
            println!();
            println!("{separator}", separator = "─".repeat(50).cyan());
            println!();
        }

        if debug_mode {
            if let Some(rank) = result.rank {
                // Add a display order field to show the actual ordering of results
                println!(
                    "Display Order: {}",
                    results
                        .iter()
                        .position(|r| r.file == result.file && r.lines == result.lines)
                        .unwrap_or(0)
                        + 1
                );

                println!("Rank: {rank}");

                if let Some(score) = result.score {
                    println!("Combined Score: {score:.4}");
                }

                // Display the combined score rank if available, otherwise calculate it
                if let Some(combined_rank) = result.combined_score_rank {
                    println!("Combined Score Rank: {combined_rank}");
                } else {
                    // Fall back to the old behavior if the field isn't set
                    println!("Combined Score Rank: {rank}");
                }

                if let Some(tfidf_score) = result.tfidf_score {
                    println!("TF-IDF Score: {tfidf_score:.4}");
                }

                if let Some(tfidf_rank) = result.tfidf_rank {
                    println!("TF-IDF Rank: {tfidf_rank}");
                }

                if let Some(bm25_score) = result.bm25_score {
                    // Check if this is actually a BERT score by looking at the rank field
                    // When BERT reranking is used, both score and bm25_score are set to BERT score
                    let is_bert_score = result.score == result.bm25_score && result.score.is_some();
                    if is_bert_score {
                        println!("BERT Score: {bm25_score:.4}");
                    } else {
                        println!("BM25 Score: {bm25_score:.4}");
                    }
                }

                if let Some(bm25_rank) = result.bm25_rank {
                    println!("BM25 Rank: {bm25_rank}");
                }

                // Display Hybrid 2 score and rank with more prominence
                if let Some(new_score) = result.new_score {
                    println!("Hybrid 2 Score: {new_score:.4}");
                }

                if let Some(hybrid2_rank) = result.hybrid2_rank {
                    println!("Hybrid 2 Rank: {hybrid2_rank}");
                } else if result.new_score.is_some() {
                    println!("Hybrid 2 Rank: N/A");
                }

                if let Some(file_unique_terms) = result.file_unique_terms {
                    println!("File Unique Terms: {file_unique_terms}");
                }

                if let Some(file_total_matches) = result.file_total_matches {
                    println!("File Total Matches: {file_total_matches}");
                }

                if let Some(file_match_rank) = result.file_match_rank {
                    println!("File Match Rank: {file_match_rank}");
                }

                if let Some(block_unique_terms) = result.block_unique_terms {
                    println!("Block Unique Terms: {block_unique_terms}");
                }

                if let Some(block_total_matches) = result.block_total_matches {
                    println!("Block Total Matches: {block_total_matches}");
                }

                println!("Type: {}", result.node_type);
            }
        }
    }
}

/// Helper function to escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
}

/// Format and print search results in JSON format
fn format_and_print_json_results(results: &[&SearchResult], symbols: bool) -> Result<()> {
    // Create a simplified version of the results for JSON output
    #[derive(serde::Serialize)]
    struct JsonResult<'a> {
        file: &'a str,
        lines: [usize; 2],
        node_type: &'a str,
        code: &'a str,
        // Symbol signature (when symbols flag is used)
        symbol_signature: Option<&'a String>,
        // Include other relevant fields
        matched_keywords: Option<&'a Vec<String>>,
        score: Option<f64>,
        tfidf_score: Option<f64>,
        bm25_score: Option<f64>,
        file_unique_terms: Option<usize>,
        file_total_matches: Option<usize>,
        block_unique_terms: Option<usize>,
        block_total_matches: Option<usize>,
    }

    let json_results: Vec<JsonResult> = results
        .iter()
        .map(|r| JsonResult {
            file: &r.file,
            lines: [r.lines.0, r.lines.1],
            node_type: &r.node_type,
            code: &r.code,
            symbol_signature: r.symbol_signature.as_ref(),
            matched_keywords: r.matched_keywords.as_ref(),
            score: r.score,
            tfidf_score: r.tfidf_score,
            bm25_score: r.bm25_score,
            file_unique_terms: r.file_unique_terms,
            file_total_matches: r.file_total_matches,
            block_unique_terms: r.block_unique_terms,
            block_total_matches: r.block_total_matches,
        })
        .collect();

    // BATCH TOKENIZATION WITH DEDUPLICATION OPTIMIZATION for JSON output:
    // Process all code blocks in batch to leverage content deduplication
    let total_tokens = if symbols {
        // In symbols mode, count tokens from symbol signatures instead of full code
        let symbol_blocks: Vec<&str> = results.iter()
            .filter_map(|r| r.symbol_signature.as_ref().map(|s| s.as_str()))
            .collect();
        sum_tokens_with_deduplication(&symbol_blocks)
    } else {
        let code_blocks: Vec<&str> = results.iter().map(|r| r.code.as_str()).collect();
        sum_tokens_with_deduplication(&code_blocks)
    };

    // Create a wrapper object with results and summary
    let wrapper = serde_json::json!({
        "results": json_results,
        "summary": {
            "count": results.len(),
            "total_bytes": if symbols {
                results.iter().map(|r| {
                    r.symbol_signature.as_ref().map(|s| s.len()).unwrap_or(0)
                }).sum::<usize>()
            } else {
                results.iter().map(|r| r.code.len()).sum::<usize>()
            },
            "total_tokens": total_tokens,
        },
        "version": probe_code::version::get_version()
    });

    println!("{json}", json = serde_json::to_string_pretty(&wrapper)?);
    Ok(())
}

/// Format and print search results in XML format
fn format_and_print_xml_results(results: &[&SearchResult], symbols: bool) -> Result<()> {
    println!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    println!("<probe_results>");

    for result in results {
        println!("  <result>");
        println!("    <file>{file}</file>", file = escape_xml(&result.file));
        println!(
            "    <lines>{start}-{end}</lines>",
            start = result.lines.0,
            end = result.lines.1
        );
        println!(
            "    <node_type>{}</node_type>",
            escape_xml(&result.node_type)
        );

        if let Some(symbol_signature) = &result.symbol_signature {
            println!(
                "    <symbol_signature>{}</symbol_signature>",
                escape_xml(symbol_signature)
            );
        }

        if let Some(keywords) = &result.matched_keywords {
            println!("    <matched_keywords>");
            for keyword in keywords {
                println!(
                    "      <keyword>{keyword}</keyword>",
                    keyword = escape_xml(keyword)
                );
            }
            println!("    </matched_keywords>");
        }

        if let Some(score) = result.score {
            println!("    <score>{score:.4}</score>");
        }

        if let Some(tfidf_score) = result.tfidf_score {
            println!("    <tfidf_score>{tfidf_score:.4}</tfidf_score>");
        }

        if let Some(bm25_score) = result.bm25_score {
            println!("    <bm25_score>{bm25_score:.4}</bm25_score>");
        }

        if let Some(file_unique_terms) = result.file_unique_terms {
            println!("    <file_unique_terms>{file_unique_terms}</file_unique_terms>");
        }

        if let Some(file_total_matches) = result.file_total_matches {
            println!("    <file_total_matches>{file_total_matches}</file_total_matches>");
        }

        if let Some(block_unique_terms) = result.block_unique_terms {
            println!("    <block_unique_terms>{block_unique_terms}</block_unique_terms>");
        }

        if let Some(block_total_matches) = result.block_total_matches {
            println!("    <block_total_matches>{block_total_matches}</block_total_matches>");
        }

        println!("    <code><![CDATA[{code}]]></code>", code = result.code);
        println!("  </result>");
    }

    // Add summary section
    println!("  <summary>");
    println!("    <count>{}</count>", results.len());
    println!(
        "    <total_bytes>{total_bytes}</total_bytes>",
        total_bytes = if symbols {
            results.iter().map(|r| {
                r.symbol_signature.as_ref().map(|s| s.len()).unwrap_or(0)
            }).sum::<usize>()
        } else {
            results.iter().map(|r| r.code.len()).sum::<usize>()
        }
    );
    // BATCH TOKENIZATION WITH DEDUPLICATION OPTIMIZATION for XML output:
    // Process all code blocks in batch to leverage content deduplication
    let total_tokens = if symbols {
        // In symbols mode, count tokens from symbol signatures instead of full code
        let symbol_blocks: Vec<&str> = results.iter()
            .filter_map(|r| r.symbol_signature.as_ref().map(|s| s.as_str()))
            .collect();
        sum_tokens_with_deduplication(&symbol_blocks)
    } else {
        let code_blocks: Vec<&str> = results.iter().map(|r| r.code.as_str()).collect();
        sum_tokens_with_deduplication(&code_blocks)
    };

    println!("    <total_tokens>{total_tokens}</total_tokens>");
    println!("  </summary>");

    println!(
        "  <version>{}</version>",
        probe_code::version::get_version()
    );

    println!("</probe_results>");
    Ok(())
}

use crate::language::factory::get_language_impl;
use crate::language::tree_cache::get_or_parse_tree_pooled;
use crate::language::language_trait::LanguageImpl;
use tree_sitter::Node;

/// Helper to get file extension as a &str
/// Find comments that precede a given node using tree-sitter AST
fn find_preceding_comments(node: &tree_sitter::Node, source: &str, node_line: usize) -> Vec<(usize, usize, String)> {
    let mut comments = Vec::new();
    let source_bytes = source.as_bytes();
    let source_lines: Vec<&str> = source.lines().collect();
    
    // Get the root node to search for comments
    let mut current = *node;
    while let Some(parent) = current.parent() {
        current = parent;
    }
    let root = current;
    
    // Find all comment nodes that appear before this node
    let mut all_comments = Vec::new();
    let mut cursor = root.walk();
    find_comment_nodes_with_range(&mut cursor, source_bytes, node_line, &mut all_comments);
    
    // Sort comments by their line position
    all_comments.sort_by_key(|(start_line, _, _)| *start_line);
    
    // Find comments that are actually immediately preceding this node
    // A comment is considered "immediately preceding" if:
    // 1. It appears before the target line
    // 2. There are no non-empty, non-comment lines between the comment and the target node
    for (comment_start_line, comment_end_line, comment_text) in all_comments {
        if comment_start_line < node_line {
            // Check if there are any non-empty, non-comment lines between this comment and the target node
            let mut is_immediately_preceding = true;
            
            // Look at all lines between the comment end and the target node
            for line_num in (comment_end_line + 1)..node_line {
                if line_num > 0 && line_num <= source_lines.len() {
                    let line = source_lines[line_num - 1].trim();
                    // If we find a non-empty line that's not a comment, this comment doesn't immediately precede the target
                    if !line.is_empty() && !line.starts_with("//") && !line.starts_with("/*") && !line.starts_with("*") {
                        is_immediately_preceding = false;
                        break;
                    }
                }
            }
            
            // Also check that the comment is within a reasonable distance (max 3 lines)
            // to avoid associating distant comments
            if is_immediately_preceding && (node_line - comment_end_line) <= 3 {
                comments.push((comment_start_line, comment_end_line, comment_text));
            }
        }
    }
    
    comments
}

/// Recursively find comment nodes in the AST with their line ranges
fn find_comment_nodes_with_range(
    cursor: &mut tree_sitter::TreeCursor,
    source: &[u8],
    target_line: usize,
    comments: &mut Vec<(usize, usize, String)>
) {
    let node = cursor.node();
    let node_start_line = node.start_position().row + 1;
    let node_end_line = node.end_position().row + 1;
    
    // Check if this node is a comment and appears before the target line
    if is_comment_node(&node) && node_start_line < target_line {
        if let Ok(comment_text) = node.utf8_text(source) {
            comments.push((node_start_line, node_end_line, comment_text.to_string()));
        }
    }
    
    // Recursively search children
    if cursor.goto_first_child() {
        loop {
            find_comment_nodes_with_range(cursor, source, target_line, comments);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

/// Check if a node represents a comment
fn is_comment_node(node: &tree_sitter::Node) -> bool {
    let kind = node.kind();
    matches!(kind, 
        "comment" | 
        "line_comment" | 
        "block_comment" |
        "//" |
        "/*" |
        "*/"
    )
}


fn file_extension(path: &std::path::Path) -> &str {
    path.extension().and_then(|ext| ext.to_str()).unwrap_or("")
}

/// Collect parent context for a specific line in the source
/// Find the complete multiline construct (expression/statement) that contains the target line
/// Modified to only return lines that actually contain search terms or are structurally necessary
fn find_complete_construct_for_line(file_path: &str, target_line: usize, source: &str) -> Vec<(usize, String)> {
    // Get file extension and use existing tree parsing infrastructure
    let extension = file_extension(std::path::Path::new(file_path));
    
    // Parse the tree (using cached tree if available)
    let tree = match get_or_parse_tree_pooled(file_path, source, extension) {
        Ok(t) => t,
        Err(_) => return Vec::new(), // Return empty if can't parse
    };
    
    let lines: Vec<&str> = source.lines().collect();
    if target_line == 0 || target_line > lines.len() {
        return Vec::new();
    }
    
    // Convert 1-based line number to 0-based for tree-sitter
    let target_row = target_line - 1;
    
    // Find the most specific node that contains the target line
    let mut cursor = tree.walk();
    let mut best_node = None;
    let mut best_size = usize::MAX;
    
    fn find_smallest_containing_node<'a>(
        cursor: &mut tree_sitter::TreeCursor<'a>,
        target_row: usize,
        best_node: &mut Option<tree_sitter::Node<'a>>,
        best_size: &mut usize,
    ) {
        let node = cursor.node();
        let start_row = node.start_position().row;
        let end_row = node.end_position().row;
        
        // Check if this node contains the target line
        if start_row <= target_row && target_row <= end_row {
            let size = end_row - start_row + 1;
            
            // Look for expression or statement nodes specifically
            // Prioritize more specific nodes over generic ones
            if matches!(node.kind(), 
                "call_expression" | "macro_invocation" | 
                "if_expression" | "while_expression" | "for_expression" | "match_expression" |
                "assignment_expression" | "binary_expression" | "unary_expression" |
                "return_statement" | "break_statement" | "continue_statement" |
                // Add more language-specific constructs as needed
                "function_call" | "method_call" | "array_expression" | "object_expression"
            ) && size < *best_size {
                *best_node = Some(node);
                *best_size = size;
            } else if matches!(node.kind(), "expression_statement" | "block") 
                && size < *best_size && size <= 10 { // Only accept small expression_statements/blocks
                *best_node = Some(node);
                *best_size = size;
            }
            
            // Recurse into children
            if cursor.goto_first_child() {
                loop {
                    find_smallest_containing_node(cursor, target_row, best_node, best_size);
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
                cursor.goto_parent();
            }
        }
    }
    
    find_smallest_containing_node(&mut cursor, target_row, &mut best_node, &mut best_size);
    
    // If we found a suitable node, extract the complete construct
    if let Some(node) = best_node {
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        
        // Extract all lines of the construct
        let mut result = Vec::new();
        for line_num in start_line..=end_line {
            if line_num > 0 && line_num <= lines.len() {
                result.push((line_num, lines[line_num - 1].to_string()));
            }
        }
        result
    } else {
        // Fallback: return just the single line
        vec![(target_line, lines[target_line - 1].to_string())]
    }
}

fn collect_parent_context_for_line(file_path: &str, line_num: usize, source: &str) -> Vec<crate::models::ParentContext> {
    let mut contexts = Vec::new();
    
    // Get file extension and language implementation  
    let extension = file_extension(std::path::Path::new(file_path));
    let language_impl = match get_language_impl(extension) {
        Some(lang) => lang,
        None => return contexts, // Return empty if can't get language
    };
    
    // Parse the tree (using cached tree if available)
    let tree = match get_or_parse_tree_pooled(file_path, source, extension) {
        Ok(t) => t,
        Err(_) => return contexts, // Return empty if can't parse
    };
    
    let root_node = tree.root_node();
    
    // Find the node at the target line
    if let Some(target_node) = find_node_at_line(&root_node, line_num) {
        // For outline mode: Traverse ALL the way up to collect the complete hierarchy
        // This includes ALL structural parents (functions, loops, conditionals, etc.)
        let mut current = target_node;
        while let Some(parent) = current.parent() {
            let start_line = parent.start_position().row + 1;
            let end_line = parent.end_position().row + 1;
            
            // In outline mode, we want to show ALL structural parents, not just "suitable" ones
            // Include functions, methods, classes, loops, conditionals, match statements, etc.
            let should_include = matches!(parent.kind(),
                // Functions and methods
                "function_item" | "function_definition" | "method_definition" | 
                "function_declaration" | "method_declaration" | "function" |
                "func_literal" | "function_expression" | "arrow_function" |
                "closure_expression" | "lambda" |
                
                // Classes and structs
                "class_definition" | "class_declaration" | "struct_item" |
                "impl_item" | "trait_item" | "interface_declaration" |
                
                // Control flow
                "if_statement" | "if_expression" | "while_statement" | "while_expression" |
                "for_statement" | "for_expression" | "loop_statement" | "loop_expression" |
                "match_statement" | "match_expression" | "switch_statement" |
                "try_statement" | "try_expression" |
                
                // Match arms - show the whole arm as a unit
                "match_arm" | "switch_case" | "case_clause" |
                
                // Blocks (only if they're significant, not match arm patterns)
                "block" | "compound_statement" |
                
                // Async/concurrency
                "async_block" | "spawn_statement" | "go_statement"
            ) || language_impl.is_acceptable_parent(&parent);
            
            // Debug: log what node types we're considering
            if std::env::var("DEBUG").unwrap_or_default() == "1" {
                eprintln!("DEBUG: Considering parent: {} at line {} (included={})", parent.kind(), start_line, should_include);
            }
            
            if should_include {
                
                // Special handling for match arms - show the complete pattern
                if parent.kind() == "match_arm" {
                    // For match arms, we want to show the complete pattern up to "=> {"
                    // Find where the pattern ends (look for "=>")
                    let source_lines: Vec<&str> = source.lines().collect();
                    let mut pattern_end_line = start_line;
                    
                    for line_idx in start_line..=end_line.min(start_line + 20) {
                        if line_idx > 0 && line_idx <= source_lines.len() {
                            let line = source_lines[line_idx - 1];
                            if line.contains("=>") {
                                pattern_end_line = line_idx;
                                break;
                            }
                        }
                    }
                    
                    // Add all lines of the match arm pattern
                    for line_num in start_line..=pattern_end_line {
                        if line_num > 0 && line_num <= source_lines.len() {
                            let already_exists = contexts.iter().any(|existing| existing.start_line == line_num);
                            if !already_exists {
                                contexts.push(crate::models::ParentContext {
                                    node_type: "match_arm_pattern".to_string(),
                                    start_line: line_num,
                                    end_line: pattern_end_line,
                                    context_line: source_lines[line_num - 1].to_string(),
                                    preceding_comments: if line_num == start_line {
                                        find_preceding_comments(&parent, source, start_line)
                                    } else {
                                        Vec::new()
                                    },
                                });
                            }
                        }
                    }
                } else {
                    // Get the first line of the parent block for context
                    if let Some(context_line) = source.lines().nth(parent.start_position().row) {
                        // Check if we already have a context at this line number
                        let already_exists = contexts.iter().any(|existing| existing.start_line == start_line);
                        
                        if !already_exists {
                            let preceding_comments = find_preceding_comments(&parent, source, start_line);
                            contexts.push(crate::models::ParentContext {
                                node_type: parent.kind().to_string(),
                                start_line,
                                end_line,
                                context_line: context_line.to_string(), // Keep original indentation
                                preceding_comments,
                            });
                        }
                    }
                }
            }
            current = parent;
        }
    }
    
    // Reverse to get outermost parent first (root -> nested)
    contexts.reverse();
    contexts
}

/// Line type for outline display
#[derive(Debug, Clone, Copy, PartialEq)]
enum OutlineLineType {
    ParentContext,  // Should be dimmed
    FunctionSignature,  // Not dimmed
    NestedContext,  // Should be dimmed
    MatchedLine,  // Not dimmed (will be highlighted)
    ClosingBrace,  // Should be dimmed
}

/// Collect all lines to display for outline format with their types
fn collect_outline_lines(
    result: &SearchResult,
    file_path: &str,
) -> Vec<(usize, OutlineLineType)> {
    let mut lines = Vec::new();
    
    // Read the full source file
    let full_source = match std::fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(_) => return lines,
    };
    
    // Debug: Check if we have matched lines
    if std::env::var("DEBUG").unwrap_or_default() == "1" {
        eprintln!("DEBUG: collect_outline_lines for result at lines {}-{}", result.lines.0, result.lines.1);
        eprintln!("DEBUG: matched_lines field = {:?}", result.matched_lines);
        eprintln!("DEBUG: matched_keywords = {:?}", result.matched_keywords);
    }
    
    // Collect matched lines - if we don't have specific matched_lines, find them in the result
    let matched_lines: Vec<usize> = if let Some(matched_line_indices) = &result.matched_lines {
        if !matched_line_indices.is_empty() {
            // Convert matched line indices to absolute line numbers
            matched_line_indices
                .iter()
                .map(|&idx| result.lines.0 + idx)
                .collect()
        } else {
            // If matched_lines is empty, scan the result for actual matches
            let mut found_lines = Vec::new();
            if let Some(keywords) = &result.matched_keywords {
                let result_lines: Vec<&str> = result.code.lines().collect();
                for (idx, line) in result_lines.iter().enumerate() {
                    for keyword in keywords {
                        if line.to_lowercase().contains(&keyword.to_lowercase()) {
                            found_lines.push(result.lines.0 + idx);
                            break;
                        }
                    }
                }
            }
            if found_lines.is_empty() {
                vec![result.lines.0]
            } else {
                found_lines
            }
        }
    } else {
        // No matched_lines field - scan the result for actual matches
        let mut found_lines = Vec::new();
        if let Some(keywords) = &result.matched_keywords {
            let result_lines: Vec<&str> = result.code.lines().collect();
            for (idx, line) in result_lines.iter().enumerate() {
                for keyword in keywords {
                    if line.to_lowercase().contains(&keyword.to_lowercase()) {
                        found_lines.push(result.lines.0 + idx);
                        break;
                    }
                }
            }
        }
        if found_lines.is_empty() {
            vec![result.lines.0]
        } else {
            found_lines
        }
    };
    
    if std::env::var("DEBUG").unwrap_or_default() == "1" {
        eprintln!("DEBUG: Found matched lines: {:?}", matched_lines);
    }
    
    if !matched_lines.is_empty() {
            
            // Collect parent contexts for all matched lines
            let mut all_contexts = Vec::new();
            for &line_num in &matched_lines {
                let contexts = collect_parent_context_for_line(file_path, line_num, &full_source);
                if std::env::var("DEBUG").unwrap_or_default() == "1" {
                    eprintln!("DEBUG: Parent contexts for line {}: {} contexts found", line_num, contexts.len());
                    for ctx in &contexts {
                        eprintln!("  - {} at line {}", ctx.node_type, ctx.start_line);
                    }
                }
                all_contexts.push((line_num, contexts));
            }
            
            // Find shared parent contexts (common to all matched lines)
            let shared_contexts = if let Some((_, first_contexts)) = all_contexts.first() {
                find_shared_parent_context(first_contexts, &all_contexts)
            } else {
                Vec::new()
            };
            
            // Add parent context lines that come BEFORE the function
            for context in &shared_contexts {
                if context.start_line < result.lines.0 {
                    // Add comment lines
                    for (comment_start, comment_end, _) in &context.preceding_comments {
                        for line in *comment_start..=*comment_end {
                            lines.push((line, OutlineLineType::ParentContext));
                        }
                    }
                    // Add the context line itself
                    lines.push((context.start_line, OutlineLineType::ParentContext));
                }
            }
            
            // Add the function signature lines (from start to opening brace or a few lines)
            let source_lines: Vec<&str> = full_source.lines().collect();
            let mut sig_end_line = result.lines.0;
            
            // Find where the signature ends (at opening brace or after params)
            for offset in 0..10.min(result.lines.1 - result.lines.0 + 1) {
                let line_idx = result.lines.0 + offset - 1;
                if line_idx < source_lines.len() {
                    let line = source_lines[line_idx];
                    sig_end_line = result.lines.0 + offset;
                    if line.contains('{') || (offset > 0 && source_lines[line_idx - 1].contains(')') && !line.trim_start().starts_with("->")) {
                        break;
                    }
                }
            }
            
            // Add function signature lines
            for line in result.lines.0..=sig_end_line {
                lines.push((line, OutlineLineType::FunctionSignature));
            }
            
            // Add ALL parent contexts from all matched lines (not just shared ones)
            // For outline format, we want to show the complete context for each match
            let mut all_nested_contexts = std::collections::HashSet::new();
            for (_, contexts) in &all_contexts {
                for context in contexts {
                    if context.start_line > result.lines.0 && context.start_line <= result.lines.1 {
                        // Skip generic block nodes if we have more specific ones like if_expression
                        if context.node_type == "block" || context.node_type == "compound_statement" {
                            // Check if there's a more specific node at the same line
                            let has_specific = contexts.iter().any(|c| 
                                c.start_line == context.start_line && 
                                c.node_type != "block" && 
                                c.node_type != "compound_statement"
                            );
                            if has_specific {
                                continue;
                            }
                        }
                        all_nested_contexts.insert(context.start_line);
                        lines.push((context.start_line, OutlineLineType::NestedContext));
                    }
                }
            }
            
            // Add the actual matched lines
            for &line_num in &matched_lines {
                // Find the complete construct for this line
                let construct_lines = find_complete_construct_for_line(file_path, line_num, &full_source);
                if std::env::var("DEBUG").unwrap_or_default() == "1" {
                    eprintln!("DEBUG: Complete construct for line {}: found {} lines", line_num, construct_lines.len());
                    for (cl, _) in &construct_lines {
                        eprintln!("  - Line {}", cl);
                    }
                }
                if !construct_lines.is_empty() {
                    for (construct_line, _) in construct_lines {
                        lines.push((construct_line, OutlineLineType::MatchedLine));
                    }
                } else {
                    lines.push((line_num, OutlineLineType::MatchedLine));
                }
            }
            
            // Add closing brace if needed
            if let Some(last_context) = shared_contexts.iter()
                .filter(|c| c.start_line > result.lines.0)
                .max_by_key(|c| c.end_line) {
                if last_context.end_line <= result.lines.1 {
                    lines.push((last_context.end_line, OutlineLineType::ClosingBrace));
                }
            }
    }
    
    // Sort by line number and deduplicate (keeping the first type for each line)
    lines.sort_unstable_by_key(|(line, _)| *line);
    lines.dedup_by_key(|(line, _)| *line);
    lines
}

/// Render lines with proper gaps and ellipsis
fn render_outline_lines(
    lines: &[(usize, OutlineLineType)],
    file_path: &str,
    displayed_lines: &mut std::collections::HashSet<usize>,
    displayed_content: &mut Vec<String>,
    displayed_ellipsis_ranges: &mut Vec<(usize, usize)>,
    keywords: &Option<Vec<String>>,
) {
    if lines.is_empty() {
        return;
    }
    
    // Read the source file
    let source = match std::fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(_) => return,
    };
    let source_lines: Vec<&str> = source.lines().collect();
    
    let mut last_displayed = 0;
    
    for &(line_num, line_type) in lines {
        // Skip if already displayed
        if displayed_lines.contains(&line_num) {
            last_displayed = line_num;
            continue;
        }
        
        // Handle gap from last displayed line
        if last_displayed > 0 && line_num > last_displayed + 1 {
            let gap_size = line_num - last_displayed - 1;
            
            if gap_size < 5 {
                // Show actual lines for small gaps (dimmed)
                for gap_line in (last_displayed + 1)..line_num {
                    if gap_line > 0 && gap_line <= source_lines.len() {
                        print_line_once(gap_line, source_lines[gap_line - 1], displayed_lines, displayed_content, true);
                    }
                }
            } else {
                // Show ellipsis for larger gaps
                print_ellipsis_once(last_displayed + 1, line_num - 1, displayed_ellipsis_ranges);
            }
        }
        
        // Display the line with optional highlighting
        if line_num > 0 && line_num <= source_lines.len() {
            let mut line_content = source_lines[line_num - 1].to_string();
            
            // Apply keyword highlighting only for matched lines
            if line_type == OutlineLineType::MatchedLine {
                if let Some(keywords) = keywords {
                    for keyword in keywords {
                        let pattern = if keyword.starts_with('"') && keyword.ends_with('"') {
                            regex::escape(&keyword[1..keyword.len()-1])
                        } else {
                            format!(r"(?i){}", regex::escape(keyword))
                        };
                        if let Ok(re) = Regex::new(&format!("({})", pattern)) {
                            line_content = re.replace_all(&line_content, |caps: &regex::Captures| {
                                use colored::*;
                                caps[1].bright_yellow().bold().to_string()
                            }).to_string();
                        }
                    }
                }
            }
            
            // Determine if line should be dimmed based on its type
            let should_dim = match line_type {
                OutlineLineType::ParentContext => true,
                OutlineLineType::FunctionSignature => false,
                OutlineLineType::NestedContext => true,
                OutlineLineType::MatchedLine => false,
                OutlineLineType::ClosingBrace => true,
            };
            
            print_line_once(line_num, &line_content, displayed_lines, displayed_content, should_dim);
        }
        
        last_displayed = line_num;
    }
}

/// Find shared parent context that is common to all matched lines
fn find_shared_parent_context(
    first_contexts: &[crate::models::ParentContext],
    all_line_contexts: &[(usize, Vec<crate::models::ParentContext>)]
) -> Vec<crate::models::ParentContext> {
    let mut shared = Vec::new();
    
    // Find the shortest context list to avoid index out of bounds
    let min_contexts = all_line_contexts.iter()
        .map(|(_, contexts)| contexts.len())
        .min()
        .unwrap_or(0);
    
    // Check each context level from outermost to innermost
    for i in 0..min_contexts.min(first_contexts.len()) {
        let candidate = &first_contexts[i];
        
        // Check if this context is common to ALL matched lines
        let is_shared = all_line_contexts.iter().all(|(_, contexts)| {
            contexts.get(i).map_or(false, |ctx| {
                ctx.start_line == candidate.start_line && 
                ctx.node_type == candidate.node_type
            })
        });
        
        if is_shared {
            shared.push(candidate.clone());
        } else {
            // Stop at the first non-shared context
            break;
        }
    }
    
    shared
}

/// Find the deepest tree-sitter node at a specific line number using cursor traversal
fn find_node_at_line<'a>(node: &'a Node<'a>, target_line: usize) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let mut best_match = None;
    let mut best_depth = 0;
    
    // Use cursor to avoid lifetime issues
    traverse_for_line(&mut cursor, target_line, &mut best_match, &mut best_depth, 0);
    
    best_match
}

/// Helper function to traverse tree with cursor and find the deepest node at target line
fn traverse_for_line<'a>(
    cursor: &mut tree_sitter::TreeCursor<'a>,
    target_line: usize,
    best_match: &mut Option<Node<'a>>,
    best_depth: &mut usize,
    current_depth: usize
) {
    let node = cursor.node();
    let node_start = node.start_position().row + 1;
    let node_end = node.end_position().row + 1;
    
    // Check if this node contains the target line
    if node_start <= target_line && target_line <= node_end {
        // This is a candidate - check if it's deeper than current best
        if current_depth > *best_depth {
            *best_match = Some(node);
            *best_depth = current_depth;
        }
        
        // Traverse children
        if cursor.goto_first_child() {
            loop {
                traverse_for_line(cursor, target_line, best_match, best_depth, current_depth + 1);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }
}

/// Check if a node represents a contextual parent (control structures, functions, etc.)
fn is_contextual_parent(node: &Node, language_impl: &Box<dyn LanguageImpl>, _source: &[u8]) -> bool {
    let node_kind = node.kind();
    
    // Language-agnostic contextual structures - but exclude simple control flow
    if matches!(node_kind, 
        // Only include loop structures and complex control flow, not simple if statements
        "while_statement" | "for_statement" | "loop_statement" |
        "match_statement" | "match_expression" | "switch_statement" | "try_statement" |
        
        // Code blocks  
        "block" | "compound_statement" |
        
        // Async/concurrency
        "async_block" | "spawn_statement" | "go_statement" |
        
        // Closures and lambdas
        "closure_expression" | "lambda" | "arrow_function"
    ) {
        return true;
    }
    
    // Special handling for if statements - only include them if they're substantial
    if matches!(node_kind, "if_statement" | "if_expression") {
        // Only consider if statements as contextual parents if they span multiple lines
        // and contain significant code (more than just a single simple statement)
        let node_lines = node.end_position().row - node.start_position().row + 1;
        
        // If the if statement spans more than 3 lines, it's likely significant enough
        // to be shown as context (e.g., complex conditional blocks)
        if node_lines > 3 {
            return true;
        }
        
        // Otherwise, don't treat simple if statements as contextual parents
        return false;
    }
    
    // Use language-specific acceptable parent check for top-level items
    language_impl.is_acceptable_parent(node)
}

/// Centralized function to print a line with deduplication
fn print_line_once(
    line_num: usize,
    content: &str,
    displayed_lines: &mut std::collections::HashSet<usize>,
    displayed_content: &mut Vec<String>,
    is_dimmed: bool,
) {
    // Only print if we haven't displayed this line already
    if !displayed_lines.contains(&line_num) {
        use colored::*;
        if is_dimmed {
            println!("{:<4} {}", line_num, content.dimmed());
        } else {
            println!("{:<4} {}", line_num, content);
        }
        displayed_lines.insert(line_num);
        displayed_content.push(content.to_string());
    }
}

/// Centralized function to print ellipsis with deduplication
/// Tracks ranges where ellipsis have been printed to prevent duplicates
fn print_ellipsis_once(
    start_line: usize,
    end_line: usize,
    displayed_ellipsis_ranges: &mut Vec<(usize, usize)>,
) {
    // Check if we already have ellipsis covering this range
    let overlaps = displayed_ellipsis_ranges.iter().any(|&(existing_start, existing_end)| {
        // Check for overlap: ranges overlap if one starts before the other ends
        !(end_line < existing_start || start_line > existing_end)
    });
    
    if !overlaps {
        println!("...");
        displayed_ellipsis_ranges.push((start_line, end_line));
    }
}

/// Format and print search results in outline format
fn format_and_print_outline_results(results: &[&SearchResult], dry_run: bool, symbols: bool) {
    // Track actual content displayed for accurate token/byte counting
    let mut displayed_content = Vec::new();
    
    // Track all displayed lines (shared context + gap lines + matched lines)
    let mut displayed_lines = std::collections::HashSet::new();
    
    // Track ellipsis ranges to prevent duplicates
    let mut displayed_ellipsis_ranges: Vec<(usize, usize)> = Vec::new();
    
    use colored::*;
    
    if results.is_empty() {
        println!("{}", "No results found.".yellow().bold());
        return;
    }
    
    // Group results by file
    let mut current_file = String::new();
    let mut last_end_line = 0;
    
    for result in results {
        // If new file, print the file separator and name
        if result.file != current_file {
            if !current_file.is_empty() {
                println!(); // Empty line between files
            }
            println!("---");
            println!("File: {}", result.file);
            println!();
            current_file = result.file.clone();
            last_end_line = 0;
        }
        
        // If there's a gap from the last result, show ellipsis
        if last_end_line > 0 && result.lines.0 > last_end_line + 1 {
            print_ellipsis_once(last_end_line + 1, result.lines.0 - 1, &mut displayed_ellipsis_ranges);
        }
        
        if dry_run {
            // In dry-run mode, just show line numbers
            println!("{:<4} // Lines {}-{}", result.lines.0, result.lines.0, result.lines.1);
        } else if symbols {
            // For symbols mode, collect all lines to display and then render them
            if let Some(_symbol_signature) = &result.symbol_signature {
                // Phase 1: Collect all lines that need to be displayed
                let lines_to_display = collect_outline_lines(&result, &result.file);
                
                // Phase 2: Render the collected lines with proper gaps
                render_outline_lines(
                    &lines_to_display,
                    &result.file,
                    &mut displayed_lines,
                    &mut displayed_content,
                    &mut displayed_ellipsis_ranges,
                    &result.matched_keywords,
                );
                
                // Update tracking
                last_end_line = result.lines.1;
            } else {
                // Fallback if symbol_signature is missing - shouldn't happen
                println!("{:<4} {}", result.lines.0, result.code);
                last_end_line = result.lines.1;
            }
        } else {
            // Handle non-symbol results - just show the code block as-is
            println!("{}", result.code);
            last_end_line = result.lines.1;
        }
        
        last_end_line = result.lines.1;
    }
    
    // Calculate bytes and tokens based on actual displayed content
    let total_bytes: usize = displayed_content.iter().map(|s| s.len()).sum();
    
    // For token counting, use deduplication on the displayed content
    let content_blocks: Vec<&str> = displayed_content.iter().map(|s| s.as_str()).collect();
    let total_tokens = sum_tokens_with_deduplication(&content_blocks);
    
    println!();
    println!("Found {} search results", results.len());
    println!("Total bytes returned: {}", total_bytes);
    println!("Total tokens returned: {}", total_tokens);
}
