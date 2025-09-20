//! Python Language Analyzer
//!
//! This module provides a specialized analyzer for Python code that understands
//! Python-specific constructs, patterns, and idioms including decorators,
//! list comprehensions, and dynamic typing patterns.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::super::framework::{AnalyzerCapabilities, CodeAnalyzer};
use super::super::tree_sitter_analyzer::TreeSitterAnalyzer;
use super::super::types::*;
use super::{LanguageFeatures, LanguageMetadata, LanguageMetrics, LanguageSpecificAnalyzer};
use crate::symbol::{SymbolKind, SymbolUIDGenerator};

/// Python-specific code analyzer
///
/// This analyzer extends the base TreeSitter analyzer with Python-specific
/// knowledge and patterns for enhanced analysis quality.
pub struct PythonAnalyzer {
    /// Base tree-sitter analyzer
    base_analyzer: TreeSitterAnalyzer,

    /// UID generator for consistent symbol identification
    uid_generator: Arc<SymbolUIDGenerator>,
}

impl PythonAnalyzer {
    /// Create a new Python analyzer
    pub fn new(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        let base_analyzer = TreeSitterAnalyzer::new(uid_generator.clone());

        Self {
            base_analyzer,
            uid_generator,
        }
    }

    /// Enhance Python symbols with language-specific information
    fn enhance_python_symbols(&self, mut symbols: Vec<ExtractedSymbol>) -> Vec<ExtractedSymbol> {
        for symbol in &mut symbols {
            // Add Python-specific metadata
            match symbol.kind {
                SymbolKind::Class => {
                    symbol.tags.push("class".to_string());

                    // Check for common Python class patterns
                    if let Some(sig) = &symbol.signature {
                        // Detect inheritance
                        if sig.contains("(") && sig.contains(")") && !sig.ends_with("():") {
                            symbol.tags.push("inherits".to_string());
                        }

                        // Detect dataclasses
                        if sig.contains("@dataclass") {
                            symbol.tags.push("dataclass".to_string());
                            symbol.metadata.insert(
                                "python_decorator".to_string(),
                                serde_json::Value::String("dataclass".to_string()),
                            );
                        }

                        // Detect abstract classes
                        if sig.contains("ABC") || sig.contains("@abstractmethod") {
                            symbol.tags.push("abstract_class".to_string());
                        }

                        // Detect exception classes
                        if symbol.name.ends_with("Exception")
                            || symbol.name.ends_with("Error")
                            || sig.contains("Exception")
                            || sig.contains("Error")
                        {
                            symbol.tags.push("exception_class".to_string());
                        }
                    }

                    // Django model detection
                    if symbol.name.ends_with("Model")
                        || symbol
                            .qualified_name
                            .as_ref()
                            .map_or(false, |qn| qn.contains("models.Model"))
                    {
                        symbol.tags.push("django_model".to_string());
                        symbol.metadata.insert(
                            "framework".to_string(),
                            serde_json::Value::String("Django".to_string()),
                        );
                    }
                }
                SymbolKind::Function | SymbolKind::Method => {
                    // Detect special Python methods
                    if symbol.name.starts_with("__") && symbol.name.ends_with("__") {
                        symbol.tags.push("dunder_method".to_string());
                        symbol.metadata.insert(
                            "python_special_method".to_string(),
                            serde_json::Value::Bool(true),
                        );

                        // Specific dunder method types
                        match symbol.name.as_str() {
                            "__init__" => symbol.tags.push("constructor".to_string()),
                            "__str__" | "__repr__" => {
                                symbol.tags.push("string_representation".to_string())
                            }
                            "__call__" => symbol.tags.push("callable_object".to_string()),
                            "__enter__" | "__exit__" => {
                                symbol.tags.push("context_manager".to_string())
                            }
                            "__iter__" | "__next__" => symbol.tags.push("iterator".to_string()),
                            "__getitem__" | "__setitem__" | "__delitem__" => {
                                symbol.tags.push("container_method".to_string())
                            }
                            _ => {}
                        }
                    } else if symbol.name.starts_with("_") && !symbol.name.starts_with("__") {
                        symbol.tags.push("protected_method".to_string());
                    }

                    // Detect decorators in signature
                    if let Some(sig) = &symbol.signature {
                        if sig.contains("@") {
                            symbol.tags.push("decorated_function".to_string());

                            // Common Python decorators
                            if sig.contains("@property") {
                                symbol.tags.push("property".to_string());
                                symbol.kind = SymbolKind::Field; // Properties are more like fields
                            } else if sig.contains("@staticmethod") {
                                symbol.tags.push("static_method".to_string());
                            } else if sig.contains("@classmethod") {
                                symbol.tags.push("class_method".to_string());
                            } else if sig.contains("@abstractmethod") {
                                symbol.tags.push("abstract_method".to_string());
                            }

                            // Framework-specific decorators
                            if sig.contains("@app.route") || sig.contains("@blueprint.route") {
                                symbol.tags.push("flask_route".to_string());
                                symbol.metadata.insert(
                                    "framework".to_string(),
                                    serde_json::Value::String("Flask".to_string()),
                                );
                            } else if sig.contains("@api.route") || sig.contains("@router.") {
                                symbol.tags.push("api_endpoint".to_string());
                            } else if sig.contains("@pytest.") || sig.contains("@patch") {
                                symbol.tags.push("test_method".to_string());
                                symbol.metadata.insert(
                                    "test_framework".to_string(),
                                    serde_json::Value::String("pytest".to_string()),
                                );
                            }
                        }

                        // Detect async functions
                        if sig.contains("async def") {
                            symbol.tags.push("async_function".to_string());
                            symbol
                                .metadata
                                .insert("python_async".to_string(), serde_json::Value::Bool(true));
                        }

                        // Detect generator functions
                        if sig.contains("yield") {
                            symbol.tags.push("generator_function".to_string());
                        }

                        // Detect lambda functions
                        if sig.contains("lambda") {
                            symbol.tags.push("lambda_function".to_string());
                            symbol.kind = SymbolKind::Anonymous;
                        }
                    }

                    // Test function detection
                    if symbol.name.starts_with("test_") || symbol.name.starts_with("Test") {
                        symbol.tags.push("test_function".to_string());
                    }
                }
                SymbolKind::Variable => {
                    // Detect constants (all uppercase)
                    if symbol
                        .name
                        .chars()
                        .all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
                    {
                        symbol.tags.push("constant".to_string());
                        symbol.kind = SymbolKind::Constant;
                    }

                    // Detect private variables
                    if symbol.name.starts_with("_") {
                        symbol.tags.push("private_variable".to_string());
                    }

                    // Detect class variables vs instance variables
                    if symbol.parent_scope.is_some() && !symbol.name.starts_with("self.") {
                        symbol.tags.push("class_variable".to_string());
                    } else if symbol.name.starts_with("self.") {
                        symbol.tags.push("instance_variable".to_string());
                    }
                }
                SymbolKind::Import => {
                    symbol.tags.push("import".to_string());

                    if let Some(sig) = &symbol.signature {
                        // Different import patterns
                        if sig.contains("from") && sig.contains("import") {
                            symbol.tags.push("from_import".to_string());
                        } else if sig.starts_with("import") {
                            symbol.tags.push("direct_import".to_string());
                        }

                        if sig.contains("as") {
                            symbol.tags.push("aliased_import".to_string());
                        }

                        if sig.contains("*") {
                            symbol.tags.push("wildcard_import".to_string());
                            // Wildcard imports are generally not recommended
                            symbol.metadata.insert(
                                "python_warning".to_string(),
                                serde_json::Value::String(
                                    "Wildcard imports can pollute namespace".to_string(),
                                ),
                            );
                        }
                    }
                }
                _ => {}
            }

            // Add general Python metadata
            symbol.metadata.insert(
                "language".to_string(),
                serde_json::Value::String("Python".to_string()),
            );
        }

        symbols
    }

