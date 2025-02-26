use clap::{Parser as ClapParser, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run in CLI mode (default)
    Cli { 
        /// Path to search in
        #[arg(short, long)]
        path: PathBuf,

        /// Query patterns to search for (can specify multiple)
        #[arg(short, long, required = true)]
        query: Vec<String>,

        /// Skip AST parsing and just output unique files
        #[arg(short, long = "files-only")]
        files_only: bool,

        /// Custom patterns to ignore (in addition to .gitignore and common patterns)
        #[arg(short, long)]
        ignore: Vec<String>,

        /// Include files whose names match query words
        #[arg(short = 'n', long = "include-filenames")]
        include_filenames: bool,

        /// Reranking method to use for search results
        #[arg(short = 'r', long = "reranker", default_value = "hybrid", value_parser = ["hybrid", "bm25", "tfidf"])]
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

        /// Match files that contain any of the search terms (by default, files must contain all terms)
        #[arg(long = "any-term")]
        any_term: bool,
    },
}

#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to search in (for CLI mode)
    #[arg(short, long)]
    pub path: Option<PathBuf>,

    /// Query patterns to search for (for CLI mode)
    #[arg(short, long)]
    pub query: Vec<String>,

    /// Skip AST parsing and just output unique files (for CLI mode)
    #[arg(short, long = "files-only")]
    pub files_only: bool,

    /// Custom patterns to ignore (in addition to .gitignore and common patterns)
    #[arg(short, long)]
    pub ignore: Vec<String>,

    /// Include files whose names match query words (for CLI mode)
    #[arg(short = 'n', long = "include-filenames")]
    pub include_filenames: bool,

    /// Reranking method to use for search results
    #[arg(short = 'r', long = "reranker", default_value = "hybrid", value_parser = ["hybrid", "bm25", "tfidf"])]
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

    /// Match files that contain any of the search terms (by default, files must contain all terms)
    #[arg(long = "any-term")]
    pub any_term: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}
