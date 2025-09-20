//! Functions for processing files and extracting code blocks.
//!
//! This module provides functions for processing files and extracting code blocks
//! based on file paths and optional line numbers.
use anyhow::{Context, Result};
use probe_code::extract::symbol_finder::find_symbol_in_file;
use probe_code::language::factory::get_language_impl;
use probe_code::language::parser::parse_file_for_code_blocks;
use probe_code::models::SearchResult;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Process a single file and extract code blocks
///
/// If a line range is specified, we find all AST blocks overlapping that range,
/// merge them into a bounding block, and return it. If no blocks are found, fallback
/// to the literal lines. If only a single line is specified, do the same but for that line.
/// If a symbol is specified, we delegate to `find_symbol_in_file`.
/// If specific lines are provided, we find AST blocks for each line and merge them.
/// If no lines or symbol are specified, return the entire file.
///
/// This function returns a single SearchResult that includes either the merged AST code
/// or the literal lines as a fallback.
#[allow(clippy::too_many_arguments)]
pub fn process_file_for_extraction(
    path: &Path,
    start_line: Option<usize>,
    end_line: Option<usize>,
    symbol: Option<&str>,
    allow_tests: bool,
    context_lines: usize,
    specific_lines: Option<&HashSet<usize>>,
    symbols: bool,
) -> Result<SearchResult> {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        eprintln!("\n[DEBUG] ===== Processing File for Extraction =====");
        eprintln!("[DEBUG] File path: {path:?}");
        eprintln!("[DEBUG] Start line: {start_line:?}");
        eprintln!("[DEBUG] End line: {end_line:?}");
        eprintln!("[DEBUG] Symbol: {symbol:?}");
        eprintln!("[DEBUG] Allow tests: {allow_tests}");
        eprintln!("[DEBUG] Context lines: {context_lines}");
        eprintln!("[DEBUG] Specific lines: {specific_lines:?}");
    }

    // Check if the file exists
    if !path.exists() {
        if debug_mode {
            eprintln!("[DEBUG] Error: File does not exist");
        }
        return Err(anyhow::anyhow!("File does not exist: {:?}", path));
    }

    // Read the file content
    let content = fs::read_to_string(path).context(format!("Failed to read file: {path:?}"))?;
    let lines: Vec<&str> = content.lines().collect();

    if debug_mode {
        eprintln!("[DEBUG] File read successfully");
        eprintln!("[DEBUG] File size: {} bytes", content.len());
        eprintln!("[DEBUG] Line count: {}", lines.len());
    }

    // If we have a symbol, find it in the file
    if let Some(symbol_name) = symbol {
        if debug_mode {
            eprintln!("[DEBUG] Looking for symbol: {symbol_name}");
        }
        // Find the symbol in the file
        return find_symbol_in_file(path, symbol_name, &content, allow_tests, context_lines);
    }

    // If we have a line range (start_line, end_line), gather AST blocks overlapping that range.
    if let (Some(start), Some(end)) = (start_line, end_line) {
        if debug_mode {
            eprintln!("[DEBUG] Extracting line range: {start}-{end} (with AST merging)");
        }

        // Clamp line numbers to valid ranges instead of failing
        // Bound start to 1..lines.len()
        let mut clamped_start = start.clamp(1, lines.len());

        // Bound end to clamped_start..lines.len()
        let mut clamped_end = end.clamp(clamped_start, lines.len());

        // If the start is still larger than the total lines, we know there's literally nothing to extract
        if clamped_start > lines.len() {
            clamped_start = lines.len();
        }

        // If the end is zero or ends up less than the start, just clamp it to the start
        if clamped_end < clamped_start {
            clamped_end = clamped_start;
        }

        if debug_mode && (clamped_start != start || clamped_end != end) {
            eprintln!(
                "[DEBUG] Requested lines {start}-{end} out of range; clamping to {clamped_start}-{clamped_end}"
            );
        }

        // Use the clamped values for the rest of the function
        let start = clamped_start;
        let end = clamped_end;

        // Parse AST for all lines in [start, end]
        let mut needed_lines = HashSet::new();
        for l in start..=end {
            needed_lines.insert(l);
        }

        // If specific_lines is provided, add those lines too
        if let Some(lines_set) = specific_lines {
            for &line in lines_set {
                needed_lines.insert(line);
            }
        }

        let code_blocks_result = parse_file_for_code_blocks(
            &content,
            file_extension(path),
            &needed_lines,
            allow_tests,
            None,
        );

        match code_blocks_result {
            Ok(blocks) if !blocks.is_empty() => {
                // Merge them into a bounding block
                // i.e. from min(block.start_row) to max(block.end_row)
                let min_start = blocks.iter().map(|b| b.start_row).min().unwrap_or(0);
                let max_end = blocks.iter().map(|b| b.end_row).max().unwrap_or(0);

                // Ensure max_end is within bounds of the file
                let max_end = std::cmp::min(max_end, lines.len() - 1);

                // Ensure min_start is not greater than max_end
                let min_start = std::cmp::min(min_start, max_end);

                // lines in the file are 0-indexed internally, so we add 1 for final display
                let merged_start = min_start + 1;
                let merged_end = max_end + 1;

                if debug_mode {
                    eprintln!(
                        "[DEBUG] Found {} overlapping AST blocks, merging into lines {}-{}",
                        blocks.len(),
                        merged_start,
                        merged_end
                    );
                }

                let merged_content = lines[min_start..=max_end].join("\n");

                // Tokenize the content
                let filename = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                let tokenized_content =
                    crate::ranking::preprocess_text_with_filename(&merged_content, &filename);

                Ok(SearchResult {
                    file: path.to_string_lossy().to_string(),
                    lines: (merged_start, merged_end),
                    node_type: "merged_ast_range".to_string(),
                    code: merged_content,
                    symbol_signature: extract_symbol_signature_for_extract(
                        path,
                        &content,
                        merged_start,
                        merged_end,
                        symbols,
                    ),
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
                    matched_keywords: None,
                    matched_lines: None,
                    tokenized_content: Some(tokenized_content),
                    lsp_info: None,
                    parent_context: None,
                })
            }
            _ => {
                // Fallback to literal extraction of lines [start..end]
                if debug_mode {
                    eprintln!(
                        "[DEBUG] No AST blocks found for the range {start}-{end}, falling back to literal lines"
                    );
                }
                let start_idx = start - 1;
                let end_idx = end;
                let range_content = lines[start_idx..end_idx].join("\n");
                // Tokenize the content
                let filename = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                let tokenized_content =
                    crate::ranking::preprocess_text_with_filename(&range_content, &filename);

                Ok(SearchResult {
                    file: path.to_string_lossy().to_string(),
                    lines: (start, end),
                    node_type: "range".to_string(),
                    code: range_content,
                    symbol_signature: extract_symbol_signature_for_extract(
                        path, &content, start, end, symbols,
                    ),
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
                    matched_keywords: None,
                    matched_lines: None,
                    tokenized_content: Some(tokenized_content),
                    lsp_info: None,
                    parent_context: None,
                })
            }
        }
    }
    // Single line extraction
    else if let Some(line_num) = start_line {
        if debug_mode {
            eprintln!("[DEBUG] Single line extraction requested: line {line_num}");
        }
        // Clamp line number to valid range instead of failing
        let clamped_line_num = line_num.clamp(1, lines.len());

        if debug_mode && clamped_line_num != line_num {
            eprintln!(
                "[DEBUG] Requested line {line_num} out of bounds; clamping to {clamped_line_num}"
            );
        }

        // Use the clamped value for the rest of the function
        let line_num = clamped_line_num;

        // We'll parse the AST for just this line
        let mut needed_lines = HashSet::new();
        needed_lines.insert(line_num);

        // If specific_lines is provided, add those lines too
        if let Some(lines_set) = specific_lines {
            for &line in lines_set {
                needed_lines.insert(line);
            }
        }

        match parse_file_for_code_blocks(
            &content,
            file_extension(path),
            &needed_lines,
            allow_tests,
            None,
        ) {
            Ok(blocks) if !blocks.is_empty() => {
                // Merge them into a bounding block (in most cases it should only be one block,
                // but let's be safe if multiple overlap)
                let min_start = blocks.iter().map(|b| b.start_row).min().unwrap_or(0);
                let max_end = blocks.iter().map(|b| b.end_row).max().unwrap_or(0);

                // Ensure max_end is within bounds of the file
                let max_end = std::cmp::min(max_end, lines.len() - 1);

                // Ensure min_start is not greater than max_end
                let min_start = std::cmp::min(min_start, max_end);

                let merged_start = min_start + 1;
                let merged_end = max_end + 1;

                if debug_mode {
                    eprintln!(
                        "[DEBUG] Found {} AST blocks for line {}, merging into lines {}-{}",
                        blocks.len(),
                        line_num,
                        merged_start,
                        merged_end
                    );
                }
                let merged_content = lines[min_start..=max_end].join("\n");

                // Tokenize the content
                let filename = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                let tokenized_content =
                    crate::ranking::preprocess_text_with_filename(&merged_content, &filename);

                Ok(SearchResult {
                    file: path.to_string_lossy().to_string(),
                    lines: (merged_start, merged_end),
                    node_type: "merged_ast_line".to_string(),
                    code: merged_content,
                    symbol_signature: extract_symbol_signature_for_extract(
                        path,
                        &content,
                        merged_start,
                        merged_end,
                        symbols,
                    ),
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
                    matched_keywords: None,
                    matched_lines: None,
                    tokenized_content: Some(tokenized_content),
                    lsp_info: None,
                    parent_context: None,
                })
            }
            _ => {
                // If no AST block found, fallback to the line + context
                if debug_mode {
                    eprintln!(
                        "[DEBUG] No AST blocks found for line {line_num}, using context-based fallback"
                    );
                }

                // Extract context
                let file_line_count = lines.len();
                let start_ctx = if line_num <= context_lines {
                    1
                } else {
                    line_num - context_lines
                };
                let end_ctx = std::cmp::min(line_num + context_lines, file_line_count);

                let start_idx = start_ctx - 1;
                let end_idx = end_ctx;

                let context_code = lines[start_idx..end_idx].join("\n");

                // Tokenize the content
                let filename = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                let tokenized_content =
                    crate::ranking::preprocess_text_with_filename(&context_code, &filename);

                Ok(SearchResult {
                    file: path.to_string_lossy().to_string(),
                    lines: (start_ctx, end_ctx),
                    node_type: "context".to_string(),
                    code: context_code,
                    symbol_signature: extract_symbol_signature_for_extract(
                        path, &content, start_ctx, end_ctx, symbols,
                    ),
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
                    matched_keywords: None,
                    matched_lines: None,
                    tokenized_content: Some(tokenized_content),
                    lsp_info: None,
                    parent_context: None,
                })
            }
        }
    } else if let Some(lines_set) = specific_lines {
        // We have specific lines to extract
        if debug_mode {
            eprintln!("[DEBUG] Extracting specific lines: {lines_set:?}");
        }

        if lines_set.is_empty() {
            if debug_mode {
                eprintln!("[DEBUG] No specific lines provided, returning entire file content");
            }

            // Tokenize the content
            let filename = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            let tokenized_content =
                crate::ranking::preprocess_text_with_filename(&content, &filename);

            return Ok(SearchResult {
                file: path.to_string_lossy().to_string(),
                lines: (1, lines.len()),
                node_type: "file".to_string(),
                code: content.clone(),
                symbol_signature: extract_symbol_signature_for_extract(
                    path,
                    &content,
                    1,
                    lines.len(),
                    symbols,
                ),
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
                matched_keywords: None,
                matched_lines: None,
                tokenized_content: Some(tokenized_content),
                lsp_info: None,
                parent_context: None,
            });
        }

        // Clamp specific lines to valid range instead of failing
        let mut clamped_lines = HashSet::new();
        let mut any_clamped = false;

        for &line in lines_set {
            if line == 0 || line > lines.len() {
                if line > 0 {
                    // Only add lines that are > 0 (clamp to max)
                    clamped_lines.insert(line.min(lines.len()));
                }
                any_clamped = true;
            } else {
                clamped_lines.insert(line);
            }
        }

        if debug_mode && any_clamped {
            eprintln!(
                "[DEBUG] Some requested lines were out of bounds; clamping to valid range 1-{}",
                lines.len()
            );
        }

        // Use the clamped set for the rest of the function
        let lines_set = &clamped_lines;

        // Parse AST for all specified lines
        let code_blocks_result = parse_file_for_code_blocks(
            &content,
            file_extension(path),
            lines_set,
            allow_tests,
            None,
        );

        match code_blocks_result {
            Ok(blocks) if !blocks.is_empty() => {
                // Merge them into a bounding block
                let min_start = blocks.iter().map(|b| b.start_row).min().unwrap_or(0);
                let max_end = blocks.iter().map(|b| b.end_row).max().unwrap_or(0);

                // Ensure max_end is within bounds of the file
                let max_end = std::cmp::min(max_end, lines.len() - 1);

                // Ensure min_start is not greater than max_end
                let min_start = std::cmp::min(min_start, max_end);

                // lines in the file are 0-indexed internally, so we add 1 for final display
                let merged_start = min_start + 1;
                let merged_end = max_end + 1;

                if debug_mode {
                    eprintln!(
                        "[DEBUG] Found {} AST blocks for specific lines, merging into lines {}-{}",
                        blocks.len(),
                        merged_start,
                        merged_end
                    );
                }

                let merged_content = lines[min_start..=max_end].join("\n");

                // Tokenize the content
                let filename = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                let tokenized_content =
                    crate::ranking::preprocess_text_with_filename(&merged_content, &filename);

                Ok(SearchResult {
                    file: path.to_string_lossy().to_string(),
                    lines: (merged_start, merged_end),
                    node_type: "merged_ast_specific_lines".to_string(),
                    code: merged_content,
                    symbol_signature: extract_symbol_signature_for_extract(
                        path,
                        &content,
                        merged_start,
                        merged_end,
                        symbols,
                    ),
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
                    matched_keywords: None,
                    matched_lines: None,
                    tokenized_content: Some(tokenized_content),
                    lsp_info: None,
                    parent_context: None,
                })
            }
            _ => {
                // Fallback to literal extraction of the specific lines
                if debug_mode {
                    eprintln!(
                        "[DEBUG] No AST blocks found for specific lines, falling back to literal lines"
                    );
                }

                // Get the min and max line numbers
                let min_line = *lines_set.iter().min().unwrap_or(&1);
                let max_line = *lines_set.iter().max().unwrap_or(&lines.len());

                // Add some context around the lines
                let start = if min_line <= context_lines {
                    1
                } else {
                    min_line - context_lines
                };
                let end = std::cmp::min(max_line + context_lines, lines.len());

                let start_idx = start - 1;
                let end_idx = end;
                let range_content = lines[start_idx..end_idx].join("\n");

                // Tokenize the content
                let filename = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                let tokenized_content =
                    crate::ranking::preprocess_text_with_filename(&range_content, &filename);

                Ok(SearchResult {
                    file: path.to_string_lossy().to_string(),
                    lines: (start, end),
                    node_type: "specific_lines".to_string(),
                    code: range_content,
                    symbol_signature: extract_symbol_signature_for_extract(
                        path, &content, start, end, symbols,
                    ),
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
                    matched_keywords: None,
                    matched_lines: None,
                    tokenized_content: Some(tokenized_content),
                    lsp_info: None,
                    parent_context: None,
                })
            }
        }
    } else {
        // No line specified, return the entire file
        if debug_mode {
            eprintln!("[DEBUG] No line or range specified, returning entire file content");
        }

        // Tokenize the content
        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let tokenized_content = crate::ranking::preprocess_text_with_filename(&content, &filename);

        Ok(SearchResult {
            file: path.to_string_lossy().to_string(),
            lines: (1, lines.len()),
            node_type: "file".to_string(),
            code: content.clone(),
            symbol_signature: extract_symbol_signature_for_extract(
                path,
                &content,
                1,
                lines.len(),
                symbols,
            ),
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
            matched_keywords: None,
            matched_lines: None,
            tokenized_content: Some(tokenized_content),
            lsp_info: None,
            parent_context: None,
        })
    }
}

