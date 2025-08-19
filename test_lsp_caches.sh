#!/bin/bash

echo "=== LSP Cache Performance Test ==="
echo

# Clear caches first
echo "1. Clearing all caches..."
./target/release/probe lsp cache clear > /dev/null

# Test 1: Call hierarchy (cold cache)
echo "2. Testing call hierarchy (cold cache)..."
echo -n "   First run (cold): "
time -p ./target/release/probe extract "./lsp-daemon/src/daemon.rs:100" --lsp > /dev/null 2>&1

# Test 2: Call hierarchy (warm cache)
echo "3. Testing call hierarchy (warm cache)..."
echo -n "   Second run (warm): "
time -p ./target/release/probe extract "./lsp-daemon/src/daemon.rs:100" --lsp > /dev/null 2>&1

# Test 3: Run multiple times to show consistent cache hits
echo "4. Running 3 more times to show consistent cache performance:"
for i in {1..3}; do
    echo -n "   Run $i: "
    time -p ./target/release/probe extract "./lsp-daemon/src/daemon.rs:100" --lsp > /dev/null 2>&1
done

# Show cache stats
echo
echo "5. Cache statistics after operations:"
./target/release/probe lsp cache stats

# Check the daemon logs for cache hits
echo
echo "6. Recent cache activity from daemon logs:"
./target/release/probe lsp logs -n 10 | grep -i "cache hit\|cache miss" || echo "   (no recent cache activity in logs)"

echo
echo "=== Test Complete ==="
