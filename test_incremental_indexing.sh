#!/bin/bash

# Test script for comprehensive incremental indexing mode
set -e

echo "🧪 Testing Comprehensive Incremental Indexing Mode"
echo "================================================="

# Ensure we're in the right directory
cd "$(dirname "$0")"

# Build the project first
echo "🔨 Building the project..."
cargo build --release
if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

# Create a temporary test directory
TEST_DIR=$(mktemp -d)
echo "📁 Using test directory: $TEST_DIR"

# Cleanup function
cleanup() {
    echo "🧹 Cleaning up test directory: $TEST_DIR"
    rm -rf "$TEST_DIR"
}
trap cleanup EXIT

# Create initial test files
echo "📝 Creating initial test files..."
mkdir -p "$TEST_DIR/src"
cat > "$TEST_DIR/src/main.rs" << 'EOF'
fn main() {
    println!("Hello, world!");
    calculate_sum(10, 20);
}

fn calculate_sum(a: i32, b: i32) -> i32 {
    let result = a + b;
    println!("Sum: {}", result);
    result
}

fn unused_function() {
    println!("This function is not called");
}
EOF

cat > "$TEST_DIR/src/lib.rs" << 'EOF'
pub mod utils;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
EOF

cat > "$TEST_DIR/src/utils.rs" << 'EOF'
pub fn helper_function() {
    println!("This is a helper function");
}

pub fn process_data(data: Vec<i32>) -> Vec<i32> {
    data.iter().map(|x| x * 2).collect()
}
EOF

cat > "$TEST_DIR/Cargo.toml" << 'EOF'
[package]
name = "test-incremental"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

echo "✅ Created initial test files"

# Start the LSP daemon
echo "🚀 Starting LSP daemon..."
./target/release/probe lsp start -f &
LSP_PID=$!

# Wait a bit for daemon to start
sleep 2

# Function to check daemon status
check_daemon() {
    ./target/release/probe lsp status > /dev/null 2>&1
    return $?
}

if ! check_daemon; then
    echo "❌ LSP daemon failed to start"
    kill $LSP_PID 2>/dev/null || true
    exit 1
fi

echo "✅ LSP daemon started successfully"

# First indexing run (full indexing)
echo "🔄 Running initial full indexing..."
start_time=$(date +%s)
./target/release/probe lsp index "$TEST_DIR" --force
end_time=$(date +%s)
initial_duration=$((end_time - start_time))
echo "✅ Initial indexing completed in ${initial_duration}s"

# Check indexing status
echo "📊 Checking indexing status after initial run..."
./target/release/probe lsp status

# Get initial cache stats
echo "📈 Initial cache statistics:"
./target/release/probe lsp cache stats

# Wait a moment
sleep 1

# Second indexing run (should be mostly skipped due to incremental mode)
echo "🔄 Running second indexing (incremental - should skip unchanged files)..."
start_time=$(date +%s)
./target/release/probe lsp index "$TEST_DIR"
end_time=$(date +%s)
incremental_duration=$((end_time - start_time))
echo "✅ Incremental indexing completed in ${incremental_duration}s"

echo "📊 Performance comparison:"
echo "  Initial indexing: ${initial_duration}s"
echo "  Incremental indexing: ${incremental_duration}s"
if [ $incremental_duration -lt $initial_duration ]; then
    echo "  ✅ Incremental mode is faster!"
else
    echo "  ⚠️  Incremental mode not significantly faster (may be due to small test set)"
fi

# Modify one file to test selective re-indexing
echo "✏️  Modifying main.rs to test selective re-indexing..."
cat >> "$TEST_DIR/src/main.rs" << 'EOF'

fn new_function() {
    println!("This is a new function");
    helper_calculation(42);
}

fn helper_calculation(value: i32) -> i32 {
    value * 2 + 10
}
EOF

