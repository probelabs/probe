---
title: GitHub Actions Integration
description: Automate issue responses, PR reviews, and code modifications using Probe with GitHub Actions.
---

# GitHub Actions Integration

Probe can be integrated into your GitHub workflow to automate responses to issues and pull requests, and even implement code changes, acting as an AI assistant powered by code context. This allows Probe to help answer questions, review code, or make modifications directly within GitHub.

## Example Workflows

### Example 1: Issue & PR Assistant (Read-Only)

This example workflow demonstrates how to set up Probe to respond to commands in issue comments and newly opened pull requests or issues without modifying code.

**File:** `.github/workflows/probe-assistant.yml`

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
  issues: write # To post comments on issues
  pull-requests: write # To post comments on PRs
  contents: read # To read repository code for context

jobs:
  trigger_probe_chat:
    # Use the reusable workflow from the main Probe repository
    uses: probelabs/probe/.github/workflows/probe.yml@main
    # Pass required inputs
    with:
      # Define the command prefix to trigger the bot
      command_prefix: "/probe" # Or '/ai', '/ask', etc.
      # Optionally override the default npx command if the secret isn't set
      # default_probe_chat_command: 'node path/to/custom/script.js'
      # Comment management options
      update_existing_comment: true # Update existing comment instead of creating new ones
      update_comment_marker: "<!-- probe-assistant -->" # Custom marker for this workflow
    # Pass necessary secrets to the reusable workflow
    secrets:
      ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
      ANTHROPIC_API_URL: ${{ secrets.ANTHROPIC_API_URL }}
      # GITHUB_TOKEN is automatically passed
```

#### How it Works (Read-Only Assistant)

1.  **Triggers**: The workflow runs when a new PR/issue is opened or a comment is made on an issue.
2.  **Permissions**: It requires `write` permissions for issues/PRs to post comments and `read` permission for `contents` to access repository code for analysis.
3.  **Reusable Workflow**: It utilizes `probelabs/probe/.github/workflows/probe.yml@main`, which handles parsing the command, invoking the Probe AI agent, searching the codebase, generating a response, and posting it back as a comment.
4.  **Configuration**:
    *   `command_prefix`: Defines how users trigger the bot (e.g., `/probe <Your question>`).
    *   `secrets`: Requires AI provider secrets (e.g., `ANTHROPIC_API_KEY`).

### Example 2: AI Engineer (Code Editing Enabled)

This example demonstrates how to configure Probe to not only respond to comments but also potentially implement code changes using a specific AI persona (`engineer`) and enabling edit capabilities.

**File:** `.github/workflows/probe-engineer.yml`

```yaml
name: Probe Engineer Handler

on:
  issue_comment:
    types: [created] # Trigger only on issue comments

# Define permissions needed for the workflow
permissions:
  issues: write # To post comments
  pull-requests: write # To potentially create/comment on PRs (depending on implement tool behavior)
  contents: write # REQUIRED to allow code modifications

jobs:
  trigger_probe_implement:
    # Use the reusable workflow
    uses: probelabs/probe/.github/workflows/probe.yml@main
    with:
      # Define the command prefix
      command_prefix: "/engineer" # Specific prefix for this persona
      # --- Enable Code Modifications ---
      allow_suggestions: true # Enable suggestions via suggest-changes (recommended for PR contexts)
      # allow_edit: true # Alternative: enable direct commits
      # --- Set AI Persona ---
      prompt: engineer # Use the 'engineer' predefined prompt
      # --- Comment Management ---
      update_existing_comment: true # Update existing comment instead of creating new ones
      update_comment_marker: "<!-- probe-engineer -->" # Custom marker for engineer workflow
    secrets:
      # AI Provider Secrets
      ANTHROPIC_API_KEY: ${{ secrets.PROBE_ANTHROPIC_API_KEY }}
      ANTHROPIC_API_URL: ${{ secrets.PROBE_ANTHROPIC_URL }}
      # GitHub App Secrets (Potentially needed for more complex git operations)
      APP_ID: ${{ secrets.PROBE_APP_ID }}
      APP_PRIVATE_KEY: ${{ secrets.PROBE_APP_PRIVATE_KEY }}
      # GITHUB_TOKEN is automatically passed
