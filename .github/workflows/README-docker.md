# Docker CI/CD Setup

This document describes the Docker CI/CD setup for the Probe project.

## Required Secrets

The following secrets need to be configured in your GitHub repository settings:

1. **`DOCKER_HUB_TOKEN`** - Docker Hub access token for pushing images
   - Create at: https://hub.docker.com/settings/security
   - Required permissions: Read, Write, Delete

## Optional Variables

The following variables can be configured in repository settings:

1. **`DOCKER_HUB_USERNAME`** - Docker Hub username (defaults to 'buger')

## Workflow Integration

### release.yml
The Docker build and publish process is integrated into the main release workflow:
- Triggers on version tags (v*)
- Builds multi-platform images (linux/amd64, linux/arm64)
- Publishes versioned images to Docker Hub
- Updates Docker Hub descriptions
- Tags: `X.Y.Z` and `latest`

The `publish-docker-images` job runs after the binary releases are complete, ensuring all release artifacts are available.

## Image Naming

- Probe CLI: `buger/probe`
- Probe Chat: `buger/probe-chat`

## Testing Locally

```bash
# Test the full release workflow (including Docker builds)
act -j publish-docker-images --secret DOCKER_HUB_TOKEN=your_token -e <(echo '{"ref": "refs/tags/v1.0.0"}')

# Test Docker builds locally
docker build -t probe-test .
docker build -t probe-chat-test -f examples/chat/Dockerfile examples/chat

# Test multi-platform builds locally
docker buildx build --platform linux/amd64,linux/arm64 -t probe-test .
```