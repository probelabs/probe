mod file_processing;
mod file_search;
pub mod query;
mod result_ranking;
// Replace the old search_execution with new modules
mod search_runner;
mod search_tokens;
mod search_output;
mod search_limiter;

// Public exports
pub use search_runner::{perform_code_search, perform_frequency_search};
pub use search_output::format_and_print_search_results;