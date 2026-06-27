use crate::extract::symbols::{
    is_c_like_extension, is_standard_text_extension, matches_text_extension,
    recover_c_like_functions,
};
use anyhow::{Context, Result};
use ast_grep_core::language::{Language, TSLanguage};
use ast_grep_core::AstGrep;
use ast_grep_language::SupportLang;
use colored::*;
use ignore::WalkBuilder;
use probe_code::path_resolver::resolve_path;
use rayon::prelude::*; // Added import
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tree_sitter::Node;

/// Represents a match found by ast-grep
pub struct AstMatch {
    pub file_path: PathBuf,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub column_start: usize,
    pub column_end: usize,
    pub matched_text: String,
    pub node_type: String,
}

/// Options for the ast-grep query
pub struct QueryOptions<'a> {
    pub path: &'a Path,
    pub pattern: &'a str,
    pub language: Option<&'a str>,
    pub ignore: &'a [String],
    pub allow_tests: bool,
    pub max_results: Option<usize>,
    pub with_context: bool,
    #[allow(dead_code)]
    pub format: &'a str,
    pub no_gitignore: bool,
    pub strict: bool,
    pub text_extensions: &'a [String],
}

#[derive(Clone, Copy)]
enum ProbeQueryLang {
    Builtin(SupportLang),
    Solidity,
    Crystal,
}

impl Language for ProbeQueryLang {
    fn get_ts_language(&self) -> TSLanguage {
        match self {
            ProbeQueryLang::Builtin(lang) => lang.get_ts_language(),
            ProbeQueryLang::Solidity => tree_sitter_solidity::LANGUAGE.into(),
            ProbeQueryLang::Crystal => tree_sitter_crystal::LANGUAGE.into(),
        }
    }
}

/// Convert a language string to the corresponding SupportLang
fn get_language(lang: &str) -> Option<ProbeQueryLang> {
    match lang.to_lowercase().as_str() {
        "rust" => Some(ProbeQueryLang::Builtin(SupportLang::Rust)),
        "javascript" => Some(ProbeQueryLang::Builtin(SupportLang::JavaScript)),
        "typescript" => Some(ProbeQueryLang::Builtin(SupportLang::TypeScript)),
        "python" => Some(ProbeQueryLang::Builtin(SupportLang::Python)),
        "go" => Some(ProbeQueryLang::Builtin(SupportLang::Go)),
        "c" => Some(ProbeQueryLang::Builtin(SupportLang::C)),
        "cpp" => Some(ProbeQueryLang::Builtin(SupportLang::Cpp)),
        "java" => Some(ProbeQueryLang::Builtin(SupportLang::Java)),
        "ruby" => Some(ProbeQueryLang::Builtin(SupportLang::Ruby)),
        "php" => Some(ProbeQueryLang::Builtin(SupportLang::Php)),
        "swift" => Some(ProbeQueryLang::Builtin(SupportLang::Swift)),
        "haskell" | "hs" | "lhs" => Some(ProbeQueryLang::Builtin(SupportLang::Haskell)),
        "solidity" | "sol" => Some(ProbeQueryLang::Solidity),
        "crystal" | "cr" => Some(ProbeQueryLang::Crystal),
        "csharp" => Some(ProbeQueryLang::Builtin(SupportLang::CSharp)),
        _ => None,
    }
}

/// Get the file extension for a language
fn get_file_extension(lang: &str) -> Vec<&str> {
    match lang.to_lowercase().as_str() {
        "rust" => vec![".rs"],
        "javascript" => vec![".js", ".jsx", ".mjs"],
        "typescript" => vec![".ts", ".tsx"],
        "python" => vec![".py"],
        "go" => vec![".go"],
        "c" => vec![".c", ".h"],
        "cpp" => vec![".cpp", ".hpp", ".cc", ".hh", ".cxx", ".hxx"],
        "java" => vec![".java"],
        "ruby" => vec![".rb"],
        "php" => vec![".php"],
        "swift" => vec![".swift"],
        "haskell" | "hs" | "lhs" => vec![".hs", ".lhs"],
        "solidity" | "sol" => vec![".sol"],
        "crystal" | "cr" => vec![".cr"],
        "csharp" => vec![".cs"],
        _ => vec![],
    }
}

