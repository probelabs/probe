//! Graph export functionality for the LSP daemon
//!
//! This module provides graph export capabilities, supporting multiple formats:
//! - JSON: Structured data with nodes and edges
//! - GraphML: XML-based graph format for visualization tools
//! - DOT: Graphviz format for graph rendering
//!
//! The exported graphs include symbols as nodes and relationships (calls, references, etc.) as edges.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::database::{DatabaseBackend, Edge, SymbolState};

/// Graph export options
#[derive(Debug, Clone)]
pub struct GraphExportOptions {
    /// Maximum depth for graph traversal (None = unlimited)
    pub max_depth: Option<u32>,
    /// Filter by symbol types (None = all types)
    pub symbol_types_filter: Option<Vec<String>>,
    /// Filter by edge types (None = all types)
    pub edge_types_filter: Option<Vec<String>>,
    /// Include only connected symbols (symbols with at least one edge)
    pub connected_only: bool,
}

impl Default for GraphExportOptions {
    fn default() -> Self {
        Self {
            max_depth: None,
            symbol_types_filter: None,
            edge_types_filter: None,
            connected_only: false,
        }
    }
}

/// Represents a graph node (symbol) for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub file_path: Option<String>,
    pub line: u32,
    pub column: u32,
    pub signature: Option<String>,
    pub visibility: Option<String>,
    pub documentation: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Represents a graph edge (relationship) for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub confidence: f32,
    pub source_location: Option<String>,
    pub target_location: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Complete graph representation for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub metadata: GraphMetadata,
}

/// Graph metadata for context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetadata {
    pub workspace_path: PathBuf,
    pub export_timestamp: String,
    pub nodes_count: usize,
    pub edges_count: usize,
    pub filtered_symbol_types: Option<Vec<String>>,
    pub filtered_edge_types: Option<Vec<String>>,
    pub max_depth: Option<u32>,
    pub connected_only: bool,
}

/// Graph exporter that handles different output formats
pub struct GraphExporter;

impl GraphExporter {
    /// Export graph from database backend with specified options
    pub async fn export_graph<T: DatabaseBackend>(
        backend: &T,
        workspace_path: PathBuf,
        options: GraphExportOptions,
    ) -> Result<ExportGraph> {
        // Step 1: Get all symbols and edges from the database
        let symbols = Self::get_filtered_symbols(backend, &options).await?;
        let edges = Self::get_filtered_edges(backend, &options).await?;

        // Step 2: Filter connected symbols if requested
        let (final_symbols, final_edges) = if options.connected_only {
            Self::filter_connected_only(symbols, edges)
        } else {
            (symbols, edges)
        };

        // Step 3: Convert to graph representation
        let nodes = Self::symbols_to_nodes(&final_symbols);
        let graph_edges = Self::edges_to_graph_edges(&final_edges);

        // Step 4: Create metadata
        let metadata = GraphMetadata {
            workspace_path: workspace_path.clone(),
            export_timestamp: chrono::Utc::now().to_rfc3339(),
            nodes_count: nodes.len(),
            edges_count: graph_edges.len(),
            filtered_symbol_types: options.symbol_types_filter,
            filtered_edge_types: options.edge_types_filter,
            max_depth: options.max_depth,
            connected_only: options.connected_only,
        };

        Ok(ExportGraph {
            nodes,
            edges: graph_edges,
            metadata,
        })
    }

    /// Serialize graph to JSON format
    pub fn to_json(graph: &ExportGraph) -> Result<String> {
        serde_json::to_string_pretty(graph)
            .map_err(|e| anyhow::anyhow!("JSON serialization failed: {}", e))
    }

    /// Serialize graph to GraphML format
    pub fn to_graphml(graph: &ExportGraph) -> Result<String> {
        let mut output = String::new();

        // GraphML header
        output.push_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns" 
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" 
         xsi:schemaLocation="http://graphml.graphdrawing.org/xmlns 
         http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd">
"#,
        );

        // Define attribute keys
        output.push_str(
            r#"  <key id="label" for="node" attr.name="label" attr.type="string"/>
  <key id="kind" for="node" attr.name="kind" attr.type="string"/>
  <key id="file_path" for="node" attr.name="file_path" attr.type="string"/>
  <key id="line" for="node" attr.name="line" attr.type="int"/>
  <key id="column" for="node" attr.name="column" attr.type="int"/>
  <key id="signature" for="node" attr.name="signature" attr.type="string"/>
  <key id="visibility" for="node" attr.name="visibility" attr.type="string"/>
  <key id="documentation" for="node" attr.name="documentation" attr.type="string"/>
  <key id="relation" for="edge" attr.name="relation" attr.type="string"/>
  <key id="confidence" for="edge" attr.name="confidence" attr.type="double"/>
"#,
        );

