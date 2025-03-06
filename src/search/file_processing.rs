use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::language::parse_file_for_code_blocks;
use crate::models::SearchResult;
use crate::ranking::preprocess_text;
use crate::search::file_search::get_filename_matched_queries_compat;
use crate::search::tokenization;

/// Parameters for file processing
pub struct FileProcessingParams<'a> {
    /// Path to the file
    pub path: &'a Path,
    /// Line numbers to process
    pub line_numbers: &'a HashSet<usize>,
    /// Whether to allow test files/functions
    pub allow_tests: bool,
    /// Map of query indices to matching line numbers
    pub term_matches: Option<&'a HashMap<usize, HashSet<usize>>>,
    /// Whether to include blocks matching any term (true) or all terms (false)
    pub any_term: bool,
    /// Total number of queries being searched
    pub num_queries: usize,
    /// Query indices that match the filename
    pub filename_matched_queries: HashSet<usize>,
    /// The query terms for calculating block matches
    pub queries_terms: &'a [Vec<(String, String)>],
    /// Optional preprocessed query terms for optimization
    pub preprocessed_queries: Option<&'a [Vec<String>]>,
    /// Whether to disable block merging
    pub no_merge: bool,
}

/// Function to check if a code block should be included based on term matches
fn filter_code_block(
    block_lines: (usize, usize),
    term_matches: &HashMap<usize, HashSet<usize>>,
    any_term: bool,
    num_queries: usize,
    filename_matched_queries: &HashSet<usize>, // New parameter for filename matches
    debug_mode: bool,                          // Added debug_mode parameter
) -> bool {
    // Note: For large files with many blocks, performance could be improved by
    // pre-computing term matches per line range instead of scanning term_matches
    // for each block. This optimization should be considered if performance
    // becomes an issue.

    let mut matched_queries = HashSet::new();

    // Check which queries have matches within the block's line range
    for (query_idx, lines) in term_matches {
        if lines
            .iter()
            .any(|&l| l >= block_lines.0 && l <= block_lines.1)
        {
            matched_queries.insert(*query_idx);
        }
    }

    // Calculate the number of unique terms in the block
    let block_unique_terms = matched_queries.len();

    // Determine if the block should be included based on term matches
    let term_match_criteria = if any_term {
        // Any term mode: include if any term matches in content
        // (we don't use filename matches in any_term mode to maintain precision)
        !matched_queries.is_empty()
    } else {
        // All terms mode: include if all queries are matched either in content or filename
        // AND at least one term is matched in the content
        !matched_queries.is_empty()
            && (0..num_queries)
                .all(|i| filename_matched_queries.contains(&i) || matched_queries.contains(&i))
    };

    // Filtering criteria with corrected formula:
    // 1 term: require 1 term
    // 2 terms: require 1 term
    // 3 terms: require 2 terms
    // 4 terms: require 2 terms
    // 5 terms: require 3 terms
    // 6 terms: require 3 terms
    // 7 terms: require 4 terms
    let min_required_terms = match num_queries {
        0 => 0,
        1 | 2 => 1,
        3 | 4 => 2,
        5 | 6 => 3,
        7 | 8 => 4,
        9 | 10 => 5,
        11 | 12 => 6,
        n => (n + 1) / 2, // General formula: ceil(n/2)
    };

    let unique_terms_criteria = block_unique_terms >= min_required_terms;

    // Final decision: both criteria must be met
    let should_include = term_match_criteria && unique_terms_criteria;

    // Add debug logging
    if debug_mode {
        println!(
            "DEBUG: Considering block at lines {}-{}",
            block_lines.0, block_lines.1
        );
        println!("DEBUG: Matched queries (indices): {:?}", matched_queries);
        println!("DEBUG: Block unique terms: {}", block_unique_terms);
        println!(
            "DEBUG: Total queries: {}, Min required terms: {}",
            num_queries, min_required_terms
        );

        if any_term {
            println!("DEBUG: Any-term mode: Include if any term matches");
        } else {
            println!("DEBUG: All-terms mode: Include if all {} queries matched (including filename matches: {:?}) AND at least one term is matched in content",
                     num_queries, filename_matched_queries);
        }

        println!("DEBUG: Term match criteria met: {}", term_match_criteria);
        println!(
            "DEBUG: Unique terms criteria met: {} (need {} of {} terms)",
            unique_terms_criteria, min_required_terms, num_queries
        );

        println!(
            "DEBUG: Block included: {} (Reason: {})",
            should_include,
            if should_include {
                "All criteria met"
            } else if !term_match_criteria {
                "Failed term match criteria"
            } else {
                "Insufficient unique terms"
            }
        );
    }

    should_include
}

