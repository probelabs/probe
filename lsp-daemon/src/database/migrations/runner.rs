//! Migration runner for executing database schema changes safely

use super::migration::MigrationResult;
use super::{Migration, MigrationError};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, error, info};
use turso::Connection;

/// Migration runner that executes schema changes safely
///
/// The runner provides:
/// - Automatic migration discovery and ordering
/// - Transaction safety (each migration runs in its own transaction)
/// - Rollback capability
/// - Progress tracking and logging
/// - Checksum validation to prevent corruption
#[derive(Debug)]
pub struct MigrationRunner {
    /// Available migrations, sorted by version
    migrations: Vec<Box<dyn Migration>>,
    /// Whether to run migrations automatically or require manual confirmation
    auto_run: bool,
    /// Whether to stop on first failure or continue
    fail_fast: bool,
}

impl MigrationRunner {
    /// Create a new migration runner with the given migrations
    pub fn new(migrations: Vec<Box<dyn Migration>>) -> MigrationResult<Self> {
        let mut sorted_migrations = migrations;

        // Sort migrations by version
        sorted_migrations.sort_by_key(|m| m.version());

        // Validate migration versions are sequential and unique
        let mut expected_version = 1;
        for migration in &sorted_migrations {
            let version = migration.version();
            if version == 0 {
                return Err(MigrationError::version_conflict(
                    "Migration version 0 is reserved for initial state",
                ));
            }
            if version != expected_version {
                return Err(MigrationError::version_conflict(format!(
                    "Expected migration version {}, found {}. Migrations must be sequential.",
                    expected_version, version
                )));
            }
            expected_version += 1;
        }

        Ok(Self {
            migrations: sorted_migrations,
            auto_run: true,
            fail_fast: true,
        })
    }

    /// Set whether to run migrations automatically
    pub fn auto_run(mut self, auto_run: bool) -> Self {
        self.auto_run = auto_run;
        self
    }

    /// Set whether to stop on first failure
    pub fn fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    /// Run all pending migrations up to the target version
    ///
    /// If target_version is None, runs all available migrations.
    /// Returns the number of migrations applied.
    pub async fn migrate_to(
        &self,
        conn: &Connection,
        target_version: Option<u32>,
    ) -> MigrationResult<u32> {
        info!("Starting migration process");

        // Initialize migrations table
        self.ensure_migrations_table(conn).await?;

        // Get current version
        let current_version = self.get_current_version(conn).await?;
        let target = target_version
            .unwrap_or_else(|| self.migrations.last().map(|m| m.version()).unwrap_or(0));

        info!(
            "Current schema version: {}, target version: {}",
            current_version, target
        );

        if current_version >= target {
            info!("Schema is already at or above target version, no migrations needed");
            return Ok(0);
        }

        // Get already applied migrations for checksum validation
        let applied = self.get_applied_migrations(conn).await?;

        // Find migrations to apply
        let pending_migrations: Vec<_> = self
            .migrations
            .iter()
            .filter(|m| m.version() > current_version && m.version() <= target)
            .collect();

        if pending_migrations.is_empty() {
            info!("No pending migrations to apply");
            return Ok(0);
        }

        info!(
            "Found {} pending migrations to apply",
            pending_migrations.len()
        );

        // Validate checksums for already applied migrations
        for migration in &self.migrations {
            let version = migration.version();
            if let Some(applied_migration) = applied.get(&version) {
                let expected_checksum = migration.checksum();
                if applied_migration.checksum != expected_checksum {
                    return Err(MigrationError::checksum_mismatch(
                        version,
                        expected_checksum,
                        applied_migration.checksum.clone(),
                    ));
                }
            }
        }

        // Apply pending migrations
        let mut applied_count = 0;
        for migration in pending_migrations {
            match self.apply_migration(conn, migration.as_ref()).await {
                Ok(()) => {
                    applied_count += 1;
                    info!(
                        "Successfully applied migration {} ({})",
                        migration.version(),
                        migration.name()
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to apply migration {} ({}): {}",
                        migration.version(),
                        migration.name(),
                        e
                    );
                    if self.fail_fast {
                        return Err(e);
                    }
                }
            }
        }

        info!(
            "Migration process completed, applied {} migrations",
            applied_count
        );
        Ok(applied_count)
    }