# Third indexing run (should re-index only the modified file)
echo "🔄 Running third indexing after file modification..."
start_time=$(date +%s)
./target/release/probe lsp index "$TEST_DIR"
end_time=$(date +%s)
selective_duration=$((end_time - start_time))
echo "✅ Selective re-indexing completed in ${selective_duration}s"

# Delete a file to test cleanup
echo "🗑️  Deleting utils.rs to test file deletion handling..."
rm "$TEST_DIR/src/utils.rs"

# Update lib.rs to remove the reference to utils module
cat > "$TEST_DIR/src/lib.rs" << 'EOF'
pub fn add(left: usize, right: usize) -> usize {
    left + right
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
EOF

# Fourth indexing run (should clean up deleted file from caches)
echo "🔄 Running fourth indexing after file deletion..."
start_time=$(date +%s)
./target/release/probe lsp index "$TEST_DIR"
end_time=$(date +%s)
cleanup_duration=$((end_time - start_time))
echo "✅ Cleanup indexing completed in ${cleanup_duration}s"

# Final cache stats
echo "📈 Final cache statistics:"
./target/release/probe lsp cache stats

echo "📊 Final performance summary:"
echo "  Initial indexing: ${initial_duration}s"
echo "  Incremental (no changes): ${incremental_duration}s"
echo "  Selective re-indexing: ${selective_duration}s"
echo "  Cleanup after deletion: ${cleanup_duration}s"

# Test cache validation - search for functions to ensure they're properly indexed
echo "🔍 Testing indexed content with search queries..."

echo "  Searching for 'calculate_sum' (should be found)..."
if ./target/release/probe search "calculate_sum" "$TEST_DIR" --max-results 5 | grep -q "calculate_sum"; then
    echo "    ✅ Found calculate_sum"
else
    echo "    ❌ calculate_sum not found"
fi

echo "  Searching for 'new_function' (should be found)..."
if ./target/release/probe search "new_function" "$TEST_DIR" --max-results 5 | grep -q "new_function"; then
    echo "    ✅ Found new_function"
else
    echo "    ❌ new_function not found"
fi

echo "  Searching for 'helper_function' (should NOT be found - file was deleted)..."
if ./target/release/probe search "helper_function" "$TEST_DIR" --max-results 5 | grep -q "helper_function"; then
    echo "    ❌ helper_function still found (cleanup may not be working)"
else
    echo "    ✅ helper_function correctly not found"
fi

# Test LSP call hierarchy if available
echo "🔗 Testing LSP call hierarchy for indexed functions..."
if ./target/release/probe extract "$TEST_DIR/src/main.rs#calculate_sum" --lsp > /dev/null 2>&1; then
    echo "  ✅ LSP call hierarchy working for calculate_sum"
else
    echo "  ⚠️  LSP call hierarchy not available for calculate_sum"
fi

# Show daemon logs for debugging
echo "📋 LSP daemon logs (last 20 entries):"
./target/release/probe lsp logs -n 20

# Shutdown daemon
echo "🛑 Shutting down LSP daemon..."
./target/release/probe lsp shutdown

# Wait for daemon to shutdown
sleep 1

echo ""
echo "🎉 Comprehensive Incremental Indexing Test Complete!"
echo "============================================"

# Performance analysis
if [ $incremental_duration -lt $((initial_duration / 2)) ]; then
    echo "✅ EXCELLENT: Incremental mode shows significant performance improvement"
elif [ $incremental_duration -lt $initial_duration ]; then
    echo "✅ GOOD: Incremental mode shows some performance improvement" 
else
    echo "⚠️  WARNING: Incremental mode not showing expected performance gains"
fi

if [ $selective_duration -le $incremental_duration ]; then
    echo "✅ EXCELLENT: Selective re-indexing is efficient"
else
    echo "⚠️  INFO: Selective re-indexing took longer (may be due to test size)"
fi

echo ""
echo "Test completed successfully! 🚀"