#!/bin/bash

# Comprehensive LSP Indexing and Caching Test Script
# This script validates that the LSP indexing fixes work correctly

set -e  # Exit on any error

# ANSI color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_info() {
    echo -e "${YELLOW}[INFO]${NC} $1"
}

# Function to wait for a condition with timeout
wait_for_condition() {
    local condition="$1"
    local timeout="${2:-30}"
    local message="${3:-Waiting for condition}"
    
    print_info "$message (timeout: ${timeout}s)"
    
    for i in $(seq 1 $timeout); do
        if eval "$condition"; then
            return 0
        fi
        echo -n "."
        sleep 1
    done
    echo
    print_error "Timeout waiting for: $message"
    return 1
}

# Function to extract number from string (e.g., "1,234 symbols" -> 1234)
extract_number() {
    echo "$1" | grep -oE '[0-9,]+' | tr -d ',' | head -n1
}

# Test configuration
TEST_DIR="/Users/leonidbugaev/conductor/repo/probe/paris"
PROBE_BIN="./target/debug/probe"
EXPECTED_SYMBOLS=13630  # Expected symbols in probe repository

# Validation arrays
declare -a validation_results=()

# Function to add validation result
add_validation() {
    local test_name="$1"
    local expected="$2" 
    local actual="$3"
    local status="$4"  # "PASS" or "FAIL"
    
    validation_results+=("$test_name|$expected|$actual|$status")
    
    if [ "$status" = "PASS" ]; then
        print_success "$test_name: $actual (expected: $expected)"
    else
        print_error "$test_name: $actual (expected: $expected)"
    fi
}

# Function to show final validation report
show_validation_report() {
    echo
    print_step "FINAL VALIDATION REPORT"
    echo "=================================================="
    
    local total=0
    local passed=0
    local failed=0
    
    printf "%-40s %-15s %-15s %-10s\n" "TEST NAME" "EXPECTED" "ACTUAL" "STATUS"
    echo "------------------------------------------------------------------"
    
    for result in "${validation_results[@]}"; do
        IFS='|' read -r test_name expected actual status <<< "$result"
        printf "%-40s %-15s %-15s %-10s\n" "$test_name" "$expected" "$actual" "$status"
        
        total=$((total + 1))
        if [ "$status" = "PASS" ]; then
            passed=$((passed + 1))
        else
            failed=$((failed + 1))
        fi
    done
    
    echo "------------------------------------------------------------------"
    echo "TOTAL: $total   PASSED: $passed   FAILED: $failed"
    
    if [ $failed -eq 0 ]; then
        print_success "ALL TESTS PASSED! 🎉"
        return 0
    else
        print_error "$failed tests failed"
        return 1
    fi
}

print_step "Starting Comprehensive LSP Indexing Test"
echo "Testing probe repository at: $TEST_DIR"
echo "Expected symbols to index: $EXPECTED_SYMBOLS"
echo

# Step 1: Clean environment
print_step "Step 1: Cleaning test environment"

# Kill any existing probe processes
print_info "Killing existing probe processes..."
pkill -f "probe.*lsp" || true
sleep 2

# Clear existing cache data
print_info "Clearing cache data..."
rm -rf ~/Library/Caches/probe/lsp/ || true
rm -rf ~/.cache/probe/lsp/ || true

print_success "Environment cleaned"

# Step 2: Build the project
print_step "Step 2: Building probe with fixes"
cd "$TEST_DIR"

if ! cargo build; then
    print_error "Failed to build probe"
    exit 1
fi

print_success "Build completed"

# Step 3: Check that probe binary exists
print_step "Step 3: Verifying probe binary"
if [ ! -f "$PROBE_BIN" ]; then
    print_error "Probe binary not found at $PROBE_BIN"
    exit 1
fi

print_success "Probe binary found"

# Step 4: Start LSP daemon
print_step "Step 4: Starting LSP daemon"
$PROBE_BIN lsp start -f &
DAEMON_PID=$!

