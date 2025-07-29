# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

ALWAYS use code-search-agent to navigate the codebase and ask questions about it.

## GitHub Integration

ALWAYS use the `gh` tool for GitHub interactions instead of web URLs or other methods:
- View issues: `gh issue view <number> --comments` (always include --comments to see all discussion)
- View pull requests: `gh pr view <number> --comments` (always include --comments to see all discussion)
- List issues/PRs: `gh issue list` or `gh pr list`
- Check releases: `gh release list`
- Any other GitHub operations should use the `gh` CLI tool

## Project Overview

Probe is an AI-friendly, fully local, semantic code search tool built in Rust that combines ripgrep's speed with tree-sitter's AST parsing. It's designed to power AI coding assistants with precise, context-aware code search and extraction capabilities.

## Common Commands

### Building and Development
```bash
# Build the project (debug mode)
cargo build

# Build for release
cargo build --release

# Install locally
cargo install --path .

# Quick build using Makefile
make build
```

### Testing
```bash
# Run all tests
make test

# Run specific test types
make test-unit          # Unit tests only
make test-integration   # Integration tests only
make test-cli          # CLI tests only

# Run tests with backtrace
RUST_BACKTRACE=1 cargo test
```

### Code Quality
```bash
# Lint with clippy
make lint
# or
cargo clippy --all-targets --all-features -- -D warnings

# Format code
make format
# or
cargo fmt --all

# Fix all issues automatically
make fix-all
```

### Running the Application
```bash
# Run in debug mode
cargo run -- search "query" ./

# Run release build
cargo run --release -- search "query" ./

# Examples of common usage
probe search "function name" ./src
probe extract src/main.rs:42
probe search "error handling" --max-tokens 5000

# SIMD optimizations are enabled by default for better performance
# Disable SIMD optimizations if needed
DISABLE_SIMD_RANKING=1 probe search "function process" ./src

# Enable debug mode to see SIMD vs traditional ranking
DEBUG=1 probe search "function process" ./src
```

### Performance Monitoring
```bash
# Get detailed timing information (DEBUG mode required)
DEBUG=1 probe search "query" ./path --max-results 10

# Extract only timing data (single command, filters out verbose debug output)
DEBUG=1 probe search "query" ./path --max-results 10 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'

# Alternative using grep (may include extra lines)
DEBUG=1 probe search "query" ./path --max-results 10 | grep -A 50 "=== SEARCH TIMING INFORMATION ==="

# Example timing extraction for performance analysis
DEBUG=1 probe search "agent workflow" ~/go/src/semantic-kernel/ --max-results 10 --timeout 120 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'
```

### Testing Chat Example with Image Support
```bash
# First, set your API key (choose one):
export ANTHROPIC_API_KEY="your_key_here"
export OPENAI_API_KEY="your_key_here" 
export GOOGLE_API_KEY="your_key_here"

# Navigate to chat example
cd examples/chat

# Test chat with image URL (single message, non-interactive)
DEBUG_CHAT=1 GOOGLE_API_KEY=$GOOGLE_API_KEY MODEL_NAME=gemini-2.5-pro-preview-06-05 node index.js --force-provider google -m "Do you see whats on the image here https://github.com/user-attachments/assets/6c1292af-3e0b-4f45-8ef9-609102dea5fb"

# Run interactive chat mode
node index.js --force-provider google

# Run web interface (accessible at http://localhost:8080)
node index.js --web --port 8080
```

## Architecture Overview

### Core Components

**Language Support (`src/language/`)**
- Modular language parsers using tree-sitter grammars
- Supports Rust, JavaScript/TypeScript, Python, Go, C/C++, Java, Ruby, PHP, Swift, C#
- Each language implements the `Language` trait for AST parsing
- Factory pattern for language detection and instantiation

**Search Engine (`src/search/`)**
- Elastic query system with boolean operators (AND, OR, NOT)
- Multiple ranking algorithms: TF-IDF, BM25, and hybrid approaches
- Token-based search with stemming and stopword removal
- Result caching and optimization for large codebases

**SIMD-Optimized Ranking (`src/simd_ranking.rs`)**
- SimSIMD library integration for accelerated vector operations
- Sparse vector representation for memory efficiency
- Up to 5-20x performance improvement for vector dot products
- Supports both hybrid (with boolean logic) and pure SIMD ranking modes

