use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Rust
pub struct RustLanguage;

impl RustLanguage {
    pub fn new() -> Self {
        RustLanguage
    }
}

impl LanguageImpl for RustLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_rust::language()
    }

    fn get_extension(&self) -> &'static str {
        "rs"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "function_item"
                | "struct_item"
                | "impl_item"
                | "trait_item"
                | "enum_item"
                | "mod_item"
                | "macro_definition"
        )
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
        let node_type = node.kind();

        // Rust: Check for #[test] attribute on function_item nodes
        if node_type == "function_item" {
            let mut cursor = node.walk();
            let mut has_test_attribute = false;

            // Look for attribute nodes
            for child in node.children(&mut cursor) {
                if child.kind() == "attribute_item" {
                    let attr_text = child.utf8_text(source).unwrap_or("");
                    if attr_text.contains("#[test") {
                        has_test_attribute = true;
                        break;
                    }
                }
            }

            if has_test_attribute {
                if debug_mode {
                    println!("DEBUG: Test node detected (Rust): #[test] attribute");
                }
                return true;
            }

            // Also check function name starting with "test_"
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = child.utf8_text(source).unwrap_or("");
                    if name.starts_with("test_") {
                        if debug_mode {
                            println!("DEBUG: Test node detected (Rust): test_ function");
                        }
                        return true;
                    }
                }
            }
        }

        false
    }
}
