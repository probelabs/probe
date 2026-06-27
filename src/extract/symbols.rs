//! Symbol tree extraction for files.
//!
//! Provides a table-of-contents view of a file's symbols (functions, structs, classes,
//! constants, etc.) with line numbers and nesting.

use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::Node;

use crate::language::{factory::get_language_impl, get_pooled_parser, return_pooled_parser};

/// Maximum nesting depth for recursive symbol collection.
/// Prevents excessive output and stack depth for deeply nested structures.
/// Depth 3 covers: module → class/impl → method, which handles all common patterns.
const MAX_SYMBOL_DEPTH: usize = 3;

/// A node in the symbol tree.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolNode {
    pub name: String,
    pub kind: String,
    pub signature: String,
    pub line: usize,
    pub end_line: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SymbolNode>,
}

/// Symbols extracted from a single file.
#[derive(Debug, Clone, Serialize)]
pub struct FileSymbols {
    pub file: String,
    pub symbols: Vec<SymbolNode>,
}

#[derive(Debug, Clone, Default)]
pub struct SymbolOptions {
    pub allow_tests: bool,
    pub strict: bool,
    pub text_extensions: Vec<String>,
}

/// Extract the symbol tree from a file.
pub fn extract_symbols(path: &Path, allow_tests: bool) -> Result<FileSymbols> {
    extract_symbols_with_options(
        path,
        &SymbolOptions {
            allow_tests,
            ..SymbolOptions::default()
        },
    )
}

/// Extract the symbol tree from a file with configurable fallback behavior.
pub fn extract_symbols_with_options(path: &Path, options: &SymbolOptions) -> Result<FileSymbols> {
    if !path.exists() {
        return Err(anyhow::anyhow!("File does not exist: {:?}", path));
    }

    let content =
        std::fs::read_to_string(path).context(format!("Failed to read file: {path:?}"))?;

    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

    let language_impl = get_language_impl(extension);
    let user_text_extension = matches_text_extension(extension, &options.text_extensions);
    let automatic_text_extension = is_standard_text_extension(extension);
    if user_text_extension || (!options.strict && automatic_text_extension) {
        return Ok(extract_plain_text_symbols(path, &content));
    }
    if language_impl.is_none() {
        if options.strict {
            return Err(anyhow::anyhow!("Unsupported file extension: {}", extension));
        }
        return Ok(extract_plain_text_symbols(path, &content));
    }

    let language_impl = language_impl.expect("checked language implementation presence");

    let mut parser = get_pooled_parser(extension)
        .map_err(|_| anyhow::anyhow!("Failed to get parser for extension: {}", extension))?;

    let tree = parser
        .parse(&content, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse file: {:?}", path))?;

    let root = tree.root_node();
    let source = content.as_bytes();

    let mut symbols = collect_symbols(
        &root,
        source,
        language_impl.as_ref(),
        options.allow_tests,
        0,
    );
    if is_c_like_extension(extension) {
        merge_recovered_c_like_functions(&mut symbols, source);
    }

    return_pooled_parser(extension, parser);

    Ok(FileSymbols {
        file: path.to_string_lossy().to_string(),
        symbols,
    })
}

fn extract_plain_text_symbols(path: &Path, content: &str) -> FileSymbols {
    let symbols = content
        .lines()
        .enumerate()
        .map(|(idx, line)| SymbolNode {
            name: String::new(),
            kind: "text".to_string(),
            signature: line.to_string(),
            line: idx + 1,
            end_line: idx + 1,
            children: Vec::new(),
        })
        .collect();

    FileSymbols {
        file: path.to_string_lossy().to_string(),
        symbols,
    }
}

pub(crate) fn normalize_extension(extension: &str) -> String {
    extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase()
}

pub(crate) fn is_standard_text_extension(extension: &str) -> bool {
    matches!(
        normalize_extension(extension).as_str(),
        "1" | "5" | "txt" | "conf" | "tex" | "sh" | "json"
    )
}

pub(crate) fn matches_text_extension(extension: &str, text_extensions: &[String]) -> bool {
    let extension = normalize_extension(extension);
    text_extensions
        .iter()
        .map(|ext| normalize_extension(ext))
        .any(|ext| ext == extension)
}

/// Container node kinds that can have child symbols.
fn is_container_node(kind: &str) -> bool {
    matches!(
        kind,
        "impl_item"
            | "trait_item"
            | "mod_item"
            | "class_declaration"
            | "class_definition"
            | "interface_declaration"
            | "namespace_declaration"
            | "module_declaration"
            | "module"
            | "contract_declaration"
            | "library_declaration"
            | "class_def"
            | "module_def"
            | "struct_def"
            | "enum_def"
            | "lib_def"
            | "union_def"
            | "class"
            | "instance"
            | "class_declarations"
            | "instance_declarations"
            | "enum_declaration"
            | "enum_item"
            | "struct_declaration"
            | "contract_body"
            | "declaration_list"
            | "class_body"
            | "body_statement"
            | "block"
    )
}

/// Recursively collect symbols from an AST node.
fn collect_symbols(
    node: &Node,
    source: &[u8],
    lang: &dyn crate::language::language_trait::LanguageImpl,
    allow_tests: bool,
    depth: usize,
) -> Vec<SymbolNode> {
    let mut symbols = Vec::with_capacity(node.child_count().min(32));
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        // Skip test nodes if not allowed
        if !allow_tests && lang.is_test_node(&child, source) {
            continue;
        }

        if lang.is_symbol_node(&child) {
            let semantic_child = semantic_symbol_node(&child, lang, source).unwrap_or(child);
            let signature = if let Some(signature) = lang.get_symbol_signature(&child, source) {
                signature
            } else if lang.allow_symbol_signature_fallback(&child) {
                {
                    // Fallback: use the first line of the node text
                    let text = semantic_child.utf8_text(source).unwrap_or("");
                    text.lines().next().unwrap_or("").trim().to_string()
                }
            } else {
                continue;
            };

            let name = extract_symbol_name(&semantic_child, source);
            let kind = normalize_kind(semantic_child.kind());
            let start_line = semantic_child.start_position().row + 1;
            let end_line = semantic_child.end_position().row + 1;

            // Recursively collect children for container nodes
            let children = if is_container_node(semantic_child.kind()) && depth < MAX_SYMBOL_DEPTH {
                collect_children_symbols(&semantic_child, source, lang, allow_tests, depth + 1)
            } else {
                Vec::new()
            };

            symbols.push(SymbolNode {
                name,
                kind,
                signature,
                line: start_line,
                end_line,
                children,
            });
        } else if child.child_count() > 0 {
            symbols.extend(collect_symbols(&child, source, lang, allow_tests, depth));
        }
    }

    symbols
}

