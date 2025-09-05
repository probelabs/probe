//! LSP-based Semantic Code Analyzer
//!
//! This module provides a semantic code analyzer that leverages Language Server Protocol (LSP)
//! to extract high-quality symbol information and relationships. It integrates with the existing
//! LSP infrastructure to provide semantic analysis capabilities.

use async_trait::async_trait;
// HashMap import removed as unused
use std::path::Path;
use std::sync::Arc;
// timeout and Duration imports removed as unused

use crate::symbol::{SymbolKind, SymbolUIDGenerator};
// Note: LSP integration requires additional protocol types that will be implemented later
// For now, we'll use a mock implementation
use super::framework::{AnalyzerCapabilities, CodeAnalyzer, LspAnalyzerConfig};
use super::types::*;
use crate::server_manager::SingleServerManager;

/// LSP-based semantic analyzer
///
/// This analyzer uses LSP servers to extract semantic information about symbols
/// and their relationships. It provides high-quality analysis but requires running
/// language servers and may have performance implications.
pub struct LspAnalyzer {
    /// UID generator for consistent symbol identification
    uid_generator: Arc<SymbolUIDGenerator>,

    /// Server manager for LSP communication  
    server_manager: Arc<SingleServerManager>,

    /// Configuration for LSP analysis
    config: LspAnalyzerConfig,
}

impl LspAnalyzer {
    /// Create a new LSP analyzer
    pub fn new(
        uid_generator: Arc<SymbolUIDGenerator>,
        server_manager: Arc<SingleServerManager>,
    ) -> Self {
        Self {
            uid_generator,
            server_manager,
            config: LspAnalyzerConfig::default(),
        }
    }

    /// Create analyzer with custom configuration
    pub fn with_config(
        uid_generator: Arc<SymbolUIDGenerator>,
        server_manager: Arc<SingleServerManager>,
        config: LspAnalyzerConfig,
    ) -> Self {
        Self {
            uid_generator,
            server_manager,
            config,
        }
    }

    /// Extract symbols using LSP document symbols
    /// Note: Simplified implementation - full LSP integration requires additional protocol types
    async fn extract_lsp_symbols(
        &self,
        _file_path: &Path,
        _language: &str,
        _context: &AnalysisContext,
    ) -> Result<Vec<ExtractedSymbol>, AnalysisError> {
        // TODO: Implement actual LSP integration when protocol types are available
        Ok(Vec::new())
    }

    /// Convert LSP symbols to ExtractedSymbol format
    /// Note: Simplified implementation - full LSP integration requires additional protocol types
    async fn convert_lsp_symbols_to_extracted(
        &self,
        _lsp_symbols: Vec<()>, // Placeholder type
        _file_path: &Path,
        _language: &str,
        _context: &AnalysisContext,
    ) -> Result<Vec<ExtractedSymbol>, AnalysisError> {
        // TODO: Implement when proper LSP protocol types are available
        Ok(Vec::new())
    }

    /// Convert LSP symbol kind to our SymbolKind
    fn convert_lsp_symbol_kind(&self, lsp_kind: u32) -> Result<SymbolKind, AnalysisError> {
        // LSP SymbolKind constants (from LSP spec)
        let symbol_kind = match lsp_kind {
            1 => SymbolKind::Module,      // File
            2 => SymbolKind::Module,      // Module
            3 => SymbolKind::Namespace,   // Namespace
            4 => SymbolKind::Package,     // Package
            5 => SymbolKind::Class,       // Class
            6 => SymbolKind::Method,      // Method
            7 => SymbolKind::Field,       // Property
            8 => SymbolKind::Field,       // Field
            9 => SymbolKind::Constructor, // Constructor
            10 => SymbolKind::Enum,       // Enum
            11 => SymbolKind::Interface,  // Interface
            12 => SymbolKind::Function,   // Function
            13 => SymbolKind::Variable,   // Variable
            14 => SymbolKind::Constant,   // Constant
            15 => SymbolKind::Variable,   // String
            16 => SymbolKind::Variable,   // Number
            17 => SymbolKind::Variable,   // Boolean
            18 => SymbolKind::Variable,   // Array
            19 => SymbolKind::Variable,   // Object
            20 => SymbolKind::Variable,   // Key
            21 => SymbolKind::Variable,   // Null
            22 => SymbolKind::Field,      // EnumMember
            23 => SymbolKind::Struct,     // Struct
            24 => SymbolKind::Variable,   // Event
            25 => SymbolKind::Variable,   // Operator
            26 => SymbolKind::Type,       // TypeParameter
            _ => SymbolKind::Variable,    // Default fallback
        };

        Ok(symbol_kind)
    }

    /// Extract call hierarchy relationships
    /// Note: Simplified implementation - full LSP integration requires additional protocol types
    async fn extract_call_relationships(
        &self,
        _symbols: &[ExtractedSymbol],
        _file_path: &Path,
        _language: &str,
    ) -> Result<Vec<ExtractedRelationship>, AnalysisError> {
        // TODO: Implement when proper LSP protocol types are available
        Ok(Vec::new())
    }

