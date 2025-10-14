use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;
use std::path::Path;
use tree_sitter::{Language as TSLanguage, Node, Parser as TSParser};

use probe_code::language::factory::get_language_impl;

#[derive(Parser)]
#[clap(name = "debug-tree-sitter")]
#[clap(about = "Debug tool for analyzing tree-sitter positions and symbols")]
struct Args {
    /// Path to the file to analyze
    file_path: String,

    /// Symbol name to search for specifically (optional)
    #[clap(short, long)]
    symbol: Option<String>,

    /// Enable verbose output with detailed AST information
    #[clap(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone)]
struct SymbolInfo {
    name: String,
    symbol_kind: String,
    node_kind: String,
    parent_start_line: u32,
    parent_start_column: u32,
    parent_end_line: u32,
    parent_end_column: u32,
    identifier_start_line: u32,
    identifier_start_column: u32,
    identifier_end_line: u32,
    identifier_end_column: u32,
    node_path: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let file_path = Path::new(&args.file_path);

    // Check if file exists
    if !file_path.exists() {
        anyhow::bail!("File does not exist: {}", args.file_path);
    }

    // Read file content
    let content = std::fs::read_to_string(file_path)
        .context(format!("Failed to read file: {}", args.file_path))?;

    // Get file extension
    let extension = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    if extension.is_empty() {
        anyhow::bail!("Could not determine file extension for: {}", args.file_path);
    }

    // Get language implementation
    let language_impl = get_language_impl(extension)
        .ok_or_else(|| anyhow::anyhow!("Unsupported file extension: {}", extension))?;

    // Set up parser
    let language = language_impl.get_tree_sitter_language();
    let mut parser = TSParser::new();
    parser
        .set_language(&language)
        .context("Failed to set tree-sitter language")?;

    // Parse the file
    let tree = parser
        .parse(&content, None)
        .context("Failed to parse file with tree-sitter")?;

    println!("{}", format!("File: {}", args.file_path).cyan().bold());
    println!(
        "{}",
        format!(
            "Language: {} (extension: {})",
            get_language_name(&language),
            extension
        )
        .cyan()
    );
    println!(
        "{}",
        format!(
            "File size: {} bytes, {} lines",
            content.len(),
            content.lines().count()
        )
        .cyan()
    );
    println!();

    // Find all symbols
    let symbols = find_all_symbols(tree.root_node(), content.as_bytes(), args.verbose);

    // Filter symbols if specific symbol is requested
    let symbols_to_show = if let Some(target_symbol) = &args.symbol {
        symbols
            .into_iter()
            .filter(|s| s.name == *target_symbol)
            .collect::<Vec<_>>()
    } else {
        symbols
    };

    if symbols_to_show.is_empty() {
        if let Some(target) = &args.symbol {
            println!("{}", format!("No symbol '{target}' found in file").red());
        } else {
            println!("{}", "No symbols found in file".red());
        }
        return Ok(());
    }

    println!(
        "{}",
        format!("Found {} symbol(s):", symbols_to_show.len())
            .green()
            .bold()
    );
    println!();

    for symbol in symbols_to_show {
        display_symbol_info(&symbol, args.verbose);
        println!();
    }

    if args.verbose {
        display_tree_structure(tree.root_node(), content.as_bytes(), 0, 3);
    }

    Ok(())
}

fn get_language_name(language: &TSLanguage) -> &str {
    let version = language.version();
    match version {
        _ if format!("{language:?}").contains("rust") => "Rust",
        _ if format!("{language:?}").contains("javascript") => "JavaScript",
        _ if format!("{language:?}").contains("typescript") => "TypeScript",
        _ if format!("{language:?}").contains("python") => "Python",
        _ if format!("{language:?}").contains("go") => "Go",
        _ if format!("{language:?}").contains("java") => "Java",
        _ if format!("{language:?}").contains("c") => "C/C++",
        _ if format!("{language:?}").contains("ruby") => "Ruby",
        _ if format!("{language:?}").contains("php") => "PHP",
        _ if format!("{language:?}").contains("swift") => "Swift",
        _ if format!("{language:?}").contains("csharp") => "C#",
        _ => "Unknown",
    }
}

fn find_all_symbols(node: Node, source: &[u8], verbose: bool) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();
    find_symbols_recursive(node, source, &mut symbols, Vec::new(), verbose);
    symbols
}

fn find_symbols_recursive(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<SymbolInfo>,
    path: Vec<String>,
    verbose: bool,
) {
    let mut new_path = path.clone();
    new_path.push(node.kind().to_string());

    // Check if this node represents a symbol declaration
    if let Some(symbol_info) = extract_symbol_info(node, source, &new_path, verbose) {
        symbols.push(symbol_info);
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_symbols_recursive(child, source, symbols, new_path.clone(), verbose);
    }
}

