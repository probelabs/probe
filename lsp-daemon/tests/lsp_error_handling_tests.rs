//! LSP Error Handling and Resilience Tests
//!
//! This module tests error handling scenarios including:
//! - Server failures and recovery
//! - Timeout handling
//! - Invalid requests and malformed responses
//! - Network failures and connection issues
//! - Resource exhaustion scenarios

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::timeout;
use tracing::{debug, info, warn};

// Import modules for error testing
use lsp_daemon::language_detector::{Language, LanguageDetector};
use lsp_daemon::lsp_registry::LspRegistry;
use lsp_daemon::relationship::lsp_client_wrapper::LspClientWrapper;
use lsp_daemon::relationship::lsp_enhancer::{
    LspEnhancementConfig, LspEnhancementError, LspRelationshipEnhancer,
};
use lsp_daemon::server_manager::SingleServerManager;
use lsp_daemon::symbol::SymbolUIDGenerator;
use lsp_daemon::universal_cache::CacheLayer;
use lsp_daemon::workspace_cache_router::{WorkspaceCacheRouter, WorkspaceCacheRouterConfig};
use lsp_daemon::workspace_resolver::WorkspaceResolver;

/// Error test scenario configuration
#[derive(Debug, Clone)]
pub struct ErrorTestConfig {
    /// Test timeout scenarios
    pub test_timeouts: bool,
    /// Test with very short timeouts (ms)
    pub short_timeout_ms: u64,
    /// Test server failure scenarios
    pub test_server_failures: bool,
    /// Test invalid requests
    pub test_invalid_requests: bool,
    /// Test resource exhaustion
    pub test_resource_exhaustion: bool,
    /// Test recovery mechanisms
    pub test_recovery: bool,
    /// Languages to test error handling for
    pub languages: Vec<Language>,
}

impl Default for ErrorTestConfig {
    fn default() -> Self {
        Self {
            test_timeouts: true,
            short_timeout_ms: 100,
            test_server_failures: true,
            test_invalid_requests: true,
            test_resource_exhaustion: false, // Disabled by default for CI safety
            test_recovery: true,
            languages: vec![Language::Rust, Language::Python],
        }
    }
}

/// Error test results
#[derive(Debug, Clone)]
pub struct ErrorTestResult {
    pub scenario: String,
    pub language: Option<Language>,
    pub expected_error: bool,
    pub actual_error: bool,
    pub error_type: String,
    pub duration: Duration,
    pub recovery_successful: bool,
}

impl ErrorTestResult {
    pub fn new(scenario: String, language: Option<Language>, expected_error: bool) -> Self {
        Self {
            scenario,
            language,
            expected_error,
            actual_error: false,
            error_type: String::new(),
            duration: Duration::ZERO,
            recovery_successful: false,
        }
    }

    pub fn success(&self) -> bool {
        self.expected_error == self.actual_error
    }
}

/// Error handling test suite
pub struct LspErrorHandlingTestSuite {
    server_manager: Arc<SingleServerManager>,
    lsp_client_wrapper: Arc<LspClientWrapper>,
    lsp_enhancer: Arc<LspRelationshipEnhancer>,
    config: ErrorTestConfig,
    test_workspace: TempDir,
}

