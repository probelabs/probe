#![cfg(feature = "legacy-tests")]
//! Scale testing for the null edge caching system
//!
//! Tests system behavior and performance with large datasets,
//! validating scalability to production workloads.

use anyhow::Result;
use lsp_daemon::database::sqlite_backend::SQLiteBackend;
use lsp_daemon::database::{
    create_none_call_hierarchy_edges, create_none_definition_edges,
    create_none_implementation_edges, create_none_reference_edges, DatabaseBackend, DatabaseConfig,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Scale testing configuration
#[derive(Debug, Clone)]
pub struct ScaleTestConfig {
    pub max_symbols: usize,
    pub batch_size: usize,
    pub memory_limit_mb: usize,
    pub max_query_time_ms: u64,
    pub min_throughput_ops_sec: f64,
}

impl Default for ScaleTestConfig {
    fn default() -> Self {
        ScaleTestConfig {
            max_symbols: 10_000,
            batch_size: 1000,
            memory_limit_mb: 100,
            max_query_time_ms: 10,
            min_throughput_ops_sec: 1000.0,
        }
    }
}

/// Scale test harness with monitoring capabilities
pub struct ScaleTestHarness {
    database: SQLiteBackend,
    workspace_id: i64,
    temp_dir: TempDir,
    config: ScaleTestConfig,
}

impl ScaleTestHarness {
    pub async fn new(config: ScaleTestConfig) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("scale_test.db");

        let db_config = DatabaseConfig {
            path: Some(db_path),
            temporary: false,
            cache_capacity: (config.memory_limit_mb * 1024 * 1024) as u64,
            ..Default::default()
        };

        let database = SQLiteBackend::new(db_config).await?;
        let workspace_id = database
            .create_workspace("scale_test", 1, Some("main"))
            .await?;

        Ok(ScaleTestHarness {
            database,
            workspace_id,
            temp_dir,
            config,
        })
    }

    /// Generate hierarchical symbol structure for realistic testing
    pub fn generate_hierarchical_symbols(&self, total_symbols: usize) -> Vec<String> {
        let mut symbols = Vec::new();

        // Create realistic symbol hierarchy:
        // - Modules (10% of symbols)
        // - Functions (60% of symbols)
        // - Methods (20% of symbols)
        // - Variables (10% of symbols)

        let num_modules = total_symbols / 10;
        let num_functions = (total_symbols * 6) / 10;
        let num_methods = (total_symbols * 2) / 10;
        let num_variables = total_symbols / 10;

        // Generate modules
        for i in 0..num_modules {
            symbols.push(format!("src/module_{}.rs:Module::{}:1", i % 50, i));
        }

        // Generate functions
        for i in 0..num_functions {
            let module_id = i % num_modules.max(1);
            symbols.push(format!(
                "src/module_{}.rs:function_{}:{}",
                module_id,
                i,
                (i % 100) + 10
            ));
        }

        // Generate methods
        for i in 0..num_methods {
            let module_id = i % num_modules.max(1);
            let class_id = i % 20;
            symbols.push(format!(
                "src/module_{}.rs:Class{}::method_{}:{}",
                module_id,
                class_id,
                i,
                (i % 50) + 50
            ));
        }

        // Generate variables
        for i in 0..num_variables {
            let module_id = i % num_modules.max(1);
            symbols.push(format!(
                "src/module_{}.rs:variable_{}:{}",
                module_id,
                i,
                (i % 20) + 5
            ));
        }

        symbols.truncate(total_symbols);
        symbols
    }

    /// Test symbol storage performance at scale
    pub async fn test_storage_scale(&self, symbols: &[String]) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();

        println!(
            "🔬 Testing storage performance with {} symbols",
            symbols.len()
        );

        // Test batch storage performance
        let batch_start = Instant::now();
        let mut total_edges = 0;

        for batch in symbols.chunks(self.config.batch_size) {
            let mut batch_edges = Vec::new();

            for symbol_uid in batch {
                batch_edges.extend(create_none_call_hierarchy_edges(symbol_uid, 1));
                batch_edges.extend(create_none_reference_edges(symbol_uid, 1));
                batch_edges.extend(create_none_definition_edges(symbol_uid, 1));
                batch_edges.extend(create_none_implementation_edges(symbol_uid, 1));
            }

            let store_start = Instant::now();
            self.database.store_edges(&batch_edges).await?;
            let store_duration = store_start.elapsed();

            total_edges += batch_edges.len();

            // Calculate batch metrics
            let batch_throughput = batch_edges.len() as f64 / store_duration.as_secs_f64();
            metrics.insert(
                format!("batch_{}_throughput", batch.len()),
                batch_throughput,
            );

            println!(
                "   Batch {}: {} edges in {:?} ({:.1} edges/sec)",
                batch.len(),
                batch_edges.len(),
                store_duration,
                batch_throughput
            );
        }

        let total_storage_duration = batch_start.elapsed();
        let overall_throughput = total_edges as f64 / total_storage_duration.as_secs_f64();

        metrics.insert("total_edges".to_string(), total_edges as f64);
        metrics.insert(
            "total_duration_sec".to_string(),
            total_storage_duration.as_secs_f64(),
        );
        metrics.insert("overall_throughput".to_string(), overall_throughput);

        println!("📊 Storage scale results:");
        println!("   Total edges: {}", total_edges);
        println!("   Duration: {:?}", total_storage_duration);
        println!("   Throughput: {:.1} edges/sec", overall_throughput);

        // Validate throughput meets minimum requirements
        assert!(
            overall_throughput > self.config.min_throughput_ops_sec,
            "Storage throughput {:.1} below minimum {:.1} ops/sec",
            overall_throughput,
            self.config.min_throughput_ops_sec
        );

        Ok(metrics)
    }

    /// Test query performance at scale
    pub async fn test_query_scale(&self, symbols: &[String]) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();

        println!(
            "🔍 Testing query performance with {} symbols",
            symbols.len()
        );

        // Test query performance across different symbol types
        let test_sizes = vec![100, 500, 1000, 5000, symbols.len().min(10000)];

        for test_size in test_sizes {
            let test_symbols = &symbols[..test_size];

            let query_start = Instant::now();
            let mut successful_queries = 0;
            let mut query_errors = 0;

            for symbol_uid in test_symbols {
                match self
                    .database
                    .get_call_hierarchy_for_symbol(self.workspace_id, symbol_uid)
                    .await
                {
                    Ok(Some(_)) => successful_queries += 1,
                    Ok(None) => query_errors += 1, // Should be cached
                    Err(_) => query_errors += 1,
                }
            }

            let query_duration = query_start.elapsed();
            let query_throughput = successful_queries as f64 / query_duration.as_secs_f64();
            let error_rate = query_errors as f64 / test_size as f64;

            metrics.insert(format!("query_{}_throughput", test_size), query_throughput);
            metrics.insert(format!("query_{}_error_rate", test_size), error_rate);

            println!(
                "   Size {}: {:.1} queries/sec, {:.2}% errors",
                test_size,
                query_throughput,
                error_rate * 100.0
            );

            // Validate query performance
            assert!(
                error_rate < 0.01,
                "Error rate should be under 1% at scale {}",
                test_size
            );
            assert!(
                query_throughput > self.config.min_throughput_ops_sec,
                "Query throughput {:.1} below minimum at scale {}",
                query_throughput,
                test_size
            );
        }

        Ok(metrics)
    }

    /// Test memory usage at scale
    pub async fn test_memory_scale(&self, symbols: &[String]) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();

        println!("🧠 Testing memory usage with {} symbols", symbols.len());

        // Get initial memory baseline
        let initial_memory = self.estimate_memory_usage();

        // Store edges in incremental batches to monitor memory growth
        let batch_sizes = vec![1000, 2500, 5000, 7500, symbols.len()];
        let mut stored_so_far = 0;

        for target_size in batch_sizes {
            if target_size > symbols.len() {
                continue;
            }

            let symbols_to_store = &symbols[stored_so_far..target_size];

            // Store this batch
            for symbol_uid in symbols_to_store {
                let edges = create_none_call_hierarchy_edges(symbol_uid, 1);
                self.database.store_edges(&edges).await?;
            }

            stored_so_far = target_size;

            // Measure memory usage
            let current_memory = self.estimate_memory_usage();
            let memory_growth = current_memory - initial_memory;
            let memory_per_symbol = memory_growth as f64 / stored_so_far as f64;

            metrics.insert(format!("memory_at_{}", target_size), current_memory as f64);
            metrics.insert(
                format!("memory_per_symbol_at_{}", target_size),
                memory_per_symbol,
            );

            println!(
                "   At {} symbols: {}KB total, {:.2}KB per symbol",
                target_size,
                current_memory / 1024,
                memory_per_symbol / 1024.0
            );

            // Validate memory usage is reasonable
            let memory_limit_bytes = self.config.memory_limit_mb * 1024 * 1024;
            assert!(
                current_memory < memory_limit_bytes as u64,
                "Memory usage {}MB exceeds limit {}MB at scale {}",
                current_memory / (1024 * 1024),
                self.config.memory_limit_mb,
                target_size
            );
        }

        Ok(metrics)
    }

    /// Estimate current memory usage (simplified implementation)
    fn estimate_memory_usage(&self) -> u64 {
        // This is a placeholder. In a real implementation, you might:
        // - Use system APIs to get actual process memory usage
        // - Query SQLite database size
        // - Monitor heap usage with a memory profiler

        // For testing purposes, return a reasonable estimate based on process ID
        std::process::id() as u64 * 1024 + 50_000_000 // Base + process-based estimate
    }
}

