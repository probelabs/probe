//! Relationship Types and Data Structures
//!
//! This module defines the core types used by the Tree-sitter relationship extractor
//! for representing different types of relationships between symbols and the patterns
//! used to detect them.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use crate::analyzer::types::RelationType;
use crate::symbol::SymbolLocation;

/// Error types for relationship extraction
#[derive(Debug, Error)]
pub enum RelationshipError {
    #[error("Parser not available for language: {language}")]
    ParserNotAvailable { language: String },

    #[error("Query compilation failed: {query} - {error}")]
    QueryCompilationError { query: String, error: String },

    #[error("Pattern matching failed: {pattern} - {error}")]
    PatternMatchingError { pattern: String, error: String },

    #[error("Symbol resolution failed: {symbol} - {error}")]
    SymbolResolutionError { symbol: String, error: String },

    #[error("Invalid relationship configuration: {message}")]
    ConfigurationError { message: String },

    #[error("Tree-sitter error: {0}")]
    TreeSitterError(String),

    #[error("Internal extraction error: {message}")]
    InternalError { message: String },
}

/// Result type for relationship extraction operations
pub type RelationshipResult<T> = Result<T, RelationshipError>;

/// Represents a pattern for detecting containment relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainmentPattern {
    /// Node types that can contain other symbols
    pub parent_node_types: Vec<String>,

    /// Node types that can be contained
    pub child_node_types: Vec<String>,

    /// The type of relationship this pattern represents
    pub relationship_type: RelationType,

    /// Confidence level of this pattern (0.0 to 1.0)
    pub confidence: f32,

    /// Optional tree-sitter query for more precise matching
    pub query: Option<String>,
}

impl ContainmentPattern {
    pub fn new(
        parent_types: Vec<String>,
        child_types: Vec<String>,
        relationship_type: RelationType,
    ) -> Self {
        Self {
            parent_node_types: parent_types,
            child_node_types: child_types,
            relationship_type,
            confidence: 1.0,
            query: None,
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn with_query(mut self, query: String) -> Self {
        self.query = Some(query);
        self
    }

    /// Check if this pattern matches the given parent and child node types
    pub fn matches(&self, parent_type: &str, child_type: &str) -> bool {
        self.parent_node_types.contains(&parent_type.to_string())
            && self.child_node_types.contains(&child_type.to_string())
    }
}

/// Represents a pattern for detecting inheritance relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InheritancePattern {
    /// Tree-sitter query to find base types/classes
    pub base_node_query: String,

    /// Tree-sitter query to find derived types/classes
    pub derived_node_query: String,

    /// Language keyword used for inheritance (e.g., "extends", "implements")
    pub inheritance_keyword: String,

    /// The type of relationship this pattern represents
    pub relationship_type: RelationType,

    /// Confidence level of this pattern
    pub confidence: f32,
}

impl InheritancePattern {
    pub fn new(
        base_query: String,
        derived_query: String,
        keyword: String,
        relationship_type: RelationType,
    ) -> Self {
        Self {
            base_node_query: base_query,
            derived_node_query: derived_query,
            inheritance_keyword: keyword,
            relationship_type,
            confidence: 0.95,
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

/// Represents a pattern for detecting call relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallPattern {
    /// Node types that represent function/method calls
    pub call_node_types: Vec<String>,

    /// Field name in the AST node that contains the function identifier
    pub function_identifier_field: String,

    /// Optional receiver/object field for method calls
    pub receiver_field: Option<String>,

    /// Confidence level of this pattern
    pub confidence: f32,

    /// Tree-sitter query for more precise call detection
    pub query: Option<String>,
}

impl CallPattern {
    pub fn new(call_types: Vec<String>, identifier_field: String) -> Self {
        Self {
            call_node_types: call_types,
            function_identifier_field: identifier_field,
            receiver_field: None,
            confidence: 0.9,
            query: None,
        }
    }

    pub fn with_receiver_field(mut self, field: String) -> Self {
        self.receiver_field = Some(field);
        self
    }

    pub fn with_query(mut self, query: String) -> Self {
        self.query = Some(query);
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn matches(&self, node_type: &str) -> bool {
        self.call_node_types.contains(&node_type.to_string())
    }
}

/// Represents a pattern for detecting import/dependency relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPattern {
    /// Node types that represent imports/includes
    pub import_node_types: Vec<String>,

    /// Field name that contains the imported module/file name
    pub module_field: String,

    /// Optional field for alias/as names
    pub alias_field: Option<String>,

    /// Tree-sitter query for extracting import information
    pub query: String,

    /// Whether this represents a relative or absolute import
    pub is_relative: Option<bool>,
}

impl ImportPattern {
    pub fn new(import_types: Vec<String>, module_field: String, query: String) -> Self {
        Self {
            import_node_types: import_types,
            module_field,
            alias_field: None,
            query,
            is_relative: None,
        }
    }

    pub fn with_alias_field(mut self, field: String) -> Self {
        self.alias_field = Some(field);
        self
    }

    pub fn matches(&self, node_type: &str) -> bool {
        self.import_node_types.contains(&node_type.to_string())
    }
}

/// Collection of patterns for a specific programming language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagePatterns {
    /// Language identifier
    pub language: String,

    /// Patterns for detecting containment relationships
    pub containment_patterns: Vec<ContainmentPattern>,

    /// Patterns for detecting inheritance relationships
    pub inheritance_patterns: Vec<InheritancePattern>,

    /// Patterns for detecting call relationships
    pub call_patterns: Vec<CallPattern>,

    /// Patterns for detecting import relationships
    pub import_patterns: Vec<ImportPattern>,
}

impl LanguagePatterns {
    pub fn new(language: String) -> Self {
        Self {
            language,
            containment_patterns: Vec::new(),
            inheritance_patterns: Vec::new(),
            call_patterns: Vec::new(),
            import_patterns: Vec::new(),
        }
    }