    /// Calculate Python-specific complexity metrics
    fn calculate_python_complexity(&self, symbols: &[ExtractedSymbol]) -> f32 {
        let mut complexity = 0.0;

        for symbol in symbols {
            match symbol.kind {
                SymbolKind::Function | SymbolKind::Method => {
                    complexity += 1.0;

                    // Add complexity for decorated functions
                    if symbol.tags.contains(&"decorated_function".to_string()) {
                        complexity += 0.3;
                    }

                    // Add complexity for async functions
                    if symbol.tags.contains(&"async_function".to_string()) {
                        complexity += 0.5;
                    }

                    // Add complexity for generator functions
                    if symbol.tags.contains(&"generator_function".to_string()) {
                        complexity += 0.7;
                    }

                    // Dunder methods add complexity
                    if symbol.tags.contains(&"dunder_method".to_string()) {
                        complexity += 0.4;
                    }

                    // Context managers are complex
                    if symbol.tags.contains(&"context_manager".to_string()) {
                        complexity += 0.6;
                    }
                }
                SymbolKind::Class => {
                    complexity += 1.5;

                    // Inheritance adds complexity
                    if symbol.tags.contains(&"inherits".to_string()) {
                        complexity += 0.5;
                    }

                    // Abstract classes are more complex
                    if symbol.tags.contains(&"abstract_class".to_string()) {
                        complexity += 0.8;
                    }

                    // Dataclasses reduce boilerplate but add conceptual complexity
                    if symbol.tags.contains(&"dataclass".to_string()) {
                        complexity += 0.3;
                    }

                    // Exception classes are simpler
                    if symbol.tags.contains(&"exception_class".to_string()) {
                        complexity += 0.2;
                    }
                }
                SymbolKind::Import => {
                    // Wildcard imports add complexity
                    if symbol.tags.contains(&"wildcard_import".to_string()) {
                        complexity += 0.5;
                    }
                }
                _ => {}
            }
        }

        complexity
    }

