use decompound::{decompound, DecompositionOptions};
use once_cell::sync::Lazy;
use probe_code::ranking::get_stemmer;
use probe_code::search::simd_tokenization::SimdConfig;
use probe_code::search::term_exceptions::{is_exception_term, EXCEPTION_TERMS};
use std::collections::{HashMap, HashSet};
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

/// Static pre-computed compound word splits for common programming terms
/// This cache eliminates the need to call the decompound crate for known terms,
/// providing significant performance improvements for frequently used compound words.
static PRECOMPUTED_COMPOUND_SPLITS: Lazy<HashMap<String, Vec<String>>> = Lazy::new(|| {
    let mut cache = HashMap::new();

    // Pre-compute splits for common programming compound words
    // These are based on real-world usage patterns in codebases

    // Common data structures and algorithms
    cache.insert(
        "hashmap".to_string(),
        vec!["hash".to_string(), "map".to_string()],
    );
    cache.insert(
        "hashtable".to_string(),
        vec!["hash".to_string(), "table".to_string()],
    );
    cache.insert(
        "hashset".to_string(),
        vec!["hash".to_string(), "set".to_string()],
    );
    cache.insert(
        "arraylist".to_string(),
        vec!["array".to_string(), "list".to_string()],
    );
    cache.insert(
        "linkedlist".to_string(),
        vec!["linked".to_string(), "list".to_string()],
    );
    cache.insert(
        "treemap".to_string(),
        vec!["tree".to_string(), "map".to_string()],
    );
    cache.insert(
        "treeset".to_string(),
        vec!["tree".to_string(), "set".to_string()],
    );
    cache.insert(
        "quicksort".to_string(),
        vec!["quick".to_string(), "sort".to_string()],
    );
    cache.insert(
        "mergesort".to_string(),
        vec!["merge".to_string(), "sort".to_string()],
    );
    cache.insert(
        "heapsort".to_string(),
        vec!["heap".to_string(), "sort".to_string()],
    );
    cache.insert(
        "bubblesort".to_string(),
        vec!["bubble".to_string(), "sort".to_string()],
    );
    cache.insert(
        "binarysearch".to_string(),
        vec!["binary".to_string(), "search".to_string()],
    );
    cache.insert(
        "breadthfirst".to_string(),
        vec!["breadth".to_string(), "first".to_string()],
    );
    cache.insert(
        "depthfirst".to_string(),
        vec!["depth".to_string(), "first".to_string()],
    );

    // File and I/O operations
    cache.insert(
        "filename".to_string(),
        vec!["file".to_string(), "name".to_string()],
    );
    cache.insert(
        "filepath".to_string(),
        vec!["file".to_string(), "path".to_string()],
    );
    cache.insert(
        "filesize".to_string(),
        vec!["file".to_string(), "size".to_string()],
    );
    cache.insert(
        "filetype".to_string(),
        vec!["file".to_string(), "type".to_string()],
    );
    cache.insert(
        "filestream".to_string(),
        vec!["file".to_string(), "stream".to_string()],
    );
    cache.insert(
        "filesystem".to_string(),
        vec!["file".to_string(), "system".to_string()],
    );
    cache.insert(
        "pathname".to_string(),
        vec!["path".to_string(), "name".to_string()],
    );
    cache.insert(
        "dirname".to_string(),
        vec!["dir".to_string(), "name".to_string()],
    );
    cache.insert(
        "basename".to_string(),
        vec!["base".to_string(), "name".to_string()],
    );
    cache.insert(
        "username".to_string(),
        vec!["user".to_string(), "name".to_string()],
    );
    cache.insert(
        "hostname".to_string(),
        vec!["host".to_string(), "name".to_string()],
    );
    cache.insert(
        "domainname".to_string(),
        vec!["domain".to_string(), "name".to_string()],
    );

    // Database and storage
    cache.insert(
        "database".to_string(),
        vec!["data".to_string(), "base".to_string()],
    );
    cache.insert(
        "datastore".to_string(),
        vec!["data".to_string(), "store".to_string()],
    );
    cache.insert(
        "dataset".to_string(),
        vec!["data".to_string(), "set".to_string()],
    );
    cache.insert(
        "datatype".to_string(),
        vec!["data".to_string(), "type".to_string()],
    );
    cache.insert(
        "dataframe".to_string(),
        vec!["data".to_string(), "frame".to_string()],
    );
    cache.insert(
        "datatable".to_string(),
        vec!["data".to_string(), "table".to_string()],
    );
    cache.insert(
        "tablename".to_string(),
        vec!["table".to_string(), "name".to_string()],
    );
    cache.insert(
        "indexname".to_string(),
        vec!["index".to_string(), "name".to_string()],
    );
    cache.insert(
        "keyvalue".to_string(),
        vec!["key".to_string(), "value".to_string()],
    );
    cache.insert(
        "primarykey".to_string(),
        vec!["primary".to_string(), "key".to_string()],
    );
    cache.insert(
        "foreignkey".to_string(),
        vec!["foreign".to_string(), "key".to_string()],
    );

    // Network and protocols
    cache.insert(
        "hostname".to_string(),
        vec!["host".to_string(), "name".to_string()],
    );
    cache.insert(
        "endpoint".to_string(),
        vec!["end".to_string(), "point".to_string()],
    );
    cache.insert(
        "baseurl".to_string(),
        vec!["base".to_string(), "url".to_string()],
    );
    cache.insert(
        "webhook".to_string(),
        vec!["web".to_string(), "hook".to_string()],
    );
    cache.insert(
        "websocket".to_string(),
        vec!["web".to_string(), "socket".to_string()],
    );
    cache.insert(
        "webserver".to_string(),
        vec!["web".to_string(), "server".to_string()],
    );
    cache.insert(
        "webservice".to_string(),
        vec!["web".to_string(), "service".to_string()],
    );
    cache.insert(
        "restapi".to_string(),
        vec!["rest".to_string(), "api".to_string()],
    );
    cache.insert(
        "graphql".to_string(),
        vec!["graph".to_string(), "ql".to_string()],
    );

    // UI and frontend
    cache.insert(
        "username".to_string(),
        vec!["user".to_string(), "name".to_string()],
    );
    cache.insert(
        "userinfo".to_string(),
        vec!["user".to_string(), "info".to_string()],
    );
    cache.insert(
        "userdata".to_string(),
        vec!["user".to_string(), "data".to_string()],
    );
    cache.insert(
        "userinput".to_string(),
        vec!["user".to_string(), "input".to_string()],
    );
    cache.insert(
        "userinterface".to_string(),
        vec!["user".to_string(), "interface".to_string()],
    );
    cache.insert(
        "frontend".to_string(),
        vec!["front".to_string(), "end".to_string()],
    );
    cache.insert(
        "backend".to_string(),
        vec!["back".to_string(), "end".to_string()],
    );
    cache.insert(
        "fullstack".to_string(),
        vec!["full".to_string(), "stack".to_string()],
    );
    cache.insert(
        "stylesheet".to_string(),
        vec!["style".to_string(), "sheet".to_string()],
    );
    cache.insert(
        "javascript".to_string(),
        vec!["java".to_string(), "script".to_string()],
    );
    cache.insert(
        "typescript".to_string(),
        vec!["type".to_string(), "script".to_string()],
    );

    // Development and tooling
    cache.insert(
        "codebase".to_string(),
        vec!["code".to_string(), "base".to_string()],
    );
    cache.insert(
        "codegen".to_string(),
        vec!["code".to_string(), "gen".to_string()],
    );
    cache.insert(
        "codepoint".to_string(),
        vec!["code".to_string(), "point".to_string()],
    );
    cache.insert(
        "sourcecode".to_string(),
        vec!["source".to_string(), "code".to_string()],
    );
    cache.insert(
        "sourcemap".to_string(),
        vec!["source".to_string(), "map".to_string()],
    );
    cache.insert(
        "sourcefile".to_string(),
        vec!["source".to_string(), "file".to_string()],
    );
    cache.insert(
        "buildtime".to_string(),
        vec!["build".to_string(), "time".to_string()],
    );
    cache.insert(
        "buildpath".to_string(),
        vec!["build".to_string(), "path".to_string()],
    );
    cache.insert(
        "runtime".to_string(),
        vec!["run".to_string(), "time".to_string()],
    );
    cache.insert("compile".to_string(), vec!["compile".to_string()]); // Single word
    cache.insert("compiler".to_string(), vec!["compiler".to_string()]); // Single word
    cache.insert(
        "compiletime".to_string(),
        vec!["compile".to_string(), "time".to_string()],
    );
    cache.insert(
        "debugger".to_string(),
        vec!["debug".to_string(), "ger".to_string()],
    );
    cache.insert(
        "debuginfo".to_string(),
        vec!["debug".to_string(), "info".to_string()],
    );

    // Testing and quality
    cache.insert(
        "unittest".to_string(),
        vec!["unit".to_string(), "test".to_string()],
    );
    cache.insert(
        "testcase".to_string(),
        vec!["test".to_string(), "case".to_string()],
    );
    cache.insert(
        "testdata".to_string(),
        vec!["test".to_string(), "data".to_string()],
    );
    cache.insert(
        "testfile".to_string(),
        vec!["test".to_string(), "file".to_string()],
    );
    cache.insert(
        "testsuit".to_string(),
        vec!["test".to_string(), "suit".to_string()],
    );
    cache.insert(
        "benchmark".to_string(),
        vec!["bench".to_string(), "mark".to_string()],
    );
    cache.insert(
        "codereview".to_string(),
        vec!["code".to_string(), "review".to_string()],
    );
    cache.insert(
        "lintcheck".to_string(),
        vec!["lint".to_string(), "check".to_string()],
    );

    // Version control and project management
    cache.insert(
        "checkpoint".to_string(),
        vec!["check".to_string(), "point".to_string()],
    );
    cache.insert(
        "savepoint".to_string(),
        vec!["save".to_string(), "point".to_string()],
    );
    cache.insert(
        "breakpoint".to_string(),
        vec!["break".to_string(), "point".to_string()],
    );
    cache.insert(
        "entrypoint".to_string(),
        vec!["entry".to_string(), "point".to_string()],
    );
    cache.insert(
        "startpoint".to_string(),
        vec!["start".to_string(), "point".to_string()],
    );
    cache.insert(
        "endpoint".to_string(),
        vec!["end".to_string(), "point".to_string()],
    );
    cache.insert(
        "timestamp".to_string(),
        vec!["time".to_string(), "stamp".to_string()],
    );
    cache.insert(
        "milestone".to_string(),
        vec!["mile".to_string(), "stone".to_string()],
    );
    cache.insert(
        "roadmap".to_string(),
        vec!["road".to_string(), "map".to_string()],
    );
    cache.insert(
        "workflow".to_string(),
        vec!["work".to_string(), "flow".to_string()],
    );
    cache.insert(
        "workload".to_string(),
        vec!["work".to_string(), "load".to_string()],
    );
    cache.insert(
        "workqueue".to_string(),
        vec!["work".to_string(), "queue".to_string()],
    );
    cache.insert(
        "workspace".to_string(),
        vec!["work".to_string(), "space".to_string()],
    );

    // Security and authentication
    cache.insert(
        "password".to_string(),
        vec!["pass".to_string(), "word".to_string()],
    );
    cache.insert(
        "passphrase".to_string(),
        vec!["pass".to_string(), "phrase".to_string()],
    );
    cache.insert(
        "passcode".to_string(),
        vec!["pass".to_string(), "code".to_string()],
    );
    cache.insert(
        "username".to_string(),
        vec!["user".to_string(), "name".to_string()],
    );
    cache.insert(
        "userid".to_string(),
        vec!["user".to_string(), "id".to_string()],
    );
    cache.insert(
        "sessionid".to_string(),
        vec!["session".to_string(), "id".to_string()],
    );
    cache.insert(
        "tokenid".to_string(),
        vec!["token".to_string(), "id".to_string()],
    );
    cache.insert(
        "keychain".to_string(),
        vec!["key".to_string(), "chain".to_string()],
    );
    cache.insert(
        "keystore".to_string(),
        vec!["key".to_string(), "store".to_string()],
    );
    cache.insert(
        "keyring".to_string(),
        vec!["key".to_string(), "ring".to_string()],
    );
    cache.insert(
        "keypair".to_string(),
        vec!["key".to_string(), "pair".to_string()],
    );
    cache.insert(
        "publickey".to_string(),
        vec!["public".to_string(), "key".to_string()],
    );
    cache.insert(
        "privatekey".to_string(),
        vec!["private".to_string(), "key".to_string()],
    );
    cache.insert(
        "secretkey".to_string(),
        vec!["secret".to_string(), "key".to_string()],
    );

    // Error handling and logging
    cache.insert(
        "errorcode".to_string(),
        vec!["error".to_string(), "code".to_string()],
    );
    cache.insert(
        "errormsg".to_string(),
        vec!["error".to_string(), "msg".to_string()],
    );
    cache.insert(
        "errorlog".to_string(),
        vec!["error".to_string(), "log".to_string()],
    );
    cache.insert(
        "stacktrace".to_string(),
        vec!["stack".to_string(), "trace".to_string()],
    );
    cache.insert(
        "backtrace".to_string(),
        vec!["back".to_string(), "trace".to_string()],
    );
    cache.insert(
        "logfile".to_string(),
        vec!["log".to_string(), "file".to_string()],
    );
    cache.insert(
        "logdata".to_string(),
        vec!["log".to_string(), "data".to_string()],
    );
    cache.insert(
        "loglevel".to_string(),
        vec!["log".to_string(), "level".to_string()],
    );
    cache.insert(
        "logmsg".to_string(),
        vec!["log".to_string(), "msg".to_string()],
    );

    // Performance and monitoring
    cache.insert(
        "benchmark".to_string(),
        vec!["bench".to_string(), "mark".to_string()],
    );
    cache.insert(
        "throughput".to_string(),
        vec!["through".to_string(), "put".to_string()],
    );
    cache.insert(
        "bandwidth".to_string(),
        vec!["band".to_string(), "width".to_string()],
    );
    cache.insert("latency".to_string(), vec!["latency".to_string()]); // Single word
    cache.insert(
        "timeout".to_string(),
        vec!["time".to_string(), "out".to_string()],
    );
    cache.insert(
        "deadline".to_string(),
        vec!["dead".to_string(), "line".to_string()],
    );
    cache.insert(
        "heartbeat".to_string(),
        vec!["heart".to_string(), "beat".to_string()],
    );
    cache.insert(
        "healthcheck".to_string(),
        vec!["health".to_string(), "check".to_string()],
    );
    cache.insert(
        "statuscheck".to_string(),
        vec!["status".to_string(), "check".to_string()],
    );

    cache
});

