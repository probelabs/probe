//! Comprehensive tests for symbol resolution across all supported languages.
//!
//! These tests verify that `find_all_symbols_in_file` and `find_symbol_in_file`
//! correctly resolve symbols using AST-level analysis (not text_search fallback)
//! for all supported language patterns, including:
//! - Go receiver methods (the fix for issue #461)
//! - Rust impl methods
//! - Python class methods
//! - JavaScript/TypeScript class methods
//! - Java class methods
//! - C# class methods
//! - PHP class methods
//! - Disambiguation of same-named symbols across types

use anyhow::Result;
use probe_code::extract::symbol_finder::{find_all_symbols_in_file, find_symbol_in_file};
use std::fs;
use std::path::PathBuf;

// ============================================================
// Go receiver method tests (Issue #461)
// ============================================================

#[test]
fn test_go_receiver_method_pointer_receiver() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // TykApi.GetOrgKeyList — pointer receiver method
    let results = find_all_symbols_in_file(&path, "TykApi.GetOrgKeyList", &content, true, 0)?;

    assert_eq!(results.len(), 1, "Should find exactly 1 match");
    let result = &results[0];

    // Must be AST-resolved, NOT text_search fallback
    assert_eq!(
        result.node_type, "method_declaration",
        "Should resolve via AST (method_declaration), not text_search fallback"
    );

    // Must return the full function body, not just the signature line
    assert!(
        result.lines.1 > result.lines.0,
        "Should return multi-line range (full body), got lines {}-{}",
        result.lines.0,
        result.lines.1
    );

    // Verify the code includes the full body
    assert!(result.code.contains("func (t *TykApi) GetOrgKeyList()"));
    assert!(
        result.code.contains("return apiKeys, nil"),
        "Should include the return statement in the body"
    );

    Ok(())
}

#[test]
fn test_go_receiver_method_value_receiver() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // RateLimitMiddleware.Name — value receiver (not pointer)
    let results = find_all_symbols_in_file(&path, "RateLimitMiddleware.Name", &content, true, 0)?;

    assert_eq!(results.len(), 1, "Should find exactly 1 match");
    let result = &results[0];

    assert_eq!(
        result.node_type, "method_declaration",
        "Value receiver methods should also resolve via AST"
    );
    assert!(result.code.contains("func (r RateLimitMiddleware) Name()"));
    assert!(result.code.contains("return \"RateLimitMiddleware\""));

    Ok(())
}

#[test]
fn test_go_multiple_methods_on_same_type() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // All TykApi methods should resolve individually
    let methods = ["GetOrgKeyList", "CreateKey", "DeleteKey"];
    for method in &methods {
        let symbol = format!("TykApi.{method}");
        let results = find_all_symbols_in_file(&path, &symbol, &content, true, 0)?;

        assert_eq!(results.len(), 1, "Should find exactly 1 match for {symbol}");
        assert_eq!(
            results[0].node_type, "method_declaration",
            "{symbol} should resolve via AST"
        );
        assert!(
            results[0].lines.1 > results[0].lines.0,
            "{symbol} should return full body, not just signature"
        );
    }

    Ok(())
}

#[test]
fn test_go_method_name_disambiguation() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // "Name" exists on both AuthMiddleware and RateLimitMiddleware
    // Qualified lookup should return only the correct one
    let auth_results = find_all_symbols_in_file(&path, "AuthMiddleware.Name", &content, true, 0)?;
    assert_eq!(auth_results.len(), 1);
    assert!(auth_results[0].code.contains("return \"AuthMiddleware\""));

    let rate_results =
        find_all_symbols_in_file(&path, "RateLimitMiddleware.Name", &content, true, 0)?;
    assert_eq!(rate_results.len(), 1);
    assert!(rate_results[0]
        .code
        .contains("return \"RateLimitMiddleware\""));

    Ok(())
}

