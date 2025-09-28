//! AST Symbol Extractor Module
//!
//! This module provides tree-sitter based symbol extraction capabilities to replace
//! regex-based symbol extraction. It leverages the main probe application's tree-sitter
//! infrastructure while providing symbol extraction capabilities for the LSP daemon's
//! indexing pipeline.

use crate::symbol::{SymbolKind, SymbolLocation, SymbolUIDGenerator, Visibility};
use anyhow::Result;
use std::collections::HashMap;
use tree_sitter::{Language as TSLanguage, Node};

// Re-export for other modules
pub use crate::analyzer::types::ExtractedSymbol;

/// Priority levels for indexing different symbols
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IndexingPriority {
    Critical = 0, // Test symbols, main functions
    High = 1,     // Public functions, classes, interfaces
    Normal = 2,   // Private functions, methods
    Low = 3,      // Variables, fields
}

/// Trait for language-specific symbol extraction
pub trait LanguageExtractor: Send + Sync {
    /// Extract symbols from an AST node with file path context
    fn extract_symbols(
        &self,
        node: Node,
        content: &[u8],
        file_path: &std::path::Path,
        language: TSLanguage,
    ) -> Result<Vec<ExtractedSymbol>>;

    /// Determine if a node represents a symbol worth extracting
    fn is_symbol_node(&self, node: Node) -> bool;

    /// Extract the name from a symbol node
    fn extract_symbol_name(&self, node: Node, content: &[u8]) -> Option<String>;

    /// Determine the symbol kind from a node
    fn determine_symbol_kind(&self, node: Node) -> String;

    /// Extract visibility information if available
    fn extract_visibility(&self, node: Node, content: &[u8]) -> Option<String>;

    /// Check if a symbol is a test
    fn is_test_symbol(&self, node: Node, content: &[u8]) -> bool;

    /// Extract function signature if available
    fn extract_function_signature(&self, node: Node, content: &[u8]) -> Option<String>;

    /// Extract documentation if available
    fn extract_documentation(&self, node: Node, content: &[u8]) -> Option<String>;
}

/// Generic language extractor that works for most languages
#[derive(Debug, Clone)]
pub struct GenericLanguageExtractor;

impl GenericLanguageExtractor {
    pub fn new() -> Self {
        Self
    }

    fn calculate_priority(
        &self,
        _node: Node,
        symbol_kind: &str,
        is_test: bool,
    ) -> IndexingPriority {
        if is_test {
            return IndexingPriority::Critical;
        }

        match symbol_kind {
            "function" | "method" => IndexingPriority::High,
            "class" | "struct" | "interface" => IndexingPriority::High,
            "variable" | "field" => IndexingPriority::Low,
            _ => IndexingPriority::Normal,
        }
    }
}

impl LanguageExtractor for GenericLanguageExtractor {
    fn extract_symbols(
        &self,
        node: Node,
        content: &[u8],
        file_path: &std::path::Path,
        _language: TSLanguage,
    ) -> Result<Vec<ExtractedSymbol>> {
        let mut symbols = Vec::new();
        self.extract_symbols_recursive(node, content, file_path, &mut symbols)?;
        Ok(symbols)
    }

    fn is_symbol_node(&self, node: Node) -> bool {
        matches!(
            node.kind(),
            "function_declaration"
                | "method_declaration"
                | "class_declaration"
                | "struct_declaration"
                | "interface_declaration"
                | "variable_declaration"
        )
    }

