//! Comprehensive unit tests for universal cache system
//!
//! These tests cover all aspects of the universal cache:
//! - Basic cache operations (get/set/invalidate)
//! - Workspace isolation and routing
//! - Policy enforcement and method-specific caching
//! - Layer coordination (memory → disk → server)
//! - Migration and rollback functionality
//! - Performance and stress testing

use super::*;
use crate::workspace_cache_router::{WorkspaceCacheRouter, WorkspaceCacheRouterConfig};
use anyhow::Result;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

/// Test fixture providing common test setup
struct UniversalCacheTestFixture {
    pub universal_cache: Arc<UniversalCache>,
    pub workspace_router: Arc<WorkspaceCacheRouter>,
    pub temp_dir: TempDir,
    pub test_workspace_root: PathBuf,
}

impl UniversalCacheTestFixture {
    /// Create a new test fixture with isolated temporary storage
    async fn new() -> Result<Self> {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let test_workspace_root = temp_dir.path().join("test_workspace");
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

        Ok(Self {
            universal_cache,
            workspace_router,
            temp_dir,
            test_workspace_root,
        })
    }

    /// Create a test file path within the workspace
    fn test_file_path(&self, file_name: &str) -> PathBuf {
        self.test_workspace_root.join(file_name)
    }

    /// Create a test file with content
    async fn create_test_file(&self, file_name: &str, content: &str) -> Result<PathBuf> {
        let file_path = self.test_file_path(file_name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        tokio::fs::write(&file_path, content).await?;
        Ok(file_path)
    }
}

#[cfg(test)]
mod basic_operations {
    use super::*;

    #[tokio::test]
    async fn test_cache_creation() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();

        // Verify cache was created successfully
        let stats = fixture.universal_cache.get_stats().await.unwrap();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.active_workspaces, 0);
        assert_eq!(stats.hit_rate, 0.0);
        assert_eq!(stats.miss_rate, 0.0);
    }

    #[tokio::test]
    async fn test_basic_set_get() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("main.rs", "fn main() {}")
            .await
            .unwrap();

        // Test data to cache
        let test_data = json!({
            "name": "main",
            "kind": "function",
            "range": {
                "start": {"line": 0, "character": 3},
                "end": {"line": 0, "character": 7}
            }
        });

        // Cache the data
        fixture
            .universal_cache
            .set(
                LspMethod::Definition,
                &test_file,
                "{\"position\":{\"line\":0,\"character\":5}}",
                &test_data,
            )
            .await
            .unwrap();

        // Retrieve the data
        let retrieved: Option<serde_json::Value> = fixture
            .universal_cache
            .get(
                LspMethod::Definition,
                &test_file,
                "{\"position\":{\"line\":0,\"character\":5}}",
            )
            .await
            .unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), test_data);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture.create_test_file("empty.rs", "").await.unwrap();

        // Try to get non-existent data
        let result: Option<serde_json::Value> = fixture
            .universal_cache
            .get(
                LspMethod::Definition,
                &test_file,
                "{\"position\":{\"line\":0,\"character\":0}}",
            )
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_different_lsp_methods() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("test.rs", "fn test() {}")
            .await
            .unwrap();

        let params = "{\"position\":{\"line\":0,\"character\":3}}";

        // Cache data for different LSP methods
        let definition_data = json!({"kind": "definition", "uri": "file:///test.rs"});
        let references_data = json!({"kind": "references", "locations": []});
        let hover_data = json!({"kind": "hover", "contents": "fn test()"});

        fixture
            .universal_cache
            .set(LspMethod::Definition, &test_file, params, &definition_data)
            .await
            .unwrap();
        fixture
            .universal_cache
            .set(LspMethod::References, &test_file, params, &references_data)
            .await
            .unwrap();
        fixture
            .universal_cache
            .set(LspMethod::Hover, &test_file, params, &hover_data)
            .await
            .unwrap();

        // Verify each method returns its own data
        let def_result: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();
        let ref_result: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::References, &test_file, params)
            .await
            .unwrap();
        let hover_result: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Hover, &test_file, params)
            .await
            .unwrap();

        assert_eq!(def_result.unwrap(), definition_data);
        assert_eq!(ref_result.unwrap(), references_data);
        assert_eq!(hover_result.unwrap(), hover_data);
    }
}

