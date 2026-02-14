# Web Interface

Browser-based chat interface for AI-powered code exploration.

---

## TL;DR

```bash
# Start web server
probe-chat --web ./my-project

# With authentication
AUTH_ENABLED=1 AUTH_USERNAME=admin AUTH_PASSWORD=secret probe-chat --web

# Custom port
probe-chat --web --port 3000 ./src
```

---

## Starting the Server

### Basic Usage

```bash
# Default port (8080)
probe-chat --web ./project

# Custom port
probe-chat --web --port 3000 ./project

# With debug logging
probe-chat --web --debug ./project
```

### Environment Variables

```bash
PORT=8080                    # Server port
AUTH_ENABLED=1               # Enable basic auth
AUTH_USERNAME=admin          # Username
AUTH_PASSWORD=password       # Password
DEBUG_CHAT=1                 # Debug logging
ALLOWED_FOLDERS=/path1,/path2  # Search paths
```

---

## Authentication

### Basic HTTP Auth

```bash
export AUTH_ENABLED=1
export AUTH_USERNAME=admin
export AUTH_PASSWORD=secure-password

probe-chat --web ./project
```

### Protected Routes

Most routes require authentication:
- `POST /chat` - Send messages
- `POST /api/search` - Code search
- `POST /api/query` - AST patterns
- `POST /api/extract` - Code extraction
- `GET /api/sessions` - Session list

### Public Routes

- `GET /` - Main UI
- `GET /chat/:sessionId` - Session pages
- `GET /api/tool-events` - SSE stream
- `GET /api/session/:id/history` - History

---

## Features

### Chat Interface

- Real-time message display
- Markdown rendering with syntax highlighting
- Image upload support (multiple per message)
- Auto-expanding input textarea
- Message copy functionality

### Session Management

- Session history dropdown
- Automatic session ID generation
- Session restoration from history
- Clear history (new session)
- Activity-based filtering

### Token Usage

Real-time display of:
- Request/response tokens
- Cache read/write metrics
- Context window size
- Total accumulated tokens

### Tool Call Display

- Tool name with timestamp
- Arguments (formatted JSON)
- Results/output
- Expandable/collapsible cards

### API Key Management

In-browser setup for:
- Anthropic Claude
- OpenAI GPT
- Google Gemini

Custom API URL support per provider.

### Visualization

- Mermaid diagram rendering
- Fullscreen zoom capability
- Syntax highlighting (Highlight.js)
- Code block rendering

---

## API Endpoints

### Chat

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/chat` | Send message |
| GET | `/chat/:sessionId` | Session page |
| POST | `/cancel-request` | Cancel request |

**POST /chat**

```javascript
{
  message: "How does auth work?",
  sessionId: "uuid",
  images: ["data:image/png;base64,..."],
  apiProvider: "anthropic",
  apiKey: "sk-ant-...",
  apiUrl: "https://api.anthropic.com",
  clearHistory: false
}
```

---

### Search Tools

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/search` | Semantic search |
| POST | `/api/query` | AST patterns |
| POST | `/api/extract` | Code extraction |
| POST | `/api/implement` | Code editing |

**POST /api/search**

```javascript
{
  query: "authentication",
  path: "./src",
  allow_tests: false,
  maxResults: 10,
  maxTokens: 5000,
  sessionId: "uuid"
}
```

---

### Sessions

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/sessions` | List sessions |
| GET | `/api/session/:id/history` | Get history |

**GET /api/sessions Response**

```javascript
{
  sessions: [
    {
      sessionId: "uuid",
      preview: "How does auth...",
      messageCount: 5,
      createdAt: "2025-01-01T10:00:00Z",
      lastActivity: "2025-01-01T10:30:00Z",
      relativeTime: "30 minutes ago"
    }
  ],
  total: 10,
  timestamp: "2025-01-01T11:00:00Z"
}
```

---

### Other

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/token-usage` | Token stats |
| GET | `/api/tool-events` | SSE stream |
| GET | `/folders` | Allowed folders |
| GET | `/openapi.yaml` | API spec |