    fn extract_symbol_name(&self, node: Node, content: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                let name = child.utf8_text(content).unwrap_or("");
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    fn determine_symbol_kind(&self, node: Node) -> String {
        match node.kind() {
            "function_declaration" => "function",
            "method_declaration" => "method",
            "class_declaration" => "class",
            "struct_declaration" => "struct",
            "interface_declaration" => "interface",
            "variable_declaration" => "variable",
            other => other,
        }
        .to_string()
    }

    fn extract_visibility(&self, _node: Node, _content: &[u8]) -> Option<String> {
        None // Generic extractor doesn't extract visibility
    }

    fn is_test_symbol(&self, node: Node, content: &[u8]) -> bool {
        // Check if symbol name contains "test"
        if let Some(name) = self.extract_symbol_name(node, content) {
            return name.to_lowercase().contains("test");
        }
        false
    }

    fn extract_function_signature(&self, node: Node, content: &[u8]) -> Option<String> {
        let full_text = node.utf8_text(content).unwrap_or("");
        if !full_text.is_empty() {
            // Find the opening brace to extract just the signature
            if let Some(end_pos) = full_text.find('{') {
                return Some(full_text[..end_pos].trim().to_string());
            }
            return Some(full_text.trim().to_string());
        }
        None
    }

    fn extract_documentation(&self, _node: Node, _content: &[u8]) -> Option<String> {
        None // Generic extractor doesn't extract documentation
    }
}

impl GenericLanguageExtractor {
    fn extract_symbols_recursive(
        &self,
        node: Node,
        content: &[u8],
        file_path: &std::path::Path,
        symbols: &mut Vec<ExtractedSymbol>,
    ) -> Result<()> {
        // Validate file path is not empty - this should never happen during AST parsing
        if file_path.as_os_str().is_empty() {
            return Err(anyhow::anyhow!(
                "AST extraction error: file_path is empty in GenericLanguageExtractor. This indicates a bug."
            ));
        }

        if self.is_symbol_node(node) {
            if let Some(name) = self.extract_symbol_name(node, content) {
                let symbol_kind = self.determine_symbol_kind(node);
                let is_test = self.is_test_symbol(node, content);

                // Generate a temporary UID for now
                let uid = format!(
                    "{}:{}:{}",
                    name,
                    node.start_position().row,
                    node.start_position().column
                );

                let location = SymbolLocation {
                    file_path: file_path.to_path_buf(), // Now properly set from parameter
                    start_line: node.start_position().row as u32,
                    start_char: node.start_position().column as u32,
                    end_line: node.end_position().row as u32,
                    end_char: node.end_position().column as u32,
                };

                let symbol_kind_enum = match symbol_kind.as_str() {
                    "function" => SymbolKind::Function,
                    "method" => SymbolKind::Method,
                    "class" => SymbolKind::Class,
                    "struct" => SymbolKind::Struct,
                    "interface" => SymbolKind::Interface,
                    "variable" => SymbolKind::Variable,
                    _ => SymbolKind::Function, // Default fallback
                };

                let mut symbol = ExtractedSymbol::new(uid, name, symbol_kind_enum, location);

                // Set optional fields
                symbol.visibility =
                    self.extract_visibility(node, content)
                        .map(|v| match v.as_str() {
                            "public" => Visibility::Public,
                            "private" => Visibility::Private,
                            "protected" => Visibility::Protected,
                            _ => Visibility::Public,
                        });
                symbol.signature = self.extract_function_signature(node, content);
                symbol.documentation = self.extract_documentation(node, content);

                if is_test {
                    symbol.tags.push("test".to_string());
                }

                symbols.push(symbol);
            }
        }

        // Recursively process children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_symbols_recursive(child, content, file_path, symbols)?;
        }

        Ok(())
    }
}

/// Create appropriate extractor for the given language
pub fn create_extractor(language_name: &str) -> Box<dyn LanguageExtractor> {
    match language_name {
        "rust" => Box::new(RustLanguageExtractor::new()),
        "python" => Box::new(PythonLanguageExtractor::new()),
        "typescript" | "javascript" => Box::new(TypeScriptLanguageExtractor::new()),
        "go" => Box::new(GoLanguageExtractor::new()),
        "java" => Box::new(JavaLanguageExtractor::new()),
        _ => Box::new(GenericLanguageExtractor::new()),
    }
}

/// Rust-specific extractor
#[derive(Debug, Clone)]
pub struct RustLanguageExtractor;

