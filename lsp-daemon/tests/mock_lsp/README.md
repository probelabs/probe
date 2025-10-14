# MockLspServer Infrastructure

This directory contains a comprehensive mock LSP server infrastructure for testing LSP daemon integration. The mock server can simulate different language servers (rust-analyzer, pylsp, gopls, typescript-language-server) with configurable response patterns.

## Overview

The MockLspServer infrastructure provides:

1. **Realistic LSP Protocol Simulation**: Full JSON-RPC over stdio communication
2. **Configurable Response Patterns**: Success, empty arrays, null, errors, timeouts, and sequences
3. **Language Server Specific Mocks**: Pre-built configurations for popular language servers
4. **Integration Testing Support**: Test harness for validating LSP daemon behavior

## Architecture

```
mock_lsp/
├── mod.rs                    # Module declarations and public API
├── protocol.rs               # LSP JSON-RPC protocol definitions
├── server.rs                 # Core MockLspServer implementation
├── rust_analyzer_mock.rs     # Rust analyzer simulation
├── pylsp_mock.rs            # Python LSP server simulation
├── gopls_mock.rs            # Go language server simulation
└── tsserver_mock.rs         # TypeScript language server simulation
```

## Core Components

### MockLspServer

The main server class that handles:
- JSON-RPC message parsing and generation
- Configurable response patterns
- Delay simulation for realistic timing
- Process management for stdio communication

### MockResponsePattern

Enum defining different response behaviors:

```rust
pub enum MockResponsePattern {
    Success { result: Value, delay_ms: Option<u64> },
    EmptyArray { delay_ms: Option<u64> },
    Null { delay_ms: Option<u64> },
    Error { code: i32, message: String, data: Option<Value>, delay_ms: Option<u64> },
    Timeout,
    Sequence { patterns: Vec<MockResponsePattern>, current_index: usize },
}
```

### MockServerConfig

Configuration structure for customizing mock server behavior:

```rust
pub struct MockServerConfig {
    pub server_name: String,
    pub method_patterns: HashMap<String, MockResponsePattern>,
    pub global_delay_ms: Option<u64>,
    pub verbose: bool,
}
```

## Language Server Mocks

### Rust Analyzer Mock (`rust_analyzer_mock.rs`)

**Features:**
- Realistic response times (50-200ms)
- Full call hierarchy support
- Comprehensive document symbols
- Rich hover information with markdown
- Multiple reference locations

**Available Configurations:**
- `create_rust_analyzer_config()` - Standard configuration
- `create_empty_rust_analyzer_config()` - Returns empty responses
- `create_slow_rust_analyzer_config()` - Simulates slow responses (2-5s)
- `create_error_rust_analyzer_config()` - Simulates various error conditions

### Python LSP Mock (`pylsp_mock.rs`)

**Features:**
- Fast response times (30-120ms)
- No call hierarchy support (returns method not found errors)
- Python-specific symbols and completions
- Multiple file references

**Available Configurations:**
- `create_pylsp_config()` - Standard configuration  
- `create_limited_pylsp_config()` - Simulates older version with limited features

### Go LSP Mock (`gopls_mock.rs`)

**Features:**
- Fast response times (40-180ms)
- Full method support including call hierarchy
- Go-specific symbols and types
- Implementation and type definition support

**Available Configurations:**
- `create_gopls_config()` - Standard configuration
- `create_slow_gopls_config()` - Simulates module loading delays

### TypeScript Server Mock (`tsserver_mock.rs`)

**Features:**
- Very fast response times (25-180ms)
- Full call hierarchy support
- Rich TypeScript/JavaScript symbols
- Interface and implementation support

**Available Configurations:**
- `create_tsserver_config()` - Standard configuration
- `create_loading_tsserver_config()` - Simulates project loading delays
- `create_incomplete_tsserver_config()` - Mixed success/failure responses

## Usage Examples

### Basic Usage

```rust
use mock_lsp::server::{MockLspServer, MockServerConfig};
use mock_lsp::rust_analyzer_mock;

// Create a rust-analyzer mock
let config = rust_analyzer_mock::create_rust_analyzer_config();
let mut server = MockLspServer::new(config);

// Start the server (spawns subprocess)
server.start().await?;

// Send requests
let request = LspRequest {
    jsonrpc: "2.0".to_string(),
    id: Some(json!(1)),
    method: "textDocument/definition".to_string(),
    params: Some(json!({
        "textDocument": {"uri": "file:///test.rs"},
        "position": {"line": 10, "character": 5}
    })),
};

let response = server.send_request(request).await?;

// Clean up
server.stop().await?;
```

### Custom Response Patterns

