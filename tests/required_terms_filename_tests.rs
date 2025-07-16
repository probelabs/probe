use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_required_terms_with_filename_matching() {
    // Create a temporary directory for our test files
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Create test files
    let file1_path = temp_path.join("file1.rs");
    let file2_path = temp_path.join("file2.rs");
    let file3_path = temp_path.join("file3.rs");
    let load_go_path = temp_path.join("load.go");

    // File 1: Contains "api" and "load" but not "process"
    let file1_content = r#"
    fn main() {
        let api = get_api();
        api.load();
    }
    "#;

    // File 2: Does not contain "api", but contains "load" and "process"
    let file2_content = r#"
    fn main() {
        let data = load();
        process(data);
    }
    "#;

    // File 3: Contains all three terms
    let file3_content = r#"
    fn main() {
        let api = get_api();
        let data = api.load();
        process(data);
    }
    "#;

    // Load.go: Filename contains "load" and content contains "process"
    let load_go_content = r#"
    func main() {
        process(data);
    }
    "#;

    // Write the files
    File::create(&file1_path)
        .unwrap()
        .write_all(file1_content.as_bytes())
        .unwrap();
    File::create(&file2_path)
        .unwrap()
        .write_all(file2_content.as_bytes())
        .unwrap();
    File::create(&file3_path)
        .unwrap()
        .write_all(file3_content.as_bytes())
        .unwrap();
    File::create(&load_go_path)
        .unwrap()
        .write_all(load_go_content.as_bytes())
        .unwrap();

    // Run the search with the query "api +load +process"
    let output = std::process::Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "api +load +process",
            temp_path.to_str().unwrap(),
        ])
        .env("DEBUG", "1")
        .env("RUST_BACKTRACE", "1")
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT: {stdout}");
    println!("STDERR: {stderr}");

    // Extract the file names from the output
    let file_names: Vec<&str> = stdout
        .lines()
        .filter(|line| line.contains("File:"))
        .collect();

    // Check that file1.rs is NOT found (it doesn't have "process" which is required)
    assert!(
        !file_names.iter().any(|&name| name.contains("file1.rs")),
        "Should NOT find file1.rs which contains 'api' and 'load' but not 'process'"
    );

    // Check that file2.rs is found (it has "load" and "process" which are required, even though it doesn't have "api")
    assert!(
        file_names.iter().any(|&name| name.contains("file2.rs")),
        "Should find file2.rs which contains 'load' and 'process' but not 'api'"
    );

    // Check that file3.rs is found (it has all three terms)
    assert!(
        file_names.iter().any(|&name| name.contains("file3.rs")),
        "Should find file3.rs which contains all three terms"
    );

    // Check that load.go is found (it has "process" in content and "load" in filename)
    assert!(
        file_names.iter().any(|&name| name.contains("load.go")),
        "Should find load.go which has 'process' in content and 'load' in filename"
    );
}
