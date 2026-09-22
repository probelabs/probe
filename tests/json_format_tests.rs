use serde_json::Value;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

// Helper function to extract JSON from command output
fn extract_json_from_output(output: &str) -> &str {
    // Find the first occurrence of '{'
    if let Some(start_index) = output.find('{') {
        // Return the substring from the first '{' to the end
        &output[start_index..]
    } else {
        // If no '{' is found, return the original string
        output
    }
}

// Helper function to create test files
fn create_test_file(dir: &TempDir, filename: &str, content: &str) -> PathBuf {
    let file_path = dir.path().join(filename);
    let parent_dir = file_path.parent().unwrap();
    fs::create_dir_all(parent_dir).expect("Failed to create parent directories");
    let mut file = File::create(&file_path).expect("Failed to create test file");
    file.write_all(content.as_bytes())
        .expect("Failed to write test content");
    file_path
}

// Helper function to create a test directory structure with various test files
fn create_test_directory_structure(root_dir: &TempDir) {
    // Create a source directory
    let src_dir = root_dir.path().join("src");
    fs::create_dir(&src_dir).expect("Failed to create src directory");

    // Create Rust files with search terms
    let rust_content = r#"
// This is a Rust file with a search term
fn search_function(query: &str) -> bool {
    println!("Searching for: {}", query);
    query.contains("search")
}
"#;
    create_test_file(root_dir, "src/search.rs", rust_content);

    // Create a JavaScript file with search terms
    let js_content = r#"
// This is a JavaScript file with a search term
function searchFunction(query) {
    console.log(`Searching for: ${query}`);
    return query.includes('search');
}
"#;
    create_test_file(root_dir, "src/search.js", js_content);

    // Create a file with special characters
    let special_chars_content = r#"
// This file contains special characters: "quotes", 'apostrophes', <tags>, &ampersands
function escapeTest(input) {
    return input.replace(/[<>&"']/g, function(c) {
        return {
            '<': '&lt;',
            '>': '&gt;',
            '&': '&amp;',
            '"': '&quot;',
            "'": '&#39;'
        }[c];
    });
}
"#;
    create_test_file(root_dir, "src/special_chars.js", special_chars_content);

    // Create a file with multiple search terms
    let multi_term_content = r#"
// This file contains multiple search terms
function processQuery(query) {
    // Check if the query contains multiple terms
    const terms = query.split(' ');

    // Process each term
    return terms.map(term => {
        return {
            term: term,
            isValid: validateTerm(term)
        };
    });
}

function validateTerm(term) {
    // Validate that the term is not empty and contains only allowed characters
    return term.length > 0 && /^[a-zA-Z0-9_-]+$/.test(term);
}
"#;
    create_test_file(root_dir, "src/multi_term.js", multi_term_content);
}

fn create_requirement_fixture(root_dir: &TempDir) {
    let service_ts = r#"export class PolicyService {
  // Implements: SYS-REQ-424
  async evaluatePolicy(input: string): Promise<boolean> {
    return input.length > 0 && input !== "deny";
  }
}

// Implements: SYS-REQ-425
export const normalizeDecision = (raw: string) => {
  return raw.trim().toLowerCase();
};
"#;
    create_test_file(root_dir, "web/src/service.ts", service_ts);

    let service_test_ts = r#"import { describe, it, expect, test } from "vitest";

// Verifies: SYS-REQ-424 [boundary]
test("accepts valid policy", () => {
  expect(true && true).toBe(true);
});

// MCDC SYS-REQ-424: input_valid=T, not_denied=T => TRUE
it("records witness row", () => {
  expect(true).toBe(true);
});

describe("normalization", () => {
  // Verifies: SYS-REQ-425
  it("normalizes decisions", () => {
    expect(" ALLOW ".trim().toLowerCase()).toBe("allow");
  });
});
"#;
    create_test_file(
        root_dir,
        "web/src/__tests__/service.test.ts",
        service_test_ts,
    );

    let demo_go = r#"package demo

// Implements: SYS-REQ-426
func RunDemo(flag bool) bool {
    return flag
}
"#;
    create_test_file(root_dir, "pkg/demo/demo.go", demo_go);

    let noise_ts = r#"export const literalOnly = "Implements: SYS-REQ-427";

export function unrelated() {
  return "SYS-REQ-428";
}

// SYS-REQ-429 appears here without an annotation verb.
export function looseComment() {
  return true;
}
"#;
    create_test_file(root_dir, "web/src/noise.ts", noise_ts);
}