    /// Detect Python frameworks and libraries
    fn detect_python_frameworks(&self, symbols: &[ExtractedSymbol]) -> Vec<String> {
        let mut frameworks = Vec::new();
        let mut detected = std::collections::HashSet::new();

        for symbol in symbols {
            if symbol.kind == SymbolKind::Import {
                if let Some(qualified_name) = &symbol.qualified_name {
                    let module_name = qualified_name.split('.').next().unwrap_or(qualified_name);

                    match module_name {
                        "django" => {
                            detected.insert("Django");
                        }
                        "flask" => {
                            detected.insert("Flask");
                        }
                        "fastapi" => {
                            detected.insert("FastAPI");
                        }
                        "tornado" => {
                            detected.insert("Tornado");
                        }
                        "numpy" | "np" => {
                            detected.insert("NumPy");
                        }
                        "pandas" | "pd" => {
                            detected.insert("Pandas");
                        }
                        "matplotlib" | "plt" => {
                            detected.insert("Matplotlib");
                        }
                        "sklearn" | "scikit-learn" => {
                            detected.insert("Scikit-learn");
                        }
                        "tensorflow" | "tf" => {
                            detected.insert("TensorFlow");
                        }
                        "torch" | "pytorch" => {
                            detected.insert("PyTorch");
                        }
                        "keras" => {
                            detected.insert("Keras");
                        }
                        "requests" => {
                            detected.insert("Requests");
                        }
                        "aiohttp" => {
                            detected.insert("aiohttp");
                        }
                        "sqlalchemy" => {
                            detected.insert("SQLAlchemy");
                        }
                        "pytest" => {
                            detected.insert("pytest");
                        }
                        "unittest" => {
                            detected.insert("unittest");
                        }
                        "celery" => {
                            detected.insert("Celery");
                        }
                        "redis" => {
                            detected.insert("Redis");
                        }
                        "asyncio" => {
                            detected.insert("asyncio");
                        }
                        _ => {}
                    }
                }
            }

            // Check for framework-specific patterns in symbols
            if symbol.tags.contains(&"django_model".to_string()) {
                detected.insert("Django");
            }

            if symbol.tags.contains(&"flask_route".to_string()) {
                detected.insert("Flask");
            }

            // Check for data science patterns
            if symbol.name.contains("DataFrame") || symbol.name.contains("Series") {
                detected.insert("Pandas");
            }

            if symbol.name.contains("array") || symbol.name.contains("ndarray") {
                detected.insert("NumPy");
            }
        }

        frameworks.extend(detected.into_iter().map(String::from));
        frameworks.sort();
        frameworks
    }

