use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Solidity.
pub struct SolidityLanguage;

impl Default for SolidityLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl SolidityLanguage {
    pub fn new() -> Self {
        SolidityLanguage
    }

    fn is_contract_like(kind: &str) -> bool {
        matches!(
            kind,
            "contract_declaration" | "interface_declaration" | "library_declaration"
        )
    }

    fn body_signature(node: &Node, source: &[u8], container: bool) -> Option<String> {
        let end = node
            .child_by_field_name("body")
            .map(|body| body.start_byte())
            .unwrap_or_else(|| node.end_byte());
        let sig = String::from_utf8_lossy(&source[node.start_byte()..end])
            .trim()
            .trim_end_matches('{')
            .trim()
            .to_string();

        if sig.is_empty() {
            None
        } else if container {
            Some(format!("{sig} {{ ... }}"))
        } else {
            Some(sig)
        }
    }
}

impl LanguageImpl for SolidityLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_solidity::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "sol"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "contract_declaration"
                | "interface_declaration"
                | "library_declaration"
                | "function_definition"
                | "constructor_definition"
                | "modifier_definition"
                | "fallback_receive_definition"
                | "struct_declaration"
                | "enum_declaration"
                | "event_definition"
                | "error_declaration"
                | "state_variable_declaration"
                | "user_defined_type_definition"
        )
    }

    fn is_symbol_node(&self, node: &Node) -> bool {
        self.is_acceptable_parent(node)
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        match node.kind() {
            "contract_declaration" => {
                if let Some(name) = node.child_by_field_name("name") {
                    let name = name.utf8_text(source).unwrap_or("");
                    return name.ends_with("Test") || name.ends_with("Tests");
                }
            }
            "function_definition" => {
                if let Some(name) = node.child_by_field_name("name") {
                    let name = name.utf8_text(source).unwrap_or("");
                    return name == "setUp"
                        || name.starts_with("test")
                        || name.starts_with("invariant_");
                }
            }
            _ => {}
        }

        false
    }

    fn find_parent_function<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut current = node;
        while let Some(parent) = current.parent() {
            if matches!(
                parent.kind(),
                "function_definition" | "constructor_definition" | "modifier_definition"
            ) {
                return Some(parent);
            }
            current = parent;
        }
        None
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            kind if Self::is_contract_like(kind) => Self::body_signature(node, source, true),
            "function_definition"
            | "constructor_definition"
            | "modifier_definition"
            | "fallback_receive_definition" => Self::body_signature(node, source, false),
            "struct_declaration" | "enum_declaration" => Self::body_signature(node, source, true),
            "event_definition"
            | "error_declaration"
            | "state_variable_declaration"
            | "user_defined_type_definition" => Some(
                String::from_utf8_lossy(&source[node.start_byte()..node.end_byte()])
                    .trim()
                    .to_string(),
            ),
            _ => None,
        }
    }
}