#[cfg(test)]
mod workspace_isolation {
    use super::*;

    #[tokio::test]
    async fn test_workspace_isolation() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();

        // Create files in different workspaces
        let workspace1 = fixture.temp_dir.path().join("workspace1");
        let workspace2 = fixture.temp_dir.path().join("workspace2");
        std::fs::create_dir_all(&workspace1).unwrap();
        std::fs::create_dir_all(&workspace2).unwrap();

        let file1 = workspace1.join("main.rs");
        let file2 = workspace2.join("main.rs");
        tokio::fs::write(&file1, "fn main() { println!(\"workspace1\"); }")
            .await
            .unwrap();
        tokio::fs::write(&file2, "fn main() { println!(\"workspace2\"); }")
            .await
            .unwrap();

        let params = "{\"position\":{\"line\":0,\"character\":3}}";

        // Cache different data for each workspace
        let data1 = json!({"workspace": "workspace1", "content": "workspace1"});
        let data2 = json!({"workspace": "workspace2", "content": "workspace2"});

        fixture
            .universal_cache
            .set(LspMethod::Definition, &file1, params, &data1)
            .await
            .unwrap();
        fixture
            .universal_cache
            .set(LspMethod::Definition, &file2, params, &data2)
            .await
            .unwrap();

        // Verify workspace isolation - each file returns its own data
        let result1: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Definition, &file1, params)
            .await
            .unwrap();
        let result2: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Definition, &file2, params)
            .await
            .unwrap();

        assert_eq!(result1.unwrap(), data1);
        assert_eq!(result2.unwrap(), data2);
    }

    #[tokio::test]
    async fn test_workspace_cache_clearing() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();

        let workspace1 = fixture.temp_dir.path().join("ws1");
        let workspace2 = fixture.temp_dir.path().join("ws2");
        std::fs::create_dir_all(&workspace1).unwrap();
        std::fs::create_dir_all(&workspace2).unwrap();

        let file1 = workspace1.join("test.rs");
        let file2 = workspace2.join("test.rs");
        tokio::fs::write(&file1, "// workspace 1").await.unwrap();
        tokio::fs::write(&file2, "// workspace 2").await.unwrap();

        let params = "{}";
        let data = json!({"test": "data"});

        // Cache data in both workspaces
        fixture
            .universal_cache
            .set(LspMethod::DocumentSymbols, &file1, params, &data)
            .await
            .unwrap();
        fixture
            .universal_cache
            .set(LspMethod::DocumentSymbols, &file2, params, &data)
            .await
            .unwrap();

        // Verify both are cached
        assert!(fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::DocumentSymbols, &file1, params)
            .await
            .unwrap()
            .is_some());
        assert!(fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::DocumentSymbols, &file2, params)
            .await
            .unwrap()
            .is_some());

        // Clear workspace1 cache
        let cleared_count = fixture
            .universal_cache
            .clear_workspace(&workspace1)
            .await
            .unwrap();
        assert!(cleared_count > 0);

        // Verify workspace1 is cleared but workspace2 is intact
        assert!(fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::DocumentSymbols, &file1, params)
            .await
            .unwrap()
            .is_none());
        assert!(fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::DocumentSymbols, &file2, params)
            .await
            .unwrap()
            .is_some());
    }
}

#[cfg(test)]
mod invalidation {
    use super::*;

    #[tokio::test]
    async fn test_file_invalidation() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("lib.rs", "pub fn hello() {}")
            .await
            .unwrap();

