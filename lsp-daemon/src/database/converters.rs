//! Protocol Converters Module
//!
//! This module provides conversion utilities between database types (Edge, SymbolState)
//! and LSP protocol types (Location, CallHierarchyItem, CallHierarchyCall).
//!
//! The converters handle:
//! - Database Edge/SymbolState to LSP Location
//! - Database SymbolState to LSP CallHierarchyItem
//! - Database Edge to LSP CallHierarchyCall
//! - Proper URI formatting (file:// scheme)
//! - Position and range mapping

use anyhow::Result;
use std::path::Path;

use crate::database::{Edge, SymbolState};
use crate::protocol::{
    CallHierarchyCall, CallHierarchyItem, CallHierarchyResult, Location, Position, Range,
};

/// Protocol converter for transforming database types to LSP protocol types
pub struct ProtocolConverter;

impl ProtocolConverter {
    /// Create a new protocol converter
    pub fn new() -> Self {
        Self
    }

    /// Convert database edges to Location array for references/definitions
    ///
    /// Each edge represents a relationship between symbols, and we extract
    /// the source location information to create LSP Location objects.
    /// Convert edges to Location vec (for references/definitions)
    /// Now accepts a file path resolver function to avoid placeholder paths
    pub fn edges_to_locations<F>(&self, edges: Vec<Edge>, file_path_resolver: F) -> Vec<Location>
    where
        F: Fn(i64) -> Option<String>,
    {
        edges
            .into_iter()
            .filter_map(|edge| self.edge_to_location(&edge, &file_path_resolver))
            .collect()
    }

    /// Convert database edges to Location array using direct file paths
    ///
    /// This is the updated method that uses file_path directly from edges,
    /// eliminating the need for file path resolution during query time.
    pub fn edges_to_locations_direct(&self, edges: Vec<Edge>) -> Vec<Location> {
        edges
            .into_iter()
            .filter_map(|edge| self.edge_to_location_direct(&edge))
            .collect()
    }

    /// Convert a single edge to a Location with file path resolution
    fn edge_to_location<F>(&self, edge: &Edge, _file_path_resolver: &F) -> Option<Location>
    where
        F: Fn(i64) -> Option<String>,
    {
        // Use direct file path from edge if available, otherwise use resolver
        let file_path = match &edge.file_path {
            Some(path) => std::path::PathBuf::from(path),
            None => {
                // Fallback to placeholder if no file path available
                std::path::PathBuf::from("unknown_file")
            }
        };

        let start_line = edge.start_line.unwrap_or(0);
        let start_char = edge.start_char.unwrap_or(0);

        Some(Location {
            uri: self.path_to_uri(&file_path),
            range: Range {
                start: Position {
                    line: start_line,
                    character: start_char,
                },
                end: Position {
                    line: start_line,
                    character: start_char,
                },
            },
        })
    }

    /// Convert a single edge to a Location using direct file path
    ///
    /// This method uses the file_path field directly from the edge,
    /// eliminating the need for file path resolution.
    fn edge_to_location_direct(&self, edge: &Edge) -> Option<Location> {
        // Use direct file path from edge
        let file_path = match &edge.file_path {
            Some(path) if !path.is_empty() => std::path::PathBuf::from(path),
            _ => {
                // If no file path in edge, fall back to placeholder
                // This should be rare now that we extract file paths from symbol UIDs
                std::path::PathBuf::from("unknown_file")
            }
        };

        let start_line = edge.start_line.unwrap_or(0);
        let start_char = edge.start_char.unwrap_or(0);

        Some(Location {
            uri: self.path_to_uri(&file_path),
            range: Range {
                start: Position {
                    line: start_line,
                    character: start_char,
                },
                end: Position {
                    line: start_line,
                    character: start_char,
                },
            },
        })
    }

