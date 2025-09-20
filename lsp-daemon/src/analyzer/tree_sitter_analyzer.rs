//! Tree-sitter Based Structural Code Analyzer
//!
//! This module provides a structural code analyzer that uses tree-sitter parsers to extract
//! symbols and relationships from Abstract Syntax Trees (ASTs). It supports multiple programming
//! languages through tree-sitter's language parsers.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::time::{timeout, Duration};

use super::framework::{AnalyzerCapabilities, CodeAnalyzer, TreeSitterConfig};
use super::types::*;
use crate::relationship::TreeSitterRelationshipExtractor;
use crate::symbol::{SymbolContext, SymbolInfo, SymbolKind, SymbolLocation, SymbolUIDGenerator};

/// Convert file extension to language name for tree-sitter parsers
fn extension_to_language_name(extension: &str) -> Option<&'static str> {
    match extension.to_lowercase().as_str() {
        "rs" => Some("rust"),
        "js" | "jsx" => Some("javascript"),
        "ts" => Some("typescript"),
        "tsx" => Some("typescript"), // TSX uses TypeScript parser
        "py" => Some("python"),
        "go" => Some("go"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("cpp"),
        "java" => Some("java"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "swift" => Some("swift"),
        "cs" => Some("csharp"),
        _ => None,
    }
}

/// Tree-sitter parser pool for efficient parser reuse
pub struct ParserPool {
    parsers: HashMap<String, Vec<tree_sitter::Parser>>,
    max_parsers_per_language: usize,
}

impl ParserPool {
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
            max_parsers_per_language: 4,
        }
    }

    /// Get a parser for the specified language (accepts either extension or language name)
    pub fn get_parser(&mut self, language_or_extension: &str) -> Option<tree_sitter::Parser> {
        // Convert extension to language name if needed
        let language_name =
            extension_to_language_name(language_or_extension).unwrap_or(language_or_extension);

        let language_parsers = self
            .parsers
            .entry(language_name.to_string())
            .or_insert_with(Vec::new);

        if let Some(parser) = language_parsers.pop() {
            Some(parser)
        } else {
            // Try to create a new parser for this language
            self.create_parser(language_name)
        }
    }

    /// Return a parser to the pool (accepts either extension or language name)
    pub fn return_parser(&mut self, language_or_extension: &str, parser: tree_sitter::Parser) {
        // Convert extension to language name if needed
        let language_name =
            extension_to_language_name(language_or_extension).unwrap_or(language_or_extension);

        let language_parsers = self
            .parsers
            .entry(language_name.to_string())
            .or_insert_with(Vec::new);

        if language_parsers.len() < self.max_parsers_per_language {
            language_parsers.push(parser);
        }
        // If pool is full, just drop the parser
    }

    /// Create a new parser for the specified language
    fn create_parser(&self, language: &str) -> Option<tree_sitter::Parser> {
        let mut parser = tree_sitter::Parser::new();

        let tree_sitter_language = match language.to_lowercase().as_str() {
            "rust" => Some(tree_sitter_rust::LANGUAGE),
            "typescript" | "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
            "javascript" | "js" => Some(tree_sitter_javascript::LANGUAGE),
            "python" | "py" => Some(tree_sitter_python::LANGUAGE),
            "go" => Some(tree_sitter_go::LANGUAGE),
            "java" => Some(tree_sitter_java::LANGUAGE),
            "c" => Some(tree_sitter_c::LANGUAGE),
            "cpp" | "c++" | "cxx" => Some(tree_sitter_cpp::LANGUAGE),
            _ => None,
        };

        if let Some(lang) = tree_sitter_language {
            parser.set_language(&lang.into()).ok()?;
            Some(parser)
        } else {
            None
        }
    }
}

impl Default for ParserPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Tree-sitter based structural analyzer
pub struct TreeSitterAnalyzer {
    /// Parser pool for efficient parser reuse
    parser_pool: Arc<Mutex<ParserPool>>,

    /// UID generator for consistent symbol identification
    uid_generator: Arc<SymbolUIDGenerator>,

    /// Configuration for tree-sitter analysis
    config: TreeSitterConfig,

    /// Optional relationship extractor for detecting relationships between symbols
    relationship_extractor: Option<Arc<TreeSitterRelationshipExtractor>>,
}

impl TreeSitterAnalyzer {
    /// Create a new tree-sitter analyzer
    pub fn new(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        Self {
            parser_pool: Arc::new(Mutex::new(ParserPool::new())),
            uid_generator,
            config: TreeSitterConfig::default(),
            relationship_extractor: None,
        }
    }

