use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for HTML
pub struct HtmlLanguage;

impl Default for HtmlLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlLanguage {
    pub fn new() -> Self {
        HtmlLanguage
    }

    /// Helper method to get the text content of a node
    fn get_node_text(&self, node: &Node, source: &[u8]) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        String::from_utf8_lossy(&source[start..end]).to_string()
    }

    /// Check if node is a semantic HTML element
    fn is_semantic_element(&self, node: &Node) -> bool {
        if node.kind() != "element" {
            return false;
        }

        // Check start tag to get element name
        if let Some(start_tag) = node.child_by_field_name("start_tag") {
            if let Some(tag_name) = start_tag.child_by_field_name("name") {
                let tag_name_text = tag_name.utf8_text(&[]).unwrap_or("").to_lowercase();
                return matches!(
                    tag_name_text.as_str(),
                    "header"
                        | "nav"
                        | "main"
                        | "section"
                        | "article"
                        | "aside"
                        | "footer"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "figure"
                        | "figcaption"
                        | "details"
                        | "summary"
                );
            }
        }
        false
    }

    /// Check if node is a block-level element
    fn is_block_element(&self, node: &Node) -> bool {
        if node.kind() != "element" {
            return false;
        }

        // Check start tag to get element name
        if let Some(start_tag) = node.child_by_field_name("start_tag") {
            if let Some(tag_name) = start_tag.child_by_field_name("name") {
                let tag_name_text = tag_name.utf8_text(&[]).unwrap_or("").to_lowercase();
                return matches!(
                    tag_name_text.as_str(),
                    "div"
                        | "p"
                        | "blockquote"
                        | "pre"
                        | "code"
                        | "ul"
                        | "ol"
                        | "li"
                        | "table"
                        | "thead"
                        | "tbody"
                        | "tfoot"
                        | "tr"
                        | "th"
                        | "td"
                        | "form"
                        | "fieldset"
                        | "legend"
                        | "address"
                        | "hr"
                );
            }
        }
        false
    }

    /// Extract element tag name
    fn get_tag_name(&self, node: &Node) -> Option<String> {
        if node.kind() == "element" {
            if let Some(start_tag) = node.child_by_field_name("start_tag") {
                if let Some(tag_name) = start_tag.child_by_field_name("name") {
                    return Some(tag_name.utf8_text(&[]).unwrap_or("").to_string());
                }
            }
        }
        None
    }

    /// Extract attributes from an element's start tag
    fn get_element_attributes(&self, node: &Node, source: &[u8]) -> Vec<String> {
        let mut attributes = Vec::new();

        if node.kind() == "element" {
            if let Some(start_tag) = node.child_by_field_name("start_tag") {
                let mut cursor = start_tag.walk();
                for child in start_tag.children(&mut cursor) {
                    if child.kind() == "attribute" {
                        let attr_text = self.get_node_text(&child, source);
                        attributes.push(attr_text);
                    }
                }
            }
        }

        attributes
    }

    /// Get important attributes for signature (id, class, data-testid, etc.)
    fn get_signature_attributes(&self, node: &Node, source: &[u8]) -> String {
        let attributes = self.get_element_attributes(node, source);
        let mut sig_attrs = Vec::new();

        for attr in attributes {
            let attr_lower = attr.to_lowercase();
            if attr_lower.starts_with("id=")
                || attr_lower.starts_with("class=")
                || attr_lower.starts_with("data-testid=")
                || attr_lower.starts_with("role=")
                || attr_lower.starts_with("aria-label=")
            {
                // Truncate long attribute values
                if attr.len() > 40 {
                    sig_attrs.push(format!("{}...", &attr[..40]));
                } else {
                    sig_attrs.push(attr);
                }
            }
        }

        if sig_attrs.is_empty() {
            String::new()
        } else {
            format!(" {}", sig_attrs.join(" "))
        }
    }

    /// Extract attributes from start_tag node
    fn extract_attributes_from_start_tag(&self, start_tag: &Node, source: &[u8]) -> String {
        let mut sig_attrs = Vec::new();
        let mut cursor = start_tag.walk();

        for child in start_tag.children(&mut cursor) {
            if child.kind() == "attribute" {
                let attr_text = self.get_node_text(&child, source);
                let attr_lower = attr_text.to_lowercase();
                if attr_lower.starts_with("id=")
                    || attr_lower.starts_with("class=")
                    || attr_lower.starts_with("data-testid=")
                    || attr_lower.starts_with("role=")
                    || attr_lower.starts_with("aria-label=")
                    || attr_lower.starts_with("type=")
                {
                    // Truncate long attribute values
                    if attr_text.len() > 40 {
                        sig_attrs.push(format!("{}...", &attr_text[..40]));
                    } else {
                        sig_attrs.push(attr_text);
                    }
                }
            }
        }

        if sig_attrs.is_empty() {
            String::new()
        } else {
            format!(" {}", sig_attrs.join(" "))
        }
    }

    /// Check if element has test-related attributes
    fn has_test_attributes(&self, node: &Node, source: &[u8]) -> bool {
        let attributes = self.get_element_attributes(node, source);

        for attr in attributes {
            let attr_lower = attr.to_lowercase();
            if attr_lower.contains("test")
                || attr_lower.contains("spec")
                || attr_lower.contains("data-testid")
                || attr_lower.contains("data-test")
            {
                return true;
            }
        }
        false
    }

    /// Check if element contains test-related text content
    fn has_test_content(&self, node: &Node, source: &[u8]) -> bool {
        let text = self.get_node_text(node, source).to_lowercase();
        text.contains("test")
            || text.contains("testing")
            || text.contains("spec")
            || text.contains("specification")
            || text.contains("example")
            || text.contains("demo")
    }

    /// Check if this is a script or style element with test-related content
    fn is_test_script_or_style(&self, node: &Node, source: &[u8]) -> bool {
        if let Some(tag_name) = self.get_tag_name(node) {
            let tag_lower = tag_name.to_lowercase();
            if tag_lower == "script" || tag_lower == "style" {
                return self.has_test_content(node, source);
            }
        }
        false
    }
}

