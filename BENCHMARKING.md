# Benchmarking System

This document describes the comprehensive benchmarking system for the Probe code search tool.

## Overview

The benchmarking system is designed to measure and track performance across different aspects of the search engine:

1. **Search Performance** - Different search patterns, result limits, and options
2. **Timing Infrastructure** - Overhead and accuracy of timing measurements
3. **Language Parsing** - AST parsing performance for different languages
4. **Memory Usage** - Memory profiling and optimization

## Running Benchmarks

### Using the CLI

```bash
# Run all benchmarks
probe benchmark

# Run specific benchmark suites
probe benchmark --bench search
probe benchmark --bench timing
probe benchmark --bench parsing

# Run with custom settings
probe benchmark --sample-size 100 --format json --output results.json

# Compare with baseline
probe benchmark --compare --baseline previous

# Quick benchmarks (faster, less accurate)
probe benchmark --fast
```

### Using the Script

```bash
# Make script executable (one time)
chmod +x scripts/benchmark.sh

# Run all benchmarks
./scripts/benchmark.sh all

# Run quick benchmarks
./scripts/benchmark.sh quick

# Run specific benchmark type
./scripts/benchmark.sh search
./scripts/benchmark.sh timing
./scripts/benchmark.sh parsing

# Performance regression testing
./scripts/benchmark.sh regression

# Memory profiling (requires valgrind)
./scripts/benchmark.sh memory

# Generate performance report
./scripts/benchmark.sh report
```

### Using Cargo Directly

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark file
cargo bench --bench search_benchmarks
cargo bench --bench timing_benchmarks
cargo bench --bench parsing_benchmarks

# Run with criterion options
cargo bench -- --quick
cargo bench -- --sample-size 100
cargo bench -- --baseline main
```

## Benchmark Types

### Search Benchmarks (`search_benchmarks.rs`)

Tests various search scenarios:

- **Search Patterns**: Different query types (simple, function calls, types, etc.)
- **Result Limits**: Performance with different result set sizes
- **Search Options**: Different rerankers, frequency search on/off
- **Query Complexity**: Single terms vs. compound queries

### Timing Benchmarks (`timing_benchmarks.rs`)

Tests the timing infrastructure:

- **Timing Overhead**: Cost of collecting timing data
- **Duration Formatting**: Performance of human-readable formatting
- **Timing Aggregation**: Sum, average, min/max calculations
- **Timing Storage**: HashMap vs. Vec storage performance

### Parsing Benchmarks (`parsing_benchmarks.rs`)

Tests language parsing performance:

- **Language Parsing**: Rust, JavaScript, Python, Go parsing
- **File Sizes**: Small, medium, large, extra-large files
- **Line Filtering**: Different line number set sizes
- **Test Inclusion**: With/without test file processing

## Existing Performance Logging

The codebase already has extensive timing infrastructure:

### SearchTimings Structure
```rust
pub struct SearchTimings {
    pub query_preprocessing: Option<Duration>,
    pub pattern_generation: Option<Duration>,
    pub file_searching: Option<Duration>,
    pub filename_matching: Option<Duration>,
    pub early_filtering: Option<Duration>,
    pub result_processing: Option<Duration>,
    pub result_ranking: Option<Duration>,
    pub total_search_time: Option<Duration>,
    // ... many more granular timings
}
```

### FileProcessingTimings Structure
```rust
pub struct FileProcessingTimings {
    pub file_io: Option<Duration>,
    pub ast_parsing: Option<Duration>,
    pub block_extraction: Option<Duration>,
    pub result_building: Option<Duration>,
    // ... detailed sub-timings for each stage
}
```

### Debug Mode
Enable detailed timing output:
```bash
DEBUG=1 probe "search_term" /path/to/code
```

## Performance Monitoring

### Regression Detection

The benchmarking system includes performance regression detection:

1. **Baseline Creation**: Save current performance as baseline
2. **Comparison**: Compare new runs against baseline
3. **Alerting**: Identify significant performance changes

### Memory Profiling

Memory usage analysis using Valgrind:

```bash
# Run memory profiling
./scripts/benchmark.sh memory

# View memory profile
ms_print target/benchmark-results/memory-profile.out
```

### Continuous Monitoring

For CI/CD integration:

```bash
# Quick benchmarks for CI
cargo bench -- --quick

# Regression testing
cargo bench -- --load-baseline main

# Save results for future comparison
cargo bench -- --save-baseline $(git rev-parse --short HEAD)
```

## Benchmark Results

### Output Locations

- **Criterion HTML Reports**: `target/criterion/report/index.html`
- **JSON Results**: `target/criterion/*/base/estimates.json`
- **Script Results**: `target/benchmark-results/`

### Interpreting Results

Key metrics to monitor:

1. **Search Time**: Total time for search operations
2. **Parsing Time**: AST parsing performance
3. **Memory Usage**: Peak memory consumption
4. **Throughput**: Operations per second

### Performance Targets

Recommended performance targets:

- **Simple Search**: < 100ms for small codebases (< 1MB)
- **Complex Search**: < 500ms for medium codebases (< 10MB)
- **Parsing**: < 50ms per file for typical source files
- **Memory**: < 100MB for typical search operations

## Optimization Guidelines

### Search Performance

1. **Pattern Optimization**: Use specific patterns over broad searches
2. **Result Limits**: Set appropriate max_results limits
3. **Language Filtering**: Use language-specific searches when possible
4. **Caching**: Leverage session caching for repeated searches

### Parsing Performance

1. **File Size**: Large files have higher parsing overhead
2. **Language Choice**: Some languages parse faster than others
3. **Line Filtering**: Specific line ranges are more efficient
4. **Test Exclusion**: Exclude test files when not needed

### Memory Usage

1. **Result Size**: Large result sets consume more memory
2. **Concurrent Processing**: Balance parallelism vs. memory usage
3. **Caching Strategy**: Monitor cache memory consumption
4. **File Processing**: Process large files in chunks

## Troubleshooting

### Common Issues

1. **Benchmark Fails**: Check cargo and criterion installation
2. **Memory Profiling**: Requires valgrind installation
3. **Slow Benchmarks**: Use `--quick` flag for faster results
4. **Missing Baselines**: Create initial baseline with `--save-baseline`

### Performance Debugging

1. **Enable Debug Mode**: `DEBUG=1` for detailed timing
2. **Profile Specific Operations**: Use targeted benchmarks
3. **Memory Analysis**: Run memory profiling for memory issues
4. **Regression Analysis**: Compare with previous versions

## Future Enhancements

Potential improvements to the benchmarking system:

1. **Automated Alerts**: Performance regression notifications
2. **Historical Tracking**: Long-term performance trends
3. **Comparative Analysis**: Cross-platform performance comparison
4. **Load Testing**: High-concurrency performance testing
5. **Real-world Scenarios**: Benchmark with actual codebases