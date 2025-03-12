// Error types for chat module

#[derive(Debug, thiserror::Error)]
#[error("Search error: {0}")]
pub struct SearchError(pub String);

#[derive(Debug, thiserror::Error)]
#[error("Query error: {0}")]
pub struct QueryError(pub String);

#[derive(Debug, thiserror::Error)]
#[error("Extract error: {0}")]
pub struct ExtractError(pub String);
