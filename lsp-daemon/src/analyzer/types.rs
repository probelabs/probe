//! Analysis Result Types and Error Handling
//!
//! This module defines the core types used throughout the analyzer framework
//! for representing analysis results, errors, and related data structures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

use super::framework::LanguageAnalyzerConfig;
use crate::database::{Edge, EdgeRelation, SymbolState};
use crate::symbol::{SymbolKind, SymbolLocation, SymbolUIDGenerator, Visibility};

/// Analysis error types
#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("Parser not available for language: {language}")]
    ParserNotAvailable { language: String },

    #[error("Parse error in {file}: {message}")]
    ParseError { file: String, message: String },

    #[error("LSP server error: {message}")]
    LspError { message: String },

    #[error("Timeout during analysis of {file} after {timeout_seconds}s")]
    Timeout { file: String, timeout_seconds: u64 },

    #[error("File too large: {size_bytes} bytes exceeds limit of {max_size} bytes")]
    FileTooLarge { size_bytes: u64, max_size: u64 },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Symbol UID generation failed: {0}")]
    UidGenerationError(#[from] crate::symbol::UIDError),

    #[error("Analysis configuration error: {message}")]
    ConfigError { message: String },

    #[error("Unsupported language: {language}")]
    UnsupportedLanguage { language: String },

    #[error("Internal analysis error: {message}")]
    InternalError { message: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl AnalysisError {
    /// Check if this error is recoverable (analysis could be retried)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            AnalysisError::Timeout { .. }
                | AnalysisError::LspError { .. }
                | AnalysisError::IoError(_)
        )
    }

    /// Check if this error indicates a configuration problem
    pub fn is_config_error(&self) -> bool {
        matches!(
            self,
            AnalysisError::ConfigError { .. }
                | AnalysisError::UnsupportedLanguage { .. }
                | AnalysisError::ParserNotAvailable { .. }
        )
    }

    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            AnalysisError::ParserNotAvailable { language } => {
                format!("No parser available for '{}' language", language)
            }
            AnalysisError::ParseError { file, .. } => {
                format!("Failed to parse file: {}", file)
            }
            AnalysisError::LspError { .. } => "Language server error occurred".to_string(),
            AnalysisError::Timeout {
                file,
                timeout_seconds,
            } => {
                format!(
                    "Analysis of '{}' timed out after {}s",
                    file, timeout_seconds
                )
            }
            AnalysisError::FileTooLarge {
                size_bytes,
                max_size,
            } => {
                format!(
                    "File size {}MB exceeds limit of {}MB",
                    size_bytes / 1_000_000,
                    max_size / 1_000_000
                )
            }
            _ => self.to_string(),
        }
    }
}

/// Context information for analysis operations
#[derive(Clone)]
pub struct AnalysisContext {
    /// Workspace identifier
    pub workspace_id: i64,

    /// File version identifier
    pub file_version_id: i64,

    /// Analysis run identifier
    pub analysis_run_id: i64,

    /// Programming language
    pub language: String,

    /// Shared UID generator for consistent symbol identification
    pub uid_generator: Arc<SymbolUIDGenerator>,

    /// Language-specific configuration
    pub language_config: LanguageAnalyzerConfig,
}

impl AnalysisContext {
    /// Create a new analysis context
    pub fn new(
        workspace_id: i64,
        file_version_id: i64,
        analysis_run_id: i64,
        language: String,
        uid_generator: Arc<SymbolUIDGenerator>,
    ) -> Self {
        Self {
            workspace_id,
            file_version_id,
            analysis_run_id,
            language,
            uid_generator,
            language_config: LanguageAnalyzerConfig::default(),
        }
    }

    /// Create context with language configuration
    pub fn with_language_config(
        workspace_id: i64,
        file_version_id: i64,
        analysis_run_id: i64,
        language: String,
        uid_generator: Arc<SymbolUIDGenerator>,
        language_config: LanguageAnalyzerConfig,
    ) -> Self {
        Self {
            workspace_id,
            file_version_id,
            analysis_run_id,
            language,
            uid_generator,
            language_config,
        }
    }
}

impl Default for AnalysisContext {
    fn default() -> Self {
        Self {
            workspace_id: 1,
            file_version_id: 1,
            analysis_run_id: 1,
            language: "unknown".to_string(),
            uid_generator: Arc::new(SymbolUIDGenerator::new()),
            language_config: LanguageAnalyzerConfig::default(),
        }
    }
}