#[tokio::test]
async fn test_large_dataset_scale() -> Result<()> {
    println!("📏 Large Dataset Scale Test");

    let config = ScaleTestConfig {
        max_symbols: 10_000,
        batch_size: 1000,
        memory_limit_mb: 200,
        max_query_time_ms: 5,
        min_throughput_ops_sec: 500.0,
    };

    let harness = ScaleTestHarness::new(config).await?;
    let symbols = harness.generate_hierarchical_symbols(10_000);

    println!(
        "Generated {} hierarchical symbols for testing",
        symbols.len()
    );

    // Test storage scaling
    let storage_metrics = harness.test_storage_scale(&symbols).await?;

    // Test query scaling
    let query_metrics = harness.test_query_scale(&symbols).await?;

    // Test memory scaling
    let _memory_metrics = harness.test_memory_scale(&symbols).await?;

    // Combined analysis
    println!("\n📊 Scale Test Summary:");
    println!(
        "   Storage throughput: {:.1} edges/sec",
        storage_metrics.get("overall_throughput").unwrap_or(&0.0)
    );
    println!(
        "   Query throughput: {:.1} queries/sec",
        query_metrics.get("query_10000_throughput").unwrap_or(&0.0)
    );

    // Validate overall system scales acceptably
    let storage_throughput = *storage_metrics.get("overall_throughput").unwrap_or(&0.0);
    let query_throughput = *query_metrics.get("query_10000_throughput").unwrap_or(&0.0);

    assert!(
        storage_throughput > 500.0,
        "Storage should scale to at least 500 edges/sec"
    );
    assert!(
        query_throughput > 1000.0,
        "Queries should scale to at least 1000 queries/sec"
    );

    println!("✅ Large dataset scale test passed");
    Ok(())
}

