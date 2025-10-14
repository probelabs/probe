//! Mock phpactor server with realistic response patterns for PHP development
//!
//! This module provides mock responses that simulate phpactor behavior
//! for various LSP methods like definition, references, hover, etc.

use super::server::{MockResponsePattern, MockServerConfig};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Create a mock phpactor server configuration
pub fn create_phpactor_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "phpactor".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: Some(40), // phpactor typically has moderate response time
        verbose: false,
    };

    // Add realistic response patterns for common LSP methods
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Success {
            result: create_php_definition_response(),
            delay_ms: Some(90),
        },
    );

    config.method_patterns.insert(
        "textDocument/references".to_string(),
        MockResponsePattern::Success {
            result: create_php_references_response(),
            delay_ms: Some(130),
        },
    );

    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Success {
            result: create_php_hover_response(),
            delay_ms: Some(70),
        },
    );

    config.method_patterns.insert(
        "textDocument/documentSymbol".to_string(),
        MockResponsePattern::Success {
            result: create_php_document_symbols_response(),
            delay_ms: Some(110),
        },
    );

    config.method_patterns.insert(
        "workspace/symbol".to_string(),
        MockResponsePattern::Success {
            result: create_php_workspace_symbols_response(),
            delay_ms: Some(250),
        },
    );

    config.method_patterns.insert(
        "textDocument/prepareCallHierarchy".to_string(),
        MockResponsePattern::Success {
            result: create_php_prepare_call_hierarchy_response(),
            delay_ms: Some(100),
        },
    );

    config.method_patterns.insert(
        "callHierarchy/incomingCalls".to_string(),
        MockResponsePattern::Success {
            result: create_php_incoming_calls_response(),
            delay_ms: Some(180),
        },
    );

    config.method_patterns.insert(
        "callHierarchy/outgoingCalls".to_string(),
        MockResponsePattern::Success {
            result: create_php_outgoing_calls_response(),
            delay_ms: Some(180),
        },
    );

    config.method_patterns.insert(
        "textDocument/completion".to_string(),
        MockResponsePattern::Success {
            result: create_php_completion_response(),
            delay_ms: Some(55),
        },
    );

    // phpactor supports implementation finding
    config.method_patterns.insert(
        "textDocument/implementation".to_string(),
        MockResponsePattern::Success {
            result: create_php_implementation_response(),
            delay_ms: Some(95),
        },
    );

    // phpactor has limited type definition support
    config.method_patterns.insert(
        "textDocument/typeDefinition".to_string(),
        MockResponsePattern::EmptyArray { delay_ms: Some(60) },
    );

    config
}

/// Create a phpactor config that returns empty responses (for testing edge cases)
pub fn create_empty_phpactor_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "phpactor-empty".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: Some(15),
        verbose: false,
    };

    // All methods return empty arrays or null
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

fn create_php_definition_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/Calculator.php",
            "range": {
                "start": {"line": 12, "character": 17},
                "end": {"line": 12, "character": 26}
            }
        }
    ])
}

fn create_php_references_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/Calculator.php",
            "range": {
                "start": {"line": 8, "character": 8},
                "end": {"line": 8, "character": 17}
            }
        },
        {
            "uri": "file:///workspace/src/MathService.php",
            "range": {
                "start": {"line": 25, "character": 12},
                "end": {"line": 25, "character": 21}
            }
        },
        {
            "uri": "file:///workspace/tests/CalculatorTest.php",
            "range": {
                "start": {"line": 15, "character": 20},
                "end": {"line": 15, "character": 29}
            }
        },
        {
            "uri": "file:///workspace/config/services.php",
            "range": {
                "start": {"line": 42, "character": 35},
                "end": {"line": 42, "character": 44}
            }
        }
    ])
}

fn create_php_hover_response() -> Value {
    json!({
        "contents": {
            "kind": "markdown",
            "value": "```php\\npublic function calculate(int $a, int $b): int\\n```\\n\\n**@param** int $a The first number\\n**@param** int $b The second number\\n**@return** int The calculated result\\n\\nCalculates the sum of two integers.\\n\\nDefined in TestProject\\Calculator"
        },
        "range": {
            "start": {"line": 12, "character": 17},
            "end": {"line": 12, "character": 26}
        }
    })
}

