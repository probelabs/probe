use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for YAML
pub struct YamlLanguage;

impl Default for YamlLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl YamlLanguage {
    pub fn new() -> Self {
        YamlLanguage
    }

    /// Helper method to get the text content of a node
    fn get_node_text(&self, node: &Node, source: &[u8]) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        String::from_utf8_lossy(&source[start..end]).to_string()
    }

    /// Check if a node is a key-value pair
    fn is_key_value_pair(&self, node: &Node) -> bool {
        node.kind() == "block_mapping_pair"
    }


    /// Extract key from a key-value pair
    fn extract_key_text(&self, node: &Node, source: &[u8]) -> Option<String> {
        if self.is_key_value_pair(node) {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                // Look for the first flow_node that contains the key
                if child.kind() == "flow_node" {
                    let key_text = self.get_node_text(&child, source);
                    if !key_text.trim().is_empty() && !key_text.trim().starts_with(':') {
                        return Some(key_text.trim().to_string());
                    }
                }
            }
        }
        None
    }

    /// Check if this is a comment node
    fn is_comment(&self, node: &Node) -> bool {
        node.kind() == "comment"
    }

    /// Check if the key or value contains test-related keywords
    fn contains_test_keywords(&self, text: &str) -> bool {
        let lower_text = text.to_lowercase();
        lower_text.contains("test")
            || lower_text.contains("testing")
            || lower_text.contains("spec")
            || lower_text.contains("spec_helper")
            || lower_text.contains("example")
            || lower_text.contains("demo")
            || lower_text.contains("jest")
            || lower_text.contains("mocha")
            || lower_text.contains("pytest")
            || lower_text.contains("rspec")
            || lower_text.contains("cucumber")
            || lower_text.contains("cypress")
    }

    /// Extract the first meaningful value from a sequence
    fn extract_sequence_preview(&self, node: &Node, source: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block_sequence_item" || child.kind() == "flow_sequence_item" {
                let item_text = self.get_node_text(&child, source);
                let cleaned = item_text.trim().trim_start_matches('-').trim();
                if !cleaned.is_empty() && cleaned.len() <= 50 {
                    return Some(cleaned.to_string());
                } else if cleaned.len() > 50 {
                    return Some(format!("{}...", &cleaned[..47]));
                }
            }
        }
        None
    }
}

