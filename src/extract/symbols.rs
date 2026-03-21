//! Symbol tree extraction for files.
//!
//! Provides a table-of-contents view of a file's symbols (functions, structs, classes,
//! constants, etc.) with line numbers and nesting.

use anyhow::{Context, Result};
use serde::Serialize;
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

/// Extract the symbol tree from a file.
pub fn extract_symbols(path: &Path, allow_tests: bool) -> Result<FileSymbols> {
    if !path.exists() {
        return Err(anyhow::anyhow!("File does not exist: {:?}", path));
    }

    let content =
        std::fs::read_to_string(path).context(format!("Failed to read file: {path:?}"))?;

    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

    let language_impl = get_language_impl(extension)
        .ok_or_else(|| anyhow::anyhow!("Unsupported file extension: {}", extension))?;

    let mut parser = get_pooled_parser(extension)
        .map_err(|_| anyhow::anyhow!("Failed to get parser for extension: {}", extension))?;

    let tree = parser
        .parse(&content, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse file: {:?}", path))?;

    let root = tree.root_node();
    let source = content.as_bytes();

    let symbols = collect_symbols(&root, source, language_impl.as_ref(), allow_tests, 0);

    return_pooled_parser(extension, parser);

    Ok(FileSymbols {
        file: path.to_string_lossy().to_string(),
        symbols,
    })
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
            | "enum_declaration"
            | "enum_item"
            | "declaration_list"
            | "class_body"
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
            let signature = lang
                .get_symbol_signature(&child, source)
                .unwrap_or_else(|| {
                    // Fallback: use the first line of the node text
                    let text = child.utf8_text(source).unwrap_or("");
                    text.lines().next().unwrap_or("").trim().to_string()
                });

            let name = extract_symbol_name(&child, source);
            let kind = normalize_kind(child.kind());
            let start_line = child.start_position().row + 1;
            let end_line = child.end_position().row + 1;

            // Recursively collect children for container nodes
            let children = if is_container_node(child.kind()) && depth < MAX_SYMBOL_DEPTH {
                collect_children_symbols(&child, source, lang, allow_tests, depth + 1)
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
        }
    }

    symbols
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
        return collect_symbols(&body, source, lang, allow_tests, depth);
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
                | "object_type"
                | "interface_body"
                | "statement_block"
        ) {
            return collect_symbols(&child, source, lang, allow_tests, depth);
        }
    }

    // Fallback: try collecting directly from the node's children
    collect_symbols(node, source, lang, allow_tests, depth)
}

/// Extract a symbol name from an AST node.
fn extract_symbol_name(node: &Node, source: &[u8]) -> String {
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

    // For variable declarations/const, try to find the identifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "identifier" | "type_identifier" | "property_identifier"
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

/// Normalize tree-sitter node kinds to user-friendly labels.
fn normalize_kind(kind: &str) -> String {
    match kind {
        "function_item"
        | "function_declaration"
        | "function_definition"
        | "function_expression"
        | "arrow_function" => "function",
        "method_declaration" | "method_definition" => "method",
        "struct_item" | "struct_type" => "struct",
        "impl_item" => "impl",
        "trait_item" => "trait",
        "enum_item" | "enum_declaration" => "enum",
        "mod_item" | "module_declaration" | "namespace_declaration" => "module",
        "class_declaration" | "class_definition" => "class",
        "interface_declaration" => "interface",
        "const_item" | "const_declaration" => "const",
        "static_item" => "static",
        "type_item" | "type_alias_declaration" | "type_declaration" | "type_spec" => "type",
        "macro_definition" => "macro",
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
pub fn handle_symbols(files: Vec<String>, format: &str, allow_tests: bool) -> Result<()> {
    let mut all_symbols = Vec::new();

    for file in &files {
        let path = Path::new(file);
        match extract_symbols(path, allow_tests) {
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
        let file = create_temp_file("hello world", "xyz");
        let result = extract_symbols(file.path(), false);
        assert!(result.is_err());
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
