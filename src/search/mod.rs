pub mod file_processing;
pub mod query;
mod result_ranking;
// Replace the old search_execution with new modules
pub mod block_merging;
pub mod cache; // New module for caching search results
pub mod elastic_query;
pub mod file_list_cache; // New module for caching file lists
mod search_limiter;
mod search_options;
mod search_output;
pub mod search_runner;
pub mod search_tokens;
pub mod simd_pattern_matching;
pub mod simd_tokenization; // SIMD-accelerated tokenization
pub mod term_exceptions; // New module for term exceptions
pub mod timeout; // New module for timeout functionality
pub mod tokenization; // New elastic search query parser // SIMD-accelerated pattern matching
                      // Temporarily commented out due to compilation issues
                      // mod temp_frequency_search;

#[cfg(test)]
mod file_processing_tests;

// Public exports
pub use search_options::SearchOptions;
pub use search_output::format_and_print_search_results;
pub use search_runner::perform_probe;
