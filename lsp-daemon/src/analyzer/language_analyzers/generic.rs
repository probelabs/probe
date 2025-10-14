//! Generic Language Analyzer
//!
//! This module provides a generic analyzer that serves as a fallback for languages
//! that don't have specialized analyzers. It uses common programming patterns and
//! tree-sitter's generic AST analysis capabilities.

use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

use super::super::framework::{AnalyzerCapabilities, CodeAnalyzer};
use super::super::tree_sitter_analyzer::TreeSitterAnalyzer;
use super::super::types::*;
use super::{LanguageFeatures, LanguageMetadata, LanguageMetrics, LanguageSpecificAnalyzer};
use crate::symbol::{SymbolKind, SymbolUIDGenerator};

/// Generic code analyzer for unknown or unsupported languages
///
/// This analyzer provides basic structural analysis capabilities using
/// tree-sitter's generic node analysis and common programming patterns.
pub struct GenericAnalyzer {
    /// Base tree-sitter analyzer
    base_analyzer: TreeSitterAnalyzer,

    /// UID generator for consistent symbol identification
    uid_generator: Arc<SymbolUIDGenerator>,
}

impl GenericAnalyzer {
    /// Create a new generic analyzer
    pub fn new(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        let base_analyzer = TreeSitterAnalyzer::new(uid_generator.clone());

        Self {
            base_analyzer,
            uid_generator,
        }
    }

    /// Enhance symbols with generic patterns
    fn enhance_generic_symbols(&self, mut symbols: Vec<ExtractedSymbol>) -> Vec<ExtractedSymbol> {
        for symbol in &mut symbols {
            // Add generic metadata
            symbol.metadata.insert(
                "analyzer".to_string(),
                serde_json::Value::String("generic".to_string()),
            );

            // Apply generic naming pattern analysis
            self.analyze_naming_patterns(&mut *symbol);

            // Apply generic structural pattern analysis
            self.analyze_structural_patterns(&mut *symbol);
        }

        symbols
    }

    /// Analyze generic naming patterns
    fn analyze_naming_patterns(&self, symbol: &mut ExtractedSymbol) {
        let name = &symbol.name;

        // Detect common naming patterns
        if name
            .chars()
            .all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
        {
            symbol.tags.push("constant_naming".to_string());
            if symbol.kind == SymbolKind::Variable {
                symbol.kind = SymbolKind::Constant;
            }
        }

        // Detect test patterns
        if name.starts_with("test")
            || name.starts_with("Test")
            || name.ends_with("Test")
            || name.ends_with("Tests")
            || name.contains("_test")
            || name.contains("test_")
        {
            symbol.tags.push("test_related".to_string());
        }

        // Detect private/internal patterns
        if name.starts_with("_") || name.starts_with("__") {
            symbol.tags.push("private_naming".to_string());
        }

        // Detect common function patterns
        if symbol.kind == SymbolKind::Function {
            if name == "main" || name == "Main" {
                symbol.tags.push("entry_point".to_string());
            } else if name.starts_with("get") || name.starts_with("Get") {
                symbol.tags.push("getter".to_string());
            } else if name.starts_with("set") || name.starts_with("Set") {
                symbol.tags.push("setter".to_string());
            } else if name.starts_with("is")
                || name.starts_with("Is")
                || name.starts_with("has")
                || name.starts_with("Has")
            {
                symbol.tags.push("predicate".to_string());
            } else if name.starts_with("create")
                || name.starts_with("Create")
                || name.starts_with("new")
                || name.starts_with("New")
                || name.starts_with("make")
                || name.starts_with("Make")
            {
                symbol.tags.push("factory".to_string());
            }
        }

        // Detect interface patterns
        if symbol.kind == SymbolKind::Interface || symbol.kind == SymbolKind::Class {
            if name.starts_with("I")
                && name.len() > 1
                && name.chars().nth(1).unwrap().is_uppercase()
            {
                symbol.tags.push("interface_naming".to_string());
            }
        }

        // Detect exception patterns
        if symbol.kind == SymbolKind::Class {
            if name.ends_with("Exception") || name.ends_with("Error") {
                symbol.tags.push("exception_class".to_string());
            }
        }
    }