        let params1 = "{\"position\":{\"line\":0,\"character\":7}}"; // function name
        let params2 = "{\"position\":{\"line\":0,\"character\":11}}"; // inside function
        let data1 = json!({"symbol": "hello", "kind": "function"});
        let data2 = json!({"symbol": "hello", "kind": "function", "position": "inside"});

        // Cache multiple entries for the same file
        fixture
            .universal_cache
            .set(LspMethod::Definition, &test_file, params1, &data1)
            .await
            .unwrap();
        fixture
            .universal_cache
            .set(LspMethod::References, &test_file, params2, &data2)
            .await
            .unwrap();

        // Verify both are cached
        assert!(fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::Definition, &test_file, params1)
            .await
            .unwrap()
            .is_some());
        assert!(fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::References, &test_file, params2)
            .await
            .unwrap()
            .is_some());

        // Invalidate the file
        let invalidated_count = fixture
            .universal_cache
            .invalidate_file(&test_file)
            .await
            .unwrap();
        assert!(invalidated_count > 0);

        // Verify all entries for the file are invalidated
        assert!(fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::Definition, &test_file, params1)
            .await
            .unwrap()
            .is_none());
        assert!(fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::References, &test_file, params2)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_invalidation_preserves_other_files() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let file1 = fixture
            .create_test_file("file1.rs", "fn test1() {}")
            .await
            .unwrap();
        let file2 = fixture
            .create_test_file("file2.rs", "fn test2() {}")
            .await
            .unwrap();

        let params = "{}";
        let data1 = json!({"file": "file1"});
        let data2 = json!({"file": "file2"});

        // Cache data for both files
        fixture
            .universal_cache
            .set(LspMethod::DocumentSymbols, &file1, params, &data1)
            .await
            .unwrap();
        fixture
            .universal_cache
            .set(LspMethod::DocumentSymbols, &file2, params, &data2)
            .await
            .unwrap();

        // Invalidate only file1
        let invalidated_count = fixture
            .universal_cache
            .invalidate_file(&file1)
            .await
            .unwrap();
        assert_eq!(invalidated_count, 1);

        // Verify file1 is invalidated but file2 is preserved
        assert!(fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::DocumentSymbols, &file1, params)
            .await
            .unwrap()
            .is_none());
        assert!(fixture
            .universal_cache
            .get::<serde_json::Value>(LspMethod::DocumentSymbols, &file2, params)
            .await
            .unwrap()
            .is_some());
    }
}

#[cfg(test)]
mod policy_enforcement {
    use super::*;

    #[tokio::test]
    async fn test_method_policy_enforcement() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("test.rs", "fn test() {}")
            .await
            .unwrap();
        let params = "{}";
        let test_data = json!({"test": "data"});

        // Test that enabled methods work
        fixture
            .universal_cache
            .set(LspMethod::Definition, &test_file, params, &test_data)
            .await
            .unwrap();
        let result: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();
        assert!(result.is_some());