    /// Add a containment pattern
    pub fn add_containment_pattern(mut self, pattern: ContainmentPattern) -> Self {
        self.containment_patterns.push(pattern);
        self
    }

    /// Add an inheritance pattern
    pub fn add_inheritance_pattern(mut self, pattern: InheritancePattern) -> Self {
        self.inheritance_patterns.push(pattern);
        self
    }

    /// Add a call pattern
    pub fn add_call_pattern(mut self, pattern: CallPattern) -> Self {
        self.call_patterns.push(pattern);
        self
    }

    /// Add an import pattern
    pub fn add_import_pattern(mut self, pattern: ImportPattern) -> Self {
        self.import_patterns.push(pattern);
        self
    }

    /// Get all patterns that match a given node type for containment
    pub fn get_containment_patterns_for_node(
        &self,
        parent_type: &str,
        child_type: &str,
    ) -> Vec<&ContainmentPattern> {
        self.containment_patterns
            .iter()
            .filter(|p| p.matches(parent_type, child_type))
            .collect()
    }

    /// Get all call patterns that match a given node type
    pub fn get_call_patterns_for_node(&self, node_type: &str) -> Vec<&CallPattern> {
        self.call_patterns
            .iter()
            .filter(|p| p.matches(node_type))
            .collect()
    }

    /// Get all import patterns that match a given node type
    pub fn get_import_patterns_for_node(&self, node_type: &str) -> Vec<&ImportPattern> {
        self.import_patterns
            .iter()
            .filter(|p| p.matches(node_type))
            .collect()
    }
}

/// Intermediate representation of a relationship being extracted
#[derive(Debug, Clone)]
pub struct RelationshipCandidate {
    /// Source symbol identifier or position information
    pub source_identifier: SymbolIdentifier,

    /// Target symbol identifier or position information
    pub target_identifier: SymbolIdentifier,

    /// Type of relationship
    pub relationship_type: RelationType,

    /// Location where the relationship is expressed
    pub location: Option<SymbolLocation>,

