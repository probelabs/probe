use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tree_sitter;

use crate::language::{is_test_file, parse_file_for_code_blocks};
use crate::models::SearchResult;
use crate::ranking;
use crate::search::tokenization;

/// Structure to hold timing information for file processing stages
pub struct FileProcessingTimings {
    pub file_io: Option<Duration>,

    // AST parsing timings
    pub ast_parsing: Option<Duration>,
    pub ast_parsing_language_init: Option<Duration>,
    pub ast_parsing_parser_init: Option<Duration>,
    pub ast_parsing_tree_parsing: Option<Duration>,
    pub ast_parsing_line_map_building: Option<Duration>,

    // Block extraction timings
    pub block_extraction: Option<Duration>,
    pub block_extraction_code_structure: Option<Duration>,
    pub block_extraction_filtering: Option<Duration>,
    pub block_extraction_result_building: Option<Duration>,

    // Detailed result building timings
    pub result_building_term_matching: Option<Duration>,
    pub result_building_compound_processing: Option<Duration>,
    pub result_building_line_matching: Option<Duration>,
    pub result_building_result_creation: Option<Duration>,
    pub result_building_synchronization: Option<Duration>,
    pub result_building_uncovered_lines: Option<Duration>,
}

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
pub fn process_file_with_results(
    params: &FileProcessingParams,
) -> Result<(Vec<SearchResult>, FileProcessingTimings)> {
    let mut timings = FileProcessingTimings {
        file_io: None,

        // AST parsing timings
        ast_parsing: None,
        ast_parsing_language_init: None,
        ast_parsing_parser_init: None,
        ast_parsing_tree_parsing: None,
        ast_parsing_line_map_building: None,

        // Block extraction timings
        block_extraction: None,
        block_extraction_code_structure: None,
        block_extraction_filtering: None,
        block_extraction_result_building: None,

        // Detailed result building timings
        result_building_term_matching: None,
        result_building_compound_processing: None,
        result_building_line_matching: None,
        result_building_result_creation: None,
        result_building_synchronization: None,
        result_building_uncovered_lines: None,
    };

    // Measure file I/O time
    let file_io_start = Instant::now();
    let content = fs::read_to_string(params.path)
        .context(format!("Failed to read file: {:?}", params.path))?;
    let file_io_duration = file_io_start.elapsed();
    timings.file_io = Some(file_io_duration);

    let extension = params
        .path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    // Get debug mode setting
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Filter out lines longer than 500 characters
    let lines: Vec<&str> = content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if line.len() > 500 {
                if debug_mode {
                    println!(
                        "DEBUG: Skipping line {} with length {} (exceeds 500 character limit)",
                        i + 1,
                        line.len()
                    );
                }
                ""
            } else {
                line
            }
        })
        .collect();
    let mut results = Vec::new();
    let mut covered_lines = HashSet::new();
    // We now use params.path.to_string_lossy() directly for tokenization

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
        println!("DEBUG:   file I/O time: {:?}", file_io_duration);
    }

    // Measure AST parsing time with sub-steps
    let ast_parsing_start = Instant::now();

    // Measure language initialization time
    let language_init_start = Instant::now();
    let language_impl = crate::language::factory::get_language_impl(extension);
    let language_init_duration = language_init_start.elapsed();
    timings.ast_parsing_language_init = Some(language_init_duration);

    // Measure parser initialization time
    let parser_init_start = Instant::now();
    let mut parser = tree_sitter::Parser::new();
    if let Some(lang_impl) = &language_impl {
        let _ = parser.set_language(&lang_impl.get_tree_sitter_language());
    }
    let parser_init_duration = parser_init_start.elapsed();
    timings.ast_parsing_parser_init = Some(parser_init_duration);

    // Measure tree parsing time
    let tree_parsing_start = Instant::now();
    let file_path = params.path.to_string_lossy().to_string();
    let cache_key = format!("{}_{}", file_path, extension);

    let _ = if language_impl.is_some() {
        crate::language::tree_cache::get_or_parse_tree(&cache_key, &content, &mut parser).ok()
    } else {
        None
    };
    let tree_parsing_duration = tree_parsing_start.elapsed();
    timings.ast_parsing_tree_parsing = Some(tree_parsing_duration);

    // Measure line map building time (this is an approximation since we can't directly measure it)
    let line_map_building_start = Instant::now();

    // Call the original parse_file_for_code_blocks function
    let code_blocks_result = parse_file_for_code_blocks(
        &content,
        extension,
        params.line_numbers,
        params.allow_tests,
        Some(params.term_matches),
    );

    let line_map_building_duration = line_map_building_start.elapsed();
    timings.ast_parsing_line_map_building = Some(line_map_building_duration);

    // Calculate total AST parsing time
    let ast_parsing_duration = ast_parsing_start.elapsed();
    timings.ast_parsing = Some(ast_parsing_duration);

    if debug_mode {
        println!("DEBUG:   AST parsing time: {:?}", ast_parsing_duration);
        println!("DEBUG:     - Language init: {:?}", language_init_duration);
        println!("DEBUG:     - Parser init: {:?}", parser_init_duration);
        println!("DEBUG:     - Tree parsing: {:?}", tree_parsing_duration);
        println!(
            "DEBUG:     - Line map building: {:?}",
            line_map_building_duration
        );
    }

    if let Ok(code_blocks) = code_blocks_result {
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

        // Measure block extraction time with sub-steps
        let block_extraction_start = Instant::now();

        // Measure code structure finding time
        let _code_structure_start = Instant::now();
        let code_structure_duration = Arc::new(Mutex::new(Duration::new(0, 0)));
        let filtering_duration = Arc::new(Mutex::new(Duration::new(0, 0)));
        let result_building_duration = Arc::new(Mutex::new(Duration::new(0, 0)));

        // Track detailed result building timings
        let term_matching_duration = Arc::new(Mutex::new(Duration::new(0, 0)));
        let compound_processing_duration = Arc::new(Mutex::new(Duration::new(0, 0)));
        let line_matching_duration = Arc::new(Mutex::new(Duration::new(0, 0)));
        let result_creation_duration = Arc::new(Mutex::new(Duration::new(0, 0)));
        let synchronization_duration = Arc::new(Mutex::new(Duration::new(0, 0)));

        // Prepare shared resources for parallel processing
        let shared_results = Arc::new(Mutex::new(Vec::new()));
        let shared_covered_lines = Arc::new(Mutex::new(HashSet::new()));

        // Process blocks in parallel
        code_blocks
            .par_iter()
            .enumerate()
            .for_each(|(block_idx, block)| {
                // Start measuring code structure finding time for this block
                let block_start = Instant::now();

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
                    // Skip empty lines (which were originally too long)
                    lines[final_start_line - 1..final_end_line]
                        .to_vec()
                        .join("\n")
                } else {
                    "".to_string()
                };

                // End code structure finding time for this block
                let block_duration = block_start.elapsed();
                {
                    let mut duration = code_structure_duration.lock().unwrap();
                    *duration += block_duration;
                }

                // Start measuring term matching time
                let term_matching_start = Instant::now();

                // Early tokenization with full path prepended
                let block_terms = ranking::preprocess_text_with_filename(
                    &full_code,
                    &params.path.to_string_lossy(),
                );

                // End term matching time measurement
                let term_matching_block_duration = term_matching_start.elapsed();
                {
                    let mut duration = term_matching_duration.lock().unwrap();
                    *duration += term_matching_block_duration;
                }

                // Start measuring filtering time
                let filtering_start = Instant::now();
                // Early filtering using tokenized content
                let should_include = {
                    if debug_mode {
                        println!(
                            "DEBUG: Using filter_tokenized_block for block {}-{}",
                            final_start_line, final_end_line
                        );
                    }

                    // Skip tokenization and evaluation when exact flag is enabled
                    if params.query_plan.exact {
                        // In exact mode, we already matched the lines in the file
                        // so we should include this block without re-evaluating
                        if debug_mode {
                            println!(
                                "DEBUG: Exact mode enabled, skipping tokenization and evaluation for block {}-{}",
                                final_start_line, final_end_line
                            );
                        }
                        true
                    } else {
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
                    }
                };

                // End filtering time measurement
                let filtering_block_duration = filtering_start.elapsed();
                {
                    let mut duration = filtering_duration.lock().unwrap();
                    *duration += filtering_block_duration;
                }

                if debug_mode {
                    println!(
                        "DEBUG: Block lines {}-{} => should_include={}",
                        final_start_line, final_end_line, should_include
                    );
                }

                // Mark lines as covered
                {
                    let mut covered = shared_covered_lines.lock().unwrap();
                    for line_num in final_start_line..=final_end_line {
                        covered.insert(line_num);
                    }
                }

                if should_include {
                    // Start measuring result building time
                    let result_building_start = Instant::now();

                    // Start measuring term matching time
                    let direct_matches_start = Instant::now();

                    // Calculate metrics using the already tokenized content
                    let direct_matches: HashSet<&String> = block_terms
                        .iter()
                        .filter(|t| unique_query_terms.contains(*t))
                        .collect();

                    let direct_matches_duration = direct_matches_start.elapsed();
                    {
                        let mut duration = term_matching_duration.lock().unwrap();
                        *duration += direct_matches_duration;
                    }

                    // Start measuring compound word processing time
                    let compound_start = Instant::now();

                    let mut compound_matches = HashSet::new();
                    // Load vocabulary once before the loop
                    let vocabulary = tokenization::load_vocabulary();
                    for qterm in &unique_query_terms {
                        if block_terms.iter().any(|bt| bt == qterm) {
                            continue;
                        }
                        let parts = tokenization::split_compound_word(qterm, vocabulary);
                        if parts.len() > 1 && parts.iter().all(|part| block_terms.contains(part)) {
                            compound_matches.insert(qterm);
                        }
                    }

                    let compound_duration = compound_start.elapsed();
                    {
                        let mut duration = compound_processing_duration.lock().unwrap();
                        *duration += compound_duration;
                    }

                    let block_unique_terms = direct_matches.len() + compound_matches.len();
                    let block_total_matches = direct_matches.len() + compound_matches.len();

                    // Collect matched keywords
                    let mut matched_keywords = Vec::new();

                    // Add direct matches
                    matched_keywords.extend(direct_matches.iter().map(|s| (*s).clone()));

                    // Add compound matches
                    matched_keywords.extend(compound_matches.iter().map(|s| (*s).clone()));

                    // Start measuring line matching time
                    let line_matching_start = Instant::now();

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

                    let line_matching_duration_value = line_matching_start.elapsed();
                    {
                        let mut duration = line_matching_duration.lock().unwrap();
                        *duration += line_matching_duration_value;
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

                    // Start measuring result creation time
                    let result_creation_start = Instant::now();

                    let result = SearchResult {
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
                    };

                    let result_creation_duration_value = result_creation_start.elapsed();
                    {
                        let mut duration = result_creation_duration.lock().unwrap();
                        *duration += result_creation_duration_value;
                    }

                    // Start measuring synchronization time
                    let sync_start = Instant::now();

                    // Add result to shared results
                    {
                        let mut results = shared_results.lock().unwrap();
                        results.push(result);
                    }

                    let sync_duration = sync_start.elapsed();
                    {
                        let mut duration = synchronization_duration.lock().unwrap();
                        *duration += sync_duration;
                    }

                    // End result building time measurement
                    let result_building_block_duration = result_building_start.elapsed();
                    {
                        let mut duration = result_building_duration.lock().unwrap();
                        *duration += result_building_block_duration;
                    }
                }
            });

        // Extract results from shared resources
        results = Arc::try_unwrap(shared_results)
            .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
            .into_inner()
            .unwrap();

        covered_lines = Arc::try_unwrap(shared_covered_lines)
            .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
            .into_inner()
            .unwrap();

        // Extract durations from Arc<Mutex<>>
        let code_structure_duration_value = Arc::try_unwrap(code_structure_duration)
            .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
            .into_inner()
            .unwrap();

        let filtering_duration_value = Arc::try_unwrap(filtering_duration)
            .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
            .into_inner()
            .unwrap();

        let result_building_duration_value = Arc::try_unwrap(result_building_duration)
            .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
            .into_inner()
            .unwrap();

        // Extract detailed result building timings
        let term_matching_duration_value = Arc::try_unwrap(term_matching_duration)
            .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
            .into_inner()
            .unwrap();

        let compound_processing_duration_value = Arc::try_unwrap(compound_processing_duration)
            .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
            .into_inner()
            .unwrap();

        let line_matching_duration_value = Arc::try_unwrap(line_matching_duration)
            .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
            .into_inner()
            .unwrap();

        let result_creation_duration_value = Arc::try_unwrap(result_creation_duration)
            .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
            .into_inner()
            .unwrap();

        let synchronization_duration_value = Arc::try_unwrap(synchronization_duration)
            .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
            .into_inner()
            .unwrap();

        // Store the sub-step timings
        let block_extraction_duration = block_extraction_start.elapsed();
        timings.block_extraction = Some(block_extraction_duration);
        timings.block_extraction_code_structure = Some(code_structure_duration_value);
        timings.block_extraction_filtering = Some(filtering_duration_value);
        timings.block_extraction_result_building = Some(result_building_duration_value);

        // Store detailed result building timings
        timings.result_building_term_matching = Some(term_matching_duration_value);
        timings.result_building_compound_processing = Some(compound_processing_duration_value);
        timings.result_building_line_matching = Some(line_matching_duration_value);
        timings.result_building_result_creation = Some(result_creation_duration_value);
        timings.result_building_synchronization = Some(synchronization_duration_value);

        if debug_mode {
            println!(
                "DEBUG:   Block extraction time: {:?}",
                block_extraction_duration
            );
            println!(
                "DEBUG:     - Code structure finding: {:?}",
                code_structure_duration_value
            );
            println!("DEBUG:     - Filtering: {:?}", filtering_duration_value);
            println!(
                "DEBUG:     - Result building: {:?}",
                result_building_duration_value
            );
        }
    }

    // Collect all uncovered lines first without processing them
    let mut uncovered_lines = Vec::new();
    for &line_num in params.line_numbers {
        if !covered_lines.contains(&line_num) {
            if debug_mode {
                println!(
                    "DEBUG: Line {} not covered, will use fallback context",
                    line_num
                );
                if line_num <= lines.len() {
                    println!("DEBUG:   Line content: '{}'", lines[line_num - 1].trim());
                }
            }
            uncovered_lines.push(line_num);
        }
    }

    // Start measuring uncovered lines processing time
    let uncovered_lines_start = Instant::now();

    // Process uncovered lines only after all AST blocks have been processed
    for line_num in uncovered_lines {
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
            if let Some(language_impl) = crate::language::factory::get_language_impl(extension) {
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

        // Use a smaller, adaptive context size (5 lines by default)
        // This reduces the chance of overshadowing more specific blocks
        let default_context_size = 5;

        // Calculate 0-based array indices for context
        // line_num is 1-based, so we subtract 1 to get 0-based index
        let line_idx = line_num - 1;
        let context_start_idx = line_idx.saturating_sub(default_context_size);
        let context_end_idx = std::cmp::min(line_idx + default_context_size, lines.len() - 1);

        // Skip if we don't have enough context
        if context_start_idx >= context_end_idx {
            continue;
        }

        // Convert back to 1-based line numbers for display and tracking
        let context_start = context_start_idx + 1;
        let context_end = context_end_idx + 1;

        // Extract the context lines using 0-based indices
        let context_code = lines[context_start_idx..=context_end_idx]
            .to_vec()
            .join("\n");

        // Determine a better node type for the fallback context by analyzing the content
        let node_type = determine_fallback_node_type(lines[line_num - 1], Some(extension));

        if debug_mode {
            println!(
                "DEBUG: Inferred node type for fallback context: {}",
                node_type
            );
            println!(
                "DEBUG: Using adaptive context size: lines {}-{} (size: {})",
                context_start,
                context_end,
                context_end - context_start + 1
            );
        }

        // Start measuring term matching time for uncovered lines
        let term_matching_start = Instant::now();

        // Early tokenization for fallback context
        let context_terms =
            ranking::preprocess_text_with_filename(&context_code, &params.path.to_string_lossy());

        // Add to term matching time
        let term_matching_duration_value = term_matching_start.elapsed();
        if let Some(duration) = timings.result_building_term_matching {
            timings.result_building_term_matching = Some(duration + term_matching_duration_value);
        } else {
            timings.result_building_term_matching = Some(term_matching_duration_value);
        }

        // Start measuring filtering time for uncovered lines
        let filtering_start = Instant::now();

        // Early filtering for fallback context
        let should_include = {
            if debug_mode {
                println!(
                    "DEBUG: Using filter_tokenized_block for fallback context {}-{}",
                    context_start, context_end
                );
            }

            // Skip tokenization and evaluation when exact flag is enabled
            if params.query_plan.exact {
                // In exact mode, we already matched the lines in the file
                // so we should include this block without re-evaluating
                if debug_mode {
                    println!(
                        "DEBUG: Exact mode enabled, skipping tokenization and evaluation for fallback context {}-{}",
                        context_start, context_end
                    );
                }
                true
            } else {
                filter_tokenized_block(
                    &context_terms,
                    &params.query_plan.term_indices,
                    params.query_plan,
                    debug_mode,
                )
            }
        };

        // We don't add this to any timing since filtering is not part of result building
        let _filtering_duration = filtering_start.elapsed();

        if debug_mode {
            println!(
                "DEBUG: Block at {}-{} filtered: included={}",
                context_start, context_end, should_include
            );
        }

        // Only mark these lines as covered if we're including the result
        // This allows for potentially better blocks to be found for these lines later
        if should_include {
            for line in context_start..=context_end {
                covered_lines.insert(line);
            }
        }

        // Add to results only if it passes the filter
        if should_include {
            // Start measuring compound word processing time
            let compound_start = Instant::now();

            // Calculate metrics for fallback context using the already tokenized content
            let direct_matches: HashSet<&String> = context_terms
                .iter()
                .filter(|t| unique_query_terms.contains(*t))
                .collect();

            let mut compound_matches = HashSet::new();
            // Load vocabulary once before the loop
            let vocabulary = tokenization::load_vocabulary();
            for qterm in &unique_query_terms {
                if context_terms.iter().any(|bt| bt == qterm) {
                    continue;
                }
                let parts = tokenization::split_compound_word(qterm, vocabulary);
                if parts.len() > 1 && parts.iter().all(|part| context_terms.contains(part)) {
                    compound_matches.insert(qterm);
                }
            }

            // Add to compound processing time
            let compound_duration = compound_start.elapsed();
            if let Some(duration) = timings.result_building_compound_processing {
                timings.result_building_compound_processing = Some(duration + compound_duration);
            } else {
                timings.result_building_compound_processing = Some(compound_duration);
            }

            let context_unique_terms = direct_matches.len() + compound_matches.len();
            let context_total_matches = direct_matches.len() + compound_matches.len();

            // Collect matched keywords for fallback context
            let mut matched_keywords = Vec::new();

            // Add direct matches
            matched_keywords.extend(direct_matches.iter().map(|s| (*s).clone()));

            // Add compound matches
            matched_keywords.extend(compound_matches.iter().map(|s| (*s).clone()));

            // Start measuring line matching time
            let line_matching_start = Instant::now();

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

            // Add to line matching time
            let line_matching_duration = line_matching_start.elapsed();
            if let Some(duration) = timings.result_building_line_matching {
                timings.result_building_line_matching = Some(duration + line_matching_duration);
            } else {
                timings.result_building_line_matching = Some(line_matching_duration);
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

            // Start measuring result creation time
            let result_creation_start = Instant::now();

            let result = SearchResult {
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
            };

            // Add to result creation time
            let result_creation_duration = result_creation_start.elapsed();
            if let Some(duration) = timings.result_building_result_creation {
                timings.result_building_result_creation = Some(duration + result_creation_duration);
            } else {
                timings.result_building_result_creation = Some(result_creation_duration);
            }

            // Start measuring synchronization time (in this case, just adding to results)
            let sync_start = Instant::now();

            results.push(result);

            // Add to synchronization time
            let sync_duration = sync_start.elapsed();
            if let Some(duration) = timings.result_building_synchronization {
                timings.result_building_synchronization = Some(duration + sync_duration);
            } else {
                timings.result_building_synchronization = Some(sync_duration);
            }
        }
    }

    // End uncovered lines processing time measurement
    let uncovered_lines_duration = uncovered_lines_start.elapsed();
    timings.result_building_uncovered_lines = Some(uncovered_lines_duration);

    if debug_mode {
        println!("DEBUG: File processing timings:");
        if let Some(duration) = timings.file_io {
            println!("DEBUG:   File I/O: {:?}", duration);
        }
        if let Some(duration) = timings.ast_parsing {
            println!("DEBUG:   AST parsing: {:?}", duration);
            if let Some(d) = timings.ast_parsing_language_init {
                println!("DEBUG:     - Language init: {:?}", d);
            }
            if let Some(d) = timings.ast_parsing_parser_init {
                println!("DEBUG:     - Parser init: {:?}", d);
            }
            if let Some(d) = timings.ast_parsing_tree_parsing {
                println!("DEBUG:     - Tree parsing: {:?}", d);
            }
            if let Some(d) = timings.ast_parsing_line_map_building {
                println!("DEBUG:     - Line map building: {:?}", d);
            }
        }
        if let Some(duration) = timings.block_extraction {
            println!("DEBUG:   Block extraction: {:?}", duration);
            if let Some(d) = timings.block_extraction_code_structure {
                println!("DEBUG:     - Code structure finding: {:?}", d);
            }
            if let Some(d) = timings.block_extraction_filtering {
                println!("DEBUG:     - Filtering: {:?}", d);
            }
            if let Some(d) = timings.block_extraction_result_building {
                println!("DEBUG:     - Result building: {:?}", d);
            }
        }
    }

    if debug_mode {
        println!("DEBUG: Detailed result building timings:");
        if let Some(duration) = timings.result_building_term_matching {
            println!("DEBUG:   Term matching: {:?}", duration);
        }
        if let Some(duration) = timings.result_building_compound_processing {
            println!("DEBUG:   Compound word processing: {:?}", duration);
        }
        if let Some(duration) = timings.result_building_line_matching {
            println!("DEBUG:   Line range matching: {:?}", duration);
        }
        if let Some(duration) = timings.result_building_result_creation {
            println!("DEBUG:   Result creation: {:?}", duration);
        }
        if let Some(duration) = timings.result_building_synchronization {
            println!("DEBUG:   Synchronization: {:?}", duration);
        }
        if let Some(duration) = timings.result_building_uncovered_lines {
            println!("DEBUG:   Uncovered lines processing: {:?}", duration);
        }
    }

    Ok((results, timings))
}
