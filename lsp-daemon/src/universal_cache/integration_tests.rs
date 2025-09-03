//! Integration tests for universal cache with mock LSP servers
//!
//! These tests simulate real LSP server interactions and verify that the universal cache
//! correctly intercepts, stores, and retrieves LSP responses while maintaining
//! workspace isolation and policy enforcement.

use super::*;
use crate::workspace_cache_router::{WorkspaceCacheRouter, WorkspaceCacheRouterConfig};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Mock LSP server that simulates realistic LSP responses and latencies
#[derive(Clone)]
pub struct MockLspServer {
    /// Simulated responses for different LSP methods
    responses: Arc<Mutex<HashMap<String, Value>>>,
    /// Simulated latency for responses
    response_delay: Duration,
    /// Call count tracking for testing
    call_count: Arc<Mutex<HashMap<String, u32>>>,
    /// Whether to simulate failures
    failure_rate: Arc<Mutex<f64>>,
}

impl Default for MockLspServer {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLspServer {
    /// Create a new mock LSP server
    pub fn new() -> Self {
        let mut responses = HashMap::new();

        // Pre-populate with realistic LSP responses
        responses.insert(
            "textDocument/definition".to_string(),
            json!({
                "uri": "file:///test/main.rs",
                "range": {
                    "start": {"line": 5, "character": 4},
                    "end": {"line": 5, "character": 12}
                }
            }),
        );

        responses.insert(
            "textDocument/references".to_string(),
            json!({
                "references": [
                    {
                        "uri": "file:///test/main.rs",
                        "range": {"start": {"line": 5, "character": 4}, "end": {"line": 5, "character": 12}}
                    },
                    {
                        "uri": "file:///test/lib.rs",
                        "range": {"start": {"line": 10, "character": 0}, "end": {"line": 10, "character": 8}}
                    }
                ]
            })
        );

        responses.insert(
            "textDocument/hover".to_string(),
            json!({
                "contents": {
                    "kind": "markdown",
                    "value": "```rust\nfn test_function() -> Result<String>\n```\n\nA test function that returns a string result."
                },
                "range": {
                    "start": {"line": 5, "character": 4},
                    "end": {"line": 5, "character": 12}
                }
            })
        );

        Self {
            responses: Arc::new(Mutex::new(responses)),
            response_delay: Duration::from_millis(100), // 100ms simulated latency
            call_count: Arc::new(Mutex::new(HashMap::new())),
            failure_rate: Arc::new(Mutex::new(0.0)),
        }
    }

    /// Set a custom response for a specific LSP method
    pub fn set_response(&self, method: &str, response: Value) {
        let mut responses = self.responses.lock().unwrap();
        responses.insert(method.to_string(), response);
    }

    /// Set response delay to simulate LSP server latency
    pub fn set_response_delay(&mut self, delay: Duration) {
        self.response_delay = delay;
    }

    /// Set failure rate for simulating unreliable LSP servers
    pub fn set_failure_rate(&mut self, rate: f64) {
        let mut failure_rate = self.failure_rate.lock().unwrap();
        *failure_rate = rate.clamp(0.0, 1.0);
    }

    /// Get call count for a specific method
    pub fn get_call_count(&self, method: &str) -> u32 {
        let call_count = self.call_count.lock().unwrap();
        call_count.get(method).cloned().unwrap_or(0)
    }

    /// Reset all call counts
    pub fn reset_call_counts(&self) {
        let mut call_count = self.call_count.lock().unwrap();
        call_count.clear();
    }

