//! Symbol Name and Signature Normalization
#![allow(dead_code, clippy::all)]
//!
//! This module provides comprehensive normalization functions for symbol names,
//! qualified names, and signatures across different programming languages.
//! Normalization ensures consistent UID generation for semantically equivalent symbols.

use super::{UIDError, UIDResult};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

/// Regular expressions for signature parsing, cached for performance
static SIGNATURE_PATTERNS: Lazy<HashMap<&'static str, Regex>> = Lazy::new(|| {
    let mut patterns = HashMap::new();

    // Rust patterns
    patterns.insert(
        "rust_function",
        Regex::new(r"fn\s+(\w+)\s*\([^)]*\)(?:\s*->\s*([^{]+))?").unwrap(),
    );
    patterns.insert("rust_params", Regex::new(r"\b(\w+):\s*([^,)]+)").unwrap());
    patterns.insert("rust_generics", Regex::new(r"<[^>]+>").unwrap());

    // Java/TypeScript patterns
    patterns.insert(
        "java_method",
        Regex::new(r"(?:public|private|protected)?\s*(?:static)?\s*(\w+)\s+(\w+)\s*\([^)]*\)")
            .unwrap(),
    );
    patterns.insert(
        "java_params",
        Regex::new(r"(\w+)\s+(\w+)(?:\s*,|\s*\))").unwrap(),
    );

    // C++ patterns
    patterns.insert(
        "cpp_function",
        Regex::new(r"(?:virtual|static|inline)?\s*(\w+)\s+(\w+)\s*\([^)]*\)(?:\s*const)?").unwrap(),
    );
    patterns.insert(
        "cpp_params",
        Regex::new(r"(?:const\s+)?(\w+)(?:\s*[&*]+)?\s+(\w+)").unwrap(),
    );

    // Python patterns
    patterns.insert(
        "python_def",
        Regex::new(r"def\s+(\w+)\s*\([^)]*\)(?:\s*->\s*([^:]+))?").unwrap(),
    );
    patterns.insert(
        "python_params",
        Regex::new(r"\b(\w+)(?::\s*([^,)]+))?").unwrap(),
    );

    // Go patterns
    patterns.insert(
        "go_func",
        Regex::new(r"func(?:\s+\([^)]*\))?\s+(\w+)\s*\([^)]*\)(?:\s*\([^)]*\)|\s*\w+)?").unwrap(),
    );
    patterns.insert(
        "go_params",
        Regex::new(r"(\w+)(?:,\s*\w+)*\s+([^,)]+)").unwrap(),
    );

    patterns
});

/// Whitespace normalization patterns
static WHITESPACE_PATTERNS: Lazy<HashMap<&'static str, Regex>> = Lazy::new(|| {
    let mut patterns = HashMap::new();
    patterns.insert("multiple_spaces", Regex::new(r"\s+").unwrap());
    patterns.insert(
        "around_operators",
        Regex::new(r"\s*([<>(){}\[\],;:])\s*").unwrap(),
    );
    patterns.insert("leading_trailing", Regex::new(r"^\s+|\s+$").unwrap());
    patterns
});

/// Main normalizer for symbol names and signatures
pub struct Normalizer {
    /// Cache for normalized results to improve performance
    normalization_cache: HashMap<String, String>,
}

impl Normalizer {
    /// Create a new normalizer
    pub fn new() -> Self {
        Self {
            normalization_cache: HashMap::new(),
        }
    }

    /// Normalize a symbol name for consistent UID generation
    pub fn normalize_symbol_name(&self, name: &str, language: &str) -> UIDResult<String> {
        if name.is_empty() {
            return Err(UIDError::InvalidSymbol(
                "Symbol name cannot be empty".to_string(),
            ));
        }

        let mut normalized = name.trim().to_string();

        // Language-specific name normalization
        match language.to_lowercase().as_str() {
            "rust" => normalized = self.normalize_rust_name(&normalized)?,
            "typescript" | "javascript" => normalized = self.normalize_js_ts_name(&normalized)?,
            "python" => normalized = self.normalize_python_name(&normalized)?,
            "go" => normalized = self.normalize_go_name(&normalized)?,
            "java" => normalized = self.normalize_java_name(&normalized)?,
            "c" | "cpp" | "c++" => normalized = self.normalize_c_cpp_name(&normalized)?,
            _ => {} // Use name as-is for unsupported languages
        }

        // Remove any leading/trailing special characters
        normalized = normalized
            .trim_matches(|c: char| c.is_whitespace() || c == '_')
            .to_string();

        if normalized.is_empty() {
            return Err(UIDError::NormalizationError {
                component: "symbol_name".to_string(),
                error: format!("Normalization resulted in empty name for input: '{}'", name),
            });
        }

        Ok(normalized)
    }

