use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tree_sitter;

use crate::language::{is_test_file, parse_file_for_code_blocks};
use crate::models::SearchResult;
use crate::ranking;
use crate::search::tokenization;

/// Parameters for file processing
pub struct FileProcessingParams<'a> {
    pub path: &'a Path,
    pub line_numbers: &'a HashSet<usize>,
    pub allow_tests: bool,
    pub term_matches: &'a HashMap<usize, HashSet<usize>>,
    #[allow(dead_code)]
    pub num_queries: usize,
    #[allow(dead_code)]
    pub filename_matched_queries: HashSet<usize>,
    pub queries_terms: &'a [Vec<(String, String)>],
    pub preprocessed_queries: Option<&'a [Vec<String>]>,
    pub query_plan: &'a crate::search::query::QueryPlan,

    #[allow(dead_code)]
    pub no_merge: bool,
}

/// Evaluate whether a block of lines satisfies a complex AST query
/// using the 'evaluate' method in `elastic_query::Expr`. We assume
/// the 'term_matches' map uses the same indexing as the AST's QueryPlan term_indices.
#[allow(dead_code)]
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

        // Add detailed information about which exact keywords matched
        println!("DEBUG: ===== MATCHED KEYWORDS DETAILS =====");
        let mut matched_keywords = Vec::new();
        for (term, &idx) in &plan.term_indices {
            if matched_terms.contains(&idx) {
                matched_keywords.push(term);
                println!(
                    "DEBUG: Keyword '{}' matched in block {}-{}",
                    term, block_lines.0, block_lines.1
                );
            }
        }
        if matched_keywords.is_empty() {
            println!("DEBUG: No keywords matched in this block");
        } else {
            println!("DEBUG: All matched keywords: {:?}", matched_keywords);
        }
        println!("DEBUG: ===================================");
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

    // Use the AST evaluation directly
    if debug_mode {
        println!("DEBUG: ===== AST EVALUATION =====");
        println!("DEBUG: Matched terms: {:?}", matched_terms);
        println!("DEBUG: Term indices: {:?}", plan.term_indices);
    }

    // Use the evaluate function from the elastic query module
    let result = plan.ast.evaluate(&matched_terms, &plan.term_indices, false);

    if debug_mode {
        println!("DEBUG: ===== EVALUATION RESULT =====");
        println!("DEBUG: AST evaluation result: {}", result);
        println!(
            "DEBUG: Block {}-{} will be {}",
            block_lines.0,
            block_lines.1,
            if result { "INCLUDED" } else { "EXCLUDED" }
        );
        println!("DEBUG: ============================");
    }

    let decision = result;

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