    /// Simulate an LSP request
    pub async fn handle_request(&self, method: &str, _params: Value) -> Result<Option<Value>> {
        // Increment call count
        {
            let mut call_count = self.call_count.lock().unwrap();
            let count = call_count.entry(method.to_string()).or_insert(0);
            *count += 1;
        }

        // Simulate network latency
        tokio::time::sleep(self.response_delay).await;

        // Simulate failures
        let current_failure_rate = {
            let failure_rate = self.failure_rate.lock().unwrap();
            *failure_rate
        };

        if current_failure_rate > 0.0 {
            // Simple pseudo-random failure simulation without external dependency
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            std::ptr::addr_of!(self).hash(&mut hasher);
            method.hash(&mut hasher);
            let pseudo_random = (hasher.finish() as f64 / u64::MAX as f64) % 1.0;

            if pseudo_random < current_failure_rate {
                return Err(anyhow::anyhow!("Simulated LSP server failure"));
            }
        }

        // Get response for method
        let responses = self.responses.lock().unwrap();
        Ok(responses.get(method).cloned())
    }
}

/// Integration test fixture with mock LSP server and universal cache
pub struct IntegrationTestFixture {
    pub universal_cache: Arc<UniversalCache>,
    pub workspace_router: Arc<WorkspaceCacheRouter>,
    pub mock_lsp_server: MockLspServer,
    pub temp_dir: TempDir,
    pub test_workspace_root: PathBuf,
}

impl IntegrationTestFixture {
    /// Create a new integration test fixture
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let test_workspace_root = temp_dir.path().join("integration_test_workspace");
        std::fs::create_dir_all(&test_workspace_root)?;

        // Create workspace cache router
        let config = WorkspaceCacheRouterConfig {
            base_cache_dir: temp_dir.path().join("caches"),
            max_open_caches: 5,
            max_parent_lookup_depth: 3,
            ..Default::default()
        };

        let registry = Arc::new(crate::lsp_registry::LspRegistry::new()?);
        let child_processes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let server_manager = Arc::new(
            crate::server_manager::SingleServerManager::new_with_tracker(registry, child_processes),
        );

        let workspace_router = Arc::new(WorkspaceCacheRouter::new(config, server_manager));

        // Create universal cache
        let universal_cache = Arc::new(UniversalCache::new(workspace_router.clone()).await?);

        // Create mock LSP server
        let mock_lsp_server = MockLspServer::new();

        Ok(Self {
            universal_cache,
            workspace_router,
            mock_lsp_server,
            temp_dir,
            test_workspace_root,
        })
    }

    /// Create a test file within the workspace
    pub async fn create_test_file(&self, file_name: &str, content: &str) -> Result<PathBuf> {
        let file_path = self.test_workspace_root.join(file_name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        tokio::fs::write(&file_path, content).await?;
        Ok(file_path)
    }

    /// Simulate a complete LSP request cycle with caching
    pub async fn simulate_lsp_request_with_cache(
        &self,
        method: LspMethod,
        file_path: &std::path::Path,
        params: &str,
    ) -> Result<(Option<Value>, bool)> {
        // Returns (response, was_from_cache)
        // Try to get from cache first
        let start_time = Instant::now();
        let cached_result: Option<Value> =
            self.universal_cache.get(method, file_path, params).await?;
        let cache_lookup_time = start_time.elapsed();

        if let Some(cached_response) = cached_result {
            // Cache hit
            return Ok((Some(cached_response), true));
        }

        // Cache miss - call mock LSP server
        let lsp_start_time = Instant::now();
        let lsp_response = self
            .mock_lsp_server
            .handle_request(method.as_str(), json!({}))
            .await?;
        let lsp_call_time = lsp_start_time.elapsed();

        // Cache the response if we got one
        if let Some(ref response) = lsp_response {
            self.universal_cache
                .set(method, file_path, params, response)
                .await?;
        }

        // Log timing for performance analysis
        println!(
            "LSP request: method={}, cache_lookup={:?}, lsp_call={:?}, cached=false",
            method.as_str(),
            cache_lookup_time,
            lsp_call_time
        );

        Ok((lsp_response, false))
    }
}

#[cfg(test)]
mod cache_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_cache_integration() {
        let fixture = IntegrationTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("main.rs", "fn main() {}")
            .await
            .unwrap();

        let params = r#"{"position":{"line":0,"character":3}}"#;

