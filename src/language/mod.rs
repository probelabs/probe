// Language module - provides functionality for parsing different programming languages
// using tree-sitter and extracting code blocks.

// Import submodules
pub mod block_handling;
pub mod common;
pub mod factory;
pub mod language_trait;
pub mod parser;
pub mod parser_pool;
pub mod test_detection;
pub mod tree_cache;

// Language implementations
pub mod c;
pub mod cpp;
pub mod csharp;
pub mod go;
pub mod java;
pub mod javascript;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod swift;
pub mod typescript;

// Re-export items for backward compatibility
pub use parser::{parse_file_for_code_blocks, parse_file_for_code_blocks_with_tree};
pub use parser_pool::{clear_parser_pool, get_pool_stats, get_pooled_parser, return_pooled_parser};
pub use test_detection::is_test_file;
#[allow(unused_imports)]
pub use tree_cache::{
    clear_tree_cache, get_cache_size, get_or_parse_tree_pooled, invalidate_cache_entry,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod javascript_specific_tests;

#[cfg(test)]
mod typescript_specific_tests;