    /// Analyze generic structural patterns
    fn analyze_structural_patterns(&self, symbol: &mut ExtractedSymbol) {
        // Analyze signature patterns if available
        if let Some(signature) = symbol.signature.clone() {
            self.analyze_signature_patterns(symbol, &signature);
        }

        // Add location-based metadata
        let file_path = &symbol.location.file_path;
        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
            if file_name.contains("test") {
                symbol.tags.push("test_file".to_string());
            }

            if file_name.contains("spec") {
                symbol.tags.push("spec_file".to_string());
            }

            if file_name.starts_with("index") || file_name.starts_with("main") {
                symbol.tags.push("main_file".to_string());
            }
        }
    }

    /// Analyze signature patterns
    fn analyze_signature_patterns(&self, symbol: &mut ExtractedSymbol, signature: &str) {
        let sig_lower = signature.to_lowercase();

        // Generic async patterns
        if sig_lower.contains("async") || sig_lower.contains("await") {
            symbol.tags.push("async_pattern".to_string());
        }

        // Generic generic patterns (templates/generics)
        if signature.contains("<") && signature.contains(">") {
            symbol.tags.push("generic_pattern".to_string());
        }

        // Generic annotation patterns
        if signature.contains("@") {
            symbol.tags.push("annotated".to_string());
        }

        // Generic visibility patterns
        if sig_lower.contains("public") {
            symbol.tags.push("public".to_string());
        } else if sig_lower.contains("private") {
            symbol.tags.push("private".to_string());
        } else if sig_lower.contains("protected") {
            symbol.tags.push("protected".to_string());
        }

        // Generic static patterns
        if sig_lower.contains("static") {
            symbol.tags.push("static".to_string());
        }

        // Generic abstract patterns
        if sig_lower.contains("abstract") {
            symbol.tags.push("abstract".to_string());
        }

        // Generic final patterns
        if sig_lower.contains("final") {
            symbol.tags.push("final".to_string());
        }

        // Generic const patterns
        if sig_lower.contains("const") {
            symbol.tags.push("const".to_string());
        }
    }

    /// Calculate generic complexity metrics
    fn calculate_generic_complexity(&self, symbols: &[ExtractedSymbol]) -> f32 {
        let mut complexity = 0.0;

        for symbol in symbols {
            match symbol.kind {
                SymbolKind::Function | SymbolKind::Method => {
                    complexity += 1.0;

                    // Add complexity for generic patterns
                    if symbol.tags.contains(&"generic_pattern".to_string()) {
                        complexity += 0.5;
                    }

                    if symbol.tags.contains(&"async_pattern".to_string()) {
                        complexity += 0.3;
                    }

                    if symbol.tags.contains(&"annotated".to_string()) {
                        complexity += 0.2;
                    }
                }
                SymbolKind::Class | SymbolKind::Struct => {
                    complexity += 1.2;

                    if symbol.tags.contains(&"generic_pattern".to_string()) {
                        complexity += 0.6;
                    }

                    if symbol.tags.contains(&"abstract".to_string()) {
                        complexity += 0.4;
                    }
                }
                SymbolKind::Interface | SymbolKind::Trait => {
                    complexity += 1.1;
                }
                _ => {
                    complexity += 0.1;
                }
            }
        }

        complexity
    }

    /// Detect common patterns that might indicate language or framework
    fn detect_language_hints(&self, symbols: &[ExtractedSymbol]) -> Vec<String> {
        let mut hints = Vec::new();
        let mut detected = std::collections::HashSet::new();

        for symbol in symbols {
            // Check file extensions
            if let Some(ext) = symbol
                .location
                .file_path
                .extension()
                .and_then(|e| e.to_str())
            {
                match ext {
                    "rs" => {
                        detected.insert("Rust");
                    }
                    "go" => {
                        detected.insert("Go");
                    }
                    "kt" | "kts" => {
                        detected.insert("Kotlin");
                    }
                    "scala" => {
                        detected.insert("Scala");
                    }
                    "rb" => {
                        detected.insert("Ruby");
                    }
                    "php" => {
                        detected.insert("PHP");
                    }
                    "sh" | "bash" => {
                        detected.insert("Shell");
                    }
                    "ps1" => {
                        detected.insert("PowerShell");
                    }
                    "lua" => {
                        detected.insert("Lua");
                    }
                    "R" => {
                        detected.insert("R");
                    }
                    "jl" => {
                        detected.insert("Julia");
                    }
                    "hs" => {
                        detected.insert("Haskell");
                    }
                    "ml" => {
                        detected.insert("OCaml");
                    }
                    "elm" => {
                        detected.insert("Elm");
                    }
                    "ex" | "exs" => {
                        detected.insert("Elixir");
                    }
                    "erl" => {
                        detected.insert("Erlang");
                    }
                    "clj" | "cljs" => {
                        detected.insert("Clojure");
                    }
                    "fs" | "fsx" => {
                        detected.insert("F#");
                    }
                    "vb" => {
                        detected.insert("Visual Basic");
                    }
                    "pas" | "pp" => {
                        detected.insert("Pascal");
                    }
                    "d" => {
                        detected.insert("D");
                    }
                    "nim" => {
                        detected.insert("Nim");
                    }
                    "cr" => {
                        detected.insert("Crystal");
                    }
                    "dart" => {
                        detected.insert("Dart");
                    }
                    "swift" => {
                        detected.insert("Swift");
                    }
                    _ => {}
                }
            }

            // Check for language-specific patterns in symbol names
            if symbol.kind == SymbolKind::Import || symbol.kind == SymbolKind::Module {
                if let Some(qualified_name) = &symbol.qualified_name {
                    if qualified_name.starts_with("std::") || qualified_name.contains("::") {
                        detected.insert("Rust-like");
                    } else if qualified_name.starts_with("java.") || qualified_name.contains("com.")
                    {
                        detected.insert("Java-like");
                    } else if qualified_name.contains("/") {
                        detected.insert("JavaScript-like");
                    }
                }
            }

            // Check for common language-specific patterns
            if symbol.name == "main" && symbol.kind == SymbolKind::Function {
                detected.insert("C-family");
            } else if symbol.name == "initialize" || symbol.name == "finalize" {
                detected.insert("Object-oriented");
            } else if symbol.name.starts_with("__") && symbol.name.ends_with("__") {
                detected.insert("Python-like");
            }
        }

        hints.extend(detected.into_iter().map(String::from));
        hints.sort();
        hints
    }
}