/// Function to process a file that was matched by filename
pub fn process_file_by_filename(
    path: &Path,
    queries_terms: &[Vec<(String, String)>],
    preprocessed_queries: Option<&[Vec<String>]>, // Optional preprocessed query terms for optimization
) -> Result<SearchResult> {
    // Read the file content
    let content = fs::read_to_string(path).context(format!("Failed to read file: {:?}", path))?;

    // Get the filename for matching
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    // Use get_filename_matched_queries_compat to determine matched terms
    let matched_terms = get_filename_matched_queries_compat(&filename, queries_terms);

    // Create a SearchResult with filename match information
    let mut search_result = SearchResult {
        file: path.to_string_lossy().to_string(),
        lines: (1, content.lines().count()),
        node_type: "file".to_string(),
        code: content.clone(), // Clone content here to avoid the move
        matched_by_filename: Some(true),
        rank: None,
        score: None,
        tfidf_score: None,
        bm25_score: None,
        tfidf_rank: None,
        bm25_rank: None,
        new_score: None,
        hybrid2_rank: None,
        combined_score_rank: None,
        file_unique_terms: Some(matched_terms.len()),
        file_total_matches: Some(0),
        file_match_rank: None,
        block_unique_terms: Some(matched_terms.len()),
        block_total_matches: Some(0),
        parent_file_id: None,
        block_id: None,
    };

    // Use preprocessed query terms if available
    if let Some(preprocessed) = preprocessed_queries {
        let query_terms: Vec<String> = preprocessed
            .iter()
            .flat_map(|terms| terms.iter().cloned())
            .collect();
        let unique_query_terms: HashSet<String> = query_terms.into_iter().collect();

        // Ensure we use the same stemming as query processing
        let block_terms = preprocess_text(&content, false);

        // Debug logging for stemming comparison
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
        if debug_mode {
            println!(
                "DEBUG: File by filename terms after stemming: {:?}",
                block_terms
            );
            println!(
                "DEBUG: Query terms after stemming: {:?}",
                unique_query_terms
            );
        }

        let block_unique_terms = if block_terms.is_empty() || unique_query_terms.is_empty() {
            0
        } else {
            // First, check for direct matches
            let direct_matches: HashSet<&String> = block_terms
                .iter()
                .filter(|t| unique_query_terms.contains(*t))
                .collect();

            // Then, check for compound word matches
            let mut compound_matches = HashSet::new();
            for query_term in &unique_query_terms {
                // Skip terms that were already directly matched
                if block_terms.iter().any(|t| t == query_term) {
                    continue;
                }

                // Check if this query term can be formed by combining adjacent terms in block_terms
                // For simplicity, we'll just check if all parts of the compound word exist in the block
                let parts =
                    tokenization::split_compound_word(query_term, tokenization::load_vocabulary());

                if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                    compound_matches.insert(query_term);
                }
            }

            direct_matches.len() + compound_matches.len()
        };

        let block_total_matches = if block_terms.is_empty() || unique_query_terms.is_empty() {
            0
        } else {
            // Count direct matches
            let direct_match_count = block_terms
                .iter()
                .filter(|t| unique_query_terms.contains(*t))
                .count();

            // Count compound matches
            let mut compound_match_count = 0;
            for query_term in &unique_query_terms {
                // Skip terms that were already directly matched
                if block_terms.iter().any(|t| t == query_term) {
                    continue;
                }

                // Check if this query term can be formed by combining adjacent terms in block_terms
                let parts =
                    tokenization::split_compound_word(query_term, tokenization::load_vocabulary());

                if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                    compound_match_count += 1;
                }
            }

            direct_match_count + compound_match_count
        };
        search_result.file_unique_terms = Some(block_unique_terms);
        search_result.file_total_matches = Some(block_total_matches);
    }

    Ok(search_result)
}

