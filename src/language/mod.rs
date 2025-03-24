// Language module - provides functionality for parsing different programming languages
// using tree-sitter and extracting code blocks.

// Import submodules
pub mod block_handling;
pub mod common;
pub mod factory;
pub mod language_trait;
pub mod parser;
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
pub use parser::parse_file_for_code_blocks;
pub use test_detection::is_test_file;
#[allow(unused_imports)]
pub use tree_cache::{clear_tree_cache, get_cache_size, invalidate_cache_entry};

#[cfg(test)]
mod tests;
