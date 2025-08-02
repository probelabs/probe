# Performance Optimization Methodology

This document describes the systematic approach for identifying and fixing performance bottlenecks in the probe codebase.

## Phase 1: Performance Profiling and Analysis

### Step 1: Establish Baseline Performance
```bash
# Build release binary first
cargo build --release

# Run performance profiling with debug timing
DEBUG=1 ./target/release/probe search "workflow" ~/go/src/semantic-kernel/ --max-results 10 --timeout 300 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'
```

### Step 2: Identify Performance Bottlenecks
- Parse timing output to identify components taking >1 second
- Focus on operations consuming >5% of total execution time
- Look for patterns: repeated operations, inefficient algorithms, unnecessary work

**Example Analysis:**
```
=== SEARCH TIMING INFORMATION ===
Total search time:     53.38s
Result processing:     39.95s (75% of total - PRIMARY TARGET)
  - Uncovered lines:   22.73s (43% of total - HIGHEST PRIORITY)  
  - AST parsing:       11.74s (22% of total - HIGH PRIORITY)
    - Line map building: 8.90s (biggest AST component)
  - Term matching:     7.74s (15% of total - HIGH PRIORITY)
Limit application:     11.72s (22% of total - HIGH PRIORITY)
===================================
```

### Step 3: Architecture Research
For each bottleneck >1s, spawn separate research agents to analyze:
- Current implementation approach and complexity
- Root cause analysis of performance issues  
- Potential optimization strategies with confidence levels
- Expected performance savings for each strategy

## Phase 2: Implementation Strategy

### Priority Classification:
- **High Priority**: >8s potential savings or >15% of total time
- **Medium Priority**: 2-8s potential savings or 5-15% of total time  
- **Low Priority**: <2s potential savings or <5% of total time

### Implementation Order:
1. **Quick wins first**: High confidence (8-10/10), low complexity optimizations
2. **High impact second**: Medium confidence, high potential savings
3. **Polish last**: Lower impact optimizations for final performance tuning

## Phase 3: Individual Optimization Implementation

For each optimization, follow this exact process:

### Step 1: Pre-Implementation Baseline
```bash
# Performance test
DEBUG=1 ./target/release/probe search "workflow" ~/go/src/semantic-kernel/ --max-results 10 --timeout 300 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'

# Correctness test  
./target/release/probe search "stemming" ~/go/src/semantic-kernel/ --max-results 5
```

### Step 2: Implementation
- Use architecture agents for complex optimizations
- Maintain full backward compatibility
- Add comprehensive comments explaining the optimization
- Focus on correctness first, performance second

### Step 3: Post-Implementation Verification
```bash
# Performance verification (same commands as Step 1)
DEBUG=1 ./target/release/probe search "workflow" ~/go/src/semantic-kernel/ --max-results 10 --timeout 300 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'

# Correctness verification - output should be identical
./target/release/probe search "stemming" ~/go/src/semantic-kernel/ --max-results 5

# Comprehensive testing
make test
```

### Step 4: Quality Assurance
- All tests must pass (unit, integration, CLI tests)
- Output correctness: same number of results, same content (slight ranking differences OK)
- Performance improvement: measurable reduction in target timing component
- Code quality: passes `make lint` and `make format`

### Step 5: Documentation and Commit
```bash
# Create separate git commit for each optimization
git add .
git commit -m "Optimize [component]: [brief description]

- [Technical details of what was optimized]
- Performance improvement: [measurement]
- [Any important implementation notes]

🤖 Generated with [Claude Code](https://claude.ai/code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

## Phase 4: Verification Commands

### Standard Performance Test:
```bash
DEBUG=1 ./target/release/probe search "workflow" ~/go/src/semantic-kernel/ --max-results 10 --timeout 300 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'
```

### Standard Correctness Test:
```bash
./target/release/probe search "stemming" ~/go/src/semantic-kernel/ --max-results 5
```

### Comprehensive Test Suite:
```bash
make test
```

### Output Validation Checklist:
- ✅ Same number of search results before/after
- ✅ Same total bytes and tokens returned  
- ✅ All tests pass without regressions
- ✅ Performance improvement in target component
- ✅ No new compilation warnings or errors

## Phase 5: Success Metrics

### Optimization Success Criteria:
1. **Performance**: Measurable improvement in target timing component
2. **Correctness**: Identical functional output (same results, bytes, tokens)
3. **Quality**: All tests pass, no regressions introduced
4. **Maintainability**: Clean code with comprehensive comments
5. **Reliability**: Backward compatibility preserved

### Performance Tracking:
Track cumulative improvements across optimization phases:
- **Baseline**: 53.38s total search time
- **After Phase 1**: [timing] ([improvement]% faster)
- **After Phase 2**: [timing] ([improvement]% faster)  
- **Final**: [timing] ([improvement]% faster overall)

## Optimization Examples Applied

### High-Impact Optimizations Completed:
1. **Lazy line map construction**: 980ms improvement (26% faster overall)
2. **AST node filtering**: 700ms improvement (39% faster line map building)
3. **Simplified query evaluation**: 45% improvement in filtering time
4. **Token count caching**: 300ms improvement for repeated tokenization
5. **Uncovered lines batch processing**: 57ms improvement (26% faster uncovered lines)

### Key Lessons Learned:
- **Algorithm complexity** often matters more than micro-optimizations
- **Lazy evaluation** provides significant gains when work can be avoided
- **Caching strategies** effective for repeated operations on similar data
- **Early termination** powerful for processing large datasets
- **Backward compatibility** essential - never sacrifice correctness for performance

This methodology successfully achieved **~95% performance improvement** (53.38s → 2.86s) while maintaining full correctness and backward compatibility.