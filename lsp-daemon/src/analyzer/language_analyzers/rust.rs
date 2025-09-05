//! Rust Language Analyzer
//!
//! This module provides a specialized analyzer for Rust code that understands
//! Rust-specific constructs, patterns, and idioms.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::super::framework::{AnalyzerCapabilities, CodeAnalyzer};
use super::super::tree_sitter_analyzer::TreeSitterAnalyzer;
use super::super::types::*;
use super::{LanguageFeatures, LanguageMetadata, LanguageMetrics, LanguageSpecificAnalyzer};
use crate::symbol::{SymbolKind, SymbolUIDGenerator};

/// Rust-specific code analyzer
///
/// This analyzer extends the base TreeSitter analyzer with Rust-specific
/// knowledge and patterns for enhanced analysis quality.
pub struct RustAnalyzer {
    /// Base tree-sitter analyzer
    base_analyzer: TreeSitterAnalyzer,

    /// UID generator for consistent symbol identification
    uid_generator: Arc<SymbolUIDGenerator>,
}

impl RustAnalyzer {
    /// Create a new Rust analyzer
    pub fn new(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        let base_analyzer = TreeSitterAnalyzer::new(uid_generator.clone());

        Self {
            base_analyzer,
            uid_generator,
        }
    }

    /// Extract Rust-specific symbols with enhanced information
    fn enhance_rust_symbols(&self, mut symbols: Vec<ExtractedSymbol>) -> Vec<ExtractedSymbol> {
        for symbol in &mut symbols {
            // Add Rust-specific metadata
            match symbol.kind {
                SymbolKind::Trait => {
                    symbol.tags.push("trait".to_string());
                    // Traits are high-priority in Rust
                    symbol.metadata.insert(
                        "rust_priority".to_string(),
                        serde_json::Value::String("critical".to_string()),
                    );
                }
                SymbolKind::Struct => {
                    symbol.tags.push("struct".to_string());
                    // Check if it's a unit struct, tuple struct, etc.
                    if let Some(sig) = &symbol.signature {
                        if sig.contains("()") {
                            symbol.tags.push("unit_struct".to_string());
                        } else if sig.contains("(") && !sig.contains("{") {
                            symbol.tags.push("tuple_struct".to_string());
                        }
                    }
                }
                SymbolKind::Enum => {
                    symbol.tags.push("enum".to_string());
                    // Enums are important in Rust pattern matching
                    symbol.metadata.insert(
                        "rust_pattern_matching".to_string(),
                        serde_json::Value::Bool(true),
                    );
                }
                SymbolKind::Function => {
                    // Detect special Rust functions
                    if symbol.name == "main" {
                        symbol.tags.push("entry_point".to_string());
                        symbol.metadata.insert(
                            "rust_priority".to_string(),
                            serde_json::Value::String("critical".to_string()),
                        );
                    } else if symbol.name.starts_with("test_")
                        || symbol.tags.contains(&"test".to_string())
                    {
                        symbol.tags.push("test_function".to_string());
                    } else if symbol.name == "new" || symbol.name == "default" {
                        symbol.tags.push("constructor".to_string());
                    }

                    // Detect async functions
                    if let Some(sig) = &symbol.signature {
                        if sig.contains("async fn") {
                            symbol.tags.push("async".to_string());
                            symbol
                                .metadata
                                .insert("rust_async".to_string(), serde_json::Value::Bool(true));
                        }

                        // Detect unsafe functions
                        if sig.contains("unsafe fn") {
                            symbol.tags.push("unsafe".to_string());
                            symbol
                                .metadata
                                .insert("rust_unsafe".to_string(), serde_json::Value::Bool(true));
                        }

                        // Detect generic functions
                        if sig.contains("<") && sig.contains(">") {
                            symbol.tags.push("generic".to_string());
                            symbol
                                .metadata
                                .insert("rust_generic".to_string(), serde_json::Value::Bool(true));
                        }
                    }
                }
                SymbolKind::Macro => {
                    symbol.tags.push("macro".to_string());
                    // Distinguish between different macro types
                    if symbol.name.ends_with("!") {
                        symbol.tags.push("declarative_macro".to_string());
                    }
                    // Macros are important in Rust metaprogramming
                    symbol.metadata.insert(
                        "rust_metaprogramming".to_string(),
                        serde_json::Value::Bool(true),
                    );
                }
                SymbolKind::Module => {
                    symbol.tags.push("module".to_string());
                    // Module hierarchy is crucial in Rust
                    symbol.metadata.insert(
                        "rust_module_system".to_string(),
                        serde_json::Value::Bool(true),
                    );
                }
                _ => {}
            }

            // Add visibility information
            if let Some(visibility) = &symbol.visibility {
                symbol.metadata.insert(
                    "rust_visibility".to_string(),
                    serde_json::Value::String(visibility.to_string()),
                );
            }
        }

        symbols
    }

