//! TypeScript/JavaScript Language Analyzer
//!
//! This module provides a specialized analyzer for TypeScript and JavaScript code
//! that understands TypeScript-specific constructs and modern JavaScript patterns.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::super::framework::{AnalyzerCapabilities, CodeAnalyzer};
use super::super::tree_sitter_analyzer::TreeSitterAnalyzer;
use super::super::types::*;
use super::{LanguageFeatures, LanguageMetadata, LanguageMetrics, LanguageSpecificAnalyzer};
use crate::symbol::{SymbolKind, SymbolUIDGenerator};

/// TypeScript/JavaScript specific code analyzer
///
/// This analyzer handles both TypeScript and JavaScript, with enhanced support
/// for TypeScript type information and modern JavaScript patterns.
pub struct TypeScriptAnalyzer {
    /// Base tree-sitter analyzer
    base_analyzer: TreeSitterAnalyzer,

    /// UID generator for consistent symbol identification
    uid_generator: Arc<SymbolUIDGenerator>,

    /// Whether this analyzer is handling TypeScript (true) or JavaScript (false)
    is_typescript: bool,
}

impl TypeScriptAnalyzer {
    /// Create a new TypeScript analyzer
    pub fn new(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        let base_analyzer = TreeSitterAnalyzer::new(uid_generator.clone());

        Self {
            base_analyzer,
            uid_generator,
            is_typescript: true, // Default to TypeScript
        }
    }

    /// Create a JavaScript-specific analyzer
    pub fn new_javascript(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        let base_analyzer = TreeSitterAnalyzer::new(uid_generator.clone());

        Self {
            base_analyzer,
            uid_generator,
            is_typescript: false,
        }
    }