    /// Confidence level (0.0 to 1.0)
    pub confidence: f32,

    /// Additional metadata about the relationship
    pub metadata: HashMap<String, String>,
}

impl RelationshipCandidate {
    pub fn new(
        source: SymbolIdentifier,
        target: SymbolIdentifier,
        relationship_type: RelationType,
    ) -> Self {
        Self {
            source_identifier: source,
            target_identifier: target,
            relationship_type,
            location: None,
            confidence: 1.0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_location(mut self, location: SymbolLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Represents different ways to identify a symbol during relationship extraction
#[derive(Debug, Clone)]
pub enum SymbolIdentifier {
    /// Direct UID reference (if already known)
    Uid(String),

    /// Name-based lookup (simple name)
    Name(String),

    /// Fully qualified name lookup
    QualifiedName(String),

    /// Position-based lookup (for anonymous symbols)
    Position {
        file_path: std::path::PathBuf,
        line: u32,
        column: u32,
    },

    /// Node-based lookup (with AST context)
    Node {
        node_kind: String,
        text: String,
        start_byte: usize,
        end_byte: usize,
    },
}

impl SymbolIdentifier {
    /// Create a name-based identifier
    pub fn name(name: String) -> Self {
        Self::Name(name)
    }

    /// Create a qualified name identifier
    pub fn qualified_name(fqn: String) -> Self {
        Self::QualifiedName(fqn)
    }

    /// Create a position-based identifier
    pub fn position(file_path: std::path::PathBuf, line: u32, column: u32) -> Self {
        Self::Position {
            file_path,
            line,
            column,
        }
    }

    /// Create a node-based identifier
    pub fn node(node_kind: String, text: String, start_byte: usize, end_byte: usize) -> Self {
        Self::Node {
            node_kind,
            text,
            start_byte,
            end_byte,
        }
    }

    /// Get a human-readable string representation
    pub fn to_string(&self) -> String {
        match self {
            Self::Uid(uid) => uid.clone(),
            Self::Name(name) => name.clone(),
            Self::QualifiedName(fqn) => fqn.clone(),
            Self::Position {
                file_path,
                line,
                column,
            } => {
                format!("{}:{}:{}", file_path.display(), line, column)
            }
            Self::Node { text, .. } => text.clone(),
        }
    }
}

/// Configuration for relationship extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipExtractionConfig {
    /// Maximum depth for recursive relationship extraction
    pub max_depth: u32,

    /// Minimum confidence threshold for including relationships
    pub min_confidence: f32,

    /// Whether to extract cross-file relationships
    pub extract_cross_file: bool,

    /// Whether to extract call relationships
    pub extract_calls: bool,

    /// Whether to extract inheritance relationships
    pub extract_inheritance: bool,

    /// Whether to extract containment relationships
    pub extract_containment: bool,

    /// Whether to extract import relationships
    pub extract_imports: bool,

    /// Language-specific configurations
    pub language_configs: HashMap<String, serde_json::Value>,
}

impl Default for RelationshipExtractionConfig {
    fn default() -> Self {
        Self {
            max_depth: 10,
            min_confidence: 0.5,
            extract_cross_file: true,
            extract_calls: true,
            extract_inheritance: true,
            extract_containment: true,
            extract_imports: true,
            language_configs: HashMap::new(),
        }
    }
}

impl RelationshipExtractionConfig {
    /// Create a configuration optimized for performance
    pub fn performance() -> Self {
        Self {
            max_depth: 5,
            min_confidence: 0.7,
            extract_cross_file: false,
            ..Default::default()
        }
    }

    /// Create a configuration optimized for completeness
    pub fn completeness() -> Self {
        Self {
            max_depth: 20,
            min_confidence: 0.3,
            extract_cross_file: true,
            ..Default::default()
        }
    }

    /// Check if a relationship type should be extracted
    pub fn should_extract_type(&self, relation_type: RelationType) -> bool {
        match relation_type {
            RelationType::Calls | RelationType::CalledBy => self.extract_calls,
            RelationType::InheritsFrom | RelationType::ExtendedBy | RelationType::Implements => {
                self.extract_inheritance
            }
            RelationType::Contains => self.extract_containment,
            RelationType::Imports => self.extract_imports,
            _ => true, // Extract other types by default
        }
    }

    /// Check if a relationship meets the confidence threshold
    pub fn meets_confidence_threshold(&self, confidence: f32) -> bool {
        confidence >= self.min_confidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_containment_pattern_matching() {
        let pattern = ContainmentPattern::new(
            vec!["struct_item".to_string()],
            vec!["field_declaration".to_string()],
            RelationType::Contains,
        );

        assert!(pattern.matches("struct_item", "field_declaration"));
        assert!(!pattern.matches("enum_item", "field_declaration"));
        assert!(!pattern.matches("struct_item", "method_declaration"));
    }

    #[test]
    fn test_call_pattern_matching() {
        let pattern = CallPattern::new(
            vec![
                "call_expression".to_string(),
                "method_invocation".to_string(),
            ],
            "function".to_string(),
        );

        assert!(pattern.matches("call_expression"));
        assert!(pattern.matches("method_invocation"));
        assert!(!pattern.matches("function_declaration"));
    }

    #[test]
    fn test_language_patterns_filtering() {
        let mut patterns = LanguagePatterns::new("rust".to_string());

        let containment = ContainmentPattern::new(
            vec!["struct_item".to_string()],
            vec!["field_declaration".to_string()],
            RelationType::Contains,
        );
        patterns = patterns.add_containment_pattern(containment);

        let call = CallPattern::new(vec!["call_expression".to_string()], "function".to_string());
        patterns = patterns.add_call_pattern(call);

        let containment_matches =
            patterns.get_containment_patterns_for_node("struct_item", "field_declaration");
        assert_eq!(containment_matches.len(), 1);

        let call_matches = patterns.get_call_patterns_for_node("call_expression");
        assert_eq!(call_matches.len(), 1);

        let no_matches = patterns.get_call_patterns_for_node("unknown_node");
        assert_eq!(no_matches.len(), 0);
    }

    #[test]
    fn test_symbol_identifier_creation() {
        let name_id = SymbolIdentifier::name("test_function".to_string());
        assert_eq!(name_id.to_string(), "test_function");

        let pos_id = SymbolIdentifier::position(PathBuf::from("test.rs"), 10, 5);
        assert!(pos_id.to_string().contains("test.rs"));
        assert!(pos_id.to_string().contains("10:5"));

        let node_id = SymbolIdentifier::node(
            "function_item".to_string(),
            "fn test() {}".to_string(),
            0,
            12,
        );
        assert_eq!(node_id.to_string(), "fn test() {}");
    }

    #[test]
    fn test_relationship_candidate_builder() {
        let source = SymbolIdentifier::name("caller".to_string());
        let target = SymbolIdentifier::name("callee".to_string());

        let candidate = RelationshipCandidate::new(source, target, RelationType::Calls)
            .with_confidence(0.95)
            .with_metadata("context".to_string(), "function_body".to_string());

        assert_eq!(candidate.confidence, 0.95);
        assert_eq!(
            candidate.metadata.get("context"),
            Some(&"function_body".to_string())
        );
    }

    #[test]
    fn test_extraction_config_type_filtering() {
        let config = RelationshipExtractionConfig::default();
        assert!(config.should_extract_type(RelationType::Calls));
        assert!(config.should_extract_type(RelationType::InheritsFrom));
        assert!(config.should_extract_type(RelationType::Contains));

        let perf_config = RelationshipExtractionConfig::performance();
        assert_eq!(perf_config.max_depth, 5);
        assert_eq!(perf_config.min_confidence, 0.7);
        assert!(!perf_config.extract_cross_file);

        assert!(config.meets_confidence_threshold(0.8));
        assert!(!config.meets_confidence_threshold(0.3));
    }
}
