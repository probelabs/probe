use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Ruby
pub struct RubyLanguage;

impl Default for RubyLanguage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::RubyLanguage;
    use crate::language::language_trait::LanguageImpl;
    use tree_sitter::{Node, Parser};

    fn parse_ruby(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_ruby::LANGUAGE.into())
            .expect("Ruby parser should initialize");
        parser
            .parse(source, None)
            .expect("Ruby source should parse")
    }

    fn find_node<'a>(
        node: Node<'a>,
        source: &[u8],
        predicate: &dyn Fn(Node<'a>, &[u8]) -> bool,
    ) -> Option<Node<'a>> {
        if predicate(node, source) {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_node(child, source, predicate) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn detects_minitest_style_test_methods() {
        let source = r#"
class UserServiceTest
  def test_authenticates_user
    assert true
  end

  def helper_method
    true
  end
end
"#;
        let tree = parse_ruby(source);
        let lang = RubyLanguage::new();
        let bytes = source.as_bytes();

        let test_method = find_node(tree.root_node(), bytes, &|node, source| {
            node.kind() == "method"
                && node
                    .utf8_text(source)
                    .is_ok_and(|text| text.contains("def test_authenticates_user"))
        })
        .expect("test_ Ruby method should be present");
        assert!(lang.is_test_node(&test_method, bytes));

        let helper_method = find_node(tree.root_node(), bytes, &|node, source| {
            node.kind() == "method"
                && node
                    .utf8_text(source)
                    .is_ok_and(|text| text.contains("def helper_method"))
        })
        .expect("helper Ruby method should be present");
        assert!(!lang.is_test_node(&helper_method, bytes));
    }

    #[test]
    fn detects_rspec_block_calls() {
        let source = r#"
RSpec.describe UserService do
  context "with credentials" do
    it "authenticates" do
      expect(true).to eq(true)
    end
  end
end
"#;
        let tree = parse_ruby(source);
        let lang = RubyLanguage::new();
        let bytes = source.as_bytes();

        for call_name in ["describe", "context", "it"] {
            let call = find_node(tree.root_node(), bytes, &|node, source| {
                node.kind() == "call"
                    && node
                        .utf8_text(source)
                        .is_ok_and(|text| text.contains(call_name))
            })
            .unwrap_or_else(|| panic!("{call_name} call should be present"));
            assert!(
                lang.is_test_node(&call, bytes),
                "{call_name} call should be detected as a Ruby test node"
            );
        }
    }
}

impl RubyLanguage {
    pub fn new() -> Self {
        RubyLanguage
    }

    fn is_container(kind: &str) -> bool {
        matches!(kind, "class" | "module")
    }

    fn is_method_like(kind: &str) -> bool {
        matches!(kind, "method" | "singleton_method")
    }

    fn is_symbol_like(kind: &str) -> bool {
        Self::is_container(kind) || Self::is_method_like(kind)
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

impl LanguageImpl for RubyLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "rb"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        node.is_named() && Self::is_symbol_like(node.kind())
    }

    fn is_symbol_node(&self, node: &Node) -> bool {
        node.is_named() && Self::is_symbol_like(node.kind())
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("PROBE_DEBUG").unwrap_or_default() == "1";
        let node_type = node.kind();

        // Ruby: Check method nodes with test_ prefix or describe/it blocks
        if node_type == "method" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = child.utf8_text(source).unwrap_or("");
                    if name.starts_with("test_") {
                        if debug_mode {
                            println!("DEBUG: Test node detected (Ruby): test_ method");
                        }
                        return true;
                    }
                }
            }
        } else if node_type == "call" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = child.utf8_text(source).unwrap_or("");
                    if name == "describe" || name == "it" || name == "context" || name == "specify"
                    {
                        if debug_mode {
                            println!("DEBUG: Test node detected (Ruby): {name} block");
                        }
                        return true;
                    }
                }
            }
        }

        false
    }

    fn find_parent_function<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut current = node;
        while let Some(parent) = current.parent() {
            if Self::is_method_like(parent.kind()) {
                return Some(parent);
            }
            current = parent;
        }
        None
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            kind if Self::is_container(kind) || Self::is_method_like(kind) => {
                Self::first_line_signature(node, source)
            }
            _ => None,
        }
    }
}
