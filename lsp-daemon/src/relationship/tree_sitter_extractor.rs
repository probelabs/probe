//! Core Tree-sitter Relationship Extractor
//!
//! This module provides the main TreeSitterRelationshipExtractor that coordinates
//! relationship detection using tree-sitter AST parsing and language-specific patterns.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::time::{timeout, Duration};

use super::structural_analyzer::StructuralAnalyzer;
use super::types::*;
use crate::analyzer::types::{
    AnalysisContext, ExtractedRelationship, ExtractedSymbol, RelationType,
};
use crate::symbol::SymbolUIDGenerator;

/// Tree-sitter parser pool for efficient parser reuse across relationship extraction
pub struct RelationshipParserPool {
    parsers: HashMap<String, Vec<tree_sitter::Parser>>,
    max_parsers_per_language: usize,
}

impl RelationshipParserPool {
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
            max_parsers_per_language: 2, // Fewer parsers for relationship extraction
        }
    }

    /// Borrow a parser for the specified language
    pub fn borrow_parser(&mut self, language: &str) -> Option<tree_sitter::Parser> {
        let language_parsers = self
            .parsers
            .entry(language.to_string())
            .or_insert_with(Vec::new);

        if let Some(parser) = language_parsers.pop() {
            Some(parser)
        } else {
            self.create_parser(language)
        }
    }

    /// Return a parser to the pool
    pub fn return_parser(&mut self, language: &str, parser: tree_sitter::Parser) {
        let language_parsers = self
            .parsers
            .entry(language.to_string())
            .or_insert_with(Vec::new);

        if language_parsers.len() < self.max_parsers_per_language {
            language_parsers.push(parser);
        }
    }

    fn create_parser(&self, language: &str) -> Option<tree_sitter::Parser> {
        let mut parser = tree_sitter::Parser::new();

        let tree_sitter_language = match language.to_lowercase().as_str() {
            #[cfg(feature = "tree-sitter-rust")]
            "rust" => Some(tree_sitter_rust::LANGUAGE),

            #[cfg(feature = "tree-sitter-typescript")]
            "typescript" | "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),

            #[cfg(feature = "tree-sitter-javascript")]
            "javascript" | "js" => Some(tree_sitter_javascript::LANGUAGE),

            #[cfg(feature = "tree-sitter-python")]
            "python" | "py" => Some(tree_sitter_python::LANGUAGE),

            #[cfg(feature = "tree-sitter-go")]
            "go" => Some(tree_sitter_go::LANGUAGE),

            #[cfg(feature = "tree-sitter-java")]
            "java" => Some(tree_sitter_java::LANGUAGE),

            #[cfg(feature = "tree-sitter-c")]
            "c" => Some(tree_sitter_c::LANGUAGE),

            #[cfg(feature = "tree-sitter-cpp")]
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

/// Registry for managing language-specific relationship patterns
pub struct PatternRegistry {
    patterns: HashMap<String, LanguagePatterns>,
}

impl PatternRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            patterns: HashMap::new(),
        };

        // Register built-in language patterns
        registry.register_rust_patterns();
        registry.register_typescript_patterns();
        registry.register_python_patterns();
        registry.register_generic_patterns();

        registry
    }

    /// Register language-specific patterns
    pub fn register_language_patterns(&mut self, language: &str, patterns: LanguagePatterns) {
        self.patterns.insert(language.to_lowercase(), patterns);
    }

    /// Get patterns for a specific language
    pub fn get_patterns(&self, language: &str) -> Option<&LanguagePatterns> {
        self.patterns.get(&language.to_lowercase())
    }

    /// Get patterns for a language with fallback to generic patterns
    pub fn get_patterns_with_fallback(&self, language: &str) -> &LanguagePatterns {
        self.patterns
            .get(&language.to_lowercase())
            .or_else(|| self.patterns.get("generic"))
            .expect("Generic patterns should always be available")
    }

    /// Register Rust-specific relationship patterns
    fn register_rust_patterns(&mut self) {
        let mut patterns = LanguagePatterns::new("rust".to_string());

        // Containment patterns
        patterns = patterns
            .add_containment_pattern(
                ContainmentPattern::new(
                    vec!["struct_item".to_string(), "enum_item".to_string()],
                    vec!["field_declaration".to_string()],
                    RelationType::Contains,
                )
                .with_confidence(1.0),
            )
            .add_containment_pattern(
                ContainmentPattern::new(
                    vec!["impl_item".to_string()],
                    vec!["function_item".to_string()],
                    RelationType::Contains,
                )
                .with_confidence(1.0),
            )
            .add_containment_pattern(
                ContainmentPattern::new(
                    vec!["mod_item".to_string()],
                    vec![
                        "function_item".to_string(),
                        "struct_item".to_string(),
                        "enum_item".to_string(),
                    ],
                    RelationType::Contains,
                )
                .with_confidence(1.0),
            );

        // Inheritance patterns
        patterns = patterns.add_inheritance_pattern(
            InheritancePattern::new(
                "(impl_item trait: (type_identifier) @trait)".to_string(),
                "(impl_item type: (type_identifier) @type)".to_string(),
                "impl".to_string(),
                RelationType::Implements,
            )
            .with_confidence(0.95),
        );

        // Call patterns
        patterns = patterns
            .add_call_pattern(
                CallPattern::new(vec!["call_expression".to_string()], "function".to_string())
                    .with_query("(call_expression function: (identifier) @function)".to_string()),
            )
            .add_call_pattern(
                CallPattern::new(
                    vec!["method_call_expression".to_string()],
                    "method".to_string(),
                )
                .with_receiver_field("object".to_string())
                .with_query(
                    "(method_call_expression object: (_) method: (field_identifier) @method)"
                        .to_string(),
                ),
            );

        // Import patterns
        patterns = patterns.add_import_pattern(ImportPattern::new(
            vec!["use_declaration".to_string()],
            "argument".to_string(),
            "(use_declaration argument: (scoped_identifier) @module)".to_string(),
        ));

        self.register_language_patterns("rust", patterns);
    }

    /// Register TypeScript-specific relationship patterns
    fn register_typescript_patterns(&mut self) {
        let mut patterns = LanguagePatterns::new("typescript".to_string());

        // Containment patterns
        patterns = patterns
            .add_containment_pattern(
                ContainmentPattern::new(
                    vec![
                        "class_declaration".to_string(),
                        "interface_declaration".to_string(),
                    ],
                    vec![
                        "method_definition".to_string(),
                        "field_definition".to_string(),
                    ],
                    RelationType::Contains,
                )
                .with_confidence(1.0),
            )
            .add_containment_pattern(
                ContainmentPattern::new(
                    vec![
                        "namespace_declaration".to_string(),
                        "module_declaration".to_string(),
                    ],
                    vec![
                        "class_declaration".to_string(),
                        "function_declaration".to_string(),
                        "interface_declaration".to_string(),
                    ],
                    RelationType::Contains,
                )
                .with_confidence(1.0),
            );

        // Inheritance patterns
        patterns = patterns
            .add_inheritance_pattern(
                InheritancePattern::new(
                    "(class_declaration superclass: (type_identifier) @superclass)".to_string(),
                    "(class_declaration name: (type_identifier) @class)".to_string(),
                    "extends".to_string(),
                    RelationType::InheritsFrom,
                )
                .with_confidence(0.98),
            )
            .add_inheritance_pattern(
                InheritancePattern::new(
                    "(class_declaration implements: (class_heritage (type_identifier) @interface))"
                        .to_string(),
                    "(class_declaration name: (type_identifier) @class)".to_string(),
                    "implements".to_string(),
                    RelationType::Implements,
                )
                .with_confidence(0.98),
            );

        // Call patterns
        patterns = patterns
            .add_call_pattern(
                CallPattern::new(
                    vec!["call_expression".to_string()],
                    "function".to_string(),
                ).with_query(
                    "(call_expression function: (identifier) @function)".to_string()
                )
            )
            .add_call_pattern(
                CallPattern::new(
                    vec!["call_expression".to_string()],
                    "property".to_string(),
                ).with_receiver_field("object".to_string())
                .with_query(
                    "(call_expression function: (member_expression object: (_) @object property: (property_identifier) @property))".to_string()
                )
            );

        // Import patterns
        patterns = patterns.add_import_pattern(
            ImportPattern::new(
                vec!["import_statement".to_string()],
                "source".to_string(),
                "(import_statement source: (string) @source)".to_string(),
            )
            .with_alias_field("import_clause".to_string()),
        );

        // Register for TypeScript
        self.register_language_patterns("typescript", patterns.clone());
        // Also register for JavaScript
        self.register_language_patterns("javascript", patterns);
    }

    /// Register Python-specific relationship patterns
    fn register_python_patterns(&mut self) {
        let mut patterns = LanguagePatterns::new("python".to_string());

        // Containment patterns
        patterns = patterns.add_containment_pattern(
            ContainmentPattern::new(
                vec!["class_definition".to_string()],
                vec!["function_definition".to_string()],
                RelationType::Contains,
            )
            .with_confidence(1.0),
        );

        // Inheritance patterns
        patterns = patterns.add_inheritance_pattern(
            InheritancePattern::new(
                "(class_definition superclasses: (argument_list (identifier) @superclass))"
                    .to_string(),
                "(class_definition name: (identifier) @class)".to_string(),
                "class".to_string(),
                RelationType::InheritsFrom,
            )
            .with_confidence(0.95),
        );

        // Call patterns
        patterns = patterns
            .add_call_pattern(
                CallPattern::new(
                    vec!["call".to_string()],
                    "function".to_string(),
                ).with_query(
                    "(call function: (identifier) @function)".to_string()
                )
            )
            .add_call_pattern(
                CallPattern::new(
                    vec!["call".to_string()],
                    "attribute".to_string(),
                ).with_receiver_field("object".to_string())
                .with_query(
                    "(call function: (attribute object: (_) @object attribute: (identifier) @attribute))".to_string()
                )
            );

        // Import patterns
        patterns = patterns.add_import_pattern(ImportPattern::new(
            vec![
                "import_statement".to_string(),
                "import_from_statement".to_string(),
            ],
            "name".to_string(),
            "(import_statement name: (dotted_name) @name)".to_string(),
        ));

        self.register_language_patterns("python", patterns);
    }

    /// Register generic patterns for unsupported languages
    fn register_generic_patterns(&mut self) {
        let mut patterns = LanguagePatterns::new("generic".to_string());

        // Basic containment patterns based on common node names
        patterns = patterns.add_containment_pattern(
            ContainmentPattern::new(
                vec![
                    "function".to_string(),
                    "method".to_string(),
                    "class".to_string(),
                    "struct".to_string(),
                ],
                vec!["statement".to_string(), "declaration".to_string()],
                RelationType::Contains,
            )
            .with_confidence(0.6),
        );

        // Basic call patterns
        patterns = patterns.add_call_pattern(
            CallPattern::new(
                vec!["call".to_string(), "invocation".to_string()],
                "name".to_string(),
            )
            .with_confidence(0.7),
        );

        self.register_language_patterns("generic", patterns);
    }
}

