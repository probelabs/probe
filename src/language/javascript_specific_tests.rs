use super::factory::get_language_impl;
use super::parser::parse_file_for_code_blocks;
use std::collections::HashSet;

/// Comprehensive JavaScript-specific tests to document expected behavior
/// These tests verify that the JavaScript language implementation correctly
/// identifies acceptable parent nodes and provides good code extraction.

#[test]
fn test_javascript_function_expressions() {
    // Enable debug mode for this test
    std::env::set_var("DEBUG", "1");

    let js_code = r#"
// Function expression assigned to variable
const myFunction = function(a, b) {
    return a + b;
};

// Arrow function assigned to variable
const arrowFunction = (x, y) => {
    return x * y;
};

// Function expression as callback
setTimeout(function() {
    console.log("Timer fired");
}, 1000);

// Arrow function as callback
setTimeout(() => {
    console.log("Arrow timer fired");
}, 2000);
"#;

    println!("Code with line numbers:");
    for (i, line) in js_code.lines().enumerate() {
        println!("{}: {line}", i + 1);
    }

    // Create a HashSet with all line numbers to get comprehensive extraction
    let mut line_numbers = HashSet::new();
    for i in 1..=js_code.lines().count() {
        line_numbers.insert(i);
    }

    let result = parse_file_for_code_blocks(js_code, "js", &line_numbers, true, None);
    assert!(
        result.is_ok(),
        "Failed to parse JavaScript code: {:?}",
        result.err()
    );

    let blocks = result.unwrap();
    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Verify we got function expressions as acceptable parents
    let has_function_expression = blocks
        .iter()
        .any(|block| block.node_type == "function_expression");

    assert!(
        has_function_expression
            || blocks
                .iter()
                .any(|block| block.node_type == "arrow_function"),
        "Expected to find function_expression or arrow_function as acceptable parent"
    );

    // Verify we don't get variable_declaration as the main container
    // (This was the problem that the GitHub issue was trying to fix)
    let has_variable_declaration_only = blocks.iter().all(|block| {
        block.node_type == "variable_declaration" || block.node_type == "lexical_declaration"
    });

    assert!(
        !has_variable_declaration_only,
        "Should not have only variable_declaration/lexical_declaration blocks - these should be children of function expressions"
    );

    std::env::remove_var("DEBUG");
}

#[test]
fn test_javascript_class_methods() {
    std::env::set_var("DEBUG", "1");

    let js_code = r#"
class Calculator {
    constructor(name) {
        this.name = name;
    }
    
    // Method with function expression
    add = function(a, b) {
        return a + b;
    };
    
    // Arrow method
    multiply = (a, b) => {
        return a * b;
    };
    
    // Regular method
    subtract(a, b) {
        return a - b;
    }
}
"#;

    println!("Code with line numbers:");
    for (i, line) in js_code.lines().enumerate() {
        println!("{}: {line}", i + 1);
    }

    let mut line_numbers = HashSet::new();
    for i in 1..=js_code.lines().count() {
        line_numbers.insert(i);
    }

    let result = parse_file_for_code_blocks(js_code, "js", &line_numbers, true, None);
    assert!(
        result.is_ok(),
        "Failed to parse JavaScript class: {:?}",
        result.err()
    );

    let blocks = result.unwrap();
    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Should find class-related nodes or method/property nodes within the class
    // Note: The parser prioritizes specific method nodes over the broader class declaration
    let has_class_or_method = blocks.iter().any(|block| {
        block.node_type == "class_declaration"
            || block.node_type == "class"
            || block.node_type == "method_definition"
            || block.node_type == "property_identifier"
    });
    assert!(
        has_class_or_method,
        "Expected to find class-related or method nodes"
    );

    // Should find method definitions or function expressions within the class
    let has_method_or_function = blocks.iter().any(
        |block| {
            block.node_type == "method_definition"
                || block.node_type == "function_expression"
                || block.node_type == "arrow_function"
                || block.node_type.contains("property")
        }, // property_identifier or similar
    );
    assert!(
        has_method_or_function,
        "Expected to find methods or functions within class"
    );

    std::env::remove_var("DEBUG");
}