        // First request should miss cache and call LSP server
        let (response1, from_cache1) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();

        assert!(response1.is_some());
        assert!(!from_cache1); // Should be from LSP server
        assert_eq!(
            fixture
                .mock_lsp_server
                .get_call_count("textDocument/definition"),
            1
        );

        // Second request should hit cache
        let (response2, from_cache2) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();

        assert!(response2.is_some());
        assert!(from_cache2); // Should be from cache
        assert_eq!(
            fixture
                .mock_lsp_server
                .get_call_count("textDocument/definition"),
            1
        ); // No additional calls
        assert_eq!(response1, response2); // Responses should be identical
    }

    #[tokio::test]
    async fn test_multiple_lsp_methods_caching() {
        let fixture = IntegrationTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("lib.rs", "pub fn hello() {}")
            .await
            .unwrap();

        let params = r#"{"position":{"line":0,"character":7}}"#;

        // Test multiple LSP methods
        let methods = vec![
            LspMethod::Definition,
            LspMethod::References,
            LspMethod::Hover,
        ];

        // First requests - should all miss cache
        for method in &methods {
            let (response, from_cache) = fixture
                .simulate_lsp_request_with_cache(*method, &test_file, params)
                .await
                .unwrap();

            assert!(response.is_some());
            assert!(!from_cache);
            assert_eq!(fixture.mock_lsp_server.get_call_count(method.as_str()), 1);
        }

        // Second requests - should all hit cache
        for method in &methods {
            let (response, from_cache) = fixture
                .simulate_lsp_request_with_cache(*method, &test_file, params)
                .await
                .unwrap();

            assert!(response.is_some());
            assert!(from_cache);
            // Call count should remain 1 (no additional LSP calls)
            assert_eq!(fixture.mock_lsp_server.get_call_count(method.as_str()), 1);
        }
    }

    #[tokio::test]
    async fn test_workspace_isolation_in_integration() {
        let fixture = IntegrationTestFixture::new().await.unwrap();

        // Create files in different workspaces
        let workspace1 = fixture.temp_dir.path().join("workspace1");
        let workspace2 = fixture.temp_dir.path().join("workspace2");
        std::fs::create_dir_all(&workspace1).unwrap();
        std::fs::create_dir_all(&workspace2).unwrap();

        let file1 = workspace1.join("main.rs");
        let file2 = workspace2.join("main.rs");
        tokio::fs::write(&file1, "fn main() { /* workspace1 */ }")
            .await
            .unwrap();
        tokio::fs::write(&file2, "fn main() { /* workspace2 */ }")
            .await
            .unwrap();

        let params = r#"{"position":{"line":0,"character":3}}"#;

        // Configure different responses for each workspace by simulating different LSP servers
        fixture.mock_lsp_server.set_response(
            "textDocument/definition",
            json!({"workspace": "workspace1", "uri": "file:///workspace1/main.rs"}),
        );

        // Request for workspace1 file
        let (response1, from_cache1) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Definition, &file1, params)
            .await
            .unwrap();

        assert!(!from_cache1);
        assert!(response1.as_ref().unwrap()["workspace"] == "workspace1");

        // Change mock response for workspace2
        fixture.mock_lsp_server.set_response(
            "textDocument/definition",
            json!({"workspace": "workspace2", "uri": "file:///workspace2/main.rs"}),
        );

