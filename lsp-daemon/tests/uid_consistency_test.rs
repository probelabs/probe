#![cfg(feature = "legacy-tests")]
//! Integration test to verify UID consistency between storage and query paths
//!
//! This test validates that both the LspDatabaseAdapter (storage path) and
//! daemon's generate_consistent_symbol_uid (query path) produce identical UIDs
//! for the same symbol using the new version-aware UID format.

use lsp_daemon::symbol::generate_version_aware_uid;
use std::path::PathBuf;

#[test]
fn test_version_aware_uid_format() {
    let workspace_root = PathBuf::from("/home/user/project");
    let file_path = PathBuf::from("/home/user/project/src/main.rs");
    let file_content = r#"
fn main() {
    println!("Hello, world!");
}

fn calculate_total(items: &[f64]) -> f64 {
    items.iter().sum()
}
"#;
    let symbol_name = "calculate_total";
    let line_number = 6;

    // Test UID generation
    let uid = generate_version_aware_uid(
        &workspace_root,
        &file_path,
        file_content,
        symbol_name,
        line_number,
    )
    .expect("Failed to generate UID");

    // Verify UID format: "relative/path:hash:symbol:line"
    let parts: Vec<&str> = uid.split(':').collect();
    assert_eq!(
        parts.len(),
        4,
        "UID should have 4 parts separated by colons"
    );

    // Verify relative path
    assert_eq!(
        parts[0], "src/main.rs",
        "First part should be relative path"
    );

    // Verify hash format (8 hex characters)
    assert_eq!(parts[1].len(), 8, "Hash should be 8 characters");
    assert!(
        parts[1].chars().all(|c| c.is_ascii_hexdigit()),
        "Hash should be hexadecimal"
    );

    // Verify symbol name
    assert_eq!(parts[2], symbol_name, "Third part should be symbol name");

    // Verify line number
    assert_eq!(
        parts[3],
        line_number.to_string(),
        "Fourth part should be line number"
    );

    println!("Generated UID: {}", uid);
}

#[test]
fn test_uid_consistency_same_input() {
    let workspace_root = PathBuf::from("/home/user/project");
    let file_path = PathBuf::from("/home/user/project/src/lib.rs");
    let file_content = "fn test() { return 42; }";
    let symbol_name = "test";
    let line_number = 1;

    // Generate UID twice with same inputs
    let uid1 = generate_version_aware_uid(
        &workspace_root,
        &file_path,
        file_content,
        symbol_name,
        line_number,
    )
    .unwrap();

    let uid2 = generate_version_aware_uid(
        &workspace_root,
        &file_path,
        file_content,
        symbol_name,
        line_number,
    )
    .unwrap();

    assert_eq!(uid1, uid2, "Same inputs should produce identical UIDs");
}

#[test]
fn test_uid_different_content() {
    let workspace_root = PathBuf::from("/home/user/project");
    let file_path = PathBuf::from("/home/user/project/src/lib.rs");
    let symbol_name = "test";
    let line_number = 1;

    let content1 = "fn test() { return 42; }";
    let content2 = "fn test() { return 43; }";

    let uid1 = generate_version_aware_uid(
        &workspace_root,
        &file_path,
        content1,
        symbol_name,
        line_number,
    )
    .unwrap();

    let uid2 = generate_version_aware_uid(
        &workspace_root,
        &file_path,
        content2,
        symbol_name,
        line_number,
    )
    .unwrap();

    assert_ne!(
        uid1, uid2,
        "Different content should produce different UIDs"
    );

    // Verify only the hash part is different
    let parts1: Vec<&str> = uid1.split(':').collect();
    let parts2: Vec<&str> = uid2.split(':').collect();

    assert_eq!(parts1[0], parts2[0], "Path should be same");
    assert_ne!(parts1[1], parts2[1], "Hash should be different");
    assert_eq!(parts1[2], parts2[2], "Symbol should be same");
    assert_eq!(parts1[3], parts2[3], "Line should be same");
}

#[test]
fn test_uid_external_file() {
    let workspace_root = PathBuf::from("/home/user/project");
    let external_file = PathBuf::from("/tmp/external.rs");
    let file_content = "fn external() {}";
    let symbol_name = "external";
    let line_number = 1;

    let uid = generate_version_aware_uid(
        &workspace_root,
        &external_file,
        file_content,
        symbol_name,
        line_number,
    )
    .unwrap();

    assert!(
        uid.starts_with("EXTERNAL:"),
        "External files should start with EXTERNAL: prefix"
    );
    assert!(
        uid.contains("/tmp/external.rs"),
        "Should contain the external file path"
    );
}

#[test]
fn test_uid_different_symbols_same_file() {
    let workspace_root = PathBuf::from("/home/user/project");
    let file_path = PathBuf::from("/home/user/project/src/math.rs");
    let file_content = r#"
fn add(a: i32, b: i32) -> i32 { a + b }
fn multiply(a: i32, b: i32) -> i32 { a * b }
"#;

    let uid1 =
        generate_version_aware_uid(&workspace_root, &file_path, file_content, "add", 2).unwrap();

    let uid2 = generate_version_aware_uid(&workspace_root, &file_path, file_content, "multiply", 3)
        .unwrap();

    assert_ne!(
        uid1, uid2,
        "Different symbols should produce different UIDs"
    );

    // Verify path and hash are same, but symbol and line are different
    let parts1: Vec<&str> = uid1.split(':').collect();
    let parts2: Vec<&str> = uid2.split(':').collect();

    assert_eq!(parts1[0], parts2[0], "Path should be same");
    assert_eq!(parts1[1], parts2[1], "Hash should be same (same content)");
    assert_ne!(parts1[2], parts2[2], "Symbol should be different");
    assert_ne!(parts1[3], parts2[3], "Line should be different");
}

#[test]
fn test_uid_empty_content() {
    let workspace_root = PathBuf::from("/home/user/project");
    let file_path = PathBuf::from("/home/user/project/src/empty.rs");
    let file_content = "";
    let symbol_name = "phantom";
    let line_number = 1;

    let uid = generate_version_aware_uid(
        &workspace_root,
        &file_path,
        file_content,
        symbol_name,
        line_number,
    )
    .unwrap();

    // Should handle empty content gracefully
    assert!(
        uid.contains("00000000"),
        "Empty content should have consistent hash"
    );
    assert!(uid.contains("phantom"), "Should contain symbol name");
}

#[test]
fn test_uid_validation_edge_cases() {
    let workspace_root = PathBuf::from("/project");
    let file_path = PathBuf::from("/project/test.rs");
    let file_content = "test";

    // Test empty symbol name - should fail
    let result = generate_version_aware_uid(&workspace_root, &file_path, file_content, "", 1);
    assert!(result.is_err(), "Empty symbol name should fail");

    // Test zero line number - should fail
    let result = generate_version_aware_uid(&workspace_root, &file_path, file_content, "test", 0);
    assert!(result.is_err(), "Zero line number should fail");

    // Test special characters in symbol name - should work
    let uid =
        generate_version_aware_uid(&workspace_root, &file_path, file_content, "operator++", 5)
            .unwrap();
    assert!(
        uid.contains("operator++"),
        "Special characters should be preserved"
    );
}
