use anyhow::Result;
use std::path::Path;

/// Resolve the best LSP cursor position for a symbol by snapping
/// to the identifier using tree-sitter when possible.
///
/// Inputs and outputs are 0-based (LSP-compatible) line/column.
/// If no better position is found, returns the input (line, column).
pub fn resolve_symbol_position(
    file_path: &Path,
    line: u32,
    column: u32,
    language: &str,
) -> Result<(u32, u32)> {
    // Delegate to the existing implementation in LspDatabaseAdapter
    crate::lsp_database_adapter::LspDatabaseAdapter::new()
        .resolve_symbol_position(file_path, line, column, language)
}