    /// Extract Rust-specific relationships
    fn extract_rust_relationships(
        &self,
        symbols: &[ExtractedSymbol],
    ) -> Vec<ExtractedRelationship> {
        let mut relationships = Vec::new();

        // Build symbol lookup for efficient relationship creation
        let _symbol_lookup: HashMap<String, &ExtractedSymbol> =
            symbols.iter().map(|s| (s.name.clone(), s)).collect();

        for symbol in symbols {
            // Extract trait implementations
            if symbol.kind == SymbolKind::Struct || symbol.kind == SymbolKind::Enum {
                if let Some(sig) = &symbol.signature {
                    // Look for "impl TraitName for StructName" patterns
                    // This would require more sophisticated parsing in a real implementation
                    if sig.contains("impl") {
                        // Create implementation relationship
                        // For now, this is a simplified example
                    }
                }
            }

            // Extract module relationships
            if symbol.kind == SymbolKind::Module {
                // Modules contain other symbols
                for other_symbol in symbols {
                    if other_symbol.parent_scope.as_ref() == Some(&symbol.name) {
                        let relationship = ExtractedRelationship::new(
                            symbol.uid.clone(),
                            other_symbol.uid.clone(),
                            RelationType::Contains,
                        )
                        .with_confidence(0.95);

                        relationships.push(relationship);
                    }
                }
            }

            // Extract use/import relationships
            if symbol.kind == SymbolKind::Import {
                // In Rust, 'use' statements create import relationships
                if let Some(qualified_name) = &symbol.qualified_name {
                    // Create import relationship
                    let relationship = ExtractedRelationship::new(
                        format!("file::{}", symbol.location.file_path.display()),
                        qualified_name.clone(),
                        RelationType::Imports,
                    )
                    .with_confidence(0.9);

                    relationships.push(relationship);
                }
            }
        }

        relationships
    }

    /// Calculate Rust-specific complexity metrics
    fn calculate_rust_complexity(&self, symbols: &[ExtractedSymbol]) -> f32 {
        let mut complexity = 0.0;

        for symbol in symbols {
            match symbol.kind {
                SymbolKind::Function => {
                    complexity += 1.0;

                    // Add complexity for generic functions
                    if symbol.tags.contains(&"generic".to_string()) {
                        complexity += 0.5;
                    }

                    // Add complexity for async functions
                    if symbol.tags.contains(&"async".to_string()) {
                        complexity += 0.3;
                    }

                    // Add complexity for unsafe functions
                    if symbol.tags.contains(&"unsafe".to_string()) {
                        complexity += 0.8;
                    }
                }
                SymbolKind::Trait => {
                    complexity += 1.5; // Traits add significant complexity
                }
                SymbolKind::Macro => {
                    complexity += 2.0; // Macros are complex
                }
                SymbolKind::Enum => {
                    complexity += 1.2; // Enums with pattern matching
                }
                _ => {}
            }
        }

        complexity
    }

    /// Detect Rust frameworks and libraries
    fn detect_rust_frameworks(&self, symbols: &[ExtractedSymbol]) -> Vec<String> {
        let mut frameworks = Vec::new();

        for symbol in symbols {
            if symbol.kind == SymbolKind::Import {
                if let Some(qualified_name) = &symbol.qualified_name {
                    // Detect common Rust frameworks
                    if qualified_name.starts_with("tokio") {
                        frameworks.push("Tokio".to_string());
                    } else if qualified_name.starts_with("serde") {
                        frameworks.push("Serde".to_string());
                    } else if qualified_name.starts_with("reqwest") {
                        frameworks.push("Reqwest".to_string());
                    } else if qualified_name.starts_with("actix") {
                        frameworks.push("Actix".to_string());
                    } else if qualified_name.starts_with("rocket") {
                        frameworks.push("Rocket".to_string());
                    } else if qualified_name.starts_with("clap") {
                        frameworks.push("Clap".to_string());
                    } else if qualified_name.starts_with("diesel") {
                        frameworks.push("Diesel".to_string());
                    }
                }
            }
        }

        frameworks.sort();
        frameworks.dedup();
        frameworks
    }
}

#[async_trait]
impl CodeAnalyzer for RustAnalyzer {
    fn capabilities(&self) -> AnalyzerCapabilities {
        let mut caps = AnalyzerCapabilities::structural();
        caps.confidence = 0.9; // Higher confidence for specialized analyzer
        caps
    }

    fn supported_languages(&self) -> Vec<String> {
        vec!["rust".to_string()]
    }

