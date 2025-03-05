// Language module - provides functionality for parsing different programming languages
// using tree-sitter and extracting code blocks.

// Import submodules
pub mod block_handling;
pub mod common;
pub mod factory;
pub mod language_trait;
pub mod parser;
pub mod test_detection;

// Language implementations
pub mod c;
pub mod cpp;
pub mod go;
pub mod java;
pub mod javascript;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod typescript;

// Re-export items for backward compatibility
pub use parser::parse_file_for_code_blocks;
pub use test_detection::is_test_file;

#[cfg(test)]
mod tests;
