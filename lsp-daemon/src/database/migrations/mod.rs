//! Database migration system for managing schema evolution
//!
//! This module provides a robust migration framework that supports:
//! - Incremental schema changes
//! - Rollback capability  
//! - Checksum validation
//! - Transaction safety
//! - Auto-discovery of migration files

use anyhow::Result;
use std::collections::HashMap;
use tracing::{info, warn};
use turso::Connection;

pub mod migration;
pub mod runner;
pub mod v001_initial_schema;
pub mod v002_example;
pub mod v003_remove_analysis_run_id;

pub use migration::{Migration, MigrationError};
pub use runner::MigrationRunner;

/// Current schema version supported by this codebase
pub const CURRENT_SCHEMA_VERSION: u32 = 3;

/// Registry of all available migrations
/// This provides compile-time guarantees that all migrations are included
pub fn all_migrations() -> Vec<Box<dyn Migration>> {
    vec![
        Box::new(v001_initial_schema::V001InitialSchema),
        Box::new(v002_example::V002Example),
        Box::new(v003_remove_analysis_run_id::V003RemoveAnalysisRunId),
    ]
}

/// Initialize the schema_migrations table for tracking applied migrations
pub async fn initialize_migrations_table(conn: &Connection) -> Result<(), MigrationError> {
    let sql = r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            checksum TEXT NOT NULL,
            applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            execution_time_ms INTEGER,
            rollback_sql TEXT
        )
    "#;

    conn.execute(sql, ())
        .await
        .map_err(|e| MigrationError::ExecutionFailed {
            version: 0,
            message: format!("Failed to create schema_migrations table: {e}"),
        })?;

    info!("Initialized schema_migrations table");
    Ok(())
}

/// Get the current schema version from the database
pub async fn get_current_version(conn: &Connection) -> Result<u32, MigrationError> {
    // First try the new schema_migrations table
    let migration_version = conn
        .query("SELECT MAX(version) FROM schema_migrations", ())
        .await;

    if let Ok(mut rows) = migration_version {
        if let Ok(Some(row)) = rows.next().await {
            if let Ok(turso::Value::Integer(version)) = row.get_value(0) {
                return Ok(version as u32);
            }
        }
    }

    // Fall back to legacy schema_version table for backward compatibility
    let legacy_version = conn
        .query("SELECT MAX(version) FROM schema_version", ())
        .await;

    if let Ok(mut rows) = legacy_version {
        if let Ok(Some(row)) = rows.next().await {
            if let Ok(turso::Value::Integer(version)) = row.get_value(0) {
                warn!("Using legacy schema_version table, consider running migrations to update");
                return Ok(version as u32);
            }
        }
    }

    // No version found, assume version 0 (pre-migration state)
    Ok(0)
}

/// Check if a specific migration has been applied
pub async fn is_migration_applied(conn: &Connection, version: u32) -> Result<bool, MigrationError> {
    let mut rows = conn
        .query(
            "SELECT 1 FROM schema_migrations WHERE version = ? LIMIT 1",
            [turso::Value::Integer(version as i64)],
        )
        .await
        .map_err(|e| MigrationError::QueryFailed {
            message: format!("Failed to check if migration {} is applied: {e}", version),
        })?;

    Ok(rows
        .next()
        .await
        .map_err(|e| MigrationError::QueryFailed {
            message: format!("Failed to read migration check result: {e}"),
        })?
        .is_some())
}

/// Get all applied migrations with their metadata
pub async fn get_applied_migrations(
    conn: &Connection,
) -> Result<HashMap<u32, AppliedMigration>, MigrationError> {
    let mut rows = conn
        .query(
            "SELECT version, name, checksum, applied_at, execution_time_ms FROM schema_migrations ORDER BY version",
            ()
        )
        .await
        .map_err(|e| MigrationError::QueryFailed {
            message: format!("Failed to query applied migrations: {e}"),
        })?;

    let mut applied = HashMap::new();

    while let Some(row) = rows.next().await.map_err(|e| MigrationError::QueryFailed {
        message: format!("Failed to iterate applied migrations: {e}"),
    })? {
        let version = match row.get_value(0) {
            Ok(turso::Value::Integer(v)) => v as u32,
            _ => continue,
        };

        let name = match row.get_value(1) {
            Ok(turso::Value::Text(n)) => n,
            _ => continue,
        };

        let checksum = match row.get_value(2) {
            Ok(turso::Value::Text(c)) => c,
            _ => continue,
        };

        let applied_at = match row.get_value(3) {
            Ok(turso::Value::Text(t)) => t,
            _ => "unknown".to_string(),
        };

        let execution_time_ms = match row.get_value(4) {
            Ok(turso::Value::Integer(t)) => Some(t as u32),
            _ => None,
        };

        applied.insert(
            version,
            AppliedMigration {
                version,
                name,
                checksum,
                applied_at,
                execution_time_ms,
            },
        );
    }

    Ok(applied)
}

/// Metadata about an applied migration
#[derive(Debug, Clone)]
pub struct AppliedMigration {
    pub version: u32,
    pub name: String,
    pub checksum: String,
    pub applied_at: String,
    pub execution_time_ms: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    use turso::{Builder, Connection};

    async fn create_test_connection() -> Result<Connection, Box<dyn std::error::Error>> {
        // Create a simple in-memory connection directly using turso
        let database = Builder::new_local(":memory:").build().await?;
        let conn = database.connect()?;

        // Initialize the schema_version table for compatibility
        conn.execute(
            r#"CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                description TEXT
            )"#,
            (),
        )
        .await?;

        Ok(conn)
    }

    #[tokio::test]
    async fn test_initialize_migrations_table() {
        let conn = create_test_connection().await.unwrap();

        // Should succeed
        initialize_migrations_table(&conn).await.unwrap();

        // Should be idempotent
        initialize_migrations_table(&conn).await.unwrap();

        // Verify table exists
        let mut rows = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
                (),
            )
            .await
            .unwrap();

        assert!(rows.next().await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_get_current_version() {
        let conn = create_test_connection().await.unwrap();
        initialize_migrations_table(&conn).await.unwrap();

        // Initially should be 0
        let version = get_current_version(&conn).await.unwrap();
        assert_eq!(version, 0);

        // Add a migration
        conn.execute(
            "INSERT INTO schema_migrations (version, name, checksum) VALUES (1, 'test', 'abc123')",
            (),
        )
        .await
        .unwrap();

        let version = get_current_version(&conn).await.unwrap();
        assert_eq!(version, 1);
    }

    #[tokio::test]
    async fn test_is_migration_applied() {
        let conn = create_test_connection().await.unwrap();
        initialize_migrations_table(&conn).await.unwrap();

        // Should be false initially
        let applied = is_migration_applied(&conn, 1).await.unwrap();
        assert!(!applied);

        // Add migration
        conn.execute(
            "INSERT INTO schema_migrations (version, name, checksum) VALUES (1, 'test', 'abc123')",
            (),
        )
        .await
        .unwrap();

        // Should be true now
        let applied = is_migration_applied(&conn, 1).await.unwrap();
        assert!(applied);
    }

    #[tokio::test]
    async fn test_all_migrations_compile() {
        let migrations = all_migrations();
        assert!(migrations.len() >= 2); // At least initial schema + example

        // Versions should be unique
        let versions: Vec<u32> = migrations.iter().map(|m| m.version()).collect();
        let mut unique_versions = versions.clone();
        unique_versions.sort();
        unique_versions.dedup();

        assert_eq!(
            versions.len(),
            unique_versions.len(),
            "Migration versions must be unique"
        );
    }
}