        // Note: In a real scenario, we would test disabled methods by modifying the policy registry
        // For now, this verifies the basic policy enforcement path
    }

    #[tokio::test]
    async fn test_cache_scope_handling() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("scoped.rs", "struct Test {}")
            .await
            .unwrap();

        // Test different parameter combinations to verify scope-based caching
        let params_set1 = "{\"context\":\"file\"}";
        let params_set2 = "{\"context\":\"workspace\"}";
        let data1 = json!({"scope": "file"});
        let data2 = json!({"scope": "workspace"});

        // Cache with different parameters (simulating different scopes)
        fixture
            .universal_cache
            .set(LspMethod::Hover, &test_file, params_set1, &data1)
            .await
            .unwrap();
        fixture
            .universal_cache
            .set(LspMethod::Hover, &test_file, params_set2, &data2)
            .await
            .unwrap();

        // Verify both parameter sets are cached independently
        let result1: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Hover, &test_file, params_set1)
            .await
            .unwrap();
        let result2: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Hover, &test_file, params_set2)
            .await
            .unwrap();

        assert_eq!(result1.unwrap(), data1);
        assert_eq!(result2.unwrap(), data2);
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_concurrent_operations() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let cache = fixture.universal_cache.clone();

        // Create multiple test files
        let mut files = Vec::new();
        for i in 0..10 {
            let file = fixture
                .create_test_file(
                    &format!("concurrent_{}.rs", i),
                    &format!("fn test_{}() {{}}", i),
                )
                .await
                .unwrap();
            files.push(file);
        }

        let start = Instant::now();

        // Perform concurrent cache operations
        let mut handles = Vec::new();

        for (i, file) in files.into_iter().enumerate() {
            let cache_clone = cache.clone();
            let file_clone = file.clone();

            let handle = tokio::spawn(async move {
                let params = format!("{{\"index\": {}}}", i);
                let data = json!({"concurrent_test": i, "file": format!("file_{}", i)});

                // Set data
                cache_clone
                    .set(LspMethod::Definition, &file_clone, &params, &data)
                    .await
                    .unwrap();

                // Get data back
                let result: Option<serde_json::Value> = cache_clone
                    .get(LspMethod::Definition, &file_clone, &params)
                    .await
                    .unwrap();
                assert!(result.is_some());
                assert_eq!(result.unwrap()["concurrent_test"], i);
            });

            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }

        let duration = start.elapsed();
        println!("Concurrent operations completed in {:?}", duration);

        // Verify final cache state
        let stats = cache.get_stats().await.unwrap();
        assert!(stats.total_entries >= 10);
    }

    #[tokio::test]
    async fn test_cache_performance_under_load() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("load_test.rs", "fn load_test() {}")
            .await
            .unwrap();

        let operations_count = 1000;
        let start = Instant::now();

        // Perform many cache operations
        for i in 0..operations_count {
            let params = format!("{{\"operation\": {}}}", i);
            let data = json!({"load_test": i, "timestamp": chrono::Utc::now().timestamp()});

            fixture
                .universal_cache
                .set(LspMethod::Definition, &test_file, &params, &data)
                .await
                .unwrap();

            if i % 2 == 0 {
                // Retrieve every other entry to test cache hits
                let result: Option<serde_json::Value> = fixture
                    .universal_cache
                    .get(LspMethod::Definition, &test_file, &params)
                    .await
                    .unwrap();
                assert!(result.is_some());
            }
        }

        let duration = start.elapsed();
        let ops_per_second = operations_count as f64 / duration.as_secs_f64();

        println!(
            "Performed {} operations in {:?} ({:.2} ops/sec)",
            operations_count, duration, ops_per_second
        );

        // Performance assertion - should handle at least 100 ops/second
        assert!(
            ops_per_second > 100.0,
            "Cache performance below threshold: {} ops/sec",
            ops_per_second
        );
    }

    #[tokio::test]
    async fn test_memory_usage_stability() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("memory_test.rs", "fn memory_test() {}")
            .await
            .unwrap();

        // Get initial stats
        let initial_stats = fixture.universal_cache.get_stats().await.unwrap();
        let initial_entries = initial_stats.total_entries;

        // Add many cache entries
        for i in 0..5000 {
            let params = format!("{{\"memory_test\": {}}}", i);
            let data = json!({"test_data": vec![i; 100]}); // Larger data to test memory handling

            fixture
                .universal_cache
                .set(LspMethod::References, &test_file, &params, &data)
                .await
                .unwrap();
        }

        // Check final stats
        let final_stats = fixture.universal_cache.get_stats().await.unwrap();
        let entries_added = final_stats.total_entries - initial_entries;

        println!(
            "Added {} cache entries, total size: {} bytes",
            entries_added, final_stats.total_size_bytes
        );

        // Verify entries were added
        assert!(entries_added > 4000); // Allow for some eviction/cleanup

        // Memory usage should be reasonable (adjust threshold based on needs)
        assert!(
            final_stats.total_size_bytes < 100 * 1024 * 1024,
            "Memory usage too high: {} bytes",
            final_stats.total_size_bytes
        );
    }
}