```

#### How it Works (AI Engineer)

1.  **Trigger**: This workflow specifically triggers on issue comments.
2.  **Permissions**: Crucially, it requires `contents: write` permission in addition to `issues: write` and potentially `pull-requests: write`. The `contents: write` permission is essential for allowing the workflow to modify files in the repository.
3.  **Reusable Workflow**: Still uses `probelabs/probe/.github/workflows/probe.yml@main`.
4.  **Configuration**:
    *   `command_prefix`: Uses `/engineer` to invoke this specific workflow.
    *   `allow_edit: true`: This flag enables Probe's code editing capabilities (likely via the `implement` tool). **Use with caution.**
    *   `prompt: engineer`: Instructs the AI agent to use the specialized "engineer" persona, focusing on analysis and implementation.
    *   `secrets`: May require additional secrets like GitHub App credentials (`APP_ID`, `APP_PRIVATE_KEY`) if the implementation involves creating commits or PRs under a specific App identity, depending on the underlying `implement` tool's behavior.

## Code Modification Options

Probe can help you make code changes in two ways:

### Direct Changes (`allow_edit: true`)

When you set `allow_edit: true`, Probe can make changes directly to your code.

*   **What it does**: Probe can modify files in your repository when you ask it to (like "Fix this bug" or "Add error handling").
*   **Requirements**: Your workflow needs permission to write to your repository (`contents: write`).
*   **Result**: Changes are saved directly to your code.

### Suggested Changes (`allow_suggestions: true`) - Recommended

When you set `allow_suggestions: true`, Probe creates suggestions instead of making direct changes.

*   **What it does**: Probe analyzes your code and creates suggestions that appear in pull requests, just like when a human reviewer suggests changes.
*   **Requirements**: Your workflow needs permission to read your code (`contents: read`) and comment on pull requests (`pull-requests: write`).
*   **Result**: You see suggested changes that you can accept or reject with one click.
*   **When it works**: Only in pull requests. For other contexts, no changes are made.

### Which Should You Choose?

**We recommend `allow_suggestions: true`** because:
- You get to review changes before they're applied
- It's safer than automatic changes
- You maintain full control over your code
- It works great with GitHub's built-in review tools

**Important**: Both options let AI modify your code, so always review the changes carefully before accepting them.

## Manual Triggering (`workflow_dispatch`)

You can allow workflows using `probe.yml` to be triggered manually from the GitHub UI. This is useful for testing or running the agent on demand without needing an issue comment or PR event.

1.  **Add `workflow_dispatch` to Triggers**: Modify the `on:` section of your workflow file:

    ```yaml
    name: Manual Probe Run

    on:
      workflow_dispatch: # Allows manual triggering
        # Optional: Define inputs for the manual run
        inputs:
          user_request:
            description: 'The request/question for Probe'
            required: true
            default: 'Explain the main function in src/main.rs'
          # Add other inputs if needed, e.g., target branch

      # Keep other triggers if needed
      issue_comment:
        types: [created]

    permissions:
      # ... your permissions ...
      contents: read # Or write if allow_edit is true

    jobs:
      trigger_probe_manual:
        uses: probelabs/probe/.github/workflows/probe.yml@main
        with:
          # Use the input from the manual trigger
          manual_input: ${{ github.event.inputs.user_request }}
          # Set other parameters as needed
          allow_edit: false # Example: disable editing for manual runs
          # command_prefix is not typically needed for workflow_dispatch
        secrets:
          # ... your secrets ...
    ```

2.  **Trigger from GitHub UI**:
    *   Go to your repository's "Actions" tab.
    *   Select the workflow (e.g., "Manual Probe Run") from the list on the left.
    *   Click the "Run workflow" dropdown button.
    *   If you defined inputs (like `user_request`), fill them in the form.
    *   Click the "Run workflow" button.

The reusable `probe.yml` workflow needs to be designed to handle the `manual_input` (or a similarly named input) when triggered via `workflow_dispatch`. Check the `probe.yml` documentation for specifics on how it handles manual inputs.

## Advanced Configuration

### OpenTelemetry Tracing

Probe's GitHub Actions integration supports OpenTelemetry tracing to monitor AI model interactions and performance metrics. This is particularly useful for understanding usage patterns, debugging issues, and optimizing AI interactions.

#### Enable Tracing

Add the `enable_tracing` parameter to your workflow:

```yaml
jobs:
  trigger_probe_chat:
    uses: probelabs/probe/.github/workflows/probe.yml@main
    with:
      command_prefix: "/probe"
      enable_tracing: true # Enable OpenTelemetry tracing
    secrets:
      ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