fn create_php_document_symbols_response() -> Value {
    json!([
        {
            "name": "Calculator",
            "kind": 5,
            "range": {
                "start": {"line": 8, "character": 0},
                "end": {"line": 35, "character": 1}
            },
            "selectionRange": {
                "start": {"line": 8, "character": 6},
                "end": {"line": 8, "character": 16}
            },
            "children": [
                {
                    "name": "$result",
                    "kind": 7,
                    "range": {
                        "start": {"line": 10, "character": 4},
                        "end": {"line": 10, "character": 24}
                    },
                    "selectionRange": {
                        "start": {"line": 10, "character": 11},
                        "end": {"line": 10, "character": 18}
                    }
                },
                {
                    "name": "__construct",
                    "kind": 9,
                    "range": {
                        "start": {"line": 12, "character": 4},
                        "end": {"line": 15, "character": 5}
                    },
                    "selectionRange": {
                        "start": {"line": 12, "character": 19},
                        "end": {"line": 12, "character": 30}
                    }
                },
                {
                    "name": "calculate",
                    "kind": 6,
                    "range": {
                        "start": {"line": 17, "character": 4},
                        "end": {"line": 22, "character": 5}
                    },
                    "selectionRange": {
                        "start": {"line": 17, "character": 17},
                        "end": {"line": 17, "character": 26}
                    }
                },
                {
                    "name": "getResult",
                    "kind": 6,
                    "range": {
                        "start": {"line": 24, "character": 4},
                        "end": {"line": 27, "character": 5}
                    },
                    "selectionRange": {
                        "start": {"line": 24, "character": 17},
                        "end": {"line": 24, "character": 26}
                    }
                }
            ]
        },
        {
            "name": "MATH_CONSTANT",
            "kind": 14,
            "range": {
                "start": {"line": 5, "character": 0},
                "end": {"line": 5, "character": 26}
            },
            "selectionRange": {
                "start": {"line": 5, "character": 6},
                "end": {"line": 5, "character": 19}
            }
        }
    ])
}

fn create_php_workspace_symbols_response() -> Value {
    json!([
        {
            "name": "Calculator",
            "kind": 5,
            "location": {
                "uri": "file:///workspace/src/Calculator.php",
                "range": {
                    "start": {"line": 8, "character": 6},
                    "end": {"line": 8, "character": 16}
                }
            }
        },
        {
            "name": "MathService",
            "kind": 5,
            "location": {
                "uri": "file:///workspace/src/MathService.php",
                "range": {
                    "start": {"line": 12, "character": 6},
                    "end": {"line": 12, "character": 17}
                }
            }
        },
        {
            "name": "MathInterface",
            "kind": 11,
            "location": {
                "uri": "file:///workspace/src/Contracts/MathInterface.php",
                "range": {
                    "start": {"line": 8, "character": 10},
                    "end": {"line": 8, "character": 23}
                }
            }
        },
        {
            "name": "calculate",
            "kind": 6,
            "location": {
                "uri": "file:///workspace/src/Calculator.php",
                "range": {
                    "start": {"line": 17, "character": 17},
                    "end": {"line": 17, "character": 26}
                }
            }
        }
    ])
}

fn create_php_prepare_call_hierarchy_response() -> Value {
    json!([
        {
            "name": "calculate",
            "kind": 6,
            "uri": "file:///workspace/src/Calculator.php",
            "range": {
                "start": {"line": 17, "character": 4},
                "end": {"line": 22, "character": 5}
            },
            "selectionRange": {
                "start": {"line": 17, "character": 17},
                "end": {"line": 17, "character": 26}
            }
        }
    ])
}

fn create_php_incoming_calls_response() -> Value {
    json!([
        {
            "from": {
                "name": "performCalculation",
                "kind": 6,
                "uri": "file:///workspace/src/MathService.php",
                "range": {
                    "start": {"line": 20, "character": 4},
                    "end": {"line": 28, "character": 5}
                },
                "selectionRange": {
                    "start": {"line": 20, "character": 17},
                    "end": {"line": 20, "character": 35}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 25, "character": 12},
                    "end": {"line": 25, "character": 21}
                }
            ]
        },
        {
            "from": {
                "name": "testBasicCalculation",
                "kind": 6,
                "uri": "file:///workspace/tests/CalculatorTest.php",
                "range": {
                    "start": {"line": 12, "character": 4},
                    "end": {"line": 18, "character": 5}
                },
                "selectionRange": {
                    "start": {"line": 12, "character": 17},
                    "end": {"line": 12, "character": 36}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 15, "character": 20},
                    "end": {"line": 15, "character": 29}
                }
            ]
        }
    ])
}

