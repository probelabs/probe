#![cfg(feature = "legacy-tests")]
/// Integration test to verify that the regex compilation issues have been fixed
use lsp_daemon::symbol::Normalizer;

#[tokio::test]
async fn test_regex_patterns_compile_successfully() {
    // This test verifies that the normalization module can be created
    // without panicking due to regex compilation errors
    let normalizer = Normalizer::new();

    // Test basic normalization operations to ensure patterns work
    let result = normalizer.normalize_symbol_name("test_func", "rust");
    assert!(
        result.is_ok(),
        "Symbol normalization should work: {:?}",
        result
    );

    let result = normalizer.normalize_signature("fn test() -> i32", "rust");
    assert!(
        result.is_ok(),
        "Signature normalization should work: {:?}",
        result
    );

    let result = normalizer.split_qualified_name("std::collections::HashMap", "rust");
    assert!(
        result.is_ok(),
        "Qualified name splitting should work: {:?}",
        result
    );
}

#[tokio::test]
async fn test_around_operators_pattern() {
    let normalizer = Normalizer::new();

    // Test that the around_operators pattern works (this was one of the broken patterns)
    let test_signature = "test( arg1 , arg2 )";
    let result = normalizer.normalize_signature(test_signature, "rust");

    assert!(
        result.is_ok(),
        "Signature with operators should normalize: {:?}",
        result
    );

    // The result should not panic and should produce some normalized output
    let normalized = result.unwrap();
    assert!(
        !normalized.is_empty(),
        "Normalized signature should not be empty"
    );
}

#[tokio::test]
async fn test_java_params_pattern() {
    let normalizer = Normalizer::new();

    // Test Java method signature (this pattern had the lookahead issue)
    let java_sig = "public static void main(String args)";
    let result = normalizer.normalize_signature(java_sig, "java");

    assert!(
        result.is_ok(),
        "Java signature should normalize: {:?}",
        result
    );

    let normalized = result.unwrap();
    assert!(
        !normalized.is_empty(),
        "Normalized Java signature should not be empty"
    );
}