```

#### File Tracing (Default)

When `enable_tracing: true` is set without `TRACING_URL`, traces are saved to a file:

- **Output**: `probe-traces.jsonl` file in JSON Lines format
- **Artifact**: Automatically uploaded as `probe-traces` artifact
- **Retention**: 30 days (configurable)
- **Download**: Available in the Actions run artifacts section

```yaml
jobs:
  trigger_probe_chat:
    uses: probelabs/probe/.github/workflows/probe.yml@main
    with:
      command_prefix: "/probe"
      enable_tracing: true
    secrets:
      ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
      # No TRACING_URL = file tracing
```

#### Remote Tracing

For remote tracing to an OpenTelemetry collector, add the `TRACING_URL` secret:

```yaml
jobs:
  trigger_probe_chat:
    uses: probelabs/probe/.github/workflows/probe.yml@main
    with:
      command_prefix: "/probe"
      enable_tracing: true
    secrets:
      ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
      TRACING_URL: ${{ secrets.TRACING_URL }} # e.g., http://localhost:4318/v1/traces
```

#### What Gets Traced

The tracing system captures comprehensive data about AI interactions:

- **AI Model Calls**: Request/response cycles with detailed timing
- **Token Usage**: Prompt tokens, completion tokens, and total consumption
- **Performance Metrics**: Response times, request durations, and throughput
- **GitHub Context**: Issue/PR numbers, repository information, and workflow details
- **Session Information**: Unique session IDs and conversation flow
- **Error Tracking**: Failed requests, timeouts, and error details

#### Example Trace Data

```json
{
  "traceId": "abc123def456...",
  "spanId": "789ghi012jkl...",
  "name": "ai.generateText",
  "startTimeUnixNano": 1704067200000000000,
  "endTimeUnixNano": 1704067201000000000,
  "attributes": {
    "ai.model.id": "claude-3-7-sonnet-20250219",
    "ai.model.provider": "anthropic",
    "ai.telemetry.functionId": "chat-issue-123",
    "ai.telemetry.metadata.sessionId": "github-issue-123",
    "ai.telemetry.metadata.iteration": "1",
    "github.repository": "owner/repo",
    "github.issue.number": "123",
    "github.event": "issue_comment"
  },
  "events": [
    {
      "name": "ai.request.start",
      "attributes": {
        "ai.request.messages": "[{\"role\":\"user\",\"content\":\"How does X work?\"}]"
      }
    },
    {
      "name": "ai.response.complete",
      "attributes": {
        "ai.usage.prompt_tokens": "245",
        "ai.usage.completion_tokens": "156",
        "ai.usage.total_tokens": "401"
      }
    }
  ]
}
```

#### Setting Up Remote Tracing

For remote tracing with Jaeger:

1. **Set up Jaeger**:
   ```bash
   docker run -d --name jaeger \
     -p 16686:16686 \
     -p 4318:4318 \
     jaegertracing/all-in-one:latest
   ```

2. **Add Repository Secret**:
   - Go to Settings > Secrets and variables > Actions
   - Add: `TRACING_URL` = `http://localhost:4318/v1/traces`

