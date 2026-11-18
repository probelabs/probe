use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// TOML configuration tables and top-level key/value pairs.
pub struct TomlLanguage;

impl LanguageImpl for TomlLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_toml_ng::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "toml"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        match node.kind() {
            "table" | "table_array_element" => true,
            // Keep the table header and sibling settings when matching a value.
            // Root-level pairs have no table and form their own complete block.
            "pair" => node.parent().is_some_and(|p| p.kind() == "document"),
            _ => false,
        }
    }

    fn is_symbol_node(&self, node: &Node) -> bool {
        matches!(node.kind(), "table" | "table_array_element" | "pair")
    }

    fn is_test_node(&self, _node: &Node, _source: &[u8]) -> bool {
        // Test and development settings are part of the configuration itself.
        false
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        if !self.is_symbol_node(node) {
            return None;
        }
        Some(
            node.utf8_text(source)
                .ok()?
                .lines()
                .next()?
                .trim()
                .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    #[test]
    fn parses_tables_arrays_comments_and_multiline_strings() {
        let source =
            "# Setup\n[agent]\nprompt = '''\n[not.a.table]\n'''\n[[tools]]\nname = 'reader'\n";
        let lang = TomlLanguage;
        let mut parser = Parser::new();
        parser
            .set_language(&lang.get_tree_sitter_language())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        assert!(!tree.root_node().has_error());
        let mut cursor = tree.root_node().walk();
        let tables: Vec<_> = tree
            .root_node()
            .named_children(&mut cursor)
            .filter(|n| lang.is_acceptable_parent(n))
            .collect();
        assert_eq!(tables.len(), 2);
        assert_eq!(
            lang.get_symbol_signature(&tables[0], source.as_bytes())
                .as_deref(),
            Some("[agent]")
        );
        assert_eq!(
            lang.get_symbol_signature(&tables[1], source.as_bytes())
                .as_deref(),
            Some("[[tools]]")
        );
        let pair = tables[0].named_child(1).unwrap();
        assert_eq!(pair.kind(), "pair");
        assert!(!lang.is_acceptable_parent(&pair));
        assert!(lang.is_symbol_node(&pair));
    }
}
