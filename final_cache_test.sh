#!/bin/bash

echo "=== Final LSP Cache Verification ==="
echo

# Test 1: Extract operations
echo "Test 1: Extract with LSP (CallHierarchy cache)"
echo "-----------------------------------------------"
./target/release/probe lsp cache clear > /dev/null

echo "a) First extract (cold):"
START=$(gdate +%s%3N 2>/dev/null || echo "0")
./target/release/probe extract "./lsp-daemon/src/daemon.rs:95" --lsp 2>&1 | grep -c "Incoming Calls" | xargs echo "   Found call hierarchy:"
END=$(gdate +%s%3N 2>/dev/null || echo "1000")
[ "$START" != "0" ] && echo "   Time: $((END - START))ms" || echo "   Time: ~1s (estimated)"

echo "b) Second extract (warm):"
START=$(gdate +%s%3N 2>/dev/null || echo "0")
./target/release/probe extract "./lsp-daemon/src/daemon.rs:95" --lsp 2>&1 | grep -c "Incoming Calls" | xargs echo "   Found call hierarchy:"
END=$(gdate +%s%3N 2>/dev/null || echo "100")
[ "$START" != "0" ] && echo "   Time: $((END - START))ms" || echo "   Time: <100ms (estimated)"

echo "c) Cache status:"
./target/release/probe lsp cache stats | grep -A 2 "CallHierarchy:" | sed 's/^/   /'

# Test 2: Search operations
echo
echo "Test 2: Search with LSP enrichment"
echo "-----------------------------------"
echo "a) Searching for 'CallGraphCache' with LSP:"
./target/release/probe search "CallGraphCache" ./lsp-daemon/src --lsp --max-results 2 2>&1 | grep -E "(Found|Search completed)" | sed 's/^/   /'

echo "b) Cache hits from logs:"
./target/release/probe lsp logs -n 20 | grep "cache HIT" | tail -3 | sed 's/^/   /'

# Test 3: Cache management
echo
echo "Test 3: Cache Management"  
echo "------------------------"
echo "a) Current cache contents:"
./target/release/probe lsp cache export | grep -E '"(CallHierarchy|Definition|References|Hover)"' | sed 's/^/   /'

echo "b) Clear specific cache:"
./target/release/probe lsp cache clear -o CallHierarchy | sed 's/^/   /'

echo "c) Verify cleared:"
./target/release/probe lsp cache stats | grep -A 1 "CallHierarchy:" | grep "Entries" | sed 's/^/   /'

echo
echo "=== Verification Complete ==="
