//! Multi-Language Analyzer Framework
#![allow(dead_code, clippy::all)]
//!
//! This module provides a comprehensive framework for analyzing source code across multiple
//! programming languages to extract symbols and relationships. The framework supports both
//! structural analysis (via tree-sitter AST parsing) and semantic analysis (via LSP integration).
//!
//! # Architecture
//!
//! The analyzer framework is built around the `CodeAnalyzer` trait which provides a unified
//! interface for different analysis strategies:
//!
//! * **TreeSitterAnalyzer** - Structural analysis using tree-sitter AST parsing
//! * **LspAnalyzer** - Semantic analysis using Language Server Protocol
//! * **HybridAnalyzer** - Combined structural + semantic analysis
//! * **LanguageAnalyzers** - Language-specific analyzer implementations
//!
//! # Usage
//!
//! ```rust
//! use analyzer::{AnalyzerManager, AnalysisContext, TreeSitterAnalyzer};
//!
//! // Create analyzer manager with UID generator
//! let uid_generator = Arc::new(SymbolUIDGenerator::new());
//! let mut manager = AnalyzerManager::new(uid_generator.clone());
//!
//! // Register language-specific analyzer
//! let rust_analyzer = Box::new(TreeSitterAnalyzer::new(uid_generator.clone()));
//! manager.register_analyzer("rust", rust_analyzer);
//!
//! // Analyze a file
//! let context = AnalysisContext::new(workspace_id, file_version_id, analysis_run_id);
//! let result = manager.analyze_file(content, &file_path, "rust", &context).await?;
//! ```
//!
//! # Language Support
//!
//! The framework provides extensible language support through:
//! - Language-specific analyzers in the `language_analyzers` module
//! - Integration with existing `LanguageIndexingStrategy` from Phase 3.1
//! - Configurable analysis capabilities per language
//!
//! # Integration
//!
//! - **Symbol UID Generation**: Uses Phase 3.1 SymbolUIDGenerator for consistent identifiers
//! - **Database Storage**: Converts analysis results to database SymbolState and Edge types
//! - **Language Strategies**: Integrates with existing indexing language strategies
//! - **Performance**: Supports parallel analysis and incremental updates

pub mod framework;
pub mod hybrid_analyzer;
pub mod language_analyzers;
pub mod lsp_analyzer;
pub mod tree_sitter_analyzer;
pub mod types;

// Re-export public types and traits
pub use framework::*;
pub use hybrid_analyzer::*;
pub use language_analyzers::*;
pub use lsp_analyzer::*;
pub use tree_sitter_analyzer::*;
pub use types::*;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::relationship::TreeSitterRelationshipExtractor;
use crate::symbol::SymbolUIDGenerator;

/// Manager for coordinating multiple code analyzers
///
/// The AnalyzerManager provides a unified interface for managing different types
/// of code analyzers and routing analysis requests to the appropriate analyzer
/// based on language and available capabilities.
pub struct AnalyzerManager {
    /// Map of language -> analyzer implementations
    analyzers: HashMap<String, Box<dyn CodeAnalyzer + Send + Sync>>,

    /// Shared UID generator for consistent symbol identification
    uid_generator: Arc<SymbolUIDGenerator>,

    /// Default analyzer for unsupported languages
    default_analyzer: Box<dyn CodeAnalyzer + Send + Sync>,
}

impl AnalyzerManager {
    /// Create a new analyzer manager with the given UID generator
    pub fn new(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        let default_analyzer = Box::new(TreeSitterAnalyzer::new(uid_generator.clone()));

        Self {
            analyzers: HashMap::new(),
            uid_generator,
            default_analyzer,
        }
    }

    /// Register an analyzer for a specific language
    pub fn register_analyzer(
        &mut self,
        language: &str,
        analyzer: Box<dyn CodeAnalyzer + Send + Sync>,
    ) {
        self.analyzers.insert(language.to_lowercase(), analyzer);
    }

    /// Analyze a file using the appropriate analyzer for the language
    pub async fn analyze_file(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        let analyzer = self.get_analyzer_for_language(language);
        analyzer
            .analyze_file(content, file_path, language, context)
            .await
    }

