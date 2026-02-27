//! Functions for finding symbols in files.
//!
//! This module provides functions for finding symbols (functions, structs, classes, etc.)
//! in files using tree-sitter.
//! Supports returning ALL matching symbols for disambiguation when names are ambiguous.

use anyhow::Result;
use probe_code::models::SearchResult;
use std::path::Path;

/// Walk up the AST parent chain to find the enclosing class/struct/namespace name.
/// Returns "ClassName.symbolName" for methods, or just "symbolName" for top-level symbols.
fn get_qualified_name<'a>(
    node: tree_sitter::Node<'a>,
    symbol_name: &str,
    language_impl: &dyn crate::language::language_trait::LanguageImpl,
    content: &[u8],
) -> String {
    let mut current = node.parent();
    while let Some(parent) = current {
        if language_impl.is_acceptable_parent(&parent) {
            // Try to get the identifier name of this parent node
            let mut cursor = parent.walk();
            for child in parent.children(&mut cursor) {
                if child.kind() == "identifier"
                    || child.kind() == "type_identifier"
                    || child.kind() == "field_identifier"
                    || child.kind() == "name"
                {
                    if let Ok(name) = child.utf8_text(content) {
                        // Skip if the parent's name is the same as the symbol
                        // (e.g., the node IS the symbol, not a container)
                        if name != symbol_name {
                            return format!("{}.{}", name, symbol_name);
                        }
                    }
                }
            }
        }
        current = parent.parent();
    }
    symbol_name.to_string()
}

/// Recursively collect ALL AST nodes matching the given symbol name.
/// Unlike `find_symbol_node` which early-returns on the first match,
/// this function pushes every match into the `matches` vector.
///
/// For nested symbols (e.g., "Class.method"), the search is scoped to
/// within the parent node, which typically yields a single result.
fn find_all_symbol_nodes<'a>(
    node: tree_sitter::Node<'a>,
    symbol_parts: &[&str],
    language_impl: &dyn crate::language::language_trait::LanguageImpl,
    content: &'a [u8],
    debug_mode: bool,
    matches: &mut Vec<tree_sitter::Node<'a>>,
) {
    let current_symbol = symbol_parts[0];
    let is_nested = symbol_parts.len() > 1;
    let mut found_here = false;

    // Check if this node is an acceptable parent (function, struct, class, etc.)
    if language_impl.is_acceptable_parent(&node) {
        if debug_mode {
            println!(
                "[DEBUG] [find_all] Checking node type '{}' at {}:{} for symbol '{}'",
                node.kind(),
                node.start_position().row + 1,
                node.start_position().column + 1,
                current_symbol
            );
        }

        // Try to extract the name of this node
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier"
                || child.kind() == "field_identifier"
                || child.kind() == "type_identifier"
                || child.kind() == "property_identifier"
                || child.kind() == "name"
            // PHP uses "name" for identifiers
            {
                if let Ok(name) = child.utf8_text(content) {
                    if debug_mode {
                        println!(
                            "[DEBUG] [find_all] Found identifier: '{name}' (looking for '{current_symbol}')"
                        );
                    }

                    if name == current_symbol {
                        if is_nested {
                            // Found the parent container — search within it for the child symbol
                            if debug_mode {
                                println!(
                                    "[DEBUG] [find_all] Found parent '{}', searching for child '{}'",
                                    current_symbol,
                                    symbol_parts[1]
                                );
                            }
                            let mut inner_cursor = node.walk();
                            for inner_child in node.children(&mut inner_cursor) {
                                find_all_symbol_nodes(
                                    inner_child,
                                    &symbol_parts[1..],
                                    language_impl,
                                    content,
                                    debug_mode,
                                    matches,
                                );
                            }
                            found_here = true;
                        } else {
                            // Simple match — this node IS the symbol
                            if debug_mode {
                                println!(
                                    "[DEBUG] [find_all] Found symbol '{}' in {} at {}:{}-{}:{}",
                                    current_symbol,
                                    node.kind(),
                                    node.start_position().row + 1,
                                    node.start_position().column + 1,
                                    node.end_position().row + 1,
                                    node.end_position().column + 1
                                );
                            }
                            matches.push(node);
                            found_here = true;
                        }
                        break; // Don't check more identifiers of the same node
                    }
                }
            }

            // For function_declarator, we need to look one level deeper
            if child.kind() == "function_declarator" {
                let mut subcursor = child.walk();
                for subchild in child.children(&mut subcursor) {
                    if subchild.kind() == "identifier" {
                        if let Ok(name) = subchild.utf8_text(content) {
                            if name == current_symbol {
                                if is_nested {
                                    let mut inner_cursor = node.walk();
                                    for inner_child in node.children(&mut inner_cursor) {
                                        find_all_symbol_nodes(
                                            inner_child,
                                            &symbol_parts[1..],
                                            language_impl,
                                            content,
                                            debug_mode,
                                            matches,
                                        );
                                    }
                                    found_here = true;
                                } else {
                                    matches.push(node);
                                    found_here = true;
                                }
                                break;
                            }
                        }
                    }
                }
                if found_here {
                    break;
                }
            }
        }
    }

    // Only recurse into children if the current node wasn't matched.
    // If matched, the node IS the result; its children are the body, not siblings.
    if !found_here {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            find_all_symbol_nodes(
                child,
                symbol_parts,
                language_impl,
                content,
                debug_mode,
                matches,
            );
        }
    }
}