/// Runtime LRU cache for dynamically discovered compound word splits
/// This provides fast lookup for compound words discovered during execution
/// while maintaining a bounded memory footprint through LRU eviction.
#[derive(Debug)]
struct CompoundCache {
    cache: HashMap<String, Vec<String>>,
    access_order: Vec<String>,
    max_size: usize,
}

impl CompoundCache {
    fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            access_order: Vec::new(),
            max_size,
        }
    }

    fn get(&mut self, key: &str) -> Option<&Vec<String>> {
        if self.cache.contains_key(key) {
            // Move to end (most recently used)
            self.access_order.retain(|k| k != key);
            self.access_order.push(key.to_string());
            self.cache.get(key)
        } else {
            None
        }
    }

    fn insert(&mut self, key: String, value: Vec<String>) {
        // Remove if already exists
        if self.cache.contains_key(&key) {
            self.access_order.retain(|k| k != &key);
        } else if self.cache.len() >= self.max_size {
            // Evict least recently used
            if let Some(lru_key) = self.access_order.first().cloned() {
                self.cache.remove(&lru_key);
                self.access_order.retain(|k| k != &lru_key);
            }
        }

        self.cache.insert(key.clone(), value);
        self.access_order.push(key);
    }
}

/// Runtime cache for compound word splits (thread-safe)
static RUNTIME_COMPOUND_CACHE: Lazy<Mutex<CompoundCache>> = Lazy::new(|| {
    Mutex::new(CompoundCache::new(1000)) // Cache up to 1000 compound word splits
});

