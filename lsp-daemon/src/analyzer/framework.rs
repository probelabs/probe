//! Core Analyzer Framework
//!
//! This module defines the core traits and types for the multi-language analyzer framework.
//! It provides the foundational `CodeAnalyzer` trait that all analyzer implementations must
//! implement, along with supporting types for analysis configuration and capabilities.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub use super::types::{
    AnalysisContext, AnalysisError, AnalysisResult, ExtractedRelationship, ExtractedSymbol,
};
use crate::indexing::language_strategies::LanguageIndexingStrategy;

/// Core trait that all code analyzers must implement
///
/// This trait provides a unified interface for analyzing source code to extract
/// symbols and relationships. Different implementations can use various approaches:
/// - Tree-sitter for structural analysis
/// - LSP for semantic analysis  
/// - Hybrid approaches combining multiple techniques
#[async_trait]
pub trait CodeAnalyzer: Send + Sync {
    /// Get the capabilities of this analyzer
    fn capabilities(&self) -> AnalyzerCapabilities;

    /// Get the languages supported by this analyzer
    fn supported_languages(&self) -> Vec<String>;

    /// Analyze a file and extract symbols and relationships
    ///
    /// # Arguments
    /// * `content` - The source code content to analyze
    /// * `file_path` - Path to the source file being analyzed
    /// * `language` - Programming language identifier (e.g., "rust", "typescript")
    /// * `context` - Analysis context including workspace and version information
    ///
    /// # Returns
    /// `AnalysisResult` containing extracted symbols, relationships, and metadata
    async fn analyze_file(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError>;

    /// Perform incremental analysis on a changed file
    ///
    /// This method allows analyzers to optimize analysis by reusing previous results
    /// when only part of a file has changed. Analyzers that don't support incremental
    /// analysis can simply delegate to `analyze_file`.
    ///
    /// # Arguments
    /// * `content` - The new source code content
    /// * `file_path` - Path to the source file  
    /// * `language` - Programming language identifier
    /// * `previous_result` - Previous analysis result to reuse if possible
    /// * `context` - Analysis context
    async fn analyze_incremental(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        previous_result: Option<&AnalysisResult>,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        // Default implementation: just re-analyze the entire file
        // Specific analyzers can override this for better performance
        let _ = previous_result; // Suppress unused warning
        self.analyze_file(content, file_path, language, context)
            .await
    }

    /// Validate that this analyzer can handle the given language
    fn can_analyze_language(&self, language: &str) -> bool {
        self.supported_languages()
            .contains(&language.to_lowercase())
    }

    /// Get analyzer-specific configuration options
    fn get_config(&self) -> AnalyzerConfig {
        AnalyzerConfig::default()
    }

    /// Update analyzer configuration
    fn set_config(&mut self, _config: AnalyzerConfig) -> Result<(), AnalysisError> {
        // Default implementation: no-op
        // Specific analyzers can override to support configuration
        Ok(())
    }
}

/// Capabilities of a code analyzer
///
/// This struct describes what analysis features an analyzer supports,
/// allowing the analyzer manager to make informed decisions about
/// which analyzer to use for different tasks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalyzerCapabilities {
    /// Whether this analyzer can extract symbol information
    pub extracts_symbols: bool,

    /// Whether this analyzer can extract relationships between symbols
    pub extracts_relationships: bool,

    /// Whether this analyzer supports incremental analysis
    pub supports_incremental: bool,

    /// Whether this analyzer requires an LSP server to be running
    pub requires_lsp: bool,

    /// Whether this analyzer is safe to run in parallel with others
    pub parallel_safe: bool,

    /// Maximum file size this analyzer can handle (in bytes)
    pub max_file_size: Option<u64>,

    /// Confidence level of analysis results (0.0 to 1.0)
    pub confidence: f32,

    /// Additional capability flags
    pub flags: HashMap<String, bool>,
}

impl Default for AnalyzerCapabilities {
    fn default() -> Self {
        Self {
            extracts_symbols: false,
            extracts_relationships: false,
            supports_incremental: false,
            requires_lsp: false,
            parallel_safe: true,
            max_file_size: Some(10 * 1024 * 1024), // 10MB default limit
            confidence: 0.8,
            flags: HashMap::new(),
        }
    }
}

