//! Symbol UID Generation System
//!
//! This module provides a comprehensive system for generating stable, unique identifiers (UIDs)
//! for symbols across different programming languages. The system creates consistent UIDs for
//! the same symbol across different analysis runs, enabling stable symbol tracking in the
//! graph database.
//!
//! # Key Components
//!
//! * [`SymbolUIDGenerator`] - Core UID generation engine with configurable hash algorithms
//! * [`LanguageRules`] - Language-specific rules for UID generation (scope separators, overloading, etc.)
//! * [`Normalizer`] - Symbol name and signature normalization functions
//! * [`SymbolInfo`] - Extended symbol information required for UID generation
//!
//! # UID Generation Algorithm
//!
//! The system follows a hierarchical approach based on the Phase 3.1 PRD:
//!
//! 1. **USR (Unified Symbol Resolution)** - If available (e.g., from Clang), use directly
//! 2. **Anonymous symbols** - Use position-based UID with scope context
//! 3. **Local variables/parameters** - Use scope + position for uniqueness
//! 4. **Methods/constructors** - Include class context and signature
//! 5. **Global symbols** - Use fully qualified name (FQN) with normalization
//!
//! # Example Usage
//!
//! ```rust
//! use crate::symbol::{SymbolUIDGenerator, SymbolInfo, SymbolContext, HashAlgorithm};
//!
//! let generator = SymbolUIDGenerator::new();
//!
//! let symbol = SymbolInfo {
//!     name: "calculate_total".to_string(),
//!     kind: SymbolKind::Function,
//!     language: "rust".to_string(),
//!     qualified_name: Some("accounting::billing::calculate_total".to_string()),
//!     signature: Some("fn calculate_total(items: &[Item]) -> f64".to_string()),
//!     // ... other fields
//! };
//!
//! let context = SymbolContext {
//!     workspace_id: 123,
//!     file_version_id: 456,
//!     analysis_run_id: 789,
//!     scope_stack: vec!["accounting".to_string(), "billing".to_string()],
//! };
//!
//! let uid = generator.generate_uid(&symbol, &context)?;
//! // Result: "rust::accounting::billing::calculate_total#fn(items:&[Item])->f64"
//! ```
//!
//! # Language Support
//!
//! The system currently supports major programming languages:
//! - Rust, TypeScript, Python, Go, Java, C, C++
//! - Extensible architecture for adding new languages
//! - Language-specific normalization and scoping rules

pub mod language_support;
pub mod normalization;
pub mod uid_generator;

// Test module
#[cfg(test)]
mod tests;

// Re-export all public types and functions
pub use language_support::*;
pub use normalization::*;
pub use uid_generator::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// Core error types for the symbol UID system
#[derive(Debug, Error)]
pub enum UIDError {
    #[error("Invalid symbol information: {0}")]
    InvalidSymbol(String),

    #[error("Unsupported language: {language}")]
    UnsupportedLanguage { language: String },

    #[error("Hash generation failed: {0}")]
    HashError(String),

    #[error("Normalization failed for {component}: {error}")]
    NormalizationError { component: String, error: String },

    #[error("Missing required context: {context}")]
    MissingContext { context: String },

    #[error("Invalid scope format: {scope}")]
    InvalidScope { scope: String },

    #[error("Signature parsing failed: {signature}")]
    SignatureParsingError { signature: String },
}

/// Result type for UID operations
pub type UIDResult<T> = Result<T, UIDError>;

/// Symbol kinds supported by the UID generation system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    // Callable symbols
    Function,
    Method,
    Constructor,
    Destructor,

    // Type definitions
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    Union,

    // Data symbols
    Variable,
    Parameter,
    Field,
    Constant,

    // Organizational symbols
    Namespace,
    Module,
    Package,

    // Other symbols
    Macro,
    Type,
    Alias,
    Anonymous,

    // Test-specific
    Test,

    // Import/Export
    Import,
    Export,
}

impl SymbolKind {
    /// Returns true if this symbol kind represents a callable (function, method, etc.)
    pub fn is_callable(&self) -> bool {
        matches!(
            self,
            SymbolKind::Function
                | SymbolKind::Method
                | SymbolKind::Constructor
                | SymbolKind::Destructor
        )
    }

    /// Returns true if this symbol kind represents a type definition
    pub fn is_type_definition(&self) -> bool {
        matches!(
            self,
            SymbolKind::Class
                | SymbolKind::Struct
                | SymbolKind::Interface
                | SymbolKind::Trait
                | SymbolKind::Enum
                | SymbolKind::Union
        )
    }

    /// Returns true if this symbol kind represents a data symbol (variable, field, etc.)
    pub fn is_data_symbol(&self) -> bool {
        matches!(
            self,
            SymbolKind::Variable | SymbolKind::Parameter | SymbolKind::Field | SymbolKind::Constant
        )
    }

