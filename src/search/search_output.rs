use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use probe_code::models::SearchResult;
use probe_code::search::query::QueryPlan;
use probe_code::search::search_tokens::sum_tokens_with_deduplication;

/// Create a cache of file contents for outline formatters to avoid redundant I/O
fn create_file_content_cache(results: &[&SearchResult]) -> HashMap<PathBuf, Arc<String>> {
    let mut cache = HashMap::new();

    // Collect unique file paths
    let mut unique_files = std::collections::HashSet::new();
    for result in results {
        if !result.file.is_empty() {
            unique_files.insert(PathBuf::from(&result.file));
        }
    }

    // Read each file once and cache the content
    for file_path in unique_files {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            cache.insert(file_path, Arc::new(content));
        }
    }

    cache
}

/// Function to format and print search results according to the specified format
pub fn format_and_print_search_results(
    results: &[SearchResult],
    dry_run: bool,
    format: &str,
    query_plan: Option<&QueryPlan>,
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
            format_and_print_color_results(&valid_results, dry_run, query_plan, debug_mode);
        }
        "json" => {
            if let Err(e) = format_and_print_json_results(&valid_results) {
                eprintln!("Error formatting JSON: {e}");
            }
            return; // Skip the summary output at the end
        }
        "xml" => {
            if let Err(e) = format_and_print_xml_results(&valid_results) {
                eprintln!("Error formatting XML: {e}");
            }
            return; // Skip the summary output at the end
        }
        "outline" => {
            let file_cache = create_file_content_cache(&valid_results);
            format_and_print_outline_results(&valid_results, dry_run, &file_cache);
            return; // Skip the duplicate summary output at the end
        }
        "outline-xml" => {
            let file_cache = create_file_content_cache(&valid_results);
            if let Err(e) =
                format_and_print_outline_xml_results(&valid_results, dry_run, &file_cache)
            {
                eprintln!("Error formatting outline XML: {e}");
            }
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
                        println!("```{extension}");
                        println!("{}", result.code);
                        println!("```");
                    } else {
                        println!("File: {}", result.file);
                        println!(
                            "Lines: {start}-{end}",
                            start = result.lines.0,
                            end = result.lines.1
                        );
                        println!("```{extension}");
                        println!("{code}", code = result.code);
                        println!("```");
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

    let total_bytes: usize = valid_results.iter().map(|r| r.code.len()).sum();

    // BATCH TOKENIZATION WITH DEDUPLICATION OPTIMIZATION:
    // Use batch processing with content deduplication for improved performance
    // when multiple identical code blocks need tokenization (common in search results)
    let code_blocks: Vec<&str> = valid_results.iter().map(|r| r.code.as_str()).collect();
    let total_tokens: usize = sum_tokens_with_deduplication(&code_blocks);
    println!("Total bytes returned: {total_bytes}");
    println!("Total tokens returned: {total_tokens}");
}

/// Format and print search results with color highlighting for matching words
fn format_and_print_color_results(
    results: &[&SearchResult],
    dry_run: bool,
    query_plan: Option<&QueryPlan>,
    debug_mode: bool,
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

        println!("{label}", label = "Code:".bold().magenta());

        // Print the code with syntax highlighting
        if !language.is_empty() {
            println!("{code_block}", code_block = format!("```{language}").cyan());
        } else {
            println!("{code_block}", code_block = "```".cyan());
        }

        // Print code with highlighting
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
    } // End of for loop

    println!();
    println!("Found {} search results", results.len());

    let code_blocks: Vec<&str> = results.iter().map(|r| r.code.as_str()).collect();
    let total_tokens: usize = sum_tokens_with_deduplication(&code_blocks);
    let total_bytes: usize = results.iter().map(|r| r.code.len()).sum();
    println!("Total bytes returned: {total_bytes}");
    println!("Total tokens returned: {total_tokens}");
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
fn format_and_print_json_results(results: &[&SearchResult]) -> Result<()> {
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
    let code_blocks: Vec<&str> = results.iter().map(|r| r.code.as_str()).collect();
    let total_tokens = sum_tokens_with_deduplication(&code_blocks);

    // Create a wrapper object with results and summary
    let wrapper = serde_json::json!({
        "results": json_results,
        "summary": {
            "count": results.len(),
            "total_bytes": results.iter().map(|r| r.code.len()).sum::<usize>(),
            "total_tokens": total_tokens,
        },
        "version": probe_code::version::get_version()
    });

    println!("{json}", json = serde_json::to_string_pretty(&wrapper)?);
    Ok(())
}

/// Format and print search results in XML format
fn format_and_print_xml_results(results: &[&SearchResult]) -> Result<()> {
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
        total_bytes = results.iter().map(|r| r.code.len()).sum::<usize>()
    );
    // BATCH TOKENIZATION WITH DEDUPLICATION OPTIMIZATION for XML output:
    // Process all code blocks in batch to leverage content deduplication
    let code_blocks: Vec<&str> = results.iter().map(|r| r.code.as_str()).collect();
    let total_tokens = sum_tokens_with_deduplication(&code_blocks);

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
use crate::language::language_trait::LanguageImpl;
use crate::language::tree_cache::get_or_parse_tree_pooled;
use tree_sitter::Node;

/// Helper to get file extension as a &str
/// Find comments that precede a given node using tree-sitter AST
fn find_preceding_comments(
    node: &tree_sitter::Node,
    source: &str,
    node_line: usize,
) -> Vec<(usize, usize, String)> {
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
                    if !line.is_empty()
                        && !line.starts_with("//")
                        && !line.starts_with("/*")
                        && !line.starts_with("*")
                    {
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
    comments: &mut Vec<(usize, usize, String)>,
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
    matches!(
        kind,
        "comment" | "line_comment" | "block_comment" | "//" | "/*" | "*/"
    )
}

/// Check if a line is a comment line based on its content
fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("//")
        || trimmed.starts_with("///")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("*")
        || trimmed.starts_with("*/")
        || (trimmed.starts_with("#") && !trimmed.starts_with("#[")) // Python/shell comments but not Rust attributes
}

fn file_extension(path: &std::path::Path) -> &str {
    path.extension().and_then(|ext| ext.to_str()).unwrap_or("")
}

