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

## Workflows

### docker.yml
- Triggers on:
  - Push to main branch
  - Pull requests affecting Docker files
  - Version tags (v*)
- Builds multi-platform images (linux/amd64, linux/arm64)
- Pushes to Docker Hub on main branch and tags
- Updates Docker Hub descriptions

### release.yml
- Enhanced to include Docker publishing
- Publishes versioned images on releases
- Tags: `vX.Y.Z` and `latest`

## Image Naming

- Probe CLI: `buger/probe`
- Probe Chat: `buger/probe-chat`

## Testing Locally

```bash
# Test the Docker build workflow
act -j docker-build-probe --secret DOCKER_HUB_TOKEN=your_token

# Test with dry run (no push)
act -j docker-build-probe --env GITHUB_EVENT_NAME=pull_request
```