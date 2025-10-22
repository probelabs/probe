# Local Image Support in Probe Agent

The probe agent now supports reading local image files directly from file paths mentioned in user messages and **automatically loads images when mentioned during the agentic loop**.

## Features Added

### Automatic Local File Detection
- Detects local image file paths in user messages
- Supports both relative and absolute paths  
- Recognizes common image extensions: `.png`, `.jpg`, `.jpeg`, `.webp`, `.bmp`, `.svg`

### 🚀 NEW: Agentic Loop Image Loading
- **Automatic detection**: Agent automatically detects when it mentions image files in its internal thinking
- **Smart loading**: Images are loaded and added to the AI context for subsequent iterations
- **Persistent context**: Loaded images remain available throughout the conversation
- **Tool result processing**: Images mentioned in tool outputs are also automatically loaded
- **Caching**: Prevents reloading the same images multiple times

### Security Features
- Path validation to prevent directory traversal attacks
- Restricts file access to allowed directories (respects `ALLOWED_FOLDERS` environment variable)
- Validates file existence and readability before processing

### Supported Path Formats
```
./image.png                    # Relative path from current directory
../assets/screenshot.jpg       # Relative path with directory traversal
/absolute/path/to/image.webp   # Absolute path
image.png                      # File in current directory
```

### Automatic Conversion
- Local files are automatically converted to base64 data URLs
- Maintains original MIME type based on file extension
- Seamlessly integrates with existing URL and base64 image support

## Usage Examples

### Basic Usage
```javascript
import { ProbeChat } from './probeChat.js';

const chat = new ProbeChat({ debug: true });

// The agent will automatically detect and process the local image
const response = await chat.chat('Analyze this screenshot: ./screenshot.png');
```

### Mixed Content
```javascript
// Mix local files with URLs
const message = `
  Compare this local image ./local.png 
  with this remote image https://example.com/remote.jpg
`;

const response = await chat.chat(message);
```

### Direct Function Usage
```javascript
import { extractImageUrls } from './probeChat.js';

const message = 'Please review this diagram: ./architecture.png';
const result = await extractImageUrls(message, true);

console.log(`Found ${result.urls.length} images`);
console.log(`Cleaned message: "${result.cleanedMessage}"`);
```

## 🤖 Agentic Loop Integration

The most powerful feature is automatic image loading during the agent's internal reasoning process.

### How It Works

When the probe agent is working through a task, it can now:

1. **Mention an image file** in its reasoning: "I need to check ./screenshot.png"
2. **Automatically load the image** before the next AI iteration
3. **Use visual context** for enhanced analysis and problem-solving

### Agentic Flow Example

```
👤 USER: "Analyze the system architecture"

🤖 AGENT: "Let me search for architecture documentation..."
   🔍 Tool: search "architecture design"
   📊 Result: "Found ./docs/system-diagram.png"
   
🤖 AGENT: "I found a system diagram at ./docs/system-diagram.png. Let me analyze it."
   🖼️  AUTO: Image ./docs/system-diagram.png loaded into context
   
🤖 AGENT: "Based on the diagram I can see..." 
   💭 AI now has visual access to the diagram and can analyze it
```

### Trigger Patterns

The agent automatically loads images when it mentions:

- **Direct paths**: `./screenshot.png`, `/path/to/image.jpg`
- **Contextual references**: "the file diagram.png shows", "looking at chart.png"  
- **Tool results**: When tools return paths to image files
- **Generated content**: "saved visualization as ./output.png"

### Benefits

- **🧠 Enhanced reasoning**: Agent gains visual understanding of referenced images
- **🔄 Seamless workflow**: No manual image loading required
- **⚡ Performance**: Intelligent caching prevents reloading
- **🔒 Security**: Same security validations as manual loading
- **📱 Persistence**: Images remain available throughout the conversation

## Security Considerations

### Path Restrictions
- Files must be within the allowed directory structure
- Prevents access to system files (e.g., `/etc/passwd`)
- Respects the `ALLOWED_FOLDERS` environment variable

### File Validation
- Verifies file existence before attempting to read
- Validates file extensions against supported image formats
- Handles file reading errors gracefully

### Error Handling
- Failed file reads are logged but don't interrupt processing
- Invalid paths are silently ignored
- Maintains functionality for valid images even if some fail

## Implementation Details

### Pattern Matching
The system uses an enhanced regex pattern to detect:
```javascript
/(?:data:image\/[a-zA-Z]*;base64,[A-Za-z0-9+/=]+|https?:\/\/(?:(?:private-user-images\.githubusercontent\.com|github\.com\/user-attachments\/assets)\/[^\s"'<>]+|[^\s"'<>]+\.(?:png|jpg|jpeg|webp|bmp|svg)(?:\?[^\s"'<>]*)?)|(?:\.?\.?\/)?[^\s"'<>]*\.(?:png|jpg|jpeg|webp|bmp|svg))/gi
```

### Processing Pipeline
1. **Pattern Detection** - Find all potential image references in text
2. **Classification** - Distinguish between URLs, base64 data, and local paths
3. **Validation** - Verify local file paths for security and existence
4. **Conversion** - Read local files and convert to base64 data URLs
5. **Integration** - Pass processed images to AI models

### File Size Limitations
- No explicit file size limits implemented
- Memory usage scales with image size
- Large images may impact performance

## Testing

Run the test suite to verify functionality:
```bash
cd examples/chat
node test-local-image-reading.js
```

The test covers:
- Basic local file detection and conversion
- Mixed URL and local file processing
- Relative path handling
- Security validation
- Error handling for missing files

## Backward Compatibility

This enhancement is fully backward compatible:
- Existing URL-based image handling unchanged
- Base64 data URL support maintained
- No breaking changes to existing APIs

## Environment Configuration

Set allowed folders to restrict file access:
```bash
export ALLOWED_FOLDERS="/path/to/project,/path/to/assets"
```

If no `ALLOWED_FOLDERS` is set, defaults to current working directory.

## Error Handling

The system gracefully handles various error conditions:
- **File not found**: Logged and ignored
- **Permission denied**: Logged and ignored  
- **Invalid format**: Logged and ignored
- **Path traversal attempts**: Blocked by security validation

Enable debug mode to see detailed logging:
```javascript
const chat = new ProbeChat({ debug: true });
```