/// Helper function to extract symbol signature for a specific line range
/// Returns Some(String) if symbols is true and extraction succeeds, None otherwise
fn extract_symbol_signature_for_extract(
    path: &Path,
    content: &str,
    start_line: usize,
    end_line: usize,
    symbols: bool,
) -> Option<String> {
    if !symbols {
        return None;
    }

    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Get file extension
    let extension = file_extension(path);

    // Get language implementation
    let language_impl = get_language_impl(extension)?;

    if debug_mode {
        eprintln!(
            "[DEBUG] Extracting symbol signature for lines {}-{} in {}",
            start_line,
            end_line,
            path.display()
        );
    }

    // Try to parse the content
    if let Ok(mut parser) = probe_code::language::get_pooled_parser(extension) {
        if let Some(tree) = parser.parse(content, None) {
            // Convert line numbers to byte ranges
            let lines: Vec<&str> = content.lines().collect();

            // Clamp line numbers to valid ranges
            let start_line = start_line.clamp(1, lines.len());
            let end_line = end_line.clamp(start_line, lines.len());

            // Calculate byte offsets for the line range
            let start_byte = if start_line <= 1 {
                0
            } else {
                lines[..start_line - 1]
                    .iter()
                    .map(|l| l.len() + 1)
                    .sum::<usize>()
            };

            let end_byte = if end_line >= lines.len() {
                content.len()
            } else {
                lines[..end_line]
                    .iter()
                    .map(|l| l.len() + 1)
                    .sum::<usize>()
                    .saturating_sub(1)
            };

            if debug_mode {
                eprintln!(
                    "[DEBUG] Line range {}-{} maps to byte range {}-{}",
                    start_line, end_line, start_byte, end_byte
                );
            }

            // Find nodes within the byte range and extract symbol signature
            let root_node = tree.root_node();
            let signature = find_node_and_extract_signature(
                &root_node,
                start_byte,
                end_byte,
                content.as_bytes(),
                &*language_impl,
                debug_mode,
            );

            // Return parser to pool
            probe_code::language::return_pooled_parser(extension, parser);

            signature
        } else {
            if debug_mode {
                eprintln!("[DEBUG] Failed to parse content for symbol signature");
            }
            probe_code::language::return_pooled_parser(extension, parser);
            None
        }
    } else {
        if debug_mode {
            eprintln!("[DEBUG] Failed to get parser for symbol signature extraction");
        }
        None
    }
}

