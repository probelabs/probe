use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

/// Implementation of LanguageImpl for Python
pub struct PythonLanguage;

impl Default for PythonLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonLanguage {
    pub fn new() -> Self {
        PythonLanguage
    }
}

impl LanguageImpl for PythonLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_python::LANGUAGE.into()
    }

    fn get_extension(&self) -> &'static str {
        "py"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "function_definition"
                | "class_definition"
                | "module"
                | "assignment"
                | "expression_statement"
                | "block"
                | "decorated_definition"
        )
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
        let node_type = node.kind();

        // Python: Check function_definition nodes with names starting with test_
        if node_type == "function_definition" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = child.utf8_text(source).unwrap_or("");
                    if name.starts_with("test_") {
                        if debug_mode {
                            println!("DEBUG: Test node detected (Python): test_ function");
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
            "function_definition" => {
                // Extract function signature without body
                // Find the colon and extract everything before it, then add colon
                if let Some(name) = node.child_by_field_name("name") {
                    let mut sig = String::new();
                    
                    // Check for async keyword
                    let mut cursor = node.walk();
                    let mut is_async = false;
                    for child in node.children(&mut cursor) {
                        if child.kind() == "async" {
                            is_async = true;
                            break;
                        }
                    }
                    
                    if is_async {
                        sig.push_str("async ");
                    }
                    
                    sig.push_str("def ");
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));
                    
                    // Add parameters
                    if let Some(params) = node.child_by_field_name("parameters") {
                        let params_text = &source[params.start_byte()..params.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(params_text));
                    }
                    
                    // Add return type if present
                    if let Some(return_type) = node.child_by_field_name("return_type") {
                        sig.push_str(" -> ");
                        let return_text = &source[return_type.start_byte()..return_type.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(return_text));
                    }
                    
                    sig.push(':');
                    Some(sig)
                } else {
                    None
                }
            }
            "class_definition" => {
                // Extract class signature
                if let Some(name) = node.child_by_field_name("name") {
                    let mut sig = String::from("class ");
                    let name_text = &source[name.start_byte()..name.end_byte()];
                    sig.push_str(&String::from_utf8_lossy(name_text));
                    
                    // Add superclasses if present
                    if let Some(superclasses) = node.child_by_field_name("superclasses") {
                        let super_text = &source[superclasses.start_byte()..superclasses.end_byte()];
                        sig.push_str(&String::from_utf8_lossy(super_text));
                    }
                    
                    sig.push(':');
                    Some(sig)
                } else {
                    None
                }
            }
            "assignment" => {
                // Extract variable/constant assignments (module level)
                // Only extract if it looks like a constant or important variable
                let assignment_text = &source[node.start_byte()..node.end_byte()];
                let assignment_str = String::from_utf8_lossy(assignment_text);
                
                // Check if it's a simple assignment (not a complex expression)
                if assignment_str.lines().count() == 1 {
                    // Find the = and only show the left side for constants/variables
                    if let Some(eq_pos) = assignment_str.find('=') {
                        let left_side = assignment_str[..eq_pos].trim();
                        // Only show if it looks like a constant (uppercase) or important variable
                        if left_side.chars().any(|c| c.is_uppercase()) || left_side.contains('_') {
                            Some(format!("{} = ...", left_side))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "decorated_definition" => {
                // Handle decorated functions/classes
                // Find the actual definition inside and get its signature
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "function_definition" || child.kind() == "class_definition" {
                        if let Some(inner_sig) = self.get_symbol_signature(&child, source) {
                            // Extract decorators
                            let mut decorators = Vec::new();
                            let mut inner_cursor = node.walk();
                            for decorator_child in node.children(&mut inner_cursor) {
                                if decorator_child.kind() == "decorator" {
                                    let dec_text = &source[decorator_child.start_byte()..decorator_child.end_byte()];
                                    decorators.push(String::from_utf8_lossy(dec_text).to_string());
                                }
                            }
                            
                            if !decorators.is_empty() {
                                let mut sig = decorators.join("\n");
                                sig.push('\n');
                                sig.push_str(&inner_sig);
                                return Some(sig);
                            } else {
                                return Some(inner_sig);
                            }
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}