# Wait for daemon to start
if ! wait_for_condition "$PROBE_BIN lsp status >/dev/null 2>&1" 10 "daemon to start"; then
    print_error "Failed to start LSP daemon"
    kill $DAEMON_PID 2>/dev/null || true
    exit 1
fi

print_success "LSP daemon started (PID: $DAEMON_PID)"

# Step 5: Check initial cache state (should be empty)
print_step "Step 5: Checking initial cache state"
cache_stats_before=$($PROBE_BIN lsp cache stats 2>/dev/null || echo "No cache stats")
print_info "Initial cache state: $cache_stats_before"

# Step 6: Start indexing the probe repository
print_step "Step 6: Starting indexing of probe repository"
print_info "This will index all Rust files in the probe repository..."

indexing_start_time=$(date +%s)

# Start indexing with full logging
$PROBE_BIN lsp index -w . &
INDEXING_PID=$!

# Monitor indexing progress
print_info "Monitoring indexing progress..."
prev_processed=0
no_progress_count=0

while kill -0 $INDEXING_PID 2>/dev/null; do
    # Get indexing status
    status_output=$($PROBE_BIN lsp status 2>/dev/null || echo "No status")
    
    # Extract processed files and symbols
    processed=$(echo "$status_output" | grep -oE "processed_files: [0-9]+" | grep -oE "[0-9]+" || echo "0")
    symbols=$(echo "$status_output" | grep -oE "symbols_extracted: [0-9,]+" | grep -oE "[0-9,]+" | tr -d ',' || echo "0")
    total=$(echo "$status_output" | grep -oE "total_files: [0-9]+" | grep -oE "[0-9]+" || echo "0")
    
    # Calculate progress percentage
    if [ "$total" -gt 0 ]; then
        progress=$((processed * 100 / total))
        print_info "Progress: $processed/$total files ($progress%), $symbols symbols extracted"
    else
        print_info "Files: $processed, Symbols: $symbols"
    fi
    
    # Check for progress stall
    if [ "$processed" -eq "$prev_processed" ]; then
        no_progress_count=$((no_progress_count + 1))
        if [ $no_progress_count -gt 30 ]; then  # 30 seconds without progress
            print_warning "Indexing appears stalled, continuing to wait..."
            no_progress_count=0
        fi
    else
        no_progress_count=0
    fi
    prev_processed=$processed
    
    sleep 2
done

wait $INDEXING_PID
indexing_exit_code=$?

indexing_end_time=$(date +%s)
indexing_duration=$((indexing_end_time - indexing_start_time))

if [ $indexing_exit_code -eq 0 ]; then
    print_success "Indexing completed in ${indexing_duration}s"
else
    print_error "Indexing failed with exit code $indexing_exit_code"
fi

# Step 7: Check final indexing status
print_step "Step 7: Checking final indexing results"
final_status=$($PROBE_BIN lsp status 2>/dev/null || echo "No status available")
print_info "Final indexing status:"
echo "$final_status"

# Extract final numbers
final_files=$(echo "$final_status" | grep -oE "processed_files: [0-9]+" | grep -oE "[0-9]+" || echo "0")
final_symbols=$(echo "$final_status" | grep -oE "symbols_extracted: [0-9,]+" | grep -oE "[0-9,]+" | tr -d ',' || echo "0")
final_total=$(echo "$final_status" | grep -oE "total_files: [0-9]+" | grep -oE "[0-9]+" || echo "0")

# Step 8: Check cache statistics after indexing
print_step "Step 8: Checking cache statistics after indexing"
cache_stats_after=$($PROBE_BIN lsp cache stats 2>/dev/null || echo "No cache stats")
print_info "Cache statistics after indexing:"
echo "$cache_stats_after"