/// Text search fallback when AST-based symbol search fails.
/// Searches for lines containing the symbol name and returns a single result with context.
fn text_search_fallback(
    path: &Path,
    symbol: &str,
    content: &str,
    context_lines: usize,
    debug_mode: bool,
) -> Result<Vec<SearchResult>> {
    let symbol_parts: Vec<&str> = symbol.split('.').collect();
    let is_nested_symbol = symbol_parts.len() > 1;

    if debug_mode {
        println!("\n[DEBUG] ===== Symbol Not Found in AST =====");
        println!("[DEBUG] Symbol '{symbol}' not found in AST");
        println!("[DEBUG] Trying text search fallback");
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut found_line = None;

    let search_terms = if is_nested_symbol {
        if debug_mode {
            println!(
                "[DEBUG] Using fallback search for nested symbol: looking for lines containing all parts"
            );
        }
        symbol_parts.to_vec()
    } else {
        vec![symbol]
    };

    if debug_mode {
        println!(
            "[DEBUG] Performing text search for '{:?}' across {} lines",
            search_terms,
            lines.len()
        );
    }

    for (i, line) in lines.iter().enumerate() {
        let found = search_terms.iter().all(|term| line.contains(term));
        if found {
            found_line = Some(i + 1);
            if debug_mode {
                println!(
                    "[DEBUG] Found symbol '{}' in line {}: '{}'",
                    symbol,
                    i + 1,
                    line.trim()
                );
            }
            break;
        }
    }

    if let Some(line_num) = found_line {
        if debug_mode {
            println!("\n[DEBUG] ===== Symbol Found via Text Search =====");
            println!("[DEBUG] Found symbol '{symbol}' using text search at line {line_num}");
        }

        let start_line = line_num.saturating_sub(context_lines);
        let end_line = std::cmp::min(line_num + context_lines, lines.len());

        let start_idx = if start_line > 0 { start_line - 1 } else { 0 };
        let context = lines[start_idx..end_line].join("\n");

        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let tokenized_content = crate::ranking::preprocess_text_with_filename(&context, &filename);

        return Ok(vec![SearchResult {
            file: path.to_string_lossy().to_string(),
            lines: (start_line, end_line),
            node_type: "text_search".to_string(),
            code: context,
            symbol_signature: None,
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
            parent_context: None,
        }]);
    }

    // Nothing found
    Ok(vec![])
}

/// Find ALL symbols matching the given name in a file.
///
/// Returns a Vec of SearchResults — one for each match. When a bare symbol name
/// like "process" matches multiple definitions (e.g., a top-level function AND class methods),
/// all matches are returned with `symbol_signature` set to qualified names for disambiguation
/// (e.g., "process", "DataProcessor.process", "StreamProcessor.process").
///
/// For dotted (nested) symbols like "DataProcessor.process", the search is scoped to
/// within the parent node, typically returning a single result.
///
/// # Arguments
///
/// * `path` - The path to the file to search in
/// * `symbol` - The name of the symbol to find (may include dots for nested symbols)
/// * `content` - The content of the file
/// * `_allow_tests` - Whether to include test files and test code blocks
/// * `context_lines` - Number of context lines to include (used in text search fallback)
///
/// # Returns
///
/// A Vec of SearchResults, or an error if the file couldn't be parsed.
/// Returns an empty Vec if no matches found (after text search fallback also fails).
pub fn find_all_symbols_in_file(
    path: &Path,
    symbol: &str,
    content: &str,
    _allow_tests: bool,
    context_lines: usize,
) -> Result<Vec<SearchResult>> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    let symbol_parts: Vec<&str> = symbol.split('.').collect();
    let leaf_symbol = *symbol_parts.last().unwrap_or(&symbol);

    if debug_mode {
        println!("\n[DEBUG] ===== Symbol Search (find_all) =====");
        if symbol_parts.len() > 1 {
            println!("[DEBUG] Searching for nested symbol '{symbol}' in file {path:?}");
        } else {
            println!("[DEBUG] Searching for symbol '{symbol}' in file {path:?}");
        }
        println!(
            "[DEBUG] Content size: {} bytes, {} lines",
            content.len(),
            content.lines().count()
        );
    }

    // Get the file extension to determine the language
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

    // Get the language implementation for this extension
    let language_impl = match crate::language::factory::get_language_impl(extension) {
        Some(impl_) => impl_,
        None => {
            if debug_mode {
                println!(
                    "[DEBUG] Language '{extension}' not supported for AST parsing, returning full file"
                );
            }
            // Return the entire file as a single SearchResult when language is unsupported
            let lines: Vec<&str> = content.lines().collect();
            let filename = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            let tokenized_content =
                crate::ranking::preprocess_text_with_filename(content, &filename);

            return Ok(vec![SearchResult {
                file: path.to_string_lossy().to_string(),
                lines: (1, lines.len()),
                node_type: "file".to_string(),
                code: content.to_string(),
                symbol_signature: None,
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
                parent_context: None,
            }]);
        }
    };

    if debug_mode {
        println!("[DEBUG] Language detected: {extension}");
        println!("[DEBUG] Using tree-sitter to parse file");
    }

    // Parse the file with tree-sitter using pooled parser
    let mut parser = crate::language::get_pooled_parser(extension)
        .map_err(|e| anyhow::anyhow!("Failed to get pooled parser: {}", e))?;

    let tree = parser
        .parse(content.as_bytes(), None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse file"))?;

    // Return parser to pool for reuse
    crate::language::return_pooled_parser(extension, parser);

    let root_node = tree.root_node();

    if debug_mode {
        println!("[DEBUG] File parsed successfully");
        println!("[DEBUG] Root node type: {}", root_node.kind());
        println!("[DEBUG] Searching for all matches of '{symbol}' in AST");
    }

    // Collect all matching nodes
    let mut matched_nodes = Vec::new();
    find_all_symbol_nodes(
        root_node,
        &symbol_parts,
        language_impl.as_ref(),
        content.as_bytes(),
        debug_mode,
        &mut matched_nodes,
    );

    if debug_mode {
        println!(
            "[DEBUG] Found {} AST matches for '{symbol}'",
            matched_nodes.len()
        );
    }

    // For nested symbols (e.g., "Type.Method"), if AST search failed,
    // try receiver-based method resolution. This handles languages like Go
    // where methods are declared at the module level with a receiver parameter
    // (e.g., `func (r *Type) Method()`) instead of being nested inside the type.
    if matched_nodes.is_empty() && symbol_parts.len() > 1 {
        let parent_name = symbol_parts[0];
        let method_name = symbol_parts[1];

        if debug_mode {
            println!(
                "[DEBUG] Trying receiver-based method resolution for '{parent_name}.{method_name}'"
            );
        }

        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            if language_impl.is_acceptable_parent(&child) {
                if let Some(receiver_type) =
                    language_impl.get_receiver_type(&child, content.as_bytes())
                {
                    if receiver_type == parent_name {
                        // Check if this node's method name matches
                        let mut name_cursor = child.walk();
                        for name_child in child.children(&mut name_cursor) {
                            if name_child.kind() == "field_identifier"
                                || name_child.kind() == "identifier"
                                || name_child.kind() == "property_identifier"
                            {
                                if let Ok(name) = name_child.utf8_text(content.as_bytes()) {
                                    if name == method_name {
                                        if debug_mode {
                                            println!(
                                                "[DEBUG] Found receiver method '{parent_name}.{method_name}' at lines {}-{}",
                                                child.start_position().row + 1,
                                                child.end_position().row + 1
                                            );
                                        }
                                        matched_nodes.push(child);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if debug_mode {
            println!(
                "[DEBUG] Receiver-based resolution found {} matches",
                matched_nodes.len()
            );
        }
    }

    // If no AST matches, fall back to text search
    if matched_nodes.is_empty() {
        return text_search_fallback(path, symbol, content, context_lines, debug_mode);
    }

    // Build SearchResults with qualified names for disambiguation
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    let results: Vec<SearchResult> = matched_nodes
        .into_iter()
        .map(|found_node| {
            let node_start_line = found_node.start_position().row + 1;
            let node_end_line = found_node.end_position().row + 1;
            let node_text = &content[found_node.start_byte()..found_node.end_byte()];
            let node_text_str = node_text.to_string();

            // Build qualified name for disambiguation
            let qualified_name = get_qualified_name(
                found_node,
                leaf_symbol,
                language_impl.as_ref(),
                content.as_bytes(),
            );

            if debug_mode {
                println!(
                    "[DEBUG] Match: {} ({}) at lines {}-{}",
                    qualified_name,
                    found_node.kind(),
                    node_start_line,
                    node_end_line
                );
            }

            let tokenized_content =
                crate::ranking::preprocess_text_with_filename(&node_text_str, &filename);

            SearchResult {
                file: path.to_string_lossy().to_string(),
                lines: (node_start_line, node_end_line),
                node_type: found_node.kind().to_string(),
                code: node_text_str,
                symbol_signature: Some(qualified_name),
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
                parent_context: None,
            }
        })
        .collect();

    Ok(results)
}

/// Find a symbol (function, struct, class, etc.) in a file by name.
///
/// Returns the FIRST matching symbol. For disambiguation when multiple symbols
/// share the same name, use `find_all_symbols_in_file` instead.
///
/// # Arguments
///
/// * `path` - The path to the file to search in
/// * `symbol` - The name of the symbol to find
/// * `content` - The content of the file
/// * `_allow_tests` - Whether to include test files and test code blocks
/// * `context_lines` - Number of context lines to include
///
/// # Returns
///
/// A SearchResult containing the extracted code block for the symbol, or an error
/// if the symbol couldn't be found.
pub fn find_symbol_in_file(
    path: &Path,
    symbol: &str,
    content: &str,
    _allow_tests: bool,
    context_lines: usize,
) -> Result<SearchResult> {
    let results = find_all_symbols_in_file(path, symbol, content, _allow_tests, context_lines)?;
    results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Symbol '{}' not found in file {:?}", symbol, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_find_symbol_unsupported_language_returns_full_file() {
        // Create a temporary terraform file for testing
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_symbol_terraform.tf");

        let content = r#"resource "test" "example" {
  name = "test"

  variable "nodes" {
    type = map(number)
  }
}"#;

        let mut file = fs::File::create(&test_file).unwrap();
        write!(file, "{content}").unwrap();

        // Try to find a symbol in a .tf file (unsupported language)
        let result = find_symbol_in_file(&test_file, "nodes", content, true, 0);

        // Should return Ok with the full file content
        assert!(result.is_ok());
        let search_result = result.unwrap();
        assert_eq!(search_result.node_type, "file");
        assert_eq!(search_result.code, content);
        assert_eq!(search_result.lines, (1, 7)); // 7 lines in the content

        // Clean up
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_find_symbol_in_rust_file() {
        // Create a temporary rust file for testing
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_symbol_rust.rs");

        let content = r#"fn hello_world() {
    println!("Hello, world!");
}

fn test_function() {
    let x = 42;
}"#;

        let mut file = fs::File::create(&test_file).unwrap();
        write!(file, "{content}").unwrap();

        // Try to find a symbol in a .rs file (supported language)
        let result = find_symbol_in_file(&test_file, "test_function", content, true, 0);

        // Should return Ok with just the test_function
        assert!(result.is_ok());
        let search_result = result.unwrap();
        assert_eq!(search_result.node_type, "function_item");
        assert!(search_result.code.contains("fn test_function()"));
        assert!(search_result.code.contains("let x = 42"));

        // Clean up
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_typescript_class_method_extraction() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_typescript_class.ts");

        let content = r#"export class FailureConditionEvaluator {
  private sandbox: Sandbox;

  /**
   * Secure expression evaluation using SandboxJS
   */
  private evaluateExpression(condition: string, context: any): boolean {
    return true;
  }

  public anotherMethod(): void {
    console.log("test");
  }
}"#;

        let mut file = fs::File::create(&test_file).unwrap();
        write!(file, "{content}").unwrap();

        // Test simple method extraction
        let result = find_symbol_in_file(&test_file, "evaluateExpression", content, true, 0);
        assert!(result.is_ok(), "Should find evaluateExpression method");
        let search_result = result.unwrap();
        assert_eq!(search_result.node_type, "method_definition");
        assert!(search_result.code.contains("evaluateExpression"));

        // Test nested class.method extraction
        let result = find_symbol_in_file(
            &test_file,
            "FailureConditionEvaluator.evaluateExpression",
            content,
            true,
            0,
        );
        assert!(
            result.is_ok(),
            "Should find nested FailureConditionEvaluator.evaluateExpression"
        );
        let search_result = result.unwrap();
        assert_eq!(search_result.node_type, "method_definition");
        assert!(search_result.code.contains("evaluateExpression"));
        assert!(search_result.code.contains("private"));

        // Test other method in the class
        let result = find_symbol_in_file(
            &test_file,
            "FailureConditionEvaluator.anotherMethod",
            content,
            true,
            0,
        );
        assert!(
            result.is_ok(),
            "Should find nested FailureConditionEvaluator.anotherMethod"
        );
        let search_result = result.unwrap();
        assert_eq!(search_result.node_type, "method_definition");
        assert!(search_result.code.contains("anotherMethod"));
        assert!(search_result.code.contains("console.log"));

        // Clean up
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_typescript_private_public_methods() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_typescript_visibility.ts");

        let content = r#"class TestClass {
  private privateMethod(): string {
    return "private";
  }

  public publicMethod(): string {
    return "public";
  }

  protected protectedMethod(): string {
    return "protected";
  }

  regularMethod(): string {
    return "regular";
  }
}"#;

        let mut file = fs::File::create(&test_file).unwrap();
        write!(file, "{content}").unwrap();

        // Test different visibility modifiers
        let methods = [
            "privateMethod",
            "publicMethod",
            "protectedMethod",
            "regularMethod",
        ];

        for method in &methods {
            let result =
                find_symbol_in_file(&test_file, &format!("TestClass.{method}"), content, true, 0);
            assert!(result.is_ok(), "Should find TestClass.{method} method");
            let search_result = result.unwrap();
            assert_eq!(search_result.node_type, "method_definition");
            assert!(search_result.code.contains(method));
        }

        // Clean up
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_find_all_symbols_with_duplicate_names() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_duplicate_symbols.js");

        let content = r#"// Top-level function named "process"
function process(data) {
  return data.map(x => x * 2);
}

// Class with a method also named "process"
class DataProcessor {
  constructor(name) {
    this.name = name;
  }

  process(data) {
    return data.filter(x => x > 0);
  }

  validate(data) {
    return data.every(x => typeof x === 'number');
  }
}

// Another class with "process" method
class StreamProcessor {
  process(stream) {
    return stream.pipe(this.transform);
  }
}

// Top-level function named "validate"
function validate(input) {
  return input !== null && input !== undefined;
}"#;

        let mut file = fs::File::create(&test_file).unwrap();
        write!(file, "{content}").unwrap();

        // Test: find_all_symbols_in_file should return ALL 3 "process" symbols
        let results = find_all_symbols_in_file(&test_file, "process", content, true, 0).unwrap();
        assert_eq!(
            results.len(),
            3,
            "Should find 3 symbols named 'process': got {} ({:?})",
            results.len(),
            results
                .iter()
                .map(|r| &r.symbol_signature)
                .collect::<Vec<_>>()
        );

        // Check that we have the right types
        let node_types: Vec<&str> = results.iter().map(|r| r.node_type.as_str()).collect();
        assert!(
            node_types.contains(&"function_declaration"),
            "Should include top-level function_declaration"
        );
        assert!(
            node_types
                .iter()
                .filter(|t| **t == "method_definition")
                .count()
                == 2,
            "Should include 2 method_definitions"
        );

        // Check qualified names for disambiguation
        let qualified_names: Vec<&str> = results
            .iter()
            .filter_map(|r| r.symbol_signature.as_deref())
            .collect();
        assert!(
            qualified_names.contains(&"process"),
            "Should have top-level 'process': {:?}",
            qualified_names
        );
        assert!(
            qualified_names.contains(&"DataProcessor.process"),
            "Should have 'DataProcessor.process': {:?}",
            qualified_names
        );
        assert!(
            qualified_names.contains(&"StreamProcessor.process"),
            "Should have 'StreamProcessor.process': {:?}",
            qualified_names
        );

        // Test: find_all_symbols_in_file with "validate" should return 2 results
        let results = find_all_symbols_in_file(&test_file, "validate", content, true, 0).unwrap();
        assert_eq!(
            results.len(),
            2,
            "Should find 2 symbols named 'validate': got {} ({:?})",
            results.len(),
            results
                .iter()
                .map(|r| &r.symbol_signature)
                .collect::<Vec<_>>()
        );

        // Test: find_symbol_in_file returns only the FIRST match (backward compat)
        let result = find_symbol_in_file(&test_file, "process", content, true, 0).unwrap();
        assert_eq!(result.node_type, "function_declaration");
        assert!(result.code.contains("data.map"));

        // Test: dotted name returns exactly 1 result (scoped)
        let results =
            find_all_symbols_in_file(&test_file, "DataProcessor.process", content, true, 0)
                .unwrap();
        assert_eq!(
            results.len(),
            1,
            "Dotted name should return exactly 1 result"
        );
        assert!(results[0].code.contains("data.filter"));

        // Clean up
        let _ = fs::remove_file(&test_file);
    }
}
