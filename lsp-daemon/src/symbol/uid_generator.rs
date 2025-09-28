//! Core UID Generation Engine
//!
//! This module implements the main `SymbolUIDGenerator` that creates stable, unique identifiers
//! for symbols across programming languages. The generator follows a hierarchical approach
//! based on symbol characteristics and language-specific rules.

use super::{SymbolContext, SymbolInfo, SymbolKind, UIDError, UIDResult};
use crate::symbol::language_support::LanguageRules;
use crate::symbol::normalization::Normalizer;
use blake3::Hasher as Blake3Hasher;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Convert file extension to language name for UID generation
fn extension_to_language_name(extension: &str) -> Option<&'static str> {
    match extension.to_lowercase().as_str() {
        "rs" => Some("rust"),
        "js" | "jsx" => Some("javascript"),
        "ts" => Some("typescript"),
        "tsx" => Some("typescript"), // TSX uses TypeScript parser
        "py" => Some("python"),
        "go" => Some("go"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("cpp"),
        "java" => Some("java"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "swift" => Some("swift"),
        "cs" => Some("csharp"),
        _ => None,
    }
}

/// Hash algorithm options for UID generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    /// Blake3 - Fast, cryptographically secure (default)
    Blake3,
    /// SHA256 - Standard, widely supported
    Sha256,
}

impl Default for HashAlgorithm {
    fn default() -> Self {
        HashAlgorithm::Blake3
    }
}

/// Main UID generator with configurable algorithms and language support
pub struct SymbolUIDGenerator {
    /// Hash algorithm to use for generating UIDs
    hash_algorithm: HashAlgorithm,

    /// Language-specific rules for UID generation
    language_rules: HashMap<String, LanguageRules>,

    /// Normalizer for symbol names and signatures
    normalizer: Normalizer,
}

impl SymbolUIDGenerator {
    /// Create a new UID generator with default settings (Blake3)
    pub fn new() -> Self {
        Self {
            hash_algorithm: HashAlgorithm::default(),
            language_rules: Self::initialize_language_rules(),
            normalizer: Normalizer::new(),
        }
    }

    /// Create a UID generator with a specific hash algorithm
    pub fn with_hash_algorithm(algorithm: HashAlgorithm) -> Self {
        Self {
            hash_algorithm: algorithm,
            language_rules: Self::initialize_language_rules(),
            normalizer: Normalizer::new(),
        }
    }

    /// Initialize language-specific rules for supported languages
    fn initialize_language_rules() -> HashMap<String, LanguageRules> {
        let mut rules = HashMap::new();

        // Rust
        rules.insert("rust".to_string(), LanguageRules::rust());

        // TypeScript/JavaScript
        rules.insert("typescript".to_string(), LanguageRules::typescript());
        rules.insert("javascript".to_string(), LanguageRules::javascript());

        // Python
        rules.insert("python".to_string(), LanguageRules::python());

        // Go
        rules.insert("go".to_string(), LanguageRules::go());

        // Java
        rules.insert("java".to_string(), LanguageRules::java());

        // C/C++
        rules.insert("c".to_string(), LanguageRules::c());
        rules.insert("cpp".to_string(), LanguageRules::cpp());
        rules.insert("c++".to_string(), LanguageRules::cpp());

        rules
    }

    /// Generate a unique identifier for a symbol
    ///
    /// This is the main public API that implements the PRD UID generation algorithm:
    /// 1. Use USR if available (Clang provides this)
    /// 2. Anonymous symbols need position-based UIDs
    /// 3. Local variables need scope + position
    /// 4. Methods need class context
    /// 5. Global symbols use FQN
    pub fn generate_uid(&self, symbol: &SymbolInfo, context: &SymbolContext) -> UIDResult<String> {
        // Validate inputs
        if symbol.name.is_empty() {
            return Err(UIDError::InvalidSymbol(
                "Symbol name cannot be empty".to_string(),
            ));
        }

        if symbol.language.is_empty() {
            return Err(UIDError::InvalidSymbol(
                "Language cannot be empty".to_string(),
            ));
        }

        // Get language rules
        let rules = self.get_language_rules(&symbol.language)?;

        // Apply the PRD UID generation algorithm
        self.generate_uid_internal(symbol, context, rules)
    }

