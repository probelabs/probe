//! V002: Example migration for future schema changes
//!
//! This migration serves as a template and example for future schema changes.
//! It demonstrates best practices for:
//! - Adding new tables with proper foreign keys
//! - Creating indexes for performance
//! - Writing rollback SQL
//! - Validation logic
//!
//! Note: This migration is currently a no-op example and should be replaced
//! with actual schema changes when needed.

use crate::database::migrations::{Migration, MigrationError};

/// Example migration (Version 2)
///
/// This is a template migration that demonstrates how to structure
/// future schema changes. Replace this with actual migrations as needed.
#[derive(Debug)]
pub struct V002Example;

impl Migration for V002Example {
    fn version(&self) -> u32 {
        2
    }

    fn name(&self) -> &str {
        "example_migration"
    }

    fn up_sql(&self) -> &str {
        r#"
-- ============================================================================
-- V002: Example Migration Template
-- This is a no-op example migration that demonstrates best practices
-- Replace this with actual schema changes when implementing V002
-- ============================================================================

-- Example: Adding a new table
-- CREATE TABLE IF NOT EXISTS example_table (
--     example_id TEXT PRIMARY KEY,
--     project_id TEXT NOT NULL,
--     example_data TEXT,
--     created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
--     FOREIGN KEY (project_id) REFERENCES project(project_id) ON DELETE CASCADE
-- );

-- Example: Adding an index for performance
-- CREATE INDEX IF NOT EXISTS idx_example_project ON example_table(project_id);

-- Example: Adding a column to an existing table
-- ALTER TABLE workspace ADD COLUMN example_column TEXT DEFAULT NULL;

-- No-op placeholder migration - creates a harmless view and immediately drops it
-- This ensures the migration runs successfully without making permanent changes
-- In a real migration, you would include actual schema changes here

-- Create a temporary view (this is a safe no-op operation)
CREATE VIEW IF NOT EXISTS migration_v002_test_view AS SELECT 'Migration V002 executed successfully' as message;

-- Drop the temporary view to leave no traces
DROP VIEW IF EXISTS migration_v002_test_view;
        "#
    }

    fn down_sql(&self) -> Option<&str> {
        Some(
            r#"
-- ============================================================================
-- V002: Rollback Example Migration
-- Demonstrates how to write proper rollback SQL
-- ============================================================================

-- Example rollbacks (corresponding to the up_sql examples above):

-- Example: Drop the table we created
-- DROP TABLE IF EXISTS example_table;

-- Example: Remove the column we added (SQLite doesn't support DROP COLUMN directly)
-- For SQLite, you would need to:
-- 1. Create new table without the column
-- 2. Copy data from old table to new table  
-- 3. Drop old table
-- 4. Rename new table

-- No-op for placeholder migration - nothing to rollback
SELECT 'V002 migration rollback completed - no changes were made' as result;
        "#,
        )
    }

    fn validate_pre_migration(&self, _conn: &turso::Connection) -> Result<(), MigrationError> {
        // Example pre-migration validation
        // For the placeholder migration, no validation is needed

        // In a real migration, you might check:
        // - Required tables exist
        // - Data constraints are satisfied
        // - Prerequisite migrations have been applied

        Ok(())
    }

    fn validate_post_migration(&self, _conn: &turso::Connection) -> Result<(), MigrationError> {
        // Example post-migration validation
        // For the placeholder migration, no validation is needed

        // In a real migration, you would check that:
        // - New tables exist
        // - Indexes were created
        // - Data integrity is maintained
        // - Constraints are working

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_properties() {
        let migration = V002Example;

        assert_eq!(migration.version(), 2);
        assert_eq!(migration.name(), "example_migration");
        assert!(migration.up_sql().contains("V002"));
        assert!(migration.down_sql().is_some());

        // Checksum should be consistent
        let checksum1 = migration.checksum();
        let checksum2 = migration.checksum();
        assert_eq!(checksum1, checksum2);
        assert_eq!(checksum1.len(), 64); // SHA-256
    }

    #[test]
    fn test_migration_sql_content() {
        let migration = V002Example;

        let up_sql = migration.up_sql();
        let down_sql = migration.down_sql().unwrap();

        // Up SQL should contain view operations (updated for new no-op design)
        assert!(up_sql.contains("migration_v002_test_view"));
        assert!(up_sql.contains("CREATE VIEW"));

        // Down SQL should mention rollback
        assert!(down_sql.contains("rollback"));
        assert!(down_sql.contains("V002"));
    }
}