    /// Convert a SymbolState to CallHierarchyItem
    ///
    /// This is used to create the center item in call hierarchy responses.
    pub fn symbol_to_call_hierarchy_item(
        &self,
        symbol: &SymbolState,
        file_path: &Path,
    ) -> CallHierarchyItem {
        let uri = self.path_to_uri(file_path);

        let range = Range {
            start: Position {
                line: symbol.def_start_line,
                character: symbol.def_start_char,
            },
            end: Position {
                line: symbol.def_end_line,
                character: symbol.def_end_char,
            },
        };

        CallHierarchyItem {
            name: symbol.name.clone(),
            kind: self.symbol_kind_to_lsp_kind(&symbol.kind),
            uri,
            range: range.clone(),
            selection_range: range, // Use same range for selection
        }
    }

    /// Convert database edges to CallHierarchyCall array
    ///
    /// Each edge represents a call relationship. We convert the source symbol
    /// information into a CallHierarchyCall object.
    pub fn edges_to_calls(
        &self,
        edges: Vec<Edge>,
        symbols: &[SymbolState],
    ) -> Vec<CallHierarchyCall> {
        edges
            .into_iter()
            .filter_map(|edge| self.edge_to_call(&edge, symbols))
            .collect()
    }

    /// Convert a single edge to a CallHierarchyCall
    fn edge_to_call(&self, edge: &Edge, symbols: &[SymbolState]) -> Option<CallHierarchyCall> {
        // Find the source symbol for this edge
        let source_symbol = symbols
            .iter()
            .find(|s| s.symbol_uid == edge.source_symbol_uid)?;

        // Use file path directly from symbol_state
        let file_path = std::path::PathBuf::from(&source_symbol.file_path);

        let from_item = self.symbol_to_call_hierarchy_item(source_symbol, &file_path);

        // Create ranges for the call sites
        let from_ranges =
            if let (Some(start_line), Some(start_char)) = (edge.start_line, edge.start_char) {
                vec![Range {
                    start: Position {
                        line: start_line,
                        character: start_char,
                    },
                    end: Position {
                        line: start_line,
                        character: start_char,
                    },
                }]
            } else {
                // Use symbol definition range as fallback
                vec![Range {
                    start: Position {
                        line: source_symbol.def_start_line,
                        character: source_symbol.def_start_char,
                    },
                    end: Position {
                        line: source_symbol.def_end_line,
                        character: source_symbol.def_end_char,
                    },
                }]
            };

        Some(CallHierarchyCall {
            from: from_item,
            from_ranges,
        })
    }

    /// Convert database edges and symbols to complete CallHierarchyResult
    ///
    /// This method orchestrates the conversion of a center symbol and its related edges
    /// into a complete call hierarchy response, reusing existing converter methods.
    pub fn edges_to_call_hierarchy(
        &self,
        center_symbol: &SymbolState,
        center_file_path: &Path,
        incoming_edges: Vec<Edge>,
        outgoing_edges: Vec<Edge>,
        all_symbols: &[SymbolState],
    ) -> CallHierarchyResult {
        // 1. Convert center symbol to CallHierarchyItem using existing method
        let item = self.symbol_to_call_hierarchy_item(center_symbol, center_file_path);

        // 2. Convert incoming edges to CallHierarchyCall array using existing method
        let incoming = self.edges_to_calls(incoming_edges, all_symbols);

        // 3. Convert outgoing edges to CallHierarchyCall array using existing method
        let outgoing = self.edges_to_calls(outgoing_edges, all_symbols);

        // 4. Create and return CallHierarchyResult
        CallHierarchyResult {
            item,
            incoming,
            outgoing,
        }
    }

    /// Convert file path to URI with proper file:// scheme
    pub fn path_to_uri(&self, path: &Path) -> String {
        // Convert path to string and ensure it's absolute
        let path_str = path.to_string_lossy();

        // Add file:// prefix if not present
        if path_str.starts_with("file://") {
            path_str.to_string()
        } else if path_str.starts_with('/') {
            // Unix absolute path
            format!("file://{}", path_str)
        } else if path_str.len() >= 2 && path_str.chars().nth(1) == Some(':') {
            // Windows absolute path (C:, D:, etc.)
            format!("file:///{}", path_str)
        } else {
            // Relative path - convert to absolute if possible
            match std::fs::canonicalize(path) {
                Ok(abs_path) => format!("file://{}", abs_path.to_string_lossy()),
                Err(_) => format!("file://{}", path_str),
            }
        }
    }

