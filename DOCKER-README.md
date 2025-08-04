# Docker Usage Guide for Probe

This guide explains how to build and use the Docker containers for both the Rust-based Probe CLI tool and the Node.js-based probe-chat interface (CLI and web modes).

---

## 1. Probe CLI (Rust)

### Build the Image
```sh
docker build -t probe-app .
```

### Run the CLI
```sh
docker run --rm probe-app --help
```

You can pass any Probe CLI arguments as needed:
```sh
docker run --rm probe-app search "fn main" /app/src
```

---

## 2. Probe Chat (Node.js CLI & Web)

### Build the Image
```sh
docker build -t probe-chat-app -f examples/chat/Dockerfile examples/chat
```

### Environment Variables
- `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`: **Required** for LLM-powered chat
- `ALLOWED_FOLDERS`: Comma-separated list of folders to search (required for web mode)
- `MODEL_NAME`, `DEBUG`, etc.: Optional, see main README for details

### Run in CLI Mode (default)
```sh
docker run --rm -e ANTHROPIC_API_KEY=your_key probe-chat-app
```

You can specify a directory to search:
```sh
docker run --rm -e ANTHROPIC_API_KEY=your_key probe-chat-app /app
```

### Run in Web Mode
By default, the web server runs on port 3000 inside the container.

```sh
docker run --rm -e ANTHROPIC_API_KEY=your_key -e ALLOWED_FOLDERS=/app -p 8080:3000 probe-chat-app --web
```
- Visit [http://localhost:8080](http://localhost:8080) in your browser.
- Change `8080` to any available port on your host if needed.

### Notes
- Replace `your_key` with your actual API key.
- You can use `OPENAI_API_KEY` instead of `ANTHROPIC_API_KEY` if preferred.
- For more options, run:
  ```sh
  docker run --rm probe-chat-app --help
  ```

---

## Cleaning Up
To remove the images:
```sh
docker rmi probe-app probe-chat-app
```