#[async_trait]
impl CodeAnalyzer for GenericAnalyzer {
    fn capabilities(&self) -> AnalyzerCapabilities {
        let mut caps = AnalyzerCapabilities::structural();
        caps.confidence = 0.6; // Lower confidence for generic analysis
        caps
    }

    fn supported_languages(&self) -> Vec<String> {
        // Generic analyzer doesn't declare specific language support
        // It serves as a fallback
        vec![]
    }

    async fn analyze_file(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        // Try to use base analyzer first (might work with tree-sitter)
        let result = self
            .base_analyzer
            .analyze_file(content, file_path, language, context)
            .await;

        let mut final_result = match result {
            Ok(mut res) => {
                // Enhance with generic patterns
                res.symbols = self.enhance_generic_symbols(res.symbols);
                res
            }
            Err(_) => {
                // Base analyzer failed, create minimal result
                AnalysisResult::new(file_path.to_path_buf(), language.to_string())
            }
        };

        // Update metadata to reflect generic analysis
        final_result.analysis_metadata.analyzer_name = "GenericAnalyzer".to_string();
        final_result.analysis_metadata.add_metric(
            "generic_complexity".to_string(),
            self.calculate_generic_complexity(&final_result.symbols) as f64,
        );

        // Add language hints
        let language_hints = self.detect_language_hints(&final_result.symbols);
        if !language_hints.is_empty() {
            final_result.analysis_metadata.custom.insert(
                "language_hints".to_string(),
                serde_json::Value::Array(
                    language_hints
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        // Add generic warning about limited analysis
        final_result.analysis_metadata.add_warning(format!(
            "Generic analysis used for language '{}' - consider adding specialized analyzer",
            language
        ));

        Ok(final_result)
    }
}

#[async_trait]
impl LanguageSpecificAnalyzer for GenericAnalyzer {
    fn language_features(&self) -> LanguageFeatures {
        // Conservative generic features
        LanguageFeatures {
            supports_generics: false,             // Don't assume
            supports_inheritance: false,          // Don't assume
            supports_interfaces: false,           // Don't assume
            supports_operator_overloading: false, // Don't assume
            supports_macros: false,               // Don't assume
            supports_closures: false,             // Don't assume
            supports_modules: true,               // Most languages have some module system
            is_statically_typed: false,           // Don't assume
            file_extensions: vec![],              // Unknown
            test_patterns: vec![
                "*test*".to_string(),
                "*Test*".to_string(),
                "*spec*".to_string(),
                "*Spec*".to_string(),
            ],
        }
    }

    async fn extract_language_metadata(
        &self,
        _content: &str,
        _file_path: &Path,
        _context: &AnalysisContext,
    ) -> Result<LanguageMetadata, AnalysisError> {
        Ok(LanguageMetadata {
            language_version: None,
            frameworks: Vec::new(),
            imports: Vec::new(),
            metrics: LanguageMetrics {
                complexity_score: 0.0,
                test_indicators: 0,
                documentation_ratio: 0.0,
                style_violations: 0,
            },
            warnings: vec![
                "Generic analysis provides limited language-specific insights".to_string(),
                "Consider implementing a specialized analyzer for better results".to_string(),
            ],
        })
    }

    fn validate_language_patterns(&self, _content: &str) -> Vec<String> {
        vec!["Generic analyzer cannot provide language-specific pattern validation".to_string()]
    }

    fn get_symbol_priority_modifier(&self, symbol: &ExtractedSymbol) -> f32 {
        // Generic priority based on common patterns
        match symbol.kind {
            SymbolKind::Function | SymbolKind::Method => {
                if symbol.tags.contains(&"entry_point".to_string()) {
                    1.8 // Entry points are very important
                } else if symbol.tags.contains(&"test_related".to_string()) {
                    0.7 // Tests are less important
                } else if symbol.tags.contains(&"factory".to_string()) {
                    1.2 // Factory methods are important
                } else {
                    1.0
                }
            }
            SymbolKind::Class | SymbolKind::Struct => {
                if symbol.tags.contains(&"exception_class".to_string()) {
                    1.1 // Exception classes are moderately important
                } else {
                    1.2 // Classes are generally important
                }
            }
            SymbolKind::Interface | SymbolKind::Trait => 1.3, // Interfaces are important
            SymbolKind::Constant => 1.1,                      // Constants are moderately important
            SymbolKind::Variable if symbol.tags.contains(&"private_naming".to_string()) => 0.8,
            _ => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{SymbolLocation, SymbolUIDGenerator};
    use std::path::PathBuf;

    fn create_generic_analyzer() -> GenericAnalyzer {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        GenericAnalyzer::new(uid_generator)
    }

    fn create_test_context() -> AnalysisContext {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        AnalysisContext::new(
            1,
            2,
            "generic".to_string(),
            PathBuf::from("."),
            PathBuf::from("test.generic"),
            uid_generator,
        )
    }

    fn create_test_symbol(name: &str, kind: SymbolKind) -> ExtractedSymbol {
        let location = SymbolLocation::new(PathBuf::from("test.unknown"), 1, 0, 1, 10);
        ExtractedSymbol::new(
            format!("generic::{}", name),
            name.to_string(),
            kind,
            location,
        )
    }

    #[test]
    fn test_generic_analyzer_capabilities() {
        let analyzer = create_generic_analyzer();
        let caps = analyzer.capabilities();

        assert!(caps.extracts_symbols);
        assert!(caps.extracts_relationships);
        assert_eq!(caps.confidence, 0.6); // Lower confidence for generic analysis
    }

    #[test]
    fn test_generic_analyzer_supported_languages() {
        let analyzer = create_generic_analyzer();
        let languages = analyzer.supported_languages();

        // Generic analyzer doesn't declare specific language support
        assert!(languages.is_empty());
    }

    #[test]
    fn test_analyze_naming_patterns() {
        let analyzer = create_generic_analyzer();

        let mut symbols = vec![
            create_test_symbol("MY_CONSTANT", SymbolKind::Variable),
            create_test_symbol("main", SymbolKind::Function),
            create_test_symbol("test_function", SymbolKind::Function),
            create_test_symbol("_private_var", SymbolKind::Variable),
            create_test_symbol("getUserName", SymbolKind::Function),
            create_test_symbol("IInterface", SymbolKind::Interface),
            create_test_symbol("NetworkException", SymbolKind::Class),
        ];

        for symbol in &mut symbols {
            analyzer.analyze_naming_patterns(symbol);
        }

        // Check constant detection
        let constant = symbols.iter().find(|s| s.name == "MY_CONSTANT").unwrap();
        assert!(constant.tags.contains(&"constant_naming".to_string()));
        assert_eq!(constant.kind, SymbolKind::Constant);

        // Check entry point detection
        let main_func = symbols.iter().find(|s| s.name == "main").unwrap();
        assert!(main_func.tags.contains(&"entry_point".to_string()));

        // Check test detection
        let test_func = symbols.iter().find(|s| s.name == "test_function").unwrap();
        assert!(test_func.tags.contains(&"test_related".to_string()));

        // Check private naming detection
        let private_var = symbols.iter().find(|s| s.name == "_private_var").unwrap();
        assert!(private_var.tags.contains(&"private_naming".to_string()));

        // Check getter detection
        let getter = symbols.iter().find(|s| s.name == "getUserName").unwrap();
        assert!(getter.tags.contains(&"getter".to_string()));

        // Check interface naming convention
        let interface = symbols.iter().find(|s| s.name == "IInterface").unwrap();
        assert!(interface.tags.contains(&"interface_naming".to_string()));

        // Check exception class detection
        let exception = symbols
            .iter()
            .find(|s| s.name == "NetworkException")
            .unwrap();
        assert!(exception.tags.contains(&"exception_class".to_string()));
    }

    #[test]
    fn test_analyze_signature_patterns() {
        let analyzer = create_generic_analyzer();

        let mut symbol = create_test_symbol("asyncFunction", SymbolKind::Function).with_signature(
            "public async function asyncFunction<T>(param: T): Promise<T>".to_string(),
        );

        let signature = symbol.signature.clone().unwrap();
        analyzer.analyze_signature_patterns(&mut symbol, &signature);

        assert!(symbol.tags.contains(&"async_pattern".to_string()));
        assert!(symbol.tags.contains(&"generic_pattern".to_string()));
        assert!(symbol.tags.contains(&"public".to_string()));
    }

    #[test]
    fn test_calculate_generic_complexity() {
        let analyzer = create_generic_analyzer();

        let mut symbols = vec![
            create_test_symbol("regularFunction", SymbolKind::Function),
            create_test_symbol("GenericClass", SymbolKind::Class),
            create_test_symbol("IInterface", SymbolKind::Interface),
        ];

        // Add tags to simulate pattern analysis
        symbols[0].tags.push("async_pattern".to_string());
        symbols[0].tags.push("annotated".to_string());
        symbols[1].tags.push("generic_pattern".to_string());
        symbols[1].tags.push("abstract".to_string());

        let complexity = analyzer.calculate_generic_complexity(&symbols);

        // Should be: 1.0 (function) + 0.3 (async) + 0.2 (annotated) + 1.2 (class) + 0.6 (generic) + 0.4 (abstract) + 1.1 (interface) = 4.8
        assert!((complexity - 4.8).abs() < 0.1);
    }

    #[test]
    fn test_detect_language_hints() {
        let analyzer = create_generic_analyzer();

        let symbols = vec![
            create_test_symbol("main", SymbolKind::Function),
            ExtractedSymbol::new(
                "rust_hint".to_string(),
                "std_module".to_string(),
                SymbolKind::Import,
                SymbolLocation::new(PathBuf::from("test.rs"), 1, 0, 1, 10),
            )
            .with_qualified_name("std::collections::HashMap".to_string()),
            create_test_symbol("__init__", SymbolKind::Method),
        ];

        let hints = analyzer.detect_language_hints(&symbols);

        assert!(hints.contains(&"Rust".to_string())); // From .rs extension
        assert!(hints.contains(&"C-family".to_string())); // From main function
        assert!(hints.contains(&"Python-like".to_string())); // From __init__ method
        assert!(hints.contains(&"Rust-like".to_string())); // From std:: pattern
    }

    #[test]
    fn test_language_features() {
        let analyzer = create_generic_analyzer();
        let features = analyzer.language_features();

        // Generic analyzer is conservative about features
        assert!(!features.supports_generics);
        assert!(!features.supports_inheritance);
        assert!(!features.supports_interfaces);
        assert!(!features.is_statically_typed);
        assert!(features.supports_modules); // Most languages have modules
        assert!(features.file_extensions.is_empty()); // Unknown language
    }

    #[test]
    fn test_symbol_priority_modifier() {
        let analyzer = create_generic_analyzer();

        let entry_point =
            create_test_symbol("main", SymbolKind::Function).with_tag("entry_point".to_string());
        assert_eq!(analyzer.get_symbol_priority_modifier(&entry_point), 1.8);

        let test_func = create_test_symbol("test_something", SymbolKind::Function)
            .with_tag("test_related".to_string());
        assert_eq!(analyzer.get_symbol_priority_modifier(&test_func), 0.7);

        let factory =
            create_test_symbol("createUser", SymbolKind::Function).with_tag("factory".to_string());
        assert_eq!(analyzer.get_symbol_priority_modifier(&factory), 1.2);

        let exception = create_test_symbol("MyException", SymbolKind::Class)
            .with_tag("exception_class".to_string());
        assert_eq!(analyzer.get_symbol_priority_modifier(&exception), 1.1);

        let interface = create_test_symbol("IMyInterface", SymbolKind::Interface);
        assert_eq!(analyzer.get_symbol_priority_modifier(&interface), 1.3);
    }

    #[tokio::test]
    async fn test_analyze_file() {
        let analyzer = create_generic_analyzer();
        let context = create_test_context();
        let file_path = PathBuf::from("test.unknown");

        let result = analyzer
            .analyze_file("unknown code", &file_path, "unknown", &context)
            .await;
        assert!(result.is_ok());

        let analysis_result = result.unwrap();
        assert_eq!(analysis_result.file_path, file_path);
        assert_eq!(analysis_result.language, "unknown");
        assert_eq!(
            analysis_result.analysis_metadata.analyzer_name,
            "GenericAnalyzer"
        );

        // Should have warning about generic analysis
        assert!(!analysis_result.analysis_metadata.warnings.is_empty());
        assert!(analysis_result
            .analysis_metadata
            .warnings
            .iter()
            .any(|w| w.contains("Generic analysis")));
    }

    #[tokio::test]
    async fn test_extract_language_metadata() {
        let analyzer = create_generic_analyzer();
        let context = create_test_context();
        let file_path = PathBuf::from("test.unknown");

        let metadata = analyzer
            .extract_language_metadata("", &file_path, &context)
            .await
            .unwrap();

        assert!(metadata.language_version.is_none());
        assert!(metadata.frameworks.is_empty());
        assert!(!metadata.warnings.is_empty());
        assert!(metadata
            .warnings
            .iter()
            .any(|w| w.contains("Generic analysis")));
    }

    #[test]
    fn test_validate_language_patterns() {
        let analyzer = create_generic_analyzer();

        let warnings = analyzer.validate_language_patterns("any content");

        assert!(!warnings.is_empty());
        assert!(warnings
            .iter()
            .any(|w| w.contains("Generic analyzer cannot provide")));
    }
}
