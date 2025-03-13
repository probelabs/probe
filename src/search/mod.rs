pub mod file_processing;
pub mod file_search;
pub mod query;
mod result_ranking;
// Replace the old search_execution with new modules
pub mod block_merging;
pub mod cache; // New module for caching search results
pub mod elastic_query;
mod search_limiter;
mod search_options;
mod search_output;
pub mod search_runner;
pub mod search_tokens;
pub mod term_exceptions; // New module for term exceptions
pub mod tokenization; // New elastic search query parser
                      // Temporarily commented out due to compilation issues
                      // mod temp_frequency_search;

// Public exports
pub use search_options::SearchOptions;
pub use search_output::format_and_print_search_results;
pub use search_runner::perform_probe;