3. **Configure Workflow**:
   ```yaml
   jobs:
     trigger_probe_chat:
       uses: probelabs/probe/.github/workflows/probe.yml@main
       with:
         enable_tracing: true
       secrets:
         TRACING_URL: ${{ secrets.TRACING_URL }}
   ```

#### Analyzing Traces

Download and analyze trace files:

```bash
# Download artifact from GitHub Actions
# Extract probe-traces.jsonl

# View traces
cat probe-traces.jsonl | jq '.'

# Count interactions by model
cat probe-traces.jsonl | jq -r '.attributes."ai.model.id"' | sort | uniq -c

# Calculate total token usage
cat probe-traces.jsonl | jq -r '.events[]? | select(.name == "ai.response.complete") | 
  .attributes."ai.usage.total_tokens"' | jq -s 'add'

# Find slow responses (>5 seconds)
cat probe-traces.jsonl | jq -r 'select(((.endTimeUnixNano - .startTimeUnixNano) / 1000000000) > 5) | 
  {duration: ((.endTimeUnixNano - .startTimeUnixNano) / 1000000000), name: .name}'
```

#### Use Cases for Tracing

- **Performance Monitoring**: Track AI response times and optimize workflows
- **Usage Analytics**: Understand token consumption patterns and costs
- **Debugging**: Identify failed requests and troubleshoot issues
- **Capacity Planning**: Monitor request volumes and plan scaling
- **Cost Optimization**: Analyze token usage to optimize AI interactions

### Comment Management

By default, Probe creates a new comment for each invocation. However, you can configure it to update an existing comment instead, which helps reduce comment clutter in issues and pull requests.

#### Parameters

*   **`update_existing_comment`** (boolean, default: `false`): When set to `true`, Probe will search for an existing comment containing the `update_comment_marker` and replace its content instead of creating a new comment.
*   **`update_comment_marker`** (string, default: `"<!-- probe-bot -->")`): A hidden HTML comment marker inserted into the comment body. This serves as a reliable anchor that allows the workflow to locate and update the correct comment on subsequent runs.

#### Benefits

*   **Reduced Clutter**: Instead of accumulating multiple bot comments, you get a single, continuously updated response.
*   **Better UX**: Users see the latest response without scrolling through multiple bot comments.
*   **Cleaner History**: Issue and PR conversations remain focused and readable.

#### Example Usage

```yaml
jobs:
  trigger_probe_chat:
    uses: probelabs/probe/.github/workflows/probe.yml@main
    with:
      command_prefix: "/probe"
      # Enable comment updating
      update_existing_comment: true
      # Use a custom marker to distinguish different workflows
      update_comment_marker: "<!-- my-probe-bot -->"
    secrets:
      ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
```

#### How It Works

1.  **First Run**: Probe creates a new comment with the marker embedded as a hidden HTML comment.
2.  **Subsequent Runs**: Probe searches for existing comments containing the specified marker and updates the first match found.
3.  **Marker Uniqueness**: Use different markers for different workflows (e.g., `<!-- probe-assistant -->` vs `<!-- probe-engineer -->`) to prevent conflicts.

#### Best Practices

*   Use descriptive markers that identify the specific workflow or purpose (e.g., `<!-- probe-code-review -->`, `<!-- probe-qa-bot -->`).
*   Enable this feature for workflows that are likely to be invoked multiple times on the same issue or PR.
*   Consider keeping it disabled for one-off assistance requests where you want to preserve the conversation history.

## Use Cases

*   **Ticket Answering Assistant**: Users ask questions (`/probe How does X work?`), and Probe analyzes code to answer.
*   **Pull Request Reviewer**: Invoke Probe (`/probe Review this change`) to get AI feedback on PRs.
*   **AI Code Implementation**: Request code changes (`/engineer Refactor this function`) and have Probe attempt to implement them (requires `allow_edit: true` and `contents: write`).

This integration streamlines development by bringing code-aware AI assistance and automation directly into your GitHub issues and pull requests.