impl LanguageImpl for HtmlLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_html::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "html"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        // Accept HTML structural elements and semantic containers
        let acceptable = matches!(
            node.kind(),
            "document"
                | "doctype"
                | "element"              // All HTML elements
                | "script_element"       // <script> tags
                | "style_element"        // <style> tags
                | "comment"              // <!-- comments -->
                | "text" // Text content (for some context)
        ) || self.is_semantic_element(node)
            || self.is_block_element(node);

        if debug_mode && acceptable {
            let tag_name = self
                .get_tag_name(node)
                .unwrap_or_else(|| node.kind().to_string());
            println!(
                "DEBUG: HTML acceptable parent: {} ({}) at lines {}-{}",
                tag_name,
                node.kind(),
                node.start_position().row + 1,
                node.end_position().row + 1
            );
        }

        acceptable
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        // Check for test-related attributes
        if self.has_test_attributes(node, source) {
            if debug_mode {
                println!("DEBUG: Test node detected (HTML): test-related attributes");
            }
            return true;
        }

        // Check for script_element and style_element nodes directly
        if matches!(node.kind(), "script_element" | "style_element")
            && self.has_test_content(node, source)
        {
            if debug_mode {
                println!(
                    "DEBUG: Test node detected (HTML): test-related {}",
                    node.kind()
                );
            }
            return true;
        }

        // Check for test-related script or style content (for regular elements)
        if self.is_test_script_or_style(node, source) {
            if debug_mode {
                println!("DEBUG: Test node detected (HTML): test-related script/style");
            }
            return true;
        }

        // Check for elements with test-related content
        if node.kind() == "element" {
            if let Some(tag_name) = self.get_tag_name(node) {
                let tag_lower = tag_name.to_lowercase();

                // Check if it's a heading or section with test-related content
                if matches!(
                    tag_lower.as_str(),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "section" | "article" | "div"
                ) && self.has_test_content(node, source)
                {
                    if debug_mode {
                        println!(
                            "DEBUG: Test node detected (HTML): test-related content in {}",
                            tag_name
                        );
                    }
                    return true;
                }
            }
        }

        false
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "element" => {
                if let Some(tag_name) = self.get_tag_name(node) {
                    let attributes = self.get_signature_attributes(node, source);

                    // For self-closing elements or void elements
                    if self.is_void_element(&tag_name) {
                        Some(format!("<{}{} />", tag_name, attributes))
                    } else {
                        // For regular elements, show opening tag
                        Some(format!("<{}{}>", tag_name, attributes))
                    }
                } else {
                    None
                }
            }
            "script_element" => {
                // For script_element, extract attributes from the start_tag child
                if let Some(start_tag) = node.child_by_field_name("start_tag") {
                    let attributes = self.extract_attributes_from_start_tag(&start_tag, source);
                    Some(format!("<script{}>", attributes))
                } else {
                    Some("<script>".to_string())
                }
            }
            "style_element" => {
                // For style_element, extract attributes from the start_tag child
                if let Some(start_tag) = node.child_by_field_name("start_tag") {
                    let attributes = self.extract_attributes_from_start_tag(&start_tag, source);
                    Some(format!("<style{}>", attributes))
                } else {
                    Some("<style>".to_string())
                }
            }
            "comment" => {
                let comment_text = self.get_node_text(node, source);
                // Truncate long comments
                if comment_text.len() > 60 {
                    Some(format!("{}...", &comment_text[..60].trim()))
                } else {
                    Some(comment_text.trim().to_string())
                }
            }
            "doctype" => {
                let doctype_text = self.get_node_text(node, source);
                Some(doctype_text.trim().to_string())
            }
            _ => None,
        }
    }

    fn find_parent_function<'a>(&self, _node: Node<'a>) -> Option<Node<'a>> {
        // HTML doesn't have functions in the traditional sense
        // We could return the parent semantic container, but that's handled elsewhere
        None
    }
}