fn find_result<'a>(results: &'a [Value], predicate: impl Fn(&'a Value) -> bool) -> &'a Value {
    results
        .iter()
        .find(|result| predicate(result))
        .expect("expected search result was not found")
}

#[test]
fn test_json_output_format_basic() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format
    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "search",
            "search", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--exclude-filenames",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the JSON part from the output
    let json_str = extract_json_from_output(&stdout);

    // Parse the JSON output
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the structure of the JSON output
    assert!(json_result.is_object(), "JSON output should be an object");
    assert!(
        json_result.get("results").is_some(),
        "JSON output should have a 'results' field"
    );
    assert!(
        json_result.get("summary").is_some(),
        "JSON output should have a 'summary' field"
    );

    // Validate the results array
    let results = json_result.get("results").unwrap();
    assert!(results.is_array(), "'results' should be an array");
    assert!(
        !results.as_array().unwrap().is_empty(),
        "'results' array should not be empty"
    );

    // Validate the structure of each result
    for result in results.as_array().unwrap() {
        assert!(
            result.get("file").is_some(),
            "Each result should have a 'file' field"
        );
        assert!(
            result.get("lines").is_some(),
            "Each result should have a 'lines' field"
        );
        assert!(
            result.get("node_type").is_some(),
            "Each result should have a 'node_type' field"
        );
        assert!(
            result.get("code").is_some(),
            "Each result should have a 'code' field"
        );
    }

    // Validate the summary object
    let summary = json_result.get("summary").unwrap();
    assert!(summary.is_object(), "'summary' should be an object");
    assert!(
        summary.get("count").is_some(),
        "'summary' should have a 'count' field"
    );
    assert!(
        summary.get("total_bytes").is_some(),
        "'summary' should have a 'total_bytes' field"
    );
    assert!(
        summary.get("total_tokens").is_some(),
        "'summary' should have a 'total_tokens' field"
    );

    // Validate the count matches the number of results
    let count = summary.get("count").unwrap().as_u64().unwrap();
    let results_count = results.as_array().unwrap().len() as u64;
    assert_eq!(
        count, results_count,
        "The 'count' in summary should match the number of results"
    );
}