    /// Normalize a fully qualified name (FQN) and split into components
    pub fn split_qualified_name(&self, fqn: &str, language: &str) -> UIDResult<Vec<String>> {
        if fqn.is_empty() {
            return Err(UIDError::InvalidSymbol(
                "Qualified name cannot be empty".to_string(),
            ));
        }

        // Determine separator based on language
        let separator = match language.to_lowercase().as_str() {
            "rust" | "c" | "cpp" | "c++" => "::",
            "java" | "typescript" | "javascript" | "python" | "go" => ".",
            _ => "::", // Default to C++ style
        };

        let mut parts: Vec<String> = fqn
            .split(separator)
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect();

        if parts.is_empty() {
            return Err(UIDError::NormalizationError {
                component: "qualified_name".to_string(),
                error: format!("No valid components found in FQN: '{}'", fqn),
            });
        }

        // Normalize each component
        for part in &mut parts {
            *part = self.normalize_symbol_name(part, language)?;
        }

        Ok(parts)
    }

    /// Normalize a function/method signature
    pub fn normalize_signature(&self, signature: &str, language: &str) -> UIDResult<String> {
        if signature.is_empty() {
            return Ok(String::new());
        }

        // Input validation for malformed signatures
        if signature.len() > 10000 {
            return Err(UIDError::NormalizationError {
                component: "signature".to_string(),
                error: format!("Signature too long: {} characters", signature.len()),
            });
        }

        let mut normalized = signature.trim().to_string();

        // Check for obviously malformed signatures and provide fallback
        if normalized
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(UIDError::NormalizationError {
                component: "signature".to_string(),
                error: "Signature contains invalid control characters".to_string(),
            });
        }

        // Language-specific signature normalization
        match language.to_lowercase().as_str() {
            "rust" => normalized = self.normalize_rust_signature(&normalized)?,
            "typescript" | "javascript" => {
                normalized = self.normalize_js_ts_signature(&normalized)?
            }
            "python" => normalized = self.normalize_python_signature(&normalized)?,
            "go" => normalized = self.normalize_go_signature(&normalized)?,
            "java" => normalized = self.normalize_java_signature(&normalized)?,
            "c" | "cpp" | "c++" => normalized = self.normalize_c_cpp_signature(&normalized)?,
            _ => normalized = self.normalize_generic_signature(&normalized)?,
        }

        // Final whitespace cleanup
        normalized = self.normalize_whitespace(&normalized);

