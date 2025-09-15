//! Tree-sitter Relationship Extraction Framework
//!
//! This module provides comprehensive relationship extraction using tree-sitter AST analysis
//! to detect structural relationships between symbols in source code. It supports multiple
//! programming languages with extensible patterns and query-based extraction.
//!
//! # Architecture
//!
//! The relationship extraction framework consists of several key components:
//!
//! * **TreeSitterRelationshipExtractor** - Main coordinator for relationship extraction
//! * **StructuralAnalyzer** - Pattern-based analysis using language-specific patterns
//! * **PatternRegistry** - Registry of language-specific relationship detection patterns
//! * **Language Patterns** - Specialized extractors for major programming languages
//! * **QueryCompiler** - Tree-sitter query compilation and execution
//!
//! # Supported Relationship Types
//!
//! The framework can detect various types of relationships:
//!
//! - **Containment**: parent-child relationships (class contains method, struct contains field)
//! - **Inheritance**: class inheritance and interface implementation
//! - **Calls**: function and method call relationships
//! - **Imports**: module and dependency relationships
//! - **References**: symbol references and usage
//!
//! # Language Support
//!
//! Built-in support for major programming languages:
//! - Rust: trait implementations, struct fields, use statements, impl blocks
//! - TypeScript/JavaScript: class inheritance, interface implementation, imports, method calls
//! - Python: class inheritance, method calls, imports, decorators
//! - Generic: fallback patterns for unsupported languages
//!
//! # Usage Example
//!
//! ```rust
//! use relationship::{TreeSitterRelationshipExtractor, RelationshipExtractionConfig};
//! use symbol::SymbolUIDGenerator;
//!
//! // Create relationship extractor
//! let uid_generator = Arc::new(SymbolUIDGenerator::new());
//! let extractor = TreeSitterRelationshipExtractor::new(uid_generator);
//!
//! // Parse source code with tree-sitter
//! let mut parser = tree_sitter::Parser::new();
//! parser.set_language(tree_sitter_rust::language()).unwrap();
//! let tree = parser.parse(source_code, None).unwrap();
//!
//! // Extract relationships
//! let relationships = extractor.extract_relationships(
//!     &tree,
//!     source_code,
//!     &file_path,
//!     "rust",
//!     &symbols,
//!     &context
//! ).await?;
//! ```
//!
//! # Integration
//!
//! This module integrates with:
//! - **Phase 3.1**: Uses SymbolUIDGenerator for consistent symbol identification
//! - **Phase 3.2**: Extends TreeSitterAnalyzer with relationship extraction capabilities
//! - **Database**: Converts relationships to database Edge types for storage
//! - **Analyzer Framework**: Provides relationship extraction for the multi-language analyzer

pub mod language_patterns;
pub mod lsp_client_wrapper;
pub mod lsp_enhancer;
pub mod merger;
pub mod structural_analyzer;
pub mod tree_sitter_extractor;
pub mod types;

// Re-export public types and traits
pub use language_patterns::*;
pub use lsp_client_wrapper::*;
pub use lsp_enhancer::*;
pub use merger::*;
pub use structural_analyzer::*;
pub use tree_sitter_extractor::*;
pub use types::*;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::analyzer::types::{AnalysisContext, ExtractedRelationship, ExtractedSymbol};
use crate::symbol::SymbolUIDGenerator;

/// Convenience function to create a relationship extractor with default configuration
pub fn create_relationship_extractor(
    uid_generator: Arc<SymbolUIDGenerator>,
) -> TreeSitterRelationshipExtractor {
    TreeSitterRelationshipExtractor::new(uid_generator)
}

/// Convenience function to create a relationship extractor with performance-optimized configuration
pub fn create_performance_relationship_extractor(
    uid_generator: Arc<SymbolUIDGenerator>,
) -> TreeSitterRelationshipExtractor {
    let config = RelationshipExtractionConfig::performance();
    TreeSitterRelationshipExtractor::with_config(uid_generator, config)
}

/// Convenience function to create a relationship extractor with completeness-optimized configuration
pub fn create_completeness_relationship_extractor(
    uid_generator: Arc<SymbolUIDGenerator>,
) -> TreeSitterRelationshipExtractor {
    let config = RelationshipExtractionConfig::completeness();
    TreeSitterRelationshipExtractor::with_config(uid_generator, config)
}

