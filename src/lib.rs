//! Code-search is a tool for searching code repositories with powerful filtering and ranking capabilities.
//!
//! This crate provides a library interface to the code-search functionality, enabling integration
//! with other tools and testing.

pub mod language;
pub mod models;
pub mod ranking;
pub mod search;

// Re-export commonly used types for convenience
pub use models::{CodeBlock, LimitedSearchResults, SearchLimits, SearchResult};
pub use search::perform_code_search;

// Tests are defined in their respective modules with #[cfg(test)]