fn extract_symbol_info(
    node: Node,
    source: &[u8],
    path: &[String],
    verbose: bool,
) -> Option<SymbolInfo> {
    let node_kind = node.kind();

    // Define what constitutes a symbol based on node type
    let (symbol_kind, identifier_kinds) = match node_kind {
        // Rust
        "function_item" => ("function", vec!["identifier"]),
        "struct_item" => ("struct", vec!["type_identifier"]),
        "impl_item" => ("impl", vec!["type_identifier"]),
        "trait_item" => ("trait", vec!["type_identifier"]),
        "enum_item" => ("enum", vec!["type_identifier"]),
        "mod_item" => ("module", vec!["identifier"]),
        "macro_definition" => ("macro", vec!["identifier"]),

        // JavaScript/TypeScript
        "function_declaration" => ("function", vec!["identifier"]),
        "method_definition" => ("method", vec!["property_identifier"]),
        "class_declaration" => ("class", vec!["type_identifier", "identifier"]),
        "arrow_function" => ("arrow_function", vec!["identifier"]),
        "function_expression" => ("function_expression", vec!["identifier"]),
        "variable_declarator" => ("variable", vec!["identifier"]),
        "interface_declaration" => ("interface", vec!["type_identifier"]),
        "type_alias_declaration" => ("type_alias", vec!["type_identifier"]),
        "enum_declaration" => ("enum", vec!["identifier"]),

        // Go
        "func_declaration" => ("function", vec!["identifier"]), // Go uses different node names
        "method_spec" => ("method", vec!["field_identifier"]),
        "type_declaration" => ("type", vec!["type_identifier"]),
        "var_declaration" => ("variable", vec!["identifier"]),
        "const_declaration" => ("constant", vec!["identifier"]),

        // Python (function_definition handled above with C/C++)
        "class_definition" => ("class", vec!["identifier"]),

        // C/C++ (function_definition handled above)
        "struct_specifier" => ("struct", vec!["type_identifier"]),
        "enum_specifier" => ("enum", vec!["identifier"]),

        // Java (method_declaration handled above)
        "constructor_declaration" => ("constructor", vec!["identifier"]),
        "field_declaration" => ("field", vec!["identifier"]),

        _ => return None,
    };

    if verbose {
        println!("[VERBOSE] Checking node kind '{node_kind}' for symbol extraction");
    }

    // Find identifier within this node
    let identifier_node = find_identifier_in_node(node, &identifier_kinds, verbose)?;

    let identifier_text = identifier_node.utf8_text(source).ok()?;

    if verbose {
        println!(
            "[VERBOSE] Found identifier '{}' of type '{}'",
            identifier_text,
            identifier_node.kind()
        );
    }

    Some(SymbolInfo {
        name: identifier_text.to_string(),
        symbol_kind: symbol_kind.to_string(),
        node_kind: node_kind.to_string(),
        parent_start_line: node.start_position().row as u32 + 1,
        parent_start_column: node.start_position().column as u32 + 1,
        parent_end_line: node.end_position().row as u32 + 1,
        parent_end_column: node.end_position().column as u32 + 1,
        identifier_start_line: identifier_node.start_position().row as u32 + 1,
        identifier_start_column: identifier_node.start_position().column as u32 + 1,
        identifier_end_line: identifier_node.end_position().row as u32 + 1,
        identifier_end_column: identifier_node.end_position().column as u32 + 1,
        node_path: path.join(" > "),
    })
}

fn find_identifier_in_node<'a>(
    node: Node<'a>,
    identifier_kinds: &[&str],
    verbose: bool,
) -> Option<Node<'a>> {
    // First, check direct children for identifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if identifier_kinds.contains(&child.kind()) {
            if verbose {
                println!("[VERBOSE] Found direct identifier child: {}", child.kind());
            }
            return Some(child);
        }
    }

    // If not found in direct children, search deeper (for complex structures)
    for child in node.children(&mut cursor) {
        if let Some(identifier) = find_identifier_recursive(child, identifier_kinds, 0, verbose) {
            return Some(identifier);
        }
    }

    None
}