impl AnalyzerCapabilities {
    /// Create capabilities for a structural analyzer (tree-sitter)
    pub fn structural() -> Self {
        Self {
            extracts_symbols: true,
            extracts_relationships: true,
            supports_incremental: false,
            requires_lsp: false,
            parallel_safe: true,
            confidence: 0.8,
            ..Default::default()
        }
    }

    /// Create capabilities for a semantic analyzer (LSP)
    pub fn semantic() -> Self {
        Self {
            extracts_symbols: true,
            extracts_relationships: true,
            supports_incremental: true,
            requires_lsp: true,
            parallel_safe: false, // LSP servers may not be thread-safe
            confidence: 0.95,
            ..Default::default()
        }
    }

    /// Create capabilities for a hybrid analyzer
    pub fn hybrid() -> Self {
        Self {
            extracts_symbols: true,
            extracts_relationships: true,
            supports_incremental: true,
            requires_lsp: true,
            parallel_safe: false,
            confidence: 0.98,
            ..Default::default()
        }
    }

    /// Check if this analyzer can extract the requested analysis type
    pub fn supports_analysis_type(&self, analysis_type: AnalysisType) -> bool {
        match analysis_type {
            AnalysisType::Symbols => self.extracts_symbols,
            AnalysisType::Relationships => self.extracts_relationships,
            AnalysisType::Both => self.extracts_symbols && self.extracts_relationships,
        }
    }

    /// Check if this analyzer meets the requirements for a given context
    pub fn meets_requirements(&self, requirements: &AnalysisRequirements) -> bool {
        if requirements.requires_symbols && !self.extracts_symbols {
            return false;
        }

        if requirements.requires_relationships && !self.extracts_relationships {
            return false;
        }

        if requirements.requires_incremental && !self.supports_incremental {
            return false;
        }

        if let Some(max_size) = requirements.max_file_size {
            if let Some(our_max) = self.max_file_size {
                if max_size > our_max {
                    return false;
                }
            }
        }

        if requirements.min_confidence > self.confidence {
            return false;
        }

        true
    }
}

/// Analysis requirements for selecting an appropriate analyzer
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisRequirements {
    /// Must be able to extract symbols
    pub requires_symbols: bool,

    /// Must be able to extract relationships  
    pub requires_relationships: bool,

    /// Must support incremental analysis
    pub requires_incremental: bool,

    /// Maximum file size to analyze
    pub max_file_size: Option<u64>,

    /// Minimum confidence level required
    pub min_confidence: f32,
}

impl Default for AnalysisRequirements {
    fn default() -> Self {
        Self {
            requires_symbols: true,
            requires_relationships: false,
            requires_incremental: false,
            max_file_size: None,
            min_confidence: 0.5,
        }
    }
}

/// Type of analysis to perform
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnalysisType {
    /// Extract only symbol information
    Symbols,
    /// Extract only relationships
    Relationships,
    /// Extract both symbols and relationships
    Both,
}

/// Generic analyzer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerConfig {
    /// Whether to enable parallel processing
    pub parallel: bool,

    /// Maximum depth for relationship extraction
    pub max_depth: u32,

    /// Timeout for analysis operations (in seconds)
    pub timeout_seconds: u64,

    /// Whether to include test files in analysis
    pub include_tests: bool,

    /// Custom configuration options
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            parallel: true,
            max_depth: 10,
            timeout_seconds: 300, // 5 minutes
            include_tests: true,
            custom: HashMap::new(),
        }
    }
}

impl AnalyzerConfig {
    /// Create configuration optimized for performance
    pub fn performance() -> Self {
        Self {
            parallel: true,
            max_depth: 5,         // Limit depth for speed
            timeout_seconds: 60,  // Shorter timeout
            include_tests: false, // Skip tests for speed
            custom: HashMap::new(),
        }
    }

    /// Create configuration optimized for completeness
    pub fn completeness() -> Self {
        Self {
            parallel: false,       // Sequential for thoroughness
            max_depth: 20,         // Deep analysis
            timeout_seconds: 1800, // 30 minutes
            include_tests: true,
            custom: HashMap::new(),
        }
    }

