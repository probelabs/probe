use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use memchr::memmem;
use std::collections::HashMap;

/// SIMD-accelerated multi-pattern string matching
///
/// This module provides high-performance pattern matching using SIMD instructions
/// for multi-term search operations. Uses memchr for simple patterns and
/// aho-corasick for complex multi-pattern scenarios.
///
/// Configuration for SIMD pattern matching
pub struct SimdPatternConfig {
    /// Use memchr for single patterns (fastest)
    pub use_memchr_single: bool,
    /// Use memchr2/memchr3 for 2-3 patterns
    pub use_memchr_multi: bool,
    /// Use aho-corasick for complex patterns
    pub use_aho_corasick: bool,
    /// Match case-insensitively
    pub case_insensitive: bool,
    /// Use leftmost-first matching (vs leftmost-longest)
    pub leftmost_first: bool,
}

impl Default for SimdPatternConfig {
    fn default() -> Self {
        Self {
            use_memchr_single: true,
            use_memchr_multi: true,
            use_aho_corasick: true,
            case_insensitive: false,
            leftmost_first: true,
        }
    }
}

/// Check if SIMD pattern matching is enabled via environment variable
/// SIMD is enabled by default, can be disabled with DISABLE_SIMD_PATTERN_MATCHING=1
pub fn is_simd_pattern_matching_enabled() -> bool {
    std::env::var("DISABLE_SIMD_PATTERN_MATCHING").unwrap_or_default() != "1"
}

/// SIMD-accelerated pattern matcher for multiple search terms
pub struct SimdPatternMatcher {
    /// Configuration
    config: SimdPatternConfig,
    /// Patterns to search for
    patterns: Vec<String>,
    /// Single pattern searcher (memchr/memmem)
    single_searcher: Option<memmem::Finder<'static>>,
    /// Multi-pattern searcher (aho-corasick)
    multi_searcher: Option<AhoCorasick>,
}

impl SimdPatternMatcher {
    /// Create a new SIMD pattern matcher
    pub fn new(patterns: Vec<String>, config: SimdPatternConfig) -> Self {
        let mut matcher = SimdPatternMatcher {
            config,
            patterns: patterns.clone(),
            single_searcher: None,
            multi_searcher: None,
        };

        matcher.build_searchers(&patterns);
        matcher
    }

    /// Create a matcher with default configuration
    pub fn with_patterns(patterns: Vec<String>) -> Self {
        Self::new(patterns, SimdPatternConfig::default())
    }

    /// Build the appropriate searchers based on pattern count and configuration
    fn build_searchers(&mut self, patterns: &[String]) {
        if patterns.is_empty() {
            return;
        }

        // For single pattern, use memmem (SIMD-accelerated)
        if patterns.len() == 1 && self.config.use_memchr_single {
            let pattern = if self.config.case_insensitive {
                patterns[0].to_lowercase()
            } else {
                patterns[0].clone()
            };

            // Use static lifetime by leaking the string (for performance)
            let static_pattern: &'static str = Box::leak(pattern.into_boxed_str());
            self.single_searcher = Some(memmem::Finder::new(static_pattern));
            return;
        }

        // For multiple patterns, use aho-corasick with SIMD
        if self.config.use_aho_corasick {
            let mut builder = AhoCorasickBuilder::new();
            builder
                .ascii_case_insensitive(self.config.case_insensitive)
                .match_kind(if self.config.leftmost_first {
                    MatchKind::LeftmostFirst
                } else {
                    MatchKind::LeftmostLongest
                });

            if let Ok(searcher) = builder.build(patterns) {
                self.multi_searcher = Some(searcher);
            }
        }
    }

    /// Find all matches of patterns in the given text
    pub fn find_all_matches(&self, text: &str) -> Vec<PatternMatch> {
        if !is_simd_pattern_matching_enabled() {
            return self.fallback_find_matches(text);
        }

        let search_text = if self.config.case_insensitive {
            text.to_lowercase()
        } else {
            text.to_string()
        };

        // Use single pattern searcher if available
        if let Some(ref searcher) = self.single_searcher {
            return self.find_single_pattern_matches(&search_text, searcher);
        }

        // Use multi-pattern searcher if available
        if let Some(ref searcher) = self.multi_searcher {
            return self.find_multi_pattern_matches(&search_text, searcher);
        }

        // Fallback to traditional matching
        self.fallback_find_matches(text)
    }