#[test]
fn test_go_regular_function_not_method() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // Regular function (not a method)
    let results = find_all_symbols_in_file(&path, "NewTykApi", &content, true, 0)?;

    assert_eq!(results.len(), 1, "Should find exactly 1 match");
    assert_eq!(
        results[0].node_type, "function_declaration",
        "Regular function should be function_declaration"
    );
    assert!(results[0].code.contains("func NewTykApi("));

    Ok(())
}

#[test]
fn test_go_function_vs_method_same_name() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // "Start" exists as both a regular function AND as Gateway.Start method
    // Bare lookup should find both
    let results = find_all_symbols_in_file(&path, "Start", &content, true, 0)?;

    assert!(
        results.len() >= 2,
        "Should find at least 2 symbols named 'Start' (function + method), got {}",
        results.len()
    );

    let node_types: Vec<&str> = results.iter().map(|r| r.node_type.as_str()).collect();
    assert!(
        node_types.contains(&"function_declaration"),
        "Should include the top-level function"
    );
    assert!(
        node_types.contains(&"method_declaration"),
        "Should include the receiver method"
    );

    // Qualified lookup should find only the method
    let qualified = find_all_symbols_in_file(&path, "Gateway.Start", &content, true, 0)?;
    assert_eq!(qualified.len(), 1, "Qualified lookup should find exactly 1");
    assert_eq!(qualified[0].node_type, "method_declaration");
    assert!(qualified[0].code.contains("func (g *Gateway) Start()"));

    Ok(())
}

#[test]
fn test_go_existing_mock_ip_whitelist() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_ip_whitelist.go");
    let content = fs::read_to_string(&path)?;

    let results = find_all_symbols_in_file(&path, "IPWhiteListMiddleware.Name", &content, true, 0)?;

    assert_eq!(results.len(), 1, "Should find exactly 1 match");
    assert_eq!(
        results[0].node_type, "method_declaration",
        "Should resolve via AST, not text_search"
    );
    assert!(results[0]
        .code
        .contains("func (i *IPWhiteListMiddleware) Name()"));
    assert!(results[0].code.contains("return \"IPWhiteListMiddleware\""));

    Ok(())
}

#[test]
fn test_go_interface_methods_process_request() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // ProcessRequest exists on both AuthMiddleware (pointer) and RateLimitMiddleware (value)
    let auth = find_all_symbols_in_file(&path, "AuthMiddleware.ProcessRequest", &content, true, 0)?;
    assert_eq!(auth.len(), 1);
    assert_eq!(auth[0].node_type, "method_declaration");
    assert!(auth[0].code.contains("missing auth token"));

    let rate = find_all_symbols_in_file(
        &path,
        "RateLimitMiddleware.ProcessRequest",
        &content,
        true,
        0,
    )?;
    assert_eq!(rate.len(), 1);
    assert_eq!(rate[0].node_type, "method_declaration");
    assert!(rate[0].code.contains("invalid rate limit config"));

    Ok(())
}

// ============================================================
// Rust impl method tests
// ============================================================

#[test]
fn test_rust_impl_method_resolution() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_rust_impl.rs");
    let content = fs::read_to_string(&path)?;

    // Cache.get — method inside impl block
    let results = find_all_symbols_in_file(&path, "Cache.get", &content, true, 0)?;

    assert_eq!(results.len(), 1, "Should find exactly 1 match");
    assert_eq!(
        results[0].node_type, "function_item",
        "Rust methods should be function_item"
    );
    assert!(results[0].code.contains("pub fn get(&self, key: &str)"));
    assert!(results[0].code.contains("self.items.get(key)"));

    Ok(())
}