/// Check if a file should be ignored based on its path
fn should_ignore_file(file_path: &Path, options: &QueryOptions) -> bool {
    let path_str = file_path.to_string_lossy();

    // Skip test files if allow_tests is false
    if !options.allow_tests
        && (path_str.contains("/test/")
            || path_str.contains("/tests/")
            || path_str.contains("_test.")
            || path_str.contains("_spec.")
            || path_str.contains(".test.")
            || path_str.contains(".spec."))
    {
        return true;
    }

    // Skip files that match custom ignore patterns
    for pattern in options.ignore {
        if path_str.contains(pattern) {
            return true;
        }
    }

    false
}

/// Perform an ast-grep query on a single file
fn query_file(file_path: &Path, options: &QueryOptions) -> Result<Vec<AstMatch>> {
    // Get the file extension
    let file_ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let user_text_extension = matches_text_extension(file_ext, options.text_extensions);
    let automatic_text_extension =
        options.language.is_none() && !options.strict && is_standard_text_extension(file_ext);
    let force_plain_text = user_text_extension || automatic_text_extension;

    // If language is provided, check if the file has the correct extension
    if let Some(language) = options.language.filter(|_| !force_plain_text) {
        let extensions = get_file_extension(language);
        let has_matching_ext = extensions
            .iter()
            .any(|&ext| file_path.to_string_lossy().ends_with(ext));

        if !has_matching_ext {
            return Ok(vec![]);
        }
    }

    // Read the file content
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

    if force_plain_text {
        return Ok(query_plain_text_file(file_path, &content, options.pattern));
    }

    // Get the language for ast-grep
    let lang = if let Some(language) = options.language {
        // If language is specified, use it
        match get_language(language) {
            Some(lang) => lang,
            None => return Ok(vec![]),
        }
    } else {
        // If language is not specified, try to infer from file extension
        let inferred_lang = match file_ext {
            "rs" => Some(ProbeQueryLang::Builtin(SupportLang::Rust)),
            "js" | "jsx" | "mjs" => Some(ProbeQueryLang::Builtin(SupportLang::JavaScript)),
            "ts" | "tsx" => Some(ProbeQueryLang::Builtin(SupportLang::TypeScript)),
            "py" => Some(ProbeQueryLang::Builtin(SupportLang::Python)),
            "go" => Some(ProbeQueryLang::Builtin(SupportLang::Go)),
            "c" | "h" => Some(ProbeQueryLang::Builtin(SupportLang::C)),
            "cpp" | "hpp" | "cc" | "hh" | "cxx" | "hxx" => {
                Some(ProbeQueryLang::Builtin(SupportLang::Cpp))
            }
            "java" => Some(ProbeQueryLang::Builtin(SupportLang::Java)),
            "rb" => Some(ProbeQueryLang::Builtin(SupportLang::Ruby)),
            "php" => Some(ProbeQueryLang::Builtin(SupportLang::Php)),
            "swift" => Some(ProbeQueryLang::Builtin(SupportLang::Swift)),
            "hs" | "lhs" => Some(ProbeQueryLang::Builtin(SupportLang::Haskell)),
            "sol" => Some(ProbeQueryLang::Solidity),
            "cr" => Some(ProbeQueryLang::Crystal),
            "cs" => Some(ProbeQueryLang::Builtin(SupportLang::CSharp)),
            _ => None, // Unsupported extension
        };

        match inferred_lang {
            Some(lang) => lang,
            None => {
                return if options.strict {
                    Ok(vec![])
                } else {
                    Ok(query_plain_text_file(file_path, &content, options.pattern))
                };
            }
        }
    };

    // Create the document and grep instance
    let grep = AstGrep::new(&content, lang);

    // Create the pattern and find all matches
    let matches = match std::panic::catch_unwind(|| {
        grep.root().find_all(options.pattern).collect::<Vec<_>>()
    }) {
        Ok(matches) => matches,
        Err(_) => {
            // Only print error if language is explicitly specified
            // This suppresses errors during auto-detection
            if options.language.is_some() {
                eprintln!(
                    "Error parsing pattern: '{}' is not a valid ast-grep pattern",
                    options.pattern
                );
            }
            return Ok(vec![]);
        }
    };

    // Convert matches to AstMatch structs
    let mut ast_matches = Vec::new();
    for node in matches {
        let range = node.range();

        // Convert byte offsets to line and column numbers
        let mut line_start = 1;
        let mut column_start = 1;
        let mut line_end = 1;
        let mut column_end = 1;

        let mut current_line = 1;
        let mut current_column = 1;

        for (i, c) in content.char_indices() {
            if i == range.start {
                line_start = current_line;
                column_start = current_column;
            }
            if i == range.end {
                line_end = current_line;
                column_end = current_column;
                break;
            }

            if c == '\n' {
                current_line += 1;
                current_column = 1;
            } else {
                current_column += 1;
            }
        }

        ast_matches.push(AstMatch {
            file_path: file_path.to_path_buf(),
            byte_start: range.start,
            byte_end: range.end,
            line_start,
            line_end,
            column_start,
            column_end,
            matched_text: node.text().to_string(),
            node_type: "match".to_string(),
        });
    }

    supplement_c_like_function_matches(
        &mut ast_matches,
        &content,
        file_path,
        options.pattern,
        options.language,
        file_ext,
    );
    supplement_rust_function_matches(
        &mut ast_matches,
        &content,
        file_path,
        options.pattern,
        options.language,
        file_ext,
    );
    supplement_python_function_matches(
        &mut ast_matches,
        &content,
        file_path,
        options.pattern,
        options.language,
        file_ext,
    );

    Ok(ast_matches)
}

