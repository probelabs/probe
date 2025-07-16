use crate::ranking::get_stemmer;
use crate::search::term_exceptions::{is_exception_term, EXCEPTION_TERMS};
use decompound::{decompound, DecompositionOptions};
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::sync::Mutex;

// Dynamic set of special terms that should not be tokenized
// This includes terms from queries with exact=true or excluded=true flags
static DYNAMIC_SPECIAL_TERMS: Lazy<Mutex<HashSet<String>>> =
    Lazy::new(|| Mutex::new(HashSet::new()));

/// Add a term to the dynamic special terms list
pub fn add_special_term(term: &str) {
    let mut special_terms = DYNAMIC_SPECIAL_TERMS.lock().unwrap();
    special_terms.insert(term.to_lowercase());

    // Debug output
    if std::env::var("DEBUG").unwrap_or_default() == "1" {
        println!("DEBUG: Added special term: {term}");
    }
}

/// Static set of common English stop words
static ENGLISH_STOP_WORDS: Lazy<HashSet<String>> = Lazy::new(|| {
    vec![
        "a",
        "about",
        "above",
        "after",
        "again",
        "against",
        "all",
        "am",
        "an",
        "and",
        "any",
        "are",
        "aren't",
        "as",
        "at",
        "be",
        "because",
        "been",
        "before",
        "being",
        "below",
        "between",
        "both",
        "but",
        "by",
        "can't",
        "cannot",
        "could",
        "couldn't",
        "did",
        "didn't",
        "do",
        "does",
        "doesn't",
        "doing",
        "don't",
        "down",
        "during",
        "each",
        "few",
        "for",
        "from",
        "further",
        "had",
        "hadn't",
        "has",
        "hasn't",
        "have",
        "haven't",
        "having",
        "he",
        "he'd",
        "he'll",
        "he's",
        "her",
        "here",
        "here's",
        "hers",
        "herself",
        "him",
        "himself",
        "his",
        "how",
        "how's",
        "i",
        "i'd",
        "i'll",
        "i'm",
        "i've",
        "if",
        "in",
        "into",
        "is",
        "isn't",
        "it",
        "it's",
        "its",
        "itself",
        "let's",
        "me",
        "more",
        "most",
        "mustn't",
        "my",
        "myself",
        "no",
        "nor",
        "not",
        "of",
        "off",
        "on",
        "once",
        "only",
        "or",
        "other",
        "ought",
        "our",
        "ours",
        "ourselves",
        "out",
        "over",
        "own",
        "same",
        "shan't",
        "she",
        "she'd",
        "she'll",
        "she's",
        "should",
        "shouldn't",
        "so",
        "some",
        "such",
        "than",
        "that",
        "that's",
        "the",
        "their",
        "theirs",
        "them",
        "themselves",
        "then",
        "there",
        "there's",
        "these",
        "they",
        "they'd",
        "they'll",
        "they're",
        "they've",
        "this",
        "those",
        "through",
        "to",
        "too",
        "under",
        "until",
        "up",
        "very",
        "was",
        "wasn't",
        "we",
        "we'd",
        "we'll",
        "we're",
        "we've",
        "were",
        "weren't",
        "what",
        "what's",
        "when",
        "when's",
        "where",
        "where's",
        "which",
        "while",
        "who",
        "who's",
        "whom",
        "why",
        "why's",
        "with",
        "won't",
        "would",
        "wouldn't",
        "you",
        "you'd",
        "you'll",
        "you're",
        "you've",
        "your",
        "yours",
        "yourself",
        "yourselves",
        "ing",
    ]
    .into_iter()
    .map(String::from)
    .collect()
});