/// Main Tree-sitter relationship extractor
pub struct TreeSitterRelationshipExtractor {
    /// Parser pool for efficient parser reuse
    parser_pool: Arc<Mutex<RelationshipParserPool>>,

    /// Pattern registry for language-specific relationship detection
    pattern_registry: Arc<PatternRegistry>,

    /// UID generator for consistent symbol identification
    uid_generator: Arc<SymbolUIDGenerator>,

    /// Structural analyzer for pattern-based extraction
    structural_analyzer: StructuralAnalyzer,

    /// Configuration for relationship extraction
    config: RelationshipExtractionConfig,
}

impl TreeSitterRelationshipExtractor {
    /// Create a new relationship extractor
    pub fn new(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        let pattern_registry = Arc::new(PatternRegistry::new());

        Self {
            parser_pool: Arc::new(Mutex::new(RelationshipParserPool::new())),
            pattern_registry: pattern_registry.clone(),
            uid_generator,
            structural_analyzer: StructuralAnalyzer::new(pattern_registry),
            config: RelationshipExtractionConfig::default(),
        }
    }

    /// Create extractor with custom configuration
    pub fn with_config(
        uid_generator: Arc<SymbolUIDGenerator>,
        config: RelationshipExtractionConfig,
    ) -> Self {
        let pattern_registry = Arc::new(PatternRegistry::new());

        Self {
            parser_pool: Arc::new(Mutex::new(RelationshipParserPool::new())),
            pattern_registry: pattern_registry.clone(),
            uid_generator,
            structural_analyzer: StructuralAnalyzer::new(pattern_registry),
            config,
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &RelationshipExtractionConfig {
        &self.config
    }

    /// Extract all relationships from a parsed tree
    pub async fn extract_relationships(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        file_path: &Path,
        language: &str,
        symbols: &[ExtractedSymbol],
        _context: &AnalysisContext,
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        let mut all_relationships = Vec::new();

        // Extract containment relationships
        if self.config.extract_containment {
            let containment = self
                .extract_containment_relationships(tree, symbols)
                .await?;
            all_relationships.extend(containment);
        }

        // Extract inheritance relationships
        if self.config.extract_inheritance {
            let inheritance = self
                .extract_inheritance_relationships(tree, content, language, symbols)
                .await?;
            all_relationships.extend(inheritance);
        }

        // Extract call relationships
        if self.config.extract_calls {
            let calls = self
                .extract_call_relationships(tree, content, language, symbols)
                .await?;
            all_relationships.extend(calls);
        }

        // Extract import relationships
        if self.config.extract_imports {
            let imports = self
                .extract_import_relationships(tree, content, file_path, language)
                .await?;
            all_relationships.extend(imports);
        }

        // Filter by confidence threshold
        let filtered: Vec<ExtractedRelationship> = all_relationships
            .into_iter()
            .filter(|rel| self.config.meets_confidence_threshold(rel.confidence))
            .collect();

        Ok(filtered)
    }

    /// Extract containment relationships (parent-child relationships)
    pub async fn extract_containment_relationships(
        &self,
        tree: &tree_sitter::Tree,
        symbols: &[ExtractedSymbol],
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        self.structural_analyzer
            .extract_containment_relationships(tree, symbols)
            .await
    }

    /// Extract inheritance relationships
    pub async fn extract_inheritance_relationships(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        language: &str,
        symbols: &[ExtractedSymbol],
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        self.structural_analyzer
            .extract_inheritance_relationships(tree, content, language, symbols)
            .await
    }

    /// Extract call relationships
    pub async fn extract_call_relationships(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        language: &str,
        symbols: &[ExtractedSymbol],
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        self.structural_analyzer
            .extract_call_relationships(tree, content, language, symbols)
            .await
    }

    /// Extract import relationships
    pub async fn extract_import_relationships(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        file_path: &Path,
        language: &str,
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        self.structural_analyzer
            .extract_import_relationships(tree, content, file_path, language)
            .await
    }

    /// Parse content with timeout protection
    async fn parse_with_timeout(
        &self,
        content: &str,
        language: &str,
    ) -> RelationshipResult<tree_sitter::Tree> {
        let parser = {
            let mut pool =
                self.parser_pool
                    .lock()
                    .map_err(|e| RelationshipError::InternalError {
                        message: format!("Parser pool lock error: {}", e),
                    })?;
            pool.borrow_parser(language)
        };

        let mut parser = parser.ok_or_else(|| RelationshipError::ParserNotAvailable {
            language: language.to_string(),
        })?;

        let pool_clone = self.parser_pool.clone();
        let language_clone = language.to_string();
        let content_owned = content.to_string();

        let parse_future = tokio::task::spawn_blocking(move || {
            let result = parser.parse(&content_owned, None);
            // Return parser to pool
            {
                let mut pool = pool_clone.lock().unwrap();
                pool.return_parser(&language_clone, parser);
            }
            result
        });

        let parse_result = timeout(Duration::from_secs(30), parse_future)
            .await
            .map_err(|_| RelationshipError::InternalError {
                message: "Parse timeout".to_string(),
            })?
            .map_err(|e| RelationshipError::InternalError {
                message: format!("Parse task failed: {:?}", e),
            })?;

        parse_result.ok_or_else(|| {
            RelationshipError::TreeSitterError("Failed to parse source code".to_string())
        })
    }

    /// Convert relationship candidates to extracted relationships
    pub async fn resolve_relationship_candidates(
        &self,
        candidates: Vec<RelationshipCandidate>,
        symbols: &[ExtractedSymbol],
        _context: &AnalysisContext,
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        let mut relationships = Vec::new();

        // Build symbol lookup maps for efficient resolution
        let mut name_lookup: HashMap<String, &ExtractedSymbol> = HashMap::new();
        let mut fqn_lookup: HashMap<String, &ExtractedSymbol> = HashMap::new();

        for symbol in symbols {
            name_lookup.insert(symbol.name.clone(), symbol);
            if let Some(ref fqn) = symbol.qualified_name {
                fqn_lookup.insert(fqn.clone(), symbol);
            }
        }

        for candidate in candidates {
            if let (Some(source_uid), Some(target_uid)) = (
                self.resolve_symbol_identifier(
                    &candidate.source_identifier,
                    &name_lookup,
                    &fqn_lookup,
                ),
                self.resolve_symbol_identifier(
                    &candidate.target_identifier,
                    &name_lookup,
                    &fqn_lookup,
                ),
            ) {
                let mut relationship =
                    ExtractedRelationship::new(source_uid, target_uid, candidate.relationship_type)
                        .with_confidence(candidate.confidence);

                if let Some(location) = candidate.location {
                    relationship = relationship.with_location(location);
                }

                // Add metadata
                for (key, value) in candidate.metadata {
                    relationship =
                        relationship.with_metadata(key, serde_json::Value::String(value));
                }

                relationships.push(relationship);
            }
        }

        Ok(relationships)
    }

    /// Resolve a symbol identifier to a UID
    fn resolve_symbol_identifier(
        &self,
        identifier: &SymbolIdentifier,
        name_lookup: &HashMap<String, &ExtractedSymbol>,
        fqn_lookup: &HashMap<String, &ExtractedSymbol>,
    ) -> Option<String> {
        match identifier {
            SymbolIdentifier::Uid(uid) => Some(uid.clone()),
            SymbolIdentifier::Name(name) => name_lookup.get(name).map(|symbol| symbol.uid.clone()),
            SymbolIdentifier::QualifiedName(fqn) => {
                fqn_lookup.get(fqn).map(|symbol| symbol.uid.clone())
            }
            SymbolIdentifier::Position { .. } => {
                // TODO: Implement position-based symbol lookup
                None
            }
            SymbolIdentifier::Node { text, .. } => {
                // Try to match by symbol text
                name_lookup.get(text).map(|symbol| symbol.uid.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{SymbolKind, SymbolLocation};
    use std::path::PathBuf;

    fn create_test_extractor() -> TreeSitterRelationshipExtractor {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        TreeSitterRelationshipExtractor::new(uid_generator)
    }

    fn create_test_symbols() -> Vec<ExtractedSymbol> {
        vec![
            ExtractedSymbol::new(
                "rust::test::struct_field".to_string(),
                "field1".to_string(),
                SymbolKind::Field,
                SymbolLocation::new(PathBuf::from("test.rs"), 2, 4, 2, 10),
            ),
            ExtractedSymbol::new(
                "rust::test::function_call".to_string(),
                "test_fn".to_string(),
                SymbolKind::Function,
                SymbolLocation::new(PathBuf::from("test.rs"), 5, 0, 7, 1),
            ),
        ]
    }

    #[test]
    fn test_extractor_creation() {
        let extractor = create_test_extractor();

        assert_eq!(extractor.config.max_depth, 10);
        assert_eq!(extractor.config.min_confidence, 0.5);
        assert!(extractor.config.extract_containment);
        assert!(extractor.config.extract_inheritance);
        assert!(extractor.config.extract_calls);
        assert!(extractor.config.extract_imports);
    }

    #[test]
    fn test_pattern_registry_creation() {
        let registry = PatternRegistry::new();

        // Check that language patterns are registered
        assert!(registry.get_patterns("rust").is_some());
        assert!(registry.get_patterns("typescript").is_some());
        assert!(registry.get_patterns("javascript").is_some());
        assert!(registry.get_patterns("python").is_some());
        assert!(registry.get_patterns("generic").is_some());
    }

    #[test]
    fn test_rust_patterns_structure() {
        let registry = PatternRegistry::new();
        let rust_patterns = registry.get_patterns("rust").unwrap();

        assert!(!rust_patterns.containment_patterns.is_empty());
        assert!(!rust_patterns.inheritance_patterns.is_empty());
        assert!(!rust_patterns.call_patterns.is_empty());
        assert!(!rust_patterns.import_patterns.is_empty());

        // Check specific patterns
        let containment_matches =
            rust_patterns.get_containment_patterns_for_node("struct_item", "field_declaration");
        assert_eq!(containment_matches.len(), 1);

        let call_matches = rust_patterns.get_call_patterns_for_node("call_expression");
        assert_eq!(call_matches.len(), 1);
    }

    #[test]
    fn test_symbol_identifier_resolution() {
        let extractor = create_test_extractor();
        let symbols = create_test_symbols();

        // Build lookup maps
        let mut name_lookup: HashMap<String, &ExtractedSymbol> = HashMap::new();
        let mut fqn_lookup: HashMap<String, &ExtractedSymbol> = HashMap::new();

        for symbol in &symbols {
            name_lookup.insert(symbol.name.clone(), symbol);
            if let Some(ref fqn) = symbol.qualified_name {
                fqn_lookup.insert(fqn.clone(), symbol);
            }
        }

        // Test name-based resolution
        let name_id = SymbolIdentifier::name("field1".to_string());
        let resolved_uid = extractor.resolve_symbol_identifier(&name_id, &name_lookup, &fqn_lookup);
        assert_eq!(resolved_uid, Some("rust::test::struct_field".to_string()));

        // Test UID-based resolution
        let uid_id = SymbolIdentifier::Uid("direct_uid".to_string());
        let resolved_uid = extractor.resolve_symbol_identifier(&uid_id, &name_lookup, &fqn_lookup);
        assert_eq!(resolved_uid, Some("direct_uid".to_string()));

        // Test unknown symbol
        let unknown_id = SymbolIdentifier::name("unknown_symbol".to_string());
        let resolved_uid =
            extractor.resolve_symbol_identifier(&unknown_id, &name_lookup, &fqn_lookup);
        assert_eq!(resolved_uid, None);
    }

    #[tokio::test]
    async fn test_relationship_candidate_resolution() {
        let extractor = create_test_extractor();
        let symbols = create_test_symbols();
        let context = AnalysisContext::new(
            1,
            2,
            3,
            "rust".to_string(),
            Arc::new(SymbolUIDGenerator::new()),
        );

        let candidates = vec![RelationshipCandidate::new(
            SymbolIdentifier::name("field1".to_string()),
            SymbolIdentifier::name("test_fn".to_string()),
            RelationType::References,
        )
        .with_confidence(0.8)];

        let relationships = extractor
            .resolve_relationship_candidates(candidates, &symbols, &context)
            .await
            .unwrap();

        assert_eq!(relationships.len(), 1);
        assert_eq!(
            relationships[0].source_symbol_uid,
            "rust::test::struct_field"
        );
        assert_eq!(
            relationships[0].target_symbol_uid,
            "rust::test::function_call"
        );
        assert_eq!(relationships[0].relation_type, RelationType::References);
        assert_eq!(relationships[0].confidence, 0.8);
    }

    #[test]
    fn test_parser_pool_operations() {
        let mut pool = RelationshipParserPool::new();

        // Try to borrow a parser (should return None in test environment without features)
        let parser = pool.borrow_parser("rust");
        assert!(parser.is_none());

        // Pool should handle unknown languages gracefully
        let parser = pool.borrow_parser("unknown_language");
        assert!(parser.is_none());
    }

    #[test]
    fn test_extraction_config_filtering() {
        let config = RelationshipExtractionConfig::performance();
        assert_eq!(config.max_depth, 5);
        assert_eq!(config.min_confidence, 0.7);

        assert!(config.should_extract_type(RelationType::Calls));
        assert!(config.meets_confidence_threshold(0.8));
        assert!(!config.meets_confidence_threshold(0.5));
    }
}