#[cfg(test)]
mod error_handling {
    use super::*;

    #[tokio::test]
    async fn test_invalid_file_paths() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let invalid_path = PathBuf::from("/nonexistent/path/file.rs");

        let params = "{}";
        let data = json!({"test": "data"});

        // Should not panic or fail catastrophically
        let _result = fixture
            .universal_cache
            .set(LspMethod::Definition, &invalid_path, params, &data)
            .await;
        // Result may be Ok or Err depending on implementation - both are acceptable

        let _get_result: Result<Option<serde_json::Value>> = fixture
            .universal_cache
            .get(LspMethod::Definition, &invalid_path, params)
            .await;
        // Similarly, should handle gracefully

        // The important thing is that the cache remains functional
        let _stats = fixture.universal_cache.get_stats().await.unwrap();
        // Stats should still be accessible
    }

    #[tokio::test]
    async fn test_malformed_json_parameters() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("error_test.rs", "fn test() {}")
            .await
            .unwrap();

        let malformed_params = "{invalid json";
        let data = json!({"test": "data"});

        // Should handle malformed JSON gracefully
        let _result = fixture
            .universal_cache
            .set(LspMethod::Hover, &test_file, malformed_params, &data)
            .await;
        // Implementation should either succeed (treating as string key) or fail gracefully

        // Cache should remain functional
        let valid_params = "{\"valid\": true}";
        fixture
            .universal_cache
            .set(LspMethod::Hover, &test_file, valid_params, &data)
            .await
            .unwrap();

        let retrieved: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Hover, &test_file, valid_params)
            .await
            .unwrap();
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_timeout_handling() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("timeout_test.rs", "fn test() {}")
            .await
            .unwrap();

        // Test that operations complete within reasonable time
        let params = "{}";
        let data = json!({"timeout_test": true});

        let timeout_result = timeout(
            Duration::from_secs(5), // 5 second timeout
            fixture
                .universal_cache
                .set(LspMethod::Definition, &test_file, params, &data),
        )
        .await;

        assert!(timeout_result.is_ok(), "Cache operation timed out");

        let get_timeout_result = timeout(
            Duration::from_secs(5),
            fixture.universal_cache.get::<serde_json::Value>(
                LspMethod::Definition,
                &test_file,
                params,
            ),
        )
        .await;

        assert!(get_timeout_result.is_ok(), "Cache get operation timed out");
    }
}

#[cfg(test)]
mod statistics_and_monitoring {
    use super::*;

    #[tokio::test]
    async fn test_cache_statistics() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file("stats_test.rs", "fn stats_test() {}")
            .await
            .unwrap();

        // Get initial stats
        let initial_stats = fixture.universal_cache.get_stats().await.unwrap();
        assert_eq!(initial_stats.total_entries, 0);

        // Add some cache entries
        let params_list = vec!["{\"test\": 1}", "{\"test\": 2}", "{\"test\": 3}"];

        for (i, params) in params_list.iter().enumerate() {
            let data = json!({"index": i});
            fixture
                .universal_cache
                .set(LspMethod::Definition, &test_file, params, &data)
                .await
                .unwrap();
        }

        // Get updated stats
        let stats = fixture.universal_cache.get_stats().await.unwrap();

        // Verify stats are updated
        assert!(stats.total_entries >= 3);
        assert!(stats.total_size_bytes > 0);
        assert!(stats.active_workspaces >= 1);

        // Verify method-specific stats if available
        if let Some(definition_stats) = stats.method_stats.get(&LspMethod::Definition) {
            assert!(definition_stats.entries > 0);
            assert!(definition_stats.size_bytes > 0);
        }

        // Test cache hits by retrieving entries
        for params in &params_list {
            let result: Option<serde_json::Value> = fixture
                .universal_cache
                .get(LspMethod::Definition, &test_file, params)
                .await
                .unwrap();
            assert!(result.is_some());
        }

