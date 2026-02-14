# Probe Documentation Guide

This guide helps you navigate the Probe documentation and find the information you need.

## Documentation Structure

The Probe documentation is organized into three main sections:

### Core Concepts & Setup

This section covers what Probe is, how to install it, and key concepts.

- [What is Probe?](/features) - Overview of Probe's core features, local privacy, and AI readiness
- [Installation](/installation) - Detailed installation instructions for all platforms
- [Quick Start](/quick-start) - Get up and running with Probe in minutes
- [Language Support](/language-support-overview) - How Probe understands different languages
- [Supported Languages](/supported-languages) - List of supported programming languages
- [How It Works](/how-it-works) - The technology behind Probe

### Use Cases / Workflows

This section shows how different user roles can integrate Probe into their workflow.

- [Using with AI Code Editors](/mcp-integration) - MCP integration and editor plugin setups
- [Using in the CLI for Advanced AI](/ai-chat) - CLI usage, scripting, and session-based caching
- [Hosting a Team Chat](/web-interface) - Web interface usage and network configuration
- [Building AI Tools with Node.js SDK](/nodejs-sdk) - SDK usage, LangChain, and custom tool building

### Reference Documentation

This section provides comprehensive reference for all commands, flags, APIs, etc.

- [CLI Reference](/cli-mode) - Command-line interface reference with all flags
- [AI Integration Reference](/ai-integration) - Environment variables, supported models, and usage patterns
- [MCP Server Reference](/mcp-server) - JSON schemas and tool call specifications
- [Output Formats Reference](/output-formats) - JSON, XML, and other output formats
- [Search Functionality](/search-functionality) - Detailed search capabilities
- [Code Extraction](/code-extraction) - Extract meaningful code blocks
- [Adding Languages](/adding-languages) - Contribute support for new languages
- [Documentation Maintenance](/documentation-maintenance) - For documentation maintainers

### Contributing

- [Contributing Guide](https://github.com/probelabs/probe/blob/main/CONTRIBUTING.md) - How to contribute to Probe
- [Code of Conduct](https://github.com/probelabs/probe/blob/main/CODE_OF_CONDUCT.md) - Community guidelines

## How to Use This Documentation

### For New Users

If you're new to Probe, we recommend following this path:

1. Start with [What is Probe?](/features) to understand the tool's capabilities
2. Follow the [Installation](/installation) guide to get Probe set up
3. Use the [Quick Start](/quick-start) guide to get up and running quickly
4. Choose a workflow that matches your needs from the Use Cases section
5. Refer to the Reference Documentation for detailed information as needed

### For Different User Roles

- **AI Code Editor Users**: Start with [Using with AI Code Editors](/mcp-integration)
- **CLI Power Users**: Check out [Using in the CLI for Advanced AI](/ai-chat)
- **Enterprise/Team Users**: See [Hosting a Team Chat](/web-interface)
- **AI Tooling Developers**: Begin with [Building AI Tools with Node.js SDK](/nodejs-sdk)

### Finding Information

There are several ways to find information in the documentation:

1. **Navigation Menu**: Use the sidebar navigation to browse the documentation by section
2. **Search**: Use the search box in the top right to find specific topics
3. **Cross-References**: Follow links within documentation pages to related topics
4. **Home Page**: The [home page](/) provides an overview of key features

## Documentation Conventions

Throughout the documentation, we use the following conventions:

### Code Examples

Code examples are shown in code blocks:

```bash
probe search "error handling" ./src
```

### Command Examples

Command examples are shown using the `<CommandExample>` component:

<CommandExample>probe search "error handling" ./src</CommandExample>

### Code Editors

Code snippets are shown using the `<CodeEditor>` component:

<CodeEditor filePath="example.js">
function handleError(error) {
  console.error(`Error: ${error.message}`);
  return { success: false, error: error.message };
}
</CodeEditor>

### Notes and Warnings

Important information is highlighted using note blocks:

:::info
This is an informational note that provides additional context.
:::

:::warning
This is a warning that highlights potential issues or important considerations.
:::

:::tip
This is a tip that provides helpful advice or best practices.
:::

## Contributing to the Documentation

We welcome contributions to the Probe documentation. If you find errors, omissions, or have suggestions for improvements, please consider contributing:

1. Fork the [Probe repository](https://github.com/probelabs/probe) on GitHub
2. Make your changes to the documentation files in the `site` directory
3. Submit a pull request with your changes

For more information on contributing, see the [Contributing Guide](https://github.com/probelabs/probe/blob/main/CONTRIBUTING.md).

## Feedback

If you have feedback on the documentation, please [open an issue](https://github.com/probelabs/probe/issues) on GitHub or reach out to the maintainers.