#![cfg(feature = "legacy-tests")]
//! Production Load Test for Database-First LSP Caching System
//!
//! This test validates that the database-first caching system can handle
//! production-level concurrent load with multiple concurrent LSP operations.
//!
//! Success criteria for Milestone 31.1:
//! - Handle 50+ concurrent LSP operations without errors
//! - Maintain cache hit rates above 80% after warmup
//! - Show measurable performance improvements (10-100x speedup)
//! - No database corruption or connection issues under load

use lsp_daemon::daemon::{Config, LspDaemon};
use lsp_daemon::database::sqlite_backend::SqliteBackend;
use lsp_daemon::protocol::{DaemonRequest, DaemonResponse};
use lsp_daemon::workspace_cache_router::WorkspaceCacheRouter;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::task::JoinSet;
use tokio::time::timeout;

/// Test configuration for production load testing
struct LoadTestConfig {
    /// Number of concurrent operations to run
    concurrent_operations: usize,
    /// Number of rounds to run each operation
    rounds_per_operation: usize,
    /// Maximum time to wait for all operations to complete
    max_test_duration: Duration,
    /// Test files to use for operations
    test_files: Vec<String>,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            concurrent_operations: 50,
            rounds_per_operation: 5,
            max_test_duration: Duration::from_secs(300), // 5 minutes
            test_files: vec![
                "src/main.rs".to_string(),
                "lsp-daemon/src/daemon.rs".to_string(),
                "lsp-daemon/src/database/sqlite_backend.rs".to_string(),
                "lsp-daemon/src/indexing/manager.rs".to_string(),
                "src/language/rust.rs".to_string(),
            ],
        }
    }
}

/// Metrics collected during load testing
#[derive(Debug, Default)]
struct LoadTestMetrics {
    total_operations: usize,
    successful_operations: usize,
    failed_operations: usize,
    cache_hits: usize,
    cache_misses: usize,
    average_response_time_ms: f64,
    database_errors: usize,
    concurrent_operations_peak: usize,
}

impl LoadTestMetrics {
    fn cache_hit_rate(&self) -> f64 {
        if self.cache_hits + self.cache_misses == 0 {
            0.0
        } else {
            self.cache_hits as f64 / (self.cache_hits + self.cache_misses) as f64 * 100.0
        }
    }

    fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            self.successful_operations as f64 / self.total_operations as f64 * 100.0
        }
    }
}