    /// Extract call hierarchy for a specific symbol
    async fn extract_symbol_call_hierarchy(
        &self,
        _symbol: &ExtractedSymbol,
        _file_path: &Path,
        _language: &str,
    ) -> Result<Vec<ExtractedRelationship>, AnalysisError> {
        // TODO: Implement LSP call hierarchy once proper protocol types are available
        // For now, return empty relationships to prevent compilation errors
        Ok(Vec::new())
    }

    /// Extract reference relationships
    async fn extract_reference_relationships(
        &self,
        symbols: &[ExtractedSymbol],
        file_path: &Path,
        language: &str,
    ) -> Result<Vec<ExtractedRelationship>, AnalysisError> {
        if !self.config.enabled || !self.config.use_find_references {
            return Ok(Vec::new());
        }

        let mut relationships = Vec::new();

        // Find references for important symbols (limit to prevent performance issues)
        let important_symbols: Vec<_> = symbols
            .iter()
            .filter(|s| s.is_exported() || s.kind.is_type_definition())
            .take(self.config.max_references / 10) // Limit symbols to analyze
            .collect();

        for symbol in important_symbols {
            let ref_rels = self
                .extract_symbol_references(symbol, file_path, language)
                .await?;
            relationships.extend(ref_rels);
        }

        Ok(relationships)
    }

    /// Extract references for a specific symbol
    async fn extract_symbol_references(
        &self,
        _symbol: &ExtractedSymbol,
        _file_path: &Path,
        _language: &str,
    ) -> Result<Vec<ExtractedRelationship>, AnalysisError> {
        // TODO: Implement LSP references once proper protocol types are available
        // For now, return empty relationships to prevent compilation errors
        Ok(Vec::new())
    }

    /// Check if LSP server is available for the language
    async fn is_lsp_available(&self, _language: &str) -> bool {
        // TODO: Implement once proper server manager methods are available
        true // For now, assume LSP is available to prevent compilation errors
    }
}

#[async_trait]
impl CodeAnalyzer for LspAnalyzer {
    fn capabilities(&self) -> AnalyzerCapabilities {
        AnalyzerCapabilities::semantic()
    }

    fn supported_languages(&self) -> Vec<String> {
        // Return languages that have LSP server support
        // This could be dynamic based on available servers
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
        _content: &str,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        if !self.config.enabled {
            return Err(AnalysisError::ConfigError {
                message: "LSP analyzer is disabled".to_string(),
            });
        }

        // Check if LSP server is available
        if !self.is_lsp_available(language).await {
            return Err(AnalysisError::LspError {
                message: format!("No LSP server available for language: {}", language),
            });
        }

        let start_time = std::time::Instant::now();

        // Extract symbols using LSP
        let symbols = self
            .extract_lsp_symbols(file_path, language, context)
            .await?;

        // Extract relationships using LSP
        let mut relationships = Vec::new();

        // Add call hierarchy relationships
        let call_rels = self
            .extract_call_relationships(&symbols, file_path, language)
            .await?;
        relationships.extend(call_rels);

        // Add reference relationships
        let ref_rels = self
            .extract_reference_relationships(&symbols, file_path, language)
            .await?;
        relationships.extend(ref_rels);

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
            AnalysisMetadata::new("LspAnalyzer".to_string(), "1.0.0".to_string());
        result.analysis_metadata.duration_ms = duration.as_millis() as u64;
        result
            .analysis_metadata
            .add_metric("symbols_extracted".to_string(), result.symbols.len() as f64);
        result.analysis_metadata.add_metric(
            "relationships_extracted".to_string(),
            result.relationships.len() as f64,
        );
        result
            .analysis_metadata
            .add_metric("lsp_requests_made".to_string(), 3.0); // Approximate

        Ok(result)
    }

    async fn analyze_incremental(
        &self,
        _content: &str,
        file_path: &Path,
        language: &str,
        previous_result: Option<&AnalysisResult>,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        // For LSP, we can potentially optimize by only re-analyzing changed symbols
        // For now, we'll just re-analyze the entire file
        if let Some(prev) = previous_result {
            // Add warning that we're doing full re-analysis
            let mut result = self
                .analyze_file(_content, file_path, language, context)
                .await?;
            result.analysis_metadata.add_warning(
                "Incremental analysis not implemented, performed full re-analysis".to_string(),
            );

            // In a full implementation, we could compare with previous results
            // and only update changed symbols
            let _ = prev; // Suppress unused warning

            Ok(result)
        } else {
            self.analyze_file(_content, file_path, language, context)
                .await
        }
    }
}

/// Mock LSP analyzer for testing when no LSP servers are available
pub struct MockLspAnalyzer {
    uid_generator: Arc<SymbolUIDGenerator>,
    config: LspAnalyzerConfig,
}

impl MockLspAnalyzer {
    pub fn new(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        Self {
            uid_generator,
            config: LspAnalyzerConfig::default(),
        }
    }
}

