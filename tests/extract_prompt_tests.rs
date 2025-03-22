use std::fs;
use std::process::Command;

#[test]
fn test_extract_with_prompt_and_instructions() {
    // Create a temporary file for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.rs");
    let content = r#"
fn test() {
    println!("Hello, world!");
}
"#;
    fs::write(&file_path, content).unwrap();

    // Get the project root directory (where Cargo.toml is)
    let project_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Run the extract command with prompt and instructions
    let output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            project_dir.join("Cargo.toml").to_string_lossy().as_ref(),
            "--",
            "extract",
            file_path.to_string_lossy().as_ref(),
            "--format",
            "json",
            "--prompt",
            "engineer",
            "--instructions",
            "Explain what this function does",
            "--allow-tests", // Add this flag to ensure test files are included
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success(), "Command failed to execute");

    // Get the output as a string
    let stdout = String::from_utf8_lossy(&output.stdout);

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

    // Extract and parse the JSON
    let json_str = extract_json_from_output(&stdout);
    let json_value: serde_json::Value =
        serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the structure of the JSON output
    assert!(json_value.is_object(), "JSON output should be an object");
    assert!(
        json_value.get("results").is_some(),
        "JSON output should have a 'results' field"
    );

    // Validate the results array
    let results = json_value.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "Results array should not be empty");

    // Check for system_prompt field at the top level
    assert!(
        json_value.get("system_prompt").is_some(),
        "JSON output should have a 'system_prompt' field"
    );
    let system_prompt = json_value.get("system_prompt").unwrap().as_str().unwrap();
    assert!(
        system_prompt.contains("senior software engineer"),
        "System prompt should contain the engineer template"
    );

    // Check for user_instructions field at the top level
    assert!(
        json_value.get("user_instructions").is_some(),
        "JSON output should have a 'user_instructions' field"
    );
    let user_instructions = json_value
        .get("user_instructions")
        .unwrap()
        .as_str()
        .unwrap();
    assert_eq!(
        user_instructions, "Explain what this function does",
        "User instructions should match the input"
    );
    assert_eq!(
        user_instructions, "Explain what this function does",
        "User instructions should match the input"
    );
}
