use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for JavaScript
pub struct JavaScriptLanguage;

impl Default for JavaScriptLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaScriptLanguage {
    pub fn new() -> Self {
        JavaScriptLanguage
    }
}

impl LanguageImpl for JavaScriptLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "js"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "function_declaration"
                | "object"
                | "array"
                | "function_expression"
                | "method_definition"
                | "class_declaration"
                | "arrow_function"
                | "function"
                | "export_statement"
                | "jsx_element"
                | "jsx_self_closing_element"
                | "property_identifier"
                | "class_body"
                | "class"
                | "expression_statement"
                | "block"
                | "statement_block"
        )
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
        let node_type = node.kind();

        // JavaScript: Check for describe/it/test blocks
        if node_type == "function_declaration"
            || node_type == "method_definition"
            || node_type == "arrow_function"
        {
            let mut cursor = node.walk();

            // Try to find the function/method name
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = child.utf8_text(source).unwrap_or("");
                    if name.contains("test") || name.contains("Test") {
                        if debug_mode {
                            println!(
                                "DEBUG: Test node detected (JavaScript): test function/method"
                            );
                        }
                        return true;
                    }
                }
            }
        }

        // Check for call expressions like describe(), it(), test()
        if node_type == "call_expression" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = child.utf8_text(source).unwrap_or("");
                    if name == "describe" || name == "it" || name == "test" || name == "expect" {
                        if debug_mode {
                            println!("DEBUG: Test node detected (JavaScript): {name} call");
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
            "function_declaration" => {
                // Extract function signature without body
                if let Some(name) = node.child_by_field_name("name") {
                    let mut sig = String::from("function ");
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));

                    // Add parameters
                    if let Some(params) = node.child_by_field_name("parameters") {
                        let params_text = &source[params.start_byte()..params.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(params_text));
                    }

                    sig.push_str(" { ... }");
                    Some(sig)
                } else {
                    None
                }
            }
            "arrow_function" => {
                // Extract arrow function signature
                let mut sig = String::new();

                // Check if it's a const/let/var assignment
                if let Some(parent) = node.parent() {
                    if parent.kind() == "variable_declarator" {
                        if let Some(name) = parent.child_by_field_name("name") {
                            sig.push_str("const ");
                            let name_text = &source[name.start_byte()..name.end_byte()];
                            sig.push_str(&String::from_utf8_lossy(name_text));
                            sig.push_str(" = ");
                        }
                    }
                }

                // Add parameters
                if let Some(params) = node.child_by_field_name("parameters") {
                    let params_text = &source[params.start_byte()..params.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(params_text));
                } else if let Some(param) = node.child_by_field_name("parameter") {
                    let param_text = &source[param.start_byte()..param.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(param_text));
                }

                sig.push_str(" => { ... }");
                Some(sig)
            }
            "function_expression" => {
                // Extract function expression signature
                let mut sig = String::from("function");

                // Add name if present
                if let Some(name) = node.child_by_field_name("name") {
                    sig.push(' ');
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));
                }

                // Add parameters
                if let Some(params) = node.child_by_field_name("parameters") {
                    let params_text = &source[params.start_byte()..params.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(params_text));
                }

                sig.push_str(" { ... }");
                Some(sig)
            }
            "class_declaration" => {
                // Extract class signature
                if let Some(name) = node.child_by_field_name("name") {
                    let mut sig = String::from("class ");
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));

                    // Add superclass if present
                    if let Some(superclass) = node.child_by_field_name("superclass") {
                        sig.push_str(" extends ");
                        let super_text = &source[superclass.start_byte()..superclass.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(super_text));
                    }

                    sig.push_str(" { ... }");
                    Some(sig)
                } else {
                    None
                }
            }
            "method_definition" => {
                // Extract method signature
                let mut sig = String::new();

                // Check for static, async, or other modifiers
                let mut cursor = node.walk();
                let mut modifiers = Vec::new();

                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "static" | "async" | "get" | "set" => {
                            let mod_text = &source[child.start_byte()..child.end_byte()];
                            modifiers.push(String::from_utf8_lossy(mod_text).to_string());
                        }
                        _ => {}
                    }
                }

                if !modifiers.is_empty() {
                    sig.push_str(&modifiers.join(" "));
                    sig.push(' ');
                }

                // Add method name
                if let Some(name) = node.child_by_field_name("name") {
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));
                }

                // Add parameters
                if let Some(params) = node.child_by_field_name("parameters") {
                    let params_text = &source[params.start_byte()..params.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(params_text));
                }

                sig.push_str(" { ... }");
                Some(sig)
            }
            "variable_declarator" => {
                // Extract variable/constant declarations
                if let Some(name) = node.child_by_field_name("name") {
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    let name_str = String::from_utf8_lossy(name_text);

                    // Only show if it looks like a constant or important variable
                    if name_str.chars().any(|c| c.is_uppercase()) || name_str.contains('_') {
                        // Get the declaration kind (const, let, var) from parent
                        if let Some(parent) = node.parent() {
                            if parent.kind() == "variable_declaration" {
                                if let Some(kind_node) = parent.children(&mut parent.walk()).next()
                                {
                                    let kind_text =
                                        &source[kind_node.start_byte()..kind_node.end_byte()];
                                    let kind = String::from_utf8_lossy(kind_text);
                                    return Some(format!("{} {} = ...", kind, name_str));
                                }
                            }
                        }
                        Some(format!("{} = ...", name_str))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "export_statement" => {
                // Handle export statements
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "function_declaration" | "class_declaration" | "variable_declaration" => {
                            if let Some(inner_sig) = self.get_symbol_signature(&child, source) {
                                return Some(format!("export {}", inner_sig));
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }
}
