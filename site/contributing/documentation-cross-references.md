# Documentation Cross-References

This guide explains how to maintain cross-references between different sections of the Probe documentation. Proper cross-referencing is essential for helping users navigate the documentation and find related information.

## Importance of Cross-References

Cross-references serve several important purposes:

1. **Navigation**: Help users move between related topics
2. **Context**: Provide additional context for complex topics
3. **Completeness**: Ensure users can find all relevant information
4. **Coherence**: Create a unified documentation experience

## Types of Cross-References

The Probe documentation uses several types of cross-references:

### 1. Section Links

Links between major sections of the documentation:

```markdown
For more information on installation, see the [Core Concepts & Setup](/installation) section.
```

### 2. Related Topics

Links to related topics within the same section:

```markdown
Learn more about [search patterns](/search-functionality#patterns) to improve your search results.
```

### 3. Workflow Connections

Links that connect different parts of a workflow:

```markdown
After [setting up the MCP server](/mcp-server#setting-up-the-mcp-server), you can [integrate it with your AI editor](/mcp-integration#editor-integration).
```

### 4. Reference Links

Links to reference documentation for specific features:

```markdown
See the [CLI Reference](/cli-mode#search-command) for a complete list of search options.
```

## Cross-Reference Patterns

When adding cross-references, follow these patterns:

### Between Core Concepts and Use Cases

Core Concepts pages should link to relevant Use Cases:

```markdown
# Language Support

[...content...]

## Using in Your Workflow

Once you understand the supported languages, you can:

- [Use Probe with AI Code Editors](/mcp-integration) for intelligent code assistance
- [Build custom tools with the Node.js SDK](/nodejs-sdk) for language-specific processing
```

Use Cases should link back to relevant Core Concepts:

```markdown
# Using with AI Code Editors

[...content...]

This integration relies on Probe's [language parsing capabilities](/language-support-overview) to provide accurate code context.
```

### Between Use Cases and Reference

Use Cases should link to relevant Reference documentation:

```markdown
# Using in the CLI for Advanced AI

[...content...]

For a complete list of CLI options, see the [CLI Reference](/cli-mode).
```

Reference documentation should mention relevant Use Cases:

```markdown
# CLI Reference

[...content...]

These commands are commonly used in [AI chat workflows](/ai-chat) and when [building custom tools](/nodejs-sdk).
```

## Maintaining Cross-References

When updating the documentation structure, follow these guidelines to maintain cross-references:

### 1. Update Links When Moving Pages

If you move a page to a new location, update all links to that page:

```bash
# Find all links to a specific page
grep -r "\[.*\](\/old-page-path)" site/
```

### 2. Check for Broken Links

Regularly check for broken links in the documentation:

```bash
# Using a tool like linkcheck
npx linkcheck https://probe-docs.example.com
```

### 3. Add Context to Links

When adding a link, include context about what the user will find:

```markdown
# Good
For details on output formatting, see the [Output Formats Reference](/output-formats).

# Better
For details on how to format search results as JSON or XML, see the [Output Formats Reference](/output-formats#json-and-xml).
```

### 4. Use Anchor Links

Link to specific sections of a page when appropriate:

```markdown
See the [search command options](/cli-mode#search-options) for more details.
```

## Cross-Reference Map

The following table shows the key cross-references between different sections of the documentation:

| From | To | Purpose |
|------|-------|---------|
| Core Concepts → | Use Cases | Show how concepts are applied in practice |
| Use Cases → | Core Concepts | Provide background on underlying concepts |
| Use Cases → | Reference | Point to detailed technical information |
| Reference → | Use Cases | Show practical applications of technical features |

## Examples of Effective Cross-References

### Example 1: From Core Concepts to Use Cases

```markdown
# What is Probe?

[...content about Probe's features...]

## Real-World Applications

Probe can be used in various workflows:

- For AI-assisted coding, see [Using with AI Code Editors](/mcp-integration)
- For team collaboration, see [Hosting a Team Chat](/web-interface)
- For custom AI tools, see [Building AI Tools with Node.js SDK](/nodejs-sdk)
```

### Example 2: From Use Cases to Reference

```markdown
# Using in the CLI for Advanced AI

[...content about CLI usage...]

## Advanced Configuration

For advanced use cases, you can configure various aspects of the CLI:

- For output formatting options, see [Output Formats Reference](/output-formats)
- For search pattern syntax, see [Search Functionality](/search-functionality#patterns)
- For environment variables, see [AI Integration Reference](/ai-integration#configuration-options)
```

### Example 3: From Reference to Core Concepts and Use Cases

```markdown
# CLI Reference

[...command reference...]

## Background

The CLI commands are built on Probe's [core search technology](/how-it-works#search-technology).

## Common Workflows

These commands are commonly used in:

- [AI chat sessions](/ai-chat) for interactive code exploration
- [Custom AI tools](/nodejs-sdk) for automated code analysis
```

## Best Practices Summary

1. **Be Specific**: Link to the most specific section that contains the relevant information
2. **Add Context**: Explain what the user will find at the linked location
3. **Maintain Bidirectional Links**: Ensure that related topics link to each other
4. **Check Regularly**: Verify that links remain valid as the documentation evolves
5. **Use Consistent Language**: Use consistent phrasing for similar types of cross-references