use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::language::{is_test_file, parse_file_for_code_blocks};
use crate::models::SearchResult;
use crate::ranking;
use crate::search::file_search::get_filename_matched_queries_compat;
use crate::search::tokenization;

/// Parameters for file processing
pub struct FileProcessingParams<'a> {
    pub path: &'a Path,
    pub line_numbers: &'a HashSet<usize>,
    pub allow_tests: bool,
    pub term_matches: Option<&'a HashMap<usize, HashSet<usize>>>,
    pub num_queries: usize,
    pub filename_matched_queries: HashSet<usize>,
    pub queries_terms: &'a [Vec<(String, String)>],
    pub preprocessed_queries: Option<&'a [Vec<String>]>,
    pub query_plan: Option<&'a crate::search::query::QueryPlan>,

    #[allow(dead_code)]
    pub no_merge: bool,
}

/// Function to check if a code block should be included based on term matches
/// This is a simplified version that relies on the AST evaluation
fn filter_code_block(
    block_lines: (usize, usize),
    term_matches: &HashMap<usize, HashSet<usize>>,
    num_queries: usize,
    filename_matched_queries: &HashSet<usize>,
    debug_mode: bool,
) -> bool {
    let mut matched_queries = HashSet::new();

    for (query_idx, lines) in term_matches {
        if lines
            .iter()
            .any(|&l| l >= block_lines.0 && l <= block_lines.1)
        {
            matched_queries.insert(*query_idx);
        }
    }

    let block_unique_terms = matched_queries.len();

    // We need to ensure all required terms are matched
    // Either directly in the content or via filename matches
    if matched_queries.is_empty() {
        // If no terms matched in content, we can't satisfy the criteria
        return false;
    }

    // Check if all required terms are covered
    let term_match_criteria = (0..num_queries)
        .all(|i| filename_matched_queries.contains(&i) || matched_queries.contains(&i));

    let min_required_terms = match num_queries {
        0 => 0,
        1 | 2 => 1,
        3 | 4 => 2,
        5 | 6 => 3,
        7 | 8 => 4,
        9 | 10 => 5,
        11 | 12 => 6,
        n => (n + 1) / 2,
    };

    let unique_terms_criteria = block_unique_terms >= min_required_terms;
    let should_include = term_match_criteria && unique_terms_criteria;

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

        println!("DEBUG: All-terms mode: Include if all {} queries matched (including filename matches: {:?}) AND at least one term is matched in content",
                 num_queries, filename_matched_queries);

        println!("DEBUG: Term match criteria met: {}", term_match_criteria);
        println!(
            "DEBUG: Unique terms criteria met: {} (need {} of {} terms)",
            unique_terms_criteria, min_required_terms, num_queries
        );

        println!(
            "DEBUG: Block lines {}-{} => matched {} terms; should_include={}",
            block_lines.0, block_lines.1, block_unique_terms, should_include
        );
    }
    should_include
}

