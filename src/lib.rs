//! Probe is a tool for searching code repositories with powerful filtering and ranking capabilities.
//!
//! This crate provides a library interface to the probe functionality, enabling integration
//! with other tools and testing.

// Make the library available as `probe` within itself
extern crate self as probe;

pub mod extract;
pub mod language;
pub mod models;
pub mod path_resolver;
pub mod query;
pub mod ranking;
pub mod search;

// Re-export commonly used types for convenience
pub use extract::{
    format_and_print_extraction_results, handle_extract, process_file_for_extraction,
};
pub use models::{CodeBlock, LimitedSearchResults, SearchLimits, SearchResult};
pub use path_resolver::resolve_path;
pub use query::{format_and_print_query_results, perform_query, AstMatch, QueryOptions};
pub use search::perform_probe;

// Tests are defined in their respective modules with #[cfg(test)]
