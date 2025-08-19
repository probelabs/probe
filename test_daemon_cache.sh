#!/bin/bash

echo "=== Direct LSP Daemon Cache Test ==="
echo ""
echo "This test will:"
echo "1. Make a call hierarchy request (cold cache)"  
echo "2. Make the same request again (warm cache)"
echo "3. Modify the file"
echo "4. Make the request again (cache invalidated)"
echo ""

# Ensure daemon is running fresh
./target/release/probe lsp shutdown 2>/dev/null
./target/release/probe lsp start -f >/dev/null 2>&1 &
sleep 3

TEST_FILE="lsp-daemon/src/call_graph_cache.rs"
SYMBOL="default"

echo "=== Test 1: Cold Cache ==="
echo "Making first call hierarchy request..."
START=$(date +%s%N)

# Use the search command which triggers call hierarchy
./target/release/probe search "$SYMBOL" "./$TEST_FILE" --lsp --max-results 1 2>&1 | grep -E "Search completed|Outgoing Calls" | head -5

END=$(date +%s%N)
ELAPSED_MS=$(( ($END - $START) / 1000000 ))
echo "Time taken: ${ELAPSED_MS}ms"

echo ""
echo "=== Test 2: Warm Cache (should be same speed, cache stores but doesn't look up yet) ==="
START=$(date +%s%N)

./target/release/probe search "$SYMBOL" "./$TEST_FILE" --lsp --max-results 1 2>&1 | grep -E "Search completed|Outgoing Calls" | head -5

END=$(date +%s%N)
ELAPSED_MS=$(( ($END - $START) / 1000000 ))
echo "Time taken: ${ELAPSED_MS}ms"

echo ""
echo "=== Test 3: After File Modification ==="
echo "Modifying file with a comment..."
echo "// Cache test $(date)" >> "$TEST_FILE"

START=$(date +%s%N)

./target/release/probe search "$SYMBOL" "./$TEST_FILE" --lsp --max-results 1 2>&1 | grep -E "Search completed|Outgoing Calls" | head -5

END=$(date +%s%N)
ELAPSED_MS=$(( ($END - $START) / 1000000 ))
echo "Time taken: ${ELAPSED_MS}ms (should re-compute due to MD5 change)"

# Restore file
git checkout -- "$TEST_FILE" 2>/dev/null

echo ""
echo "=== Cache Statistics ==="
echo "Checking daemon logs for cache activity..."
./target/release/probe lsp logs -n 200 | grep -E "Caching call hierarchy|Computing call|Successfully cached|md5" | head -10 || echo "No cache logs found"

echo ""
echo "Total daemon requests handled:"
./target/release/probe lsp status | grep "Total Requests"

echo ""
echo "=== Test Complete ==="
echo ""
echo "Note: The cache is implemented and stores results, but currently"
echo "doesn't look up cached results on repeated calls because we need"
echo "symbol resolution to create the cache key BEFORE the LSP call."
echo "The cache will be effective when:"
echo "1. We implement symbol extraction at position"
echo "2. We can create NodeKey before making the LSP call"
echo "3. Then check cache first before calling LSP"