#[test]
fn test_rust_impl_disambiguation() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_rust_impl.rs");
    let content = fs::read_to_string(&path)?;

    // "get" exists as Cache.get, Registry.get, and top-level get
    let results = find_all_symbols_in_file(&path, "get", &content, true, 0)?;

    assert!(
        results.len() >= 3,
        "Should find at least 3 symbols named 'get', got {}",
        results.len()
    );

    // Qualified lookups should each return exactly 1
    let cache_get = find_all_symbols_in_file(&path, "Cache.get", &content, true, 0)?;
    assert_eq!(cache_get.len(), 1);
    assert!(cache_get[0].code.contains("self.items.get(key)"));

    let registry_get = find_all_symbols_in_file(&path, "Registry.get", &content, true, 0)?;
    assert_eq!(registry_get.len(), 1);
    assert!(registry_get[0].code.contains("self.entries.get(index)"));

    Ok(())
}

#[test]
fn test_rust_struct_extraction() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_rust_impl.rs");
    let content = fs::read_to_string(&path)?;

    let results = find_all_symbols_in_file(&path, "Cache", &content, true, 0)?;

    assert!(!results.is_empty(), "Should find Cache struct");
    assert_eq!(results[0].node_type, "struct_item");
    assert!(results[0].code.contains("pub struct Cache"));

    Ok(())
}

// ============================================================
// Python class method tests
// ============================================================

#[test]
fn test_python_class_method_resolution() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_python_classes.py");
    let content = fs::read_to_string(&path)?;

    // DataProcessor.process
    let results = find_all_symbols_in_file(&path, "DataProcessor.process", &content, true, 0)?;

    assert_eq!(results.len(), 1, "Should find exactly 1 match");
    assert_eq!(results[0].node_type, "function_definition");
    assert!(results[0].code.contains("def process(self, data)"));

    Ok(())
}

#[test]
fn test_python_method_disambiguation() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_python_classes.py");
    let content = fs::read_to_string(&path)?;

    // "process" exists in DataProcessor, StreamProcessor, and as top-level function
    let results = find_all_symbols_in_file(&path, "process", &content, true, 0)?;

    assert!(
        results.len() >= 3,
        "Should find at least 3 symbols named 'process', got {}",
        results.len()
    );

    // Qualified lookups
    let dp = find_all_symbols_in_file(&path, "DataProcessor.process", &content, true, 0)?;
    assert_eq!(dp.len(), 1);
    assert!(dp[0].code.contains("x * 2"));

    let sp = find_all_symbols_in_file(&path, "StreamProcessor.process", &content, true, 0)?;
    assert_eq!(sp.len(), 1);
    assert!(sp[0].code.contains("for chunk in stream"));

    Ok(())
}

// ============================================================
// JavaScript class method tests
// ============================================================

#[test]
fn test_javascript_class_method_resolution() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("test_js_classes.js");

    let content = r#"class Calculator {
    constructor(precision) {
        this.precision = precision;
    }

    add(a, b) {
        return a + b;
    }

    subtract(a, b) {
        return a - b;
    }
}

class ScientificCalculator extends Calculator {
    add(a, b) {
        return super.add(a, b).toFixed(this.precision);
    }

    power(base, exp) {
        return Math.pow(base, exp);
    }
}

function add(a, b) {
    return a + b;
}
"#;

    fs::write(&path, content)?;

    // Calculator.add
    let results = find_all_symbols_in_file(&path, "Calculator.add", content, true, 0)?;
    assert_eq!(results.len(), 1, "Should find Calculator.add");
    assert_eq!(results[0].node_type, "method_definition");
    assert!(results[0].code.contains("return a + b"));

    // "add" bare name should find 3: Calculator.add, ScientificCalculator.add, top-level add
    let results = find_all_symbols_in_file(&path, "add", content, true, 0)?;
    assert!(
        results.len() >= 3,
        "Should find at least 3 symbols named 'add', got {}",
        results.len()
    );

    Ok(())
}

// ============================================================
// TypeScript class method tests
// ============================================================