    /// Rollback migrations down to the target version
    ///
    /// Returns the number of migrations rolled back.
    pub async fn rollback_to(
        &self,
        conn: &Connection,
        target_version: u32,
    ) -> MigrationResult<u32> {
        info!("Starting rollback process to version {}", target_version);

        let current_version = self.get_current_version(conn).await?;

        if current_version <= target_version {
            info!("Schema is already at or below target version, no rollbacks needed");
            return Ok(0);
        }

        // Find migrations to rollback (in reverse order)
        let rollback_migrations: Vec<_> = self
            .migrations
            .iter()
            .filter(|m| m.version() > target_version && m.version() <= current_version)
            .rev()
            .collect();

        if rollback_migrations.is_empty() {
            info!("No migrations to rollback");
            return Ok(0);
        }

        info!("Found {} migrations to rollback", rollback_migrations.len());

        let mut rollback_count = 0;
        for migration in rollback_migrations {
            match self.rollback_migration(conn, migration.as_ref()).await {
                Ok(()) => {
                    rollback_count += 1;
                    info!(
                        "Successfully rolled back migration {} ({})",
                        migration.version(),
                        migration.name()
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to rollback migration {} ({}): {}",
                        migration.version(),
                        migration.name(),
                        e
                    );
                    if self.fail_fast {
                        return Err(e);
                    }
                }
            }
        }

        info!(
            "Rollback process completed, rolled back {} migrations",
            rollback_count
        );
        Ok(rollback_count)
    }

    /// Get the current schema version
    pub async fn get_current_version(&self, conn: &Connection) -> MigrationResult<u32> {
        super::get_current_version(conn).await
    }

    /// Check if migrations are needed
    pub async fn needs_migration(&self, conn: &Connection) -> MigrationResult<bool> {
        let current_version = self.get_current_version(conn).await?;
        let latest_version = self.migrations.last().map(|m| m.version()).unwrap_or(0);
        Ok(current_version < latest_version)
    }

    /// Get list of pending migrations
    pub async fn pending_migrations(
        &self,
        conn: &Connection,
    ) -> MigrationResult<Vec<&dyn Migration>> {
        let current_version = self.get_current_version(conn).await?;
        Ok(self
            .migrations
            .iter()
            .filter(|m| m.version() > current_version)
            .map(|m| m.as_ref())
            .collect())
    }

    /// Apply a single migration
    async fn apply_migration(
        &self,
        conn: &Connection,
        migration: &dyn Migration,
    ) -> MigrationResult<()> {
        let version = migration.version();
        let name = migration.name();

        debug!("Applying migration {} ({})", version, name);

        // Pre-migration validation
        migration.validate_pre_migration(conn)?;

        // Execute migration in a transaction
        let start_time = Instant::now();

        // Start transaction
        conn.execute("BEGIN TRANSACTION", ()).await.map_err(|e| {
            MigrationError::transaction_failed(format!("Failed to start transaction: {e}"))
        })?;

        // Execute up SQL
        let result = self.execute_sql(conn, migration.up_sql()).await;

        match result {
            Ok(()) => {
                // Record migration in migrations table
                let execution_time_ms = start_time.elapsed().as_millis() as u32;
                let checksum = migration.checksum();
                let rollback_sql = migration.down_sql().unwrap_or("");

                let insert_result = conn.execute(
                    "INSERT INTO schema_migrations (version, name, checksum, execution_time_ms, rollback_sql) VALUES (?, ?, ?, ?, ?)",
                    [
                        turso::Value::Integer(version as i64),
                        turso::Value::Text(name.to_string()),
                        turso::Value::Text(checksum),
                        turso::Value::Integer(execution_time_ms as i64),
                        turso::Value::Text(rollback_sql.to_string()),
                    ]
                ).await;

                match insert_result {
                    Ok(_) => {
                        // Commit transaction
                        conn.execute("COMMIT", ()).await.map_err(|e| {
                            MigrationError::transaction_failed(format!(
                                "Failed to commit transaction: {e}"
                            ))
                        })?;

                        // Post-migration validation
                        migration.validate_post_migration(conn)?;

                        info!(
                            "Migration {} applied successfully in {}ms",
                            version, execution_time_ms
                        );
                        Ok(())
                    }
                    Err(e) => {
                        // Rollback transaction
                        let _ = conn.execute("ROLLBACK", ()).await;
                        Err(MigrationError::execution_failed(
                            version,
                            format!("Failed to record migration: {e}"),
                        ))
                    }
                }
            }
            Err(e) => {
                // Rollback transaction
                let _ = conn.execute("ROLLBACK", ()).await;
                Err(e)
            }
        }
    }

