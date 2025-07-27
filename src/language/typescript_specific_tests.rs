use super::factory::get_language_impl;

/// Quick verification that TypeScript implementation has the same improvements as JavaScript
#[test]
fn test_typescript_acceptable_parent_specific_nodes() {
    // This test specifically verifies the is_acceptable_parent implementation for TypeScript
    let ts_impl = get_language_impl("ts").unwrap();

    let ts_code = r#"
const func: () => number = function() { return 42; };
const arrow = (): number => 42;
function regular(): number { return 42; }
class MyClass { method(): number { return 42; } }
interface MyInterface { prop: number; }
"#;

    let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(ts_code, None).unwrap();
    let root_node = tree.root_node();

    let mut acceptable_nodes = Vec::new();

    fn collect_nodes(
        node: tree_sitter::Node,
        acceptable_nodes: &mut Vec<String>,
        ts_impl: &dyn super::language_trait::LanguageImpl,
    ) {
        let node_info = format!(
            "{} ({}:{})",
            node.kind(),
            node.start_position().row + 1,
            node.end_position().row + 1
        );

        if ts_impl.is_acceptable_parent(&node) {
            acceptable_nodes.push(node_info);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_nodes(child, acceptable_nodes, ts_impl);
        }
    }

    collect_nodes(root_node, &mut acceptable_nodes, ts_impl.as_ref());

    println!("TypeScript acceptable parent nodes:");
    for node in &acceptable_nodes {
        println!("  {}", node);
    }

    // Verify function_expression is acceptable
    let has_function_expression = acceptable_nodes
        .iter()
        .any(|node| node.contains("function_expression"));
    assert!(
        has_function_expression,
        "function_expression should be acceptable parent in TypeScript"
    );

    // Verify arrow_function is acceptable
    let has_arrow_function = acceptable_nodes
        .iter()
        .any(|node| node.contains("arrow_function"));
    assert!(
        has_arrow_function,
        "arrow_function should be acceptable parent in TypeScript"
    );

    // Verify function_declaration is acceptable
    let has_function_declaration = acceptable_nodes
        .iter()
        .any(|node| node.contains("function_declaration"));
    assert!(
        has_function_declaration,
        "function_declaration should be acceptable parent in TypeScript"
    );

    // Verify interface_declaration is acceptable (TypeScript-specific)
    let has_interface_declaration = acceptable_nodes
        .iter()
        .any(|node| node.contains("interface_declaration"));
    assert!(
        has_interface_declaration,
        "interface_declaration should be acceptable parent in TypeScript"
    );

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
