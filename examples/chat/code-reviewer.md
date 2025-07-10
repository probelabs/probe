# Code Reviewer with Probe

This example demonstrates how to use Probe as a code reviewer that can analyze code changes and provide feedback.

## Overview

The code reviewer uses Probe's search and analysis capabilities to:
- Examine code changes in pull requests
- Identify potential issues and improvements
- Provide detailed feedback on code quality
- Check for common patterns and anti-patterns

## Setup

1. Install dependencies:
```bash
npm install
```

2. Set up your API keys:
```bash
export ANTHROPIC_API_KEY="your-key-here"
# or
export OPENAI_API_KEY="your-key-here"
# or
export GOOGLE_API_KEY="your-key-here"
```

3. Configure allowed folders (optional):
```bash
export ALLOWED_FOLDERS="/path/to/your/project,/path/to/another/project"
```

## Usage

### Basic Code Review

```bash
node mcp-agent/src/cli.js "Please review the authentication module in src/auth/"
```

### Review Specific Files

```bash
node mcp-agent/src/cli.js "Review the changes in src/components/Button.tsx and check for accessibility issues"
```

### Security Review

```bash
node mcp-agent/src/cli.js "Perform a security review of the API endpoints in src/api/"
```

## GitHub Integration

When using Probe as a code reviewer in GitHub Actions or other CI/CD pipelines, you can use the failure tag feature to fail the build when critical issues are found.

### Failure Tag Usage

The code reviewer can use the `<probe-failure>` tag to indicate when a review should fail the CI/CD pipeline:

```markdown
<probe-failure>
Critical security vulnerability found: SQL injection in user authentication
</probe-failure>

The authentication module contains a serious SQL injection vulnerability...
```

When this tag is detected:

1. The failure message is extracted and displayed prominently at the top of the response
2. The original `<probe-failure>` tags are removed from the response
3. The process exits with status code 1, causing CI/CD pipelines to fail

### Example GitHub Action

```yaml
name: Code Review
on: [pull_request]

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      - run: npm install
      - name: Run Code Review
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
        run: |
          node mcp-agent/src/cli.js "Review this pull request for security issues and code quality problems"
```

## Features

### Automated Analysis
- **Security scanning**: Identifies potential security vulnerabilities
- **Code quality**: Checks for maintainability and best practices
- **Performance**: Spots potential performance issues
- **Testing**: Ensures adequate test coverage

### Intelligent Feedback
- **Context-aware**: Understands the broader codebase context
- **Actionable suggestions**: Provides specific improvement recommendations
- **Priority levels**: Distinguishes between critical issues and suggestions

### Integration Ready
- **CI/CD compatible**: Works with GitHub Actions, GitLab CI, and other platforms
- **Configurable**: Customizable rules and severity levels
- **Failure handling**: Can fail builds when critical issues are found

## Configuration

### Environment Variables

- `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `GOOGLE_API_KEY`: AI provider API key
- `ALLOWED_FOLDERS`: Comma-separated list of allowed directories
- `DEBUG`: Enable debug logging
- `MAX_TOKENS`: Maximum tokens per response
- `FORCE_PROVIDER`: Force specific AI provider (anthropic/openai/google)

### Review Prompts

You can customize the review behavior by providing specific prompts:

```bash
# Security-focused review
node mcp-agent/src/cli.js "Focus on security vulnerabilities, especially authentication and data validation"

# Performance review
node mcp-agent/src/cli.js "Review for performance issues and optimization opportunities"

# Architecture review
node mcp-agent/src/cli.js "Analyze the overall architecture and suggest improvements"
```
