//! Language-Specific UID Generation Rules
//!
//! This module defines language-specific rules and behaviors for UID generation.
//! Each language has different conventions for scoping, overloading, visibility,
//! and symbol naming that affect how UIDs should be generated.

use serde::{Deserialize, Serialize};

/// Signature normalization strategies for different languages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureNormalization {
    /// Use signature as-is without modification
    None,
    /// Remove parameter names, keep only types
    RemoveParameterNames,
    /// Normalize type representations (e.g., "int" -> "i32")
    CanonicalTypes,
    /// Complete normalization including whitespace and parameter names
    Full,
}

/// Language-specific rules for UID generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageRules {
    /// Separator used between scope elements (e.g., "::" for C++, "." for Java)
    pub scope_separator: String,

    /// Prefix used for anonymous symbols (e.g., "anon" for C++, "lambda" for Python)
    pub anonymous_prefix: String,

    /// Whether this language supports function/method overloading
    pub supports_overloading: bool,

    /// Whether symbol names are case-sensitive
    pub case_sensitive: bool,

    /// How to normalize signatures for this language
    pub signature_normalization: SignatureNormalization,

    /// Whether visibility modifiers affect UID generation
    pub visibility_affects_uid: bool,

    /// Default visibility for symbols without explicit visibility
    pub default_visibility: String,

    /// File extensions associated with this language
    pub file_extensions: Vec<String>,

    /// Keywords that might appear in signatures that should be normalized
    pub signature_keywords: Vec<String>,

    /// Type aliases that should be normalized (e.g., "string" -> "String")
    pub type_aliases: Vec<(String, String)>,
}

impl LanguageRules {
    /// Create rules for Rust
    pub fn rust() -> Self {
        Self {
            scope_separator: "::".to_string(),
            anonymous_prefix: "anon".to_string(),
            supports_overloading: false, // Rust doesn't support function overloading
            case_sensitive: true,
            signature_normalization: SignatureNormalization::RemoveParameterNames,
            visibility_affects_uid: false, // pub/private doesn't change the symbol identity
            default_visibility: "private".to_string(),
            file_extensions: vec!["rs".to_string()],
            signature_keywords: vec![
                "fn".to_string(),
                "pub".to_string(),
                "const".to_string(),
                "static".to_string(),
                "mut".to_string(),
                "unsafe".to_string(),
                "async".to_string(),
            ],
            type_aliases: vec![
                ("str".to_string(), "&str".to_string()),
                ("String".to_string(), "std::string::String".to_string()),
            ],
        }
    }

    /// Create rules for TypeScript
    pub fn typescript() -> Self {
        Self {
            scope_separator: ".".to_string(),
            anonymous_prefix: "anon".to_string(),
            supports_overloading: true, // TypeScript supports function overloading
            case_sensitive: true,
            signature_normalization: SignatureNormalization::Full,
            visibility_affects_uid: false,
            default_visibility: "public".to_string(),
            file_extensions: vec!["ts".to_string(), "tsx".to_string()],
            signature_keywords: vec![
                "function".to_string(),
                "async".to_string(),
                "export".to_string(),
                "default".to_string(),
                "public".to_string(),
                "private".to_string(),
                "protected".to_string(),
                "readonly".to_string(),
                "static".to_string(),
            ],
            type_aliases: vec![
                ("number".to_string(), "number".to_string()),
                ("string".to_string(), "string".to_string()),
                ("boolean".to_string(), "boolean".to_string()),
            ],
        }
    }

    /// Create rules for JavaScript  
    pub fn javascript() -> Self {
        Self {
            scope_separator: ".".to_string(),
            anonymous_prefix: "anon".to_string(),
            supports_overloading: false, // JavaScript doesn't have true overloading
            case_sensitive: true,
            signature_normalization: SignatureNormalization::RemoveParameterNames,
            visibility_affects_uid: false,
            default_visibility: "public".to_string(),
            file_extensions: vec!["js".to_string(), "jsx".to_string(), "mjs".to_string()],
            signature_keywords: vec![
                "function".to_string(),
                "async".to_string(),
                "export".to_string(),
                "default".to_string(),
                "const".to_string(),
                "let".to_string(),
                "var".to_string(),
            ],
            type_aliases: vec![],
        }
    }