        // Graph opening
        output.push_str("  <graph id=\"codebase_graph\" edgedefault=\"directed\">\n");

        // Add nodes
        for node in &graph.nodes {
            output.push_str(&format!(
                "    <node id=\"{}\">\n",
                Self::escape_xml(&node.id)
            ));
            output.push_str(&format!(
                "      <data key=\"label\">{}</data>\n",
                Self::escape_xml(&node.label)
            ));
            output.push_str(&format!(
                "      <data key=\"kind\">{}</data>\n",
                Self::escape_xml(&node.kind)
            ));

            if let Some(file_path) = &node.file_path {
                output.push_str(&format!(
                    "      <data key=\"file_path\">{}</data>\n",
                    Self::escape_xml(file_path)
                ));
            }

            output.push_str(&format!("      <data key=\"line\">{}</data>\n", node.line));
            output.push_str(&format!(
                "      <data key=\"column\">{}</data>\n",
                node.column
            ));

            if let Some(signature) = &node.signature {
                output.push_str(&format!(
                    "      <data key=\"signature\">{}</data>\n",
                    Self::escape_xml(signature)
                ));
            }

            if let Some(visibility) = &node.visibility {
                output.push_str(&format!(
                    "      <data key=\"visibility\">{}</data>\n",
                    Self::escape_xml(visibility)
                ));
            }

            if let Some(documentation) = &node.documentation {
                output.push_str(&format!(
                    "      <data key=\"documentation\">{}</data>\n",
                    Self::escape_xml(documentation)
                ));
            }

            output.push_str("    </node>\n");
        }

        // Add edges
        for (i, edge) in graph.edges.iter().enumerate() {
            output.push_str(&format!(
                "    <edge id=\"e{}\" source=\"{}\" target=\"{}\">\n",
                i,
                Self::escape_xml(&edge.source),
                Self::escape_xml(&edge.target)
            ));
            output.push_str(&format!(
                "      <data key=\"relation\">{}</data>\n",
                Self::escape_xml(&edge.relation)
            ));
            output.push_str(&format!(
                "      <data key=\"confidence\">{}</data>\n",
                edge.confidence
            ));
            output.push_str("    </edge>\n");
        }

        // Graph closing
        output.push_str("  </graph>\n</graphml>\n");

