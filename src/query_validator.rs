use anyhow::{bail, Result};

/// Validates ElasticSearch query syntax when strict mode is enabled
pub fn validate_strict_elastic_syntax(query: &str) -> Result<()> {
    // Remove leading/trailing whitespace
    let query = query.trim();

    if query.is_empty() {
        bail!("Query cannot be empty");
    }

    // Check for vague query patterns (multiple words without operators)
    if has_vague_query_format(query) {
        bail!(
            "Vague query format detected. When using --strict-elastic-syntax:\n\
             - Use explicit AND/OR operators: (term1 AND term2) OR term3\n\
             - Wrap exact matches in quotes: \"functionName\"\n\
             - Use parentheses for grouping complex queries\n\
             \n\
             Examples:\n\
             - \"getUserId\" (exact match)\n\
             - (error AND handler)\n\
             - (\"getUserId\" AND NOT deprecated)"
        );
    }

    // Check for terms with special characters that should be quoted
    if let Some(problematic_term) = find_unquoted_special_terms(query) {
        bail!(
            "Term '{}' contains special characters (snake_case, camelCase, etc.) and should be wrapped in quotes.\n\
             \n\
             Options:\n\
             - For exact match: \"{}\" (with quotes)\n\
             - For separate keywords: split into individual terms with AND/OR operators\n\
             \n\
             Examples:\n\
             - \"get_user_id\" (exact match for snake_case)\n\
             - \"getUserId\" (exact match for camelCase)\n\
             - (get AND user AND id) (separate keywords)",
            problematic_term, problematic_term
        );
    }

    Ok(())
}

/// Detects vague query formats (multiple words without proper ES operators)
fn has_vague_query_format(query: &str) -> bool {
    // Check if query has multiple space-separated words but no ES operators
    let has_multiple_words = query.split_whitespace().count() > 1;

    if !has_multiple_words {
        return false;
    }

    // Check if it has proper ES operators (AND, OR, NOT)
    let has_operators =
        query.contains(" AND ") || query.contains(" OR ") || query.contains(" NOT ");

    // Check if entire query is quoted (which is valid)
    let is_fully_quoted = query.starts_with('"') && query.ends_with('"');

    // Vague if multiple words, no operators, and not fully quoted
    has_multiple_words && !has_operators && !is_fully_quoted
}

/// Finds terms with special characters that should be quoted
fn find_unquoted_special_terms(query: &str) -> Option<String> {
    // Parse the query to extract individual terms
    // We need to skip terms that are already quoted

    let mut in_quotes = false;
    let mut _in_parens = 0;
    let mut current_term = String::new();

    for ch in query.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                if !in_quotes && !current_term.is_empty() {
                    // Term ended, clear it
                    current_term.clear();
                }
            }
            '(' => {
                if !in_quotes {
                    _in_parens += 1;
                }
            }
            ')' => {
                if !in_quotes {
                    _in_parens -= 1;
                }
            }
            ' ' if !in_quotes => {
                // Check the accumulated term
                if !current_term.is_empty()
                    && !is_operator(&current_term)
                    && has_special_chars(&current_term)
                {
                    return Some(current_term.clone());
                }
                current_term.clear();
            }
            _ => {
                if !in_quotes {
                    current_term.push(ch);
                }
            }
        }
    }

    // Check the last term
    if !current_term.is_empty() && !is_operator(&current_term) && has_special_chars(&current_term) {
        return Some(current_term);
    }

    None
}

/// Checks if a string is an ES operator
fn is_operator(s: &str) -> bool {
    matches!(s, "AND" | "OR" | "NOT")
}

/// Checks if a term has special characters (underscore, mixed case)
fn has_special_chars(term: &str) -> bool {
    // Check for underscore (snake_case)
    if term.contains('_') {
        return true;
    }

    // Check for camelCase or PascalCase (has both upper and lower case letters)
    // Exclude single characters and require actual mixed case
    if term.len() <= 1 {
        return false;
    }

    let has_upper = term.chars().any(|c| c.is_uppercase());
    let has_lower = term.chars().any(|c| c.is_lowercase());

    has_upper && has_lower
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_queries() {
        // These should all pass
        assert!(validate_strict_elastic_syntax("\"functionName\"").is_ok());
        assert!(validate_strict_elastic_syntax("(error AND handler)").is_ok());
        assert!(validate_strict_elastic_syntax("(\"getUserId\" AND NOT deprecated)").is_ok());
        assert!(validate_strict_elastic_syntax("\"get_user_id\"").is_ok());
        assert!(validate_strict_elastic_syntax("singleword").is_ok());
    }

    #[test]
    fn test_vague_queries() {
        // Multiple words without operators should fail
        assert!(validate_strict_elastic_syntax("error handler").is_err());
        assert!(validate_strict_elastic_syntax("function name search").is_err());
    }

    #[test]
    fn test_unquoted_special_chars() {
        // snake_case without quotes should fail
        assert!(validate_strict_elastic_syntax("get_user_id").is_err());

        // camelCase without quotes should fail
        assert!(validate_strict_elastic_syntax("getUserId").is_err());

        // PascalCase without quotes should fail
        assert!(validate_strict_elastic_syntax("GetUserId").is_err());
    }

    #[test]
    fn test_quoted_special_chars() {
        // Quoted versions should pass
        assert!(validate_strict_elastic_syntax("\"get_user_id\"").is_ok());
        assert!(validate_strict_elastic_syntax("\"getUserId\"").is_ok());
        assert!(validate_strict_elastic_syntax("\"GetUserId\"").is_ok());
    }

    #[test]
    fn test_complex_queries() {
        // Valid complex queries
        assert!(validate_strict_elastic_syntax("(\"get_user_id\" AND NOT test)").is_ok());
        assert!(validate_strict_elastic_syntax("(error OR warning) AND handler").is_ok());

        // Invalid complex queries
        assert!(validate_strict_elastic_syntax("get_user_id AND test").is_err()); // unquoted snake_case
        assert!(validate_strict_elastic_syntax("error warning").is_err()); // no operator
    }

    #[test]
    fn test_single_character_terms() {
        // Single uppercase letters should be allowed (not treated as camelCase)
        assert!(validate_strict_elastic_syntax("A").is_ok());
        assert!(validate_strict_elastic_syntax("I").is_ok());
        assert!(validate_strict_elastic_syntax("X").is_ok());

        // Single lowercase letters should be allowed
        assert!(validate_strict_elastic_syntax("a").is_ok());
        assert!(validate_strict_elastic_syntax("i").is_ok());

        // Single character with underscore still requires quotes
        assert!(validate_strict_elastic_syntax("_").is_err());
    }

    #[test]
    fn test_edge_cases() {
        // Empty string
        assert!(validate_strict_elastic_syntax("").is_err());

        // Just whitespace
        assert!(validate_strict_elastic_syntax("   ").is_err());

        // Parentheses only
        assert!(validate_strict_elastic_syntax("()").is_ok());

        // Mix of valid single chars and operators
        assert!(validate_strict_elastic_syntax("(A OR B)").is_ok());
    }
}