impl RustLanguageExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageExtractor for RustLanguageExtractor {
    fn extract_symbols(
        &self,
        node: Node,
        content: &[u8],
        file_path: &std::path::Path,
        _language: TSLanguage,
    ) -> Result<Vec<ExtractedSymbol>> {
        let mut symbols = Vec::new();
        self.extract_symbols_recursive(node, content, file_path, &mut symbols)?;
        Ok(symbols)
    }

    fn is_symbol_node(&self, node: Node) -> bool {
        matches!(
            node.kind(),
            "function_item" | "impl_item" | "struct_item" | "enum_item" | "trait_item"
        )
    }

    fn extract_symbol_name(&self, node: Node, content: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                let name = child.utf8_text(content).unwrap_or("");
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    fn determine_symbol_kind(&self, node: Node) -> String {
        match node.kind() {
            "function_item" => "function",
            "impl_item" => "impl",
            "struct_item" => "struct",
            "enum_item" => "enum",
            "trait_item" => "trait",
            other => other,
        }
        .to_string()
    }

    fn extract_visibility(&self, node: Node, content: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                let vis = child.utf8_text(content).unwrap_or("");
                if !vis.is_empty() {
                    return Some(vis.to_string());
                }
            }
        }
        None
    }

    fn is_test_symbol(&self, node: Node, content: &[u8]) -> bool {
        // Check for #[test] attribute
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "attribute_item" {
                let attr_text = child.utf8_text(content).unwrap_or("");
                if attr_text.contains("#[test") {
                    return true;
                }
            }
        }

        // Check function name starting with "test_"
        if let Some(name) = self.extract_symbol_name(node, content) {
            return name.starts_with("test_");
        }

        false
    }

    fn extract_function_signature(&self, node: Node, content: &[u8]) -> Option<String> {
        if node.kind() == "function_item" {
            let full_text = node.utf8_text(content).unwrap_or("");
            if !full_text.is_empty() {
                // Find the opening brace
                if let Some(end_pos) = full_text.find('{') {
                    let signature = full_text[..end_pos].trim().to_string();
                    return Some(signature);
                }
                return Some(full_text.trim().to_string());
            }
        }
        None
    }

    fn extract_documentation(&self, node: Node, content: &[u8]) -> Option<String> {
        // Look for doc comments immediately preceding the symbol
        let mut current = node.prev_sibling();
        let mut doc_comments = Vec::new();

        while let Some(sibling) = current {
            if sibling.kind() == "line_comment" {
                let comment_text = sibling.utf8_text(content).unwrap_or("");
                if comment_text.starts_with("///") {
                    doc_comments.insert(0, comment_text.to_string());
                    current = sibling.prev_sibling();
                    continue;
                }
            }
            break;
        }

        if !doc_comments.is_empty() {
            Some(doc_comments.join("\n"))
        } else {
            None
        }
    }
}

impl RustLanguageExtractor {
    fn extract_symbols_recursive(
        &self,
        node: Node,
        content: &[u8],
        file_path: &std::path::Path,
        symbols: &mut Vec<ExtractedSymbol>,
    ) -> Result<()> {
        // Validate file path is not empty - this should never happen during AST parsing
        if file_path.as_os_str().is_empty() {
            return Err(anyhow::anyhow!(
                "AST extraction error: file_path is empty in RustLanguageExtractor. This indicates a bug."
            ));
        }

        if self.is_symbol_node(node) {
            if let Some(name) = self.extract_symbol_name(node, content) {
                let symbol_kind = self.determine_symbol_kind(node);
                let is_test = self.is_test_symbol(node, content);

                let _priority = if is_test {
                    IndexingPriority::Critical
                } else {
                    match symbol_kind.as_str() {
                        "function" => IndexingPriority::High,
                        "struct" | "enum" | "trait" => IndexingPriority::High,
                        _ => IndexingPriority::Normal,
                    }
                };

                // Generate a temporary UID for now
                let uid = format!(
                    "{}:{}:{}",
                    name,
                    node.start_position().row,
                    node.start_position().column
                );

                let location = SymbolLocation {
                    file_path: file_path.to_path_buf(), // Now properly set from parameter
                    start_line: node.start_position().row as u32,
                    start_char: node.start_position().column as u32,
                    end_line: node.end_position().row as u32,
                    end_char: node.end_position().column as u32,
                };

                let symbol_kind_enum = match symbol_kind.as_str() {
                    "function" => SymbolKind::Function,
                    "impl" => SymbolKind::Class, // Treat impl as class-like
                    "struct" => SymbolKind::Struct,
                    "enum" => SymbolKind::Enum,
                    "trait" => SymbolKind::Trait,
                    _ => SymbolKind::Function,
                };

                let mut symbol = ExtractedSymbol::new(uid, name, symbol_kind_enum, location);

                // Set optional fields
                symbol.visibility =
                    self.extract_visibility(node, content)
                        .map(|v| match v.as_str() {
                            "pub" => Visibility::Public,
                            _ => Visibility::Private,
                        });
                symbol.signature = self.extract_function_signature(node, content);
                symbol.documentation = self.extract_documentation(node, content);

                if is_test {
                    symbol.tags.push("test".to_string());
                }

                symbols.push(symbol);
            }
        }

        // Recursively process children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_symbols_recursive(child, content, file_path, symbols)?;
        }