    /// Enhance TypeScript/JavaScript symbols with language-specific information
    fn enhance_typescript_symbols(
        &self,
        mut symbols: Vec<ExtractedSymbol>,
    ) -> Vec<ExtractedSymbol> {
        for symbol in &mut symbols {
            // Add TypeScript/JavaScript specific metadata
            match symbol.kind {
                SymbolKind::Class => {
                    symbol.tags.push("class".to_string());

                    // Check for React components
                    if symbol.name.starts_with("Component")
                        || symbol.name.ends_with("Component")
                        || symbol
                            .signature
                            .as_ref()
                            .map_or(false, |s| s.contains("React.Component"))
                    {
                        symbol.tags.push("react_component".to_string());
                        symbol.metadata.insert(
                            "framework".to_string(),
                            serde_json::Value::String("React".to_string()),
                        );
                    }
                }
                SymbolKind::Interface => {
                    symbol.tags.push("interface".to_string());
                    if self.is_typescript {
                        symbol.metadata.insert(
                            "typescript_construct".to_string(),
                            serde_json::Value::Bool(true),
                        );
                    }

                    // Check for common interface patterns
                    if symbol.name.starts_with("I") && symbol.name.len() > 1 {
                        symbol.tags.push("interface_naming_convention".to_string());
                    }
                }
                SymbolKind::Type => {
                    if self.is_typescript {
                        symbol.tags.push("type_alias".to_string());
                        symbol
                            .metadata
                            .insert("typescript_type".to_string(), serde_json::Value::Bool(true));

                        // Detect utility types
                        if let Some(sig) = &symbol.signature {
                            if sig.contains("Partial<")
                                || sig.contains("Required<")
                                || sig.contains("Pick<")
                                || sig.contains("Omit<")
                            {
                                symbol.tags.push("utility_type".to_string());
                            }

                            if sig.contains("Promise<") {
                                symbol.tags.push("promise_type".to_string());
                            }
                        }
                    }
                }
                SymbolKind::Function => {
                    // Detect special function types
                    if let Some(sig) = &symbol.signature {
                        if sig.contains("async") || sig.contains("Promise") {
                            symbol.tags.push("async_function".to_string());
                            symbol
                                .metadata
                                .insert("async".to_string(), serde_json::Value::Bool(true));
                        }

                        if sig.contains("=>") {
                            symbol.tags.push("arrow_function".to_string());
                        }

                        if sig.contains("*") && sig.contains("yield") {
                            symbol.tags.push("generator_function".to_string());
                        }

                        // Detect generic functions
                        if sig.contains("<") && sig.contains(">") {
                            symbol.tags.push("generic_function".to_string());
                            if self.is_typescript {
                                symbol.metadata.insert(
                                    "typescript_generic".to_string(),
                                    serde_json::Value::Bool(true),
                                );
                            }
                        }
                    }

                    // React hooks detection
                    if symbol.name.starts_with("use") && symbol.name.len() > 3 {
                        symbol.tags.push("react_hook".to_string());
                        symbol.metadata.insert(
                            "framework".to_string(),
                            serde_json::Value::String("React".to_string()),
                        );
                    }

                    // Test function detection
                    if symbol.name.starts_with("test")
                        || symbol.name.starts_with("it")
                        || symbol.name.starts_with("describe")
                    {
                        symbol.tags.push("test_function".to_string());
                    }
                }
                SymbolKind::Variable => {
                    if let Some(sig) = &symbol.signature {
                        // Detect const assertions and readonly
                        if sig.contains("const") {
                            symbol.tags.push("const_variable".to_string());
                        }

                        if sig.contains("readonly") && self.is_typescript {
                            symbol.tags.push("readonly".to_string());
                        }

                        // Detect React JSX elements
                        if sig.contains("JSX.Element") || sig.contains("ReactNode") {
                            symbol.tags.push("react_element".to_string());
                            symbol.metadata.insert(
                                "framework".to_string(),
                                serde_json::Value::String("React".to_string()),
                            );
                        }
                    }
                }
                SymbolKind::Import => {
                    symbol.tags.push("import".to_string());

                    // Detect different import patterns
                    if let Some(sig) = &symbol.signature {
                        if sig.contains("import type") && self.is_typescript {
                            symbol.tags.push("type_only_import".to_string());
                        }

                        if sig.contains("* as") {
                            symbol.tags.push("namespace_import".to_string());
                        }

                        if sig.contains("default") {
                            symbol.tags.push("default_import".to_string());
                        }
                    }
                }
                SymbolKind::Export => {
                    symbol.tags.push("export".to_string());

                    if let Some(sig) = &symbol.signature {
                        if sig.contains("export default") {
                            symbol.tags.push("default_export".to_string());
                        }

                        if sig.contains("export type") && self.is_typescript {
                            symbol.tags.push("type_only_export".to_string());
                        }
                    }
                }
                _ => {}
            }

            // Add language-specific priority modifiers
            if self.is_typescript {
                symbol.metadata.insert(
                    "language".to_string(),
                    serde_json::Value::String("TypeScript".to_string()),
                );
            } else {
                symbol.metadata.insert(
                    "language".to_string(),
                    serde_json::Value::String("JavaScript".to_string()),
                );
            }
        }

        symbols
    }