    /// Find matches using single pattern SIMD search
    fn find_single_pattern_matches(
        &self,
        text: &str,
        searcher: &memmem::Finder,
    ) -> Vec<PatternMatch> {
        let mut matches = Vec::new();
        let mut start = 0;

        while let Some(pos) = searcher.find(&text.as_bytes()[start..]) {
            let absolute_pos = start + pos;
            matches.push(PatternMatch {
                pattern_index: 0,
                start: absolute_pos,
                end: absolute_pos + self.patterns[0].len(),
                pattern: self.patterns[0].clone(),
            });
            start = absolute_pos + 1;
        }

        matches
    }

    /// Find matches using multi-pattern SIMD search
    fn find_multi_pattern_matches(&self, text: &str, searcher: &AhoCorasick) -> Vec<PatternMatch> {
        searcher
            .find_iter(text)
            .map(|mat| PatternMatch {
                pattern_index: mat.pattern().as_usize(),
                start: mat.start(),
                end: mat.end(),
                pattern: self.patterns[mat.pattern().as_usize()].clone(),
            })
            .collect()
    }

    /// Fallback to traditional string matching
    fn fallback_find_matches(&self, text: &str) -> Vec<PatternMatch> {
        let mut matches = Vec::new();
        let search_text = if self.config.case_insensitive {
            text.to_lowercase()
        } else {
            text.to_string()
        };

        for (pattern_index, pattern) in self.patterns.iter().enumerate() {
            let search_pattern = if self.config.case_insensitive {
                pattern.to_lowercase()
            } else {
                pattern.clone()
            };

            let mut start = 0;
            while let Some(pos) = search_text[start..].find(&search_pattern) {
                let absolute_pos = start + pos;
                matches.push(PatternMatch {
                    pattern_index,
                    start: absolute_pos,
                    end: absolute_pos + pattern.len(),
                    pattern: pattern.clone(),
                });
                start = absolute_pos + 1;
            }
        }

        // Sort matches by position for consistency
        matches.sort_by_key(|m| (m.start, m.pattern_index));
        matches
    }

    /// Check if any pattern matches in the text (optimized for boolean queries)
    pub fn has_match(&self, text: &str) -> bool {
        if !is_simd_pattern_matching_enabled() {
            return self.fallback_has_match(text);
        }

        let search_text = if self.config.case_insensitive {
            text.to_lowercase()
        } else {
            text.to_string()
        };

        // Use single pattern searcher if available
        if let Some(ref searcher) = self.single_searcher {
            return searcher.find(search_text.as_bytes()).is_some();
        }

        // Use multi-pattern searcher if available
        if let Some(ref searcher) = self.multi_searcher {
            return searcher.is_match(&search_text);
        }

        // Fallback
        self.fallback_has_match(text)
    }

    /// Fallback boolean matching
    fn fallback_has_match(&self, text: &str) -> bool {
        let search_text = if self.config.case_insensitive {
            text.to_lowercase()
        } else {
            text.to_string()
        };

        for pattern in &self.patterns {
            let search_pattern = if self.config.case_insensitive {
                pattern.to_lowercase()
            } else {
                pattern.clone()
            };

            if search_text.contains(&search_pattern) {
                return true;
            }
        }

        false
    }

    /// Get the patterns being searched for
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}

/// A match found by the pattern matcher
#[derive(Debug, Clone, PartialEq)]
pub struct PatternMatch {
    /// Index of the matched pattern
    pub pattern_index: usize,
    /// Start position in the text
    pub start: usize,
    /// End position in the text
    pub end: usize,
    /// The matched pattern text
    pub pattern: String,
}

/// SIMD-accelerated character boundary detection
pub struct SimdBoundaryDetector {
    /// Character classification table (ASCII-optimized)
    char_classes: [u8; 256],
}