```rust
let mut config = MockServerConfig {
    server_name: "custom-server".to_string(),
    method_patterns: HashMap::new(),
    global_delay_ms: Some(100),
    verbose: true,
};

// Custom success response
config.method_patterns.insert(
    "textDocument/definition".to_string(),
    MockResponsePattern::Success {
        result: json!([{
            "uri": "file:///custom.rs",
            "range": {"start": {"line": 42, "character": 0}, "end": {"line": 42, "character": 10}}
        }]),
        delay_ms: Some(200),
    },
);

// Error response
config.method_patterns.insert(
    "textDocument/references".to_string(),
    MockResponsePattern::Error {
        code: -32603,
        message: "Internal error".to_string(),
        data: Some(json!({"details": "Custom error"})),
        delay_ms: Some(50),
    },
);

// Timeout simulation
config.method_patterns.insert(
    "textDocument/hover".to_string(),
    MockResponsePattern::Timeout,
);
```

### Sequence Testing

```rust
// Test retry logic with sequence of responses
config.method_patterns.insert(
    "textDocument/definition".to_string(),
    MockResponsePattern::Sequence {
        patterns: vec![
            MockResponsePattern::Error { code: -32603, message: "First attempt fails".to_string(), data: None, delay_ms: Some(100) },
            MockResponsePattern::EmptyArray { delay_ms: Some(50) },
            MockResponsePattern::Success { result: json!([{"uri": "file:///success.rs", "range": {...}}]), delay_ms: Some(75) },
        ],
        current_index: 0,
    },
);
```

## Testing Integration

The mock servers are designed to work seamlessly with the LSP daemon's testing infrastructure:

```rust
#[tokio::test]
async fn test_lsp_daemon_with_mock_rust_analyzer() -> Result<()> {
    // Start mock server
    let config = rust_analyzer_mock::create_rust_analyzer_config();
    let mut mock_server = MockLspServer::new(config);
    mock_server.start().await?;
    
    // Configure LSP daemon to use mock server
    let mut daemon = LspDaemon::new_for_testing(&mock_server.socket_path()).await?;
    
    // Test LSP operations
    let definition_result = daemon.get_definition("file:///test.rs", 10, 5).await?;
    assert!(!definition_result.is_empty());
    
    // Cleanup
    mock_server.stop().await?;
    Ok(())
}
```

## Response Data Structure

All mock responses follow the LSP specification format:

### Definition Response
```json
[{
    "uri": "file:///workspace/src/main.rs",
    "range": {
        "start": {"line": 10, "character": 4},
        "end": {"line": 10, "character": 12}
    }
}]
```

### Call Hierarchy Response
```json
{
    "item": {
        "name": "function_name",
        "kind": 12,
        "uri": "file:///workspace/src/main.rs",
        "range": {...},
        "selectionRange": {...}
    },
    "incoming": [...],
    "outgoing": [...]
}
```

### Error Response
```json
{
    "code": -32603,
    "message": "Internal error",
    "data": {"details": "Additional error information"}
}
```

## Validation

Use the provided validation script to ensure proper implementation:

```bash
python3 validate_mock_server.py
```

The validation script checks:
- File structure and existence
- Basic Rust syntax
- Required protocol definitions
- Response pattern completeness
- Language-specific mock configurations
- Test coverage

## Performance Characteristics

The mock servers simulate realistic response times based on actual language server behavior:

| Server | Typical Range | Notes |
|--------|---------------|-------|
| rust-analyzer | 50-200ms | Slower for complex operations |
| pylsp | 30-120ms | Generally faster |
| gopls | 40-180ms | Variable based on module loading |
| tsserver | 25-180ms | Very responsive for basic operations |

## Integration with LSP Daemon Tests

The mock infrastructure supports various testing scenarios:

1. **Normal Operation Testing**: Validate expected request/response flows
2. **Error Handling Testing**: Simulate various error conditions  
3. **Timeout Testing**: Validate timeout handling and recovery
4. **Performance Testing**: Measure daemon performance with predictable response times
5. **Sequence Testing**: Test retry logic and state management

## Extending the Mock Infrastructure

To add support for a new language server:

1. Create a new file `new_language_mock.rs`
2. Implement configuration functions following the existing patterns
3. Add response creation functions for common LSP methods
4. Add the new mock to the module exports in `mod.rs`
5. Update tests to include the new mock
6. Run validation script to ensure completeness

## Troubleshooting

### Common Issues

1. **Mock server not responding**: Check that `start()` was called and succeeded
2. **Unexpected responses**: Verify method patterns are configured correctly
3. **Compilation errors**: Ensure all dependencies are properly imported
4. **Test failures**: Check that expected response formats match test assertions

### Debug Mode

Enable verbose logging for debugging:

```rust
let config = MockServerConfig {
    verbose: true,
    // ... other configuration
};
```

This will print all requests and responses to stderr.

## Future Enhancements

Potential improvements to the mock infrastructure:

1. **Real subprocess implementation**: Currently uses simplified in-process simulation
2. **Dynamic pattern modification**: Allow changing patterns during runtime  
3. **Request validation**: Validate that incoming requests match LSP specification
4. **Statistics collection**: Track request counts and timing information
5. **Configuration persistence**: Save/load configurations from files
6. **Interactive mode**: Allow manual control of responses during testing

## Contributing

When contributing to the mock server infrastructure:

1. Follow existing naming conventions
2. Add comprehensive test coverage
3. Update documentation for new features
4. Run validation script before submitting changes
5. Ensure compatibility with existing tests