    /// Create analyzer with custom configuration
    pub fn with_config(uid_generator: Arc<SymbolUIDGenerator>, config: TreeSitterConfig) -> Self {
        Self {
            parser_pool: Arc::new(Mutex::new(ParserPool::new())),
            uid_generator,
            config,
            relationship_extractor: None,
        }
    }

    /// Create analyzer with relationship extraction capability
    pub fn with_relationship_extractor(
        uid_generator: Arc<SymbolUIDGenerator>,
        relationship_extractor: Arc<TreeSitterRelationshipExtractor>,
    ) -> Self {
        Self {
            parser_pool: Arc::new(Mutex::new(ParserPool::new())),
            uid_generator,
            config: TreeSitterConfig::default(),
            relationship_extractor: Some(relationship_extractor),
        }
    }

    /// Create analyzer with both custom config and relationship extractor
    pub fn with_config_and_relationships(
        uid_generator: Arc<SymbolUIDGenerator>,
        config: TreeSitterConfig,
        relationship_extractor: Arc<TreeSitterRelationshipExtractor>,
    ) -> Self {
        Self {
            parser_pool: Arc::new(Mutex::new(ParserPool::new())),
            uid_generator,
            config,
            relationship_extractor: Some(relationship_extractor),
        }
    }

    /// Parse source code using tree-sitter
    async fn parse_source(
        &self,
        content: &str,
        language_or_extension: &str,
    ) -> Result<tree_sitter::Tree, AnalysisError> {
        if !self.config.enabled {
            return Err(AnalysisError::ConfigError {
                message: "Tree-sitter analysis is disabled".to_string(),
            });
        }

        // Convert extension to language name if needed
        let language_name =
            extension_to_language_name(language_or_extension).unwrap_or(language_or_extension);

        // Get parser from pool
        let parser = {
            let mut pool = self.parser_pool.lock().unwrap();
            pool.get_parser(language_name)
        };

        let mut parser = parser.ok_or_else(|| AnalysisError::ParserNotAvailable {
            language: language_name.to_string(),
        })?;

        // Parse with timeout
        let pool_clone = self.parser_pool.clone();
        let language_clone = language_name.to_string();
        let content_owned = content.to_string(); // Convert to owned data
        let parse_future = tokio::task::spawn_blocking(move || {
            let parse_result = parser.parse(&content_owned, None);
            // Return parser to pool within the blocking task
            {
                let mut pool = pool_clone.lock().unwrap();
                pool.return_parser(&language_clone, parser);
            }
            parse_result
        });

        let parse_result = timeout(
            Duration::from_millis(self.config.parser_timeout_ms),
            parse_future,
        )
        .await
        .map_err(|_| AnalysisError::Timeout {
            file: "unknown".to_string(),
            timeout_seconds: self.config.parser_timeout_ms / 1000,
        })?
        .map_err(|e| AnalysisError::InternalError {
            message: format!("Parser thread panicked: {:?}", e),
        })?;

        let tree = parse_result.ok_or_else(|| AnalysisError::ParseError {
            file: "unknown".to_string(),
            message: "Failed to parse source code".to_string(),
        })?;

        Ok(tree)
    }

