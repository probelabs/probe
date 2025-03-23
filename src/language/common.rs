use std::collections::HashSet;
use tree_sitter::Node;

/// Helper function to collect all node types in the AST
pub fn collect_node_types(node: Node, node_types: &mut HashSet<String>) {
    node_types.insert(node.kind().to_string());

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_types(child, node_types);
    }
}