    /// Internal UID generation logic implementing the PRD algorithm
    fn generate_uid_internal(
        &self,
        symbol: &SymbolInfo,
        context: &SymbolContext,
        rules: &LanguageRules,
    ) -> UIDResult<String> {
        // 1. Use USR if available (highest priority)
        if let Some(usr) = &symbol.usr {
            return Ok(self.normalize_usr(usr, rules));
        }

        // 2. Anonymous symbols need position-based UID
        if self.is_anonymous_symbol(symbol) {
            return self.generate_anonymous_uid(symbol, context, rules);
        }

        // 3. Local variables and parameters need scope + position
        if self.is_local_symbol(symbol) {
            return self.generate_local_uid(symbol, context, rules);
        }

        // 4. Methods, constructors, destructors need class context
        if self.is_method_symbol(symbol) {
            return self.generate_method_uid(symbol, context, rules);
        }

        // 5. Global symbols use fully qualified name
        self.generate_global_uid(symbol, context, rules)
    }

    /// Check if a symbol is anonymous (lambda, closure, etc.)
    fn is_anonymous_symbol(&self, symbol: &SymbolInfo) -> bool {
        symbol.is_anonymous()
    }

    /// Check if a symbol is local (variable, parameter)
    fn is_local_symbol(&self, symbol: &SymbolInfo) -> bool {
        matches!(symbol.kind, SymbolKind::Variable | SymbolKind::Parameter)
    }

    /// Check if a symbol is a method/constructor/destructor
    fn is_method_symbol(&self, symbol: &SymbolInfo) -> bool {
        matches!(
            symbol.kind,
            SymbolKind::Method | SymbolKind::Constructor | SymbolKind::Destructor
        )
    }

    /// Generate UID for anonymous symbols (position-based)
    fn generate_anonymous_uid(
        &self,
        symbol: &SymbolInfo,
        context: &SymbolContext,
        rules: &LanguageRules,
    ) -> UIDResult<String> {
        // Format: lang::anon::<hash(file_path + position + scope)>
        let mut components = vec![symbol.language.clone()];
        components.push(rules.anonymous_prefix.clone());

        // Create a unique hash based on position and context
        let position_key = format!(
            "{}:{}:{}:{}",
            symbol.location.file_path.display(),
            symbol.location.start_line,
            symbol.location.start_char,
            context.current_scope(&rules.scope_separator)
        );

        let position_hash = self.hash_string(&position_key)?;
        components.push(position_hash);

        // Use "::" as the standard separator for UIDs, regardless of language-specific scope separators
        Ok(components.join("::"))
    }

    /// Generate UID for local symbols (scope + position based)
    fn generate_local_uid(
        &self,
        symbol: &SymbolInfo,
        context: &SymbolContext,
        _rules: &LanguageRules,
    ) -> UIDResult<String> {
        // Format: lang::scope::name#pos_hash
        let mut components = vec![symbol.language.clone()];

        // Add scope context
        if !context.scope_stack.is_empty() {
            components.extend(context.scope_stack.iter().cloned());
        } else if let Some(parent_scope) = &symbol.parent_scope {
            components.push(parent_scope.clone());
        }

        // Add symbol name (normalized)
        let normalized_name = self
            .normalizer
            .normalize_symbol_name(&symbol.name, &symbol.language)?;
        components.push(normalized_name);

        // Add position hash for uniqueness (local variables can have same name in different scopes)
        let position_key = format!(
            "{}:{}",
            symbol.location.start_line, symbol.location.start_char
        );
        let position_hash = self.hash_string(&position_key)?;

        // Use "::" as the standard separator for UIDs, regardless of language-specific scope separators
        Ok(format!(
            "{}#{}",
            components.join("::"),
            position_hash[..8].to_string()
        ))
    }

    /// Generate UID for methods (including class context)
    fn generate_method_uid(
        &self,
        symbol: &SymbolInfo,
        context: &SymbolContext,
        rules: &LanguageRules,
    ) -> UIDResult<String> {
        // Format: lang::class::method_name#signature_hash
        let mut components = vec![symbol.language.clone()];

        // Add class/struct context from FQN or scope (ignore empty/whitespace FQNs)
        if let Some(fqn) = symbol
            .qualified_name
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            let fqn_parts = self
                .normalizer
                .split_qualified_name(fqn, &symbol.language)?;
            components.extend(fqn_parts);
        } else {
            // Fallback to scope context
            components.extend(context.scope_stack.iter().cloned());
            components.push(
                self.normalizer
                    .normalize_symbol_name(&symbol.name, &symbol.language)?,
            );
        }