impl LspErrorHandlingTestSuite {
    pub async fn new(config: ErrorTestConfig) -> Result<Self> {
        let test_workspace = TempDir::new()?;

        // Create cache infrastructure
        let workspace_config = WorkspaceCacheRouterConfig {
            base_cache_dir: test_workspace.path().join("caches"),
            max_open_caches: 3,
            max_parent_lookup_depth: 2,
            ..Default::default()
        };

        // Create LSP infrastructure
        let registry = Arc::new(LspRegistry::new()?);
        let child_processes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let server_manager = Arc::new(SingleServerManager::new_with_tracker(
            registry,
            child_processes,
        ));

        let workspace_router = Arc::new(WorkspaceCacheRouter::new(
            workspace_config,
            server_manager.clone(),
        ));

        let universal_cache =
            Arc::new(lsp_daemon::universal_cache::UniversalCache::new(workspace_router).await?);

        let cache_layer = Arc::new(CacheLayer::new(universal_cache, None, None));

        let language_detector = Arc::new(LanguageDetector::new());
        let workspace_resolver = Arc::new(tokio::sync::Mutex::new(WorkspaceResolver::new(None)));

        let lsp_client_wrapper = Arc::new(LspClientWrapper::new(
            server_manager.clone(),
            language_detector.clone(),
            workspace_resolver.clone(),
        ));

        let uid_generator = Arc::new(SymbolUIDGenerator::new());

        let lsp_config = LspEnhancementConfig {
            timeout_ms: 5000,
            cache_lsp_responses: true,
            ..Default::default()
        };

        let lsp_enhancer = Arc::new(LspRelationshipEnhancer::with_config(
            Some(server_manager.clone()),
            language_detector,
            workspace_resolver,
            cache_layer,
            uid_generator,
            lsp_config,
        ));

        Ok(Self {
            server_manager,
            lsp_client_wrapper,
            lsp_enhancer,
            config,
            test_workspace,
        })
    }

    /// Run all error handling tests
    pub async fn run_all_tests(&self) -> Result<Vec<ErrorTestResult>> {
        info!("🧪 Starting LSP error handling tests");
        let mut results = Vec::new();

        // Test 1: Timeout scenarios
        if self.config.test_timeouts {
            info!("⏰ Testing timeout scenarios...");
            let timeout_results = self.test_timeout_scenarios().await?;
            results.extend(timeout_results);
        }

        // Test 2: Server failure scenarios
        if self.config.test_server_failures {
            info!("💥 Testing server failure scenarios...");
            let server_failure_results = self.test_server_failure_scenarios().await?;
            results.extend(server_failure_results);
        }

        // Test 3: Invalid request scenarios
        if self.config.test_invalid_requests {
            info!("🚫 Testing invalid request scenarios...");
            let invalid_request_results = self.test_invalid_request_scenarios().await?;
            results.extend(invalid_request_results);
        }

        // Test 4: Recovery mechanisms
        if self.config.test_recovery {
            info!("🔄 Testing recovery mechanisms...");
            let recovery_results = self.test_recovery_scenarios().await?;
            results.extend(recovery_results);
        }

        // Test 5: Resource exhaustion (if enabled)
        if self.config.test_resource_exhaustion {
            info!("📊 Testing resource exhaustion scenarios...");
            let resource_results = self.test_resource_exhaustion_scenarios().await?;
            results.extend(resource_results);
        }

        info!("✅ Error handling tests completed");
        Ok(results)
    }

    /// Test various timeout scenarios
    async fn test_timeout_scenarios(&self) -> Result<Vec<ErrorTestResult>> {
        let mut results = Vec::new();

        // Test 1: Very short timeout
        for &language in &self.config.languages {
            let workspace = self.create_test_workspace(language).await?;
            let test_file = workspace
                .join("main")
                .with_extension(Self::get_extension(language));

            let mut result = ErrorTestResult::new(
                format!("short_timeout_{:?}", language),
                Some(language),
                true, // We expect this to timeout/fail
            );

            let start_time = Instant::now();

            let timeout_result = self
                .lsp_client_wrapper
                .get_references(
                    &test_file,
                    1,
                    1,
                    false,
                    self.config.short_timeout_ms, // Very short timeout
                )
                .await;

            result.duration = start_time.elapsed();
            result.actual_error = timeout_result.is_err();

            if let Err(e) = timeout_result {
                result.error_type = format!("{:?}", e);
                debug!(
                    "✅ Short timeout test for {:?} failed as expected: {}",
                    language, e
                );
            } else {
                debug!(
                    "⚠️  Short timeout test for {:?} unexpectedly succeeded",
                    language
                );
            }

            results.push(result);
        }

        // Test 2: Nonexistent file with timeout
        let mut result = ErrorTestResult::new(
            "nonexistent_file_timeout".to_string(),
            None,
            true, // Expect error
        );

        let nonexistent_file = PathBuf::from("/nonexistent/path/file.rs");
        let start_time = Instant::now();

        let nonexistent_result = self
            .lsp_client_wrapper
            .get_references(
                &nonexistent_file,
                1,
                1,
                false,
                1000, // 1 second timeout
            )
            .await;

        result.duration = start_time.elapsed();
        result.actual_error = nonexistent_result.is_err();

        if let Err(e) = nonexistent_result {
            result.error_type = format!("{:?}", e);
            debug!("✅ Nonexistent file test failed as expected: {}", e);
        }

        results.push(result);

        Ok(results)
    }