fn query_plain_text_file(file_path: &Path, content: &str, pattern: &str) -> Vec<AstMatch> {
    let mut matches = Vec::new();
    let mut byte_offset = 0usize;

    for (idx, line) in content.lines().enumerate() {
        if let Some(match_start) = line.find(pattern) {
            let line_start = idx + 1;
            let byte_start = byte_offset + match_start;
            let byte_end = byte_offset + line.len();
            matches.push(AstMatch {
                file_path: file_path.to_path_buf(),
                byte_start,
                byte_end,
                line_start,
                line_end: line_start,
                column_start: match_start + 1,
                column_end: line.len() + 1,
                matched_text: line.to_string(),
                node_type: "text".to_string(),
            });
        }
        byte_offset += line.len() + 1;
    }

    matches
}

fn supplement_c_like_function_matches(
    ast_matches: &mut Vec<AstMatch>,
    content: &str,
    file_path: &Path,
    pattern: &str,
    language: Option<&str>,
    file_ext: &str,
) {
    if !should_recover_c_like_functions(pattern, language, file_ext) {
        return;
    }

    let required_prefix = c_like_required_prefix(pattern);
    let mut existing_lines: HashSet<usize> = ast_matches.iter().map(|m| m.line_start).collect();

    for recovered in recover_c_like_functions(content.as_bytes()) {
        if existing_lines.contains(&recovered.line) {
            continue;
        }
        if let Some(prefix) = &required_prefix {
            if !normalize_c_like_signature(&recovered.signature).starts_with(prefix) {
                continue;
            }
        }

        let matched_text = content[recovered.byte_start..recovered.byte_end].to_string();
        let (line_start, column_start) = byte_to_line_column(content, recovered.byte_start);
        let (line_end, column_end) = byte_to_line_column(content, recovered.byte_end);

        existing_lines.insert(line_start);
        ast_matches.push(AstMatch {
            file_path: file_path.to_path_buf(),
            byte_start: recovered.byte_start,
            byte_end: recovered.byte_end,
            line_start,
            line_end,
            column_start,
            column_end,
            matched_text,
            node_type: "match".to_string(),
        });
    }

    ast_matches.sort_by_key(|m| (m.file_path.clone(), m.byte_start));
}

fn supplement_rust_function_matches(
    ast_matches: &mut Vec<AstMatch>,
    content: &str,
    file_path: &Path,
    pattern: &str,
    language: Option<&str>,
    file_ext: &str,
) {
    if !should_recover_rust_functions(pattern, language, file_ext) {
        return;
    }

    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .is_err()
    {
        return;
    }

    let Some(tree) = parser.parse(content, None) else {
        return;
    };

    let mut existing_lines: HashSet<usize> = ast_matches.iter().map(|m| m.line_start).collect();
    collect_rust_function_matches(
        tree.root_node(),
        content,
        file_path,
        ast_matches,
        &mut existing_lines,
    );
    ast_matches.sort_by_key(|m| (m.file_path.clone(), m.byte_start));
}

