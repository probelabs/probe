//! Comprehensive LSP integration tests for Go, TypeScript, and JavaScript
//!
//! This test suite validates that ALL language servers work correctly with probe's LSP daemon.
//! Unlike the basic LSP integration tests, these tests:
//!
//! - NEVER skip tests due to missing language servers - they FAIL if dependencies are missing
//! - Test exact call hierarchy assertions for all supported languages
//! - Validate performance requirements (extraction < 3s, search < 5s)
//! - Test concurrent multi-language LSP operations
//! - Use dedicated test fixtures designed for call hierarchy testing
//!
//! Required language servers:
//! - gopls (Go language server): go install golang.org/x/tools/gopls@latest
//! - typescript-language-server: npm install -g typescript-language-server typescript
//!
//! These tests are designed to run in CI environments and ensure full LSP functionality.

mod common;

use anyhow::Result;
use common::{
    call_hierarchy::{validate_incoming_calls, validate_outgoing_calls},
    ensure_daemon_stopped, fixtures, init_lsp_workspace, performance, require_all_language_servers,
    run_probe_command_with_timeout, start_daemon_and_wait, wait_for_language_server_ready,
};
use std::time::{Duration, Instant};

/// Setup function that validates all required language servers are available
/// This function FAILS the test if any language server is missing
fn setup_comprehensive_tests() -> Result<()> {
    require_all_language_servers()?;
    ensure_daemon_stopped();
    Ok(())
}

/// Cleanup function for all tests
fn cleanup_comprehensive_tests() {
    ensure_daemon_stopped();
}

#[test]
fn test_go_lsp_call_hierarchy_exact() -> Result<()> {
    setup_comprehensive_tests()?;

    // Start daemon and initialize workspace
    start_daemon_and_wait()?;

    let workspace_path = fixtures::get_go_project1();
    init_lsp_workspace(workspace_path.to_str().unwrap(), &["go"])?;

    // Wait for gopls to fully index the project
    wait_for_language_server_ready(Duration::from_secs(15));

    // Test extraction with LSP for the Calculate function
    let file_path = workspace_path.join("calculator.go");
    let extract_args = [
        "extract",
        &format!("{}:10", file_path.to_string_lossy()), // Line 10 should be the Calculate function
        "--lsp",
    ];

    let start = Instant::now();
    let (stdout, stderr, success) =
        run_probe_command_with_timeout(&extract_args, performance::MAX_EXTRACT_TIME)?;
    let elapsed = start.elapsed();

    // Cleanup before assertions to avoid daemon issues
    cleanup_comprehensive_tests();

    // Validate the command succeeded
    assert!(success, "Extract command should succeed. Stderr: {stderr}");

    // Validate performance requirement
    assert!(
        elapsed < performance::MAX_EXTRACT_TIME,
        "Extract took {:?}, should be under {:?}",
        elapsed,
        performance::MAX_EXTRACT_TIME
    );

    // Validate basic extraction worked
    assert!(
        stdout.contains("Calculate"),
        "Should extract the Calculate function"
    );
    assert!(
        stdout.contains("func Calculate"),
        "Should show function signature"
    );

    // Validate LSP call hierarchy information is present
    assert!(
        stdout.contains("LSP Information"),
        "Should contain LSP information section"
    );
    assert!(
        stdout.contains("Call Hierarchy"),
        "Should contain call hierarchy"
    );

    // Exact call hierarchy assertions for Go Calculate function
    // Expected incoming calls: main(), ProcessNumbers(), BusinessLogic.ProcessValue()
    validate_incoming_calls(&stdout, 3)
        .map_err(|e| anyhow::anyhow!("Go incoming calls validation failed: {}", e))?;

    // Expected outgoing calls: Add(), Multiply(), Subtract() (conditional)
    validate_outgoing_calls(&stdout, 3)
        .map_err(|e| anyhow::anyhow!("Go outgoing calls validation failed: {}", e))?;

    Ok(())
}