    /// Rollback a single migration
    async fn rollback_migration(
        &self,
        conn: &Connection,
        migration: &dyn Migration,
    ) -> MigrationResult<()> {
        let version = migration.version();
        let name = migration.name();

        debug!("Rolling back migration {} ({})", version, name);

        // Check if migration supports rollback
        let down_sql = migration
            .down_sql()
            .ok_or_else(|| MigrationError::rollback_not_supported(version))?;

        // Start transaction
        conn.execute("BEGIN TRANSACTION", ()).await.map_err(|e| {
            MigrationError::transaction_failed(format!("Failed to start transaction: {e}"))
        })?;

        // Execute down SQL
        let result = self.execute_sql(conn, down_sql).await;

        match result {
            Ok(()) => {
                // Remove migration record
                let delete_result = conn
                    .execute(
                        "DELETE FROM schema_migrations WHERE version = ?",
                        [turso::Value::Integer(version as i64)],
                    )
                    .await;

                match delete_result {
                    Ok(_) => {
                        // Commit transaction
                        conn.execute("COMMIT", ()).await.map_err(|e| {
                            MigrationError::transaction_failed(format!(
                                "Failed to commit rollback: {e}"
                            ))
                        })?;

                        info!("Migration {} rolled back successfully", version);
                        Ok(())
                    }
                    Err(e) => {
                        // Rollback transaction
                        let _ = conn.execute("ROLLBACK", ()).await;
                        Err(MigrationError::execution_failed(
                            version,
                            format!("Failed to remove migration record: {e}"),
                        ))
                    }
                }
            }
            Err(e) => {
                // Rollback transaction
                let _ = conn.execute("ROLLBACK", ()).await;
                Err(e)
            }
        }
    }

    /// Execute SQL statements safely
    async fn execute_sql(&self, conn: &Connection, sql: &str) -> MigrationResult<()> {
        // Split SQL into individual statements
        let statements = self.split_sql_statements(sql);

        for statement in statements {
            let trimmed = statement.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }

            debug!("Executing SQL: {}", trimmed);

            conn.execute(trimmed, ()).await.map_err(|e| {
                MigrationError::query_failed(format!("Failed to execute SQL '{}': {}", trimmed, e))
            })?;
        }