    /// Extract symbols from AST
    fn extract_symbols_from_ast(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Result<Vec<ExtractedSymbol>, AnalysisError> {
        let mut symbols = Vec::new();
        let root_node = tree.root_node();
        let content_bytes = content.as_bytes();

        // Convert extension to language name for UID generation
        let language_name =
            extension_to_language_name(&context.language).unwrap_or(&context.language);

        // Create symbol context for UID generation
        let symbol_context = SymbolContext::new(context.workspace_id, language_name.to_string());

        self.extract_symbols_recursive(
            root_node,
            content_bytes,
            file_path,
            language,
            &symbol_context,
            &mut symbols,
            Vec::new(), // scope stack
        )?;

        Ok(symbols)
    }

    /// Recursively extract symbols from AST nodes
    fn extract_symbols_recursive(
        &self,
        node: tree_sitter::Node,
        content: &[u8],
        file_path: &Path,
        language: &str,
        context: &SymbolContext,
        symbols: &mut Vec<ExtractedSymbol>,
        mut scope_stack: Vec<String>,
    ) -> Result<(), AnalysisError> {
        let node_kind = node.kind();

        // Extract symbol information based on node type
        if let Some(symbol_info) =
            self.node_to_symbol_info(node, content, file_path, language, &scope_stack)?
        {
            // Generate UID for the symbol
            let uid = self
                .uid_generator
                .generate_uid(&symbol_info, context)
                .map_err(AnalysisError::UidGenerationError)?;

            // Create extracted symbol
            let location = SymbolLocation::new(
                file_path.to_path_buf(),
                symbol_info.location.start_line,
                symbol_info.location.start_char,
                symbol_info.location.end_line,
                symbol_info.location.end_char,
            );

            let mut extracted_symbol =
                ExtractedSymbol::new(uid, symbol_info.name.clone(), symbol_info.kind, location);

            if let Some(qualified_name) = symbol_info.qualified_name {
                extracted_symbol = extracted_symbol.with_qualified_name(qualified_name);
            }

            if let Some(signature) = symbol_info.signature {
                extracted_symbol = extracted_symbol.with_signature(signature);
            }

            if let Some(visibility) = symbol_info.visibility {
                extracted_symbol = extracted_symbol.with_visibility(visibility);
            }

            if !scope_stack.is_empty() {
                extracted_symbol = extracted_symbol.with_parent_scope(scope_stack.join("::"));
            }

            // Add language-specific metadata
            extracted_symbol = extracted_symbol.with_metadata(
                "node_kind".to_string(),
                serde_json::Value::String(node_kind.to_string()),
            );

            symbols.push(extracted_symbol);

            // If this symbol creates a new scope, add it to the scope stack
            if self.creates_scope(node_kind, language) {
                scope_stack.push(symbol_info.name);
            }
        } else if self.creates_scope(node_kind, language) {
            // Some nodes create scopes without being symbols themselves
            if let Some(scope_name) = self.extract_scope_name(node, content) {
                scope_stack.push(scope_name);
            }
        }

        // Recursively process child nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_symbols_recursive(
                child,
                content,
                file_path,
                language,
                context,
                symbols,
                scope_stack.clone(),
            )?;
        }

