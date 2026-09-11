use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Bash (shell scripts: .sh / .bash)
pub struct BashLanguage;

impl Default for BashLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl BashLanguage {
    pub fn new() -> Self {
        BashLanguage
    }

    /// Helper method to get the text content of a node
    fn get_node_text(&self, node: &Node, source: &[u8]) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        String::from_utf8_lossy(&source[start..end]).to_string()
    }

    /// Truncate a single-line signature to a reasonable length
    fn truncate_signature(&self, text: &str, max_len: usize) -> String {
        let trimmed = text.trim();
        if trimmed.len() > max_len {
            format!("{}...", &trimmed[..max_len - 3])
        } else {
            trimmed.to_string()
        }
    }
}

impl LanguageImpl for BashLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_bash::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "sh"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "function_definition"    // foo() { ... } / function foo { ... }
                | "if_statement"     // if ...; then ...; fi (incl. elif/else clauses)
                | "case_statement"   // case ... in ... esac
                | "for_statement"    // for x in ...; do ...; done
                | "c_style_for_statement" // for ((i=0; i<n; i++)); do ...; done
                | "while_statement"  // while ...; do ...; done / until ...
                | "subshell"         // ( ... ) subshell blocks
                | "compound_statement" // { ... } brace groups
                | "variable_assignment" // FOO=bar top-level assignments
                | "declaration_command" // local/readonly/export/declare//typeset
                | "heredoc_body"     // <<EOF ... EOF bodies
                | "comment" // # comments
        )
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("PROBE_DEBUG").unwrap_or_default() == "1";
        let node_type = node.kind();

        // Bash: check function_definition nodes with test in the name
        // (covers `test_foo()`, `foo_test()`, `runTests()` conventions)
        if node_type == "function_definition" {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source).unwrap_or("");
                if name.contains("test") || name.contains("Test") {
                    if debug_mode {
                        println!("DEBUG: Test node detected (Bash): test function '{name}'");
                    }
                    return true;
                }
            }
        }

        // Bats-style tests: `@test "description" { ... }` parses as a command
        // whose name word starts with `@test`
        if node_type == "command" {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = self.get_node_text(&name_node, source);
                if name.trim().starts_with("@test") {
                    if debug_mode {
                        println!("DEBUG: Test node detected (Bash): bats @test block");
                    }
                    return true;
                }
            }
        }

        false
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "function_definition" => {
                // Extract function name and present a clean signature
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = name_node.utf8_text(source).unwrap_or("");
                    Some(format!("{name}() {{ ... }}"))
                } else {
                    None
                }
            }
            "variable_assignment" => {
                // FOO=bar (possibly with array or command substitution value)
                let text = self.get_node_text(node, source);
                Some(self.truncate_signature(text.lines().next().unwrap_or(""), 80))
            }
            "declaration_command" => {
                // local/readonly/export/declare statements
                let text = self.get_node_text(node, source);
                Some(self.truncate_signature(text.lines().next().unwrap_or(""), 80))
            }
            "if_statement" => {
                // Show the condition line
                let text = self.get_node_text(node, source);
                let first_line = text.lines().next().unwrap_or("");
                Some(format!("{} ... fi", self.truncate_signature(first_line, 60)))
            }
            "case_statement" => {
                let text = self.get_node_text(node, source);
                let first_line = text.lines().next().unwrap_or("");
                Some(format!("{} ... esac", self.truncate_signature(first_line, 60)))
            }
            "for_statement" | "c_style_for_statement" => {
                let text = self.get_node_text(node, source);
                let first_line = text.lines().next().unwrap_or("");
                Some(format!("{} ... done", self.truncate_signature(first_line, 60)))
            }
            "while_statement" => {
                let text = self.get_node_text(node, source);
                let first_line = text.lines().next().unwrap_or("");
                Some(format!("{} ... done", self.truncate_signature(first_line, 60)))
            }
            "heredoc_body" => {
                // Show a preview of the heredoc content
                let text = self.get_node_text(node, source);
                let first_line = text.lines().next().unwrap_or("").trim();
                if first_line.is_empty() {
                    Some("<<... (heredoc)".to_string())
                } else {
                    Some(format!("<<... {}", self.truncate_signature(first_line, 57)))
                }
            }
            "comment" => {
                let comment_text = self.get_node_text(node, source);
                Some(self.truncate_signature(&comment_text, 60))
            }
            "subshell" => Some("( ... )".to_string()),
            "compound_statement" => Some("{ ... }".to_string()),
            _ => None,
        }
    }

    fn find_parent_function<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut current = node;

        while let Some(parent) = current.parent() {
            if parent.kind() == "function_definition" {
                return Some(parent);
            }
            current = parent;
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_bash(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_bash::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn get_first_node_of_kind<'a>(
        node: tree_sitter::Node<'a>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == kind {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = get_first_node_of_kind(child, kind) {
                return Some(found);
            }
        }
        None
    }

    fn get_all_nodes_of_kind<'a>(
        node: tree_sitter::Node<'a>,
        kind: &str,
        out: &mut Vec<tree_sitter::Node<'a>>,
    ) {
        if node.kind() == kind {
            out.push(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            get_all_nodes_of_kind(child, kind, out);
        }
    }

    #[test]
    fn test_bash_language_creation() {
        let _lang = BashLanguage::new();
        // Basic test to ensure BashLanguage can be created
    }

    #[test]
    fn test_bash_get_extension() {
        let lang = BashLanguage::new();
        #[allow(deprecated)]
        let ext = lang.get_extension();
        assert_eq!(ext, "sh");
    }

    #[test]
    fn test_acceptable_parents() {
        let lang = BashLanguage::new();
        let source = r#"#!/usr/bin/env bash
# A comment
FOO=bar

greet() {
    echo "hello"
}

if [ -n "$FOO" ]; then
    echo "set"
fi

for i in 1 2 3; do
    echo "$i"
done

while read -r line; do
    echo "$line"
done < input.txt

case "$FOO" in
    bar) echo "bar" ;;
esac

cat <<EOF
heredoc content
EOF
"#;

        let tree = parse_bash(source);
        let root = tree.root_node();

        let kinds = [
            "function_definition",
            "if_statement",
            "for_statement",
            "while_statement",
            "case_statement",
            "variable_assignment",
            "comment",
            "heredoc_body",
        ];
        for kind in kinds {
            let node = get_first_node_of_kind(root, kind);
            assert!(node.is_some(), "expected to find node kind {kind}");
            assert!(
                lang.is_acceptable_parent(&node.unwrap()),
                "{kind} should be an acceptable parent"
            );
        }

        // Non-acceptable: plain command, words
        if let Some(word) = get_first_node_of_kind(root, "word") {
            assert!(!lang.is_acceptable_parent(&word));
        }
    }

    #[test]
    fn test_function_signature_extraction() {
        let lang = BashLanguage::new();
        let source = r#"build_release() {
    cargo build --release
}