        Ok(())
    }
}

/// Placeholder implementations for other languages - using the proven pattern
macro_rules! impl_language_extractor {
    ($name:ident, $symbol_nodes:expr) => {
        #[derive(Debug, Clone)]
        pub struct $name;

        impl $name {
            pub fn new() -> Self {
                Self
            }
        }

        impl LanguageExtractor for $name {
            fn extract_symbols(
                &self,
                node: Node,
                content: &[u8],
                file_path: &std::path::Path,
                _language: TSLanguage,
            ) -> Result<Vec<ExtractedSymbol>> {
                let mut symbols = Vec::new();
                self.extract_symbols_recursive(node, content, file_path, &mut symbols)?;
                Ok(symbols)
            }

            fn is_symbol_node(&self, node: Node) -> bool {
                $symbol_nodes.contains(&node.kind())
            }

            fn extract_symbol_name(&self, node: Node, content: &[u8]) -> Option<String> {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name = child.utf8_text(content).unwrap_or("");
                        if !name.is_empty() {
                            return Some(name.to_string());
                        }
                    }
                }
                None
            }

            fn determine_symbol_kind(&self, node: Node) -> String {
                node.kind().to_string()
            }

            fn extract_visibility(&self, _node: Node, _content: &[u8]) -> Option<String> {
                None
            }

            fn is_test_symbol(&self, node: Node, content: &[u8]) -> bool {
                if let Some(name) = self.extract_symbol_name(node, content) {
                    return name.to_lowercase().contains("test");
                }
                false
            }

            fn extract_function_signature(&self, node: Node, content: &[u8]) -> Option<String> {
                let full_text = node.utf8_text(content).unwrap_or("");
                if !full_text.is_empty() {
                    if let Some(end_pos) = full_text.find('{') {
                        return Some(full_text[..end_pos].trim().to_string());
                    }
                    return Some(full_text.trim().to_string());
                }
                None
            }

            fn extract_documentation(&self, _node: Node, _content: &[u8]) -> Option<String> {
                None
            }
        }

        impl $name {
            fn extract_symbols_recursive(
                &self,
                node: Node,
                content: &[u8],
                file_path: &std::path::Path,
                symbols: &mut Vec<ExtractedSymbol>,
            ) -> Result<()> {
                // Validate file path is not empty - this should never happen during AST parsing
                if file_path.as_os_str().is_empty() {
                    return Err(anyhow::anyhow!(
                        "AST extraction error: file_path is empty in {}, This indicates a bug.",
                        stringify!($name)
                    ));
                }

                if self.is_symbol_node(node) {
                    if let Some(name) = self.extract_symbol_name(node, content) {
                        let symbol_kind = self.determine_symbol_kind(node);
                        let is_test = self.is_test_symbol(node, content);

                        let _priority = if is_test {
                            IndexingPriority::Critical
                        } else {
                            IndexingPriority::Normal
                        };

                        // Generate a temporary UID for now
                        let uid = format!(
                            "{}:{}:{}",
                            name,
                            node.start_position().row,
                            node.start_position().column
                        );

                        let location = SymbolLocation {
                            file_path: file_path.to_path_buf(), // Now properly set from parameter
                            start_line: node.start_position().row as u32,
                            start_char: node.start_position().column as u32,
                            end_line: node.end_position().row as u32,
                            end_char: node.end_position().column as u32,
                        };

                        let symbol_kind_enum = match symbol_kind.as_str() {
                            "function_definition" | "function_declaration" => SymbolKind::Function,
                            "method_declaration" => SymbolKind::Method,
                            "class_definition" | "class_declaration" => SymbolKind::Class,
                            "interface_declaration" => SymbolKind::Interface,
                            "type_declaration" => SymbolKind::Type,
                            _ => SymbolKind::Function,
                        };

                        let mut symbol =
                            ExtractedSymbol::new(uid, name, symbol_kind_enum, location);

                        // Set optional fields
                        symbol.signature = self.extract_function_signature(node, content);

                        if is_test {
                            symbol.tags.push("test".to_string());
                        }

                        symbols.push(symbol);
                    }
                }

                // Recursively process children
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.extract_symbols_recursive(child, content, file_path, symbols)?;
                }

                Ok(())
            }
        }
    };
}

