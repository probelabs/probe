use roxmltree::{Document, Node};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;
// We'll use roxmltree for XML validation since xml-schema doesn't have the expected validator module

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

// Helper function to validate XML structure
fn validate_xml_structure(xml_str: &str) -> Result<(), String> {
    // Parse the XML to verify its structure
    let doc = Document::parse(xml_str).map_err(|e| format!("Failed to parse XML: {e}"))?;

    let root = doc.root_element();

    // Check root element name
    if root.tag_name().name() != "probe_results" {
        return Err(format!(
            "Root element should be 'probe_results', found '{}'",
            root.tag_name().name()
        ));
    }

    // Check for summary element
    let summary = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "summary");
    if summary.is_none() {
        return Err("Missing required 'summary' element".to_string());
    }

    // Check summary structure
    if let Some(summary) = summary {
        let count = summary
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "count");
        if count.is_none() {
            return Err("Missing required 'count' element in summary".to_string());
        }

        let total_bytes = summary
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "total_bytes");
        if total_bytes.is_none() {
            return Err("Missing required 'total_bytes' element in summary".to_string());
        }

        let total_tokens = summary
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "total_tokens");
        if total_tokens.is_none() {
            return Err("Missing required 'total_tokens' element in summary".to_string());
        }
    }

    // Check result elements (if any)
    let results: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    for result in results {
        // Check required elements in each result
        let file = result
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "file");
        if file.is_none() {
            return Err("Missing required 'file' element in result".to_string());
        }

        let lines = result
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "lines");
        if lines.is_none() {
            return Err("Missing required 'lines' element in result".to_string());
        }

        let node_type = result
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "node_type");
        if node_type.is_none() {
            return Err("Missing required 'node_type' element in result".to_string());
        }

        let code = result
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code");
        if code.is_none() {
            return Err("Missing required 'code' element in result".to_string());
        }
    }

    Ok(())
}

#[test]
fn test_xml_schema_validation_basic() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with XML output format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "search", // Pattern to search for
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

    // Validate the XML structure
    let validation_result = validate_xml_structure(xml_str);
    assert!(
        validation_result.is_ok(),
        "XML output does not conform to expected structure: {:?}",
        validation_result.err()
    );

    // Parse the XML to verify its structure
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Validate the structure of the XML output
    assert_eq!(
        root.tag_name().name(),
        "probe_results",
        "Root element should be 'probe_results'"
    );

    // Validate that there are result elements
    let results: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert!(
        !results.is_empty(),
        "Should have at least one result element"
    );
}

#[test]
fn test_xml_schema_validation_special_characters() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with XML output format, searching for special characters
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "special", // Pattern to search for
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

    // Validate the XML structure
    let validation_result = validate_xml_structure(xml_str);
    assert!(
        validation_result.is_ok(),
        "XML output with special characters does not conform to expected structure: {:?}",
        validation_result.err()
    );

    // Parse the XML to verify its structure
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Find the result with special characters
    let results: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    let special_chars_result = results.iter().find(|&r| {
        if let Some(file) = r
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "file")
        {
            if let Some(text) = file.text() {
                return text.contains("special_chars.js");
            }
        }
        false
    });

    assert!(
        special_chars_result.is_some(),
        "Should find the special_chars.js file"
    );

    // Verify that special characters are properly escaped in the XML
    if let Some(result) = special_chars_result {
        if let Some(code) = result
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code")
        {
            if let Some(text) = code.text() {
                // The special characters should be properly escaped in XML
                // Since we're using CDATA, the characters should be preserved as-is
                assert!(
                    text.contains("\"quotes\""),
                    "Double quotes should be preserved in CDATA"
                );
                assert!(
                    text.contains("'apostrophes'"),
                    "Apostrophes should be preserved in CDATA"
                );
                assert!(text.contains("<tags>"), "Tags should be preserved in CDATA");
                assert!(
                    text.contains("&ampersands"),
                    "Ampersands should be preserved in CDATA"
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
fn test_xml_schema_validation_empty_results() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with XML output format, searching for a term that doesn't exist
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "nonexistentterm", // Term that doesn't exist in any file
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

    // Validate the XML structure
    let validation_result = validate_xml_structure(xml_str);
    assert!(
        validation_result.is_ok(),
        "XML output with empty results does not conform to expected structure: {:?}",
        validation_result.err()
    );

    // Parse the XML to verify its structure
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Validate that there are no result elements
    let results: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert!(results.is_empty(), "Should have no result elements");

    // Validate the summary element
    let summary = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "summary");
    assert!(summary.is_some(), "Should have a summary element");

    if let Some(summary) = summary {
        // Validate the summary contains count, total_bytes, and total_tokens with zero values
        if let Some(count) = summary
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "count")
        {
            let count_value = count.text().unwrap_or("0").parse::<usize>().unwrap_or(0);
            assert_eq!(count_value, 0, "Count should be 0");
        } else {
            panic!("Summary should have a count element");
        }
    }
}

#[test]
fn test_xml_schema_validation_multiple_terms() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with XML output format, searching for multiple terms
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "term process", // Multiple terms to search for
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

    // Validate the XML structure
    let validation_result = validate_xml_structure(xml_str);
    assert!(
        validation_result.is_ok(),
        "XML output with multiple terms does not conform to expected structure: {:?}",
        validation_result.err()
    );

    // Parse the XML to verify its structure
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Find the result with multiple terms
    let results: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    let multi_term_result = results.iter().find(|&r| {
        if let Some(file) = r
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "file")
        {
            if let Some(text) = file.text() {
                return text.contains("multi_term.js");
            }
        }
        false
    });

    assert!(
        multi_term_result.is_some(),
        "Should find the multi_term.js file"
    );

    // Check if matched_keywords element contains both search terms
    if let Some(result) = multi_term_result {
        if let Some(matched_keywords) = result
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "matched_keywords")
        {
            let keywords: Vec<Node> = matched_keywords
                .children()
                .filter(|n| n.is_element() && n.tag_name().name() == "keyword")
                .collect();

            // Check if both terms are present
            let has_term = keywords.iter().any(|k| {
                if let Some(text) = k.text() {
                    text.contains("term")
                } else {
                    false
                }
            });

            let has_process = keywords.iter().any(|k| {
                if let Some(text) = k.text() {
                    text.contains("process")
                } else {
                    false
                }
            });

            assert!(has_term, "matched_keywords should contain 'term'");
            assert!(has_process, "matched_keywords should contain 'process'");
        }
    }
}