/// Evaluate whether a tokenized block satisfies a complex AST query
/// using the 'evaluate' method in `elastic_query::Expr`.
pub fn filter_tokenized_block(
    tokenized_content: &[String],
    term_indices: &HashMap<String, usize>,
    plan: &crate::search::query::QueryPlan,
    debug_mode: bool,
) -> bool {
    // Create a set of matched term indices based on tokenized content
    let mut matched_terms = HashSet::new();

    // For each token in the tokenized content, check if it's in the term_indices
    for token in tokenized_content {
        if let Some(&idx) = term_indices.get(token) {
            matched_terms.insert(idx);
        }
    }

    // Special handling for compound words like "whitelist"
    // Check if any term in the plan is a compound of tokens in the content
    for (term, &idx) in &plan.term_indices {
        // Skip if we already matched this term
        if matched_terms.contains(&idx) {
            continue;
        }

        // Check if this term is a special case that should be treated as a single token
        if crate::search::tokenization::is_special_case(term) {
            // If the tokenized content contains this special case term, add it to matched terms
            if tokenized_content.contains(&term.to_lowercase()) {
                matched_terms.insert(idx);
                if debug_mode {
                    println!(
                        "DEBUG: Special case term '{}' matched in tokenized content",
                        term
                    );
                }
            }
        }
    }

    if debug_mode {
        println!("DEBUG: Checking for terms in tokenized block");
        println!("DEBUG: Tokenized content: {:?}", tokenized_content);
        println!("DEBUG: Matched terms: {:?}", matched_terms);
        println!("DEBUG: Term indices: {:?}", plan.term_indices);
        println!("DEBUG: Excluded terms: {:?}", plan.excluded_terms);
        println!("DEBUG: AST: {:?}", plan.ast);

        // Add detailed information about which exact keywords matched
        println!("DEBUG: ===== MATCHED KEYWORDS DETAILS =====");
        let mut matched_keywords = Vec::new();
        for (term, &idx) in &plan.term_indices {
            if matched_terms.contains(&idx) {
                matched_keywords.push(term);
                println!("DEBUG: Keyword '{}' matched in tokenized block", term);
            }
        }
        if matched_keywords.is_empty() {
            println!("DEBUG: No keywords matched in this block");
        } else {
            println!("DEBUG: All matched keywords: {:?}", matched_keywords);
        }
        println!("DEBUG: ===================================");
    }

    // Check if we have any matches at all
    if matched_terms.is_empty() {
        if debug_mode {
            println!("DEBUG: No matched terms in tokenized block, returning false");
        }
        return false;
    }

    // Use the AST evaluation directly
    if debug_mode {
        println!("DEBUG: ===== AST EVALUATION =====");
        println!("DEBUG: Matched terms: {:?}", matched_terms);
        println!("DEBUG: Term indices: {:?}", plan.term_indices);
    }

    // Use the evaluate function from the elastic query module
    let result = plan.ast.evaluate(&matched_terms, &plan.term_indices, false);

    if debug_mode {
        println!("DEBUG: ===== EVALUATION RESULT =====");
        println!("DEBUG: AST evaluation result: {}", result);
        println!(
            "DEBUG: Block will be {}",
            if result { "INCLUDED" } else { "EXCLUDED" }
        );
        println!("DEBUG: ============================");
    }

    let decision = result;

    if debug_mode {
        println!("DEBUG: Tokenized block matched terms: {:?}", matched_terms);
        println!("DEBUG: AST evaluation result: {}", decision);
        println!(
            "DEBUG: filter_tokenized_block => matched {:?}, decision={}",
            matched_terms, decision
        );
    }

    decision
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

    // Get the filename for tokenization - do this once for the entire file
    let filename = params
        .path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    // Prepare query terms once for the entire file
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

    if debug_mode {
        println!("DEBUG: Processing file: {:?}", params.path);
        println!("DEBUG:   matched lines: {:?}", params.line_numbers);
    }

    if let Ok(code_blocks) = parse_file_for_code_blocks(
        &content,
        extension,
        params.line_numbers,
        params.allow_tests,
        Some(params.term_matches),
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

            // Early tokenization with filename prepended
            let block_terms = ranking::preprocess_text_with_filename(&full_code, &filename);

            // Early filtering using tokenized content
            let should_include = {
                if debug_mode {
                    println!(
                        "DEBUG: Using filter_tokenized_block for block {}-{}",
                        final_start_line, final_end_line
                    );
                }
                // Use the AST evaluation directly to ensure correct handling of complex queries
                let result = filter_tokenized_block(
                    &block_terms,
                    &params.query_plan.term_indices,
                    params.query_plan,
                    debug_mode,
                );

                if debug_mode {
                    println!(
                        "DEBUG: Block {}-{} filter result: {}",
                        final_start_line, final_end_line, result
                    );
                }

                result
            };

            if debug_mode {
                println!(
                    "DEBUG: Block lines {}-{} => should_include={}",
                    final_start_line, final_end_line, should_include
                );
            }

            // Mark lines as covered
            for line_num in final_start_line..=final_end_line {
                covered_lines.insert(line_num);
            }

            if should_include {
                // Calculate metrics using the already tokenized content
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
                let block_total_matches = direct_matches.len() + compound_matches.len();

                // Collect matched keywords
                let mut matched_keywords = Vec::new();

                // Add direct matches
                matched_keywords.extend(direct_matches.iter().map(|s| (*s).clone()));

                // Add compound matches
                matched_keywords.extend(compound_matches.iter().map(|s| (*s).clone()));

                // Get the matched term indices for this block
                let mut matched_term_indices = HashSet::new();
                for (&term_idx, lines) in params.term_matches {
                    if lines
                        .iter()
                        .any(|&l| l >= final_start_line && l <= final_end_line)
                    {
                        matched_term_indices.insert(term_idx);
                    }
                }

                // Add the corresponding terms from the query plan
                for (term, &idx) in &params.query_plan.term_indices {
                    if matched_term_indices.contains(&idx)
                        && !params.query_plan.excluded_terms.contains(term)
                    {
                        matched_keywords.push(term.clone());
                    }
                }

                // Remove duplicates
                matched_keywords.sort();
                matched_keywords.dedup();

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
                    code: full_code,
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
                    matched_keywords: if matched_keywords.is_empty() {
                        None
                    } else {
                        Some(matched_keywords)
                    },
                    tokenized_content: Some(block_terms),
                });
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

            // Check if the line is in a test function/module using language-specific detection
            if !params.allow_tests && line_num <= lines.len() {
                // Get the language implementation for this file extension
                if let Some(language_impl) = crate::language::factory::get_language_impl(extension)
                {
                    let line_content = lines[line_num - 1];

                    // Create a simple parser to check this line
                    let mut parser = tree_sitter::Parser::new();
                    if parser
                        .set_language(&language_impl.get_tree_sitter_language())
                        .is_ok()
                    {
                        // Try to parse just this line to get a node
                        if let Some(tree) = parser.parse(line_content, None) {
                            let node = tree.root_node();

                            // Use the language-specific test detection
                            if language_impl.is_test_node(&node, line_content.as_bytes()) {
                                if debug_mode {
                                    println!(
                                        "DEBUG: Skipping fallback context for test code: '{}'",
                                        line_content.trim()
                                    );
                                }
                                continue;
                            }
                        }
                    }
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

            // Early tokenization for fallback context
            let context_terms = ranking::preprocess_text_with_filename(&context_code, &filename);

            // Early filtering for fallback context
            let should_include = {
                if debug_mode {
                    println!(
                        "DEBUG: Using filter_tokenized_block for fallback context {}-{}",
                        context_start, context_end
                    );
                }
                filter_tokenized_block(
                    &context_terms,
                    &params.query_plan.term_indices,
                    params.query_plan,
                    debug_mode,
                )
            };

            if debug_mode {
                println!(
                    "DEBUG: Block at {}-{} filtered: included={}",
                    context_start, context_end, should_include
                );
            }

            // Mark these lines as covered (even if we don't include the result)
            for line in context_start..=context_end {
                covered_lines.insert(line);
            }

            // Add to results only if it passes the filter
            if should_include {
                // Calculate metrics for fallback context using the already tokenized content
                let direct_matches: HashSet<&String> = context_terms
                    .iter()
                    .filter(|t| unique_query_terms.contains(*t))
                    .collect();

                let mut compound_matches = HashSet::new();
                for qterm in &unique_query_terms {
                    if context_terms.iter().any(|bt| bt == qterm) {
                        continue;
                    }
                    let parts =
                        tokenization::split_compound_word(qterm, tokenization::load_vocabulary());
                    if parts.len() > 1 && parts.iter().all(|part| context_terms.contains(part)) {
                        compound_matches.insert(qterm);
                    }
                }

                let context_unique_terms = direct_matches.len() + compound_matches.len();
                let context_total_matches = direct_matches.len() + compound_matches.len();

                // Collect matched keywords for fallback context
                let mut matched_keywords = Vec::new();

                // Add direct matches
                matched_keywords.extend(direct_matches.iter().map(|s| (*s).clone()));

                // Add compound matches
                matched_keywords.extend(compound_matches.iter().map(|s| (*s).clone()));

                // Get the matched term indices for this context block
                let mut matched_term_indices = HashSet::new();
                for (&term_idx, lines) in params.term_matches {
                    if lines
                        .iter()
                        .any(|&l| l >= context_start && l <= context_end)
                    {
                        matched_term_indices.insert(term_idx);
                    }
                }

                // Add the corresponding terms from the query plan
                for (term, &idx) in &params.query_plan.term_indices {
                    if matched_term_indices.contains(&idx)
                        && !params.query_plan.excluded_terms.contains(term)
                    {
                        matched_keywords.push(term.clone());
                    }
                }

                // Remove duplicates
                matched_keywords.sort();
                matched_keywords.dedup();

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
                    file_unique_terms: Some(context_unique_terms),
                    file_total_matches: Some(context_total_matches),
                    file_match_rank: None,
                    block_unique_terms: Some(context_unique_terms),
                    block_total_matches: Some(context_total_matches),
                    parent_file_id: None,
                    block_id: None,
                    matched_keywords: if matched_keywords.is_empty() {
                        None
                    } else {
                        Some(matched_keywords)
                    },
                    tokenized_content: Some(context_terms),
                });
            }
        }
    }

    Ok(results)
}