impl HtmlLanguage {
    /// Check if an element is a void/self-closing element
    fn is_void_element(&self, tag_name: &str) -> bool {
        matches!(
            tag_name.to_lowercase().as_str(),
            "area"
                | "base"
                | "br"
                | "col"
                | "embed"
                | "hr"
                | "img"
                | "input"
                | "link"
                | "meta"
                | "param"
                | "source"
                | "track"
                | "wbr"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_html(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_html::LANGUAGE.into())
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

    fn find_element_by_tag<'a>(
        node: tree_sitter::Node<'a>,
        tag_name: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == "element" {
            if let Some(start_tag) = node.child_by_field_name("start_tag") {
                if let Some(name_node) = start_tag.child_by_field_name("name") {
                    if name_node.utf8_text(&[]).unwrap_or("") == tag_name {
                        return Some(node);
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_element_by_tag(child, tag_name) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn test_html_language_creation() {
        let _lang = HtmlLanguage::new();
        // Basic test to ensure HtmlLanguage can be created
    }

    #[test]
    fn test_acceptable_parents() {
        let lang = HtmlLanguage::new();
        let source = r#"<!DOCTYPE html>
<html>
<head>
    <title>Test</title>
</head>
<body>
    <header>
        <h1>Main Header</h1>
    </header>
    <main>
        <section>
            <article>
                <h2>Article Title</h2>
                <p>Content</p>
            </article>
        </section>
    </main>
    <footer>
        <p>Footer content</p>
    </footer>
</body>
</html>"#;

        let tree = parse_html(source);
        let root = tree.root_node();

        // Find and test different node types
        if let Some(header) = find_element_by_tag(root, "header") {
            assert!(lang.is_acceptable_parent(&header));
        }

        if let Some(main_elem) = find_element_by_tag(root, "main") {
            assert!(lang.is_acceptable_parent(&main_elem));
        }

        if let Some(section) = find_element_by_tag(root, "section") {
            assert!(lang.is_acceptable_parent(&section));
        }

        if let Some(article) = find_element_by_tag(root, "article") {
            assert!(lang.is_acceptable_parent(&article));
        }

        if let Some(h1) = find_element_by_tag(root, "h1") {
            assert!(lang.is_acceptable_parent(&h1));
        }
    }

    #[test]
    fn test_tag_signature_extraction() {
        let lang = HtmlLanguage::new();
        let source = r#"<div class="container" id="main">
    <h1>Title</h1>
    <img src="test.jpg" alt="Test image" />
</div>"#;
        let tree = parse_html(source);
        let root = tree.root_node();

        if let Some(div) = find_element_by_tag(root, "div") {
            let sig = lang.get_symbol_signature(&div, source.as_bytes());
            assert!(sig.is_some());
            let sig_str = sig.unwrap();
            assert!(sig_str.starts_with("<div"));
            assert!(sig_str.contains("class") || sig_str.contains("id"));
        }

        if let Some(img) = find_element_by_tag(root, "img") {
            let sig = lang.get_symbol_signature(&img, source.as_bytes());
            assert!(sig.is_some());
            let sig_str = sig.unwrap();
            assert!(sig_str.contains("<img") && sig_str.contains("/>"));
        }
    }

    #[test]
    fn test_test_node_detection() {
        let lang = HtmlLanguage::new();

        // Test attribute detection
        let test_source = r#"<div data-testid="user-profile" class="profile">
    <h1>User Testing Guide</h1>
    <button data-test="submit-btn">Submit</button>
</div>"#;
        let tree = parse_html(test_source);
        let root = tree.root_node();

        if let Some(div) = find_element_by_tag(root, "div") {
            assert!(lang.is_test_node(&div, test_source.as_bytes()));
        }

        if let Some(button) = find_element_by_tag(root, "button") {
            assert!(lang.is_test_node(&button, test_source.as_bytes()));
        }

        // Test content detection
        if let Some(h1) = find_element_by_tag(root, "h1") {
            assert!(lang.is_test_node(&h1, test_source.as_bytes()));
        }
    }

    #[test]
    fn test_script_and_style_detection() {
        let lang = HtmlLanguage::new();
        let source = r#"<script>
// Test suite configuration
describe("Component tests", function() {
    it("should render correctly", function() {});
});
</script>
<style>
/* Test-specific styles */
.test-container { display: none; }
</style>"#;
        let tree = parse_html(source);
        let root = tree.root_node();

        if let Some(script) = get_first_node_of_kind(root, "script_element") {
            assert!(lang.is_test_node(&script, source.as_bytes()));
        }

        if let Some(style) = get_first_node_of_kind(root, "style_element") {
            assert!(lang.is_test_node(&style, source.as_bytes()));
        }
    }

    #[test]
    fn test_comment_signature_extraction() {
        let lang = HtmlLanguage::new();
        let source = "<!-- This is a test comment -->";
        let tree = parse_html(source);
        let root = tree.root_node();

        if let Some(comment) = get_first_node_of_kind(root, "comment") {
            let sig = lang.get_symbol_signature(&comment, source.as_bytes());
            assert!(sig.is_some());
            assert!(sig.unwrap().contains("test comment"));
        }
    }

    #[test]
    fn test_doctype_signature_extraction() {
        let lang = HtmlLanguage::new();
        let source = "<!DOCTYPE html>";
        let tree = parse_html(source);
        let root = tree.root_node();

        if let Some(doctype) = get_first_node_of_kind(root, "doctype") {
            let sig = lang.get_symbol_signature(&doctype, source.as_bytes());
            assert_eq!(sig, Some("<!DOCTYPE html>".to_string()));
        }
    }

    #[test]
    fn test_void_element_detection() {
        let lang = HtmlLanguage::new();

        assert!(lang.is_void_element("img"));
        assert!(lang.is_void_element("br"));
        assert!(lang.is_void_element("hr"));
        assert!(lang.is_void_element("input"));
        assert!(lang.is_void_element("meta"));

        assert!(!lang.is_void_element("div"));
        assert!(!lang.is_void_element("span"));
        assert!(!lang.is_void_element("p"));
    }

    #[test]
    fn test_semantic_element_detection() {
        let lang = HtmlLanguage::new();
        let source = r#"<header>
    <nav>Navigation</nav>
</header>
<main>
    <section>
        <article>Content</article>
    </section>
</main>
<footer>Footer</footer>"#;

        let tree = parse_html(source);
        let root = tree.root_node();

        if let Some(header) = find_element_by_tag(root, "header") {
            assert!(lang.is_semantic_element(&header));
        }

        if let Some(nav) = find_element_by_tag(root, "nav") {
            assert!(lang.is_semantic_element(&nav));
        }

        if let Some(main_elem) = find_element_by_tag(root, "main") {
            assert!(lang.is_semantic_element(&main_elem));
        }

        if let Some(section) = find_element_by_tag(root, "section") {
            assert!(lang.is_semantic_element(&section));
        }

        if let Some(article) = find_element_by_tag(root, "article") {
            assert!(lang.is_semantic_element(&article));
        }

        if let Some(footer) = find_element_by_tag(root, "footer") {
            assert!(lang.is_semantic_element(&footer));
        }
    }
}
