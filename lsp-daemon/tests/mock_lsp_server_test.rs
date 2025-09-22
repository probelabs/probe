//! Integration tests for the mock LSP server infrastructure
//!
//! These tests validate that the MockLspServer can properly simulate different
//! language server behaviors and response patterns.

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Child;

mod mock_lsp;

use mock_lsp::protocol::{LspRequest, LspResponse};
use mock_lsp::server::{MockResponsePattern, MockServerConfig};
use mock_lsp::{gopls_mock, phpactor_mock, pylsp_mock, rust_analyzer_mock, tsserver_mock};

/// Helper struct to manage a mock LSP server process for testing
struct TestMockServer {
    process: Option<Child>,
    config: MockServerConfig,
}

impl TestMockServer {
    /// Start a mock server process with the given configuration
    async fn start(config: MockServerConfig) -> Result<Self> {
        // For testing purposes, we'll create a simplified mock server subprocess
        // In a real implementation, this would spawn the actual mock server binary
        let server = Self {
            process: None,
            config,
        };

        // Store the server for now - we'll implement the actual subprocess later
        Ok(server)
    }

    /// Send a request and get response (simplified version for testing)
    async fn send_request(&mut self, request: LspRequest) -> Result<Option<LspResponse>> {
        // Simulate the request handling based on the config
        if request.method == "initialize" {
            return Ok(Some(LspResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(json!({
                    "capabilities": {
                        "textDocumentSync": 1,
                        "hoverProvider": true,
                        "definitionProvider": true,
                        "referencesProvider": true,
                        "documentSymbolProvider": true,
                        "workspaceSymbolProvider": true,
                        "callHierarchyProvider": true
                    },
                    "serverInfo": {
                        "name": self.config.server_name,
                        "version": "mock-0.1.0"
                    }
                })),
                error: None,
            }));
        }

        if request.method == "shutdown" {
            return Ok(Some(LspResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(Value::Null),
                error: None,
            }));
        }

        // Get pattern for this method
        if let Some(pattern) = self.config.method_patterns.get(&request.method) {
            self.generate_response_from_pattern(pattern.clone(), request.id)
                .await
        } else {
            // Default to empty array
            Ok(Some(LspResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(json!([])),
                error: None,
            }))
        }
    }

    async fn generate_response_from_pattern(
        &self,
        pattern: MockResponsePattern,
        id: Option<Value>,
    ) -> Result<Option<LspResponse>> {
        self.generate_response_from_pattern_inner(pattern, id, 0)
            .await
    }

    fn generate_response_from_pattern_inner(
        &self,
        pattern: MockResponsePattern,
        id: Option<Value>,
        depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<LspResponse>>> + Send + '_>>
    {
        Box::pin(async move {
            // Prevent infinite recursion
            if depth > 100 {
                return Ok(Some(LspResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!([])),
                    error: None,
                }));
            }

            match pattern {
                MockResponsePattern::Success {
                    result,
                    delay_ms: _,
                } => Ok(Some(LspResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(result),
                    error: None,
                })),
                MockResponsePattern::EmptyArray { delay_ms: _ } => Ok(Some(LspResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!([])),
                    error: None,
                })),
                MockResponsePattern::Null { delay_ms: _ } => Ok(Some(LspResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(Value::Null),
                    error: None,
                })),
                MockResponsePattern::Error {
                    code,
                    message,
                    data,
                    delay_ms: _,
                } => Ok(Some(LspResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(mock_lsp::protocol::LspError {
                        code,
                        message,
                        data,
                    }),
                })),
                MockResponsePattern::Timeout => {
                    // Return None to simulate timeout
                    Ok(None)
                }
                MockResponsePattern::Sequence {
                    patterns,
                    current_index,
                } => {
                    if current_index < patterns.len() {
                        self.generate_response_from_pattern_inner(
                            patterns[current_index].clone(),
                            id,
                            depth + 1,
                        )
                        .await
                    } else {
                        // Default to empty array when sequence is exhausted
                        Ok(Some(LspResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(json!([])),
                            error: None,
                        }))
                    }
                }
            }
        })
    }

    /// Stop the mock server
    async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
        Ok(())
    }
}

