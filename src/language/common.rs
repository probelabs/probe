use std::collections::HashSet;
use tree_sitter::Node;

/// Helper function to find the most specific node that contains a given line
pub fn find_most_specific_node<'a>(node: Node<'a>, line: usize) -> Node<'a> {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    // Check if the node contains the line
    if line < start_line || line > end_line {
        return node;
    }

    // Check children for a more specific match
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let child_start = child.start_position().row + 1;
        let child_end = child.end_position().row + 1;

        if line >= child_start && line <= child_end {
            // Recursively check this child
            return find_most_specific_node(child, line);
        }
    }

    // If no child contains the line, this is the most specific node
    node
}

/// Helper function to collect all node types in the AST
pub fn collect_node_types(node: Node, node_types: &mut HashSet<String>) {
    node_types.insert(node.kind().to_string());

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_types(child, node_types);
    }
}
