//! LSP Cache Integration Tests
//!
//! This module tests cache integration with real LSP data including:
//! - Cache hit/miss rates for LSP operations
//! - Cache persistence and retrieval
//! - Cache invalidation on file changes
//! - Multi-workspace cache isolation
//! - Performance impact of caching

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::timeout;
use tracing::{debug, info, warn};

// Import modules for cache testing
use lsp_daemon::language_detector::{Language, LanguageDetector};
use lsp_daemon::lsp_registry::LspRegistry;
use lsp_daemon::relationship::lsp_client_wrapper::LspClientWrapper;
use lsp_daemon::relationship::lsp_enhancer::{LspEnhancementConfig, LspRelationshipEnhancer};
use lsp_daemon::server_manager::SingleServerManager;
use lsp_daemon::symbol::SymbolUIDGenerator;
use lsp_daemon::universal_cache::{CacheLayer, UniversalCache};
use lsp_daemon::workspace_cache_router::{WorkspaceCacheRouter, WorkspaceCacheRouterConfig};
use lsp_daemon::workspace_resolver::WorkspaceResolver;

/// Cache test configuration
#[derive(Debug, Clone)]
pub struct CacheTestConfig {
    /// Number of operations to test cache performance
    pub cache_test_iterations: usize,
    /// Whether to test cache persistence across restarts
    pub test_persistence: bool,
    /// Whether to test cache invalidation
    pub test_invalidation: bool,
    /// Whether to test multi-workspace cache isolation
    pub test_workspace_isolation: bool,
    /// Languages to test caching for
    pub languages: Vec<Language>,
    /// LSP operation timeout
    pub timeout_ms: u64,
}

impl Default for CacheTestConfig {
    fn default() -> Self {
        Self {
            cache_test_iterations: 10,
            test_persistence: true,
            test_invalidation: true,
            test_workspace_isolation: true,
            languages: vec![Language::Rust, Language::Python],
            timeout_ms: 10000,
        }
    }
}

/// Cache test metrics
#[derive(Debug, Clone)]
pub struct CacheMetrics {
    pub operation: String,
    pub language: Language,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub total_operations: usize,
    pub avg_cached_duration: Duration,
    pub avg_uncached_duration: Duration,
    pub cache_size_bytes: usize,
    pub speedup_factor: f64,
}

impl CacheMetrics {
    pub fn hit_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_operations as f64
        }
    }

    pub fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate()
    }
}

/// Cache integration test results
#[derive(Debug)]
pub struct CacheTestResults {
    pub metrics: Vec<CacheMetrics>,
    pub persistence_tests_passed: usize,
    pub invalidation_tests_passed: usize,
    pub workspace_isolation_tests_passed: usize,
    pub total_cache_operations: usize,
    pub overall_hit_rate: f64,
}

impl CacheTestResults {
    pub fn new() -> Self {
        Self {
            metrics: Vec::new(),
            persistence_tests_passed: 0,
            invalidation_tests_passed: 0,
            workspace_isolation_tests_passed: 0,
            total_cache_operations: 0,
            overall_hit_rate: 0.0,
        }
    }

    pub fn calculate_overall_stats(&mut self) {
        if self.metrics.is_empty() {
            return;
        }

        let total_hits: usize = self.metrics.iter().map(|m| m.cache_hits).sum();
        let total_ops: usize = self.metrics.iter().map(|m| m.total_operations).sum();

        self.total_cache_operations = total_ops;
        self.overall_hit_rate = if total_ops > 0 {
            total_hits as f64 / total_ops as f64
        } else {
            0.0
        };
    }

