# Probe Documentation Maintenance Guide

This guide is for documentation maintainers and contributors. It provides technical information about the documentation structure, cross-references, and navigation configuration.

## Navigation Structure

The Probe documentation uses VitePress and follows a specific navigation structure designed to provide a logical organization of content.

### VitePress Configuration

Add this configuration to your VitePress configuration file (`.vitepress/config.js`):

```javascript
export default {
  // ... other VitePress configuration options
  
  themeConfig: {
    // ... other theme configuration options
    
    sidebar: {
      '/': [
        {
          text: 'Getting Started',
          collapsed: false,
          items: [
            { text: 'Quick Start', link: '/quick-start' },
            { text: 'Installation', link: '/installation' },
            { text: 'Core Features', link: '/features' },
          ]
        },
        {
          text: 'Core Functionality',
          collapsed: false,
          items: [
            { text: 'How It Works', link: '/how-it-works' },
            { text: 'Search Functionality', link: '/search-functionality' },
            { text: 'Code Extraction', link: '/code-extraction' },
            { text: 'CLI Mode', link: '/cli-mode' },
          ]
        },
        {
          text: 'AI Integration',
          collapsed: false,
          items: [
            { text: 'AI Integration Overview', link: '/ai-integration' },
            { text: 'AI Chat Mode', link: '/ai-chat' },
            { text: 'MCP Server', link: '/mcp-server' },
            { text: 'MCP Integration', link: '/mcp-integration' },
            { text: 'Node.js SDK', link: '/nodejs-sdk' },
            { text: 'Web Interface', link: '/web-interface' },
          ]
        },
        {
          text: 'Language Support',
          collapsed: false,
          items: [
            { text: 'Language Support Overview', link: '/language-support-overview' },
            { text: 'Supported Languages', link: '/supported-languages' },
            { text: 'Adding Languages', link: '/adding-languages' },
          ]
        },
        {
          text: 'Advanced Topics',
          collapsed: true,
          items: [
            { text: 'Result Ranking', link: '/result-ranking' },
            { text: 'Pattern Matching', link: '/pattern-matching' },
            { text: 'Performance Optimization', link: '/performance-optimization' },
            { text: 'Customization', link: '/customization' },
          ]
        },
        {
          text: 'Contributing',
          collapsed: true,
          items: [
            { text: 'Contributing Guide', link: '/contributing' },
            { text: 'Code of Conduct', link: '/code-of-conduct' },
            { text: 'Development Setup', link: '/development-setup' },
          ]
        },
      ]
    },
    
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Quick Start', link: '/quick-start' },
      { 
        text: 'Documentation', 
        items: [
          { text: 'Getting Started', items: [
            { text: 'Quick Start', link: '/quick-start' },
            { text: 'Installation', link: '/installation' },
            { text: 'Core Features', link: '/features' },
          ]},
          { text: 'Core Functionality', items: [
            { text: 'How It Works', link: '/how-it-works' },
            { text: 'Search Functionality', link: '/search-functionality' },
            { text: 'Code Extraction', link: '/code-extraction' },
            { text: 'CLI Mode', link: '/cli-mode' },
          ]},
          { text: 'AI Integration', items: [
            { text: 'AI Integration Overview', link: '/ai-integration' },
            { text: 'AI Chat Mode', link: '/ai-chat' },
            { text: 'MCP Server', link: '/mcp-server' },
            { text: 'MCP Integration', link: '/mcp-integration' },
            { text: 'Node.js SDK', link: '/nodejs-sdk' },
            { text: 'Web Interface', link: '/web-interface' },
          ]},
          { text: 'Language Support', items: [
            { text: 'Language Support Overview', link: '/language-support-overview' },
            { text: 'Supported Languages', link: '/supported-languages' },
            { text: 'Adding Languages', link: '/adding-languages' },
          ]},
        ]
      },
      { text: 'GitHub', link: 'https://github.com/probelabs/probe' }
    ],
  }
}
```

## Cross-Reference Guidelines

Proper cross-referencing improves navigation, provides contextual understanding, reduces duplication, and ensures comprehensive coverage.

### Cross-Reference Implementation

When implementing cross-references, follow these guidelines:

1. **Contextual References**: Include references where they make sense in the context of the document, not just at the end.

2. **Descriptive Link Text**: Use descriptive link text that explains what the linked document contains:
   ```markdown
   Learn more about [how Probe works](/how-it-works)
   ```
   Instead of:
   ```markdown
   [how-it-works.md](/how-it-works)
   ```

3. **Section References**: When appropriate, link to specific sections within documents using anchor links:
   ```markdown
   [Search Syntax](/search-functionality#search-syntax)
   ```