    /// Merge this configuration with another, preferring the other's values
    pub fn merge(self, other: AnalyzerConfig) -> Self {
        let mut custom = self.custom;
        custom.extend(other.custom);

        AnalyzerConfig {
            parallel: other.parallel,
            max_depth: other.max_depth,
            timeout_seconds: other.timeout_seconds,
            include_tests: other.include_tests,
            custom,
        }
    }
}

/// Language-specific analyzer configuration
///
/// This extends the generic AnalyzerConfig with language-specific settings
/// and integrates with the existing LanguageIndexingStrategy system.
#[derive(Debug, Clone)]
pub struct LanguageAnalyzerConfig {
    /// Base analyzer configuration
    pub base: AnalyzerConfig,

    /// Language-specific indexing strategy (from Phase 3.1)
    pub indexing_strategy: Option<LanguageIndexingStrategy>,

    /// Language-specific tree-sitter parser configuration
    pub tree_sitter_config: TreeSitterConfig,

    /// LSP-specific configuration
    pub lsp_config: LspAnalyzerConfig,
}

impl Default for LanguageAnalyzerConfig {
    fn default() -> Self {
        Self {
            base: AnalyzerConfig::default(),
            indexing_strategy: None,
            tree_sitter_config: TreeSitterConfig::default(),
            lsp_config: LspAnalyzerConfig::default(),
        }
    }
}

impl LanguageAnalyzerConfig {
    /// Create configuration with indexing strategy
    pub fn with_indexing_strategy(strategy: LanguageIndexingStrategy) -> Self {
        Self {
            indexing_strategy: Some(strategy),
            ..Default::default()
        }
    }

    /// Check if analysis should include test files for this language
    pub fn should_include_tests(&self, file_path: &Path) -> bool {
        if let Some(strategy) = &self.indexing_strategy {
            self.base.include_tests && !strategy.is_test_file(file_path)
        } else {
            self.base.include_tests
        }
    }

    /// Get symbol priority for this language
    pub fn get_symbol_priority(
        &self,
        symbol_type: &str,
        visibility: Option<&str>,
        has_docs: bool,
        is_exported: bool,
    ) -> Option<crate::indexing::language_strategies::IndexingPriority> {
        self.indexing_strategy.as_ref().map(|strategy| {
            strategy.calculate_symbol_priority(symbol_type, visibility, has_docs, is_exported)
        })
    }
}

/// Tree-sitter specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeSitterConfig {
    /// Whether to enable tree-sitter analysis
    pub enabled: bool,

    /// Parser timeout in milliseconds  
    pub parser_timeout_ms: u64,

    /// Whether to cache parse trees
    pub cache_trees: bool,

    /// Maximum tree cache size
    pub max_cache_size: usize,
}

impl Default for TreeSitterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            parser_timeout_ms: 5000, // 5 seconds
            cache_trees: true,
            max_cache_size: 100,
        }
    }
}

/// LSP analyzer specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspAnalyzerConfig {
    /// Whether to enable LSP analysis
    pub enabled: bool,

    /// LSP request timeout in seconds
    pub request_timeout_seconds: u64,

    /// Whether to use call hierarchy
    pub use_call_hierarchy: bool,

    /// Whether to use find references
    pub use_find_references: bool,

    /// Whether to use document symbols
    pub use_document_symbols: bool,

    /// Maximum number of references to retrieve
    pub max_references: usize,
}

