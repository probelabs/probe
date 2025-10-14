//! Migration trait and error types for database schema evolution

use sha2::{Digest, Sha256};
use std::fmt;
use thiserror::Error;

/// Trait that all database migrations must implement
///
/// Migrations are versioned schema changes that can be applied incrementally
/// and optionally rolled back. Each migration must provide:
/// - A unique version number
/// - A descriptive name
/// - Forward migration SQL (up)
/// - Optional backward migration SQL (down)  
/// - Checksum for integrity validation
pub trait Migration: fmt::Debug + Send + Sync {
    /// Get the migration version number
    ///
    /// Versions should be sequential integers starting from 1.
    /// Version 0 is reserved for the initial empty state.
    fn version(&self) -> u32;

    /// Get a human-readable name for the migration
    ///
    /// Should be descriptive and match the filename convention,
    /// e.g., "initial_schema", "add_user_table", etc.
    fn name(&self) -> &str;

    /// Get the SQL statements to apply this migration (forward direction)
    ///
    /// Should contain all DDL and DML statements needed to upgrade
    /// from the previous version to this version.
    fn up_sql(&self) -> &str;

    /// Get the SQL statements to rollback this migration (backward direction)
    ///
    /// Optional - if None, this migration cannot be rolled back.
    /// Should contain all DDL and DML statements to downgrade
    /// from this version to the previous version.
    fn down_sql(&self) -> Option<&str>;

    /// Get a checksum for this migration to detect changes
    ///
    /// The default implementation creates a SHA-256 hash of the version,
    /// name, and up_sql. Override if you need custom checksum logic.
    fn checksum(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.version().to_string().as_bytes());
        hasher.update(self.name().as_bytes());
        hasher.update(self.up_sql().as_bytes());
        if let Some(down_sql) = self.down_sql() {
            hasher.update(down_sql.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    /// Optional pre-migration validation
    ///
    /// Called before applying the migration. Can check preconditions,
    /// validate data integrity, etc. Return an error to abort migration.
    fn validate_pre_migration(&self, _conn: &turso::Connection) -> Result<(), MigrationError> {
        Ok(())
    }

    /// Optional post-migration validation
    ///
    /// Called after applying the migration. Can verify the migration
    /// was applied correctly, check constraints, etc.
    fn validate_post_migration(&self, _conn: &turso::Connection) -> Result<(), MigrationError> {
        Ok(())
    }
}

/// Errors that can occur during migration operations
#[derive(Error, Debug)]
pub enum MigrationError {
    /// Migration execution failed
    #[error("Migration {version} failed to execute: {message}")]
    ExecutionFailed { version: u32, message: String },

    /// Migration validation failed  
    #[error("Migration {version} validation failed: {message}")]
    ValidationFailed { version: u32, message: String },

    /// Migration checksum mismatch (indicates tampering or version drift)
    #[error("Migration {version} checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        version: u32,
        expected: String,
        actual: String,
    },

    /// Migration version conflict (duplicate version or ordering issue)
    #[error("Migration version conflict: {message}")]
    VersionConflict { message: String },

    /// Rollback not supported for this migration
    #[error("Migration {version} does not support rollback")]
    RollbackNotSupported { version: u32 },

    /// Database query failed during migration
    #[error("Database query failed: {message}")]
    QueryFailed { message: String },

    /// Transaction failed during migration
    #[error("Transaction failed during migration: {message}")]
    TransactionFailed { message: String },

    /// Migration dependency not satisfied
    #[error("Migration {version} dependency not satisfied: {message}")]
    DependencyNotSatisfied { version: u32, message: String },

    /// Generic migration error
    #[error("Migration error: {message}")]
    Generic { message: String },
}

impl MigrationError {
    /// Create an execution failed error
    pub fn execution_failed(version: u32, message: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            version,
            message: message.into(),
        }
    }

    /// Create a validation failed error
    pub fn validation_failed(version: u32, message: impl Into<String>) -> Self {
        Self::ValidationFailed {
            version,
            message: message.into(),
        }
    }

    /// Create a checksum mismatch error
    pub fn checksum_mismatch(
        version: u32,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self::ChecksumMismatch {
            version,
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Create a version conflict error
    pub fn version_conflict(message: impl Into<String>) -> Self {
        Self::VersionConflict {
            message: message.into(),
        }
    }

    /// Create a rollback not supported error
    pub fn rollback_not_supported(version: u32) -> Self {
        Self::RollbackNotSupported { version }
    }

    /// Create a query failed error
    pub fn query_failed(message: impl Into<String>) -> Self {
        Self::QueryFailed {
            message: message.into(),
        }
    }

    /// Create a transaction failed error
    pub fn transaction_failed(message: impl Into<String>) -> Self {
        Self::TransactionFailed {
            message: message.into(),
        }
    }

    /// Create a dependency not satisfied error
    pub fn dependency_not_satisfied(version: u32, message: impl Into<String>) -> Self {
        Self::DependencyNotSatisfied {
            version,
            message: message.into(),
        }
    }

    /// Create a generic error
    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic {
            message: message.into(),
        }
    }
}

/// Result type for migration operations
pub type MigrationResult<T> = Result<T, MigrationError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestMigration {
        version: u32,
        name: String,
        up_sql: String,
        down_sql: Option<String>,
    }

    impl Migration for TestMigration {
        fn version(&self) -> u32 {
            self.version
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn up_sql(&self) -> &str {
            &self.up_sql
        }

        fn down_sql(&self) -> Option<&str> {
            self.down_sql.as_deref()
        }
    }

    #[test]
    fn test_migration_checksum_consistency() {
        let migration = TestMigration {
            version: 1,
            name: "test_migration".to_string(),
            up_sql: "CREATE TABLE test (id INTEGER)".to_string(),
            down_sql: Some("DROP TABLE test".to_string()),
        };

        let checksum1 = migration.checksum();
        let checksum2 = migration.checksum();

        // Checksums should be consistent
        assert_eq!(checksum1, checksum2);
        assert!(!checksum1.is_empty());
        assert_eq!(checksum1.len(), 64); // SHA-256 is 64 hex chars
    }

    #[test]
    fn test_migration_checksum_sensitivity() {
        let migration1 = TestMigration {
            version: 1,
            name: "test_migration".to_string(),
            up_sql: "CREATE TABLE test (id INTEGER)".to_string(),
            down_sql: None,
        };

        let migration2 = TestMigration {
            version: 1,
            name: "test_migration".to_string(),
            up_sql: "CREATE TABLE test (id INTEGER PRIMARY KEY)".to_string(), // Different SQL
            down_sql: None,
        };

        // Checksums should be different
        assert_ne!(migration1.checksum(), migration2.checksum());
    }

    #[test]
    fn test_migration_error_construction() {
        let err = MigrationError::execution_failed(1, "test error");
        assert!(matches!(
            err,
            MigrationError::ExecutionFailed { version: 1, .. }
        ));
        assert!(err.to_string().contains("test error"));

        let err = MigrationError::checksum_mismatch(2, "abc123", "def456");
        assert!(matches!(
            err,
            MigrationError::ChecksumMismatch { version: 2, .. }
        ));

        let err = MigrationError::rollback_not_supported(3);
        assert!(matches!(
            err,
            MigrationError::RollbackNotSupported { version: 3 }
        ));
    }
}