        Ok(output)
    }

    /// Serialize graph to DOT format (Graphviz)
    pub fn to_dot(graph: &ExportGraph) -> Result<String> {
        let mut output = String::new();

        // DOT header
        output.push_str("digraph codebase_graph {\n");
        output.push_str("  rankdir=TB;\n");
        output.push_str("  node [shape=box, style=filled];\n");
        output.push_str("  edge [fontsize=10];\n\n");

        // Add nodes with styling based on kind
        for node in &graph.nodes {
            let color = Self::get_node_color(&node.kind);
            let escaped_id = Self::escape_dot_id(&node.id);
            let escaped_label = Self::escape_dot_label(&node.label);

            let mut tooltip = format!(
                "{}\\n{}",
                node.kind,
                node.file_path.as_deref().unwrap_or("")
            );
            if let Some(sig) = &node.signature {
                tooltip.push_str(&format!("\\n{}", sig));
            }

            output.push_str(&format!(
                "  {} [label=\"{}\", fillcolor=\"{}\", tooltip=\"{}\"];\n",
                escaped_id,
                escaped_label,
                color,
                Self::escape_dot_label(&tooltip)
            ));
        }

        output.push_str("\n");

        // Add edges with labels
        for edge in &graph.edges {
            let escaped_source = Self::escape_dot_id(&edge.source);
            let escaped_target = Self::escape_dot_id(&edge.target);
            let edge_style = Self::get_edge_style(&edge.relation);

            output.push_str(&format!(
                "  {} -> {} [label=\"{}\", {}];\n",
                escaped_source, escaped_target, edge.relation, edge_style
            ));
        }

        output.push_str("}\n");

        Ok(output)
    }

    // Helper methods

    async fn get_filtered_symbols<T: DatabaseBackend>(
        backend: &T,
        options: &GraphExportOptions,
    ) -> Result<Vec<SymbolState>> {
        // Get all symbols from database
        let mut symbols = backend
            .get_all_symbols()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get all symbols: {}", e))?;

        // Filter by symbol types if specified
        if let Some(symbol_types) = &options.symbol_types_filter {
            symbols.retain(|symbol| symbol_types.contains(&symbol.kind));
        }

        Ok(symbols)
    }

    async fn get_filtered_edges<T: DatabaseBackend>(
        backend: &T,
        options: &GraphExportOptions,
    ) -> Result<Vec<Edge>> {
        // Get all edges from database
        let mut edges = backend
            .get_all_edges()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get all edges: {}", e))?;

        // Filter by edge types if specified
        if let Some(edge_types) = &options.edge_types_filter {
            edges.retain(|edge| edge_types.iter().any(|et| et == edge.relation.to_string()));
        }

        Ok(edges)
    }

    fn filter_connected_only(
        symbols: Vec<SymbolState>,
        edges: Vec<Edge>,
    ) -> (Vec<SymbolState>, Vec<Edge>) {
        // Create set of all symbol UIDs that have at least one edge
        let mut connected_symbols = HashSet::new();

        for edge in &edges {
            connected_symbols.insert(edge.source_symbol_uid.clone());
            connected_symbols.insert(edge.target_symbol_uid.clone());
        }

        // Filter symbols to only include connected ones
        let filtered_symbols: Vec<SymbolState> = symbols
            .into_iter()
            .filter(|symbol| connected_symbols.contains(&symbol.symbol_uid))
            .collect();

        (filtered_symbols, edges)
    }

    fn symbols_to_nodes(symbols: &[SymbolState]) -> Vec<GraphNode> {
        symbols
            .iter()
            .map(|symbol| {
                let mut metadata = HashMap::new();

                if let Some(fqn) = &symbol.fqn {
                    metadata.insert("fqn".to_string(), fqn.clone());
                }

                if symbol.is_definition {
                    metadata.insert("is_definition".to_string(), "true".to_string());
                }

                metadata.insert("language".to_string(), symbol.language.clone());

                GraphNode {
                    id: symbol.symbol_uid.clone(),
                    label: symbol.name.clone(),
                    kind: symbol.kind.clone(),
                    file_path: None, // TODO: Resolve file path from file_version_id
                    line: symbol.def_start_line,
                    column: symbol.def_start_char,
                    signature: symbol.signature.clone(),
                    visibility: symbol.visibility.clone(),
                    documentation: symbol.documentation.clone(),
                    metadata,
                }
            })
            .collect()
    }

    fn edges_to_graph_edges(edges: &[Edge]) -> Vec<GraphEdge> {
        edges
            .iter()
            .map(|edge| {
                let mut metadata = HashMap::new();
                metadata.insert("language".to_string(), edge.language.clone());

                if let Some(meta) = &edge.metadata {
                    metadata.insert("extra_metadata".to_string(), meta.clone());
                }

                GraphEdge {
                    source: edge.source_symbol_uid.clone(),
                    target: edge.target_symbol_uid.clone(),
                    relation: edge.relation.to_string().to_string(),
                    confidence: edge.confidence,
                    source_location: edge
                        .start_line
                        .map(|line| format!("{}:{}", line, edge.start_char.unwrap_or(0))),
                    target_location: None, // TODO: Add target location if available
                    metadata,
                }
            })
            .collect()
    }

    fn escape_xml(s: &str) -> String {
        s.replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace("\"", "&quot;")
            .replace("'", "&apos;")
    }

    fn escape_dot_id(s: &str) -> String {
        format!("\"{}\"", s.replace("\"", "\\\""))
    }

    fn escape_dot_label(s: &str) -> String {
        s.replace("\"", "\\\"")
            .replace("\n", "\\n")
            .replace("\t", "\\t")
    }

    fn get_node_color(kind: &str) -> &'static str {
        match kind {
            "function" | "method" => "lightblue",
            "class" | "struct" => "lightgreen",
            "interface" | "trait" => "lightyellow",
            "enum" => "lightpink",
            "variable" | "field" => "lightgray",
            "module" | "namespace" => "lightcyan",
            _ => "white",
        }
    }

    fn get_edge_style(relation: &str) -> &'static str {
        match relation {
            "calls" => "color=blue",
            "references" => "color=gray, style=dashed",
            "inherits_from" => "color=green, style=bold",
            "implements" => "color=green, style=dotted",
            "has_child" => "color=purple",
            _ => "color=black",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_export_options_default() {
        let options = GraphExportOptions::default();
        assert_eq!(options.max_depth, None);
        assert_eq!(options.symbol_types_filter, None);
        assert_eq!(options.edge_types_filter, None);
        assert!(!options.connected_only);
    }

    #[test]
    fn test_escape_xml() {
        let input = r#"<function name="test" & 'other'>"#;
        let expected = "&lt;function name=&quot;test&quot; &amp; &apos;other&apos;&gt;";
        assert_eq!(GraphExporter::escape_xml(input), expected);
    }

    #[test]
    fn test_escape_dot_label() {
        let input = "function\ntest()";
        let expected = "function\\ntest()";
        assert_eq!(GraphExporter::escape_dot_label(input), expected);
    }

    #[test]
    fn test_get_node_color() {
        assert_eq!(GraphExporter::get_node_color("function"), "lightblue");
        assert_eq!(GraphExporter::get_node_color("class"), "lightgreen");
        assert_eq!(GraphExporter::get_node_color("unknown"), "white");
    }

    #[test]
    fn test_get_edge_style() {
        assert_eq!(GraphExporter::get_edge_style("calls"), "color=blue");
        assert_eq!(
            GraphExporter::get_edge_style("references"),
            "color=gray, style=dashed"
        );
        assert_eq!(GraphExporter::get_edge_style("unknown"), "color=black");
    }

    #[tokio::test]
    async fn test_graph_export_with_real_data() -> Result<(), Box<dyn std::error::Error>> {
        use crate::database::{
            DatabaseBackend, DatabaseConfig, Edge, EdgeRelation, SQLiteBackend, SymbolState,
        };
        use std::sync::Arc;

        // Create a temporary database
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let db = Arc::new(SQLiteBackend::new(config).await?);

        // Create test symbols
        let symbols = vec![SymbolState {
            symbol_uid: "test_fn_1".to_string(),
            file_path: "test/test_fn.rs".to_string(),
            language: "rust".to_string(),
            name: "test_function".to_string(),
            fqn: Some("mod::test_function".to_string()),
            kind: "function".to_string(),
            signature: Some("fn test_function()".to_string()),
            visibility: Some("pub".to_string()),
            def_start_line: 10,
            def_start_char: 4,
            def_end_line: 15,
            def_end_char: 5,
            is_definition: true,
            documentation: None,
            metadata: None,
        }];

        // Create test edges
        let edges = vec![Edge {
            relation: EdgeRelation::Calls,
            source_symbol_uid: "test_fn_1".to_string(),
            target_symbol_uid: "test_fn_2".to_string(),
            file_path: Some("test/test_fn.rs".to_string()),
            start_line: Some(12),
            start_char: Some(8),
            confidence: 0.9,
            language: "rust".to_string(),
            metadata: None,
        }];

        // Store test data
        db.store_symbols(&symbols).await?;
        db.store_edges(&edges).await?;

        // Test graph export
        let options = GraphExportOptions::default();
        let graph = GraphExporter::export_graph(&*db, PathBuf::from("/test"), options).await?;

        // Verify results
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.nodes[0].id, "test_fn_1");
        assert_eq!(graph.edges[0].source, "test_fn_1");

        println!(
            "✅ Graph export test passed: {} nodes, {} edges",
            graph.nodes.len(),
            graph.edges.len()
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_to_json_serialization() {
        let graph = ExportGraph {
            nodes: vec![GraphNode {
                id: "test_fn".to_string(),
                label: "test".to_string(),
                kind: "function".to_string(),
                file_path: Some("test.rs".to_string()),
                line: 10,
                column: 4,
                signature: Some("fn test()".to_string()),
                visibility: Some("pub".to_string()),
                documentation: None,
                metadata: HashMap::new(),
            }],
            edges: vec![],
            metadata: GraphMetadata {
                workspace_path: PathBuf::from("/test/workspace"),
                export_timestamp: "2024-01-01T00:00:00Z".to_string(),
                nodes_count: 1,
                edges_count: 0,
                filtered_symbol_types: None,
                filtered_edge_types: None,
                max_depth: None,
                connected_only: false,
            },
        };

        let json = GraphExporter::to_json(&graph).unwrap();
        assert!(json.contains("test_fn"));
        assert!(json.contains("function"));
        assert!(json.contains("/test/workspace"));

        // Verify it's valid JSON by parsing it back
        let _parsed: ExportGraph = serde_json::from_str(&json).unwrap();
    }
}