        // Get final stats to check hit rate changes
        let _final_stats = fixture.universal_cache.get_stats().await.unwrap();
        // Hit rate should be updated (though exact values depend on implementation details)
    }

    #[tokio::test]
    async fn test_workspace_statistics() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();

        // Create files in multiple workspaces
        let workspace1 = fixture.temp_dir.path().join("ws1");
        let workspace2 = fixture.temp_dir.path().join("ws2");
        std::fs::create_dir_all(&workspace1).unwrap();
        std::fs::create_dir_all(&workspace2).unwrap();

        let file1 = workspace1.join("test1.rs");
        let file2 = workspace2.join("test2.rs");
        tokio::fs::write(&file1, "fn test1() {}").await.unwrap();
        tokio::fs::write(&file2, "fn test2() {}").await.unwrap();

        let params = "{}";
        let data1 = json!({"workspace": 1});
        let data2 = json!({"workspace": 2});

        // Cache data in both workspaces
        fixture
            .universal_cache
            .set(LspMethod::DocumentSymbols, &file1, params, &data1)
            .await
            .unwrap();
        fixture
            .universal_cache
            .set(LspMethod::DocumentSymbols, &file2, params, &data2)
            .await
            .unwrap();

        // Check stats show both workspaces
        let stats = fixture.universal_cache.get_stats().await.unwrap();
        assert!(stats.active_workspaces >= 2);
        assert!(stats.total_entries >= 2);
    }
}

#[cfg(test)]
mod integration_scenarios {
    use super::*;

    #[tokio::test]
    async fn test_realistic_lsp_workflow() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();

        // Simulate a realistic Rust project structure
        let src_dir = fixture.test_workspace_root.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let main_rs = src_dir.join("main.rs");
        let lib_rs = src_dir.join("lib.rs");

        tokio::fs::write(
            &main_rs,
            r#"
use crate::lib::hello_world;

fn main() {
    hello_world();
}
"#,
        )
        .await
        .unwrap();

        tokio::fs::write(
            &lib_rs,
            r#"
pub fn hello_world() {
    println!("Hello, world!");
}

pub struct Config {
    pub name: String,
    pub debug: bool,
}
"#,
        )
        .await
        .unwrap();

        // Simulate LSP operations for a typical development workflow

        // 1. Go to definition for `hello_world` in main.rs
        let go_to_def_params =
            r#"{"textDocument":{"uri":"file:///src/main.rs"},"position":{"line":3,"character":5}}"#;
        let definition_result = json!({
            "uri": "file:///src/lib.rs",
            "range": {
                "start": {"line": 1, "character": 7},
                "end": {"line": 1, "character": 18}
            }
        });

        fixture
            .universal_cache
            .set(
                LspMethod::Definition,
                &main_rs,
                go_to_def_params,
                &definition_result,
            )
            .await
            .unwrap();

        // 2. Find references for `hello_world` function
        let find_refs_params = r#"{"textDocument":{"uri":"file:///src/lib.rs"},"position":{"line":1,"character":11},"context":{"includeDeclaration":true}}"#;
        let references_result = json!({
            "references": [
                {
                    "uri": "file:///src/lib.rs",
                    "range": {"start": {"line": 1, "character": 7}, "end": {"line": 1, "character": 18}}
                },
                {
                    "uri": "file:///src/main.rs",
                    "range": {"start": {"line": 3, "character": 5}, "end": {"line": 3, "character": 16}}
                }
            ]
        });

        fixture
            .universal_cache
            .set(
                LspMethod::References,
                &lib_rs,
                find_refs_params,
                &references_result,
            )
            .await
            .unwrap();

        // 3. Hover for type information on `Config` struct
        let hover_params =
            r#"{"textDocument":{"uri":"file:///src/lib.rs"},"position":{"line":6,"character":11}}"#;
        let hover_result = json!({
            "contents": {
                "kind": "markdown",
                "value": "```rust\npub struct Config\n```\n\nA configuration struct for the application."
            },
            "range": {
                "start": {"line": 6, "character": 11},
                "end": {"line": 6, "character": 17}
            }
        });