fn find_identifier_recursive<'a>(
    node: Node<'a>,
    identifier_kinds: &[&str],
    depth: usize,
    verbose: bool,
) -> Option<Node<'a>> {
    // Limit search depth to avoid going too deep
    if depth > 3 {
        return None;
    }

    if identifier_kinds.contains(&node.kind()) {
        if verbose {
            println!(
                "[VERBOSE] Found nested identifier: {} at depth {}",
                node.kind(),
                depth
            );
        }
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(identifier) =
            find_identifier_recursive(child, identifier_kinds, depth + 1, verbose)
        {
            return Some(identifier);
        }
    }

    None
}

fn display_symbol_info(symbol: &SymbolInfo, verbose: bool) {
    println!(
        "{}",
        format!("Symbol: {} ({})", symbol.name, symbol.symbol_kind)
            .yellow()
            .bold()
    );
    println!(
        "  {}: {}:{} - {}:{}",
        "Parent node".green(),
        symbol.parent_start_line,
        symbol.parent_start_column,
        symbol.parent_end_line,
        symbol.parent_end_column
    );
    println!(
        "  {}: {}:{} - {}:{}",
        "Identifier".green(),
        symbol.identifier_start_line,
        symbol.identifier_start_column,
        symbol.identifier_end_line,
        symbol.identifier_end_column
    );
    println!("  {}: {}", "Node path".green(), symbol.node_path);

    if verbose {
        println!(
            "  {}: {}",
            "Tree-sitter node kind".green(),
            symbol.node_kind
        );

        // Calculate identifier offset from parent start
        let line_offset = (symbol.identifier_start_line as i32) - (symbol.parent_start_line as i32);
        let column_offset = if line_offset == 0 {
            (symbol.identifier_start_column as i32) - (symbol.parent_start_column as i32)
        } else {
            symbol.identifier_start_column as i32
        };

        println!(
            "  {}: +{} lines, +{} columns from parent start",
            "Identifier offset".green(),
            line_offset,
            column_offset
        );
    }
}

fn display_tree_structure(node: Node, source: &[u8], depth: usize, max_depth: usize) {
    if depth > max_depth {
        return;
    }

    let indent = "  ".repeat(depth);
    let node_text = node.utf8_text(source).unwrap_or("???");
    let truncated_text = if node_text.len() > 50 {
        format!("{}...", &node_text[..47])
    } else {
        node_text.to_string()
    };

    println!(
        "{}{}[{}:{}-{}:{}] \"{}\"",
        indent,
        node.kind().blue(),
        node.start_position().row + 1,
        node.start_position().column + 1,
        node.end_position().row + 1,
        node.end_position().column + 1,
        truncated_text.replace('\n', "\\n")
    );

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        display_tree_structure(child, source, depth + 1, max_depth);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_rust_function_detection() {
        let rust_code = r#"
fn main() {
    println!("Hello, world!");
}

struct MyStruct {
    field: i32,
}

impl MyStruct {
    fn new(value: i32) -> Self {
        Self { field: value }
    }
}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(rust_code.as_bytes()).unwrap();
        let temp_path = temp_file.path().with_extension("rs");
        std::fs::copy(temp_file.path(), &temp_path).unwrap();

        // Set up parser
        let language_impl = get_language_impl("rs").unwrap();
        let language = language_impl.get_tree_sitter_language();
        let mut parser = TSParser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(rust_code, None).unwrap();

        let symbols = find_all_symbols(tree.root_node(), rust_code.as_bytes(), false);

        assert!(!symbols.is_empty());
        assert!(symbols
            .iter()
            .any(|s| s.name == "main" && s.symbol_kind == "function"));
        assert!(symbols
            .iter()
            .any(|s| s.name == "MyStruct" && s.symbol_kind == "struct"));
        assert!(symbols
            .iter()
            .any(|s| s.name == "new" && s.symbol_kind == "function"));

        // Clean up
        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn test_javascript_function_detection() {
        let js_code = r#"
function main() {
    console.log("Hello, world!");
}

class MyClass {
    constructor(value) {
        this.value = value;
    }
    
    getValue() {
        return this.value;
    }
}

const arrow = (x, y) => x + y;
"#;

        // Set up parser
        let language_impl = get_language_impl("js").unwrap();
        let language = language_impl.get_tree_sitter_language();
        let mut parser = TSParser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(js_code, None).unwrap();

        let symbols = find_all_symbols(tree.root_node(), js_code.as_bytes(), false);

        assert!(!symbols.is_empty());
        assert!(symbols
            .iter()
            .any(|s| s.name == "main" && s.symbol_kind == "function"));
        assert!(symbols
            .iter()
            .any(|s| s.name == "MyClass" && s.symbol_kind == "class"));
        assert!(symbols
            .iter()
            .any(|s| s.name == "getValue" && s.symbol_kind == "method"));
    }
}