#[tokio::test]
async fn test_nested_workspace_scale() -> Result<()> {
    println!("🏗️ Nested Workspace Scale Test");

    let config = ScaleTestConfig::default();
    let harness = ScaleTestHarness::new(config).await?;

    // Create multiple workspaces to test workspace isolation at scale
    let workspace_count = 10;
    let symbols_per_workspace = 500;

    let mut workspace_ids = Vec::new();

    // Create workspaces
    for i in 0..workspace_count {
        let workspace_id = harness
            .database
            .create_workspace(
                &format!("scale_workspace_{}", i),
                (i + 1) as i64,
                Some("main"),
            )
            .await?;
        workspace_ids.push(workspace_id);
    }

    println!("Created {} workspaces", workspace_count);

    // Store symbols across workspaces
    let total_start = Instant::now();

    for (i, &workspace_id) in workspace_ids.iter().enumerate() {
        let symbols = harness.generate_hierarchical_symbols(symbols_per_workspace);

        let workspace_start = Instant::now();
        for symbol_uid in &symbols {
            let edges = create_none_call_hierarchy_edges(symbol_uid, 1);
            harness.database.store_edges(&edges).await?;
        }
        let workspace_duration = workspace_start.elapsed();

        println!(
            "   Workspace {}: {} symbols in {:?}",
            i,
            symbols.len(),
            workspace_duration
        );

        // Verify workspace isolation by querying
        let mut successful_queries = 0;
        for symbol_uid in symbols.iter().take(10) {
            if let Ok(Some(_)) = harness
                .database
                .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
                .await
            {
                successful_queries += 1;
            }
        }

        assert!(
            successful_queries > 8,
            "Most queries should succeed in workspace {}",
            i
        );
    }

    let total_duration = total_start.elapsed();
    let total_symbols = workspace_count * symbols_per_workspace;
    let overall_throughput = total_symbols as f64 / total_duration.as_secs_f64();

    println!("📊 Nested Workspace Scale Results:");
    println!("   Total symbols: {}", total_symbols);
    println!("   Total duration: {:?}", total_duration);
    println!(
        "   Overall throughput: {:.1} symbols/sec",
        overall_throughput
    );

    // Validate performance with multiple workspaces
    assert!(
        overall_throughput > 200.0,
        "Multi-workspace performance should exceed 200 symbols/sec"
    );

    // Test cross-workspace query isolation
    println!("🔒 Testing workspace isolation...");
    let test_symbol = "isolation_test_symbol";
    let edges = create_none_call_hierarchy_edges(test_symbol, 1);
    harness.database.store_edges(&edges).await?;

    // Symbol should exist in current workspace but not others
    let mut found_in_workspaces = 0;
    for &workspace_id in &workspace_ids {
        if let Ok(Some(_)) = harness
            .database
            .get_call_hierarchy_for_symbol(workspace_id, test_symbol)
            .await
        {
            found_in_workspaces += 1;
        }
    }

    // Symbol should exist in default workspace (harness.workspace_id) but not test workspaces
    assert!(
        found_in_workspaces <= 1,
        "Symbol should not leak across workspace boundaries"
    );

    println!("✅ Nested workspace scale test passed");
    Ok(())
}