impl Drop for TestMockServer {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

#[tokio::test]
async fn test_mock_server_initialization() -> Result<()> {
    let config = rust_analyzer_mock::create_rust_analyzer_config();
    let mut server = TestMockServer::start(config).await?;

    // Test initialize request
    let init_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "initialize".to_string(),
        params: Some(json!({
            "processId": null,
            "rootUri": "file:///workspace",
            "capabilities": {}
        })),
    };

    let response = server.send_request(init_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert_eq!(response.jsonrpc, "2.0");
    assert_eq!(response.id, Some(json!(1)));
    assert!(response.result.is_some());
    assert!(response.error.is_none());

    // Verify capabilities are present
    let result = response.result.unwrap();
    assert!(result.get("capabilities").is_some());
    assert!(result.get("serverInfo").is_some());

    server.stop().await?;
    Ok(())
}

#[tokio::test]
async fn test_rust_analyzer_mock_responses() -> Result<()> {
    let config = rust_analyzer_mock::create_rust_analyzer_config();
    let mut server = TestMockServer::start(config).await?;

    // Test definition request
    let definition_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "textDocument/definition".to_string(),
        params: Some(json!({
            "textDocument": {"uri": "file:///workspace/src/main.rs"},
            "position": {"line": 10, "character": 5}
        })),
    };

    let response = server.send_request(definition_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert!(result.is_array());
    let locations = result.as_array().unwrap();
    assert!(!locations.is_empty());

    // Verify location structure
    let location = &locations[0];
    assert!(location.get("uri").is_some());
    assert!(location.get("range").is_some());

    server.stop().await?;
    Ok(())
}

#[tokio::test]
async fn test_pylsp_mock_responses() -> Result<()> {
    let config = pylsp_mock::create_pylsp_config();
    let mut server = TestMockServer::start(config).await?;

    // Test hover request
    let hover_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(3)),
        method: "textDocument/hover".to_string(),
        params: Some(json!({
            "textDocument": {"uri": "file:///workspace/src/main.py"},
            "position": {"line": 15, "character": 8}
        })),
    };

    let response = server.send_request(hover_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert!(result.get("contents").is_some());

    // Test call hierarchy (should return error for pylsp)
    let call_hierarchy_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(4)),
        method: "textDocument/prepareCallHierarchy".to_string(),
        params: Some(json!({
            "textDocument": {"uri": "file:///workspace/src/main.py"},
            "position": {"line": 15, "character": 8}
        })),
    };

    let response = server.send_request(call_hierarchy_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_some());
    assert_eq!(response.error.unwrap().code, -32601);

    server.stop().await?;
    Ok(())
}

#[tokio::test]
async fn test_gopls_mock_responses() -> Result<()> {
    let config = gopls_mock::create_gopls_config();
    let mut server = TestMockServer::start(config).await?;

    // Test references request
    let references_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(5)),
        method: "textDocument/references".to_string(),
        params: Some(json!({
            "textDocument": {"uri": "file:///workspace/main.go"},
            "position": {"line": 12, "character": 8},
            "context": {"includeDeclaration": true}
        })),
    };

    let response = server.send_request(references_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert!(result.is_array());
    let locations = result.as_array().unwrap();
    assert!(!locations.is_empty());

    // Should have multiple references
    assert!(locations.len() >= 2);

    server.stop().await?;
    Ok(())
}