**Code Extraction (`src/extract/`)**
- AST-based code block extraction
- Symbol-aware extraction (functions, classes, structs)
- Context-preserving extraction with configurable limits

**CLI Interface (`src/cli.rs`)**
- Two main commands: `search` and `extract`
- Flexible output formats: markdown, JSON, plain text
- Token limiting for AI integration

### Key Files
- `src/main.rs` - Entry point and CLI argument parsing
- `src/lib.rs` - Library interface and main API
- `src/models.rs` - Core data structures and types
- `src/query.rs` - Query parsing and validation
- `src/ranking.rs` - Search result ranking algorithms (traditional + SIMD)
- `src/simd_ranking.rs` - SIMD-optimized sparse vector operations

## Integration Points

### Examples Directory
- `examples/chat/` - AI chat interface using Anthropic/OpenAI APIs
- `mcp/` - Model Context Protocol server implementation
- `npm/` - Node.js SDK and CLI wrapper

### Multi-Platform Support
The project includes comprehensive cross-platform build configuration:
- GitHub Actions for automated releases
- Makefile targets for Linux, macOS (x86_64/ARM64), and Windows
- Platform-specific installation scripts

## Development Notes

### Adding New Languages
1. Add tree-sitter parser dependency to `Cargo.toml`
2. Create language module in `src/language/`
3. Implement the `Language` trait
4. Register in the language factory

### Performance Considerations
- Uses ripgrep for initial file scanning (extremely fast)
- AST parsing is parallelized using rayon
- Implements various caching mechanisms for repeated searches
- Token counting uses tiktoken-rs for AI integration
- SIMD-optimized ranking with SimSIMD for vector operations
- Sparse vector representation reduces memory usage by 20-40%
- Automatic CPU capability detection for optimal SIMD utilization

### Testing Strategy
- Comprehensive test suite covering parsing, search, and extraction
- Property-based testing with proptest
- CLI integration tests
- Performance benchmarks in `benches/` including SIMD vs traditional comparisons

### SIMD Optimization Usage

**SIMD optimizations (enabled by default):**
```bash
# SIMD optimizations are enabled by default for better performance
probe search "function process data" ./src

# Disable specific SIMD optimizations if needed
DISABLE_SIMD_RANKING=1 probe search "function process data" ./src
DISABLE_SIMD_TOKENIZATION=1 probe search "function process data" ./src
DISABLE_SIMD_PATTERN_MATCHING=1 probe search "function process data" ./src

# Disable all SIMD optimizations
DISABLE_SIMD_RANKING=1 DISABLE_SIMD_TOKENIZATION=1 DISABLE_SIMD_PATTERN_MATCHING=1 probe search "function process data" ./src
```

**Benchmark SIMD performance:**
```bash
# Run SIMD-specific benchmarks
cargo bench --bench simd_benchmarks

# Compare all ranking implementations
cargo bench ranking_synthetic
cargo bench ranking_realistic
```

**SIMD Implementation Details:**
- Two modes: `rank_documents_simd()` (hybrid with boolean logic) and `rank_documents_simd_simple()` (pure SIMD)
- Sparse vector format with sorted indices for optimal SimSIMD performance
- Automatic fallback to manual computation if SIMD operations fail
- Cross-platform support (x86, ARM64, all major operating systems)

## Performance Optimization Methodology

This section documents the systematic approach used to identify, analyze, and fix performance bottlenecks in the probe codebase.

### Phase 1: Performance Profiling and Analysis

**Step 1: Establish Baseline Performance**
```bash
# Build release binary first
cargo build --release

# Run performance profiling with debug timing
DEBUG=1 ./target/release/probe search "workflow" ~/go/src/semantic-kernel/ --max-results 10 --timeout 300 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'
```

**Step 2: Identify Performance Bottlenecks**
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

**Step 3: Architecture Research**
For each bottleneck >1s, spawn separate research agents to analyze:
- Current implementation approach and complexity
- Root cause analysis of performance issues  
- Potential optimization strategies with confidence levels
- Expected performance savings for each strategy

### Phase 2: Implementation Strategy

