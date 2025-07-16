use std::path::Path;
use std::process::Command;
use std::time::Instant;
use tempfile::TempDir;

/// Create a large test file that will take time to process
fn create_large_test_file(temp_dir: &Path) {
    let file_path = temp_dir.join("large_file.rs");

    // Create a very large file with many lines to ensure the search takes longer than the timeout
    let mut content = String::new();
    for i in 0..500000 {
        // Increased to 500,000 lines
        content.push_str(&format!("// Line {i} with searchable content\n"));
        content.push_str(&format!("fn function_{i}() {{\n"));
        content.push_str(&format!("    let search_term = {i};\n"));
        content.push_str("    println!(\"Found: {}\", search_term);\n");
        content.push_str("}\n\n");
    }

    // Write the file
    std::fs::write(file_path, content).expect("Failed to write large test file");
}

/// Test that the search operation times out after the specified timeout
/// This test runs the search command in a separate process since our timeout
/// implementation calls std::process::exit(1)
#[test]
fn test_search_timeout() {
    // Create a temporary directory
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a large test file
    create_large_test_file(temp_dir.path());

    // Measure the time it takes to run the search
    let start_time = Instant::now();

    // Run the search command with a very short timeout using cargo run
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("search")
        .arg("search_term") // This term appears in every function
        .arg("/")
        .arg("--timeout")
        .arg("1") // 1 second timeout should be short enough to trigger
        .output()
        .expect("Failed to execute command");

    // Check how long it took
    let elapsed = start_time.elapsed();

    // Print the output for debugging
    println!(
        "Command output: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    println!("Command error: {}", String::from_utf8_lossy(&output.stderr));

    // The command should have failed (non-zero exit code)
    assert_ne!(
        output.status.code().unwrap_or(-1),
        0,
        "Command should have failed with a non-zero exit code"
    );

    // The error output should mention timeout
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("timeout") || stderr.contains("timed out"),
        "Error output should mention timeout: {stderr}"
    );

    // The elapsed time should be close to the timeout value
    assert!(
        elapsed.as_secs() >= 1 && elapsed.as_secs() <= 5,
        "Search should have timed out after about 1 second, but took {elapsed:?}"
    );

    println!("✓ Search timed out correctly after {elapsed:?}");
}
