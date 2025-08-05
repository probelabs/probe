---
title: Docker Integration
description: Deploy and run Probe using Docker containers for both CLI and web interface usage.
---

# Docker Integration

Probe provides Docker images for both the CLI tool and the web interface, making it easy to deploy and use in containerized environments.

## Docker Hub Images

Pre-built images are available on Docker Hub:
- **Probe CLI**: `buger/probe:latest`
- **Probe Chat**: `buger/probe-chat:latest`

### Available Tags
- `latest` - Latest stable release
- `X.Y.Z` - Specific version (e.g., `1.0.0`)
- `X.Y` - Minor version (e.g., `1.0`)
- `X` - Major version (e.g., `1`)

## Quick Start

### Probe CLI

```bash
# Pull the image
docker pull buger/probe:latest

# Basic usage
docker run --rm -v $(pwd):/workspace buger/probe search "function" /workspace

# Create an alias for convenience
alias probe='docker run --rm -v $(pwd):/workspace buger/probe'
probe search "class" .
```

### Probe Chat Interface

#### CLI Mode
```bash
# Interactive chat with your codebase
docker run --rm -it \
  -e ANTHROPIC_API_KEY=your_api_key \
  -v $(pwd):/workspace \
  buger/probe-chat
```

#### Web Mode
```bash
# Run web interface on port 3000
docker run --rm \
  -e ANTHROPIC_API_KEY=your_api_key \
  -v $(pwd):/workspace \
  -p 3000:3000 \
  buger/probe-chat --web
```

## Docker Compose

For easier local development and testing, use the included Docker Compose configuration:

### Setup

1. **Create a `.env` file** with your API keys:
```bash
ANTHROPIC_API_KEY=your_api_key_here
# Or use OpenAI:
# OPENAI_API_KEY=your_api_key_here
```

2. **Run services**:
```bash
# Run Probe CLI
docker compose run --rm probe search "function" .

# Run Probe Chat CLI
docker compose run --rm probe-chat-cli

# Run Probe Chat Web (accessible at http://localhost:3000)
docker compose up probe-chat-web
```

### Available Services

- **probe**: Probe CLI tool for code search
- **probe-chat-cli**: Interactive chat interface (CLI mode)
- **probe-chat-web**: Web interface (port 3000)
- **probe-dev**: Development build with cargo cache (dev profile)

## Environment Variables

### Probe Chat
- `ANTHROPIC_API_KEY` - Anthropic API key for Claude models
- `OPENAI_API_KEY` - OpenAI API key for GPT models
- `ALLOWED_FOLDERS` - Optional, restricts which folders can be searched

## Use Cases

### CI/CD Integration

```yaml
# Example GitHub Actions usage
- name: Analyze Code Structure
  run: |
    docker run --rm -v ${{ github.workspace }}:/workspace \
      buger/probe search "TODO|FIXME" /workspace --format json > analysis.json
```

### Development Teams

```bash
# Team development environment
docker run --rm -it \
  -e ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY \
  -v $(pwd):/workspace \
  -p 3000:3000 \
  buger/probe-chat --web
```

### Code Review Automation

```bash
# Automated code review
docker run --rm \
  -v $(pwd):/workspace \
  buger/probe extract "security|auth" /workspace --format markdown
```

## Multi-Platform Support

Probe Docker images support multiple architectures:
- `linux/amd64` - x86_64 Linux systems
- `linux/arm64` - ARM64 Linux systems (Apple Silicon, ARM servers)

Docker will automatically pull the correct architecture for your system.

## Building Locally

If you prefer to build the images locally:

```bash
# Build Probe CLI
docker build -t probe-local .

# Build Probe Chat
docker build -t probe-chat-local -f examples/chat/Dockerfile examples/chat

# Build with Docker Compose
docker compose build
```

## Health Checks

Both images include health checks:
- **Probe CLI**: Tests `probe --version`
- **Probe Chat**: Tests application endpoint (web mode) or Node.js version (CLI mode)

## Security Features

- **Non-root execution**: Both containers run as non-root user `probe`
- **Minimal base images**: Based on `debian:bookworm-slim` and `node:slim`
- **Security labels**: OCI-compliant labels for metadata
- **Read-only volumes**: Mount code directories as read-only when possible

## Troubleshooting

### Port Already in Use
If port 3000 is already in use:
```bash
docker run --rm -e ANTHROPIC_API_KEY=your_key -p 8080:3000 buger/probe-chat --web
```

### Permission Issues
If you need to write to mounted volumes:
```bash
# Set ownership for mounted directory
sudo chown -R 1000:1000 /path/to/mounted/directory
```

### API Key Issues
Ensure your API key is properly set:
```bash
# Check if key is set
echo $ANTHROPIC_API_KEY

# Set key if needed
export ANTHROPIC_API_KEY=your_actual_key
```

## Advanced Configuration

### Custom Dockerfile

For advanced use cases, you can extend the base images:

```dockerfile
FROM buger/probe-chat:latest

# Add custom configurations
COPY custom-config.json /app/config/
ENV CUSTOM_CONFIG_PATH=/app/config/custom-config.json

# Add additional tools
USER root
RUN apt-get update && apt-get install -y git
USER probe

ENTRYPOINT ["node", "index.js"]
```

### Production Deployment

For production deployments, consider:
- Using specific version tags instead of `latest`
- Setting resource limits
- Implementing proper logging
- Using secrets management for API keys
- Setting up monitoring and health checks

```yaml
# docker-compose.prod.yml
version: '3.8'
services:
  probe-chat:
    image: buger/probe-chat:1.0.0
    environment:
      - ANTHROPIC_API_KEY_FILE=/run/secrets/anthropic_api_key
    secrets:
      - anthropic_api_key
    deploy:
      resources:
        limits:
          memory: 1G
          cpus: '0.5'
    restart: unless-stopped

secrets:
  anthropic_api_key:
    external: true
```

## Related Documentation

- [Installation Guide](../installation.md)
- [Quick Start](../quick-start.md)
- [GitHub Actions Integration](./github-actions.md)
- [Web Interface](../web-interface.md)