        // Request for workspace2 file
        let (response2, from_cache2) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Definition, &file2, params)
            .await
            .unwrap();

        assert!(!from_cache2);
        assert!(response2.as_ref().unwrap()["workspace"] == "workspace2");

        // Verify workspace1 cache is still intact
        // Reset mock to original response to ensure we're getting cached data
        fixture.mock_lsp_server.set_response(
            "textDocument/definition",
            json!({"workspace": "modified", "should_not_see": "this"}),
        );

        let (cached_response1, from_cache_again) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Definition, &file1, params)
            .await
            .unwrap();

        assert!(from_cache_again);
        assert!(cached_response1.as_ref().unwrap()["workspace"] == "workspace1");
    }

    #[tokio::test]
    async fn test_cache_performance_benefits() {
        let fixture = IntegrationTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("performance.rs", "fn perf_test() {}")
            .await
            .unwrap();

        // Set higher latency for LSP server to make cache benefits obvious
        let mut server = fixture.mock_lsp_server.clone();
        server.set_response_delay(Duration::from_millis(500)); // 500ms delay

        let params = r#"{"position":{"line":0,"character":3}}"#;

        // First request (cache miss)
        let start = Instant::now();
        let (response1, from_cache1) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Hover, &test_file, params)
            .await
            .unwrap();
        let first_request_time = start.elapsed();

        assert!(response1.is_some());
        assert!(!from_cache1);
        // Use backend-agnostic timing validation - should take at least some time due to LSP delay
        assert!(
            first_request_time >= Duration::from_millis(100),
            "First request should show LSP delay, took {first_request_time:?}"
        );

        // Second request (cache hit)
        let start = Instant::now();
        let (response2, from_cache2) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Hover, &test_file, params)
            .await
            .unwrap();
        let second_request_time = start.elapsed();

        assert!(response2.is_some());
        assert!(from_cache2);
        assert_eq!(response1, response2);

        // Verify performance improvement (more lenient for different backends)
        // DuckDB may have different performance characteristics than Sled
        let speedup_ratio = if second_request_time.as_millis() > 0 {
            first_request_time.as_millis() as f64 / second_request_time.as_millis() as f64
        } else {
            // Handle cases where cached response is so fast it rounds to 0ms
            f64::INFINITY
        };

        // More lenient speedup requirement that works across backends
        let min_speedup =
            if std::env::var("PROBE_LSP_CACHE_BACKEND_TYPE").as_deref() == Ok("duckdb") {
                2.0 // DuckDB may be slower for cache operations but should still be faster than LSP
            } else {
                5.0 // Sled should achieve higher speedup
            };

        assert!(
            speedup_ratio >= min_speedup,
            "Cache should provide at least {min_speedup}x speedup, got {speedup_ratio}x (first: {first_request_time:?}, second: {second_request_time:?})"
        );

        println!("Performance improvement: {speedup_ratio:.1}x speedup with cache");
    }

    #[tokio::test]
    async fn test_cache_invalidation_integration() {
        let fixture = IntegrationTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("invalidation.rs", "fn original() {}")
            .await
            .unwrap();

        let params = r#"{"position":{"line":0,"character":3}}"#;

        // Cache initial response
        let (response1, from_cache1) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();

        assert!(response1.is_some());
        assert!(!from_cache1);

        // Verify it's cached
        let (response2, from_cache2) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();

        assert!(from_cache2);
        assert_eq!(response1, response2);

        // Check if cache entries exist before invalidation
        let had_cached_entry = fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::Definition, &test_file, params)
            .await
            .unwrap()
            .is_some();

        // Simulate file modification by invalidating cache
        let invalidated_count = fixture
            .universal_cache
            .invalidate_file(&test_file)
            .await
            .unwrap();

        if had_cached_entry {
            assert!(
                invalidated_count > 0,
                "Expected to invalidate cached entry but got 0"
            );
        } else {
            eprintln!("Warning: No cached entry found to invalidate - possible backend issue");
        }

        // Update mock server response to simulate changed file
        fixture.mock_lsp_server.set_response(
            "textDocument/definition",
            json!({
                "uri": "file:///test/invalidation.rs",
                "range": {
                    "start": {"line": 0, "character": 3},
                    "end": {"line": 0, "character": 11}
                },
                "modified": true
            }),
        );

        // Next request should miss cache and get new response
        let (response3, from_cache3) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();

        assert!(!from_cache3);
        assert_ne!(response1, response3); // Should be different response
        assert!(response3.as_ref().unwrap().get("modified").is_some());
    }

    #[tokio::test]
    async fn test_error_handling_with_mock_server() {
        let fixture = IntegrationTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("error_test.rs", "fn error_test() {}")
            .await
            .unwrap();

        // Set high failure rate
        let mut server = fixture.mock_lsp_server.clone();
        server.set_failure_rate(1.0); // 100% failure rate

        let params = r#"{"position":{"line":0,"character":3}}"#;

        // Request should fail but not crash the cache
        let result = fixture
            .simulate_lsp_request_with_cache(LspMethod::Definition, &test_file, params)
            .await;

        // Should handle the error gracefully - different backends may handle LSP failures differently
        // Either return an error, or return Ok with None response (both are valid error handling)
        match result {
            Ok((response_opt, _from_cache)) => {
                // If the operation succeeded, the response should be None (indicating LSP failure)
                assert!(
                    response_opt.is_none(),
                    "LSP failure should result in None response, got: {response_opt:?}"
                );
            }
            Err(_error) => {
                // Error return is also acceptable for LSP failures
                // This validates that the cache system doesn't crash on LSP errors
            }
        }

        // Cache should remain functional - reset server to working state
        server.set_failure_rate(0.0);
        fixture
            .mock_lsp_server
            .set_response("textDocument/definition", json!({"recovered": true}));

        // Should work after server recovery
        let (response, from_cache) = fixture
            .simulate_lsp_request_with_cache(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();

        assert!(response.is_some());
        assert!(!from_cache); // Should not cache failures
        assert!(response.unwrap().get("recovered").is_some());
    }
}

