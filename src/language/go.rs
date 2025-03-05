use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Go
pub struct GoLanguage;

impl Default for GoLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl GoLanguage {
    pub fn new() -> Self {
        GoLanguage
    }
}

impl LanguageImpl for GoLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_go::language()
    }

    fn get_extension(&self) -> &'static str {
        "go"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "function_declaration" |
            "method_declaration" |
            "type_declaration" |
            "struct_type" |
            "interface_type" |
            // "const_declaration" |
            // "var_declaration" |
            // "const_spec" |
            // "var_spec" |
            // "short_var_declaration" |
            "type_spec" // Added for type definitions
        )
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
        let node_type = node.kind();

        // Go: Check function_declaration nodes with names starting with Test
        if node_type == "function_declaration" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = child.utf8_text(source).unwrap_or("");
                    if name.starts_with("Test") {
                        if debug_mode {
                            println!("DEBUG: Test node detected (Go): Test function");
                        }
                        return true;
                    }
                }
            }
        }

        false
    }

    fn find_topmost_struct_type<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        if debug_mode {
            println!("DEBUG: Finding topmost struct_type for {}", node.kind());
        }

        // First check if we're inside a function
        let mut current = node;
        let mut function_node = None;

        while let Some(parent) = current.parent() {
            if parent.kind() == "function_declaration" || parent.kind() == "method_declaration" {
                if debug_mode {
                    println!("DEBUG: Found parent {} for struct_type", parent.kind());
                }
                function_node = Some(parent);
                break;
            }
            current = parent;
        }

        // If we found a function, return it as the container
        if function_node.is_some() {
            if debug_mode {
                println!("DEBUG: Returning function node as container");
            }
            return function_node;
        }

        // Otherwise look for the struct_type
        let mut current = node;
        let mut struct_type = None;

        if node.kind() == "struct_type" {
            struct_type = Some(node);
        }

        while let Some(parent) = current.parent() {
            if parent.kind() == "struct_type" {
                struct_type = Some(parent);
            }
            current = parent;
        }

        if debug_mode {
            if struct_type.is_some() {
                println!("DEBUG: Returning struct_type as container");
            } else {
                println!("DEBUG: No struct_type found, returning original node");
            }
        }

        struct_type.or(Some(node))
    }

    fn find_parent_function<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        if debug_mode {
            println!("DEBUG: Finding parent function for {}", node.kind());
        }

        let mut current = node;

        while let Some(parent) = current.parent() {
            if parent.kind() == "function_declaration" || parent.kind() == "method_declaration" {
                if debug_mode {
                    println!("DEBUG: Found parent function: {}", parent.kind());
                }
                return Some(parent);
            }
            current = parent;
        }

        if debug_mode {
            println!("DEBUG: No parent function found for {}", node.kind());
        }

        None
    }
}