/// Return the actual declaration represented by wrapper nodes such as
/// TypeScript/JavaScript `export_statement`.
fn semantic_symbol_node<'a>(
    node: &Node<'a>,
    lang: &dyn crate::language::language_trait::LanguageImpl,
    source: &[u8],
) -> Option<Node<'a>> {
    if !matches!(node.kind(), "export_statement" | "declare_statement") {
        return None;
    }

    let mut cursor = node.walk();
    let found = node.children(&mut cursor).find(|child| {
        lang.is_symbol_node(child) && extract_symbol_name(child, source) != child.kind()
    });
    found
}

/// Collect child symbols from inside a container node's body.
fn collect_children_symbols(
    node: &Node,
    source: &[u8],
    lang: &dyn crate::language::language_trait::LanguageImpl,
    allow_tests: bool,
    depth: usize,
) -> Vec<SymbolNode> {
    // Look for the body/block child that contains the actual children
    if let Some(body) = node
        .child_by_field_name("body")
        .or_else(|| node.child_by_field_name("members"))
    {
        let symbols = collect_symbols(&body, source, lang, allow_tests, depth);
        if !symbols.is_empty() {
            return symbols;
        }
    }

    // Try finding a body node among direct children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "declaration_list"
                | "class_body"
                | "block"
                | "field_declaration_list"
                | "enum_body"
                | "struct_body"
                | "contract_body"
                | "expressions"
                | "object_type"
                | "interface_body"
                | "statement_block"
                | "class_declarations"
                | "instance_declarations"
                | "declarations"
                | "body_statement"
        ) {
            let symbols = collect_symbols(&child, source, lang, allow_tests, depth);
            if !symbols.is_empty() {
                return symbols;
            }
        }
    }

    // Fallback: try collecting directly from the node's children
    collect_symbols(node, source, lang, allow_tests, depth)
}

