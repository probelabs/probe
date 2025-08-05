# Docker Usage Guide for Probe

This guide explains how to build and use the Docker containers for both the Rust-based Probe CLI tool and the Node.js-based probe-chat interface (CLI and web modes).

## Docker Hub Images

Pre-built images are available on Docker Hub:
- **Probe CLI**: `docker pull buger/probe:latest`
- **Probe Chat**: `docker pull buger/probe-chat:latest`

### Available Tags
- `latest` - Latest stable release
- `vX.Y.Z` - Specific version (e.g., `v1.0.0`)
- `main` - Latest development build from main branch

---

## 1. Probe CLI (Rust)

### Using Docker Hub Image
```sh
docker pull buger/probe:latest
```

### Build the Image Locally
```sh
docker build -t probe-app .
```

### Run the CLI
```sh
# Using Docker Hub image
docker run --rm buger/probe --help

# Using locally built image
docker run --rm probe-app --help
```

You can pass any Probe CLI arguments as needed:
```sh
# Using Docker Hub image
docker run --rm -v $(pwd):/workspace buger/probe search "fn main" /workspace

# Using locally built image
docker run --rm -v $(pwd):/workspace probe-app search "fn main" /workspace
```

### Optional
You can alias the Docker command to make the interaction identical to a local installation:
```sh
# Using Docker Hub image
alias probe='docker run --rm -v $(pwd):/workspace buger/probe'

# Using locally built image  
alias probe='docker run --rm -v $(pwd):/workspace probe-app'
```
---

## 2. Probe Chat (Node.js CLI & Web)

### Using Docker Hub Image
```sh
docker pull buger/probe-chat:latest
```

### Build the Image Locally
```sh
docker build -t probe-chat-app -f examples/chat/Dockerfile examples/chat
```

### Environment Variables
- `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`: **Required** for LLM-powered features
- `ALLOWED_FOLDERS`: Optional, restricts which folders can be searched

### Run in CLI Mode
```sh
# Using Docker Hub image
docker run --rm -e ANTHROPIC_API_KEY=your_api_key buger/probe-chat

# Using locally built image
docker run --rm -e ANTHROPIC_API_KEY=your_api_key probe-chat-app
```

### Run in Web Mode
```sh
# Using Docker Hub image
docker run --rm -e ANTHROPIC_API_KEY=your_api_key -p 8080:3000 buger/probe-chat --web

# Using locally built image
docker run --rm -e ANTHROPIC_API_KEY=your_api_key -p 8080:3000 probe-chat-app --web
```


---

## Usage Examples

### Basic Search
```sh
# Search for functions in a mounted directory
docker run --rm -v $(pwd):/app/src probe-app search "function main"

# Search with specific format
docker run --rm -v $(pwd):/app/src probe-app search "class" --format json
```

### Chat Interface
```sh
# CLI chat with mounted codebase
docker run --rm -e ANTHROPIC_API_KEY=your_key -v $(pwd):/app/src probe-chat-app

# Web interface on custom port
docker run --rm -e ANTHROPIC_API_KEY=your_key -p 9000:3000 probe-chat-app --web
```

### Advanced Usage
```sh
# Extract code blocks
docker run --rm -v $(pwd):/app/src probe-app extract "function" --format markdown

# Query with AST patterns
docker run --rm -v $(pwd):/app/src probe-app query "function_declaration"
```

---

## Image Information

### probe-app (Rust CLI)
- **Base Image:** `rust:latest` (build) + `debian:bookworm-slim` (runtime)
- **Size:** ~200MB
- **User:** `probe` (non-root)
- **Ports:** None (CLI only)

### probe-chat-app (Node.js)
- **Base Image:** `node:20.12.2-slim`
- **Size:** ~1GB
- **User:** `probe` (non-root)
- **Ports:** 3000 (web mode)

---

## Docker Compose

For easier local development and testing, use Docker Compose:

### Quick Start with Docker Compose

1. **Create a `.env` file** with your API keys:
```sh
ANTHROPIC_API_KEY=your_api_key_here
# Or use OpenAI:
# OPENAI_API_KEY=your_api_key_here
```

2. **Run services**:
```sh
# Run Probe CLI
docker compose run --rm probe search "function" .

# Run Probe Chat CLI
docker compose run --rm probe-chat-cli

# Run Probe Chat Web (accessible at http://localhost:3000)
docker compose up probe-chat-web
```

3. **Build locally** (for development):
```sh
# Build all services
docker compose build

# Build and run with dev profile
docker compose --profile dev up
```

### Docker Compose Services

- **probe**: Probe CLI tool for code search
- **probe-chat-cli**: Interactive chat interface (CLI mode)
- **probe-chat-web**: Web interface (port 3000)
- **probe-dev**: Development build with cargo cache (dev profile)

---

## Cleanup

Remove images when no longer needed:
```sh
docker rmi probe-app probe-chat-app
```

Remove all unused images:
```sh
docker image prune -a
```

---

## Troubleshooting

### Port Already in Use
If port 3000 is already in use, use a different port:
```sh
docker run --rm -e ANTHROPIC_API_KEY=your_key -p 8080:3000 probe-chat-app --web
```

### Permission Issues
Both containers run as non-root users. If you need to write to mounted volumes, ensure proper permissions:
```sh
# Set ownership for mounted directory
sudo chown -R 1000:1000 /path/to/mounted/directory
```

### API Key Issues
Ensure your API key is properly set:
```sh
# Check if key is set
echo $ANTHROPIC_API_KEY

# Set key if needed
export ANTHROPIC_API_KEY=your_actual_key
```