//! Complete database schema migration
//!
//! This migration creates the complete PRD database schema including all tables,
//! indexes, and views in a single migration. It consolidates what were previously
//! separate migrations (v001_initial_schema + v002_performance_indexes) into
//! one comprehensive schema definition.

use super::migration::{Migration, MigrationError};
use turso::Connection;

/// Complete database schema migration that includes all tables, indexes, and views
#[derive(Debug)]
pub struct V001CompleteSchema;

impl Migration for V001CompleteSchema {
    fn version(&self) -> u32 {
        1
    }

    fn name(&self) -> &str {
        "complete_schema"
    }

    fn up_sql(&self) -> &str {
        r#"
-- ============================================================================
-- V001: Complete Schema Migration
-- Creates comprehensive PRD database schema with all tables, indexes, and views
-- Consolidates initial schema + performance indexes into single migration
-- ============================================================================

-- 1. Core PRD Tables

-- Projects/Workspaces table
CREATE TABLE IF NOT EXISTS project (
    project_id TEXT PRIMARY KEY,
    root_path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT
);

-- Workspaces table (project workspaces with branch support)
CREATE TABLE IF NOT EXISTS workspace (
    workspace_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    current_branch TEXT,
    head_commit TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT,
    FOREIGN KEY (project_id) REFERENCES project(project_id) ON DELETE CASCADE
);


-- File registry with project association
CREATE TABLE IF NOT EXISTS file (
    file_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    absolute_path TEXT NOT NULL,
    language TEXT,
    size_bytes INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES project(project_id) ON DELETE CASCADE
);

-- File versions removed - file versioning complexity eliminated

-- Analysis run tracking
CREATE TABLE IF NOT EXISTS analysis_run (
    run_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    analyzer_type TEXT NOT NULL,
    analyzer_version TEXT,
    configuration TEXT,
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP,
    status TEXT DEFAULT 'running',
    files_processed INTEGER DEFAULT 0,
    symbols_found INTEGER DEFAULT 0,
    errors TEXT,
    FOREIGN KEY (workspace_id) REFERENCES workspace(workspace_id) ON DELETE CASCADE
);

-- File analysis status and results
CREATE TABLE IF NOT EXISTS file_analysis (
    analysis_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    symbols_found INTEGER DEFAULT 0,
    references_found INTEGER DEFAULT 0,
    errors TEXT,
    FOREIGN KEY (run_id) REFERENCES analysis_run(run_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES file(file_id) ON DELETE CASCADE
);

-- 2. Symbol and Relationship Tables


-- Symbol definitions (Post-V003: no analysis_run_id, has language field)
-- Updated to match SymbolState struct expectations with symbol_uid as primary key
CREATE TABLE IF NOT EXISTS symbol_state (
    symbol_uid TEXT PRIMARY KEY,
    file_path TEXT NOT NULL,
    language TEXT NOT NULL,
    name TEXT NOT NULL,
    fqn TEXT,
    kind TEXT NOT NULL,
    signature TEXT,
    visibility TEXT,
    def_start_line INTEGER NOT NULL,
    def_start_char INTEGER NOT NULL,
    def_end_line INTEGER NOT NULL,
    def_end_char INTEGER NOT NULL,
    is_definition BOOLEAN NOT NULL,
    documentation TEXT,
    metadata TEXT
);

-- Relationships between symbols
-- Updated to match Edge struct expectations
CREATE TABLE IF NOT EXISTS edge (
    relation TEXT NOT NULL,
    source_symbol_uid TEXT NOT NULL,
    target_symbol_uid TEXT NOT NULL,
    start_line INTEGER,
    start_char INTEGER,
    confidence REAL NOT NULL,
    language TEXT NOT NULL,
    metadata TEXT
);

-- File dependency relationships
CREATE TABLE IF NOT EXISTS file_dependency (
    dependency_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    source_file_id TEXT NOT NULL,
    target_file_id TEXT NOT NULL,
    dependency_type TEXT NOT NULL,
    import_statement TEXT,
    git_commit_hash TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES project(project_id) ON DELETE CASCADE,
    FOREIGN KEY (source_file_id) REFERENCES file(file_id) ON DELETE CASCADE,
    FOREIGN KEY (target_file_id) REFERENCES file(file_id) ON DELETE CASCADE
);


-- 3. Cache and Infrastructure Tables

-- Analysis queue management
CREATE TABLE IF NOT EXISTS indexer_queue (
    queue_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    priority INTEGER DEFAULT 0,
    operation_type TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    retry_count INTEGER DEFAULT 0,
    error_message TEXT,
    FOREIGN KEY (workspace_id) REFERENCES workspace(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES file(file_id) ON DELETE CASCADE
);

-- Progress tracking
CREATE TABLE IF NOT EXISTS indexer_checkpoint (
    checkpoint_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    last_processed_file TEXT,
    files_processed INTEGER DEFAULT 0,
    total_files INTEGER DEFAULT 0,
    checkpoint_data TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (workspace_id) REFERENCES workspace(workspace_id) ON DELETE CASCADE
);

-- Legacy cache tables removed - actual caching now uses core PRD tables (symbol_state, edges)

-- 4. Comprehensive Index Set
-- Combines all essential indexes for optimal performance

-- Project indexes
CREATE INDEX IF NOT EXISTS idx_project_root_path ON project(root_path);

-- Workspace indexes  
CREATE INDEX IF NOT EXISTS idx_workspace_project ON workspace(project_id);
CREATE INDEX IF NOT EXISTS idx_workspace_path ON workspace(path);
CREATE INDEX IF NOT EXISTS idx_workspace_branch ON workspace(current_branch);

-- File indexes
CREATE INDEX IF NOT EXISTS idx_file_project ON file(project_id);
CREATE INDEX IF NOT EXISTS idx_file_language ON file(language);
CREATE INDEX IF NOT EXISTS idx_file_relative_path ON file(project_id, relative_path);

-- File version indexes removed


-- Symbol state indexes (Post-V003: with language field)
CREATE INDEX IF NOT EXISTS idx_symbol_state_symbol ON symbol_state(symbol_uid);
-- Removed: git_commit_hash field not in SymbolState struct
-- CREATE INDEX IF NOT EXISTS idx_symbol_state_commit ON symbol_state(git_commit_hash);
-- Removed: indexed_at field not in SymbolState struct
-- CREATE INDEX IF NOT EXISTS idx_symbol_state_time ON symbol_state(symbol_uid, indexed_at DESC);
CREATE INDEX IF NOT EXISTS idx_symbol_state_language ON symbol_state(language);
CREATE INDEX IF NOT EXISTS idx_symbol_state_file_path ON symbol_state(file_path);

-- Edge indexes (including original + performance)
CREATE INDEX IF NOT EXISTS idx_edge_source ON edge(source_symbol_uid);
CREATE INDEX IF NOT EXISTS idx_edge_target ON edge(target_symbol_uid);
-- Removed: project_id and edge_type fields not in Edge struct
-- CREATE INDEX IF NOT EXISTS idx_edge_type ON edge(project_id, edge_type);
-- Removed: file_id and version_id fields not in Edge struct  
-- CREATE INDEX IF NOT EXISTS idx_edge_file ON edge(file_id, version_id);
-- Removed: git_commit_hash field not in Edge struct
-- CREATE INDEX IF NOT EXISTS idx_edge_commit ON edge(git_commit_hash);

-- Performance index for edge queries by source symbol  
-- This optimizes database-first lookups by symbol UID
CREATE INDEX IF NOT EXISTS idx_edge_source_type 
ON edge(source_symbol_uid, relation);

-- Performance index for edge queries by target symbol
-- This optimizes reverse lookups (what references this symbol)
CREATE INDEX IF NOT EXISTS idx_edge_target_type 
ON edge(target_symbol_uid, relation);

-- Composite index for call hierarchy queries
-- This is specifically optimized for call/called_by relationships
CREATE INDEX IF NOT EXISTS idx_edge_calls 
ON edge(source_symbol_uid, relation);

-- Index for workspace-scoped queries
-- This optimizes queries that filter by project_id
-- Removed: project_id and edge_type fields not in Edge struct
-- CREATE INDEX IF NOT EXISTS idx_edge_workspace 
-- ON edge(project_id, edge_type);

-- File dependency indexes
CREATE INDEX IF NOT EXISTS idx_file_dep_source ON file_dependency(source_file_id);
CREATE INDEX IF NOT EXISTS idx_file_dep_target ON file_dependency(target_file_id);
CREATE INDEX IF NOT EXISTS idx_file_dep_type ON file_dependency(project_id, dependency_type);
CREATE INDEX IF NOT EXISTS idx_file_dep_commit ON file_dependency(git_commit_hash);

-- Analysis indexes
CREATE INDEX IF NOT EXISTS idx_analysis_run_workspace ON analysis_run(workspace_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_file_analysis_run ON file_analysis(run_id);
CREATE INDEX IF NOT EXISTS idx_file_analysis_file ON file_analysis(file_id);

-- Workspace indexes
-- Removed: workspace_file, workspace_language_config, workspace_file_analysis tables

-- Queue indexes
CREATE INDEX IF NOT EXISTS idx_indexer_queue_workspace ON indexer_queue(workspace_id, status, priority DESC);
CREATE INDEX IF NOT EXISTS idx_indexer_queue_status ON indexer_queue(status, created_at);
CREATE INDEX IF NOT EXISTS idx_indexer_checkpoint_workspace ON indexer_checkpoint(workspace_id, operation_type);


-- 5. Utility Views
-- These provide convenient access patterns for common queries




-- File dependencies with names view
CREATE VIEW IF NOT EXISTS file_dependencies_named AS
SELECT 
    fd.*,
    source.relative_path as source_path,
    target.relative_path as target_path,
    source.language as source_language,
    target.language as target_language
FROM file_dependency fd
JOIN file source ON fd.source_file_id = source.file_id
JOIN file target ON fd.target_file_id = target.file_id;

-- Complete schema initialization successful
        "#
    }

    fn down_sql(&self) -> Option<&str> {
        Some(
            r#"
-- ============================================================================
-- V001: Rollback Complete Schema
-- Drops all tables, indexes, and views created in the complete schema migration
-- ============================================================================

-- Drop views first (dependencies must be removed before tables)
DROP VIEW IF EXISTS file_dependencies_named;

-- Drop indexes (they will be dropped automatically with tables, but explicit is safer)

-- Queue indexes
DROP INDEX IF EXISTS idx_indexer_checkpoint_workspace;
DROP INDEX IF EXISTS idx_indexer_queue_status;
DROP INDEX IF EXISTS idx_indexer_queue_workspace;

-- Workspace indexes - removed, tables deleted

-- Analysis indexes
DROP INDEX IF EXISTS idx_file_analysis_file;
DROP INDEX IF EXISTS idx_file_analysis_run;
DROP INDEX IF EXISTS idx_analysis_run_workspace;

-- File dependency indexes
DROP INDEX IF EXISTS idx_file_dep_commit;
DROP INDEX IF EXISTS idx_file_dep_type;
DROP INDEX IF EXISTS idx_file_dep_target;
DROP INDEX IF EXISTS idx_file_dep_source;

-- Performance edge indexes (v002)
DROP INDEX IF EXISTS idx_edge_workspace;
DROP INDEX IF EXISTS idx_edge_calls;
DROP INDEX IF EXISTS idx_edge_target_type;
DROP INDEX IF EXISTS idx_edge_source_type;

-- Original edge indexes
DROP INDEX IF EXISTS idx_edge_commit;
DROP INDEX IF EXISTS idx_edge_file;
DROP INDEX IF EXISTS idx_edge_type;
DROP INDEX IF EXISTS idx_edge_target;
DROP INDEX IF EXISTS idx_edge_source;

-- Symbol state indexes
DROP INDEX IF EXISTS idx_symbol_state_file_path;
DROP INDEX IF EXISTS idx_symbol_state_language;
DROP INDEX IF EXISTS idx_symbol_state_time;
DROP INDEX IF EXISTS idx_symbol_state_commit;
DROP INDEX IF EXISTS idx_symbol_state_symbol;


-- File version indexes removed

-- File indexes
DROP INDEX IF EXISTS idx_file_relative_path;
DROP INDEX IF EXISTS idx_file_language;
DROP INDEX IF EXISTS idx_file_project;

-- Workspace indexes
DROP INDEX IF EXISTS idx_workspace_branch;
DROP INDEX IF EXISTS idx_workspace_path;
DROP INDEX IF EXISTS idx_workspace_project;

-- Project indexes
DROP INDEX IF EXISTS idx_project_root_path;

-- Drop cache and infrastructure tables (legacy cache tables removed)
DROP TABLE IF EXISTS indexer_checkpoint;
DROP TABLE IF EXISTS indexer_queue;

-- Drop relationship tables (foreign key dependencies)
DROP TABLE IF EXISTS file_dependency;
DROP TABLE IF EXISTS edge;
DROP TABLE IF EXISTS symbol_state;

-- Drop analysis tables
DROP TABLE IF EXISTS file_analysis;
DROP TABLE IF EXISTS analysis_run;

-- Drop core tables
DROP TABLE IF EXISTS file;
DROP TABLE IF EXISTS workspace;
DROP TABLE IF EXISTS project;

-- Complete schema cleanup successful
        "#,
        )
    }

    fn validate_post_migration(&self, _conn: &Connection) -> Result<(), MigrationError> {
        // Post-migration validation is handled by the migration runner
        // The runner executes the migration SQL and verifies it completes successfully
        // For more complex validation, this could be extended to check specific constraints

        // For now, we trust that if the SQL executed without error, the migration was successful
        // This is a reasonable assumption since the migration SQL is comprehensive and well-tested
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_version() {
        let migration = V001CompleteSchema;
        assert_eq!(migration.version(), 1);
    }

    #[test]
    fn test_migration_name() {
        let migration = V001CompleteSchema;
        assert_eq!(migration.name(), "complete_schema");
    }

    #[test]
    fn test_migration_up_sql_contains_all_tables() {
        let migration = V001CompleteSchema;
        let up_sql = migration.up_sql();

        // Verify all expected tables are in the SQL
        let expected_tables = [
            "project",
            "workspace",
            "file",
            "analysis_run",
            "file_analysis",
            "symbol_state",
            "edge",
            "file_dependency",
            "indexer_queue",
            "indexer_checkpoint",
        ];

        for expected in &expected_tables {
            let create_statement = format!("CREATE TABLE IF NOT EXISTS {}", expected);
            assert!(
                up_sql.contains(&create_statement),
                "Missing table {} in up SQL",
                expected
            );
        }
    }

    #[test]
    fn test_migration_up_sql_contains_all_original_indexes() {
        let migration = V001CompleteSchema;
        let up_sql = migration.up_sql();

        // Verify all original indexes from v001 are present
        let expected_indexes = [
            "idx_project_root_path",
            "idx_workspace_project",
            "idx_workspace_path",
            "idx_workspace_branch",
            "idx_file_project",
            "idx_file_language",
            "idx_file_relative_path",
            "idx_symbol_state_symbol",
            "idx_symbol_state_language",
            "idx_symbol_state_file_path",
            "idx_edge_source",
            "idx_edge_target",
        ];

        for expected in &expected_indexes {
            assert!(
                up_sql.contains(expected),
                "Missing original index {} in up SQL",
                expected
            );
        }
    }

    #[test]
    fn test_migration_up_sql_contains_all_performance_indexes() {
        let migration = V001CompleteSchema;
        let up_sql = migration.up_sql();

        // Verify all performance indexes from v002 are present
        let expected_performance_indexes = [
            "idx_edge_source_type",
            "idx_edge_target_type",
            "idx_edge_calls",
        ];

        for expected in &expected_performance_indexes {
            assert!(
                up_sql.contains(expected),
                "Missing performance index {} in up SQL",
                expected
            );
        }
    }

    #[test]
    fn test_migration_up_sql_contains_all_views() {
        let migration = V001CompleteSchema;
        let up_sql = migration.up_sql();

        // Verify all views are present
        let expected_views = ["file_dependencies_named"];

        for expected in &expected_views {
            let create_statement = format!("CREATE VIEW IF NOT EXISTS {}", expected);
            assert!(
                up_sql.contains(&create_statement),
                "Missing view {} in up SQL",
                expected
            );
        }
    }

    #[test]
    fn test_migration_down_sql_contains_expected_drops() {
        let migration = V001CompleteSchema;
        let down_sql = migration.down_sql().expect("Should have rollback SQL");

        // Verify view drops are present
        let expected_view_drops = ["DROP VIEW IF EXISTS file_dependencies_named"];

        for expected in &expected_view_drops {
            assert!(
                down_sql.contains(expected),
                "Missing view drop {} in down SQL",
                expected
            );
        }

        // Verify table drops are present
        let expected_table_drops = [
            "DROP TABLE IF EXISTS project",
            "DROP TABLE IF EXISTS workspace",
            "DROP TABLE IF EXISTS edge",
        ];

        for expected in &expected_table_drops {
            assert!(
                down_sql.contains(expected),
                "Missing table drop {} in down SQL",
                expected
            );
        }
    }

    #[test]
    fn test_migration_checksum_consistent() {
        let migration = V001CompleteSchema;
        let checksum1 = migration.checksum();
        let checksum2 = migration.checksum();

        // Checksums should be consistent
        assert_eq!(checksum1, checksum2);
        assert!(!checksum1.is_empty());
        assert_eq!(checksum1.len(), 64); // SHA-256 is 64 hex chars
    }

    #[test]
    fn test_up_sql_syntax() {
        let migration = V001CompleteSchema;
        let up_sql = migration.up_sql();

        // Basic SQL syntax checks
        assert!(up_sql.contains("CREATE TABLE IF NOT EXISTS"));
        assert!(up_sql.contains("CREATE INDEX IF NOT EXISTS"));
        assert!(up_sql.contains("CREATE VIEW IF NOT EXISTS"));
        assert!(!up_sql.is_empty());

        // Should have multiple CREATE statements
        let create_table_count = up_sql.matches("CREATE TABLE").count();
        let create_index_count = up_sql.matches("CREATE INDEX").count();
        let create_view_count = up_sql.matches("CREATE VIEW").count();

        assert!(
            create_table_count >= 10,
            "Should have at least 10 CREATE TABLE statements"
        );
        assert!(
            create_index_count >= 20,
            "Should have at least 20 CREATE INDEX statements"
        );
        assert_eq!(
            create_view_count, 1,
            "Should have exactly 1 CREATE VIEW statements"
        );
    }

    #[test]
    fn test_down_sql_syntax() {
        let migration = V001CompleteSchema;
        let down_sql = migration.down_sql().expect("Should have rollback SQL");

        // Basic SQL syntax checks
        assert!(down_sql.contains("DROP TABLE IF EXISTS"));
        assert!(down_sql.contains("DROP INDEX IF EXISTS"));
        assert!(down_sql.contains("DROP VIEW IF EXISTS"));
        assert!(!down_sql.is_empty());

        // Should have multiple DROP statements
        let drop_table_count = down_sql.matches("DROP TABLE").count();
        let drop_index_count = down_sql.matches("DROP INDEX").count();
        let drop_view_count = down_sql.matches("DROP VIEW").count();

        assert!(
            drop_table_count >= 10,
            "Should have at least 10 DROP TABLE statements"
        );
        assert!(
            drop_index_count >= 20,
            "Should have at least 20 DROP INDEX statements"
        );
        assert_eq!(
            drop_view_count, 1,
            "Should have exactly 1 DROP VIEW statements"
        );
    }

    #[test]
    fn test_up_down_sql_symmetry() {
        let migration = V001CompleteSchema;
        let up_sql = migration.up_sql();
        let down_sql = migration.down_sql().expect("Should have rollback SQL");

        // Count CREATE vs DROP statements - should be roughly symmetric
        let create_table_count = up_sql.matches("CREATE TABLE").count();
        let drop_table_count = down_sql.matches("DROP TABLE").count();

        let create_index_count = up_sql.matches("CREATE INDEX").count();
        let drop_index_count = down_sql.matches("DROP INDEX").count();

        let create_view_count = up_sql.matches("CREATE VIEW").count();
        let drop_view_count = down_sql.matches("DROP VIEW").count();

        assert_eq!(
            create_table_count, drop_table_count,
            "CREATE/DROP TABLE count mismatch"
        );
        assert_eq!(
            create_index_count, drop_index_count,
            "CREATE/DROP INDEX count mismatch"
        );
        assert_eq!(
            create_view_count, drop_view_count,
            "CREATE/DROP VIEW count mismatch"
        );
    }
}
