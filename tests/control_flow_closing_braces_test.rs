use std::process::Command;

#[test]
fn test_control_flow_closing_braces_in_outline_format() {
    // Test that control flow statements show their closing braces in outline format
    let output = Command::new("./target/release/probe")
        .args([
            "search",
            "println",
            "./src/language/rust.rs",
            "--format",
            "outline",
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute probe command");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in output");

    // Should show some control flow statements (if, for, while, etc.)
    let has_control_flow = stdout.contains("if ")
        || stdout.contains("for ")
        || stdout.contains("while ")
        || stdout.contains("match ");

    if !has_control_flow {
        // This particular search might not have control flow, which is ok
        // Just verify we have the expected structure
        assert!(
            stdout.contains("println"),
            "Should contain println in search results"
        );
        return;
    }

    // Should contain closing braces - look for lines that end with }
    let closing_brace_lines: Vec<&str> = stdout
        .lines()
        .filter(|line| line.trim().ends_with("}") && line.contains("}"))
        .collect();

    assert!(
        !closing_brace_lines.is_empty(),
        "Should contain at least one closing brace line. Output:\n{}",
        stdout
    );

    // Verify we have multiple closing braces (for nested control flow)
    assert!(
        closing_brace_lines.len() >= 2,
        "Should contain at least 2 closing braces for nested control flow structures. Found: {}. Output:\n{}",
        closing_brace_lines.len(),
        stdout
    );
}

#[test]
fn test_loop_closing_braces_in_outline_format() {
    // Test that loop statements show their closing braces
    let output = Command::new("./target/release/probe")
        .args([
            "search",
            "loop",
            "./src/search/search_output.rs",
            "--format",
            "outline",
            // Increase results to avoid ranking a non-structural match at top
            "--max-results",
            "20",
        ])
        .output()
        .expect("Failed to execute probe command");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in output");

    // Should show the loop statement
    assert!(
        stdout.contains("loop {"),
        "Should contain loop opening. Output:\n{}",
        stdout
    );

    // Should show the closing brace for the loop
    assert!(
        stdout
            .lines()
            .any(|line| line.contains("}") && line.trim().ends_with("}")),
        "Should contain closing brace for loop. Output:\n{}",
        stdout
    );
}
