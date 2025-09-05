//! Language-Specific Analyzers
//!
//! This module provides specialized analyzers for different programming languages.
//! Each analyzer is tailored to understand the specific constructs, patterns, and
//! idioms of its target language, providing enhanced analysis quality.

pub mod generic;
pub mod python;
pub mod rust;
pub mod typescript;

// Re-export all language-specific analyzers
pub use generic::GenericAnalyzer;
pub use python::PythonAnalyzer;
pub use rust::RustAnalyzer;
pub use typescript::TypeScriptAnalyzer;

use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

use super::framework::CodeAnalyzer;
use super::types::*;
use crate::symbol::SymbolUIDGenerator;

/// Factory for creating language-specific analyzers
pub struct LanguageAnalyzerFactory;

impl LanguageAnalyzerFactory {
    /// Create an analyzer for the specified language
    pub fn create_analyzer(
        language: &str,
        uid_generator: Arc<SymbolUIDGenerator>,
    ) -> Box<dyn CodeAnalyzer + Send + Sync> {
        match language.to_lowercase().as_str() {
            "rust" => Box::new(RustAnalyzer::new(uid_generator)),
            "typescript" | "ts" => Box::new(TypeScriptAnalyzer::new(uid_generator)),
            "javascript" | "js" => Box::new(TypeScriptAnalyzer::new(uid_generator)), // JS uses TS analyzer
            "python" | "py" => Box::new(PythonAnalyzer::new(uid_generator)),
            _ => Box::new(GenericAnalyzer::new(uid_generator)),
        }
    }

    /// Get list of supported languages with specialized analyzers
    pub fn supported_languages() -> Vec<String> {
        vec![
            "rust".to_string(),
            "typescript".to_string(),
            "javascript".to_string(),
            "python".to_string(),
        ]
    }

    /// Check if a language has a specialized analyzer
    pub fn has_specialized_analyzer(language: &str) -> bool {
        Self::supported_languages().contains(&language.to_lowercase())
    }
}

/// Base trait for language-specific analyzers
///
/// This trait extends the basic CodeAnalyzer with language-specific functionality
/// that might be useful for certain languages but not others.
#[async_trait]
pub trait LanguageSpecificAnalyzer: CodeAnalyzer {
    /// Get language-specific analysis features
    fn language_features(&self) -> LanguageFeatures;

    /// Extract language-specific metadata
    async fn extract_language_metadata(
        &self,
        content: &str,
        file_path: &Path,
        context: &AnalysisContext,
    ) -> Result<LanguageMetadata, AnalysisError>;

    /// Validate language-specific syntax patterns
    fn validate_language_patterns(&self, content: &str) -> Vec<String>;

    /// Get language-specific symbol priority modifiers
    fn get_symbol_priority_modifier(&self, _symbol: &ExtractedSymbol) -> f32 {
        // Default implementation - no modification
        1.0
    }
}

/// Language-specific features and capabilities
#[derive(Debug, Clone, Default)]
pub struct LanguageFeatures {
    /// Whether the language supports generic types/templates
    pub supports_generics: bool,

    /// Whether the language supports inheritance
    pub supports_inheritance: bool,

    /// Whether the language supports interfaces/traits
    pub supports_interfaces: bool,

    /// Whether the language supports operator overloading
    pub supports_operator_overloading: bool,

    /// Whether the language supports macros/meta-programming
    pub supports_macros: bool,

    /// Whether the language supports closures/lambdas
    pub supports_closures: bool,

    /// Whether the language supports modules/namespaces
    pub supports_modules: bool,

    /// Whether the language has strict typing
    pub is_statically_typed: bool,

    /// Common file extensions for this language
    pub file_extensions: Vec<String>,

    /// Test file patterns
    pub test_patterns: Vec<String>,
}

/// Language-specific metadata extracted from analysis
#[derive(Debug, Clone, Default)]
pub struct LanguageMetadata {
    /// Language version information (if detectable)
    pub language_version: Option<String>,

    /// Framework/library information detected
    pub frameworks: Vec<String>,

    /// Import/dependency information
    pub imports: Vec<String>,

    /// Language-specific quality metrics
    pub metrics: LanguageMetrics,

    /// Language-specific warnings
    pub warnings: Vec<String>,
}

/// Language-specific quality metrics
#[derive(Debug, Clone, Default)]
pub struct LanguageMetrics {
    /// Estimated code complexity (language-specific calculation)
    pub complexity_score: f32,

    /// Test coverage indicators
    pub test_indicators: u32,

    /// Documentation coverage
    pub documentation_ratio: f32,

    /// Language-specific best practice violations
    pub style_violations: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolUIDGenerator;

    #[test]
    fn test_language_analyzer_factory() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());

        // Test Rust analyzer creation
        let rust_analyzer = LanguageAnalyzerFactory::create_analyzer("rust", uid_generator.clone());
        assert!(rust_analyzer.can_analyze_language("rust"));

        // Test TypeScript analyzer creation
        let ts_analyzer =
            LanguageAnalyzerFactory::create_analyzer("typescript", uid_generator.clone());
        assert!(ts_analyzer.can_analyze_language("typescript"));

        // Test JavaScript uses TypeScript analyzer
        let js_analyzer =
            LanguageAnalyzerFactory::create_analyzer("javascript", uid_generator.clone());
        assert!(js_analyzer.can_analyze_language("javascript"));

        // Test Python analyzer creation
        let py_analyzer = LanguageAnalyzerFactory::create_analyzer("python", uid_generator.clone());
        assert!(py_analyzer.can_analyze_language("python"));

        // Test generic analyzer for unknown language
        let generic_analyzer =
            LanguageAnalyzerFactory::create_analyzer("unknown", uid_generator.clone());
        assert!(
            generic_analyzer.supported_languages().is_empty()
                || generic_analyzer.can_analyze_language("unknown")
        );
    }

    #[test]
    fn test_supported_languages() {
        let languages = LanguageAnalyzerFactory::supported_languages();
        assert!(languages.contains(&"rust".to_string()));
        assert!(languages.contains(&"typescript".to_string()));
        assert!(languages.contains(&"javascript".to_string()));
        assert!(languages.contains(&"python".to_string()));
    }

    #[test]
    fn test_has_specialized_analyzer() {
        assert!(LanguageAnalyzerFactory::has_specialized_analyzer("rust"));
        assert!(LanguageAnalyzerFactory::has_specialized_analyzer("RUST")); // Case insensitive
        assert!(LanguageAnalyzerFactory::has_specialized_analyzer(
            "typescript"
        ));
        assert!(LanguageAnalyzerFactory::has_specialized_analyzer("python"));
        assert!(!LanguageAnalyzerFactory::has_specialized_analyzer(
            "unknown"
        ));
    }

    #[test]
    fn test_language_features_default() {
        let features = LanguageFeatures::default();
        assert!(!features.supports_generics);
        assert!(!features.supports_inheritance);
        assert!(!features.supports_interfaces);
        assert!(features.file_extensions.is_empty());
    }

    #[test]
    fn test_language_metadata_default() {
        let metadata = LanguageMetadata::default();
        assert!(metadata.language_version.is_none());
        assert!(metadata.frameworks.is_empty());
        assert!(metadata.imports.is_empty());
        assert_eq!(metadata.metrics.complexity_score, 0.0);
    }

    #[test]
    fn test_language_metrics_default() {
        let metrics = LanguageMetrics::default();
        assert_eq!(metrics.complexity_score, 0.0);
        assert_eq!(metrics.test_indicators, 0);
        assert_eq!(metrics.documentation_ratio, 0.0);
        assert_eq!(metrics.style_violations, 0);
    }
}
