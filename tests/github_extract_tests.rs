use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Integration tests specifically for the GitHub files extract bug fix.
///
/// This tests the issue where `probe extract` was incorrectly ignoring files
/// in .github directories due to substring matching on ".git".

#[test]
fn test_extract_github_workflow_file() {
    let temp_dir = TempDir::new().unwrap();

    // Create .github directory structure
    let github_dir = temp_dir.path().join(".github");
    let workflows_dir = github_dir.join("workflows");
    fs::create_dir_all(&workflows_dir).unwrap();

    // Create a GitHub workflow file
    let workflow_file = workflows_dir.join("ci.yml");
    let workflow_content = r#"name: CI Pipeline
on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v2
    - name: Run tests
      run: |
        echo "Running tests"
        cargo test --all
        
    - name: Check formatting
      run: cargo fmt --check
      
  build:
    needs: test
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v2
    - name: Build
      run: cargo build --release
"#;
    fs::write(&workflow_file, workflow_content).unwrap();

    // Test extracting the entire file
    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args(["extract", &workflow_file.to_string_lossy()])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute probe extract");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "probe extract should succeed for .github files. stderr: {stderr}"
    );
    assert!(
        stdout.contains("CI Pipeline"),
        "Should extract content from GitHub workflow file. stdout: {stdout}"
    );
    assert!(
        !stdout.contains("No results found"),
        "Should not return 'No results found' for .github files"
    );
}

#[test]
fn test_extract_github_workflow_with_line_range() {
    let temp_dir = TempDir::new().unwrap();

    // Create .github directory structure
    let github_dir = temp_dir.path().join(".github");
    let workflows_dir = github_dir.join("workflows");
    fs::create_dir_all(&workflows_dir).unwrap();

    // Create a GitHub workflow file
    let workflow_file = workflows_dir.join("test.yml");
    let workflow_content = r#"name: Test Workflow
on: [push]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - name: Checkout
      uses: actions/checkout@v2
    - name: Test
      run: echo "testing"
"#;
    fs::write(&workflow_file, workflow_content).unwrap();

    // Test extracting specific lines (lines 3-7)
    let file_with_range = format!("{}:3-7", workflow_file.to_string_lossy());
    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args(["extract", &file_with_range])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute probe extract with line range");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "probe extract should succeed for .github files with line ranges. stderr: {stderr}"
    );
    assert!(
        stdout.contains("jobs:"),
        "Should extract the specified line range. stdout: {stdout}"
    );
    assert!(
        stdout.contains("Lines: 3-7"),
        "Should show the correct line range in output. stdout: {stdout}"
    );
}

#[test]
fn test_git_directory_still_ignored() {
    let temp_dir = TempDir::new().unwrap();

    // Create a .git directory (should be ignored)
    let git_dir = temp_dir.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    let git_config = git_dir.join("config");
    fs::write(&git_config, "[core]\n    repositoryformatversion = 0\n").unwrap();

    // Test that .git files are still ignored
    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args(["extract", &git_config.to_string_lossy()])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute probe extract");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // .git files should still be ignored (should return "No results found")
    assert!(
        stdout.contains("No results found"),
        ".git directory files should still be ignored. stdout: {stdout}"
    );
}

#[test]
fn test_gitignore_file_not_ignored() {
    let temp_dir = TempDir::new().unwrap();

    // Create a .gitignore file (should NOT be ignored)
    let gitignore_file = temp_dir.path().join(".gitignore");
    fs::write(&gitignore_file, "*.log\ntmp/\n*.tmp\n").unwrap();

    // Test that .gitignore files are NOT ignored
    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args(["extract", &gitignore_file.to_string_lossy()])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute probe extract");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "probe extract should succeed for .gitignore files. stderr: {stderr}"
    );
    assert!(
        stdout.contains("*.log"),
        "Should extract content from .gitignore file. stdout: {stdout}"
    );
    assert!(
        !stdout.contains("No results found"),
        "Should not return 'No results found' for .gitignore files"
    );
}

#[test]
fn test_github_issue_template() {
    let temp_dir = TempDir::new().unwrap();

    // Create .github directory structure
    let github_dir = temp_dir.path().join(".github");
    fs::create_dir_all(&github_dir).unwrap();

    // Create a GitHub issue template
    let issue_template = github_dir.join("issue_template.md");
    let template_content = r#"---
name: Bug report
about: Create a report to help us improve
---

**Describe the bug**
A clear and concise description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Go to '...'
2. Click on '....'
3. Scroll down to '....'
4. See error

**Expected behavior**
A clear and concise description of what you expected to happen.
"#;
    fs::write(&issue_template, template_content).unwrap();

    // Test extracting the issue template
    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args(["extract", &issue_template.to_string_lossy()])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute probe extract");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "probe extract should succeed for .github issue templates. stderr: {stderr}"
    );
    assert!(
        stdout.contains("Bug report"),
        "Should extract content from GitHub issue template. stdout: {stdout}"
    );
    assert!(
        stdout.contains("Describe the bug"),
        "Should extract the issue template content. stdout: {stdout}"
    );
}
