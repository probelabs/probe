use roxmltree::{Document, Node};
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

// Helper function to extract XML from command output
fn extract_xml_from_output(output: &str) -> &str {
    // Find the first occurrence of '<?xml'
    if let Some(start_index) = output.find("<?xml") {
        // Return the substring from the first '<?xml' to the end
        &output[start_index..]
    } else {
        // If no '<?xml' is found, return the original string
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

// Helper function to create a test directory with edge case files
fn create_edge_case_test_files(root_dir: &TempDir) {
    // Create a source directory
    let src_dir = root_dir.path().join("src");
    fs::create_dir(&src_dir).expect("Failed to create src directory");

    // File with extremely long lines
    let long_line_content = format!(
        r#"
// This file contains an extremely long line
function longLine() {{
    const longString = "{}";
    return longString;
}}
"#,
        "x".repeat(10000) // Create a very long string
    );
    create_test_file(root_dir, "src/long_line.js", &long_line_content);

    // File with a large number of short lines
    let mut many_lines_content = String::from(
        r#"
// This file contains many short lines
function manyLines() {
    const lines = [
"#,
    );

    for i in 0..1000 {
        many_lines_content.push_str(&format!("        \"Line {}\",\n", i));
    }

    many_lines_content.push_str(
        r#"    ];
    return lines;
}
"#,
    );
    create_test_file(root_dir, "src/many_lines.js", &many_lines_content);

    // File with nested structures
    let nested_content = r#"
// This file contains deeply nested structures
function nestedStructure() {
    const nested = {
        level1: {
            level2: {
                level3: {
                    level4: {
                        level5: {
                            level6: {
                                level7: {
                                    level8: {
                                        level9: {
                                            level10: {
                                                value: "deeply nested"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    return nested;
}
"#;
    create_test_file(root_dir, "src/nested_structure.js", nested_content);

    // File with binary data representation
    let binary_content = r#"
// This file contains binary data representation
function binaryData() {
    const buffer = new Uint8Array([
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
        0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
        0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
        0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F,
        0x7F, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86,
        0x87, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E,
        0xFF
    ]);
    return buffer;
}
"#;
    create_test_file(root_dir, "src/binary_data.js", binary_content);

    // File with zero-width characters and other invisible characters
    let invisible_chars_content = r#"
// This file contains zero-width and invisible characters
function invisibleChars() {
    // Zero-width space (U+200B): "\u{200B}"
    // Zero-width non-joiner (U+200C): "‌"
    // Zero-width joiner (U+200D): "‍"
    // Left-to-right mark (U+200E): "‎"
    // Right-to-left mark (U+200F): "‏"
    
    const text = "This\u{200B} has‌ invisible‍ characters‎ in‏ it";
    return text;
}
"#;
    create_test_file(root_dir, "src/invisible_chars.js", invisible_chars_content);

    // File with mixed encodings
    let mixed_encodings_content = r#"
// This file contains characters from different encodings
function mixedEncodings() {
    // Latin: "Hello, World!"
    // Cyrillic: "Привет, мир!"
    // Greek: "Γειά σου, κόσμε!"
    // Arabic: "مرحبا بالعالم!"
    // Hebrew: "שלום, עולם!"
    // Chinese: "你好，世界！"
    // Japanese: "こんにちは、世界！"
    // Korean: "안녕하세요, 세계!"
    // Thai: "สวัสดี, โลก!"
    
    const greetings = [
        "Hello, World!",
        "Привет, мир!",
        "Γειά σου, κόσμε!",
        "مرحبا بالعالم!",
        "שלום, עולם!",
        "你好，世界！",
        "こんにちは、世界！",
        "안녕하세요, 세계!",
        "สวัสดี, โลก!"
    ];
    
    return greetings;
}
"#;
    create_test_file(root_dir, "src/mixed_encodings.js", mixed_encodings_content);
}

#[test]
fn test_json_output_with_long_lines() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_edge_case_test_files(&temp_dir);

    // Run the CLI with JSON output format, searching for "longLine"
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "longLine", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
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
    let json_result: Value =
        serde_json::from_str(json_str).expect("Failed to parse JSON output with long lines");

    // Find the result with long lines
    let results = json_result.get("results").unwrap().as_array().unwrap();
    let long_line_result = results.iter().find(|r| {
        r.get("file")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("long_line.js")
    });

    assert!(
        long_line_result.is_some(),
        "Should find the long_line.js file"
    );

    // Verify that the long line is properly handled in the JSON
    let code = long_line_result
        .unwrap()
        .get("code")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(
        code.contains("longLine"),
        "Should contain the function name"
    );
    assert!(
        code.contains("longString"),
        "Should contain the variable name"
    );

    // Check that the JSON is valid with the long line
    let code_json = serde_json::json!({ "code": code });
    let code_str = serde_json::to_string(&code_json).expect("Failed to serialize code to JSON");

    // Deserialize to verify it's valid JSON
    let _: Value = serde_json::from_str(&code_str).expect("Failed to parse serialized code JSON");
}

#[test]
fn test_json_output_with_many_lines() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_edge_case_test_files(&temp_dir);

    // Run the CLI with JSON output format, searching for "Line"
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "Line", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
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
    let json_result: Value =
        serde_json::from_str(json_str).expect("Failed to parse JSON output with many lines");

    // Check that we have results
    let results = json_result.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "Should have at least one result");

    // Just verify that the JSON is valid and can be parsed
    let summary = json_result.get("summary").unwrap();
    assert!(
        summary.get("count").is_some(),
        "Should have a count field in summary"
    );
    assert!(
        summary.get("total_bytes").is_some(),
        "Should have a total_bytes field in summary"
    );
    assert!(
        summary.get("total_tokens").is_some(),
        "Should have a total_tokens field in summary"
    );

    // Check that the JSON itself is valid
    let json_str = serde_json::to_string(&json_result).expect("Failed to serialize JSON result");
    let _: Value = serde_json::from_str(&json_str).expect("Failed to parse serialized JSON");
}

#[test]
fn test_json_output_with_nested_structures() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_edge_case_test_files(&temp_dir);

    // Run the CLI with JSON output format, searching for "nestedStructure"
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "nestedStructure", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
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
    let json_result: Value =
        serde_json::from_str(json_str).expect("Failed to parse JSON output with nested structures");

    // Find the result with nested structures
    let results = json_result.get("results").unwrap().as_array().unwrap();
    let nested_result = results.iter().find(|r| {
        r.get("file")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("nested_structure.js")
    });

    assert!(
        nested_result.is_some(),
        "Should find the nested_structure.js file"
    );

    // Verify that the nested structures are properly handled in the JSON
    let code = nested_result
        .unwrap()
        .get("code")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(
        code.contains("nestedStructure"),
        "Should contain the function name"
    );
    assert!(code.contains("level1"), "Should contain level1");
    assert!(code.contains("level10"), "Should contain level10");
    assert!(
        code.contains("deeply nested"),
        "Should contain the nested value"
    );

    // Check that the JSON is valid with nested structures
    let code_json = serde_json::json!({ "code": code });
    let code_str = serde_json::to_string(&code_json).expect("Failed to serialize code to JSON");

    // Deserialize to verify it's valid JSON
    let _: Value = serde_json::from_str(&code_str).expect("Failed to parse serialized code JSON");
}

#[test]
fn test_json_output_with_mixed_encodings() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_edge_case_test_files(&temp_dir);

    // Run the CLI with JSON output format, searching for "mixedEncodings"
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "mixedEncodings", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
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
    let json_result: Value =
        serde_json::from_str(json_str).expect("Failed to parse JSON output with mixed encodings");

    // Find the result with mixed encodings
    let results = json_result.get("results").unwrap().as_array().unwrap();
    let mixed_encodings_result = results.iter().find(|r| {
        r.get("file")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("mixed_encodings.js")
    });

    assert!(
        mixed_encodings_result.is_some(),
        "Should find the mixed_encodings.js file"
    );

    // Verify that the mixed encodings are properly handled in the JSON
    let code = mixed_encodings_result
        .unwrap()
        .get("code")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(
        code.contains("mixedEncodings"),
        "Should contain the function name"
    );
    assert!(code.contains("Hello, World!"), "Should contain Latin text");
    assert!(
        code.contains("Привет, мир!"),
        "Should contain Cyrillic text"
    );
    assert!(code.contains("你好，世界！"), "Should contain Chinese text");

    // Check that the JSON is valid with mixed encodings
    let code_json = serde_json::json!({ "code": code });
    let code_str = serde_json::to_string(&code_json).expect("Failed to serialize code to JSON");

    // Deserialize to verify it's valid JSON
    let _: Value = serde_json::from_str(&code_str).expect("Failed to parse serialized code JSON");
}

#[test]
fn test_xml_output_with_long_lines() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_edge_case_test_files(&temp_dir);

    // Run the CLI with XML output format, searching for "longLine"
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "longLine", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the XML part from the output
    let xml_str = extract_xml_from_output(&stdout);

    // Parse the XML output
    let doc = Document::parse(xml_str).expect("Failed to parse XML output with long lines");
    let root = doc.root_element();

    // Find the result with long lines
    let results: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    let long_line_result = results.iter().find(|&r| {
        if let Some(file) = r
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "file")
        {
            if let Some(text) = file.text() {
                return text.contains("long_line.js");
            }
        }
        false
    });

    assert!(
        long_line_result.is_some(),
        "Should find the long_line.js file"
    );

    // Verify that the long line is properly handled in the XML
    if let Some(result) = long_line_result {
        if let Some(code) = result
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code")
        {
            if let Some(text) = code.text() {
                assert!(
                    text.contains("longLine"),
                    "Should contain the function name"
                );
                assert!(
                    text.contains("longString"),
                    "Should contain the variable name"
                );
            } else {
                panic!("Code element should have text content");
            }
        } else {
            panic!("Result should have a code element");
        }
    }
}

#[test]
fn test_xml_output_with_nested_structures() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_edge_case_test_files(&temp_dir);

    // Run the CLI with XML output format, searching for "nestedStructure"
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "nestedStructure", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the XML part from the output
    let xml_str = extract_xml_from_output(&stdout);

    // Parse the XML output
    let doc = Document::parse(xml_str).expect("Failed to parse XML output with nested structures");
    let root = doc.root_element();

    // Find the result with nested structures
    let results: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    let nested_result = results.iter().find(|&r| {
        if let Some(file) = r
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "file")
        {
            if let Some(text) = file.text() {
                return text.contains("nested_structure.js");
            }
        }
        false
    });

    assert!(
        nested_result.is_some(),
        "Should find the nested_structure.js file"
    );

    // Verify that the nested structures are properly handled in the XML
    if let Some(result) = nested_result {
        if let Some(code) = result
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code")
        {
            if let Some(text) = code.text() {
                assert!(
                    text.contains("nestedStructure"),
                    "Should contain the function name"
                );
                assert!(text.contains("level1"), "Should contain level1");
                assert!(text.contains("level10"), "Should contain level10");
                assert!(
                    text.contains("deeply nested"),
                    "Should contain the nested value"
                );
            } else {
                panic!("Code element should have text content");
            }
        } else {
            panic!("Result should have a code element");
        }
    }
}

#[test]
fn test_xml_output_with_mixed_encodings() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_edge_case_test_files(&temp_dir);

    // Run the CLI with XML output format, searching for "mixedEncodings"
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "mixedEncodings", // Pattern to search for
            temp_dir.path().to_str().unwrap(),
            "--format",
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the XML part from the output
    let xml_str = extract_xml_from_output(&stdout);

    // Parse the XML output
    let doc = Document::parse(xml_str).expect("Failed to parse XML output with mixed encodings");
    let root = doc.root_element();

    // Find the result with mixed encodings
    let results: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    let mixed_encodings_result = results.iter().find(|&r| {
        if let Some(file) = r
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "file")
        {
            if let Some(text) = file.text() {
                return text.contains("mixed_encodings.js");
            }
        }
        false
    });

    assert!(
        mixed_encodings_result.is_some(),
        "Should find the mixed_encodings.js file"
    );

    // Verify that the mixed encodings are properly handled in the XML
    if let Some(result) = mixed_encodings_result {
        if let Some(code) = result
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code")
        {
            if let Some(text) = code.text() {
                assert!(
                    text.contains("mixedEncodings"),
                    "Should contain the function name"
                );
                assert!(text.contains("Hello, World!"), "Should contain Latin text");
                assert!(
                    text.contains("Привет, мир!"),
                    "Should contain Cyrillic text"
                );
                assert!(text.contains("你好，世界！"), "Should contain Chinese text");
            } else {
                panic!("Code element should have text content");
            }
        } else {
            panic!("Result should have a code element");
        }
    }
}