    /// Extract TypeScript/JavaScript specific relationships
    fn extract_typescript_relationships(
        &self,
        symbols: &[ExtractedSymbol],
    ) -> Vec<ExtractedRelationship> {
        let mut relationships = Vec::new();

        // Build symbol lookup for efficient relationship creation
        let symbol_lookup: HashMap<String, &ExtractedSymbol> =
            symbols.iter().map(|s| (s.name.clone(), s)).collect();

        for symbol in symbols {
            // Extract class inheritance relationships
            if symbol.kind == SymbolKind::Class {
                if let Some(sig) = &symbol.signature {
                    if sig.contains("extends") {
                        // Extract parent class name (simplified)
                        // In a full implementation, this would use proper AST parsing
                        if let Some(extends_pos) = sig.find("extends") {
                            let after_extends = &sig[extends_pos + 7..].trim();
                            if let Some(parent_name) = after_extends.split_whitespace().next() {
                                if let Some(parent_symbol) = symbol_lookup.get(parent_name) {
                                    let relationship = ExtractedRelationship::new(
                                        symbol.uid.clone(),
                                        parent_symbol.uid.clone(),
                                        RelationType::InheritsFrom,
                                    )
                                    .with_confidence(0.9);

                                    relationships.push(relationship);
                                }
                            }
                        }
                    }

                    if sig.contains("implements") {
                        // Extract implemented interfaces (simplified)
                        if let Some(implements_pos) = sig.find("implements") {
                            let after_implements = &sig[implements_pos + 10..].trim();
                            if let Some(interface_name) = after_implements.split_whitespace().next()
                            {
                                if let Some(interface_symbol) = symbol_lookup.get(interface_name) {
                                    let relationship = ExtractedRelationship::new(
                                        symbol.uid.clone(),
                                        interface_symbol.uid.clone(),
                                        RelationType::Implements,
                                    )
                                    .with_confidence(0.9);

                                    relationships.push(relationship);
                                }
                            }
                        }
                    }
                }
            }

            // Extract module/namespace relationships
            if symbol.kind == SymbolKind::Namespace {
                // Namespaces contain other symbols
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

            // Extract import/export relationships
            if symbol.kind == SymbolKind::Import {
                if let Some(qualified_name) = &symbol.qualified_name {
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

    /// Calculate TypeScript/JavaScript complexity metrics
    fn calculate_typescript_complexity(&self, symbols: &[ExtractedSymbol]) -> f32 {
        let mut complexity = 0.0;

        for symbol in symbols {
            match symbol.kind {
                SymbolKind::Function => {
                    complexity += 1.0;

                    // Add complexity for async functions
                    if symbol.tags.contains(&"async_function".to_string()) {
                        complexity += 0.5;
                    }

                    // Add complexity for generator functions
                    if symbol.tags.contains(&"generator_function".to_string()) {
                        complexity += 0.8;
                    }

                    // Add complexity for generic functions
                    if symbol.tags.contains(&"generic_function".to_string()) && self.is_typescript {
                        complexity += 0.6;
                    }

                    // React hooks add complexity
                    if symbol.tags.contains(&"react_hook".to_string()) {
                        complexity += 0.3;
                    }
                }
                SymbolKind::Class => {
                    complexity += 1.5; // Classes are more complex

                    // React components add complexity
                    if symbol.tags.contains(&"react_component".to_string()) {
                        complexity += 0.4;
                    }
                }
                SymbolKind::Interface => {
                    if self.is_typescript {
                        complexity += 1.0; // Interfaces add type complexity

                        if symbol.tags.contains(&"utility_type".to_string()) {
                            complexity += 0.5; // Utility types are complex
                        }
                    }
                }
                SymbolKind::Type => {
                    if self.is_typescript {
                        complexity += 0.8; // Type aliases add complexity

                        if symbol.tags.contains(&"promise_type".to_string()) {
                            complexity += 0.3; // Promise types add async complexity
                        }
                    }
                }
                _ => {}
            }
        }

        complexity
    }

    /// Detect TypeScript/JavaScript frameworks and libraries
    fn detect_typescript_frameworks(&self, symbols: &[ExtractedSymbol]) -> Vec<String> {
        let mut frameworks = Vec::new();
        let mut detected = std::collections::HashSet::new();

        for symbol in symbols {
            // Check imports for framework detection
            if symbol.kind == SymbolKind::Import {
                if let Some(qualified_name) = &symbol.qualified_name {
                    let module_name = qualified_name.split('/').next().unwrap_or(qualified_name);

                    match module_name {
                        "react" | "@types/react" => {
                            detected.insert("React");
                        }
                        "vue" | "@vue/composition-api" => {
                            detected.insert("Vue");
                        }
                        "angular" | "@angular/core" => {
                            detected.insert("Angular");
                        }
                        "express" => {
                            detected.insert("Express");
                        }
                        "lodash" | "_" => {
                            detected.insert("Lodash");
                        }
                        "axios" => {
                            detected.insert("Axios");
                        }
                        "moment" => {
                            detected.insert("Moment.js");
                        }
                        "jquery" | "$" => {
                            detected.insert("jQuery");
                        }
                        "typescript" | "ts-node" => {
                            detected.insert("TypeScript");
                        }
                        "jest" | "@jest/globals" => {
                            detected.insert("Jest");
                        }
                        "mocha" => {
                            detected.insert("Mocha");
                        }
                        "next" | "next/router" => {
                            detected.insert("Next.js");
                        }
                        "nuxt" => {
                            detected.insert("Nuxt.js");
                        }
                        "svelte" => {
                            detected.insert("Svelte");
                        }
                        _ => {}
                    }
                }
            }

            // Check for React patterns in symbols
            if symbol.tags.contains(&"react_component".to_string())
                || symbol.tags.contains(&"react_hook".to_string())
            {
                detected.insert("React");
            }

            // Check for Vue patterns
            if symbol.name == "setup"
                || symbol.name.starts_with("use")
                    && symbol
                        .parent_scope
                        .as_ref()
                        .map_or(false, |s| s.contains("Component"))
            {
                detected.insert("Vue");
            }

            // Check for Node.js patterns
            if symbol.qualified_name.as_ref().map_or(false, |qn| {
                qn.starts_with("process.")
                    || qn.starts_with("require(")
                    || qn.starts_with("module.")
            }) {
                detected.insert("Node.js");
            }
        }

        frameworks.extend(detected.into_iter().map(String::from));
        frameworks.sort();
        frameworks
    }

    /// Count test indicators in the symbols
    fn count_test_indicators(&self, symbols: &[ExtractedSymbol]) -> u32 {
        symbols
            .iter()
            .filter(|s| s.tags.contains(&"test_function".to_string()))
            .count() as u32
    }

    /// Calculate documentation ratio
    fn calculate_documentation_ratio(&self, symbols: &[ExtractedSymbol]) -> f32 {
        if symbols.is_empty() {
            return 0.0;
        }

        let documented_count = symbols.iter().filter(|s| s.documentation.is_some()).count();

        documented_count as f32 / symbols.len() as f32
    }
}

#[async_trait]
impl CodeAnalyzer for TypeScriptAnalyzer {
    fn capabilities(&self) -> AnalyzerCapabilities {
        let mut caps = AnalyzerCapabilities::structural();
        caps.confidence = if self.is_typescript { 0.9 } else { 0.85 }; // TypeScript gives higher confidence
        caps
    }

    fn supported_languages(&self) -> Vec<String> {
        if self.is_typescript {
            vec!["typescript".to_string(), "javascript".to_string()]
        } else {
            vec!["javascript".to_string()]
        }
    }

    async fn analyze_file(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        // Determine if we're analyzing TypeScript or JavaScript
        let is_typescript_file = language == "typescript"
            || file_path
                .extension()
                .map_or(false, |ext| ext == "ts" || ext == "tsx");

        // Use base analyzer first
        let mut result = self
            .base_analyzer
            .analyze_file(content, file_path, language, context)
            .await?;

        // Enhance with TypeScript/JavaScript specific analysis
        result.symbols = self.enhance_typescript_symbols(result.symbols);

        // Add TypeScript/JavaScript specific relationships
        let ts_relationships = self.extract_typescript_relationships(&result.symbols);
        result.relationships.extend(ts_relationships);

        // Update metadata to reflect TypeScript/JavaScript specific analysis
        result.analysis_metadata.analyzer_name = if is_typescript_file {
            "TypeScriptAnalyzer"
        } else {
            "JavaScriptAnalyzer"
        }
        .to_string();

        result.analysis_metadata.add_metric(
            "typescript_complexity".to_string(),
            self.calculate_typescript_complexity(&result.symbols) as f64,
        );

        // Add framework detection
        let frameworks = self.detect_typescript_frameworks(&result.symbols);
        if !frameworks.is_empty() {
            result
                .analysis_metadata
                .add_metric("detected_frameworks".to_string(), frameworks.len() as f64);
            result.analysis_metadata.custom.insert(
                "frameworks".to_string(),
                serde_json::Value::Array(
                    frameworks
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        // Add test indicators
        let test_count = self.count_test_indicators(&result.symbols);
        if test_count > 0 {
            result
                .analysis_metadata
                .add_metric("test_functions".to_string(), test_count as f64);
        }

        // Add documentation ratio
        let doc_ratio = self.calculate_documentation_ratio(&result.symbols);
        result
            .analysis_metadata
            .add_metric("documentation_ratio".to_string(), doc_ratio as f64);

        Ok(result)
    }
}

#[async_trait]
impl LanguageSpecificAnalyzer for TypeScriptAnalyzer {
    fn language_features(&self) -> LanguageFeatures {
        LanguageFeatures {
            supports_generics: self.is_typescript,
            supports_inheritance: true,
            supports_interfaces: self.is_typescript,
            supports_operator_overloading: false,
            supports_macros: false,
            supports_closures: true,
            supports_modules: true,
            is_statically_typed: self.is_typescript,
            file_extensions: if self.is_typescript {
                vec![".ts".to_string(), ".tsx".to_string(), ".d.ts".to_string()]
            } else {
                vec![".js".to_string(), ".jsx".to_string(), ".mjs".to_string()]
            },
            test_patterns: vec![
                "*.test.ts".to_string(),
                "*.test.js".to_string(),
                "*.spec.ts".to_string(),
                "*.spec.js".to_string(),
                "__tests__/**/*".to_string(),
                "tests/**/*".to_string(),
            ],
        }
    }

    async fn extract_language_metadata(
        &self,
        _content: &str,
        _file_path: &Path,
        _context: &AnalysisContext,
    ) -> Result<LanguageMetadata, AnalysisError> {
        // This would analyze the file for TypeScript/JavaScript specific metadata
        Ok(LanguageMetadata {
            language_version: None, // Could parse from package.json or tsconfig.json
            frameworks: Vec::new(), // Would be detected from imports
            imports: Vec::new(),    // Would be extracted from import statements
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

        // Check for common TypeScript/JavaScript anti-patterns
        if content.contains("== ") || content.contains("!= ") {
            warnings.push("Consider using strict equality operators (=== and !==)".to_string());
        }

        if content.contains("var ") {
            warnings.push("Consider using 'let' or 'const' instead of 'var'".to_string());
        }

        if self.is_typescript && content.contains(": any") {
            warnings.push("Avoid using 'any' type in TypeScript - use specific types".to_string());
        }

        if content.contains("console.log(") && !content.contains("// TODO") {
            warnings.push("Remove console.log statements before production".to_string());
        }

        if content.contains("eval(") {
            warnings.push("Avoid using eval() - it's a security risk".to_string());
        }

        warnings
    }

    fn get_symbol_priority_modifier(&self, symbol: &ExtractedSymbol) -> f32 {
        match symbol.kind {
            SymbolKind::Interface if self.is_typescript => 1.4, // Interfaces are important in TS
            SymbolKind::Type if self.is_typescript => 1.2,      // Type aliases are important
            SymbolKind::Class => 1.3,                           // Classes are important
            SymbolKind::Function if symbol.tags.contains(&"react_component".to_string()) => 1.5,
            SymbolKind::Function if symbol.tags.contains(&"react_hook".to_string()) => 1.3,
            SymbolKind::Function if symbol.tags.contains(&"async_function".to_string()) => 1.1,
            SymbolKind::Function if symbol.tags.contains(&"test_function".to_string()) => 0.8,
            SymbolKind::Export if symbol.tags.contains(&"default_export".to_string()) => 1.2,
            _ => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{SymbolLocation, SymbolUIDGenerator};
    use std::path::PathBuf;

    fn create_typescript_analyzer() -> TypeScriptAnalyzer {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        TypeScriptAnalyzer::new(uid_generator)
    }

    fn create_javascript_analyzer() -> TypeScriptAnalyzer {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        TypeScriptAnalyzer::new_javascript(uid_generator)
    }

    fn create_test_symbol(name: &str, kind: SymbolKind) -> ExtractedSymbol {
        let location = SymbolLocation::new(PathBuf::from("test.ts"), 1, 0, 1, 10);
        ExtractedSymbol::new(format!("ts::{}", name), name.to_string(), kind, location)
    }

    #[test]
    fn test_typescript_analyzer_capabilities() {
        let analyzer = create_typescript_analyzer();
        let caps = analyzer.capabilities();

        assert!(caps.extracts_symbols);
        assert!(caps.extracts_relationships);
        assert_eq!(caps.confidence, 0.9);

        let js_analyzer = create_javascript_analyzer();
        let js_caps = js_analyzer.capabilities();
        assert_eq!(js_caps.confidence, 0.85);
    }

    #[test]
    fn test_typescript_analyzer_supported_languages() {
        let analyzer = create_typescript_analyzer();
        let languages = analyzer.supported_languages();

        assert!(languages.contains(&"typescript".to_string()));
        assert!(languages.contains(&"javascript".to_string()));

        let js_analyzer = create_javascript_analyzer();
        let js_languages = js_analyzer.supported_languages();
        assert!(js_languages.contains(&"javascript".to_string()));
        assert!(!js_languages.contains(&"typescript".to_string()));
    }

    #[test]
    fn test_enhance_typescript_symbols() {
        let analyzer = create_typescript_analyzer();

        let symbols = vec![
            create_test_symbol("MyComponent", SymbolKind::Class)
                .with_signature("class MyComponent extends React.Component".to_string()),
            create_test_symbol("IUser", SymbolKind::Interface),
            create_test_symbol("UserType", SymbolKind::Type)
                .with_signature("type UserType = Partial<User>".to_string()),
            create_test_symbol("useAuth", SymbolKind::Function)
                .with_signature("function useAuth()".to_string()),
        ];

        let enhanced = analyzer.enhance_typescript_symbols(symbols);

        // Check React component enhancement
        let component = enhanced.iter().find(|s| s.name == "MyComponent").unwrap();
        assert!(component.tags.contains(&"react_component".to_string()));
        assert_eq!(component.metadata.get("framework").unwrap(), "React");

        // Check interface enhancement
        let interface = enhanced.iter().find(|s| s.name == "IUser").unwrap();
        assert!(interface.tags.contains(&"interface".to_string()));
        assert!(interface
            .tags
            .contains(&"interface_naming_convention".to_string()));

        // Check type enhancement
        let type_alias = enhanced.iter().find(|s| s.name == "UserType").unwrap();
        assert!(type_alias.tags.contains(&"utility_type".to_string()));

        // Check hook enhancement
        let hook = enhanced.iter().find(|s| s.name == "useAuth").unwrap();
        assert!(hook.tags.contains(&"react_hook".to_string()));
    }

    #[test]
    fn test_calculate_typescript_complexity() {
        let analyzer = create_typescript_analyzer();

        let mut symbols = vec![
            create_test_symbol("regularFunction", SymbolKind::Function),
            create_test_symbol("MyClass", SymbolKind::Class),
            create_test_symbol("IInterface", SymbolKind::Interface),
        ];

        // Add tags to simulate enhanced symbols
        symbols[0].tags.push("async_function".to_string());
        symbols[1].tags.push("react_component".to_string());

        let complexity = analyzer.calculate_typescript_complexity(&symbols);

        // Should be: 1.0 (function) + 0.5 (async) + 1.5 (class) + 0.4 (react component) + 1.0 (interface) = 4.4
        assert!((complexity - 4.4).abs() < 0.1);
    }

    #[test]
    fn test_detect_typescript_frameworks() {
        let analyzer = create_typescript_analyzer();

        let symbols = vec![
            create_test_symbol("react", SymbolKind::Import)
                .with_qualified_name("react".to_string()),
            create_test_symbol("express", SymbolKind::Import)
                .with_qualified_name("express".to_string()),
            create_test_symbol("useEffect", SymbolKind::Function)
                .with_tag("react_hook".to_string()),
        ];

        let frameworks = analyzer.detect_typescript_frameworks(&symbols);

        assert!(frameworks.contains(&"React".to_string()));
        assert!(frameworks.contains(&"Express".to_string()));
    }

    #[test]
    fn test_language_features() {
        let ts_analyzer = create_typescript_analyzer();
        let ts_features = ts_analyzer.language_features();

        assert!(ts_features.supports_generics);
        assert!(ts_features.supports_interfaces);
        assert!(ts_features.is_statically_typed);
        assert!(ts_features.file_extensions.contains(&".ts".to_string()));

        let js_analyzer = create_javascript_analyzer();
        let js_features = js_analyzer.language_features();

        assert!(!js_features.supports_generics);
        assert!(!js_features.supports_interfaces);
        assert!(!js_features.is_statically_typed);
        assert!(js_features.file_extensions.contains(&".js".to_string()));
    }

    #[test]
    fn test_validate_language_patterns() {
        let analyzer = create_typescript_analyzer();

        let code_with_issues = r#"
            var oldVar = "should use let/const";
            let value = something == null;  // should use ===
            let anyValue: any = "avoid any type";
            console.log("debug statement");
            eval("dangerous code");
        "#;

        let warnings = analyzer.validate_language_patterns(code_with_issues);

        assert!(warnings.iter().any(|w| w.contains("strict equality")));
        assert!(warnings.iter().any(|w| w.contains("let' or 'const'")));
        assert!(warnings.iter().any(|w| w.contains("any")));
        assert!(warnings.iter().any(|w| w.contains("console.log")));
        assert!(warnings.iter().any(|w| w.contains("eval")));
    }

    #[test]
    fn test_symbol_priority_modifier() {
        let analyzer = create_typescript_analyzer();

        let interface_symbol = create_test_symbol("IUser", SymbolKind::Interface);
        assert_eq!(
            analyzer.get_symbol_priority_modifier(&interface_symbol),
            1.4
        );

        let type_symbol = create_test_symbol("UserType", SymbolKind::Type);
        assert_eq!(analyzer.get_symbol_priority_modifier(&type_symbol), 1.2);

        let react_component = create_test_symbol("MyComponent", SymbolKind::Function)
            .with_tag("react_component".to_string());
        assert_eq!(analyzer.get_symbol_priority_modifier(&react_component), 1.5);

        let hook =
            create_test_symbol("useAuth", SymbolKind::Function).with_tag("react_hook".to_string());
        assert_eq!(analyzer.get_symbol_priority_modifier(&hook), 1.3);
    }

    #[test]
    fn test_test_indicators_and_documentation() {
        let analyzer = create_typescript_analyzer();

        let symbols = vec![
            create_test_symbol("testFunction", SymbolKind::Function)
                .with_tag("test_function".to_string()),
            create_test_symbol("regularFunction", SymbolKind::Function)
                .with_documentation("This is documented".to_string()),
            create_test_symbol("undocumentedFunction", SymbolKind::Function),
        ];

        let test_count = analyzer.count_test_indicators(&symbols);
        assert_eq!(test_count, 1);

        let doc_ratio = analyzer.calculate_documentation_ratio(&symbols);
        assert!((doc_ratio - (1.0 / 3.0)).abs() < 0.01); // 1 out of 3 documented
    }
}