    /// Perform incremental analysis on a changed file
    pub async fn analyze_incremental(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        previous_result: Option<&AnalysisResult>,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        let analyzer = self.get_analyzer_for_language(language);
        analyzer
            .analyze_incremental(content, file_path, language, previous_result, context)
            .await
    }

    /// Get the analyzer for a specific language
    pub fn get_analyzer_for_language(&self, language: &str) -> &dyn CodeAnalyzer {
        let lang_key = language.to_lowercase();
        self.analyzers
            .get(&lang_key)
            .map(|a| a.as_ref())
            .unwrap_or(self.default_analyzer.as_ref())
    }

    /// Get list of supported languages
    pub fn supported_languages(&self) -> Vec<String> {
        self.analyzers.keys().cloned().collect()
    }

    /// Get capabilities for a specific language
    pub fn get_capabilities(&self, language: &str) -> AnalyzerCapabilities {
        self.get_analyzer_for_language(language).capabilities()
    }

    /// Create a pre-configured analyzer manager with default analyzers
    pub fn with_default_analyzers(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        let mut manager = Self::new(uid_generator.clone());

        // Register default analyzers for major languages
        manager.register_analyzer(
            "rust",
            Box::new(language_analyzers::RustAnalyzer::new(uid_generator.clone())),
        );
        manager.register_analyzer(
            "typescript",
            Box::new(language_analyzers::TypeScriptAnalyzer::new(
                uid_generator.clone(),
            )),
        );
        manager.register_analyzer(
            "javascript",
            Box::new(language_analyzers::TypeScriptAnalyzer::new(
                uid_generator.clone(),
            )),
        ); // JS uses TS analyzer
        manager.register_analyzer(
            "python",
            Box::new(language_analyzers::PythonAnalyzer::new(
                uid_generator.clone(),
            )),
        );

        // Generic tree-sitter analyzer for other languages
        manager.register_analyzer(
            "go",
            Box::new(TreeSitterAnalyzer::new(uid_generator.clone())),
        );
        manager.register_analyzer(
            "java",
            Box::new(TreeSitterAnalyzer::new(uid_generator.clone())),
        );
        manager.register_analyzer(
            "c",
            Box::new(TreeSitterAnalyzer::new(uid_generator.clone())),
        );
        manager.register_analyzer(
            "cpp",
            Box::new(TreeSitterAnalyzer::new(uid_generator.clone())),
        );
        manager.register_analyzer(
            "c++",
            Box::new(TreeSitterAnalyzer::new(uid_generator.clone())),
        );

        manager
    }

    /// Create analyzer manager with relationship extraction enabled for tree-sitter analyzers
    pub fn with_relationship_extraction(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        let mut manager = Self::new(uid_generator.clone());

        // Create shared relationship extractor
        let relationship_extractor =
            Arc::new(TreeSitterRelationshipExtractor::new(uid_generator.clone()));

        // Register default analyzers for major languages with relationship extraction
        manager.register_analyzer(
            "rust",
            Box::new(language_analyzers::RustAnalyzer::new(uid_generator.clone())),
        );
        manager.register_analyzer(
            "typescript",
            Box::new(language_analyzers::TypeScriptAnalyzer::new(
                uid_generator.clone(),
            )),
        );
        manager.register_analyzer(
            "javascript",
            Box::new(language_analyzers::TypeScriptAnalyzer::new(
                uid_generator.clone(),
            )),
        );
        manager.register_analyzer(
            "python",
            Box::new(language_analyzers::PythonAnalyzer::new(
                uid_generator.clone(),
            )),
        );

        // Tree-sitter analyzers with relationship extraction
        manager.register_analyzer(
            "go",
            Box::new(TreeSitterAnalyzer::with_relationship_extractor(
                uid_generator.clone(),
                relationship_extractor.clone(),
            )),
        );
        manager.register_analyzer(
            "java",
            Box::new(TreeSitterAnalyzer::with_relationship_extractor(
                uid_generator.clone(),
                relationship_extractor.clone(),
            )),
        );
        manager.register_analyzer(
            "c",
            Box::new(TreeSitterAnalyzer::with_relationship_extractor(
                uid_generator.clone(),
                relationship_extractor.clone(),
            )),
        );
        manager.register_analyzer(
            "cpp",
            Box::new(TreeSitterAnalyzer::with_relationship_extractor(
                uid_generator.clone(),
                relationship_extractor.clone(),
            )),
        );
        manager.register_analyzer(
            "c++",
            Box::new(TreeSitterAnalyzer::with_relationship_extractor(
                uid_generator.clone(),
                relationship_extractor,
            )),
        );

        manager
    }