fn should_recover_rust_functions(pattern: &str, language: Option<&str>, file_ext: &str) -> bool {
    let is_rust = language
        .map(|lang| matches!(lang.to_lowercase().as_str(), "rust" | "rs"))
        .unwrap_or(file_ext == "rs");
    if !is_rust {
        return false;
    }

    let normalized = pattern.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized == "fn $NAME($$$PARAMS) $$$BODY"
}

fn supplement_python_function_matches(
    ast_matches: &mut Vec<AstMatch>,
    content: &str,
    file_path: &Path,
    pattern: &str,
    language: Option<&str>,
    file_ext: &str,
) {
    if !should_recover_python_functions(pattern, language, file_ext) {
        return;
    }

    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .is_err()
    {
        return;
    }

    let Some(tree) = parser.parse(content, None) else {
        return;
    };

    let mut existing_lines: HashSet<usize> = ast_matches.iter().map(|m| m.line_start).collect();
    collect_function_node_matches(
        tree.root_node(),
        "function_definition",
        content,
        file_path,
        ast_matches,
        &mut existing_lines,
    );
    ast_matches.sort_by_key(|m| (m.file_path.clone(), m.byte_start));
}

fn should_recover_python_functions(pattern: &str, language: Option<&str>, file_ext: &str) -> bool {
    let is_python = language
        .map(|lang| matches!(lang.to_lowercase().as_str(), "python" | "py"))
        .unwrap_or(file_ext == "py");
    if !is_python {
        return false;
    }

    let normalized = pattern.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized == "def $NAME($$$PARAMS): $$$BODY"
}

fn collect_rust_function_matches(
    node: Node,
    content: &str,
    file_path: &Path,
    matches: &mut Vec<AstMatch>,
    existing_lines: &mut HashSet<usize>,
) {
    collect_function_node_matches(
        node,
        "function_item",
        content,
        file_path,
        matches,
        existing_lines,
    );
}

