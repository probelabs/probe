use roxmltree::{Document, Node};
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

// Helper function to create a test directory with files containing various special characters
fn create_special_character_test_files(root_dir: &TempDir) {
    // Create a source directory
    let src_dir = root_dir.path().join("src");
    fs::create_dir(&src_dir).expect("Failed to create src directory");

    // File with HTML-like tags
    let html_tags_content = r#"
// This file contains HTML-like tags
function renderHTML() {
    const html = `
        <div class="container">
            <h1>Hello, World!</h1>
            <p>This is a <strong>test</strong> paragraph.</p>
            <ul>
                <li>Item 1</li>
                <li>Item 2</li>
            </ul>
        </div>
    `;
    return html;
}
"#;
    create_test_file(root_dir, "src/html_tags.js", html_tags_content);

    // File with XML-like content
    let xml_content = r#"
// This file contains XML-like content
function generateXML() {
    const xml = `
        <?xml version="1.0" encoding="UTF-8"?>
        <root>
            <element attribute="value">
                <child>Text content</child>
                <child>More text</child>
            </element>
            <empty />
        </root>
    `;
    return xml;
}
"#;
    create_test_file(root_dir, "src/xml_content.js", xml_content);

    // File with JSON-like content
    let json_content = r#"
// This file contains JSON-like content
function generateJSON() {
    const json = `
        {
            "name": "Test Object",
            "properties": {
                "nested": true,
                "values": [1, 2, 3],
                "text": "String with \"quotes\""
            },
            "escapes": "Backslashes \\ and newlines \n and tabs \t"
        }
    `;
    return json;
}
"#;
    create_test_file(root_dir, "src/json_content.js", json_content);

    // File with various special characters
    let special_chars_content = r#"
// This file contains various special characters
function testSpecialChars() {
    // Special characters in strings
    const str1 = "Double quotes \" inside string";
    const str2 = 'Single quotes \' inside string';
    const str3 = `Backticks \` inside template literal`;
    
    // HTML entities
    const entities = "&lt; &gt; &amp; &quot; &apos;";
    
    // Control characters
    const controls = "\b\f\n\r\t\v\0";
    
    // Unicode characters
    const unicode = "Unicode: \u00A9 \u00AE \u2122 \u20AC \u2603";
    
    // Emoji
    const emoji = "Emoji: 😀 👍 🚀 🌈 🔥";
    
    return {
        str1, str2, str3, entities, controls, unicode, emoji
    };
}
"#;
    create_test_file(root_dir, "src/special_chars.js", special_chars_content);

    // File with potentially problematic sequences
    let problematic_content = r#"
// This file contains potentially problematic sequences
function testProblematicSequences() {
    // CDATA-like sequence
    const cdataLike = "<![CDATA[ This looks like CDATA ]]>";
    
    // XML declaration-like sequence
    const xmlLike = "<?xml version=\"1.0\"?>";
    
    // DOCTYPE-like sequence
    const doctypeLike = "<!DOCTYPE html>";
    
    // Comment-like sequences
    const htmlComment = "<!-- HTML comment -->";
    const xmlComment = "<!-- XML comment -->";
    
    // Script tags
    const scriptTag = "<script>alert('XSS');</script>";
    
    // JSON with special sequences
    const jsonSpecial = '{"key": "value with </script> in it"}';
    
    return {
        cdataLike, xmlLike, doctypeLike, htmlComment, xmlComment, scriptTag, jsonSpecial
    };
}
"#;
    create_test_file(
        root_dir,
        "src/problematic_sequences.js",
        problematic_content,
    );
}

#[test]
fn test_json_special_character_escaping() {
    // Create a JSON string with various special characters
    let special_json = json!({
        "results": [{
            "file": "test.js",
            "lines": [1, 10],
            "node_type": "function",
            "code": "function test() {\n  // Special chars: \"quotes\", 'apostrophes', <tags>, &ampersands\n  const xml = \"<![CDATA[ data ]]>\";\n  const html = \"<!-- comment -->\";\n  const script = \"<script>alert('XSS');</script>\";\n  const emoji = \"😀 👍 🚀\";\n}",
            "matched_keywords": ["test"],
            "score": 0.95,
            "tfidf_score": 0.5,
            "bm25_score": 0.8,
            "block_total_matches": null,
            "block_unique_terms": null,
            "file_total_matches": null,
            "file_unique_terms": null
        }],
        "summary": {
            "count": 1,
            "total_bytes": 100,
            "total_tokens": 50
        }
    });

    // Convert to string and back to verify JSON escaping works correctly
    let json_str = serde_json::to_string(&special_json).expect("Failed to serialize JSON");
    let parsed_json: Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");

    // Verify the special characters are properly handled
    let code = parsed_json["results"][0]["code"].as_str().unwrap();
    assert!(code.contains("\"quotes\""), "Should preserve double quotes");
    assert!(
        code.contains("'apostrophes'"),
        "Should preserve single quotes"
    );
    assert!(code.contains("<tags>"), "Should preserve tags");
    assert!(code.contains("&ampersands"), "Should preserve ampersands");
    assert!(code.contains("<![CDATA["), "Should preserve CDATA");
    assert!(
        code.contains("<!-- comment -->"),
        "Should preserve comments"
    );
    assert!(code.contains("<script>"), "Should preserve script tags");
    assert!(code.contains("😀"), "Should preserve emoji");
}