impl Default for LspAnalyzerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            request_timeout_seconds: 30,
            use_call_hierarchy: true,
            use_find_references: true,
            use_document_symbols: true,
            max_references: 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_capabilities_default() {
        let caps = AnalyzerCapabilities::default();
        assert!(!caps.extracts_symbols);
        assert!(!caps.extracts_relationships);
        assert!(!caps.supports_incremental);
        assert!(!caps.requires_lsp);
        assert!(caps.parallel_safe);
        assert_eq!(caps.confidence, 0.8);
    }

    #[test]
    fn test_structural_capabilities() {
        let caps = AnalyzerCapabilities::structural();
        assert!(caps.extracts_symbols);
        assert!(caps.extracts_relationships);
        assert!(!caps.supports_incremental);
        assert!(!caps.requires_lsp);
        assert!(caps.parallel_safe);
    }

    #[test]
    fn test_semantic_capabilities() {
        let caps = AnalyzerCapabilities::semantic();
        assert!(caps.extracts_symbols);
        assert!(caps.extracts_relationships);
        assert!(caps.supports_incremental);
        assert!(caps.requires_lsp);
        assert!(!caps.parallel_safe);
        assert_eq!(caps.confidence, 0.95);
    }

    #[test]
    fn test_hybrid_capabilities() {
        let caps = AnalyzerCapabilities::hybrid();
        assert!(caps.extracts_symbols);
        assert!(caps.extracts_relationships);
        assert!(caps.supports_incremental);
        assert!(caps.requires_lsp);
        assert_eq!(caps.confidence, 0.98);
    }

    #[test]
    fn test_analysis_type_support() {
        let caps = AnalyzerCapabilities::structural();
        assert!(caps.supports_analysis_type(AnalysisType::Symbols));
        assert!(caps.supports_analysis_type(AnalysisType::Relationships));
        assert!(caps.supports_analysis_type(AnalysisType::Both));

        let limited_caps = AnalyzerCapabilities {
            extracts_symbols: true,
            extracts_relationships: false,
            ..Default::default()
        };
        assert!(limited_caps.supports_analysis_type(AnalysisType::Symbols));
        assert!(!limited_caps.supports_analysis_type(AnalysisType::Relationships));
        assert!(!limited_caps.supports_analysis_type(AnalysisType::Both));
    }

    #[test]
    fn test_requirements_matching() {
        let caps = AnalyzerCapabilities::semantic();

        let basic_reqs = AnalysisRequirements {
            requires_symbols: true,
            requires_relationships: false,
            requires_incremental: false,
            max_file_size: None,
            min_confidence: 0.7,
        };
        assert!(caps.meets_requirements(&basic_reqs));

        let high_reqs = AnalysisRequirements {
            requires_symbols: true,
            requires_relationships: true,
            requires_incremental: true,
            max_file_size: Some(5 * 1024 * 1024), // 5MB
            min_confidence: 0.9,
        };
        assert!(caps.meets_requirements(&high_reqs));

        let impossible_reqs = AnalysisRequirements {
            requires_symbols: true,
            requires_relationships: false,
            requires_incremental: false,
            max_file_size: None,
            min_confidence: 1.0, // Perfect confidence impossible
        };
        assert!(!caps.meets_requirements(&impossible_reqs));
    }

    #[test]
    fn test_analyzer_config_merge() {
        let base = AnalyzerConfig {
            parallel: false,
            max_depth: 5,
            timeout_seconds: 100,
            include_tests: false,
            custom: {
                let mut map = HashMap::new();
                map.insert(
                    "base_key".to_string(),
                    serde_json::Value::String("base_value".to_string()),
                );
                map
            },
        };

        let override_config = AnalyzerConfig {
            parallel: true,
            max_depth: 10,
            timeout_seconds: 200,
            include_tests: true,
            custom: {
                let mut map = HashMap::new();
                map.insert(
                    "override_key".to_string(),
                    serde_json::Value::String("override_value".to_string()),
                );
                map
            },
        };

        let merged = base.merge(override_config);
        assert!(merged.parallel);
        assert_eq!(merged.max_depth, 10);
        assert_eq!(merged.timeout_seconds, 200);
        assert!(merged.include_tests);
        assert_eq!(merged.custom.len(), 2); // Both keys should be present
    }

    #[test]
    fn test_performance_config() {
        let config = AnalyzerConfig::performance();
        assert!(config.parallel);
        assert_eq!(config.max_depth, 5);
        assert_eq!(config.timeout_seconds, 60);
        assert!(!config.include_tests);
    }

    #[test]
    fn test_completeness_config() {
        let config = AnalyzerConfig::completeness();
        assert!(!config.parallel);
        assert_eq!(config.max_depth, 20);
        assert_eq!(config.timeout_seconds, 1800);
        assert!(config.include_tests);
    }
}