        // Use "::" as the standard separator for UIDs, regardless of language-specific scope separators
        let base_uid = components.join("::");

        // Add signature hash if available and language supports overloading
        if rules.supports_overloading {
            if let Some(signature) = &symbol.signature {
                let normalized_signature = self
                    .normalizer
                    .normalize_signature(signature, &symbol.language)?;
                let sig_hash = self.hash_string(&normalized_signature)?;
                return Ok(format!("{}#{}", base_uid, &sig_hash[..8]));
            }
        }

        Ok(base_uid)
    }

    /// Generate UID for global symbols (FQN-based)
    fn generate_global_uid(
        &self,
        symbol: &SymbolInfo,
        context: &SymbolContext,
        rules: &LanguageRules,
    ) -> UIDResult<String> {
        // Format: lang::fqn or lang::scope::name
        let mut components = vec![symbol.language.clone()];

        // Prefer FQN if available (ignore empty/whitespace FQNs)
        if let Some(fqn) = symbol
            .qualified_name
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            let fqn_parts = self
                .normalizer
                .split_qualified_name(fqn, &symbol.language)?;
            components.extend(fqn_parts);
        } else {
            // Construct from scope + name
            components.extend(context.scope_stack.iter().cloned());
            components.push(
                self.normalizer
                    .normalize_symbol_name(&symbol.name, &symbol.language)?,
            );
        }

        // Use "::" as the standard separator for UIDs, regardless of language-specific scope separators
        let base_uid = components.join("::");

        // Add signature hash for overloaded functions
        if rules.supports_overloading && symbol.kind.is_callable() {
            if let Some(signature) = &symbol.signature {
                let normalized_signature = self
                    .normalizer
                    .normalize_signature(signature, &symbol.language)?;
                let sig_hash = self.hash_string(&normalized_signature)?;
                return Ok(format!("{}#{}", base_uid, &sig_hash[..8]));
            }
        }

        Ok(base_uid)
    }

    /// Normalize USR (Unified Symbol Resolution) identifiers
    fn normalize_usr(&self, usr: &str, _rules: &LanguageRules) -> String {
        // USRs are already unique, but we might want to normalize the format
        usr.to_string()
    }

    /// Generate hash of a string using the configured algorithm
    fn hash_string(&self, input: &str) -> UIDResult<String> {
        match self.hash_algorithm {
            HashAlgorithm::Blake3 => {
                let mut hasher = Blake3Hasher::new();
                hasher.update(input.as_bytes());
                Ok(hasher.finalize().to_hex().to_string())
            }
            HashAlgorithm::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(input.as_bytes());
                Ok(format!("{:x}", hasher.finalize()))
            }
        }
    }

    /// Get language rules for a specific language (supports extensions and language names)
    fn get_language_rules(&self, language: &str) -> UIDResult<&LanguageRules> {
        // Convert extension to language name if needed
        let language_name = extension_to_language_name(language).unwrap_or(language);

        let lang_key = language_name.to_lowercase();
        self.language_rules
            .get(&lang_key)
            .ok_or_else(|| UIDError::UnsupportedLanguage {
                language: language.to_string(),
            })
    }

    /// Generate batch UIDs for multiple symbols (performance optimization)
    pub fn generate_batch_uids(
        &self,
        symbols: &[(SymbolInfo, SymbolContext)],
    ) -> Vec<UIDResult<String>> {
        symbols
            .iter()
            .map(|(symbol, context)| self.generate_uid(symbol, context))
            .collect()
    }

    /// Validate that a UID is properly formatted
    pub fn validate_uid(&self, uid: &str) -> bool {
        if uid.is_empty() || uid.len() < 3 || !uid.contains("::") {
            return false;
        }

        // Check for edge cases
        if uid == "::" || uid.starts_with("::") {
            return false;
        }

        // Must have at least language::something format
        let parts: Vec<&str> = uid.split("::").collect();
        parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty()
    }

    /// Extract language from a UID
    pub fn extract_language_from_uid(&self, uid: &str) -> Option<String> {
        if uid.is_empty() || !uid.contains("::") {
            return None;
        }
        uid.split("::").next().map(|s| s.to_string())
    }

    /// Get statistics about UID generation
    pub fn get_stats(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();
        stats.insert(
            "hash_algorithm".to_string(),
            format!("{:?}", self.hash_algorithm),
        );
        stats.insert(
            "supported_languages".to_string(),
            self.language_rules.len().to_string(),
        );
        stats.insert(
            "languages".to_string(),
            self.language_rules
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
        stats
    }
}

