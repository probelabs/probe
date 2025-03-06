use std::collections::HashSet;
use once_cell::sync::Lazy;
use crate::ranking::get_stemmer;

/// Static set of common English stop words
static ENGLISH_STOP_WORDS: Lazy<HashSet<String>> = Lazy::new(|| {
    vec![
        "a", "about", "above", "after", "again", "against", "all", "am", "an", "and",
        "any", "are", "aren't", "as", "at", "be", "because", "been", "before", "being",
        "below", "between", "both", "but", "by", "can't", "cannot", "could", "couldn't",
        "did", "didn't", "do", "does", "doesn't", "doing", "don't", "down", "during",
        "each", "few", "for", "from", "further", "had", "hadn't", "has", "hasn't",
        "have", "haven't", "having", "he", "he'd", "he'll", "he's", "her", "here",
        "here's", "hers", "herself", "him", "himself", "his", "how", "how's", "i",
        "i'd", "i'll", "i'm", "i've", "if", "in", "into", "is", "isn't", "it", "it's",
        "its", "itself", "let's", "me", "more", "most", "mustn't", "my", "myself",
        "no", "nor", "not", "of", "off", "on", "once", "only", "or", "other", "ought",
        "our", "ours", "ourselves", "out", "over", "own", "same", "shan't", "she",
        "she'd", "she'll", "she's", "should", "shouldn't", "so", "some", "such",
        "than", "that", "that's", "the", "their", "theirs", "them", "themselves",
        "then", "there", "there's", "these", "they", "they'd", "they'll", "they're",
        "they've", "this", "those", "through", "to", "too", "under", "until", "up",
        "very", "was", "wasn't", "we", "we'd", "we'll", "we're", "we've", "were",
        "weren't", "what", "what's", "when", "when's", "where", "where's", "which",
        "while", "who", "who's", "whom", "why", "why's", "with", "won't", "would",
        "wouldn't", "you", "you'd", "you'll", "you're", "you've", "your", "yours",
        "yourself", "yourselves",
    ]
    .into_iter()
    .map(String::from)
    .collect()
});

/// Static set of programming language stop words
static PROGRAMMING_STOP_WORDS: Lazy<HashSet<String>> = Lazy::new(|| {
    vec![
        // Go-specific keywords
        "func", "type", "struct", "interface", "chan", "map", "go", "defer",
        
        // Common programming keywords
        "var", "let", "const", "return", "if", "else", "for", "while",
        "switch", "case", "break", "continue", "default", "try", "catch",
        "finally", "throw", "new", "super", "extends", "implements",
        "function", "class", "method", "this",
        
        // Common modifiers
        "public", "private", "protected", "static", "final", "async", "await",
        
        // Common types and declarations
        "string", "int", "bool", "float", "void", "null", "nil",
        "class", "enum", "impl", "fn", "mod",
    ]
    .into_iter()
    .map(String::from)
    .collect()
});

/// Static set of special case words that should be treated as single tokens
/// These are typically technology names, protocols, or common programming terms
/// with non-standard capitalization patterns
static SPECIAL_CASE_WORDS: Lazy<HashSet<String>> = Lazy::new(|| {
    vec![
        // Common technology terms with specific capitalization
        "oauth", "oauth2",
        "ipv4", "ipv6", "ipv",
        "graphql",
        "postgresql", "mysql", "mongodb",
        "javascript", "typescript",
        "nodejs", "reactjs", "vuejs", "angularjs",
        "github", "gitlab", "bitbucket",
        "kubernetes", "docker",
        "webpack", "rollup", "vite",
        "eslint", "prettier",
        "axios", "fetch",
    ]
    .into_iter()
    .map(String::from)
    .collect()
});

/// Returns true if the character is uppercase
#[inline]
fn is_uppercase(c: char) -> bool {
    c.is_ascii_uppercase()
}

/// Returns true if the character is lowercase
#[inline]
fn is_lowercase(c: char) -> bool {
    c.is_ascii_lowercase()
}

/// Returns true if the character is a number
#[inline]
fn is_number(c: char) -> bool {
    c.is_ascii_digit()
}

