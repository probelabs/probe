---
title: Changelog
description: Release notes and changelog for Probe versions
layout: doc
---

# Changelog

All notable changes to Probe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2025-07-17

### 🚀 Major Features

#### Implement Tool for Code Editing
- **New `implement` tool** for AI assistants to directly edit code files
- Integrated with Aider for advanced code modification capabilities
- Configurable via `allow_edit` flag in GitHub Actions workflows
- Enables AI assistants to make direct code changes during conversations

#### Enhanced GitHub Actions Integration
- **Allow Suggestions Feature**: New `allow_suggestions` flag for reviewdog integration
- **Failure Tagging**: Automatic tagging of failed GitHub Probe runs for better tracking
- **Improved Workflows**: Enhanced probe.yml with better error handling and configuration
- **Engineer Workflow**: New probe-engineer.yml for specialized engineering tasks
- **Integration Testing**: Comprehensive integration test workflow added

#### Crates.io Publishing
- **Automated Publishing**: Probe is now published to crates.io as a Rust library
- **Library Interface**: New `src/lib.rs` with public API for Rust integration
- **Release Automation**: Automatic crates.io publishing in release workflow

### 🔧 Improvements

#### AI Chat Enhancements
- **Configurable Iterations**: `MAX_TOOL_ITERATIONS` environment variable support
- **Enhanced Tool Support**: New file listing and search tools
- **Better Session Management**: Improved chat session handling and token tracking
- **Web Interface**: Enhanced web server with better error handling

#### MCP Protocol Updates
- **Mandatory Path Parameters**: Improved MCP tool definitions with required path parameters
- **Better Error Handling**: Enhanced error messages and validation
- **Tool Consistency**: Standardized tool interfaces across MCP implementations

#### Developer Experience
- **Windows Support**: Improved Windows compatibility for npm packages
- **Binary Management**: Enhanced binary download and path resolution
- **Documentation**: Updated documentation for new features and workflows

### 🐛 Bug Fixes

#### Search and File Handling
- **Underscore Directories**: Fixed recursive search in directories with underscores
- **Path Resolution**: Improved file path handling across different platforms
- **Binary Permissions**: Fixed Windows binary permission issues

#### GitHub Actions
- **Output Masking**: Fixed issue_number output to avoid GitHub Actions masking
- **Reviewdog Integration**: Updated to stable reviewdog action version v1.21.0
- **Workflow Stability**: Multiple fixes for workflow reliability and error handling

#### Build and CI
- **Clippy Warnings**: Fixed uninlined_format_args warnings across codebase
- **Formatting**: Consistent code formatting with cargo fmt
- **Cross-platform**: Improved build compatibility across Linux, macOS, and Windows

### 📚 Documentation

#### Website Updates
- **Blog Infrastructure**: Added blog support with VitePress integration
- **Technical Guides**: New agentic flow guide with XML protocol documentation
- **Navigation**: Improved site navigation and structure
- **Discord Integration**: Updated Discord invite links

#### API Documentation
- **Tool Definitions**: Comprehensive documentation for all available tools
- **Configuration**: Detailed configuration options for GitHub Actions
- **Examples**: Enhanced examples and use case documentation

### 🔧 Infrastructure

#### Release Process
- **Multi-platform Builds**: Automated builds for Linux, macOS, Windows
- **NPM Publishing**: Automated npm package publishing
- **Version Management**: Improved version handling and release automation
- **Testing**: Enhanced integration and unit testing coverage

#### Development Tools
- **Linting**: Improved clippy and formatting rules
- **CI/CD**: Enhanced continuous integration with better error reporting
- **Dependencies**: Updated dependencies and security improvements

---

## [0.5.0] - Previous Release

For changes in version 0.5.0 and earlier, please refer to the [GitHub Releases](https://github.com/buger/probe/releases) page.

---

## Contributing

When contributing to Probe, please:

1. Follow the [Contributing Guidelines](https://github.com/buger/probe/blob/main/CONTRIBUTING.md)
2. Update this changelog for any user-facing changes
3. Use conventional commit messages for automatic changelog generation
4. Test your changes across supported platforms

## Links

- [GitHub Repository](https://github.com/buger/probe)
- [Release Downloads](https://github.com/buger/probe/releases)
- [NPM Package](https://www.npmjs.com/package/@buger/probe)
- [Crates.io Package](https://crates.io/crates/probe)
