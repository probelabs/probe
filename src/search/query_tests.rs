use crate::search::query::{preprocess_query, regex_escape, create_term_patterns};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_query() {
        // Test basic preprocessing
        let query = "search for code";
        let terms = preprocess_query(query, false); // Use non-exact mode
        
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
        let terms = preprocess_query(query, false); // Use non-exact mode
        
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
        let terms = preprocess_query(query, false); // Use non-exact mode
        
        assert_eq!(terms.len(), 0);
    }

    #[test]
    fn test_preprocess_query_only_stop_words() {
        // Test with only stop words
        let query = "the and of";
        let terms = preprocess_query(query, false); // Use non-exact mode
        
        assert_eq!(terms.len(), 0);
    }

    #[test]
    fn test_preprocess_query_exact_mode() {
        // Test exact mode preprocessing
        let query = "ip whitelist";
        
        // In exact mode
        let exact_terms = preprocess_query(query, true);
        
        // Should preserve both words without stemming
        assert_eq!(exact_terms.len(), 2);
        
        // Check that both words are preserved as-is
        let has_ip = exact_terms.iter().any(|(orig, stemmed)| orig == "ip" && stemmed == "ip");
        let has_whitelist = exact_terms.iter().any(|(orig, stemmed)| orig == "whitelist" && stemmed == "whitelist");
        
        assert!(has_ip);
        assert!(has_whitelist);
        
        // Test with stop words in exact mode
        let query_with_stop = "the ip whitelist for security";
        let exact_with_stop = preprocess_query(query_with_stop, true);
        
        // Should preserve all words including stop words
        assert_eq!(exact_with_stop.len(), 5);
        
        // Check that stop words are preserved
        let has_the = exact_with_stop.iter().any(|(orig, _)| orig == "the");
        let has_for = exact_with_stop.iter().any(|(orig, _)| orig == "for");
        
        assert!(has_the);
        assert!(has_for);
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
        
        // Check that the pattern for "search" has word boundaries (since stem is the same)
        assert_eq!(patterns[0], "\\bsearch\\b");
        
        // Check that the pattern for "function" has word boundaries (since stem is the same)
        assert_eq!(patterns[1], "\\bfunction\\b");
        
        // Check that the pattern for "running" includes both original and stemmed versions with word boundaries
        assert_eq!(patterns[2], "\\b(running|run)\\b");
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
        assert_eq!(patterns[0], "\\bsearch\\.term\\b");
        assert_eq!(patterns[1], "\\bfunction\\(x\\)\\b");
    }
    
    #[test]
    fn test_create_term_patterns_with_word_boundaries() {
        // Test that word boundaries are added correctly
        let terms = vec![
            ("ip".to_string(), "ip".to_string()),
            ("whitelist".to_string(), "whitelist".to_string()),
            ("running".to_string(), "run".to_string()),
        ];
        
        let patterns = create_term_patterns(&terms);
        
        assert_eq!(patterns.len(), 3);
        
        // Check that word boundaries are added
        assert_eq!(patterns[0], "\\bip\\b");
        assert_eq!(patterns[1], "\\bwhitelist\\b");
        assert_eq!(patterns[2], "\\b(running|run)\\b");
        
        // Verify that the patterns will match correctly
        let re_ip = regex::Regex::new(&patterns[0]).unwrap();
        let re_whitelist = regex::Regex::new(&patterns[1]).unwrap();
        let re_running = regex::Regex::new(&patterns[2]).unwrap();
        
        // Should match whole words
        assert!(re_ip.is_match("ip"));
        assert!(re_whitelist.is_match("whitelist"));
        assert!(re_running.is_match("running"));
        assert!(re_running.is_match("run"));
        
        // Should not match partial words
        assert!(!re_ip.is_match("ipaddress"));
        assert!(!re_whitelist.is_match("whitelistitem"));
        assert!(!re_running.is_match("running_fast"));
    }
}