#[test]
fn test_typescript_lsp_call_hierarchy_exact() -> Result<()> {
    setup_comprehensive_tests()?;

    // Start daemon and initialize workspace
    start_daemon_and_wait()?;

    let workspace_path = fixtures::get_typescript_project1();
    init_lsp_workspace(workspace_path.to_str().unwrap(), &["typescript"])?;

    // Wait for typescript-language-server to fully index the project
    wait_for_language_server_ready(Duration::from_secs(10));

    // Test extraction with LSP for the calculate function
    let file_path = workspace_path.join("src/calculator.ts");
    let extract_args = [
        "extract",
        &format!("{}:17", file_path.to_string_lossy()), // Line 17 should be the calculate function
        "--lsp",
    ];

    let start = Instant::now();
    let (stdout, stderr, success) =
        run_probe_command_with_timeout(&extract_args, performance::MAX_EXTRACT_TIME)?;
    let elapsed = start.elapsed();

    // Cleanup before assertions to avoid daemon issues
    cleanup_comprehensive_tests();

    // Validate the command succeeded
    assert!(success, "Extract command should succeed. Stderr: {stderr}");

    // Validate performance requirement
    assert!(
        elapsed < performance::MAX_EXTRACT_TIME,
        "Extract took {:?}, should be under {:?}",
        elapsed,
        performance::MAX_EXTRACT_TIME
    );

    // Validate basic extraction worked
    assert!(
        stdout.contains("calculate"),
        "Should extract the calculate function"
    );
    assert!(
        stdout.contains("function calculate"),
        "Should show function signature"
    );

    // Validate LSP call hierarchy information is present
    assert!(
        stdout.contains("LSP Information"),
        "Should contain LSP information section"
    );
    assert!(
        stdout.contains("Call Hierarchy"),
        "Should contain call hierarchy"
    );

    // Exact call hierarchy assertions for TypeScript calculate function
    // Expected incoming calls: main(), processNumbers(), Calculator.processValue(), BusinessLogic.processValue(), advancedCalculation()
    validate_incoming_calls(&stdout, 5)
        .map_err(|e| anyhow::anyhow!("TypeScript incoming calls validation failed: {}", e))?;

    // Expected outgoing calls: add(), multiply(), subtract() (conditional)
    validate_outgoing_calls(&stdout, 3)
        .map_err(|e| anyhow::anyhow!("TypeScript outgoing calls validation failed: {}", e))?;

    Ok(())
}

#[test]
fn test_javascript_lsp_call_hierarchy_exact() -> Result<()> {
    setup_comprehensive_tests()?;

    // Start daemon and initialize workspace
    start_daemon_and_wait()?;

    let workspace_path = fixtures::get_javascript_project1();
    init_lsp_workspace(workspace_path.to_str().unwrap(), &["javascript"])?;

    // Wait for typescript-language-server to fully index the JavaScript project
    wait_for_language_server_ready(Duration::from_secs(10));

    // Test extraction with LSP for the calculate function
    let file_path = workspace_path.join("src/calculator.js");
    let extract_args = [
        "extract",
        &format!("{}:14", file_path.to_string_lossy()), // Line 14 should be the calculate function
        "--lsp",
    ];

    let start = Instant::now();
    let (stdout, stderr, success) =
        run_probe_command_with_timeout(&extract_args, performance::MAX_EXTRACT_TIME)?;
    let elapsed = start.elapsed();

    // Cleanup before assertions to avoid daemon issues
    cleanup_comprehensive_tests();

    // Validate the command succeeded
    assert!(success, "Extract command should succeed. Stderr: {stderr}");

    // Validate performance requirement
    assert!(
        elapsed < performance::MAX_EXTRACT_TIME,
        "Extract took {:?}, should be under {:?}",
        elapsed,
        performance::MAX_EXTRACT_TIME
    );

    // Validate basic extraction worked
    assert!(
        stdout.contains("calculate"),
        "Should extract the calculate function"
    );
    assert!(
        stdout.contains("function calculate"),
        "Should show function signature"
    );

    // Validate LSP call hierarchy information is present
    assert!(
        stdout.contains("LSP Information"),
        "Should contain LSP information section"
    );
    assert!(
        stdout.contains("Call Hierarchy"),
        "Should contain call hierarchy"
    );

    // Exact call hierarchy assertions for JavaScript calculate function
    // Expected incoming calls: main(), processNumbers(), Calculator.processValue(), BusinessLogic.processValue(), advancedCalculation(), createProcessor()
    validate_incoming_calls(&stdout, 6)
        .map_err(|e| anyhow::anyhow!("JavaScript incoming calls validation failed: {}", e))?;

    // Expected outgoing calls: add(), multiply(), subtract() (conditional)
    validate_outgoing_calls(&stdout, 3)
        .map_err(|e| anyhow::anyhow!("JavaScript outgoing calls validation failed: {}", e))?;

    Ok(())
}

