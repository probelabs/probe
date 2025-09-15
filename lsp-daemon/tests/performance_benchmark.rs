//! Performance benchmarks for the null edge caching system
//!
//! Measures the performance improvement from caching empty LSP results
//! vs making repeated LSP server calls.

use anyhow::Result;
use lsp_daemon::database::sqlite_backend::SQLiteBackend;
use lsp_daemon::database::{
    create_none_call_hierarchy_edges, create_none_definition_edges,
    create_none_implementation_edges, create_none_reference_edges, DatabaseBackend, DatabaseConfig,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Statistical analysis for performance data
#[derive(Debug, Clone)]
pub struct StatisticalSummary {
    pub mean: Duration,
    pub median: Duration,
    pub std_dev: Duration,
    pub p90: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub min: Duration,
    pub max: Duration,
    pub sample_count: usize,
    pub confidence_interval_95: (Duration, Duration),
}

impl StatisticalSummary {
    pub fn from_measurements(mut measurements: Vec<Duration>) -> Self {
        measurements.sort();
        let n = measurements.len();

        if n == 0 {
            panic!("Cannot calculate statistics from empty measurements");
        }

        let mean_nanos = measurements.iter().map(|d| d.as_nanos()).sum::<u128>() / n as u128;
        let mean = Duration::from_nanos(mean_nanos as u64);

        let median = measurements[n / 2];
        let p90 = measurements[(n as f64 * 0.90) as usize];
        let p95 = measurements[(n as f64 * 0.95) as usize];
        let p99 = measurements[(n as f64 * 0.99) as usize];
        let min = measurements[0];
        let max = measurements[n - 1];

        // Calculate standard deviation
        let variance_nanos = measurements
            .iter()
            .map(|d| {
                let diff = d.as_nanos() as i128 - mean_nanos as i128;
                (diff * diff) as u128
            })
            .sum::<u128>()
            / n as u128;
        let std_dev = Duration::from_nanos((variance_nanos as f64).sqrt() as u64);

        // Calculate 95% confidence interval (assuming normal distribution)
        let std_error = std_dev.as_nanos() as f64 / (n as f64).sqrt();
        let margin_of_error = 1.96 * std_error; // 95% CI for normal distribution
        let ci_lower = Duration::from_nanos((mean_nanos as f64 - margin_of_error).max(0.0) as u64);
        let ci_upper = Duration::from_nanos((mean_nanos as f64 + margin_of_error) as u64);

        StatisticalSummary {
            mean,
            median,
            std_dev,
            p90,
            p95,
            p99,
            min,
            max,
            sample_count: n,
            confidence_interval_95: (ci_lower, ci_upper),
        }
    }

    pub fn print_detailed_report(&self, title: &str) {
        println!("\n📊 Statistical Analysis: {}", title);
        println!("   Sample count:     {}", self.sample_count);
        println!("   Mean:             {:?}", self.mean);
        println!("   Median:           {:?}", self.median);
        println!("   Std Deviation:    {:?}", self.std_dev);
        println!("   Min:              {:?}", self.min);
        println!("   Max:              {:?}", self.max);
        println!("   P90:              {:?}", self.p90);
        println!("   P95:              {:?}", self.p95);
        println!("   P99:              {:?}", self.p99);
        println!(
            "   95% CI:           {:?} to {:?}",
            self.confidence_interval_95.0, self.confidence_interval_95.1
        );
    }

    pub fn compare_with(&self, other: &StatisticalSummary, title: &str) {
        let speedup = other.mean.as_nanos() as f64 / self.mean.as_nanos() as f64;
        let median_speedup = other.median.as_nanos() as f64 / self.median.as_nanos() as f64;
        let p95_speedup = other.p95.as_nanos() as f64 / self.p95.as_nanos() as f64;

        println!("\n🔍 Performance Comparison: {}", title);
        println!("   Mean speedup:     {:.2}x", speedup);
        println!("   Median speedup:   {:.2}x", median_speedup);
        println!("   P95 speedup:      {:.2}x", p95_speedup);

        // Variability comparison
        let cv_self = self.std_dev.as_nanos() as f64 / self.mean.as_nanos() as f64;
        let cv_other = other.std_dev.as_nanos() as f64 / other.mean.as_nanos() as f64;
        println!(
            "   Consistency improvement: {:.1}x less variable",
            cv_other / cv_self
        );
    }
}

/// Performance benchmark result
#[derive(Debug)]
pub struct BenchmarkResult {
    pub operation: String,
    pub cache_miss_stats: StatisticalSummary,
    pub cache_hit_stats: StatisticalSummary,
    pub overall_speedup: f64,
    pub throughput_ops_per_sec: f64,
}

#[tokio::test]
async fn benchmark_cache_performance() -> Result<()> {
    let config = DatabaseConfig {
        path: None, // In-memory for speed
        temporary: true,
        cache_capacity: 1024 * 1024,
        ..Default::default()
    };

    let database = SQLiteBackend::new(config).await?;
    let workspace_id = 1i64;

    // Test with multiple symbols for better statistical significance
    let test_symbols = (0..500)
        .map(|i| format!("benchmark_symbol_{}", i))
        .collect::<Vec<_>>();

    println!(
        "🔬 Advanced Statistical Benchmarking with {} symbols",
        test_symbols.len()
    );

    // Phase 1: Detailed cache miss measurement
    println!("\n⏱️ Phase 1: Cache Miss Performance (Cold Cache)");
    let mut miss_measurements = Vec::new();
    let mut miss_count = 0;

    for symbol_uid in &test_symbols {
        let start = Instant::now();
        let result = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await?;
        let duration = start.elapsed();
        miss_measurements.push(duration);

        if result.is_none() {
            miss_count += 1;
        }
    }

    let miss_stats = StatisticalSummary::from_measurements(miss_measurements);
    miss_stats.print_detailed_report("Cache Miss Performance");

    // Store none edges for all symbols
    println!("\n💾 Storing None Edges...");
    let store_start = Instant::now();
    for symbol_uid in &test_symbols {
        let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
        database.store_edges(&none_edges).await?;
    }
    let store_duration = store_start.elapsed();
    let store_throughput = test_symbols.len() as f64 / store_duration.as_secs_f64();

    println!(
        "   Storage: {} symbols in {:?} ({:.1} symbols/sec)",
        test_symbols.len(),
        store_duration,
        store_throughput
    );

    // Phase 2: Detailed cache hit measurement
    println!("\n⚡ Phase 2: Cache Hit Performance (Warm Cache)");
    let mut hit_measurements = Vec::new();
    let mut hit_count = 0;

    for symbol_uid in &test_symbols {
        let start = Instant::now();
        let result = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await?;
        let duration = start.elapsed();
        hit_measurements.push(duration);

        if result.is_some() {
            hit_count += 1;
        }
    }

    let hit_stats = StatisticalSummary::from_measurements(hit_measurements);
    hit_stats.print_detailed_report("Cache Hit Performance");

    // Statistical comparison
    miss_stats.compare_with(&hit_stats, "Cache Performance Improvement");

    // Overall performance metrics
    let overall_speedup = miss_stats.mean.as_nanos() as f64 / hit_stats.mean.as_nanos() as f64;
    let throughput_improvement =
        hit_stats.mean.as_nanos() as f64 / miss_stats.mean.as_nanos() as f64;
    let efficiency = (1.0 - 1.0 / overall_speedup) * 100.0;

    println!("\n🚀 Advanced Performance Results:");
    println!(
        "   Overall speedup:          {:.2}x faster",
        overall_speedup
    );
    println!(
        "   Median speedup:           {:.2}x faster",
        miss_stats.median.as_nanos() as f64 / hit_stats.median.as_nanos() as f64
    );
    println!(
        "   P95 speedup:              {:.2}x faster",
        miss_stats.p95.as_nanos() as f64 / hit_stats.p95.as_nanos() as f64
    );
    println!(
        "   Throughput improvement:   {:.1}x",
        throughput_improvement
    );
    println!("   Efficiency gain:          {:.1}% time saved", efficiency);
    println!(
        "   Storage throughput:       {:.1} symbols/sec",
        store_throughput
    );

    // Enhanced validation with statistical significance
    assert!(
        overall_speedup > 10.0,
        "Cache should provide at least 10x speedup, got {:.2}x",
        overall_speedup
    );
    assert_eq!(
        hit_count,
        test_symbols.len(),
        "All symbols should be cache hits"
    );
    assert_eq!(
        miss_count,
        test_symbols.len(),
        "All initial queries should be cache misses"
    );

    // Statistical validation
    assert!(
        hit_stats.p95 < miss_stats.p95,
        "Cache hit P95 should be faster than cache miss P95"
    );
    assert!(
        hit_stats.std_dev < miss_stats.std_dev,
        "Cache hits should be more consistent"
    );

    // Performance targets
    assert!(
        hit_stats.p95 < Duration::from_millis(1),
        "Cache hit P95 should be sub-millisecond"
    );
    assert!(
        store_throughput > 100.0,
        "Should store at least 100 symbols/sec"
    );

    Ok(())
}

#[tokio::test]
async fn benchmark_different_edge_types() -> Result<()> {
    let config = DatabaseConfig {
        path: None,
        temporary: true,
        cache_capacity: 1024 * 1024,
        ..Default::default()
    };

    let database = SQLiteBackend::new(config).await?;
    let workspace_id = 1i64;

    let test_cases = vec![
        ("call_hierarchy", "test_call_hierarchy"),
        ("references", "test_references"),
        ("definitions", "test_definitions"),
        ("implementations", "test_implementations"),
    ];

    let mut benchmark_results = HashMap::new();

    for (edge_type, symbol_prefix) in test_cases {
        println!("\n🔬 Statistical Benchmarking: {} edge type", edge_type);

        let symbols: Vec<String> = (0..200)
            .map(|i| format!("{}_{}", symbol_prefix, i))
            .collect();

        // Cache miss measurements
        let mut miss_measurements = Vec::new();
        for symbol_uid in &symbols {
            let start = Instant::now();
            match edge_type {
                "call_hierarchy" => {
                    database
                        .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
                        .await?;
                }
                "references" => {
                    database
                        .get_references_for_symbol(workspace_id, symbol_uid, true)
                        .await?;
                }
                "definitions" => {
                    database
                        .get_definitions_for_symbol(workspace_id, symbol_uid)
                        .await?;
                }
                "implementations" => {
                    database
                        .get_implementations_for_symbol(workspace_id, symbol_uid)
                        .await?;
                }
                _ => unreachable!(),
            }
            miss_measurements.push(start.elapsed());
        }

        // Store appropriate none edges
        let store_start = Instant::now();
        for symbol_uid in &symbols {
            let none_edges = match edge_type {
                "call_hierarchy" => create_none_call_hierarchy_edges(symbol_uid, 1),
                "references" => create_none_reference_edges(symbol_uid, 1),
                "definitions" => create_none_definition_edges(symbol_uid, 1),
                "implementations" => create_none_implementation_edges(symbol_uid, 1),
                _ => unreachable!(),
            };
            database.store_edges(&none_edges).await?;
        }
        let store_duration = store_start.elapsed();

        // Cache hit measurements
        let mut hit_measurements = Vec::new();
        for symbol_uid in &symbols {
            let start = Instant::now();
            match edge_type {
                "call_hierarchy" => {
                    let result = database
                        .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
                        .await?;
                    assert!(result.is_some(), "Should be cache hit for call hierarchy");
                }
                "references" => {
                    let _result = database
                        .get_references_for_symbol(workspace_id, symbol_uid, true)
                        .await?;
                }
                "definitions" => {
                    let _result = database
                        .get_definitions_for_symbol(workspace_id, symbol_uid)
                        .await?;
                }
                "implementations" => {
                    let _result = database
                        .get_implementations_for_symbol(workspace_id, symbol_uid)
                        .await?;
                }
                _ => unreachable!(),
            }
            hit_measurements.push(start.elapsed());
        }

        // Statistical analysis
        let miss_stats = StatisticalSummary::from_measurements(miss_measurements);
        let hit_stats = StatisticalSummary::from_measurements(hit_measurements);

        let speedup = miss_stats.mean.as_nanos() as f64 / hit_stats.mean.as_nanos() as f64;
        let throughput = symbols.len() as f64 / store_duration.as_secs_f64();

        let benchmark_result = BenchmarkResult {
            operation: edge_type.to_string(),
            cache_miss_stats: miss_stats.clone(),
            cache_hit_stats: hit_stats.clone(),
            overall_speedup: speedup,
            throughput_ops_per_sec: throughput,
        };

        benchmark_results.insert(edge_type, benchmark_result);

        println!("   {} Performance:", edge_type);
        println!("     Cache miss P95:  {:?}", miss_stats.p95);
        println!("     Cache hit P95:   {:?}", hit_stats.p95);
        println!("     Speedup:         {:.2}x", speedup);
        println!("     Storage rate:    {:.1} ops/sec", throughput);

        // Validation for each edge type
        assert!(
            speedup > 5.0,
            "{} should provide at least 5x speedup",
            edge_type
        );
        assert!(
            hit_stats.p95 < miss_stats.p95,
            "{} cache hits should be faster than misses",
            edge_type
        );
    }

    // Comprehensive summary report
    println!("\n📈 Comprehensive Edge Type Performance Analysis:");
    println!(
        "{:<15} {:<12} {:<12} {:<12} {:<12}",
        "Edge Type", "Speedup", "Miss P95", "Hit P95", "Storage/s"
    );
    println!("{}", "-".repeat(65));

    for (edge_type, result) in benchmark_results.iter() {
        println!(
            "{:<15} {:<12.1}x {:<12.3}ms {:<12.3}μs {:<12.1}",
            edge_type,
            result.overall_speedup,
            result.cache_miss_stats.p95.as_millis(),
            result.cache_hit_stats.p95.as_micros(),
            result.throughput_ops_per_sec
        );
    }

    // Cross-edge-type validation
    let average_speedup: f64 = benchmark_results
        .values()
        .map(|r| r.overall_speedup)
        .sum::<f64>()
        / benchmark_results.len() as f64;

    assert!(
        average_speedup > 8.0,
        "Average speedup across edge types should exceed 8x"
    );

    println!("\n🎯 Cross-Edge-Type Metrics:");
    println!("   Average speedup:     {:.2}x", average_speedup);
    println!("   Performance consistency validated across all edge types");

    Ok(())
}

#[tokio::test]
async fn benchmark_scale_testing() -> Result<()> {
    let config = DatabaseConfig {
        path: None,
        temporary: true,
        cache_capacity: 10 * 1024 * 1024, // 10MB for larger tests
        ..Default::default()
    };

    let database = SQLiteBackend::new(config).await?;
    let workspace_id = 1i64;

    // Test different scales to see how performance changes
    let scales = vec![10, 100, 500, 1000];

    println!("📏 Testing cache performance at different scales");

    for scale in scales {
        let symbols: Vec<String> = (0..scale).map(|i| format!("scale_test_{}", i)).collect();

        println!("\n🔬 Testing with {} symbols", scale);

        // Measure cache miss time
        let miss_start = Instant::now();
        for symbol_uid in &symbols {
            database
                .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
                .await?;
        }
        let miss_duration = miss_start.elapsed();

        // Store none edges
        for symbol_uid in &symbols {
            let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
            database.store_edges(&none_edges).await?;
        }

        // Measure cache hit time
        let hit_start = Instant::now();
        for symbol_uid in &symbols {
            let result = database
                .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
                .await?;
            assert!(result.is_some(), "Should be cache hit");
        }
        let hit_duration = hit_start.elapsed();

        let speedup = miss_duration.as_nanos() as f64 / hit_duration.as_nanos() as f64;
        let miss_per_symbol = miss_duration / scale;
        let hit_per_symbol = hit_duration / scale;

        println!("   Scale {}: {:.1}x speedup", scale, speedup);
        println!("   Miss per symbol: {:?}", miss_per_symbol);
        println!("   Hit per symbol:  {:?}", hit_per_symbol);

        // Verify performance doesn't degrade significantly with scale
        assert!(
            speedup > 2.0,
            "Speedup should remain above 2x at scale {}",
            scale
        );
    }

    Ok(())
}

#[tokio::test]
async fn benchmark_concurrent_performance() -> Result<()> {
    let config = DatabaseConfig {
        path: None,
        temporary: true,
        cache_capacity: 5 * 1024 * 1024,
        ..Default::default()
    };

    let database = Arc::new(SQLiteBackend::new(config).await?);
    let workspace_id = 1i64;

    let concurrency_levels = vec![1, 4, 8, 16];
    let symbols_per_task = 25;

    println!("⚡ Testing concurrent cache performance");

    for concurrency in concurrency_levels {
        println!("\n🔬 Testing with {} concurrent tasks", concurrency);

        let total_symbols = concurrency * symbols_per_task;

        // Sequential test (baseline)
        let sequential_start = Instant::now();
        for i in 0..total_symbols {
            let symbol_uid = format!("sequential_{}_{}", concurrency, i);
            database
                .get_call_hierarchy_for_symbol(workspace_id, &symbol_uid)
                .await?;

            let none_edges = create_none_call_hierarchy_edges(&symbol_uid, 1);
            database.store_edges(&none_edges).await?;

            let _result = database
                .get_call_hierarchy_for_symbol(workspace_id, &symbol_uid)
                .await?;
        }
        let sequential_duration = sequential_start.elapsed();

        // Concurrent test
        let concurrent_start = Instant::now();
        let mut handles = vec![];

        for task_id in 0..concurrency {
            let db = Arc::clone(&database);

            let handle = tokio::spawn(async move {
                for i in 0..symbols_per_task {
                    let symbol_uid = format!("concurrent_{}_{}_{}", concurrency, task_id, i);

                    // Cache miss
                    db.get_call_hierarchy_for_symbol(workspace_id, &symbol_uid)
                        .await?;

                    // Store none edges
                    let none_edges = create_none_call_hierarchy_edges(&symbol_uid, 1);
                    db.store_edges(&none_edges).await?;

                    // Cache hit
                    let result = db
                        .get_call_hierarchy_for_symbol(workspace_id, &symbol_uid)
                        .await?;
                    assert!(result.is_some(), "Should be cache hit");
                }

                Ok::<_, anyhow::Error>(())
            });

            handles.push(handle);
        }

        // Wait for all concurrent tasks
        for handle in handles {
            handle.await??;
        }

        let concurrent_duration = concurrent_start.elapsed();

        let concurrent_speedup =
            sequential_duration.as_nanos() as f64 / concurrent_duration.as_nanos() as f64;

        println!("   Sequential time: {:?}", sequential_duration);
        println!("   Concurrent time: {:?}", concurrent_duration);
        println!("   Concurrency speedup: {:.1}x", concurrent_speedup);

        // Expect some speedup from concurrency (but not linear due to database contention)
        if concurrency > 1 {
            assert!(
                concurrent_speedup > 1.1,
                "Should get some concurrency benefit"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn benchmark_memory_usage() -> Result<()> {
    let config = DatabaseConfig {
        path: None,
        temporary: true,
        cache_capacity: 1024 * 1024, // 1MB limit for testing
        ..Default::default()
    };

    let database = SQLiteBackend::new(config).await?;
    let workspace_id = 1i64;

    println!("🧠 Testing memory usage with cache limits");

    // Create more symbols than can fit in cache
    let num_symbols = 1000;
    let symbols: Vec<String> = (0..num_symbols)
        .map(|i| format!("memory_test_{}", i))
        .collect();

    // Store none edges for many symbols
    for (i, symbol_uid) in symbols.iter().enumerate() {
        let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
        database.store_edges(&none_edges).await?;

        // Every 100 symbols, check that operations still work
        if i % 100 == 0 && i > 0 {
            println!("   Stored {} symbols...", i);

            // Test that we can still query successfully
            let result = database
                .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
                .await?;
            assert!(result.is_some(), "Cache should still work at {} symbols", i);
        }
    }

    // Test that recent symbols are still cached
    let recent_symbols = &symbols[symbols.len() - 10..];
    let mut cache_hits = 0;

    for symbol_uid in recent_symbols {
        let result = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await?;
        if result.is_some() {
            cache_hits += 1;
        }
    }

    println!(
        "   Recent cache hits: {}/{}",
        cache_hits,
        recent_symbols.len()
    );

    // Most recent symbols should still be cached
    assert!(
        cache_hits >= recent_symbols.len() / 2,
        "At least half of recent symbols should be cached"
    );

    println!("✅ Memory usage test completed");

    Ok(())
}

#[tokio::test]
async fn benchmark_mixed_workload() -> Result<()> {
    let config = DatabaseConfig {
        path: None,
        temporary: true,
        cache_capacity: 2 * 1024 * 1024,
        ..Default::default()
    };

    let database = SQLiteBackend::new(config).await?;
    let workspace_id = 1i64;

    println!("🔄 Advanced Mixed Workload Statistical Analysis");

    let num_symbols = 1000;
    let symbols: Vec<String> = (0..num_symbols)
        .map(|i| format!("mixed_test_{}", i))
        .collect();

    // Track detailed operation metrics
    let mut operation_measurements = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;
    let mut operations_by_type = HashMap::new();

    let workload_start = Instant::now();

    for (i, symbol_uid) in symbols.iter().enumerate() {
        let operation_start = Instant::now();
        let operation_type = match i % 4 {
            0 => "call_hierarchy",
            1 => "references",
            2 => "definitions",
            3 => "implementations",
            _ => unreachable!(),
        };

        // Perform operation based on type
        match i % 4 {
            0 => {
                // Call hierarchy
                let result = database
                    .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
                    .await?;
                if result.is_none() {
                    cache_misses += 1;
                    let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
                    database.store_edges(&none_edges).await?;
                } else {
                    cache_hits += 1;
                }
            }
            1 => {
                // References
                let _result = database
                    .get_references_for_symbol(workspace_id, symbol_uid, true)
                    .await?;
                cache_misses += 1; // First time always miss
                let none_edges = create_none_reference_edges(symbol_uid, 1);
                database.store_edges(&none_edges).await?;
            }
            2 => {
                // Definitions
                let _result = database
                    .get_definitions_for_symbol(workspace_id, symbol_uid)
                    .await?;
                cache_misses += 1;
                let none_edges = create_none_definition_edges(symbol_uid, 1);
                database.store_edges(&none_edges).await?;
            }
            3 => {
                // Implementations
                let _result = database
                    .get_implementations_for_symbol(workspace_id, symbol_uid)
                    .await?;
                cache_misses += 1;
                let none_edges = create_none_implementation_edges(symbol_uid, 1);
                database.store_edges(&none_edges).await?;
            }
            _ => unreachable!(),
        }

        let operation_duration = operation_start.elapsed();
        operation_measurements.push(operation_duration);
        operations_by_type
            .entry(operation_type)
            .or_insert_with(Vec::new)
            .push(operation_duration);

        // Periodically test cache hits
        if i % 50 == 25 && i > 100 {
            let previous_symbol = &symbols[i - 25];
            let cache_test_start = Instant::now();
            let result = database
                .get_call_hierarchy_for_symbol(workspace_id, previous_symbol)
                .await?;
            let cache_test_duration = cache_test_start.elapsed();

            operation_measurements.push(cache_test_duration);
            operations_by_type
                .entry("cache_hit_test")
                .or_insert_with(Vec::new)
                .push(cache_test_duration);

            if result.is_some() {
                cache_hits += 1;
            } else {
                cache_misses += 1;
            }
        }
    }

    let workload_duration = workload_start.elapsed();

    // Statistical analysis of overall workload
    let overall_stats = StatisticalSummary::from_measurements(operation_measurements);
    overall_stats.print_detailed_report("Mixed Workload Performance");

    // Per-operation-type analysis
    println!("\n🔍 Per-Operation-Type Statistical Analysis:");
    for (operation_type, measurements) in operations_by_type {
        if measurements.len() > 10 {
            let stats = StatisticalSummary::from_measurements(measurements);
            println!("\n   {} Operations:", operation_type);
            println!("     Count:       {}", stats.sample_count);
            println!("     Mean:        {:?}", stats.mean);
            println!("     P95:         {:?}", stats.p95);
            println!("     Std Dev:     {:?}", stats.std_dev);
        }
    }

    // Performance metrics
    let total_operations = cache_hits + cache_misses;
    let ops_per_second = total_operations as f64 / workload_duration.as_secs_f64();
    let cache_hit_rate = cache_hits as f64 / total_operations as f64;
    let throughput_per_symbol = num_symbols as f64 / workload_duration.as_secs_f64();

    println!("\n📊 Advanced Mixed Workload Results:");
    println!("   Total operations:     {}", total_operations);
    println!("   Symbols processed:    {}", num_symbols);
    println!("   Duration:             {:?}", workload_duration);
    println!("   Operations per sec:   {:.1}", ops_per_second);
    println!("   Symbols per sec:      {:.1}", throughput_per_symbol);
    println!("   Cache hits:           {}", cache_hits);
    println!("   Cache misses:         {}", cache_misses);
    println!("   Cache hit rate:       {:.2}%", cache_hit_rate * 100.0);
    println!("   Mean op time:         {:?}", overall_stats.mean);
    println!("   P95 op time:          {:?}", overall_stats.p95);
    println!(
        "   Operation consistency: {:?} std dev",
        overall_stats.std_dev
    );

    // Enhanced validation
    assert!(
        ops_per_second > 400.0,
        "Should achieve at least 400 mixed ops/sec, got {:.1}",
        ops_per_second
    );
    assert!(
        throughput_per_symbol > 200.0,
        "Should process at least 200 symbols/sec, got {:.1}",
        throughput_per_symbol
    );
    assert!(
        overall_stats.p95 < Duration::from_millis(10),
        "P95 operation time should be under 10ms"
    );
    assert!(
        cache_hit_rate > 0.05,
        "Should achieve at least 5% cache hit rate in mixed workload"
    );

    // Performance consistency validation
    let coefficient_of_variation =
        overall_stats.std_dev.as_nanos() as f64 / overall_stats.mean.as_nanos() as f64;
    assert!(
        coefficient_of_variation < 2.0,
        "Operations should have reasonable consistency (CV < 2.0)"
    );

    println!("\n✅ Advanced mixed workload statistical analysis completed");

    Ok(())
}
