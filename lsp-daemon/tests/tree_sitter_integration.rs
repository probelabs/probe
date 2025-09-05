//! Integration tests for tree-sitter dependencies
//!
//! These tests verify that tree-sitter parsers can be created and used
//! for structural code analysis in the relationship extraction system.

use lsp_daemon::analyzer::framework::CodeAnalyzer;
use lsp_daemon::analyzer::tree_sitter_analyzer::{ParserPool, TreeSitterAnalyzer};
use lsp_daemon::symbol::SymbolUIDGenerator;
use std::sync::Arc;

#[test]
fn test_tree_sitter_supported_languages() {
    let uid_generator = Arc::new(SymbolUIDGenerator::new());
    let analyzer = TreeSitterAnalyzer::new(uid_generator);

    let languages = analyzer.supported_languages();

    // Check that default languages are supported when features are enabled
    #[cfg(feature = "tree-sitter-rust")]
    assert!(
        languages.contains(&"rust".to_string()),
        "Rust should be supported"
    );

    #[cfg(feature = "tree-sitter-typescript")]
    assert!(
        languages.contains(&"typescript".to_string()),
        "TypeScript should be supported"
    );

    #[cfg(feature = "tree-sitter-python")]
    assert!(
        languages.contains(&"python".to_string()),
        "Python should be supported"
    );

    // Ensure we have at least one supported language with default features
    assert!(
        !languages.is_empty(),
        "Should have at least one supported language"
    );
}

#[test]
fn test_parser_pool_creation() {
    let mut pool = ParserPool::new();

    // Test Rust parser creation
    #[cfg(feature = "tree-sitter-rust")]
    {
        let parser = pool.get_parser("rust");
        assert!(parser.is_some(), "Should be able to create Rust parser");

        if let Some(parser) = parser {
            pool.return_parser("rust", parser);
        }
    }

    // Test TypeScript parser creation
    #[cfg(feature = "tree-sitter-typescript")]
    {
        let parser = pool.get_parser("typescript");
        assert!(
            parser.is_some(),
            "Should be able to create TypeScript parser"
        );

        if let Some(parser) = parser {
            pool.return_parser("typescript", parser);
        }
    }

    // Test Python parser creation
    #[cfg(feature = "tree-sitter-python")]
    {
        let parser = pool.get_parser("python");
        assert!(parser.is_some(), "Should be able to create Python parser");

        if let Some(parser) = parser {
            pool.return_parser("python", parser);
        }
    }

    // Test unsupported language
    let unsupported_parser = pool.get_parser("unsupported");
    assert!(
        unsupported_parser.is_none(),
        "Should not create parser for unsupported language"
    );
}

#[cfg(all(test, feature = "tree-sitter-rust"))]
mod rust_parsing_tests {
    use super::*;
    use lsp_daemon::analyzer::framework::{AnalysisContext, CodeAnalyzer};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_rust_code_parsing() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let analyzer = TreeSitterAnalyzer::new(uid_generator);

        let rust_code = r#"
            struct MyStruct {
                field: i32,
            }
            
            impl MyStruct {
                fn new() -> Self {
                    Self { field: 0 }
                }
            }
            
            trait Display {
                fn fmt(&self) -> String;
            }
            
            impl Display for MyStruct {
                fn fmt(&self) -> String {
                    format!("{}", self.field)
                }
            }
        "#;

        let context = AnalysisContext::default();
        let file_path = PathBuf::from("test.rs");

        let result = analyzer
            .analyze_file(rust_code, &file_path, "rust", &context)
            .await;

        assert!(
            result.is_ok(),
            "Rust code analysis should succeed: {:?}",
            result.err()
        );

        let analysis_result = result.unwrap();
        assert!(
            !analysis_result.symbols.is_empty(),
            "Should extract symbols from Rust code"
        );
        // Note: relationships might be empty if the relationship extractor isn't fully configured
        // but the basic parsing should work
    }
}

#[cfg(all(test, feature = "tree-sitter-python"))]
mod python_parsing_tests {
    use super::*;
    use lsp_daemon::analyzer::framework::{AnalysisContext, CodeAnalyzer};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_python_code_parsing() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let analyzer = TreeSitterAnalyzer::new(uid_generator);

        let python_code = r#"
class MyClass:
    def __init__(self, value):
        self.value = value
    
    def display(self):
        return str(self.value)

class ChildClass(MyClass):
    def __init__(self, value, extra):
        super().__init__(value)
        self.extra = extra
        "#;

        let context = AnalysisContext::default();
        let file_path = PathBuf::from("test.py");

        let result = analyzer
            .analyze_file(python_code, &file_path, "python", &context)
            .await;

        assert!(
            result.is_ok(),
            "Python code analysis should succeed: {:?}",
            result.err()
        );

        let analysis_result = result.unwrap();
        assert!(
            !analysis_result.symbols.is_empty(),
            "Should extract symbols from Python code"
        );
    }
}

#[cfg(all(test, feature = "tree-sitter-typescript"))]
mod typescript_parsing_tests {
    use super::*;
    use lsp_daemon::analyzer::framework::{AnalysisContext, CodeAnalyzer};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_typescript_code_parsing() {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let analyzer = TreeSitterAnalyzer::new(uid_generator);

        let typescript_code = r#"
interface Displayable {
    display(): string;
}

class MyClass implements Displayable {
    private value: number;
    
    constructor(value: number) {
        this.value = value;
    }
    
    display(): string {
        return this.value.toString();
    }
}

class ExtendedClass extends MyClass {
    private extra: string;
    
    constructor(value: number, extra: string) {
        super(value);
        this.extra = extra;
    }
}
        "#;

        let context = AnalysisContext::default();
        let file_path = PathBuf::from("test.ts");

        let result = analyzer
            .analyze_file(typescript_code, &file_path, "typescript", &context)
            .await;

        assert!(
            result.is_ok(),
            "TypeScript code analysis should succeed: {:?}",
            result.err()
        );

        let analysis_result = result.unwrap();
        assert!(
            !analysis_result.symbols.is_empty(),
            "Should extract symbols from TypeScript code"
        );
    }
}
