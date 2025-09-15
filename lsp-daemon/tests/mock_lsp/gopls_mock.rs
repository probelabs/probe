//! Mock Go language server (gopls) with realistic response patterns

use super::server::{MockResponsePattern, MockServerConfig};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Create a mock gopls server configuration
pub fn create_gopls_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "gopls".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: Some(40), // gopls is typically quite fast
        verbose: false,
    };

    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Success {
            result: create_go_definition_response(),
            delay_ms: Some(60),
        },
    );

    config.method_patterns.insert(
        "textDocument/references".to_string(),
        MockResponsePattern::Success {
            result: create_go_references_response(),
            delay_ms: Some(100),
        },
    );

    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Success {
            result: create_go_hover_response(),
            delay_ms: Some(50),
        },
    );

    config.method_patterns.insert(
        "textDocument/documentSymbol".to_string(),
        MockResponsePattern::Success {
            result: create_go_document_symbols_response(),
            delay_ms: Some(80),
        },
    );

    config.method_patterns.insert(
        "workspace/symbol".to_string(),
        MockResponsePattern::Success {
            result: create_go_workspace_symbols_response(),
            delay_ms: Some(150),
        },
    );

    config.method_patterns.insert(
        "textDocument/completion".to_string(),
        MockResponsePattern::Success {
            result: create_go_completion_response(),
            delay_ms: Some(30),
        },
    );

    config.method_patterns.insert(
        "textDocument/implementation".to_string(),
        MockResponsePattern::Success {
            result: create_go_implementation_response(),
            delay_ms: Some(90),
        },
    );

    config.method_patterns.insert(
        "textDocument/typeDefinition".to_string(),
        MockResponsePattern::Success {
            result: create_go_type_definition_response(),
            delay_ms: Some(70),
        },
    );

    // gopls has limited call hierarchy support
    config.method_patterns.insert(
        "textDocument/prepareCallHierarchy".to_string(),
        MockResponsePattern::Success {
            result: create_go_prepare_call_hierarchy_response(),
            delay_ms: Some(120),
        },
    );

    config.method_patterns.insert(
        "callHierarchy/incomingCalls".to_string(),
        MockResponsePattern::Success {
            result: create_go_incoming_calls_response(),
            delay_ms: Some(180),
        },
    );

    config.method_patterns.insert(
        "callHierarchy/outgoingCalls".to_string(),
        MockResponsePattern::Success {
            result: create_go_outgoing_calls_response(),
            delay_ms: Some(180),
        },
    );

    config
}

fn create_go_definition_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/main.go",
            "range": {
                "start": {"line": 12, "character": 5},
                "end": {"line": 12, "character": 17}
            }
        }
    ])
}

fn create_go_references_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/main.go",
            "range": {
                "start": {"line": 8, "character": 10},
                "end": {"line": 8, "character": 22}
            }
        },
        {
            "uri": "file:///workspace/utils/helper.go",
            "range": {
                "start": {"line": 15, "character": 8},
                "end": {"line": 15, "character": 20}
            }
        },
        {
            "uri": "file:///workspace/cmd/server/main.go",
            "range": {
                "start": {"line": 25, "character": 12},
                "end": {"line": 25, "character": 24}
            }
        }
    ])
}

fn create_go_hover_response() -> Value {
    json!({
        "contents": {
            "kind": "markdown",
            "value": "```go\\nfunc MyFunction(param string) int\\n```\\n\\nMyFunction does something useful with the given parameter and returns an integer result.\\n\\nDefined in package main at main.go:12:5"
        },
        "range": {
            "start": {"line": 12, "character": 5},
            "end": {"line": 12, "character": 17}
        }
    })
}

fn create_go_document_symbols_response() -> Value {
    json!([
        {
            "name": "main",
            "kind": 12,
            "range": {
                "start": {"line": 5, "character": 0},
                "end": {"line": 10, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 5, "character": 5},
                "end": {"line": 5, "character": 9}
            }
        },
        {
            "name": "MyStruct",
            "kind": 23,
            "range": {
                "start": {"line": 12, "character": 0},
                "end": {"line": 16, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 12, "character": 5},
                "end": {"line": 12, "character": 13}
            },
            "children": [
                {
                    "name": "Name",
                    "kind": 8,
                    "range": {
                        "start": {"line": 13, "character": 1},
                        "end": {"line": 13, "character": 12}
                    },
                    "selectionRange": {
                        "start": {"line": 13, "character": 1},
                        "end": {"line": 13, "character": 5}
                    }
                },
                {
                    "name": "Value",
                    "kind": 8,
                    "range": {
                        "start": {"line": 14, "character": 1},
                        "end": {"line": 14, "character": 10}
                    },
                    "selectionRange": {
                        "start": {"line": 14, "character": 1},
                        "end": {"line": 14, "character": 6}
                    }
                }
            ]
        },
        {
            "name": "DoSomething",
            "kind": 12,
            "range": {
                "start": {"line": 18, "character": 0},
                "end": {"line": 22, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 18, "character": 5},
                "end": {"line": 18, "character": 16}
            }
        }
    ])
}