        Ok(normalized)
    }

    /// Normalize a type name for consistent representation
    pub fn normalize_typename(&self, type_name: &str, language: &str) -> UIDResult<String> {
        if type_name.is_empty() {
            return Ok(String::new());
        }

        let mut normalized = type_name.trim().to_string();

        // Language-specific type normalization
        match language.to_lowercase().as_str() {
            "rust" => {
                // Handle Rust-specific type normalization
                normalized = normalized.replace("&mut ", "&mut");
                normalized = normalized.replace("& ", "&");

                // Normalize common Rust types
                let type_map = vec![
                    ("str", "&str"),
                    ("String", "String"),
                    ("i8", "i8"),
                    ("i16", "i16"),
                    ("i32", "i32"),
                    ("i64", "i64"),
                    ("u8", "u8"),
                    ("u16", "u16"),
                    ("u32", "u32"),
                    ("u64", "u64"),
                    ("f32", "f32"),
                    ("f64", "f64"),
                    ("bool", "bool"),
                    ("char", "char"),
                    ("usize", "usize"),
                    ("isize", "isize"),
                ];

                for (from, to) in type_map {
                    if normalized == from {
                        normalized = to.to_string();
                        break;
                    }
                }
            }
            "java" => {
                // Normalize Java types to full qualified names where appropriate
                let type_map = vec![
                    ("String", "java.lang.String"),
                    ("Object", "java.lang.Object"),
                    ("Integer", "java.lang.Integer"),
                    ("Double", "java.lang.Double"),
                    ("Boolean", "java.lang.Boolean"),
                    ("List", "java.util.List"),
                    ("Map", "java.util.Map"),
                    ("Set", "java.util.Set"),
                ];

                for (from, to) in type_map {
                    if normalized == from {
                        normalized = to.to_string();
                        break;
                    }
                }
            }
            "typescript" | "javascript" => {
                // Normalize TypeScript/JavaScript types
                let type_map = vec![
                    ("number", "number"),
                    ("string", "string"),
                    ("boolean", "boolean"),
                    ("object", "object"),
                    ("any", "any"),
                    ("void", "void"),
                    ("null", "null"),
                    ("undefined", "undefined"),
                ];

                for (from, to) in type_map {
                    if normalized == from {
                        normalized = to.to_string();
                        break;
                    }
                }
            }
            "python" => {
                // Normalize Python types
                let type_map = vec![
                    ("str", "str"),
                    ("int", "int"),
                    ("float", "float"),
                    ("bool", "bool"),
                    ("list", "list"),
                    ("dict", "dict"),
                    ("tuple", "tuple"),
                    ("set", "set"),
                ];

                for (from, to) in type_map {
                    if normalized == from {
                        normalized = to.to_string();
                        break;
                    }
                }
            }
            "go" => {
                // Normalize Go types
                let type_map = vec![
                    ("string", "string"),
                    ("int", "int"),
                    ("int8", "int8"),
                    ("int16", "int16"),
                    ("int32", "int32"),
                    ("int64", "int64"),
                    ("uint", "uint"),
                    ("uint8", "uint8"),
                    ("uint16", "uint16"),
                    ("uint32", "uint32"),
                    ("uint64", "uint64"),
                    ("float32", "float32"),
                    ("float64", "float64"),
                    ("bool", "bool"),
                    ("byte", "uint8"), // byte is alias for uint8
                    ("rune", "int32"), // rune is alias for int32
                ];

                for (from, to) in type_map {
                    if normalized == from {
                        normalized = to.to_string();
                        break;
                    }
                }
            }
            "c" | "cpp" | "c++" => {
                // Normalize C/C++ types
                normalized = normalized.replace("unsigned int", "unsigned");
                normalized = normalized.replace("signed int", "int");
                normalized = normalized.replace("long int", "long");
                normalized = normalized.replace("short int", "short");

                let type_map = vec![
                    ("std::string", "std::string"),
                    ("std::vector", "std::vector"),
                    ("std::map", "std::map"),
                    ("std::set", "std::set"),
                    ("size_t", "size_t"),
                ];

                for (from, to) in type_map {
                    if normalized.contains(from) {
                        normalized = normalized.replace(from, to);
                    }
                }
            }
            _ => {} // Use as-is for unknown languages
        }

        Ok(normalized)
    }

    // Language-specific normalization methods

    fn normalize_rust_name(&self, name: &str) -> UIDResult<String> {
        // Rust names are generally well-formed, just trim underscores
        let normalized = name
            .trim_start_matches('_')
            .trim_end_matches('_')
            .to_string();
        if normalized.is_empty() {
            return Err(UIDError::NormalizationError {
                component: "rust_name".to_string(),
                error: format!("Name consists only of underscores: '{}'", name),
            });
        }
        Ok(normalized)
    }

    fn normalize_js_ts_name(&self, name: &str) -> UIDResult<String> {
        // Handle JavaScript/TypeScript naming conventions
        let mut normalized = name.to_string();

        // Remove TypeScript type annotations if present
        if let Some(pos) = normalized.find(':') {
            normalized = normalized[..pos].trim().to_string();
        }

        Ok(normalized)
    }

    fn normalize_python_name(&self, name: &str) -> UIDResult<String> {
        // Python name normalization
        let normalized = name
            .trim_start_matches('_')
            .trim_end_matches('_')
            .to_string();
        if normalized.is_empty() {
            return Err(UIDError::NormalizationError {
                component: "python_name".to_string(),
                error: format!("Name consists only of underscores: '{}'", name),
            });
        }
        Ok(normalized)
    }

    fn normalize_go_name(&self, name: &str) -> UIDResult<String> {
        // Go names are straightforward
        Ok(name.to_string())
    }

    fn normalize_java_name(&self, name: &str) -> UIDResult<String> {
        // Java name normalization
        Ok(name.to_string())
    }

    fn normalize_c_cpp_name(&self, name: &str) -> UIDResult<String> {
        // C/C++ name normalization - handle operator overloading
        if name.starts_with("operator") {
            return Ok(name.to_string()); // Keep operator names as-is
        }
        Ok(name.to_string())
    }

    // Signature normalization methods

    fn normalize_rust_signature(&self, signature: &str) -> UIDResult<String> {
        if let Some(pattern) = SIGNATURE_PATTERNS.get("rust_function") {
            match pattern.captures(signature) {
                Some(captures) => {
                    let mut normalized = String::new();

                    // Add function name
                    if let Some(name) = captures.get(1) {
                        normalized.push_str("fn ");
                        normalized.push_str(name.as_str());
                    } else {
                        // Fallback if function name not captured
                        return Ok(self.normalize_whitespace(signature));
                    }

                    // Normalize parameters with error handling
                    normalized.push('(');
                    match self.extract_and_normalize_rust_params(signature) {
                        Ok(params) => normalized.push_str(&params),
                        Err(_) => {
                            // Fallback to simpler normalization if parameter extraction fails
                            return Ok(self.normalize_whitespace(signature));
                        }
                    }
                    normalized.push(')');

                    // Add return type if present
                    if let Some(ret_type) = captures.get(2) {
                        normalized.push_str(" -> ");
                        match self.normalize_typename(ret_type.as_str().trim(), "rust") {
                            Ok(normalized_type) => normalized.push_str(&normalized_type),
                            Err(_) => normalized.push_str(ret_type.as_str().trim()),
                        }
                    }

                    return Ok(normalized);
                }
                None => {
                    // Pattern didn't match - fall through to generic normalization
                }
            }
        }

        // Fallback to basic whitespace normalization
        Ok(self.normalize_whitespace(signature))
    }

    fn normalize_js_ts_signature(&self, signature: &str) -> UIDResult<String> {
        let mut normalized = signature.to_string();

        // Remove access modifiers and keywords
        let keywords = [
            "export",
            "default",
            "async",
            "function",
            "public",
            "private",
            "protected",
            "static",
        ];
        for keyword in &keywords {
            normalized = normalized.replace(&format!("{} ", keyword), " ");
        }

        // Normalize arrow functions
        if normalized.contains("=>") {
            normalized = normalized.replace("=>", " => ");
        }

        Ok(self.normalize_whitespace(&normalized))
    }

    fn normalize_python_signature(&self, signature: &str) -> UIDResult<String> {
        if let Some(pattern) = SIGNATURE_PATTERNS.get("python_def") {
            if let Some(captures) = pattern.captures(signature) {
                let mut normalized = String::new();

                // Add function name
                if let Some(name) = captures.get(1) {
                    normalized.push_str("def ");
                    normalized.push_str(name.as_str());
                }

                // Normalize parameters
                normalized.push('(');
                let params = self.extract_and_normalize_python_params(signature)?;
                normalized.push_str(&params);
                normalized.push(')');

                // Add return annotation if present
                if let Some(ret_type) = captures.get(2) {
                    normalized.push_str(" -> ");
                    normalized
                        .push_str(&self.normalize_typename(ret_type.as_str().trim(), "python")?);
                }

                return Ok(normalized);
            }
        }

        Ok(signature.to_string())
    }

    fn normalize_go_signature(&self, signature: &str) -> UIDResult<String> {
        // Go signature normalization
        let mut normalized = signature.to_string();

        // Remove receiver if present (for methods)
        if let Some(start) = normalized.find("func") {
            normalized = normalized[start..].to_string();
        }

        Ok(self.normalize_whitespace(&normalized))
    }

    fn normalize_java_signature(&self, signature: &str) -> UIDResult<String> {
        let mut normalized = signature.to_string();

        // Remove access modifiers
        let modifiers = [
            "public",
            "private",
            "protected",
            "static",
            "final",
            "abstract",
            "synchronized",
        ];
        for modifier in &modifiers {
            normalized = normalized.replace(&format!("{} ", modifier), " ");
        }

        Ok(self.normalize_whitespace(&normalized))
    }

    fn normalize_c_cpp_signature(&self, signature: &str) -> UIDResult<String> {
        let mut normalized = signature.to_string();

        // Remove C++ keywords that don't affect function identity
        let keywords = ["virtual", "override", "final", "inline", "static", "extern"];
        for keyword in &keywords {
            // Remove keyword followed by space
            normalized = normalized.replace(&format!("{} ", keyword), " ");
            // Remove keyword at the end of string
            if normalized.ends_with(keyword) {
                normalized = normalized[..normalized.len() - keyword.len()]
                    .trim()
                    .to_string();
            }
            // Remove keyword at the beginning of string
            if normalized.starts_with(&format!("{} ", keyword)) {
                normalized = normalized[keyword.len() + 1..].to_string();
            }
        }

        // Handle const methods
        if normalized.ends_with(" const") {
            normalized = format!("{} const", normalized.trim_end_matches(" const"));
        }

        Ok(self.normalize_whitespace(&normalized))
    }

    fn normalize_generic_signature(&self, signature: &str) -> UIDResult<String> {
        // Generic normalization for unknown languages
        Ok(self.normalize_whitespace(signature))
    }

    // Helper methods

    fn extract_and_normalize_rust_params(&self, signature: &str) -> UIDResult<String> {
        if let Some(pattern) = SIGNATURE_PATTERNS.get("rust_params") {
            let mut params = Vec::new();

            for captures in pattern.captures_iter(signature) {
                if let (Some(name), Some(type_str)) = (captures.get(1), captures.get(2)) {
                    match self.normalize_typename(type_str.as_str().trim(), "rust") {
                        Ok(normalized_type) => {
                            params.push(format!("{}: {}", name.as_str(), normalized_type));
                        }
                        Err(_) => {
                            // If type normalization fails, use the original type string
                            params.push(format!("{}: {}", name.as_str(), type_str.as_str().trim()));
                        }
                    }
                }
            }

            return Ok(params.join(", "));
        }

        // If pattern not found, return empty string (graceful degradation)
        Ok(String::new())
    }

    fn extract_and_normalize_python_params(&self, signature: &str) -> UIDResult<String> {
        // Extract parameters between parentheses
        if let Some(start) = signature.find('(') {
            if let Some(end) = signature.rfind(')') {
                let param_str = &signature[start + 1..end];
                let params: Vec<&str> = param_str.split(',').map(|p| p.trim()).collect();

                let mut normalized_params = Vec::new();
                for param in params {
                    if param.is_empty() || param == "self" || param == "cls" {
                        continue;
                    }

                    // Handle type annotations
                    if let Some(colon_pos) = param.find(':') {
                        let name = param[..colon_pos].trim();
                        let type_str = param[colon_pos + 1..].trim();
                        let normalized_type = self.normalize_typename(type_str, "python")?;
                        normalized_params.push(format!("{}: {}", name, normalized_type));
                    } else {
                        normalized_params.push(param.to_string());
                    }
                }

                return Ok(normalized_params.join(", "));
            }
        }

        Ok(String::new())
    }

    fn normalize_whitespace(&self, text: &str) -> String {
        if let Some(pattern) = WHITESPACE_PATTERNS.get("multiple_spaces") {
            let normalized = pattern.replace_all(text, " ");
            let normalized = normalized.trim();
            return normalized.to_string();
        }

        text.trim().to_string()
    }
}

