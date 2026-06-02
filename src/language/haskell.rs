use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Haskell.
pub struct HaskellLanguage;

impl Default for HaskellLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl HaskellLanguage {
    pub fn new() -> Self {
        HaskellLanguage
    }

    fn is_type_like(kind: &str) -> bool {
        matches!(
            kind,
            "data_type"
                | "newtype"
                | "type_synomym"
                | "type_family"
                | "type_instance"
                | "data_family"
                | "data_instance"
                | "kind_signature"
        )
    }

    fn is_function_like(kind: &str) -> bool {
        matches!(
            kind,
            "function" | "bind" | "foreign_import" | "foreign_export" | "pattern_synonym"
        )
    }

    fn named_child_text(node: &Node, source: &[u8]) -> Option<String> {
        let name = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("synonym"))?;
        let text = name.utf8_text(source).ok()?.trim();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }

    fn has_child_kind(node: &Node, kind: &str) -> bool {
        let mut cursor = node.walk();
        let found = node.children(&mut cursor).any(|child| child.kind() == kind);
        found
    }

    fn first_line_signature(node: &Node, source: &[u8]) -> Option<String> {
        let sig = String::from_utf8_lossy(&source[node.start_byte()..node.end_byte()])
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        (!sig.is_empty()).then_some(sig)
    }
}

impl LanguageImpl for HaskellLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_haskell::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "hs"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        if matches!(node.kind(), "function" | "bind") {
            return node.child_by_field_name("name").is_some();
        }

        if matches!(node.kind(), "class" | "instance") {
            return node.named_child_count() > 0;
        }

        matches!(
            node.kind(),
            "data_type"
                | "newtype"
                | "type_synomym"
                | "type_family"
                | "type_instance"
                | "data_family"
                | "data_instance"
                | "kind_signature"
                | "foreign_import"
                | "foreign_export"
                | "pattern_synonym"
        )
    }

    fn is_symbol_node(&self, node: &Node) -> bool {
        self.is_acceptable_parent(node)
            || matches!(node.kind(), "signature" | "default_signature")
            || (node.kind() == "module"
                && node
                    .parent()
                    .is_some_and(|parent| parent.kind() == "header"))
                && Self::has_child_kind(node, "module_id")
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        if let Some(name) = Self::named_child_text(node, source) {
            return name.starts_with("prop_")
                || name.starts_with("test_")
                || name.starts_with("spec_");
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
            "module" => Some(format!("module {}", node.utf8_text(source).ok()?.trim())),
            "class" | "instance" => Self::first_line_signature(node, source),
            "signature" | "default_signature" => Self::first_line_signature(node, source),
            kind if Self::is_type_like(kind) => Self::first_line_signature(node, source),
            kind if Self::is_function_like(kind) => Self::first_line_signature(node, source),
            _ => None,
        }
    }
}