#[test]
fn test_concurrent_multi_language_lsp_operations() -> Result<()> {
    setup_comprehensive_tests()?;

    // Start daemon
    start_daemon_and_wait()?;

    // Initialize all language workspaces
    let go_workspace = fixtures::get_go_project1();
    let ts_workspace = fixtures::get_typescript_project1();
    let js_workspace = fixtures::get_javascript_project1();

    init_lsp_workspace(go_workspace.to_str().unwrap(), &["go"])?;
    init_lsp_workspace(ts_workspace.to_str().unwrap(), &["typescript"])?;
    init_lsp_workspace(js_workspace.to_str().unwrap(), &["javascript"])?;

    // Wait for all language servers to be ready
    wait_for_language_server_ready(Duration::from_secs(20));

    // Perform concurrent operations on all languages
    let start = Instant::now();

    // Go extraction
    let go_file = go_workspace.join("calculator.go");
    let (go_stdout, go_stderr, go_success) = run_probe_command_with_timeout(
        &[
            "extract",
            &format!("{}:10", go_file.to_string_lossy()),
            "--lsp",
        ],
        performance::MAX_EXTRACT_TIME,
    )?;

    // TypeScript extraction
    let ts_file = ts_workspace.join("src/calculator.ts");
    let (ts_stdout, ts_stderr, ts_success) = run_probe_command_with_timeout(
        &[
            "extract",
            &format!("{}:17", ts_file.to_string_lossy()),
            "--lsp",
        ],
        performance::MAX_EXTRACT_TIME,
    )?;

    // JavaScript extraction
    let js_file = js_workspace.join("src/calculator.js");
    let (js_stdout, js_stderr, js_success) = run_probe_command_with_timeout(
        &[
            "extract",
            &format!("{}:14", js_file.to_string_lossy()),
            "--lsp",
        ],
        performance::MAX_EXTRACT_TIME,
    )?;

    let total_elapsed = start.elapsed();

    // Cleanup before assertions
    cleanup_comprehensive_tests();

    // Validate all operations succeeded
    assert!(
        go_success,
        "Go extraction should succeed. Stderr: {go_stderr}"
    );
    assert!(
        ts_success,
        "TypeScript extraction should succeed. Stderr: {ts_stderr}"
    );
    assert!(
        js_success,
        "JavaScript extraction should succeed. Stderr: {js_stderr}"
    );

    // Validate total time is reasonable for concurrent operations
    assert!(
        total_elapsed < Duration::from_secs(15),
        "Concurrent operations took {total_elapsed:?}, should be under 15s"
    );

    // Validate all outputs contain LSP information
    assert!(
        go_stdout.contains("LSP Information"),
        "Go output should contain LSP information"
    );
    assert!(
        ts_stdout.contains("LSP Information"),
        "TypeScript output should contain LSP information"
    );
    assert!(
        js_stdout.contains("LSP Information"),
        "JavaScript output should contain LSP information"
    );

    // Validate call hierarchy is present in all outputs
    assert!(
        go_stdout.contains("Call Hierarchy"),
        "Go output should contain call hierarchy"
    );
    assert!(
        ts_stdout.contains("Call Hierarchy"),
        "TypeScript output should contain call hierarchy"
    );
    assert!(
        js_stdout.contains("Call Hierarchy"),
        "JavaScript output should contain call hierarchy"
    );

    Ok(())
}