    /// Returns true if this symbol kind is likely to be scoped (local variable, parameter)
    pub fn is_scoped(&self) -> bool {
        matches!(self, SymbolKind::Variable | SymbolKind::Parameter)
    }
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind_str = match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Constructor => "constructor",
            SymbolKind::Destructor => "destructor",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Interface => "interface",
            SymbolKind::Trait => "trait",
            SymbolKind::Enum => "enum",
            SymbolKind::Union => "union",
            SymbolKind::Variable => "variable",
            SymbolKind::Parameter => "parameter",
            SymbolKind::Field => "field",
            SymbolKind::Constant => "constant",
            SymbolKind::Namespace => "namespace",
            SymbolKind::Module => "module",
            SymbolKind::Package => "package",
            SymbolKind::Macro => "macro",
            SymbolKind::Type => "type",
            SymbolKind::Alias => "alias",
            SymbolKind::Anonymous => "anonymous",
            SymbolKind::Test => "test",
            SymbolKind::Import => "import",
            SymbolKind::Export => "export",
        };
        write!(f, "{}", kind_str)
    }
}

/// Convert string to SymbolKind
impl From<&str> for SymbolKind {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "function" | "func" | "fn" => SymbolKind::Function,
            "method" | "meth" => SymbolKind::Method,
            "constructor" | "ctor" | "init" => SymbolKind::Constructor,
            "destructor" | "dtor" | "finalize" => SymbolKind::Destructor,
            "class" | "cls" => SymbolKind::Class,
            "struct" | "structure" => SymbolKind::Struct,
            "interface" | "iface" => SymbolKind::Interface,
            "trait" => SymbolKind::Trait,
            "enum" | "enumeration" => SymbolKind::Enum,
            "union" => SymbolKind::Union,
            "variable" | "var" | "let" => SymbolKind::Variable,
            "parameter" | "param" | "arg" => SymbolKind::Parameter,
            "field" | "member" => SymbolKind::Field,
            "constant" | "const" => SymbolKind::Constant,
            "namespace" | "ns" => SymbolKind::Namespace,
            "module" | "mod" => SymbolKind::Module,
            "package" | "pkg" => SymbolKind::Package,
            "macro" => SymbolKind::Macro,
            "type" | "typedef" => SymbolKind::Type,
            "alias" => SymbolKind::Alias,
            "anonymous" | "anon" => SymbolKind::Anonymous,
            "test" => SymbolKind::Test,
            "import" => SymbolKind::Import,
            "export" => SymbolKind::Export,
            _ => SymbolKind::Anonymous,
        }
    }
}

/// Symbol visibility levels
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
    Package,
    Export, // For JavaScript/TypeScript
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vis_str = match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Protected => "protected",
            Visibility::Internal => "internal",
            Visibility::Package => "package",
            Visibility::Export => "export",
        };
        write!(f, "{}", vis_str)
    }
}

impl From<&str> for Visibility {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "public" | "pub" => Visibility::Public,
            "private" | "priv" => Visibility::Private,
            "protected" | "prot" => Visibility::Protected,
            "internal" | "int" => Visibility::Internal,
            "package" | "pkg" => Visibility::Package,
            "export" | "exp" => Visibility::Export,
            _ => Visibility::Private, // Default to most restrictive
        }
    }
}

/// Location information for a symbol in source code
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolLocation {
    pub file_path: PathBuf,
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
}

impl SymbolLocation {
    /// Create a new symbol location
    pub fn new(
        file_path: PathBuf,
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
    ) -> Self {
        Self {
            file_path,
            start_line,
            start_char,
            end_line,
            end_char,
        }
    }

    /// Create a single-point location (start == end)
    pub fn point(file_path: PathBuf, line: u32, char: u32) -> Self {
        Self {
            file_path,
            start_line: line,
            start_char: char,
            end_line: line,
            end_char: char,
        }
    }

    /// Check if this location spans multiple lines
    pub fn is_multiline(&self) -> bool {
        self.start_line != self.end_line
    }

    /// Get the location as a compact string representation
    pub fn to_position_string(&self) -> String {
        if self.is_multiline() {
            format!(
                "{}:{}-{}:{}",
                self.start_line, self.start_char, self.end_line, self.end_char
            )
        } else {
            format!("{}:{}", self.start_line, self.start_char)
        }
    }
}

/// Extended symbol information required for UID generation
/// This extends the existing indexing::pipelines::SymbolInfo with additional fields
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// Symbol name (required)
    pub name: String,

    /// Symbol kind (required)
    pub kind: SymbolKind,

    /// Programming language (required)
    pub language: String,

    /// Fully qualified name (optional, preferred for global symbols)
    pub qualified_name: Option<String>,

    /// Function/method signature (optional, important for overloading)
    pub signature: Option<String>,

    /// Symbol visibility (optional)
    pub visibility: Option<Visibility>,

    /// Source location (required)
    pub location: SymbolLocation,

    /// Parent scope context (optional, for nested symbols)
    pub parent_scope: Option<String>,

    /// USR (Unified Symbol Resolution) from language servers like Clang (optional, highest priority)
    pub usr: Option<String>,

    /// Whether this symbol is a definition vs. reference
    pub is_definition: bool,

    /// Additional metadata for language-specific features
    pub metadata: HashMap<String, String>,
}