#[tokio::test]
async fn test_tsserver_mock_responses() -> Result<()> {
    let config = tsserver_mock::create_tsserver_config();
    let mut server = TestMockServer::start(config).await?;

    // Test document symbols request
    let symbols_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(6)),
        method: "textDocument/documentSymbol".to_string(),
        params: Some(json!({
            "textDocument": {"uri": "file:///workspace/src/main.ts"}
        })),
    };

    let response = server.send_request(symbols_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert!(result.is_array());
    let symbols = result.as_array().unwrap();
    assert!(!symbols.is_empty());

    // Verify symbol structure
    let symbol = &symbols[0];
    assert!(symbol.get("name").is_some());
    assert!(symbol.get("kind").is_some());
    assert!(symbol.get("range").is_some());

    server.stop().await?;
    Ok(())
}

#[tokio::test]
async fn test_empty_responses() -> Result<()> {
    let config = rust_analyzer_mock::create_empty_rust_analyzer_config();
    let mut server = TestMockServer::start(config).await?;

    // Test definition request that should return empty array
    let definition_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(7)),
        method: "textDocument/definition".to_string(),
        params: Some(json!({
            "textDocument": {"uri": "file:///workspace/src/main.rs"},
            "position": {"line": 10, "character": 5}
        })),
    };

    let response = server.send_request(definition_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert!(result.is_array());
    let locations = result.as_array().unwrap();
    assert!(locations.is_empty());

    server.stop().await?;
    Ok(())
}

#[tokio::test]
async fn test_error_responses() -> Result<()> {
    let config = rust_analyzer_mock::create_error_rust_analyzer_config();
    let mut server = TestMockServer::start(config).await?;

    // Test definition request that should return error
    let definition_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(8)),
        method: "textDocument/definition".to_string(),
        params: Some(json!({
            "textDocument": {"uri": "file:///workspace/src/main.rs"},
            "position": {"line": 10, "character": 5}
        })),
    };

    let response = server.send_request(definition_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.result.is_none());
    assert!(response.error.is_some());

    let error = response.error.unwrap();
    assert_eq!(error.code, -32603);
    assert!(error.message.contains("Internal error"));

    server.stop().await?;
    Ok(())
}

#[tokio::test]
async fn test_custom_response_patterns() -> Result<()> {
    let mut config = MockServerConfig {
        server_name: "custom-server".to_string(),
        method_patterns: HashMap::new(),
        global_delay_ms: None,
        verbose: false,
    };

    // Add custom patterns
    config.method_patterns.insert(
        "textDocument/definition".to_string(),
        MockResponsePattern::Success {
            result: json!([{
                "uri": "file:///custom/path.rs",
                "range": {
                    "start": {"line": 42, "character": 0},
                    "end": {"line": 42, "character": 10}
                }
            }]),
            delay_ms: None,
        },
    );

    config.method_patterns.insert(
        "textDocument/hover".to_string(),
        MockResponsePattern::Null { delay_ms: None },
    );

    let mut server = TestMockServer::start(config).await?;

    // Test custom definition response
    let definition_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(9)),
        method: "textDocument/definition".to_string(),
        params: Some(json!({})),
    };

    let response = server.send_request(definition_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    let locations = result.as_array().unwrap();
    assert_eq!(locations.len(), 1);

    let location = &locations[0];
    assert_eq!(location["uri"], "file:///custom/path.rs");
    assert_eq!(location["range"]["start"]["line"], 42);

    // Test null hover response
    let hover_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(10)),
        method: "textDocument/hover".to_string(),
        params: Some(json!({})),
    };

    let response = server.send_request(hover_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_none());
    assert!(response.result.is_some());
    assert!(response.result.unwrap().is_null());

    server.stop().await?;
    Ok(())
}

#[tokio::test]
async fn test_shutdown_sequence() -> Result<()> {
    let config = rust_analyzer_mock::create_rust_analyzer_config();
    let mut server = TestMockServer::start(config).await?;

    // Test shutdown request
    let shutdown_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(999)),
        method: "shutdown".to_string(),
        params: None,
    };

    let response = server.send_request(shutdown_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert_eq!(response.id, Some(json!(999)));
    assert!(response.result.is_some());
    assert!(response.result.unwrap().is_null());
    assert!(response.error.is_none());

    server.stop().await?;
    Ok(())
}