/// Determines a better node type for fallback context by analyzing the line content
fn determine_fallback_node_type(line: &str, extension: Option<&str>) -> String {
    let trimmed = line.trim();

    // First try to detect comments
    if trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("*")
        || (trimmed.starts_with("#") && extension.is_some_and(|ext| ext == "py" || ext == "rb"))
        || trimmed.starts_with("'''")
        || trimmed.starts_with("\"\"\"")
    {
        return "comment".to_string();
    }

    // Try to detect common code structures based on the line content
    let lowercase = trimmed.to_lowercase();

    // Check for function/method declarations
    if (trimmed.contains("fn ")
        && (trimmed.contains("(") || trimmed.contains(")"))
        && extension == Some("rs"))
        || (trimmed.contains("func ") && extension == Some("go"))
        || (trimmed.contains("function ")
            && extension.is_some_and(|ext| ext == "js" || ext == "ts"))
        || (lowercase.contains("def ") && extension == Some("py"))
        || (trimmed.contains("public")
            && trimmed.contains("void")
            && extension.is_some_and(|ext| ext == "java" || ext == "kt"))
    {
        return "function".to_string();
    }

    // Check for class declarations
    if (trimmed.contains("class ") || trimmed.contains("interface "))
        || (trimmed.contains("struct ")
            && extension
                .is_some_and(|ext| ext == "rs" || ext == "go" || ext == "c" || ext == "cpp"))
        || (trimmed.contains("type ") && trimmed.contains("struct") && extension == Some("go"))
        || (trimmed.contains("enum "))
    {
        return "class".to_string();
    }

    // Check for imports/requires
    if trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("require ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("#include ")
    {
        return "import".to_string();
    }

    // Check for variable declarations
    if (trimmed.starts_with("let ") || trimmed.starts_with("var ") || trimmed.starts_with("const "))
        || (trimmed.contains("=") && !trimmed.contains("==") && !trimmed.contains("=>"))
    {
        return "variable_declaration".to_string();
    }

    // Check for control flow statements
    if trimmed.starts_with("if ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("switch ")
        || trimmed.starts_with("match ")
    {
        return "control_flow".to_string();
    }

    // If we can't determine a specific type, use "code" instead of "context"
    "code".to_string()
}

/// Function to process a file with line numbers and return SearchResult structs
pub fn process_file_with_results(params: &FileProcessingParams) -> Result<Vec<SearchResult>> {
    // Read the file content
    let content = fs::read_to_string(params.path)
        .context(format!("Failed to read file: {:?}", params.path))?;

    // Get the file extension
    let extension = params
        .path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    // Split the content into lines for context processing
    let lines: Vec<&str> = content.lines().collect();

    // Create SearchResult structs for each match
    let mut results = Vec::new();

    // Track which line numbers have been covered
    let mut covered_lines = HashSet::new();

    // Debug mode
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Processing file with results: {:?}", params.path);
        println!("DEBUG:   Matched line numbers: {:?}", params.line_numbers);
        println!("DEBUG:   File extension: {}", extension);
        println!("DEBUG:   Total lines in file: {}", lines.len());

        // Log filename matches if present
        if !params.filename_matched_queries.is_empty() {
            println!(
                "DEBUG: Filename '{}' matched queries (indices): {:?}",
                params
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                params.filename_matched_queries
            );
        }
    }

    // First try to use AST parsing
    if let Ok(code_blocks) = parse_file_for_code_blocks(
        &content,
        extension,
        params.line_numbers,
        params.allow_tests,
        params.term_matches,
    ) {
        if debug_mode {
            println!("DEBUG: AST parsing successful");
            println!("DEBUG:   Found {} code blocks", code_blocks.len());

            for (i, block) in code_blocks.iter().enumerate() {
                println!(
                    "DEBUG:   Block {}: type={}, lines={}-{}",
                    i + 1,
                    block.node_type,
                    block.start_row + 1,
                    block.end_row + 1
                );
            }
        }

        // Generate a unique file ID for block correlation
        let file_id = format!("{}", params.path.to_string_lossy());

        // Process all individual blocks (no merging)
        for (block_idx, block) in code_blocks.iter().enumerate() {
            // Get the line start and end based on AST
            let start_line = block.start_row + 1; // Convert to 1-based line numbers
            let end_line = block.end_row + 1;

            // Check if this is a struct_type inside a function in Go code
            let (final_start_line, final_end_line, is_nested_struct) = if extension == "go"
                && block.node_type == "struct_type"
                && block
                    .parent_node_type
                    .as_ref()
                    .is_some_and(|p| p == "function_declaration" || p == "method_declaration")
            {
                // Use the parent function's boundaries instead of just the struct
                if let Some(parent_start) = block.parent_start_row {
                    if let Some(parent_end) = block.parent_end_row {
                        if debug_mode {
                            println!(
                                    "DEBUG: Expanding nested struct at {}-{} to parent function at {}-{}",
                                    start_line, end_line, parent_start + 1, parent_end + 1
                                );
                        }
                        (parent_start + 1, parent_end + 1, true)
                    } else {
                        (start_line, end_line, false)
                    }
                } else {
                    (start_line, end_line, false)
                }
            } else {
                (start_line, end_line, false)
            };

            // Extract the full code for this block
            let full_code = if final_start_line > 0 && final_end_line <= lines.len() {
                lines[final_start_line - 1..final_end_line].join("\n")
            } else {
                "".to_string()
            };

            // Calculate block term matches - ensure we use the same stemming as query processing
            let block_terms = preprocess_text(&full_code, false);

            // Use preprocessed query terms if available, otherwise generate them
            let query_terms: Vec<String> = if let Some(preprocessed) = params.preprocessed_queries {
                preprocessed
                    .iter()
                    .flat_map(|terms| terms.iter().cloned())
                    .collect()
            } else {
                params
                    .queries_terms
                    .iter()
                    .flat_map(|terms| terms.iter().map(|(_, stemmed)| stemmed.clone()))
                    .collect()
            };

            let unique_query_terms: HashSet<String> = query_terms.into_iter().collect();

            // Debug logging for stemming comparison
            if debug_mode {
                println!("DEBUG: Block terms after stemming: {:?}", block_terms);
                println!(
                    "DEBUG: Query terms after stemming: {:?}",
                    unique_query_terms
                );
            }

            // Calculate unique terms matched in the block
            let block_unique_terms = if block_terms.is_empty() || unique_query_terms.is_empty() {
                0
            } else {
                // First, check for direct matches
                let direct_matches: HashSet<&String> = block_terms
                    .iter()
                    .filter(|t| unique_query_terms.contains(*t))
                    .collect();

                // Then, check for compound word matches
                let mut compound_matches = HashSet::new();
                for query_term in &unique_query_terms {
                    // Skip terms that were already directly matched
                    if block_terms.iter().any(|t| t == query_term) {
                        continue;
                    }

                    // Check if this query term can be formed by combining adjacent terms in block_terms
                    // For simplicity, we'll just check if all parts of the compound word exist in the block
                    let parts = tokenization::split_compound_word(
                        query_term,
                        tokenization::load_vocabulary(),
                    );

                    if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                        compound_matches.insert(query_term);
                    }
                }

                direct_matches.len() + compound_matches.len()
            };

            // Calculate total matches in the block
            let block_total_matches = if block_terms.is_empty() || unique_query_terms.is_empty() {
                0
            } else {
                // Count direct matches
                let direct_match_count = block_terms
                    .iter()
                    .filter(|t| unique_query_terms.contains(*t))
                    .count();

                // Count compound matches
                let mut compound_match_count = 0;
                for query_term in &unique_query_terms {
                    // Skip terms that were already directly matched
                    if block_terms.iter().any(|t| t == query_term) {
                        continue;
                    }

                    // Check if this query term can be formed by combining adjacent terms in block_terms
                    let parts = tokenization::split_compound_word(
                        query_term,
                        tokenization::load_vocabulary(),
                    );

                    if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                        compound_match_count += 1;
                    }
                }

                direct_match_count + compound_match_count
            };

            if debug_mode {
                println!(
                    "DEBUG: Block at {}-{} has {} unique term matches and {} total matches",
                    final_start_line, final_end_line, block_unique_terms, block_total_matches
                );
            }

            // Mark all lines in this block as covered
            for line_num in final_start_line..=final_end_line {
                covered_lines.insert(line_num);
            }

            // Apply term filtering if term_matches is provided
            let should_include = if let Some(term_matches_map) = params.term_matches {
                // Use the filter_code_block function with the filename_matched_queries parameter
                filter_code_block(
                    (final_start_line, final_end_line),
                    term_matches_map,
                    params.any_term,
                    params.num_queries,
                    &params.filename_matched_queries,
                    debug_mode,
                )
            } else {
                // If no term_matches provided, include all blocks
                true
            };

            if debug_mode {
                println!(
                    "DEBUG: Filtered code block at {}-{}: included={}",
                    final_start_line, final_end_line, should_include
                );
                println!(
                    "DEBUG: Block at {}-{} filtered: included={}",
                    final_start_line, final_end_line, should_include
                );
            }

            // Add to results only if it passes the filter
            if should_include {
                results.push(SearchResult {
                    file: params.path.to_string_lossy().to_string(),
                    lines: (final_start_line, final_end_line),
                    node_type: if is_nested_struct {
                        block
                            .parent_node_type
                            .clone()
                            .unwrap_or_else(|| block.node_type.clone())
                    } else {
                        block.node_type.clone()
                    },
                    code: full_code.clone(),
                    matched_by_filename: None,
                    rank: None,
                    score: None,
                    tfidf_score: None,
                    bm25_score: None,
                    tfidf_rank: None,
                    bm25_rank: None,
                    new_score: None,
                    hybrid2_rank: None,
                    combined_score_rank: None,
                    file_unique_terms: Some(block_unique_terms),
                    file_total_matches: Some(block_total_matches),
                    file_match_rank: None,
                    block_unique_terms: Some(block_unique_terms),
                    block_total_matches: Some(block_total_matches),
                    parent_file_id: Some(file_id.clone()),
                    block_id: Some(block_idx),
                });
            }
        }
    } else if debug_mode {
        println!("DEBUG: AST parsing failed, using line-based context only");
    }

    // Check for any line numbers that weren't covered
    for &line_num in params.line_numbers {
        if !covered_lines.contains(&line_num) {
            if debug_mode {
                println!(
                    "DEBUG: Line {} not covered, using fallback context",
                    line_num
                );
                if line_num <= lines.len() {
                    println!("DEBUG:   Line content: '{}'", lines[line_num - 1].trim());
                }
            }

            // Skip fallback context for test files if allow_tests is false
            if !params.allow_tests && crate::language::is_test_file(params.path) {
                if debug_mode {
                    println!(
                        "DEBUG: Skipping fallback context for test file: {:?}",
                        params.path
                    );
                }
                continue;
            }

            // Check if the line is in a test function/module by examining its content
            if !params.allow_tests && line_num <= lines.len() {
                let line_content = lines[line_num - 1];
                // Simple heuristic check for test functions/modules
                if line_content.contains("fn test_")
                    || line_content.contains("#[test]")
                    || line_content.contains("#[cfg(test)]")
                    || line_content.contains("mod tests")
                {
                    if debug_mode {
                        println!(
                            "DEBUG: Skipping fallback context for test code: '{}'",
                            line_content.trim()
                        );
                    }
                    continue;
                }
            }

            // Fallback: Get context around the line (20 lines before and after)
            let context_start = line_num.saturating_sub(10); // Expanded from 10
            let context_end = std::cmp::min(line_num + 10, lines.len());

            // Skip if we don't have enough context
            if context_start >= context_end {
                continue;
            }

            // Extract the context lines - ensure context_start is at least 1 to avoid underflow
            let context_code = if context_start > 0 {
                lines[context_start - 1..context_end].join("\n")
            } else {
                lines[0..context_end].join("\n")
            };

            // Determine a better node type for the fallback context by analyzing the content
            let node_type = determine_fallback_node_type(lines[line_num - 1], Some(extension));

            if debug_mode {
                println!(
                    "DEBUG: Inferred node type for fallback context: {}",
                    node_type
                );
            }

            // Calculate block term matches - ensure we use the same stemming as query processing
            let block_terms = preprocess_text(&context_code, false);

            // Use preprocessed query terms if available, otherwise generate them
            let query_terms: Vec<String> = if let Some(preprocessed) = params.preprocessed_queries {
                preprocessed
                    .iter()
                    .flat_map(|terms| terms.iter().cloned())
                    .collect()
            } else {
                params
                    .queries_terms
                    .iter()
                    .flat_map(|terms| terms.iter().map(|(_, stemmed)| stemmed.clone()))
                    .collect()
            };

            let unique_query_terms: HashSet<String> = query_terms.into_iter().collect();

            // Debug logging for stemming comparison
            if debug_mode {
                println!(
                    "DEBUG: Fallback context block terms after stemming: {:?}",
                    block_terms
                );
                println!(
                    "DEBUG: Query terms after stemming: {:?}",
                    unique_query_terms
                );
            }

            // Calculate unique terms matched in the block
            let block_unique_terms = if block_terms.is_empty() || unique_query_terms.is_empty() {
                0
            } else {
                // First, check for direct matches
                let direct_matches: HashSet<&String> = block_terms
                    .iter()
                    .filter(|t| unique_query_terms.contains(*t))
                    .collect();

                // Then, check for compound word matches
                let mut compound_matches = HashSet::new();
                for query_term in &unique_query_terms {
                    // Skip terms that were already directly matched
                    if block_terms.iter().any(|t| t == query_term) {
                        continue;
                    }

                    // Check if this query term can be formed by combining adjacent terms in block_terms
                    // For simplicity, we'll just check if all parts of the compound word exist in the block
                    let parts = tokenization::split_compound_word(
                        query_term,
                        tokenization::load_vocabulary(),
                    );

                    if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                        compound_matches.insert(query_term);
                    }
                }

                direct_matches.len() + compound_matches.len()
            };

            // Calculate total matches in the block
            let block_total_matches = if block_terms.is_empty() || unique_query_terms.is_empty() {
                0
            } else {
                // Count direct matches
                let direct_match_count = block_terms
                    .iter()
                    .filter(|t| unique_query_terms.contains(*t))
                    .count();

                // Count compound matches
                let mut compound_match_count = 0;
                for query_term in &unique_query_terms {
                    // Skip terms that were already directly matched
                    if block_terms.iter().any(|t| t == query_term) {
                        continue;
                    }

                    // Check if this query term can be formed by combining adjacent terms in block_terms
                    let parts = tokenization::split_compound_word(
                        query_term,
                        tokenization::load_vocabulary(),
                    );

                    if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                        compound_match_count += 1;
                    }
                }

                direct_match_count + compound_match_count
            };

            if debug_mode {
                println!(
                    "DEBUG: Context block at {}-{} has {} unique term matches and {} total matches",
                    context_start, context_end, block_unique_terms, block_total_matches
                );
                println!(
                    "DEBUG: Fallback context at lines {}-{}",
                    context_start, context_end
                );
            }

            // Apply term filtering if term_matches is provided
            let should_include = if let Some(term_matches_map) = params.term_matches {
                // Use the filter_code_block function with the filename_matched_queries parameter
                filter_code_block(
                    (context_start, context_end),
                    term_matches_map,
                    params.any_term,
                    params.num_queries,
                    &params.filename_matched_queries,
                    debug_mode,
                )
            } else {
                // If no term_matches provided, include all blocks
                true
            };

            if debug_mode {
                println!(
                    "DEBUG: Filtered context block at {}-{}: included={}",
                    context_start, context_end, should_include
                );
                println!(
                    "DEBUG: Block at {}-{} filtered: included={}",
                    context_start, context_end, should_include
                );
            }

            // Add to results only if it passes the filter
            if should_include {
                results.push(SearchResult {
                    file: params.path.to_string_lossy().to_string(),
                    lines: (context_start, context_end),
                    node_type,
                    code: context_code.clone(), // Clone context_code here to avoid the move
                    matched_by_filename: None,
                    rank: None,
                    score: None,
                    tfidf_score: None,
                    bm25_score: None,
                    tfidf_rank: None,
                    bm25_rank: None,
                    new_score: None,
                    hybrid2_rank: None,
                    combined_score_rank: None,
                    file_unique_terms: None,
                    file_total_matches: None,
                    file_match_rank: None,
                    block_unique_terms: Some(block_unique_terms),
                    block_total_matches: Some(block_total_matches),
                    parent_file_id: None,
                    block_id: None,
                });
            }

            // Mark these lines as covered (even if we don't include the result)
            // This prevents duplicate processing of the same lines
            for line in context_start..=context_end {
                covered_lines.insert(line);
            }
        }
    }

    // Define a function to determine if we should return the full file
    fn should_return_full_file(
        coverage_percentage: f64,
        total_lines: usize,
        no_merge: bool,
    ) -> bool {
        !no_merge && total_lines >= 5 && coverage_percentage >= 80.0
    }

    // Calculate coverage percentage with safeguards for division by zero
    let total_lines = lines.len();
    let covered_line_count = covered_lines.len();
    let coverage_percentage = if total_lines > 0 {
        (covered_line_count as f64 / total_lines as f64) * 100.0
    } else {
        0.0
    };

    if debug_mode {
        println!(
            "DEBUG: File coverage: {}/{} lines ({:.2}%)",
            covered_line_count, total_lines, coverage_percentage
        );
    }

    // Check if we should return the full file based on coverage and minimum line count
    if should_return_full_file(coverage_percentage, total_lines, params.no_merge) {
        if debug_mode {
            println!("DEBUG: Coverage exceeds 80%, returning entire file");
        }

        // Clear the previous results and return the entire file
        results.clear();

        // Calculate block term matches for the entire file - ensure we use the same stemming as query processing
        let block_terms = preprocess_text(&content, false);

        // Use preprocessed query terms if available, otherwise generate them
        let query_terms: Vec<String> = if let Some(preprocessed) = params.preprocessed_queries {
            preprocessed
                .iter()
                .flat_map(|terms| terms.iter().cloned())
                .collect()
        } else {
            params
                .queries_terms
                .iter()
                .flat_map(|terms| terms.iter().map(|(_, stemmed)| stemmed.clone()))
                .collect()
        };

        let unique_query_terms: HashSet<String> = query_terms.into_iter().collect();

        // Debug logging for stemming comparison
        if debug_mode {
            println!("DEBUG: Full file terms after stemming: {:?}", block_terms);
            println!(
                "DEBUG: Query terms after stemming: {:?}",
                unique_query_terms
            );
        }

        // Calculate unique terms matched in the file
        let block_unique_terms = if block_terms.is_empty() || unique_query_terms.is_empty() {
            0
        } else {
            // First, check for direct matches
            let direct_matches: HashSet<&String> = block_terms
                .iter()
                .filter(|t| unique_query_terms.contains(*t))
                .collect();

            // Then, check for compound word matches
            let mut compound_matches = HashSet::new();
            for query_term in &unique_query_terms {
                // Skip terms that were already directly matched
                if block_terms.iter().any(|t| t == query_term) {
                    continue;
                }

                // Check if this query term can be formed by combining adjacent terms in block_terms
                // For simplicity, we'll just check if all parts of the compound word exist in the block
                let parts =
                    tokenization::split_compound_word(query_term, tokenization::load_vocabulary());

                if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                    compound_matches.insert(query_term);
                }
            }

            direct_matches.len() + compound_matches.len()
        };

        // Calculate total matches in the file
        let block_total_matches = if block_terms.is_empty() || unique_query_terms.is_empty() {
            0
        } else {
            // Count direct matches
            let direct_match_count = block_terms
                .iter()
                .filter(|t| unique_query_terms.contains(*t))
                .count();

            // Count compound matches
            let mut compound_match_count = 0;
            for query_term in &unique_query_terms {
                // Skip terms that were already directly matched
                if block_terms.iter().any(|t| t == query_term) {
                    continue;
                }

                // Check if this query term can be formed by combining adjacent terms in block_terms
                let parts =
                    tokenization::split_compound_word(query_term, tokenization::load_vocabulary());

                if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                    compound_match_count += 1;
                }
            }

            direct_match_count + compound_match_count
        };

        if debug_mode {
            println!(
                "DEBUG: Full file has {} unique term matches and {} total matches",
                block_unique_terms, block_total_matches
            );
        }

        results.push(SearchResult {
            file: params.path.to_string_lossy().to_string(),
            lines: (1, total_lines),
            node_type: "file".to_string(), // Mark as full file result
            code: content.clone(),         // Clone content here to avoid the move
            matched_by_filename: None,
            rank: None,
            score: None,
            tfidf_score: None,
            bm25_score: None,
            tfidf_rank: None,
            bm25_rank: None,
            new_score: None,
            hybrid2_rank: None,
            combined_score_rank: None,
            file_unique_terms: None,
            file_total_matches: None,
            file_match_rank: None,
            block_unique_terms: Some(block_unique_terms),
            block_total_matches: Some(block_total_matches),
            parent_file_id: None,
            block_id: None,
        });
    }

    // Log debug information outside the conditional block
    if debug_mode {
        println!(
            "DEBUG: Generated {} search results for file {:?}",
            results.len(),
            params.path
        );
    }

    Ok(results)
}