---

## Server-Sent Events

Real-time tool call updates via SSE.

### Connection

```javascript
const eventSource = new EventSource(
  `/api/tool-events?sessionId=${sessionId}`
);
```

### Events

**connection**
```javascript
eventSource.addEventListener('connection', (event) => {
  const data = JSON.parse(event.data);
  // { type: 'connection', sessionId, timestamp }
});
```

**toolCall**
```javascript
eventSource.addEventListener('toolCall', (event) => {
  const data = JSON.parse(event.data);
  // { name, status, args, result, timestamp }
});
```

### Tool Call Status

| Status | Description |
|--------|-------------|
| `started` | Tool execution began |
| `completed` | Tool finished |

---

## Frontend Stack

### Libraries (CDN)

- **Marked.js** - Markdown rendering
- **Highlight.js** - Syntax highlighting
- **Mermaid.js** - Diagrams

### No Build Required

The frontend is vanilla HTML/CSS/JavaScript:
- No framework dependencies
- Client-side state management
- Fetch API for HTTP
- EventSource for SSE

---

## Session Storage

Sessions persist in `~/.probe/sessions/`:

```
~/.probe/
└── sessions/
    ├── {session-1}.json
    ├── {session-2}.json
    └── ...
```

### Auto-Cleanup

Sessions older than 2 hours hidden from list.

### Storage Stats

```javascript
GET /api/sessions

// Response includes storage stats
{
  sessions: [...],
  total: 25
}
```

---

## Configuration

### Full Example

```bash
export ANTHROPIC_API_KEY=sk-ant-...
export PORT=3000
export AUTH_ENABLED=1
export AUTH_USERNAME=admin
export AUTH_PASSWORD=secure-password
export DEBUG_CHAT=1

probe-chat \
  --web \
  --port 3000 \
  --allow-edit \
  --enable-bash \
  --max-iterations 50 \
  ./my-project
```

### Code Editing

Enable with `--allow-edit`:

```bash
probe-chat --web --allow-edit ./project
```

Returns 403 if not enabled.

### Bash Execution

Enable with `--enable-bash`:

```bash
probe-chat --web --enable-bash ./project
```

---

## CORS Support

All endpoints support CORS:

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, OPTIONS
Access-Control-Allow-Headers: Content-Type, Authorization
```

---

## Error Handling

### HTTP Status Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 400 | Bad Request |
| 401 | Unauthorized |
| 403 | Forbidden (feature disabled) |
| 404 | Not Found |
| 499 | Client Closed |
| 500 | Server Error |

### Error Response

```javascript
{
  error: "Error message",
  status: 500,
  tokenUsage: { ... },
  sessionId: "uuid",
  timestamp: "ISO8601"
}
```

---

## Tracing

### Enable Tracing

```bash
# File tracing
probe-chat --web --trace-file ./traces.jsonl

# Remote tracing
probe-chat --web --trace-remote http://localhost:4318/v1/traces

# Console tracing
probe-chat --web --trace-console
```

### Traced Information

- AI model requests/responses
- Token usage metrics
- Tool call information
- Session information
- Error details

---

## Security

### Best Practices

1. **Enable authentication** in production
2. **Use HTTPS** (via reverse proxy)
3. **Restrict `--allow-edit`** to trusted environments
4. **Limit `--enable-bash`** carefully
5. **Set `ALLOWED_FOLDERS`** to restrict search scope

### Reverse Proxy

```nginx
server {
    listen 443 ssl;
    server_name probe.example.com;

    location / {
        proxy_pass http://localhost:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
    }
}
```

---

## Related Documentation

- [CLI Usage](./cli-usage.md) - Terminal interface
- [Configuration](./configuration.md) - All config options
- [Storage Adapters](../sdk/storage-adapters.md) - Session persistence

