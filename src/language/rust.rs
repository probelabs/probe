use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Rust
pub struct RustLanguage;

impl Default for RustLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl RustLanguage {
    pub fn new() -> Self {
        RustLanguage
    }
}

impl LanguageImpl for RustLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "rs"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        let debug_mode = std::env::var("PROBE_DEBUG").unwrap_or_default() == "1";

        // Check for standard Rust items
        if matches!(
            node.kind(),
            "function_item"
                | "struct_item"
                | "impl_item"
                | "trait_item"
                | "enum_item"
                | "mod_item"
                | "macro_definition"
        ) {
            return true;
        }

        // For expression_statement nodes, we need to find the parent function
        if node.kind() == "expression_statement" {
            if debug_mode {
                println!(
                    "DEBUG: Found expression_statement at lines {}-{}",
                    node.start_position().row + 1,
                    node.end_position().row + 1
                );
            }

            // Instead of returning true directly, we'll look for the parent function
            // and return that node in the parser.rs code
            return false;
        }

        // Special handling for token trees inside macros
        if node.kind() == "token_tree" {
            // Check if this token tree is inside a macro invocation
            if let Some(parent) = node.parent() {
                if parent.kind() == "macro_invocation" {
                    let debug_mode = std::env::var("PROBE_DEBUG").unwrap_or_default() == "1";

                    // For Rust property tests, we want to consider token trees inside macros
                    // as acceptable parents, especially for proptest! macros
                    if debug_mode {
                        println!(
                            "DEBUG: Found token_tree in macro_invocation at lines {}-{}",
                            node.start_position().row + 1,
                            node.end_position().row + 1
                        );
                    }

                    // We previously tried to use the file path as a heuristic,
                    // but we don't have access to the actual file path here

                    // If the token tree is large enough (contains multiple lines of code),
                    // it's likely a meaningful code block that should be extracted
                    let node_size = node.end_position().row - node.start_position().row;
                    if node_size > 5 {
                        if debug_mode {
                            println!(
                                "DEBUG: Considering large token_tree in macro as acceptable parent (size: {node_size} lines)"
                            );
                        }
                        return true;
                    }
                }
            }
        }

        false
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("PROBE_DEBUG").unwrap_or_default() == "1";
        let node_type = node.kind();

        // Rust: Check for #[test] attribute on function_item nodes
        if node_type == "function_item" {
            let mut cursor = node.walk();
            let mut has_test_attribute = false;

            // Look for attribute nodes
            for child in node.children(&mut cursor) {
                if child.kind() == "attribute_item" {
                    let attr_text = child.utf8_text(source).unwrap_or("");
                    if attr_text.contains("#[test") {
                        has_test_attribute = true;
                        break;
                    }
                }
            }

            if has_test_attribute {
                if debug_mode {
                    println!("DEBUG: Test node detected (Rust): #[test] attribute");
                }
                return true;
            }

            // Also check function name starting with "test_"
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = child.utf8_text(source).unwrap_or("");
                    if name.starts_with("test_") {
                        if debug_mode {
                            println!("DEBUG: Test node detected (Rust): test_ function");
                        }
                        return true;
                    }
                }
            }
        }

        false
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "function_item" => {
                // Extract function signature without body
                // Find the block node and extract everything before it
                if let Some(block) = node.child_by_field_name("body") {
                    let sig_end = block.start_byte();
                    let sig = &source[node.start_byte()..sig_end];
                    let sig_str = String::from_utf8_lossy(sig).trim().to_string();
                    // Remove trailing { if present
                    Some(sig_str.trim_end_matches('{').trim().to_string())
                } else {
                    // For function declarations without body
                    let sig = &source[node.start_byte()..node.end_byte()];
                    Some(String::from_utf8_lossy(sig).trim().to_string())
                }
            }
            "struct_item" => {
                // Extract struct signature
                // For structs, we want the struct name and generic parameters
                if let Some(name) = node.child_by_field_name("name") {
                    let mut sig = String::new();

                    // Add visibility if present
                    if let Some(vis) = node.child_by_field_name("visibility") {
                        let vis_text = &source[vis.start_byte()..vis.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(vis_text));
                        sig.push(' ');
                    }

                    sig.push_str("struct ");
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));

                    // Add generic parameters if present
                    if let Some(generics) = node.child_by_field_name("type_parameters") {
                        let gen_text = &source[generics.start_byte()..generics.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(gen_text));
                    }

                    // Add field summary
                    if let Some(body) = node.child_by_field_name("body") {
                        if body.kind() == "field_declaration_list" {
                            sig.push_str(" { ... }");
                        }
                    }

                    Some(sig)
                } else {
                    None
                }
            }
            "impl_item" => {
                // Extract impl signature
                let mut sig = String::new();

                // Check for impl keyword
                sig.push_str("impl");

                // Add generic parameters if present
                if let Some(generics) = node.child_by_field_name("type_parameters") {
                    let gen_text = &source[generics.start_byte()..generics.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(gen_text));
                }

                // Add type
                if let Some(type_node) = node.child_by_field_name("type") {
                    sig.push(' ');
                    let type_text = &source[type_node.start_byte()..type_node.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(type_text));
                }

                // Add trait if present (for trait implementations)
                if let Some(trait_node) = node.child_by_field_name("trait") {
                    sig.push_str(" for ");
                    let trait_text = &source[trait_node.start_byte()..trait_node.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(trait_text));
                }

                sig.push_str(" { ... }");
                Some(sig)
            }
            "trait_item" => {
                // Extract trait signature
                if let Some(name) = node.child_by_field_name("name") {
                    let mut sig = String::new();

                    // Add visibility if present
                    if let Some(vis) = node.child_by_field_name("visibility") {
                        let vis_text = &source[vis.start_byte()..vis.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(vis_text));
                        sig.push(' ');
                    }

                    sig.push_str("trait ");
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));

                    // Add generic parameters if present
                    if let Some(generics) = node.child_by_field_name("type_parameters") {
                        let gen_text = &source[generics.start_byte()..generics.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(gen_text));
                    }

                    sig.push_str(" { ... }");
                    Some(sig)
                } else {
                    None
                }
            }
            "enum_item" => {
                // Extract enum signature
                if let Some(name) = node.child_by_field_name("name") {
                    let mut sig = String::new();

                    // Add visibility if present
                    if let Some(vis) = node.child_by_field_name("visibility") {
                        let vis_text = &source[vis.start_byte()..vis.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(vis_text));
                        sig.push(' ');
                    }

                    sig.push_str("enum ");
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));

                    // Add generic parameters if present
                    if let Some(generics) = node.child_by_field_name("type_parameters") {
                        let gen_text = &source[generics.start_byte()..generics.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(gen_text));
                    }

                    sig.push_str(" { ... }");
                    Some(sig)
                } else {
                    None
                }
            }
            "const_item" | "static_item" => {
                // Extract const/static signature without value
                let sig = &source[node.start_byte()..node.end_byte()];
                let sig_str = String::from_utf8_lossy(sig);

                // Find the = and remove everything after it
                if let Some(eq_pos) = sig_str.find('=') {
                    Some(sig_str[..eq_pos].trim().to_string())
                } else {
                    Some(sig_str.trim().to_string())
                }
            }
            "mod_item" => {
                // Extract module signature
                if let Some(name) = node.child_by_field_name("name") {
                    let mut sig = String::new();

                    // Add visibility if present
                    if let Some(vis) = node.child_by_field_name("visibility") {
                        let vis_text = &source[vis.start_byte()..vis.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(vis_text));
                        sig.push(' ');
                    }

                    sig.push_str("mod ");
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));

                    Some(sig)
                } else {
                    None
                }
            }
            "macro_definition" => {
                // Extract macro signature
                if let Some(name) = node.child_by_field_name("name") {
                    let mut sig = String::from("macro_rules! ");
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));
                    sig.push_str(" { ... }");
                    Some(sig)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
