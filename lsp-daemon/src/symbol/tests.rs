//! Comprehensive Test Suite for Symbol UID Generation
//!
//! This module contains extensive tests covering all aspects of the UID generation system,
//! including edge cases, performance tests, and integration scenarios.

use super::*;
use crate::symbol::{
    HashAlgorithm, SymbolContext, SymbolInfo, SymbolKind, SymbolLocation, SymbolUIDGenerator,
    Visibility,
};
use std::collections::HashSet;
use std::path::PathBuf;

/// Helper function to create a test symbol
fn create_symbol(name: &str, kind: SymbolKind, language: &str, line: u32, char: u32) -> SymbolInfo {
    let location = SymbolLocation::point(PathBuf::from("test.rs"), line, char);
    SymbolInfo::new(name.to_string(), kind, language.to_string(), location)
}

/// Helper function to create a test context
fn create_context(workspace_id: i64, scopes: Vec<&str>) -> SymbolContext {
    let mut context = SymbolContext::new(workspace_id, "rust".to_string());
    for scope in scopes {
        context = context.push_scope(scope.to_string());
    }
    context
}

#[cfg(test)]
mod uid_generation_tests {
    use super::*;

    #[test]
    fn test_global_function_uid_generation() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["module", "submodule"]);

        let symbol = create_symbol("calculate_total", SymbolKind::Function, "rust", 10, 5)
            .with_qualified_name("accounting::billing::calculate_total".to_string())
            .with_signature("fn calculate_total(items: &[Item]) -> f64".to_string());

        let uid = generator.generate_uid(&symbol, &context).unwrap();

        assert!(uid.starts_with("rust::"));
        assert!(uid.contains("accounting"));
        assert!(uid.contains("billing"));
        assert!(uid.contains("calculate_total"));

        // Should be deterministic
        let uid2 = generator.generate_uid(&symbol, &context).unwrap();
        assert_eq!(uid, uid2);
    }

    #[test]
    fn test_method_uid_with_overloading() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        // Java methods with same name but different signatures
        let method1 = create_symbol("process", SymbolKind::Method, "java", 20, 10)
            .with_qualified_name("com.example.Service.process".to_string())
            .with_signature("void process(String input)".to_string());

        let method2 = create_symbol("process", SymbolKind::Method, "java", 25, 10)
            .with_qualified_name("com.example.Service.process".to_string())
            .with_signature("void process(String input, int count)".to_string());

        let uid1 = generator.generate_uid(&method1, &context).unwrap();
        let uid2 = generator.generate_uid(&method2, &context).unwrap();

        // Should be different due to different signatures
        assert_ne!(uid1, uid2);
        assert!(uid1.contains("#")); // Should have signature hash
        assert!(uid2.contains("#"));

        // Base part should be the same
        let base1 = uid1.split('#').next().unwrap();
        let base2 = uid2.split('#').next().unwrap();
        assert_eq!(base1, base2);
    }

    #[test]
    fn test_local_variable_uid() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["function", "block"]);

        let var1 = create_symbol("local_var", SymbolKind::Variable, "rust", 30, 8);
        let var2 = create_symbol("local_var", SymbolKind::Variable, "rust", 35, 12); // Same name, different position

        let uid1 = generator.generate_uid(&var1, &context).unwrap();
        let uid2 = generator.generate_uid(&var2, &context).unwrap();

        // Should be different due to different positions
        assert_ne!(uid1, uid2);
        assert!(uid1.contains("local_var"));
        assert!(uid2.contains("local_var"));
        assert!(uid1.contains("#")); // Should have position hash
        assert!(uid2.contains("#"));
    }

    #[test]
    fn test_anonymous_symbol_uid() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["function"]);

        let lambda1 = create_symbol("lambda@123", SymbolKind::Anonymous, "python", 15, 20);
        let lambda2 = create_symbol("lambda@456", SymbolKind::Anonymous, "python", 15, 30); // Same line, different column

        let uid1 = generator.generate_uid(&lambda1, &context).unwrap();
        let uid2 = generator.generate_uid(&lambda2, &context).unwrap();

        // Should be different due to different positions
        assert_ne!(uid1, uid2);
        assert!(uid1.starts_with("python::"));
        assert!(uid2.starts_with("python::"));
        assert!(uid1.contains("lambda")); // Should use language-specific anonymous prefix
        assert!(uid2.contains("lambda"));
    }

    #[test]
    fn test_usr_based_uid() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        let symbol = create_symbol("test_func", SymbolKind::Function, "c", 10, 5)
            .with_usr("c:@F@test_func#I#".to_string());

        let uid = generator.generate_uid(&symbol, &context).unwrap();

        // Should use USR directly (highest priority)
        assert_eq!(uid, "c:@F@test_func#I#");
    }

    #[test]
    fn test_class_member_uid() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        let field = create_symbol("name", SymbolKind::Field, "typescript", 12, 5)
            .with_qualified_name("UserService.User.name".to_string())
            .with_visibility(Visibility::Private);

        let uid = generator.generate_uid(&field, &context).unwrap();

        assert!(uid.starts_with("typescript::"));
        assert!(uid.contains("UserService"));
        assert!(uid.contains("User"));
        assert!(uid.contains("name"));
    }
}