/// Evaluate whether a block of lines satisfies a complex AST query
/// using the 'evaluate' method in `elastic_query::Expr`. We assume
/// the 'term_matches' map uses the same indexing as the AST's QueryPlan term_indices.
pub fn filter_code_block_with_ast(
    block_lines: (usize, usize),
    term_matches: &HashMap<usize, HashSet<usize>>,
    plan: &crate::search::query::QueryPlan,
    debug_mode: bool,
) -> bool {
    // Gather matched term indices for this block
    let mut matched_terms = HashSet::new();
    for (&term_idx, lines) in term_matches {
        if lines
            .iter()
            .any(|&l| l >= block_lines.0 && l <= block_lines.1)
        {
            matched_terms.insert(term_idx);
        }
    }

    if debug_mode {
        println!(
            "DEBUG: Checking for terms in block {}-{}",
            block_lines.0, block_lines.1
        );
        println!("DEBUG: Matched terms: {:?}", matched_terms);
        println!("DEBUG: Term indices: {:?}", plan.term_indices);
        println!("DEBUG: Excluded terms: {:?}", plan.excluded_terms);
        println!("DEBUG: AST: {:?}", plan.ast);
    }

    // Check if we have any matches at all
    if matched_terms.is_empty() {
        if debug_mode {
            println!(
                "DEBUG: No matched terms in block {}-{}, returning false",
                block_lines.0, block_lines.1
            );
        }
        return false;
    }

    // Check if any excluded term is present
    let has_excluded_term = plan.excluded_terms.iter().any(|term| {
        // For excluded terms, we only check if the exact term is present
        // We don't do compound word splitting for excluded terms
        if let Some(&idx) = plan.term_indices.get(term) {
            if matched_terms.contains(&idx) {
                if debug_mode {
                    println!(
                        "DEBUG: Block contains excluded term '{}', returning false",
                        term
                    );
                }
                return true;
            }
        }

        // We don't check for compound words or substrings for excluded terms
        // This ensures we only exclude exact matches of the excluded term
        false
    });

    if has_excluded_term {
        if debug_mode {
            println!("DEBUG: Block contains excluded term, returning false");
        }
        return false;
    }

    // Special handling for expressions with excluded terms
    // If we have a query like "keyword1 -keyword3" or "(key OR word OR 1) -keyword3",
    // we need to make sure we find blocks that match the required terms but don't contain the excluded term
    let has_excluded_terms = !plan.excluded_terms.is_empty();

    // Check if we have any OR expressions in the AST
    let has_or_expr = match &plan.ast {
        crate::search::elastic_query::Expr::Or(_, _) => true,
        crate::search::elastic_query::Expr::And(left, right) => {
            matches!(**left, crate::search::elastic_query::Expr::Or(_, _))
                || matches!(**right, crate::search::elastic_query::Expr::Or(_, _))
        }
        _ => false,
    };

    // If we have excluded terms, we need to handle them specially
    let decision = if has_excluded_terms {
        // First check if any required terms match
        let mut required_term_matches = false;

        // For OR expressions, any term can match
        // For AND expressions, we rely on the AST evaluation
        if has_or_expr {
            for (term, &idx) in &plan.term_indices {
                if !plan.excluded_terms.contains(term) && matched_terms.contains(&idx) {
                    required_term_matches = true;
                    break;
                }
            }
        } else {
            // For non-OR expressions, use the AST evaluation but ignore excluded terms
            // We'll check excluded terms separately
            required_term_matches = plan.ast.evaluate(&matched_terms, &plan.term_indices);
        }

        // Then check if any excluded term is present
        let excluded_term_present = plan.excluded_terms.iter().any(|term| {
            if let Some(&idx) = plan.term_indices.get(term) {
                matched_terms.contains(&idx)
            } else {
                false
            }
        });

        if debug_mode {
            println!("DEBUG: Required term matches: {}", required_term_matches);
            println!("DEBUG: Excluded term present: {}", excluded_term_present);
        }

        // Return true if required terms match and no excluded term is present
        required_term_matches && !excluded_term_present
    } else {
        // Use the normal AST evaluation when there are no excluded terms
        plan.ast.evaluate(&matched_terms, &plan.term_indices)
    };

    if debug_mode {
        println!(
            "DEBUG: Block {}-{} matched terms: {:?}",
            block_lines.0, block_lines.1, matched_terms
        );
        println!("DEBUG: AST evaluation result: {}", decision);
    }

    if debug_mode {
        println!(
            "DEBUG: filter_code_block_with_ast => lines {:?} => matched {:?}, decision={}",
            block_lines, matched_terms, decision
        );
    }
    decision
}