/// Extract a symbol name from an AST node.
fn extract_symbol_name(node: &Node, source: &[u8]) -> String {
    if node.kind() == "function_definition" {
        if let Some(name) = extract_c_like_function_name(node, source) {
            return name;
        }
    }

    // Try common field names for the symbol's name
    if let Some(name_node) = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("type"))
    {
        if let Ok(text) = name_node.utf8_text(source) {
            return text.to_string();
        }
    }

    // For impl blocks, try to construct "impl Type" or "impl Trait for Type"
    if node.kind() == "impl_item" {
        let mut parts = Vec::new();
        if let Some(trait_node) = node.child_by_field_name("trait") {
            if let Ok(text) = trait_node.utf8_text(source) {
                parts.push(text.to_string());
                parts.push("for".to_string());
            }
        }
        if let Some(type_node) = node.child_by_field_name("type") {
            if let Ok(text) = type_node.utf8_text(source) {
                parts.push(text.to_string());
            }
        }
        if !parts.is_empty() {
            return parts.join(" ");
        }
    }

    if node.kind() == "constructor_definition" {
        return "constructor".to_string();
    }

    if node.kind() == "fallback_receive_definition" {
        if let Ok(text) = node.utf8_text(source) {
            let trimmed = text.trim_start();
            if trimmed.starts_with("receive") {
                return "receive".to_string();
            }
        }
        return "fallback".to_string();
    }

    // For variable declarations/const, try to find the identifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "identifier"
                | "type_identifier"
                | "property_identifier"
                | "constant"
                | "name"
                | "variable"
                | "constructor"
                | "module_id"
                | "field_name"
                | "prefix_id"
        ) {
            if let Ok(text) = child.utf8_text(source) {
                return text.to_string();
            }
        }
        // For variable_declarator inside lexical_declaration
        if child.kind() == "variable_declarator" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if let Ok(text) = name_node.utf8_text(source) {
                    return text.to_string();
                }
            }
        }
    }

    // Fallback: use node kind
    node.kind().to_string()
}

fn extract_c_like_function_name(node: &Node, source: &[u8]) -> Option<String> {
    let declarator = node
        .child_by_field_name("declarator")
        .or_else(|| find_child_by_kind(node, "function_declarator"))?;
    extract_identifier_from_declarator(&declarator, source)
}

fn find_child_by_kind<'a>(node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let found = node
        .children(&mut cursor)
        .find(|child| child.kind() == kind);
    found
}