/// Extract relationships from parsed tree with given symbols
///
/// This is a convenience function that wraps the main relationship extraction functionality
/// for easy integration with existing analyzers.
pub async fn extract_relationships_from_tree(
    tree: &tree_sitter::Tree,
    content: &str,
    file_path: &Path,
    language: &str,
    symbols: &[ExtractedSymbol],
    context: &AnalysisContext,
    uid_generator: Arc<SymbolUIDGenerator>,
) -> Result<Vec<ExtractedRelationship>, RelationshipError> {
    let extractor = TreeSitterRelationshipExtractor::new(uid_generator);
    extractor
        .extract_relationships(tree, content, file_path, language, symbols, context)
        .await
}

/// Batch extract relationships for multiple files
///
/// This function provides efficient batch processing for relationship extraction
/// across multiple files, with shared parser pools and pattern registries.
pub async fn batch_extract_relationships(
    files: Vec<(
        tree_sitter::Tree,
        String,
        std::path::PathBuf,
        String,
        Vec<ExtractedSymbol>,
    )>,
    context: &AnalysisContext,
    uid_generator: Arc<SymbolUIDGenerator>,
    config: Option<RelationshipExtractionConfig>,
) -> Result<Vec<(std::path::PathBuf, Vec<ExtractedRelationship>)>, RelationshipError> {
    let extractor = if let Some(config) = config {
        TreeSitterRelationshipExtractor::with_config(uid_generator, config)
    } else {
        TreeSitterRelationshipExtractor::new(uid_generator)
    };

    let mut results = Vec::new();

    for (tree, content, file_path, language, symbols) in files {
        let relationships = extractor
            .extract_relationships(&tree, &content, &file_path, &language, &symbols, context)
            .await?;

        results.push((file_path, relationships));
    }

    Ok(results)
}

/// Get statistics about relationship extraction for a given language
pub fn get_language_relationship_stats(language: &str) -> HashMap<String, usize> {
    let registry = PatternRegistry::new();
    let mut stats = HashMap::new();

    if let Some(patterns) = registry.get_patterns(language) {
        stats.insert(
            "containment_patterns".to_string(),
            patterns.containment_patterns.len(),
        );
        stats.insert(
            "inheritance_patterns".to_string(),
            patterns.inheritance_patterns.len(),
        );
        stats.insert("call_patterns".to_string(), patterns.call_patterns.len());
        stats.insert(
            "import_patterns".to_string(),
            patterns.import_patterns.len(),
        );
    } else {
        // Use generic patterns
        if let Some(generic_patterns) = registry.get_patterns("generic") {
            stats.insert(
                "containment_patterns".to_string(),
                generic_patterns.containment_patterns.len(),
            );
            stats.insert(
                "inheritance_patterns".to_string(),
                generic_patterns.inheritance_patterns.len(),
            );
            stats.insert(
                "call_patterns".to_string(),
                generic_patterns.call_patterns.len(),
            );
            stats.insert(
                "import_patterns".to_string(),
                generic_patterns.import_patterns.len(),
            );
        }
    }

    stats
}

/// Get list of supported languages for relationship extraction
pub fn supported_languages() -> Vec<String> {
    let registry = PatternRegistry::new();
    let mut languages = Vec::new();

    // Known supported languages
    if registry.get_patterns("rust").is_some() {
        languages.push("rust".to_string());
    }
    if registry.get_patterns("typescript").is_some() {
        languages.push("typescript".to_string());
    }
    if registry.get_patterns("javascript").is_some() {
        languages.push("javascript".to_string());
    }
    if registry.get_patterns("python").is_some() {
        languages.push("python".to_string());
    }

    // Always include generic fallback
    languages.push("generic".to_string());

    languages
}

/// Check if a language is supported for relationship extraction
pub fn is_language_supported(language: &str) -> bool {
    let registry = PatternRegistry::new();
    registry.get_patterns(language).is_some() || registry.get_patterns("generic").is_some()
}