/// Complete result of analyzing a source file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Path to the analyzed file
    pub file_path: PathBuf,

    /// Programming language of the analyzed file
    pub language: String,

    /// Extracted symbols from the file
    pub symbols: Vec<ExtractedSymbol>,

    /// Extracted relationships between symbols
    pub relationships: Vec<ExtractedRelationship>,

    /// File dependencies discovered during analysis
    pub dependencies: Vec<FileDependency>,

    /// Metadata about the analysis process
    pub analysis_metadata: AnalysisMetadata,
}

impl AnalysisResult {
    /// Create a new analysis result
    pub fn new(file_path: PathBuf, language: String) -> Self {
        Self {
            file_path,
            language,
            symbols: Vec::new(),
            relationships: Vec::new(),
            dependencies: Vec::new(),
            analysis_metadata: AnalysisMetadata::default(),
        }
    }

    /// Add a symbol to the result
    pub fn add_symbol(&mut self, symbol: ExtractedSymbol) {
        self.symbols.push(symbol);
    }

    /// Add a relationship to the result
    pub fn add_relationship(&mut self, relationship: ExtractedRelationship) {
        self.relationships.push(relationship);
    }

    /// Add a dependency to the result
    pub fn add_dependency(&mut self, dependency: FileDependency) {
        self.dependencies.push(dependency);
    }

    /// Get symbols by kind
    pub fn symbols_by_kind(&self, kind: SymbolKind) -> Vec<&ExtractedSymbol> {
        self.symbols.iter().filter(|s| s.kind == kind).collect()
    }

    /// Get relationships by type
    pub fn relationships_by_type(
        &self,
        relation_type: RelationType,
    ) -> Vec<&ExtractedRelationship> {
        self.relationships
            .iter()
            .filter(|r| r.relation_type == relation_type)
            .collect()
    }

    /// Convert to database storage format
    pub fn to_database_symbols(&self, context: &AnalysisContext) -> Vec<SymbolState> {
        self.symbols
            .iter()
            .map(|symbol| {
                SymbolState {
                    symbol_uid: symbol.uid.clone(),
                    file_version_id: context.file_version_id,
                    language: context.language.clone(),
                    name: symbol.name.clone(),
                    fqn: symbol.qualified_name.clone(),
                    kind: symbol.kind.to_string(),
                    signature: symbol.signature.clone(),
                    visibility: symbol.visibility.as_ref().map(|v| v.to_string()),
                    def_start_line: symbol.location.start_line,
                    def_start_char: symbol.location.start_char,
                    def_end_line: symbol.location.end_line,
                    def_end_char: symbol.location.end_char,
                    is_definition: true, // Analysis results are typically definitions
                    documentation: symbol.documentation.clone(),
                    metadata: if symbol.metadata.is_empty() {
                        None
                    } else {
                        Some(serde_json::to_string(&symbol.metadata).unwrap_or_default())
                    },
                }
            })
            .collect()
    }

    /// Convert relationships to database edges
    pub fn to_database_edges(&self, context: &AnalysisContext) -> Vec<Edge> {
        self.relationships
            .iter()
            .map(|rel| Edge {
                language: context.language.clone(),
                relation: rel.relation_type.to_edge_relation(),
                source_symbol_uid: rel.source_symbol_uid.clone(),
                target_symbol_uid: rel.target_symbol_uid.clone(),
                anchor_file_version_id: context.file_version_id,
                start_line: rel.location.as_ref().map(|l| l.start_line),
                start_char: rel.location.as_ref().map(|l| l.start_char),
                confidence: rel.confidence,
                metadata: if rel.metadata.is_empty() {
                    None
                } else {
                    Some(serde_json::to_string(&rel.metadata).unwrap_or_default())
                },
            })
            .collect()
    }

    /// Merge with another analysis result
    pub fn merge(&mut self, other: AnalysisResult) {
        self.symbols.extend(other.symbols);
        self.relationships.extend(other.relationships);
        self.dependencies.extend(other.dependencies);
        self.analysis_metadata.merge(other.analysis_metadata);
    }