/// Find a node within the specified byte range and extract its symbol signature
fn find_node_and_extract_signature(
    node: &tree_sitter::Node,
    start_byte: usize,
    end_byte: usize,
    source: &[u8],
    language_impl: &dyn probe_code::language::language_trait::LanguageImpl,
    debug_mode: bool,
) -> Option<String> {
    // Check if this node overlaps with the byte range
    if node.start_byte() <= end_byte && node.end_byte() >= start_byte {
        // First, search children to find more specific nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(child_signature) = find_node_and_extract_signature(
                &child,
                start_byte,
                end_byte,
                source,
                language_impl,
                debug_mode,
            ) {
                return Some(child_signature);
            }
        }

        // If no child provides a signature, try the current node
        // Skip root-level nodes like 'source_file' unless they're the only option
        if node.kind() != "source_file"
            || (node.start_byte() == start_byte && node.end_byte() == end_byte)
        {
            if debug_mode {
                eprintln!(
                    "[DEBUG] Checking node of type '{}' for symbol signature (range {}-{})",
                    node.kind(),
                    node.start_byte(),
                    node.end_byte()
                );
            }

            let signature = language_impl.get_symbol_signature(node, source);
            if let Some(ref sig) = signature {
                if debug_mode {
                    eprintln!(
                        "[DEBUG] Found symbol signature for node type '{}': {}",
                        node.kind(),
                        sig
                    );
                }
                return signature;
            } else if debug_mode {
                eprintln!(
                    "[DEBUG] No symbol signature available for node type '{}'",
                    node.kind()
                );
            }
        }
    }
    None
}

