//! Mock rust-analyzer server with realistic response patterns
//!
//! This module provides mock responses that simulate rust-analyzer behavior
//! for various LSP methods like definition, references, hover, etc.

use super::server::{MockResponsePattern, MockServerConfig};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Create a mock rust-analyzer server configuration
pub fn create_rust_analyzer_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "rust-analyzer".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: Some(50), // Simulate typical rust-analyzer response time
        verbose: false,
    };

    // Add realistic response patterns for common LSP methods
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Success {
            result: create_definition_response(),
            delay_ms: Some(100),
        },
    );

    config.method_patterns.insert(
        "textDocument/references".to_string(),
        MockResponsePattern::Success {
            result: create_references_response(),
            delay_ms: Some(150),
        },
    );

    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Success {
            result: create_hover_response(),
            delay_ms: Some(75),
        },
    );

    config.method_patterns.insert(
        "textDocument/documentSymbol".to_string(),
        MockResponsePattern::Success {
            result: create_document_symbols_response(),
            delay_ms: Some(200),
        },
    );

    config.method_patterns.insert(
        "workspace/symbol".to_string(),
        MockResponsePattern::Success {
            result: create_workspace_symbols_response(),
            delay_ms: Some(300),
        },
    );

    config.method_patterns.insert(
        "textDocument/prepareCallHierarchy".to_string(),
        MockResponsePattern::Success {
            result: create_prepare_call_hierarchy_response(),
            delay_ms: Some(100),
        },
    );

    config.method_patterns.insert(
        "callHierarchy/incomingCalls".to_string(),
        MockResponsePattern::Success {
            result: create_incoming_calls_response(),
            delay_ms: Some(200),
        },
    );

    config.method_patterns.insert(
        "callHierarchy/outgoingCalls".to_string(),
        MockResponsePattern::Success {
            result: create_outgoing_calls_response(),
            delay_ms: Some(200),
        },
    );

    config.method_patterns.insert(
        "textDocument/completion".to_string(),
        MockResponsePattern::Success {
            result: create_completion_response(),
            delay_ms: Some(50),
        },
    );

    // Add patterns that simulate empty responses (common in real usage)
    config.method_patterns.insert(
        "textDocument/implementation".to_string(),
        MockResponsePattern::EmptyArray {
            delay_ms: Some(100),
        },
    );

    config.method_patterns.insert(
        "textDocument/typeDefinition".to_string(),
        MockResponsePattern::Success {
            result: create_type_definition_response(),
            delay_ms: Some(120),
        },
    );

    config
}

/// Create a mock rust-analyzer config that returns empty responses (for testing edge cases)
pub fn create_empty_rust_analyzer_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "rust-analyzer-empty".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: Some(10),
        verbose: false,
    };

    // All methods return empty arrays
    let empty_pattern = MockResponsePattern::EmptyArray { delay_ms: None };

    config
        .method_patterns
        .insert("textDocument/definition".to_string(), empty_pattern.clone());
    config
        .method_patterns
        .insert("textDocument/references".to_string(), empty_pattern.clone());
    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Null { delay_ms: None },
    );
    config.method_patterns.insert(
        "textDocument/documentSymbol".to_string(),
        empty_pattern.clone(),
    );
    config
        .method_patterns
        .insert("workspace/symbol".to_string(), empty_pattern.clone());
    config.method_patterns.insert(
        "textDocument/prepareCallHierarchy".to_string(),
        empty_pattern.clone(),
    );
    config.method_patterns.insert(
        "callHierarchy/incomingCalls".to_string(),
        empty_pattern.clone(),
    );
    config.method_patterns.insert(
        "callHierarchy/outgoingCalls".to_string(),
        empty_pattern.clone(),
    );

    config
}

fn create_definition_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/main.rs",
            "range": {
                "start": {"line": 10, "character": 4},
                "end": {"line": 10, "character": 12}
            }
        }
    ])
}

fn create_references_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/main.rs",
            "range": {
                "start": {"line": 5, "character": 8},
                "end": {"line": 5, "character": 16}
            }
        },
        {
            "uri": "file:///workspace/src/lib.rs",
            "range": {
                "start": {"line": 42, "character": 12},
                "end": {"line": 42, "character": 20}
            }
        }
    ])
}

fn create_hover_response() -> Value {
    json!({
        "contents": {
            "kind": "markdown",
            "value": "```rust\\nfn main()\\n```\\n\\nThe main function is the entry point of the program."
        },
        "range": {
            "start": {"line": 0, "character": 3},
            "end": {"line": 0, "character": 7}
        }
    })
}