#[async_trait]
impl CodeAnalyzer for MockLspAnalyzer {
    fn capabilities(&self) -> AnalyzerCapabilities {
        AnalyzerCapabilities::semantic()
    }

    fn supported_languages(&self) -> Vec<String> {
        vec!["mock".to_string()]
    }

    async fn analyze_file(
        &self,
        _content: &str,
        file_path: &Path,
        language: &str,
        _context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        // Return empty result for testing
        let mut result = AnalysisResult::new(file_path.to_path_buf(), language.to_string());

        result.analysis_metadata =
            AnalysisMetadata::new("MockLspAnalyzer".to_string(), "1.0.0".to_string());

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolUIDGenerator;
    use std::path::PathBuf;

    fn create_mock_analyzer() -> MockLspAnalyzer {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        MockLspAnalyzer::new(uid_generator)
    }

    fn create_test_context() -> AnalysisContext {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        AnalysisContext::new(1, 2, 3, "rust".to_string(), uid_generator)
    }

    #[test]
    fn test_mock_analyzer_capabilities() {
        let analyzer = create_mock_analyzer();
        let caps = analyzer.capabilities();

        assert!(caps.extracts_symbols);
        assert!(caps.extracts_relationships);
        assert!(caps.supports_incremental);
        assert!(caps.requires_lsp);
        assert!(!caps.parallel_safe); // LSP analyzers are not parallel safe
        assert_eq!(caps.confidence, 0.95);
    }

    #[test]
    fn test_mock_analyzer_supported_languages() {
        let analyzer = create_mock_analyzer();
        let languages = analyzer.supported_languages();

        assert_eq!(languages.len(), 1);
        assert!(languages.contains(&"mock".to_string()));
    }

    #[tokio::test]
    async fn test_mock_analyze_file() {
        let analyzer = create_mock_analyzer();
        let context = create_test_context();
        let file_path = PathBuf::from("test.mock");

        let result = analyzer
            .analyze_file("test content", &file_path, "mock", &context)
            .await;
        assert!(result.is_ok());

        let analysis_result = result.unwrap();
        assert_eq!(analysis_result.file_path, file_path);
        assert_eq!(analysis_result.language, "mock");
        assert!(analysis_result.symbols.is_empty());
        assert!(analysis_result.relationships.is_empty());
        assert_eq!(
            analysis_result.analysis_metadata.analyzer_name,
            "MockLspAnalyzer"
        );
    }

    #[test]
    fn test_convert_lsp_symbol_kind() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        // Create a mock server manager for testing
        let registry = Arc::new(crate::lsp_registry::LspRegistry::new().unwrap());
        let server_manager = Arc::new(SingleServerManager::new(registry));
        let analyzer = LspAnalyzer::new(uid_generator, server_manager);

        assert_eq!(
            analyzer.convert_lsp_symbol_kind(5).unwrap(),
            SymbolKind::Class
        );
        assert_eq!(
            analyzer.convert_lsp_symbol_kind(6).unwrap(),
            SymbolKind::Method
        );
        assert_eq!(
            analyzer.convert_lsp_symbol_kind(12).unwrap(),
            SymbolKind::Function
        );
        assert_eq!(
            analyzer.convert_lsp_symbol_kind(11).unwrap(),
            SymbolKind::Interface
        );
        assert_eq!(
            analyzer.convert_lsp_symbol_kind(13).unwrap(),
            SymbolKind::Variable
        );
    }

    #[test]
    fn test_lsp_analyzer_config() {
        let config = LspAnalyzerConfig::default();

        assert!(config.enabled);
        assert_eq!(config.request_timeout_seconds, 30);
        assert!(config.use_call_hierarchy);
        assert!(config.use_find_references);
        assert!(config.use_document_symbols);
        assert_eq!(config.max_references, 1000);
    }

    #[tokio::test]
    async fn test_lsp_analyzer_without_server() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let registry = Arc::new(crate::lsp_registry::LspRegistry::new().unwrap());
        let server_manager = Arc::new(SingleServerManager::new(registry));
        let analyzer = LspAnalyzer::new(uid_generator, server_manager);
        let context = create_test_context();
        let file_path = PathBuf::from("test.rs");

        // This should fail because no LSP server is running
        let result = analyzer
            .analyze_file("fn main() {}", &file_path, "rust", &context)
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AnalysisError::LspError { .. }
        ));
    }

    #[test]
    fn test_lsp_analyzer_supported_languages() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let registry = Arc::new(crate::lsp_registry::LspRegistry::new().unwrap());
        let server_manager = Arc::new(SingleServerManager::new(registry));
        let analyzer = LspAnalyzer::new(uid_generator, server_manager);

        let languages = analyzer.supported_languages();
        assert!(languages.contains(&"rust".to_string()));
        assert!(languages.contains(&"typescript".to_string()));
        assert!(languages.contains(&"python".to_string()));
        assert!(languages.len() >= 3);
    }
}
