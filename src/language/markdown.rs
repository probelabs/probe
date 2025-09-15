use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Markdown
pub struct MarkdownLanguage;

impl Default for MarkdownLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownLanguage {
    pub fn new() -> Self {
        MarkdownLanguage
    }

    /// Helper method to get the text content of a node
    fn get_node_text(&self, node: &Node, source: &[u8]) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        String::from_utf8_lossy(&source[start..end]).to_string()
    }

    /// Check if a header is at a specific level
    #[allow(dead_code)]
    fn is_header_level(&self, node: &Node, level: usize) -> bool {
        if node.kind() != "atx_heading" {
            return false;
        }

        // Count the number of # characters at the beginning
        if let Some(marker) = node.child_by_field_name("marker") {
            marker.utf8_text(&[]).unwrap_or("").len() == level
        } else {
            false
        }
    }

    /// Extract header text without the # markers
    fn extract_header_text(&self, node: &Node, source: &[u8]) -> Option<String> {
        if node.kind() == "atx_heading" {
            if let Some(heading_content) = node.child_by_field_name("heading_content") {
                let text = self.get_node_text(&heading_content, source);
                return Some(text.trim().to_string());
            }
        }
        None
    }

    /// Check if node is a code block (fenced or indented)
    #[allow(dead_code)]
    fn is_code_block(&self, node: &Node) -> bool {
        matches!(node.kind(), "fenced_code_block" | "indented_code_block")
    }

    /// Check if node is a table
    #[allow(dead_code)]
    fn is_table(&self, node: &Node) -> bool {
        node.kind() == "pipe_table"
    }

    /// Check if node is a list (ordered or unordered)
    #[allow(dead_code)]
    fn is_list(&self, node: &Node) -> bool {
        matches!(node.kind(), "list" | "list_item")
    }

    /// Check if node is a blockquote
    #[allow(dead_code)]
    fn is_blockquote(&self, node: &Node) -> bool {
        node.kind() == "block_quote"
    }

    /// Extract code block language if present
    fn get_code_block_language(&self, node: &Node, source: &[u8]) -> Option<String> {
        if node.kind() == "fenced_code_block" {
            // Look for info_string child which contains the language
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "info_string" {
                    let lang = self.get_node_text(&child, source);
                    if !lang.trim().is_empty() {
                        return Some(lang.trim().to_string());
                    }
                }
            }
        }
        None
    }
}