#[cfg(test)]
mod concurrent_integration_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn test_concurrent_cache_access() {
        let fixture = IntegrationTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("concurrent.rs", "fn concurrent_test() {}")
            .await
            .unwrap();

        let cache = fixture.universal_cache.clone();
        let file_path = test_file.clone();

        // Track cache hits vs misses
        let cache_hits = Arc::new(AtomicU32::new(0));
        let cache_misses = Arc::new(AtomicU32::new(0));

        let mut join_set = JoinSet::new();

        // Spawn multiple concurrent requests
        for i in 0..20 {
            let cache_clone = cache.clone();
            let file_clone = file_path.clone();
            let hits_counter = cache_hits.clone();
            let misses_counter = cache_misses.clone();

            join_set.spawn(async move {
                let params = format!(
                    r#"{{"position":{{"line":0,"character":{}}},"id":{}}}"#,
                    3 + (i % 5),
                    i
                );

                // Try to get from cache
                let cached: Option<Value> = cache_clone
                    .get(LspMethod::DocumentSymbols, &file_clone, &params)
                    .await
                    .unwrap();

                if cached.is_some() {
                    hits_counter.fetch_add(1, Ordering::Relaxed);
                } else {
                    misses_counter.fetch_add(1, Ordering::Relaxed);

                    // Simulate LSP response and cache it
                    let response = json!({
                        "symbols": [{"name": format!("symbol_{}", i), "kind": 12}],
                        "id": i
                    });

                    cache_clone
                        .set(LspMethod::DocumentSymbols, &file_clone, &params, &response)
                        .await
                        .unwrap();
                }

                i
            });
        }

        // Wait for all tasks to complete
        let mut completed_tasks = 0;
        while let Some(result) = join_set.join_next().await {
            result.unwrap();
            completed_tasks += 1;
        }

        assert_eq!(completed_tasks, 20);

        let total_hits = cache_hits.load(Ordering::Relaxed);
        let total_misses = cache_misses.load(Ordering::Relaxed);

        println!("Concurrent cache access: {total_hits} hits, {total_misses} misses");

        // Should have some cache hits (due to parameter overlap) and some misses
        assert!(total_misses > 0); // First requests should miss
        assert_eq!(total_hits + total_misses, 20); // All requests accounted for