/// Extract all root-level symbols from a file
/// Returns a vector of SearchResults, one for each root-level symbol
#[allow(dead_code)]
pub fn extract_all_symbols_from_file(path: &Path, allow_tests: bool) -> Result<Vec<SearchResult>> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        eprintln!("[DEBUG] Extracting all symbols from file: {:?}", path);
    }

    // Check if the file exists
    if !path.exists() {
        return Err(anyhow::anyhow!("File does not exist: {:?}", path));
    }

    // Read the file content
    let content = fs::read_to_string(path).context(format!("Failed to read file: {path:?}"))?;

    // Get file extension and language implementation
    let extension = file_extension(path);
    let language_impl = get_language_impl(extension)
        .ok_or_else(|| anyhow::anyhow!("Unsupported file extension: {}", extension))?;

    if debug_mode {
        eprintln!("[DEBUG] File extension: {}, Language detected", extension);
    }

    // Parse the file with tree-sitter
    let mut results = Vec::new();

    if let Ok(mut parser) = probe_code::language::get_pooled_parser(extension) {
        if let Some(tree) = parser.parse(&content, None) {
            let root_node = tree.root_node();

            if debug_mode {
                eprintln!("[DEBUG] Successfully parsed file, traversing root-level nodes");
            }

            // Find all root-level acceptable parent nodes
            let mut cursor = root_node.walk();
            for child in root_node.children(&mut cursor) {
                if debug_mode {
                    eprintln!(
                        "[DEBUG] Checking root-level node: {} at lines {}-{}",
                        child.kind(),
                        child.start_position().row + 1,
                        child.end_position().row + 1
                    );
                }

                // Skip test nodes if not allowed
                if !allow_tests && language_impl.is_test_node(&child, content.as_bytes()) {
                    if debug_mode {
                        eprintln!("[DEBUG] Skipping test node: {}", child.kind());
                    }
                    continue;
                }

                // Check if this is an acceptable parent (symbol we want to extract)
                if language_impl.is_acceptable_parent(&child) {
                    if debug_mode {
                        eprintln!(
                            "[DEBUG] Found acceptable symbol: {} at lines {}-{}",
                            child.kind(),
                            child.start_position().row + 1,
                            child.end_position().row + 1
                        );
                    }

                    // Get the symbol signature
                    if let Some(signature) =
                        language_impl.get_symbol_signature(&child, content.as_bytes())
                    {
                        let start_line = child.start_position().row + 1;
                        let end_line = child.end_position().row + 1;

                        // Create a SearchResult for this symbol
                        let result = SearchResult {
                            file: path.to_string_lossy().to_string(),
                            lines: (start_line, end_line),
                            node_type: child.kind().to_string(),
                            code: String::new(), // Empty code since we only want the signature
                            symbol_signature: Some(signature),
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
                            matched_keywords: None,
                            matched_lines: None,
                            tokenized_content: None,
                            lsp_info: None,
                            parent_context: None,
                        };

                        results.push(result);

                        if debug_mode {
                            eprintln!(
                                "[DEBUG] Added symbol result: {} (lines {}-{})",
                                child.kind(),
                                start_line,
                                end_line
                            );
                        }
                    } else if debug_mode {
                        eprintln!("[DEBUG] No signature available for node: {}", child.kind());
                    }
                } else if debug_mode {
                    eprintln!("[DEBUG] Node not acceptable as symbol: {}", child.kind());
                }
            }
        } else {
            if debug_mode {
                eprintln!("[DEBUG] Failed to parse file with tree-sitter");
            }
            probe_code::language::return_pooled_parser(extension, parser);
            return Err(anyhow::anyhow!("Failed to parse file: {:?}", path));
        }

        // Return parser to pool
        probe_code::language::return_pooled_parser(extension, parser);
    } else {
        return Err(anyhow::anyhow!("Failed to get parser for file: {:?}", path));
    }

    // Sort results by line number for consistent ordering
    results.sort_by(|a, b| a.lines.0.cmp(&b.lines.0));

    if debug_mode {
        eprintln!(
            "[DEBUG] Found {} symbols in file (sorted by line number)",
            results.len()
        );
        for result in &results {
            eprintln!(
                "[DEBUG]   {} at lines {}-{}",
                result.node_type, result.lines.0, result.lines.1
            );
        }
    }

    Ok(results)
}

/// Helper to get file extension as a &str
fn file_extension(path: &Path) -> &str {
    path.extension().and_then(|ext| ext.to_str()).unwrap_or("")
}