    /// Create rules for Python
    pub fn python() -> Self {
        Self {
            scope_separator: ".".to_string(),
            anonymous_prefix: "lambda".to_string(),
            supports_overloading: false, // Python doesn't support method overloading (uses default args)
            case_sensitive: true,
            signature_normalization: SignatureNormalization::RemoveParameterNames,
            visibility_affects_uid: false, // Python uses naming convention, not keywords
            default_visibility: "public".to_string(),
            file_extensions: vec!["py".to_string(), "pyx".to_string(), "pyi".to_string()],
            signature_keywords: vec![
                "def".to_string(),
                "async".to_string(),
                "class".to_string(),
                "self".to_string(),
                "cls".to_string(),
                "staticmethod".to_string(),
                "classmethod".to_string(),
                "property".to_string(),
            ],
            type_aliases: vec![
                ("str".to_string(), "str".to_string()),
                ("int".to_string(), "int".to_string()),
                ("float".to_string(), "float".to_string()),
                ("bool".to_string(), "bool".to_string()),
            ],
        }
    }

    /// Create rules for Go
    pub fn go() -> Self {
        Self {
            scope_separator: ".".to_string(),
            anonymous_prefix: "anon".to_string(),
            supports_overloading: false, // Go doesn't support function overloading
            case_sensitive: true,
            signature_normalization: SignatureNormalization::RemoveParameterNames,
            visibility_affects_uid: false, // Go uses capitalization for visibility
            default_visibility: "private".to_string(),
            file_extensions: vec!["go".to_string()],
            signature_keywords: vec![
                "func".to_string(),
                "type".to_string(),
                "struct".to_string(),
                "interface".to_string(),
                "const".to_string(),
                "var".to_string(),
            ],
            type_aliases: vec![
                ("string".to_string(), "string".to_string()),
                ("int".to_string(), "int".to_string()),
                ("bool".to_string(), "bool".to_string()),
                ("byte".to_string(), "uint8".to_string()),
                ("rune".to_string(), "int32".to_string()),
            ],
        }
    }

    /// Create rules for Java
    pub fn java() -> Self {
        Self {
            scope_separator: ".".to_string(),
            anonymous_prefix: "anon".to_string(),
            supports_overloading: true, // Java supports method overloading
            case_sensitive: true,
            signature_normalization: SignatureNormalization::Full,
            visibility_affects_uid: false, // public/private doesn't change symbol identity
            default_visibility: "package".to_string(),
            file_extensions: vec!["java".to_string()],
            signature_keywords: vec![
                "public".to_string(),
                "private".to_string(),
                "protected".to_string(),
                "static".to_string(),
                "final".to_string(),
                "abstract".to_string(),
                "synchronized".to_string(),
                "native".to_string(),
                "strictfp".to_string(),
                "volatile".to_string(),
                "transient".to_string(),
            ],
            type_aliases: vec![
                ("String".to_string(), "java.lang.String".to_string()),
                ("Object".to_string(), "java.lang.Object".to_string()),
                ("Integer".to_string(), "java.lang.Integer".to_string()),
            ],
        }
    }

    /// Create rules for C
    pub fn c() -> Self {
        Self {
            scope_separator: "::".to_string(),
            anonymous_prefix: "anon".to_string(),
            supports_overloading: false, // C doesn't support function overloading
            case_sensitive: true,
            signature_normalization: SignatureNormalization::CanonicalTypes,
            visibility_affects_uid: false,
            default_visibility: "public".to_string(), // C functions are global by default
            file_extensions: vec!["c".to_string(), "h".to_string()],
            signature_keywords: vec![
                "static".to_string(),
                "extern".to_string(),
                "inline".to_string(),
                "const".to_string(),
                "volatile".to_string(),
                "restrict".to_string(),
                "typedef".to_string(),
            ],
            type_aliases: vec![
                ("size_t".to_string(), "unsigned long".to_string()),
                ("ptrdiff_t".to_string(), "long".to_string()),
            ],
        }
    }