**Priority Classification:**
- **High Priority**: >8s potential savings or >15% of total time
- **Medium Priority**: 2-8s potential savings or 5-15% of total time  
- **Low Priority**: <2s potential savings or <5% of total time

**Implementation Order:**
1. **Quick wins first**: High confidence (8-10/10), low complexity optimizations
2. **High impact second**: Medium confidence, high potential savings
3. **Polish last**: Lower impact optimizations for final performance tuning

### Phase 3: Individual Optimization Implementation

**For each optimization, follow this exact process:**

**Step 1: Pre-Implementation Baseline**
```bash
# Performance test
DEBUG=1 ./target/release/probe search "workflow" ~/go/src/semantic-kernel/ --max-results 10 --timeout 300 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'

# Correctness test  
./target/release/probe search "stemming" ~/go/src/semantic-kernel/ --max-results 5
```

**Step 2: Implementation**
- Use architecture agents for complex optimizations
- Maintain full backward compatibility
- Add comprehensive comments explaining the optimization
- Focus on correctness first, performance second

**Step 3: Post-Implementation Verification**
```bash
# Performance verification (same commands as Step 1)
DEBUG=1 ./target/release/probe search "workflow" ~/go/src/semantic-kernel/ --max-results 10 --timeout 300 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'

# Correctness verification - output should be identical
./target/release/probe search "stemming" ~/go/src/semantic-kernel/ --max-results 5

# Comprehensive testing
make test
```

**Step 4: Quality Assurance**
- All tests must pass (unit, integration, CLI tests)
- Output correctness: same number of results, same content (slight ranking differences OK)
- Performance improvement: measurable reduction in target timing component
- Code quality: passes `make lint` and `make format`

**Step 5: Documentation and Commit**
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

### Phase 4: Verification Commands

**Standard Performance Test:**
```bash
DEBUG=1 ./target/release/probe search "workflow" ~/go/src/semantic-kernel/ --max-results 10 --timeout 300 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'
```

**Standard Correctness Test:**
```bash
./target/release/probe search "stemming" ~/go/src/semantic-kernel/ --max-results 5
```

**Comprehensive Test Suite:**
```bash
make test
```

**Output Validation Checklist:**
- ✅ Same number of search results before/after
- ✅ Same total bytes and tokens returned  
- ✅ All tests pass without regressions
- ✅ Performance improvement in target component
- ✅ No new compilation warnings or errors

### Phase 5: Success Metrics

**Optimization Success Criteria:**
1. **Performance**: Measurable improvement in target timing component
2. **Correctness**: Identical functional output (same results, bytes, tokens)
3. **Quality**: All tests pass, no regressions introduced
4. **Maintainability**: Clean code with comprehensive comments
5. **Reliability**: Backward compatibility preserved

**Performance Tracking:**
Track cumulative improvements across optimization phases:
- **Baseline**: 53.38s total search time
- **After Phase 1**: [timing] ([improvement]% faster)
- **After Phase 2**: [timing] ([improvement]% faster)  
- **Final**: [timing] ([improvement]% faster overall)

### Optimization Examples Applied

**High-Impact Optimizations Completed:**
1. **Lazy line map construction**: 980ms improvement (26% faster overall)
2. **AST node filtering**: 700ms improvement (39% faster line map building)
3. **Simplified query evaluation**: 45% improvement in filtering time
4. **Token count caching**: 300ms improvement for repeated tokenization
5. **Uncovered lines batch processing**: 57ms improvement (26% faster uncovered lines)

**Key Lessons Learned:**
- **Algorithm complexity** often matters more than micro-optimizations
- **Lazy evaluation** provides significant gains when work can be avoided
- **Caching strategies** effective for repeated operations on similar data
- **Early termination** powerful for processing large datasets
- **Backward compatibility** essential - never sacrifice correctness for performance

This methodology successfully achieved **~95% performance improvement** (53.38s → 2.86s) while maintaining full correctness and backward compatibility.

### NPM Package Management
**IMPORTANT**: When adding new JavaScript files to `examples/chat/`, always update the `files` array in `examples/chat/package.json`. The npm package (@buger/probe-chat) is published from this directory and only includes files listed in the `files` array. Missing files will cause "Cannot find module" errors when the package is installed via npm.
