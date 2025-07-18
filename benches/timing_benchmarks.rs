use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use probe::search::search_runner::{format_duration, SearchTimings};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Benchmark the timing infrastructure overhead
fn benchmark_timing_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("timing_overhead");

    // Test timing collection overhead
    group.bench_function("timing_collection", |b| {
        b.iter(|| {
            let start = Instant::now();

            // Simulate work
            black_box((0..1000).map(|i| i * 2).collect::<Vec<_>>());

            let duration = start.elapsed();
            black_box(duration)
        })
    });

    // Test SearchTimings struct creation
    group.bench_function("search_timings_creation", |b| {
        b.iter(|| {
            let timings = SearchTimings {
                query_preprocessing: Some(Duration::from_millis(10)),
                pattern_generation: Some(Duration::from_millis(5)),
                file_searching: Some(Duration::from_millis(100)),
                filename_matching: Some(Duration::from_millis(20)),
                early_filtering: Some(Duration::from_millis(30)),
                early_caching: Some(Duration::from_millis(15)),
                result_processing: Some(Duration::from_millis(200)),
                result_processing_file_io: Some(Duration::from_millis(50)),
                result_processing_line_collection: Some(Duration::from_millis(25)),
                result_processing_ast_parsing: Some(Duration::from_millis(75)),
                result_processing_block_extraction: Some(Duration::from_millis(40)),
                result_processing_result_building: Some(Duration::from_millis(60)),
                result_processing_ast_parsing_language_init: Some(Duration::from_millis(5)),
                result_processing_ast_parsing_parser_init: Some(Duration::from_millis(3)),
                result_processing_ast_parsing_tree_parsing: Some(Duration::from_millis(45)),
                result_processing_ast_parsing_line_map_building: Some(Duration::from_millis(8)),
                result_processing_block_extraction_code_structure: Some(Duration::from_millis(15)),
                result_processing_block_extraction_filtering: Some(Duration::from_millis(10)),
                result_processing_block_extraction_result_building: Some(Duration::from_millis(20)),
                result_processing_term_matching: Some(Duration::from_millis(12)),
                result_processing_compound_processing: Some(Duration::from_millis(8)),
                result_processing_line_matching: Some(Duration::from_millis(18)),
                result_processing_result_creation: Some(Duration::from_millis(25)),
                result_processing_synchronization: Some(Duration::from_millis(5)),
                result_processing_uncovered_lines: Some(Duration::from_millis(15)),
                result_ranking: Some(Duration::from_millis(35)),
                limit_application: Some(Duration::from_millis(2)),
                block_merging: Some(Duration::from_millis(8)),
                final_caching: Some(Duration::from_millis(12)),
                total_search_time: Some(Duration::from_millis(500)),
            };
            black_box(timings)
        })
    });

    group.finish();
}

/// Benchmark duration formatting
fn benchmark_duration_formatting(c: &mut Criterion) {
    let durations = [
        Duration::from_nanos(500),
        Duration::from_micros(100),
        Duration::from_millis(50),
        Duration::from_millis(999),
        Duration::from_secs(1),
        Duration::from_secs(10),
        Duration::from_secs(60),
    ];

    let mut group = c.benchmark_group("duration_formatting");

    for (i, duration) in durations.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("format_duration", i),
            duration,
            |b, duration| b.iter(|| black_box(format_duration(*duration))),
        );
    }

    group.finish();
}

/// Benchmark timing aggregation
fn benchmark_timing_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("timing_aggregation");

    // Create sample timings data
    let create_sample_timings = || {
        vec![
            Duration::from_millis(10),
            Duration::from_millis(25),
            Duration::from_millis(5),
            Duration::from_millis(50),
            Duration::from_millis(100),
            Duration::from_millis(75),
            Duration::from_millis(30),
            Duration::from_millis(15),
            Duration::from_millis(8),
            Duration::from_millis(45),
        ]
    };

    // Test aggregation operations
    group.bench_function("sum_durations", |b| {
        b.iter(|| {
            let durations = create_sample_timings();
            let total: Duration = durations.iter().sum();
            black_box(total)
        })
    });

    group.bench_function("average_duration", |b| {
        b.iter(|| {
            let durations = create_sample_timings();
            let total: Duration = durations.iter().sum();
            let avg = total / durations.len() as u32;
            black_box(avg)
        })
    });

    group.bench_function("min_max_duration", |b| {
        b.iter(|| {
            let durations = create_sample_timings();
            let min = *durations.iter().min().unwrap();
            let max = *durations.iter().max().unwrap();
            black_box((min, max))
        })
    });

    group.finish();
}

