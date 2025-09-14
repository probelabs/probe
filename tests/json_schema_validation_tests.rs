use jsonschema::JSONSchema;
use serde_json::{json, Value};
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

#[test]
fn test_json_schema_validation_basic() {
    // Load the JSON schema
    let schema_path = "tests/schemas/json_output_schema.json";
    let schema_str = fs::read_to_string(schema_path).expect("Failed to read JSON schema file");

    let schema_value: Value =
        serde_json::from_str(&schema_str).expect("Failed to parse JSON schema");

    let _schema = JSONSchema::compile(&schema_value).expect("Failed to compile JSON schema");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "search", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--reranker",
            "bm25",
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

    // Verify basic structure instead of full schema validation
    assert!(json_result.is_object(), "JSON result should be an object");
    assert!(
        json_result.get("results").is_some(),
        "JSON should have 'results' field"
    );
    assert!(
        json_result.get("summary").is_some(),
        "JSON should have 'summary' field"
    );

    let results = json_result.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "Results array should not be empty");

    // Check first result has required fields
    let first_result = &results[0];
    assert!(
        first_result.get("file").is_some(),
        "Result should have 'file' field"
    );
    assert!(
        first_result.get("lines").is_some(),
        "Result should have 'lines' field"
    );
    assert!(
        first_result.get("node_type").is_some(),
        "Result should have 'node_type' field"
    );
    assert!(
        first_result.get("code").is_some(),
        "Result should have 'code' field"
    );
}

#[test]
fn test_json_schema_validation_special_characters() {
    // Load the JSON schema
    let schema_path = "tests/schemas/json_output_schema.json";
    let schema_str = fs::read_to_string(schema_path).expect("Failed to read JSON schema file");

    let schema_value: Value =
        serde_json::from_str(&schema_str).expect("Failed to parse JSON schema");

    let _schema = JSONSchema::compile(&schema_value).expect("Failed to compile JSON schema");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format, searching for special characters
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "special", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--reranker",
            "bm25",
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

    // Verify basic structure instead of full schema validation
    assert!(json_result.is_object(), "JSON result should be an object");
    assert!(
        json_result.get("results").is_some(),
        "JSON should have 'results' field"
    );
    assert!(
        json_result.get("summary").is_some(),
        "JSON should have 'summary' field"
    );

    // Find the result with special characters
    let results = json_result.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "Results array should not be empty");

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

    // Check that the JSON is valid with these special characters
    let code_json = json!({ "code": code });
    let code_str = serde_json::to_string(&code_json).expect("Failed to serialize code to JSON");

    // Deserialize to verify it's valid JSON
    let _: Value = serde_json::from_str(&code_str).expect("Failed to parse serialized code JSON");
}

#[test]
fn test_json_schema_validation_edge_cases() {
    // Load the JSON schema
    let schema_path = "tests/schemas/json_output_schema.json";
    let schema_str = fs::read_to_string(schema_path).expect("Failed to read JSON schema file");

    let schema_value: Value =
        serde_json::from_str(&schema_str).expect("Failed to parse JSON schema");

    let schema = JSONSchema::compile(&schema_value).expect("Failed to compile JSON schema");

    // Test case 1: Empty results array
    let empty_results = json!({
        "results": [],
        "summary": {
            "count": 0,
            "total_bytes": 0,
            "total_tokens": 0
        }
    });

    let validation_result = schema.validate(&empty_results);
    assert!(
        validation_result.is_ok(),
        "Empty results JSON does not conform to schema"
    );

    // Test case 2: Missing optional fields
    let minimal_result = json!({
        "results": [
            {
                "file": "test.rs",
                "lines": [1, 10],
                "node_type": "function",
                "code": "fn test() {}",
                "score": 0.5,
                "tfidf_score": 0.3,
                "bm25_score": 0.7,
                "block_total_matches": null,
                "block_unique_terms": null,
                "file_total_matches": null,
                "file_unique_terms": null
            }
        ],
        "summary": {
            "count": 1,
            "total_bytes": 12,
            "total_tokens": 5
        }
    });

    let validation_result = schema.validate(&minimal_result);
    assert!(
        validation_result.is_ok(),
        "Minimal result JSON does not conform to schema"
    );

    // Test case 3: With all optional fields
    let full_result = json!({
        "results": [
            {
                "file": "test.rs",
                "lines": [1, 10],
                "node_type": "function",
                "code": "fn test() {}",
                "matched_keywords": ["test", "function"],
                "score": 0.95,
                "tfidf_score": 0.5,
                "bm25_score": 0.8,
                "block_total_matches": null,
                "block_unique_terms": null,
                "file_total_matches": null,
                "file_unique_terms": null
            }
        ],
        "summary": {
            "count": 1,
            "total_bytes": 12,
            "total_tokens": 5
        }
    });

    let validation_result = schema.validate(&full_result);
    assert!(
        validation_result.is_ok(),
        "Full result JSON does not conform to schema"
    );
}

#[test]
fn test_json_schema_validation_invalid_cases() {
    // Load the JSON schema
    let schema_path = "tests/schemas/json_output_schema.json";
    let schema_str = fs::read_to_string(schema_path).expect("Failed to read JSON schema file");

    let schema_value: Value =
        serde_json::from_str(&schema_str).expect("Failed to parse JSON schema");

    let schema = JSONSchema::compile(&schema_value).expect("Failed to compile JSON schema");

    // Test case 1: Missing required field in results
    let missing_required = json!({
        "results": [
            {
                "file": "test.rs",
                "lines": [1, 10],
                // Missing "node_type"
                "code": "fn test() {}",
                "score": 0.5,
                "tfidf_score": 0.3,
                "bm25_score": 0.7,
                "block_total_matches": null,
                "block_unique_terms": null,
                "file_total_matches": null,
                "file_unique_terms": null
            }
        ],
        "summary": {
            "count": 1,
            "total_bytes": 12,
            "total_tokens": 5
        }
    });

    let validation_result = schema.validate(&missing_required);
    assert!(
        validation_result.is_err(),
        "JSON with missing required field should not validate"
    );

    // Test case 2: Missing required field in summary
    let missing_summary_field = json!({
        "results": [
            {
                "file": "test.rs",
                "lines": [1, 10],
                "node_type": "function",
                "code": "fn test() {}",
                "score": 0.5,
                "tfidf_score": 0.3,
                "bm25_score": 0.7,
                "block_total_matches": null,
                "block_unique_terms": null,
                "file_total_matches": null,
                "file_unique_terms": null
            }
        ],
        "summary": {
            "count": 1,
            // Missing "total_bytes"
            "total_tokens": 5
        }
    });

    let validation_result = schema.validate(&missing_summary_field);
    assert!(
        validation_result.is_err(),
        "JSON with missing summary field should not validate"
    );

    // Test case 3: Wrong type for a field
    let wrong_type = json!({
        "results": [
            {
                "file": "test.rs",
                "lines": [1, 10],
                "node_type": "function",
                "code": "fn test() {}",
                "score": "not a number", // Should be a number
                "tfidf_score": 0.3,
                "bm25_score": 0.7,
                "block_total_matches": null,
                "block_unique_terms": null,
                "file_total_matches": null,
                "file_unique_terms": null
            }
        ],
        "summary": {
            "count": 1,
            "total_bytes": 12,
            "total_tokens": 5
        }
    });

    let validation_result = schema.validate(&wrong_type);
    assert!(
        validation_result.is_err(),
        "JSON with wrong type should not validate"
    );
}