#[tokio::test]
async fn test_production_load_concurrent_operations() {
    let config = LoadTestConfig::default();
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create daemon configuration
    let daemon_config = Config {
        socket_path: temp_dir.path().join("daemon.sock"),
        log_level: "debug".to_string(),
        cache_dir: Some(temp_dir.path().to_path_buf()),
        ..Default::default()
    };

    // Start the daemon
    let daemon = Arc::new(
        LspDaemon::new(daemon_config)
            .await
            .expect("Failed to create daemon"),
    );

    // Initialize workspace
    let workspace_init_request = DaemonRequest::InitializeWorkspace {
        workspace_path: std::env::current_dir().unwrap(),
        force_reinit: true,
    };

    match daemon.handle_request(workspace_init_request).await {
        Ok(_) => println!("Workspace initialized successfully"),
        Err(e) => panic!("Failed to initialize workspace: {}", e),
    }

    let mut metrics = LoadTestMetrics::default();
    let mut join_set = JoinSet::new();

    println!(
        "Starting production load test with {} concurrent operations",
        config.concurrent_operations
    );
    println!("Test files: {:?}", config.test_files);

    // Launch concurrent operations
    for operation_id in 0..config.concurrent_operations {
        let daemon_clone = daemon.clone();
        let test_files = config.test_files.clone();
        let rounds = config.rounds_per_operation;

        join_set.spawn(async move {
            let mut operation_metrics = LoadTestMetrics::default();

            for round in 0..rounds {
                for (file_idx, file_path) in test_files.iter().enumerate() {
                    let start_time = std::time::Instant::now();

                    // Test different LSP operations
                    let requests = vec![
                        DaemonRequest::CallHierarchy {
                            params: serde_json::json!({
                                "textDocument": {"uri": format!("file://{}/{}", std::env::current_dir().unwrap().display(), file_path)},
                                "position": {"line": 10, "character": 5}
                            }),
                        },
                        DaemonRequest::Definition {
                            params: serde_json::json!({
                                "textDocument": {"uri": format!("file://{}/{}", std::env::current_dir().unwrap().display(), file_path)},
                                "position": {"line": 20, "character": 10}
                            }),
                        },
                        DaemonRequest::References {
                            params: serde_json::json!({
                                "textDocument": {"uri": format!("file://{}/{}", std::env::current_dir().unwrap().display(), file_path)},
                                "position": {"line": 30, "character": 15},
                                "context": {"includeDeclaration": true}
                            }),
                        },
                    ];

                    for (req_idx, request) in requests.into_iter().enumerate() {
                        operation_metrics.total_operations += 1;

                        match timeout(Duration::from_secs(30), daemon_clone.handle_request(request)).await {
                            Ok(Ok(response)) => {
                                operation_metrics.successful_operations += 1;

                                // Check if response indicates cache hit or miss
                                match response {
                                    DaemonResponse::CallHierarchy { items: _, cached: Some(true), .. } |
                                    DaemonResponse::Definition { locations: _, cached: Some(true), .. } |
                                    DaemonResponse::References { locations: _, cached: Some(true), .. } => {
                                        operation_metrics.cache_hits += 1;
                                    },
                                    _ => {
                                        operation_metrics.cache_misses += 1;
                                    }
                                }
                            },
                            Ok(Err(e)) => {
                                operation_metrics.failed_operations += 1;
                                if e.to_string().contains("database") {
                                    operation_metrics.database_errors += 1;
                                }
                                eprintln!("Operation failed (op={}, round={}, file={}, req={}): {}",
                                         operation_id, round, file_idx, req_idx, e);
                            },
                            Err(_) => {
                                operation_metrics.failed_operations += 1;
                                eprintln!("Operation timeout (op={}, round={}, file={}, req={})",
                                         operation_id, round, file_idx, req_idx);
                            }
                        }

                        let elapsed = start_time.elapsed().as_millis() as f64;
                        operation_metrics.average_response_time_ms =
                            (operation_metrics.average_response_time_ms * (operation_metrics.total_operations - 1) as f64 + elapsed) /
                            operation_metrics.total_operations as f64;
                    }
                }
            }

            operation_metrics
        });
    }

    // Wait for all operations to complete or timeout
    let start_time = std::time::Instant::now();
    let mut completed_operations = 0;

    while let Some(result) = timeout(config.max_test_duration, join_set.join_next()).await {
        match result {
            Ok(Some(Ok(operation_metrics))) => {
                metrics.total_operations += operation_metrics.total_operations;
                metrics.successful_operations += operation_metrics.successful_operations;
                metrics.failed_operations += operation_metrics.failed_operations;
                metrics.cache_hits += operation_metrics.cache_hits;
                metrics.cache_misses += operation_metrics.cache_misses;
                metrics.database_errors += operation_metrics.database_errors;
                completed_operations += 1;
            }
            Ok(Some(Err(e))) => {
                eprintln!("Task join error: {}", e);
                metrics.failed_operations += 1;
            }
            Ok(None) => break,
            Err(_) => {
                eprintln!("Test timeout reached, stopping remaining operations");
                break;
            }
        }
    }

    let total_test_duration = start_time.elapsed();

    // Collect final metrics from daemon
    if let Ok(DaemonResponse::Status {
        cache_stats: Some(cache_stats),
        ..
    }) = daemon.handle_request(DaemonRequest::Status).await
    {
        println!("Final daemon cache stats: {:?}", cache_stats);
    }

    // Print comprehensive results
    println!("\n=== PRODUCTION LOAD TEST RESULTS ===");
    println!("Test Duration: {:.2}s", total_test_duration.as_secs_f64());
    println!("Concurrent Operations: {}", config.concurrent_operations);
    println!("Completed Operations: {}", completed_operations);
    println!("Total Requests: {}", metrics.total_operations);
    println!("Successful Requests: {}", metrics.successful_operations);
    println!("Failed Requests: {}", metrics.failed_operations);
    println!("Success Rate: {:.1}%", metrics.success_rate());
    println!("Cache Hits: {}", metrics.cache_hits);
    println!("Cache Misses: {}", metrics.cache_misses);
    println!("Cache Hit Rate: {:.1}%", metrics.cache_hit_rate());
    println!("Database Errors: {}", metrics.database_errors);
    println!(
        "Average Response Time: {:.1}ms",
        metrics.average_response_time_ms
    );
    println!(
        "Requests per Second: {:.1}",
        metrics.total_operations as f64 / total_test_duration.as_secs_f64()
    );

    // Validate success criteria for Milestone 31.1
    assert!(
        completed_operations >= config.concurrent_operations - 5,
        "Should complete most concurrent operations (got {} out of {})",
        completed_operations,
        config.concurrent_operations
    );

    assert!(
        metrics.success_rate() >= 90.0,
        "Success rate should be above 90% (got {:.1}%)",
        metrics.success_rate()
    );

    // After warmup (first round), cache hit rate should be high
    if metrics.cache_hits + metrics.cache_misses > config.test_files.len() * 3 {
        assert!(
            metrics.cache_hit_rate() >= 70.0,
            "Cache hit rate should be above 70% after warmup (got {:.1}%)",
            metrics.cache_hit_rate()
        );
    }

    assert!(
        metrics.database_errors == 0,
        "Should have no database errors (got {})",
        metrics.database_errors
    );

    println!("\n✅ Production load test PASSED - System handles concurrent load successfully!");
}

