use clap::{Parser as ClapParser, Subcommand};
use std::path::PathBuf;

#[derive(ClapParser, Debug)]
#[command(
    author,
    version,
    about = "AI-friendly, fully local, semantic code search tool for large codebases",
    long_about = "Probe is a powerful code search tool designed for developers and AI assistants. \
    It provides semantic code search with intelligent ranking, code block extraction, \
    and language-aware parsing. Run without arguments to see this help message."
)]
pub struct Args {
    /// Search pattern (used when no subcommand is provided)
    #[arg(value_name = "PATTERN")]
    pub pattern: Option<String>,

    /// Files or directories to search (used when no subcommand is provided)
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Skip AST parsing and just output unique files
    #[arg(short, long = "files-only")]
    pub files_only: bool,

    /// Custom patterns to ignore (in addition to .gitignore and common patterns)
    #[arg(short, long)]
    pub ignore: Vec<String>,

    /// Exclude files whose names match query words (filename matching is enabled by default)
    #[arg(short = 'n', long = "exclude-filenames")]
    pub exclude_filenames: bool,

    /// BM25 ranking for search results
    #[arg(short = 'r', long = "reranker", default_value = "bm25", value_parser = ["bm25"])]
    pub reranker: String,

    /// Use frequency-based search with stemming and stopword removal (enabled by default)
    #[arg(short = 's', long = "frequency", default_value = "true")]
    pub frequency_search: bool,

    /// Use exact matching without stemming or stopword removal
    #[arg(long = "exact")]
    pub exact: bool,

    /// Maximum number of results to return
    #[arg(long = "max-results")]
    pub max_results: Option<usize>,

    /// Maximum total bytes of code content to return
    #[arg(long = "max-bytes")]
    pub max_bytes: Option<usize>,

    /// Maximum total tokens in code content to return (for AI usage)
    #[arg(long = "max-tokens")]
    pub max_tokens: Option<usize>,

    /// Allow test files and test code blocks in search results
    #[arg(long = "allow-tests")]
    pub allow_tests: bool,

    /// Disable merging of adjacent code blocks after ranking (merging enabled by default)
    #[arg(long = "no-merge", default_value = "false")]
    pub no_merge: bool,

    /// Maximum number of lines between code blocks to consider them adjacent for merging (default: 5)
    #[arg(long = "merge-threshold")]
    pub merge_threshold: Option<usize>,

    /// Output only file names and line numbers without full content
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Output format (default: color)
    /// Use 'json' or 'xml' for machine-readable output
    #[arg(short = 'o', long = "format", default_value = "color", value_parser = ["terminal", "markdown", "plain", "json", "xml", "color"])]
    pub format: String,

    /// Session ID for caching search results
    #[arg(long = "session")]
    pub session: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Search code using patterns with intelligent ranking
    ///
    /// This command searches your codebase using regex patterns with semantic understanding.
    /// It uses frequency-based search with stemming and stopword removal by default,
    /// and ranks results using the BM25 algorithm.
    /// Results are presented as code blocks with relevant context.
    Search {
        /// Search pattern (regex supported)
        #[arg(value_name = "PATTERN")]
        pattern: String,

        /// Files or directories to search (defaults to current directory)
        #[arg(value_name = "PATH", default_value = ".")]
        paths: Vec<PathBuf>,

        /// Skip AST parsing and just output unique files
        #[arg(short, long = "files-only")]
        files_only: bool,

        /// Custom patterns to ignore (in addition to .gitignore and common patterns)
        #[arg(short, long)]
        ignore: Vec<String>,

        /// Exclude files whose names match query words (filename matching is enabled by default)
        #[arg(short = 'n', long = "exclude-filenames")]
        exclude_filenames: bool,

        /// BM25 ranking for search results
        #[arg(short = 'r', long = "reranker", default_value = "bm25", value_parser = ["bm25"])]
        reranker: String,

        /// Use frequency-based search with stemming and stopword removal (enabled by default)
        #[arg(short = 's', long = "frequency", default_value = "true")]
        frequency_search: bool,

        /// Use exact matching without stemming or stopword removal
        #[arg(long = "exact")]
        exact: bool,

        /// Maximum number of results to return
        #[arg(long = "max-results")]
        max_results: Option<usize>,

        /// Maximum total bytes of code content to return
        #[arg(long = "max-bytes")]
        max_bytes: Option<usize>,

        /// Maximum total tokens in code content to return (for AI usage)
        #[arg(long = "max-tokens")]
        max_tokens: Option<usize>,

        /// Allow test files and test code blocks in search results
        #[arg(long = "allow-tests")]
        allow_tests: bool,

        /// Disable merging of adjacent code blocks after ranking (merging enabled by default)
        #[arg(long = "no-merge", default_value = "false")]
        no_merge: bool,

        /// Maximum number of lines between code blocks to consider them adjacent for merging (default: 5)
        #[arg(long = "merge-threshold")]
        merge_threshold: Option<usize>,

        /// Output only file names and line numbers without full content
        #[arg(long = "dry-run")]
        dry_run: bool,

        /// Output format (default: color)
        /// Use 'json' or 'xml' for machine-readable output with structured data
        #[arg(short = 'o', long = "format", default_value = "color", value_parser = ["terminal", "markdown", "plain", "json", "xml", "color"])]
        format: String,

        /// Session ID for caching search results
        #[arg(long = "session")]
        session: Option<String>,
    },

