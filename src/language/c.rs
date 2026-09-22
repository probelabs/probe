use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for C
pub struct CLanguage;

impl Default for CLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl CLanguage {
    pub fn new() -> Self {
        CLanguage
    }
}

fn signature_before_body(node: &Node, source: &[u8]) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    let signature = &source[node.start_byte()..body.start_byte()];
    Some(
        String::from_utf8_lossy(signature)
            .trim()
            .trim_end_matches('{')
            .trim()
            .to_string(),
    )
}

impl LanguageImpl for CLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_c::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "c"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "function_definition" | "declaration" | "struct_specifier" | "enum_specifier"
        )
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("PROBE_DEBUG").unwrap_or_default() == "1";
        let node_type = node.kind();

        // C: Check function_definition nodes with test in the name
        if node_type == "function_definition" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "function_declarator" {
                    let mut subcursor = child.walk();
                    for subchild in child.children(&mut subcursor) {
                        if subchild.kind() == "identifier" {
                            let name = subchild.utf8_text(source).unwrap_or("");
                            if name.contains("test") || name.contains("Test") {
                                if debug_mode {
                                    println!("DEBUG: Test node detected (C): test function");
                                }
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "function_definition" => signature_before_body(node, source),
            _ => None,
        }
    }
}