#[cfg(test)]
mod language_specific_tests {
    use super::*;

    #[test]
    fn test_rust_uid_generation() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["std", "collections"]);

        // Rust function
        let func = create_symbol("hash_map", SymbolKind::Function, "rust", 10, 5)
            .with_qualified_name("std::collections::hash_map".to_string())
            .with_signature("fn hash_map<K, V>() -> HashMap<K, V>".to_string());

        let uid = generator.generate_uid(&func, &context).unwrap();
        assert!(uid.starts_with("rust::"));
        assert!(uid.contains("std"));
        assert!(uid.contains("collections"));

        // Rust struct
        let struct_sym = create_symbol("HashMap", SymbolKind::Struct, "rust", 20, 5)
            .with_qualified_name("std::collections::HashMap".to_string());

        let struct_uid = generator.generate_uid(&struct_sym, &context).unwrap();
        assert!(struct_uid.starts_with("rust::"));
        assert!(struct_uid.contains("HashMap"));
    }

    #[test]
    fn test_typescript_uid_generation() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["services"]);

        // TypeScript interface
        let interface = create_symbol("UserService", SymbolKind::Interface, "typescript", 15, 5)
            .with_qualified_name("services.UserService".to_string())
            .with_signature("interface UserService { getUser(id: string): User; }".to_string());

        let uid = generator.generate_uid(&interface, &context).unwrap();
        assert!(uid.starts_with("typescript::"));
        assert!(uid.contains("services"));
        assert!(uid.contains("UserService"));

        // TypeScript method with overloading
        let method1 = create_symbol("getUser", SymbolKind::Method, "typescript", 20, 10)
            .with_qualified_name("services.UserService.getUser".to_string())
            .with_signature("getUser(id: string): User".to_string());

        let method2 = create_symbol("getUser", SymbolKind::Method, "typescript", 21, 10)
            .with_qualified_name("services.UserService.getUser".to_string())
            .with_signature("getUser(id: number): User".to_string());

        let uid1 = generator.generate_uid(&method1, &context).unwrap();
        let uid2 = generator.generate_uid(&method2, &context).unwrap();

        // TypeScript supports overloading, so UIDs should be different
        assert_ne!(uid1, uid2);
    }

    #[test]
    fn test_python_uid_generation() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["package", "module"]);

        // Python class
        let class = create_symbol("UserService", SymbolKind::Class, "python", 10, 5)
            .with_qualified_name("package.module.UserService".to_string());

        let uid = generator.generate_uid(&class, &context).unwrap();
        assert!(uid.starts_with("python::"));
        assert!(uid.contains("package"));
        assert!(uid.contains("module"));
        assert!(uid.contains("UserService"));

        // Python lambda (anonymous)
        let lambda = create_symbol("lambda@line_25", SymbolKind::Anonymous, "python", 25, 15);
        let lambda_uid = generator.generate_uid(&lambda, &context).unwrap();
        assert!(lambda_uid.contains("lambda")); // Should use Python's lambda prefix
    }

    #[test]
    fn test_go_uid_generation() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        // Go function
        let func = create_symbol("ProcessData", SymbolKind::Function, "go", 20, 5)
            .with_qualified_name("github.com/example/service.ProcessData".to_string())
            .with_signature("func ProcessData(data []string) error".to_string());

        let uid = generator.generate_uid(&func, &context).unwrap();
        assert!(uid.starts_with("go::"));
        assert!(uid.contains("ProcessData"));

        // Go struct
        let struct_sym = create_symbol("User", SymbolKind::Struct, "go", 30, 5)
            .with_qualified_name("github.com/example/models.User".to_string());

        let struct_uid = generator.generate_uid(&struct_sym, &context).unwrap();
        assert!(struct_uid.contains("User"));
    }

    #[test]
    fn test_java_uid_generation() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        // Java class
        let class = create_symbol("UserService", SymbolKind::Class, "java", 10, 5)
            .with_qualified_name("com.example.service.UserService".to_string());

        let uid = generator.generate_uid(&class, &context).unwrap();
        assert!(uid.starts_with("java::"));
        assert!(uid.contains("com"));
        assert!(uid.contains("example"));
        assert!(uid.contains("service"));
        assert!(uid.contains("UserService"));
    }

    #[test]
    fn test_cpp_uid_generation() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        // C++ class with namespace
        let class = create_symbol("Vector", SymbolKind::Class, "cpp", 15, 5)
            .with_qualified_name("std::vector::Vector".to_string());

        let uid = generator.generate_uid(&class, &context).unwrap();
        assert!(uid.starts_with("cpp::"));
        assert!(uid.contains("std"));
        assert!(uid.contains("vector"));
        assert!(uid.contains("Vector"));

        // C++ function overloading
        let func1 = create_symbol("process", SymbolKind::Function, "cpp", 20, 5)
            .with_qualified_name("utils::process".to_string())
            .with_signature("void process(int value)".to_string());

        let func2 = create_symbol("process", SymbolKind::Function, "cpp", 25, 5)
            .with_qualified_name("utils::process".to_string())
            .with_signature("void process(std::string value)".to_string());

        let uid1 = generator.generate_uid(&func1, &context).unwrap();
        let uid2 = generator.generate_uid(&func2, &context).unwrap();

        // C++ supports overloading, UIDs should be different
        assert_ne!(uid1, uid2);
        assert!(uid1.contains("#"));
        assert!(uid2.contains("#"));
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_empty_inputs() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        // Empty symbol name should fail
        let location = SymbolLocation::point(PathBuf::from("test.rs"), 10, 5);
        let empty_name = SymbolInfo::new(
            "".to_string(),
            SymbolKind::Function,
            "rust".to_string(),
            location.clone(),
        );
        assert!(generator.generate_uid(&empty_name, &context).is_err());

        // Empty language should fail
        let empty_lang = SymbolInfo::new(
            "test".to_string(),
            SymbolKind::Function,
            "".to_string(),
            location,
        );
        assert!(generator.generate_uid(&empty_lang, &context).is_err());
    }

    #[test]
    fn test_unsupported_language() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        let symbol = create_symbol("test", SymbolKind::Function, "unsupported_language", 10, 5);
        let result = generator.generate_uid(&symbol, &context);

        assert!(result.is_err());
        match result.unwrap_err() {
            UIDError::UnsupportedLanguage { language } => {
                assert_eq!(language, "unsupported_language");
            }
            _ => panic!("Expected UnsupportedLanguage error"),
        }
    }

    #[test]
    fn test_special_characters_in_names() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        // Rust operator overloading (allowed special characters)
        let operator = create_symbol("operator+", SymbolKind::Function, "cpp", 10, 5)
            .with_qualified_name("MyClass::operator+".to_string());

        let uid = generator.generate_uid(&operator, &context).unwrap();
        assert!(uid.contains("operator+"));

        // Names with Unicode characters
        let unicode_name = create_symbol("测试函数", SymbolKind::Function, "rust", 15, 5);
        let unicode_uid = generator.generate_uid(&unicode_name, &context).unwrap();
        assert!(unicode_uid.contains("测试函数"));
    }

    #[test]
    fn test_very_long_names() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        // Very long symbol name
        let long_name = "a".repeat(1000);
        let symbol = create_symbol(&long_name, SymbolKind::Function, "rust", 10, 5);

        let uid = generator.generate_uid(&symbol, &context).unwrap();
        assert!(uid.len() > 0);
        assert!(uid.len() < 2000); // Should not be excessively long due to hashing
    }

    #[test]
    fn test_deeply_nested_scopes() {
        let generator = SymbolUIDGenerator::new();

        // Create deeply nested scope
        let deep_scopes: Vec<&str> = (0..100).map(|_i| "scope").collect();
        let context = create_context(1, deep_scopes);

        let symbol = create_symbol("nested_func", SymbolKind::Function, "rust", 10, 5);
        let uid = generator.generate_uid(&symbol, &context).unwrap();

        assert!(uid.contains("nested_func"));
        assert!(uid.contains("scope"));
    }

    #[test]
    fn test_duplicate_scope_names() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["module", "module", "submodule", "module"]);

        let symbol = create_symbol("func", SymbolKind::Function, "rust", 10, 5);
        let uid = generator.generate_uid(&symbol, &context).unwrap();

        assert!(uid.contains("func"));
        // Should handle duplicate scope names gracefully
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    fn test_batch_uid_generation_performance() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["module"]);

        // Create a batch of symbols
        let mut symbols = Vec::new();
        for i in 0..1000 {
            let symbol = create_symbol(
                &format!("func_{}", i),
                SymbolKind::Function,
                "rust",
                i as u32 + 10,
                5,
            );
            symbols.push((symbol, context.clone()));
        }

        let start = std::time::Instant::now();
        let results = generator.generate_batch_uids(&symbols);
        let duration = start.elapsed();

        // Should complete in reasonable time (adjust threshold as needed)
        assert!(duration.as_millis() < 1000);

        // All should succeed
        assert_eq!(results.len(), 1000);
        for result in &results {
            assert!(result.is_ok());
        }

        // All should be unique
        let uids: HashSet<String> = results.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(uids.len(), 1000);
    }

    #[test]
    fn test_uid_generation_consistency() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["module"]);

        let symbol = create_symbol("test_func", SymbolKind::Function, "rust", 10, 5)
            .with_qualified_name("module::test_func".to_string());

        // Generate UID multiple times
        let mut uids = Vec::new();
        for _ in 0..100 {
            let uid = generator.generate_uid(&symbol, &context).unwrap();
            uids.push(uid);
        }

        // All UIDs should be identical (deterministic)
        for uid in &uids[1..] {
            assert_eq!(uid, &uids[0]);
        }
    }

    #[test]
    fn test_hash_algorithm_performance() {
        let blake3_gen = SymbolUIDGenerator::with_hash_algorithm(HashAlgorithm::Blake3);
        let sha256_gen = SymbolUIDGenerator::with_hash_algorithm(HashAlgorithm::Sha256);

        let context = create_context(1, vec![]);
        let symbols: Vec<_> = (0..100)
            .map(|i| {
                create_symbol(
                    &format!("func_{}", i),
                    SymbolKind::Function,
                    "rust",
                    i as u32 + 10,
                    5,
                )
            })
            .collect();

        // Measure Blake3 performance
        let start = std::time::Instant::now();
        for symbol in &symbols {
            let _ = blake3_gen.generate_uid(symbol, &context).unwrap();
        }
        let blake3_duration = start.elapsed();

        // Measure SHA256 performance
        let start = std::time::Instant::now();
        for symbol in &symbols {
            let _ = sha256_gen.generate_uid(symbol, &context).unwrap();
        }
        let sha256_duration = start.elapsed();

        // Both should be reasonably fast
        assert!(blake3_duration.as_millis() < 100);
        assert!(sha256_duration.as_millis() < 100);

        println!(
            "Blake3: {:?}, SHA256: {:?}",
            blake3_duration, sha256_duration
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_database_integration() {
        use crate::database::SymbolState;

        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["module"]);

        let symbol = create_symbol("test_func", SymbolKind::Function, "rust", 10, 5)
            .with_qualified_name("module::test_func".to_string())
            .with_signature("fn test_func() -> i32".to_string())
            .with_visibility(Visibility::Public);

        // Generate UID
        let uid = generator.generate_uid(&symbol, &context).unwrap();

        // Convert to database format
        let mut db_symbol: SymbolState = symbol.into();
        db_symbol.symbol_uid = uid;
        db_symbol.file_path = "test/path.rs".to_string();
        db_symbol.language = context.language.clone();

        // Verify conversion
        assert!(!db_symbol.symbol_uid.is_empty());
        assert_eq!(db_symbol.name, "test_func");
        assert_eq!(db_symbol.fqn, Some("module::test_func".to_string()));
        assert_eq!(db_symbol.kind, "function");
        assert_eq!(db_symbol.visibility, Some("public".to_string()));
        assert_eq!(db_symbol.file_path, "test/path.rs".to_string());
        assert_eq!(db_symbol.language, context.language);
    }

    #[test]
    fn test_indexing_pipeline_integration() {
        use crate::indexing::pipelines::SymbolInfo as IndexingSymbolInfo;

        // Create indexing symbol
        let indexing_symbol = IndexingSymbolInfo {
            name: "test_func".to_string(),
            kind: "function".to_string(),
            line: 10,
            column: 5,
            end_line: Some(15),
            end_column: Some(10),
            documentation: Some("Test function".to_string()),
            signature: Some("fn test_func() -> i32".to_string()),
            visibility: Some("public".to_string()),
            priority: None,
            is_exported: true,
            attributes: std::collections::HashMap::new(),
        };

        // Convert to symbol UID format
        let mut symbol: SymbolInfo = indexing_symbol.into();
        symbol.location.file_path = PathBuf::from("src/lib.rs");
        symbol.language = "rust".to_string();

        // Generate UID
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec!["crate"]);
        let uid = generator.generate_uid(&symbol, &context).unwrap();

        assert!(!uid.is_empty());
        assert!(uid.contains("test_func"));
    }

    #[test]
    fn test_cross_language_consistency() {
        let generator = SymbolUIDGenerator::new();

        // Similar functions in different languages should have different UIDs due to language prefix
        let rust_func = create_symbol("process", SymbolKind::Function, "rust", 10, 5)
            .with_qualified_name("module::process".to_string());
        let java_func = create_symbol("process", SymbolKind::Function, "java", 10, 5)
            .with_qualified_name("module.process".to_string());

        let context = create_context(1, vec![]);

        let rust_uid = generator.generate_uid(&rust_func, &context).unwrap();
        let java_uid = generator.generate_uid(&java_func, &context).unwrap();

        assert_ne!(rust_uid, java_uid);
        assert!(rust_uid.starts_with("rust::"));
        assert!(java_uid.starts_with("java::"));
    }

    #[test]
    fn test_workspace_isolation() {
        let generator = SymbolUIDGenerator::new();

        let symbol = create_symbol("func", SymbolKind::Function, "rust", 10, 5)
            .with_qualified_name("module::func".to_string());

        let context1 = create_context(1, vec!["module"]);
        let context2 = create_context(2, vec!["module"]);

        let uid1 = generator.generate_uid(&symbol, &context1).unwrap();
        let uid2 = generator.generate_uid(&symbol, &context2).unwrap();

        // Same symbol in different workspaces should have same UID (workspace doesn't affect UID)
        assert_eq!(uid1, uid2);
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn test_uid_validation() {
        let generator = SymbolUIDGenerator::new();

        // Valid UIDs
        assert!(generator.validate_uid("rust::module::function"));
        assert!(generator.validate_uid("java::com::example::Class::method#abc12345"));
        assert!(generator.validate_uid("typescript::services::UserService"));

        // Invalid UIDs
        assert!(!generator.validate_uid(""));
        assert!(!generator.validate_uid("a"));
        assert!(!generator.validate_uid("no_separator"));
        assert!(!generator.validate_uid("::"));
        assert!(!generator.validate_uid("::empty"));
    }

    #[test]
    fn test_language_extraction() {
        let generator = SymbolUIDGenerator::new();

        assert_eq!(
            generator.extract_language_from_uid("rust::module::function"),
            Some("rust".to_string())
        );
        assert_eq!(
            generator.extract_language_from_uid("java::com::example::Class"),
            Some("java".to_string())
        );
        assert_eq!(
            generator.extract_language_from_uid("typescript::services::UserService"),
            Some("typescript".to_string())
        );

        // Edge cases
        assert_eq!(generator.extract_language_from_uid("single"), None);
        assert_eq!(generator.extract_language_from_uid(""), None);
        assert_eq!(
            generator.extract_language_from_uid("::no_language"),
            Some("".to_string())
        );
    }

    #[test]
    fn test_uid_format_consistency() {
        let generator = SymbolUIDGenerator::new();
        let context = create_context(1, vec![]);

        // Generate UIDs for different types of symbols
        let symbols = vec![
            create_symbol("func", SymbolKind::Function, "rust", 10, 5),
            create_symbol("Class", SymbolKind::Class, "java", 20, 10),
            create_symbol("interface", SymbolKind::Interface, "typescript", 30, 15),
            create_symbol("variable", SymbolKind::Variable, "python", 40, 20),
        ];

        for symbol in symbols {
            let uid = generator.generate_uid(&symbol, &context).unwrap();

            // All UIDs should be valid
            assert!(generator.validate_uid(&uid));

            // Should contain language prefix
            assert!(uid.contains("::"));

            // Should extract language correctly
            let extracted_lang = generator.extract_language_from_uid(&uid);
            assert!(extracted_lang.is_some());
            assert_eq!(extracted_lang.unwrap(), symbol.language);
        }
    }

    #[test]
    fn test_generator_statistics() {
        let generator = SymbolUIDGenerator::new();
        let stats = generator.get_stats();

        assert!(stats.contains_key("hash_algorithm"));
        assert!(stats.contains_key("supported_languages"));
        assert!(stats.contains_key("languages"));

        // Verify Blake3 is default
        assert_eq!(stats["hash_algorithm"], "Blake3");

        // Verify we support multiple languages
        let lang_count: usize = stats["supported_languages"].parse().unwrap();
        assert!(lang_count >= 7); // At least Rust, TS, JS, Python, Go, Java, C, C++

        // Verify language list contains expected languages
        let languages = &stats["languages"];
        assert!(languages.contains("rust"));
        assert!(languages.contains("java"));
        assert!(languages.contains("typescript"));
    }
}
