use super::*;
use tree_sitter::Parser as TSParser;

// Helper function to parse test code and get the root node
fn parse_rust_code(code: &str) -> tree_sitter::Tree {
    let mut parser = TSParser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).expect("Error loading Rust grammar");
    parser.parse(code, None).expect("Failed to parse code")
}

// Helper function to find a comment node in the tree
fn find_comment_node(code: &str, line: usize) -> Option<tree_sitter::Node<'_>> {
    let tree = parse_rust_code(code);
    let root_node = tree.root_node();
    let target_node = find_most_specific_node(root_node, line);
    
    if target_node.kind() == "line_comment" || target_node.kind() == "block_comment" {
        Some(target_node)
    } else {
        None
    }
}

#[test]
fn test_comment_within_function() {
    let code = r#"
fn example_function() {
    // Inside comment
    let x = 42;
}
"#;
    
    let comment = find_comment_node(code, 3).expect("Failed to find comment");
    let related = find_related_code_node(comment, "rs").expect("Failed to find related node");
    
    assert_eq!(related.kind(), "function_item");
    assert_eq!(related.start_position().row + 1, 2); // Function starts on line 2
}

#[test]
fn test_comment_within_struct() {
    let code = r#"
struct Example {
    // Field comment
    field: i32
}
"#;
    
    let comment = find_comment_node(code, 3).expect("Failed to find comment");
    let related = find_related_code_node(comment, "rs").expect("Failed to find related node");
    
    assert_eq!(related.kind(), "struct_item");
    assert_eq!(related.start_position().row + 1, 2); // Struct starts on line 2
}

#[test]
fn test_root_level_comments() {
    let code = r#"
// First root comment
// Second root comment

fn first_function() {
    let x = 1;
}
"#;
    
    let comment = find_comment_node(code, 2).expect("Failed to find comment");
    let related = find_related_code_node(comment, "rs").expect("Failed to find related node");
    
    // Should find the function as the next significant node
    assert_eq!(related.kind(), "function_item");
    assert_eq!(related.start_position().row + 1, 4); // Function starts on line 4
}

#[test]
fn test_adjacent_comments() {
    let code = r#"
// First comment
// Second comment
fn example() {}
"#;
    
    let first_comment = find_comment_node(code, 2).expect("Failed to find first comment");
    let second_comment = find_comment_node(code, 3).expect("Failed to find second comment");
    
    let first_related = find_related_code_node(first_comment, "rs").expect("Failed to find related node");
    let second_related = find_related_code_node(second_comment, "rs").expect("Failed to find related node");
    
    // Both comments should be associated with the function
    assert_eq!(first_related.kind(), "function_item");
    assert_eq!(second_related.kind(), "function_item");
    assert_eq!(first_related.start_position().row, second_related.start_position().row);
}

#[test]
fn test_mixed_scope_comments() {
    let code = r#"
// Root comment 1

struct Example {
    // Struct field comment
    field: i32,
    
    // Method comment
    fn method() {
        // Inside method comment
        let x = 1;
    }
}

// Root comment 2
"#;
    
    // Test root comment 1
    let root_comment1 = find_comment_node(code, 2).expect("Failed to find root comment 1");
    let related1 = find_related_code_node(root_comment1, "rs").expect("Failed to find related node");
    assert_eq!(related1.kind(), "struct_item");
    
    // Test struct field comment
    let field_comment = find_comment_node(code, 5).expect("Failed to find field comment");
    let related2 = find_related_code_node(field_comment, "rs").expect("Failed to find related node");
    assert_eq!(related2.kind(), "struct_item");
    
    // Test method comment
    let method_comment = find_comment_node(code, 8).expect("Failed to find method comment");
    let related3 = find_related_code_node(method_comment, "rs").expect("Failed to find related node");
    assert_eq!(related3.kind(), "function_item");
    
    // Test inside method comment
    let inner_comment = find_comment_node(code, 10).expect("Failed to find inner comment");
    let related4 = find_related_code_node(inner_comment, "rs").expect("Failed to find related node");
    assert_eq!(related4.kind(), "function_item");
}

#[test]
fn test_adjacent_structs() {
    let code = r#"
// First struct comment
struct FirstStruct {
    field1: i32
}

// Second struct comment
struct SecondStruct {
    field2: i32
}
"#;
    
    // Test first comment - should only relate to FirstStruct
    let first_comment = find_comment_node(code, 2).expect("Failed to find first comment");
    let first_related = find_related_code_node(first_comment, "rs").expect("Failed to find related node");
    assert_eq!(first_related.kind(), "struct_item");
    assert_eq!(first_related.start_position().row + 1, 3); // FirstStruct starts on line 3
    
    // Test second comment - should only relate to SecondStruct
    let second_comment = find_comment_node(code, 7).expect("Failed to find second comment");
    let second_related = find_related_code_node(second_comment, "rs").expect("Failed to find related node");
    assert_eq!(second_related.kind(), "struct_item");
    assert_eq!(second_related.start_position().row + 1, 8); // SecondStruct starts on line 8
    
    // Verify the two structs are different
    assert_ne!(first_related.start_position().row, second_related.start_position().row);
}

#[test]
fn test_complex_nesting() {
    let code = r#"
mod example {
    // Module-level comment
    
    struct Inner {
        // Struct comment
        field: i32,
        
        fn method() {
            // Method comment
            let x = 1;
        }
    }
    
    // Another module-level comment
}
"#;
    
    // Test module-level comment
    let mod_comment = find_comment_node(code, 3).expect("Failed to find module comment");
    let related1 = find_related_code_node(mod_comment, "rs").expect("Failed to find related node");
    assert_eq!(related1.kind(), "mod_item");
    
    // Test struct comment
    let struct_comment = find_comment_node(code, 6).expect("Failed to find struct comment");
    let related2 = find_related_code_node(struct_comment, "rs").expect("Failed to find related node");
    assert_eq!(related2.kind(), "struct_item");
    
    // Test method comment
    let method_comment = find_comment_node(code, 10).expect("Failed to find method comment");
    let related3 = find_related_code_node(method_comment, "rs").expect("Failed to find related node");
    assert_eq!(related3.kind(), "function_item");
}