function deploy {
    ./deploy.sh
}
"#;
        let tree = parse_bash(source);
        let root = tree.root_node();

        let mut funcs = Vec::new();
        get_all_nodes_of_kind(root, "function_definition", &mut funcs);
        assert_eq!(funcs.len(), 2);

        let sig = lang
            .get_symbol_signature(&funcs[0], source.as_bytes())
            .unwrap();
        assert_eq!(sig, "build_release() { ... }");

        let sig2 = lang
            .get_symbol_signature(&funcs[1], source.as_bytes())
            .unwrap();
        assert_eq!(sig2, "deploy() { ... }");
    }

    #[test]
    fn test_variable_assignment_signature() {
        let lang = BashLanguage::new();
        let source = "APP_NAME=\"reqforge\"\nreadonly LOG_DIR=/var/log/app\n";
        let tree = parse_bash(source);
        let root = tree.root_node();

        let assign = get_first_node_of_kind(root, "variable_assignment").unwrap();
        let sig = lang
            .get_symbol_signature(&assign, source.as_bytes())
            .unwrap();
        assert_eq!(sig, "APP_NAME=\"reqforge\"");

        let decl = get_first_node_of_kind(root, "declaration_command").unwrap();
        assert!(lang.is_acceptable_parent(&decl));
        let sig2 = lang
            .get_symbol_signature(&decl, source.as_bytes())
            .unwrap();
        assert!(sig2.contains("readonly"));
        assert!(sig2.contains("LOG_DIR"));
    }

    #[test]
    fn test_control_block_signatures() {
        let lang = BashLanguage::new();
        let source = r#"if [ -f /tmp/x ]; then
    cat /tmp/x
fi
"#;
        let tree = parse_bash(source);
        let root = tree.root_node();

        let if_node = get_first_node_of_kind(root, "if_statement").unwrap();
        let sig = lang
            .get_symbol_signature(&if_node, source.as_bytes())
            .unwrap();
        assert!(sig.starts_with("if [ -f /tmp/x ]"));
        assert!(sig.ends_with("fi"));
    }

    #[test]
    fn test_heredoc_acceptable() {
        let lang = BashLanguage::new();
        let source = "cat <<EOF\nline one\nline two\nEOF\n";
        let tree = parse_bash(source);
        let root = tree.root_node();

        let heredoc = get_first_node_of_kind(root, "heredoc_body").unwrap();
        assert!(lang.is_acceptable_parent(&heredoc));
        let sig = lang
            .get_symbol_signature(&heredoc, source.as_bytes())
            .unwrap();
        assert!(sig.contains("line one"));
    }

    #[test]
    fn test_test_node_detection() {
        let lang = BashLanguage::new();

        // test_* function naming convention
        let test_source = "test_build() {\n    assert_true\n}\n\nbuild() {\n    make\n}\n";
        let tree = parse_bash(test_source);
        let root = tree.root_node();

        let mut funcs = Vec::new();
        get_all_nodes_of_kind(root, "function_definition", &mut funcs);
        assert_eq!(funcs.len(), 2);

        assert!(lang.is_test_node(&funcs[0], test_source.as_bytes()));
        assert!(!lang.is_test_node(&funcs[1], test_source.as_bytes()));

        // bats-style @test detection
        let bats_source = "@test \"addition works\" {\n    run add 1 2\n}\n";
        let bats_tree = parse_bash(bats_source);
        let bats_root = bats_tree.root_node();

        let mut commands = Vec::new();
        get_all_nodes_of_kind(bats_root, "command", &mut commands);
        let bats_test = commands
            .iter()
            .find(|c| {
                c.child_by_field_name("name")
                    .map(|n| n.utf8_text(bats_source.as_bytes()).unwrap_or(""))
                    .unwrap_or("")
                    .starts_with("@test")
            })
            .copied();
        if let Some(cmd) = bats_test {
            assert!(lang.is_test_node(&cmd, bats_source.as_bytes()));
        }

        // Non-test nodes should not be detected
        let plain_source = "echo hello\n";
        let plain_tree = parse_bash(plain_source);
        let plain_root = plain_tree.root_node();
        if let Some(cmd) = get_first_node_of_kind(plain_root, "command") {
            assert!(!lang.is_test_node(&cmd, plain_source.as_bytes()));
        }
    }

    #[test]
    fn test_comment_signature() {
        let lang = BashLanguage::new();
        let source = "# Deployment configuration\necho hi\n";
        let tree = parse_bash(source);
        let root = tree.root_node();

        let comment = get_first_node_of_kind(root, "comment").unwrap();
        assert!(lang.is_acceptable_parent(&comment));
        let sig = lang
            .get_symbol_signature(&comment, source.as_bytes())
            .unwrap();
        assert!(sig.contains("Deployment configuration"));
    }

    #[test]
    fn test_find_parent_function() {
        let lang = BashLanguage::new();
        let source = "outer() {\n    echo \"inside\"\n}\n\necho \"top level\"\n";
        let tree = parse_bash(source);
        let root = tree.root_node();

        // A command inside a function should resolve to that function
        let mut commands = Vec::new();
        get_all_nodes_of_kind(root, "command", &mut commands);
        assert!(commands.len() >= 2);

        let inside = commands
            .iter()
            .find(|c| {
                lang.get_node_text(c, source.as_bytes()).contains("inside")
            })
            .copied()
            .unwrap();
        let parent_fn = lang.find_parent_function(inside);
        assert!(parent_fn.is_some());
        assert_eq!(parent_fn.unwrap().kind(), "function_definition");

        let top = commands
            .iter()
            .find(|c| {
                lang.get_node_text(c, source.as_bytes())
                    .contains("top level")
            })
            .copied()
            .unwrap();
        assert!(lang.find_parent_function(top).is_none());
    }

    #[test]
    fn test_is_symbol_node_defaults() {
        let lang = BashLanguage::new();
        let source = "myfunc() {\n    true\n}\n";
        let tree = parse_bash(source);
        let root = tree.root_node();

        let func = get_first_node_of_kind(root, "function_definition").unwrap();
        assert!(lang.is_symbol_node(&func));
    }
}