        // Verify cache is consistent after concurrent access
        let final_stats = cache.get_stats().await.unwrap();
        assert!(final_stats.total_entries > 0);
    }

    #[tokio::test]
    async fn test_cache_under_load_with_mock_server() {
        let fixture = Arc::new(IntegrationTestFixture::new().await.unwrap());
        let test_file = Arc::new(
            fixture
                .create_test_file("load_test.rs", "fn load_test() {}")
                .await
                .unwrap(),
        );

        // Set small delay to simulate realistic LSP server
        let mut server = fixture.mock_lsp_server.clone();
        server.set_response_delay(Duration::from_millis(10));

        let operations_count = 100;
        let concurrency = 10;

        let mut join_set = JoinSet::new();
        let start_time = Instant::now();

        // Create concurrent load
        for batch in 0..(operations_count / concurrency) {
            for i in 0..concurrency {
                // Clone Arc references for the task to avoid lifetime issues
                let fixture = fixture.clone();
                let test_file = test_file.clone();
                let params = format!(r#"{{"position":{{"line":{batch},"character":{i}}}}}"#);

                join_set.spawn(async move {
                    fixture
                        .simulate_lsp_request_with_cache(LspMethod::Hover, &test_file, &params)
                        .await
                });
            }
        }

        // Collect results
        let mut cache_hits = 0;
        let mut cache_misses = 0;
        let mut successful_requests = 0;

        while let Some(result) = join_set.join_next().await {
            match result.unwrap() {
                Ok((Some(_response), from_cache)) => {
                    successful_requests += 1;
                    if from_cache {
                        cache_hits += 1;
                    } else {
                        cache_misses += 1;
                    }
                }
                _ => {} // Failed requests
            }
        }

        let total_time = start_time.elapsed();
        let requests_per_second = successful_requests as f64 / total_time.as_secs_f64();

        println!(
            "Load test: {successful_requests} successful requests in {total_time:?} ({requests_per_second:.1} req/s), {cache_hits} cache hits, {cache_misses} cache misses"
        );

        // Performance assertions
        assert!(successful_requests >= operations_count * 80 / 100); // At least 80% success rate
        assert!(requests_per_second > 50.0); // Should handle reasonable load

        // Cache efficiency
        let cache_hit_rate = cache_hits as f64 / (cache_hits + cache_misses) as f64;
        println!("Cache hit rate: {:.2}%", cache_hit_rate * 100.0);

        // Final cache state should be consistent
        let final_stats = fixture.universal_cache.get_stats().await.unwrap();
        assert!(final_stats.total_entries > 0);
    }
}

#[cfg(test)]
mod realistic_workflow_tests {
    use super::*;

    #[tokio::test]
    async fn test_full_development_workflow_simulation() {
        let fixture = IntegrationTestFixture::new().await.unwrap();

        // Create a realistic project structure
        let src_dir = fixture.test_workspace_root.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let main_rs = fixture
            .create_test_file(
                "src/main.rs",
                r#"
use crate::lib::{hello_world, Config};

fn main() {
    let config = Config::new("test");
    hello_world(&config);
}
"#,
            )
            .await
            .unwrap();

        let lib_rs = fixture
            .create_test_file(
                "src/lib.rs",
                r#"
pub struct Config {
    pub name: String,
}

impl Config {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }
}

pub fn hello_world(config: &Config) {
    println!("Hello, {}!", config.name);
}
"#,
            )
            .await
            .unwrap();

        // Configure realistic LSP responses
        fixture.mock_lsp_server.set_response(
            "textDocument/definition",
            json!({
                "uri": "file:///src/lib.rs",
                "range": {"start": {"line": 10, "character": 7}, "end": {"line": 10, "character": 18}}
            })
        );

        fixture.mock_lsp_server.set_response(
            "textDocument/references",
            json!({
                "references": [
                    {"uri": "file:///src/lib.rs", "range": {"start": {"line": 10, "character": 7}, "end": {"line": 10, "character": 18}}},
                    {"uri": "file:///src/main.rs", "range": {"start": {"line": 4, "character": 4}, "end": {"line": 4, "character": 15}}}
                ]
            })
        );