impl SimdBoundaryDetector {
    /// Create a new boundary detector
    pub fn new() -> Self {
        let mut char_classes = [0u8; 256];

        // Set character classes
        for (i, char_class) in char_classes.iter_mut().enumerate() {
            let ch = i as u8;
            let mut class = 0u8;

            if ch.is_ascii_alphabetic() {
                class |= 1; // ALPHA_MASK
            }
            if ch.is_ascii_digit() {
                class |= 2; // DIGIT_MASK
            }
            if ch.is_ascii_whitespace() {
                class |= 4; // SPACE_MASK
            }

            *char_class = class;
        }

        Self { char_classes }
    }

    /// Check if a character is a word boundary using SIMD-optimized lookup
    pub fn is_word_boundary(&self, text: &[u8], pos: usize) -> bool {
        if pos >= text.len() {
            return true;
        }

        let curr_class = self.char_classes[text[pos] as usize];

        if pos == 0 {
            return (curr_class & 3) != 0; // Start of text, current is alphanumeric
        }

        let prev_class = self.char_classes[text[pos - 1] as usize];

        // Boundary if transition between alphanumeric and non-alphanumeric
        let prev_alnum = (prev_class & 3) != 0;
        let curr_alnum = (curr_class & 3) != 0;

        prev_alnum != curr_alnum
    }

    /// Find all word boundaries in the text using vectorized processing
    pub fn find_word_boundaries(&self, text: &[u8]) -> Vec<usize> {
        let mut boundaries = Vec::new();

        if text.is_empty() {
            return boundaries;
        }

        // Start is always a boundary if first char is alphanumeric
        if (self.char_classes[text[0] as usize] & 3) != 0 {
            boundaries.push(0);
        }

        // Check each position for boundary transition
        for i in 1..text.len() {
            if self.is_word_boundary(text, i) {
                boundaries.push(i);
            }
        }

        // End is always a boundary if last char is alphanumeric
        let last_idx = text.len() - 1;
        if (self.char_classes[text[last_idx] as usize] & 3) != 0 {
            boundaries.push(text.len());
        }

        boundaries
    }
}

impl Default for SimdBoundaryDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for SIMD pattern matching integration
pub mod utils {
    use super::*;

    /// Create a SIMD pattern matcher from query terms
    pub fn create_matcher_from_terms(terms: &[String]) -> SimdPatternMatcher {
        SimdPatternMatcher::with_patterns(terms.to_vec())
    }

    /// Check if text contains any of the patterns using SIMD
    pub fn simd_contains_any(text: &str, patterns: &[String]) -> bool {
        if patterns.is_empty() {
            return false;
        }

        let matcher = SimdPatternMatcher::with_patterns(patterns.to_vec());
        matcher.has_match(text)
    }

    /// Find the first occurrence of any pattern using SIMD
    pub fn simd_find_first(text: &str, patterns: &[String]) -> Option<PatternMatch> {
        if patterns.is_empty() {
            return None;
        }

        let matcher = SimdPatternMatcher::with_patterns(patterns.to_vec());
        let matches = matcher.find_all_matches(text);
        matches.into_iter().min_by_key(|m| m.start)
    }