#[test]
fn test_typescript_class_method_resolution() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("test_ts_classes.ts");

    let content = r#"export class ApiClient {
    private baseUrl: string;

    constructor(baseUrl: string) {
        this.baseUrl = baseUrl;
    }

    async fetch(endpoint: string): Promise<Response> {
        return fetch(`${this.baseUrl}/${endpoint}`);
    }

    async post(endpoint: string, body: object): Promise<Response> {
        return fetch(`${this.baseUrl}/${endpoint}`, {
            method: 'POST',
            body: JSON.stringify(body),
        });
    }
}

class CachedApiClient extends ApiClient {
    private cache: Map<string, any>;

    async fetch(endpoint: string): Promise<Response> {
        if (this.cache.has(endpoint)) {
            return this.cache.get(endpoint);
        }
        return super.fetch(endpoint);
    }
}
"#;

    fs::write(&path, content)?;

    // Qualified lookup
    let results = find_all_symbols_in_file(&path, "ApiClient.fetch", content, true, 0)?;
    assert_eq!(results.len(), 1, "Should find ApiClient.fetch");
    assert_eq!(results[0].node_type, "method_definition");
    assert!(results[0].code.contains("async fetch(endpoint: string)"));

    // Different class same method name
    let results = find_all_symbols_in_file(&path, "CachedApiClient.fetch", content, true, 0)?;
    assert_eq!(results.len(), 1);
    assert!(results[0].code.contains("this.cache.has(endpoint)"));

    Ok(())
}

// ============================================================
// Java class method tests
// ============================================================

#[test]
fn test_java_class_method_resolution() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_java_classes.java");
    let content = fs::read_to_string(&path)?;

    // UserService.getUsers
    let results = find_all_symbols_in_file(&path, "UserService.getUsers", &content, true, 0)?;
    assert_eq!(results.len(), 1, "Should find UserService.getUsers");
    assert_eq!(results[0].node_type, "method_declaration");

    // "getUsers" exists in both UserService and OrderService
    let results = find_all_symbols_in_file(&path, "getUsers", &content, true, 0)?;
    assert!(
        results.len() >= 2,
        "Should find at least 2 symbols named 'getUsers', got {}",
        results.len()
    );

    // Qualified lookup for OrderService
    let results = find_all_symbols_in_file(&path, "OrderService.getUsers", &content, true, 0)?;
    assert_eq!(results.len(), 1, "Should find OrderService.getUsers");

    Ok(())
}

// ============================================================
// C# class method tests
// ============================================================

#[test]
fn test_csharp_class_method_resolution() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_csharp_classes.cs");
    let content = fs::read_to_string(&path)?;

    // UserController.GetAll
    let results = find_all_symbols_in_file(&path, "UserController.GetAll", &content, true, 0)?;
    assert_eq!(results.len(), 1, "Should find UserController.GetAll");
    assert_eq!(results[0].node_type, "method_declaration");
    assert!(results[0].code.contains("GetAllUsers"));

    // "GetAll" exists in both UserController and ProductController
    let results = find_all_symbols_in_file(&path, "GetAll", &content, true, 0)?;
    assert!(
        results.len() >= 2,
        "Should find at least 2 symbols named 'GetAll', got {}",
        results.len()
    );

    Ok(())
}

// ============================================================
// PHP class method tests
// ============================================================

#[test]
fn test_php_class_method_resolution() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_php_classes.php");
    let content = fs::read_to_string(&path)?;

    // UserRepository.findAll
    let results = find_all_symbols_in_file(&path, "UserRepository.findAll", &content, true, 0)?;
    assert_eq!(results.len(), 1, "Should find UserRepository.findAll");
    assert!(results[0].code.contains("SELECT * FROM users"));

    // "findAll" exists in both repositories
    let results = find_all_symbols_in_file(&path, "findAll", &content, true, 0)?;
    assert!(
        results.len() >= 2,
        "Should find at least 2 symbols named 'findAll', got {}",
        results.len()
    );

    Ok(())
}

// ============================================================
// Cross-language: verify AST resolution (not text_search)
// ============================================================

