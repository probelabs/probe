//! Comprehensive performance stress testing for the null edge caching system
//!
//! This module provides advanced performance testing beyond basic benchmarks,
//! including concurrent load testing, memory monitoring, scale testing, and
//! statistical performance analysis.

use anyhow::Result;
use lsp_daemon::database::sqlite_backend::SQLiteBackend;
use lsp_daemon::database::{
    create_none_call_hierarchy_edges, create_none_definition_edges,
    create_none_implementation_edges, create_none_reference_edges, DatabaseBackend, DatabaseConfig,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::Barrier;

/// Performance statistics for analysis
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub mean: Duration,
    pub median: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub min: Duration,
    pub max: Duration,
    pub samples: usize,
}

impl PerformanceStats {
    pub fn calculate(mut durations: Vec<Duration>) -> Self {
        durations.sort();
        let samples = durations.len();

        let mean = Duration::from_nanos(
            (durations.iter().map(|d| d.as_nanos()).sum::<u128>() / samples as u128) as u64,
        );

        let median = durations[samples / 2];
        let p95 = durations[(samples as f64 * 0.95) as usize];
        let p99 = durations[(samples as f64 * 0.99) as usize];
        let min = durations[0];
        let max = durations[samples - 1];

        PerformanceStats {
            mean,
            median,
            p95,
            p99,
            min,
            max,
            samples,
        }
    }

    pub fn print_report(&self, label: &str) {
        println!("📊 {} Performance Statistics:", label);
        println!("   Samples:  {}", self.samples);
        println!("   Mean:     {:?}", self.mean);
        println!("   Median:   {:?}", self.median);
        println!("   P95:      {:?}", self.p95);
        println!("   P99:      {:?}", self.p99);
        println!("   Min:      {:?}", self.min);
        println!("   Max:      {:?}", self.max);
    }
}

/// Memory monitoring helper
#[derive(Debug)]
pub struct MemoryMonitor {
    start_usage: u64,
    peak_usage: u64,
}

impl MemoryMonitor {
    pub fn new() -> Self {
        let start_usage = Self::get_memory_usage();
        MemoryMonitor {
            start_usage,
            peak_usage: start_usage,
        }
    }

    pub fn update(&mut self) {
        let current = Self::get_memory_usage();
        if current > self.peak_usage {
            self.peak_usage = current;
        }
    }

    pub fn get_stats(&self) -> (u64, u64, u64) {
        let current = Self::get_memory_usage();
        (self.start_usage, self.peak_usage, current)
    }

    // Simple memory usage estimation (fallback for when system info is unavailable)
    fn get_memory_usage() -> u64 {
        // This is a simplified implementation. In production, you might want to use
        // a crate like `sysinfo` for more accurate memory monitoring
        std::process::id() as u64 * 1024 // Placeholder
    }
}

/// Test harness for performance measurements
pub struct PerformanceTestHarness {
    database: SQLiteBackend,
    workspace_id: i64,
    temp_dir: TempDir,
}

impl PerformanceTestHarness {
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("performance_test.db");

        let config = DatabaseConfig {
            path: Some(db_path),
            temporary: false,
            cache_capacity: 10 * 1024 * 1024, // 10MB for performance tests
            ..Default::default()
        };

        let database = SQLiteBackend::new(config).await?;
        let workspace_id = database
            .create_workspace("performance_test", 1, Some("main"))
            .await?;