fn create_go_workspace_symbols_response() -> Value {
    json!([
        {
            "name": "main",
            "kind": 12,
            "location": {
                "uri": "file:///workspace/main.go",
                "range": {
                    "start": {"line": 5, "character": 5},
                    "end": {"line": 5, "character": 9}
                }
            }
        },
        {
            "name": "MyStruct",
            "kind": 23,
            "location": {
                "uri": "file:///workspace/main.go",
                "range": {
                    "start": {"line": 12, "character": 5},
                    "end": {"line": 12, "character": 13}
                }
            }
        },
        {
            "name": "HttpServer",
            "kind": 23,
            "location": {
                "uri": "file:///workspace/server/server.go",
                "range": {
                    "start": {"line": 8, "character": 5},
                    "end": {"line": 8, "character": 15}
                }
            }
        },
        {
            "name": "Start",
            "kind": 6,
            "location": {
                "uri": "file:///workspace/server/server.go",
                "range": {
                    "start": {"line": 15, "character": 18},
                    "end": {"line": 15, "character": 23}
                }
            }
        }
    ])
}

fn create_go_completion_response() -> Value {
    json!({
        "isIncomplete": false,
        "items": [
            {
                "label": "fmt.Println",
                "kind": 3,
                "detail": "func(a ...interface{}) (n int, err error)",
                "documentation": "Println formats using the default formats for its operands and writes to standard output.",
                "insertText": "fmt.Println(${1})"
            },
            {
                "label": "make",
                "kind": 3,
                "detail": "func(Type, ...IntegerType) Type",
                "documentation": "Built-in function make allocates and initializes an object of type slice, map, or chan.",
                "insertText": "make(${1})"
            },
            {
                "label": "len",
                "kind": 3,
                "detail": "func(v Type) int",
                "documentation": "Built-in function len returns the length of v.",
                "insertText": "len(${1})"
            },
            {
                "label": "string",
                "kind": 25,
                "detail": "type string",
                "documentation": "string is the set of all strings of 8-bit bytes.",
                "insertText": "string"
            }
        ]
    })
}

fn create_go_implementation_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/impl.go",
            "range": {
                "start": {"line": 20, "character": 0},
                "end": {"line": 25, "character": 1}
            }
        },
        {
            "uri": "file:///workspace/impl2.go",
            "range": {
                "start": {"line": 10, "character": 0},
                "end": {"line": 15, "character": 1}
            }
        }
    ])
}

fn create_go_type_definition_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/types.go",
            "range": {
                "start": {"line": 8, "character": 5},
                "end": {"line": 12, "character": 1}
            }
        }
    ])
}

fn create_go_prepare_call_hierarchy_response() -> Value {
    json!([
        {
            "name": "DoSomething",
            "kind": 12,
            "uri": "file:///workspace/main.go",
            "range": {
                "start": {"line": 18, "character": 0},
                "end": {"line": 22, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 18, "character": 5},
                "end": {"line": 18, "character": 16}
            }
        }
    ])
}

fn create_go_incoming_calls_response() -> Value {
    json!([
        {
            "from": {
                "name": "main",
                "kind": 12,
                "uri": "file:///workspace/main.go",
                "range": {
                    "start": {"line": 5, "character": 0},
                    "end": {"line": 10, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 5, "character": 5},
                    "end": {"line": 5, "character": 9}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 8, "character": 1},
                    "end": {"line": 8, "character": 12}
                }
            ]
        }
    ])
}

fn create_go_outgoing_calls_response() -> Value {
    json!([
        {
            "to": {
                "name": "fmt.Println",
                "kind": 12,
                "uri": "file:///workspace/main.go",
                "range": {
                    "start": {"line": 20, "character": 1},
                    "end": {"line": 20, "character": 23}
                },
                "selectionRange": {
                    "start": {"line": 20, "character": 1},
                    "end": {"line": 20, "character": 12}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 20, "character": 1},
                    "end": {"line": 20, "character": 12}
                }
            ]
        }
    ])
}

/// Create a gopls config that simulates module loading delays
pub fn create_slow_gopls_config() -> MockServerConfig {
    let mut config = create_gopls_config();
    config.server_name = "gopls-slow".to_string();
    config.global_delay_ms = Some(1000); // Simulate slow module loading

    // First few requests are very slow (module loading)
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Sequence {
            patterns: vec![
                MockResponsePattern::Success {
                    result: create_go_definition_response(),
                    delay_ms: Some(3000), // First request very slow
                },
                MockResponsePattern::Success {
                    result: create_go_definition_response(),
                    delay_ms: Some(500), // Second request medium slow
                },
                MockResponsePattern::Success {
                    result: create_go_definition_response(),
                    delay_ms: Some(60), // Subsequent requests fast
                },
            ],
            current_index: 0,
        },
    );

    config
}