    /// Count Python-specific test indicators
    fn count_test_indicators(&self, symbols: &[ExtractedSymbol]) -> u32 {
        symbols
            .iter()
            .filter(|s| {
                s.tags.contains(&"test_function".to_string())
                    || s.tags.contains(&"test_method".to_string())
                    || s.name.starts_with("test_")
                    || s.name.starts_with("Test")
            })
            .count() as u32
    }

    /// Count style violations (simple heuristics)
    fn count_style_violations(&self, symbols: &[ExtractedSymbol]) -> u32 {
        let mut violations = 0;

        for symbol in symbols {
            // Check for wildcard imports
            if symbol.tags.contains(&"wildcard_import".to_string()) {
                violations += 1;
            }

            // Check naming conventions
            match symbol.kind {
                SymbolKind::Class => {
                    // Classes should be PascalCase
                    if !symbol.name.chars().next().unwrap_or('a').is_uppercase() {
                        violations += 1;
                    }
                }
                SymbolKind::Function | SymbolKind::Method => {
                    // Functions should be snake_case (unless dunder methods)
                    if !symbol.tags.contains(&"dunder_method".to_string())
                        && symbol.name.contains(char::is_uppercase)
                        && !symbol.name.starts_with("test")
                    {
                        violations += 1;
                    }
                }
                SymbolKind::Variable => {
                    // Variables should be snake_case (unless constants)
                    if !symbol.tags.contains(&"constant".to_string())
                        && symbol.name.contains(char::is_uppercase)
                    {
                        violations += 1;
                    }
                }
                _ => {}
            }
        }

        violations
    }
    /// Extract Python-specific relationships with enhanced detection
    fn extract_python_relationships(
        &self,
        symbols: &[ExtractedSymbol],
        content: &str,
    ) -> Vec<ExtractedRelationship> {
        let mut relationships = Vec::new();

        // Build comprehensive symbol lookup maps
        let symbol_lookup: HashMap<String, &ExtractedSymbol> =
            symbols.iter().map(|s| (s.name.clone(), s)).collect();
        let _fqn_lookup: HashMap<String, &ExtractedSymbol> = symbols
            .iter()
            .filter_map(|s| s.qualified_name.as_ref().map(|fqn| (fqn.clone(), s)))
            .collect();

        // Extract class inheritance relationships
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("class ") {
                if let Some(colon_pos) = trimmed.find(':') {
                    let class_def = &trimmed[6..colon_pos]; // Skip "class "

                    if let Some(paren_start) = class_def.find('(') {
                        let class_name = class_def[..paren_start].trim();
                        let paren_end = class_def.rfind(')').unwrap_or(class_def.len());
                        let base_classes_str = &class_def[paren_start + 1..paren_end];

                        if let Some(class_symbol) = symbol_lookup.get(class_name) {
                            for base_class in base_classes_str.split(',') {
                                let base_class_name = base_class
                                    .trim()
                                    .split('.')
                                    .last()
                                    .unwrap_or(base_class.trim());

                                if let Some(base_symbol) = symbol_lookup.get(base_class_name) {
                                    let relationship =
                                        ExtractedRelationship::new(
                                            class_symbol.uid.clone(),
                                            base_symbol.uid.clone(),
                                            RelationType::InheritsFrom,
                                        )
                                        .with_confidence(0.95)
                                        .with_context(
                                            format!("class {}({})", class_name, base_class.trim()),
                                        );

                                    relationships.push(relationship);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Extract import relationships
        for symbol in symbols {
            if symbol.kind == SymbolKind::Import {
                if let Some(qualified_name) = &symbol.qualified_name {
                    let file_uid = format!("file::{}", symbol.location.file_path.display());
                    let relationship = ExtractedRelationship::new(
                        file_uid,
                        qualified_name.clone(),
                        RelationType::Imports,
                    )
                    .with_confidence(0.9);

                    relationships.push(relationship);
                }
            }
        }

        // Extract method containment relationships
        for symbol in symbols {
            if symbol.kind == SymbolKind::Method || symbol.kind == SymbolKind::Field {
                if let Some(parent_scope) = &symbol.parent_scope {
                    if let Some(parent_symbol) = symbol_lookup.get(parent_scope) {
                        if parent_symbol.kind == SymbolKind::Class {
                            let relationship = ExtractedRelationship::new(
                                parent_symbol.uid.clone(),
                                symbol.uid.clone(),
                                RelationType::Contains,
                            )
                            .with_confidence(1.0)
                            .with_context(format!(
                                "Class {} contains {}",
                                parent_scope, symbol.name
                            ));

                            relationships.push(relationship);
                        }
                    }
                }
            }
        }

        relationships
    }
}

#[async_trait]
impl CodeAnalyzer for PythonAnalyzer {
    fn capabilities(&self) -> AnalyzerCapabilities {
        let mut caps = AnalyzerCapabilities::structural();
        caps.confidence = 0.88; // Python's dynamic nature makes some analysis less certain
        caps
    }

    fn supported_languages(&self) -> Vec<String> {
        vec!["python".to_string()]
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

        // Enhance with Python-specific analysis
        result.symbols = self.enhance_python_symbols(result.symbols);

        // Add Python-specific relationships
        let python_relationships = self.extract_python_relationships(&result.symbols, content);
        result.relationships.extend(python_relationships);

        // Update metadata to reflect Python-specific analysis
        result.analysis_metadata.analyzer_name = "PythonAnalyzer".to_string();
        result.analysis_metadata.add_metric(
            "python_complexity".to_string(),
            self.calculate_python_complexity(&result.symbols) as f64,
        );

        // Add framework detection
        let frameworks = self.detect_python_frameworks(&result.symbols);
        if !frameworks.is_empty() {
            result
                .analysis_metadata
                .add_metric("detected_frameworks".to_string(), frameworks.len() as f64);
            result.analysis_metadata.custom.insert(
                "python_frameworks".to_string(),
                serde_json::Value::Array(
                    frameworks
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        // Add test metrics
        let test_count = self.count_test_indicators(&result.symbols);
        if test_count > 0 {
            result
                .analysis_metadata
                .add_metric("test_functions".to_string(), test_count as f64);
        }

        // Add style violation metrics
        let style_violations = self.count_style_violations(&result.symbols);
        result
            .analysis_metadata
            .add_metric("style_violations".to_string(), style_violations as f64);

        Ok(result)
    }
}

#[async_trait]
impl LanguageSpecificAnalyzer for PythonAnalyzer {
    fn language_features(&self) -> LanguageFeatures {
        LanguageFeatures {
            supports_generics: true, // Python 3.5+ has typing generics
            supports_inheritance: true,
            supports_interfaces: false, // Python has protocols, but not traditional interfaces
            supports_operator_overloading: true,
            supports_macros: false, // Python doesn't have traditional macros
            supports_closures: true,
            supports_modules: true,
            is_statically_typed: false, // Dynamic typing with optional static typing
            file_extensions: vec![".py".to_string(), ".pyi".to_string(), ".pyw".to_string()],
            test_patterns: vec![
                "test_*.py".to_string(),
                "*_test.py".to_string(),
                "tests/**/*.py".to_string(),
                "test_*.py".to_string(),
            ],
        }
    }

    async fn extract_language_metadata(
        &self,
        _content: &str,
        _file_path: &Path,
        _context: &AnalysisContext,
    ) -> Result<LanguageMetadata, AnalysisError> {
        // This would analyze the file for Python-specific metadata
        Ok(LanguageMetadata {
            language_version: None, // Could parse from setup.py, pyproject.toml
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

        // Check for common Python anti-patterns
        if content.contains("from") && content.contains("import *") {
            warnings.push("Avoid wildcard imports (from module import *)".to_string());
        }

        if content.contains("eval(") {
            warnings.push("Avoid using eval() - it's a security risk".to_string());
        }

        if content.contains("exec(") {
            warnings.push("Avoid using exec() - it's a security risk".to_string());
        }

        if content.contains("global ") {
            warnings.push("Minimize use of global variables".to_string());
        }

        if content.contains("except:") && !content.contains("except Exception:") {
            warnings.push("Use specific exception types instead of bare except".to_string());
        }

        if content.contains("lambda") && content.matches("lambda").count() > 3 {
            warnings
                .push("Consider using regular functions instead of complex lambdas".to_string());
        }

        warnings
    }

    fn get_symbol_priority_modifier(&self, symbol: &ExtractedSymbol) -> f32 {
        match symbol.kind {
            SymbolKind::Class => {
                if symbol.tags.contains(&"django_model".to_string()) {
                    1.6 // Django models are very important
                } else if symbol.tags.contains(&"exception_class".to_string()) {
                    1.1 // Exception classes are moderately important
                } else if symbol.tags.contains(&"abstract_class".to_string()) {
                    1.4 // Abstract classes are important
                } else {
                    1.3 // Regular classes
                }
            }
            SymbolKind::Function | SymbolKind::Method => {
                if symbol.name == "__init__" {
                    1.5 // Constructors are important
                } else if symbol.tags.contains(&"dunder_method".to_string()) {
                    1.3 // Special methods are important
                } else if symbol.tags.contains(&"flask_route".to_string())
                    || symbol.tags.contains(&"api_endpoint".to_string())
                {
                    1.4 // API endpoints are important
                } else if symbol.tags.contains(&"test_function".to_string()) {
                    0.8 // Tests are less important for code understanding
                } else if symbol.tags.contains(&"property".to_string()) {
                    1.2 // Properties are moderately important
                } else if symbol.tags.contains(&"async_function".to_string()) {
                    1.1 // Async functions slightly more important
                } else {
                    1.0 // Regular functions
                }
            }
            SymbolKind::Variable | SymbolKind::Constant => {
                if symbol.tags.contains(&"constant".to_string()) {
                    1.2 // Constants are moderately important
                } else if symbol.tags.contains(&"class_variable".to_string()) {
                    1.1 // Class variables are slightly important
                } else {
                    1.0 // Instance/local variables
                }
            }
            SymbolKind::Import => {
                if symbol.tags.contains(&"wildcard_import".to_string()) {
                    0.7 // Wildcard imports are problematic
                } else {
                    0.9 // Imports are less important for understanding
                }
            }
            _ => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{SymbolLocation, SymbolUIDGenerator};
    use std::path::PathBuf;

    fn create_python_analyzer() -> PythonAnalyzer {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        PythonAnalyzer::new(uid_generator)
    }

    fn create_test_symbol(name: &str, kind: SymbolKind) -> ExtractedSymbol {
        let location = SymbolLocation::new(PathBuf::from("test.py"), 1, 0, 1, 10);
        ExtractedSymbol::new(
            format!("python::{}", name),
            name.to_string(),
            kind,
            location,
        )
    }

    #[test]
    fn test_python_analyzer_capabilities() {
        let analyzer = create_python_analyzer();
        let caps = analyzer.capabilities();

        assert!(caps.extracts_symbols);
        assert!(caps.extracts_relationships);
        assert_eq!(caps.confidence, 0.88);
    }

    #[test]
    fn test_python_analyzer_supported_languages() {
        let analyzer = create_python_analyzer();
        let languages = analyzer.supported_languages();

        assert_eq!(languages.len(), 1);
        assert!(languages.contains(&"python".to_string()));
    }

    #[test]
    fn test_enhance_python_symbols() {
        let analyzer = create_python_analyzer();

        let symbols = vec![
            create_test_symbol("MyModel", SymbolKind::Class)
                .with_signature("class MyModel(models.Model):".to_string()),
            create_test_symbol("__init__", SymbolKind::Method),
            create_test_symbol("my_property", SymbolKind::Function)
                .with_signature("@property\ndef my_property(self):".to_string()),
            create_test_symbol("async_func", SymbolKind::Function)
                .with_signature("async def async_func():".to_string()),
            create_test_symbol("MY_CONSTANT", SymbolKind::Variable),
        ];

        let enhanced = analyzer.enhance_python_symbols(symbols);

        // Check Django model enhancement
        let model = enhanced.iter().find(|s| s.name == "MyModel").unwrap();
        assert!(model.tags.contains(&"django_model".to_string()));
        assert_eq!(model.metadata.get("framework").unwrap(), "Django");

        // Check dunder method enhancement
        let init_method = enhanced.iter().find(|s| s.name == "__init__").unwrap();
        assert!(init_method.tags.contains(&"dunder_method".to_string()));
        assert!(init_method.tags.contains(&"constructor".to_string()));

        // Check property enhancement
        let property = enhanced.iter().find(|s| s.name == "my_property").unwrap();
        assert!(property.tags.contains(&"property".to_string()));
        assert_eq!(property.kind, SymbolKind::Field);

        // Check async function enhancement
        let async_func = enhanced.iter().find(|s| s.name == "async_func").unwrap();
        assert!(async_func.tags.contains(&"async_function".to_string()));

        // Check constant enhancement
        let constant = enhanced.iter().find(|s| s.name == "MY_CONSTANT").unwrap();
        assert!(constant.tags.contains(&"constant".to_string()));
        assert_eq!(constant.kind, SymbolKind::Constant);
    }

    #[test]
    fn test_calculate_python_complexity() {
        let analyzer = create_python_analyzer();

        let mut symbols = vec![
            create_test_symbol("regular_function", SymbolKind::Function),
            create_test_symbol("MyClass", SymbolKind::Class),
            create_test_symbol("__enter__", SymbolKind::Method),
        ];

        // Add tags to simulate enhanced symbols
        symbols[0].tags.push("decorated_function".to_string());
        symbols[0].tags.push("async_function".to_string());
        symbols[1].tags.push("inherits".to_string());
        symbols[2].tags.push("context_manager".to_string());
        symbols[2].tags.push("dunder_method".to_string());

        let complexity = analyzer.calculate_python_complexity(&symbols);

        // Should be: 1.0 (function) + 0.3 (decorated) + 0.5 (async) + 1.5 (class) + 0.5 (inherits) + 1.0 (method) + 0.6 (context manager) + 0.4 (dunder) = 5.8
        assert!((complexity - 5.8).abs() < 0.1);
    }

    #[test]
    fn test_detect_python_frameworks() {
        let analyzer = create_python_analyzer();

        let symbols = vec![
            create_test_symbol("django", SymbolKind::Import)
                .with_qualified_name("django.db.models".to_string()),
            create_test_symbol("pandas", SymbolKind::Import)
                .with_qualified_name("pandas".to_string()),
            create_test_symbol("UserModel", SymbolKind::Class).with_tag("django_model".to_string()),
        ];

        let frameworks = analyzer.detect_python_frameworks(&symbols);

        assert!(frameworks.contains(&"Django".to_string()));
        assert!(frameworks.contains(&"Pandas".to_string()));
    }

    #[test]
    fn test_language_features() {
        let analyzer = create_python_analyzer();
        let features = analyzer.language_features();

        assert!(features.supports_generics);
        assert!(features.supports_inheritance);
        assert!(!features.supports_interfaces); // No traditional interfaces
        assert!(features.supports_operator_overloading);
        assert!(!features.supports_macros);
        assert!(features.supports_closures);
        assert!(features.supports_modules);
        assert!(!features.is_statically_typed); // Dynamic typing
        assert!(features.file_extensions.contains(&".py".to_string()));
    }

    #[test]
    fn test_validate_language_patterns() {
        let analyzer = create_python_analyzer();

        let code_with_issues = r#"
            from module import *  # Wildcard import
            
            def dangerous():
                eval("some_code")  # Security risk
                exec("more_code")  # Security risk
                global my_var     # Global usage
                
            try:
                risky_operation()
            except:  # Bare except
                pass
        "#;

        let warnings = analyzer.validate_language_patterns(code_with_issues);

        assert!(warnings.iter().any(|w| w.contains("wildcard imports")));
        assert!(warnings.iter().any(|w| w.contains("eval")));
        assert!(warnings.iter().any(|w| w.contains("exec")));
        assert!(warnings.iter().any(|w| w.contains("global")));
        assert!(warnings.iter().any(|w| w.contains("bare except")));
    }

    #[test]
    fn test_symbol_priority_modifier() {
        let analyzer = create_python_analyzer();

        let django_model =
            create_test_symbol("UserModel", SymbolKind::Class).with_tag("django_model".to_string());
        assert_eq!(analyzer.get_symbol_priority_modifier(&django_model), 1.6);

        let init_method = create_test_symbol("__init__", SymbolKind::Method);
        assert_eq!(analyzer.get_symbol_priority_modifier(&init_method), 1.5);

        let dunder_method =
            create_test_symbol("__str__", SymbolKind::Method).with_tag("dunder_method".to_string());
        assert_eq!(analyzer.get_symbol_priority_modifier(&dunder_method), 1.3);

        let property = create_test_symbol("my_property", SymbolKind::Function)
            .with_tag("property".to_string());
        assert_eq!(analyzer.get_symbol_priority_modifier(&property), 1.2);

        let test_function = create_test_symbol("test_something", SymbolKind::Function)
            .with_tag("test_function".to_string());
        assert_eq!(analyzer.get_symbol_priority_modifier(&test_function), 0.8);
    }

    #[test]
    fn test_count_test_indicators() {
        let analyzer = create_python_analyzer();

        let symbols = vec![
            create_test_symbol("test_function", SymbolKind::Function)
                .with_tag("test_function".to_string()),
            create_test_symbol("TestClass", SymbolKind::Class),
            create_test_symbol("regular_function", SymbolKind::Function),
        ];

        let test_count = analyzer.count_test_indicators(&symbols);
        assert_eq!(test_count, 2); // test_function + TestClass
    }

    #[test]
    fn test_count_style_violations() {
        let analyzer = create_python_analyzer();

        let symbols = vec![
            create_test_symbol("wildcard_import", SymbolKind::Import)
                .with_tag("wildcard_import".to_string()),
            create_test_symbol("badClassName", SymbolKind::Class), // Should be PascalCase but starting with lowercase
            create_test_symbol("BadFunctionName", SymbolKind::Function), // Should be snake_case
        ];

        let violations = analyzer.count_style_violations(&symbols);
        assert_eq!(violations, 3); // All three should be violations
    }
}