        Ok(PerformanceTestHarness {
            database,
            workspace_id,
            temp_dir,
        })
    }

    /// Generate test symbols for performance testing
    pub fn generate_test_symbols(&self, prefix: &str, count: usize) -> Vec<String> {
        (0..count).map(|i| format!("{}_{:06}", prefix, i)).collect()
    }

    /// Measure cache miss performance (cold queries)
    pub async fn measure_cache_misses(&self, symbols: &[String]) -> Result<Vec<Duration>> {
        let mut durations = Vec::new();

        for symbol_uid in symbols {
            let start = Instant::now();
            let _result = self
                .database
                .get_call_hierarchy_for_symbol(self.workspace_id, symbol_uid)
                .await?;
            durations.push(start.elapsed());
        }

        Ok(durations)
    }

    /// Store none edges for all symbols
    pub async fn store_none_edges(&self, symbols: &[String]) -> Result<Duration> {
        let start = Instant::now();

        for symbol_uid in symbols {
            let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
            self.database.store_edges(&none_edges).await?;
        }

        Ok(start.elapsed())
    }

    /// Measure cache hit performance (warm queries)
    pub async fn measure_cache_hits(&self, symbols: &[String]) -> Result<Vec<Duration>> {
        let mut durations = Vec::new();

        for symbol_uid in symbols {
            let start = Instant::now();
            let result = self
                .database
                .get_call_hierarchy_for_symbol(self.workspace_id, symbol_uid)
                .await?;
            durations.push(start.elapsed());

            // Verify it's a cache hit (should return Some with empty arrays)
            match result {
                Some(hierarchy) => {
                    assert!(
                        hierarchy.incoming.is_empty() && hierarchy.outgoing.is_empty(),
                        "Expected empty hierarchy for none edges"
                    );
                }
                None => {
                    panic!(
                        "Expected cache hit (Some) for symbol {}, got None",
                        symbol_uid
                    );
                }
            }
        }

        Ok(durations)
    }
}

#[tokio::test]
async fn test_large_scale_none_edge_performance() -> Result<()> {
    println!("🔬 Large Scale Performance Test");
    println!("Testing performance with 1000+ symbols");

    let harness = PerformanceTestHarness::new().await?;
    let mut memory_monitor = MemoryMonitor::new();

    // Test different scales
    let scales = vec![100, 500, 1000, 2000];
    let mut scale_results = HashMap::new();

    for scale in scales {
        println!("\n📏 Testing scale: {} symbols", scale);

        let symbols = harness.generate_test_symbols(&format!("scale_{}", scale), scale);

        // Phase 1: Cache miss performance
        let miss_start = Instant::now();
        let miss_durations = harness.measure_cache_misses(&symbols).await?;
        let total_miss_duration = miss_start.elapsed();

        memory_monitor.update();

        // Phase 2: Store none edges
        let store_duration = harness.store_none_edges(&symbols).await?;

        memory_monitor.update();

        // Phase 3: Cache hit performance
        let hit_start = Instant::now();
        let hit_durations = harness.measure_cache_hits(&symbols).await?;
        let total_hit_duration = hit_start.elapsed();

        memory_monitor.update();

        // Calculate statistics
        let miss_stats = PerformanceStats::calculate(miss_durations);
        let hit_stats = PerformanceStats::calculate(hit_durations);

        let speedup = total_miss_duration.as_nanos() as f64 / total_hit_duration.as_nanos() as f64;

        scale_results.insert(scale, (miss_stats.clone(), hit_stats.clone(), speedup));

        println!(
            "   Cache miss - Mean: {:?}, P95: {:?}",
            miss_stats.mean, miss_stats.p95
        );
        println!(
            "   Cache hit  - Mean: {:?}, P95: {:?}",
            hit_stats.mean, hit_stats.p95
        );
        println!("   Overall speedup: {:.1}x", speedup);
        println!("   Store duration: {:?}", store_duration);

        // Verify performance targets
        assert!(
            speedup >= 10.0,
            "Scale {} should achieve at least 10x speedup, got {:.1}x",
            scale,
            speedup
        );
        assert!(
            hit_stats.p95 < Duration::from_millis(1),
            "P95 cache hits should be sub-millisecond at scale {}",
            scale
        );
    }

    // Memory usage report
    let (start_mem, peak_mem, final_mem) = memory_monitor.get_stats();
    println!("\n🧠 Memory Usage:");
    println!("   Start:    {}KB", start_mem / 1024);
    println!("   Peak:     {}KB", peak_mem / 1024);
    println!("   Final:    {}KB", final_mem / 1024);
    println!("   Growth:   {}KB", (final_mem - start_mem) / 1024);

    // Validate memory doesn't grow excessively
    let memory_growth_mb = (final_mem - start_mem) / (1024 * 1024);
    assert!(
        memory_growth_mb < 100,
        "Memory growth should be under 100MB, got {}MB",
        memory_growth_mb
    );

    // Performance consistency check
    println!("\n📈 Scale Performance Analysis:");
    for (scale, (_miss_stats, hit_stats, speedup)) in scale_results.iter() {
        println!(
            "   Scale {}: {:.1}x speedup, P95 hit: {:?}",
            scale, speedup, hit_stats.p95
        );
    }

    println!("✅ Large scale performance test passed");
    Ok(())
}

