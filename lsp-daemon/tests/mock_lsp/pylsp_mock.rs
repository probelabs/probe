//! Mock Python LSP server (pylsp) with realistic response patterns

use super::server::{MockResponsePattern, MockServerConfig};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Create a mock pylsp server configuration
pub fn create_pylsp_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "pylsp".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: Some(30), // pylsp is typically faster than rust-analyzer
        verbose: false,
    };

    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Success {
            result: create_python_definition_response(),
            delay_ms: Some(80),
        },
    );

    config.method_patterns.insert(
        "textDocument/references".to_string(),
        MockResponsePattern::Success {
            result: create_python_references_response(),
            delay_ms: Some(120),
        },
    );

    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Success {
            result: create_python_hover_response(),
            delay_ms: Some(60),
        },
    );

    config.method_patterns.insert(
        "textDocument/documentSymbol".to_string(),
        MockResponsePattern::Success {
            result: create_python_document_symbols_response(),
            delay_ms: Some(100),
        },
    );

    config.method_patterns.insert(
        "workspace/symbol".to_string(),
        MockResponsePattern::Success {
            result: create_python_workspace_symbols_response(),
            delay_ms: Some(200),
        },
    );

    config.method_patterns.insert(
        "textDocument/completion".to_string(),
        MockResponsePattern::Success {
            result: create_python_completion_response(),
            delay_ms: Some(40),
        },
    );

    // pylsp doesn't support call hierarchy
    config.method_patterns.insert(
        "textDocument/prepareCallHierarchy".to_string(),
        MockResponsePattern::Error {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
            delay_ms: Some(10),
        },
    );

    config.method_patterns.insert(
        "callHierarchy/incomingCalls".to_string(),
        MockResponsePattern::Error {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
            delay_ms: Some(10),
        },
    );

    config.method_patterns.insert(
        "callHierarchy/outgoingCalls".to_string(),
        MockResponsePattern::Error {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
            delay_ms: Some(10),
        },
    );

    config
}

fn create_python_definition_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/main.py",
            "range": {
                "start": {"line": 15, "character": 4},
                "end": {"line": 15, "character": 16}
            }
        }
    ])
}

fn create_python_references_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/main.py",
            "range": {
                "start": {"line": 8, "character": 12},
                "end": {"line": 8, "character": 24}
            }
        },
        {
            "uri": "file:///workspace/src/utils.py",
            "range": {
                "start": {"line": 22, "character": 8},
                "end": {"line": 22, "character": 20}
            }
        },
        {
            "uri": "file:///workspace/tests/test_main.py",
            "range": {
                "start": {"line": 5, "character": 16},
                "end": {"line": 5, "character": 28}
            }
        }
    ])
}

fn create_python_hover_response() -> Value {
    json!({
        "contents": {
            "kind": "markdown",
            "value": "```python\\ndef my_function(param: str) -> int:\\n    pass\\n```\\n\\nA sample Python function that takes a string parameter and returns an integer."
        },
        "range": {
            "start": {"line": 15, "character": 4},
            "end": {"line": 15, "character": 16}
        }
    })
}

fn create_python_document_symbols_response() -> Value {
    json!([
        {
            "name": "MyClass",
            "kind": 5,
            "range": {
                "start": {"line": 5, "character": 0},
                "end": {"line": 20, "character": 0}
            },
            "selectionRange": {
                "start": {"line": 5, "character": 6},
                "end": {"line": 5, "character": 13}
            },
            "children": [
                {
                    "name": "__init__",
                    "kind": 6,
                    "range": {
                        "start": {"line": 6, "character": 4},
                        "end": {"line": 9, "character": 0}
                    },
                    "selectionRange": {
                        "start": {"line": 6, "character": 8},
                        "end": {"line": 6, "character": 16}
                    }
                },
                {
                    "name": "my_method",
                    "kind": 6,
                    "range": {
                        "start": {"line": 10, "character": 4},
                        "end": {"line": 15, "character": 0}
                    },
                    "selectionRange": {
                        "start": {"line": 10, "character": 8},
                        "end": {"line": 10, "character": 17}
                    }
                }
            ]
        },
        {
            "name": "standalone_function",
            "kind": 12,
            "range": {
                "start": {"line": 22, "character": 0},
                "end": {"line": 25, "character": 0}
            },
            "selectionRange": {
                "start": {"line": 22, "character": 4},
                "end": {"line": 22, "character": 23}
            }
        }
    ])
}

fn create_python_workspace_symbols_response() -> Value {
    json!([
        {
            "name": "MyClass",
            "kind": 5,
            "location": {
                "uri": "file:///workspace/src/main.py",
                "range": {
                    "start": {"line": 5, "character": 6},
                    "end": {"line": 5, "character": 13}
                }
            }
        },
        {
            "name": "standalone_function",
            "kind": 12,
            "location": {
                "uri": "file:///workspace/src/main.py",
                "range": {
                    "start": {"line": 22, "character": 4},
                    "end": {"line": 22, "character": 23}
                }
            }
        },
        {
            "name": "UtilityClass",
            "kind": 5,
            "location": {
                "uri": "file:///workspace/src/utils.py",
                "range": {
                    "start": {"line": 10, "character": 6},
                    "end": {"line": 10, "character": 18}
                }
            }
        }
    ])
}

fn create_python_completion_response() -> Value {
    json!({
        "isIncomplete": false,
        "items": [
            {
                "label": "print",
                "kind": 3,
                "detail": "builtin function",
                "documentation": "Print objects to the text stream file.",
                "insertText": "print(${1})"
            },
            {
                "label": "len",
                "kind": 3,
                "detail": "builtin function",
                "documentation": "Return the length of an object.",
                "insertText": "len(${1})"
            },
            {
                "label": "str",
                "kind": 7,
                "detail": "builtin class",
                "documentation": "Create a new string object from the given encoding.",
                "insertText": "str"
            },
            {
                "label": "list",
                "kind": 7,
                "detail": "builtin class",
                "documentation": "Built-in mutable sequence type.",
                "insertText": "list"
            }
        ]
    })
}

/// Create a pylsp config with limited capabilities (simulates older version)
pub fn create_limited_pylsp_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "pylsp-limited".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: Some(50),
        verbose: false,
    };

    // Only basic methods supported
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Success {
            result: create_python_definition_response(),
            delay_ms: Some(100),
        },
    );

    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Success {
            result: create_python_hover_response(),
            delay_ms: Some(80),
        },
    );

    // Other methods not supported
    let not_supported = MockResponsePattern::Error {
        code: -32601,
        message: "Method not found".to_string(),
        data: None,
        delay_ms: Some(10),
    };

    config
        .method_patterns
        .insert("textDocument/references".to_string(), not_supported.clone());
    config.method_patterns.insert(
        "textDocument/documentSymbol".to_string(),
        not_supported.clone(),
    );
    config
        .method_patterns
        .insert("workspace/symbol".to_string(), not_supported.clone());
    config
        .method_patterns
        .insert("textDocument/completion".to_string(), not_supported);

    config
}