    /// Test server failure and unavailability scenarios
    async fn test_server_failure_scenarios(&self) -> Result<Vec<ErrorTestResult>> {
        let mut results = Vec::new();

        // Test 1: Server not available for unsupported language
        let mut result = ErrorTestResult::new(
            "unsupported_language".to_string(),
            None,
            true, // Expect error
        );

        let test_file = PathBuf::from("test.unknown");
        let start_time = Instant::now();

        // This should fail because we don't support ".unknown" files
        let unsupported_result = self
            .lsp_client_wrapper
            .get_references(&test_file, 1, 1, false, 5000)
            .await;

        result.duration = start_time.elapsed();
        result.actual_error = unsupported_result.is_err();

        if let Err(e) = unsupported_result {
            result.error_type = format!("{:?}", e);
            debug!("✅ Unsupported language test failed as expected: {}", e);
        }

        results.push(result);

        // Test 2: Server initialization failure simulation
        // This is more complex and would require mocking or special test setup
        // For now, we'll test with a workspace that has no valid configuration
        let mut result = ErrorTestResult::new(
            "invalid_workspace".to_string(),
            Some(Language::Rust),
            true, // Expect error or degraded performance
        );

        let invalid_workspace = self.test_workspace.path().join("invalid_workspace");
        std::fs::create_dir_all(&invalid_workspace)?;

        // Create a file but no proper workspace configuration
        let invalid_file = invalid_workspace.join("isolated.rs");
        std::fs::write(&invalid_file, "fn main() { }")?;

        let start_time = Instant::now();
        let invalid_workspace_result = self
            .lsp_client_wrapper
            .get_references(
                &invalid_file,
                1,
                1,
                false,
                10000, // Give it time
            )
            .await;

        result.duration = start_time.elapsed();
        result.actual_error = invalid_workspace_result.is_err();

        if let Err(e) = invalid_workspace_result {
            result.error_type = format!("{:?}", e);
            debug!("✅ Invalid workspace test failed as expected: {}", e);
        } else {
            debug!("ℹ️  Invalid workspace test succeeded (server handled gracefully)");
        }

        results.push(result);

        Ok(results)
    }

    /// Test invalid request scenarios
    async fn test_invalid_request_scenarios(&self) -> Result<Vec<ErrorTestResult>> {
        let mut results = Vec::new();

        for &language in &self.config.languages {
            let workspace = self.create_test_workspace(language).await?;
            let test_file = workspace
                .join("main")
                .with_extension(Self::get_extension(language));

            // Test 1: Invalid position (way out of bounds)
            let mut result = ErrorTestResult::new(
                format!("invalid_position_{:?}", language),
                Some(language),
                false, // This usually doesn't error, just returns empty results
            );

            let start_time = Instant::now();

            let invalid_pos_result = self
                .lsp_client_wrapper
                .get_references(
                    &test_file, 99999, 99999, false, // Invalid position
                    5000,
                )
                .await;

            result.duration = start_time.elapsed();
            result.actual_error = invalid_pos_result.is_err();

            if let Err(e) = invalid_pos_result {
                result.error_type = format!("{:?}", e);
                debug!("Invalid position test for {:?}: {}", language, e);
            } else if let Ok(refs) = invalid_pos_result {
                debug!(
                    "Invalid position test for {:?} returned {} references (expected)",
                    language,
                    refs.len()
                );
            }

            results.push(result);

            // Test 2: File exists but is not parseable
            let corrupt_file = workspace
                .join("corrupt")
                .with_extension(Self::get_extension(language));
            std::fs::write(
                &corrupt_file,
                "This is not valid code in any language!@#$%^&*()",
            )?;

            let mut corrupt_result = ErrorTestResult::new(
                format!("corrupt_file_{:?}", language),
                Some(language),
                false, // Language servers usually handle this gracefully
            );

            let start_time = Instant::now();

            let corrupt_file_result = self
                .lsp_client_wrapper
                .get_references(&corrupt_file, 1, 1, false, 5000)
                .await;

            corrupt_result.duration = start_time.elapsed();
            corrupt_result.actual_error = corrupt_file_result.is_err();

            if let Err(e) = corrupt_file_result {
                corrupt_result.error_type = format!("{:?}", e);
                debug!("Corrupt file test for {:?}: {}", language, e);
            } else {
                debug!("Corrupt file test for {:?} handled gracefully", language);
            }

            results.push(corrupt_result);
        }

        Ok(results)
    }

