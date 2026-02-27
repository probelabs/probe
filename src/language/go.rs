use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Go
pub struct GoLanguage;

impl Default for GoLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl GoLanguage {
    pub fn new() -> Self {
        GoLanguage
    }
}

impl LanguageImpl for GoLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_go::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "go"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "function_declaration" |
            "method_declaration" |
            "type_declaration" |
            "struct_type" |
            "interface_type" |
            // "const_declaration" |
            // "var_declaration" |
            // "const_spec" |
            // "var_spec" |
            // "short_var_declaration" |
            "type_spec" // Added for type definitions
        )
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
        let node_type = node.kind();

        // Go: Check function_declaration nodes with names starting with Test
        if node_type == "function_declaration" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = child.utf8_text(source).unwrap_or("");
                    if name.starts_with("Test") {
                        if debug_mode {
                            println!("DEBUG: Test node detected (Go): Test function");
                        }
                        return true;
                    }
                }
            }
        }

        false
    }

    fn get_receiver_type(&self, node: &Node, source: &[u8]) -> Option<String> {
        if node.kind() != "method_declaration" {
            return None;
        }

        // Go method_declaration AST:
        //   method_declaration
        //     parameter_list (receiver)
        //       parameter_declaration
        //         identifier "r"
        //         pointer_type | type_identifier
        //           type_identifier "TypeName"
        //     field_identifier "MethodName"
        //     ...

        // The first parameter_list child is the receiver
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "parameter_list" {
                // This is the receiver parameter list
                let mut param_cursor = child.walk();
                for param in child.children(&mut param_cursor) {
                    if param.kind() == "parameter_declaration" {
                        // Look for the type in this parameter
                        let mut type_cursor = param.walk();
                        for type_child in param.children(&mut type_cursor) {
                            match type_child.kind() {
                                "type_identifier" => {
                                    // Value receiver: func (r Type) Method()
                                    return type_child.utf8_text(source).ok().map(String::from);
                                }
                                "pointer_type" => {
                                    // Pointer receiver: func (r *Type) Method()
                                    let mut ptr_cursor = type_child.walk();
                                    for ptr_child in type_child.children(&mut ptr_cursor) {
                                        if ptr_child.kind() == "type_identifier"
                                            || ptr_child.kind() == "generic_type"
                                        {
                                            // For generic_type, extract just the base type name
                                            if ptr_child.kind() == "generic_type" {
                                                let mut gen_cursor = ptr_child.walk();
                                                for gen_child in ptr_child.children(&mut gen_cursor)
                                                {
                                                    if gen_child.kind() == "type_identifier" {
                                                        return gen_child
                                                            .utf8_text(source)
                                                            .ok()
                                                            .map(String::from);
                                                    }
                                                }
                                            }
                                            return ptr_child
                                                .utf8_text(source)
                                                .ok()
                                                .map(String::from);
                                        }
                                    }
                                }
                                "generic_type" => {
                                    // Generic value receiver: func (r Type[K, V]) Method()
                                    let mut gen_cursor = type_child.walk();
                                    for gen_child in type_child.children(&mut gen_cursor) {
                                        if gen_child.kind() == "type_identifier" {
                                            return gen_child
                                                .utf8_text(source)
                                                .ok()
                                                .map(String::from);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                // Only check the first parameter_list (the receiver)
                break;
            }
        }

        None
    }

    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "function_declaration" => {
                // Extract: func name(params) return_type
                let mut name = None;
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        name = child.utf8_text(source).ok();
                        break;
                    }
                }
                name.map(String::from)
            }
            "method_declaration" => {
                // Extract: receiver_type.method_name
                let receiver_type = self.get_receiver_type(node, source);
                let mut method_name = None;
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "field_identifier" {
                        method_name = child.utf8_text(source).ok();
                        break;
                    }
                }
                match (receiver_type, method_name) {
                    (Some(recv), Some(method)) => Some(format!("{recv}.{method}")),
                    (None, Some(method)) => Some(method.to_string()),
                    _ => None,
                }
            }
            "type_declaration" | "type_spec" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_identifier" || child.kind() == "identifier" {
                        return child.utf8_text(source).ok().map(String::from);
                    }
                    // For type_declaration, look inside type_spec
                    if child.kind() == "type_spec" {
                        let mut inner_cursor = child.walk();
                        for inner_child in child.children(&mut inner_cursor) {
                            if inner_child.kind() == "type_identifier" {
                                return inner_child.utf8_text(source).ok().map(String::from);
                            }
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn find_parent_function<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        if debug_mode {
            println!(
                "DEBUG: Finding parent function for {node_kind}",
                node_kind = node.kind()
            );
        }

        let mut current = node;

        while let Some(parent) = current.parent() {
            if parent.kind() == "function_declaration" || parent.kind() == "method_declaration" {
                if debug_mode {
                    println!(
                        "DEBUG: Found parent function: {parent_kind}",
                        parent_kind = parent.kind()
                    );
                }
                return Some(parent);
            }
            current = parent;
        }

        if debug_mode {
            println!(
                "DEBUG: No parent function found for {node_kind}",
                node_kind = node.kind()
            );
        }

        None
    }
}