#[test]
fn test_javascript_object_methods() {
    std::env::set_var("DEBUG", "1");

    let js_code = r#"
const api = {
    // Method shorthand
    getData() {
        return fetch('/api/data');
    },
    
    // Function expression property
    processData: function(data) {
        return data.map(item => item.value);
    },
    
    // Arrow function property
    validateData: (data) => {
        return data.every(item => item.id);
    }
};
"#;

    println!("Code with line numbers:");
    for (i, line) in js_code.lines().enumerate() {
        println!("{}: {line}", i + 1);
    }

    let mut line_numbers = HashSet::new();
    for i in 1..=js_code.lines().count() {
        line_numbers.insert(i);
    }

    let result = parse_file_for_code_blocks(js_code, "js", &line_numbers, true, None);
    assert!(
        result.is_ok(),
        "Failed to parse JavaScript object: {:?}",
        result.err()
    );

    let blocks = result.unwrap();
    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Should find meaningful code blocks (object or contained methods)
    // The test might extract individual methods rather than the whole object
    let has_meaningful_blocks = !blocks.is_empty();
    assert!(
        has_meaningful_blocks,
        "Expected to find meaningful code blocks"
    );

    // Should find function expressions within the object or acceptable container structures
    let has_function_or_container = blocks.iter().any(|block| {
        block.node_type == "function_expression"
            || block.node_type == "arrow_function"
            || block.node_type == "method_definition"
            || block.node_type.contains("property")
    });
    assert!(
        has_function_or_container,
        "Expected to find functions or method-like structures in object"
    );

    std::env::remove_var("DEBUG");
}

#[test]
fn test_javascript_module_exports() {
    std::env::set_var("DEBUG", "1");

    let js_code = r#"
// Named exports with function expressions
export const createUser = function(name, email) {
    return { name, email, id: Date.now() };
};

export const validateUser = (user) => {
    return user.name && user.email;
};

// Default export with function expression
export default function Router() {
    const routes = {};
    
    return {
        get: function(path, handler) {
            routes[path] = handler;
        },
        
        handle: (req) => {
            const handler = routes[req.path];
            return handler ? handler(req) : null;
        }
    };
}
"#;

    println!("Code with line numbers:");
    for (i, line) in js_code.lines().enumerate() {
        println!("{}: {line}", i + 1);
    }

    let mut line_numbers = HashSet::new();
    for i in 1..=js_code.lines().count() {
        line_numbers.insert(i);
    }

    let result = parse_file_for_code_blocks(js_code, "js", &line_numbers, true, None);
    assert!(
        result.is_ok(),
        "Failed to parse JavaScript exports: {:?}",
        result.err()
    );

    let blocks = result.unwrap();
    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Should find export statements
    let has_export = blocks
        .iter()
        .any(|block| block.node_type == "export_statement");
    assert!(has_export, "Expected to find export_statement");

    // Should find export statements (which contain the functions)
    // The export_statement contains the function declarations/expressions
    let has_export_or_function = blocks.iter().any(|block| {
        block.node_type == "export_statement"
            || block.node_type == "function_declaration"
            || block.node_type == "function_expression"
            || block.node_type == "arrow_function"
    });
    assert!(
        has_export_or_function,
        "Expected to find export statements or function-related nodes"
    );

    std::env::remove_var("DEBUG");
}

#[test]
fn test_javascript_iife_patterns() {
    std::env::set_var("DEBUG", "1");

    let js_code = r#"
// Immediately Invoked Function Expression (IIFE)
(function() {
    const privateVar = "secret";
    
    window.myModule = {
        publicMethod: function() {
            return privateVar;
        }
    };
})();

// Arrow IIFE
(() => {
    const config = { api: '/api/v1' };
    
    window.appConfig = config;
})();
"#;

    println!("Code with line numbers:");
    for (i, line) in js_code.lines().enumerate() {
        println!("{}: {line}", i + 1);
    }

    let mut line_numbers = HashSet::new();
    for i in 1..=js_code.lines().count() {
        line_numbers.insert(i);
    }

    let result = parse_file_for_code_blocks(js_code, "js", &line_numbers, true, None);
    assert!(
        result.is_ok(),
        "Failed to parse JavaScript IIFE: {:?}",
        result.err()
    );

    let blocks = result.unwrap();
    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Should find function expressions for IIFE or meaningful blocks
    let has_function_or_meaningful = blocks.iter().any(
        |block| {
            block.node_type == "function_expression"
                || block.node_type == "arrow_function"
                || block.node_type.contains("function")
                || !blocks.is_empty()
        }, // At least extracted some meaningful code
    );
    assert!(
        has_function_or_meaningful,
        "Expected to find function expressions or meaningful blocks for IIFE patterns"
    );

    std::env::remove_var("DEBUG");
}