fn extract_identifier_from_declarator(node: &Node, source: &[u8]) -> Option<String> {
    if matches!(
        node.kind(),
        "identifier" | "field_identifier" | "qualified_identifier"
    ) {
        return node.utf8_text(source).ok().map(String::from);
    }

    if let Some(declarator) = node.child_by_field_name("declarator") {
        if let Some(name) = extract_identifier_from_declarator(&declarator, source) {
            return Some(name);
        }
    }

    if let Some(name) = node.child_by_field_name("name") {
        if let Ok(text) = name.utf8_text(source) {
            return Some(text.to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "parameter_list" {
            continue;
        }
        if let Some(name) = extract_identifier_from_declarator(&child, source) {
            return Some(name);
        }
    }

    None
}

pub(crate) struct RecoveredCLikeFunction {
    pub(crate) name: String,
    pub(crate) signature: String,
    pub(crate) line: usize,
    pub(crate) end_line: usize,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
}

pub(crate) fn is_c_like_extension(extension: &str) -> bool {
    matches!(extension, "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hxx")
}

fn merge_recovered_c_like_functions(symbols: &mut Vec<SymbolNode>, source: &[u8]) {
    let mut existing = HashSet::new();
    collect_function_keys(symbols, &mut existing);

    for recovered in recover_c_like_functions(source)
        .into_iter()
        .map(RecoveredCLikeFunction::into_symbol)
    {
        if existing.insert((recovered.name.clone(), recovered.line)) {
            symbols.push(recovered);
        }
    }

    symbols.sort_by_key(|symbol| (symbol.line, symbol.end_line, symbol.name.clone()));
}

fn collect_function_keys(symbols: &[SymbolNode], keys: &mut HashSet<(String, usize)>) {
    for symbol in symbols {
        if symbol.kind == "function" {
            keys.insert((symbol.name.clone(), symbol.line));
        }
        collect_function_keys(&symbol.children, keys);
    }
}

impl RecoveredCLikeFunction {
    fn into_symbol(self) -> SymbolNode {
        SymbolNode {
            name: self.name,
            kind: "function".to_string(),
            signature: self.signature,
            line: self.line,
            end_line: self.end_line,
            children: Vec::new(),
        }
    }
}

pub(crate) fn recover_c_like_functions(source: &[u8]) -> Vec<RecoveredCLikeFunction> {
    let Ok(text) = std::str::from_utf8(source) else {
        return Vec::new();
    };

    let function_re = regex::Regex::new(
        r"(?ms)^[ \t]*(?P<header>(?:(?:[A-Za-z_]\w*|\*)[\s*]+)+(?P<name>[A-Za-z_]\w*)\s*\([^;{}]*\)\s*(?:__attribute__\s*\(\([^)]*\)\)\s*)?)\{",
    )
    .expect("valid C-like function recovery regex");

    let mut recovered = Vec::new();
    for caps in function_re.captures_iter(text) {
        let Some(matched) = caps.get(0) else {
            continue;
        };
        let Some(header) = caps.name("header") else {
            continue;
        };
        let Some(name) = caps.name("name") else {
            continue;
        };

        let header_text = header.as_str().trim();
        if is_c_like_control_header(header_text)
            || is_c_like_comment_header(header_text)
            || header_text.contains('=')
            || header_text.contains('#')
            || is_c_like_keyword(name.as_str())
        {
            continue;
        }

        let open_brace = matched.end() - 1;
        let Some(close_brace) = find_matching_brace(text.as_bytes(), open_brace) else {
            continue;
        };

        recovered.push(RecoveredCLikeFunction {
            name: name.as_str().to_string(),
            signature: header_text.to_string(),
            line: byte_to_line(source, matched.start()),
            end_line: byte_to_line(source, close_brace),
            byte_start: matched.start(),
            byte_end: close_brace + 1,
        });
    }

    recovered
}

fn is_c_like_control_header(header: &str) -> bool {
    header
        .split_whitespace()
        .next()
        .is_some_and(|word| matches!(word, "if" | "for" | "while" | "switch" | "return"))
}

fn is_c_like_comment_header(header: &str) -> bool {
    header.contains("/*")
        || header.contains("*/")
        || header
            .lines()
            .any(|line| matches!(line.trim_start().chars().next(), Some('*' | '/')))
}

fn is_c_like_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "else"
            | "for"
            | "while"
            | "switch"
            | "case"
            | "do"
            | "return"
            | "sizeof"
            | "typedef"
            | "struct"
            | "union"
            | "enum"
    )
}

