use std::process::Command;

#[test]
fn test_outline_format_preserves_keywords_in_truncated_arrays() {
    // Run probe search with outline format on a file known to have large arrays with keywords
    let output = Command::new("./target/release/probe")
        .args([
            "search",
            "stemming",
            "./src/search/tokenization.rs",
            "--format",
            "outline",
        ])
        .output()
        .expect("Failed to execute probe command");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in output");

    // The output should contain the keyword "stemming" even in truncated arrays
    assert!(
        stdout.contains("stemming"),
        "Output should contain 'stemming' keyword even in truncated arrays"
    );

    // The output should show truncation with "..."
    assert!(
        stdout.contains("..."),
        "Output should show truncation with ellipsis"
    );

    // The output should have reasonable length (not thousands of lines like before)
    let line_count = stdout.lines().count();
    assert!(
        line_count < 200,
        "Output should be truncated to reasonable size, got {} lines",
        line_count
    );
}

#[test]
fn test_outline_format_highlights_keywords_in_comments() {
    // Test that keywords are highlighted in function signatures and comments
    let output = Command::new("./target/release/probe")
        .args([
            "search",
            "stem",
            "./src/search/tokenization.rs",
            "--format",
            "outline",
        ])
        .output()
        .expect("Failed to execute probe command");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in output");

    // Should contain the function name with highlighting (though we can't test ANSI codes easily)
    assert!(
        stdout.contains("tokenize_and_stem"),
        "Should contain function name with stem keyword"
    );

    // Should contain comment lines with the keyword
    assert!(
        stdout.contains("apply stemming"),
        "Should contain comment with stemming keyword"
    );
}