impl LanguageImpl for MarkdownLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_md::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "md"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        // Accept markdown structural elements
        let acceptable = matches!(
            node.kind(),
            "document"
                | "atx_heading"          // # Headers
                | "setext_heading"       // Underlined headers
                | "fenced_code_block"    // ```code```
                | "indented_code_block"  // Indented code
                | "pipe_table"           // | Table |
                | "list"                 // Lists
                | "list_item"            // List items
                | "block_quote"          // > Blockquote
                | "paragraph"            // Text paragraphs
                | "thematic_break"       // ---
                | "html_block"           // <html>
                | "link_reference_definition" // [ref]: url
                | "section" // Logical sections
        );

        if debug_mode && acceptable {
            println!(
                "DEBUG: Markdown acceptable parent: {} at lines {}-{}",
                node.kind(),
                node.start_position().row + 1,
                node.end_position().row + 1
            );
        }

        acceptable
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        // In markdown, we can consider test-related sections
        // Check for headers that contain test-related keywords
        if node.kind() == "atx_heading" {
            if let Some(header_text) = self.extract_header_text(node, source) {
                let lower_text = header_text.to_lowercase();
                let is_test = lower_text.contains("test")
                    || lower_text.contains("testing")
                    || lower_text.contains("spec")
                    || lower_text.contains("specification")
                    || lower_text.contains("example")
                    || lower_text.contains("demo");

                if debug_mode && is_test {
                    println!("DEBUG: Test node detected (Markdown): test-related header");
                }
                return is_test;
            }
        }

        // Check for code blocks with test-related languages
        if node.kind() == "fenced_code_block" {
            if let Some(lang) = self.get_code_block_language(node, source) {
                let lower_lang = lang.to_lowercase();
                let is_test_lang = lower_lang.contains("test")
                    || lower_lang.contains("spec")
                    || lower_lang == "jest"
                    || lower_lang == "mocha"
                    || lower_lang == "pytest";

                if debug_mode && is_test_lang {
                    println!("DEBUG: Test node detected (Markdown): test-related code block");
                }
                return is_test_lang;
            }
        }

        false
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "atx_heading" => {
                // Extract header with proper level indication
                if let Some(marker) = node.child_by_field_name("marker") {
                    let marker_text = self.get_node_text(&marker, source);
                    if let Some(content_text) = self.extract_header_text(node, source) {
                        return Some(format!("{} {}", marker_text, content_text));
                    }
                }
                None
            }
            "setext_heading" => {
                // Extract underlined header
                if let Some(heading_content) = node.child_by_field_name("heading_content") {
                    let text = self.get_node_text(&heading_content, source);
                    // Determine level based on underline character
                    let level = if node.utf8_text(source).unwrap_or("").contains('=') {
                        "# " // H1
                    } else {
                        "## " // H2
                    };
                    return Some(format!("{}{}", level, text.trim()));
                }
                None
            }
            "fenced_code_block" => {
                // Extract code block signature with language
                if let Some(lang) = self.get_code_block_language(node, source) {
                    Some(format!("```{}", lang))
                } else {
                    Some("```".to_string())
                }
            }
            "indented_code_block" => {
                // Indented code block
                Some("    code".to_string())
            }
            "pipe_table" => {
                // Extract table signature - show just the header row if available
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "pipe_table_header" {
                        let header_text = self.get_node_text(&child, source);
                        // Truncate if too long
                        if header_text.len() > 60 {
                            return Some(format!("{}...", &header_text[..60].trim()));
                        } else {
                            return Some(header_text.trim().to_string());
                        }
                    }
                }
                Some("| Table |".to_string())
            }
            "list" => {
                // Extract list type and first item
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "list_item" {
                        let item_text = self.get_node_text(&child, source);
                        let first_line = item_text.lines().next().unwrap_or("");
                        if first_line.len() > 50 {
                            return Some(format!("{}...", &first_line[..50].trim()));
                        } else {
                            return Some(first_line.trim().to_string());
                        }
                    }
                }
                Some("- List".to_string())
            }
            "block_quote" => {
                // Extract blockquote first line
                let quote_text = self.get_node_text(node, source);
                let first_line = quote_text.lines().next().unwrap_or("");
                if first_line.len() > 60 {
                    Some(format!("{}...", &first_line[..60].trim()))
                } else {
                    Some(first_line.trim().to_string())
                }
            }
            "thematic_break" => {
                // Horizontal rule
                Some("---".to_string())
            }
            "link_reference_definition" => {
                // Link reference: [label]: url "title"
                let ref_text = self.get_node_text(node, source);
                if ref_text.len() > 50 {
                    Some(format!("{}...", &ref_text[..50].trim()))
                } else {
                    Some(ref_text.trim().to_string())
                }
            }
            _ => None,
        }
    }

    fn find_parent_function<'a>(&self, _node: Node<'a>) -> Option<Node<'a>> {
        // Markdown doesn't have functions in the traditional sense
        // We could return the parent header/section, but that's handled elsewhere
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_markdown(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_md::LANGUAGE.into())
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
    fn test_markdown_language_creation() {
        let _lang = MarkdownLanguage::new();
        // Basic test to ensure MarkdownLanguage can be created
    }

    #[test]
    fn test_acceptable_parents() {
        let lang = MarkdownLanguage::new();
        let source = r#"# Header

```rust
fn main() {}
```

| Table | Header |
|-------|--------|
| Cell  | Cell   |

- List item
- Another item

> Blockquote
> content
"#;

        let tree = parse_markdown(source);
        let root = tree.root_node();

        // Find and test different node types
        if let Some(header) = get_first_node_of_kind(root, "atx_heading") {
            assert!(lang.is_acceptable_parent(&header));
        }

        if let Some(code_block) = get_first_node_of_kind(root, "fenced_code_block") {
            assert!(lang.is_acceptable_parent(&code_block));
        }

        if let Some(table) = get_first_node_of_kind(root, "pipe_table") {
            assert!(lang.is_acceptable_parent(&table));
        }

        if let Some(list) = get_first_node_of_kind(root, "list") {
            assert!(lang.is_acceptable_parent(&list));
        }

        if let Some(blockquote) = get_first_node_of_kind(root, "block_quote") {
            assert!(lang.is_acceptable_parent(&blockquote));
        }
    }

    #[test]
    fn test_header_signature_extraction() {
        let lang = MarkdownLanguage::new();
        let source = "# Main Header\n## Subheader\n### Deep Header";
        let tree = parse_markdown(source);
        let root = tree.root_node();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "atx_heading" {
                if let Some(sig) = lang.get_symbol_signature(&child, source.as_bytes()) {
                    assert!(sig.starts_with('#'));
                    assert!(sig.contains("Header"));
                }
            }
        }
    }

    #[test]
    fn test_code_block_signature_extraction() {
        let lang = MarkdownLanguage::new();
        let source = r#"```rust
fn main() {
    println!("Hello");
}
```"#;
        let tree = parse_markdown(source);
        let root = tree.root_node();

        if let Some(code_block) = get_first_node_of_kind(root, "fenced_code_block") {
            let sig = lang.get_symbol_signature(&code_block, source.as_bytes());
            assert_eq!(sig, Some("```rust".to_string()));
        }
    }

    #[test]
    fn test_test_node_detection() {
        let lang = MarkdownLanguage::new();

        // Test header detection
        let test_source = "# Testing Guide\n## Examples";
        let tree = parse_markdown(test_source);
        let root = tree.root_node();

        if let Some(header) = get_first_node_of_kind(root, "atx_heading") {
            assert!(lang.is_test_node(&header, test_source.as_bytes()));
        }

        // Test code block detection
        let code_source = r#"```test
describe("test suite", () => {
  it("should work", () => {});
});
```"#;
        let code_tree = parse_markdown(code_source);
        let code_root = code_tree.root_node();

        if let Some(code_block) = get_first_node_of_kind(code_root, "fenced_code_block") {
            // This should be true if the language contains "test"
            assert!(lang.is_test_node(&code_block, code_source.as_bytes()));
        }
    }

    #[test]
    fn test_table_signature_extraction() {
        let lang = MarkdownLanguage::new();
        let source = r#"| Header 1 | Header 2 | Header 3 |
|----------|----------|----------|
| Cell 1   | Cell 2   | Cell 3   |"#;
        let tree = parse_markdown(source);
        let root = tree.root_node();

        if let Some(table) = get_first_node_of_kind(root, "pipe_table") {
            if let Some(sig) = lang.get_symbol_signature(&table, source.as_bytes()) {
                assert!(sig.contains("Header"));
            }
        }
    }

    #[test]
    fn test_list_signature_extraction() {
        let lang = MarkdownLanguage::new();
        let source = r#"- First item
- Second item
- Third item"#;
        let tree = parse_markdown(source);
        let root = tree.root_node();

        if let Some(list) = get_first_node_of_kind(root, "list") {
            if let Some(sig) = lang.get_symbol_signature(&list, source.as_bytes()) {
                assert!(sig.contains("First item") || sig.contains("- First item"));
            }
        }
    }

    #[test]
    fn test_blockquote_signature_extraction() {
        let lang = MarkdownLanguage::new();
        let source = "> This is a blockquote\n> with multiple lines";
        let tree = parse_markdown(source);
        let root = tree.root_node();

        if let Some(blockquote) = get_first_node_of_kind(root, "block_quote") {
            if let Some(sig) = lang.get_symbol_signature(&blockquote, source.as_bytes()) {
                assert!(
                    sig.contains("This is a blockquote") || sig.contains("> This is a blockquote")
                );
            }
        }
    }
}