// Implement simple extractors for other languages using the proven pattern
impl_language_extractor!(
    PythonLanguageExtractor,
    &["function_definition", "class_definition"]
);

impl_language_extractor!(
    TypeScriptLanguageExtractor,
    &[
        "function_declaration",
        "class_declaration",
        "interface_declaration"
    ]
);

impl_language_extractor!(
    GoLanguageExtractor,
    &[
        "function_declaration",
        "method_declaration",
        "type_declaration"
    ]
);

impl_language_extractor!(
    JavaLanguageExtractor,
    &[
        "method_declaration",
        "class_declaration",
        "interface_declaration"
    ]
);

/// Main AST symbol extractor that orchestrates language-specific extraction
pub struct AstSymbolExtractor {
    /// Language-specific extractors
    extractors: HashMap<TSLanguage, Box<dyn LanguageExtractor>>,

    /// UID generator for creating unique symbol identifiers
    uid_generator: SymbolUIDGenerator,
}

impl std::fmt::Debug for AstSymbolExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AstSymbolExtractor")
            .field("extractors_count", &self.extractors.len())
            .field("uid_generator", &"SymbolUIDGenerator")
            .finish()
    }
}

impl AstSymbolExtractor {
    pub fn new() -> Self {
        let extractors: HashMap<TSLanguage, Box<dyn LanguageExtractor>> = HashMap::new();

        // We'll populate these as needed based on the language
        Self {
            extractors,
            uid_generator: SymbolUIDGenerator::new(),
        }
    }

    /// Get tree-sitter language for a given language enum
    fn get_tree_sitter_language(
        &self,
        language: crate::language_detector::Language,
    ) -> Result<TSLanguage> {
        match language {
            crate::language_detector::Language::Rust => Ok(tree_sitter_rust::LANGUAGE.into()),
            crate::language_detector::Language::TypeScript => {
                Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            }
            crate::language_detector::Language::JavaScript => {
                Ok(tree_sitter_javascript::LANGUAGE.into())
            }
            crate::language_detector::Language::Python => Ok(tree_sitter_python::LANGUAGE.into()),
            crate::language_detector::Language::Go => Ok(tree_sitter_go::LANGUAGE.into()),
            crate::language_detector::Language::Java => Ok(tree_sitter_java::LANGUAGE.into()),
            crate::language_detector::Language::C => Ok(tree_sitter_c::LANGUAGE.into()),
            crate::language_detector::Language::Cpp => Ok(tree_sitter_cpp::LANGUAGE.into()),
            _ => Err(anyhow::anyhow!("Unsupported language: {:?}", language)),
        }
    }

