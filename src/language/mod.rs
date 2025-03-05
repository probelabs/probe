// Language module - provides functionality for parsing different programming languages
// using tree-sitter and extracting code blocks.

// Import submodules
pub mod language_trait;
pub mod factory;
pub mod common;
pub mod parser;
pub mod test_detection;

// Language implementations
pub mod rust;
pub mod javascript;
pub mod typescript;
pub mod python;
pub mod go;
pub mod c;
pub mod cpp;
pub mod java;
pub mod ruby;
pub mod php;

// Re-export items for backward compatibility
pub use parser::parse_file_for_code_blocks;
pub use test_detection::is_test_file;

#[cfg(test)]
mod tests;