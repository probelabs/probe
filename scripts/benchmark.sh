#!/bin/bash

# Performance benchmarking script for Probe
# This script provides convenient wrappers for running various benchmark scenarios

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    print_error "Cargo is not installed or not in PATH"
    exit 1
fi

# Create benchmark results directory
mkdir -p target/benchmark-results

# Function to run a specific benchmark
run_benchmark() {
    local name=$1
    local bench_type=$2
    local extra_args=$3
    
    print_status "Running $name benchmarks..."
    
    local output_file="target/benchmark-results/${name}-$(date +%Y%m%d-%H%M%S).txt"
    
    if cargo bench --bench "${bench_type}" -- $extra_args > "$output_file" 2>&1; then
        print_status "✓ $name benchmarks completed successfully"
        print_status "Results saved to: $output_file"
    else
        print_error "✗ $name benchmarks failed"
        cat "$output_file" 1>&2
        return 1
    fi
}

# Function to run all benchmarks
run_all_benchmarks() {
    print_status "Running comprehensive performance benchmarks..."
    
    # Run search benchmarks
    run_benchmark "search" "search_benchmarks" ""
    
    # Run timing benchmarks
    run_benchmark "timing" "timing_benchmarks" ""
    
    # Run parsing benchmarks
    run_benchmark "parsing" "parsing_benchmarks" ""
    
    print_status "All benchmarks completed successfully!"
}

# Function to run quick benchmarks
run_quick_benchmarks() {
    print_status "Running quick benchmarks..."
    
    # Run search benchmarks with reduced samples
    run_benchmark "search-quick" "search_benchmarks" "--quick"
    
    # Run timing benchmarks with reduced samples
    run_benchmark "timing-quick" "timing_benchmarks" "--quick"
    
    print_status "Quick benchmarks completed!"
}

# Function to run performance regression tests
run_regression_tests() {
    print_status "Running performance regression tests..."
    
    # Create baseline if it doesn't exist
    if [ ! -d "target/criterion" ]; then
        print_warning "No baseline found. Creating initial baseline..."
        cargo bench --bench search_benchmarks -- --save-baseline initial
    fi
    
    # Run benchmarks and compare with baseline
    run_benchmark "regression" "search_benchmarks" "--load-baseline initial"
    
    print_status "Performance regression tests completed!"
}

# Function to run memory profiling
run_memory_profiling() {
    print_status "Running memory profiling..."
    
    if command -v valgrind &> /dev/null; then
        cargo build --release
        valgrind --tool=massif --massif-out-file=target/benchmark-results/memory-profile.out \
            ./target/release/probe "HashMap" . --max-results 100 > /dev/null 2>&1
        
        if command -v ms_print &> /dev/null; then
            ms_print target/benchmark-results/memory-profile.out > target/benchmark-results/memory-profile.txt
            print_status "Memory profile saved to: target/benchmark-results/memory-profile.txt"
        else
            print_warning "ms_print not found. Raw memory profile saved to: target/benchmark-results/memory-profile.out"
        fi
    else
        print_warning "Valgrind not found. Skipping memory profiling."
        print_warning "Install valgrind to enable memory profiling: sudo apt-get install valgrind (Ubuntu/Debian)"
    fi
}

# Function to generate performance report
generate_report() {
    print_status "Generating performance report..."
    
    local report_file="target/benchmark-results/performance-report-$(date +%Y%m%d-%H%M%S).md"
    
    cat > "$report_file" << EOF
# Performance Benchmark Report

Generated on: $(date)

## System Information
- OS: $(uname -s)
- Architecture: $(uname -m)
- Rust Version: $(rustc --version)
- Cargo Version: $(cargo --version)

## Benchmark Results

### Search Benchmarks
$(find target/benchmark-results -name "search-*.txt" -newer target/benchmark-results/performance-report*.md 2>/dev/null | head -1 | xargs cat 2>/dev/null || echo "No recent search benchmark results found")

### Timing Benchmarks
$(find target/benchmark-results -name "timing-*.txt" -newer target/benchmark-results/performance-report*.md 2>/dev/null | head -1 | xargs cat 2>/dev/null || echo "No recent timing benchmark results found")

### Parsing Benchmarks
$(find target/benchmark-results -name "parsing-*.txt" -newer target/benchmark-results/performance-report*.md 2>/dev/null | head -1 | xargs cat 2>/dev/null || echo "No recent parsing benchmark results found")

## Criterion HTML Reports
Detailed HTML reports are available at: target/criterion/report/index.html

## Performance Recommendations
1. Monitor search pattern performance for common queries
2. Optimize parsing for frequently used languages
3. Consider caching strategies for repeated searches
4. Review memory usage for large codebases

EOF
    
    print_status "Performance report generated: $report_file"
}

# Function to show help
show_help() {
    cat << EOF
Probe Performance Benchmarking Script

Usage: $0 [COMMAND] [OPTIONS]

Commands:
    all         Run all benchmarks (default)
    quick       Run quick benchmarks with reduced samples
    search      Run search-related benchmarks only
    timing      Run timing infrastructure benchmarks only
    parsing     Run parsing benchmarks only
    regression  Run performance regression tests
    memory      Run memory profiling with valgrind
    report      Generate comprehensive performance report
    help        Show this help message

Options:
    --baseline NAME    Use specific baseline for comparison
    --save-baseline    Save current results as baseline
    --verbose         Enable verbose output

Examples:
    $0 all                    # Run all benchmarks
    $0 quick                  # Run quick benchmarks
    $0 search --verbose       # Run search benchmarks with verbose output
    $0 regression             # Run regression tests
    $0 memory                 # Run memory profiling

EOF
}

# Parse command line arguments
COMMAND=${1:-all}
VERBOSE=false
BASELINE=""
SAVE_BASELINE=false

shift || true

while [[ $# -gt 0 ]]; do
    case $1 in
        --verbose)
            VERBOSE=true
            shift
            ;;
        --baseline)
            BASELINE="$2"
            shift 2
            ;;
        --save-baseline)
            SAVE_BASELINE=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Main execution
print_status "Starting Probe performance benchmarks..."
print_status "Command: $COMMAND"

case $COMMAND in
    all)
        run_all_benchmarks
        ;;
    quick)
        run_quick_benchmarks
        ;;
    search)
        run_benchmark "search" "search_benchmarks" ""
        ;;
    timing)
        run_benchmark "timing" "timing_benchmarks" ""
        ;;
    parsing)
        run_benchmark "parsing" "parsing_benchmarks" ""
        ;;
    regression)
        run_regression_tests
        ;;
    memory)
        run_memory_profiling
        ;;
    report)
        generate_report
        ;;
    help)
        show_help
        ;;
    *)
        print_error "Unknown command: $COMMAND"
        show_help
        exit 1
        ;;
esac

print_status "Benchmark session completed!"