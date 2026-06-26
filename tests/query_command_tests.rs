use anyhow::Result;
use probe_code::query::{perform_query, QueryOptions};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_query_rust_function() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path();

    // Create a test Rust file with a function
    let rust_file_path = temp_path.join("test_function.rs");
    let rust_content = r#"
fn hello_world() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
    fs::write(&rust_file_path, rust_content)?;

    // Create query options to search for function definitions
    let options = QueryOptions {
        path: temp_path,
        pattern: "fn $NAME($$$PARAMS) $$$BODY",
        language: Some("rust"),
        ignore: &[],
        allow_tests: true,
        max_results: None,
        with_context: false,
        format: "plain",
        no_gitignore: false,
    };

    // Perform the query
    let matches = perform_query(&options)?;

    // Verify that we found both functions
    assert_eq!(matches.len(), 2);

    // Check that the first match is the hello_world function
    let hello_match = matches
        .iter()
        .find(|m| m.matched_text.contains("hello_world"))
        .unwrap();
    assert!(hello_match.matched_text.contains("println!"));

    // Check that the second match is the add function
    let add_match = matches
        .iter()
        .find(|m| m.matched_text.contains("add"))
        .unwrap();
    assert!(add_match.matched_text.contains("a + b"));

    Ok(())
}

#[test]
fn test_query_javascript_function() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path();

    // Create a test JavaScript file with a function
    let js_file_path = temp_path.join("test_function.js");
    let js_content = r#"
function greet(name) {
    return `Hello, ${name}!`;
}

const multiply = (a, b) => a * b;
"#;
    fs::write(&js_file_path, js_content)?;

    // Create query options to search for function declarations
    let options = QueryOptions {
        path: temp_path,
        pattern: "function $NAME($$$PARAMS) $$$BODY",
        language: Some("javascript"),
        ignore: &[],
        allow_tests: true,
        max_results: None,
        with_context: false,
        format: "plain",
        no_gitignore: false,
    };

    // Perform the query
    let matches = perform_query(&options)?;

    // Verify that we found the function declaration (not the arrow function)
    assert_eq!(matches.len(), 1);
    assert!(matches[0].matched_text.contains("greet"));
    assert!(matches[0].matched_text.contains("return"));

    // Now search for arrow functions
    let arrow_options = QueryOptions {
        path: temp_path,
        pattern: "const $NAME = ($$$PARAMS) => $$$BODY",
        language: Some("javascript"),
        ignore: &[],
        allow_tests: true,
        max_results: None,
        with_context: false,
        format: "plain",
        no_gitignore: false,
    };

    // Perform the query
    let arrow_matches = perform_query(&arrow_options)?;

    // Verify that we found the arrow function
    assert_eq!(arrow_matches.len(), 1);
    assert!(arrow_matches[0].matched_text.contains("multiply"));
    assert!(arrow_matches[0].matched_text.contains("a * b"));

    Ok(())
}

#[test]
fn test_query_c_function_inside_preprocessor_heavy_body() -> Result<()> {
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path();

    let c_file_path = temp_path.join("acls_like.c");
    let c_content = r#"
typedef int mode_t;
typedef int SMB_ACL_T;
typedef int rsync_acl;

#ifndef HAVE_OSX_ACLS
static mode_t change_sacl_perms(SMB_ACL_T sacl, rsync_acl *racl, mode_t old_mode, mode_t mode)
{
    if (mode) {
#ifdef SMB_ACL_LOSES_SPECIAL_MODE_BITS
        if (mode & 01000)
            mode &= ~0077;
#else
        if (mode & 01000 && !(old_mode & 01000))
            mode &= ~0077;
    } else {
        if ((old_mode & 04000 && !(mode & 04000))
         || (old_mode & 02000 && !(mode & 02000)))
            mode &= ~0077;
#endif
    }

    return mode;
}
#endif
"#;
    fs::write(&c_file_path, c_content)?;

    let options = QueryOptions {
        path: temp_path,
        pattern: "$RET $NAME($$$PARAMS) { $$$BODY }",
        language: Some("c"),
        ignore: &[],
        allow_tests: true,
        max_results: None,
        with_context: false,
        format: "plain",
        no_gitignore: false,
    };

    let matches = perform_query(&options)?;

    assert!(
        matches
            .iter()
            .any(|m| m.matched_text.contains("change_sacl_perms")),
        "missing preprocessor-heavy C function in query results"
    );

    Ok(())
}

