//! Mock TypeScript language server (typescript-language-server) with realistic response patterns

use super::server::{MockResponsePattern, MockServerConfig};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Create a mock typescript-language-server configuration
pub fn create_tsserver_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "typescript-language-server".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: Some(35), // TS server is usually quite responsive
        verbose: false,
    };

    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_definition_response(),
            delay_ms: Some(70),
        },
    );

    config.method_patterns.insert(
        "textDocument/references".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_references_response(),
            delay_ms: Some(110),
        },
    );

    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_hover_response(),
            delay_ms: Some(45),
        },
    );

    config.method_patterns.insert(
        "textDocument/documentSymbol".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_document_symbols_response(),
            delay_ms: Some(90),
        },
    );

    config.method_patterns.insert(
        "workspace/symbol".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_workspace_symbols_response(),
            delay_ms: Some(180),
        },
    );

    config.method_patterns.insert(
        "textDocument/completion".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_completion_response(),
            delay_ms: Some(25),
        },
    );

    config.method_patterns.insert(
        "textDocument/implementation".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_implementation_response(),
            delay_ms: Some(85),
        },
    );

    config.method_patterns.insert(
        "textDocument/typeDefinition".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_type_definition_response(),
            delay_ms: Some(65),
        },
    );

    // TypeScript language server has good call hierarchy support
    config.method_patterns.insert(
        "textDocument/prepareCallHierarchy".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_prepare_call_hierarchy_response(),
            delay_ms: Some(100),
        },
    );

    config.method_patterns.insert(
        "callHierarchy/incomingCalls".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_incoming_calls_response(),
            delay_ms: Some(150),
        },
    );

    config.method_patterns.insert(
        "callHierarchy/outgoingCalls".to_string(),
        MockResponsePattern::Success {
            result: create_typescript_outgoing_calls_response(),
            delay_ms: Some(150),
        },
    );

    config
}

fn create_typescript_definition_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/main.ts",
            "range": {
                "start": {"line": 8, "character": 9},
                "end": {"line": 8, "character": 20}
            }
        }
    ])
}

fn create_typescript_references_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/main.ts",
            "range": {
                "start": {"line": 3, "character": 6},
                "end": {"line": 3, "character": 17}
            }
        },
        {
            "uri": "file:///workspace/src/utils.ts",
            "range": {
                "start": {"line": 12, "character": 8},
                "end": {"line": 12, "character": 19}
            }
        },
        {
            "uri": "file:///workspace/src/components/Button.tsx",
            "range": {
                "start": {"line": 25, "character": 14},
                "end": {"line": 25, "character": 25}
            }
        },
        {
            "uri": "file:///workspace/tests/main.test.ts",
            "range": {
                "start": {"line": 7, "character": 18},
                "end": {"line": 7, "character": 29}
            }
        }
    ])
}

fn create_typescript_hover_response() -> Value {
    json!({
        "contents": {
            "kind": "markdown",
            "value": "```typescript\\nfunction myFunction(param: string): Promise<number>\\n```\\n\\n**@param** param - The input string parameter\\n\\n**@returns** A promise that resolves to a number\\n\\nDefined in src/main.ts:8:9"
        },
        "range": {
            "start": {"line": 8, "character": 9},
            "end": {"line": 8, "character": 20}
        }
    })
}

