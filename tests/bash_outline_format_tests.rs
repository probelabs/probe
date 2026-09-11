use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_bash_outline_basic_functions() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("deploy.sh");

    let content = r#"#!/usr/bin/env bash
# Deployment script

set -euo pipefail

APP_NAME="myapp"

# Print a timestamped message
log_message() {
    echo "[$(date +%T)] $1"
}

build_release() {
    log_message "building"
    cargo build --release
}

restart_service() {
    sudo systemctl restart "$APP_NAME"
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "build_release",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Verify Bash functions are found in outline format
    assert!(
        output.contains("build_release"),
        "Missing build_release function - output: {}",
        output
    );

    // Should be in outline format with file delimiter
    assert!(
        output.contains("---\nFile:") || output.contains("File:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_bash_outline_control_blocks_and_comments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("process.bash");

    let content = r#"#!/bin/bash

# Process input files one by one
process_files() {
    for f in "$@"; do
        if [ -f "$f" ]; then
            echo "processing $f"
        else
            echo "missing $f" >&2
        fi
    done
}

cleanup() {
    while read -r tmp; do
        rm -f "$tmp"
    done < /tmp/list.txt
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();

    // Search for a term inside a control block should extract structured code
    let output = ctx.run_probe(&[
        "search",
        "processing",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("process_files"),
        "Should contain enclosing function - output: {}",
        output
    );

    // Comment extraction: the comment should be associated with the function
    let output = ctx.run_probe(&[
        "search",
        "Process input files",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("Process input files"),
        "Should find comment content - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_bash_outline_heredoc_and_variables() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("config.sh");

    let content = r#"#!/bin/sh
readonly CONFIG_PATH="/etc/myapp"

generate_config() {
    cat <<EOF
server {
    port 8080
    host example.com
}
EOF
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();

    // Heredoc content search
    let output = ctx.run_probe(&[
        "search",
        "example.com",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("generate_config") || output.contains("example.com"),
        "Should find heredoc content - output: {}",
        output
    );

    // Variable declaration search
    let output = ctx.run_probe(&[
        "search",
        "CONFIG_PATH",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("CONFIG_PATH"),
        "Should find variable declaration - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_bash_extract_function_by_line() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("utils.sh");

    let content = r#"#!/usr/bin/env bash
# Utility functions

greet_user() {
    local name="$1"
    echo "Hello, $name!"
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    // Extract the block at line 6 (the echo inside greet_user)
    let output = ctx.run_probe(&["extract", &format!("{}:6", test_file.to_str().unwrap())])?;

    assert!(
        output.contains("greet_user"),
        "Should extract the greet_user function - output: {}",
        output
    );
    assert!(
        output.contains("Hello, $name!"),
        "Should extract the function body - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_bash_test_function_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("build.sh");

    let content = r#"#!/usr/bin/env bash

build_all() {
    make all
}

test_build() {
    build_all
    assert_success
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();

    // With --allow-tests, test functions should be included
    let output = ctx.run_probe(&[
        "search",
        "assert_success",
        test_file.to_str().unwrap(),
        "--allow-tests",
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("test_build"),
        "Should find test_build function with --allow-tests - output: {}",
        output
    );

    Ok(())
}