/// Static set of programming language stop words
static PROGRAMMING_STOP_WORDS: Lazy<HashSet<String>> = Lazy::new(|| {
    vec![
        // Go-specific keywords
        "func",
        "type",
        "struct",
        "interface",
        "chan",
        "map",
        "go",
        "defer",
        // Common programming keywords
        "var",
        "let",
        "const",
        "return",
        "if",
        "else",
        "for",
        "while",
        "switch",
        "case",
        "break",
        "continue",
        "default",
        "try",
        "catch",
        "finally",
        "throw",
        "new",
        "super",
        "extends",
        "implements",
        "function",
        "class",
        "method",
        "this",
        // Common modifiers
        "public",
        "private",
        "protected",
        "static",
        "final",
        "async",
        "await",
        // Common types and declarations
        "string",
        "int",
        "bool",
        "float",
        "void",
        "null",
        "nil",
        "class",
        "enum",
        "impl",
        "fn",
        "mod",
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
        "oauth",
        "oauth2",
        "ipv4",
        "ipv6",
        "ipv",
        "graphql",
        "postgresql",
        "mysql",
        "mongodb",
        "javascript",
        "typescript",
        "nodejs",
        "reactjs",
        "vuejs",
        "angularjs",
        "github",
        "gitlab",
        "bitbucket",
        "kubernetes",
        "docker",
        "webpack",
        "rollup",
        "vite",
        "eslint",
        "prettier",
        "axios",
        "fetch",
        "grpc",
        "http2",
        "whitelist",
        "blacklist",
        "allowlist",
        "blocklist",
        "denylist",
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
pub fn is_special_case(word: &str) -> bool {
    // Convert to lowercase for case-insensitive comparison
    let lowercase = word.to_lowercase();

    // Check if the word is in the static special case list
    if SPECIAL_CASE_WORDS.contains(&lowercase) {
        return true;
    }

    // Check if the word is in the dynamic special terms list
    let special_terms = DYNAMIC_SPECIAL_TERMS.lock().unwrap();
    if special_terms.contains(&lowercase) {
        // Debug output
        if std::env::var("DEBUG").unwrap_or_default() == "1" {
            println!("DEBUG: Found dynamic special term: {lowercase}");
        }
        return true;
    }

    false
}

/// Splits a string on camel case boundaries
/// This function handles:
/// - camelCase -> ["camel", "case"]
/// - PascalCase -> ["pascal", "case"]
/// - acronyms and numbers -> ["parse", "json", "to", "html", "5"]
/// - special cases like OAuth2 -> ["oauth2"]
/// - also attempts to split lowercase identifiers that might have been camelCase originally
pub fn split_camel_case(input: &str) -> Vec<String> {
    let _debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

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

    // Get all special case words and sort by length (longest first)
    // This ensures that longer matches like "ipv4" are checked before shorter ones like "ipv"
    let mut special_cases: Vec<&String> = SPECIAL_CASE_WORDS.iter().collect();
    special_cases.sort_by_key(|b| std::cmp::Reverse(b.len()));

    // General special case handling
    for special_case in special_cases {
        if lowercase.starts_with(special_case) {
            // Find the corresponding part in the original input
            let _original_part = &input[0..special_case.len()];
            let remaining = &input[special_case.len()..];

            if !remaining.is_empty() {
                let mut result = vec![special_case.clone()];
                result.extend(split_camel_case(remaining));

                return result;
            }
        }
    }

    // If input is all lowercase, try to identify potential camelCase boundaries
    // This is for handling cases where the input was already lowercased
    if input == lowercase && !input.contains('_') && input.len() > 3 {
        // Check for common patterns in identifiers
        let _potential_splits: Vec<String> = Vec::new();

        // Use the exception terms from our centralized list
        let common_terms = EXCEPTION_TERMS
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>();

        for term in common_terms {
            if input.contains(term) && term != input {
                let parts: Vec<&str> = input.split(term).collect();
                if parts.len() > 1 {
                    let mut result = Vec::new();
                    for (i, part) in parts.iter().enumerate() {
                        if !part.is_empty() {
                            result.push(part.to_string());
                        }
                        if i < parts.len() - 1 {
                            result.push(term.to_string());
                        }
                    }
                    if !result.is_empty() {
                        // println!("Split by common term '{}': {:?}", term, result);
                        return result;
                    }
                }
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

    // println!("Camel case split result: {:?}", final_result);
    result.into_iter().map(|word| word.to_lowercase()).collect()
}

/// Checks if a word is a common English stop word or a simple number (0-10)
pub fn is_english_stop_word(word: &str) -> bool {
    // Check if the word is a simple number (0-10)
    if let Ok(num) = word.parse::<u32>() {
        if num <= 10 {
            return true;
        }
    }

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

/// Attempts to split a compound word into its constituent parts using a vocabulary
/// Returns the original word if it cannot be split
pub fn split_compound_word(word: &str, vocab: &HashSet<String>) -> Vec<String> {
    // First check if this is a special case word that should never be split
    if is_special_case(word) {
        return vec![word.to_lowercase()];
    }

    // Use the exception terms from our centralized list
    let common_terms = EXCEPTION_TERMS
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    // If the word is in common_terms, don't split it
    if common_terms.contains(&word.to_lowercase().as_str()) {
        return vec![word.to_string()];
    }

    // Check if the word is in the vocabulary as a whole
    // This handles cases where the vocabulary contains both the compound word
    // and its constituent parts
    if vocab.contains(&word.to_lowercase()) {
        return vec![word.to_string()];
    }

    let is_valid_word = |w: &str| vocab.contains(&w.to_lowercase());

    match decompound(word, &is_valid_word, DecompositionOptions::empty()) {
        Ok(parts) if !parts.is_empty() => parts,
        _ => vec![word.to_string()],
    }
}

/// Loads a vocabulary for compound word splitting
/// This is a simplified version that could be expanded with a real dictionary
pub fn load_vocabulary() -> &'static HashSet<String> {
    static VOCABULARY: Lazy<HashSet<String>> = Lazy::new(|| {
        // This is a simplified vocabulary for demonstration
        // In a real application, this would be loaded from a file or database
        vec![
            // Common English words that might appear in compound words
            "white",
            "black",
            "list",
            "mail",
            "back",
            "ground",
            "book",
            "mark",
            "key",
            "word",
            "pass",
            "fire",
            "wall",
            "firewall",
            "water",
            "fall",
            "data",
            "base",
            "time",
            "stamp",
            "air",
            "port",
            "blue",
            "tooth",
            "green",
            "house",
            "red",
            "hat",
            "yellow",
            "pages",
            "blue",
            "print",
            "type",
            "script",
            "java",
            "script",
            "note",
            "pad",
            "web",
            "site",
            "page",
            "view",
            "code",
            "base",
            "name",
            "space",
            "class",
            "room",
            "work",
            "flow",
            "life",
            "cycle",
            "end",
            "point",
            "check",
            "box",
            "drop",
            "down",
            "pop",
            "up",
            "side",
            "bar",
            "tool",
            "tip",
            "drag",
            "drop",
            "click",
            "stream",
            "line",
            "dead",
            "lock",
            "race",
            "condition",
            "thread",
            "safe",
            "memory",
            "leak",
            "stack",
            "trace",
            "heap",
            "dump",
            "core",
            "file",
            "system",
            "disk",
            "drive",
            "hard",
            "soft",
            "ware",
            "firm",
            "middle",
            "front",
            "back",
            "end",
            "full",
            "stack",
            "dev",
            "ops",
            "micro",
            "service",
            "mono",
            "lith",
            "container",
            "docker",
            "pod",
            "cloud",
            "native",
            "server",
            "less",
            "function",
            "as",
            "service",
            "infra",
            "structure",
            "platform",
            "test",
            "driven",
            "behavior",
            "continuous",
            "integration",
            "deployment",
            "delivery",
            "pipeline",
            "git",
            "hub",
            "lab",
            "version",
            "control",
            "branch",
            "merge",
            "pull",
            "request",
            "commit",
            "push",
            "clone",
            "fork",
            "repository",
            "issue",
            "bug",
            "feature",
            "release",
            "tag",
            "semantic",
            "versioning",
            "major",
            "minor",
            "patch",
            "alpha",
            "beta",
            "stable",
            "unstable",
            "deprecated",
            "legacy",
            "modern",
            "framework",
            "library",
            "package",
            "module",
            "component",
            "prop",
            "state",
            "hook",
            "effect",
            "context",
            "provider",
            "consumer",
            "reducer",
            "action",
            "store",
            "dispatch",
            "subscribe",
            "publish",
            "event",
            "handler",
            "listener",
            "callback",
            "promise",
            "async",
            "await",
            "future",
            "stream",
            "observable",
            "reactive",
            "functional",
            "object",
            "oriented",
            "procedural",
            "declarative",
            "imperative",
            "mutable",
            "immutable",
            "pure",
            "side",
            "effect",
            "higher",
            "order",
            "first",
            "class",
            "citizen",
            "closure",
            "scope",
            "lexical",
            "dynamic",
            "static",
            "type",
            "inference",
            "checking",
            "compile",
            "time",
            "run",
            "error",
            "exception",
            "try",
            "catch",
            "finally",
            "throw",
            "raise",
            "handle",
            "logging",
            "debug",
            "info",
            "warn",
            "error",
            "fatal",
            "trace",
            "metric",
            "monitor",
            "alert",
            "notification",
            "dashboard",
            "report",
            "analytics",
            "insight",
            "data",
            "science",
            "machine",
            "learning",
            "artificial",
            "intelligence",
            "neural",
            "network",
            "deep",
            "reinforcement",
            "supervised",
            "unsupervised",
            "classification",
            "regression",
            "clustering",
            "recommendation",
            "prediction",
            "inference",
            "training",
            "validation",
            "test",
            "accuracy",
            "precision",
            "recall",
            "f1",
            "score",
            "loss",
            "function",
            "gradient",
            "descent",
            "back",
            "propagation",
            "forward",
            "pass",
            "epoch",
            "batch",
            "mini",
            "over",
            "fitting",
            "under",
            "regularization",
            "dropout",
            "batch",
            "normalization",
            "activation",
            "sigmoid",
            "tanh",
            "relu",
            "leaky",
            "softmax",
            "convolution",
            "pooling",
            "recurrent",
            "lstm",
            "gru",
            "transformer",
            "attention",
            "encoder",
            "decoder",
            "embedding",
            "token",
            "tokenization",
            "stemming",
            "lemmatization",
            "stop",
            "word",
            "n",
            "gram",
            "tf",
            "idf",
            "cosine",
            "similarity",
            "euclidean",
            "distance",
            "manhattan",
            "jaccard",
            "index",
            "precision",
            "recall",
            "relevance",
            "ranking",
            "page",
            "rank",
            "search",
            "engine",
            "crawler",
            "indexer",
            "query",
            "result",
            "snippet",
            "cache",
            "hit",
            "miss",
            "eviction",
            "policy",
            "lru",
            "fifo",
            "lifo",
            "priority",
            "queue",
            "stack",
            "heap",
            "tree",
            "binary",
            "balanced",
            "avl",
            "red",
            "black",
            "b",
            "trie",
            "hash",
            "map",
            "set",
            "list",
            "linked",
            "doubly",
            "circular",
            "array",
            "vector",
            "matrix",
            "tensor",
            "graph",
            "directed",
            "undirected",
            "weighted",
            "unweighted",
            "adjacency",
            "matrix",
            "list",
            "edge",
            "vertex",
            "node",
            "path",
            "cycle",
            "traversal",
            "breadth",
            "first",
            "depth",
            "topological",
            "sort",
            "minimum",
            "spanning",
            "tree",
            "shortest",
            "path",
            "dijkstra",
            "bellman",
            "ford",
            "floyd",
            "warshall",
            "kruskal",
            "prim",
            "greedy",
            "dynamic",
            "programming",
            "divide",
            "conquer",
            "backtracking",
            "branch",
            "bound",
            "heuristic",
            "approximation",
            "randomized",
            "parallel",
            "concurrent",
            "distributed",
            "synchronous",
            "asynchronous",
            "blocking",
            "non",
            "mutex",
            "semaphore",
            "lock",
            "atomic",
            "volatile",
            "transaction",
            "acid",
            "consistency",
            "isolation",
            "durability",
            "serializable",
            "repeatable",
            "read",
            "committed",
            "uncommitted",
            "phantom",
            "dirty",
            "read",
            "write",
            "skew",
            "conflict",
            "resolution",
            "optimistic",
            "pessimistic",
            "two",
            "phase",
            "commit",
            "rollback",
            "savepoint",
            "checkpoint",
            "recovery",
            "backup",
            "restore",
            "archive",
            "log",
            "journal",
            "redo",
            "undo",
            "write",
            "ahead",
            "logging",
            "snapshot",
            "isolation",
            "level",
            "serializable",
            "repeatable",
            "read",
            "committed",
            "uncommitted",
            "phantom",
            "dirty",
            "read",
            "write",
            "skew",
            "conflict",
            "resolution",
            "optimistic",
            "pessimistic",
            "two",
            "phase",
            "commit",
            "rollback",
            "savepoint",
            "checkpoint",
            "recovery",
            "backup",
            "restore",
            "archive",
            "log",
            "journal",
            "redo",
            "undo",
            "write",
            "ahead",
            "logging",
            "snapshot",
            "isolation",
            "level",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    });

    &VOCABULARY
}

/// Tokenize and stem a keyword, handling camel case and compound word splitting
/// This function is used by the elastic query parser to process terms in the AST
#[allow(dead_code)]
pub fn tokenize_and_stem(keyword: &str) -> Vec<String> {
    let stemmer = get_stemmer();
    let vocabulary = load_vocabulary();

    // First try camel case splitting
    let camel_parts = split_camel_case(keyword);

    if camel_parts.len() > 1 {
        // Return stemmed camel case parts, filtering out stop words
        camel_parts
            .into_iter()
            .filter(|part| !is_stop_word(part))
            .map(|part| stemmer.stem(&part).to_string())
            .collect()
    } else {
        // Try compound word splitting
        let compound_parts = split_compound_word(keyword, vocabulary);

        if compound_parts.len() > 1 {
            // Return stemmed compound parts, filtering out stop words
            compound_parts
                .into_iter()
                .filter(|part| !is_stop_word(part))
                .map(|part| stemmer.stem(&part).to_string())
                .collect()
        } else {
            // Just stem the original keyword
            vec![stemmer.stem(keyword).to_string()]
        }
    }
}

/// Tokenizes text into words by splitting on whitespace and non-alphanumeric characters,
/// removes stop words, and applies stemming. Also splits camelCase/PascalCase identifiers
/// and compound words.
///
/// The tokenization flow follows these steps:
/// 1. Split input text on whitespace
/// 2. For each token, further split on non-alphanumeric characters (except for leading "-")
/// 3. For each resulting token, check if it has mixed case
/// 4. If it has mixed case, split using camel case rules
/// 5. For each part, attempt to split compound words
/// 6. Process each part: remove stop words and apply stemming
/// 7. Collect unique tokens
/// 8. Exclude terms that were negated with a "-" prefix
pub fn tokenize(text: &str) -> Vec<String> {
    let stemmer = get_stemmer();
    let vocabulary = load_vocabulary();

    // Track negated terms to exclude them from the final result
    let mut negated_terms = HashSet::new();

    // println!("Tokenizing text: {}", text);

    // Split by whitespace and collect words
    let mut tokens = Vec::new();
    for word in text.split_whitespace() {
        // Check if this is a negated term
        let is_negated = word.starts_with('-');

        // Further split by non-alphanumeric characters
        let mut current_token = String::new();

        // Process the characters, skipping the leading "-" if this is a negated term
        let mut chars = word.chars();
        if is_negated {
            // Skip the leading "-"
            chars.next();
        }

        for c in chars {
            if c.is_alphanumeric() {
                current_token.push(c);
            } else if !current_token.is_empty() {
                // We found a non-alphanumeric character, add the current token if not empty
                if is_negated {
                    // Track this as a negated term
                    negated_terms.insert(current_token.to_lowercase());
                }
                tokens.push(current_token);
                current_token = String::new();
            }
        }

        // Add the last token if not empty
        if !current_token.is_empty() {
            if is_negated {
                // Track this as a negated term
                negated_terms.insert(current_token.to_lowercase());
            }
            tokens.push(current_token);
        }
    }

    // Create a set to track unique tokens after processing
    let mut processed_tokens = HashSet::new();
    let mut result = Vec::new();

    // Process each token: filter stop words, apply stemming, and add to result if unique
    for token in tokens {
        // Always try to split using camel case rules, even for lowercase tokens
        // This allows us to handle tokens that were already lowercased
        let parts = split_camel_case(&token);

        // Process each part
        for part in parts {
            let lowercase_part = part.to_lowercase();

            // Skip both English and programming stop words
            if is_stop_word(&lowercase_part) {
                continue;
            }

            // Skip if this is a negated term
            if negated_terms.contains(&lowercase_part) {
                continue;
            }

            // Try to split compound words
            let compound_parts = split_compound_word(&lowercase_part, vocabulary);

            for compound_part in compound_parts {
                // Skip stop words in compound parts
                if is_stop_word(&compound_part) {
                    continue;
                }

                // Skip if this is a negated term
                if negated_terms.contains(&compound_part) {
                    continue;
                }

                // Preserve the original form for all exception terms
                if is_exception_term(&compound_part)
                    && processed_tokens.insert(compound_part.clone())
                {
                    result.push(compound_part.clone());
                }

                // Also add the stemmed part if it's unique
                let stemmed_part = stemmer.stem(&compound_part).to_string();
                // Skip if the stemmed version is a negated term
                if negated_terms.contains(&stemmed_part) {
                    continue;
                }

                if processed_tokens.insert(stemmed_part.clone()) {
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

    #[test]
    fn test_split_camel_case() {
        // Test basic camel case
        assert_eq!(split_camel_case("camelCase"), vec!["camel", "case"]);

        // Test pascal case
        assert_eq!(split_camel_case("PascalCase"), vec!["pascal", "case"]);

        // Test acronyms
        assert_eq!(
            split_camel_case("parseJSONToHTML5"),
            vec!["parse", "json", "to", "html", "5"]
        );

        // Test consecutive uppercase letters
        assert_eq!(split_camel_case("APIDefinition"), vec!["api", "definition"]);

        // Test special case with OAuth2
        assert_eq!(
            split_camel_case("OAuth2Provider"),
            vec!["oauth2", "provider"]
        );

        // Test mixed case with type prefix
        assert_eq!(split_camel_case("typeIgnore"), vec!["type", "ignore"]);

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
                                                         // With compound word splitting, "endpoint" might be split into "end" and "point"
                                                         // So we check for both possibilities
        assert!(
            tokens.contains(&"endpoint".to_string())
                || (tokens.contains(&"end".to_string()) && tokens.contains(&"point".to_string()))
        );
        assert!(tokens.contains(&"meta".to_string()));

        // Test complex identifier with acronyms and numbers
        let tokens = tokenize("func ParseJSONToHTML5()");
        assert!(tokens.contains(&"pars".to_string())); // stemmed "parse"
        assert!(tokens.contains(&"json".to_string()));
        assert!(tokens.contains(&"html".to_string()));
        // Numbers 0-10 are now treated as stop words, so we don't expect "5" to be included
        // assert!(tokens.contains(&"5".to_string()));

        // Test mixed case with type prefix
        let tokens = tokenize("typeIgnore typeWhitelist");
        assert!(tokens.contains(&"ignor".to_string())); // stemmed "ignore"

        // Test compound word splitting
        let tokens = tokenize("whitelist blackmail firewall");
        // "whitelist" is now a special case word that should not be split
        assert!(tokens.contains(&"whitelist".to_string()));
        assert!(tokens.contains(&"black".to_string()));
        assert!(tokens.contains(&"mail".to_string()));
        assert!(tokens.contains(&"firewall".to_string()));

        // Test compound word in camelCase
        let tokens = tokenize("enableFirewallWhitelist");
        assert!(tokens.contains(&"enabl".to_string())); // stemmed "enable"
        assert!(tokens.contains(&"firewall".to_string())); // Now we keep firewall as a whole
                                                           // "whitelist" is now a special case word that should not be split
        assert!(tokens.contains(&"whitelist".to_string()));
    }
    #[test]
    fn test_compound_word_splitting() {
        // Test basic compound word splitting
        let vocab = HashSet::from([
            "white".to_string(),
            "list".to_string(),
            "black".to_string(),
            "mail".to_string(),
        ]);

        // "whitelist" is now a special case word that should not be split
        let parts = split_compound_word("whitelist", &vocab);
        assert_eq!(parts, vec!["whitelist".to_string()]);

        // "blackmail" is not in the special case list, so it should still be split
        let parts = split_compound_word("blackmail", &vocab);
        assert_eq!(parts, vec!["black".to_string(), "mail".to_string()]);

        // Test word that can't be split
        let parts = split_compound_word("computer", &vocab);
        assert_eq!(parts, vec!["computer".to_string()]);
    }

    #[test]
    fn test_tokenize_with_compound_words() {
        // Test tokenization with compound word splitting
        let tokens = tokenize("whitelist blackmail firewall");

        // "whitelist" is now a special case word that should not be split
        assert!(tokens.contains(&"whitelist".to_string()));
        // "blackmail" is not in the special case list, so it should still be split
        assert!(tokens.contains(&"black".to_string()));
        assert!(tokens.contains(&"mail".to_string()));
        assert!(tokens.contains(&"firewall".to_string()));
    }
}
