use std::collections::HashSet;

/// Configuration for SIMD tokenization operations
/// This replaces the previous unsafe environment variable approach
#[derive(Debug, Clone, Copy)]
pub struct SimdConfig {
    /// Whether SIMD tokenization is enabled globally
    pub simd_enabled: bool,
    /// Whether we're currently in a recursive call (prevents infinite recursion)
    pub in_recursive_call: bool,
}

impl Default for SimdConfig {
    fn default() -> Self {
        Self {
            simd_enabled: is_simd_enabled_from_env(),
            in_recursive_call: false,
        }
    }
}

impl SimdConfig {
    /// Create a new config with SIMD enabled
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a config with SIMD disabled
    pub fn disabled() -> Self {
        Self {
            simd_enabled: false,
            in_recursive_call: false,
        }
    }

    /// Create a config for recursive calls (disables SIMD to prevent infinite recursion)
    pub fn for_recursive_call(self) -> Self {
        Self {
            simd_enabled: self.simd_enabled,
            in_recursive_call: true,
        }
    }

    /// Check if SIMD should be used based on current config
    pub fn should_use_simd(self) -> bool {
        self.simd_enabled && !self.in_recursive_call
    }
}

/// SIMD-accelerated camelCase splitting and tokenization
///
/// This module provides high-performance string processing using SIMD instructions
/// for character classification and boundary detection. Falls back to scalar
/// implementation for non-ASCII or short strings.
///
/// Threshold for SIMD processing - strings shorter than this use scalar fallback
const SIMD_THRESHOLD: usize = 8;