impl LanguageImpl for YamlLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_yaml::language().into()
    }

    fn get_extension(&self) -> &'static str {
        "yaml"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        // Accept YAML structural elements (based on actual tree-sitter-yaml node types)
        let acceptable = matches!(
            node.kind(),
            "stream"                 // Root stream
                | "document"             // YAML document
                | "block_node"           // Block-style node
                | "flow_node"            // Flow-style node
                | "block_mapping"        // Key-value mappings
                | "block_mapping_pair"   // Individual key-value pairs
                | "flow_mapping"         // Inline mappings
                | "flow_mapping_pair"    // Inline key-value pairs
                | "block_sequence"       // Lists/arrays
                | "block_sequence_item"  // List items
                | "flow_sequence"        // Inline arrays
                | "flow_sequence_item"   // Inline array items
                | "plain_scalar"         // Unquoted values
                | "single_quoted_scalar" // 'quoted values'
                | "double_quoted_scalar" // "quoted values"
                | "literal_scalar"       // | literal blocks
                | "folded_scalar"        // > folded blocks
                | "string_scalar"        // String content
                | "anchor"               // &anchors
                | "alias"                // *aliases
                | "tag"                  // !tags
                | "comment"              // # comments
                | "directive"            // %directives
        );

        if debug_mode && acceptable {
            println!(
                "DEBUG: YAML acceptable parent: {} at lines {}-{}",
                node.kind(),
                node.start_position().row + 1,
                node.end_position().row + 1
            );
        }

        acceptable
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        // Check key-value pairs for test-related keys
        if self.is_key_value_pair(node) {
            if let Some(key_text) = self.extract_key_text(node, source) {
                if self.contains_test_keywords(&key_text) {
                    if debug_mode {
                        println!("DEBUG: Test node detected (YAML): test-related key");
                    }
                    return true;
                }
            }

            // Also check the full text of the pair for test-related values
            let full_text = self.get_node_text(node, source);
            if self.contains_test_keywords(&full_text) {
                if debug_mode {
                    println!("DEBUG: Test node detected (YAML): test-related value");
                }
                return true;
            }
        }

        // Check comments for test-related content
        if self.is_comment(node) {
            let comment_text = self.get_node_text(node, source);
            if self.contains_test_keywords(&comment_text) {
                if debug_mode {
                    println!("DEBUG: Test node detected (YAML): test-related comment");
                }
                return true;
            }
        }

        // Check scalar values that might contain test file paths or names
        if matches!(node.kind(), "plain_scalar" | "single_quoted_scalar" | "double_quoted_scalar") {
            let scalar_text = self.get_node_text(node, source);
            if self.contains_test_keywords(&scalar_text) {
                if debug_mode {
                    println!("DEBUG: Test node detected (YAML): test-related scalar");
                }
                return true;
            }
        }

        false
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "block_mapping_pair" | "flow_mapping_pair" => {
                // Extract key-value pair signature
                if let Some(key_text) = self.extract_key_text(node, source) {
                    // Try to get a preview of the value
                    let full_text = self.get_node_text(node, source);
                    let lines: Vec<&str> = full_text.lines().collect();

                    if lines.len() == 1 {
                        // Single line key-value pair
                        if full_text.len() > 80 {
                            Some(format!("{}...", &full_text[..77].trim()))
                        } else {
                            Some(full_text.trim().to_string())
                        }
                    } else {
                        // Multi-line, just show the key with indication of complex value
                        if lines.len() > 1 && lines[1].trim().starts_with('-') {
                            Some(format!("{}: [...]", key_text))
                        } else {
                            Some(format!("{}: {{...}}", key_text))
                        }
                    }
                } else {
                    None
                }
            }
            "block_sequence" | "flow_sequence" => {
                // Extract sequence signature
                if let Some(preview) = self.extract_sequence_preview(node, source) {
                    Some(format!("- {}", preview))
                } else {
                    Some("- [...]".to_string())
                }
            }
            "block_sequence_item" | "flow_sequence_item" => {
                // Extract individual sequence item
                let item_text = self.get_node_text(node, source);
                let cleaned = item_text.trim().trim_start_matches('-').trim();
                if cleaned.len() > 60 {
                    Some(format!("- {}...", &cleaned[..57]))
                } else if !cleaned.is_empty() {
                    Some(format!("- {}", cleaned))
                } else {
                    Some("- ...".to_string())
                }
            }
            "literal_scalar" => {
                // Literal block scalar (|)
                let text = self.get_node_text(node, source);
                let first_line = text
                    .lines()
                    .nth(1) // Skip the | line
                    .unwrap_or("")
                    .trim();
                if first_line.len() > 50 {
                    Some(format!("| {}...", &first_line[..47]))
                } else if !first_line.is_empty() {
                    Some(format!("| {}", first_line))
                } else {
                    Some("| ...".to_string())
                }
            }
            "folded_scalar" => {
                // Folded block scalar (>)
                let text = self.get_node_text(node, source);
                let first_line = text
                    .lines()
                    .nth(1) // Skip the > line
                    .unwrap_or("")
                    .trim();
                if first_line.len() > 50 {
                    Some(format!("> {}...", &first_line[..47]))
                } else if !first_line.is_empty() {
                    Some(format!("> {}", first_line))
                } else {
                    Some("> ...".to_string())
                }
            }
            "comment" => {
                // Comment
                let comment_text = self.get_node_text(node, source);
                if comment_text.len() > 60 {
                    Some(format!("{}...", &comment_text[..57].trim()))
                } else {
                    Some(comment_text.trim().to_string())
                }
            }
            "directive" => {
                // YAML directive like %YAML 1.2
                let directive_text = self.get_node_text(node, source);
                Some(directive_text.trim().to_string())
            }
            "anchor" => {
                // Anchor definition &anchor
                let anchor_text = self.get_node_text(node, source);
                Some(anchor_text.trim().to_string())
            }
            "alias" => {
                // Alias reference *alias
                let alias_text = self.get_node_text(node, source);
                Some(alias_text.trim().to_string())
            }
            "tag" => {
                // Tag !tag
                let tag_text = self.get_node_text(node, source);
                Some(tag_text.trim().to_string())
            }
            _ => None,
        }
    }

    fn find_parent_function<'a>(&self, _node: Node<'a>) -> Option<Node<'a>> {
        // YAML doesn't have functions in the traditional sense
        // We could return the parent mapping or document, but that's handled elsewhere
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_yaml(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_yaml::language().into())
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

    #[test]
    fn test_yaml_language_creation() {
        let _lang = YamlLanguage::new();
        // Basic test to ensure YamlLanguage can be created
    }


    #[test]
    fn test_acceptable_parents() {
        let lang = YamlLanguage::new();
        let source = r#"---
name: Test Config
version: 1.0
features:
  - authentication
  - logging
  - monitoring
database:
  host: localhost
  port: 5432
# This is a comment
"#;

        let tree = parse_yaml(source);
        let root = tree.root_node();

        // Find and test different node types
        if let Some(mapping) = get_first_node_of_kind(root, "block_mapping") {
            assert!(lang.is_acceptable_parent(&mapping));
        }

        if let Some(pair) = get_first_node_of_kind(root, "block_mapping_pair") {
            assert!(lang.is_acceptable_parent(&pair));
        }

        if let Some(sequence) = get_first_node_of_kind(root, "block_sequence") {
            assert!(lang.is_acceptable_parent(&sequence));
        }

        if let Some(comment) = get_first_node_of_kind(root, "comment") {
            assert!(lang.is_acceptable_parent(&comment));
        }
    }

    #[test]
    fn test_key_value_signature_extraction() {
        let lang = YamlLanguage::new();
        let source = r#"name: MyProject
version: 1.0.0
description: A test project"#;
        let tree = parse_yaml(source);
        let root = tree.root_node();

        // Use the helper function to find block_mapping_pair nodes
        let mut pair_count = 0;
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if let Some(pair) = get_first_node_of_kind(child, "block_mapping_pair") {
                if let Some(sig) = lang.get_symbol_signature(&pair, source.as_bytes()) {
                    assert!(sig.contains(":"));
                    pair_count += 1;
                }
            }
        }
        assert!(pair_count > 0);
    }

    #[test]
    fn test_sequence_signature_extraction() {
        let lang = YamlLanguage::new();
        let source = r#"features:
  - authentication
  - logging
  - monitoring"#;
        let tree = parse_yaml(source);
        let root = tree.root_node();

        if let Some(sequence) = get_first_node_of_kind(root, "block_sequence") {
            let sig = lang.get_symbol_signature(&sequence, source.as_bytes());
            assert!(sig.is_some());
            if let Some(signature) = sig {
                assert!(signature.starts_with("-"));
            }
        }
    }

    #[test]
    fn test_test_node_detection() {
        let lang = YamlLanguage::new();

        // Test key-value pair detection
        let test_source = r#"test_database:
  host: localhost
  port: 5432
jest_config:
  verbose: true"#;
        let tree = parse_yaml(test_source);
        let root = tree.root_node();

        // Use helper function to find all block_mapping_pair nodes
        let mut found_test_node = false;
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if let Some(pair) = get_first_node_of_kind(inner_child, "block_mapping_pair") {
                    if lang.is_test_node(&pair, test_source.as_bytes()) {
                        found_test_node = true;
                        break;
                    }
                }
            }
            if found_test_node {
                break;
            }
        }
        assert!(found_test_node);

        // Test comment detection
        let comment_source = "# This is a test configuration file\nname: app";
        let comment_tree = parse_yaml(comment_source);
        let comment_root = comment_tree.root_node();

        if let Some(comment) = get_first_node_of_kind(comment_root, "comment") {
            assert!(lang.is_test_node(&comment, comment_source.as_bytes()));
        }
    }

    #[test]
    fn test_literal_scalar_signature() {
        let lang = YamlLanguage::new();
        let source = r#"script: |
  echo "Hello World"
  echo "This is a test""#;
        let tree = parse_yaml(source);
        let root = tree.root_node();

        if let Some(literal) = get_first_node_of_kind(root, "literal_scalar") {
            if let Some(sig) = lang.get_symbol_signature(&literal, source.as_bytes()) {
                assert!(sig.starts_with("|"));
                assert!(sig.contains("Hello World"));
            }
        }
    }

    #[test]
    fn test_folded_scalar_signature() {
        let lang = YamlLanguage::new();
        let source = r#"description: >
  This is a long description
  that spans multiple lines"#;
        let tree = parse_yaml(source);
        let root = tree.root_node();

        if let Some(folded) = get_first_node_of_kind(root, "folded_scalar") {
            if let Some(sig) = lang.get_symbol_signature(&folded, source.as_bytes()) {
                assert!(sig.starts_with(">"));
                assert!(sig.contains("This is a long"));
            }
        }
    }

    #[test]
    fn test_comment_signature() {
        let lang = YamlLanguage::new();
        let source = "# This is a configuration comment\nname: test";
        let tree = parse_yaml(source);
        let root = tree.root_node();

        if let Some(comment) = get_first_node_of_kind(root, "comment") {
            if let Some(sig) = lang.get_symbol_signature(&comment, source.as_bytes()) {
                assert!(sig.contains("configuration comment"));
            }
        }
    }

    #[test]
    fn test_complex_mapping_signature() {
        let lang = YamlLanguage::new();
        let source = r#"database:
  connections:
    - host: db1
      port: 5432
    - host: db2
      port: 5433"#;
        let tree = parse_yaml(source);
        let root = tree.root_node();

        if let Some(pair) = get_first_node_of_kind(root, "block_mapping_pair") {
            if let Some(sig) = lang.get_symbol_signature(&pair, source.as_bytes()) {
                // Should show the key with indication of complex structure
                assert!(sig.contains("database"));
                assert!(sig.contains("...") || sig.contains("[...]") || sig.contains("{...}"));
            }
        }
    }
}