    /// Count occurrences of patterns using SIMD
    pub fn simd_count_matches(text: &str, patterns: &[String]) -> HashMap<String, usize> {
        let mut counts = HashMap::new();

        if patterns.is_empty() {
            return counts;
        }

        // Initialize counts
        for pattern in patterns {
            counts.insert(pattern.clone(), 0);
        }

        let matcher = SimdPatternMatcher::with_patterns(patterns.to_vec());
        let matches = matcher.find_all_matches(text);

        for match_info in matches {
            *counts.entry(match_info.pattern).or_insert(0) += 1;
        }

        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_pattern_matching() {
        let patterns = vec!["test".to_string()];
        let matcher = SimdPatternMatcher::with_patterns(patterns);

        let text = "This is a test string with test pattern";
        let matches = matcher.find_all_matches(text);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].start, 10);
        assert_eq!(matches[0].pattern, "test");
        assert_eq!(matches[1].start, 27); // "test" in "with test pattern"
    }

    #[test]
    fn test_multi_pattern_matching() {
        let patterns = vec!["foo".to_string(), "bar".to_string(), "baz".to_string()];
        let matcher = SimdPatternMatcher::with_patterns(patterns);

        let matches = matcher.find_all_matches("foo bar baz foo");
        assert_eq!(matches.len(), 4);
        assert_eq!(matches[0].pattern, "foo");
        assert_eq!(matches[1].pattern, "bar");
        assert_eq!(matches[2].pattern, "baz");
        assert_eq!(matches[3].pattern, "foo");
    }

    #[test]
    fn test_case_insensitive_matching() {
        let config = SimdPatternConfig {
            case_insensitive: true,
            ..Default::default()
        };

        let patterns = vec!["Test".to_string()];
        let matcher = SimdPatternMatcher::new(patterns, config);

        let matches = matcher.find_all_matches("test TEST Test");
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_boolean_matching() {
        let patterns = vec!["function".to_string(), "class".to_string()];
        let matcher = SimdPatternMatcher::with_patterns(patterns);

        assert!(matcher.has_match("function test() {}"));
        assert!(matcher.has_match("class MyClass {}"));
        assert!(!matcher.has_match("const variable = 5;"));
    }

    #[test]
    fn test_boundary_detection() {
        let detector = SimdBoundaryDetector::new();
        let text = "hello world 123";
        let boundaries = detector.find_word_boundaries(text.as_bytes());

        // Should find boundaries at: 0 (start), 5 (space), 6 (start of 'world'), 11 (space),
        // 12 (start of '123'), 15 (end)
        assert!(boundaries.contains(&0)); // start of "hello"
        assert!(boundaries.contains(&6)); // start of "world"
        assert!(boundaries.contains(&12)); // start of "123"
    }

    #[test]
    fn test_simd_vs_fallback_equivalence() {
        let patterns = vec!["parse".to_string(), "user".to_string(), "email".to_string()];
        let test_text = "parseUserEmail function processes user email addresses";

        // Test with SIMD enabled (default behavior)
        std::env::remove_var("DISABLE_SIMD_PATTERN_MATCHING");
        let matcher = SimdPatternMatcher::with_patterns(patterns.clone());
        let simd_matches = matcher.find_all_matches(test_text);

        // Test with SIMD disabled
        std::env::set_var("DISABLE_SIMD_PATTERN_MATCHING", "1");
        let fallback_matches = matcher.find_all_matches(test_text);

        // Results should be equivalent
        assert_eq!(simd_matches.len(), fallback_matches.len());
        for (simd, fallback) in simd_matches.iter().zip(fallback_matches.iter()) {
            assert_eq!(simd.start, fallback.start);
            assert_eq!(simd.end, fallback.end);
            assert_eq!(simd.pattern, fallback.pattern);
        }

        // Clean up
        std::env::remove_var("DISABLE_SIMD_PATTERN_MATCHING");
    }

    #[test]
    fn test_utility_functions() {
        let patterns = vec!["test".to_string(), "pattern".to_string()];
        let text = "This is a test pattern matching test";

        // Test contains_any
        assert!(utils::simd_contains_any(text, &patterns));
        assert!(!utils::simd_contains_any(text, &["missing".to_string()]));

        // Test find_first
        let first_match = utils::simd_find_first(text, &patterns);
        assert!(first_match.is_some());
        assert_eq!(first_match.unwrap().start, 10); // "test" at position 10

        // Test count_matches
        let counts = utils::simd_count_matches(text, &patterns);
        assert_eq!(counts.get("test"), Some(&2));
        assert_eq!(counts.get("pattern"), Some(&1));
    }

    #[test]
    fn test_empty_patterns() {
        let matcher = SimdPatternMatcher::with_patterns(vec![]);
        let matches = matcher.find_all_matches("some text");
        assert!(matches.is_empty());
        assert!(!matcher.has_match("some text"));
    }
}