    /// Test recovery mechanisms after failures
    async fn test_recovery_scenarios(&self) -> Result<Vec<ErrorTestResult>> {
        let mut results = Vec::new();

        for &language in &self.config.languages {
            let workspace = self.create_test_workspace(language).await?;
            let test_file = workspace
                .join("main")
                .with_extension(Self::get_extension(language));

            let mut result = ErrorTestResult::new(
                format!("recovery_after_timeout_{:?}", language),
                Some(language),
                false, // Recovery should succeed
            );

            // Step 1: Make a request that might fail/timeout
            let _timeout_result = self
                .lsp_client_wrapper
                .get_references(
                    &test_file, 1, 1, false, 50, // Very short timeout
                )
                .await;

            // Step 2: Wait a bit and try again with normal timeout
            tokio::time::sleep(Duration::from_millis(200)).await;

            let start_time = Instant::now();
            let recovery_result = self
                .lsp_client_wrapper
                .get_references(
                    &test_file, 5, 10, false, // Different position
                    10000, // Generous timeout
                )
                .await;

            result.duration = start_time.elapsed();
            result.actual_error = recovery_result.is_err();
            result.recovery_successful = recovery_result.is_ok();

            if let Err(e) = recovery_result {
                result.error_type = format!("{:?}", e);
                debug!("❌ Recovery failed for {:?}: {}", language, e);
            } else {
                debug!("✅ Recovery successful for {:?}", language);
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Test resource exhaustion scenarios
    async fn test_resource_exhaustion_scenarios(&self) -> Result<Vec<ErrorTestResult>> {
        let mut results = Vec::new();

        // Test 1: Many concurrent requests
        for &language in &self.config.languages {
            let workspace = self.create_test_workspace(language).await?;
            let test_file = workspace
                .join("main")
                .with_extension(Self::get_extension(language));

            let mut result = ErrorTestResult::new(
                format!("concurrent_overload_{:?}", language),
                Some(language),
                false, // Should handle gracefully
            );

            let start_time = Instant::now();

            // Launch many concurrent requests
            let mut handles = Vec::new();
            for i in 0..20 {
                let client = self.lsp_client_wrapper.clone();
                let file = test_file.clone();

                let handle = tokio::spawn(async move {
                    client
                        .get_references(&file, (i % 10) as u32 + 1, (i % 5) as u32 + 1, false, 5000)
                        .await
                });
                handles.push(handle);
            }

            // Wait for all to complete
            let mut successful = 0;
            let mut failed = 0;

            for handle in handles {
                match handle.await {
                    Ok(Ok(_)) => successful += 1,
                    Ok(Err(_)) => failed += 1,
                    Err(_) => failed += 1,
                }
            }

            result.duration = start_time.elapsed();
            result.actual_error = failed > successful;
            result.recovery_successful = successful > 0;

            debug!(
                "Concurrent overload test for {:?}: {}/{} successful",
                language,
                successful,
                successful + failed
            );

            results.push(result);
        }

        Ok(results)
    }

    /// Create a test workspace for a specific language
    async fn create_test_workspace(&self, language: Language) -> Result<PathBuf> {
        let workspace_dir = self
            .test_workspace
            .path()
            .join(format!("{:?}_workspace", language));
        std::fs::create_dir_all(&workspace_dir)?;

        // Create basic workspace structure
        match language {
            Language::Rust => {
                std::fs::write(
                    workspace_dir.join("Cargo.toml"),
                    r#"
[package]
name = "error_test"
version = "0.1.0"
edition = "2021"
"#,
                )?;

                let src_dir = workspace_dir.join("src");
                std::fs::create_dir_all(&src_dir)?;

                std::fs::write(
                    src_dir.join("main.rs"),
                    r#"
fn main() {
    let result = test_function(42);
    println!("Result: {}", result);
}

fn test_function(x: i32) -> i32 {
    x * 2
}

pub struct TestStruct {
    pub value: i32,
}

impl TestStruct {
    pub fn new(value: i32) -> Self {
        Self { value }
    }
    
    pub fn get_value(&self) -> i32 {
        self.value
    }
}
"#,
                )?;
            }
            Language::Python => {
                std::fs::write(
                    workspace_dir.join("main.py"),
                    r#"
def main():
    result = test_function(42)
    print(f"Result: {result}")

def test_function(x: int) -> int:
    return x * 2

class TestClass:
    def __init__(self, value: int):
        self.value = value
    
    def get_value(self) -> int:
        return self.value

if __name__ == "__main__":
    main()
"#,
                )?;
            }
            Language::Go => {
                std::fs::write(
                    workspace_dir.join("go.mod"),
                    "module error_test\n\ngo 1.19\n",
                )?;
                std::fs::write(
                    workspace_dir.join("main.go"),
                    r#"
package main

import "fmt"

func main() {
    result := testFunction(42)
    fmt.Printf("Result: %d\n", result)
}

func testFunction(x int) int {
    return x * 2
}

type TestStruct struct {
    Value int
}

func NewTestStruct(value int) *TestStruct {
    return &TestStruct{Value: value}
}

func (t *TestStruct) GetValue() int {
    return t.Value
}
"#,
                )?;
            }
            Language::TypeScript => {
                std::fs::write(
                    workspace_dir.join("package.json"),
                    r#"
{
  "name": "error_test",
  "version": "1.0.0",
  "main": "main.ts",
  "devDependencies": {
    "typescript": "^4.9.0"
  }
}
"#,
                )?;

                std::fs::write(
                    workspace_dir.join("main.ts"),
                    r#"
function main(): void {
    const result = testFunction(42);
    console.log(`Result: ${result}`);
}

function testFunction(x: number): number {
    return x * 2;
}

class TestClass {
    constructor(private value: number) {}
    
    getValue(): number {
        return this.value;
    }
}

interface TestInterface {
    getValue(): number;
}

if (require.main === module) {
    main();
}
"#,
                )?;
            }
            _ => {
                // Generic file for unsupported languages
                std::fs::write(workspace_dir.join("main.txt"), "Test file content")?;
            }
        }

        Ok(workspace_dir)
    }

    fn get_extension(language: Language) -> &'static str {
        match language {
            Language::Rust => "rs",
            Language::Python => "py",
            Language::Go => "go",
            Language::TypeScript => "ts",
            Language::JavaScript => "js",
            _ => "txt",
        }
    }
}

/// Print error handling test results
pub fn print_error_test_results(results: &[ErrorTestResult]) {
    println!("\n🧪 LSP Error Handling Test Results");
    println!("=================================");

    let mut passed = 0;
    let mut failed = 0;
    let mut by_scenario: std::collections::HashMap<String, Vec<&ErrorTestResult>> =
        std::collections::HashMap::new();

    for result in results {
        if result.success() {
            passed += 1;
        } else {
            failed += 1;
        }

        by_scenario
            .entry(result.scenario.clone())
            .or_default()
            .push(result);
    }

    println!("\n📊 Overall Results:");
    println!("  ✅ Passed: {}", passed);
    println!("  ❌ Failed: {}", failed);
    println!(
        "  Success Rate: {:.1}%",
        (passed as f64 / (passed + failed) as f64) * 100.0
    );

    println!("\n📋 Detailed Results:");
    println!("┌─────────────────────────────────┬──────────────┬─────────────┬──────────────┬─────────────────────────────┐");
    println!("│ Scenario                        │ Language     │ Expected    │ Actual       │ Status                      │");
    println!("├─────────────────────────────────┼──────────────┼─────────────┼──────────────┼─────────────────────────────┤");

    for result in results {
        let language_str = result
            .language
            .map(|l| format!("{:?}", l))
            .unwrap_or_else(|| "N/A".to_string());

        let expected_str = if result.expected_error {
            "Error"
        } else {
            "Success"
        };
        let actual_str = if result.actual_error {
            "Error"
        } else {
            "Success"
        };
        let status_str = if result.success() {
            "✅ PASS"
        } else {
            "❌ FAIL"
        };

        println!(
            "│ {:<31} │ {:<12} │ {:<11} │ {:<12} │ {:<27} │",
            truncate_string(&result.scenario, 31),
            truncate_string(&language_str, 12),
            expected_str,
            actual_str,
            status_str
        );
    }
    println!("└─────────────────────────────────┴──────────────┴─────────────┴──────────────┴─────────────────────────────┘");

    // Show error types for failed scenarios
    println!("\n🔍 Error Details:");
    for result in results {
        if result.actual_error && !result.error_type.is_empty() {
            println!(
                "  {} ({}): {}",
                result.scenario,
                result
                    .language
                    .map(|l| format!("{:?}", l))
                    .unwrap_or_else(|| "N/A".to_string()),
                truncate_string(&result.error_type, 80)
            );
        }
    }

    // Recovery success rate
    let recovery_tests: Vec<_> = results
        .iter()
        .filter(|r| r.scenario.contains("recovery"))
        .collect();

    if !recovery_tests.is_empty() {
        let successful_recoveries = recovery_tests
            .iter()
            .filter(|r| r.recovery_successful)
            .count();

        println!(
            "\n🔄 Recovery Success Rate: {}/{} ({:.1}%)",
            successful_recoveries,
            recovery_tests.len(),
            (successful_recoveries as f64 / recovery_tests.len() as f64) * 100.0
        );
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Main error handling test runner
#[tokio::test]
async fn run_lsp_error_handling_tests() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("lsp_daemon=info,lsp_error_handling_tests=debug")
        .with_test_writer()
        .init();

    let config = ErrorTestConfig {
        languages: vec![Language::Rust, Language::Python], // Limit for CI
        test_resource_exhaustion: false,                   // Disable for CI safety
        ..Default::default()
    };

    let test_suite = LspErrorHandlingTestSuite::new(config).await?;
    let results = test_suite.run_all_tests().await?;

    print_error_test_results(&results);

    // Assert that we have reasonable success rate
    let passed = results.iter().filter(|r| r.success()).count();
    let total = results.len();
    let success_rate = passed as f64 / total as f64;

    assert!(
        success_rate >= 0.7, // 70% success rate minimum
        "Error handling tests success rate too low: {:.1}% ({}/{})",
        success_rate * 100.0,
        passed,
        total
    );

    // Assert that recovery tests mostly succeed
    let recovery_tests: Vec<_> = results
        .iter()
        .filter(|r| r.scenario.contains("recovery"))
        .collect();

    if !recovery_tests.is_empty() {
        let successful_recoveries = recovery_tests
            .iter()
            .filter(|r| r.recovery_successful)
            .count();
        let recovery_rate = successful_recoveries as f64 / recovery_tests.len() as f64;

        assert!(
            recovery_rate >= 0.5, // 50% recovery success rate minimum
            "Recovery success rate too low: {:.1}%",
            recovery_rate * 100.0
        );
    }

    info!("✅ Error handling tests completed successfully!");
    Ok(())
}

/// Unit tests for error handling utilities
#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_error_test_result() {
        let mut result =
            ErrorTestResult::new("test_scenario".to_string(), Some(Language::Rust), true);

        // Initially not success because actual_error is false but expected is true
        assert!(!result.success());

        result.actual_error = true;
        assert!(result.success());
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("short", 10), "short");
        assert_eq!(
            truncate_string("this is a very long string", 10),
            "this is..."
        );
        assert_eq!(truncate_string("exactly10c", 10), "exactly10c");
    }

    #[tokio::test]
    async fn test_error_test_suite_creation() -> Result<()> {
        let config = ErrorTestConfig {
            languages: vec![Language::Rust],
            test_resource_exhaustion: false,
            ..Default::default()
        };

        let _suite = LspErrorHandlingTestSuite::new(config).await?;
        Ok(())
    }
}
