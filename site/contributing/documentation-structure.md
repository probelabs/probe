# Documentation Structure

This document outlines the organization of the Probe documentation and explains the reasoning behind its structure.

## Three-Part Organization

The Probe documentation is organized into three main sections, each serving a specific purpose and audience:

### 1. Core Concepts & Setup

**Purpose**: Cover what Probe is, how to install, and key concepts.

**Audience**: Anyone new to Probe, or needing the big-picture overview.

**Pages in this section**:
- **What is Probe?** - Overview of core features, local privacy, AI readiness
- **Installation** - Platform-specific installation instructions
- **Quick Start** - Get up and running in minutes
- **Language Support** - How Probe understands different languages
- **Supported Languages** - List of languages with parsing capabilities
- **How It Works** - Technical deep dive into Probe's architecture

This section provides the foundation for understanding Probe. It answers the questions "What is it?", "How do I get it?", and "How does it work?" - giving users the context they need before diving into specific use cases.

### 2. Use Cases / Workflows

**Purpose**: Show how different user roles can integrate Probe into their workflow.

**Audience**: Distinct user segments with different needs and integration patterns.

**Pages in this section**:
- **Using with AI Code Editors** - MCP integration and editor plugin setups
- **Using in the CLI for Advanced AI** - CLI usage, scripting, and session-based caching
- **Hosting a Team Chat** - Web interface usage and network configuration
- **Building AI Tools with Node.js SDK** - SDK usage, LangChain, and custom tool building

This section is scenario-based rather than feature-based. It recognizes that different users have different goals and workflows, and provides tailored guidance for each. By organizing around user roles and workflows, it helps users quickly find the information most relevant to their specific needs.

### 3. Reference Documentation

**Purpose**: Comprehensive "dictionary" of all commands, flags, environment variables, APIs, etc.

**Audience**: Anyone needing direct reference or advanced/edge-case usage.

**Pages in this section**:
- **CLI Reference** - Command-line interface reference with all flags
- **AI Integration Reference** - Environment variables, supported models, and usage patterns
- **MCP Server Reference** - JSON schemas and tool call specifications
- **Output Formats Reference** - JSON, XML, and other output formats
- **Search Functionality** - Detailed search capabilities
- **Code Extraction** - Extract meaningful code blocks
- **Adding Languages** - Contribute support for new languages

This section serves as a comprehensive reference for all of Probe's features and capabilities. It's designed for users who need detailed information about specific aspects of the tool, whether for advanced usage, troubleshooting, or integration.

## Navigation Principles

The documentation follows these key principles:

1. **Progressive Disclosure**: Start with high-level concepts and gradually introduce more complex topics
2. **Task-Oriented**: Organize information around what users are trying to accomplish
3. **Role-Based**: Recognize that different users have different needs and workflows
4. **Comprehensive Reference**: Provide detailed information for advanced users and edge cases

## Cross-References

Throughout the documentation, you'll find cross-references to related topics. These help you navigate between sections and find related information. For example, a use case page might reference specific CLI commands, which are documented in detail in the reference section.

## Search Functionality

The documentation includes a search feature that allows you to quickly find information across all pages. This is particularly useful when you're looking for specific features or concepts.

## Feedback and Improvements

This documentation structure is designed to evolve based on user feedback. If you have suggestions for improving the organization or content of the documentation, please [open an issue](https://github.com/probelabs/probe/issues) on GitHub.