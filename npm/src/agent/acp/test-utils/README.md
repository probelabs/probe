# ACP Test Utilities

This directory contains comprehensive testing tools for Agent Client Protocol (ACP) implementations. Since the official ACP specification doesn't provide dedicated test tooling, we've created our own utilities to ensure protocol compliance and facilitate development.

## 🧪 Test Tools Overview

### 1. ACP Tester (`acp-tester.js`)
**Protocol compliance testing tool**

A comprehensive test suite that validates ACP implementations against the protocol specification.

**Features:**
- Protocol initialization and capability testing
- Session management verification
- Error handling validation
- Performance benchmarking
- JSON-RPC 2.0 compliance checking

**Usage:**
```bash
node acp-tester.js --agent-command "node my-agent.js --acp"
node acp-tester.js --agent-command "python agent.py" --timeout 60 --verbose
```

### 2. Mock ACP Client (`mock-acp-client.js`)
**Interactive client simulator**

Simulates a code editor or IDE communicating with ACP agents, useful for manual testing and development.

**Features:**
- Interactive chat sessions
- Session mode management
- Tool call progress monitoring
- Notification handling
- Demo conversation flows

**Usage:**
```bash
node mock-acp-client.js --agent "node agent.js --acp"
node mock-acp-client.js --agent "my-agent-binary" --interactive
```

## 🔧 Test Suite Details

### ACP Tester Test Cases

#### 1. Protocol Initialize
- Validates JSON-RPC 2.0 format
- Checks protocol version negotiation
- Verifies server information structure
- Confirms capability advertisement

#### 2. Session Management
- Tests session creation with unique IDs
- Validates session loading and persistence
- Checks session metadata (timestamps, history)

#### 3. Error Handling
- Unknown method rejection (-32601)
- Invalid parameter handling (-32602)
- Session not found errors
- Timeout behavior

#### 4. Performance Benchmark
- Measures response times
- Tests concurrent requests
- Evaluates throughput
- Reports performance metrics

### Mock Client Features

#### Interactive Mode
```bash
> hello agent
> mode planning
> find all functions in this project
> status
> quit
```

#### Demo Mode
Runs predefined conversation flow:
1. Agent introduction
2. File listing request
3. Code search queries
4. Entry point analysis

## 📊 Example Test Output

### ACP Tester Results
```
📋 ACP Protocol Compliance Test Suite
📋 ========================================
✅ Protocol Initialize (9ms)
✅ Session Management (4ms)
✅ Error Handling (1ms)
✅ Performance Benchmark (2ms)

📋 Test Results Summary
📋 ====================
📋 Total time: 1024ms
📋 Passed: 4
📋 Failed: 0
📋 Total: 4
✅ Agent is ACP compliant! 🎉
```

### Mock Client Session
```
[14:30:15] 🚀 Starting agent: node agent.js --acp
[14:30:16] ✅ Connected to my-agent v1.0.0
[14:30:16] 🛠️  Available tools: search, query, extract
[14:30:16] 📝 Creating new session...
[14:30:16] ✅ Session created: abc-123-def
[14:30:17] 💬 Sending: "Hello! Can you tell me about yourself?"
[14:30:18] 🤖 Agent response:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
I'm an AI agent that helps with code analysis and search...
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

## 🎯 Testing Your ACP Implementation

### Step 1: Basic Compliance
```bash
node acp-tester.js --agent-command "your-agent-command"
```

This will run the core compliance tests and report any issues.

### Step 2: Interactive Testing
```bash
node mock-acp-client.js --agent "your-agent-command" --interactive
```

Test your agent manually with realistic interactions.

### Step 3: Performance Testing
```bash
node acp-tester.js --agent-command "your-agent-command" --verbose
```

Get detailed performance metrics and timing information.

## 🏗️ Test Development

### Adding New Tests to ACP Tester

```javascript
async testCustomFeature() {
  return this.runTest('Custom Feature Test', async () => {
    // Your test logic here
    const result = await this.sendRequest('customMethod', { param: 'value' });
    
    // Validate result
    if (!result.expected) {
      throw new Error('Custom feature not working');
    }
    
    return { customMetric: result.value };
  });
}
```

### Extending Mock Client

```javascript
handleCustomNotification(notification) {
  if (notification.method === 'customEvent') {
    this.log(`🎯 Custom event: ${notification.params.data}`);
  }
}
```

## 🛠️ Utilities API

### ACPTester Class

**Methods:**
- `runAllTests()` - Execute complete test suite
- `runTest(name, testFn)` - Run individual test
- `sendRequest(method, params)` - Send JSON-RPC request
- `validateInitializeResponse(result)` - Validate init response

**Options:**
- `agentCommand` - Command to start agent
- `timeout` - Request timeout (ms)
- `verbose` - Enable detailed output

### MockACPClient Class

**Methods:**
- `initialize()` - Initialize ACP connection
- `createSession()` - Create new session
- `sendPrompt(message)` - Send user message
- `setSessionMode(mode)` - Change session mode
- `runInteractiveSession()` - Start interactive mode

**Options:**
- `agentCommand` - Command to start agent
- `interactive` - Enable interactive mode
- `debug` - Show debug information
- `timeout` - Request timeout (ms)

## 🎨 Customization

### Custom Test Configuration

```javascript
const tester = new ACPTester({
  agentCommand: 'python my_agent.py',
  timeout: 60000, // 60 seconds
  verbose: true
});

// Add custom validation
tester.validateCustomResponse = (result) => {
  // Your custom validation logic
};

await tester.runAllTests();
```

### Client Behavior Customization

```javascript
const client = new MockACPClient({
  agentCommand: './my-agent-binary',
  interactive: false,
  debug: true
});

// Custom prompt handling
client.handleCustomPrompt = async (prompt) => {
  // Your custom logic
};

await client.run();
```

## 🚀 Integration with CI/CD

### GitHub Actions Example

```yaml
name: ACP Compliance Test
on: [push, pull_request]

jobs:
  test-acp:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      
      - name: Build agent
        run: npm install && npm run build
      
      - name: Test ACP compliance
        run: |
          node test-utils/acp-tester.js \
            --agent-command "node dist/agent.js --acp" \
            --timeout 30
```

### Docker Testing

```dockerfile
FROM node:18-alpine

COPY . /app
WORKDIR /app

RUN npm install
RUN npm run build

# Test ACP compliance
RUN node test-utils/acp-tester.js \
    --agent-command "node dist/agent.js --acp"
```

## 📚 Reference Implementations

The test utilities work with these ACP implementations:

**Official Examples:**
- Zed Industries TypeScript agent/client examples
- Reference Rust implementations

**Tested Implementations:**
- Probe Agent (this repository)
- Custom Python agents
- Go-based agents

## 🤝 Contributing Test Tools

To add new test utilities:

1. Create new test file in this directory
2. Follow the established patterns (ACPTester/MockACPClient)
3. Add comprehensive documentation
4. Include usage examples
5. Test against multiple implementations

### Test Tool Guidelines

- **Comprehensive**: Cover all protocol aspects
- **Robust**: Handle edge cases and errors gracefully
- **Informative**: Provide clear success/failure messages
- **Configurable**: Allow customization for different needs
- **Well-documented**: Include usage examples and API docs

These test utilities ensure ACP implementations are reliable, compliant, and ready for production use with code editors and development tools.