fn byte_to_line(source: &[u8], byte: usize) -> usize {
    source[..byte.min(source.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

fn find_matching_brace(source: &[u8], open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open_brace;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;

    while i < source.len() {
        let b = source[i];
        let next = source.get(i + 1).copied();

        if in_line_comment {
            if b == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }
        if in_block_comment {
            if b == b'*' && next == Some(b'/') {
                in_block_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if in_char {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'\'' {
                in_char = false;
            }
            i += 1;
            continue;
        }

        match (b, next) {
            (b'/', Some(b'/')) => {
                in_line_comment = true;
                i += 2;
            }
            (b'/', Some(b'*')) => {
                in_block_comment = true;
                i += 2;
            }
            (b'"', _) => {
                in_string = true;
                i += 1;
            }
            (b'\'', _) => {
                in_char = true;
                i += 1;
            }
            (b'{', _) => {
                depth += 1;
                i += 1;
            }
            (b'}', _) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }

    None
}

/// Normalize tree-sitter node kinds to user-friendly labels.
fn normalize_kind(kind: &str) -> String {
    match kind {
        "function_item"
        | "function_declaration"
        | "function_definition"
        | "function_expression"
        | "arrow_function" => "function",
        "method"
        | "singleton_method"
        | "method_declaration"
        | "method_definition"
        | "method_def"
        | "abstract_method_def" => "method",
        "struct_item" | "struct_type" | "struct_declaration" | "struct_def" => "struct",
        "impl_item" => "impl",
        "trait_item" => "trait",
        "enum_item" | "enum_declaration" | "enum_def" => "enum",
        "mod_item" | "module_declaration" | "namespace_declaration" | "module_def" => "module",
        "module" => "module",
        "contract_declaration" => "contract",
        "library_declaration" => "library",
        "class_declaration" | "class_definition" | "class_def" => "class",
        "interface_declaration" => "interface",
        "const_item" | "const_declaration" => "const",
        "state_variable_declaration" => "variable",
        "static_item" => "static",
        "type_item"
        | "type_alias_declaration"
        | "type_declaration"
        | "type_spec"
        | "user_defined_type_definition"
        | "type_def"
        | "union_def"
        | "data_type"
        | "newtype"
        | "type_synomym"
        | "type_family"
        | "type_instance"
        | "data_family"
        | "data_instance"
        | "kind_signature" => "type",
        "macro_definition" | "macro_def" => "macro",
        "function" | "bind" | "foreign_import" | "foreign_export" => "function",
        "signature" | "default_signature" => "signature",
        "class" => "class",
        "instance" => "instance",
        "pattern_synonym" => "pattern",
        "lib_def" => "library",
        "fun_def" => "function",
        "alias" => "alias",
        "annotation_def" => "annotation",
        "use_declaration" => "use",
        "variable_declarator"
        | "lexical_declaration"
        | "variable_declaration"
        | "var_declaration"
        | "assignment"
        | "expression_statement" => "variable",
        "decorated_definition" => "decorated",
        "export_statement" => "export",
        "declare_statement" => "declare",
        "constructor_declaration" => "constructor",
        "constructor_definition" => "constructor",
        "modifier_definition" => "modifier",
        "fallback_receive_definition" => "function",
        "event_definition" => "event",
        "error_declaration" => "error",
        "field_declaration" => "field",
        other => other,
    }
    .to_string()
}

/// Format symbols as plain text with indentation.
pub fn format_symbols_text(file_symbols: &[FileSymbols]) -> String {
    let mut output = String::with_capacity(file_symbols.len() * 256);

    for fs in file_symbols {
        output.push_str(&fs.file);
        output.push_str(":\n");
        format_symbol_list(&fs.symbols, &mut output, 1);
        output.push('\n');
    }

    output
}

fn format_symbol_list(symbols: &[SymbolNode], output: &mut String, indent: usize) {
    let prefix = "  ".repeat(indent);
    for sym in symbols {
        if sym.line == sym.end_line {
            output.push_str(&format!("{}{:<8} {}\n", prefix, sym.line, sym.signature));
        } else {
            output.push_str(&format!(
                "{}{:<4}:{:<3} {}\n",
                prefix, sym.line, sym.end_line, sym.signature
            ));
        }
        if !sym.children.is_empty() {
            format_symbol_list(&sym.children, output, indent + 1);
        }
    }
}

/// Handle the `symbols` CLI command.
pub fn handle_symbols(files: Vec<String>, format: &str, options: SymbolOptions) -> Result<()> {
    let mut all_symbols = Vec::new();

    for file in &files {
        let path = Path::new(file);
        match extract_symbols_with_options(path, &options) {
            Ok(fs) => all_symbols.push(fs),
            Err(e) => eprintln!("Warning: {}: {}", file, e),
        }
    }

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&all_symbols)?;
            println!("{json}");
        }
        _ => {
            print!("{}", format_symbols_text(&all_symbols));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(content: &str, extension: &str) -> NamedTempFile {
        let mut file = tempfile::Builder::new()
            .suffix(&format!(".{}", extension))
            .tempfile()
            .unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_extract_rust_symbols() {
        let content = r#"
pub fn hello() {
    println!("hello");
}

pub struct Config {
    pub name: String,
}

impl Config {
    pub fn new(name: String) -> Config {
        Config { name }
    }

    fn validate(&self) -> bool {
        true
    }
}

const MAX_SIZE: usize = 1024;

enum Status {
    Active,
    Inactive,
}
"#;
        let file = create_temp_file(content, "rs");
        let result = extract_symbols(file.path(), false).unwrap();

        assert!(!result.symbols.is_empty());

        // Check we have the main symbols
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"hello"),
            "missing function hello, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Config"),
            "missing struct Config, got: {:?}",
            names
        );
        assert!(
            names.contains(&"MAX_SIZE"),
            "missing const MAX_SIZE, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Status"),
            "missing enum Status, got: {:?}",
            names
        );

        // Check impl has children
        let impl_sym = result.symbols.iter().find(|s| s.kind == "impl").unwrap();
        assert!(!impl_sym.children.is_empty(), "impl should have children");
        let child_names: Vec<&str> = impl_sym.children.iter().map(|s| s.name.as_str()).collect();
        assert!(
            child_names.contains(&"new"),
            "missing method new in impl, got: {:?}",
            child_names
        );
        assert!(
            child_names.contains(&"validate"),
            "missing method validate in impl, got: {:?}",
            child_names
        );
    }

    #[test]
    fn test_extract_python_symbols() {
        let content = r#"
def greet(name):
    print(f"Hello {name}")

class Animal:
    def __init__(self, name):
        self.name = name

    def speak(self):
        pass

MAX_COUNT = 100
"#;
        let file = create_temp_file(content, "py");
        let result = extract_symbols(file.path(), false).unwrap();

        assert!(!result.symbols.is_empty());
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"greet"),
            "missing function greet, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Animal"),
            "missing class Animal, got: {:?}",
            names
        );
    }

    #[test]
    fn test_extract_c_function_names_from_declarators() {
        let content = r#"
typedef int BOOL;
#define NORETURN __attribute__((noreturn))

static void check_timeout(BOOL allow_keepalive, int keepalive_flags)
{
}

static NORETURN void whine_about_eof(BOOL allow_kluge)
{
}

static size_t safe_read(int fd, char *buf, size_t len)
{
    return len;
}

static const char *what_fd_is(int fd)
{
    return "fd";
}
"#;
        let file = create_temp_file(content, "c");
        let result = extract_symbols(file.path(), false).unwrap();
        let functions: Vec<_> = result
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == "function")
            .collect();
        let names: Vec<&str> = functions.iter().map(|s| s.name.as_str()).collect();

        assert!(
            names.contains(&"check_timeout"),
            "missing check_timeout, got: {:?}",
            names
        );
        assert!(
            names.contains(&"whine_about_eof"),
            "missing whine_about_eof, got: {:?}",
            names
        );
        assert!(
            names.contains(&"safe_read"),
            "missing safe_read, got: {:?}",
            names
        );
        assert!(
            names.contains(&"what_fd_is"),
            "missing what_fd_is, got: {:?}",
            names
        );
        assert!(
            !names
                .iter()
                .any(|name| matches!(*name, "void" | "NORETURN" | "size_t" | "char")),
            "function names should not be return types, got: {:?}",
            names
        );
    }

    #[test]
    fn test_extract_c_function_inside_preprocessor_heavy_body() {
        let content = r#"
typedef int mode_t;
typedef int SMB_ACL_T;
typedef int rsync_acl;

#ifndef HAVE_OSX_ACLS
static mode_t change_sacl_perms(SMB_ACL_T sacl, rsync_acl *racl, mode_t old_mode, mode_t mode)
{
    if (mode) {
#ifdef SMB_ACL_LOSES_SPECIAL_MODE_BITS
        if (mode & 01000)
            mode &= ~0077;
#else
        if (mode & 01000 && !(old_mode & 01000))
            mode &= ~0077;
    } else {
        if ((old_mode & 04000 && !(mode & 04000))
         || (old_mode & 02000 && !(mode & 02000)))
            mode &= ~0077;
#endif
    }

    return mode;
}
#endif

int set_acl(void)
{
    return 0;
}
"#;
        let file = create_temp_file(content, "c");
        let result = extract_symbols(file.path(), false).unwrap();
        let names: Vec<&str> = result
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == "function")
            .map(|symbol| symbol.name.as_str())
            .collect();

        assert!(
            names.contains(&"change_sacl_perms"),
            "missing preprocessor-heavy C function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"set_acl"),
            "missing set_acl, got: {:?}",
            names
        );
    }

    #[test]
    fn test_extract_typescript_symbols() {
        let content = r#"
function hello(): void {
    console.log("hello");
}

interface Config {
    name: string;
}

class App {
    constructor() {}

    start(): void {}
}

const MAX_SIZE = 1024;

enum Status {
    Active,
    Inactive,
}
"#;
        let file = create_temp_file(content, "ts");
        let result = extract_symbols(file.path(), false).unwrap();

        assert!(!result.symbols.is_empty());
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"hello"),
            "missing function hello, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Config"),
            "missing interface Config, got: {:?}",
            names
        );
        assert!(
            names.contains(&"App"),
            "missing class App, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Status"),
            "missing enum Status, got: {:?}",
            names
        );
    }

    #[test]
    fn test_extract_go_symbols() {
        let content = r#"
package main

func main() {
    fmt.Println("hello")
}

type Config struct {
    Name string
}

func (c *Config) Validate() bool {
    return true
}
"#;
        let file = create_temp_file(content, "go");
        let result = extract_symbols(file.path(), false).unwrap();

        assert!(!result.symbols.is_empty());
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"main"),
            "missing function main, got: {:?}",
            names
        );
    }

    #[test]
    fn test_extract_nested_ruby_symbols() {
        let content = r#"
module RuboCop
  module Cop
    class Base
      def self.documentation_url(config = nil)
        Documentation.url_for(self, config)
      end

      def add_offense(node_or_range, message: nil, severity: nil, &block)
        current_offenses << node_or_range
      end
    end
  end
end
"#;
        let file = create_temp_file(content, "rb");
        let result = extract_symbols(file.path(), false).unwrap();

        let rubocop = result
            .symbols
            .iter()
            .find(|s| s.name == "RuboCop")
            .expect("RuboCop module should be collected");
        let cop = rubocop
            .children
            .iter()
            .find(|s| s.name == "Cop")
            .expect("nested Cop module should be collected");
        let base = cop
            .children
            .iter()
            .find(|s| s.name == "Base")
            .expect("nested Base class should be collected");

        let child_names = base
            .children
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>();
        assert!(
            child_names.contains(&"documentation_url"),
            "singleton method should be collected, got: {child_names:?}"
        );
        assert!(
            child_names.contains(&"add_offense"),
            "instance method should be collected, got: {child_names:?}"
        );
    }

    #[test]
    fn test_symbols_line_numbers() {
        let content = "fn first() {\n}\n\nfn second() {\n    let x = 1;\n}\n";
        let file = create_temp_file(content, "rs");
        let result = extract_symbols(file.path(), false).unwrap();

        assert_eq!(result.symbols.len(), 2);
        assert_eq!(result.symbols[0].line, 1);
        assert_eq!(result.symbols[0].end_line, 2);
        assert_eq!(result.symbols[1].line, 4);
        assert_eq!(result.symbols[1].end_line, 6);
    }

    #[test]
    fn test_symbols_unsupported_extension() {
        let file = create_temp_file("hello world\nreqproof:documents SW-REQ-1", "xyz");
        let result = extract_symbols(file.path(), false).unwrap();

        assert_eq!(result.symbols.len(), 2);
        assert_eq!(result.symbols[0].kind, "text");
        assert_eq!(result.symbols[0].signature, "hello world");
        assert_eq!(result.symbols[0].line, 1);
        assert_eq!(result.symbols[1].signature, "reqproof:documents SW-REQ-1");
        assert_eq!(result.symbols[1].line, 2);
    }

    #[test]
    fn test_symbols_strict_unsupported_extension_errors() {
        let file = create_temp_file("hello world", "xyz");
        let result = extract_symbols_with_options(
            file.path(),
            &SymbolOptions {
                strict: true,
                ..SymbolOptions::default()
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_symbols_standard_text_extensions_use_text_mode() {
        let file = create_temp_file(".TH rsync 1\n.SH NAME", "1");
        let result = extract_symbols(file.path(), false).unwrap();

        assert_eq!(result.symbols.len(), 2);
        assert!(result.symbols.iter().all(|symbol| symbol.kind == "text"));
        assert_eq!(result.symbols[0].signature, ".TH rsync 1");
        assert_eq!(result.symbols[1].signature, ".SH NAME");
    }

    #[test]
    fn test_symbols_strict_standard_text_extension_errors_without_override() {
        let file = create_temp_file(".TH rsync 1", "1");
        let result = extract_symbols_with_options(
            file.path(),
            &SymbolOptions {
                strict: true,
                ..SymbolOptions::default()
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_symbols_custom_text_extension_forces_text_mode() {
        let file = create_temp_file("fn not_a_symbol() {}\nplain text", "rs");
        let result = extract_symbols_with_options(
            file.path(),
            &SymbolOptions {
                text_extensions: vec!["rs".to_string()],
                ..SymbolOptions::default()
            },
        )
        .unwrap();

        assert_eq!(result.symbols.len(), 2);
        assert!(result.symbols.iter().all(|symbol| symbol.kind == "text"));
        assert_eq!(result.symbols[0].signature, "fn not_a_symbol() {}");
    }

    #[test]
    fn test_symbols_empty_file() {
        let file = create_temp_file("", "rs");
        let result = extract_symbols(file.path(), false).unwrap();
        assert!(result.symbols.is_empty());
    }

    #[test]
    fn test_format_text_output() {
        let symbols = vec![FileSymbols {
            file: "test.rs".to_string(),
            symbols: vec![
                SymbolNode {
                    name: "main".to_string(),
                    kind: "function".to_string(),
                    signature: "fn main()".to_string(),
                    line: 1,
                    end_line: 10,
                    children: vec![],
                },
                SymbolNode {
                    name: "Config".to_string(),
                    kind: "impl".to_string(),
                    signature: "impl Config { ... }".to_string(),
                    line: 12,
                    end_line: 20,
                    children: vec![SymbolNode {
                        name: "new".to_string(),
                        kind: "function".to_string(),
                        signature: "fn new() -> Config".to_string(),
                        line: 13,
                        end_line: 15,
                        children: vec![],
                    }],
                },
            ],
        }];
        let output = format_symbols_text(&symbols);
        assert!(output.contains("test.rs:"));
        assert!(output.contains("fn main()"));
        assert!(output.contains("impl Config"));
        assert!(output.contains("fn new() -> Config"));
    }

    #[test]
    fn test_normalize_kind() {
        assert_eq!(normalize_kind("function_item"), "function");
        assert_eq!(normalize_kind("struct_item"), "struct");
        assert_eq!(normalize_kind("impl_item"), "impl");
        assert_eq!(normalize_kind("class_declaration"), "class");
        assert_eq!(normalize_kind("const_item"), "const");
    }

    #[test]
    fn test_json_output() {
        let content = "fn hello() {}\n";
        let file = create_temp_file(content, "rs");
        let result = extract_symbols(file.path(), false).unwrap();
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"kind\":\"function\""));
        assert!(json.contains("\"name\":\"hello\""));
    }

    #[test]
    fn test_exported_typescript_symbols_use_inner_declaration_names() {
        let content = r#"
export class PolicyService {
    async evaluatePolicy(input: string): Promise<boolean> {
        return input.length > 0;
    }
}

export const normalizeDecision = (raw: string) => {
    return raw.trim().toLowerCase();
};
"#;
        let file = create_temp_file(content, "ts");
        let result = extract_symbols(file.path(), false).unwrap();
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        let kinds: Vec<&str> = result.symbols.iter().map(|s| s.kind.as_str()).collect();

        assert!(
            names.contains(&"PolicyService"),
            "exported class should use class name, got: {:?}",
            names
        );
        assert!(
            names.contains(&"normalizeDecision"),
            "exported const arrow should use variable name, got: {:?}",
            names
        );
        assert!(
            !names.contains(&"export_statement"),
            "export wrappers should not leak as symbol names: {:?}",
            names
        );
        assert!(
            kinds.contains(&"class") && kinds.contains(&"variable"),
            "expected semantic kinds for exported declarations, got: {:?}",
            kinds
        );
    }

    #[test]
    fn test_allow_tests_filtering() {
        let content = r#"
fn normal_fn() {}

#[test]
fn test_something() {}
"#;
        let file = create_temp_file(content, "rs");

        // Without tests
        let result = extract_symbols(file.path(), false).unwrap();
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"normal_fn"));
        assert!(
            !names.contains(&"test_something"),
            "test should be filtered out"
        );

        // With tests
        let result = extract_symbols(file.path(), true).unwrap();
        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"normal_fn"));
        assert!(names.contains(&"test_something"), "test should be included");
    }
}