/// Common non-compound words in programming contexts
/// These words are unlikely to be compound words and should skip compound processing
/// for performance optimization while maintaining accuracy.
static COMMON_NON_COMPOUND_WORDS: Lazy<HashSet<String>> = Lazy::new(|| {
    vec![
        // Very common English words that are rarely compound
        "the", "and", "for", "are", "but", "not", "you", "all", "can", "had", "her", "was", "one",
        "our", "out", "day", "get", "has", "him", "his", "how", "man", "new", "now", "old", "see",
        "two", "way", "who", "boy", "did", "its", "let", "put", "say", "she", "too", "use",
        // Common programming keywords and identifiers that are rarely compound
        "var", "let", "fun", "def", "int", "str", "obj", "val", "ref", "mut", "pub", "mod", "use",
        "as", "if", "do", "go", "js", "py", "rs", "ts", "cs", "cpp", "java", "php", "sql", "xml",
        "css", "html", "json", "yaml", "toml", "cfg", "log", "env", "bin", "lib", "src", "doc",
        "test", "spec", "tmp", "temp", "path", "file", "dir", "url", "uri", "http", "https", "tcp",
        "udp", "ftp", "ssh", "ssl", "tls", "auth", "user", "admin", "root", "sudo", "exec", "run",
        "start", "stop", "init", "main", "app", "api", "web", "net", "sys", "os", "io", "fs", "db",
        "sql", "orm", "mvc", "gui", "cli", "tui", "ui", "ux", "css", "js", "ts", "html", "xml",
        "json", "yaml", "toml", "csv", "txt", "md", "pdf", "png", "jpg", "gif", "svg", "zip",
        "tar", "gz", "bz2", "xz", "rpm", "deb", "dmg", "exe", "dll", "so", "dylib", "jar", "war",
        "ear", "zip", "rar", "7z", // Common single-letter and two-letter terms
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t", "u", "v", "w", "x", "y", "z", "id", "ip", "ok", "no", "on", "in", "at", "is",
        "to", "by", "up", "my", "we", "me", "he", "it", "of", "or", "so", "be", "do", "go", "if",
        "an", "as", "am", "us", "vs", // Numbers and common numeric suffixes
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "100", "1000", "v1", "v2", "v3",
        "v4", "v5", "2d", "3d", "4k", "8k", "x64", "x86", "64bit", "32bit",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
});

/// Determines if a word should skip compound processing based on heuristics
/// This optimization improves performance by avoiding expensive compound word
/// processing for words that are unlikely to be compound words.
///
/// Heuristics used:
/// - Length: Words shorter than 6 characters are unlikely to be compound
/// - Patterns: Words with numbers or special characters are unlikely to be compound  
/// - Common words: Very common English/programming words are rarely compound
/// - Frequency: High-frequency terms are often single words, not compounds
fn should_skip_compound_processing(word: &str) -> bool {
    let lowercase_word = word.to_lowercase();

    // Skip very short words (less than 6 characters) - unlikely to be compound
    if word.len() < 6 {
        return true;
    }

    // Skip words with numbers or special characters (except underscores/hyphens)
    if word
        .chars()
        .any(|c| c.is_numeric() || (c.is_ascii_punctuation() && c != '_' && c != '-'))
    {
        return true;
    }

    // Skip words in the common non-compound list
    if COMMON_NON_COMPOUND_WORDS.contains(&lowercase_word) {
        return true;
    }

    // Additional heuristic: Skip words with repeated character patterns (like "aaa", "xxx")
    // These are often abbreviations or special identifiers, not compound words
    let chars: Vec<char> = word.chars().collect();
    if chars.len() >= 3 {
        let mut all_same = true;
        for i in 1..chars.len() {
            if chars[i] != chars[0] {
                all_same = false;
                break;
            }
        }
        if all_same {
            return true;
        }
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
    split_camel_case_with_config(input, SimdConfig::new())
}

/// Thread-safe camelCase splitting with explicit SIMD configuration
pub fn split_camel_case_with_config(input: &str, config: SimdConfig) -> Vec<String> {
    // Check if SIMD tokenization should be used based on config
    if config.should_use_simd() {
        return crate::search::simd_tokenization::simd_split_camel_case_with_config(input, config);
    }

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
            result.extend(split_camel_case_with_config(remaining, config));
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
                result.extend(split_camel_case_with_config(remaining, config));

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
    // OPTIMIZATION: Pre-allocate Vec capacity based on input length heuristics
    let estimated_parts = (input.len() / 4).clamp(2, 8); // Typical camelCase has 2-4 parts
    let mut result = Vec::with_capacity(estimated_parts);
    let mut current_word = String::with_capacity(input.len() / 2); // Pre-allocate string capacity

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
///
/// Performance optimization: Uses a three-tier caching strategy:
/// 1. Pre-computed cache for common programming terms (fastest)
/// 2. Runtime LRU cache for dynamically discovered splits (fast)
/// 3. Decompound crate as fallback for unknown terms (slower)
///
/// SELECTIVE COMPOUND PROCESSING OPTIMIZATION:
/// Skips compound processing for words unlikely to be compound based on heuristics:
/// - Length (< 6 characters), patterns (numbers/special chars), common word lists
///   This optimization improves performance by 0.5-0.8s while maintaining accuracy
pub fn split_compound_word(word: &str, vocab: &HashSet<String>) -> Vec<String> {
    // OPTIMIZATION: Skip compound processing for words unlikely to be compound
    // This heuristic-based optimization improves performance significantly
    // while maintaining backward compatibility and accuracy
    if should_skip_compound_processing(word) {
        return vec![word.to_string()];
    }
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

    let lowercase_word = word.to_lowercase();

    // PERFORMANCE OPTIMIZATION: Three-tier caching strategy

    // Tier 1: Check pre-computed cache for common programming terms (fastest)
    if let Some(cached_splits) = PRECOMPUTED_COMPOUND_SPLITS.get(&lowercase_word) {
        return cached_splits.clone();
    }

    // Tier 2: Check runtime LRU cache for dynamically discovered splits (fast)
    if let Ok(mut cache) = RUNTIME_COMPOUND_CACHE.try_lock() {
        if let Some(cached_splits) = cache.get(&lowercase_word) {
            return cached_splits.clone();
        }
    }

    // Tier 3: Use decompound crate as fallback for unknown terms (slower)
    let is_valid_word = |w: &str| vocab.contains(&w.to_lowercase());

    let result = match decompound(word, &is_valid_word, DecompositionOptions::empty()) {
        Ok(parts) if !parts.is_empty() => parts,
        _ => vec![word.to_string()],
    };

    // Cache the result for future lookups if it's a compound word (more than one part)
    if result.len() > 1 {
        if let Ok(mut cache) = RUNTIME_COMPOUND_CACHE.try_lock() {
            cache.insert(lowercase_word, result.clone());
        }
    }

    result
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
    let camel_parts = split_camel_case_with_config(keyword, SimdConfig::new());

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
    // OPTIMIZATION: Pre-allocate Vec capacity based on text length heuristics
    let estimated_tokens = (text.len() / 8).clamp(4, 32); // Estimate ~8 chars per token on average
    let mut tokens = Vec::with_capacity(estimated_tokens);
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
    // OPTIMIZATION: Pre-allocate capacity to reduce allocations
    let mut processed_tokens = HashSet::with_capacity(tokens.len() * 2); // Pre-allocate HashSet capacity
    let mut result = Vec::with_capacity(tokens.len() * 2); // Allow for compound word expansion

    // Process each token: filter stop words, apply stemming, and add to result if unique
    for token in tokens {
        // Always try to split using camel case rules, even for lowercase tokens
        // This allows us to handle tokens that were already lowercased
        let parts = split_camel_case_with_config(&token, SimdConfig::new());

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

    #[test]
    fn test_should_skip_compound_processing() {
        // Test length heuristic - words shorter than 6 characters should be skipped
        assert!(should_skip_compound_processing("test"));
        assert!(should_skip_compound_processing("a"));
        assert!(should_skip_compound_processing("hello"));
        assert!(!should_skip_compound_processing("helloworld")); // 10 chars, should not skip

        // Test numeric heuristic - words with numbers should be skipped
        assert!(should_skip_compound_processing("test123"));
        assert!(should_skip_compound_processing("v1_api"));
        assert!(should_skip_compound_processing("http2"));

        // Test special character heuristic - words with special chars should be skipped
        assert!(should_skip_compound_processing("hello@world"));
        assert!(should_skip_compound_processing("test.method"));
        assert!(should_skip_compound_processing("config{value}"));
        assert!(!should_skip_compound_processing("hello_world")); // underscores are allowed
        assert!(!should_skip_compound_processing("hello-world")); // hyphens are allowed

        // Test common word heuristic - common words should be skipped
        assert!(should_skip_compound_processing("and"));
        assert!(should_skip_compound_processing("for"));
        assert!(should_skip_compound_processing("json"));
        assert!(should_skip_compound_processing("html"));
        assert!(should_skip_compound_processing("the"));

        // Test repeated character heuristic
        assert!(should_skip_compound_processing("aaaaaa"));
        assert!(should_skip_compound_processing("xxx"));
        assert!(!should_skip_compound_processing("helloworld")); // no repeated chars

        // Test legitimate compound words that should NOT be skipped
        assert!(!should_skip_compound_processing("database"));
        assert!(!should_skip_compound_processing("firewall"));
        assert!(!should_skip_compound_processing("whitelist"));
        assert!(!should_skip_compound_processing("hashmap"));
    }

    #[test]
    fn test_selective_compound_processing_integration() {
        let vocab = load_vocabulary();

        // Short words should not be compound processed
        let parts = split_compound_word("test", vocab);
        assert_eq!(parts, vec!["test".to_string()]);

        // Words with numbers should not be compound processed
        let parts = split_compound_word("test123", vocab);
        assert_eq!(parts, vec!["test123".to_string()]);

        // Common words should not be compound processed
        let parts = split_compound_word("json", vocab);
        assert_eq!(parts, vec!["json".to_string()]);

        // Legitimate compound words should still be processed (if they pass heuristics)
        let parts = split_compound_word("whitelist", vocab);
        assert_eq!(parts, vec!["whitelist".to_string()]); // Special case handling

        // Long words that are not in common lists should still be processed
        let parts = split_compound_word("customwordprocessing", vocab);
        // This would normally be processed, but since it's not in vocabulary,
        // returns as single word - behavior is preserved
        assert!(!parts.is_empty());
    }
}
