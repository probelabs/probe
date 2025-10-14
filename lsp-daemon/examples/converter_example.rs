#![cfg_attr(not(feature = "legacy-tests"), allow(dead_code, unused_imports))]
#[cfg(not(feature = "legacy-tests"))]
fn main() {}

// Example of using the ProtocolConverter
//
// This example demonstrates how to convert database types (Edge, SymbolState)
// to LSP protocol types (Location, CallHierarchyItem, CallHierarchyCall)

#[cfg(feature = "legacy-tests")]
use lsp_daemon::database::{Edge, EdgeRelation, ProtocolConverter, SymbolState};
use std::path::Path;

#[cfg(feature = "legacy-tests")]
fn main() {
    let converter = ProtocolConverter::new();

    // Example 1: Convert SymbolState to CallHierarchyItem
    let symbol = SymbolState {
        symbol_uid: "rust_function_123".to_string(),
        file_path: "/src/config/parser.rs".to_string(),
        language: "rust".to_string(),
        name: "parse_config".to_string(),
        fqn: Some("config::parser::parse_config".to_string()),
        kind: "function".to_string(),
        signature: Some("fn parse_config(path: &Path) -> Result<Config>".to_string()),
        visibility: Some("public".to_string()),
        def_start_line: 42,
        def_start_char: 0,
        def_end_line: 58,
        def_end_char: 1,
        is_definition: true,
        documentation: Some("Parse configuration from file".to_string()),
        metadata: None,
    };

    let file_path = Path::new("/src/config/parser.rs");
    let call_hierarchy_item = converter.symbol_to_call_hierarchy_item(&symbol, file_path);

    println!("CallHierarchyItem:");
    println!("  Name: {}", call_hierarchy_item.name);
    println!("  Kind: {}", call_hierarchy_item.kind);
    println!("  URI: {}", call_hierarchy_item.uri);
    println!(
        "  Range: {}:{} -> {}:{}",
        call_hierarchy_item.range.start.line,
        call_hierarchy_item.range.start.character,
        call_hierarchy_item.range.end.line,
        call_hierarchy_item.range.end.character
    );

    // Example 2: Convert Edge to Location
    let edge = Edge {
        relation: EdgeRelation::Calls,
        source_symbol_uid: "caller_function_456".to_string(),
        target_symbol_uid: "rust_function_123".to_string(),
        file_path: Some("/src/config/parser.rs".to_string()),
        start_line: Some(15),
        start_char: Some(8),
        confidence: 0.95,
        language: "rust".to_string(),
        metadata: Some("LSP call hierarchy".to_string()),
    };

    let edges = vec![edge];
    let locations = converter.edges_to_locations_direct(edges);

    println!("\nLocations:");
    for location in &locations {
        println!("  URI: {}", location.uri);
        println!(
            "  Position: {}:{}",
            location.range.start.line, location.range.start.character
        );
    }

    // Example 3: Convert Edges to CallHierarchyCall
    let caller_symbol = SymbolState {
        symbol_uid: "caller_function_456".to_string(),
        file_path: "/src/main.rs".to_string(),
        language: "rust".to_string(),
        name: "main".to_string(),
        fqn: Some("main".to_string()),
        kind: "function".to_string(),
        signature: Some("fn main()".to_string()),
        visibility: Some("public".to_string()),
        def_start_line: 10,
        def_start_char: 0,
        def_end_line: 20,
        def_end_char: 1,
        is_definition: true,
        documentation: None,
        metadata: None,
    };

    let call_edge = Edge {
        relation: EdgeRelation::Calls,
        source_symbol_uid: "caller_function_456".to_string(),
        target_symbol_uid: "rust_function_123".to_string(),
        file_path: Some("/src/main.rs".to_string()),
        start_line: Some(15),
        start_char: Some(4),
        confidence: 0.9,
        language: "rust".to_string(),
        metadata: None,
    };

    let symbols = vec![caller_symbol];
    let call_edges = vec![call_edge];
    let calls = converter.edges_to_calls(call_edges, &symbols);

    println!("\nCallHierarchyCall:");
    for call in &calls {
        println!("  From: {}", call.from.name);
        println!("  From URI: {}", call.from.uri);
        println!("  Call ranges: {}", call.from_ranges.len());
        for range in &call.from_ranges {
            println!(
                "    Range: {}:{} -> {}:{}",
                range.start.line, range.start.character, range.end.line, range.end.character
            );
        }
    }

    // Example 4: URI conversion
    println!("\nURI Conversion Examples:");
    let unix_path = Path::new("/home/user/project/src/main.rs");
    let uri = converter.path_to_uri(unix_path);
    println!("  Path: {} -> URI: {}", unix_path.display(), uri);

    match converter.uri_to_path(&uri) {
        Ok(converted_path) => {
            println!("  URI: {} -> Path: {}", uri, converted_path.display());
        }
        Err(e) => {
            println!("  Failed to convert URI: {}", e);
        }
    }
}