fn create_document_symbols_response() -> Value {
    json!([
        {
            "name": "main",
            "kind": 12,
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": 10, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 0, "character": 3},
                "end": {"line": 0, "character": 7}
            },
            "children": []
        },
        {
            "name": "helper_function",
            "kind": 12,
            "range": {
                "start": {"line": 12, "character": 0},
                "end": {"line": 15, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 12, "character": 3},
                "end": {"line": 12, "character": 18}
            },
            "children": []
        }
    ])
}

fn create_workspace_symbols_response() -> Value {
    json!([
        {
            "name": "main",
            "kind": 12,
            "location": {
                "uri": "file:///workspace/src/main.rs",
                "range": {
                    "start": {"line": 0, "character": 3},
                    "end": {"line": 0, "character": 7}
                }
            }
        },
        {
            "name": "MyStruct",
            "kind": 5,
            "location": {
                "uri": "file:///workspace/src/lib.rs",
                "range": {
                    "start": {"line": 10, "character": 0},
                    "end": {"line": 15, "character": 1}
                }
            }
        }
    ])
}

fn create_prepare_call_hierarchy_response() -> Value {
    json!([
        {
            "name": "main",
            "kind": 12,
            "uri": "file:///workspace/src/main.rs",
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": 10, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 0, "character": 3},
                "end": {"line": 0, "character": 7}
            }
        }
    ])
}

fn create_incoming_calls_response() -> Value {
    json!([
        {
            "from": {
                "name": "caller_function",
                "kind": 12,
                "uri": "file:///workspace/src/lib.rs",
                "range": {
                    "start": {"line": 20, "character": 0},
                    "end": {"line": 25, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 20, "character": 3},
                    "end": {"line": 20, "character": 18}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 22, "character": 4},
                    "end": {"line": 22, "character": 8}
                }
            ]
        }
    ])
}

fn create_outgoing_calls_response() -> Value {
    json!([
        {
            "to": {
                "name": "println!",
                "kind": 12,
                "uri": "file:///workspace/src/main.rs",
                "range": {
                    "start": {"line": 2, "character": 4},
                    "end": {"line": 2, "character": 32}
                },
                "selectionRange": {
                    "start": {"line": 2, "character": 4},
                    "end": {"line": 2, "character": 12}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 2, "character": 4},
                    "end": {"line": 2, "character": 12}
                }
            ]
        },
        {
            "to": {
                "name": "helper_function",
                "kind": 12,
                "uri": "file:///workspace/src/main.rs",
                "range": {
                    "start": {"line": 12, "character": 0},
                    "end": {"line": 15, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 12, "character": 3},
                    "end": {"line": 12, "character": 18}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 5, "character": 4},
                    "end": {"line": 5, "character": 19}
                }
            ]
        }
    ])
}

fn create_completion_response() -> Value {
    json!({
        "isIncomplete": false,
        "items": [
            {
                "label": "println!",
                "kind": 3,
                "detail": "macro",
                "documentation": "Prints to the standard output, with a newline.",
                "insertText": "println!(\"${1}\")"
            },
            {
                "label": "String",
                "kind": 7,
                "detail": "struct",
                "documentation": "A UTF-8 encoded, growable string.",
                "insertText": "String"
            }
        ]
    })
}

fn create_type_definition_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/types.rs",
            "range": {
                "start": {"line": 5, "character": 0},
                "end": {"line": 8, "character": 1}
            }
        }
    ])
}

/// Create a rust-analyzer config that simulates slow responses
pub fn create_slow_rust_analyzer_config() -> MockServerConfig {
    let mut config = create_rust_analyzer_config();
    config.server_name = "rust-analyzer-slow".to_string();
    config.global_delay_ms = Some(2000); // 2 second delay

    // Make some specific methods even slower
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Success {
            result: create_definition_response(),
            delay_ms: Some(5000), // 5 second delay
        },
    );

    config
}

/// Create a rust-analyzer config that simulates errors
pub fn create_error_rust_analyzer_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "rust-analyzer-error".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: None,
        verbose: false,
    };

    // Return errors for most methods
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Error {
            code: -32603,
            message: "Internal error: analysis failed".to_string(),
            data: Some(json!({"details": "Mock error for testing"})),
            delay_ms: Some(100),
        },
    );

    config.method_patterns.insert(
        "textDocument/references".to_string(),
        MockResponsePattern::Error {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
            delay_ms: Some(50),
        },
    );

    // Some methods timeout
    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Timeout,
    );

    config
}
