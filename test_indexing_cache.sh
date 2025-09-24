#!/bin/bash
set -e

echo "🧪 Testing indexing cache fix"

# Build the project
echo "🔨 Building project..."
cargo build

# Start the daemon in the background
echo "🚀 Starting LSP daemon..."
./target/debug/probe lsp start -f &
DAEMON_PID=$!

# Give daemon time to start
sleep 2

# Function to clean up
cleanup() {
    echo "🧹 Cleaning up..."
    kill $DAEMON_PID 2>/dev/null || true
    ./target/debug/probe lsp shutdown 2>/dev/null || true
    wait $DAEMON_PID 2>/dev/null || true
}

# Set trap to cleanup on script exit
trap cleanup EXIT

# Check daemon status
echo "📊 Checking daemon status..."
./target/debug/probe lsp status

# Check initial cache stats
echo "📈 Initial cache stats:"
./target/debug/probe lsp cache stats

# Start indexing on current directory (which has Rust code)
echo "🔍 Starting indexing on current directory..."
./target/debug/probe lsp index -w . --wait

# Check cache stats after indexing
echo "📈 Cache stats after indexing:"
./target/debug/probe lsp cache stats

# Try to extract a symbol (should hit cache if our fix worked)
echo "🎯 Testing symbol extraction from cache..."
time ./target/debug/probe extract src/main.rs#main --lsp

echo "✅ Test completed successfully!"