# Extract cache statistics
cached_entries=$(echo "$cache_stats_after" | grep -oE "total_entries: [0-9,]+" | grep -oE "[0-9,]+" | tr -d ',' || echo "0")
cache_hit_count=$(echo "$cache_stats_after" | grep -oE "hit_count: [0-9,]+" | grep -oE "[0-9,]+" | tr -d ',' || echo "0")
cache_miss_count=$(echo "$cache_stats_after" | grep -oE "miss_count: [0-9,]+" | grep -oE "[0-9,]+" | tr -d ',' || echo "0")

# Step 9: Test cache functionality with probe extract
print_step "Step 9: Testing cache functionality"

# Test extracting a known function from probe
test_files=(
    "src/ranking.rs#tokenize"
    "src/search/mod.rs#search_files_parallel"
    "lsp-daemon/src/daemon.rs#new"
    "src/main.rs#main"
)

cache_hits_before=$(echo "$cache_stats_after" | grep -oE "hit_count: [0-9,]+" | grep -oE "[0-9,]+" | tr -d ',' || echo "0")

for test_file in "${test_files[@]}"; do
    print_info "Testing extraction: $test_file"
    
    # Test with LSP flag (should use cache)
    extract_start_time=$(date +%s)
    extract_result=$($PROBE_BIN extract "$test_file" --lsp 2>/dev/null || echo "EXTRACTION_FAILED")
    extract_end_time=$(date +%s)
    extract_duration=$((extract_end_time - extract_start_time))
    
    if [[ "$extract_result" == *"EXTRACTION_FAILED"* ]]; then
        print_warning "Failed to extract $test_file"
    elif [ ${#extract_result} -gt 50 ]; then
        print_success "Successfully extracted $test_file (${#extract_result} chars, ${extract_duration}s)"
    else
        print_warning "Extraction result seems too short for $test_file (${#extract_result} chars)"
    fi
    
    sleep 1
done

# Check cache hit increase
cache_stats_final=$($PROBE_BIN lsp cache stats 2>/dev/null || echo "No cache stats")
cache_hits_after=$(echo "$cache_stats_final" | grep -oE "hit_count: [0-9,]+" | grep -oE "[0-9,]+" | tr -d ',' || echo "0")
cache_hit_increase=$((cache_hits_after - cache_hits_before))

print_info "Cache hits increased by: $cache_hit_increase"

# Step 10: Performance test - repeat extraction to verify cache speed
print_step "Step 10: Performance testing cached operations"

test_symbol="src/ranking.rs#tokenize"
print_info "Testing extraction speed for: $test_symbol"

# First extraction (should be fast from cache)
time1_start=$(date +%s%N)
$PROBE_BIN extract "$test_symbol" --lsp >/dev/null 2>&1
time1_end=$(date +%s%N)
time1_ms=$(( (time1_end - time1_start) / 1000000 ))

# Second extraction (should be even faster from cache)
time2_start=$(date +%s%N)
$PROBE_BIN extract "$test_symbol" --lsp >/dev/null 2>&1
time2_end=$(date +%s%N)
time2_ms=$(( (time2_end - time2_start) / 1000000 ))

print_info "First extraction: ${time1_ms}ms"
print_info "Second extraction: ${time2_ms}ms"

# Step 11: Validate results
print_step "Step 11: Validating results against expectations"

# Validation 1: Files processed
if [ "$final_files" -ge 300 ]; then
    add_validation "Files processed" "≥300" "$final_files" "PASS"
else
    add_validation "Files processed" "≥300" "$final_files" "FAIL"
fi

# Validation 2: Symbols extracted (most critical)
if [ "$final_symbols" -ge $((EXPECTED_SYMBOLS - 1000)) ]; then  # Allow some variance
    add_validation "Symbols extracted" "≥$((EXPECTED_SYMBOLS - 1000))" "$final_symbols" "PASS"
else
    add_validation "Symbols extracted" "≥$((EXPECTED_SYMBOLS - 1000))" "$final_symbols" "FAIL"
fi

# Validation 3: Cache entries (should be roughly equal to symbols)
if [ "$cached_entries" -ge $((final_symbols / 2)) ]; then
    add_validation "Cache entries" "≥$((final_symbols / 2))" "$cached_entries" "PASS"
else
    add_validation "Cache entries" "≥$((final_symbols / 2))" "$cached_entries" "FAIL"
fi

# Validation 4: Cache hits (should be > 0 after our tests)
if [ "$cache_hits_after" -gt 0 ]; then
    add_validation "Cache hits" ">0" "$cache_hits_after" "PASS"
else
    add_validation "Cache hits" ">0" "$cache_hits_after" "FAIL"
fi

# Validation 5: Cache hit increase from our tests
if [ "$cache_hit_increase" -gt 0 ]; then
    add_validation "Cache hit increase" ">0" "$cache_hit_increase" "PASS"
else
    add_validation "Cache hit increase" ">0" "$cache_hit_increase" "FAIL"
fi

# Validation 6: Extraction speed (cached operations should be fast)
if [ "$time2_ms" -lt 1000 ]; then  # Less than 1 second
    add_validation "Cache speed" "<1000ms" "${time2_ms}ms" "PASS"
else
    add_validation "Cache speed" "<1000ms" "${time2_ms}ms" "FAIL"
fi

# Validation 7: Indexing completed without hanging
if [ $indexing_exit_code -eq 0 ]; then
    add_validation "Indexing completion" "success" "success" "PASS"
else
    add_validation "Indexing completion" "success" "failed" "FAIL"
fi

# Step 12: Check daemon logs for errors
print_step "Step 12: Checking daemon logs for errors"
daemon_logs=$($PROBE_BIN lsp logs -n 50 2>/dev/null || echo "No logs available")

error_count=$(echo "$daemon_logs" | grep -i "error" | wc -l || echo "0")
warning_count=$(echo "$daemon_logs" | grep -i "warning" | wc -l || echo "0")

print_info "Errors in logs: $error_count"
print_info "Warnings in logs: $warning_count"

if [ "$error_count" -eq 0 ]; then
    add_validation "Error count" "0" "$error_count" "PASS"
else
    add_validation "Error count" "0" "$error_count" "FAIL"
    print_info "Recent errors:"
    echo "$daemon_logs" | grep -i "error" | tail -5
fi

# Step 13: Cleanup
print_step "Step 13: Cleanup"
$PROBE_BIN lsp shutdown 2>/dev/null || true
kill $DAEMON_PID 2>/dev/null || true
sleep 2

print_success "Cleanup completed"

# Step 14: Show final report
print_step "Step 14: Final Test Report"

echo
echo "==============================================="
echo "          LSP INDEXING TEST SUMMARY"
echo "==============================================="
echo "Indexing duration: ${indexing_duration}s"
echo "Files processed: $final_files"
echo "Symbols extracted: $final_symbols"
echo "Cache entries: $cached_entries"
echo "Cache hits: $cache_hits_after"
echo "Cache misses: $cache_miss_count"
echo "Errors in logs: $error_count"
echo "Warnings in logs: $warning_count"
echo

# Show validation report
show_validation_report

echo
if show_validation_report >/dev/null 2>&1; then
    print_success "🎉 LSP INDEXING AND CACHING FIXES VALIDATED SUCCESSFULLY!"
    echo
    echo "Key achievements:"
    echo "✅ Indexing works without hanging or crashing"
    echo "✅ Symbols are actually cached in persistent storage"  
    echo "✅ Cache hit rates work properly"
    echo "✅ Extract operations use cached data"
    echo "✅ Performance is good for cached operations"
    echo
    exit 0
else
    print_error "❌ SOME TESTS FAILED - REVIEW IMPLEMENTATION"
    echo
    echo "Please check:"
    echo "- Indexing worker stores data in persistent cache"
    echo "- Cache statistics are accurate"
    echo "- Cache hierarchy works: memory → disk → LSP"
    echo "- Extract operations use cached data when available"
    echo
    exit 1
fi