# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with the probe codebase.

**IMPORTANT**: ALWAYS use code-search-agent to navigate the codebase and ask questions about it.

## Project Overview

Probe is an AI-friendly, fully local, semantic code search tool built in Rust that combines ripgrep's speed with tree-sitter's AST parsing. It's designed to power AI coding assistants with precise, context-aware code search and extraction capabilities.

## Development Guidelines

### 1. Testing Requirements

**EVERY feature, bug fix, or change MUST include tests:**
- Unit tests for new functions/modules (in same file using `#[cfg(test)]`)
- Integration tests for cross-module functionality (in `tests/` directory)
- CLI tests for command-line interface changes (`tests/cli_tests.rs`)
- Property-based tests with `proptest` for complex logic
- Test coverage for edge cases and error conditions

**Test patterns in this codebase:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_name() {
        // Arrange
        let input = create_test_data();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result, expected_value);
        assert!(condition);
        assert_ne!(unexpected, actual);
    }
}
```

**Before committing:**
```bash
make test           # Run all tests (unit + integration + CLI)
make test-unit      # Run unit tests only
make test-cli       # Run CLI tests
make test-property  # Run property-based tests
```

### 2. Error Handling

**Always use proper error handling with anyhow:**
```rust
use anyhow::{Context, Result};

// Good - use Result<T> with context
pub fn parse_file(path: &Path) -> Result<ParsedData> {
    let content = fs::read_to_string(path)
        .context(format!("Failed to read file: {:?}", path))?;
    
    parse_content(&content)
        .context("Failed to parse file content")
}

// Bad - using unwrap() in production code
pub fn parse_file(path: &Path) -> ParsedData {
    let content = fs::read_to_string(path).unwrap(); // NO!
    parse_content(&content).unwrap() // NO!
}
```

**Key patterns:**
- Return `Result<T>` for all fallible operations
- Use `.context()` to add error context
- Use `anyhow::Error` for flexible error handling
- Create custom error types when domain-specific errors needed
- Never use `.unwrap()` except in tests

### 3. Code Quality Standards

**Before EVERY commit run these commands:**
```bash
make format         # Format code with rustfmt
make lint           # Run clippy linter
make test           # Run all tests
# OR simply:
make fix-all        # Runs format + lint fixes + tests
```

**Code organization principles:**
- Keep modules focused and single-purpose
- Use descriptive names (avoid abbreviations like `cfg`, `ctx`)
- Add doc comments (`///`) for all public APIs
- Keep functions under 50 lines when possible
- Group related functionality in modules

### 4. Performance Considerations

- Profile before optimizing using `DEBUG=1` environment variable
- Document performance-critical code with comments
- Prefer lazy evaluation and iterators over collecting
- Use `rayon` for CPU-bound parallel tasks
- Leverage caching for expensive operations
- See `docs/PERFORMANCE_OPTIMIZATION.md` for detailed methodology

### 5. Adding New Features

**Process for new features:**
1. Search existing code for similar patterns using probe itself
2. Write tests FIRST (TDD approach)
3. Implement the feature
4. Ensure all tests pass
5. Update relevant documentation
6. Run `make fix-all` before committing

**For new language support:**
1. Add tree-sitter parser to `Cargo.toml`
2. Create module in `src/language/` implementing `Language` trait
3. Add comprehensive tests including test detection
4. Register in `src/language/factory.rs`
5. Update docs in `site/supported-languages.md`

## Common Commands

```bash
# Building
make build              # Debug build
cargo build --release   # Release build with optimizations

# Testing & Quality
make test              # All tests
make test-unit         # Unit tests only
make lint              # Run clippy
make format            # Format code
make fix-all           # Fix everything automatically

# Running probe
cargo run -- search "query" ./path
cargo run -- extract file.rs:42
probe search "function" ./src --max-results 10

# Performance debugging
DEBUG=1 probe search "query" ./path  # Shows timing information
```

## Architecture Quick Reference

### Core Components
- `src/language/` - Language parsers using tree-sitter
- `src/search/` - Search engine with Elastic query support
- `src/extract/` - Code extraction logic
- `src/ranking.rs` - BM25 and TF-IDF ranking
- `src/simd_ranking.rs` - SIMD-optimized ranking

### Key Design Patterns
- **Result<T> everywhere**: All fallible operations return Result
- **Lazy evaluation**: Parse only what's needed
- **Parallel processing**: Use rayon for file processing
- **Caching**: Multiple cache levels (parser pool, tree cache, search cache)
- **Zero-copy where possible**: Use references and slices

## GitHub Integration

**ALWAYS use `gh` CLI for GitHub operations:**
```bash
gh issue view <number> --comments    # View issue with full discussion
gh pr view <number> --comments       # View PR with reviews
gh pr create                         # Create pull request
```

## Project-Specific Patterns

### Testing Patterns
- Unit tests use `#[cfg(test)]` modules in same file
- Integration tests go in `tests/` directory  
- Use `assert!`, `assert_eq!`, `assert_ne!` macros
- Property tests with `proptest` for fuzzing
- Test both success and error cases

### Module Organization
```rust
// Standard module layout
pub mod module_name;              // Public module
mod internal_module;              // Private module
pub use module_name::PublicItem;  // Re-exports

#[cfg(test)]
mod tests {                       // Test module
    use super::*;
}
```

### Common Idioms
- Use `Arc<>` for shared immutable data
- Prefer `&str` over `String` for function parameters
- Use `PathBuf` for owned paths, `&Path` for borrowed
- Implement `Display` and `Debug` for public types
- Use `#[derive()]` for common traits

## Important Notes

### NPM Package Management
When adding files to `examples/chat/`, ALWAYS update the `files` array in `examples/chat/package.json`.

### Pre-commit Hook
The project has git hooks in `.githooks/`. Install with:
```bash
make install-hooks
```

### Debugging Tips
- Use `DEBUG=1` for verbose output
- Check `error.log` for detailed errors
- Use `RUST_BACKTRACE=1` for stack traces
- Profile with `cargo flamegraph` for performance

## Getting Help

1. Search codebase first: `probe search "topic" ./src`
2. Check existing tests for usage examples
3. Review similar implementations
4. Consult docs in `site/` directory

Remember: **Quality > Speed**. Write tests, handle errors properly, and maintain code standards.