/// Existing code below (abbreviated) --------------------------------------
pub fn process_file_by_filename(
    path: &Path,
    queries_terms: &[Vec<(String, String)>],
    preprocessed_queries: Option<&[Vec<String>]>,
) -> Result<SearchResult> {
    let content = fs::read_to_string(path).context(format!("Failed to read file: {:?}", path))?;
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    let matched_terms = get_filename_matched_queries_compat(&filename, queries_terms);

    let mut search_result = SearchResult {
        file: path.to_string_lossy().to_string(),
        lines: (1, content.lines().count()),
        node_type: "file".to_string(),
        code: content.clone(),
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

    if let Some(preprocessed) = preprocessed_queries {
        let query_terms: Vec<String> = preprocessed
            .iter()
            .flat_map(|terms| terms.iter().cloned())
            .collect();
        let unique_query_terms: HashSet<String> = query_terms.into_iter().collect();
        let block_terms = ranking::preprocess_text(&content, false);
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        let block_unique_terms = if block_terms.is_empty() || unique_query_terms.is_empty() {
            0
        } else {
            let direct_matches: HashSet<&String> = block_terms
                .iter()
                .filter(|t| unique_query_terms.contains(*t))
                .collect();
            let mut compound_matches = HashSet::new();
            for query_term in &unique_query_terms {
                if block_terms.iter().any(|t| t == query_term) {
                    continue;
                }
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
            let direct_match_count = block_terms
                .iter()
                .filter(|t| unique_query_terms.contains(*t))
                .count();

            let mut compound_match_count = 0;
            for query_term in &unique_query_terms {
                if block_terms.iter().any(|t| t == query_term) {
                    continue;
                }
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
    }

    Ok(search_result)
}

/// Determines a better node type for fallback context by analyzing the line content
fn determine_fallback_node_type(line: &str, extension: Option<&str>) -> String {
    let trimmed = line.trim();

    if trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("*")
        || (trimmed.starts_with("#") && extension.is_some_and(|ext| ext == "py" || ext == "rb"))
        || trimmed.starts_with("'''")
        || trimmed.starts_with("\"\"\"")
    {
        return "comment".to_string();
    }

    let lowercase = trimmed.to_lowercase();

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

    if (trimmed.contains("class ") || trimmed.contains("interface "))
        || (trimmed.contains("struct ")
            && extension
                .is_some_and(|ext| ext == "rs" || ext == "go" || ext == "c" || ext == "cpp"))
        || (trimmed.contains("type ") && trimmed.contains("struct") && extension == Some("go"))
        || (trimmed.contains("enum "))
    {
        return "class".to_string();
    }

    if trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("require ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("#include ")
    {
        return "import".to_string();
    }

    if (trimmed.starts_with("let ") || trimmed.starts_with("var ") || trimmed.starts_with("const "))
        || (trimmed.contains("=") && !trimmed.contains("==") && !trimmed.contains("=>"))
    {
        return "variable_declaration".to_string();
    }

    if trimmed.starts_with("if ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("switch ")
        || trimmed.starts_with("match ")
    {
        return "control_flow".to_string();
    }

    "code".to_string()
}

/// Main function for processing a file with matched lines
pub fn process_file_with_results(params: &FileProcessingParams) -> Result<Vec<SearchResult>> {
    let content = fs::read_to_string(params.path)
        .context(format!("Failed to read file: {:?}", params.path))?;

    let extension = params
        .path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    let lines: Vec<&str> = content.lines().collect();
    let mut results = Vec::new();
    let mut covered_lines = HashSet::new();
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Processing file: {:?}", params.path);
        println!("DEBUG:   matched lines: {:?}", params.line_numbers);
    }

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

        let file_id = format!("{}", params.path.to_string_lossy());

        for (block_idx, block) in code_blocks.iter().enumerate() {
            let start_line = block.start_row + 1;
            let end_line = block.end_row + 1;

            let (final_start_line, final_end_line, is_nested_struct) = if extension == "go"
                && block.node_type == "struct_type"
                && block
                    .parent_node_type
                    .as_ref()
                    .is_some_and(|p| p == "function_declaration" || p == "method_declaration")
            {
                if let Some(ps) = block.parent_start_row {
                    if let Some(pe) = block.parent_end_row {
                        (ps + 1, pe + 1, true)
                    } else {
                        (start_line, end_line, false)
                    }
                } else {
                    (start_line, end_line, false)
                }
            } else {
                (start_line, end_line, false)
            };

            let full_code = if final_start_line > 0 && final_end_line <= lines.len() {
                lines[final_start_line - 1..final_end_line].join("\n")
            } else {
                "".to_string()
            };

            let block_terms = ranking::preprocess_text(&full_code, false);
            let query_terms: Vec<String> = if let Some(prep) = params.preprocessed_queries {
                prep.iter().flat_map(|v| v.iter().cloned()).collect()
            } else {
                params
                    .queries_terms
                    .iter()
                    .flat_map(|pairs| pairs.iter().map(|(_, s)| s.clone()))
                    .collect()
            };
            let unique_query_terms: HashSet<String> = query_terms.into_iter().collect();

            let direct_matches: HashSet<&String> = block_terms
                .iter()
                .filter(|t| unique_query_terms.contains(*t))
                .collect();
            let mut compound_matches = HashSet::new();
            for qterm in &unique_query_terms {
                if block_terms.iter().any(|bt| bt == qterm) {
                    continue;
                }
                let parts =
                    tokenization::split_compound_word(qterm, tokenization::load_vocabulary());
                if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                    compound_matches.insert(qterm);
                }
            }
            let block_unique_terms = direct_matches.len() + compound_matches.len();

            let block_total_matches = {
                let direct_count = direct_matches.len();
                let mut comp_count = 0;
                for qterm in &unique_query_terms {
                    if block_terms.iter().any(|bt| bt == qterm) {
                        continue;
                    }
                    let parts =
                        tokenization::split_compound_word(qterm, tokenization::load_vocabulary());
                    if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                        comp_count += 1;
                    }
                }
                direct_count + comp_count
            };

            for line_num in final_start_line..=final_end_line {
                covered_lines.insert(line_num);
            }

            let should_include = if let Some(tm) = params.term_matches {
                if let Some(plan) = params.query_plan {
                    if debug_mode {
                        println!(
                            "DEBUG: Using filter_code_block_with_ast for lines {}-{}",
                            final_start_line, final_end_line
                        );
                    }
                    filter_code_block_with_ast(
                        (final_start_line, final_end_line),
                        tm,
                        plan,
                        debug_mode,
                    )
                } else {
                    // Fall back to original filtering when no query plan is present
                    filter_code_block(
                        (final_start_line, final_end_line),
                        tm,
                        params.num_queries,
                        &params.filename_matched_queries,
                        debug_mode,
                    )
                }
            } else {
                true
            };

            if debug_mode {
                println!(
                    "DEBUG: Block lines {}-{} => should_include={}",
                    final_start_line, final_end_line, should_include
                );
            }

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
            } else if debug_mode {
                println!("DEBUG: AST parsing failed, using line-based context only");
            }
        }
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
            if !params.allow_tests && is_test_file(params.path) {
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
            let context_start = line_num.saturating_sub(10);
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

            // Apply term filtering if term_matches is provided
            let should_include = if let Some(term_matches_map) = params.term_matches {
                if let Some(plan) = params.query_plan {
                    if debug_mode {
                        println!(
                            "DEBUG: Using filter_code_block_with_ast for fallback context {}-{}",
                            context_start, context_end
                        );
                    }
                    filter_code_block_with_ast(
                        (context_start, context_end),
                        term_matches_map,
                        plan,
                        debug_mode,
                    )
                } else {
                    filter_code_block(
                        (context_start, context_end),
                        term_matches_map,
                        params.num_queries,
                        &params.filename_matched_queries,
                        debug_mode,
                    )
                }
            } else {
                // If no term_matches provided, include all blocks
                true
            };

            if debug_mode {
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
                    code: context_code,
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
                    block_unique_terms: None,
                    block_total_matches: None,
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

    Ok(results)
}