    /// Create rules for C++
    pub fn cpp() -> Self {
        Self {
            scope_separator: "::".to_string(),
            anonymous_prefix: "anon".to_string(),
            supports_overloading: true, // C++ supports function overloading
            case_sensitive: true,
            signature_normalization: SignatureNormalization::Full,
            visibility_affects_uid: false,
            default_visibility: "private".to_string(), // C++ class members are private by default
            file_extensions: vec![
                "cpp".to_string(),
                "cxx".to_string(),
                "cc".to_string(),
                "hpp".to_string(),
                "hxx".to_string(),
                "h".to_string(),
            ],
            signature_keywords: vec![
                "public".to_string(),
                "private".to_string(),
                "protected".to_string(),
                "virtual".to_string(),
                "static".to_string(),
                "const".to_string(),
                "mutable".to_string(),
                "inline".to_string(),
                "explicit".to_string(),
                "constexpr".to_string(),
                "noexcept".to_string(),
                "override".to_string(),
                "final".to_string(),
            ],
            type_aliases: vec![
                ("string".to_string(), "std::string".to_string()),
                ("vector".to_string(), "std::vector".to_string()),
                ("map".to_string(), "std::map".to_string()),
            ],
        }
    }

    /// Check if this language supports a specific feature
    pub fn supports_feature(&self, feature: &str) -> bool {
        match feature {
            "overloading" => self.supports_overloading,
            "case_sensitive" => self.case_sensitive,
            "visibility_uid" => self.visibility_affects_uid,
            _ => false,
        }
    }

    /// Get the normalized type name for this language
    pub fn normalize_type(&self, type_name: &str) -> String {
        // Check type aliases first
        for (alias, canonical) in &self.type_aliases {
            if type_name == alias {
                return canonical.clone();
            }
        }

        // Apply case normalization if needed
        if !self.case_sensitive {
            type_name.to_lowercase()
        } else {
            type_name.to_string()
        }
    }

    /// Check if a symbol name follows the language's anonymous naming convention
    pub fn is_anonymous_name(&self, name: &str) -> bool {
        name.starts_with(&self.anonymous_prefix)
            || name.contains('@')
            || name.contains('$')
            || (name.starts_with("lambda") && self.anonymous_prefix == "lambda")
            || name.starts_with("__anon")
    }

    /// Get the file extension priority for this language (higher = more specific)
    pub fn get_extension_priority(&self, extension: &str) -> u8 {
        match extension {
            ext if self.file_extensions.contains(&ext.to_string()) => {
                // More specific extensions get higher priority
                match ext {
                    "tsx" | "jsx" => 10, // React specific
                    "pyi" => 10,         // Python interface files
                    "hpp" | "hxx" => 10, // C++ headers
                    "cxx" | "cc" => 8,   // C++ source variants
                    "mjs" => 8,          // ES modules
                    "pyx" => 8,          // Cython
                    _ => 5,              // Standard extensions
                }
            }
            _ => 0, // Not supported
        }
    }

    /// Determine if two signatures are equivalent in this language
    pub fn signatures_equivalent(&self, sig1: &str, sig2: &str) -> bool {
        if !self.supports_overloading {
            // If language doesn't support overloading, signature doesn't matter for identity
            return true;
        }

        // Normalize both signatures and compare
        let norm1 = self.normalize_signature_internal(sig1);
        let norm2 = self.normalize_signature_internal(sig2);
        norm1 == norm2
    }