    /// Extract symbols from source code using appropriate language extractor
    pub fn extract_symbols(
        &mut self,
        _content: &[u8],
        language_name: &str,
    ) -> Result<Vec<ExtractedSymbol>> {
        let _extractor = create_extractor(language_name);

        // For now, return empty results since we need proper tree-sitter integration
        // This is a minimal implementation to fix compilation
        Ok(vec![])
    }

    /// Extract symbols from file using appropriate language extractor
    pub fn extract_symbols_from_file<P: AsRef<std::path::Path>>(
        &mut self,
        file_path: P,
        content: &str,
        language: crate::language_detector::Language,
    ) -> Result<Vec<ExtractedSymbol>> {
        let file_path = file_path.as_ref();

        // Get tree-sitter language for parsing
        let ts_language = match self.get_tree_sitter_language(language) {
            Ok(lang) => lang,
            Err(_) => {
                // Language not supported for AST extraction, return empty
                return Ok(vec![]);
            }
        };

        // Parse the file content with tree-sitter
        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&ts_language).is_err() {
            return Ok(vec![]);
        }

        let tree = match parser.parse(content.as_bytes(), None) {
            Some(tree) => tree,
            None => return Ok(vec![]),
        };

        let root_node = tree.root_node();
        let content_bytes = content.as_bytes();

        // Extract symbols using tree traversal
        let mut symbols = Vec::new();
        self.traverse_node(root_node, content_bytes, file_path, &mut symbols, language)?;

