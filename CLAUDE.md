# CLAUDE.md - Code Style and Development Guidelines

## Build, Lint, Test Commands
- Build: `cargo build` or `make build`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings` or `make lint`
- Format: `cargo fmt --all` or `make format`
- Run all tests: `cargo test` or `make test-all`
- Run unit tests: `RUST_BACKTRACE=1 cargo test --lib` or `make test-unit`
- Run specific test: `cargo test test_function_name`
- Run tests with output: `cargo test -- --show-output`

## Code Style Guidelines
- Use snake_case for functions, variables, files
- Use PascalCase for types, structs, enums
- Use SCREAMING_SNAKE_CASE for constants
- Group imports by std, external crates, then internal modules
- Add context to errors with `context()` method
- Prefer `?` operator for error propagation
- Use `anyhow::Result` for error handling in most functions
- Document public interfaces with doc comments
- Follow builder pattern for command-line interface
- Prefer functional programming style with iterators
- Use strong typing throughout with explicit conversions
- Add unit tests for all new functionality