    /// Extract code blocks from files
    ///
    /// This command extracts code blocks from files based on file paths and optional line numbers.
    /// When a line number is specified (e.g., file.rs:10), the command uses tree-sitter to find
    /// the closest suitable parent node (function, struct, class, etc.) for that line.
    /// You can also specify a symbol name using the hash syntax (e.g., file.rs#function_name) to
    /// extract the code block for that specific symbol.
    Extract {
        /// Files to extract from (can include line numbers with colon, e.g., file.rs:10, or symbol names with hash, e.g., file.rs#function_name)
        #[arg(value_name = "FILES")]
        files: Vec<String>,

        /// Custom patterns to ignore (in addition to .gitignore and common patterns)
        #[arg(short, long)]
        ignore: Vec<String>,

        /// Number of context lines to include before and after the extracted block
        #[arg(short = 'c', long = "context", default_value = "0")]
        context_lines: usize,

        /// Output format (default: color)
        /// Use 'json' or 'xml' for machine-readable output with structured data
        #[arg(short = 'o', long = "format", default_value = "color", value_parser = ["markdown", "plain", "json", "xml", "color"])]
        format: String,

        /// Read input from clipboard instead of files
        #[arg(short = 'f', long = "from-clipboard")]
        from_clipboard: bool,

        /// Write output to clipboard
        #[arg(short = 't', long = "to-clipboard")]
        to_clipboard: bool,

        /// Output only file names and line numbers without full content
        #[arg(long = "dry-run")]
        dry_run: bool,

        /// Parse input as git diff format
        #[arg(long = "diff")]
        diff: bool,

        /// Allow test files and test code blocks in extraction results (only applies when reading from stdin or clipboard)
        #[arg(long = "allow-tests")]
        allow_tests: bool,
    },

    /// Search code using AST patterns for precise structural matching
    ///
    /// This command uses ast-grep to search for structural patterns in code.
    /// It allows for more precise code searching based on the Abstract Syntax Tree,
    /// which is particularly useful for finding specific code structures regardless
    /// of variable names or formatting. This is more powerful than regex for
    /// certain types of code searches.
    Query {
        /// AST pattern to search for (e.g., "fn $NAME() { $$$BODY }")
        #[arg(value_name = "PATTERN")]
        pattern: String,

        /// Files or directories to search (defaults to current directory)
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Programming language to use for parsing (auto-detected if not specified)
        #[arg(short = 'l', long = "language", value_parser = [
            "rust", "javascript", "typescript", "python", "go",
            "c", "cpp", "java", "ruby", "php", "swift", "csharp"
        ])]
        language: Option<String>,

        /// Custom patterns to ignore (in addition to .gitignore and common patterns)
        #[arg(short, long)]
        ignore: Vec<String>,

        /// Allow test files in search results
        #[arg(long = "allow-tests")]
        allow_tests: bool,

        /// Maximum number of results to return
        #[arg(long = "max-results")]
        max_results: Option<usize>,

        /// Output format (default: color)
        /// Use 'json' or 'xml' for machine-readable output with structured data
        #[arg(short = 'o', long = "format", default_value = "color", value_parser = ["markdown", "plain", "json", "xml", "color"])]
        format: String,
    },
}
