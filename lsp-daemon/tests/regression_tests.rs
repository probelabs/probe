//! Performance regression prevention tests
//!
//! Validates that performance doesn't degrade beyond acceptable thresholds.

use anyhow::Result;
use lsp_daemon::database::sqlite_backend::SQLiteBackend;
use lsp_daemon::database::{create_none_call_hierarchy_edges, DatabaseBackend, DatabaseConfig};
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Performance thresholds for regression detection
pub struct PerformanceThresholds {
    pub cache_hit_p95_us: f64,
    pub cache_miss_p95_ms: f64,
    pub storage_throughput_ops_sec: f64,
    pub query_throughput_ops_sec: f64,
    pub min_speedup_ratio: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        PerformanceThresholds {
            cache_hit_p95_us: 1000.0,          // 1ms P95 for cache hits
            cache_miss_p95_ms: 20.0,           // 20ms P95 for cache misses
            storage_throughput_ops_sec: 500.0, // 500 ops/sec storage
            query_throughput_ops_sec: 1000.0,  // 1000 ops/sec queries
            min_speedup_ratio: 5.0,            // 5x minimum speedup
        }
    }
}

#[tokio::test]
async fn test_baseline_performance_regression() -> Result<()> {
    println!("🎯 Baseline Performance Regression Test");

    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("regression_test.db");

    let config = DatabaseConfig {
        path: Some(db_path),
        temporary: false,
        cache_capacity: 5 * 1024 * 1024, // 5MB
        ..Default::default()
    };

    let database = SQLiteBackend::new(config).await?;
    let workspace_id = database
        .create_workspace("regression_test", 1, Some("main"))
        .await?;

    let symbols: Vec<String> = (0..200)
        .map(|i| format!("regression_test_symbol_{}", i))
        .collect();

    // Phase 1: Measure cache miss performance
    let mut miss_times = Vec::new();
    for symbol_uid in &symbols {
        let start = Instant::now();
        let _result = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await?;
        miss_times.push(start.elapsed());
    }

    // Phase 2: Store none edges
    let storage_start = Instant::now();
    for symbol_uid in &symbols {
        let edges = create_none_call_hierarchy_edges(symbol_uid, 1);
        database.store_edges(&edges).await?;
    }
    let storage_duration = storage_start.elapsed();
    let storage_throughput = symbols.len() as f64 / storage_duration.as_secs_f64();

    // Phase 3: Measure cache hit performance
    let mut hit_times = Vec::new();
    for symbol_uid in &symbols {
        let start = Instant::now();
        let result = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await?;
        hit_times.push(start.elapsed());
        assert!(result.is_some(), "Should be cache hit");
    }

    let query_throughput = symbols.len() as f64 / hit_times.iter().sum::<Duration>().as_secs_f64();

    // Calculate P95 values
    miss_times.sort();
    hit_times.sort();
    let miss_p95 = miss_times[(miss_times.len() as f64 * 0.95) as usize];
    let hit_p95 = hit_times[(hit_times.len() as f64 * 0.95) as usize];

    let speedup_ratio = miss_p95.as_nanos() as f64 / hit_p95.as_nanos() as f64;

    // Performance validation
    let thresholds = PerformanceThresholds::default();

    println!("📊 Performance Results:");
    println!(
        "   Cache hit P95:      {:?} ({:.1}μs)",
        hit_p95,
        hit_p95.as_micros()
    );
    println!(
        "   Cache miss P95:     {:?} ({:.1}ms)",
        miss_p95,
        miss_p95.as_millis()
    );
    println!("   Storage throughput: {:.1} ops/sec", storage_throughput);
    println!("   Query throughput:   {:.1} ops/sec", query_throughput);
    println!("   Speedup ratio:      {:.1}x", speedup_ratio);

    // Validate against thresholds
    assert!(
        hit_p95.as_micros() as f64 <= thresholds.cache_hit_p95_us,
        "Cache hit P95 regression: {:.1}μs > {:.1}μs",
        hit_p95.as_micros(),
        thresholds.cache_hit_p95_us
    );

    assert!(
        miss_p95.as_millis() as f64 <= thresholds.cache_miss_p95_ms,
        "Cache miss P95 regression: {:.1}ms > {:.1}ms",
        miss_p95.as_millis(),
        thresholds.cache_miss_p95_ms
    );

    assert!(
        storage_throughput >= thresholds.storage_throughput_ops_sec,
        "Storage throughput regression: {:.1} < {:.1} ops/sec",
        storage_throughput,
        thresholds.storage_throughput_ops_sec
    );

    assert!(
        query_throughput >= thresholds.query_throughput_ops_sec,
        "Query throughput regression: {:.1} < {:.1} ops/sec",
        query_throughput,
        thresholds.query_throughput_ops_sec
    );

    assert!(
        speedup_ratio >= thresholds.min_speedup_ratio,
        "Speedup ratio regression: {:.1}x < {:.1}x",
        speedup_ratio,
        thresholds.min_speedup_ratio
    );

    println!("✅ Baseline performance regression test passed");
    Ok(())
}

#[tokio::test]
async fn test_scale_performance_regression() -> Result<()> {
    println!("📈 Scale Performance Regression Test");

    // Test with larger workload
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("scale_regression_test.db");

    let config = DatabaseConfig {
        path: Some(db_path),
        temporary: false,
        cache_capacity: 10 * 1024 * 1024, // 10MB for scale test
        ..Default::default()
    };

    let database = SQLiteBackend::new(config).await?;
    let workspace_id = database
        .create_workspace("scale_regression_test", 1, Some("main"))
        .await?;

    let symbols: Vec<String> = (0..1000)
        .map(|i| format!("scale_regression_symbol_{}", i))
        .collect();

    // Store none edges first
    for symbol_uid in &symbols {
        let edges = create_none_call_hierarchy_edges(symbol_uid, 1);
        database.store_edges(&edges).await?;
    }

    // Test query performance at scale
    let query_start = Instant::now();
    let mut successful_queries = 0;

    for symbol_uid in &symbols {
        if let Ok(Some(_)) = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await
        {
            successful_queries += 1;
        }
    }

    let query_duration = query_start.elapsed();
    let query_throughput = successful_queries as f64 / query_duration.as_secs_f64();

    println!("📊 Scale Performance Results:");
    println!("   Symbols tested:     {}", symbols.len());
    println!("   Successful queries: {}", successful_queries);
    println!("   Query duration:     {:?}", query_duration);
    println!("   Query throughput:   {:.1} ops/sec", query_throughput);

    // Relaxed thresholds for scale testing
    assert!(
        query_throughput >= 500.0,
        "Scale query throughput should exceed 500 ops/sec, got {:.1}",
        query_throughput
    );
    assert!(
        successful_queries >= symbols.len() * 95 / 100,
        "Should achieve at least 95% success rate"
    );

    println!("✅ Scale performance regression test passed");
    Ok(())
}
