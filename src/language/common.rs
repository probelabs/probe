use std::collections::HashSet;
use tree_sitter::Node;

/// Last row occupied by a node, converting tree-sitter's exclusive end position.
pub fn node_end_row(node: &Node) -> usize {
    let end = node.end_position();
    if end.column == 0 && end.row > node.start_position().row {
        end.row - 1
    } else {
        end.row
    }
}

/// Helper function to collect all node types in the AST
pub fn collect_node_types(node: Node, node_types: &mut HashSet<String>) {
    node_types.insert(node.kind().to_string());

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_types(child, node_types);
    }
}
