//! Language-specific relationship patterns
//!
//! This module contains language-specific pattern implementations for extracting
//! relationships between symbols using tree-sitter AST analysis.

pub mod python;
pub mod rust;
pub mod typescript;

pub use python::PythonRelationshipExtractor;
pub use rust::RustRelationshipExtractor;
pub use typescript::TypeScriptRelationshipExtractor;
