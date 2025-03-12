# Web Interface

Probe includes a web-based chat interface that provides a user-friendly way to interact with your codebase using AI. The web interface offers a modern UI for code search and AI-powered code exploration.

## Features

### Interactive Chat UI

The web interface provides a clean, modern chat interface with:

- Markdown rendering for rich text formatting
- Syntax highlighting for code blocks
- Support for multiple programming languages
- Persistent conversation history

### AI-Powered Code Search

The web interface uses Claude AI to:

- Search your codebase based on natural language queries
- Explain code functionality and architecture
- Generate diagrams and visualizations
- Provide context-aware responses

### Mermaid Diagram Support

The web interface can render visual diagrams for:

- Code architecture
- Data flows
- Class hierarchies
- Sequence diagrams
- State machines

### Configurable Search Paths

You can define which directories can be searched via environment variables, allowing you to:

- Limit search to specific projects
- Include multiple codebases
- Exclude sensitive directories
- Control access to different parts of your filesystem

## Setup and Configuration

### Prerequisites

- Node.js (v14 or later)
- NPM (v6 or later)
- An Anthropic API key for Claude

### Installation

1. **Navigate to the web directory**:
   ```bash
   cd web
   ```

2. **Install dependencies**:
   ```bash
   npm install
   ```

3. **Configure environment variables**:
   Create or edit the `.env` file in the web directory:
   ```
   ANTHROPIC_API_KEY=your_anthropic_api_key
   PORT=8080
   ALLOWED_FOLDERS=/path/to/folder1,/path/to/folder2
   ```

4. **Start the server**:
   ```bash
   npm start
   ```

5. **Access the web interface**:
   Open your browser and navigate to `http://localhost:8080`

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `ANTHROPIC_API_KEY` | Your Anthropic API key for Claude | (Required) |
| `PORT` | The port to run the server on | 8080 |
| `ALLOWED_FOLDERS` | Comma-separated list of folders that can be searched | Current directory |
| `MODEL_NAME` | The Claude model to use | claude-3-opus-20240229 |
| `MAX_TOKENS` | Maximum tokens for code search results | 40000 |

## Using the Web Interface

### Starting a Conversation

1. Open your browser and navigate to `http://localhost:8080`
2. Type your question or request in the input field
3. Press Enter or click the Send button

### Example Queries

- "Explain the architecture of this project"
- "How does the search functionality work?"
- "Create a diagram showing the main components"
- "Find all code related to authentication"
- "Explain the ranking algorithm implementation"

### Viewing Code

Code blocks are displayed with syntax highlighting for better readability. You can:

- Copy code blocks with the copy button
- Expand/collapse long code blocks
- See the file path for each code block

### Viewing Diagrams

When you ask for a diagram, the AI will generate a Mermaid diagram that is rendered directly in the chat. You can:

- Zoom in/out of diagrams
- Copy the diagram as an image
- Copy the Mermaid source code

## Technical Details

### Architecture

The web interface consists of:

- A Node.js backend server
- A vanilla JavaScript frontend
- The Vercel AI SDK for Claude integration
- Probe command execution via the probeTool.js module

### Technologies Used

- **Marked.js**: For rendering markdown
- **Highlight.js**: For syntax highlighting
- **Mermaid.js**: For diagram generation and visualization
- **Vercel AI SDK**: For streaming AI responses
- **Express.js**: For the backend server

### Security Considerations

- The web interface only allows searching in directories specified in the `ALLOWED_FOLDERS` environment variable
- API keys are kept server-side and never exposed to the client
- User inputs are sanitized to prevent command injection
- The server runs locally by default and is not exposed to the internet

## Customization

### Styling

You can customize the appearance of the web interface by editing the CSS in the `web/index.html` file.

### Adding Features

The web interface is built with vanilla JavaScript, making it easy to modify and extend. Common customizations include:

- Adding authentication
- Implementing user preferences
- Adding support for additional AI models
- Creating custom visualizations

### Deployment

While the web interface is designed to run locally, you can deploy it to a server:

1. Build a Docker image using the provided Dockerfile
2. Deploy the image to your preferred hosting platform
3. Set the required environment variables
4. Ensure proper security measures are in place

For more information on deployment options, see the `web/README.md` file.