#[test]
fn test_go_never_returns_text_search_for_valid_methods() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    let test_cases = vec![
        "TykApi.GetOrgKeyList",
        "TykApi.CreateKey",
        "TykApi.DeleteKey",
        "Gateway.Start",
        "Gateway.Stop",
        "AuthMiddleware.Name",
        "AuthMiddleware.ProcessRequest",
        "RateLimitMiddleware.Name",
        "RateLimitMiddleware.ProcessRequest",
    ];

    for symbol in &test_cases {
        let results = find_all_symbols_in_file(&path, symbol, &content, true, 0)?;

        assert!(!results.is_empty(), "Should find symbol '{symbol}'");
        assert_ne!(
            results[0].node_type, "text_search",
            "Symbol '{symbol}' should NOT fall back to text_search — should be method_declaration"
        );
        assert_eq!(
            results[0].node_type, "method_declaration",
            "Symbol '{symbol}' should resolve as method_declaration"
        );
    }

    Ok(())
}

#[test]
fn test_go_method_returns_full_body_not_just_signature() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // GetOrgKeyList has many lines — verify we get them all
    let results = find_all_symbols_in_file(&path, "TykApi.GetOrgKeyList", &content, true, 0)?;
    let result = &results[0];

    let line_count = result.lines.1 - result.lines.0 + 1;
    assert!(
        line_count > 5,
        "GetOrgKeyList body should span many lines, got {} (lines {}-{})",
        line_count,
        result.lines.0,
        result.lines.1
    );

    // Verify both the opening and closing of the function
    assert!(result.code.contains("func (t *TykApi) GetOrgKeyList()"));
    assert!(
        result.code.trim_end().ends_with('}'),
        "Should include the closing brace"
    );

    Ok(())
}

// ============================================================
// Edge cases
// ============================================================

#[test]
fn test_go_nonexistent_method_on_type() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // Method that doesn't exist on TykApi
    let results = find_all_symbols_in_file(&path, "TykApi.NonExistentMethod", &content, true, 0)?;

    // Should either return empty or fall back to text_search (not crash)
    if !results.is_empty() {
        assert_eq!(
            results[0].node_type, "text_search",
            "Non-existent method should only be found via text_search fallback"
        );
    }

    Ok(())
}

#[test]
fn test_go_nonexistent_type() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // Type that doesn't exist
    let results = find_all_symbols_in_file(&path, "NonExistentType.SomeMethod", &content, true, 0)?;

    // Should return empty (nothing found)
    assert!(
        results.is_empty(),
        "Non-existent type should return empty results"
    );

    Ok(())
}

#[test]
fn test_find_symbol_in_file_returns_full_go_method() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // find_symbol_in_file (singular) should also work
    let result = find_symbol_in_file(&path, "TykApi.GetOrgKeyList", &content, true, 0)?;

    assert_eq!(result.node_type, "method_declaration");
    assert!(result.code.contains("func (t *TykApi) GetOrgKeyList()"));
    assert!(result.code.contains("return apiKeys, nil"));

    Ok(())
}

#[test]
fn test_go_bare_name_finds_all_receiver_methods() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_go_methods.go");
    let content = fs::read_to_string(&path)?;

    // "Name" exists on both AuthMiddleware and RateLimitMiddleware
    // Bare lookup should find both (via regular AST traversal, since method_declaration
    // nodes with field_identifier "Name" are directly findable)
    let results = find_all_symbols_in_file(&path, "Name", &content, true, 0)?;

    assert!(
        results.len() >= 2,
        "Should find at least 2 symbols named 'Name' (AuthMiddleware + RateLimitMiddleware), got {}",
        results.len()
    );

    // All should be method_declaration
    for result in &results {
        assert_eq!(
            result.node_type, "method_declaration",
            "All 'Name' results should be method_declaration"
        );
    }

    Ok(())
}

