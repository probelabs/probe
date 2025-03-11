//! Probe is a tool for searching code repositories with powerful filtering and ranking capabilities.
//!
//! This crate provides a library interface to the probe functionality, enabling integration
//! with other tools and testing.

pub mod extract;
pub mod language;
pub mod models;
pub mod ranking;
pub mod search;

// Re-export commonly used types for convenience
pub use extract::{format_and_print_extraction_results, process_file_for_extraction};
pub use models::{CodeBlock, LimitedSearchResults, SearchLimits, SearchResult};
pub use search::perform_probe;

// Tests are defined in their respective modules with #[cfg(test)]
