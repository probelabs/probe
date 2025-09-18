# Probe Documentation Site

This directory contains the VitePress-powered documentation site for the Probe project. This README provides information about the documentation structure, how to contribute, and how to maintain the documentation.

## Documentation Structure

The documentation is organized into several main sections:

- **Getting Started**: Quick Start, Installation, Core Features
- **Core Functionality**: How It Works, Search Functionality, Code Extraction, CLI Mode
- **AI Integration**: AI Integration Overview, AI Chat Mode, MCP Server, MCP Integration, Node.js SDK, Web Interface
- **Language Support**: Language Support Overview, Supported Languages, Adding Languages

For a complete overview of the documentation structure, see [documentation-maintenance.md](./documentation-maintenance.md).

## Key Documentation Files

- **index.md**: Main landing page with feature highlights
- **quick-start.md**: Getting started guide
- **installation.md**: Detailed installation instructions
- **features.md**: Overview of core features
- **how-it-works.md**: Technical explanation of Probe's architecture
- **ai-integration.md**: Overview of AI integration capabilities
- **language-support-overview.md**: Overview of language support features
- **documentation-guide.md**: Guide for users on how to use the documentation
- **documentation-maintenance.md**: Technical guide for documentation maintainers

## Cross-References

The documentation uses cross-references to help users navigate between related topics. For a complete guide on cross-references and documentation maintenance, see [documentation-maintenance.md](./documentation-maintenance.md).

When adding new documentation or updating existing documentation, please ensure that cross-references are maintained and updated as needed.

## Defining Features in index.md

The `index.md` file supports a custom features section that can be defined in the frontmatter. Each feature can have the following properties:

```yaml
features:
  - icon: 🔎                      # Emoji icon (optional if image is provided)
    title: Feature Title          # Required
    details: Feature description  # Required
    link: /page-link              # Optional link to another page
    linkText: Learn more          # Optional custom text for the link (defaults to "Learn more")
    theme: alt                    # Optional theme (alt, brand, or default)
    image: /icons/my-icon.svg     # Optional image path (replaces the emoji icon)
    highlight: true               # Optional boolean to highlight the feature
```

### Example Feature Definition

Here's an example of how to define features in the index.md frontmatter:

```yaml
---
layout: home

hero:
  name: "Probe"
  text: "Local, AI-ready\nCode Exploration"
  tagline: "Open-source tooling that truly understands your codebase."
  
features:
  - icon: 🔎
    title: AST-Aware Code Search
    details: Search your code with semantic understanding. Find functions, classes, and patterns across your entire codebase.
    link: /how-it-works
    linkText: Learn how it works
    highlight: true
    
  - icon: ⚡
    title: Lightning Fast
    details: Built on ripgrep and tree-sitter for blazing fast performance. No indexing needed.
    theme: alt
    
  - icon: 🔒
    title: 100% Local & Private
    details: Your code stays on your machine. Perfect for sensitive projects.
    image: /icons/privacy-icon.svg
---
```

## Custom Components

The documentation uses several custom components:

### CodeEditor

The `CodeEditor` component displays code snippets with syntax highlighting:

```markdown
<CodeEditor filePath="example.js">
function handleError(error) {
  console.error(`Error: ${error.message}`);
  return { success: false, error: error.message };
}
</CodeEditor>
```

### CommandExample

The `CommandExample` component displays command-line examples:

```markdown
<CommandExample>probe search "error handling AND ext:rs" ./src</CommandExample>
```

With output:

```markdown
<CommandExample output="Found 3 matches in 2 files">probe search "user authentication" ./src</CommandExample>
```

## Documentation Conventions

Please follow these conventions when contributing to the documentation:

1. **File Names**: Use kebab-case for file names (e.g., `quick-start.md`, `how-it-works.md`)
2. **Headings**: Use Title Case for headings (e.g., "Getting Started", "Core Functionality")
3. **Code Examples**: Use appropriate syntax highlighting for code examples
4. **Cross-References**: Use relative links for cross-references (e.g., `[Quick Start](./quick-start.md)`)
5. **Images**: Store images in the `public` directory and reference them with absolute paths (e.g., `/images/example.png`)

## Running the Site Locally

To run the site locally:

1. Navigate to the site directory
2. Install dependencies: `npm install`
3. Start the development server: `npm run dev`

## Deployment

The site is deployed using **Cloudflare Pages** with automatic deployments from the main branch to **probelabs.com**.

### Deployment Configuration
- Platform: Cloudflare Pages
- Domain: probelabs.com
- Build command: `npm run build`
- Output directory: `.vitepress/dist`
- Node.js version: 20

For detailed deployment instructions, see [DEPLOYMENT.md](./DEPLOYMENT.md).

### Previous Deployment
The site was previously deployed via GitHub Pages. The old workflow has been disabled and renamed to `vitepress-gh-pages.yml.disabled`.

## Building for Production

To build the site for production:

```bash
npm run build
```

The built site will be in the `.vitepress/dist` directory.

## Contributing to the Documentation

We welcome contributions to the Probe documentation. If you find errors, omissions, or have suggestions for improvements, please consider contributing:

1. Fork the [Probe repository](https://github.com/probelabs/probe) on GitHub
2. Make your changes to the documentation files in the `site` directory
3. Ensure that cross-references are maintained and updated as needed
4. Run the site locally to verify your changes
5. Submit a pull request with your changes

## Maintenance Guidelines

When maintaining the documentation:

1. **Keep Cross-References Updated**: When adding or removing documentation, update cross-references in related files
2. **Maintain Consistent Style**: Follow the established style and formatting conventions
3. **Update Navigation**: When adding new documentation, update the navigation structure as described in [documentation-maintenance.md](./documentation-maintenance.md)
4. **Check for Broken Links**: Regularly check for broken links and fix them
5. **Update Examples**: Keep code examples and command examples up to date with the latest version of Probe
6. **Review Regularly**: Regularly review the documentation for accuracy and completeness

## VitePress Configuration

The VitePress configuration is located in the `.vitepress/config.js` file. When updating the documentation structure, be sure to update the sidebar and navigation configuration in this file.

For more information on VitePress configuration, see the [VitePress documentation](https://vitepress.dev/reference/site-config).