#[test]
fn test_go_single_line_method_body() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("test_single_line.go");

    let content = r#"package main

type Svc struct{}

func (s *Svc) Noop() {}

func (s *Svc) OneReturn() string { return "ok" }
"#;

    fs::write(&path, content)?;

    // Single-line empty body
    let results = find_all_symbols_in_file(&path, "Svc.Noop", content, true, 0)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].node_type, "method_declaration");
    assert!(results[0].code.contains("func (s *Svc) Noop()"));

    // Single-line with return
    let results = find_all_symbols_in_file(&path, "Svc.OneReturn", content, true, 0)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].node_type, "method_declaration");
    assert!(results[0].code.contains("return \"ok\""));

    Ok(())
}

#[test]
fn test_go_multi_line_signature() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("test_multiline_sig.go");

    let content = r#"package main

type Handler struct{}

func (h *Handler) Process(
	input string,
	count int,
	verbose bool,
) (string, error) {
	if verbose {
		return input, nil
	}
	return "", nil
}
"#;

    fs::write(&path, content)?;

    let results = find_all_symbols_in_file(&path, "Handler.Process", content, true, 0)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].node_type, "method_declaration");

    // Should include the full function body
    assert!(results[0].code.contains("func (h *Handler) Process("));
    assert!(results[0].code.contains("return \"\", nil"));

    // Should span multiple lines
    let line_count = results[0].lines.1 - results[0].lines.0 + 1;
    assert!(
        line_count >= 7,
        "Multi-line signature method should span 7+ lines, got {}",
        line_count
    );

    Ok(())
}

#[test]
fn test_go_generic_receiver() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("test_generic.go");

    // Go 1.18+ generic type with method
    let content = r#"package main

type Cache[K comparable, V any] struct {
	items map[K]V
}

func (c *Cache[K, V]) Get(key K) (V, bool) {
	v, ok := c.items[key]
	return v, ok
}

func (c *Cache[K, V]) Set(key K, value V) {
	c.items[key] = value
}
"#;

    fs::write(&path, content)?;

    // Test with generic type receiver
    let results = find_all_symbols_in_file(&path, "Cache.Get", content, true, 0)?;
    assert_eq!(
        results.len(),
        1,
        "Should find Cache.Get with generic receiver"
    );
    assert_eq!(results[0].node_type, "method_declaration");
    assert!(results[0].code.contains("func (c *Cache[K, V]) Get("));
    assert!(results[0].code.contains("c.items[key]"));

    let results = find_all_symbols_in_file(&path, "Cache.Set", content, true, 0)?;
    assert_eq!(
        results.len(),
        1,
        "Should find Cache.Set with generic receiver"
    );

    Ok(())
}

// ============================================================
// Cross-language: find_symbol_in_file (singular) for key languages
// ============================================================

#[test]
fn test_find_symbol_singular_python() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_python_classes.py");
    let content = fs::read_to_string(&path)?;

    let result = find_symbol_in_file(&path, "DataProcessor.process", &content, true, 0)?;
    assert_eq!(result.node_type, "function_definition");
    assert!(result.code.contains("def process(self, data)"));

    Ok(())
}

#[test]
fn test_find_symbol_singular_rust() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_rust_impl.rs");
    let content = fs::read_to_string(&path)?;

    let result = find_symbol_in_file(&path, "Cache.get", &content, true, 0)?;
    assert_eq!(result.node_type, "function_item");
    assert!(result.code.contains("pub fn get(&self, key: &str)"));

    Ok(())
}

#[test]
fn test_find_symbol_singular_java() -> Result<()> {
    let path = PathBuf::from("tests/mocks/test_java_classes.java");
    let content = fs::read_to_string(&path)?;

    let result = find_symbol_in_file(&path, "UserService.findUser", &content, true, 0)?;
    assert_eq!(result.node_type, "method_declaration");
    assert!(result.code.contains("findUser"));

    Ok(())
}