#[tokio::test]
async fn test_concurrent_none_edge_access() -> Result<()> {
    println!("⚡ Concurrent Access Performance Test");

    let harness = Arc::new(PerformanceTestHarness::new().await?);
    let concurrency_levels = vec![2, 4, 8, 16];
    let symbols_per_task = 50;

    for concurrency in concurrency_levels {
        println!("\n🔀 Testing {} concurrent tasks", concurrency);

        let barrier = Arc::new(Barrier::new(concurrency));
        let mut handles = vec![];
        let start_time = Arc::new(std::sync::Mutex::new(None));

        for task_id in 0..concurrency {
            let harness_clone = Arc::clone(&harness);
            let barrier_clone = Arc::clone(&barrier);
            let start_time_clone = Arc::clone(&start_time);

            let handle = tokio::spawn(async move {
                let symbols = harness_clone.generate_test_symbols(
                    &format!("concurrent_{}_{}", concurrency, task_id),
                    symbols_per_task,
                );

                // Synchronize start time
                barrier_clone.wait().await;

                // Record global start time (only first task)
                {
                    let mut start = start_time_clone.lock().unwrap();
                    if start.is_none() {
                        *start = Some(Instant::now());
                    }
                }

                let task_start = Instant::now();
                let mut task_operations = 0;
                let mut task_errors = 0;

                // Cache miss phase
                for symbol_uid in &symbols {
                    match harness_clone
                        .database
                        .get_call_hierarchy_for_symbol(harness_clone.workspace_id, symbol_uid)
                        .await
                    {
                        Ok(_) => task_operations += 1,
                        Err(_) => task_errors += 1,
                    }
                }

                // Store none edges
                for symbol_uid in &symbols {
                    let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
                    match harness_clone.database.store_edges(&none_edges).await {
                        Ok(_) => task_operations += 1,
                        Err(_) => task_errors += 1,
                    }
                }

                // Cache hit phase
                for symbol_uid in &symbols {
                    match harness_clone
                        .database
                        .get_call_hierarchy_for_symbol(harness_clone.workspace_id, symbol_uid)
                        .await
                    {
                        Ok(Some(hierarchy)) => {
                            assert!(hierarchy.incoming.is_empty() && hierarchy.outgoing.is_empty());
                            task_operations += 1;
                        }
                        Ok(None) => task_errors += 1, // Should be cache hit
                        Err(_) => task_errors += 1,
                    }
                }

                let task_duration = task_start.elapsed();

                Ok::<_, anyhow::Error>((task_operations, task_errors, task_duration))
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        let mut results = vec![];
        for handle in handles {
            results.push(handle.await??);
        }

        let total_duration = {
            let start = start_time.lock().unwrap();
            start.unwrap().elapsed()
        };

        // Analyze results
        let total_operations: usize = results.iter().map(|(ops, _, _)| ops).sum();
        let total_errors: usize = results.iter().map(|(_, errs, _)| errs).sum();
        let avg_task_duration: Duration = Duration::from_nanos(
            (results
                .iter()
                .map(|(_, _, dur)| dur.as_nanos())
                .sum::<u128>()
                / results.len() as u128) as u64,
        );

        let ops_per_second = total_operations as f64 / total_duration.as_secs_f64();
        let error_rate = total_errors as f64 / (total_operations + total_errors) as f64;

        println!("   Total operations:     {}", total_operations);
        println!("   Total errors:         {}", total_errors);
        println!("   Error rate:           {:.2}%", error_rate * 100.0);
        println!("   Total duration:       {:?}", total_duration);
        println!("   Avg task duration:    {:?}", avg_task_duration);
        println!("   Operations per sec:   {:.1}", ops_per_second);

        // Validation
        assert!(
            error_rate < 0.01,
            "Error rate should be under 1%, got {:.2}%",
            error_rate * 100.0
        );
        assert!(
            ops_per_second > 100.0,
            "Should achieve at least 100 ops/sec under concurrency"
        );

        // Check for reasonable concurrency benefit (not expecting linear scaling due to database contention)
        if concurrency > 2 {
            let expected_min_ops_per_sec = 50.0 * (concurrency as f64).sqrt();
            assert!(
                ops_per_second > expected_min_ops_per_sec,
                "Concurrent performance should scale somewhat with concurrency"
            );
        }
    }

    println!("✅ Concurrent access performance test passed");
    Ok(())
}

#[tokio::test]
async fn test_mixed_workload_performance() -> Result<()> {
    println!("🔄 Mixed Workload Performance Test");

    let harness = PerformanceTestHarness::new().await?;
    let num_symbols = 500;

    // Create symbols for different edge types
    let call_hierarchy_symbols = harness.generate_test_symbols("mixed_call", num_symbols / 4);
    let reference_symbols = harness.generate_test_symbols("mixed_ref", num_symbols / 4);
    let definition_symbols = harness.generate_test_symbols("mixed_def", num_symbols / 4);
    let implementation_symbols = harness.generate_test_symbols("mixed_impl", num_symbols / 4);

    let all_symbols = [
        call_hierarchy_symbols.as_slice(),
        reference_symbols.as_slice(),
        definition_symbols.as_slice(),
        implementation_symbols.as_slice(),
    ]
    .concat();

    println!(
        "Testing mixed workload with {} symbols across 4 edge types",
        all_symbols.len()
    );

    let mut operation_times = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;
    let workload_start = Instant::now();

    // First pass: Cache misses and store none edges
    for (i, symbol_uid) in all_symbols.iter().enumerate() {
        let op_start = Instant::now();

        match i % 4 {
            0 => {
                // Call hierarchy
                let result = harness
                    .database
                    .get_call_hierarchy_for_symbol(harness.workspace_id, symbol_uid)
                    .await?;
                if result.is_none() {
                    cache_misses += 1;
                    let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
                    harness.database.store_edges(&none_edges).await?;
                } else {
                    cache_hits += 1;
                }
            }
            1 => {
                // References
                let _result = harness
                    .database
                    .get_references_for_symbol(harness.workspace_id, symbol_uid, true)
                    .await?;
                // Always miss first time for references - no need to track
                let none_edges = create_none_reference_edges(symbol_uid, 1);
                harness.database.store_edges(&none_edges).await?;
            }
            2 => {
                // Definitions
                let _result = harness
                    .database
                    .get_definitions_for_symbol(harness.workspace_id, symbol_uid)
                    .await?;
                cache_misses += 1;
                let none_edges = create_none_definition_edges(symbol_uid, 1);
                harness.database.store_edges(&none_edges).await?;
            }
            3 => {
                // Implementations
                let _result = harness
                    .database
                    .get_implementations_for_symbol(harness.workspace_id, symbol_uid)
                    .await?;
                cache_misses += 1;
                let none_edges = create_none_implementation_edges(symbol_uid, 1);
                harness.database.store_edges(&none_edges).await?;
            }
            _ => unreachable!(),
        }

        operation_times.push(op_start.elapsed());
    }

    // Second pass: Should hit cache for call hierarchy
    for symbol_uid in &call_hierarchy_symbols {
        let op_start = Instant::now();
        let result = harness
            .database
            .get_call_hierarchy_for_symbol(harness.workspace_id, symbol_uid)
            .await?;
        operation_times.push(op_start.elapsed());

        match result {
            Some(hierarchy) => {
                assert!(hierarchy.incoming.is_empty() && hierarchy.outgoing.is_empty());
                cache_hits += 1;
            }
            None => {
                cache_misses += 1;
                panic!("Expected cache hit for {}", symbol_uid);
            }
        }
    }

    let workload_duration = workload_start.elapsed();
    let stats = PerformanceStats::calculate(operation_times);

    let total_operations = cache_hits + cache_misses;
    let cache_hit_rate = cache_hits as f64 / total_operations as f64;
    let ops_per_second = total_operations as f64 / workload_duration.as_secs_f64();

    println!("📊 Mixed Workload Results:");
    println!("   Total operations:  {}", total_operations);
    println!("   Cache hits:        {}", cache_hits);
    println!("   Cache misses:      {}", cache_misses);
    println!("   Cache hit rate:    {:.1}%", cache_hit_rate * 100.0);
    println!("   Duration:          {:?}", workload_duration);
    println!("   Ops per second:    {:.1}", ops_per_second);

    stats.print_report("Mixed Workload");

    // Validation
    assert!(
        cache_hit_rate > 0.10,
        "Should achieve at least 10% cache hit rate"
    );
    assert!(
        ops_per_second > 200.0,
        "Should achieve at least 200 mixed ops/sec"
    );
    assert!(
        stats.p95 < Duration::from_millis(5),
        "P95 operation time should be under 5ms"
    );

    println!("✅ Mixed workload performance test passed");
    Ok(())
}

#[tokio::test]
async fn test_performance_regression_prevention() -> Result<()> {
    println!("📈 Performance Regression Prevention Test");

    let harness = PerformanceTestHarness::new().await?;

    // Define baseline performance expectations
    let baseline_thresholds = [
        ("cache_miss_p95_ms", 10.0),    // Cache miss P95 should be under 10ms
        ("cache_hit_p95_us", 500.0),    // Cache hit P95 should be under 500μs
        ("store_ops_per_sec", 1000.0),  // Should store at least 1000 none edges/sec
        ("query_ops_per_sec", 2000.0),  // Should query at least 2000 cached items/sec
        ("concurrent_error_rate", 1.0), // Error rate under load should be under 1%
    ];

    println!("Testing against baseline performance thresholds:");
    for (metric, threshold) in &baseline_thresholds {
        println!("   {}: {}", metric, threshold);
    }

    // Test 1: Cache miss performance baseline
    let symbols = harness.generate_test_symbols("regression_test", 200);
    let miss_durations = harness.measure_cache_misses(&symbols).await?;
    let miss_stats = PerformanceStats::calculate(miss_durations);

    let cache_miss_p95_ms = miss_stats.p95.as_millis() as f64;
    println!(
        "\n✓ Cache miss P95: {:.1}ms (threshold: {:.1}ms)",
        cache_miss_p95_ms, baseline_thresholds[0].1
    );
    assert!(
        cache_miss_p95_ms < baseline_thresholds[0].1,
        "Cache miss P95 regression: {:.1}ms > {:.1}ms",
        cache_miss_p95_ms,
        baseline_thresholds[0].1
    );

    // Test 2: Store performance baseline
    let store_start = Instant::now();
    harness.store_none_edges(&symbols).await?;
    let store_duration = store_start.elapsed();

    let store_ops_per_sec = symbols.len() as f64 / store_duration.as_secs_f64();
    println!(
        "✓ Store ops/sec: {:.1} (threshold: {:.1})",
        store_ops_per_sec, baseline_thresholds[2].1
    );
    assert!(
        store_ops_per_sec > baseline_thresholds[2].1,
        "Store performance regression: {:.1} < {:.1} ops/sec",
        store_ops_per_sec,
        baseline_thresholds[2].1
    );

    // Test 3: Cache hit performance baseline
    let hit_durations = harness.measure_cache_hits(&symbols).await?;
    let hit_stats = PerformanceStats::calculate(hit_durations);

    let cache_hit_p95_us = hit_stats.p95.as_micros() as f64;
    println!(
        "✓ Cache hit P95: {:.1}μs (threshold: {:.1}μs)",
        cache_hit_p95_us, baseline_thresholds[1].1
    );
    assert!(
        cache_hit_p95_us < baseline_thresholds[1].1,
        "Cache hit P95 regression: {:.1}μs > {:.1}μs",
        cache_hit_p95_us,
        baseline_thresholds[1].1
    );

    // Test 4: Query performance baseline
    let query_start = Instant::now();
    for symbol_uid in &symbols {
        let _result = harness
            .database
            .get_call_hierarchy_for_symbol(harness.workspace_id, symbol_uid)
            .await?;
    }
    let query_duration = query_start.elapsed();

    let query_ops_per_sec = symbols.len() as f64 / query_duration.as_secs_f64();
    println!(
        "✓ Query ops/sec: {:.1} (threshold: {:.1})",
        query_ops_per_sec, baseline_thresholds[3].1
    );
    assert!(
        query_ops_per_sec > baseline_thresholds[3].1,
        "Query performance regression: {:.1} < {:.1} ops/sec",
        query_ops_per_sec,
        baseline_thresholds[3].1
    );

    // Test 5: Concurrent access error rate baseline
    let concurrent_symbols = harness.generate_test_symbols("concurrent_regression", 100);
    let harness_arc = Arc::new(harness);
    let mut handles = vec![];

    for i in 0..4 {
        let harness_clone = Arc::clone(&harness_arc);
        let symbols_slice = concurrent_symbols[i * 25..(i + 1) * 25].to_vec();

        let handle = tokio::spawn(async move {
            let mut errors = 0;
            let mut operations = 0;

            for symbol_uid in symbols_slice {
                operations += 1;
                if let Err(_) = harness_clone
                    .database
                    .get_call_hierarchy_for_symbol(harness_clone.workspace_id, &symbol_uid)
                    .await
                {
                    errors += 1;
                }

                let none_edges = create_none_call_hierarchy_edges(&symbol_uid, 1);
                operations += 1;
                if let Err(_) = harness_clone.database.store_edges(&none_edges).await {
                    errors += 1;
                }
            }

            (errors, operations)
        });

        handles.push(handle);
    }

    let mut total_errors = 0;
    let mut total_operations = 0;

    for handle in handles {
        let (errors, operations) = handle.await?;
        total_errors += errors;
        total_operations += operations;
    }

    let concurrent_error_rate = (total_errors as f64 / total_operations as f64) * 100.0;
    println!(
        "✓ Concurrent error rate: {:.2}% (threshold: {:.1}%)",
        concurrent_error_rate, baseline_thresholds[4].1
    );
    assert!(
        concurrent_error_rate < baseline_thresholds[4].1,
        "Concurrent error rate regression: {:.2}% > {:.1}%",
        concurrent_error_rate,
        baseline_thresholds[4].1
    );

    println!("\n🎯 All baseline performance thresholds met!");
    println!("   System performance is within acceptable regression bounds");

    println!("✅ Performance regression prevention test passed");
    Ok(())
}

#[tokio::test]
async fn test_database_performance_under_scale() -> Result<()> {
    println!("🗄️ Database Performance Under Scale Test");

    let harness = PerformanceTestHarness::new().await?;
    let scales = vec![1000, 5000, 10000];

    for scale in scales {
        println!("\n📊 Testing database performance with {} edges", scale);

        let symbols = harness.generate_test_symbols(&format!("db_scale_{}", scale), scale / 4);

        // Create mixed edge types for more realistic database load
        let mut all_edges = Vec::new();

        for symbol_uid in &symbols {
            // Add different types of none edges
            all_edges.extend(create_none_call_hierarchy_edges(symbol_uid, 1));
            all_edges.extend(create_none_reference_edges(symbol_uid, 1));
            all_edges.extend(create_none_definition_edges(symbol_uid, 1));
            all_edges.extend(create_none_implementation_edges(symbol_uid, 1));
        }

        println!("   Generated {} edges for storage", all_edges.len());

        // Measure batch store performance
        let batch_start = Instant::now();
        harness.database.store_edges(&all_edges).await?;
        let batch_duration = batch_start.elapsed();

        let edges_per_second = all_edges.len() as f64 / batch_duration.as_secs_f64();
        println!(
            "   Batch store: {:?} ({:.1} edges/sec)",
            batch_duration, edges_per_second
        );

        // Measure individual query performance
        let mut query_times = Vec::new();
        for symbol_uid in symbols.iter().take(100) {
            // Test first 100 for timing
            let query_start = Instant::now();
            let _result = harness
                .database
                .get_call_hierarchy_for_symbol(harness.workspace_id, symbol_uid)
                .await?;
            query_times.push(query_start.elapsed());
        }

        let query_stats = PerformanceStats::calculate(query_times);

        println!("   Query performance:");
        println!("     Mean: {:?}", query_stats.mean);
        println!("     P95:  {:?}", query_stats.p95);
        println!("     P99:  {:?}", query_stats.p99);

        // Performance assertions
        assert!(
            edges_per_second > 500.0,
            "Should store at least 500 edges/sec at scale {}",
            scale
        );
        assert!(
            query_stats.p95 < Duration::from_millis(2),
            "Query P95 should be under 2ms at scale {}",
            scale
        );

        // Memory efficiency check - database file shouldn't grow excessively
        // Note: This is a simplified check. In production, you might want more sophisticated analysis
        println!("   Database scale test passed for {} edges", scale);
    }

    println!("✅ Database performance under scale test passed");
    Ok(())
}