fn create_php_outgoing_calls_response() -> Value {
    json!([
        {
            "to": {
                "name": "validateInput",
                "kind": 6,
                "uri": "file:///workspace/src/Calculator.php",
                "range": {
                    "start": {"line": 29, "character": 4},
                    "end": {"line": 33, "character": 5}
                },
                "selectionRange": {
                    "start": {"line": 29, "character": 17},
                    "end": {"line": 29, "character": 30}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 19, "character": 8},
                    "end": {"line": 19, "character": 21}
                }
            ]
        },
        {
            "to": {
                "name": "log",
                "kind": 6,
                "uri": "file:///workspace/src/Logger.php",
                "range": {
                    "start": {"line": 15, "character": 4},
                    "end": {"line": 18, "character": 5}
                },
                "selectionRange": {
                    "start": {"line": 15, "character": 17},
                    "end": {"line": 15, "character": 20}
                }
            },
            "fromRanges": [
                {
                    "start": {"line": 21, "character": 8},
                    "end": {"line": 21, "character": 11}
                }
            ]
        }
    ])
}

fn create_php_completion_response() -> Value {
    json!({
        "isIncomplete": false,
        "items": [
            {
                "label": "array_map",
                "kind": 3,
                "detail": "array array_map(callable $callback, array $array1, array ...$arrays)",
                "documentation": "Applies the callback to the elements of the given arrays",
                "insertText": "array_map(${1:callback}, ${2:array})"
            },
            {
                "label": "$this",
                "kind": 6,
                "detail": "Calculator",
                "documentation": "Reference to the current object instance",
                "insertText": "$this"
            },
            {
                "label": "strlen",
                "kind": 3,
                "detail": "int strlen(string $string)",
                "documentation": "Returns the length of the given string",
                "insertText": "strlen(${1:string})"
            },
            {
                "label": "public function",
                "kind": 15,
                "detail": "Create a public method",
                "documentation": "PHP public method declaration",
                "insertText": "public function ${1:methodName}(${2:parameters}): ${3:returnType}\\n{\\n    ${4:// method body}\\n}"
            },
            {
                "label": "namespace",
                "kind": 15,
                "detail": "Namespace declaration",
                "documentation": "PHP namespace declaration",
                "insertText": "namespace ${1:NamespaceName};"
            }
        ]
    })
}

fn create_php_implementation_response() -> Value {
    json!([
        {
            "uri": "file:///workspace/src/Calculator.php",
            "range": {
                "start": {"line": 8, "character": 0},
                "end": {"line": 35, "character": 1}
            }
        },
        {
            "uri": "file:///workspace/src/AdvancedCalculator.php",
            "range": {
                "start": {"line": 8, "character": 0},
                "end": {"line": 45, "character": 1}
            }
        }
    ])
}

/// Create a phpactor config that simulates slow responses
pub fn create_slow_phpactor_config() -> MockServerConfig {
    let mut config = create_phpactor_config();
    config.server_name = "phpactor-slow".to_string();
    config.global_delay_ms = Some(1500); // 1.5 second delay

    // Make some specific methods even slower
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Success {
            result: create_php_definition_response(),
            delay_ms: Some(4000), // 4 second delay
        },
    );

    config
}

/// Create a phpactor config that simulates errors
pub fn create_error_phpactor_config() -> MockServerConfig {
    let mut config = MockServerConfig {
        server_name: "phpactor-error".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: None,
        verbose: false,
    };

    // Return errors for most methods
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Error {
            code: -32603,
            message: "Internal error: PHP analysis failed".to_string(),
            data: Some(json!({"details": "Mock error for testing PHP parsing"})),
            delay_ms: Some(80),
        },
    );

    config.method_patterns.insert(
        "textDocument/references".to_string(),
        MockResponsePattern::Error {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
            delay_ms: Some(40),
        },
    );

    // Some methods timeout
    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Timeout,
    );

    config
}

/// Create a phpactor config that simulates partial indexing
pub fn create_indexing_phpactor_config() -> MockServerConfig {
    let mut config = create_phpactor_config();
    config.server_name = "phpactor-indexing".to_string();
    config.global_delay_ms = Some(300); // Simulate indexing delay

    // Initial requests return empty while indexing
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Sequence {
            patterns: vec![
                MockResponsePattern::EmptyArray {
                    delay_ms: Some(1500), // First request slow (indexing)
                },
                MockResponsePattern::EmptyArray {
                    delay_ms: Some(800), // Second request still indexing
                },
                MockResponsePattern::Success {
                    result: create_php_definition_response(),
                    delay_ms: Some(90), // Subsequent requests normal
                },
            ],
            current_index: 0,
        },
    );

    config
}