    /// Internal signature normalization
    fn normalize_signature_internal(&self, signature: &str) -> String {
        let mut normalized = signature.trim().to_string();

        // Remove language keywords that don't affect signature identity
        for keyword in &self.signature_keywords {
            normalized = normalized.replace(&format!("{} ", keyword), " ");
            normalized = normalized.replace(&format!(" {}", keyword), " ");
        }

        // Normalize whitespace
        normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");

        // Apply type aliases
        for (alias, canonical) in &self.type_aliases {
            normalized = normalized.replace(alias, canonical);
        }

        match self.signature_normalization {
            SignatureNormalization::None => signature.to_string(),
            SignatureNormalization::RemoveParameterNames => {
                self.remove_parameter_names(&normalized)
            }
            SignatureNormalization::CanonicalTypes => self.canonicalize_types(&normalized),
            SignatureNormalization::Full => {
                let without_params = self.remove_parameter_names(&normalized);
                self.canonicalize_types(&without_params)
            }
        }
    }

    /// Remove parameter names from signature, keeping only types
    fn remove_parameter_names(&self, signature: &str) -> String {
        // This is a simplified implementation - a full implementation would use
        // language-specific parsers
        match self.scope_separator.as_str() {
            "::" => self.remove_cpp_parameter_names(signature),
            "." => self.remove_java_python_parameter_names(signature),
            _ => signature.to_string(),
        }
    }

    /// Remove parameter names for C++ style signatures
    fn remove_cpp_parameter_names(&self, signature: &str) -> String {
        // Simplified: remove identifiers after type keywords
        let mut result = signature.to_string();

        // Pattern: "type name" -> "type"
        let patterns = vec![
            (r"\bint\s+\w+\b", "int"),
            (r"\bdouble\s+\w+\b", "double"),
            (r"\bfloat\s+\w+\b", "float"),
            (r"\bbool\s+\w+\b", "bool"),
            (r"\bchar\s+\w+\b", "char"),
        ];

        for (pattern, replacement) in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                result = re.replace_all(&result, replacement).to_string();
            }
        }

        result
    }

    /// Remove parameter names for Java/Python style signatures
    fn remove_java_python_parameter_names(&self, signature: &str) -> String {
        // For Java-style signatures like "void method(Type name, OtherType other)"
        // we want to keep only the types: "void method(Type, OtherType)"
        let mut result = signature.to_string();

        // Simple pattern matching for common Java/Python patterns
        let patterns = vec![
            // Java: "Type paramName" -> "Type"
            (
                r"\b(int|long|short|byte|char|boolean|float|double|String|Object)\s+\w+\b",
                r"$1",
            ),
            // Generic types: "List<T> paramName" -> "List<T>"
            (r"\b(\w+(?:<[^>]*>)?)\s+\w+\b", r"$1"),
        ];

        for (pattern, replacement) in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                result = re.replace_all(&result, replacement).to_string();
            }
        }

        // Clean up multiple spaces
        result = result.split_whitespace().collect::<Vec<_>>().join(" ");
        result
    }

    /// Canonicalize type names in signature
    fn canonicalize_types(&self, signature: &str) -> String {
        let mut result = signature.to_string();

        // Apply type aliases
        for (alias, canonical) in &self.type_aliases {
            result = result.replace(alias, canonical);
        }

        result
    }
}

/// Factory for creating language rules
pub struct LanguageRulesFactory;

impl LanguageRulesFactory {
    /// Create rules for a language by name
    pub fn create_rules(language: &str) -> Option<LanguageRules> {
        match language.to_lowercase().as_str() {
            "rust" | "rs" => Some(LanguageRules::rust()),
            "typescript" | "ts" => Some(LanguageRules::typescript()),
            "javascript" | "js" => Some(LanguageRules::javascript()),
            "python" | "py" => Some(LanguageRules::python()),
            "go" => Some(LanguageRules::go()),
            "java" => Some(LanguageRules::java()),
            "c" => Some(LanguageRules::c()),
            "cpp" | "c++" | "cxx" => Some(LanguageRules::cpp()),
            _ => None,
        }
    }

