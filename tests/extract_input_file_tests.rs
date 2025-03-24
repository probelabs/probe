use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_extract_command_with_input_file() {
    // Create a temporary directory for our test files
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a test source file with simple content
    let source_file_path = temp_dir.path().join("test_source.rs");
    let source_content = r#"
fn main() {
    println!("Hello, world!");
}
"#;
    fs::write(&source_file_path, source_content).unwrap();

    // Create an input file that contains the path to the source file
    let input_file_path = temp_dir.path().join("input.txt");
    fs::write(
        &input_file_path,
        source_file_path.to_string_lossy().as_bytes(),
    )
    .unwrap();

    // Get the project root directory (where Cargo.toml is)
    let project_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Run the extract command with --help to verify the new option exists
    let help_output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            project_dir.join("Cargo.toml").to_string_lossy().as_ref(),
            "--",
            "extract",
            "--help",
        ])
        .output()
        .expect("Failed to execute help command");

    // Check that the help output includes the new option
    let help_text = String::from_utf8_lossy(&help_output.stdout);
    assert!(
        help_text.contains("-F, --input-file"),
        "Help text should include the new --input-file option"
    );
}

#[test]
fn test_extract_command_with_nonexistent_input_file() {
    // Get the project root directory (where Cargo.toml is)
    let project_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Run the extract command with a nonexistent input file
    let output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            project_dir.join("Cargo.toml").to_string_lossy().as_ref(),
            "--",
            "extract",
            "--input-file",
            "nonexistent_file.txt",
        ])
        .output()
        .expect("Failed to execute command");

    // The command should fail
    assert!(
        !output.status.success(),
        "Command should fail with nonexistent file"
    );

    // Get the error output as a string
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The error should mention the nonexistent file
    assert!(
        stderr.contains("nonexistent_file.txt"),
        "Error should mention the nonexistent file"
    );
}