/// Checks if a word is a special case that should be treated as a single token
fn is_special_case(word: &str) -> bool {
    // Convert to lowercase for case-insensitive comparison
    let lowercase = word.to_lowercase();
    
    // Check if the word is in the special case list
    SPECIAL_CASE_WORDS.contains(&lowercase)
}

/// Splits a string on camel case boundaries
/// This function handles:
/// - camelCase -> ["camel", "case"]
/// - PascalCase -> ["pascal", "case"]
/// - acronyms and numbers -> ["parse", "json", "to", "html", "5"]
/// - special cases like OAuth2 -> ["oauth2"]
pub fn split_camel_case(input: &str) -> Vec<String> {
    if input.is_empty() {
        return vec![];
    }
    
    // Check if the input is a special case word
    if is_special_case(input) {
        return vec![input.to_lowercase()];
    }
    
    // Special case for OAuth2Provider and similar patterns
    let lowercase = input.to_lowercase();
    
    // Special case for OAuth2Provider -> ["oauth2", "provider"]
    if lowercase.starts_with("oauth2") {
        let remaining = &input[6..]; // "oauth2".len() = 6
        if !remaining.is_empty() {
            let mut result = vec!["oauth2".to_string()];
            result.extend(split_camel_case(remaining));
            return result;
        }
    }
    
    // General special case handling
    for special_case in SPECIAL_CASE_WORDS.iter() {
        if lowercase.starts_with(special_case) {
            let remaining = &input[special_case.len()..];
            if !remaining.is_empty() {
                let mut result = vec![special_case.clone()];
                result.extend(split_camel_case(remaining));
                return result;
            }
        }
    }

    let chars: Vec<char> = input.chars().collect();
    let mut result = Vec::new();
    let mut current_word = String::new();
    
    // State tracking
    let mut prev_is_lower = false;
    let mut prev_is_upper = false;
    let mut prev_is_digit = false;

    for (i, &c) in chars.iter().enumerate() {
        let is_upper = is_uppercase(c);
        let is_lower = is_lowercase(c);
        let is_digit = is_number(c);
        
        // Start a new word when:
        // 1. Transition from lowercase to uppercase (camelCase)
        // 2. Transition from uppercase to uppercase followed by lowercase (APIClient -> API, Client)
        // 3. Transition to/from digits
        let start_new_word = 
            // Empty current word - no need to start a new one
            !current_word.is_empty() && (
                // Case 1: camelCase boundary
                (prev_is_lower && is_upper) ||
                // Case 2: Digit boundaries
                (prev_is_digit != is_digit) ||
                // Case 3: Uppercase followed by lowercase, but only if we have multiple uppercase in a row
                (prev_is_upper && is_upper && i + 1 < chars.len() && is_lowercase(chars[i + 1]))
            );
            
        if start_new_word {
            result.push(current_word);
            current_word = String::new();
        }
        
        current_word.push(c);
        
        // Update state for next iteration
        prev_is_lower = is_lower;
        prev_is_upper = is_upper;
        prev_is_digit = is_digit;
    }
    
    // Add the last word
    if !current_word.is_empty() {
        result.push(current_word);
    }
    
    // Convert all to lowercase for consistency
    result.into_iter().map(|word| word.to_lowercase()).collect()
}

/// Checks if a word is a common English stop word
pub fn is_english_stop_word(word: &str) -> bool {
    ENGLISH_STOP_WORDS.contains(word)
}

/// Checks if a word is a programming language stop word
pub fn is_programming_stop_word(word: &str) -> bool {
    PROGRAMMING_STOP_WORDS.contains(word)
}

/// Checks if a word is either an English or programming stop word
pub fn is_stop_word(word: &str) -> bool {
    is_english_stop_word(word) || is_programming_stop_word(word)
}