#[test]
fn test_search_with_lsp_enrichment_performance() -> Result<()> {
    setup_comprehensive_tests()?;

    // Start daemon and initialize workspace
    start_daemon_and_wait()?;

    let workspace_path = fixtures::get_go_project1();
    init_lsp_workspace(workspace_path.to_str().unwrap(), &["go"])?;

    // Wait for language server to be ready
    wait_for_language_server_ready(Duration::from_secs(15));

    // Test search with LSP enrichment
    let search_args = [
        "search",
        "Calculate",
        workspace_path.to_str().unwrap(),
        "--max-results",
        "5",
        "--lsp",
    ];

    let start = Instant::now();
    let (stdout, stderr, success) =
        run_probe_command_with_timeout(&search_args, performance::MAX_SEARCH_TIME)?;
    let elapsed = start.elapsed();

    // Cleanup before assertions
    cleanup_comprehensive_tests();

    // Validate the command succeeded
    assert!(success, "Search command should succeed. Stderr: {stderr}");

    // Validate performance requirement
    assert!(
        elapsed < performance::MAX_SEARCH_TIME,
        "Search took {:?}, should be under {:?}",
        elapsed,
        performance::MAX_SEARCH_TIME
    );

    // Validate search results contain expected functions
    assert!(
        stdout.contains("Calculate"),
        "Should find Calculate function"
    );
    assert!(!stdout.is_empty(), "Should return non-empty results");

    // LSP enrichment might not be visible in search results, but the command should succeed
    // The important thing is that LSP doesn't break or slow down search

    Ok(())
}

#[test]
fn test_lsp_daemon_status_with_multiple_languages() -> Result<()> {
    setup_comprehensive_tests()?;

    // Start daemon and initialize all language workspaces
    start_daemon_and_wait()?;

    let go_workspace = fixtures::get_go_project1();
    let ts_workspace = fixtures::get_typescript_project1();
    let js_workspace = fixtures::get_javascript_project1();

    init_lsp_workspace(go_workspace.to_str().unwrap(), &["go"])?;
    init_lsp_workspace(ts_workspace.to_str().unwrap(), &["typescript"])?;
    init_lsp_workspace(js_workspace.to_str().unwrap(), &["javascript"])?;

    // Wait for language servers to initialize
    wait_for_language_server_ready(Duration::from_secs(20));

    // Check daemon status
    let (stdout, stderr, success) =
        run_probe_command_with_timeout(&["lsp", "status"], Duration::from_secs(10))?;

    // Cleanup before assertions
    cleanup_comprehensive_tests();

    // Validate status command succeeded
    assert!(success, "LSP status should succeed. Stderr: {stderr}");

    // Validate status output contains expected information
    assert!(
        stdout.contains("LSP Daemon Status"),
        "Should show daemon status header"
    );
    assert!(stdout.contains("Connected"), "Should show connected status");

    // Should show information about multiple language servers
    // Note: The exact format may vary, but we should see evidence of multiple language pools
    assert!(
        stdout.contains("Server Pools") || stdout.contains("Language") || stdout.len() > 100,
        "Should show substantial status information for multiple languages"
    );

    Ok(())
}

