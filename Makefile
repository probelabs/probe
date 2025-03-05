# Makefile for probe Rust project

# Configuration
CARGO := cargo
RUSTC := rustc
RUSTFMT := rustfmt
CLIPPY := cargo clippy
SCRIPTS_DIR := scripts
TESTS_DIR := tests

# Default target
.PHONY: all
all: build

# Build targets
.PHONY: build
build:
	$(CARGO) build

.PHONY: release
release:
	$(CARGO) build --release

# Test targets
.PHONY: test
test: test-unit test-integration test-property test-cli

.PHONY: test-unit
test-unit:
	RUST_BACKTRACE=1 $(CARGO) test --lib

.PHONY: test-integration
test-integration:
	RUST_BACKTRACE=1 $(CARGO) test --test integration_tests

.PHONY: test-property
test-property:
	RUST_BACKTRACE=1 $(CARGO) test --test property_tests

.PHONY: test-cli
test-cli:
	RUST_BACKTRACE=1 $(CARGO) test --test cli_tests

.PHONY: test-all
test-all:
	RUST_BACKTRACE=1 $(CARGO) test

# Code quality targets
.PHONY: lint
lint:
	$(CLIPPY) --all-targets --all-features -- -D warnings

.PHONY: format
format:
	$(CARGO) fmt --all

.PHONY: check-format
check-format:
	$(CARGO) fmt --all -- --check

# Documentation
.PHONY: doc
doc:
	$(CARGO) doc --no-deps

.PHONY: doc-open
doc-open:
	$(CARGO) doc --no-deps --open

# Clean targets
.PHONY: clean
clean:
	$(CARGO) clean

.PHONY: clean-all
clean-all: clean
	rm -rf Cargo.lock

# Run targets
.PHONY: run
run:
	$(CARGO) run

.PHONY: run-release
run-release:
	$(CARGO) run --release

# Help target
.PHONY: help
help:
	@echo "Available targets:"
	@echo "  all               - Build the project (default)"
	@echo "  build             - Build the project in debug mode"
	@echo "  release           - Build the project in release mode"
	@echo "  test              - Run all tests (unit, integration, property, CLI)"
	@echo "  test-unit         - Run unit tests"
	@echo "  test-integration  - Run integration tests"
	@echo "  test-property     - Run property tests"
	@echo "  test-cli          - Run CLI tests"
	@echo "  test-all          - Run all tests (including doc tests and examples)"
	@echo "  lint              - Run clippy linter"
	@echo "  format            - Format code using rustfmt"
	@echo "  check-format      - Check if code is properly formatted"
	@echo "  doc               - Generate documentation"
	@echo "  doc-open          - Generate documentation and open in browser"
	@echo "  clean             - Clean build artifacts"
	@echo "  clean-all         - Clean build artifacts and Cargo.lock"
	@echo "  run               - Run the application in debug mode"
	@echo "  run-release       - Run the application in release mode"
	@echo "  help              - Show this help message"