    /// Get analysis statistics
    pub fn get_stats(&self) -> HashMap<String, u64> {
        let mut stats = HashMap::new();
        stats.insert("total_symbols".to_string(), self.symbols.len() as u64);
        stats.insert(
            "total_relationships".to_string(),
            self.relationships.len() as u64,
        );
        stats.insert(
            "total_dependencies".to_string(),
            self.dependencies.len() as u64,
        );

        // Count by symbol kind
        for symbol in &self.symbols {
            let key = format!("symbols_{}", symbol.kind.to_string().to_lowercase());
            *stats.entry(key).or_insert(0) += 1;
        }

        // Count by relationship type
        for rel in &self.relationships {
            let key = format!(
                "relationships_{}",
                rel.relation_type.to_string().to_lowercase()
            );
            *stats.entry(key).or_insert(0) += 1;
        }

        stats
    }
}

/// Symbol extracted from source code analysis
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedSymbol {
    /// Unique identifier for this symbol (generated using SymbolUIDGenerator)
    pub uid: String,

    /// Symbol name as it appears in source code
    pub name: String,

    /// Kind of symbol (function, class, variable, etc.)
    pub kind: SymbolKind,

    /// Fully qualified name if available
    pub qualified_name: Option<String>,

    /// Function/method signature if applicable
    pub signature: Option<String>,

    /// Visibility modifier (public, private, etc.)
    pub visibility: Option<Visibility>,

    /// Location in source code
    pub location: SymbolLocation,

    /// Parent scope context
    pub parent_scope: Option<String>,

    /// Documentation string if available
    pub documentation: Option<String>,

    /// Additional tags/attributes
    pub tags: Vec<String>,

    /// Analyzer-specific metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExtractedSymbol {
    /// Create a new extracted symbol
    pub fn new(uid: String, name: String, kind: SymbolKind, location: SymbolLocation) -> Self {
        Self {
            uid,
            name,
            kind,
            qualified_name: None,
            signature: None,
            visibility: None,
            location,
            parent_scope: None,
            documentation: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Builder pattern methods
    pub fn with_qualified_name(mut self, qualified_name: String) -> Self {
        self.qualified_name = Some(qualified_name);
        self
    }

    pub fn with_signature(mut self, signature: String) -> Self {
        self.signature = Some(signature);
        self
    }

    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = Some(visibility);
        self
    }

    pub fn with_documentation(mut self, documentation: String) -> Self {
        self.documentation = Some(documentation);
        self
    }

    pub fn with_parent_scope(mut self, parent_scope: String) -> Self {
        self.parent_scope = Some(parent_scope);
        self
    }

    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Check if this symbol is callable (function, method, etc.)
    pub fn is_callable(&self) -> bool {
        self.kind.is_callable()
    }

    /// Check if this symbol is a type definition
    pub fn is_type_definition(&self) -> bool {
        self.kind.is_type_definition()
    }

    /// Check if this symbol is likely exported/public
    pub fn is_exported(&self) -> bool {
        matches!(
            self.visibility,
            Some(Visibility::Public) | Some(Visibility::Export)
        ) || self.tags.contains(&"export".to_string())
            || self
                .metadata
                .get("exported")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
    }
}

/// Relationship between two symbols
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedRelationship {
    /// UID of the source symbol
    pub source_symbol_uid: String,

    /// UID of the target symbol
    pub target_symbol_uid: String,

    /// Type of relationship
    pub relation_type: RelationType,

    /// Location where relationship is expressed (optional)
    pub location: Option<SymbolLocation>,

    /// Confidence level (0.0 to 1.0)
    pub confidence: f32,

    /// Additional metadata about the relationship
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExtractedRelationship {
    /// Create a new relationship
    pub fn new(
        source_symbol_uid: String,
        target_symbol_uid: String,
        relation_type: RelationType,
    ) -> Self {
        Self {
            source_symbol_uid,
            target_symbol_uid,
            relation_type,
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

    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Types of relationships between symbols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationType {
    // Structural relationships
    Contains,
    InheritsFrom,
    Implements,
    Overrides,
    ExtendedBy,

    // Usage relationships
    References,
    Calls,
    CalledBy,
    Instantiates,
    Imports,

    // Type relationships
    TypeOf,
    InstanceOf,
}

impl RelationType {
    /// Convert to string representation
    pub fn to_string(self) -> &'static str {
        match self {
            RelationType::Contains => "contains",
            RelationType::InheritsFrom => "inherits_from",
            RelationType::Implements => "implements",
            RelationType::Overrides => "overrides",
            RelationType::ExtendedBy => "extended_by",
            RelationType::References => "references",
            RelationType::Calls => "calls",
            RelationType::CalledBy => "called_by",
            RelationType::Instantiates => "instantiates",
            RelationType::Imports => "imports",
            RelationType::TypeOf => "type_of",
            RelationType::InstanceOf => "instance_of",
        }
    }

    /// Convert to database EdgeRelation
    pub fn to_edge_relation(self) -> EdgeRelation {
        match self {
            RelationType::Contains => EdgeRelation::HasChild,
            RelationType::InheritsFrom => EdgeRelation::InheritsFrom,
            RelationType::Implements => EdgeRelation::Implements,
            RelationType::Overrides => EdgeRelation::Overrides,
            RelationType::ExtendedBy => EdgeRelation::InheritsFrom, // Reverse relationship
            RelationType::References => EdgeRelation::References,
            RelationType::Calls => EdgeRelation::Calls,
            RelationType::CalledBy => EdgeRelation::Calls, // Reverse relationship
            RelationType::Instantiates => EdgeRelation::Instantiates,
            RelationType::Imports => EdgeRelation::Imports,
            RelationType::TypeOf => EdgeRelation::References, // Map to generic reference
            RelationType::InstanceOf => EdgeRelation::References,
        }
    }

    /// Get the inverse relationship type
    pub fn inverse(self) -> Option<RelationType> {
        match self {
            RelationType::Contains => None, // Contains is not typically inversed
            RelationType::InheritsFrom => Some(RelationType::ExtendedBy),
            RelationType::ExtendedBy => Some(RelationType::InheritsFrom),
            RelationType::Calls => Some(RelationType::CalledBy),
            RelationType::CalledBy => Some(RelationType::Calls),
            _ => None,
        }
    }

    /// Check if this is a structural relationship
    pub fn is_structural(self) -> bool {
        matches!(
            self,
            RelationType::Contains
                | RelationType::InheritsFrom
                | RelationType::Implements
                | RelationType::Overrides
                | RelationType::ExtendedBy
        )
    }

    /// Check if this is a usage relationship
    pub fn is_usage(self) -> bool {
        matches!(
            self,
            RelationType::References
                | RelationType::Calls
                | RelationType::CalledBy
                | RelationType::Instantiates
                | RelationType::Imports
        )
    }
}

/// File dependency information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileDependency {
    /// Path to the dependent file
    pub file_path: PathBuf,

    /// Type of dependency
    pub dependency_type: DependencyType,

    /// Import/include statement if applicable
    pub import_statement: Option<String>,

    /// Location of the dependency declaration
    pub location: Option<SymbolLocation>,

    /// Whether this is a direct or transitive dependency
    pub is_direct: bool,
}

/// Types of file dependencies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DependencyType {
    /// Direct import/include
    Import,
    /// Module/namespace dependency
    Module,
    /// Type dependency
    Type,
    /// Resource dependency (e.g., assets)
    Resource,
}

/// Metadata about the analysis process
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    /// Analysis timestamp
    pub timestamp: Option<String>,

    /// Analyzer that produced this result
    pub analyzer_name: String,

    /// Analyzer version
    pub analyzer_version: String,

    /// Analysis duration in milliseconds
    pub duration_ms: u64,

    /// Any warnings generated during analysis
    pub warnings: Vec<String>,

    /// Performance metrics
    pub metrics: HashMap<String, f64>,

    /// Additional metadata
    pub custom: HashMap<String, serde_json::Value>,
}

impl AnalysisMetadata {
    /// Create new metadata with analyzer information
    pub fn new(analyzer_name: String, analyzer_version: String) -> Self {
        Self {
            analyzer_name,
            analyzer_version,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        }
    }

    /// Add a warning
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// Add a performance metric
    pub fn add_metric(&mut self, name: String, value: f64) {
        self.metrics.insert(name, value);
    }

    /// Merge with other metadata
    pub fn merge(&mut self, other: AnalysisMetadata) {
        self.duration_ms += other.duration_ms;
        self.warnings.extend(other.warnings);
        self.metrics.extend(other.metrics);
        self.custom.extend(other.custom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_analysis_error_properties() {
        let timeout_error = AnalysisError::Timeout {
            file: "test.rs".to_string(),
            timeout_seconds: 30,
        };
        assert!(timeout_error.is_recoverable());
        assert!(!timeout_error.is_config_error());

        let config_error = AnalysisError::UnsupportedLanguage {
            language: "unknown".to_string(),
        };
        assert!(!config_error.is_recoverable());
        assert!(config_error.is_config_error());
    }

    #[test]
    fn test_analysis_result_creation() {
        let result = AnalysisResult::new(PathBuf::from("test.rs"), "rust".to_string());

        assert_eq!(result.file_path, PathBuf::from("test.rs"));
        assert_eq!(result.language, "rust");
        assert!(result.symbols.is_empty());
        assert!(result.relationships.is_empty());
        assert!(result.dependencies.is_empty());
    }

    #[test]
    fn test_extracted_symbol_builder() {
        let location = SymbolLocation::new(PathBuf::from("test.rs"), 1, 0, 1, 10);
        let symbol = ExtractedSymbol::new(
            "rust::test::function".to_string(),
            "test_function".to_string(),
            SymbolKind::Function,
            location,
        )
        .with_qualified_name("test::test_function".to_string())
        .with_visibility(Visibility::Public)
        .with_tag("exported".to_string());

        assert_eq!(symbol.name, "test_function");
        assert_eq!(symbol.kind, SymbolKind::Function);
        assert_eq!(
            symbol.qualified_name.as_ref().unwrap(),
            "test::test_function"
        );
        assert_eq!(symbol.visibility.as_ref().unwrap(), &Visibility::Public);
        assert!(symbol.tags.contains(&"exported".to_string()));
        assert!(symbol.is_callable());
        assert!(symbol.is_exported());
    }

    #[test]
    fn test_extracted_relationship() {
        let rel = ExtractedRelationship::new(
            "source_uid".to_string(),
            "target_uid".to_string(),
            RelationType::Calls,
        )
        .with_confidence(0.95);

        assert_eq!(rel.source_symbol_uid, "source_uid");
        assert_eq!(rel.target_symbol_uid, "target_uid");
        assert_eq!(rel.relation_type, RelationType::Calls);
        assert_eq!(rel.confidence, 0.95);
    }

    #[test]
    fn test_relation_type_conversions() {
        assert_eq!(RelationType::Calls.to_string(), "calls");
        assert_eq!(
            RelationType::InheritsFrom.to_edge_relation(),
            EdgeRelation::InheritsFrom
        );

        assert_eq!(RelationType::Calls.inverse(), Some(RelationType::CalledBy));
        assert_eq!(RelationType::CalledBy.inverse(), Some(RelationType::Calls));
        assert_eq!(RelationType::References.inverse(), None);

        assert!(RelationType::InheritsFrom.is_structural());
        assert!(!RelationType::InheritsFrom.is_usage());
        assert!(RelationType::Calls.is_usage());
        assert!(!RelationType::Calls.is_structural());
    }

    #[test]
    fn test_analysis_metadata() {
        let mut metadata =
            AnalysisMetadata::new("TreeSitterAnalyzer".to_string(), "1.0.0".to_string());

        metadata.add_warning("Unused variable".to_string());
        metadata.add_metric("parse_time_ms".to_string(), 123.45);

        assert_eq!(metadata.analyzer_name, "TreeSitterAnalyzer");
        assert_eq!(metadata.warnings.len(), 1);
        assert_eq!(metadata.metrics.len(), 1);
        assert!(metadata.timestamp.is_some());
    }

    #[test]
    fn test_analysis_result_stats() {
        let mut result = AnalysisResult::new(PathBuf::from("test.rs"), "rust".to_string());

        let location = SymbolLocation::new(PathBuf::from("test.rs"), 1, 0, 1, 10);
        result.add_symbol(ExtractedSymbol::new(
            "uid1".to_string(),
            "func1".to_string(),
            SymbolKind::Function,
            location.clone(),
        ));
        result.add_symbol(ExtractedSymbol::new(
            "uid2".to_string(),
            "struct1".to_string(),
            SymbolKind::Struct,
            location,
        ));

        result.add_relationship(ExtractedRelationship::new(
            "uid1".to_string(),
            "uid2".to_string(),
            RelationType::Calls,
        ));

        let stats = result.get_stats();
        assert_eq!(stats["total_symbols"], 2);
        assert_eq!(stats["total_relationships"], 1);
        assert_eq!(stats["symbols_function"], 1);
        assert_eq!(stats["symbols_struct"], 1);
        assert_eq!(stats["relationships_calls"], 1);
    }
}