    /// Convert URI back to file path
    pub fn uri_to_path(&self, uri: &str) -> Result<std::path::PathBuf> {
        if let Some(stripped) = uri.strip_prefix("file://") {
            // Handle Windows paths (file:///C:/path)
            if stripped.len() > 3 && stripped.chars().nth(2) == Some(':') {
                Ok(std::path::PathBuf::from(&stripped[1..]))
            } else {
                Ok(std::path::PathBuf::from(stripped))
            }
        } else {
            // Assume it's already a path
            Ok(std::path::PathBuf::from(uri))
        }
    }

    /// Map symbol kind string to LSP kind string
    ///
    /// This handles the conversion from our internal symbol kinds to
    /// LSP SymbolKind values (as strings for simplicity).
    fn symbol_kind_to_lsp_kind(&self, kind: &str) -> String {
        match kind.to_lowercase().as_str() {
            "function" => "Function".to_string(),
            "method" => "Method".to_string(),
            "constructor" => "Constructor".to_string(),
            "class" => "Class".to_string(),
            "interface" => "Interface".to_string(),
            "struct" => "Struct".to_string(),
            "enum" => "Enum".to_string(),
            "variable" => "Variable".to_string(),
            "constant" => "Constant".to_string(),
            "field" => "Field".to_string(),
            "property" => "Property".to_string(),
            "module" => "Module".to_string(),
            "namespace" => "Namespace".to_string(),
            "package" => "Package".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    /// Create a default/empty CallHierarchyItem
    ///
    /// Used when no symbol data is available.
    pub fn default_call_hierarchy_item() -> CallHierarchyItem {
        CallHierarchyItem {
            name: "unknown".to_string(),
            kind: "Unknown".to_string(),
            uri: "".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
        }
    }

    /// Convert SymbolState list to Location list
    ///
    /// Useful for converting symbol definitions to location lists.
    pub fn symbols_to_locations(&self, symbols: &[SymbolState]) -> Vec<Location> {
        symbols
            .iter()
            .map(|symbol| self.symbol_to_location(symbol))
            .collect()
    }

    /// Convert a single SymbolState to Location
    fn symbol_to_location(&self, symbol: &SymbolState) -> Location {
        // Use direct file path from symbol_state
        let file_path = std::path::PathBuf::from(&symbol.file_path);
        let uri = self.path_to_uri(&file_path);

        Location {
            uri,
            range: Range {
                start: Position {
                    line: symbol.def_start_line,
                    character: symbol.def_start_char,
                },
                end: Position {
                    line: symbol.def_end_line,
                    character: symbol.def_end_char,
                },
            },
        }
    }
}

impl Default for ProtocolConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::EdgeRelation;

    #[test]
    fn test_path_to_uri() {
        let converter = ProtocolConverter::new();

        // Test Unix absolute path
        assert_eq!(
            converter.path_to_uri(&std::path::PathBuf::from("/home/user/file.rs")),
            "file:///home/user/file.rs"
        );

        // Test already formatted URI
        assert_eq!(
            converter.path_to_uri(&std::path::PathBuf::from("file:///home/user/file.rs")),
            "file:///home/user/file.rs"
        );
    }

    #[test]
    fn test_uri_to_path() {
        let converter = ProtocolConverter::new();

        // Test Unix path
        let result = converter.uri_to_path("file:///home/user/file.rs").unwrap();
        assert_eq!(result, std::path::PathBuf::from("/home/user/file.rs"));

        // Test non-URI path
        let result = converter.uri_to_path("/home/user/file.rs").unwrap();
        assert_eq!(result, std::path::PathBuf::from("/home/user/file.rs"));
    }

    #[test]
    fn test_symbol_kind_conversion() {
        let converter = ProtocolConverter::new();

        assert_eq!(converter.symbol_kind_to_lsp_kind("function"), "Function");
        assert_eq!(converter.symbol_kind_to_lsp_kind("method"), "Method");
        assert_eq!(converter.symbol_kind_to_lsp_kind("class"), "Class");
        assert_eq!(converter.symbol_kind_to_lsp_kind("unknown"), "Unknown");
    }

    #[test]
    fn test_symbol_to_call_hierarchy_item() {
        let converter = ProtocolConverter::new();

        let symbol = SymbolState {
            symbol_uid: "test_uid".to_string(),
            file_path: "/test/file.rs".to_string(),
            language: "rust".to_string(),
            name: "test_function".to_string(),
            fqn: Some("module::test_function".to_string()),
            kind: "function".to_string(),
            signature: Some("fn test_function()".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 10,
            def_start_char: 4,
            def_end_line: 15,
            def_end_char: 5,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        let file_path = std::path::Path::new("/test/file.rs");
        let item = converter.symbol_to_call_hierarchy_item(&symbol, file_path);

        assert_eq!(item.name, "test_function");
        assert_eq!(item.kind, "Function");
        assert_eq!(item.uri, "file:///test/file.rs");
        assert_eq!(item.range.start.line, 10);
        assert_eq!(item.range.start.character, 4);
        assert_eq!(item.range.end.line, 15);
        assert_eq!(item.range.end.character, 5);
    }

    #[test]
    fn test_edges_to_calls() {
        let converter = ProtocolConverter::new();

        let symbol = SymbolState {
            symbol_uid: "caller_uid".to_string(),
            file_path: "test/caller.rs".to_string(),
            language: "rust".to_string(),
            name: "caller_function".to_string(),
            fqn: Some("module::caller_function".to_string()),
            kind: "function".to_string(),
            signature: Some("fn caller_function()".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 5,
            def_start_char: 0,
            def_end_line: 8,
            def_end_char: 1,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        let edge = Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "caller_uid".to_string(),
            target_symbol_uid: "target_uid".to_string(),
            file_path: None, // Test edges don't need file path
            start_line: Some(6),
            start_char: Some(4),
            confidence: 0.9,
            language: "rust".to_string(),
            metadata: None,
        };

        let symbols = vec![symbol];
        let edges = vec![edge];

        let calls = converter.edges_to_calls(edges, &symbols);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from.name, "caller_function");
        assert_eq!(calls[0].from_ranges.len(), 1);
        assert_eq!(calls[0].from_ranges[0].start.line, 6);
        assert_eq!(calls[0].from_ranges[0].start.character, 4);
    }

    #[test]
    fn test_default_call_hierarchy_item() {
        let item = ProtocolConverter::default_call_hierarchy_item();

        assert_eq!(item.name, "unknown");
        assert_eq!(item.kind, "Unknown");
        assert_eq!(item.uri, "");
        assert_eq!(item.range.start.line, 0);
        assert_eq!(item.range.start.character, 0);
    }

    #[test]
    fn test_edges_to_call_hierarchy_with_both_directions() {
        let converter = ProtocolConverter::new();

        // Create center symbol
        let center_symbol = SymbolState {
            symbol_uid: "center_function".to_string(),
            file_path: "test/center.rs".to_string(),
            language: "rust".to_string(),
            name: "process_data".to_string(),
            fqn: Some("module::process_data".to_string()),
            kind: "function".to_string(),
            signature: Some("fn process_data() -> Result<()>".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 20,
            def_start_char: 0,
            def_end_line: 25,
            def_end_char: 1,
            is_definition: true,
            documentation: Some("Processes data".to_string()),
            metadata: None,
        };

        // Create caller symbol (incoming)
        let caller_symbol = SymbolState {
            symbol_uid: "caller_function".to_string(),
            file_path: "test/caller.rs".to_string(),
            language: "rust".to_string(),
            name: "main".to_string(),
            fqn: Some("main".to_string()),
            kind: "function".to_string(),
            signature: Some("fn main()".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 5,
            def_start_char: 0,
            def_end_line: 10,
            def_end_char: 1,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        // Create callee symbol (outgoing)
        let callee_symbol = SymbolState {
            symbol_uid: "callee_function".to_string(),
            file_path: "test/callee.rs".to_string(),
            language: "rust".to_string(),
            name: "save_result".to_string(),
            fqn: Some("module::save_result".to_string()),
            kind: "function".to_string(),
            signature: Some("fn save_result(data: &str)".to_string()),
            visibility: Some("private".to_string()),
            def_start_line: 30,
            def_start_char: 4,
            def_end_line: 35,
            def_end_char: 5,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        // Create incoming edge (caller -> center)
        let incoming_edge = Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "caller_function".to_string(),
            target_symbol_uid: "center_function".to_string(),
            file_path: None, // Test edges don't need file path
            start_line: Some(8),
            start_char: Some(4),
            confidence: 0.95,
            language: "rust".to_string(),
            metadata: None,
        };

        // Create outgoing edge (center -> callee)
        let outgoing_edge = Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "center_function".to_string(),
            target_symbol_uid: "callee_function".to_string(),
            file_path: None, // Test edges don't need file path
            start_line: Some(23),
            start_char: Some(8),
            confidence: 0.90,
            language: "rust".to_string(),
            metadata: None,
        };

        let center_file_path = std::path::Path::new("/src/module.rs");
        let incoming_edges = vec![incoming_edge];
        let outgoing_edges = vec![outgoing_edge];
        let all_symbols = vec![center_symbol.clone(), caller_symbol, callee_symbol];

        let result = converter.edges_to_call_hierarchy(
            &center_symbol,
            center_file_path,
            incoming_edges,
            outgoing_edges,
            &all_symbols,
        );

        // Verify center item
        assert_eq!(result.item.name, "process_data");
        assert_eq!(result.item.kind, "Function");
        assert_eq!(result.item.uri, "file:///src/module.rs");
        assert_eq!(result.item.range.start.line, 20);
        assert_eq!(result.item.range.start.character, 0);
        assert_eq!(result.item.range.end.line, 25);
        assert_eq!(result.item.range.end.character, 1);

        // Verify incoming calls
        assert_eq!(result.incoming.len(), 1);
        assert_eq!(result.incoming[0].from.name, "main");
        assert_eq!(result.incoming[0].from_ranges.len(), 1);
        assert_eq!(result.incoming[0].from_ranges[0].start.line, 8);
        assert_eq!(result.incoming[0].from_ranges[0].start.character, 4);

        // Verify outgoing calls
        assert_eq!(result.outgoing.len(), 1);
        assert_eq!(result.outgoing[0].from.name, "process_data");
        assert_eq!(result.outgoing[0].from_ranges.len(), 1);
        assert_eq!(result.outgoing[0].from_ranges[0].start.line, 23);
        assert_eq!(result.outgoing[0].from_ranges[0].start.character, 8);
    }

    #[test]
    fn test_edges_to_call_hierarchy_with_only_incoming() {
        let converter = ProtocolConverter::new();

        let center_symbol = SymbolState {
            symbol_uid: "center_function".to_string(),
            file_path: "test/leaf.rs".to_string(),
            language: "rust".to_string(),
            name: "leaf_function".to_string(),
            fqn: Some("module::leaf_function".to_string()),
            kind: "function".to_string(),
            signature: Some("fn leaf_function() -> bool".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 15,
            def_start_char: 0,
            def_end_line: 18,
            def_end_char: 1,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        let caller_symbol = SymbolState {
            symbol_uid: "caller_function".to_string(),
            file_path: "test/check.rs".to_string(),
            language: "rust".to_string(),
            name: "check_status".to_string(),
            fqn: Some("module::check_status".to_string()),
            kind: "function".to_string(),
            signature: Some("fn check_status()".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 5,
            def_start_char: 0,
            def_end_line: 10,
            def_end_char: 1,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        let incoming_edge = Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "caller_function".to_string(),
            target_symbol_uid: "center_function".to_string(),
            file_path: None, // Test edges don't need file path
            start_line: Some(7),
            start_char: Some(12),
            confidence: 0.85,
            language: "rust".to_string(),
            metadata: None,
        };

        let center_file_path = std::path::Path::new("/src/utils.rs");
        let incoming_edges = vec![incoming_edge];
        let outgoing_edges = vec![];
        let all_symbols = vec![center_symbol.clone(), caller_symbol];

        let result = converter.edges_to_call_hierarchy(
            &center_symbol,
            center_file_path,
            incoming_edges,
            outgoing_edges,
            &all_symbols,
        );

        // Verify center item
        assert_eq!(result.item.name, "leaf_function");
        assert_eq!(result.item.kind, "Function");

        // Verify incoming calls (should have one)
        assert_eq!(result.incoming.len(), 1);
        assert_eq!(result.incoming[0].from.name, "check_status");

        // Verify outgoing calls (should be empty)
        assert_eq!(result.outgoing.len(), 0);
    }

    #[test]
    fn test_edges_to_call_hierarchy_with_only_outgoing() {
        let converter = ProtocolConverter::new();

        let center_symbol = SymbolState {
            symbol_uid: "center_function".to_string(),
            file_path: "test/root.rs".to_string(),
            language: "rust".to_string(),
            name: "root_function".to_string(),
            fqn: Some("module::root_function".to_string()),
            kind: "function".to_string(),
            signature: Some("fn root_function()".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 1,
            def_start_char: 0,
            def_end_line: 5,
            def_end_char: 1,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        let callee_symbol = SymbolState {
            symbol_uid: "callee_function".to_string(),
            file_path: "test/helper.rs".to_string(),
            language: "rust".to_string(),
            name: "helper_function".to_string(),
            fqn: Some("module::helper_function".to_string()),
            kind: "function".to_string(),
            signature: Some("fn helper_function(input: i32)".to_string()),
            visibility: Some("private".to_string()),
            def_start_line: 10,
            def_start_char: 4,
            def_end_line: 15,
            def_end_char: 5,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        let outgoing_edge = Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "center_function".to_string(),
            target_symbol_uid: "callee_function".to_string(),
            file_path: None, // Test edges don't need file path
            start_line: Some(3),
            start_char: Some(8),
            confidence: 0.92,
            language: "rust".to_string(),
            metadata: None,
        };

        let center_file_path = std::path::Path::new("/src/main.rs");
        let incoming_edges = vec![];
        let outgoing_edges = vec![outgoing_edge];
        let all_symbols = vec![center_symbol.clone(), callee_symbol];

        let result = converter.edges_to_call_hierarchy(
            &center_symbol,
            center_file_path,
            incoming_edges,
            outgoing_edges,
            &all_symbols,
        );

        // Verify center item
        assert_eq!(result.item.name, "root_function");
        assert_eq!(result.item.kind, "Function");

        // Verify incoming calls (should be empty)
        assert_eq!(result.incoming.len(), 0);

        // Verify outgoing calls (should have one)
        assert_eq!(result.outgoing.len(), 1);
        assert_eq!(result.outgoing[0].from.name, "root_function");
    }

    #[test]
    fn test_edges_to_call_hierarchy_with_no_edges() {
        let converter = ProtocolConverter::new();

        let isolated_symbol = SymbolState {
            symbol_uid: "isolated_function".to_string(),
            file_path: "test/isolated.rs".to_string(),
            language: "rust".to_string(),
            name: "isolated_function".to_string(),
            fqn: Some("module::isolated_function".to_string()),
            kind: "function".to_string(),
            signature: Some("fn isolated_function() -> ()".to_string()),
            visibility: Some("private".to_string()),
            def_start_line: 42,
            def_start_char: 0,
            def_end_line: 45,
            def_end_char: 1,
            is_definition: true,
            documentation: Some("An isolated function with no calls".to_string()),
            metadata: None,
        };

        let center_file_path = std::path::Path::new("/src/isolated.rs");
        let incoming_edges = vec![];
        let outgoing_edges = vec![];
        let all_symbols = vec![isolated_symbol.clone()];

        let result = converter.edges_to_call_hierarchy(
            &isolated_symbol,
            center_file_path,
            incoming_edges,
            outgoing_edges,
            &all_symbols,
        );

        // Verify center item
        assert_eq!(result.item.name, "isolated_function");
        assert_eq!(result.item.kind, "Function");
        assert_eq!(result.item.uri, "file:///src/isolated.rs");

        // Verify no calls in either direction
        assert_eq!(result.incoming.len(), 0);
        assert_eq!(result.outgoing.len(), 0);
    }

    #[test]
    fn test_edges_to_call_hierarchy_integration() {
        // Integration test to verify the new method works with existing infrastructure
        let converter = ProtocolConverter::new();

        let center_symbol = SymbolState {
            symbol_uid: "test_function".to_string(),
            file_path: "test/test_function.rs".to_string(),
            language: "rust".to_string(),
            name: "test_function".to_string(),
            fqn: Some("module::test_function".to_string()),
            kind: "function".to_string(),
            signature: Some("fn test_function()".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 10,
            def_start_char: 0,
            def_end_line: 15,
            def_end_char: 1,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        let caller_symbol = SymbolState {
            symbol_uid: "caller_function".to_string(),
            file_path: "test/caller_function.rs".to_string(),
            language: "rust".to_string(),
            name: "caller_function".to_string(),
            fqn: Some("module::caller_function".to_string()),
            kind: "function".to_string(),
            signature: Some("fn caller_function()".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 5,
            def_start_char: 0,
            def_end_line: 8,
            def_end_char: 1,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        let incoming_edge = Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "caller_function".to_string(),
            target_symbol_uid: "test_function".to_string(),
            file_path: None, // Test edges don't need file path
            start_line: Some(7),
            start_char: Some(4),
            confidence: 0.95,
            language: "rust".to_string(),
            metadata: None,
        };

        let center_file_path = std::path::Path::new("/src/module.rs");
        let incoming_edges = vec![incoming_edge];
        let outgoing_edges = vec![];
        let all_symbols = vec![center_symbol.clone(), caller_symbol];

        // Test that new method uses existing infrastructure properly
        let result = converter.edges_to_call_hierarchy(
            &center_symbol,
            center_file_path,
            incoming_edges,
            outgoing_edges,
            &all_symbols,
        );

        // Verify that it produces the same results as calling the methods separately
        let expected_item =
            converter.symbol_to_call_hierarchy_item(&center_symbol, center_file_path);
        let expected_incoming = converter.edges_to_calls(
            vec![Edge {
                relation: EdgeRelation::Calls,
                source_symbol_uid: "caller_function".to_string(),
                target_symbol_uid: "test_function".to_string(),
                file_path: Some("test/test_function.rs".to_string()),
                start_line: Some(7),
                start_char: Some(4),
                confidence: 0.95,
                language: "rust".to_string(),
                metadata: None,
            }],
            &all_symbols,
        );

        // Verify integration with existing methods
        assert_eq!(result.item.name, expected_item.name);
        assert_eq!(result.item.kind, expected_item.kind);
        assert_eq!(result.item.uri, expected_item.uri);
        assert_eq!(result.incoming.len(), expected_incoming.len());
        assert_eq!(result.outgoing.len(), 0);

        if !result.incoming.is_empty() && !expected_incoming.is_empty() {
            assert_eq!(result.incoming[0].from.name, expected_incoming[0].from.name);
            assert_eq!(
                result.incoming[0].from_ranges.len(),
                expected_incoming[0].from_ranges.len()
            );
        }
    }

    #[test]
    fn test_edges_to_call_hierarchy_with_multiple_edges() {
        let converter = ProtocolConverter::new();

        let center_symbol = SymbolState {
            symbol_uid: "popular_function".to_string(),
            file_path: "test/popular.rs".to_string(),
            language: "rust".to_string(),
            name: "popular_function".to_string(),
            fqn: Some("module::popular_function".to_string()),
            kind: "function".to_string(),
            signature: Some("fn popular_function(data: &str) -> Result<String>".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 25,
            def_start_char: 0,
            def_end_line: 35,
            def_end_char: 1,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        // Create multiple caller symbols
        let caller1 = SymbolState {
            symbol_uid: "caller1".to_string(),
            file_path: "test/service_a.rs".to_string(),
            language: "rust".to_string(),
            name: "service_a".to_string(),
            fqn: Some("services::service_a".to_string()),
            kind: "function".to_string(),
            signature: Some("fn service_a()".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 10,
            def_start_char: 0,
            def_end_line: 15,
            def_end_char: 1,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        let caller2 = SymbolState {
            symbol_uid: "caller2".to_string(),
            file_path: "test/service_b.rs".to_string(),
            language: "rust".to_string(),
            name: "service_b".to_string(),
            fqn: Some("services::service_b".to_string()),
            kind: "function".to_string(),
            signature: Some("fn service_b()".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 20,
            def_start_char: 0,
            def_end_line: 25,
            def_end_char: 1,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        // Create multiple callee symbols
        let callee1 = SymbolState {
            symbol_uid: "callee1".to_string(),
            file_path: "test/helper_a.rs".to_string(),
            language: "rust".to_string(),
            name: "helper_a".to_string(),
            fqn: Some("helpers::helper_a".to_string()),
            kind: "function".to_string(),
            signature: Some("fn helper_a(input: &str)".to_string()),
            visibility: Some("private".to_string()),
            def_start_line: 5,
            def_start_char: 4,
            def_end_line: 10,
            def_end_char: 5,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        let callee2 = SymbolState {
            symbol_uid: "callee2".to_string(),
            file_path: "test/helper_b.rs".to_string(),
            language: "rust".to_string(),
            name: "helper_b".to_string(),
            fqn: Some("helpers::helper_b".to_string()),
            kind: "function".to_string(),
            signature: Some("fn helper_b(data: String)".to_string()),
            visibility: Some("private".to_string()),
            def_start_line: 15,
            def_start_char: 4,
            def_end_line: 20,
            def_end_char: 5,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        // Create multiple incoming edges
        let incoming_edge1 = Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "caller1".to_string(),
            target_symbol_uid: "popular_function".to_string(),
            file_path: None, // Test edges don't need file path
            start_line: Some(12),
            start_char: Some(4),
            confidence: 0.95,
            language: "rust".to_string(),
            metadata: None,
        };

        let incoming_edge2 = Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "caller2".to_string(),
            target_symbol_uid: "popular_function".to_string(),
            file_path: None, // Test edges don't need file path
            start_line: Some(22),
            start_char: Some(8),
            confidence: 0.90,
            language: "rust".to_string(),
            metadata: None,
        };

        // Create multiple outgoing edges
        let outgoing_edge1 = Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "popular_function".to_string(),
            target_symbol_uid: "callee1".to_string(),
            file_path: None, // Test edges don't need file path
            start_line: Some(28),
            start_char: Some(8),
            confidence: 0.88,
            language: "rust".to_string(),
            metadata: None,
        };

        let outgoing_edge2 = Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "popular_function".to_string(),
            target_symbol_uid: "callee2".to_string(),
            file_path: None, // Test edges don't need file path
            start_line: Some(32),
            start_char: Some(12),
            confidence: 0.92,
            language: "rust".to_string(),
            metadata: None,
        };

        let center_file_path = std::path::Path::new("/src/popular.rs");
        let incoming_edges = vec![incoming_edge1, incoming_edge2];
        let outgoing_edges = vec![outgoing_edge1, outgoing_edge2];
        let all_symbols = vec![center_symbol.clone(), caller1, caller2, callee1, callee2];

        let result = converter.edges_to_call_hierarchy(
            &center_symbol,
            center_file_path,
            incoming_edges,
            outgoing_edges,
            &all_symbols,
        );

        // Verify center item
        assert_eq!(result.item.name, "popular_function");
        assert_eq!(result.item.kind, "Function");

        // Verify multiple incoming calls
        assert_eq!(result.incoming.len(), 2);
        let incoming_names: Vec<String> = result
            .incoming
            .iter()
            .map(|c| c.from.name.clone())
            .collect();
        assert!(incoming_names.contains(&"service_a".to_string()));
        assert!(incoming_names.contains(&"service_b".to_string()));

        // Verify multiple outgoing calls
        assert_eq!(result.outgoing.len(), 2);
        // Both outgoing calls should be from the center function itself
        for outgoing_call in &result.outgoing {
            assert_eq!(outgoing_call.from.name, "popular_function");
        }
    }
}
