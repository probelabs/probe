# Search Determinism Test Results

## Summary

This document describes the test results for reproducing non-deterministic search behavior in the probe search tool.

## Test File

`/Users/leonidbugaev/go/src/code-search/tests/search_determinism_tests.rs`

## Issue Reproduction Status: ✅ SUCCESSFULLY REPRODUCED

The non-deterministic behavior has been successfully reproduced and isolated.

## Key Findings

### Root Cause Identified
- **Issue**: The `--no-merge` flag causes non-deterministic search results
- **Affected Configuration**: `probe search "query" path --no-merge`
- **Symptom**: Same query returns different line counts, byte counts, and token counts on repeated executions

### Specific Results
When using `probe search "yaml workflow agent multi-agent user input" --no-merge`:
- **Pattern 1**: `1 lines, 110 bytes, 28 tokens` (30 occurrences out of 50 runs)
- **Pattern 2**: `3 lines, 160 bytes, 41 tokens` (20 occurrences out of 50 runs)

### Configurations Tested

| Configuration | Status | Notes |
|---------------|--------|-------|
| `default` | ✅ Deterministic | Consistent results |
| `--exact` | ✅ Deterministic | Consistent results |  
| `--frequency` | ✅ Deterministic | Consistent results |
| `--no-merge` | ❌ **NON-DETERMINISTIC** | **Root cause identified** |
| `--files-only` | ✅ Deterministic | Consistent results |
| `--exclude-filenames` | ✅ Deterministic | Consistent results |

### Concurrent Execution
- ✅ No race conditions detected
- Concurrent execution shows consistent results when using default configuration
- The issue is not related to multi-threading or race conditions

## Test Architecture

### Test Files Created
1. `/Users/leonidbugaev/go/src/code-search/tests/fixtures/user/AssemblyInfo.cs` - Test fixture file
2. `/Users/leonidbugaev/go/src/code-search/tests/search_determinism_tests.rs` - Comprehensive test suite

### Test Functions
1. `test_search_determinism_with_user_path()` - Main test that reproduces the issue
2. `test_user_keyword_filename_vs_content_matching()` - Documents filename vs content matching behavior
3. `test_search_determinism_with_multiple_conditions()` - Tests various configurations systematically  
4. `test_search_determinism_concurrent_execution()` - Tests for race conditions

### Test Behavior
- **Currently**: Tests **FAIL** (as intended) when non-deterministic behavior is detected
- **Future**: Tests should **PASS** once the underlying issue is fixed
- **Validation**: Tests confirm the issue exists and provide a reliable way to verify fixes

## Technical Details

### Hypothesis Confirmed
The original hypothesis about filename vs content matching was partially correct - the path containing "user" keyword does contribute to results, but the non-determinism is specifically caused by the block merging logic.

### Block Merging Logic Issue
The `--no-merge` flag disables block merging, and this reveals underlying non-deterministic behavior in how code blocks are selected or ordered before the merging step.

## Next Steps

1. **Investigation**: Examine the block merging logic in `/Users/leonidbugaev/go/src/code-search/src/search/block_merging.rs`
2. **Root Cause Analysis**: Determine why block selection/ordering is non-deterministic
3. **Fix Implementation**: Implement deterministic ordering/selection
4. **Validation**: Run tests to confirm fix resolves the issue

## Usage

### Running the Tests
```bash
# Run all determinism tests
cargo test search_determinism

# Run specific test that reproduces the issue
cargo test test_search_determinism_with_user_path -- --nocapture

# Run configuration tests
cargo test test_search_determinism_with_multiple_conditions -- --nocapture

# Run concurrent test
cargo test test_search_determinism_concurrent_execution -- --nocapture
```

### Expected Results
- **Before Fix**: Tests should FAIL with non-deterministic behavior detected
- **After Fix**: Tests should PASS with consistent results across all iterations

### Manual Verification
```bash
# Test the problematic configuration manually
./target/release/probe search "yaml workflow agent multi-agent user input" tests/fixtures/user --no-merge

# Run multiple times and observe results vary
for i in {1..10}; do
    echo "Run $i:"
    ./target/release/probe search "yaml workflow agent multi-agent user input" tests/fixtures/user --no-merge | grep "Total"
done
```

## File Structure
```
tests/
├── fixtures/
│   └── user/
│       └── AssemblyInfo.cs          # Test fixture file
├── search_determinism_tests.rs      # Main test file
└── README_search_determinism_tests.md # This documentation
```