/// Test that validates the mock server can handle method patterns correctly
#[tokio::test]
async fn test_method_pattern_resolution() -> Result<()> {
    // Test that each language server mock has the expected methods configured

    // rust-analyzer should support call hierarchy
    let rust_config = rust_analyzer_mock::create_rust_analyzer_config();
    assert!(rust_config
        .method_patterns
        .contains_key("textDocument/prepareCallHierarchy"));
    assert!(rust_config
        .method_patterns
        .contains_key("callHierarchy/incomingCalls"));
    assert!(rust_config
        .method_patterns
        .contains_key("callHierarchy/outgoingCalls"));

    // pylsp should NOT support call hierarchy (should have error patterns)
    let pylsp_config = pylsp_mock::create_pylsp_config();
    if let Some(pattern) = pylsp_config
        .method_patterns
        .get("textDocument/prepareCallHierarchy")
    {
        match pattern {
            MockResponsePattern::Error { code, .. } => {
                assert_eq!(*code, -32601); // Method not found
            }
            _ => panic!("Expected error pattern for pylsp call hierarchy"),
        }
    }

    // gopls should support most methods
    let gopls_config = gopls_mock::create_gopls_config();
    assert!(gopls_config
        .method_patterns
        .contains_key("textDocument/definition"));
    assert!(gopls_config
        .method_patterns
        .contains_key("textDocument/references"));
    assert!(gopls_config
        .method_patterns
        .contains_key("textDocument/implementation"));

    // TypeScript should support call hierarchy
    let ts_config = tsserver_mock::create_tsserver_config();
    assert!(ts_config
        .method_patterns
        .contains_key("textDocument/prepareCallHierarchy"));

    Ok(())
}

#[tokio::test]
async fn test_phpactor_mock_responses() -> Result<()> {
    let config = phpactor_mock::create_phpactor_config();
    let mut server = TestMockServer::start(config).await?;

    // Test definition request
    let definition_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "textDocument/definition".to_string(),
        params: Some(json!({
            "textDocument": {"uri": "file:///workspace/src/Calculator.php"},
            "position": {"line": 17, "character": 20}
        })),
    };

    let response = server.send_request(definition_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert!(result.is_array());
    let locations = result.as_array().unwrap();
    assert!(!locations.is_empty());

    // Verify location structure
    let location = &locations[0];
    assert!(location.get("uri").is_some());
    assert!(location.get("range").is_some());

    // Test hover request
    let hover_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "textDocument/hover".to_string(),
        params: Some(json!({
            "textDocument": {"uri": "file:///workspace/src/Calculator.php"},
            "position": {"line": 12, "character": 20}
        })),
    };

    let response = server.send_request(hover_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert!(result.get("contents").is_some());

    // Test call hierarchy (phpactor supports it)
    let prepare_call_hierarchy_request = LspRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(3)),
        method: "textDocument/prepareCallHierarchy".to_string(),
        params: Some(json!({
            "textDocument": {"uri": "file:///workspace/src/Calculator.php"},
            "position": {"line": 17, "character": 20}
        })),
    };

    let response = server.send_request(prepare_call_hierarchy_request).await?;
    assert!(response.is_some());

    let response = response.unwrap();
    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert!(result.is_array());
    let items = result.as_array().unwrap();
    assert!(!items.is_empty());

    // Verify call hierarchy item structure
    let item = &items[0];
    assert!(item.get("name").is_some());
    assert!(item.get("kind").is_some());
    assert!(item.get("uri").is_some());
    assert!(item.get("range").is_some());
    assert!(item.get("selectionRange").is_some());

    server.stop().await?;
    Ok(())
}