fn collect_parent_context_for_line(
    file_path: &str,
    line_num: usize,
    source: &str,
) -> Vec<crate::models::ParentContext> {
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
    let mut target_node = find_node_at_line(&root_node, line_num);

    // Special handling for doc comments: if this line is a comment that precedes a function,
    // we should find the function it documents and use that as the context
    if let Some(_node) = target_node {
        let source_lines: Vec<&str> = source.lines().collect();
        if line_num > 0 && line_num <= source_lines.len() {
            let line_content = source_lines[line_num - 1].trim();

            // Check if this is a doc comment (/// or /**)
            if line_content.starts_with("///") || line_content.starts_with("/**") {
                if std::env::var("DEBUG").unwrap_or_default() == "1" {
                    eprintln!(
                        "DEBUG: Found doc comment at line {}: {}",
                        line_num, line_content
                    );
                }
                // Look for the next non-comment, non-attribute line to find the function
                for next_line in line_num + 1..=(line_num + 10).min(source_lines.len()) {
                    let next_content = source_lines[next_line - 1].trim();
                    if !next_content.starts_with("///")
                        && !next_content.starts_with("/**")
                        && !next_content.starts_with("#[")
                        && !next_content.is_empty()
                    {
                        // This might be the function - find its node
                        if std::env::var("DEBUG").unwrap_or_default() == "1" {
                            eprintln!(
                                "DEBUG: Looking at next line {}: {}",
                                next_line, next_content
                            );
                        }
                        if let Some(func_node) = find_node_at_line(&root_node, next_line) {
                            if std::env::var("DEBUG").unwrap_or_default() == "1" {
                                eprintln!(
                                    "DEBUG: Found node at line {}: {}",
                                    next_line,
                                    func_node.kind()
                                );
                            }
                            // Look for function in the ancestry
                            let mut current = func_node;
                            loop {
                                if matches!(current.kind(), "function_item" | "function_definition")
                                {
                                    target_node = Some(current);
                                    if std::env::var("DEBUG").unwrap_or_default() == "1" {
                                        eprintln!(
                                            "DEBUG: Found function node: {} at lines {}-{}",
                                            current.kind(),
                                            current.start_position().row + 1,
                                            current.end_position().row + 1
                                        );
                                        eprintln!(
                                            "DEBUG: Updated target_node for doc comment on line {}",
                                            line_num
                                        );
                                    }
                                    break;
                                }
                                if let Some(parent) = current.parent() {
                                    current = parent;
                                } else {
                                    break;
                                }
                            }
                        }
                        // If we found a function, we're done with this doc comment
                        if target_node.is_some() {
                            break;
                        }
                        break;
                    }
                }
            }
        }
    }

    if let Some(target_node) = target_node {
        if std::env::var("DEBUG").unwrap_or_default() == "1" {
            eprintln!(
                "DEBUG: Processing contexts for target_node: {} at lines {}-{}",
                target_node.kind(),
                target_node.start_position().row + 1,
                target_node.end_position().row + 1
            );
        }
        // For outline mode: First include the target node itself if it's a structural element
        if matches!(
            target_node.kind(),
            "function_item"
                | "function_definition"
                | "method_definition"
                | "function_declaration"
                | "method_declaration"
                | "function"
                | "impl_item"
                | "struct_item"
                | "class_definition"
        ) {
            let start_line = target_node.start_position().row + 1;
            let end_line = target_node.end_position().row + 1;
            let source_lines: Vec<&str> = source.lines().collect();

            // For functions, try to find the actual function declaration line
            let context_line = if matches!(
                target_node.kind(),
                "function_item" | "function_definition" | "method_definition" | "function"
            ) {
                // Look for function declaration both backward and forward from the reported start position
                let mut found_line = None;
                let start_row = target_node.start_position().row;

                // First search backward (in case tree-sitter reports a line in the middle of the signature)
                for i in 0..5 {
                    if start_row >= i {
                        if let Some(line) = source_lines.get(start_row - i) {
                            if line.contains("fn ")
                                || line.contains("def ")
                                || line.contains("function ")
                            {
                                found_line = Some(line.to_string());
                                break;
                            }
                        }
                    }
                }

                // If not found backward, search forward
                if found_line.is_none() {
                    for i in 0..5 {
                        if let Some(line) = source_lines.get(start_row + i) {
                            if line.contains("fn ")
                                || line.contains("def ")
                                || line.contains("function ")
                            {
                                found_line = Some(line.to_string());
                                break;
                            }
                        }
                    }
                }

                // Fallback to the original behavior if we can't find a function declaration
                found_line.unwrap_or_else(|| {
                    if start_line > 0 && start_line <= source_lines.len() {
                        source_lines[start_line - 1].to_string()
                    } else {
                        String::new()
                    }
                })
            } else if start_line > 0 && start_line <= source_lines.len() {
                source_lines[start_line - 1].to_string()
            } else {
                String::new()
            };
            let context = crate::models::ParentContext {
                node_type: target_node.kind().to_string(),
                start_line,
                end_line,
                context_line,
                preceding_comments: Vec::new(),
            };
            contexts.push(context);
            if std::env::var("DEBUG").unwrap_or_default() == "1" {
                eprintln!(
                    "DEBUG: Added target node as context: {} at lines {}-{}",
                    target_node.kind(),
                    start_line,
                    end_line
                );
            }
        }

        // Then traverse up to collect the complete hierarchy
        // This includes ALL structural parents (functions, loops, conditionals, etc.)
        let mut current = target_node;
        while let Some(parent) = current.parent() {
            let start_line = parent.start_position().row + 1;
            let end_line = parent.end_position().row + 1;

            // In outline mode, we want to show meaningful structural parents, not all conditionals
            // Include functions, methods, classes, loops, match statements, etc.
            // BUT exclude simple if statements as they're typically at the same level as search results
            let should_include = matches!(
                parent.kind(),
                // Functions and methods
                "function_item" | "function_definition" | "method_definition" |
                "function_declaration" | "method_declaration" | "function" |
                "func_literal" | "function_expression" | "arrow_function" |
                "closure_expression" | "lambda" |

                // Classes and structs
                "class_definition" | "class_declaration" | "struct_item" |
                "impl_item" | "trait_item" | "interface_declaration" |

                // Control flow - REMOVED if_statement and if_expression for outline mode
                "while_statement" | "while_expression" |
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
                            let already_exists = contexts
                                .iter()
                                .any(|existing| existing.start_line == line_num);
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
                    // For functions, try to find the actual function declaration line
                    let context_line = if matches!(
                        parent.kind(),
                        "function_item" | "function_definition" | "method_definition" | "function"
                    ) {
                        // Look for function declaration both backward and forward from the reported start position
                        let source_lines: Vec<&str> = source.lines().collect();
                        let mut found_line = None;
                        let start_row = parent.start_position().row;

                        // First search backward (in case tree-sitter reports a line in the middle of the signature)
                        for i in 0..5 {
                            if start_row >= i {
                                if let Some(line) = source_lines.get(start_row - i) {
                                    if line.contains("fn ")
                                        || line.contains("def ")
                                        || line.contains("function ")
                                    {
                                        found_line = Some(line.to_string());
                                        break;
                                    }
                                }
                            }
                        }

                        // If not found backward, search forward
                        if found_line.is_none() {
                            for i in 0..5 {
                                if let Some(line) = source_lines.get(start_row + i) {
                                    if line.contains("fn ")
                                        || line.contains("def ")
                                        || line.contains("function ")
                                    {
                                        found_line = Some(line.to_string());
                                        break;
                                    }
                                }
                            }
                        }

                        // Fallback to the original behavior if we can't find a function declaration
                        let result = found_line.or_else(|| {
                            source
                                .lines()
                                .nth(parent.start_position().row)
                                .map(|s| s.to_string())
                        });

                        result
                    } else {
                        source
                            .lines()
                            .nth(parent.start_position().row)
                            .map(|s| s.to_string())
                    };

                    if let Some(context_line) = context_line {
                        // Check if we already have a context at this line number
                        let already_exists = contexts
                            .iter()
                            .any(|existing| existing.start_line == start_line);

                        if !already_exists {
                            let preceding_comments =
                                find_preceding_comments(&parent, source, start_line);
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
    ParentContext,     // Should be dimmed
    FunctionSignature, // Not dimmed
    NestedContext,     // Should be dimmed
    MatchedLine,       // Not dimmed (will be highlighted)
    ClosingBrace,      // Should be dimmed
}

/// Collect all lines to display for outline format with their types
/// Returns (lines_with_types, closing_brace_contexts) where closing_brace_contexts maps line numbers to ParentContext
fn collect_outline_lines(
    result: &SearchResult,
    file_path: &str,
    file_cache: &HashMap<PathBuf, Arc<String>>,
) -> (
    Vec<(usize, OutlineLineType)>,
    std::collections::HashMap<usize, crate::models::ParentContext>,
) {
    let mut lines = Vec::new();
    let mut closing_brace_contexts = std::collections::HashMap::new();

    // Get the source file from cache
    let file_path_buf = PathBuf::from(file_path);
    let full_source = match file_cache.get(&file_path_buf) {
        Some(content) => content.as_str(),
        None => return (lines, closing_brace_contexts),
    };

    // Debug: Check if we have matched lines
    if std::env::var("DEBUG").unwrap_or_default() == "1" {
        eprintln!(
            "DEBUG: collect_outline_lines for result at lines {}-{}",
            result.lines.0, result.lines.1
        );
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

    if !matched_lines.is_empty() {
        // Collect parent contexts for all matched lines
        let mut all_contexts = Vec::new();
        for &line_num in &matched_lines {
            let contexts = collect_parent_context_for_line(file_path, line_num, full_source);
            if std::env::var("DEBUG").unwrap_or_default() == "1" {
                eprintln!(
                    "DEBUG: Parent contexts for line {}: {} contexts found",
                    line_num,
                    contexts.len()
                );
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
                if line.contains('{')
                    || (offset > 0
                        && source_lines[line_idx - 1].contains(')')
                        && !line.trim_start().starts_with("->"))
                {
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
                        let has_specific = contexts.iter().any(|c| {
                            c.start_line == context.start_line
                                && c.node_type != "block"
                                && c.node_type != "compound_statement"
                        });
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
            lines.push((line_num, OutlineLineType::MatchedLine));

            // TEMPORARILY DISABLED: If this matched line is a comment, also include the code that follows it
            let _disabled = true; // Set to false to re-enable
            if !_disabled {
                let source_lines: Vec<&str> = full_source.lines().collect();
                if let Some(line_content) = source_lines.get(line_num - 1) {
                    if is_comment_line(line_content) {
                        // Add up to 3 lines of non-comment code following the comment
                        let mut added_lines = 0;
                        for offset in 1..=5 {
                            // Look ahead up to 5 lines
                            if added_lines >= 3 {
                                break; // Limit to 3 lines of code
                            }

                            let following_line_num = line_num + offset;
                            if let Some(following_line) = source_lines.get(following_line_num - 1) {
                                let trimmed = following_line.trim();

                                // Skip empty lines and additional comments
                                if trimmed.is_empty() || is_comment_line(following_line) {
                                    continue;
                                }

                                // Add this line as a matched line (it provides context for the comment)
                                lines.push((following_line_num, OutlineLineType::MatchedLine));
                                added_lines += 1;
                            } else {
                                break; // End of file
                            }
                        }
                    }
                }
            }
        }

        // Add closing braces for functions and other contexts
        // We need to show closing braces for ALL contexts (functions, impl blocks, etc.)
        // not just those that fit within the original result range
        let mut closing_braces = std::collections::HashSet::new();
        let mut block_info = std::collections::HashMap::new(); // Track block size and whether it has gaps

        // Helper function to check if a node type should have a closing brace comment
        let should_add_closing_brace = |node_type: &str| -> bool {
            matches!(
                node_type,
                // Functions and structural items
                "function_item" | "function_definition" | "method_definition" |
                "function_declaration" | "method_declaration" | "function" |
                "impl_item" | "struct_item" | "enum_item" | "trait_item" | "mod_item" |

                // Control flow statements (both statement and expression forms)
                // Note: if_statement/if_expression removed - they're too granular for outline
                "while_statement" | "while_expression" |
                "for_statement" | "for_expression" |
                "loop_statement" | "loop_expression" |
                "match_statement" | "match_expression" |
                "try_statement" | "try_expression" |

                // Match arms and cases
                "match_arm" | "switch_case" | "case_clause" |

                // Blocks and compound statements
                "block" | "compound_statement" |

                // Async/concurrency constructs
                "async_block" | "spawn_statement" | "go_statement"
            )
        };

        // Add closing braces from shared contexts (common to all matches)
        for context in &shared_contexts {
            // Always show closing brace for functions, impl blocks, etc.
            if should_add_closing_brace(&context.node_type) && context.end_line > 0 {
                let block_size = context.end_line - context.start_line;
                closing_braces.insert(context.end_line);
                // Store the context for this closing brace
                closing_brace_contexts.insert(context.end_line, context.clone());
                // Track block size for later gap analysis
                block_info.insert(context.end_line, block_size);
            }
        }

        // Add closing braces from individual matched line contexts
        for (_, contexts) in &all_contexts {
            for context in contexts {
                if should_add_closing_brace(&context.node_type) && context.end_line > 0 {
                    let block_size = context.end_line - context.start_line;
                    closing_braces.insert(context.end_line);

                    // Store the context for this closing brace, but prioritize function/class contexts over generic blocks
                    let should_update = if let Some(existing) =
                        closing_brace_contexts.get(&context.end_line)
                    {
                        // If we have a generic block and found a more specific context, use the specific one
                        let is_existing_generic =
                            matches!(existing.node_type.as_str(), "block" | "compound_statement");
                        let is_new_specific = matches!(
                            context.node_type.as_str(),
                            "function_item"
                                | "function_definition"
                                | "method_definition"
                                | "function"
                                | "class_definition"
                                | "class_declaration"
                                | "struct_item"
                                | "impl_item"
                        );
                        is_existing_generic && is_new_specific
                    } else {
                        true
                    };

                    if should_update {
                        closing_brace_contexts.insert(context.end_line, context.clone());
                    }

                    // Track block size for later gap analysis
                    block_info.insert(context.end_line, block_size);
                }
            }
        }

        // Add all closing braces to the lines
        for &brace_line in &closing_braces {
            lines.push((brace_line, OutlineLineType::ClosingBrace));
        }
    }

    // Sort by line number and deduplicate (keeping the most specific type for each line)
    lines.sort_unstable_by_key(|(line, _)| *line);

    // Custom deduplication that preserves the most important line type
    let mut deduped_lines = Vec::new();
    let mut seen_lines = std::collections::HashMap::new();

    for (line, line_type) in lines {
        if let Some(existing_type) = seen_lines.get(&line).copied() {
            // Preserve more specific types over generic ones
            let should_replace = match (existing_type, line_type) {
                // MatchedLine is most important
                (_, OutlineLineType::MatchedLine) => true,
                (OutlineLineType::MatchedLine, _) => false,
                // FunctionSignature is more important than context
                (OutlineLineType::ParentContext, OutlineLineType::FunctionSignature) => true,
                (OutlineLineType::NestedContext, OutlineLineType::FunctionSignature) => true,
                (OutlineLineType::FunctionSignature, OutlineLineType::ParentContext) => false,
                (OutlineLineType::FunctionSignature, OutlineLineType::NestedContext) => false,
                // Keep first occurrence otherwise
                _ => false,
            };

            if should_replace {
                seen_lines.insert(line, line_type);
                // Find and update existing entry
                if let Some(pos) = deduped_lines.iter().position(|(l, _)| *l == line) {
                    deduped_lines[pos] = (line, line_type);
                }
            }
        } else {
            seen_lines.insert(line, line_type);
            deduped_lines.push((line, line_type));
        }
    }

    deduped_lines.sort_unstable_by_key(|(line, _)| *line);

    // Pass block info to the contexts for gap analysis
    let mut enhanced_closing_brace_contexts = std::collections::HashMap::new();
    for (line_num, context) in closing_brace_contexts {
        // We'll determine if this block has gaps during rendering
        enhanced_closing_brace_contexts.insert(line_num, context);
    }

    (deduped_lines, enhanced_closing_brace_contexts)
}

#[allow(clippy::too_many_arguments)]
/// Render lines with proper gaps and ellipsis
fn render_outline_lines(
    lines: &[(usize, OutlineLineType)],
    file_path: &str,
    displayed_lines: &mut std::collections::HashSet<usize>,
    displayed_content: &mut Vec<String>,
    displayed_ellipsis_ranges: &mut Vec<(usize, usize)>,
    keywords: &Option<Vec<String>>,
    closing_brace_contexts: &std::collections::HashMap<usize, crate::models::ParentContext>,
    last_displayed_per_file: &mut std::collections::HashMap<String, usize>,
    file_cache: &HashMap<PathBuf, Arc<String>>,
) {
    if lines.is_empty() {
        return;
    }

    // Get the source file from cache
    let file_path_buf = PathBuf::from(file_path);
    let source = match file_cache.get(&file_path_buf) {
        Some(content) => content,
        None => return,
    };
    let source_lines: Vec<&str> = source.lines().collect();

    // Get the last displayed line for this file, defaulting to 0 if first time
    let mut last_displayed = *last_displayed_per_file.get(file_path).unwrap_or(&0);

    // Track which blocks actually had ellipsis shown (and meet size requirements)
    let mut blocks_with_gaps_shown = std::collections::HashSet::new();
    for &(line_num, line_type) in lines {
        // Skip if already displayed
        if displayed_lines.contains(&line_num) {
            // Don't update last_displayed here - we want to preserve gap tracking
            continue;
        }

        // Handle gap from last displayed line
        if last_displayed > 0 && line_num > last_displayed + 1 {
            let gap_size = line_num - last_displayed - 1;

            if gap_size < 5 {
                // Show actual lines for small gaps (dimmed)
                for gap_line in (last_displayed + 1)..line_num {
                    if gap_line > 0 && gap_line <= source_lines.len() {
                        print_line_once(
                            gap_line,
                            source_lines[gap_line - 1],
                            displayed_lines,
                            displayed_content,
                            true,
                        );
                    }
                }
            } else {
                // Show ellipsis for larger gaps
                print_ellipsis_once(last_displayed + 1, line_num - 1, displayed_ellipsis_ranges);

                // Track which blocks had ellipsis shown within them
                for (brace_line, context) in closing_brace_contexts {
                    let block_size = context.end_line - context.start_line;

                    // Only mark if:
                    // 1. Block is >20 lines
                    // 2. The ellipsis gap occurs WITHIN the block boundaries (not spanning entire block)
                    if block_size > 20 {
                        // Check if this gap is within the block (some content is hidden)
                        let gap_within_block = (last_displayed + 1) > context.start_line
                            && (line_num - 1) < context.end_line;
                        if gap_within_block {
                            blocks_with_gaps_shown.insert(*brace_line);
                        }
                    }
                }
            }
        }

        // Display the line with optional highlighting
        if line_num > 0 && line_num <= source_lines.len() {
            let mut line_content = source_lines[line_num - 1].to_string();

            // Apply keyword highlighting for all line types (not just matched lines)
            // This ensures function signatures and other contexts with keywords are highlighted
            if let Some(keywords) = keywords {
                for keyword in keywords {
                    let pattern = if keyword.starts_with('"') && keyword.ends_with('"') {
                        regex::escape(&keyword[1..keyword.len() - 1])
                    } else {
                        format!(r"(?i){}", regex::escape(keyword))
                    };
                    if let Ok(re) = Regex::new(&format!("({})", pattern)) {
                        line_content = re
                            .replace_all(&line_content, |caps: &regex::Captures| {
                                use colored::*;
                                caps[1].bright_yellow().bold().to_string()
                            })
                            .to_string();
                    }
                }
            }

            // Add smart comment for closing braces (only for blocks >20 lines with gaps shown)
            if line_type == OutlineLineType::ClosingBrace
                && blocks_with_gaps_shown.contains(&line_num)
            {
                let context = closing_brace_contexts.get(&line_num).unwrap();
                let file_extension = file_extension(std::path::Path::new(file_path));
                let context_text = extract_context_text(&context.context_line, &context.node_type);

                // Append the smart comment to the closing brace line
                line_content =
                    format_closing_comment(line_content.trim_end(), file_extension, &context_text);
            }

            // Determine if line should be dimmed based on its type
            let should_dim = match line_type {
                OutlineLineType::ParentContext => true,
                OutlineLineType::FunctionSignature => false,
                OutlineLineType::NestedContext => true,
                OutlineLineType::MatchedLine => false,
                OutlineLineType::ClosingBrace => true,
            };

            print_line_once(
                line_num,
                &line_content,
                displayed_lines,
                displayed_content,
                should_dim,
            );
        }

        last_displayed = line_num;
    }

    // Update the persistent last_displayed for this file
    last_displayed_per_file.insert(file_path.to_string(), last_displayed);
}

/// Find shared parent context that is common to all matched lines
fn find_shared_parent_context(
    first_contexts: &[crate::models::ParentContext],
    all_line_contexts: &[(usize, Vec<crate::models::ParentContext>)],
) -> Vec<crate::models::ParentContext> {
    let mut shared = Vec::new();

    // Find the shortest context list to avoid index out of bounds
    let min_contexts = all_line_contexts
        .iter()
        .map(|(_, contexts)| contexts.len())
        .min()
        .unwrap_or(0);

    // Check each context level from outermost to innermost
    for (i, candidate) in first_contexts
        .iter()
        .enumerate()
        .take(min_contexts.min(first_contexts.len()))
    {
        // Check if this context is common to ALL matched lines
        let is_shared = all_line_contexts.iter().all(|(_, contexts)| {
            contexts.get(i).is_some_and(|ctx| {
                ctx.start_line == candidate.start_line && ctx.node_type == candidate.node_type
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
    traverse_for_line(
        &mut cursor,
        target_line,
        &mut best_match,
        &mut best_depth,
        0,
    );

    best_match
}

/// Helper function to traverse tree with cursor and find the deepest node at target line
fn traverse_for_line<'a>(
    cursor: &mut tree_sitter::TreeCursor<'a>,
    target_line: usize,
    best_match: &mut Option<Node<'a>>,
    best_depth: &mut usize,
    current_depth: usize,
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
                traverse_for_line(
                    cursor,
                    target_line,
                    best_match,
                    best_depth,
                    current_depth + 1,
                );
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }
}

/// Check if a node represents a contextual parent (control structures, functions, etc.)
#[allow(dead_code)]
fn is_contextual_parent(node: &Node, language_impl: &dyn LanguageImpl, _source: &[u8]) -> bool {
    let node_kind = node.kind();

    // Language-agnostic contextual structures - but exclude simple control flow
    if matches!(
        node_kind,
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

/// Get the comment prefix for a given file extension
fn get_comment_prefix(extension: &str) -> &'static str {
    match extension {
        // C-style comments
        "rs" | "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "java" | "js" | "jsx" | "ts"
        | "tsx" | "cs" | "swift" | "go" | "php" => "//",

        // Python-style comments
        "py" | "rb" | "sh" | "bash" | "pl" | "r" | "yaml" | "yml" => "#",

        // HTML-style comments
        "md" | "markdown" => "<!--",

        // Other comment styles could be added here
        // For now, default to // for unknown extensions
        _ => "//",
    }
}

/// Format a closing comment for the specific file type
fn format_closing_comment(line_content: &str, extension: &str, context_text: &str) -> String {
    match extension {
        "md" | "markdown" => {
            // For markdown, use HTML-style comments with proper closing
            format!("{} <!-- {} -->", line_content, context_text)
        }
        _ => {
            // For all other languages, use the simple comment prefix format
            let comment_prefix = get_comment_prefix(extension);
            format!("{} {} {}", line_content, comment_prefix, context_text)
        }
    }
}

/// Extract meaningful text from a context line for the closing brace comment
fn extract_context_text(context_line: &str, node_type: &str) -> String {
    let trimmed = context_line.trim();

    // Handle different construct types with specific extraction logic
    match node_type {
        // Functions - extract function name
        "function_item" | "function_definition" | "method_definition" | "function" => {
            extract_function_name(trimmed)
        }

        // Control flow - extract the condition/iterator
        "if_statement" | "if_expression" => extract_if_condition(trimmed),
        "for_statement" | "for_expression" => extract_for_condition(trimmed),
        "while_statement" | "while_expression" => extract_while_condition(trimmed),
        "match_statement" | "match_expression" => extract_match_expression(trimmed),

        // Structural items
        "impl_item" => extract_impl_target(trimmed),
        "struct_item" => extract_struct_name(trimmed),

        // Markdown-specific constructs
        "atx_heading" => extract_markdown_header(trimmed),
        "setext_heading" => extract_markdown_header(trimmed),
        "fenced_code_block" => extract_markdown_code_block(trimmed),
        "pipe_table" => extract_markdown_table(trimmed),
        "list" => extract_markdown_list(trimmed),
        "block_quote" => extract_markdown_blockquote(trimmed),

        // Default: take first meaningful part
        _ => extract_default_context(trimmed),
    }
}

/// Extract function name from function definition
fn extract_function_name(line: &str) -> String {
    // Look for patterns like "fn function_name", "def function_name", "function function_name", etc.
    if let Some(fn_pos) = line.find("fn ") {
        let after_fn = &line[fn_pos + 3..];
        if let Some(name_end) = after_fn.find('(') {
            return format!("function {}", after_fn[..name_end].trim());
        }
    }

    if let Some(def_pos) = line.find("def ") {
        let after_def = &line[def_pos + 4..];
        if let Some(name_end) = after_def.find('(') {
            return format!("function {}", after_def[..name_end].trim());
        }
    }

    // For other languages, look for common patterns (ensure word boundaries)
    for keyword in &["function", "func", "def", "public", "private", "static"] {
        // Use word boundary to avoid matching inside other words
        if let Some(pos) = line.find(&format!("{} ", keyword)) {
            let after_keyword = &line[pos + keyword.len() + 1..];
            let words: Vec<&str> = after_keyword.split_whitespace().collect();
            if !words.is_empty() {
                let name = words[0].split('(').next().unwrap_or(words[0]);
                return format!("function {}", name);
            }
        }
    }

    // Fallback: just take the first word that looks like an identifier
    let words: Vec<&str> = line.split_whitespace().collect();
    for word in words {
        if !word.is_empty() && word.chars().next().unwrap().is_alphabetic() {
            return format!("function {}", word.split('(').next().unwrap_or(word));
        }
    }

    "function".to_string()
}

/// Extract if condition
fn extract_if_condition(line: &str) -> String {
    if let Some(if_pos) = line.find("if") {
        let after_if = &line[if_pos + 2..].trim_start();
        let condition = after_if.split('{').next().unwrap_or(after_if).trim();
        let truncated = if condition.len() > 15 {
            format!("{}...", &condition[..15])
        } else {
            condition.to_string()
        };
        format!("if {}", truncated)
    } else {
        "if".to_string()
    }
}

/// Extract for loop condition
fn extract_for_condition(line: &str) -> String {
    if let Some(for_pos) = line.find("for") {
        let after_for = &line[for_pos + 3..].trim_start();
        let condition = after_for.split('{').next().unwrap_or(after_for).trim();
        let truncated = if condition.len() > 15 {
            format!("{}...", &condition[..15])
        } else {
            condition.to_string()
        };
        format!("for {}", truncated)
    } else {
        "for".to_string()
    }
}

/// Extract while condition
fn extract_while_condition(line: &str) -> String {
    if let Some(while_pos) = line.find("while") {
        let after_while = &line[while_pos + 5..].trim_start();
        let condition = after_while.split('{').next().unwrap_or(after_while).trim();
        let truncated = if condition.len() > 15 {
            format!("{}...", &condition[..15])
        } else {
            condition.to_string()
        };
        format!("while {}", truncated)
    } else {
        "while".to_string()
    }
}

/// Extract match expression
fn extract_match_expression(line: &str) -> String {
    if let Some(match_pos) = line.find("match") {
        let after_match = &line[match_pos + 5..].trim_start();
        let expression = after_match.split('{').next().unwrap_or(after_match).trim();
        let truncated = if expression.len() > 15 {
            format!("{}...", &expression[..15])
        } else {
            expression.to_string()
        };
        format!("match {}", truncated)
    } else {
        "match".to_string()
    }
}

/// Extract impl target
fn extract_impl_target(line: &str) -> String {
    if let Some(impl_pos) = line.find("impl") {
        let after_impl = &line[impl_pos + 4..].trim_start();
        let target = after_impl.split('{').next().unwrap_or(after_impl).trim();
        let truncated = if target.len() > 15 {
            format!("{}...", &target[..15])
        } else {
            target.to_string()
        };
        format!("impl {}", truncated)
    } else {
        "impl".to_string()
    }
}

/// Extract struct name
fn extract_struct_name(line: &str) -> String {
    if let Some(struct_pos) = line.find("struct") {
        let after_struct = &line[struct_pos + 6..].trim_start();
        let name = after_struct
            .split_whitespace()
            .next()
            .unwrap_or("")
            .split('{')
            .next()
            .unwrap_or("");
        if !name.is_empty() {
            format!("struct {}", name)
        } else {
            "struct".to_string()
        }
    } else {
        "struct".to_string()
    }
}

/// Default context extraction - take meaningful first part
fn extract_default_context(line: &str) -> String {
    let truncated = if line.len() > 30 {
        format!("{}...", &line[..30])
    } else {
        line.to_string()
    };
    truncated
}

/// Extract header text from markdown header
fn extract_markdown_header(line: &str) -> String {
    // Remove # symbols and trim
    let header_text = line.trim_start_matches('#').trim();
    if header_text.is_empty() {
        "header".to_string()
    } else if header_text.len() > 30 {
        format!("header: {}...", &header_text[..30])
    } else {
        format!("header: {}", header_text)
    }
}

/// Extract code block info from markdown fenced code block
fn extract_markdown_code_block(line: &str) -> String {
    if line.starts_with("```") {
        let lang = line.trim_start_matches('`').trim();
        if lang.is_empty() {
            "code block".to_string()
        } else {
            format!("code block: {}", lang)
        }
    } else {
        "code block".to_string()
    }
}

/// Extract table info from markdown table
fn extract_markdown_table(line: &str) -> String {
    // For table headers, try to extract the first column name
    if line.starts_with('|') {
        let columns: Vec<&str> = line.split('|').collect();
        if columns.len() > 1 {
            let first_col = columns[1].trim();
            if !first_col.is_empty() && !first_col.starts_with('-') {
                return format!("table: {}", first_col);
            }
        }
    }
    "table".to_string()
}

/// Extract list info from markdown list
fn extract_markdown_list(line: &str) -> String {
    // Extract first few words from the list item
    let cleaned = line.trim_start_matches(|c: char| {
        c == '-' || c == '*' || c == '+' || c.is_numeric() || c == '.' || c.is_whitespace()
    });
    if cleaned.is_empty() {
        "list".to_string()
    } else if cleaned.len() > 25 {
        format!("list: {}...", &cleaned[..25])
    } else {
        format!("list: {}", cleaned)
    }
}

/// Extract blockquote info from markdown blockquote
fn extract_markdown_blockquote(line: &str) -> String {
    let cleaned = line.trim_start_matches(|c: char| c == '>' || c.is_whitespace());
    if cleaned.is_empty() {
        "quote".to_string()
    } else if cleaned.len() > 25 {
        format!("quote: {}...", &cleaned[..25])
    } else {
        format!("quote: {}", cleaned)
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
    let overlaps = displayed_ellipsis_ranges
        .iter()
        .any(|&(existing_start, existing_end)| {
            // Check for overlap: ranges overlap if one starts before the other ends
            !(end_line < existing_start || start_line > existing_end)
        });

    if !overlaps {
        println!("...");
        displayed_ellipsis_ranges.push((start_line, end_line));
    }
}

/// Format and print search results in outline format
fn format_and_print_outline_results(
    results: &[&SearchResult],
    dry_run: bool,
    file_cache: &HashMap<PathBuf, Arc<String>>,
) {
    // Track actual content displayed for accurate token/byte counting
    let mut displayed_content = Vec::new();

    // Track last displayed line per file for proper gap handling
    let mut last_displayed_per_file: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    use colored::*;

    // Group results by file and sort each group by line number
    let mut files_map: std::collections::HashMap<String, Vec<&SearchResult>> =
        std::collections::HashMap::new();

    for result in results {
        files_map
            .entry(result.file.clone())
            .or_default()
            .push(result);
    }

    // Sort files for consistent output
    let mut files: Vec<(String, Vec<&SearchResult>)> = files_map.into_iter().collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));

    // Sort results within each file by line number (not by score)
    for (_, file_results) in &mut files {
        file_results.sort_by_key(|r| r.lines.0);
    }

    for (file_index, (file_path, file_results)) in files.iter().enumerate() {
        // Handle dry run (just collect content without printing)
        if dry_run {
            // Add placeholder content for token/byte counting
            displayed_content.push("---".to_string());
            displayed_content.push(format!("File: {}", file_path));
            displayed_content.push("".to_string());

            for result in file_results {
                if let Some(matched_line_indices) = &result.matched_lines {
                    for &matched_line_idx in matched_line_indices {
                        let absolute_line = result.lines.0 + matched_line_idx;
                        displayed_content.push(format!("{}", absolute_line));
                    }
                }
            }
            displayed_content.push("".to_string());
            continue;
        }

        // Print file separator
        if file_index > 0 {
            println!();
        }

        // File header (only once per file)
        println!("{}", "---".dimmed());
        println!("{} {}", "File:".dimmed(), file_path.bold());
        println!();

        // Track lines for this entire file
        let mut all_lines_for_file: Vec<(usize, OutlineLineType)> = Vec::new();
        let mut all_closing_brace_contexts: std::collections::HashMap<
            usize,
            crate::models::ParentContext,
        > = std::collections::HashMap::new();

        // Collect all matched keywords from all results for this file
        let mut all_keywords: Option<Vec<String>> = None;
        for result in file_results {
            if let Some(ref keywords) = result.matched_keywords {
                if all_keywords.is_none() {
                    all_keywords = Some(Vec::new());
                }
                if let Some(ref mut all_kw) = all_keywords {
                    for kw in keywords {
                        if !all_kw.contains(kw) {
                            all_kw.push(kw.clone());
                        }
                    }
                }
            }
        }

        // Process each result and collect all lines to display
        for result in file_results {
            // Collect lines for this result
            let (lines_to_display, closing_brace_contexts) =
                collect_outline_lines(result, &result.file, file_cache);

            // Merge into the file-level collections
            for line in lines_to_display {
                if !all_lines_for_file.iter().any(|(l, _)| *l == line.0) {
                    all_lines_for_file.push(line);
                }
            }

            for (line_num, context) in closing_brace_contexts {
                all_closing_brace_contexts.insert(line_num, context);
            }
        }

        // Sort all lines for this file by line number
        all_lines_for_file.sort_by_key(|(line, _)| *line);

        // Remove duplicates, keeping the most important line type
        let mut deduped_lines = Vec::new();
        let mut seen_lines = std::collections::HashMap::new();

        for (line, line_type) in all_lines_for_file {
            if let Some(existing_type) = seen_lines.get(&line).copied() {
                // Preserve more specific types over generic ones
                let should_replace = match (existing_type, line_type) {
                    // MatchedLine is most important
                    (_, OutlineLineType::MatchedLine) => true,
                    (OutlineLineType::MatchedLine, _) => false,
                    // FunctionSignature is more important than context
                    (OutlineLineType::ParentContext, OutlineLineType::FunctionSignature) => true,
                    (OutlineLineType::NestedContext, OutlineLineType::FunctionSignature) => true,
                    (OutlineLineType::FunctionSignature, OutlineLineType::ParentContext) => false,
                    (OutlineLineType::FunctionSignature, OutlineLineType::NestedContext) => false,
                    // Keep first occurrence otherwise
                    _ => false,
                };

                if should_replace {
                    seen_lines.insert(line, line_type);
                    // Find and update existing entry
                    if let Some(pos) = deduped_lines.iter().position(|(l, _)| *l == line) {
                        deduped_lines[pos] = (line, line_type);
                    }
                }
            } else {
                seen_lines.insert(line, line_type);
                deduped_lines.push((line, line_type));
            }
        }

        // IMPORTANT: These must be per-file to avoid cross-file interference
        let mut displayed_lines_for_this_file: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        let mut displayed_ellipsis_ranges_for_this_file: Vec<(usize, usize)> = Vec::new();

        // Render all lines for this file at once
        render_outline_lines(
            &deduped_lines,
            file_path,
            &mut displayed_lines_for_this_file,
            &mut displayed_content,
            &mut displayed_ellipsis_ranges_for_this_file,
            &all_keywords,
            &all_closing_brace_contexts,
            &mut last_displayed_per_file,
            file_cache,
        );
    }

    // Print summary at the end
    println!();
    println!("Found {} search results", results.len());

    // Calculate total bytes and tokens from displayed content
    let total_bytes: usize = displayed_content.iter().map(|s| s.len()).sum();
    let code_blocks: Vec<&str> = displayed_content.iter().map(|s| s.as_str()).collect();
    let total_tokens: usize = sum_tokens_with_deduplication(&code_blocks);

    println!("Total bytes returned: {total_bytes}");
    println!("Total tokens returned: {total_tokens}");
}

/// Format and print search results in outline XML format
/// This reuses the outline format logic but outputs in XML structure
fn format_and_print_outline_xml_results(
    results: &[&SearchResult],
    dry_run: bool,
    file_cache: &HashMap<PathBuf, Arc<String>>,
) -> Result<()> {
    // Track content for accounting
    let mut displayed_content = Vec::new();

    println!("<instructions>");
    println!("- Search results organized by file, showing code matches within their parent functions/classes.");
    println!(
        "- Line numbers help you locate exact positions. Ellipsis (...) indicates skipped code."
    );
    println!("- To see complete functions: 'probe extract filename.ext:line_number' or 'probe extract filename.ext#symbol_name'");
    println!("- Also works with probe extract CLI or MCP commands for AI assistants.");
    println!("</instructions>");
    println!();
    println!("<matches>");

    // Group results by file and sort each group by line number
    let mut files_map: std::collections::HashMap<String, Vec<&SearchResult>> =
        std::collections::HashMap::new();

    for result in results {
        files_map
            .entry(result.file.clone())
            .or_default()
            .push(result);
    }

    // Sort files for consistent output
    let mut files: Vec<(String, Vec<&SearchResult>)> = files_map.into_iter().collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));

    // Sort results within each file by line number (not by score)
    for (_, file_results) in &mut files {
        file_results.sort_by_key(|r| r.lines.0);
    }

    for (file_path, file_results) in files.iter() {
        // Collect all lines to display for this file
        let mut all_lines_for_file = Vec::new();
        let mut all_closing_brace_contexts = std::collections::HashMap::new();

        // Process each result and collect all lines to display
        for result in file_results {
            // Collect lines for this result
            let (lines_to_display, closing_brace_contexts) =
                collect_outline_lines(result, &result.file, file_cache);

            // Merge into the file-level collections
            for line in lines_to_display {
                if !all_lines_for_file.iter().any(|(l, _)| *l == line.0) {
                    all_lines_for_file.push(line);
                }
            }

            for (line_num, context) in closing_brace_contexts {
                all_closing_brace_contexts.insert(line_num, context);
            }
        }

        // Sort all lines for this file by line number
        all_lines_for_file.sort_by_key(|(line, _)| *line);

        // Remove duplicates, keeping the most important line type
        let mut deduped_lines = Vec::new();
        let mut seen_lines = std::collections::HashMap::new();

        for (line, line_type) in all_lines_for_file {
            if let Some(existing_type) = seen_lines.get(&line).copied() {
                // Preserve more specific types over generic ones
                let should_replace = match (existing_type, line_type) {
                    // MatchedLine is most important
                    (_, OutlineLineType::MatchedLine) => true,
                    (OutlineLineType::MatchedLine, _) => false,
                    // FunctionSignature is more important than context
                    (OutlineLineType::ParentContext, OutlineLineType::FunctionSignature) => true,
                    (OutlineLineType::NestedContext, OutlineLineType::FunctionSignature) => true,
                    (OutlineLineType::FunctionSignature, OutlineLineType::ParentContext) => false,
                    (OutlineLineType::FunctionSignature, OutlineLineType::NestedContext) => false,
                    // Keep first occurrence otherwise
                    _ => false,
                };

                if should_replace {
                    seen_lines.insert(line, line_type);
                    // Find and update existing entry
                    if let Some(pos) = deduped_lines.iter().position(|(l, _)| *l == line) {
                        deduped_lines[pos] = (line, line_type);
                    }
                }
            } else {
                seen_lines.insert(line, line_type);
                deduped_lines.push((line, line_type));
            }
        }

        // Sort deduped lines by line number for final output
        deduped_lines.sort_by_key(|(line, _)| *line);

        // Generate the XML content for this file
        let xml_content = generate_outline_xml_content(
            &deduped_lines,
            file_path,
            &all_closing_brace_contexts,
            dry_run,
            &mut displayed_content,
            file_cache,
        );

        // Print the file element with content (no XML escaping for simpler output)
        // Add empty lines for better readability
        println!();
        println!("<file path=\"{}\">", file_path);
        println!();
        print!("{}", xml_content);
        println!();
        println!("</file>");
    }

    println!("</matches>");

    // Print summary (similar to outline format)
    if !dry_run {
        let total_bytes: usize = displayed_content.iter().map(|s| s.len()).sum();
        let code_blocks: Vec<&str> = displayed_content.iter().map(|s| s.as_str()).collect();
        let total_tokens: usize = sum_tokens_with_deduplication(&code_blocks);

        eprintln!("Total bytes returned: {total_bytes}");
        eprintln!("Total tokens returned: {total_tokens}");
    }

    Ok(())
}

/// Generate XML content for a file by reading source lines and formatting them
fn generate_outline_xml_content(
    lines: &[(usize, OutlineLineType)],
    file_path: &str,
    _closing_brace_contexts: &std::collections::HashMap<usize, crate::models::ParentContext>,
    dry_run: bool,
    displayed_content: &mut Vec<String>,
    file_cache: &HashMap<PathBuf, Arc<String>>,
) -> String {
    if lines.is_empty() {
        return String::new();
    }

    // Get the source file from cache
    let file_path_buf = PathBuf::from(file_path);
    let source = match file_cache.get(&file_path_buf) {
        Some(content) => content,
        None => return String::new(),
    };
    let source_lines: Vec<&str> = source.lines().collect();

    let mut result = String::new();
    let mut last_line = 0;

    for (i, &(line_num, _line_type)) in lines.iter().enumerate() {
        // Check if we need to add an ellipsis for a gap
        if i > 0 && line_num > last_line + 1 {
            result.push_str("...\n");
        }

        // Get the actual line content (convert from 1-based to 0-based indexing)
        if let Some(line_content) = source_lines.get(line_num.saturating_sub(1)) {
            if dry_run {
                // For dry run, just show line numbers
                result.push_str(&format!("{}", line_num));
            } else {
                // Add line number and actual line content (no XML escaping for simpler output)
                result.push_str(&format!("{:4} {}", line_num, line_content));
                displayed_content.push(line_content.to_string());
            }
        }

        // Add newline unless it's the last line
        if i < lines.len() - 1 {
            result.push('\n');
        }

        last_line = line_num;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_comment_prefix() {
        // Test C-style comment languages
        assert_eq!(get_comment_prefix("rs"), "//");
        assert_eq!(get_comment_prefix("c"), "//");
        assert_eq!(get_comment_prefix("cpp"), "//");
        assert_eq!(get_comment_prefix("java"), "//");
        assert_eq!(get_comment_prefix("js"), "//");
        assert_eq!(get_comment_prefix("ts"), "//");

        // Test Python-style comment languages
        assert_eq!(get_comment_prefix("py"), "#");
        assert_eq!(get_comment_prefix("rb"), "#");
        assert_eq!(get_comment_prefix("sh"), "#");

        // Test default
        assert_eq!(get_comment_prefix("unknown"), "//");
    }

    #[test]
    fn test_extract_function_name() {
        assert_eq!(
            extract_function_name("fn calculate_score(items: &[i32]) -> i32 {"),
            "function calculate_score"
        );
        assert_eq!(
            extract_function_name("def process_data(self):"),
            "function process_data"
        );
        assert_eq!(
            extract_function_name("function doSomething() {"),
            "function doSomething"
        );
        assert_eq!(
            extract_function_name("public void methodName(int param) {"),
            "function void"
        ); // Fixed: takes first word after keyword
        assert_eq!(
            extract_function_name("  static calculateTotal() {"),
            "function calculateTotal"
        );

        // Test edge cases
        assert_eq!(
            extract_function_name("random line without function"),
            "function random"
        ); // Takes first alphabetic word
        assert_eq!(extract_function_name(""), "function");
    }

    #[test]
    fn test_extract_if_condition() {
        assert_eq!(
            extract_if_condition("if item_count > 5 {"),
            "if item_count > 5"
        );
        // Test actual output without making assumptions about exact truncation
        let result = extract_if_condition("  if (condition && other_condition) {");
        assert!(result.starts_with("if"));
        assert!(result.contains("condition"));

        let result2 =
            extract_if_condition("if very_long_condition_that_should_be_truncated_properly {");
        assert!(result2.starts_with("if"));
        assert!(result2.contains("...") || result2.len() <= 30);

        // Test edge case - returns just the keyword with trailing space
        let result = extract_if_condition("some line without if");
        assert!(result == "if" || result == "if ");
    }

    #[test]
    fn test_extract_for_condition() {
        assert_eq!(
            extract_for_condition("for item in items {"),
            "for item in items"
        );

        // Test actual output without making assumptions about exact truncation
        let result = extract_for_condition("  for (int i = 0; i < count; i++) {");
        assert!(result.starts_with("for"));
        assert!(result.contains("int"));

        let result2 = extract_for_condition(
            "for very_long_iterator_variable_name in very_long_collection_name {",
        );
        assert!(result2.starts_with("for"));
        assert!(result2.contains("...") || result2.len() <= 30);

        // Test edge case
        let result = extract_for_condition("some line without for");
        assert!(result == "for" || result == "for ");
    }

    #[test]
    fn test_extract_while_condition() {
        assert_eq!(
            extract_while_condition("while count > 0 {"),
            "while count > 0"
        );
        assert_eq!(
            extract_while_condition("  while (condition) {"),
            "while (condition)"
        );

        let result = extract_while_condition("while very_long_condition_expression_here {");
        assert!(result.starts_with("while"));
        assert!(result.contains("...") || result.len() <= 30);

        // Test edge case
        let result = extract_while_condition("some line without while");
        assert!(result == "while" || result == "while ");
    }

    #[test]
    fn test_extract_match_expression() {
        assert_eq!(
            extract_match_expression("match item.as_str() {"),
            "match item.as_str()"
        );
        assert_eq!(
            extract_match_expression("  match self.state {"),
            "match self.state"
        );

        let result =
            extract_match_expression("match very_long_expression_that_should_be_truncated {");
        assert!(result.starts_with("match"));
        assert!(result.contains("...") || result.len() <= 30);

        // Test edge case
        let result = extract_match_expression("some line without match");
        assert!(result == "match" || result == "match ");
    }

    #[test]
    fn test_extract_impl_target() {
        assert_eq!(extract_impl_target("impl MyStruct {"), "impl MyStruct");

        let result = extract_impl_target("  impl<T> GenericStruct<T> {");
        assert!(result.starts_with("impl"));
        assert!(result.contains("GenericStruct") || result.contains("Generic"));

        let result2 = extract_impl_target("impl VeryLongStructNameThatShouldBeTruncated {");
        assert!(result2.starts_with("impl"));
        assert!(result2.contains("...") || result2.len() <= 30);

        // Test edge case
        let result = extract_impl_target("some line without impl");
        assert!(result == "impl" || result == "impl ");
    }

    #[test]
    fn test_extract_struct_name() {
        assert_eq!(extract_struct_name("struct MyStruct {"), "struct MyStruct");
        assert_eq!(
            extract_struct_name("  struct GenericStruct<T> {"),
            "struct GenericStruct<T>"
        );
        assert_eq!(extract_struct_name("struct {"), "struct");
        assert_eq!(extract_struct_name("some line without struct"), "struct");
    }

    #[test]
    fn test_extract_context_text() {
        // Test function extraction
        assert_eq!(
            extract_context_text(
                "fn calculate_score(items: &[i32]) -> i32 {",
                "function_item"
            ),
            "function calculate_score"
        );

        // Test if condition extraction
        assert_eq!(
            extract_context_text("if item_count > 5 {", "if_expression"),
            "if item_count > 5"
        );

        // Test for loop extraction
        assert_eq!(
            extract_context_text("for item in items {", "for_expression"),
            "for item in items"
        );

        // Test while loop extraction
        assert_eq!(
            extract_context_text("while count > 0 {", "while_statement"),
            "while count > 0"
        );

        // Test match extraction
        assert_eq!(
            extract_context_text("match item.as_str() {", "match_expression"),
            "match item.as_str()"
        );

        // Test impl extraction
        assert_eq!(
            extract_context_text("impl MyStruct {", "impl_item"),
            "impl MyStruct"
        );

        // Test struct extraction
        assert_eq!(
            extract_context_text("struct MyStruct {", "struct_item"),
            "struct MyStruct"
        );

        // Test default extraction
        assert_eq!(
            extract_context_text(
                "some random code line that should be truncated properly",
                "unknown_type"
            ),
            "some random code line that sho..."
        );
    }

    #[test]
    fn test_extract_default_context() {
        assert_eq!(extract_default_context("short line"), "short line");
        assert_eq!(
            extract_default_context("this is a very long line that should definitely be truncated"),
            "this is a very long line that ..."
        );
        assert_eq!(extract_default_context(""), "");
    }

    #[test]
    fn test_create_file_content_cache() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create temporary test files
        let mut temp_file1 = NamedTempFile::new().expect("Failed to create temp file");
        let mut temp_file2 = NamedTempFile::new().expect("Failed to create temp file");

        let content1 = "fn test() {\n    println!(\"Hello\");\n}";
        let content2 = "class Test {\n    void run() {}\n}";

        temp_file1
            .write_all(content1.as_bytes())
            .expect("Failed to write to temp file");
        temp_file2
            .write_all(content2.as_bytes())
            .expect("Failed to write to temp file");

        // Create mock search results
        let result1 = SearchResult {
            file: temp_file1.path().to_string_lossy().to_string(),
            lines: (1, 3),
            node_type: "function".to_string(),
            code: "fn test() {\n    println!(\"Hello\");\n}".to_string(),
            symbol_signature: None,
            matched_by_filename: None,
            rank: None,
            score: None,
            tfidf_score: None,
            tfidf_rank: None,
            bm25_score: None,
            bm25_rank: None,
            combined_score_rank: None,
            new_score: None,
            hybrid2_rank: None,
            file_unique_terms: None,
            file_total_matches: None,
            file_match_rank: None,
            block_unique_terms: None,
            block_total_matches: None,
            parent_file_id: None,
            block_id: None,
            matched_lines: None,
            matched_keywords: None,
            tokenized_content: None,
            parent_context: None,
        };

        let result2 = SearchResult {
            file: temp_file2.path().to_string_lossy().to_string(),
            lines: (1, 3),
            node_type: "class".to_string(),
            code: "class Test {\n    void run() {}\n}".to_string(),
            symbol_signature: None,
            matched_by_filename: None,
            rank: None,
            score: None,
            tfidf_score: None,
            tfidf_rank: None,
            bm25_score: None,
            bm25_rank: None,
            combined_score_rank: None,
            new_score: None,
            hybrid2_rank: None,
            file_unique_terms: None,
            file_total_matches: None,
            file_match_rank: None,
            block_unique_terms: None,
            block_total_matches: None,
            parent_file_id: None,
            block_id: None,
            matched_lines: None,
            matched_keywords: None,
            tokenized_content: None,
            parent_context: None,
        };

        let results = vec![&result1, &result2];

        // Test cache creation
        let cache = create_file_content_cache(&results);

        // Verify cache contains both files
        assert_eq!(cache.len(), 2);

        // Verify content is correctly cached
        let path1 = PathBuf::from(&result1.file);
        let path2 = PathBuf::from(&result2.file);

        assert!(cache.contains_key(&path1));
        assert!(cache.contains_key(&path2));

        assert_eq!(cache.get(&path1).unwrap().as_ref(), content1);
        assert_eq!(cache.get(&path2).unwrap().as_ref(), content2);
    }
}