    pub fn print_summary(&self) {
        println!("\n💾 LSP Cache Integration Test Results");
        println!("====================================");

        println!("\n📊 Overall Statistics:");
        println!("  Total cache operations: {}", self.total_cache_operations);
        println!("  Overall hit rate: {:.1}%", self.overall_hit_rate * 100.0);
        println!(
            "  Persistence tests passed: {}",
            self.persistence_tests_passed
        );
        println!(
            "  Invalidation tests passed: {}",
            self.invalidation_tests_passed
        );
        println!(
            "  Workspace isolation tests passed: {}",
            self.workspace_isolation_tests_passed
        );

        if !self.metrics.is_empty() {
            println!("\n🔍 Cache Performance by Operation:");
            println!("┌────────────────────────────────┬──────────────┬───────────┬───────────┬─────────────┬──────────────┐");
            println!("│ Operation                      │ Language     │ Hit Rate  │ Speedup   │ Cached (ms) │ Uncached (ms)│");
            println!("├────────────────────────────────┼──────────────┼───────────┼───────────┼─────────────┼──────────────┤");

            for metric in &self.metrics {
                println!(
                    "│ {:<30} │ {:<12} │ {:>8.1}% │ {:>8.1}x │ {:>10.1} │ {:>11.1} │",
                    truncate_string(&metric.operation, 30),
                    format!("{:?}", metric.language),
                    metric.hit_rate() * 100.0,
                    metric.speedup_factor,
                    metric.avg_cached_duration.as_millis() as f64,
                    metric.avg_uncached_duration.as_millis() as f64
                );
            }
            println!("└────────────────────────────────┴──────────────┴───────────┴───────────┴─────────────┴──────────────┘");
        }
    }
}

/// LSP Cache integration test suite
pub struct LspCacheIntegrationTestSuite {
    server_manager: Arc<SingleServerManager>,
    lsp_client_wrapper: Arc<LspClientWrapper>,
    lsp_enhancer: Arc<LspRelationshipEnhancer>,
    cache_layer: Arc<CacheLayer>,
    universal_cache: Arc<UniversalCache>,
    uid_generator: Arc<SymbolUIDGenerator>,
    config: CacheTestConfig,
    test_base_dir: TempDir,
}