impl Default for Normalizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_name_normalization() {
        let normalizer = Normalizer::new();

        // Rust names
        assert_eq!(
            normalizer
                .normalize_symbol_name("_private_func", "rust")
                .unwrap(),
            "private_func"
        );
        assert_eq!(
            normalizer
                .normalize_symbol_name("__internal__", "rust")
                .unwrap(),
            "internal"
        );

        // JavaScript/TypeScript names
        assert_eq!(
            normalizer
                .normalize_symbol_name("myFunc: () => void", "typescript")
                .unwrap(),
            "myFunc"
        );

        // Error cases
        assert!(normalizer.normalize_symbol_name("", "rust").is_err());
        assert!(normalizer.normalize_symbol_name("____", "rust").is_err());
    }

    #[test]
    fn test_qualified_name_splitting() {
        let normalizer = Normalizer::new();

        // Rust FQN
        let rust_parts = normalizer
            .split_qualified_name("std::collections::HashMap", "rust")
            .unwrap();
        assert_eq!(rust_parts, vec!["std", "collections", "HashMap"]);

        // Java FQN
        let java_parts = normalizer
            .split_qualified_name("com.example.service.UserService", "java")
            .unwrap();
        assert_eq!(java_parts, vec!["com", "example", "service", "UserService"]);

        // Error cases
        assert!(normalizer.split_qualified_name("", "rust").is_err());
        assert!(normalizer.split_qualified_name("::", "rust").is_err());
    }

    #[test]
    fn test_type_normalization() {
        let normalizer = Normalizer::new();

        // Rust types
        assert_eq!(
            normalizer.normalize_typename("&mut str", "rust").unwrap(),
            "&mutstr"
        );
        assert_eq!(
            normalizer.normalize_typename("String", "rust").unwrap(),
            "String"
        );

        // Java types
        assert_eq!(
            normalizer.normalize_typename("String", "java").unwrap(),
            "java.lang.String"
        );
        assert_eq!(
            normalizer.normalize_typename("List", "java").unwrap(),
            "java.util.List"
        );

        // Go types
        assert_eq!(
            normalizer.normalize_typename("byte", "go").unwrap(),
            "uint8"
        );
        assert_eq!(
            normalizer.normalize_typename("rune", "go").unwrap(),
            "int32"
        );
    }

    #[test]
    fn test_signature_normalization() {
        let normalizer = Normalizer::new();

        // Rust signature
        let rust_sig = "fn calculate(x: i32, y: i32) -> i32";
        let normalized_rust = normalizer.normalize_signature(rust_sig, "rust").unwrap();
        assert!(normalized_rust.contains("fn"));
        assert!(normalized_rust.contains("calculate"));

        // Java signature
        let java_sig = "public static void main(String[] args)";
        let normalized_java = normalizer.normalize_signature(java_sig, "java").unwrap();
        assert!(!normalized_java.contains("public")); // Should remove access modifiers
        assert!(!normalized_java.contains("static"));

        // Empty signature
        assert_eq!(normalizer.normalize_signature("", "rust").unwrap(), "");
    }

    #[test]
    fn test_whitespace_normalization() {
        let normalizer = Normalizer::new();

        let text = "  function   test  (  param1  ,  param2  )  ";
        let normalized = normalizer.normalize_whitespace(text);

        assert!(!normalized.starts_with(' '));
        assert!(!normalized.ends_with(' '));
        assert!(!normalized.contains("  ")); // No double spaces
    }

    #[test]
    fn test_rust_parameter_extraction() {
        let normalizer = Normalizer::new();

        let signature = "fn test(x: i32, y: &str, z: Vec<String>) -> bool";
        let params = normalizer
            .extract_and_normalize_rust_params(signature)
            .unwrap();

        assert!(params.contains("x: i32"));
        assert!(params.contains("y: &str"));
        assert!(params.contains("z: Vec<String>"));
    }

    #[test]
    fn test_python_parameter_extraction() {
        let normalizer = Normalizer::new();

        let signature = "def process(self, data: str, count: int = 10) -> List[str]";
        let params = normalizer
            .extract_and_normalize_python_params(signature)
            .unwrap();

        // Should not include 'self'
        assert!(!params.contains("self"));
        assert!(params.contains("data: str"));
        assert!(params.contains("count: int"));
    }

    #[test]
    fn test_language_specific_patterns() {
        let normalizer = Normalizer::new();

        // Test C++ const methods
        let cpp_sig = "virtual int getValue() const override";
        let normalized = normalizer.normalize_signature(cpp_sig, "cpp").unwrap();
        assert!(!normalized.contains("virtual"));
        assert!(!normalized.contains("override"));
        assert!(normalized.contains("const")); // const should be preserved for method identity

        // Test TypeScript arrow functions
        let ts_sig = "export const myFunc = (x: number) => boolean";
        let normalized = normalizer
            .normalize_signature(ts_sig, "typescript")
            .unwrap();
        assert!(!normalized.contains("export"));
        assert!(normalized.contains("=>"));
    }
}