#[test]
fn test_query_c_recovery_does_not_widen_body_specific_patterns() -> Result<()> {
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path();

    let c_file_path = temp_path.join("body_specific.c");
    let c_content = r#"
static int change_sacl_perms(int mode)
{
#ifdef FEATURE
    if (mode)
        return mode;
#endif
    return mode;
}

int exact_return(void)
{
    return 7;
}
"#;
    fs::write(&c_file_path, c_content)?;

    let options = QueryOptions {
        path: temp_path,
        pattern: "int $NAME($$$PARAMS) { return 7; }",
        language: Some("c"),
        ignore: &[],
        allow_tests: true,
        max_results: None,
        with_context: false,
        format: "plain",
        no_gitignore: false,
    };

    let matches = perform_query(&options)?;

    assert!(
        matches
            .iter()
            .all(|m| !m.matched_text.contains("change_sacl_perms")),
        "C recovery should not add functions that do not satisfy a body-specific query"
    );

    Ok(())
}

#[test]
fn test_query_with_max_results() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path();

    // Create a test Rust file with multiple functions
    let rust_file_path = temp_path.join("test_multiple.rs");
    let rust_content = r#"
fn func1() {}
fn func2() {}
fn func3() {}
fn func4() {}
fn func5() {}
"#;
    fs::write(&rust_file_path, rust_content)?;

    // Create query options with max_results = 3
    let options = QueryOptions {
        path: temp_path,
        pattern: "fn $NAME() {}",
        language: Some("rust"),
        ignore: &[],
        allow_tests: true,
        max_results: Some(3),
        with_context: false,
        format: "plain",
        no_gitignore: false,
    };

    // Perform the query
    let matches = perform_query(&options)?;

    // Verify that we only got 3 matches (due to max_results)
    assert_eq!(matches.len(), 3);

    Ok(())
}

#[test]
fn test_query_ignore_patterns() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path();

    // Create a test file in the main directory
    let main_file_path = temp_path.join("main.rs");
    fs::write(&main_file_path, "fn main() {}")?;

    // Create a test file in a subdirectory that should be ignored
    let test_dir_path = temp_path.join("test");
    fs::create_dir(&test_dir_path)?;
    let test_file_path = test_dir_path.join("test_file.rs");
    fs::write(&test_file_path, "fn test_function() {}")?;

    // Create query options with ignore patterns
    let options = QueryOptions {
        path: temp_path,
        pattern: "fn $NAME() {}",
        language: Some("rust"),
        ignore: &["test".to_string()],
        allow_tests: false,
        max_results: None,
        with_context: false,
        format: "plain",
        no_gitignore: false,
    };

    // Perform the query
    let matches = perform_query(&options)?;

    // Verify that we only found the main function (test_function was ignored)
    assert_eq!(matches.len(), 1);
    assert!(matches[0].matched_text.contains("main"));
    assert!(!matches[0].matched_text.contains("test_function"));

    Ok(())
}

#[test]
fn test_query_with_auto_detect_language() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path();

    // Create a test Rust file with a function
    let rust_file_path = temp_path.join("test_auto_detect.rs");
    let rust_content = r#"
fn auto_detected_function() {
    println!("This function should be found with auto-detection");
}
"#;
    fs::write(&rust_file_path, rust_content)?;

    // Create query options without specifying language
    let options = QueryOptions {
        path: temp_path,
        pattern: "fn $NAME($$$PARAMS) $$$BODY",
        language: None, // No language specified, should auto-detect
        ignore: &[],
        allow_tests: true,
        max_results: None,
        with_context: false,
        format: "plain",
        no_gitignore: false,
    };

    // Perform the query
    let matches = perform_query(&options)?;

    // Verify that we found the function through auto-detection
    assert_eq!(matches.len(), 1);
    assert!(matches[0].matched_text.contains("auto_detected_function"));

    Ok(())
}
