#!/bin/bash

echo "=== Real Operations Cache Test ==="
echo

# Clear cache
echo "1. Starting fresh..."
./target/release/probe lsp cache clear > /dev/null

# Test multiple extract operations
echo "2. Testing multiple extract operations..."
LOCATIONS=(
    "./lsp-daemon/src/daemon.rs:100"
    "./lsp-daemon/src/daemon.rs:200" 
    "./lsp-daemon/src/daemon.rs:300"
    "./lsp-daemon/src/server_manager.rs:100"
    "./lsp-daemon/src/lsp_cache.rs:100"
)

echo "   Cold cache runs:"
for loc in "${LOCATIONS[@]}"; do
    START=$(date +%s%3N 2>/dev/null || date +%s)
    ./target/release/probe extract "$loc" --lsp > /dev/null 2>&1
    END=$(date +%s%3N 2>/dev/null || date +%s)
    echo "   - $loc: ~$((END - START))ms"
done

echo
echo "3. Cache statistics after cold runs:"
./target/release/probe lsp cache stats | grep -A 2 "CallHierarchy:"

echo
echo "4. Re-running same locations (warm cache):"
for loc in "${LOCATIONS[@]}"; do
    START=$(date +%s%3N 2>/dev/null || date +%s)
    ./target/release/probe extract "$loc" --lsp > /dev/null 2>&1
    END=$(date +%s%3N 2>/dev/null || date +%s)
    echo "   - $loc: ~$((END - START))ms"
done

echo
echo "5. Cache hit confirmations from logs:"
./target/release/probe lsp logs -n 20 | grep "cache HIT" | wc -l | xargs echo "   Total cache hits:"

echo
echo "6. Testing search with LSP enrichment..."
echo "   First search (may use some cached data):"
time -p ./target/release/probe search "Arc::new" ./lsp-daemon/src --lsp --max-results 3 2>&1 | grep "Search completed"

echo
echo "7. Final cache statistics:"
./target/release/probe lsp cache stats

echo
echo "=== Test Complete ==="