4. **Related Topics Sections**: Include a "Related Topics" section at the end of each document that lists related documents.

5. **Next Steps**: Where appropriate, include a "Next Steps" section that guides users to logical next documents to read.

### Example Implementation

Here's an example of how to implement these cross-references in a document:

```markdown
# Quick Start

This guide will help you get up and running with Probe quickly.

## Installation

The easiest way to install Probe is via npm:

```bash
npm install -g @probelabs/probe@latest
```

For more detailed installation instructions, including manual installation and building from source, see the [Installation Guide](/installation).

...

## Next Steps

- Learn more about the [CLI Mode](/cli-mode) for detailed command options
- Explore the [AI Chat Mode](/ai-chat) for interactive code exploration
- Check out the [Web Interface](/web-interface) for a browser-based experience
- Understand [How It Works](/how-it-works) to get the most out of Probe

## Related Topics

- [Installation](/installation) - Detailed installation instructions
- [CLI Mode](/cli-mode) - Command-line interface reference
- [AI Chat Mode](/ai-chat) - Interactive CLI interface for asking questions about your code
- [Web Interface](/web-interface) - Browser-based exploration
```

## Cross-Reference Map

The following map shows how different documentation files should reference each other:

### Getting Started Section

- **index.md**
  - Links to: quick-start.md, installation.md, features.md, how-it-works.md, ai-integration.md, language-support-overview.md
  
- **quick-start.md**
  - Links to: installation.md, cli-mode.md, ai-chat.md, web-interface.md, how-it-works.md
  - Referenced by: index.md, installation.md

- **installation.md**
  - Links to: quick-start.md, cli-mode.md
  - Referenced by: index.md, quick-start.md

- **features.md**
  - Links to: how-it-works.md, search-functionality.md, code-extraction.md, ai-integration.md, language-support-overview.md
  - Referenced by: index.md, how-it-works.md

### Core Functionality Section

- **how-it-works.md**
  - Links to: search-functionality.md, code-extraction.md, features.md, language-support-overview.md
  - Referenced by: index.md, features.md, search-functionality.md, code-extraction.md

- **search-functionality.md**
  - Links to: how-it-works.md, code-extraction.md, cli-mode.md
  - Referenced by: how-it-works.md, features.md, cli-mode.md, language-support-overview.md

- **code-extraction.md**
  - Links to: how-it-works.md, search-functionality.md, language-support-overview.md
  - Referenced by: how-it-works.md, features.md, search-functionality.md, language-support-overview.md

- **cli-mode.md**
  - Links to: search-functionality.md, code-extraction.md, installation.md
  - Referenced by: quick-start.md, search-functionality.md, installation.md

### AI Integration Section

- **ai-integration.md**
  - Links to: ai-chat.md, mcp-server.md, mcp-integration.md, nodejs-sdk.md, web-interface.md
  - Referenced by: index.md, features.md

- **ai-chat.md**
  - Links to: ai-integration.md, nodejs-sdk.md
  - Referenced by: ai-integration.md, quick-start.md, index.md

- **mcp-server.md**
  - Links to: ai-integration.md, mcp-integration.md
  - Referenced by: ai-integration.md, mcp-integration.md

- **mcp-integration.md**
  - Links to: ai-integration.md, mcp-server.md
  - Referenced by: ai-integration.md, index.md

- **nodejs-sdk.md**
  - Links to: ai-integration.md, ai-chat.md, web-interface.md
  - Referenced by: ai-integration.md, index.md

- **web-interface.md**
  - Links to: ai-integration.md, nodejs-sdk.md
  - Referenced by: ai-integration.md, quick-start.md

### Language Support Section

- **language-support-overview.md**
  - Links to: supported-languages.md, adding-languages.md, search-functionality.md, code-extraction.md
  - Referenced by: index.md, features.md, how-it-works.md

- **supported-languages.md**
  - Links to: language-support-overview.md, adding-languages.md
  - Referenced by: language-support-overview.md

- **adding-languages.md**
  - Links to: language-support-overview.md, supported-languages.md
  - Referenced by: language-support-overview.md

## Maintenance Best Practices

As the documentation evolves:

1. **Update the cross-reference map** when adding new documentation files
2. **Review existing cross-references** when updating documentation to ensure they remain relevant
3. **Check for broken links** regularly
4. **Ensure all document links are correct** and point to existing files
5. **Update the navigation** when adding new documentation files
6. **Maintain consistent formatting and style** across all documentation pages
7. **Consider automating cross-reference validation** as part of the documentation build process