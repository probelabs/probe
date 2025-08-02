# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**IMPORTANT**: ALWAYS use code-search-agent to navigate the codebase and ask questions about it.

## Project Overview

Probe is an AI-friendly, fully local, semantic code search tool built in Rust that combines ripgrep's speed with tree-sitter's AST parsing. It's designed to power AI coding assistants with precise, context-aware code search and extraction capabilities.

## Development Guidelines

### 1. Testing Requirements

**EVERY feature, bug fix, or change MUST include tests:**
- Unit tests for new functions/modules
- Integration tests for cross-module functionality  
- CLI tests for command-line interface changes
- Use `assert!`, `assert_eq!`, and `assert_ne!` for test assertions
- Property-based tests with `proptest` for complex logic

**Test patterns in this codebase:**
```rust
#[test]
fn test_feature_name() {
    // Arrange
    let input = create_test_data();
    
    // Act
    let result = function_under_test(input);
    
    // Assert
    assert_eq!(result, expected_value);
}
```

**Before committing:**
```bash
make test           # Run all tests
make test-unit      # Run unit tests only
make test-cli       # Run CLI tests
```

### 2. Error Handling

**Always use proper error handling:**
- Return `Result<T, E>` for fallible operations
- Use descriptive error types (avoid generic errors)
- Chain errors with context using `.context()` or `.with_context()`
- Never use `.unwrap()` in production code (except in tests)

```rust
// Good
pub fn parse_query(input: &str) -> Result<Query, QueryError> {
    // implementation
}

// Bad
pub fn parse_query(input: &str) -> Query {
    // uses unwrap() internally
}
```

### 3. Code Quality Standards

**Before EVERY commit:**
```bash
make format    # Format code with rustfmt
make lint      # Run clippy linter
make test      # Run all tests
```

**Code organization:**
- Keep modules focused and single-purpose
- Use descriptive names (no abbreviations)
- Add doc comments for public APIs
- Keep functions under 50 lines when possible

### 4. Performance Considerations

- Profile before optimizing (use `DEBUG=1` for timing info)
- Document performance-critical code
- Prefer lazy evaluation where appropriate
- Use parallel processing with `rayon` for CPU-bound tasks
- For detailed optimization methodology, see `docs/PERFORMANCE_OPTIMIZATION.md`

### 5. Adding New Features

**Process for new features:**
1. Search existing code for similar patterns
2. Write tests FIRST (TDD approach)
3. Implement the feature
4. Ensure all tests pass
5. Update documentation if needed
6. Run `make fix-all` before committing

**For new language support:**
1. Add tree-sitter parser to `Cargo.toml`
2. Create module in `src/language/`
3. Implement the `Language` trait
4. Add comprehensive tests
5. Register in language factory
6. Update supported languages documentation

## Common Commands

### Quick Reference
```bash
# Building
make build              # Debug build
cargo build --release   # Release build

# Testing
make test              # All tests
make test-unit         # Unit tests only
make test-integration  # Integration tests
make test-cli          # CLI tests

# Code Quality
make lint              # Run clippy
make format            # Format code
make fix-all           # Fix all issues automatically

# Running
cargo run -- search "query" ./path
cargo run -- extract file.rs:10
probe search "function name" ./src --max-results 10

# Performance Analysis
DEBUG=1 probe search "query" ./path --max-results 10
```

### GitHub Integration

**ALWAYS use `gh` CLI for GitHub operations:**
```bash
gh issue view <number> --comments    # View issue with discussion
gh pr view <number> --comments       # View PR with discussion
gh pr create                         # Create pull request
gh issue list                        # List issues
gh pr list                          # List PRs
```

## Architecture Quick Reference

### Core Components
- `src/language/` - Language parsers (tree-sitter based)
- `src/search/` - Search engine with Elastic query support
- `src/extract/` - Code extraction logic
- `src/cli.rs` - Command-line interface
- `src/ranking.rs` - Result ranking algorithms
- `src/simd_ranking.rs` - SIMD-optimized ranking

### Key Concepts
- **Elastic queries**: Boolean operators (AND, OR, NOT)
- **AST parsing**: Tree-sitter for accurate code understanding
- **Token limits**: AI-friendly output size control
- **SIMD optimization**: High-performance vector operations
- **Caching**: Multiple levels for performance

### Testing Infrastructure
- Unit tests: Next to implementation files
- Integration tests: `tests/` directory
- CLI tests: `tests/cli_tests.rs`
- Property tests: Using `proptest` crate
- Benchmarks: `benches/` directory

## Important Project-Specific Notes

### NPM Package Management
When adding files to `examples/chat/`, ALWAYS update the `files` array in `examples/chat/package.json`. The npm package (@buger/probe-chat) only includes explicitly listed files.

### Performance Debugging
```bash
# Get timing breakdown
DEBUG=1 probe search "query" ./path 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'

# Disable SIMD for comparison
DISABLE_SIMD_RANKING=1 probe search "query" ./path
```

### Common Patterns
- Use `#[cfg(test)]` for test modules
- Implement `Display` and `Debug` for public types
- Use `Arc<>` for shared immutable data
- Prefer iterators over collecting into vectors
- Cache expensive computations

## Commit Guidelines

**Commit messages should:**
- Start with imperative verb (Add, Fix, Update, etc.)
- Be specific about what changed
- Include performance metrics if relevant
- Reference issue numbers when applicable

**Example:**
```
Fix AST parsing for nested Python decorators

- Handle multiple decorator levels correctly
- Add comprehensive test coverage
- Performance impact: negligible (<1ms)

Fixes #123
```

## Getting Help

- Search codebase first using probe itself
- Check existing tests for usage examples
- Review similar implementations in codebase
- Consult documentation in `site/` directory

Remember: **Quality over speed**. Take time to write tests, handle errors properly, and maintain code quality standards.