    /// Get statistics about registered analyzers
    pub fn get_stats(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();
        stats.insert(
            "total_analyzers".to_string(),
            self.analyzers.len().to_string(),
        );
        stats.insert(
            "supported_languages".to_string(),
            self.supported_languages().join(", "),
        );

        // Collect capabilities statistics
        let mut structural_count = 0;
        let mut semantic_count = 0;
        let mut incremental_count = 0;
        let mut lsp_count = 0;

        for analyzer in self.analyzers.values() {
            let caps = analyzer.capabilities();
            if caps.extracts_symbols {
                structural_count += 1;
            }
            if caps.extracts_relationships {
                semantic_count += 1;
            }
            if caps.supports_incremental {
                incremental_count += 1;
            }
            if caps.requires_lsp {
                lsp_count += 1;
            }
        }

        stats.insert(
            "structural_analyzers".to_string(),
            structural_count.to_string(),
        );
        stats.insert("semantic_analyzers".to_string(), semantic_count.to_string());
        stats.insert(
            "incremental_analyzers".to_string(),
            incremental_count.to_string(),
        );
        stats.insert("lsp_analyzers".to_string(), lsp_count.to_string());

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolUIDGenerator;
    use std::path::PathBuf;

    fn create_test_context() -> AnalysisContext {
        AnalysisContext {
            workspace_id: 1,
            analysis_run_id: 1,
            language: "rust".to_string(),
            workspace_path: PathBuf::from("/test/workspace"),
            file_path: PathBuf::from("/test/workspace/test.rs"),
            uid_generator: Arc::new(SymbolUIDGenerator::new()),
            language_config: LanguageAnalyzerConfig::default(),
        }
    }

    #[test]
    fn test_analyzer_manager_creation() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let manager = AnalyzerManager::new(uid_generator);

        assert!(manager.analyzers.is_empty());
        assert!(manager.supported_languages().is_empty());
    }

    #[test]
    fn test_default_analyzers() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let manager = AnalyzerManager::with_default_analyzers(uid_generator);

        let supported = manager.supported_languages();
        assert!(supported.contains(&"rust".to_string()));
        assert!(supported.contains(&"typescript".to_string()));
        assert!(supported.contains(&"python".to_string()));
        assert!(supported.len() >= 3);
    }

    #[test]
    fn test_analyzer_registration() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let mut manager = AnalyzerManager::new(uid_generator.clone());

        let analyzer = Box::new(TreeSitterAnalyzer::new(uid_generator));
        manager.register_analyzer("test_lang", analyzer);

        assert_eq!(manager.supported_languages().len(), 1);
        assert!(manager
            .supported_languages()
            .contains(&"test_lang".to_string()));
    }

    #[test]
    fn test_get_analyzer_capabilities() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let manager = AnalyzerManager::with_default_analyzers(uid_generator);

        let rust_caps = manager.get_capabilities("rust");
        assert!(rust_caps.extracts_symbols);

        // Unknown language should use default analyzer
        let unknown_caps = manager.get_capabilities("unknown_language");
        assert!(unknown_caps.extracts_symbols);
    }

    #[tokio::test]
    async fn test_analyze_file_routing() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let manager = AnalyzerManager::with_default_analyzers(uid_generator);
        let context = create_test_context();

        // Test that analysis is routed to appropriate analyzer
        let rust_code = "fn main() { println!(\"Hello, world!\"); }";
        let file_path = PathBuf::from("test.rs");

        // This should not panic and should return a result
        let result = manager
            .analyze_file(rust_code, &file_path, "rust", &context)
            .await;
        assert!(result.is_ok() || matches!(result, Err(AnalysisError::ParserNotAvailable { .. })));
    }

    #[test]
    fn test_manager_stats() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let manager = AnalyzerManager::with_default_analyzers(uid_generator);

        let stats = manager.get_stats();
        assert!(stats.contains_key("total_analyzers"));
        assert!(stats.contains_key("supported_languages"));
        assert!(stats["total_analyzers"].parse::<usize>().unwrap() > 0);
    }
}
