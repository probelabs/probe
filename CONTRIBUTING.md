# Contributing to Probe

Thank you for considering contributing to Probe! This document provides guidelines and instructions to make the contribution process smooth and effective.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Setup](#development-setup)
- [Development Workflow](#development-workflow)
- [Pull Request Process](#pull-request-process)
- [Testing Guidelines](#testing-guidelines)
- [Coding Standards](#coding-standards)
- [Documentation](#documentation)
- [Release Process](#release-process)

## Code of Conduct

By participating in this project, you are expected to uphold our Code of Conduct. Please report unacceptable behavior to the project maintainers.

## Development Setup

### Prerequisites

- Rust and Cargo (latest stable version)
- Git
- Make (for using the Makefile commands)

### Getting Started

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/YOUR-USERNAME/probe.git
   cd probe
   ```
3. Set up the git hooks to ensure code quality:
   ```bash
   make install-hooks
   ```
4. Add the original repository as a remote to keep your fork updated:
   ```bash
   git remote add upstream https://github.com/buger/probe.git
   ```

### Building the Project

To build the project in debug mode:
```bash
make build
```

To run the application:
```bash
make run
```

For a release build:
```bash
make run-release
```

## Development Workflow

1. Create a new branch for your feature or bugfix:
   ```bash
   git checkout -b feature/your-feature-name
   ```
   or
   ```bash
   git checkout -b fix/issue-you-are-fixing
   ```

2. Make your changes, following the [Coding Standards](#coding-standards)

3. Run tests to ensure your changes don't break existing functionality:
   ```bash
   make test
   ```

4. Format your code:
   ```bash
   make format
   ```

5. Run the linter:
   ```bash
   make lint
   ```

6. Commit your changes using the [Git Commit Message Guidelines](#git-commit-message-guidelines)

7. Push your branch to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```

8. Create a Pull Request from your fork to the main repository

## Pull Request Process

1. Ensure your PR includes a clear description of the changes and the purpose
2. Link any related issues using keywords like "Fixes #123" or "Resolves #456"
3. Make sure all tests pass and there are no linting errors
4. Add tests for new functionality
5. Update documentation as needed
6. Your PR will be reviewed by maintainers who may request changes
7. Once approved, your PR will be merged

## Testing Guidelines

Probe uses several types of tests to ensure quality:

### Running Tests

- Run all tests:
  ```bash
  make test
  ```

- Run specific test types:
  ```bash
  make test-unit          # Unit tests
  make test-integration   # Integration tests
  make test-property      # Property-based tests
  make test-cli           # CLI tests
  ```

### Writing Tests

- **Unit Tests**: Place in the same file as the code being tested, using Rust's `#[cfg(test)]` module
- **Integration Tests**: Add to the `tests/` directory
- **Property Tests**: Use the proptest framework for property-based testing
- **CLI Tests**: Test the command-line interface functionality

## Coding Standards

### Rust Style Guidelines

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for consistent formatting (run `make format`)
- Use `clippy` to catch common mistakes (run `make lint`)

### Git Commit Message Guidelines

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters or less
- Reference issues and pull requests after the first line
- Consider using emoji prefixes for better readability:
  - 🎨 `:art:` when improving code structure/format
  - 🐎 `:racehorse:` when improving performance
  - 📝 `:memo:` when writing docs
  - 🐛 `:bug:` when fixing a bug
  - ✅ `:white_check_mark:` when adding tests
  - 🔒 `:lock:` when dealing with security

## Documentation

- Update the README.md if you change functionality
- Document all public API items with rustdoc comments
- Include examples in documentation where appropriate
- Generate documentation with:
  ```bash
  make doc
  ```

## Adding Support for New Languages

If you want to add support for a new programming language:

1. Add the appropriate tree-sitter grammar dependency to `Cargo.toml`
2. Implement the language parser in the `src/language` directory
3. Update the language detection logic
4. Add tests for the new language support
5. Update the documentation to include the new supported language

## Release Process

Releases are managed by the project maintainers. The process involves:

1. Updating the version in `Cargo.toml`
2. Building release packages for all supported platforms:
   ```bash
   VERSION=vX.Y.Z make release
   ```
3. Creating a new GitHub release with release notes
4. Publishing the updated package

## Questions?

If you have any questions about contributing, feel free to open an issue or contact the project maintainers.

Thank you for contributing to Probe!