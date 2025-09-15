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
use tracing::info;
use turso::{Connection, Value};

pub mod migration;
pub mod runner;
pub mod v001_complete_schema;

pub use migration::{Migration, MigrationError};
pub use runner::MigrationRunner;

/// Current schema version supported by this codebase
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Registry of all available migrations
/// This provides compile-time guarantees that all migrations are included
pub fn all_migrations() -> Vec<Box<dyn Migration>> {
    vec![Box::new(v001_complete_schema::V001CompleteSchema)]
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

    conn.execute(sql, Vec::<Value>::new())
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
    match conn
        .prepare("SELECT MAX(version) FROM schema_migrations")
        .await
    {
        Ok(mut stmt) => {
            match stmt.query(Vec::<Value>::new()).await {
                Ok(mut rows) => {
                    if let Some(row) =
                        rows.next().await.map_err(|e| MigrationError::QueryFailed {
                            message: format!("Failed to fetch schema version: {e}"),
                        })?
                    {
                        let version =
                            row.get_value(0).map_err(|e| MigrationError::QueryFailed {
                                message: format!("Failed to read version value: {e}"),
                            })?;
                        match version {
                            Value::Integer(v) => Ok(v as u32),
                            Value::Null => Ok(0),
                            _ => Ok(0),
                        }
                    } else {
                        Ok(0)
                    }
                }
                Err(_) => Ok(0), // Table might not exist yet
            }
        }
        Err(_) => Ok(0), // Table doesn't exist, assume version 0
    }
}

/// Check if a specific migration has been applied
pub async fn is_migration_applied(conn: &Connection, version: u32) -> Result<bool, MigrationError> {
    match conn
        .prepare("SELECT 1 FROM schema_migrations WHERE version = ? LIMIT 1")
        .await
    {
        Ok(mut stmt) => {
            let params = vec![Value::Integer(version as i64)];
            match stmt.query(params).await {
                Ok(mut rows) => {
                    match rows.next().await {
                        Ok(Some(_)) => Ok(true),
                        Ok(None) => Ok(false),
                        Err(_) => Ok(false), // Error means migration not found
                    }
                }
                Err(_) => Ok(false), // Query failed, assume not applied
            }
        }
        Err(_) => Ok(false), // Table doesn't exist, assume not applied
    }
}

