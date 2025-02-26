// Re-export all search module components
mod file_processing;
mod file_search;
pub mod query;
mod result_ranking;
mod search_execution;

// Public exports
pub use search_execution::{format_and_print_search_results, perform_code_search};