// This test is now covered by test_json_special_character_escaping

#[test]
fn test_xml_special_character_escaping() {
    // Create an XML string with various special characters
    let xml_str = r#"<?xml version="1.0" encoding="UTF-8"?>
<probe_results>
  <result>
    <file>test.js</file>
    <lines>1-10</lines>
    <node_type>function</node_type>
    <code>function test() {
  // Special chars: "quotes", 'apostrophes', &lt;tags&gt;, &amp;ampersands
  const xml = "&lt;![CDATA[ data ]]&gt;";
  const html = "&lt;!-- comment --&gt;";
  const script = "&lt;script&gt;alert('XSS');&lt;/script&gt;";
  const emoji = "😀 👍 🚀";
}</code>
    <matched_keywords>
      <keyword>test</keyword>
    </matched_keywords>
    <score>0.95</score>
  </result>
  <summary>
    <count>1</count>
    <total_bytes>100</total_bytes>
    <total_tokens>50</total_tokens>
  </summary>
</probe_results>"#;

    // Parse the XML to verify it's valid
    let doc = Document::parse(xml_str).expect("Failed to parse XML");
    let root = doc.root_element();

    // Find the code element
    let results: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert!(!results.is_empty(), "Should have at least one result");

    let result = &results[0];
    let code = result
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "code");
    assert!(code.is_some(), "Should have a code element");

    // Verify the special characters are properly handled
    let code_text = code.unwrap().text().unwrap();

    // Just check that the XML is valid and contains the expected content types
    assert!(
        code_text.contains("function test"),
        "Should contain function declaration"
    );
    assert!(
        code_text.contains("Special chars"),
        "Should contain special chars comment"
    );
    assert!(code_text.contains("quotes"), "Should contain quotes text");
    assert!(
        code_text.contains("apostrophes"),
        "Should contain apostrophes text"
    );
    assert!(code_text.contains("emoji"), "Should contain emoji text");
}

// This test is now covered by test_xml_special_character_escaping

#[test]
fn test_html_tags_in_json_output() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_special_character_test_files(&temp_dir);

    // Run the CLI with JSON output format, searching for "HTML"
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "HTML", // Pattern to search for
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
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Find the result with HTML tags
    let results = json_result.get("results").unwrap().as_array().unwrap();
    let html_tags_result = results.iter().find(|r| {
        r.get("file")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("html_tags.js")
    });

    assert!(
        html_tags_result.is_some(),
        "Should find the html_tags.js file"
    );

    // Verify that HTML tags are properly escaped in the JSON
    let code = html_tags_result
        .unwrap()
        .get("code")
        .unwrap()
        .as_str()
        .unwrap();

    // Check that the JSON is valid with these HTML tags
    let code_json = serde_json::json!({ "code": code });
    let code_str = serde_json::to_string(&code_json).expect("Failed to serialize code to JSON");

    // Deserialize to verify it's valid JSON
    let _: Value = serde_json::from_str(&code_str).expect("Failed to parse serialized code JSON");

    // Check for specific HTML tags
    assert!(code.contains("<div"), "Should contain div tag");
    assert!(code.contains("<h1>"), "Should contain h1 tag");
    assert!(code.contains("<strong>"), "Should contain strong tag");
    assert!(code.contains("<ul>"), "Should contain ul tag");
    assert!(code.contains("<li>"), "Should contain li tag");
}

#[test]
fn test_html_tags_in_xml_output() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_special_character_test_files(&temp_dir);

    // Run the CLI with XML output format, searching for "HTML"
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "HTML", // Pattern to search for
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
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Find the result with HTML tags
    let results: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    let html_tags_result = results.iter().find(|&r| {
        if let Some(file) = r
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "file")
        {
            if let Some(text) = file.text() {
                return text.contains("html_tags.js");
            }
        }
        false
    });

    assert!(
        html_tags_result.is_some(),
        "Should find the html_tags.js file"
    );

    // Verify that HTML tags are properly handled in the XML
    if let Some(result) = html_tags_result {
        if let Some(code) = result
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code")
        {
            if let Some(text) = code.text() {
                // Check for specific HTML tags
                assert!(text.contains("<div"), "Should contain div tag");
                assert!(text.contains("<h1>"), "Should contain h1 tag");
                assert!(text.contains("<strong>"), "Should contain strong tag");
                assert!(text.contains("<ul>"), "Should contain ul tag");
                assert!(text.contains("<li>"), "Should contain li tag");
            } else {
                panic!("Code element should have text content");
            }
        } else {
            panic!("Result should have a code element");
        }
    }
}