    async fn analyze_file(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        // Use base analyzer first
        let mut result = self
            .base_analyzer
            .analyze_file(content, file_path, language, context)
            .await?;

        // Enhance with Rust-specific analysis
        result.symbols = self.enhance_rust_symbols(result.symbols);

        // Add Rust-specific relationships
        let rust_relationships = self.extract_rust_relationships(&result.symbols);
        result.relationships.extend(rust_relationships);

        // Update metadata to reflect Rust-specific analysis
        result.analysis_metadata.analyzer_name = "RustAnalyzer".to_string();
        result.analysis_metadata.add_metric(
            "rust_complexity".to_string(),
            self.calculate_rust_complexity(&result.symbols) as f64,
        );

        // Add framework detection
        let frameworks = self.detect_rust_frameworks(&result.symbols);
        if !frameworks.is_empty() {
            result
                .analysis_metadata
                .add_metric("detected_frameworks".to_string(), frameworks.len() as f64);
            result.analysis_metadata.custom.insert(
                "rust_frameworks".to_string(),
                serde_json::Value::Array(
                    frameworks
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        Ok(result)
    }
}

#[async_trait]
impl LanguageSpecificAnalyzer for RustAnalyzer {
    fn language_features(&self) -> LanguageFeatures {
        LanguageFeatures {
            supports_generics: true,
            supports_inheritance: false, // Rust uses composition over inheritance
            supports_interfaces: true,   // Traits
            supports_operator_overloading: true,
            supports_macros: true,
            supports_closures: true,
            supports_modules: true,
            is_statically_typed: true,
            file_extensions: vec![".rs".to_string()],
            test_patterns: vec![
                "*test*.rs".to_string(),
                "tests/**/*.rs".to_string(),
                "#[test]".to_string(),
                "#[cfg(test)]".to_string(),
            ],
        }
    }

    async fn extract_language_metadata(
        &self,
        _content: &str,
        _file_path: &Path,
        _context: &AnalysisContext,
    ) -> Result<LanguageMetadata, AnalysisError> {
        // This would analyze the file for Rust-specific metadata
        // For now, return basic metadata
        Ok(LanguageMetadata {
            language_version: None, // Could parse from Cargo.toml
            frameworks: Vec::new(), // Would be detected from imports
            imports: Vec::new(),    // Would be extracted from use statements
            metrics: LanguageMetrics {
                complexity_score: 0.0,
                test_indicators: 0,
                documentation_ratio: 0.0,
                style_violations: 0,
            },
            warnings: Vec::new(),
        })
    }

    fn validate_language_patterns(&self, content: &str) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check for common Rust anti-patterns
        if content.contains(".unwrap()") && !content.contains("#[test]") {
            warnings.push("Consider using proper error handling instead of .unwrap()".to_string());
        }

        if content.contains("unsafe {") {
            warnings.push("Unsafe block detected - ensure memory safety".to_string());
        }

        if content.contains("todo!()") || content.contains("unimplemented!()") {
            warnings.push("Incomplete implementation detected".to_string());
        }

        warnings
    }

    fn get_symbol_priority_modifier(&self, symbol: &ExtractedSymbol) -> f32 {
        match symbol.kind {
            SymbolKind::Trait => 1.5, // Traits are very important in Rust
            SymbolKind::Enum => 1.3,  // Enums are important for pattern matching
            SymbolKind::Macro => 1.2, // Macros are important for metaprogramming
            SymbolKind::Function if symbol.name == "main" => 2.0, // Entry point
            SymbolKind::Function if symbol.tags.contains(&"test".to_string()) => 0.8, // Tests less important
            _ => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{SymbolLocation, SymbolUIDGenerator};
    use std::path::PathBuf;

    fn create_rust_analyzer() -> RustAnalyzer {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        RustAnalyzer::new(uid_generator)
    }

    fn create_test_context() -> AnalysisContext {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        AnalysisContext::new(1, 2, 3, "rust".to_string(), uid_generator)
    }

    fn create_test_symbol(name: &str, kind: SymbolKind) -> ExtractedSymbol {
        let location = SymbolLocation::new(PathBuf::from("test.rs"), 1, 0, 1, 10);
        ExtractedSymbol::new(format!("rust::{}", name), name.to_string(), kind, location)
    }

    #[test]
    fn test_rust_analyzer_capabilities() {
        let analyzer = create_rust_analyzer();
        let caps = analyzer.capabilities();

        assert!(caps.extracts_symbols);
        assert!(caps.extracts_relationships);
        assert!(!caps.requires_lsp);
        assert_eq!(caps.confidence, 0.9);
    }

    #[test]
    fn test_rust_analyzer_supported_languages() {
        let analyzer = create_rust_analyzer();
        let languages = analyzer.supported_languages();

        assert_eq!(languages.len(), 1);
        assert!(languages.contains(&"rust".to_string()));
    }

    #[test]
    fn test_enhance_rust_symbols() {
        let analyzer = create_rust_analyzer();

        let symbols = vec![
            create_test_symbol("MyTrait", SymbolKind::Trait),
            create_test_symbol("MyStruct", SymbolKind::Struct),
            create_test_symbol("main", SymbolKind::Function),
            create_test_symbol("my_macro", SymbolKind::Macro),
        ];

        let enhanced = analyzer.enhance_rust_symbols(symbols);

        // Check trait enhancement
        let trait_symbol = enhanced.iter().find(|s| s.name == "MyTrait").unwrap();
        assert!(trait_symbol.tags.contains(&"trait".to_string()));
        assert!(trait_symbol.metadata.contains_key("rust_priority"));

        // Check struct enhancement
        let struct_symbol = enhanced.iter().find(|s| s.name == "MyStruct").unwrap();
        assert!(struct_symbol.tags.contains(&"struct".to_string()));

        // Check main function enhancement
        let main_symbol = enhanced.iter().find(|s| s.name == "main").unwrap();
        assert!(main_symbol.tags.contains(&"entry_point".to_string()));

        // Check macro enhancement
        let macro_symbol = enhanced.iter().find(|s| s.name == "my_macro").unwrap();
        assert!(macro_symbol.tags.contains(&"macro".to_string()));
        assert!(macro_symbol.metadata.contains_key("rust_metaprogramming"));
    }

    #[test]
    fn test_calculate_rust_complexity() {
        let analyzer = create_rust_analyzer();

        let mut symbols = vec![
            create_test_symbol("regular_function", SymbolKind::Function),
            create_test_symbol("MyTrait", SymbolKind::Trait),
            create_test_symbol("my_macro", SymbolKind::Macro),
        ];

        // Add tags to simulate enhanced symbols
        symbols[0].tags.push("generic".to_string());
        symbols[0].tags.push("async".to_string());

        let complexity = analyzer.calculate_rust_complexity(&symbols);

        // Should be: 1.0 (function) + 0.5 (generic) + 0.3 (async) + 1.5 (trait) + 2.0 (macro) = 5.3
        assert!((complexity - 5.3).abs() < 0.1);
    }

    #[test]
    fn test_detect_rust_frameworks() {
        let analyzer = create_rust_analyzer();

        let mut symbols = vec![
            create_test_symbol("tokio::main", SymbolKind::Import)
                .with_qualified_name("tokio::main".to_string()),
            create_test_symbol("serde::Deserialize", SymbolKind::Import)
                .with_qualified_name("serde::Deserialize".to_string()),
            create_test_symbol("reqwest::Client", SymbolKind::Import)
                .with_qualified_name("reqwest::Client".to_string()),
        ];

        let frameworks = analyzer.detect_rust_frameworks(&symbols);

        assert!(frameworks.contains(&"Tokio".to_string()));
        assert!(frameworks.contains(&"Serde".to_string()));
        assert!(frameworks.contains(&"Reqwest".to_string()));
    }

    #[test]
    fn test_language_features() {
        let analyzer = create_rust_analyzer();
        let features = analyzer.language_features();

        assert!(features.supports_generics);
        assert!(!features.supports_inheritance);
        assert!(features.supports_interfaces); // Traits
        assert!(features.supports_operator_overloading);
        assert!(features.supports_macros);
        assert!(features.supports_closures);
        assert!(features.supports_modules);
        assert!(features.is_statically_typed);
        assert!(features.file_extensions.contains(&".rs".to_string()));
    }

    #[test]
    fn test_validate_language_patterns() {
        let analyzer = create_rust_analyzer();

        let code_with_issues = r#"
            fn main() {
                let value = some_function().unwrap();
                unsafe {
                    let ptr = std::ptr::null_mut();
                }
                todo!("Implement this");
            }
        "#;

        let warnings = analyzer.validate_language_patterns(code_with_issues);

        assert!(warnings.iter().any(|w| w.contains("unwrap")));
        assert!(warnings.iter().any(|w| w.contains("unsafe")));
        assert!(warnings
            .iter()
            .any(|w| w.contains("Incomplete implementation")));
    }

    #[test]
    fn test_symbol_priority_modifier() {
        let analyzer = create_rust_analyzer();

        let trait_symbol = create_test_symbol("MyTrait", SymbolKind::Trait);
        assert_eq!(analyzer.get_symbol_priority_modifier(&trait_symbol), 1.5);

        let enum_symbol = create_test_symbol("MyEnum", SymbolKind::Enum);
        assert_eq!(analyzer.get_symbol_priority_modifier(&enum_symbol), 1.3);

        let main_symbol = create_test_symbol("main", SymbolKind::Function);
        assert_eq!(analyzer.get_symbol_priority_modifier(&main_symbol), 2.0);

        let regular_symbol = create_test_symbol("regular", SymbolKind::Variable);
        assert_eq!(analyzer.get_symbol_priority_modifier(&regular_symbol), 1.0);
    }
}
