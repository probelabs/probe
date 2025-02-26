use crate::search::query::{preprocess_query, regex_escape, create_term_patterns};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_query() {
        // Test basic preprocessing
        let query = "search for code";
        let terms = preprocess_query(query);
        
        // Should contain (search, search) and (code, code)
        // "for" should be removed as a stop word
        assert_eq!(terms.len(), 2);
        
        let has_search = terms.iter().any(|(orig, stemmed)| orig == "search");
        let has_code = terms.iter().any(|(orig, stemmed)| orig == "code");
        
        assert!(has_search);
        assert!(has_code);
        
        // Check that stop words are removed
        let has_for = terms.iter().any(|(orig, _)| orig == "for");
        assert!(!has_for);
    }

    #[test]
    fn test_preprocess_query_with_stemming() {
        // Test preprocessing with stemming
        let query = "searching functions";
        let terms = preprocess_query(query);
        
        // "searching" should be stemmed to "search"
        assert_eq!(terms.len(), 2);
        
        // Find the stemmed version of "searching"
        let search_term = terms.iter().find(|(orig, _)| orig == "searching");
        assert!(search_term.is_some());
        
        let (orig, stemmed) = search_term.unwrap();
        assert_eq!(orig, "searching");
        assert_eq!(stemmed, "search"); // Stemming should reduce "searching" to "search"
    }

    #[test]
    fn test_preprocess_query_empty() {
        // Test with empty query
        let query = "";
        let terms = preprocess_query(query);
        
        assert_eq!(terms.len(), 0);
    }

    #[test]
    fn test_preprocess_query_only_stop_words() {
        // Test with only stop words
        let query = "the and of";
        let terms = preprocess_query(query);
        
        assert_eq!(terms.len(), 0);
    }

    #[test]
    fn test_regex_escape() {
        // Test escaping special regex characters
        let special_chars = ".*+?()[]{}|^$\\";
        let escaped = regex_escape(special_chars);
        
        // Each special character should be escaped with a backslash
        assert_eq!(escaped, "\\.\\*\\+\\?\\(\\)\\[\\]\\{\\}\\|\\^\\$\\\\");
        
        // Test with normal text
        let normal_text = "normal text";
        let escaped_normal = regex_escape(normal_text);
        
        // Normal text should remain unchanged
        assert_eq!(escaped_normal, normal_text);
    }

    #[test]
    fn test_create_term_patterns() {
        // Test creating term patterns
        let terms = vec![
            ("search".to_string(), "search".to_string()),
            ("function".to_string(), "function".to_string()),
            ("running".to_string(), "run".to_string()),
        ];
        
        let patterns = create_term_patterns(&terms);
        
        assert_eq!(patterns.len(), 3);
        
        // Check that the pattern for "search" is just "search" (since stem is the same)
        assert_eq!(patterns[0], "search");
        
        // Check that the pattern for "function" is just "function" (since stem is the same)
        assert_eq!(patterns[1], "function");
        
        // Check that the pattern for "running" includes both original and stemmed versions
        assert_eq!(patterns[2], "(running|run)");
    }

    #[test]
    fn test_create_term_patterns_with_regex_chars() {
        // Test creating term patterns with regex special characters
        let terms = vec![
            ("search.term".to_string(), "search.term".to_string()),
            ("function(x)".to_string(), "function(x)".to_string()),
        ];
        
        let patterns = create_term_patterns(&terms);
        
        assert_eq!(patterns.len(), 2);
        
        // Check that regex special characters are escaped
        assert_eq!(patterns[0], "search\\.term");
        assert_eq!(patterns[1], "function\\(x\\)");
    }
}