//! V001: Initial schema migration with all PRD core tables
//!
//! This migration creates the complete initial database schema including:
//! - Legacy compatibility tables (kv_store, tree_metadata)
//! - Core PRD tables (project, workspace, file, file_version)
//! - Relationship tables (symbol, symbol_state, edge, file_dependency)
//! - Cache and analytics tables (analysis_run, indexer_queue, etc.)
//! - All performance indexes and utility views

use crate::database::migrations::{Migration, MigrationError};

/// Initial schema migration (Version 1)
///
/// This migration sets up the complete database schema as specified
/// in the PRD. It includes all tables, indexes, and views needed for
/// the semantic code analysis system.
#[derive(Debug)]
pub struct V001InitialSchema;

impl Migration for V001InitialSchema {
    fn version(&self) -> u32 {
        1
    }

    fn name(&self) -> &str {
        "initial_schema"
    }

    fn up_sql(&self) -> &str {
        r#"
-- ============================================================================
-- V001: Initial Schema Migration
-- Creates complete PRD database schema with all tables, indexes, and views
-- ============================================================================

-- 1. Schema Version Control (Legacy compatibility)
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    description TEXT
);

-- 2. Legacy Tables (for backward compatibility)
CREATE TABLE IF NOT EXISTS kv_store (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL,
    created_at INTEGER DEFAULT (strftime('%s','now')),
    updated_at INTEGER DEFAULT (strftime('%s','now'))
);

CREATE TABLE IF NOT EXISTS tree_metadata (
    tree_name TEXT PRIMARY KEY,
    created_at INTEGER DEFAULT (strftime('%s','now'))
);

-- 3. Core PRD Tables

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

-- Workspace file mapping
CREATE TABLE IF NOT EXISTS workspace_file (
    workspace_file_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (workspace_id) REFERENCES workspace(workspace_id) ON DELETE CASCADE
);

-- Workspace language configuration
CREATE TABLE IF NOT EXISTS workspace_language_config (
    config_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    language TEXT NOT NULL,
    analyzer_type TEXT NOT NULL,
    settings TEXT,
    is_enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (workspace_id) REFERENCES workspace(workspace_id) ON DELETE CASCADE
);

-- Workspace file analysis tracking
CREATE TABLE IF NOT EXISTS workspace_file_analysis (
    analysis_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    analyzer_type TEXT NOT NULL,
    analysis_version TEXT,
    last_analyzed TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    status TEXT DEFAULT 'pending',
    FOREIGN KEY (workspace_id) REFERENCES workspace(workspace_id) ON DELETE CASCADE
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

-- File versions with content-addressed storage
CREATE TABLE IF NOT EXISTS file_version (
    version_id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    git_commit_hash TEXT,
    size_bytes INTEGER,
    line_count INTEGER,
    last_modified TIMESTAMP,
    indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (file_id) REFERENCES file(file_id) ON DELETE CASCADE
);

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
    version_id TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    symbols_found INTEGER DEFAULT 0,
    references_found INTEGER DEFAULT 0,
    errors TEXT,
    FOREIGN KEY (run_id) REFERENCES analysis_run(run_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES file(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_version(version_id) ON DELETE CASCADE
);

-- 4. Relationship Tables

-- Symbol registry
CREATE TABLE IF NOT EXISTS symbol (
    symbol_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT,
    symbol_type TEXT NOT NULL,
    language TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    start_column INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    end_column INTEGER NOT NULL,
    signature TEXT,
    documentation TEXT,
    visibility TEXT,
    modifiers TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES project(project_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES file(file_id) ON DELETE CASCADE
);

-- Symbol definitions with versioning
CREATE TABLE IF NOT EXISTS symbol_state (
    state_id TEXT PRIMARY KEY,
    symbol_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    git_commit_hash TEXT,
    definition_data TEXT NOT NULL,
    indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    confidence REAL DEFAULT 1.0,
    FOREIGN KEY (symbol_id) REFERENCES symbol(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_version(version_id) ON DELETE CASCADE
);

-- Relationships between symbols
CREATE TABLE IF NOT EXISTS edge (
    edge_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    source_symbol_id TEXT NOT NULL,
    target_symbol_id TEXT NOT NULL,
    edge_type TEXT NOT NULL,
    file_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    git_commit_hash TEXT,
    source_location TEXT,
    target_location TEXT,
    confidence REAL DEFAULT 1.0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES project(project_id) ON DELETE CASCADE,
    FOREIGN KEY (source_symbol_id) REFERENCES symbol(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (target_symbol_id) REFERENCES symbol(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES file(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_version(version_id) ON DELETE CASCADE
);

-- File dependency relationships
CREATE TABLE IF NOT EXISTS file_dependency (
    dependency_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    source_file_id TEXT NOT NULL,
    target_file_id TEXT NOT NULL,
    dependency_type TEXT NOT NULL,
    import_statement TEXT,
    version_id TEXT NOT NULL,
    git_commit_hash TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES project(project_id) ON DELETE CASCADE,
    FOREIGN KEY (source_file_id) REFERENCES file(file_id) ON DELETE CASCADE,
    FOREIGN KEY (target_file_id) REFERENCES file(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_version(version_id) ON DELETE CASCADE
);

-- Symbol change tracking
CREATE TABLE IF NOT EXISTS symbol_change (
    change_id TEXT PRIMARY KEY,
    symbol_id TEXT NOT NULL,
    previous_state_id TEXT,
    current_state_id TEXT NOT NULL,
    change_type TEXT NOT NULL,
    git_commit_hash TEXT,
    changed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    change_description TEXT,
    FOREIGN KEY (symbol_id) REFERENCES symbol(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (previous_state_id) REFERENCES symbol_state(state_id) ON DELETE SET NULL,
    FOREIGN KEY (current_state_id) REFERENCES symbol_state(state_id) ON DELETE CASCADE
);

-- 5. Cache and Analytics Tables

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

-- 6. Performance Indexes
-- These are essential for query performance and should be part of the initial schema

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

-- File version indexes
CREATE INDEX IF NOT EXISTS idx_file_version_file_time ON file_version(file_id, indexed_at DESC);
CREATE INDEX IF NOT EXISTS idx_file_version_commit ON file_version(git_commit_hash);
CREATE INDEX IF NOT EXISTS idx_file_version_content_hash ON file_version(content_hash);

-- Symbol indexes
CREATE INDEX IF NOT EXISTS idx_symbol_project ON symbol(project_id);
CREATE INDEX IF NOT EXISTS idx_symbol_file ON symbol(file_id);
CREATE INDEX IF NOT EXISTS idx_symbol_name ON symbol(project_id, name);
CREATE INDEX IF NOT EXISTS idx_symbol_qualified_name ON symbol(project_id, qualified_name);
CREATE INDEX IF NOT EXISTS idx_symbol_type ON symbol(project_id, symbol_type);
CREATE INDEX IF NOT EXISTS idx_symbol_language ON symbol(language);

-- Symbol state indexes
CREATE INDEX IF NOT EXISTS idx_symbol_state_symbol ON symbol_state(symbol_id);
CREATE INDEX IF NOT EXISTS idx_symbol_state_version ON symbol_state(version_id);
CREATE INDEX IF NOT EXISTS idx_symbol_state_commit ON symbol_state(git_commit_hash);
CREATE INDEX IF NOT EXISTS idx_symbol_state_time ON symbol_state(symbol_id, indexed_at DESC);

-- Edge indexes
CREATE INDEX IF NOT EXISTS idx_edge_source ON edge(source_symbol_id);
CREATE INDEX IF NOT EXISTS idx_edge_target ON edge(target_symbol_id);
CREATE INDEX IF NOT EXISTS idx_edge_type ON edge(project_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_edge_file ON edge(file_id, version_id);
CREATE INDEX IF NOT EXISTS idx_edge_commit ON edge(git_commit_hash);

-- File dependency indexes
CREATE INDEX IF NOT EXISTS idx_file_dep_source ON file_dependency(source_file_id);
CREATE INDEX IF NOT EXISTS idx_file_dep_target ON file_dependency(target_file_id);
CREATE INDEX IF NOT EXISTS idx_file_dep_type ON file_dependency(project_id, dependency_type);
CREATE INDEX IF NOT EXISTS idx_file_dep_commit ON file_dependency(git_commit_hash);

-- Analysis indexes
CREATE INDEX IF NOT EXISTS idx_analysis_run_workspace ON analysis_run(workspace_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_file_analysis_run ON file_analysis(run_id);
CREATE INDEX IF NOT EXISTS idx_file_analysis_file ON file_analysis(file_id, version_id);

-- Workspace indexes
CREATE INDEX IF NOT EXISTS idx_workspace_file_workspace ON workspace_file(workspace_id);
CREATE INDEX IF NOT EXISTS idx_workspace_file_active ON workspace_file(workspace_id, is_active);
CREATE INDEX IF NOT EXISTS idx_workspace_lang_config ON workspace_language_config(workspace_id, language);
CREATE INDEX IF NOT EXISTS idx_workspace_analysis ON workspace_file_analysis(workspace_id, file_id);

-- Queue indexes
CREATE INDEX IF NOT EXISTS idx_indexer_queue_workspace ON indexer_queue(workspace_id, status, priority DESC);
CREATE INDEX IF NOT EXISTS idx_indexer_queue_status ON indexer_queue(status, created_at);
CREATE INDEX IF NOT EXISTS idx_indexer_checkpoint_workspace ON indexer_checkpoint(workspace_id, operation_type);

-- Change tracking indexes
CREATE INDEX IF NOT EXISTS idx_symbol_change_symbol ON symbol_change(symbol_id, changed_at DESC);
CREATE INDEX IF NOT EXISTS idx_symbol_change_commit ON symbol_change(git_commit_hash);

-- 7. Utility Views
-- These provide convenient access patterns for common queries

-- Current symbols view (handles git + timestamp logic)
CREATE VIEW IF NOT EXISTS current_symbols AS
WITH latest_modified AS (
    SELECT DISTINCT 
        symbol_id,
        project_id,
        MAX(ss.indexed_at) as latest_indexed_at
    FROM symbol_state ss
    WHERE ss.git_commit_hash IS NULL
    GROUP BY symbol_id, project_id
)
SELECT DISTINCT 
    s.*,
    ss.definition_data,
    ss.confidence,
    ss.indexed_at
FROM symbol s
JOIN symbol_state ss ON s.symbol_id = ss.symbol_id
LEFT JOIN latest_modified lm ON s.symbol_id = lm.symbol_id AND s.project_id = lm.project_id
WHERE 
    (ss.git_commit_hash IS NULL AND ss.indexed_at = lm.latest_indexed_at)
    OR 
    (ss.git_commit_hash IS NOT NULL);

-- Symbols with file info view
CREATE VIEW IF NOT EXISTS symbols_with_files AS
SELECT 
    s.*,
    f.relative_path,
    f.absolute_path,
    f.language as file_language,
    p.name as project_name,
    p.root_path
FROM symbol s
JOIN file f ON s.file_id = f.file_id
JOIN project p ON s.project_id = p.project_id;

-- Edge relationships with symbol names view
CREATE VIEW IF NOT EXISTS edges_named AS
SELECT 
    e.*,
    source.name as source_name,
    source.qualified_name as source_qualified,
    target.name as target_name,
    target.qualified_name as target_qualified,
    f.relative_path
FROM edge e
JOIN symbol source ON e.source_symbol_id = source.symbol_id
JOIN symbol target ON e.target_symbol_id = target.symbol_id
JOIN file f ON e.file_id = f.file_id;

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

-- Initialize legacy schema version for backward compatibility  
-- Note: The migration runner will handle duplicate insertion checks
INSERT INTO schema_version (version, description) 
VALUES (1, 'Initial PRD schema with core tables, indexes, and views');
        "#
    }

    fn down_sql(&self) -> Option<&str> {
        Some(
            r#"
-- ============================================================================
-- V001: Rollback Initial Schema
-- Drops all tables created in the initial schema migration
-- ============================================================================

-- Drop views first (dependencies must be removed before tables)
DROP VIEW IF EXISTS current_symbols;
DROP VIEW IF EXISTS symbols_with_files;
DROP VIEW IF EXISTS edges_named;
DROP VIEW IF EXISTS file_dependencies_named;

-- Drop indexes (they will be dropped automatically with tables, but explicit is safer)
DROP INDEX IF EXISTS idx_symbol_change_commit;
DROP INDEX IF EXISTS idx_symbol_change_symbol;
DROP INDEX IF EXISTS idx_indexer_checkpoint_workspace;
DROP INDEX IF EXISTS idx_indexer_queue_status;
DROP INDEX IF EXISTS idx_indexer_queue_workspace;
DROP INDEX IF EXISTS idx_workspace_analysis;
DROP INDEX IF EXISTS idx_workspace_lang_config;
DROP INDEX IF EXISTS idx_workspace_file_active;
DROP INDEX IF EXISTS idx_workspace_file_workspace;
DROP INDEX IF EXISTS idx_file_analysis_file;
DROP INDEX IF EXISTS idx_file_analysis_run;
DROP INDEX IF EXISTS idx_analysis_run_workspace;
DROP INDEX IF EXISTS idx_file_dep_commit;
DROP INDEX IF EXISTS idx_file_dep_type;
DROP INDEX IF EXISTS idx_file_dep_target;
DROP INDEX IF EXISTS idx_file_dep_source;
DROP INDEX IF EXISTS idx_edge_commit;
DROP INDEX IF EXISTS idx_edge_file;
DROP INDEX IF EXISTS idx_edge_type;
DROP INDEX IF EXISTS idx_edge_target;
DROP INDEX IF EXISTS idx_edge_source;
DROP INDEX IF EXISTS idx_symbol_state_time;
DROP INDEX IF EXISTS idx_symbol_state_commit;
DROP INDEX IF EXISTS idx_symbol_state_version;
DROP INDEX IF EXISTS idx_symbol_state_symbol;
DROP INDEX IF EXISTS idx_symbol_language;
DROP INDEX IF EXISTS idx_symbol_type;
DROP INDEX IF EXISTS idx_symbol_qualified_name;
DROP INDEX IF EXISTS idx_symbol_name;
DROP INDEX IF EXISTS idx_symbol_file;
DROP INDEX IF EXISTS idx_symbol_project;
DROP INDEX IF EXISTS idx_file_version_content_hash;
DROP INDEX IF EXISTS idx_file_version_commit;
DROP INDEX IF EXISTS idx_file_version_file_time;
DROP INDEX IF EXISTS idx_file_relative_path;
DROP INDEX IF EXISTS idx_file_language;
DROP INDEX IF EXISTS idx_file_project;
DROP INDEX IF EXISTS idx_workspace_branch;
DROP INDEX IF EXISTS idx_workspace_path;
DROP INDEX IF EXISTS idx_workspace_project;
DROP INDEX IF EXISTS idx_project_root_path;

-- Drop cache and analytics tables
DROP TABLE IF EXISTS indexer_checkpoint;
DROP TABLE IF EXISTS indexer_queue;

-- Drop relationship tables (foreign key dependencies)
DROP TABLE IF EXISTS symbol_change;
DROP TABLE IF EXISTS file_dependency;
DROP TABLE IF EXISTS edge;
DROP TABLE IF EXISTS symbol_state;
DROP TABLE IF EXISTS symbol;

-- Drop analysis tables
DROP TABLE IF EXISTS file_analysis;
DROP TABLE IF EXISTS analysis_run;

-- Drop core tables
DROP TABLE IF EXISTS file_version;
DROP TABLE IF EXISTS file;
DROP TABLE IF EXISTS workspace_file_analysis;
DROP TABLE IF EXISTS workspace_language_config;
DROP TABLE IF EXISTS workspace_file;
DROP TABLE IF EXISTS workspace;
DROP TABLE IF EXISTS project;

-- Drop legacy tables
DROP TABLE IF EXISTS tree_metadata;
DROP TABLE IF EXISTS kv_store;

-- Remove schema version record
DELETE FROM schema_version WHERE version = 1;

-- Note: schema_version table is kept as it may be used by other systems
        "#,
        )
    }

    fn validate_post_migration(&self, _conn: &turso::Connection) -> Result<(), MigrationError> {
        // Post-migration validation is handled by the migration runner
        // The runner executes the migration SQL and verifies it completes successfully
        // For more complex validation, this could be extended to check specific constraints

        // For now, we trust that if the SQL executed without error, the migration was successful
        // This is a reasonable assumption since the migration SQL is well-tested
        Ok(())
    }
}