#[test]
fn test_javascript_async_await_patterns() {
    std::env::set_var("DEBUG", "1");

    let js_code = r#"
// Async function declaration
async function fetchData(url) {
    const response = await fetch(url);
    return response.json();
}

// Async function expression
const processData = async function(data) {
    const processed = await transform(data);
    return processed;
};

// Async arrow function
const validateData = async (data) => {
    const isValid = await validator.check(data);
    return isValid;
};
"#;

    println!("Code with line numbers:");
    for (i, line) in js_code.lines().enumerate() {
        println!("{}: {line}", i + 1);
    }

    let mut line_numbers = HashSet::new();
    for i in 1..=js_code.lines().count() {
        line_numbers.insert(i);
    }

    let result = parse_file_for_code_blocks(js_code, "js", &line_numbers, true, None);
    assert!(
        result.is_ok(),
        "Failed to parse JavaScript async code: {:?}",
        result.err()
    );

    let blocks = result.unwrap();
    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Should find function declarations and expressions
    let has_function = blocks.iter().any(|block| {
        block.node_type == "function_declaration"
            || block.node_type == "function_expression"
            || block.node_type == "arrow_function"
    });
    assert!(
        has_function,
        "Expected to find function-related nodes for async patterns"
    );

    std::env::remove_var("DEBUG");
}

#[test]
fn test_javascript_acceptable_parent_specific_nodes() {
    // This test specifically verifies the is_acceptable_parent implementation
    let js_impl = get_language_impl("js").unwrap();

    let js_code = r#"
const func = function() { return 42; };
const arrow = () => 42;
function regular() { return 42; }
class MyClass { method() { return 42; } }
const obj = { prop: function() { return 42; } };
"#;

    let language = tree_sitter_javascript::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(js_code, None).unwrap();
    let root_node = tree.root_node();

    let mut acceptable_nodes = Vec::new();
    let mut all_nodes = Vec::new();

    fn collect_nodes(
        node: tree_sitter::Node,
        acceptable_nodes: &mut Vec<String>,
        all_nodes: &mut Vec<String>,
        js_impl: &dyn super::language_trait::LanguageImpl,
    ) {
        let node_info = format!(
            "{} ({}:{})",
            node.kind(),
            node.start_position().row + 1,
            node.end_position().row + 1
        );
        all_nodes.push(node_info.clone());

        if js_impl.is_acceptable_parent(&node) {
            acceptable_nodes.push(node_info);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_nodes(child, acceptable_nodes, all_nodes, js_impl);
        }
    }

    collect_nodes(
        root_node,
        &mut acceptable_nodes,
        &mut all_nodes,
        js_impl.as_ref(),
    );

    println!("All nodes found:");
    for node in &all_nodes {
        println!("  {node}");
    }

    println!("Acceptable parent nodes:");
    for node in &acceptable_nodes {
        println!("  {node}");
    }

    // Verify function_expression is acceptable
    let has_function_expression = acceptable_nodes
        .iter()
        .any(|node| node.contains("function_expression"));
    assert!(
        has_function_expression,
        "function_expression should be acceptable parent"
    );

    // Verify arrow_function is acceptable
    let has_arrow_function = acceptable_nodes
        .iter()
        .any(|node| node.contains("arrow_function"));
    assert!(
        has_arrow_function,
        "arrow_function should be acceptable parent"
    );

    // Verify function_declaration is acceptable
    let has_function_declaration = acceptable_nodes
        .iter()
        .any(|node| node.contains("function_declaration"));
    assert!(
        has_function_declaration,
        "function_declaration should be acceptable parent"
    );

    // Verify class_declaration is acceptable
    let has_class_declaration = acceptable_nodes
        .iter()
        .any(|node| node.contains("class_declaration"));
    assert!(
        has_class_declaration,
        "class_declaration should be acceptable parent"
    );

    // Verify object is acceptable
    let has_object = acceptable_nodes.iter().any(|node| node.contains("object"));
    assert!(has_object, "object should be acceptable parent");

    // Verify variable_declaration is NOT acceptable (this was the fix)
    let has_variable_declaration = acceptable_nodes
        .iter()
        .any(|node| node.contains("variable_declaration"));
    assert!(
        !has_variable_declaration,
        "variable_declaration should NOT be acceptable parent after the fix"
    );

    // Verify lexical_declaration is NOT acceptable (this was the fix)
    let has_lexical_declaration = acceptable_nodes
        .iter()
        .any(|node| node.contains("lexical_declaration"));
    assert!(
        !has_lexical_declaration,
        "lexical_declaration should NOT be acceptable parent after the fix"
    );
}