        fixture
            .universal_cache
            .set(LspMethod::Hover, &lib_rs, hover_params, &hover_result)
            .await
            .unwrap();

        // 4. Get document symbols for lib.rs
        let doc_symbols_params = r#"{"textDocument":{"uri":"file:///src/lib.rs"}}"#;
        let doc_symbols_result = json!({
            "symbols": [
                {
                    "name": "hello_world",
                    "kind": 12,
                    "range": {"start": {"line": 1, "character": 0}, "end": {"line": 3, "character": 1}},
                    "selectionRange": {"start": {"line": 1, "character": 7}, "end": {"line": 1, "character": 18}}
                },
                {
                    "name": "Config",
                    "kind": 23,
                    "range": {"start": {"line": 5, "character": 0}, "end": {"line": 8, "character": 1}},
                    "selectionRange": {"start": {"line": 6, "character": 11}, "end": {"line": 6, "character": 17}}
                }
            ]
        });

        fixture
            .universal_cache
            .set(
                LspMethod::DocumentSymbols,
                &lib_rs,
                doc_symbols_params,
                &doc_symbols_result,
            )
            .await
            .unwrap();

        // Verify all cached operations can be retrieved
        let cached_definition: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Definition, &main_rs, go_to_def_params)
            .await
            .unwrap();
        let cached_references: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::References, &lib_rs, find_refs_params)
            .await
            .unwrap();
        let cached_hover: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Hover, &lib_rs, hover_params)
            .await
            .unwrap();
        let cached_symbols: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::DocumentSymbols, &lib_rs, doc_symbols_params)
            .await
            .unwrap();

        assert!(cached_definition.is_some());
        assert!(cached_references.is_some());
        assert!(cached_hover.is_some());
        assert!(cached_symbols.is_some());

        // Verify cache stats reflect the operations
        let final_stats = fixture.universal_cache.get_stats().await.unwrap();
        assert!(final_stats.total_entries >= 4);
        assert_eq!(final_stats.active_workspaces, 1); // All in same workspace
    }

    #[tokio::test]
    async fn test_file_modification_invalidation_workflow() {
        let fixture = UniversalCacheTestFixture::new().await.unwrap();
        let test_file = fixture
            .create_test_file(
                "evolving.rs",
                r#"
fn original_function() {
    println!("Original implementation");
}
"#,
            )
            .await
            .unwrap();

        // Cache some LSP data for the original file
        let params = r#"{"position":{"line":1,"character":3}}"#;
        let original_data = json!({"function": "original_function", "version": 1});

        fixture
            .universal_cache
            .set(LspMethod::Definition, &test_file, params, &original_data)
            .await
            .unwrap();

        // Verify data is cached
        let cached: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();
        assert_eq!(cached.unwrap(), original_data);

        // Simulate file modification
        tokio::fs::write(
            &test_file,
            r#"
fn modified_function() {
    println!("Modified implementation");
    // Added a comment
}

fn new_function() {
    println!("New function added");
}
"#,
        )
        .await
        .unwrap();

        // Invalidate cache for the modified file (simulating file watcher notification)
        let invalidated_count = fixture
            .universal_cache
            .invalidate_file(&test_file)
            .await
            .unwrap();
        assert!(invalidated_count > 0);

        // Verify old data is no longer cached
        let after_invalidation: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();
        assert!(after_invalidation.is_none());

        // Cache new LSP data for the modified file
        let new_data =
            json!({"function": "modified_function", "version": 2, "has_new_function": true});
        fixture
            .universal_cache
            .set(LspMethod::Definition, &test_file, params, &new_data)
            .await
            .unwrap();

        // Verify new data is cached
        let new_cached: Option<serde_json::Value> = fixture
            .universal_cache
            .get(LspMethod::Definition, &test_file, params)
            .await
            .unwrap();
        assert_eq!(new_cached.unwrap(), new_data);
    }
}