        Ok(())
    }

    /// Split SQL into individual statements
    pub fn split_sql_statements(&self, sql: &str) -> Vec<String> {
        // More sophisticated SQL statement splitting that handles multi-line statements
        let mut statements = Vec::new();
        let mut current_statement = String::new();
        let mut in_string = false;
        let mut string_char = '\'';
        let mut paren_depth = 0;

        let lines: Vec<&str> = sql.lines().collect();

        for line in lines {
            let trimmed = line.trim();

            // Skip comment lines
            if trimmed.starts_with("--") || trimmed.is_empty() {
                continue;
            }

            // Track parentheses depth and string literals
            let mut chars = trimmed.chars().peekable();
            let mut line_content = String::new();

            while let Some(ch) = chars.next() {
                match ch {
                    '\'' | '"' if !in_string => {
                        in_string = true;
                        string_char = ch;
                        line_content.push(ch);
                    }
                    c if c == string_char && in_string => {
                        in_string = false;
                        line_content.push(ch);
                    }
                    '(' if !in_string => {
                        paren_depth += 1;
                        line_content.push(ch);
                    }
                    ')' if !in_string => {
                        paren_depth -= 1;
                        line_content.push(ch);
                    }
                    ';' if !in_string && paren_depth == 0 => {
                        // End of statement
                        if !current_statement.is_empty() || !line_content.trim().is_empty() {
                            current_statement.push(' ');
                            current_statement.push_str(line_content.trim());
                            if !current_statement.trim().is_empty() {
                                statements.push(current_statement.trim().to_string());
                            }
                            current_statement.clear();
                        }
                        line_content.clear();
                    }
                    _ => {
                        line_content.push(ch);
                    }
                }
            }

            // Add remaining content to current statement
            if !line_content.trim().is_empty() {
                if !current_statement.is_empty() {
                    current_statement.push(' ');
                }
                current_statement.push_str(line_content.trim());
            }
        }

        // Add final statement if exists and is not just a comment
        let final_stmt = current_statement.trim();
        if !final_stmt.is_empty() && !final_stmt.starts_with("--") {
            statements.push(final_stmt.to_string());
        }

        statements
    }

    /// Ensure the migrations table exists
    async fn ensure_migrations_table(&self, conn: &Connection) -> MigrationResult<()> {
        super::initialize_migrations_table(conn).await
    }

    /// Get applied migrations
    async fn get_applied_migrations(
        &self,
        conn: &Connection,
    ) -> MigrationResult<HashMap<u32, super::AppliedMigration>> {
        super::get_applied_migrations(conn).await
    }
}

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

    fn create_test_migrations() -> Vec<Box<dyn Migration>> {
        vec![
            Box::new(TestMigration {
                version: 1,
                name: "create_users".to_string(),
                up_sql: "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)".to_string(),
                down_sql: Some("DROP TABLE users".to_string()),
            }) as Box<dyn Migration>,
            Box::new(TestMigration {
                version: 2,
                name: "add_email_column".to_string(),
                up_sql: "ALTER TABLE users ADD COLUMN email TEXT".to_string(),
                down_sql: None, // Cannot rollback ALTER TABLE ADD COLUMN in SQLite
            }) as Box<dyn Migration>,
        ]
    }

    #[test]
    fn test_migration_runner_creation() {
        let migrations = create_test_migrations();
        let runner = MigrationRunner::new(migrations).unwrap();

        assert_eq!(runner.migrations.len(), 2);
        assert!(runner.auto_run);
        assert!(runner.fail_fast);
    }

    #[test]
    fn test_migration_version_validation() {
        // Test sequential versions work
        let good_migrations = create_test_migrations();
        let runner = MigrationRunner::new(good_migrations);
        assert!(runner.is_ok());

        // Test non-sequential versions fail
        let bad_migrations: Vec<Box<dyn Migration>> = vec![
            Box::new(TestMigration {
                version: 1,
                name: "first".to_string(),
                up_sql: "CREATE TABLE test (id INTEGER)".to_string(),
                down_sql: None,
            }),
            Box::new(TestMigration {
                version: 3, // Skip version 2
                name: "third".to_string(),
                up_sql: "CREATE TABLE other (id INTEGER)".to_string(),
                down_sql: None,
            }),
        ];

        let runner = MigrationRunner::new(bad_migrations);
        assert!(runner.is_err());

        // Test version 0 fails
        let zero_migrations: Vec<Box<dyn Migration>> = vec![Box::new(TestMigration {
            version: 0, // Reserved version
            name: "zero".to_string(),
            up_sql: "CREATE TABLE zero (id INTEGER)".to_string(),
            down_sql: None,
        })];

        let runner = MigrationRunner::new(zero_migrations);
        assert!(runner.is_err());
    }

    #[test]
    fn test_sql_statement_splitting() {
        let runner = MigrationRunner::new(vec![]).unwrap();

        let sql = "CREATE TABLE users (id INTEGER); INSERT INTO users VALUES (1); -- Comment";
        let statements = runner.split_sql_statements(sql);

        assert_eq!(statements.len(), 2);
        assert!(statements[0].contains("CREATE TABLE users"));
        assert!(statements[1].contains("INSERT INTO users"));
    }
}