/// Create language-specific configuration for relationship extraction
pub fn create_language_config(language: &str) -> RelationshipExtractionConfig {
    let mut config = RelationshipExtractionConfig::default();

    // Language-specific optimizations
    match language.to_lowercase().as_str() {
        "rust" => {
            // Rust has comprehensive type information, increase confidence
            config.min_confidence = 0.8;
            config.extract_inheritance = true;
            config.extract_containment = true;
        }
        "typescript" | "javascript" => {
            // TypeScript has good type information
            config.min_confidence = 0.7;
            config.extract_inheritance = true;
            config.extract_imports = true;
        }
        "python" => {
            // Python is dynamically typed, lower confidence
            config.min_confidence = 0.6;
            config.extract_inheritance = true;
            config.extract_imports = true;
        }
        "c" | "cpp" | "c++" => {
            // C/C++ focus on structural relationships
            config.extract_containment = true;
            config.extract_calls = true;
            config.extract_inheritance = false; // Less common in C
        }
        _ => {
            // Generic configuration
            config.min_confidence = 0.5;
        }
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolUIDGenerator;
    use std::path::PathBuf;

    #[test]
    fn test_create_relationship_extractor() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let extractor = create_relationship_extractor(uid_generator);

        // Should create with default configuration
        assert_eq!(extractor.config().max_depth, 10);
        assert_eq!(extractor.config().min_confidence, 0.5);
    }

    #[test]
    fn test_create_performance_extractor() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let extractor = create_performance_relationship_extractor(uid_generator);

        // Should create with performance configuration
        assert_eq!(extractor.config().max_depth, 5);
        assert_eq!(extractor.config().min_confidence, 0.7);
        assert!(!extractor.config().extract_cross_file);
    }

    #[test]
    fn test_create_completeness_extractor() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let extractor = create_completeness_relationship_extractor(uid_generator);

        // Should create with completeness configuration
        assert_eq!(extractor.config().max_depth, 20);
        assert_eq!(extractor.config().min_confidence, 0.3);
        assert!(extractor.config().extract_cross_file);
    }

    #[test]
    fn test_supported_languages() {
        let languages = supported_languages();

        // Should always include generic
        assert!(languages.contains(&"generic".to_string()));

        // Should include registered languages
        assert!(!languages.is_empty());
    }

    #[test]
    fn test_language_support_check() {
        assert!(is_language_supported("generic"));
        assert!(is_language_supported("rust"));
        assert!(is_language_supported("typescript"));
        assert!(is_language_supported("python"));

        // Unknown languages should still be supported via generic fallback
        assert!(is_language_supported("unknown_language"));
    }

    #[test]
    fn test_language_relationship_stats() {
        let rust_stats = get_language_relationship_stats("rust");
        assert!(rust_stats.contains_key("containment_patterns"));
        assert!(rust_stats.contains_key("inheritance_patterns"));
        assert!(rust_stats.contains_key("call_patterns"));
        assert!(rust_stats.contains_key("import_patterns"));

        // Rust should have patterns for all categories
        assert!(rust_stats["containment_patterns"] > 0);
        assert!(rust_stats["inheritance_patterns"] > 0);
        assert!(rust_stats["call_patterns"] > 0);
        assert!(rust_stats["import_patterns"] > 0);
    }

    #[test]
    fn test_language_specific_config() {
        let rust_config = create_language_config("rust");
        assert_eq!(rust_config.min_confidence, 0.8);
        assert!(rust_config.extract_inheritance);
        assert!(rust_config.extract_containment);

        let typescript_config = create_language_config("typescript");
        assert_eq!(typescript_config.min_confidence, 0.7);
        assert!(typescript_config.extract_inheritance);
        assert!(typescript_config.extract_imports);

        let python_config = create_language_config("python");
        assert_eq!(python_config.min_confidence, 0.6);

        let generic_config = create_language_config("unknown");
        assert_eq!(generic_config.min_confidence, 0.5);
    }

    #[tokio::test]
    async fn test_batch_extract_relationships_empty() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let context = AnalysisContext::new(
            1,
            2,
            "rust".to_string(),
            PathBuf::from("."),
            PathBuf::from("test.rs"),
            uid_generator.clone(),
        );

        let files = Vec::new();
        let results = batch_extract_relationships(files, &context, uid_generator, None).await;

        assert!(results.is_ok());
        assert!(results.unwrap().is_empty());
    }
}