/// Tokenizes text into words by splitting on whitespace and non-alphanumeric characters,
/// removes stop words, and applies stemming. Also splits camelCase/PascalCase identifiers.
/// 
/// The tokenization flow follows these steps:
/// 1. Split input text on whitespace
/// 2. For each token, further split on non-alphanumeric characters
/// 3. For each resulting token, check if it has mixed case
/// 4. If it has mixed case, split using camel case rules
/// 5. Process each part: remove stop words and apply stemming
/// 6. Collect unique tokens
pub fn tokenize(text: &str) -> Vec<String> {
    let stemmer = get_stemmer();

    // Split by whitespace and collect words
    let mut tokens = Vec::new();
    for word in text.split_whitespace() {
        // Further split by non-alphanumeric characters
        let mut current_token = String::new();
        for c in word.chars() {
            if c.is_alphanumeric() {
                current_token.push(c);
            } else if !current_token.is_empty() {
                // We found a non-alphanumeric character, add the current token if not empty
                tokens.push(current_token);
                current_token = String::new();
            }
        }

        // Add the last token if not empty
        if !current_token.is_empty() {
            tokens.push(current_token);
        }
    }

    // Create a set to track unique tokens after processing
    let mut processed_tokens = HashSet::new();
    let mut result = Vec::new();

    // Process each token: filter stop words, apply stemming, and add to result if unique
    for token in tokens {
        // First, split camelCase/PascalCase identifiers
        let mut parts = Vec::new();
        
        // Only try to split if the original token has mixed case
        if token.chars().any(|c| c.is_uppercase()) {
            parts = split_camel_case(&token);
        } else {
            parts.push(token.to_lowercase());
        }

        // Process each part
        for part in parts {
            let lowercase_part = part.to_lowercase();
            
            // Skip both English and programming stop words
            if is_stop_word(&lowercase_part) {
                continue;
            }

            // Add the stemmed part if it's unique
            let stemmed_part = stemmer.stem(&lowercase_part).to_string();
            if processed_tokens.insert(stemmed_part.clone()) {
                result.push(stemmed_part);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_camel_case() {
        // Test basic camel case
        assert_eq!(
            split_camel_case("camelCase"),
            vec!["camel", "case"]
        );

        // Test pascal case
        assert_eq!(
            split_camel_case("PascalCase"),
            vec!["pascal", "case"]
        );

        // Test acronyms
        assert_eq!(
            split_camel_case("parseJSONToHTML5"),
            vec!["parse", "json", "to", "html", "5"]
        );

        // Test consecutive uppercase letters
        assert_eq!(
            split_camel_case("APIDefinition"),
            vec!["api", "definition"]
        );

        // Test special case with OAuth2
        assert_eq!(
            split_camel_case("OAuth2Provider"),
            vec!["oauth2", "provider"]
        );

        // Test mixed case with type prefix
        assert_eq!(
            split_camel_case("typeIgnore"),
            vec!["type", "ignore"]
        );

        // Test complex identifiers
        assert_eq!(
            split_camel_case("migrateEndpointMetaByType"),
            vec!["migrate", "endpoint", "meta", "by", "type"]
        );
    }

    #[test]
    fn test_stop_words() {
        assert!(is_programming_stop_word("func"));
        assert!(is_programming_stop_word("type"));
        assert!(is_programming_stop_word("struct"));
        assert!(!is_programming_stop_word("migrate"));
        assert!(!is_programming_stop_word("endpoint"));
    }
    
    #[test]
    fn test_tokenize() {
        // Test method with API acronym
        let tokens = tokenize("func (a *APIDefinition) MigrateEndpointMeta()");
        assert!(tokens.contains(&"api".to_string()));
        assert!(tokens.contains(&"definit".to_string())); // stemmed "definition"
        assert!(tokens.contains(&"migrat".to_string())); // stemmed "migrate"
        assert!(tokens.contains(&"endpoint".to_string()));
        assert!(tokens.contains(&"meta".to_string()));
        
        // Test complex identifier with acronyms and numbers
        let tokens = tokenize("func ParseJSONToHTML5()");
        assert!(tokens.contains(&"pars".to_string())); // stemmed "parse"
        assert!(tokens.contains(&"json".to_string()));
        assert!(tokens.contains(&"html".to_string()));
        assert!(tokens.contains(&"5".to_string()));
        
        // Test mixed case with type prefix
        let tokens = tokenize("typeIgnore typeWhitelist");
        assert!(tokens.contains(&"ignor".to_string())); // stemmed "ignore"
        assert!(tokens.contains(&"whitelist".to_string()));
    }
} 