impl SymbolInfo {
    /// Create a new SymbolInfo with minimal required fields
    pub fn new(name: String, kind: SymbolKind, language: String, location: SymbolLocation) -> Self {
        Self {
            name,
            kind,
            language,
            qualified_name: None,
            signature: None,
            visibility: None,
            location,
            parent_scope: None,
            usr: None,
            is_definition: true,
            metadata: HashMap::new(),
        }
    }

    /// Builder pattern for setting optional fields
    pub fn with_qualified_name(mut self, fqn: String) -> Self {
        self.qualified_name = Some(fqn);
        self
    }

    pub fn with_signature(mut self, signature: String) -> Self {
        self.signature = Some(signature);
        self
    }

    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = Some(visibility);
        self
    }

    pub fn with_usr(mut self, usr: String) -> Self {
        self.usr = Some(usr);
        self
    }

    pub fn with_parent_scope(mut self, scope: String) -> Self {
        self.parent_scope = Some(scope);
        self
    }

    /// Check if this is an anonymous symbol (lambda, closure, etc.)
    pub fn is_anonymous(&self) -> bool {
        self.kind == SymbolKind::Anonymous
            || self.name.starts_with("lambda")
            || self.name.starts_with("anon")
            || self.name.starts_with("$")
            || self.name.contains("@")
    }

    /// Check if this symbol likely needs position-based UID (local variables, anonymous symbols)
    pub fn needs_position_based_uid(&self) -> bool {
        self.is_anonymous() || self.kind.is_scoped()
    }
}

/// Context information required for UID generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolContext {
    /// Workspace identifier
    pub workspace_id: i64,

    /// File version identifier
    pub file_version_id: i64,

    /// Programming language for this analysis
    pub language: String,

    /// Scope stack (from outermost to innermost)
    pub scope_stack: Vec<String>,
}

impl SymbolContext {
    /// Create new context
    pub fn new(workspace_id: i64, file_version_id: i64, language: String) -> Self {
        Self {
            workspace_id,
            file_version_id,
            language,
            scope_stack: Vec::new(),
        }
    }

    /// Add scope to the stack
    pub fn push_scope(mut self, scope: String) -> Self {
        self.scope_stack.push(scope);
        self
    }

    /// Get the current scope as a joined string
    pub fn current_scope(&self, separator: &str) -> String {
        self.scope_stack.join(separator)
    }

    /// Get the immediate parent scope
    pub fn parent_scope(&self) -> Option<&String> {
        self.scope_stack.last()
    }
}

/// Convert from existing indexing::pipelines::SymbolInfo to our SymbolInfo
impl From<crate::indexing::pipelines::SymbolInfo> for SymbolInfo {
    fn from(indexing_symbol: crate::indexing::pipelines::SymbolInfo) -> Self {
        let location = SymbolLocation {
            file_path: PathBuf::new(), // Will need to be set separately
            start_line: indexing_symbol.line,
            start_char: indexing_symbol.column,
            end_line: indexing_symbol.end_line.unwrap_or(indexing_symbol.line),
            end_char: indexing_symbol.end_column.unwrap_or(indexing_symbol.column),
        };

        Self {
            name: indexing_symbol.name,
            kind: SymbolKind::from(indexing_symbol.kind.as_str()),
            language: String::new(), // Will need to be set separately
            qualified_name: None,    // Not available in indexing::SymbolInfo
            signature: indexing_symbol.signature,
            visibility: indexing_symbol
                .visibility
                .map(|v| Visibility::from(v.as_str())),
            location,
            parent_scope: None,
            usr: None,
            is_definition: true, // Assume definition by default
            metadata: indexing_symbol.attributes,
        }
    }
}

/// Convert to database::SymbolState for storage
impl From<SymbolInfo> for crate::database::SymbolState {
    fn from(symbol: SymbolInfo) -> Self {
        crate::database::SymbolState {
            symbol_uid: String::new(),       // Will be generated by SymbolUIDGenerator
            file_version_id: 0,              // Will be set from context
            language: "unknown".to_string(), // Will be set from context
            name: symbol.name,
            fqn: symbol.qualified_name,
            kind: symbol.kind.to_string(),
            signature: symbol.signature,
            visibility: symbol.visibility.map(|v| v.to_string()),
            def_start_line: symbol.location.start_line,
            def_start_char: symbol.location.start_char,
            def_end_line: symbol.location.end_line,
            def_end_char: symbol.location.end_char,
            is_definition: symbol.is_definition,
            documentation: None, // Not available in SymbolInfo
            metadata: if symbol.metadata.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&symbol.metadata).unwrap_or_default())
            },
        }
    }
}