fn collect_function_node_matches(
    node: Node,
    target_kind: &str,
    content: &str,
    file_path: &Path,
    matches: &mut Vec<AstMatch>,
    existing_lines: &mut HashSet<usize>,
) {
    if node.kind() == target_kind {
        let line_start = node.start_position().row + 1;
        if existing_lines.insert(line_start) {
            let byte_start = node.start_byte();
            let byte_end = node.end_byte();
            matches.push(AstMatch {
                file_path: file_path.to_path_buf(),
                byte_start,
                byte_end,
                line_start,
                line_end: node.end_position().row + 1,
                column_start: node.start_position().column + 1,
                column_end: node.end_position().column + 1,
                matched_text: content[byte_start..byte_end].to_string(),
                node_type: "match".to_string(),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_function_node_matches(
            child,
            target_kind,
            content,
            file_path,
            matches,
            existing_lines,
        );
    }
}

fn should_recover_c_like_functions(pattern: &str, language: Option<&str>, file_ext: &str) -> bool {
    if !language
        .map(is_c_like_language)
        .unwrap_or_else(|| is_c_like_extension(file_ext))
    {
        return false;
    }

    let trimmed = pattern.trim();
    trimmed == "function_definition"
        || (trimmed.contains("$NAME")
            && trimmed.contains("$$$BODY")
            && trimmed.contains('(')
            && trimmed.contains(')')
            && trimmed.contains('{')
            && trimmed.contains('}'))
}

fn is_c_like_language(language: &str) -> bool {
    matches!(
        language.to_lowercase().as_str(),
        "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hxx"
    )
}

fn c_like_required_prefix(pattern: &str) -> Option<String> {
    let prefix = pattern.split("$NAME").next()?.trim();
    if prefix.is_empty() || prefix.contains('$') {
        return None;
    }
    Some(normalize_c_like_signature(prefix))
}

fn normalize_c_like_signature(signature: &str) -> String {
    signature.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn byte_to_line_column(content: &str, byte: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for (i, ch) in content.char_indices() {
        if i >= byte {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

pub fn perform_query(options: &QueryOptions) -> Result<Vec<AstMatch>> {
    // Suppress panic output if language is not specified
    let suppress_output = options.language.is_none();

    // Set a custom panic hook to suppress panic messages if needed
    let original_hook = if suppress_output {
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {
            // Do nothing, effectively suppressing the panic message
        }));
        Some(original_hook)
    } else {
        None
    };

    // Resolve the path if it's a special format (e.g., "go:github.com/user/repo")
    let resolved_path = if let Some(path_str) = options.path.to_str() {
        match resolve_path(path_str) {
            Ok(resolved_path) => {
                if std::env::var("PROBE_DEBUG").unwrap_or_default() == "1" {
                    println!(
                        "DEBUG: Resolved path '{}' to '{}'",
                        path_str,
                        resolved_path.display()
                    );
                }
                resolved_path
            }
            Err(err) => {
                if std::env::var("PROBE_DEBUG").unwrap_or_default() == "1" {
                    println!("DEBUG: Failed to resolve path '{path_str}': {err}");
                }
                // Fall back to the original path
                options.path.to_path_buf()
            }
        }
    } else {
        // If we can't convert the path to a string, use it as is
        options.path.to_path_buf()
    };

    // Collect file paths using WalkBuilder to conditionally respect gitignore
    let mut builder = WalkBuilder::new(&resolved_path);

    // Follow symlinks by default. Loop detection is handled by walkdir internally -
    // it detects and reports symlink loops as errors, preventing infinite traversal.
    builder.follow_links(true);

    // Configure gitignore handling based on the no_gitignore option
    if !options.no_gitignore {
        builder.git_ignore(true);
        builder.git_global(true);
        builder.git_exclude(true);
    } else {
        builder.git_ignore(false);
        builder.git_global(false);
        builder.git_exclude(false);
    }

    let file_paths: Vec<PathBuf> = builder
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .filter(|entry| !should_ignore_file(entry.path(), options))
        .map(|entry| entry.path().to_path_buf())
        .collect();

    // Process files in parallel
    let all_matches: Vec<AstMatch> = file_paths
        .par_iter()
        .flat_map(|path| {
            std::panic::catch_unwind(|| query_file(path, options))
                .unwrap_or_else(|_| {
                    // Panic was caught, return empty results
                    Ok(vec![])
                })
                .unwrap_or_else(|_| {
                    // Error was caught, return empty results
                    vec![]
                })
        })
        .collect();

    // Restore the original panic hook if we changed it
    if let Some(hook) = original_hook {
        std::panic::set_hook(hook);
    }

    // Apply max_results limit
    let mut all_matches = all_matches;
    if let Some(max) = options.max_results {
        all_matches.truncate(max);
    }

    Ok(all_matches)
}

/// Helper function to escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
}

/// Format and print the query results
pub fn format_and_print_query_results(
    matches: &[AstMatch],
    format: &str,
    pattern: &str,
    with_context: bool,
) -> Result<()> {
    match format {
        "color" | "terminal" => {
            for m in matches {
                println!(
                    "{}",
                    format!(
                        "{}:{}:{}",
                        m.file_path.display(),
                        m.line_start,
                        m.column_start
                    )
                    .cyan()
                );
                println!("{}", m.matched_text.trim());
                println!();
            }
        }
        "plain" => {
            for m in matches {
                println!(
                    "{}:{}:{}",
                    m.file_path.display(),
                    m.line_start,
                    m.column_start
                );
                println!("{}", m.matched_text.trim());
                println!();
            }
        }
        "markdown" => {
            for m in matches {
                println!(
                    "**{}:{}:{}**",
                    m.file_path.display(),
                    m.line_start,
                    m.column_start
                );

                // Determine language for code block
                let lang = m
                    .file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");

                println!("```{lang}");
                println!("{}", m.matched_text.trim());
                println!("```");
                println!();
            }
        }
        "json" => {
            use std::collections::HashMap;

            // BATCH TOKENIZATION WITH DEDUPLICATION OPTIMIZATION for query JSON output:
            // Process all matched text in batch to leverage content deduplication
            use probe_code::search::search_tokens::sum_tokens_with_deduplication;
            use probe_code::semantic_context::ParsedSourceContext;

            let matched_texts: Vec<&str> =
                matches.iter().map(|m| m.matched_text.as_str()).collect();
            let total_tokens = sum_tokens_with_deduplication(&matched_texts);

            let mut parsed_files: HashMap<std::path::PathBuf, Option<ParsedSourceContext>> =
                HashMap::new();

            // Create standardized results
            let json_matches_standardized: Vec<_> = matches
                .iter()
                .map(|m| {
                    let mut result = serde_json::json!({
                        "file": m.file_path.to_string_lossy(),
                        "lines": [m.line_start, m.line_end],
                        "node_type": m.node_type,
                        "content": m.matched_text,
                        "column_start": m.column_start,
                        "column_end": m.column_end
                    });

                    if with_context {
                        let parsed = parsed_files
                            .entry(m.file_path.clone())
                            .or_insert_with(|| ParsedSourceContext::parse(&m.file_path));
                        if let Some(context) = parsed.as_ref().and_then(|parsed| {
                            parsed.query_source_context(m.byte_start, m.byte_end, &m.matched_text)
                        }) {
                            result["language"] = serde_json::json!(context.language);
                            result["pattern"] = serde_json::json!({
                                "source": pattern,
                                "id": serde_json::Value::Null,
                            });
                            result["match"] = serde_json::json!(context.r#match);
                            if let Some(owner) = context.owner {
                                result["owner"] = serde_json::json!(owner);
                            }
                        }
                    }

                    result
                })
                .collect();

            // Create the wrapper object
            let mut wrapper = serde_json::json!({
                "results": json_matches_standardized,
                "summary": {
                    "count": matches.len(),
                    "total_bytes": matches.iter().map(|m| m.matched_text.len()).sum::<usize>(),
                    "total_tokens": total_tokens
                },
                "version": probe_code::version::get_version()
            });
            if with_context {
                wrapper["schema_version"] = serde_json::json!("probe.query.context.v1");
            }

            println!("{}", serde_json::to_string_pretty(&wrapper)?);
        }
        "xml" => {
            println!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
            println!("<probe_results>");

            for m in matches {
                println!("  <result>");
                println!(
                    "    <file>{}</file>",
                    escape_xml(&m.file_path.to_string_lossy())
                );
                println!("    <lines>{}-{}</lines>", m.line_start, m.line_end);
                println!("    <node_type>{}</node_type>", escape_xml(&m.node_type));
                println!("    <column_start>{}</column_start>", m.column_start);
                println!("    <column_end>{}</column_end>", m.column_end);
                println!("    <code><![CDATA[{}]]></code>", m.matched_text.trim());
                println!("  </result>");
            }

            // Add summary section
            println!("  <summary>");
            println!("    <count>{}</count>", matches.len());
            println!(
                "    <total_bytes>{}</total_bytes>",
                matches.iter().map(|m| m.matched_text.len()).sum::<usize>()
            );

            // BATCH TOKENIZATION WITH DEDUPLICATION OPTIMIZATION for query XML output:
            // Process all matched text in batch to leverage content deduplication
            use probe_code::search::search_tokens::sum_tokens_with_deduplication;
            let matched_texts: Vec<&str> =
                matches.iter().map(|m| m.matched_text.as_str()).collect();
            let total_tokens = sum_tokens_with_deduplication(&matched_texts);

            println!("    <total_tokens>{total_tokens}</total_tokens>");
            println!("  </summary>");

            println!(
                "  <version>{}</version>",
                probe_code::version::get_version()
            );

            println!("</probe_results>");
        }
        _ => {
            // Default to color format
            format_and_print_query_results(matches, "color", pattern, with_context)?;
        }
    }

    Ok(())
}

/// Handle the query command
#[allow(clippy::too_many_arguments)]
pub fn handle_query(
    pattern: &str,
    path: &Path,
    language: Option<&str>,
    ignore: &[String],
    allow_tests: bool,
    max_results: Option<usize>,
    format: &str,
    no_gitignore: bool,
    with_context: bool,
    strict: bool,
    text_extensions: Vec<String>,
) -> Result<()> {
    // Print version at the start for text-based formats
    if format != "json" && format != "xml" {
        println!("Probe version: {}", probe_code::version::get_version());
    }

    // Only print information for non-JSON/XML formats
    if format != "json" && format != "xml" {
        println!("{} {}", "Pattern:".bold().green(), pattern);
        println!("{} {}", "Path:".bold().green(), path.display());

        // Print language if provided, otherwise show auto-detect
        if let Some(lang) = language {
            println!("{} {}", "Language:".bold().green(), lang);
        } else {
            println!("{} auto-detect", "Language:".bold().green());
        }

        // Show advanced options if they differ from defaults
        let mut advanced_options = Vec::<String>::new();
        if allow_tests {
            advanced_options.push("Including tests".to_string());
        }
        if no_gitignore {
            advanced_options.push("Ignoring .gitignore".to_string());
        }
        if let Some(max) = max_results {
            advanced_options.push(format!("Max results: {max}"));
        }

        if !advanced_options.is_empty() {
            println!(
                "{} {}",
                "Options:".bold().green(),
                advanced_options.join(", ")
            );
        }
    }

    let start_time = Instant::now();

    let options = QueryOptions {
        path,
        pattern,
        language,
        ignore,
        allow_tests,
        max_results,
        with_context,
        format,
        no_gitignore,
        strict,
        text_extensions: &text_extensions,
    };

    let matches = perform_query(&options)?;

    // Calculate search time
    let duration = start_time.elapsed();

    if matches.is_empty() {
        // For JSON and XML formats, still call format_and_print_query_results
        if format == "json" || format == "xml" {
            format_and_print_query_results(&matches, format, pattern, with_context)?;
        } else {
            // For other formats, print the "No results found" message
            println!("{}", "No results found.".yellow().bold());
            println!("Search completed in {duration:.2?}");
        }
    } else {
        // For non-JSON/XML formats, print search time
        if format != "json" && format != "xml" {
            println!("Found {} matches in {:.2?}", matches.len(), duration);
            println!();
        }

        format_and_print_query_results(&matches, format, pattern, with_context)?;

        // Skip summary for JSON and XML formats
        if format != "json" && format != "xml" {
            // Calculate and display total bytes and tokens
            let total_bytes: usize = matches.iter().map(|m| m.matched_text.len()).sum();
            let total_tokens: usize = matches
                .iter()
                .map(|m| {
                    // Import the count_tokens function locally to avoid unused import warning
                    use probe_code::search::search_tokens::count_tokens;
                    count_tokens(&m.matched_text)
                })
                .sum();

            println!("Total bytes returned: {total_bytes}");
            println!("Total tokens returned: {total_tokens}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_solidity_query_support() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("Counter.sol");
        fs::write(
            &file,
            r#"
contract Counter {
    uint256 private _value;

    function increment() public {
        _value += 1;
    }
}
"#,
        )
        .unwrap();

        let options = QueryOptions {
            path: temp_dir.path(),
            pattern: "function $NAME() public { $$$BODY }",
            language: Some("solidity"),
            ignore: &[],
            allow_tests: true,
            max_results: Some(10),
            with_context: false,
            format: "json",
            no_gitignore: true,
            strict: false,
            text_extensions: &[],
        };

        let matches = perform_query(&options).expect("Solidity query should run");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file_path, file);
        assert!(matches[0].matched_text.contains("function increment()"));
    }

    #[test]
    fn test_crystal_query_support() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("counter.cr");
        fs::write(
            &file,
            r#"
class Counter
  def increment : Int32
    1
  end
end
"#,
        )
        .unwrap();

        let options = QueryOptions {
            path: temp_dir.path(),
            pattern: "def increment : Int32",
            language: Some("crystal"),
            ignore: &[],
            allow_tests: true,
            max_results: Some(10),
            with_context: false,
            format: "json",
            no_gitignore: true,
            strict: false,
            text_extensions: &[],
        };

        let matches = perform_query(&options).expect("Crystal query should run");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file_path, file);
        assert!(matches[0].matched_text.contains("def increment"));
    }
}