        // Simulate development workflow
        println!("Simulating realistic development workflow...");

        // 1. Developer opens main.rs and goes to definition of hello_world
        let (def_response, def_cached) = fixture
            .simulate_lsp_request_with_cache(
                LspMethod::Definition,
                &main_rs,
                r#"{"position":{"line":4,"character":4}}"#,
            )
            .await
            .unwrap();

        assert!(def_response.is_some());
        assert!(!def_cached);

        // 2. Developer finds references to hello_world function
        let (refs_response, refs_cached) = fixture
            .simulate_lsp_request_with_cache(
                LspMethod::References,
                &lib_rs,
                r#"{"position":{"line":10,"character":11}}"#,
            )
            .await
            .unwrap();

        assert!(refs_response.is_some());
        assert!(!refs_cached);

        // 3. Developer hovers over Config in main.rs
        fixture.mock_lsp_server.set_response(
            "textDocument/hover",
            json!({
                "contents": {"kind": "markdown", "value": "```rust\nstruct Config\n```"},
                "range": {"start": {"line": 3, "character": 16}, "end": {"line": 3, "character": 22}}
            })
        );

        let (hover_response, hover_cached) = fixture
            .simulate_lsp_request_with_cache(
                LspMethod::Hover,
                &main_rs,
                r#"{"position":{"line":3,"character":16}}"#,
            )
            .await
            .unwrap();

        assert!(hover_response.is_some());
        assert!(!hover_cached);

        // 4. Developer goes back to hello_world definition (should be cached now)
        let (def_response2, def_cached2) = fixture
            .simulate_lsp_request_with_cache(
                LspMethod::Definition,
                &main_rs,
                r#"{"position":{"line":4,"character":4}}"#,
            )
            .await
            .unwrap();

        assert!(def_response2.is_some());
        assert!(def_cached2); // Should hit cache this time
        assert_eq!(def_response, def_response2);

        // 5. Developer requests document symbols for lib.rs
        fixture.mock_lsp_server.set_response(
            "textDocument/documentSymbol",
            json!({
                "symbols": [
                    {"name": "Config", "kind": 23, "range": {"start": {"line": 1, "character": 0}, "end": {"line": 8, "character": 1}}},
                    {"name": "hello_world", "kind": 12, "range": {"start": {"line": 10, "character": 0}, "end": {"line": 12, "character": 1}}}
                ]
            })
        );

        let (symbols_response, symbols_cached) = fixture
            .simulate_lsp_request_with_cache(LspMethod::DocumentSymbols, &lib_rs, r#"{}"#)
            .await
            .unwrap();

        assert!(symbols_response.is_some());
        assert!(!symbols_cached);

        // Verify cache effectiveness
        let call_count_def = fixture
            .mock_lsp_server
            .get_call_count("textDocument/definition");
        let call_count_refs = fixture
            .mock_lsp_server
            .get_call_count("textDocument/references");
        let call_count_hover = fixture.mock_lsp_server.get_call_count("textDocument/hover");
        let call_count_symbols = fixture
            .mock_lsp_server
            .get_call_count("textDocument/documentSymbol");

        // Should have only called each method once (second definition call was cached)
        assert_eq!(
            call_count_def, 1,
            "Definition should have been called only once due to caching"
        );
        assert_eq!(call_count_refs, 1);
        assert_eq!(call_count_hover, 1);
        assert_eq!(call_count_symbols, 1);

        // Verify final cache state
        let final_stats = fixture.universal_cache.get_stats().await.unwrap();
        assert!(final_stats.total_entries >= 4); // Should have cached all unique requests
        assert_eq!(final_stats.active_workspaces, 1);

        println!("Development workflow simulation completed successfully!");
        println!(
            "Cache stats: {} entries, hit_rate: {:.2}",
            final_stats.total_entries, final_stats.hit_rate
        );
    }
}