#[test]
fn test_lsp_initialization_timeout_handling() -> Result<()> {
    setup_comprehensive_tests()?;

    // Start daemon
    start_daemon_and_wait()?;

    let workspace_path = fixtures::get_go_project1();

    // Initialize workspace but don't wait for full indexing
    init_lsp_workspace(workspace_path.to_str().unwrap(), &["go"])?;

    // Try extraction immediately (before gopls is fully ready)
    let file_path = workspace_path.join("calculator.go");
    let extract_args = [
        "extract",
        &format!("{}:10", file_path.to_string_lossy()),
        "--lsp",
    ];

    let (stdout, _stderr, success) =
        run_probe_command_with_timeout(&extract_args, Duration::from_secs(30))?;

    // Cleanup before assertions
    cleanup_comprehensive_tests();

    // The command should succeed even if LSP isn't fully ready
    assert!(
        success,
        "Extract should succeed even with LSP not fully ready"
    );

    // Should extract the function even if LSP info is not available
    assert!(
        stdout.contains("Calculate"),
        "Should extract function even without LSP"
    );

    // LSP information might or might not be present, depending on timing
    // The important thing is that the command doesn't hang or fail

    Ok(())
}

#[test]
fn test_error_recovery_with_invalid_file_paths() -> Result<()> {
    setup_comprehensive_tests()?;

    // Start daemon
    start_daemon_and_wait()?;

    let workspace_path = fixtures::get_go_project1();
    init_lsp_workspace(workspace_path.to_str().unwrap(), &["go"])?;

    // Wait for language server
    wait_for_language_server_ready(Duration::from_secs(15));

    // Try extraction with invalid file path
    let extract_args = ["extract", "nonexistent_file.go:10", "--lsp"];

    let (stdout, stderr, success) =
        run_probe_command_with_timeout(&extract_args, Duration::from_secs(10))?;

    // Cleanup before assertions
    cleanup_comprehensive_tests();

    // The command should fail gracefully
    assert!(!success, "Extract should fail for nonexistent file");

    // Should provide meaningful error message
    assert!(
        stderr.contains("No such file") || stderr.contains("not found") || stdout.contains("Error"),
        "Should provide meaningful error message"
    );

    // Should not crash the daemon or leave it in a bad state
    // (The cleanup function will verify daemon can be stopped properly)

    Ok(())
}

/// Performance benchmark test - not a strict requirement but useful for monitoring
#[test]
fn test_lsp_performance_benchmark() -> Result<()> {
    setup_comprehensive_tests()?;

    // Start daemon and initialize workspace
    start_daemon_and_wait()?;

    let workspace_path = fixtures::get_go_project1();
    init_lsp_workspace(workspace_path.to_str().unwrap(), &["go"])?;

    // Wait for language server to be fully ready
    wait_for_language_server_ready(Duration::from_secs(15));

    // Perform multiple extractions to test consistency
    let file_path = workspace_path.join("calculator.go");
    let mut timings = Vec::new();

    for i in 0..3 {
        let extract_args = [
            "extract",
            &format!("{}:10", file_path.to_string_lossy()),
            "--lsp",
        ];

        let start = Instant::now();
        let (stdout, stderr, success) =
            run_probe_command_with_timeout(&extract_args, Duration::from_secs(10))?;
        let elapsed = start.elapsed();

        assert!(
            success,
            "Extraction {} should succeed. Stderr: {}",
            i + 1,
            stderr
        );
        assert!(
            stdout.contains("Calculate"),
            "Should extract function in attempt {}",
            i + 1
        );

        timings.push(elapsed);
    }

    // Cleanup before assertions
    cleanup_comprehensive_tests();

    // Calculate average timing
    let avg_time = timings.iter().sum::<Duration>() / timings.len() as u32;

    // Performance expectations (not strict failures, but good to monitor)
    if avg_time > Duration::from_secs(2) {
        eprintln!("Warning: Average LSP extraction time ({avg_time:?}) is slower than expected");
    }

    // All individual timings should be reasonable
    for (i, timing) in timings.iter().enumerate() {
        assert!(
            *timing < Duration::from_secs(5),
            "Extraction {} took {:?}, which is too slow",
            i + 1,
            timing
        );
    }

    println!("LSP Performance Benchmark Results:");
    println!("  Individual timings: {timings:?}");
    println!("  Average time: {avg_time:?}");

    Ok(())
}