impl Default for SymbolUIDGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{SymbolKind, SymbolLocation};
    use std::path::PathBuf;

    fn create_test_symbol(name: &str, kind: SymbolKind, language: &str) -> SymbolInfo {
        let location = SymbolLocation::point(PathBuf::from("test.rs"), 10, 5);
        SymbolInfo::new(name.to_string(), kind, language.to_string(), location)
    }

    fn create_test_context() -> SymbolContext {
        SymbolContext::new(1, "rust".to_string())
            .push_scope("module".to_string())
            .push_scope("class".to_string())
    }

    #[test]
    fn test_uid_generator_creation() {
        let generator = SymbolUIDGenerator::new();
        assert_eq!(generator.hash_algorithm, HashAlgorithm::Blake3);
        assert!(!generator.language_rules.is_empty());

        let blake3_generator = SymbolUIDGenerator::with_hash_algorithm(HashAlgorithm::Blake3);
        assert_eq!(blake3_generator.hash_algorithm, HashAlgorithm::Blake3);

        let sha256_generator = SymbolUIDGenerator::with_hash_algorithm(HashAlgorithm::Sha256);
        assert_eq!(sha256_generator.hash_algorithm, HashAlgorithm::Sha256);
    }

    #[test]
    fn test_global_symbol_uid() {
        let generator = SymbolUIDGenerator::new();
        let context = create_test_context();

        let symbol = create_test_symbol("calculate_total", SymbolKind::Function, "rust")
            .with_qualified_name("accounting::billing::calculate_total".to_string());

        let uid = generator.generate_uid(&symbol, &context).unwrap();
        assert!(uid.starts_with("rust::"));
        assert!(uid.contains("accounting"));
        assert!(uid.contains("billing"));
        assert!(uid.contains("calculate_total"));
    }

    #[test]
    fn test_method_uid_with_overloading() {
        let generator = SymbolUIDGenerator::new();
        let context = create_test_context();

        let method1 = create_test_symbol("process", SymbolKind::Method, "java")
            .with_qualified_name("com.example.Service.process".to_string())
            .with_signature("void process(String input)".to_string());

        let method2 = create_test_symbol("process", SymbolKind::Method, "java")
            .with_qualified_name("com.example.Service.process".to_string())
            .with_signature("void process(String input, int count)".to_string());

        let uid1 = generator.generate_uid(&method1, &context).unwrap();
        let uid2 = generator.generate_uid(&method2, &context).unwrap();

        assert_ne!(uid1, uid2); // Different signatures should generate different UIDs
        assert!(uid1.contains("#"));
        assert!(uid2.contains("#"));
    }

    #[test]
    fn test_local_variable_uid() {
        let generator = SymbolUIDGenerator::new();
        let context = create_test_context();

        let var_symbol = create_test_symbol("local_var", SymbolKind::Variable, "rust");
        let uid = generator.generate_uid(&var_symbol, &context).unwrap();

        assert!(uid.starts_with("rust::"));
        assert!(uid.contains("local_var"));
        assert!(uid.contains("#")); // Should have position hash
    }

    #[test]
    fn test_anonymous_symbol_uid() {
        let generator = SymbolUIDGenerator::new();
        let context = create_test_context();

        let lambda_symbol = create_test_symbol("lambda@123", SymbolKind::Anonymous, "python");
        let uid = generator.generate_uid(&lambda_symbol, &context).unwrap();

        assert!(uid.starts_with("python::"));
        assert!(uid.contains("lambda")); // Should use anonymous prefix
    }

    #[test]
    fn test_usr_symbol_uid() {
        let generator = SymbolUIDGenerator::new();
        let context = create_test_context();

        let symbol = create_test_symbol("test_func", SymbolKind::Function, "c")
            .with_usr("c:@F@test_func".to_string());

        let uid = generator.generate_uid(&symbol, &context).unwrap();
        assert_eq!(uid, "c:@F@test_func"); // USR should be used directly
    }

    #[test]
    fn test_batch_uid_generation() {
        let generator = SymbolUIDGenerator::new();
        let context = create_test_context();

        let symbols = vec![
            (
                create_test_symbol("func1", SymbolKind::Function, "rust"),
                context.clone(),
            ),
            (
                create_test_symbol("func2", SymbolKind::Function, "rust"),
                context.clone(),
            ),
            (
                create_test_symbol("var1", SymbolKind::Variable, "rust"),
                context.clone(),
            ),
        ];

        let uids = generator.generate_batch_uids(&symbols);
        assert_eq!(uids.len(), 3);

        for uid_result in &uids {
            assert!(uid_result.is_ok());
        }

        let uid_strings: Vec<String> = uids.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(uid_strings.len(), 3);

        // All UIDs should be unique
        for i in 0..uid_strings.len() {
            for j in i + 1..uid_strings.len() {
                assert_ne!(uid_strings[i], uid_strings[j]);
            }
        }
    }

    #[test]
    fn test_uid_validation() {
        let generator = SymbolUIDGenerator::new();

        assert!(generator.validate_uid("rust::module::function"));
        assert!(generator.validate_uid("java::com::example::Class::method#hash123"));

        assert!(!generator.validate_uid(""));
        assert!(!generator.validate_uid("a"));
        assert!(!generator.validate_uid("no_separator"));
    }

    #[test]
    fn test_language_extraction() {
        let generator = SymbolUIDGenerator::new();

        assert_eq!(
            generator.extract_language_from_uid("rust::module::function"),
            Some("rust".to_string())
        );
        assert_eq!(
            generator.extract_language_from_uid("java::com::example::Class"),
            Some("java".to_string())
        );
        assert_eq!(generator.extract_language_from_uid("invalid"), None);
    }

    #[test]
    fn test_error_handling() {
        let generator = SymbolUIDGenerator::new();
        let context = create_test_context();

        // Empty symbol name
        let location = SymbolLocation::point(PathBuf::from("test.rs"), 10, 5);
        let empty_name_symbol = SymbolInfo::new(
            "".to_string(),
            SymbolKind::Function,
            "rust".to_string(),
            location.clone(),
        );
        assert!(generator
            .generate_uid(&empty_name_symbol, &context)
            .is_err());

        // Empty language
        let empty_lang_symbol = SymbolInfo::new(
            "test".to_string(),
            SymbolKind::Function,
            "".to_string(),
            location.clone(),
        );
        assert!(generator
            .generate_uid(&empty_lang_symbol, &context)
            .is_err());

        // Unsupported language
        let unsupported_symbol = SymbolInfo::new(
            "test".to_string(),
            SymbolKind::Function,
            "unsupported_lang".to_string(),
            location,
        );
        assert!(generator
            .generate_uid(&unsupported_symbol, &context)
            .is_err());
    }

    #[test]
    fn test_hash_algorithms() {
        let blake3_gen = SymbolUIDGenerator::with_hash_algorithm(HashAlgorithm::Blake3);
        let sha256_gen = SymbolUIDGenerator::with_hash_algorithm(HashAlgorithm::Sha256);

        let context = create_test_context();
        let symbol = create_test_symbol("test_func", SymbolKind::Function, "rust");

        let blake3_uid = blake3_gen.generate_uid(&symbol, &context).unwrap();
        let sha256_uid = sha256_gen.generate_uid(&symbol, &context).unwrap();

        // Different algorithms might produce different hashes for position-based components
        // but the structure should be similar
        assert!(blake3_uid.starts_with("rust::"));
        assert!(sha256_uid.starts_with("rust::"));
    }

    #[test]
    fn test_generator_stats() {
        let generator = SymbolUIDGenerator::new();
        let stats = generator.get_stats();

        assert!(stats.contains_key("hash_algorithm"));
        assert!(stats.contains_key("supported_languages"));
        assert!(stats.contains_key("languages"));

        assert_eq!(stats["hash_algorithm"], "Blake3");
        assert!(stats["supported_languages"].parse::<usize>().unwrap() > 0);
    }
}