#[test]
fn test_search_json_no_merge_reports_req_id_source_metadata() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_requirement_fixture(&temp_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "search",
            "--allow-tests",
            "--strict-elastic-syntax",
            "--max-results",
            "20",
            "--no-merge",
            "--format",
            "json",
            r#""SYS-REQ-424" OR "SYS-REQ-425""#,
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "search command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_str = extract_json_from_output(&stdout);
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");
    let results = json_result
        .get("results")
        .and_then(Value::as_array)
        .expect("results should be an array");

    assert_eq!(
        results.len(),
        5,
        "expected one result per semantic-ish block"
    );

    let method_result = find_result(results, |result| {
        result
            .get("code")
            .and_then(Value::as_str)
            .is_some_and(|code| code.contains("evaluatePolicy") && code.contains("SYS-REQ-424"))
    });
    assert_eq!(
        method_result.get("language").and_then(Value::as_str),
        Some("typescript")
    );
    assert_eq!(
        method_result.get("node_type").and_then(Value::as_str),
        Some("method_definition")
    );
    assert_eq!(
        method_result.get("owner_symbol").and_then(Value::as_str),
        Some("evaluatePolicy")
    );
    assert_eq!(
        method_result
            .get("owner_qualified_symbol")
            .and_then(Value::as_str),
        Some("PolicyService.evaluatePolicy")
    );
    let enclosing_symbols = method_result
        .get("enclosing_symbols")
        .and_then(Value::as_array)
        .expect("method result should expose containing symbols");
    assert_eq!(
        enclosing_symbols[0].get("kind").and_then(Value::as_str),
        Some("class")
    );
    assert_eq!(
        enclosing_symbols[0].get("name").and_then(Value::as_str),
        Some("PolicyService")
    );
    assert_eq!(
        method_result.get("scope").and_then(Value::as_str),
        Some("function")
    );

    let arrow_result = find_result(results, |result| {
        result
            .get("code")
            .and_then(Value::as_str)
            .is_some_and(|code| code.contains("normalizeDecision") && code.contains("SYS-REQ-425"))
    });
    assert_eq!(
        arrow_result.get("node_type").and_then(Value::as_str),
        Some("export_statement")
    );
    assert_eq!(
        arrow_result.get("owner_symbol").and_then(Value::as_str),
        Some("normalizeDecision")
    );
    assert_eq!(
        arrow_result
            .get("owner_qualified_symbol")
            .and_then(Value::as_str),
        Some("normalizeDecision")
    );
    assert_eq!(
        arrow_result.get("scope").and_then(Value::as_str),
        Some("function")
    );

    let callback_result = find_result(results, |result| {
        result
            .get("code")
            .and_then(Value::as_str)
            .is_some_and(|code| code.contains("test(\"accepts valid policy\""))
    });
    assert_eq!(
        callback_result.get("node_type").and_then(Value::as_str),
        Some("arrow_function")
    );
    assert_eq!(
        callback_result.get("scope").and_then(Value::as_str),
        Some("test")
    );
    assert_eq!(
        callback_result.get("is_test").and_then(Value::as_bool),
        Some(true)
    );
    let enclosing_calls = callback_result
        .get("enclosing_calls")
        .and_then(Value::as_array)
        .expect("callback result should expose enclosing call context");
    assert_eq!(
        enclosing_calls[0].get("callee").and_then(Value::as_str),
        Some("test")
    );
    assert_eq!(
        enclosing_calls[0]
            .get("first_arg_literal")
            .and_then(Value::as_str),
        Some("accepts valid policy")
    );

    let leading_comments = callback_result
        .get("leading_comments")
        .and_then(Value::as_array)
        .expect("callback result should expose leading comments");
    assert_eq!(
        leading_comments[0].get("text").and_then(Value::as_str),
        Some("// Verifies: SYS-REQ-424 [boundary]")
    );

    let matches = callback_result
        .get("matches")
        .and_then(Value::as_array)
        .expect("callback result should expose classified matches");
    assert_eq!(
        matches[0].get("text").and_then(Value::as_str),
        Some("SYS-REQ-424")
    );
    assert_eq!(
        matches[0].get("kind").and_then(Value::as_str),
        Some("comment")
    );
    assert_eq!(
        matches[0].get("comment_role").and_then(Value::as_str),
        Some("leading")
    );

    let nested_callback_result = find_result(results, |result| {
        result
            .get("code")
            .and_then(Value::as_str)
            .is_some_and(|code| code.contains("it(\"normalizes decisions\""))
    });
    let nested_calls = nested_callback_result
        .get("enclosing_calls")
        .and_then(Value::as_array)
        .expect("nested callback result should expose enclosing call chain");
    assert_eq!(
        nested_calls[0].get("callee").and_then(Value::as_str),
        Some("describe")
    );
    assert_eq!(
        nested_calls[0]
            .get("first_arg_literal")
            .and_then(Value::as_str),
        Some("normalization")
    );
    assert_eq!(
        nested_calls[1].get("callee").and_then(Value::as_str),
        Some("it")
    );
    assert_eq!(
        nested_calls[1]
            .get("first_arg_literal")
            .and_then(Value::as_str),
        Some("normalizes decisions")
    );
}

#[test]
fn test_search_json_classifies_req_id_noise_without_policy_interpretation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_requirement_fixture(&temp_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "search",
            "--allow-tests",
            "--strict-elastic-syntax",
            "--max-results",
            "20",
            "--no-merge",
            "--format",
            "json",
            r#""SYS-REQ-427" OR "SYS-REQ-428" OR "SYS-REQ-429""#,
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "search command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_str = extract_json_from_output(&stdout);
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");
    let results = json_result
        .get("results")
        .and_then(Value::as_array)
        .expect("results should be an array");

    let literal_result = find_result(results, |result| {
        result
            .get("code")
            .and_then(Value::as_str)
            .is_some_and(|code| code.contains("SYS-REQ-427"))
    });
    let literal_matches = literal_result
        .get("matches")
        .and_then(Value::as_array)
        .expect("literal result should expose classified matches");
    assert_eq!(
        literal_matches[0].get("kind").and_then(Value::as_str),
        Some("string")
    );

    let string_return_result = find_result(results, |result| {
        result
            .get("code")
            .and_then(Value::as_str)
            .is_some_and(|code| code.contains("SYS-REQ-428"))
    });
    let string_return_matches = string_return_result
        .get("matches")
        .and_then(Value::as_array)
        .expect("string return result should expose classified matches");
    assert_eq!(
        string_return_matches[0].get("kind").and_then(Value::as_str),
        Some("string")
    );

    let loose_comment_result = find_result(results, |result| {
        result
            .get("code")
            .and_then(Value::as_str)
            .is_some_and(|code| code.contains("SYS-REQ-429"))
    });
    let loose_comment_matches = loose_comment_result
        .get("matches")
        .and_then(Value::as_array)
        .expect("loose comment result should expose classified matches");
    assert_eq!(
        loose_comment_matches[0].get("kind").and_then(Value::as_str),
        Some("comment")
    );
    assert_eq!(
        loose_comment_matches[0]
            .get("comment_role")
            .and_then(Value::as_str),
        Some("leading")
    );
    assert!(
        loose_comment_result.get("requirement_policy").is_none(),
        "Probe should not add domain-specific requirement interpretation"
    );
}