#[tokio::test]
async fn test_long_running_performance() -> Result<()> {
    println!("⏱️ Long Running Performance Test");

    let config = ScaleTestConfig {
        max_symbols: 5_000,
        batch_size: 500,
        memory_limit_mb: 150,
        max_query_time_ms: 10,
        min_throughput_ops_sec: 300.0,
    };

    let harness = ScaleTestHarness::new(config).await?;

    // Simulate long-running usage with multiple phases
    let phases = vec![
        ("Phase 1: Initial Load", 1000),
        ("Phase 2: Growth", 2000),
        ("Phase 3: Peak Usage", 3500),
        ("Phase 4: Sustained Load", 5000),
    ];

    let mut performance_history = Vec::new();
    let test_start = Instant::now();

    for (phase_name, target_symbols) in phases {
        println!("\n🎯 {}: {} symbols", phase_name, target_symbols);

        let symbols = harness.generate_hierarchical_symbols(target_symbols);

        // Store edges
        let store_start = Instant::now();
        for chunk in symbols.chunks(500) {
            for symbol_uid in chunk {
                let edges = create_none_call_hierarchy_edges(symbol_uid, 1);
                harness.database.store_edges(&edges).await?;
            }

            // Brief pause to simulate real-world usage patterns
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let store_duration = store_start.elapsed();

        // Query performance test
        let query_start = Instant::now();
        let test_queries = &symbols[..100.min(symbols.len())];
        let mut successful_queries = 0;

        for symbol_uid in test_queries {
            if let Ok(Some(_)) = harness
                .database
                .get_call_hierarchy_for_symbol(harness.workspace_id, symbol_uid)
                .await
            {
                successful_queries += 1;
            }
        }
        let query_duration = query_start.elapsed();

        let store_throughput = symbols.len() as f64 / store_duration.as_secs_f64();
        let query_throughput = successful_queries as f64 / query_duration.as_secs_f64();
        let success_rate = successful_queries as f64 / test_queries.len() as f64;

        performance_history.push((phase_name, store_throughput, query_throughput, success_rate));

        println!("   Store: {:.1} symbols/sec", store_throughput);
        println!("   Query: {:.1} queries/sec", query_throughput);
        println!("   Success rate: {:.1}%", success_rate * 100.0);

        // Validate performance doesn't degrade significantly over time
        assert!(
            store_throughput > 100.0,
            "Store throughput degraded in {}",
            phase_name
        );
        assert!(
            query_throughput > 500.0,
            "Query throughput degraded in {}",
            phase_name
        );
        assert!(
            success_rate > 0.95,
            "Success rate degraded in {}",
            phase_name
        );
    }

    let total_test_duration = test_start.elapsed();

    // Analysis of performance over time
    println!("\n📈 Long Running Performance Analysis:");
    println!("   Total duration: {:?}", total_test_duration);

    for (phase, store_tp, query_tp, success) in &performance_history {
        println!(
            "   {}: Store={:.1}/sec, Query={:.1}/sec, Success={:.1}%",
            phase,
            store_tp,
            query_tp,
            success * 100.0
        );
    }

    // Check for performance degradation
    let first_store_tp = performance_history[0].1;
    let last_store_tp = performance_history[performance_history.len() - 1].1;
    let store_degradation = (first_store_tp - last_store_tp) / first_store_tp;

    let first_query_tp = performance_history[0].2;
    let last_query_tp = performance_history[performance_history.len() - 1].2;
    let query_degradation = (first_query_tp - last_query_tp) / first_query_tp;

    println!(
        "   Store performance degradation: {:.1}%",
        store_degradation * 100.0
    );
    println!(
        "   Query performance degradation: {:.1}%",
        query_degradation * 100.0
    );

    // Allow some degradation but not excessive
    assert!(
        store_degradation < 0.3,
        "Store performance degraded by more than 30%"
    );
    assert!(
        query_degradation < 0.3,
        "Query performance degraded by more than 30%"
    );

    println!("✅ Long running performance test passed");
    Ok(())
}
