use clap::{Parser as ClapParser, Subcommand};
use probe_code::lsp_integration::LspSubcommands;
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

    /// Ranking algorithm for search results. BERT models (ms-marco-*) require --features bert-reranker
    #[arg(short = 'r', long = "reranker", default_value = "bm25", value_parser = ["bm25", "hybrid", "hybrid2", "tfidf", "ms-marco-tinybert", "ms-marco-minilm-l6", "ms-marco-minilm-l12"])]
    pub reranker: String,

    /// Use frequency-based search with stemming and stopword removal (enabled by default)
    #[arg(short = 's', long = "frequency", default_value = "true")]
    pub frequency_search: bool,

    /// Perform exact search without tokenization (case-insensitive)
    #[arg(short = 'e', long = "exact")]
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

    /// Do not respect .gitignore files and patterns (gitignore is respected by default)
    #[arg(long = "no-gitignore")]
    pub no_gitignore: bool,

    /// Disable merging of adjacent code blocks after ranking (merging enabled by default)
    #[arg(long = "no-merge", default_value = "false")]
    pub no_merge: bool,

    /// Maximum number of lines between code blocks to consider them adjacent for merging (default: 5)
    #[arg(long = "merge-threshold")]
    pub merge_threshold: Option<usize>,

    /// Output only file names and line numbers without full content
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Output format (default: outline)
    /// Use 'json' or 'xml' for machine-readable output
    #[arg(short = 'o', long = "format", default_value = "outline", value_parser = ["terminal", "markdown", "plain", "json", "xml", "color", "outline", "outline-xml"])]
    pub format: String,

    /// Session ID for caching search results
    #[arg(long = "session")]
    pub session: Option<String>,

    /// Timeout in seconds for search operation (default: 30)
    #[arg(long = "timeout", default_value = "30")]
    pub timeout: u64,

    /// Natural language question for BERT reranking (requires --features bert-reranker)
    #[arg(long = "question")]
    pub question: Option<String>,

    /// Enable LSP integration for enhanced symbol information
    #[arg(long = "lsp")]
    pub lsp: bool,

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
    ///
    /// Search hints can be used to filter results by file properties:
    /// - ext:<extension>: Filter by file extension (e.g., "ext:rs")
    /// - file:<pattern>: Filter by file path pattern (e.g., "file:src/**/*.py")
    /// - dir:<pattern>: Filter by directory pattern (e.g., "dir:tests")
    /// - type:<filetype>: Filter by ripgrep file type (e.g., "type:rust")
    /// - lang:<language>: Filter by programming language (e.g., "lang:javascript")
    ///
    /// Example: probe search "function AND ext:rs" ./
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

        /// Ranking algorithm for search results. BERT models (ms-marco-*) require --features bert-reranker
        #[arg(short = 'r', long = "reranker", default_value = "bm25", value_parser = ["bm25", "hybrid", "hybrid2", "tfidf", "ms-marco-tinybert", "ms-marco-minilm-l6", "ms-marco-minilm-l12"])]
        reranker: String,

        /// Use frequency-based search with stemming and stopword removal (enabled by default)
        #[arg(short = 's', long = "frequency", default_value = "true")]
        frequency_search: bool,

        /// Perform exact search without tokenization (case-insensitive)
        #[arg(short = 'e', long = "exact")]
        exact: bool,

        /// Programming language to limit search to specific file extensions
        #[arg(short = 'l', long = "language", value_parser = [
            "rust", "rs",
            "javascript", "js", "jsx",
            "typescript", "ts", "tsx",
            "python", "py",
            "go",
            "c", "h",
            "cpp", "cc", "cxx", "hpp", "hxx",
            "java",
            "ruby", "rb",
            "php",
            "swift",
            "csharp", "cs",
            "yaml", "yml"
        ])]
        language: Option<String>,

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

        /// Do not respect .gitignore files and patterns (gitignore is respected by default)
        #[arg(long = "no-gitignore")]
        no_gitignore: bool,

        /// Disable merging of adjacent code blocks after ranking (merging enabled by default)
        #[arg(long = "no-merge", default_value = "false")]
        no_merge: bool,

        /// Maximum number of lines between code blocks to consider them adjacent for merging (default: 5)
        #[arg(long = "merge-threshold")]
        merge_threshold: Option<usize>,

        /// Output only file names and line numbers without full content
        #[arg(long = "dry-run")]
        dry_run: bool,

        /// Output format (default: outline)
        /// Use 'json' or 'xml' for machine-readable output with structured data
        #[arg(short = 'o', long = "format", default_value = "outline", value_parser = ["terminal", "markdown", "plain", "json", "xml", "color", "outline", "outline-xml"])]
        format: String,

        /// Session ID for caching search results
        #[arg(long = "session")]
        session: Option<String>,

        /// Timeout in seconds for search operation (default: 30)
        #[arg(long = "timeout", default_value = "30")]
        timeout: u64,

        /// Natural language question for BERT reranking (requires --features bert-reranker)
        #[arg(long = "question")]
        question: Option<String>,

        /// Enable LSP integration for enhanced symbol information
        #[arg(long = "lsp")]
        lsp: bool,
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

        /// Do not respect .gitignore files and patterns (gitignore is respected by default)
        #[arg(long = "no-gitignore")]
        no_gitignore: bool,

        /// Number of context lines to include before and after the extracted block
        #[arg(short = 'c', long = "context", default_value = "0")]
        context_lines: usize,

        /// Output format (default: color)
        /// Use 'json' or 'xml' for machine-readable output with structured data
        #[arg(short = 'o', long = "format", default_value = "color", value_parser = ["markdown", "plain", "json", "xml", "color", "outline-xml"])]
        format: String,

        /// Read input from clipboard instead of files
        #[arg(short = 'f', long = "from-clipboard")]
        from_clipboard: bool,
        /// Read input from a file (treats file content like stdin or clipboard)
        #[arg(short = 'F', long = "input-file")]
        input_file: Option<String>,

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

        /// Keep and display the original, unstructured input content
        #[arg(short = 'k', long = "keep-input")]
        keep_input: bool,

        /// System prompt template for LLM models (engineer, architect, code-review, code-review-template, or path to file)
        #[arg(long = "prompt")]
        prompt: Option<String>,

        /// User instructions for LLM models
        #[arg(long = "instructions")]
        instructions: Option<String>,

        /// Enable LSP integration for call hierarchy and reference graphs
        #[arg(long = "lsp")]
        lsp: bool,

        /// Include standard library references in LSP results (when using --lsp flag)
        #[arg(long = "include-stdlib")]
        include_stdlib: bool,
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
            "rust", "rs",
            "javascript", "js", "jsx",
            "typescript", "ts", "tsx",
            "python", "py",
            "go",
            "c", "h",
            "cpp", "cc", "cxx", "hpp", "hxx",
            "java",
            "ruby", "rb",
            "php",
            "swift",
            "csharp", "cs",
            "yaml", "yml"
        ])]
        language: Option<String>,

        /// Custom patterns to ignore (in addition to .gitignore and common patterns)
        #[arg(short, long)]
        ignore: Vec<String>,

        /// Allow test files in search results
        #[arg(long = "allow-tests")]
        allow_tests: bool,

        /// Do not respect .gitignore files and patterns (gitignore is respected by default)
        #[arg(long = "no-gitignore")]
        no_gitignore: bool,

        /// Maximum number of results to return
        #[arg(long = "max-results")]
        max_results: Option<usize>,

        /// Output format (default: color)
        /// Use 'json' or 'xml' for machine-readable output with structured data
        #[arg(short = 'o', long = "format", default_value = "color", value_parser = ["markdown", "plain", "json", "xml", "color", "outline-xml"])]
        format: String,
    },

    /// Run performance benchmarks
    ///
    /// This command runs comprehensive performance benchmarks using the Criterion framework.
    /// It tests various aspects of the search engine including search patterns, result limits,
    /// different options, timing infrastructure, and language parsing performance.
    /// Results are saved to the target/criterion directory.
    Benchmark {
        /// Specific benchmark to run (default: all)
        #[arg(long = "bench", value_parser = ["all", "search", "timing", "parsing"])]
        bench: Option<String>,

        /// Number of iterations for each benchmark (default: auto)
        #[arg(long = "sample-size")]
        sample_size: Option<usize>,

        /// Benchmark output format
        #[arg(long = "format", default_value = "pretty", value_parser = ["pretty", "json", "csv"])]
        format: String,

        /// Save benchmark results to file
        #[arg(long = "output")]
        output: Option<String>,

        /// Compare with previous benchmark results
        #[arg(long = "compare")]
        compare: bool,

        /// Baseline to compare against
        #[arg(long = "baseline")]
        baseline: Option<String>,

        /// Run only fast benchmarks (shorter duration)
        #[arg(long = "fast")]
        fast: bool,
    },

    /// Manage LSP daemon and language servers
    ///
    /// This command provides tools for managing the LSP daemon that powers
    /// call hierarchy and reference graph features. Use it to check daemon status,
    /// restart servers, or troubleshoot LSP integration issues.
    Lsp {
        #[command(subcommand)]
        subcommand: LspSubcommands,
    },

    /// Manage probe configuration
    ///
    /// This command provides tools for managing probe's configuration settings.
    /// Use it to view the current configuration, validate config files, or
    /// see what environment variables and config file settings are in effect.
    Config {
        #[command(subcommand)]
        subcommand: ConfigSubcommands,
    },
    Grep {
        /// Pattern to search for (regex supported)
        #[arg(value_name = "PATTERN")]
        pattern: String,

        /// Files or directories to search (defaults to current directory)
        #[arg(value_name = "PATH", default_value = ".")]
        paths: Vec<PathBuf>,

        /// Case-insensitive search
        #[arg(short = 'i', long = "ignore-case")]
        ignore_case: bool,

        /// Show line numbers (enabled by default)
        #[arg(short = 'n', long = "line-number", default_value = "true")]
        line_number: bool,

        /// Count matching lines per file instead of showing matches
        #[arg(short = 'c', long = "count")]
        count: bool,

        /// Show only filenames with matches
        #[arg(short = 'l', long = "files-with-matches")]
        files_with_matches: bool,

        /// Show only filenames without matches
        #[arg(short = 'L', long = "files-without-match")]
        files_without_match: bool,

        /// Invert match (show non-matching lines)
        #[arg(short = 'v', long = "invert-match")]
        invert_match: bool,

        /// Show NUM lines before each match
        #[arg(short = 'B', long = "before-context", value_name = "NUM")]
        before_context: Option<usize>,

        /// Show NUM lines after each match
        #[arg(short = 'A', long = "after-context", value_name = "NUM")]
        after_context: Option<usize>,

        /// Show NUM lines before and after each match
        #[arg(short = 'C', long = "context", value_name = "NUM")]
        context: Option<usize>,

        /// Custom patterns to ignore (in addition to .gitignore)
        #[arg(long = "ignore")]
        ignore: Vec<String>,

        /// Do not respect .gitignore files
        #[arg(long = "no-gitignore")]
        no_gitignore: bool,

        /// Enable colored output
        #[arg(long = "color", value_parser = ["auto", "always", "never"], default_value = "auto")]
        color: String,

        /// Maximum number of matches to show
        #[arg(short = 'm', long = "max-count")]
        max_count: Option<usize>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigSubcommands {
    /// Show the current effective configuration
    Show {
        /// Output format
        #[arg(short = 'o', long = "format", default_value = "json", value_parser = ["json", "env"])]
        format: String,
    },

    /// Validate the configuration file
    Validate {
        /// Path to config file to validate (defaults to ~/.config/probe/config.json)
        #[arg(short = 'f', long = "file")]
        file: Option<String>,
    },

    /// Set a configuration value
    Set {
        /// Configuration key in dot notation (e.g., "search.max_results", "lsp.enable_lsp")
        key: String,

        /// Value to set (will be parsed based on the key type)
        value: String,

        /// Configuration scope
        #[arg(short = 's', long = "scope", default_value = "user", value_parser = ["user", "project", "local"])]
        scope: String,

        /// Force creation of config file if it doesn't exist
        #[arg(short = 'f', long = "force")]
        force: bool,
    },

    /// Get a specific configuration value
    Get {
        /// Configuration key in dot notation (e.g., "search.max_results", "lsp.enable_lsp")
        key: String,

        /// Show the source of the configuration value
        #[arg(long = "show-source")]
        show_source: bool,
    },

    /// Reset configuration to defaults
    Reset {
        /// Configuration scope to reset
        #[arg(short = 's', long = "scope", default_value = "user", value_parser = ["user", "project", "local", "all"])]
        scope: String,

        /// Force reset without confirmation
        #[arg(short = 'f', long = "force")]
        force: bool,
    },
}