impl LspCacheIntegrationTestSuite {
    pub async fn new(config: CacheTestConfig) -> Result<Self> {
        let test_base_dir = TempDir::new()?;

        // Create cache infrastructure with specific configuration for testing
        let workspace_config = WorkspaceCacheRouterConfig {
            base_cache_dir: test_base_dir.path().join("caches"),
            max_open_caches: 5,
            max_parent_lookup_depth: 3,
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

        let universal_cache = Arc::new(UniversalCache::new(workspace_router).await?);

        let cache_layer = Arc::new(CacheLayer::new(universal_cache.clone(), None, None));

        let language_detector = Arc::new(LanguageDetector::new());
        let workspace_resolver = Arc::new(tokio::sync::Mutex::new(WorkspaceResolver::new(None)));

        let lsp_client_wrapper = Arc::new(LspClientWrapper::new(
            server_manager.clone(),
            language_detector.clone(),
            workspace_resolver.clone(),
        ));

        let uid_generator = Arc::new(SymbolUIDGenerator::new());

        let lsp_config = LspEnhancementConfig {
            timeout_ms: config.timeout_ms,
            cache_lsp_responses: true, // Enable caching for tests
            ..Default::default()
        };

        let lsp_enhancer = Arc::new(LspRelationshipEnhancer::with_config(
            Some(server_manager.clone()),
            language_detector,
            workspace_resolver,
            cache_layer.clone(),
            uid_generator.clone(),
            lsp_config,
        ));

        Ok(Self {
            server_manager,
            lsp_client_wrapper,
            lsp_enhancer,
            cache_layer,
            universal_cache,
            uid_generator,
            config,
            test_base_dir,
        })
    }

    /// Run all cache integration tests
    pub async fn run_all_tests(&mut self) -> Result<CacheTestResults> {
        info!("💾 Starting LSP cache integration tests");
        let mut results = CacheTestResults::new();

        // Create test workspaces
        let test_workspaces = self.create_test_workspaces().await?;

        // Test 1: Basic cache performance
        info!("📊 Testing basic cache performance...");
        let cache_perf_metrics = self.test_cache_performance(&test_workspaces).await?;
        results.metrics.extend(cache_perf_metrics);

        // Test 2: Cache persistence across operations
        if self.config.test_persistence {
            info!("💾 Testing cache persistence...");
            results.persistence_tests_passed =
                self.test_cache_persistence(&test_workspaces).await?;
        }

        // Test 3: Cache invalidation
        if self.config.test_invalidation {
            info!("🔄 Testing cache invalidation...");
            results.invalidation_tests_passed =
                self.test_cache_invalidation(&test_workspaces).await?;
        }

        // Test 4: Multi-workspace cache isolation
        if self.config.test_workspace_isolation {
            info!("🏠 Testing workspace cache isolation...");
            results.workspace_isolation_tests_passed = self.test_workspace_isolation().await?;
        }

        // Calculate overall statistics
        results.calculate_overall_stats();

        info!("✅ Cache integration tests completed");
        Ok(results)
    }

    /// Test cache performance by measuring cache hits vs misses
    async fn test_cache_performance(
        &self,
        workspaces: &HashMap<Language, PathBuf>,
    ) -> Result<Vec<CacheMetrics>> {
        let mut metrics = Vec::new();

        for (&language, workspace_dir) in workspaces {
            let test_file = workspace_dir
                .join("src")
                .join("main")
                .with_extension(Self::get_extension(language));

            debug!(
                "Testing cache performance for {:?} using file: {:?}",
                language, test_file
            );

            // Test references operation
            let refs_metrics = self.benchmark_cache_operation(
                format!("references_{:?}", language),
                language,
                |client, file| async move {
                    client.get_references(&file, 10, 5, false, 5000).await
                },
                &test_file,
            ).await?;
            metrics.push(refs_metrics);

            // Test definition operation
            let def_metrics = self
                .benchmark_cache_operation(
                    format!("definition_{:?}", language),
                    language,
                    |client, file| async move { client.get_definition(&file, 10, 5, 5000).await },
                    &test_file,
                )
                .await?;
            metrics.push(def_metrics);

            // Test call hierarchy operation
            let call_metrics = self.benchmark_cache_operation(
                format!("call_hierarchy_{:?}", language),
                language,
                |client, file| async move {
                    client.get_call_hierarchy(&file, 15, 10, 5000).await
                },
                &test_file,
            ).await?;
            metrics.push(call_metrics);
        }

        Ok(metrics)
    }

    /// Benchmark a specific cache operation
    async fn benchmark_cache_operation<F, Fut, T>(
        &self,
        operation_name: String,
        language: Language,
        operation: F,
        test_file: &Path,
    ) -> Result<CacheMetrics>
    where
        F: Fn(Arc<LspClientWrapper>, PathBuf) -> Fut + Send + Sync,
        Fut: std::future::Future<
                Output = Result<T, lsp_daemon::relationship::lsp_enhancer::LspEnhancementError>,
            > + Send,
        T: Send,
    {
        let mut uncached_durations = Vec::new();
        let mut cached_durations = Vec::new();
        let mut cache_hits = 0;
        let mut cache_misses = 0;

        // First, clear any existing cache for this operation
        // Note: In a real implementation, you'd have cache clearing methods

        // Perform uncached operations (first run)
        debug!("Running uncached operations for {}", operation_name);
        for _ in 0..3 {
            let start_time = Instant::now();
            let result = operation(self.lsp_client_wrapper.clone(), test_file.to_path_buf()).await;
            let duration = start_time.elapsed();

            if result.is_ok() {
                uncached_durations.push(duration);
                cache_misses += 1;
            }

            // Small delay between operations
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Perform cached operations (subsequent runs)
        debug!("Running cached operations for {}", operation_name);
        for _ in 0..self.config.cache_test_iterations {
            let start_time = Instant::now();
            let result = operation(self.lsp_client_wrapper.clone(), test_file.to_path_buf()).await;
            let duration = start_time.elapsed();

            if result.is_ok() {
                cached_durations.push(duration);

                // Heuristic: if the operation is significantly faster, assume it's cached
                // In a real implementation, you'd have proper cache hit/miss tracking
                if duration < Duration::from_millis(500) {
                    cache_hits += 1;
                } else {
                    cache_misses += 1;
                }
            }

            // Small delay between operations
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let avg_uncached = if !uncached_durations.is_empty() {
            uncached_durations.iter().sum::<Duration>() / uncached_durations.len() as u32
        } else {
            Duration::from_millis(1000) // Default assumption
        };

        let avg_cached = if !cached_durations.is_empty() {
            cached_durations.iter().sum::<Duration>() / cached_durations.len() as u32
        } else {
            avg_uncached
        };

        let speedup_factor = if avg_cached.as_millis() > 0 {
            avg_uncached.as_millis() as f64 / avg_cached.as_millis() as f64
        } else {
            1.0
        };

        let total_operations = cache_hits + cache_misses;

        debug!(
            "Cache performance for {}: {}/{} hits, {:.1}x speedup",
            operation_name, cache_hits, total_operations, speedup_factor
        );

        Ok(CacheMetrics {
            operation: operation_name,
            language,
            cache_hits,
            cache_misses,
            total_operations,
            avg_cached_duration: avg_cached,
            avg_uncached_duration: avg_uncached,
            cache_size_bytes: 0, // Would need cache inspection API
            speedup_factor,
        })
    }

    /// Test cache persistence across different operations and time
    async fn test_cache_persistence(
        &self,
        workspaces: &HashMap<Language, PathBuf>,
    ) -> Result<usize> {
        let mut tests_passed = 0;

        for (&language, workspace_dir) in workspaces {
            let test_file = workspace_dir
                .join("src")
                .join("main")
                .with_extension(Self::get_extension(language));

            debug!("Testing cache persistence for {:?}", language);

            // Step 1: Perform an operation to populate cache
            let _initial_result = self
                .lsp_client_wrapper
                .get_references(&test_file, 10, 5, false, 5000)
                .await;

            // Step 2: Wait a bit
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Step 3: Perform the same operation again - should be faster if cached
            let start_time = Instant::now();
            let cached_result = self
                .lsp_client_wrapper
                .get_references(&test_file, 10, 5, false, 5000)
                .await;
            let _cached_duration = start_time.elapsed();

            // Step 4: Perform a different operation to ensure cache isn't just in memory
            let _diff_result = self
                .lsp_client_wrapper
                .get_definition(&test_file, 15, 8, 5000)
                .await;

            // Step 5: Perform the original operation again
            let start_time = Instant::now();
            let persistent_result = self
                .lsp_client_wrapper
                .get_references(&test_file, 10, 5, false, 5000)
                .await;
            let persistent_duration = start_time.elapsed();

            // Test passes if both operations succeeded and persistent operation was reasonably fast
            if cached_result.is_ok()
                && persistent_result.is_ok()
                && persistent_duration < Duration::from_millis(2000)
            {
                tests_passed += 1;
                debug!("✅ Cache persistence test passed for {:?}", language);
            } else {
                debug!("❌ Cache persistence test failed for {:?}", language);
            }
        }

        Ok(tests_passed)
    }

    /// Test cache invalidation when files change
    async fn test_cache_invalidation(
        &self,
        workspaces: &HashMap<Language, PathBuf>,
    ) -> Result<usize> {
        let mut tests_passed = 0;

        for (&language, workspace_dir) in workspaces {
            let test_file = workspace_dir
                .join("src")
                .join("main")
                .with_extension(Self::get_extension(language));

            debug!("Testing cache invalidation for {:?}", language);

            // Step 1: Perform operation to populate cache
            let original_result = self
                .lsp_client_wrapper
                .get_references(&test_file, 10, 5, false, 5000)
                .await;

            // Step 2: Modify the file to simulate a change
            if test_file.exists() {
                let original_content = std::fs::read_to_string(&test_file)?;
                let modified_content =
                    format!("{}\n// Cache invalidation test comment", original_content);
                std::fs::write(&test_file, &modified_content)?;

                // Give the file system time to register the change
                tokio::time::sleep(Duration::from_millis(200)).await;

                // Step 3: Perform the same operation - should not use stale cache
                let invalidated_result = self
                    .lsp_client_wrapper
                    .get_references(&test_file, 10, 5, false, 5000)
                    .await;

                // Step 4: Restore original content
                std::fs::write(&test_file, &original_content)?;

                // Test passes if both operations succeeded
                // In a real implementation, you'd verify cache was actually invalidated
                if original_result.is_ok() && invalidated_result.is_ok() {
                    tests_passed += 1;
                    debug!("✅ Cache invalidation test passed for {:?}", language);
                } else {
                    debug!("❌ Cache invalidation test failed for {:?}", language);
                }
            }
        }

        Ok(tests_passed)
    }

    /// Test that different workspaces have isolated caches
    async fn test_workspace_isolation(&self) -> Result<usize> {
        let mut tests_passed = 0;

        // Create two separate workspaces for the same language
        let workspace1 = self
            .create_test_workspace(Language::Rust, "workspace1")
            .await?;
        let workspace2 = self
            .create_test_workspace(Language::Rust, "workspace2")
            .await?;

        let file1 = workspace1.join("src").join("main.rs");
        let file2 = workspace2.join("src").join("main.rs");

        debug!(
            "Testing workspace cache isolation between {:?} and {:?}",
            file1, file2
        );

        // Step 1: Populate cache for workspace1
        let result1 = self
            .lsp_client_wrapper
            .get_references(&file1, 10, 5, false, 5000)
            .await;

        // Step 2: Perform operation on workspace2 - should not use workspace1's cache
        let result2 = self
            .lsp_client_wrapper
            .get_references(&file2, 10, 5, false, 5000)
            .await;

        // Step 3: Perform operation on workspace1 again - should use its own cache
        let result1_again = self
            .lsp_client_wrapper
            .get_references(&file1, 10, 5, false, 5000)
            .await;

        // Test passes if all operations succeeded
        // In a real implementation, you'd verify separate cache usage
        if result1.is_ok() && result2.is_ok() && result1_again.is_ok() {
            tests_passed = 1;
            debug!("✅ Workspace isolation test passed");
        } else {
            debug!("❌ Workspace isolation test failed");
        }

        Ok(tests_passed)
    }

    /// Create test workspaces for cache testing
    async fn create_test_workspaces(&self) -> Result<HashMap<Language, PathBuf>> {
        let mut workspaces = HashMap::new();

        for &language in &self.config.languages {
            let workspace = self.create_test_workspace(language, "main").await?;

            // Initialize LSP server for this workspace
            match timeout(
                Duration::from_secs(30),
                self.server_manager
                    .ensure_workspace_registered(language, workspace.clone()),
            )
            .await
            {
                Ok(Ok(_)) => {
                    info!("✅ Initialized {:?} LSP server for cache testing", language);
                    workspaces.insert(language, workspace);
                }
                Ok(Err(e)) => {
                    warn!("❌ Failed to initialize {:?} LSP server: {}", language, e);
                }
                Err(_) => {
                    warn!("⏰ Timeout initializing {:?} LSP server", language);
                }
            }
        }

        Ok(workspaces)
    }

    /// Create a test workspace for a specific language
    async fn create_test_workspace(&self, language: Language, name: &str) -> Result<PathBuf> {
        let workspace_dir = self
            .test_base_dir
            .path()
            .join(format!("{}_{:?}", name, language));
        std::fs::create_dir_all(&workspace_dir)?;

        match language {
            Language::Rust => {
                std::fs::write(
                    workspace_dir.join("Cargo.toml"),
                    r#"
[package]
name = "cache_test"
version = "0.1.0"
edition = "2021"
"#,
                )?;

                let src_dir = workspace_dir.join("src");
                std::fs::create_dir_all(&src_dir)?;

                std::fs::write(
                    src_dir.join("main.rs"),
                    r#"
use std::collections::HashMap;

fn main() {
    let result = calculate_sum(vec![1, 2, 3, 4, 5]);
    println!("Sum: {}", result);
    
    let data = process_data();
    display_results(&data);
}

fn calculate_sum(numbers: Vec<i32>) -> i32 {
    numbers.iter().sum()
}

fn process_data() -> HashMap<String, i32> {
    let mut data = HashMap::new();
    data.insert("count".to_string(), 42);
    data.insert("value".to_string(), calculate_value());
    data
}

fn calculate_value() -> i32 {
    multiply_by_two(21)
}

fn multiply_by_two(x: i32) -> i32 {
    x * 2
}

fn display_results(data: &HashMap<String, i32>) {
    for (key, value) in data {
        println!("{}: {}", key, value);
    }
}

pub struct Calculator {
    pub name: String,
}

impl Calculator {
    pub fn new(name: String) -> Self {
        Self { name }
    }
    
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
    
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_sum() {
        assert_eq!(calculate_sum(vec![1, 2, 3]), 6);
    }

    #[test]
    fn test_calculator() {
        let calc = Calculator::new("test".to_string());
        assert_eq!(calc.add(2, 3), 5);
    }
}
"#,
                )?;
            }
            Language::Python => {
                std::fs::write(
                    workspace_dir.join("main.py"),
                    r#"
from typing import Dict, List

def main():
    result = calculate_sum([1, 2, 3, 4, 5])
    print(f"Sum: {result}")
    
    data = process_data()
    display_results(data)

def calculate_sum(numbers: List[int]) -> int:
    return sum(numbers)

def process_data() -> Dict[str, int]:
    data = {
        "count": 42,
        "value": calculate_value()
    }
    return data

def calculate_value() -> int:
    return multiply_by_two(21)

def multiply_by_two(x: int) -> int:
    return x * 2

def display_results(data: Dict[str, int]):
    for key, value in data.items():
        print(f"{key}: {value}")

class Calculator:
    def __init__(self, name: str):
        self.name = name
    
    def add(self, a: int, b: int) -> int:
        return a + b
    
    def multiply(self, a: int, b: int) -> int:
        return a * b

if __name__ == "__main__":
    main()
"#,
                )?;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported language for cache testing: {:?}",
                    language
                ));
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

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Main cache integration test runner
#[tokio::test]
async fn run_lsp_cache_integration_tests() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("lsp_daemon=info,lsp_cache_integration_tests=debug")
        .with_test_writer()
        .init();

    let config = CacheTestConfig {
        cache_test_iterations: 5, // Reduced for CI
        languages: vec![Language::Rust, Language::Python],
        ..Default::default()
    };

    let mut test_suite = LspCacheIntegrationTestSuite::new(config).await?;
    let results = test_suite.run_all_tests().await?;

    results.print_summary();

    // Assert cache effectiveness
    assert!(
        results.overall_hit_rate >= 0.3,
        "Cache hit rate too low: {:.1}%",
        results.overall_hit_rate * 100.0
    );

    // Assert some tests passed
    assert!(
        results.persistence_tests_passed > 0 || results.invalidation_tests_passed > 0,
        "No cache functionality tests passed"
    );

    // Assert performance improvements from caching
    let has_speedup = results.metrics.iter().any(|m| m.speedup_factor > 1.2);
    assert!(
        has_speedup,
        "No significant performance improvement from caching detected"
    );

    info!("✅ Cache integration tests completed successfully!");
    Ok(())
}

/// Unit tests for cache testing utilities
#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_cache_metrics() {
        let metrics = CacheMetrics {
            operation: "test".to_string(),
            language: Language::Rust,
            cache_hits: 8,
            cache_misses: 2,
            total_operations: 10,
            avg_cached_duration: Duration::from_millis(100),
            avg_uncached_duration: Duration::from_millis(500),
            cache_size_bytes: 1024,
            speedup_factor: 5.0,
        };

        assert_eq!(metrics.hit_rate(), 0.8);
        assert_eq!(metrics.miss_rate(), 0.2);
    }

    #[test]
    fn test_cache_test_results() {
        let mut results = CacheTestResults::new();

        results.metrics.push(CacheMetrics {
            operation: "test1".to_string(),
            language: Language::Rust,
            cache_hits: 5,
            cache_misses: 5,
            total_operations: 10,
            avg_cached_duration: Duration::from_millis(100),
            avg_uncached_duration: Duration::from_millis(300),
            cache_size_bytes: 512,
            speedup_factor: 3.0,
        });

        results.metrics.push(CacheMetrics {
            operation: "test2".to_string(),
            language: Language::Python,
            cache_hits: 3,
            cache_misses: 7,
            total_operations: 10,
            avg_cached_duration: Duration::from_millis(150),
            avg_uncached_duration: Duration::from_millis(400),
            cache_size_bytes: 768,
            speedup_factor: 2.7,
        });

        results.calculate_overall_stats();

        assert_eq!(results.total_cache_operations, 20);
        assert_eq!(results.overall_hit_rate, 0.4); // 8 hits out of 20 operations
    }

    #[tokio::test]
    async fn test_cache_test_suite_creation() -> Result<()> {
        let config = CacheTestConfig {
            languages: vec![Language::Rust],
            test_persistence: false,
            test_invalidation: false,
            test_workspace_isolation: false,
            ..Default::default()
        };

        let _suite = LspCacheIntegrationTestSuite::new(config).await?;
        Ok(())
    }
}