fn create_typescript_document_symbols_response() -> Value {
    json!([
        {
            "name": "MyInterface",
            "kind": 11,
            "range": {
                "start": {"line": 2, "character": 0},
                "end": {"line": 6, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 2, "character": 10},
                "end": {"line": 2, "character": 21}
            },
            "children": [
                {
                    "name": "name",
                    "kind": 7,
                    "range": {
                        "start": {"line": 3, "character": 2},
                        "end": {"line": 3, "character": 14}
                    },
                    "selectionRange": {
                        "start": {"line": 3, "character": 2},
                        "end": {"line": 3, "character": 6}
                    }
                },
                {
                    "name": "value",
                    "kind": 7,
                    "range": {
                        "start": {"line": 4, "character": 2},
                        "end": {"line": 4, "character": 16}
                    },
                    "selectionRange": {
                        "start": {"line": 4, "character": 2},
                        "end": {"line": 4, "character": 7}
                    }
                }
            ]
        },
        {
            "name": "MyClass",
            "kind": 5,
            "range": {
                "start": {"line": 8, "character": 0},
                "end": {"line": 20, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 8, "character": 6},
                "end": {"line": 8, "character": 13}
            },
            "children": [
                {
                    "name": "constructor",
                    "kind": 9,
                    "range": {
                        "start": {"line": 9, "character": 2},
                        "end": {"line": 11, "character": 3}
                    },
                    "selectionRange": {
                        "start": {"line": 9, "character": 2},
                        "end": {"line": 9, "character": 13}
                    }
                },
                {
                    "name": "doSomething",
                    "kind": 6,
                    "range": {
                        "start": {"line": 13, "character": 2},
                        "end": {"line": 17, "character": 3}
                    },
                    "selectionRange": {
                        "start": {"line": 13, "character": 2},
                        "end": {"line": 13, "character": 13}
                    }
                }
            ]
        },
        {
            "name": "helperFunction",
            "kind": 12,
            "range": {
                "start": {"line": 22, "character": 0},
                "end": {"line": 26, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 22, "character": 9},
                "end": {"line": 22, "character": 23}
            }
        }
    ])
}

fn create_typescript_workspace_symbols_response() -> Value {
    json!([
        {
            "name": "MyInterface",
            "kind": 11,
            "location": {
                "uri": "file:///workspace/src/main.ts",
                "range": {
                    "start": {"line": 2, "character": 10},
                    "end": {"line": 2, "character": 21}
                }
            }
        },
        {
            "name": "MyClass",
            "kind": 5,
            "location": {
                "uri": "file:///workspace/src/main.ts",
                "range": {
                    "start": {"line": 8, "character": 6},
                    "end": {"line": 8, "character": 13}
                }
            }
        },
        {
            "name": "Button",
            "kind": 5,
            "location": {
                "uri": "file:///workspace/src/components/Button.tsx",
                "range": {
                    "start": {"line": 5, "character": 6},
                    "end": {"line": 5, "character": 12}
                }
            }
        },
        {
            "name": "ApiService",
            "kind": 5,
            "location": {
                "uri": "file:///workspace/src/services/api.ts",
                "range": {
                    "start": {"line": 10, "character": 6},
                    "end": {"line": 10, "character": 16}
                }
            }
        }
    ])
}

fn create_typescript_completion_response() -> Value {
    json!({
        "isIncomplete": false,
        "items": [
            {
                "label": "console.log",
                "kind": 6,
                "detail": "(method) Console.log(...data: any[]): void",
                "documentation": "Prints to stdout with newline.",
                "insertText": "console.log(${1})",
                "filterText": "console.log"
            },
            {
                "label": "Promise",
                "kind": 7,
                "detail": "interface Promise<T>",
                "documentation": "Represents the completion of an asynchronous operation.",
                "insertText": "Promise<${1}>",
                "filterText": "Promise"
            },
            {
                "label": "Array",
                "kind": 7,
                "detail": "interface Array<T>",
                "documentation": "An array is a JavaScript object that can store multiple values at once.",
                "insertText": "Array<${1}>",
                "filterText": "Array"
            },
            {
                "label": "string",
                "kind": 25,
                "detail": "type string",
                "documentation": "Primitive type for textual data.",
                "insertText": "string"
            },
            {
                "label": "number",
                "kind": 25,
                "detail": "type number",
                "documentation": "Primitive type for numeric data.",
                "insertText": "number"
            }
        ]
    })
}

fn create_typescript_implementation_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/impl/MyClassImpl.ts",
            "range": {
                "start": {"line": 5, "character": 0},
                "end": {"line": 15, "character": 1}
            }
        },
        {
            "uri": "file:///workspace/src/impl/AnotherImpl.ts",
            "range": {
                "start": {"line": 8, "character": 0},
                "end": {"line": 18, "character": 1}
            }
        }
    ])
}

fn create_typescript_type_definition_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/types.ts",
            "range": {
                "start": {"line": 12, "character": 0},
                "end": {"line": 16, "character": 1}
            }
        }
    ])
}