#[test]
fn test_json_output_with_special_characters() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format, searching for special characters
    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "search",
            "special", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--exclude-filenames",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the JSON part from the output
    let json_str = extract_json_from_output(&stdout);

    // Parse the JSON output
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Find the result with special characters
    let results = json_result.get("results").unwrap().as_array().unwrap();
    let special_chars_result = results.iter().find(|r| {
        r.get("file")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("special_chars.js")
    });

    assert!(
        special_chars_result.is_some(),
        "Should find the special_chars.js file"
    );

    // Verify that special characters are properly escaped in the JSON
    let code = special_chars_result
        .unwrap()
        .get("code")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(
        code.contains("\"quotes\""),
        "Double quotes should be properly escaped in JSON"
    );
    assert!(
        code.contains("'apostrophes'"),
        "Apostrophes should be properly escaped in JSON"
    );
    assert!(
        code.contains("<tags>"),
        "Tags should be properly escaped in JSON"
    );
    assert!(
        code.contains("&ampersands"),
        "Ampersands should be properly escaped in JSON"
    );
}

#[test]
fn test_json_output_with_multiple_terms() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format, searching for multiple terms
    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "search",
            "term process", // Multiple terms to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--exclude-filenames",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the JSON part from the output
    let json_str = extract_json_from_output(&stdout);

    // Parse the JSON output
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the results array
    let results = json_result.get("results").unwrap().as_array().unwrap();

    // Should find the multi_term.js file
    let multi_term_result = results.iter().find(|r| {
        r.get("file")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("multi_term.js")
    });

    assert!(
        multi_term_result.is_some(),
        "Should find the multi_term.js file"
    );

    // Check if matched_keywords field contains both search terms
    if let Some(matched_keywords) = multi_term_result.unwrap().get("matched_keywords") {
        let keywords = matched_keywords.as_array().unwrap();
        let has_term = keywords
            .iter()
            .any(|k| k.as_str().unwrap().contains("term"));
        let has_process = keywords
            .iter()
            .any(|k| k.as_str().unwrap().contains("process"));

        assert!(has_term, "matched_keywords should contain 'term'");
        assert!(has_process, "matched_keywords should contain 'process'");
    }
}

#[test]
fn test_json_output_with_no_results() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format, searching for a term that doesn't exist
    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "search",
            "zzzzqqqqxxxx", // Term that doesn't exist in any file
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--exclude-filenames",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the JSON part from the output
    let json_str = extract_json_from_output(&stdout);

    // Parse the JSON output
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the structure of the JSON output
    assert!(json_result.is_object(), "JSON output should be an object");
    assert!(
        json_result.get("results").is_some(),
        "JSON output should have a 'results' field"
    );
    assert!(
        json_result.get("summary").is_some(),
        "JSON output should have a 'summary' field"
    );

    // Validate the results array is empty
    let results = json_result.get("results").unwrap().as_array().unwrap();
    assert!(results.is_empty(), "'results' array should be empty");

    // Validate the summary object
    let summary = json_result.get("summary").unwrap();
    assert_eq!(
        summary.get("count").unwrap().as_u64().unwrap(),
        0,
        "'count' should be 0"
    );
    assert_eq!(
        summary.get("total_bytes").unwrap().as_u64().unwrap(),
        0,
        "'total_bytes' should be 0"
    );
    assert_eq!(
        summary.get("total_tokens").unwrap().as_u64().unwrap(),
        0,
        "'total_tokens' should be 0"
    );
}

#[test]
fn test_json_output_with_files_only() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format and files-only option
    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "search",
            "search", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--files-only",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the JSON part from the output
    let json_str = extract_json_from_output(&stdout);

    // Parse the JSON output
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the results array
    let results = json_result.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "'results' array should not be empty");

    // Verify that all results have node_type = "file"
    for result in results {
        assert_eq!(
            result.get("node_type").unwrap().as_str().unwrap(),
            "file",
            "With --files-only, all results should have node_type = 'file'"
        );
    }
}