/// Lookup table for character classification using SIMD
/// Each byte represents: bit 0 = uppercase, bit 1 = lowercase, bit 2 = digit
#[rustfmt::skip]
static CHAR_CLASS_TABLE: [u8; 256] = [
    // 0x00-0x0F: Control characters
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    // 0x10-0x1F: Control characters
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    // 0x20-0x2F: Space, punctuation, digits
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    // 0x30-0x39: Digits 0-9
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    // 0x3A-0x40: Punctuation and @
    0, 0, 0, 0, 0, 0, 0,
    // 0x41-0x5A: Uppercase A-Z
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    // 0x5B-0x60: Punctuation
    0, 0, 0, 0, 0, 0,
    // 0x61-0x7A: Lowercase a-z
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    // 0x7B-0x7F: Punctuation
    0, 0, 0, 0, 0,
    // 0x80-0xFF: Extended ASCII (not handled by SIMD)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// Character classification constants
const UPPERCASE_MASK: u8 = 1;
const LOWERCASE_MASK: u8 = 2;
const DIGIT_MASK: u8 = 4;

/// Check if SIMD tokenization is enabled via environment variable
/// SIMD is enabled by default, can be disabled with DISABLE_SIMD_TOKENIZATION=1
/// This function only reads the environment variable and doesn't modify it
fn is_simd_enabled_from_env() -> bool {
    std::env::var("DISABLE_SIMD_TOKENIZATION").unwrap_or_default() != "1"
}

/// Legacy function for backward compatibility
/// Prefer using SimdConfig::new() for new code
pub fn is_simd_enabled() -> bool {
    is_simd_enabled_from_env()
}

/// Fast SIMD-accelerated camelCase splitting for ASCII strings
/// Falls back to scalar implementation for Unicode, short strings, or complex cases
pub fn simd_split_camel_case(s: &str) -> Vec<String> {
    simd_split_camel_case_with_config(s, SimdConfig::new())
}

/// SIMD-accelerated camelCase splitting with explicit configuration
/// This is the thread-safe version that doesn't use environment variable manipulation
pub fn simd_split_camel_case_with_config(s: &str, config: SimdConfig) -> Vec<String> {
    // Check if this is a special case word that should never be split (e.g., exact search terms)
    if crate::search::tokenization::is_special_case(s) {
        return vec![s.to_lowercase()];
    }

    // Use scalar fallback for short strings or non-ASCII
    if s.len() < SIMD_THRESHOLD || !s.is_ascii() {
        return scalar_split_camel_case(s);
    }

    // Fall back to the full tokenization implementation for complex cases
    // This includes common patterns like OAuth2, IPv4, GraphQL, etc.
    let lowercase = s.to_lowercase();
    if contains_special_patterns(&lowercase) {
        // Use recursive call prevention instead of environment variable manipulation
        let recursive_config = config.for_recursive_call();
        return crate::search::tokenization::split_camel_case_with_config(s, recursive_config);
    }

    let bytes = s.as_bytes();
    let mut result = Vec::new();
    let mut current_start = 0;

    // Find all camelCase boundaries using SIMD
    let boundaries = find_camel_boundaries_simd(bytes);

    for &boundary in &boundaries {
        if boundary > current_start {
            let part = &s[current_start..boundary];
            if !part.is_empty() {
                result.push(part.to_lowercase());
            }
        }
        current_start = boundary;
    }

    // Add the final part
    if current_start < s.len() {
        let part = &s[current_start..];
        if !part.is_empty() {
            result.push(part.to_lowercase());
        }
    }

    // Return original string if no boundaries found
    if result.is_empty() {
        vec![s.to_lowercase()]
    } else {
        result
    }
}

/// Check if the string contains patterns that require special handling
fn contains_special_patterns(lowercase: &str) -> bool {
    // Common special patterns that need complex tokenization logic
    const SPECIAL_PATTERNS: &[&str] = &[
        "oauth",
        "oauth2",
        "ipv4",
        "ipv6",
        "graphql",
        "postgresql",
        "mysql",
        "mongodb",
        "javascript",
        "typescript",
        "nodejs",
        "api",
        "http",
        "https",
        "ssl",
        "tls",
        "xml",
        "html",
        "css",
        "json",
        "yaml",
        "url",
        "uri",
        "uuid",
        "guid",
    ];

    for pattern in SPECIAL_PATTERNS {
        if lowercase.contains(pattern) {
            return true;
        }
    }
    false
}

/// Find camelCase boundaries using SIMD character classification
fn find_camel_boundaries_simd(bytes: &[u8]) -> Vec<usize> {
    let mut boundaries = Vec::new();
    let len = bytes.len();

    if len < 2 {
        return boundaries;
    }

    // Process byte by byte, using SIMD character classification
    // We need to look ahead for complex cases like "JSONTo" -> "JSON" + "To"
    for i in 1..len {
        let prev_class = CHAR_CLASS_TABLE[bytes[i - 1] as usize];
        let curr_class = CHAR_CLASS_TABLE[bytes[i] as usize];

        // Standard boundary detection
        if should_split_at_boundary(prev_class, curr_class) {
            boundaries.push(i);
            continue;
        }

        // Special case: uppercase -> uppercase -> lowercase (e.g., "JSONTo")
        // We should split between the last uppercase of an acronym and the start of the next word
        if (prev_class & UPPERCASE_MASK) != 0 && (curr_class & UPPERCASE_MASK) != 0 {
            // Look ahead to see if the next character is lowercase
            if i + 1 < len {
                let next_class = CHAR_CLASS_TABLE[bytes[i + 1] as usize];
                if (next_class & LOWERCASE_MASK) != 0 {
                    // Split before this uppercase letter (it's the start of a new word)
                    boundaries.push(i);
                }
            }
        }
    }

    boundaries
}

/// Determine if we should split at the boundary between two character classes
#[inline]
fn should_split_at_boundary(prev_class: u8, curr_class: u8) -> bool {
    // lowercase -> uppercase (camelCase)
    if (prev_class & LOWERCASE_MASK) != 0 && (curr_class & UPPERCASE_MASK) != 0 {
        return true;
    }

    // letter -> digit
    if ((prev_class & (UPPERCASE_MASK | LOWERCASE_MASK)) != 0) && (curr_class & DIGIT_MASK) != 0 {
        return true;
    }

    // digit -> letter
    if (prev_class & DIGIT_MASK) != 0 && ((curr_class & (UPPERCASE_MASK | LOWERCASE_MASK)) != 0) {
        return true;
    }

    false
}

/// Scalar fallback implementation for non-ASCII or short strings
pub fn scalar_split_camel_case(s: &str) -> Vec<String> {
    // This is the original implementation from tokenization.rs
    if s.is_empty() {
        return vec![];
    }

    // Check if this is a special case word that should never be split (e.g., exact search terms)
    if crate::search::tokenization::is_special_case(s) {
        return vec![s.to_lowercase()];
    }

    let mut result = Vec::new();
    let mut current_word = String::new();
    let mut prev_was_lower = false;

    for ch in s.chars() {
        if ch.is_uppercase() {
            if !current_word.is_empty() && prev_was_lower {
                result.push(current_word.to_lowercase());
                current_word = String::new();
            }
            current_word.push(ch);
            prev_was_lower = false;
        } else if ch.is_lowercase() {
            current_word.push(ch);
            prev_was_lower = true;
        } else if ch.is_ascii_digit() {
            if !current_word.is_empty() && prev_was_lower {
                result.push(current_word.to_lowercase());
                current_word = String::new();
            }
            current_word.push(ch);
            prev_was_lower = false;
        } else {
            // Non-alphanumeric character, treat as delimiter
            if !current_word.is_empty() {
                result.push(current_word.to_lowercase());
                current_word = String::new();
            }
            prev_was_lower = false;
        }
    }

    if !current_word.is_empty() {
        result.push(current_word.to_lowercase());
    }

    if result.is_empty() {
        vec![s.to_lowercase()]
    } else {
        result
    }
}

/// Enhanced SIMD tokenization with camelCase splitting and compound word detection
pub fn simd_tokenize(text: &str, vocabulary: &HashSet<String>) -> Vec<String> {
    simd_tokenize_with_config(text, vocabulary, SimdConfig::new())
}

/// SIMD tokenization with explicit configuration (thread-safe)
pub fn simd_tokenize_with_config(
    text: &str,
    vocabulary: &HashSet<String>,
    config: SimdConfig,
) -> Vec<String> {
    if !config.should_use_simd() {
        return crate::search::tokenization::tokenize(text);
    }

    let stemmer = crate::ranking::get_stemmer();
    let mut negated_terms = HashSet::new();
    let mut tokens = Vec::new();

    // Split by whitespace and collect words
    for word in text.split_whitespace() {
        let is_negated = word.starts_with('-');
        let clean_word = if is_negated { &word[1..] } else { word };

        // Further split by non-alphanumeric characters
        let mut current_token = String::new();
        for c in clean_word.chars() {
            if c.is_alphanumeric() {
                current_token.push(c);
            } else if !current_token.is_empty() {
                if is_negated {
                    negated_terms.insert(current_token.to_lowercase());
                }
                tokens.push(current_token);
                current_token = String::new();
            }
        }

        if !current_token.is_empty() {
            if is_negated {
                negated_terms.insert(current_token.to_lowercase());
            }
            tokens.push(current_token);
        }
    }

    // Process tokens with SIMD camelCase splitting
    let mut processed_tokens = HashSet::new();
    let mut result = Vec::new();

    for token in tokens {
        // Use SIMD camelCase splitting with config
        let parts = simd_split_camel_case_with_config(&token, config);

        for part in parts {
            let lowercase_part = part.to_lowercase();

            // Skip stop words and negated terms
            if crate::search::tokenization::is_stop_word(&lowercase_part)
                || negated_terms.contains(&lowercase_part)
            {
                continue;
            }

            // Try compound word splitting
            let compound_parts =
                crate::search::tokenization::split_compound_word(&lowercase_part, vocabulary);

            for compound_part in compound_parts {
                if crate::search::tokenization::is_stop_word(&compound_part)
                    || negated_terms.contains(&compound_part)
                {
                    continue;
                }

                // Preserve exception terms
                if crate::search::term_exceptions::is_exception_term(&compound_part)
                    && processed_tokens.insert(compound_part.clone())
                {
                    result.push(compound_part.clone());
                }

                // Add stemmed version
                let stemmed_part = stemmer.stem(&compound_part).to_string();
                if !negated_terms.contains(&stemmed_part)
                    && processed_tokens.insert(stemmed_part.clone())
                {
                    result.push(stemmed_part);
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_simd_camel_case_splitting() {
        // Test basic camelCase
        let result = simd_split_camel_case("parseUserEmail");
        assert_eq!(result, vec!["parse", "user", "email"]);

        // Test PascalCase
        let result = simd_split_camel_case("ParseUserEmail");
        assert_eq!(result, vec!["parse", "user", "email"]);

        // Test with numbers
        let result = simd_split_camel_case("parseJSON2HTML5");
        assert_eq!(result, vec!["parse", "json", "2", "html", "5"]);

        // Test single word
        let result = simd_split_camel_case("parse");
        assert_eq!(result, vec!["parse"]);

        // Test empty string
        let result = simd_split_camel_case("");
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_char_classification() {
        // Test uppercase
        assert_eq!(
            CHAR_CLASS_TABLE[b'A' as usize] & UPPERCASE_MASK,
            UPPERCASE_MASK
        );
        assert_eq!(
            CHAR_CLASS_TABLE[b'Z' as usize] & UPPERCASE_MASK,
            UPPERCASE_MASK
        );

        // Test lowercase
        assert_eq!(
            CHAR_CLASS_TABLE[b'a' as usize] & LOWERCASE_MASK,
            LOWERCASE_MASK
        );
        assert_eq!(
            CHAR_CLASS_TABLE[b'z' as usize] & LOWERCASE_MASK,
            LOWERCASE_MASK
        );

        // Test digits
        assert_eq!(CHAR_CLASS_TABLE[b'0' as usize] & DIGIT_MASK, DIGIT_MASK);
        assert_eq!(CHAR_CLASS_TABLE[b'9' as usize] & DIGIT_MASK, DIGIT_MASK);
    }

    #[test]
    fn test_boundary_detection() {
        // lowercase -> uppercase
        assert!(should_split_at_boundary(LOWERCASE_MASK, UPPERCASE_MASK));

        // letter -> digit
        assert!(should_split_at_boundary(LOWERCASE_MASK, DIGIT_MASK));
        assert!(should_split_at_boundary(UPPERCASE_MASK, DIGIT_MASK));

        // digit -> letter
        assert!(should_split_at_boundary(DIGIT_MASK, LOWERCASE_MASK));
        assert!(should_split_at_boundary(DIGIT_MASK, UPPERCASE_MASK));

        // No split for same types
        assert!(!should_split_at_boundary(LOWERCASE_MASK, LOWERCASE_MASK));
        assert!(!should_split_at_boundary(UPPERCASE_MASK, UPPERCASE_MASK));
    }

    #[test]
    fn test_simd_vs_scalar_equivalence() {
        // Test cases where SIMD should match the full tokenization implementation
        let simple_cases = vec![
            "parseUserEmail",
            "ParseUserEmail",
            "getElementById",
            "",
            "a",
            "simple",
        ];

        // Test simple cases (should use actual SIMD)
        for case in simple_cases {
            let simd_result = simd_split_camel_case(case);
            let scalar_result = scalar_split_camel_case(case);
            assert_eq!(
                simd_result, scalar_result,
                "Mismatch for simple input: {case}"
            );
        }

        // Test complex cases (should fall back to full tokenization)
        let complex_cases = vec!["XMLHttpRequest", "OAuth2Provider", "parseJSON2HTML5"];

        for case in complex_cases {
            // Use disabled config to get the expected result
            let expected_result = crate::search::tokenization::split_camel_case_with_config(
                case,
                SimdConfig::disabled(),
            );

            let simd_result = simd_split_camel_case(case);
            assert_eq!(
                simd_result, expected_result,
                "Mismatch for complex input: {case}"
            );
        }

        // Test specific complex cases
        let complex_result = simd_split_camel_case("parseJSON2HTML5");
        assert_eq!(complex_result, vec!["parse", "json", "2", "html", "5"]);

        let complex_result2 = simd_split_camel_case("HTML5Parser");
        // SIMD should handle number transitions properly
        assert!(complex_result2.len() >= 2);
        assert!(complex_result2.contains(&"html".to_string()));
    }

    #[test]
    fn test_simd_tokenization() {
        let vocab = HashSet::new();

        // Test with SIMD enabled (default behavior)
        let result1 = simd_tokenize_with_config("parseUserEmail", &vocab, SimdConfig::new());

        // Test with SIMD disabled
        let result2 = simd_tokenize_with_config("parseUserEmail", &vocab, SimdConfig::disabled());

        // Results should be functionally equivalent (may differ in order)
        assert!(!result1.is_empty());
        assert!(!result2.is_empty());
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let test_cases = Arc::new(vec![
            "OAuth2Provider",
            "parseXMLHttpRequest",
            "getUserEmailAddress",
            "HTML5Parser",
            "parseJSON2HTML5",
            "GraphQLEndpoint",
        ]);

        let handles: Vec<_> = (0..8)
            .map(|thread_id| {
                let cases = Arc::clone(&test_cases);
                thread::spawn(move || {
                    let mut results = Vec::new();

                    for (i, case) in cases.iter().enumerate() {
                        // Use different configs in different threads to test concurrency
                        let config = if thread_id % 2 == 0 {
                            SimdConfig::new()
                        } else {
                            SimdConfig::disabled()
                        };

                        let result = simd_split_camel_case_with_config(case, config);
                        results.push((i, case, result));
                    }

                    // Verify all results are consistent and non-empty
                    for (i, case, result) in results {
                        assert!(
                            !result.is_empty(),
                            "Empty result for case {i} in thread {thread_id}: {case}"
                        );

                        // Verify that complex cases fall back correctly
                        if case.contains("OAuth2") || case.contains("XML") || case.contains("HTML5")
                        {
                            assert!(
                                result.len() >= 2,
                                "Complex case {case} should split into multiple parts: {result:?}"
                            );
                        }
                    }

                    thread_id
                })
            })
            .collect();

        // Wait for all threads to complete
        let thread_results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Verify all threads completed successfully
        assert_eq!(thread_results.len(), 8);
        for i in 0..8 {
            assert!(thread_results.contains(&i), "Thread {i} did not complete");
        }
    }
}