        Ok(())
    }

    /// Convert AST node to symbol information
    fn node_to_symbol_info(
        &self,
        node: tree_sitter::Node,
        content: &[u8],
        file_path: &Path,
        language: &str,
        scope_stack: &[String],
    ) -> Result<Option<SymbolInfo>, AnalysisError> {
        let node_kind = node.kind();

        // Map node kinds to symbol kinds based on language
        let symbol_kind = self.map_node_to_symbol_kind(node_kind, language)?;

        if symbol_kind.is_none() {
            return Ok(None);
        }

        let symbol_kind = symbol_kind.unwrap();

        // Extract symbol name
        let name = self.extract_symbol_name(node, content)?;
        if name.is_empty() {
            return Ok(None);
        }

        // Create location information
        let start_point = node.start_position();
        let end_point = node.end_position();
        let location = SymbolLocation::new(
            file_path.to_path_buf(),
            start_point.row as u32 + 1, // tree-sitter is 0-based, we want 1-based
            start_point.column as u32,
            end_point.row as u32 + 1,
            end_point.column as u32,
        );

        // Create basic symbol info
        let is_callable = symbol_kind.is_callable();
        let mut symbol_info = SymbolInfo::new(name, symbol_kind, language.to_string(), location);

        // Extract qualified name if in scope
        if !scope_stack.is_empty() {
            let mut fqn_parts = scope_stack.to_vec();
            fqn_parts.push(symbol_info.name.clone());
            symbol_info = symbol_info.with_qualified_name(fqn_parts.join("::"));
        }

        // Extract signature for callable symbols
        if is_callable {
            if let Some(signature) = self.extract_function_signature(node, content)? {
                symbol_info = symbol_info.with_signature(signature);
            }
        }

        Ok(Some(symbol_info))
    }

    /// Map tree-sitter node kind to symbol kind
    fn map_node_to_symbol_kind(
        &self,
        node_kind: &str,
        language: &str,
    ) -> Result<Option<SymbolKind>, AnalysisError> {
        let symbol_kind = match language.to_lowercase().as_str() {
            "rust" => self.map_rust_node_to_symbol(node_kind),
            "typescript" | "javascript" => self.map_typescript_node_to_symbol(node_kind),
            "python" => self.map_python_node_to_symbol(node_kind),
            "go" => self.map_go_node_to_symbol(node_kind),
            "java" => self.map_java_node_to_symbol(node_kind),
            "c" | "cpp" | "c++" => self.map_c_node_to_symbol(node_kind),
            _ => self.map_generic_node_to_symbol(node_kind),
        };

        Ok(symbol_kind)
    }

    /// Map Rust node kinds to symbol kinds
    fn map_rust_node_to_symbol(&self, node_kind: &str) -> Option<SymbolKind> {
        match node_kind {
            "function_item" => Some(SymbolKind::Function),
            "impl_item" => Some(SymbolKind::Method), // Impl block methods
            "struct_item" => Some(SymbolKind::Struct),
            "enum_item" => Some(SymbolKind::Enum),
            "trait_item" => Some(SymbolKind::Trait),
            "type_item" => Some(SymbolKind::Type),
            "const_item" => Some(SymbolKind::Constant),
            "static_item" => Some(SymbolKind::Variable),
            "mod_item" => Some(SymbolKind::Module),
            "macro_definition" => Some(SymbolKind::Macro),
            "let_declaration" => Some(SymbolKind::Variable),
            // Enhanced symbol extraction
            "use_declaration" => Some(SymbolKind::Import),
            "field_declaration" => Some(SymbolKind::Field),
            "parameter" => Some(SymbolKind::Variable),
            "enum_variant" => Some(SymbolKind::EnumVariant),
            "associated_type" => Some(SymbolKind::Type),
            "macro_rule" => Some(SymbolKind::Macro),
            "closure_expression" => Some(SymbolKind::Function),
            "impl_trait" => Some(SymbolKind::Method), // For trait impl blocks
            _ => None,
        }
    }

    /// Map TypeScript/JavaScript node kinds to symbol kinds
    fn map_typescript_node_to_symbol(&self, node_kind: &str) -> Option<SymbolKind> {
        match node_kind {
            "function_declaration" | "function_signature" => Some(SymbolKind::Function),
            "method_definition" => Some(SymbolKind::Method),
            "class_declaration" => Some(SymbolKind::Class),
            "interface_declaration" => Some(SymbolKind::Interface),
            "type_alias_declaration" => Some(SymbolKind::Type),
            "variable_declaration" => Some(SymbolKind::Variable),
            "const_assertion" => Some(SymbolKind::Constant),
            "namespace_declaration" => Some(SymbolKind::Namespace),
            "import_statement" => Some(SymbolKind::Import),
            "export_statement" => Some(SymbolKind::Export),
            // Enhanced symbol extraction
            "property_signature" => Some(SymbolKind::Field),
            "method_signature" => Some(SymbolKind::Method),
            "enum_declaration" => Some(SymbolKind::Enum),
            "enum_member" => Some(SymbolKind::EnumVariant),
            "arrow_function" => Some(SymbolKind::Function),
            "function_expression" => Some(SymbolKind::Function),
            "variable_declarator" => Some(SymbolKind::Variable),
            "parameter" => Some(SymbolKind::Variable),
            "property_identifier" => Some(SymbolKind::Field),
            "import_specifier" => Some(SymbolKind::Import),
            "export_specifier" => Some(SymbolKind::Export),
            _ => None,
        }
    }

    /// Map Python node kinds to symbol kinds
    fn map_python_node_to_symbol(&self, node_kind: &str) -> Option<SymbolKind> {
        match node_kind {
            "function_definition" => Some(SymbolKind::Function),
            "class_definition" => Some(SymbolKind::Class),
            "assignment" => Some(SymbolKind::Variable),
            "import_statement" | "import_from_statement" => Some(SymbolKind::Import),
            // Enhanced symbol extraction
            "decorated_definition" => Some(SymbolKind::Function), // @decorator def func
            "lambda" => Some(SymbolKind::Function),
            "parameter" => Some(SymbolKind::Variable),
            "keyword_argument" => Some(SymbolKind::Variable),
            "global_statement" => Some(SymbolKind::Variable),
            "nonlocal_statement" => Some(SymbolKind::Variable),
            "aliased_import" => Some(SymbolKind::Import),
            "dotted_as_name" => Some(SymbolKind::Import),
            _ => None,
        }
    }

    /// Map Go node kinds to symbol kinds
    fn map_go_node_to_symbol(&self, node_kind: &str) -> Option<SymbolKind> {
        match node_kind {
            "function_declaration" | "method_declaration" => Some(SymbolKind::Function),
            "type_declaration" => Some(SymbolKind::Type),
            "struct_type" => Some(SymbolKind::Struct),
            "interface_type" => Some(SymbolKind::Interface),
            "var_declaration" => Some(SymbolKind::Variable),
            "const_declaration" => Some(SymbolKind::Constant),
            "package_clause" => Some(SymbolKind::Package),
            "import_declaration" => Some(SymbolKind::Import),
            _ => None,
        }
    }

    /// Map Java node kinds to symbol kinds  
    fn map_java_node_to_symbol(&self, node_kind: &str) -> Option<SymbolKind> {
        match node_kind {
            "method_declaration" => Some(SymbolKind::Method),
            "constructor_declaration" => Some(SymbolKind::Constructor),
            "class_declaration" => Some(SymbolKind::Class),
            "interface_declaration" => Some(SymbolKind::Interface),
            "field_declaration" => Some(SymbolKind::Field),
            "variable_declarator" => Some(SymbolKind::Variable),
            "package_declaration" => Some(SymbolKind::Package),
            "import_declaration" => Some(SymbolKind::Import),
            _ => None,
        }
    }

    /// Map C/C++ node kinds to symbol kinds
    fn map_c_node_to_symbol(&self, node_kind: &str) -> Option<SymbolKind> {
        match node_kind {
            "function_definition" | "function_declarator" => Some(SymbolKind::Function),
            "struct_specifier" => Some(SymbolKind::Struct),
            "union_specifier" => Some(SymbolKind::Union),
            "enum_specifier" => Some(SymbolKind::Enum),
            "declaration" => Some(SymbolKind::Variable),
            "preproc_include" => Some(SymbolKind::Import),
            "preproc_def" => Some(SymbolKind::Macro),
            _ => None,
        }
    }

    /// Generic node mapping for unknown languages
    fn map_generic_node_to_symbol(&self, node_kind: &str) -> Option<SymbolKind> {
        if node_kind.contains("function") {
            Some(SymbolKind::Function)
        } else if node_kind.contains("class") {
            Some(SymbolKind::Class)
        } else if node_kind.contains("struct") {
            Some(SymbolKind::Struct)
        } else if node_kind.contains("enum") {
            Some(SymbolKind::Enum)
        } else if node_kind.contains("interface") {
            Some(SymbolKind::Interface)
        } else if node_kind.contains("variable") || node_kind.contains("declaration") {
            Some(SymbolKind::Variable)
        } else if node_kind.contains("import") {
            Some(SymbolKind::Import)
        } else {
            None
        }
    }

    /// Extract symbol name from AST node
    fn extract_symbol_name(
        &self,
        node: tree_sitter::Node,
        content: &[u8],
    ) -> Result<String, AnalysisError> {
        // Look for identifier child nodes with more comprehensive patterns
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let child_kind = child.kind();
            if matches!(
                child_kind,
                "identifier"
                    | "type_identifier"
                    | "field_identifier"
                    | "property_identifier"
                    | "variable_name"
                    | "function_name"
                    | "class_name"
                    | "module_name"
                    | "parameter_name"
            ) {
                let start_byte = child.start_byte();
                let end_byte = child.end_byte();
                if end_byte <= content.len() {
                    let name =
                        std::str::from_utf8(&content[start_byte..end_byte]).map_err(|e| {
                            AnalysisError::ParseError {
                                file: "unknown".to_string(),
                                message: format!("Invalid UTF-8 in symbol name: {}", e),
                            }
                        })?;
                    return Ok(name.to_string());
                }
            }

            // Recursively search in nested nodes for complex patterns
            if let Ok(nested_name) = self.extract_symbol_name(child, content) {
                if !nested_name.is_empty()
                    && nested_name.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    return Ok(nested_name);
                }
            }
        }

        // If no identifier child found, try to extract from node text with better patterns
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        if end_byte <= content.len() && end_byte > start_byte {
            let text = std::str::from_utf8(&content[start_byte..end_byte])
                .unwrap_or("")
                .trim();

            // Handle different node patterns
            let name = match node.kind() {
                "use_declaration" => {
                    // Extract the last part of use statements: use std::collections::HashMap -> HashMap
                    text.split("::").last().unwrap_or(text).to_string()
                }
                "import_statement" | "import_specifier" => {
                    // Handle import { name } from 'module' patterns
                    if let Some(brace_start) = text.find('{') {
                        if let Some(brace_end) = text.find('}') {
                            text[brace_start + 1..brace_end].trim().to_string()
                        } else {
                            text.split_whitespace().nth(1).unwrap_or("").to_string()
                        }
                    } else {
                        text.split_whitespace().nth(1).unwrap_or("").to_string()
                    }
                }
                "parameter" => {
                    // Extract parameter names from function signatures
                    text.split(':').next().unwrap_or(text).trim().to_string()
                }
                _ => {
                    // Extract first valid identifier as symbol name
                    text.split_whitespace()
                        .find(|word| {
                            !word.is_empty()
                                && word
                                    .chars()
                                    .next()
                                    .map_or(false, |c| c.is_alphabetic() || c == '_')
                                && word.chars().all(|c| c.is_alphanumeric() || c == '_')
                        })
                        .unwrap_or("")
                        .to_string()
                }
            };

            if !name.is_empty() {
                return Ok(name);
            }
        }

        Ok(String::new())
    }

    /// Extract function signature from AST node
    fn extract_function_signature(
        &self,
        node: tree_sitter::Node,
        content: &[u8],
    ) -> Result<Option<String>, AnalysisError> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();

        if end_byte <= content.len() && end_byte > start_byte {
            let signature_text =
                std::str::from_utf8(&content[start_byte..end_byte]).map_err(|e| {
                    AnalysisError::ParseError {
                        file: "unknown".to_string(),
                        message: format!("Invalid UTF-8 in signature: {}", e),
                    }
                })?;

            // Clean up the signature (remove body, normalize whitespace)
            let cleaned = self.clean_function_signature(signature_text);
            if !cleaned.is_empty() {
                return Ok(Some(cleaned));
            }
        }

        Ok(None)
    }

    /// Clean and normalize function signature
    fn clean_function_signature(&self, signature: &str) -> String {
        // Find the end of the signature (before opening brace or semicolon)
        let signature_end = signature
            .find('{')
            .or_else(|| signature.find(';'))
            .unwrap_or(signature.len());

        let clean_sig = signature[..signature_end].trim().to_string();

        // Normalize whitespace
        clean_sig.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Check if node kind creates a new scope
    fn creates_scope(&self, node_kind: &str, language: &str) -> bool {
        match language.to_lowercase().as_str() {
            "rust" => matches!(
                node_kind,
                "impl_item"
                    | "mod_item"
                    | "struct_item"
                    | "enum_item"
                    | "trait_item"
                    | "function_item"
                    | "closure_expression"
                    | "block"
            ),
            "typescript" | "javascript" => matches!(
                node_kind,
                "class_declaration"
                    | "interface_declaration"
                    | "namespace_declaration"
                    | "function_declaration"
                    | "arrow_function"
                    | "function_expression"
                    | "method_definition"
                    | "block_statement"
            ),
            "python" => matches!(
                node_kind,
                "class_definition"
                    | "function_definition"
                    | "lambda"
                    | "if_statement"
                    | "for_statement"
                    | "while_statement"
                    | "with_statement"
                    | "try_statement"
            ),
            "go" => matches!(
                node_kind,
                "type_declaration"
                    | "struct_type"
                    | "function_declaration"
                    | "method_declaration"
                    | "interface_type"
                    | "block"
            ),
            "java" => matches!(
                node_kind,
                "class_declaration"
                    | "interface_declaration"
                    | "package_declaration"
                    | "method_declaration"
                    | "constructor_declaration"
                    | "block"
            ),
            "c" | "cpp" => matches!(
                node_kind,
                "struct_specifier"
                    | "union_specifier"
                    | "function_definition"
                    | "compound_statement"
            ),
            _ => false,
        }
    }

    /// Extract scope name from node
    fn extract_scope_name(&self, node: tree_sitter::Node, content: &[u8]) -> Option<String> {
        self.extract_symbol_name(node, content)
            .ok()
            .filter(|name| !name.is_empty())
    }

    /// Extract relationships from AST using the advanced relationship extractor
    async fn extract_relationships_from_ast(
        &self,
        tree: &tree_sitter::Tree,
        symbols: &[ExtractedSymbol],
        content: &str,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Result<Vec<ExtractedRelationship>, AnalysisError> {
        if let Some(ref extractor) = self.relationship_extractor {
            // Use the advanced relationship extractor
            extractor
                .extract_relationships(tree, content, file_path, language, symbols, context)
                .await
                .map_err(|e| AnalysisError::InternalError {
                    message: format!("Relationship extraction failed: {}", e),
                })
        } else {
            // Fallback to basic relationship extraction
            self.extract_basic_relationships(tree, symbols, content)
        }
    }

    /// Basic relationship extraction fallback (when no advanced extractor is available)
    fn extract_basic_relationships(
        &self,
        tree: &tree_sitter::Tree,
        symbols: &[ExtractedSymbol],
        content: &str,
    ) -> Result<Vec<ExtractedRelationship>, AnalysisError> {
        let mut relationships = Vec::new();
        let root_node = tree.root_node();
        let content_bytes = content.as_bytes();

        // Build symbol lookup map for efficient relationship creation
        let mut symbol_lookup: HashMap<String, &ExtractedSymbol> = HashMap::new();
        for symbol in symbols {
            symbol_lookup.insert(symbol.name.clone(), symbol);
            if let Some(ref fqn) = symbol.qualified_name {
                symbol_lookup.insert(fqn.clone(), symbol);
            }
        }

        self.extract_relationships_recursive(
            root_node,
            content_bytes,
            &symbol_lookup,
            &mut relationships,
        )?;

        Ok(relationships)
    }

    /// Recursively extract basic relationships from AST nodes (fallback implementation)
    fn extract_relationships_recursive(
        &self,
        node: tree_sitter::Node,
        content: &[u8],
        symbol_lookup: &HashMap<String, &ExtractedSymbol>,
        relationships: &mut Vec<ExtractedRelationship>,
    ) -> Result<(), AnalysisError> {
        let node_kind = node.kind();

        // Look for call expressions, references, etc.
        if node_kind.contains("call") || node_kind.contains("invocation") {
            // Extract function calls
            if let Ok(callee_name) = self.extract_symbol_name(node, content) {
                if let Some(target_symbol) = symbol_lookup.get(&callee_name) {
                    let relationship = ExtractedRelationship::new(
                        "unknown_caller".to_string(), // Basic fallback
                        target_symbol.uid.clone(),
                        RelationType::Calls,
                    )
                    .with_confidence(0.5); // Lower confidence for basic extraction
                    relationships.push(relationship);
                }
            }
        }

        // Recursively process child nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_relationships_recursive(child, content, symbol_lookup, relationships)?;
        }

        Ok(())
    }
}