    /// Get all supported languages
    pub fn supported_languages() -> Vec<String> {
        vec![
            "rust".to_string(),
            "typescript".to_string(),
            "javascript".to_string(),
            "python".to_string(),
            "go".to_string(),
            "java".to_string(),
            "c".to_string(),
            "cpp".to_string(),
        ]
    }

    /// Check if a language is supported
    pub fn is_supported(language: &str) -> bool {
        Self::create_rules(language).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_rules_creation() {
        let rust_rules = LanguageRules::rust();
        assert_eq!(rust_rules.scope_separator, "::");
        assert!(!rust_rules.supports_overloading);
        assert!(rust_rules.case_sensitive);

        let java_rules = LanguageRules::java();
        assert_eq!(java_rules.scope_separator, ".");
        assert!(java_rules.supports_overloading);

        let python_rules = LanguageRules::python();
        assert_eq!(python_rules.anonymous_prefix, "lambda");
    }

    #[test]
    fn test_language_features() {
        let cpp_rules = LanguageRules::cpp();
        assert!(cpp_rules.supports_feature("overloading"));
        assert!(cpp_rules.supports_feature("case_sensitive"));
        assert!(!cpp_rules.supports_feature("unknown_feature"));
    }

    #[test]
    fn test_type_normalization() {
        let cpp_rules = LanguageRules::cpp();
        assert_eq!(cpp_rules.normalize_type("string"), "std::string");
        assert_eq!(cpp_rules.normalize_type("unknown"), "unknown");

        let java_rules = LanguageRules::java();
        assert_eq!(java_rules.normalize_type("String"), "java.lang.String");
    }

    #[test]
    fn test_anonymous_name_detection() {
        let python_rules = LanguageRules::python();
        assert!(python_rules.is_anonymous_name("lambda123"));
        assert!(python_rules.is_anonymous_name("test@456"));
        assert!(!python_rules.is_anonymous_name("normal_function"));

        let cpp_rules = LanguageRules::cpp();
        assert!(cpp_rules.is_anonymous_name("anon_class"));
        assert!(cpp_rules.is_anonymous_name("__anon_function"));
    }

    #[test]
    fn test_extension_priority() {
        let ts_rules = LanguageRules::typescript();
        assert_eq!(ts_rules.get_extension_priority("tsx"), 10);
        assert_eq!(ts_rules.get_extension_priority("ts"), 5);
        assert_eq!(ts_rules.get_extension_priority("unknown"), 0);
    }

    #[test]
    fn test_signature_equivalence() {
        let java_rules = LanguageRules::java();

        // Java supports overloading, so different signatures are not equivalent
        assert!(!java_rules.signatures_equivalent("void method(int a)", "void method(String a)"));
        assert!(java_rules.signatures_equivalent("void method(int a)", "void method(int b)")); // Parameter names don't matter

        let rust_rules = LanguageRules::rust();
        // Rust doesn't support overloading, so signatures are equivalent for UID purposes
        assert!(rust_rules.signatures_equivalent("fn test(a: i32)", "fn test(b: String)"));
    }

    #[test]
    fn test_language_rules_factory() {
        assert!(LanguageRulesFactory::is_supported("rust"));
        assert!(LanguageRulesFactory::is_supported("TypeScript"));
        assert!(LanguageRulesFactory::is_supported("C++"));
        assert!(!LanguageRulesFactory::is_supported("unknown"));

        let supported = LanguageRulesFactory::supported_languages();
        assert!(supported.contains(&"rust".to_string()));
        assert!(supported.contains(&"java".to_string()));

        let rust_rules = LanguageRulesFactory::create_rules("rust").unwrap();
        assert_eq!(rust_rules.scope_separator, "::");
    }

    #[test]
    fn test_signature_normalization_strategies() {
        let rules = LanguageRules::cpp();

        let signature = "  public   static   void   method  ( int  param )  ";
        let normalized = rules.normalize_signature_internal(signature);

        // Should remove extra whitespace and normalize
        assert!(!normalized.contains("  ")); // No double spaces
        assert!(normalized.len() < signature.len()); // Should be shorter
    }
}
