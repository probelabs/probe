use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Crystal.
pub struct CrystalLanguage;

impl Default for CrystalLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl CrystalLanguage {
    pub fn new() -> Self {
        CrystalLanguage
    }

    fn is_container(kind: &str) -> bool {
        matches!(
            kind,
            "class_def" | "module_def" | "struct_def" | "enum_def" | "lib_def" | "union_def"
        )
    }

    fn is_function_like(kind: &str) -> bool {
        matches!(
            kind,
            "method_def" | "abstract_method_def" | "macro_def" | "fun_def"
        )
    }

    fn body_signature(node: &Node, source: &[u8], container: bool) -> Option<String> {
        let end = node
            .child_by_field_name("body")
            .map(|body| body.start_byte())
            .unwrap_or_else(|| node.end_byte());
        let sig = String::from_utf8_lossy(&source[node.start_byte()..end])
            .trim()
            .to_string();

        if sig.is_empty() {
            None
        } else if container {
            Some(format!("{sig} ... end"))
        } else {
            Some(sig)
        }
    }

    fn named_child_text(node: &Node, source: &[u8]) -> Option<String> {
        let name = node.child_by_field_name("name")?;
        let text = name.utf8_text(source).ok()?.trim();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }
}

impl LanguageImpl for CrystalLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_crystal::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "cr"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "class_def"
                | "module_def"
                | "struct_def"
                | "enum_def"
                | "method_def"
                | "abstract_method_def"
                | "macro_def"
                | "lib_def"
                | "fun_def"
                | "alias"
                | "annotation_def"
                | "type_def"
                | "union_def"
        )
    }

    fn is_symbol_node(&self, node: &Node) -> bool {
        self.is_acceptable_parent(node)
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        match node.kind() {
            "method_def" | "abstract_method_def" | "macro_def" => {
                if let Some(name) = Self::named_child_text(node, source) {
                    return name.starts_with("test_");
                }
            }
            "call" | "implicit_object_call" | "assign_call" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if matches!(child.kind(), "identifier" | "constant") {
                        let name = child.utf8_text(source).unwrap_or("");
                        if matches!(name, "describe" | "context" | "it" | "pending") {
                            return true;
                        }
                    }
                }
            }
            _ => {}
        }

        false
    }

    fn find_parent_function<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut current = node;
        while let Some(parent) = current.parent() {
            if Self::is_function_like(parent.kind()) {
                return Some(parent);
            }
            current = parent;
        }
        None
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            kind if Self::is_container(kind) => Self::body_signature(node, source, true),
            kind if Self::is_function_like(kind) => Self::body_signature(node, source, false),
            "alias" | "annotation_def" | "type_def" => Some(
                String::from_utf8_lossy(&source[node.start_byte()..node.end_byte()])
                    .trim()
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            )
            .filter(|sig| !sig.is_empty()),
            _ => None,
        }
    }
}
