use tree_sitter::{Language as TSLanguage, Node};

/// Trait that defines the interface for all language implementations.
pub trait LanguageImpl {
    /// Get the tree-sitter language for parsing
    fn get_tree_sitter_language(&self) -> TSLanguage;

    /// Check if a node is an acceptable container/parent entity
    fn is_acceptable_parent(&self, node: &Node) -> bool;

    /// Check if a node represents a test
    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool;

    /// Get the file extension for this language
    #[deprecated(since = "0.1.0", note = "this method is not used")]
    #[allow(dead_code)]
    fn get_extension(&self) -> &'static str;

    /// Find the parent function or method declaration for a node (if any)
    fn find_parent_function<'a>(&self, _node: Node<'a>) -> Option<Node<'a>> {
        // Default implementation returns None
        None
    }

    /// Extract the symbol signature without implementation body
    /// Returns a clean signature for functions, structs, classes, methods, constants, etc.
    fn get_symbol_signature(&self, _node: &Node, _source: &[u8]) -> Option<String> {
        // Default implementation returns None
        // Each language should implement this to extract clean signatures
        None
    }
}