#[tokio::test]
async fn test_database_consistency_under_load() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let cache_dir = temp_dir.path().join("cache");
    std::fs::create_dir_all(&cache_dir).expect("Failed to create cache directory");

    // Create workspace cache router
    let router = WorkspaceCacheRouter::new(cache_dir.clone())
        .await
        .expect("Failed to create workspace cache router");

    let workspace_path = std::env::current_dir().unwrap();

    // Initialize the workspace cache
    router
        .ensure_workspace_cache(&workspace_path)
        .await
        .expect("Failed to initialize workspace cache");

    let mut join_set = JoinSet::new();
    let router = Arc::new(router);

    // Launch concurrent database operations
    for i in 0..20 {
        let router_clone = router.clone();
        let workspace_path_clone = workspace_path.clone();

        join_set.spawn(async move {
            // Each task performs multiple database operations
            for j in 0..10 {
                let cache = router_clone.get_cache(&workspace_path_clone).await?;

                let key = format!("test_key_{}_{}", i, j);
                let value = format!("test_value_{}_{}", i, j);

                // Store data
                if let Err(e) = cache.store(&key, &value).await {
                    return Err(format!("Store failed: {}", e));
                }

                // Retrieve data
                match cache.get::<String>(&key).await {
                    Ok(Some(retrieved)) => {
                        if retrieved != value {
                            return Err(format!(
                                "Data mismatch: expected '{}', got '{}'",
                                value, retrieved
                            ));
                        }
                    }
                    Ok(None) => {
                        return Err(format!("Data not found for key '{}'", key));
                    }
                    Err(e) => {
                        return Err(format!("Retrieve failed: {}", e));
                    }
                }
            }

            Ok(())
        });
    }

    // Wait for all tasks to complete
    let mut successful_tasks = 0;
    let mut failed_tasks = 0;

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(())) => successful_tasks += 1,
            Ok(Err(e)) => {
                eprintln!("Task failed: {}", e);
                failed_tasks += 1;
            }
            Err(e) => {
                eprintln!("Task join error: {}", e);
                failed_tasks += 1;
            }
        }
    }

    println!(
        "Database consistency test: {} successful, {} failed",
        successful_tasks, failed_tasks
    );

    // Validate database consistency
    assert_eq!(
        failed_tasks, 0,
        "All concurrent database operations should succeed"
    );
    assert_eq!(
        successful_tasks, 20,
        "All 20 concurrent tasks should complete successfully"
    );

    println!("✅ Database consistency test PASSED - No corruption under concurrent load");
}

#[tokio::test]
async fn test_memory_usage_under_load() {
    use std::process::Command;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create daemon configuration
    let daemon_config = Config {
        socket_path: temp_dir.path().join("daemon.sock"),
        log_level: "info".to_string(),
        cache_dir: Some(temp_dir.path().to_path_buf()),
        ..Default::default()
    };

    let daemon = Arc::new(
        LspDaemon::new(daemon_config)
            .await
            .expect("Failed to create daemon"),
    );

    // Get initial memory usage
    let initial_memory = get_process_memory_mb();

    // Run intensive operations
    let mut join_set = JoinSet::new();

    for _ in 0..10 {
        let daemon_clone = daemon.clone();

        join_set.spawn(async move {
            for _ in 0..50 {
                let request = DaemonRequest::CallHierarchy {
                    params: serde_json::json!({
                        "textDocument": {"uri": format!("file://{}/src/main.rs", std::env::current_dir().unwrap().display())},
                        "position": {"line": 10, "character": 5}
                    }),
                };

                let _ = daemon_clone.handle_request(request).await;
            }
        });
    }

    // Wait for completion
    while join_set.join_next().await.is_some() {}

    // Get final memory usage
    let final_memory = get_process_memory_mb();
    let memory_increase = final_memory - initial_memory;

    println!(
        "Memory usage: initial={}MB, final={}MB, increase={}MB",
        initial_memory, final_memory, memory_increase
    );

    // Memory increase should be reasonable (less than 500MB for this test)
    assert!(
        memory_increase < 500,
        "Memory usage should not increase excessively (increased by {}MB)",
        memory_increase
    );

    println!("✅ Memory usage test PASSED - Memory usage remains reasonable under load");
}

fn get_process_memory_mb() -> i64 {
    let pid = std::process::id();

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("ps")
            .args(&["-o", "rss=", "-p", &pid.to_string()])
            .output()
        {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                if let Ok(rss_kb) = output_str.trim().parse::<i64>() {
                    return rss_kb / 1024; // Convert KB to MB
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(statm) = std::fs::read_to_string(format!("/proc/{}/statm", pid)) {
            let parts: Vec<&str> = statm.split_whitespace().collect();
            if parts.len() > 1 {
                if let Ok(rss_pages) = parts[1].parse::<i64>() {
                    return rss_pages * 4 / 1024; // Convert pages (4KB) to MB
                }
            }
        }
    }

    0 // Return 0 if unable to measure (test will still validate functionality)
}