/// Get all applied migrations with their metadata
pub async fn get_applied_migrations(
    conn: &Connection,
) -> Result<HashMap<u32, AppliedMigration>, MigrationError> {
    let mut stmt = conn.prepare(
        "SELECT version, name, checksum, applied_at, execution_time_ms FROM schema_migrations ORDER BY version"
    ).await.map_err(|e| MigrationError::QueryFailed {
        message: format!("Failed to prepare applied migrations query: {e}"),
    })?;

    let mut applied = HashMap::new();
    let mut rows =
        stmt.query(Vec::<Value>::new())
            .await
            .map_err(|e| MigrationError::QueryFailed {
                message: format!("Failed to query applied migrations: {e}"),
            })?;

    while let Some(row) = rows.next().await.map_err(|e| MigrationError::QueryFailed {
        message: format!("Failed to fetch migration row: {e}"),
    })? {
        let version = match row.get_value(0).map_err(|e| MigrationError::QueryFailed {
            message: format!("Failed to read version: {e}"),
        })? {
            Value::Integer(v) => v as u32,
            _ => continue, // Skip invalid versions
        };

        let name = match row.get_value(1).map_err(|e| MigrationError::QueryFailed {
            message: format!("Failed to read name: {e}"),
        })? {
            Value::Text(t) => t,
            _ => continue, // Skip invalid names
        };

        let checksum = match row.get_value(2).map_err(|e| MigrationError::QueryFailed {
            message: format!("Failed to read checksum: {e}"),
        })? {
            Value::Text(t) => t,
            _ => continue, // Skip invalid checksums
        };

        let applied_at = match row.get_value(3).map_err(|e| MigrationError::QueryFailed {
            message: format!("Failed to read applied_at: {e}"),
        })? {
            Value::Text(t) => t,
            _ => continue, // Skip invalid timestamps
        };

        let execution_time_ms = match row.get_value(4) {
            Ok(Value::Integer(t)) => Some(t as u32),
            Ok(Value::Null) => None,
            _ => None,
        };

        let migration = AppliedMigration {
            version,
            name,
            checksum,
            applied_at,
            execution_time_ms,
        };

        applied.insert(migration.version, migration);
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
    use turso::Connection;

    async fn create_test_connection() -> Result<Connection, Box<dyn std::error::Error>> {
        // Create a simple in-memory connection using turso
        use turso::Builder;
        let database = Builder::new_local(":memory:").build().await?;
        let conn = database.connect()?;
        Ok(conn)
    }

    #[tokio::test]
    async fn test_initialize_migrations_table() {
        let conn = create_test_connection().await.unwrap();

        // Should succeed
        initialize_migrations_table(&conn).await.unwrap();

        // Should be idempotent
        initialize_migrations_table(&conn).await.unwrap();

        // Verify table exists by trying to query it
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM schema_migrations")
            .await
            .unwrap();
        let mut rows = stmt.query(Vec::<Value>::new()).await.unwrap();

        // Table should exist and be queryable
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
            "INSERT INTO schema_migrations (version, name, checksum) VALUES (?, ?, ?)",
            vec![
                Value::Integer(1),
                Value::Text("test".to_string()),
                Value::Text("abc123".to_string()),
            ],
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
            "INSERT INTO schema_migrations (version, name, checksum) VALUES (?, ?, ?)",
            vec![
                Value::Integer(1),
                Value::Text("test".to_string()),
                Value::Text("abc123".to_string()),
            ],
        )
        .await
        .unwrap();

        // Should be true now
        let applied = is_migration_applied(&conn, 1).await.unwrap();
        assert!(applied);
    }

    #[test]
    fn test_all_migrations_compile() {
        let migrations = all_migrations();
        assert_eq!(migrations.len(), 1); // Single consolidated migration

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

        // Should have version 1 (the complete schema)
        assert_eq!(versions[0], 1);
    }

    #[tokio::test]
    async fn test_complete_schema_migration_end_to_end() {
        // Create a test connection
        let conn = create_test_connection().await.unwrap();

        // Initialize migrations table
        initialize_migrations_table(&conn).await.unwrap();

        // Get the migration
        let migrations = all_migrations();
        let migration = &migrations[0];

        // Verify it's version 1 with the complete schema
        assert_eq!(migration.version(), 1);
        assert_eq!(migration.name(), "complete_schema");

        // Execute the up SQL directly since we're testing the SQL itself
        let up_sql = migration.up_sql();

        // Split SQL into individual statements and execute them one by one
        // We need to handle multi-line statements properly
        let mut statements = Vec::new();
        let mut current_statement = String::new();

        for line in up_sql.lines() {
            let trimmed_line = line.trim();

            // Skip comment-only lines
            if trimmed_line.starts_with("--") || trimmed_line.is_empty() {
                continue;
            }

            // Add line to current statement
            current_statement.push_str(line);
            current_statement.push('\n');

            // Check if statement is complete (ends with semicolon)
            if trimmed_line.ends_with(';') {
                let stmt = current_statement.trim();
                if !stmt.is_empty() && !stmt.starts_with("--") {
                    statements.push(stmt.to_string());
                }
                current_statement.clear();
            }
        }

        // Add any remaining statement
        if !current_statement.trim().is_empty() {
            let stmt = current_statement.trim();
            if !stmt.starts_with("--") {
                statements.push(stmt.to_string());
            }
        }

        // Execute each statement
        for (i, statement) in statements.iter().enumerate() {
            conn.execute(statement, Vec::<Value>::new())
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "Failed to execute statement #{}: '{}'\nError: {}",
                        i + 1,
                        statement,
                        e
                    )
                });
        }

        // Verify key tables were created by checking they can be queried
        let table_checks = [
            "project",
            "workspace",
            "file",
            "symbol",
            "edge",
            "symbol_state",
        ];

        for table in &table_checks {
            let mut stmt = conn
                .prepare(&format!("SELECT COUNT(*) FROM {}", table))
                .await
                .unwrap_or_else(|e| panic!("Table {} was not created: {}", table, e));

            let mut rows = stmt.query(Vec::<Value>::new()).await.unwrap();
            assert!(
                rows.next().await.unwrap().is_some(),
                "Could not query table {}",
                table
            );
        }

        // Verify key views were created
        let view_checks = ["current_symbols", "symbols_with_files", "edges_named"];

        for view in &view_checks {
            let mut stmt = conn
                .prepare(&format!("SELECT COUNT(*) FROM {}", view))
                .await
                .unwrap_or_else(|e| panic!("View {} was not created: {}", view, e));

            let mut rows = stmt.query(Vec::<Value>::new()).await.unwrap();
            assert!(
                rows.next().await.unwrap().is_some(),
                "Could not query view {}",
                view
            );
        }

        // Test that the schema version is correctly set to 1
        let current_version = get_current_version(&conn).await.unwrap();
        // Note: Version will still be 0 because we didn't record the migration in schema_migrations table
        // This is expected since we just tested the SQL execution, not the full migration runner
        assert_eq!(current_version, 0);
    }
}