        Ok(symbols)
    }

    /// Recursively traverse tree-sitter nodes to find symbols
    fn traverse_node(
        &self,
        node: tree_sitter::Node,
        content: &[u8],
        file_path: &std::path::Path,
        symbols: &mut Vec<ExtractedSymbol>,
        language: crate::language_detector::Language,
    ) -> Result<()> {
        // Check if this node represents a symbol we want to extract
        if let Some(symbol) = self.node_to_symbol(node, content, file_path, language)? {
            symbols.push(symbol);
        }

        // Recursively traverse children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.traverse_node(child, content, file_path, symbols, language)?;
        }

        Ok(())
    }

    /// Convert a tree-sitter node to an ExtractedSymbol if it represents a symbol
    fn node_to_symbol(
        &self,
        node: tree_sitter::Node,
        content: &[u8],
        file_path: &std::path::Path,
        language: crate::language_detector::Language,
    ) -> Result<Option<ExtractedSymbol>> {
        // Validate file path is not empty - this should never happen during AST parsing
        if file_path.as_os_str().is_empty() {
            return Err(anyhow::anyhow!(
                "AST extraction error: file_path is empty. This indicates a bug in the AST extractor."
            ));
        }

        let node_kind = node.kind();

        // Map node types to symbol kinds based on language
        let (symbol_kind, should_extract) = match language {
            crate::language_detector::Language::Rust => {
                match node_kind {
                    "function_item" | "impl_item" => (SymbolKind::Function, true),
                    "struct_item" => (SymbolKind::Class, true), // Rust structs are like classes
                    "enum_item" => (SymbolKind::Enum, true),
                    "trait_item" => (SymbolKind::Interface, true), // Rust traits are like interfaces
                    "const_item" | "static_item" => (SymbolKind::Constant, true),
                    "type_item" => (SymbolKind::Type, true),
                    _ => (SymbolKind::Function, false),
                }
            }
            crate::language_detector::Language::JavaScript
            | crate::language_detector::Language::TypeScript => match node_kind {
                "function_declaration" | "method_definition" | "arrow_function" => {
                    (SymbolKind::Function, true)
                }
                "class_declaration" => (SymbolKind::Class, true),
                "interface_declaration" => (SymbolKind::Interface, true),
                "variable_declaration" => (SymbolKind::Variable, true),
                "const_declaration" => (SymbolKind::Constant, true),
                _ => (SymbolKind::Function, false),
            },
            crate::language_detector::Language::Python => match node_kind {
                "function_definition" => (SymbolKind::Function, true),
                "class_definition" => (SymbolKind::Class, true),
                _ => (SymbolKind::Function, false),
            },
            crate::language_detector::Language::Go => match node_kind {
                "function_declaration" | "method_declaration" => (SymbolKind::Function, true),
                "type_declaration" => (SymbolKind::Type, true),
                _ => (SymbolKind::Function, false),
            },
            crate::language_detector::Language::Java => match node_kind {
                "method_declaration" | "constructor_declaration" => (SymbolKind::Function, true),
                "class_declaration" => (SymbolKind::Class, true),
                "interface_declaration" => (SymbolKind::Interface, true),
                "field_declaration" => (SymbolKind::Variable, true),
                _ => (SymbolKind::Function, false),
            },
            _ => {
                // For other languages, try some common patterns
                match node_kind {
                    "function_declaration" | "method_declaration" | "function_definition" => {
                        (SymbolKind::Function, true)
                    }
                    "class_declaration" | "class_definition" => (SymbolKind::Class, true),
                    _ => (SymbolKind::Function, false),
                }
            }
        };

        if !should_extract {
            return Ok(None);
        }

        // Extract the symbol name
        let name = self
            .extract_symbol_name(node, content)
            .unwrap_or_else(|| "unknown".to_string());
        if name.is_empty() || name == "unknown" {
            return Ok(None);
        }

        // Calculate line and column positions
        let start_point = node.start_position();
        let end_point = node.end_position();

        // Create the symbol location
        let location = SymbolLocation {
            file_path: file_path.to_path_buf(),
            start_line: start_point.row as u32,
            start_char: start_point.column as u32,
            end_line: end_point.row as u32,
            end_char: end_point.column as u32,
        };

        // Generate UID using the UID generator with proper context
        let uid_symbol_kind = match symbol_kind {
            SymbolKind::Function => crate::symbol::SymbolKind::Function,
            SymbolKind::Method => crate::symbol::SymbolKind::Method,
            SymbolKind::Class => crate::symbol::SymbolKind::Class,
            SymbolKind::Struct => crate::symbol::SymbolKind::Struct,
            SymbolKind::Interface => crate::symbol::SymbolKind::Interface,
            SymbolKind::Trait => crate::symbol::SymbolKind::Trait,
            SymbolKind::Enum => crate::symbol::SymbolKind::Enum,
            SymbolKind::Variable => crate::symbol::SymbolKind::Variable,
            SymbolKind::Constant => crate::symbol::SymbolKind::Constant,
            SymbolKind::Type => crate::symbol::SymbolKind::Type,
            _ => crate::symbol::SymbolKind::Function, // Default fallback
        };
        let symbol_info = crate::symbol::SymbolInfo::new(
            name.clone(),
            uid_symbol_kind,
            language.as_str().to_string(),
            location.clone(),
        );
        let context = crate::symbol::SymbolContext::new(0, language.as_str().to_string());
        let uid = self
            .uid_generator
            .generate_uid(&symbol_info, &context)
            .unwrap_or_else(|_| format!("{}:{}:{}", name, start_point.row, start_point.column));

        // Attempt to compute FQN using centralized implementation
        let mut symbol = ExtractedSymbol::new(uid, name.clone(), symbol_kind, location);
        if let Ok(content_str) = std::str::from_utf8(content) {
            if let Ok(fqn) = crate::fqn::get_fqn_from_ast_with_content(
                file_path,
                content_str,
                start_point.row as u32,
                start_point.column as u32,
                Some(language.as_str()),
            ) {
                if !fqn.is_empty() {
                    symbol.qualified_name = Some(fqn);
                }
            }
        }

        Ok(Some(symbol))
    }

    /// Extract symbol name from a tree-sitter node
    fn extract_symbol_name(&self, node: tree_sitter::Node, content: &[u8]) -> Option<String> {
        let mut cursor = node.walk();

        // Look for identifier nodes in the children
        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" | "type_identifier" | "field_identifier" => {
                    let name = child.utf8_text(content).unwrap_or("");
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
                _ => continue,
            }
        }

        None
    }
}

impl Default for AstSymbolExtractor {
    fn default() -> Self {
        Self::new()
    }
}