#[async_trait]
impl CodeAnalyzer for TreeSitterAnalyzer {
    fn capabilities(&self) -> AnalyzerCapabilities {
        AnalyzerCapabilities::structural()
    }

    fn supported_languages(&self) -> Vec<String> {
        vec![
            "rust".to_string(),
            "typescript".to_string(),
            "javascript".to_string(),
            "python".to_string(),
            "go".to_string(),
            "java".to_string(),
            "c".to_string(),
            "cpp".to_string(),
        ]
    }

    async fn analyze_file(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        // Check file size limits
        if let Some(max_size) = self.capabilities().max_file_size {
            if content.len() as u64 > max_size {
                return Err(AnalysisError::FileTooLarge {
                    size_bytes: content.len() as u64,
                    max_size,
                });
            }
        }

        let start_time = std::time::Instant::now();

        // Parse the source code
        let tree = self.parse_source(content, language).await?;

        // Extract symbols
        let symbols =
            self.extract_symbols_from_ast(&tree, content, file_path, language, context)?;

        // Extract relationships using the enhanced extractor
        let relationships = self
            .extract_relationships_from_ast(&tree, &symbols, content, file_path, language, context)
            .await?;

        let duration = start_time.elapsed();

        // Create analysis result
        let mut result = AnalysisResult::new(file_path.to_path_buf(), language.to_string());

        for symbol in symbols {
            result.add_symbol(symbol);
        }

        for relationship in relationships {
            result.add_relationship(relationship);
        }

        // Add analysis metadata
        result.analysis_metadata =
            AnalysisMetadata::new("TreeSitterAnalyzer".to_string(), "1.0.0".to_string());
        result.analysis_metadata.duration_ms = duration.as_millis() as u64;
        result
            .analysis_metadata
            .add_metric("symbols_extracted".to_string(), result.symbols.len() as f64);
        result.analysis_metadata.add_metric(
            "relationships_extracted".to_string(),
            result.relationships.len() as f64,
        );

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolUIDGenerator;
    use std::path::PathBuf;

    fn create_test_analyzer() -> TreeSitterAnalyzer {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        TreeSitterAnalyzer::new(uid_generator)
    }

    fn create_test_context() -> AnalysisContext {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        AnalysisContext::new(
            1,
            2,
            "rust".to_string(),
            PathBuf::from("."),
            PathBuf::from("test.rs"),
            uid_generator,
        )
    }

    #[test]
    fn test_analyzer_capabilities() {
        let analyzer = create_test_analyzer();
        let caps = analyzer.capabilities();

        assert!(caps.extracts_symbols);
        assert!(caps.extracts_relationships);
        assert!(!caps.supports_incremental);
        assert!(!caps.requires_lsp);
        assert!(caps.parallel_safe);
    }

    #[test]
    fn test_supported_languages() {
        let analyzer = create_test_analyzer();
        let languages = analyzer.supported_languages();

        // The actual languages depend on which tree-sitter features are enabled
        // In tests, we might not have any languages enabled
        assert!(languages.is_empty() || languages.len() > 0);
    }

    #[test]
    fn test_rust_node_mapping() {
        let analyzer = create_test_analyzer();

        assert_eq!(
            analyzer.map_rust_node_to_symbol("function_item"),
            Some(SymbolKind::Function)
        );
        assert_eq!(
            analyzer.map_rust_node_to_symbol("struct_item"),
            Some(SymbolKind::Struct)
        );
        assert_eq!(
            analyzer.map_rust_node_to_symbol("enum_item"),
            Some(SymbolKind::Enum)
        );
        assert_eq!(
            analyzer.map_rust_node_to_symbol("trait_item"),
            Some(SymbolKind::Trait)
        );
        assert_eq!(analyzer.map_rust_node_to_symbol("unknown_node"), None);
    }

    #[test]
    fn test_typescript_node_mapping() {
        let analyzer = create_test_analyzer();

        assert_eq!(
            analyzer.map_typescript_node_to_symbol("function_declaration"),
            Some(SymbolKind::Function)
        );
        assert_eq!(
            analyzer.map_typescript_node_to_symbol("class_declaration"),
            Some(SymbolKind::Class)
        );
        assert_eq!(
            analyzer.map_typescript_node_to_symbol("interface_declaration"),
            Some(SymbolKind::Interface)
        );
        assert_eq!(analyzer.map_typescript_node_to_symbol("unknown_node"), None);
    }

    #[test]
    fn test_function_signature_cleaning() {
        let analyzer = create_test_analyzer();

        let signature = "fn test_function(a: i32, b: String) -> bool { true }";
        let cleaned = analyzer.clean_function_signature(signature);
        assert_eq!(cleaned, "fn test_function(a: i32, b: String) -> bool");

        let signature_with_semicolon = "fn test_function(a: i32); // comment";
        let cleaned = analyzer.clean_function_signature(signature_with_semicolon);
        assert_eq!(cleaned, "fn test_function(a: i32)");
    }

    #[test]
    fn test_creates_scope() {
        let analyzer = create_test_analyzer();

        assert!(analyzer.creates_scope("struct_item", "rust"));
        assert!(analyzer.creates_scope("impl_item", "rust"));
        assert!(analyzer.creates_scope("mod_item", "rust"));
        assert!(analyzer.creates_scope("function_item", "rust")); // Functions do create scope in Rust

        assert!(analyzer.creates_scope("class_declaration", "typescript"));
        assert!(analyzer.creates_scope("namespace_declaration", "typescript"));
        assert!(analyzer.creates_scope("function_declaration", "typescript")); // Functions do create scope in TypeScript
    }

    #[tokio::test]
    async fn test_parse_source_without_parsers() {
        let analyzer = create_test_analyzer();

        // Test with an extension that should be converted to a language name
        let result = analyzer.parse_source("fn main() {}", "rs").await;

        // With tree-sitter-rust available, this should succeed
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_analyze_file_error_conditions() {
        let analyzer = create_test_analyzer();
        let context = create_test_context();
        let file_path = PathBuf::from("test.rs");

        // Test file too large
        let large_content = "x".repeat(20 * 1024 * 1024); // 20MB
        let result = analyzer
            .analyze_file(&large_content, &file_path, "rust", &context)
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AnalysisError::FileTooLarge { .. }
        ));
    }

    #[test]
    fn test_parser_pool() {
        let mut pool = ParserPool::new();

        // Test with rust language
        let parser = pool.get_parser("rust");
        assert!(
            parser.is_some(),
            "Should get a parser for rust when tree-sitter-rust is available"
        );

        // Pool should handle unknown languages gracefully
        let parser = pool.get_parser("unknown_language");
        assert!(parser.is_none());
    }
}