fn create_typescript_prepare_call_hierarchy_response() -> Value {
    json!([
        {
            "name": "doSomething",
            "kind": 6,
            "uri": "file:///workspace/src/main.ts",
            "range": {
                "start": {"line": 13, "character": 2},
                "end": {"line": 17, "character": 3}
            },
            "selectionRange": {
                "start": {"line": 13, "character": 2},
                "end": {"line": 13, "character": 13}
            }
        }
    ])
}

fn create_typescript_incoming_calls_response() -> Value {
    json!([
        {
            "from": {
                "name": "helperFunction",
                "kind": 12,
                "uri": "file:///workspace/src/main.ts",
                "range": {
                    "start": {"line": 22, "character": 0},
                    "end": {"line": 26, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 22, "character": 9},
                    "end": {"line": 22, "character": 23}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 24, "character": 2},
                    "end": {"line": 24, "character": 13}
                }
            ]
        },
        {
            "from": {
                "name": "onClick",
                "kind": 6,
                "uri": "file:///workspace/src/components/Button.tsx",
                "range": {
                    "start": {"line": 10, "character": 2},
                    "end": {"line": 15, "character": 3}
                },
                "selectionRange": {
                    "start": {"line": 10, "character": 2},
                    "end": {"line": 10, "character": 9}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 12, "character": 4},
                    "end": {"line": 12, "character": 15}
                }
            ]
        }
    ])
}

fn create_typescript_outgoing_calls_response() -> Value {
    json!([
        {
            "to": {
                "name": "console.log",
                "kind": 6,
                "uri": "file:///workspace/src/main.ts",
                "range": {
                    "start": {"line": 15, "character": 4},
                    "end": {"line": 15, "character": 26}
                },
                "selectionRange": {
                    "start": {"line": 15, "character": 4},
                    "end": {"line": 15, "character": 15}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 15, "character": 4},
                    "end": {"line": 15, "character": 15}
                }
            ]
        },
        {
            "to": {
                "name": "helperFunction",
                "kind": 12,
                "uri": "file:///workspace/src/main.ts",
                "range": {
                    "start": {"line": 22, "character": 0},
                    "end": {"line": 26, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 22, "character": 9},
                    "end": {"line": 22, "character": 23}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 16, "character": 11},
                    "end": {"line": 16, "character": 25}
                }
            ]
        }
    ])
}

/// Create a typescript-language-server config that simulates project loading delays
pub fn create_loading_tsserver_config() -> MockServerConfig {
    let mut config = create_tsserver_config();
    config.server_name = "typescript-language-server-loading".to_string();
    config.global_delay_ms = Some(500); // Simulate project loading

    // Initial requests are slow while TypeScript loads project
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Sequence {
            patterns: vec![
                MockResponsePattern::Success {
                    result: create_typescript_definition_response(),
                    delay_ms: Some(2000), // First request very slow (project loading)
                },
                MockResponsePattern::Success {
                    result: create_typescript_definition_response(),
                    delay_ms: Some(800), // Second request still slow
                },
                MockResponsePattern::Success {
                    result: create_typescript_definition_response(),
                    delay_ms: Some(70), // Subsequent requests normal speed
                },
            ],
            current_index: 0,
        },
    );

    config
}

/// Create a tsserver config that simulates incomplete/partial responses
pub fn create_incomplete_tsserver_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "typescript-language-server-incomplete".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: Some(35),
        verbose: false,
    };

    // Mix of successful and incomplete responses
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Sequence {
            patterns: vec![
                MockResponsePattern::EmptyArray { delay_ms: Some(70) },
                MockResponsePattern::Success {
                    result: create_typescript_definition_response(),
                    delay_ms: Some(70),
                },
                MockResponsePattern::Null { delay_ms: Some(70) },
            ],
            current_index: 0,
        },
    );

    config.method_patterns.insert(
        "textDocument/references".to_string(),
        MockResponsePattern::EmptyArray {
            delay_ms: Some(110),
        },
    );

    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Null { delay_ms: Some(45) },
    );

    config
}
