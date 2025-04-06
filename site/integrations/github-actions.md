---
title: GitHub Actions Integration
description: Automate issue responses and PR reviews using Probe with GitHub Actions.
---

# GitHub Actions Integration

Probe can be integrated into your GitHub workflow to automate responses to issues and pull requests, acting as an AI assistant powered by code context. This allows Probe to help answer questions about issues or review code changes directly within GitHub.

## Example Workflow: Issue & PR Assistant

This example workflow demonstrates how to set up Probe to respond to commands in issue comments and newly opened pull requests or issues.

**File:** `.github/workflows/probe-issues.yml`

```yaml
name: AI Comment Handler

on:
  pull_request:
    types: [opened] # Trigger on new PRs
  issue_comment:
    types: [created] # Trigger on new issue comments
  issues:
    types: [opened] # Trigger on new issues

# Define permissions needed for the workflow
permissions:
  issues: write
  pull-requests: write
  contents: read

jobs:
  trigger_probe_chat:
    # Use the reusable workflow from the main Probe repository
    uses: buger/probe/.github/workflows/probe.yml@main
    # Pass required inputs
    with:
      # Define the command prefix to trigger the bot
      command_prefix: "/probe" # Or '/ai', '/ask', etc.
      # Optionally override the default npx command if the secret isn't set
      # default_probe_chat_command: 'node path/to/custom/script.js'
    # Pass necessary secrets to the reusable workflow
    secrets:
      ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
      ANTHROPIC_API_URL: ${{ secrets.ANTHROPIC_API_URL }}
      # GITHUB_TOKEN is automatically passed
```

### How it Works

1.  **Triggers**: The workflow runs when:
    *   A new pull request is opened.
    *   A new issue is opened.
    *   A comment is created on an existing issue.
    *   *(Note: The commented-out `if` condition shows how to restrict triggers further, e.g., based on labels)*.

2.  **Permissions**: It requires `write` permissions for issues and pull requests to post comments, and `read` permission for contents to access repository code if needed by the underlying Probe process.

3.  **Reusable Workflow**: It utilizes `buger/probe/.github/workflows/probe.yml@main`, which contains the core logic for:
    *   Parsing the comment or issue/PR body for the command prefix.
    *   Invoking the Probe AI agent (likely using the Node.js SDK or CLI).
    *   Using Probe to search the codebase for relevant context.
    *   Generating a response using an AI model (like Anthropic Claude, configured via secrets).
    *   Posting the response back as a comment.

4.  **Configuration**:
    *   `command_prefix`: This input (`/probe` in the example) determines how users interact with the bot. To trigger it, a user would comment `/probe <Your question or request>`.
    *   `secrets`: You must configure `ANTHROPIC_API_KEY` (and optionally `ANTHROPIC_API_URL`) in your repository's secrets for the AI model to function. `GITHUB_TOKEN` is automatically available and used by the reusable workflow to interact with the GitHub API.

### Use Cases

*   **Ticket Answering Assistant**: Users can ask questions about an issue by commenting `/probe How does function X relate to this bug?`. The workflow triggers Probe to analyze the code and provide an answer.
*   **Pull Request Reviewer**: When a PR is opened, or in a PR comment, users can invoke the bot with `/probe Review this change for potential issues related to Y`. Probe can analyze the diff and relevant code sections to provide feedback. *(The exact review capabilities depend on the implementation within the reusable `probe.yml` workflow)*.

This integration streamlines workflows by bringing code-aware AI assistance directly into your GitHub issues and pull requests.