/// Benchmark timing storage and retrieval
fn benchmark_timing_storage(c: &mut Criterion) {
    let mut group = c.benchmark_group("timing_storage");

    // Test HashMap storage for timing data
    group.bench_function("hashmap_storage", |b| {
        b.iter(|| {
            let mut timings = HashMap::new();
            timings.insert("query_preprocessing", Duration::from_millis(10));
            timings.insert("pattern_generation", Duration::from_millis(5));
            timings.insert("file_searching", Duration::from_millis(100));
            timings.insert("filename_matching", Duration::from_millis(20));
            timings.insert("early_filtering", Duration::from_millis(30));
            timings.insert("result_processing", Duration::from_millis(200));
            timings.insert("result_ranking", Duration::from_millis(35));
            timings.insert("limit_application", Duration::from_millis(2));
            timings.insert("block_merging", Duration::from_millis(8));
            timings.insert("final_caching", Duration::from_millis(12));

            // Simulate retrieval
            let total: Duration = timings.values().sum();
            black_box(total)
        })
    });

    // Test Vec storage for timing data
    group.bench_function("vec_storage", |b| {
        b.iter(|| {
            let timings = vec![
                ("query_preprocessing", Duration::from_millis(10)),
                ("pattern_generation", Duration::from_millis(5)),
                ("file_searching", Duration::from_millis(100)),
                ("filename_matching", Duration::from_millis(20)),
                ("early_filtering", Duration::from_millis(30)),
                ("result_processing", Duration::from_millis(200)),
                ("result_ranking", Duration::from_millis(35)),
                ("limit_application", Duration::from_millis(2)),
                ("block_merging", Duration::from_millis(8)),
                ("final_caching", Duration::from_millis(12)),
            ];

            // Simulate retrieval
            let total: Duration = timings.iter().map(|(_, d)| *d).sum();
            black_box(total)
        })
    });

    group.finish();
}

/// Benchmark debug mode timing output
fn benchmark_debug_output(c: &mut Criterion) {
    let mut group = c.benchmark_group("debug_output");

    // Create sample timings
    let create_sample_search_timings = || SearchTimings {
        query_preprocessing: Some(Duration::from_millis(10)),
        pattern_generation: Some(Duration::from_millis(5)),
        file_searching: Some(Duration::from_millis(100)),
        filename_matching: Some(Duration::from_millis(20)),
        early_filtering: Some(Duration::from_millis(30)),
        early_caching: Some(Duration::from_millis(15)),
        result_processing: Some(Duration::from_millis(200)),
        result_processing_file_io: Some(Duration::from_millis(50)),
        result_processing_line_collection: Some(Duration::from_millis(25)),
        result_processing_ast_parsing: Some(Duration::from_millis(75)),
        result_processing_block_extraction: Some(Duration::from_millis(40)),
        result_processing_result_building: Some(Duration::from_millis(60)),
        result_processing_ast_parsing_language_init: Some(Duration::from_millis(5)),
        result_processing_ast_parsing_parser_init: Some(Duration::from_millis(3)),
        result_processing_ast_parsing_tree_parsing: Some(Duration::from_millis(45)),
        result_processing_ast_parsing_line_map_building: Some(Duration::from_millis(8)),
        result_processing_block_extraction_code_structure: Some(Duration::from_millis(15)),
        result_processing_block_extraction_filtering: Some(Duration::from_millis(10)),
        result_processing_block_extraction_result_building: Some(Duration::from_millis(20)),
        result_processing_term_matching: Some(Duration::from_millis(12)),
        result_processing_compound_processing: Some(Duration::from_millis(8)),
        result_processing_line_matching: Some(Duration::from_millis(18)),
        result_processing_result_creation: Some(Duration::from_millis(25)),
        result_processing_synchronization: Some(Duration::from_millis(5)),
        result_processing_uncovered_lines: Some(Duration::from_millis(15)),
        result_ranking: Some(Duration::from_millis(35)),
        limit_application: Some(Duration::from_millis(2)),
        block_merging: Some(Duration::from_millis(8)),
        final_caching: Some(Duration::from_millis(12)),
        total_search_time: Some(Duration::from_millis(500)),
    };

    // Note: We can't actually test print_timings in benchmarks since it uses println!
    // Instead, we'll benchmark the logic that would be involved
    group.bench_function("timing_analysis", |b| {
        b.iter(|| {
            let timings = create_sample_search_timings();

            // Simulate the work that print_timings would do
            let mut total_time = Duration::from_secs(0);

            if let Some(d) = timings.query_preprocessing {
                total_time += d;
            }
            if let Some(d) = timings.pattern_generation {
                total_time += d;
            }
            if let Some(d) = timings.file_searching {
                total_time += d;
            }
            if let Some(d) = timings.filename_matching {
                total_time += d;
            }
            if let Some(d) = timings.early_filtering {
                total_time += d;
            }
            if let Some(d) = timings.result_processing {
                total_time += d;
            }
            if let Some(d) = timings.result_ranking {
                total_time += d;
            }
            if let Some(d) = timings.limit_application {
                total_time += d;
            }
            if let Some(d) = timings.block_merging {
                total_time += d;
            }
            if let Some(d) = timings.final_caching {
                total_time += d;
            }

            // Simulate formatting
            let formatted = format_duration(total_time);
            black_box(formatted)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_timing_overhead,
    benchmark_duration_formatting,
    benchmark_timing_aggregation,
    benchmark_timing_storage,
    benchmark_debug_output
);
criterion_main!(benches);
