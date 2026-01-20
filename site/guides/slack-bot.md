# Build a Codebase-Aware Slack Bot with Visor

This guide walks you through creating a Slack bot that can answer questions about your codebase. Your team can ask questions in Slack and get intelligent answers grounded in your actual code.

**Time to complete:** ~10 minutes

## What You'll Build

A Slack bot that:
- Responds to mentions and direct messages
- Uses Probe to search and understand your codebase
- Provides accurate, context-aware answers about your code
- Works in channels, DMs, and group messages

## Prerequisites

- A Slack workspace where you have permission to create apps
- An Anthropic API key (or other LLM provider)
- Visor installed (`npm install -g @probelabs/visor` or via npx)
- Your codebase accessible locally or via git

## Step 1: Create a Slack App

### 1.1 Go to Slack App Creation

1. Visit [api.slack.com/apps](https://api.slack.com/apps)
2. Click **Create New App**
3. Select **From an app manifest**
4. Choose your workspace

### 1.2 Use the App Manifest

Copy and paste this manifest in the JSON tab:

```json
{
    "display_information": {
        "name": "Probe",
        "description": "A friendly bot to answer questions about code",
        "background_color": "#000000"
    },
    "features": {
        "app_home": {
            "home_tab_enabled": true,
            "messages_tab_enabled": false,
            "messages_tab_read_only_enabled": false
        },
        "bot_user": {
            "display_name": "Probe",
            "always_online": true
        }
    },
    "oauth_config": {
        "scopes": {
            "bot": [
                "app_mentions:read",
                "channels:history",
                "chat:write",
                "groups:history",
                "im:history",
                "mpim:history",
                "reactions:read",
                "reactions:write",
                "im:read",
                "im:write"
            ]
        }
    },
    "settings": {
        "event_subscriptions": {
            "bot_events": [
                "app_mention",
                "message.channels",
                "message.groups",
                "message.im",
                "message.mpim"
            ]
        },
        "interactivity": {
            "is_enabled": true
        },
        "org_deploy_enabled": false,
        "socket_mode_enabled": true,
        "token_rotation_enabled": false
    }
}
```

You can customize the `display_name` to whatever you'd like your bot to be called in Slack.

Click **Create** to create the app.

### Understanding the Manifest

| Section | Purpose |
|---------|---------|
| `display_information` | Bot name and appearance in Slack |
| `bot_user` | The bot's display name and online status |
| `oauth_config.scopes` | Permissions the bot needs to read messages and respond |
| `event_subscriptions.bot_events` | Events that trigger the bot (mentions, messages) |
| `socket_mode_enabled` | Enables real-time communication without a public URL |

## Step 2: Get Your Tokens

After creating the app, you need two tokens:

### 2.1 Bot Token (SLACK_BOT_TOKEN)

1. Go to **OAuth & Permissions** in the left sidebar
2. Click **Install to Workspace**
3. Authorize the app
4. Copy the **Bot User OAuth Token** (starts with `xoxb-`)

### 2.2 App Token (SLACK_APP_TOKEN)

1. Go to **Basic Information** in the left sidebar
2. Scroll to **App-Level Tokens**
3. Click **Generate Token and Scopes**
4. Name it `socket-mode` or similar
5. Add the scope `connections:write`
6. Click **Generate**
7. Copy the token (starts with `xapp-`)

## Step 3: Create the Visor Workflow

Create a file called `slack-bot.yaml` in your project:

```yaml
steps:
  ask:
    type: human-input
    prompt: |
      Hi! Ask me something.

  checkout:
    type: git-checkout
    depends_on: [ask]
    ref: "main"
    repository: "probelabs/probe"

  answer:
    type: ai
    depends_on: [ask, checkout]
    prompt: |
      User asked: {{ outputs['ask'].text }}
      Repo path: {{ outputs['checkout'].path }}
      Reply in 1–3 sentences.
```

## Step 4: Set Environment Variables

Create a `.env` file (add to `.gitignore`!):

```bash
# Slack tokens (from Step 2)
SLACK_BOT_TOKEN=xoxb-your-bot-token-here
SLACK_APP_TOKEN=xapp-your-app-token-here

# LLM API key
ANTHROPIC_API_KEY=sk-ant-your-key-here

# Optional: Use a different model provider
# OPENAI_API_KEY=sk-your-key-here
```

Or export them in your shell:

```bash
export SLACK_BOT_TOKEN="xoxb-..."
export SLACK_APP_TOKEN="xapp-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

## Step 5: Run the Bot

Run with npx (no install needed):

```bash
npx -y @probelabs/visor@latest --config slack-bot.yaml --slack
```

You should see:
```
✓ Connected to Slack
✓ Listening for events...
```

### Test It

1. Go to your Slack workspace
2. Invite the bot to a channel: `/invite @Probe`
3. Mention the bot with a question: `@Probe how does authentication work?`

The bot will:
1. Search your codebase using Probe
2. Find relevant code
3. Formulate an answer grounded in what it found
4. Reply in the thread

## Step 6: Deploy to Production

### Self-Host with Docker

```dockerfile
FROM node:20-slim

WORKDIR /app
COPY slack-bot.yaml .

RUN npm install -g @probelabs/visor

CMD ["npx", "-y", "@probelabs/visor@latest", "--config", "slack-bot.yaml", "--slack"]
```

```bash
docker build -t slack-bot .
docker run -d --name slack-bot \
  -e SLACK_BOT_TOKEN=xoxb-... \
  -e SLACK_APP_TOKEN=xapp-... \
  -e ANTHROPIC_API_KEY=sk-ant-... \
  slack-bot
```

### Run on Your Infrastructure

Use systemd, PM2, or your preferred process manager:

```bash
# With PM2
pm2 start "npx -y @probelabs/visor@latest --config slack-bot.yaml --slack" --name slack-bot
```

## Troubleshooting

### Bot Not Responding

1. **Check the bot is running:** Look for "Connected to Slack" in logs
2. **Verify tokens:** Ensure both `SLACK_BOT_TOKEN` and `SLACK_APP_TOKEN` are set
3. **Check permissions:** The bot needs to be invited to channels
4. **Socket mode:** Ensure socket mode is enabled in Slack app settings

### "Not Authorized" Errors

- Reinstall the app to your workspace (OAuth & Permissions > Install to Workspace)
- Verify the bot token starts with `xoxb-`
- Verify the app token starts with `xapp-`

### Bot Responds Slowly

- Probe searches can take a few seconds on large codebases
- Consider narrowing the search paths in `context.paths`
- Use `context.exclude` to skip irrelevant directories

### Empty or Unhelpful Answers

- Check that your codebase paths are correct
- Verify Probe can search your code: `probe search "your query" ./src`
- Add more specific instructions in the system prompt

## Environment Variables Reference

| Variable | Required | Description |
|----------|----------|-------------|
| `SLACK_BOT_TOKEN` | Yes | Bot User OAuth Token (xoxb-...) |
| `SLACK_APP_TOKEN` | Yes | App-Level Token with connections:write (xapp-...) |
| `ANTHROPIC_API_KEY` | Yes* | Anthropic API key for Claude |
| `OPENAI_API_KEY` | Alt* | OpenAI API key (alternative to Anthropic) |

*One LLM provider API key is required.

---

**Questions?** [Join our Discord](https://discord.gg/hBN4UsTZ) or [book a demo](https://cal.com/leonid-bugaev/30min).
