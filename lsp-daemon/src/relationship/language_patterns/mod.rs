//! Language-specific relationship patterns
//!
//! This module contains language-specific pattern implementations for extracting
//! relationships between symbols using tree-sitter AST analysis.

pub mod python;
pub mod rust;
pub mod rust_simplified;
pub mod typescript;

pub use python::PythonRelationshipExtractor;
pub use rust::RustRelationshipExtractor;
pub use rust_simplified::SimplifiedRustRelationshipExtractor;
pub use typescript::TypeScriptRelationshipExtractor;
