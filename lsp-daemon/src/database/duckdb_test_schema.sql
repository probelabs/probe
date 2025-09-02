-- DuckDB Test Schema for Bootstrap Testing
-- Simplified schema without CASCADE constraints for DuckDB 1.3 compatibility

-- Wrap entire DDL in single transaction for atomicity
BEGIN TRANSACTION;

-- Enable basic optimizations
PRAGMA enable_object_cache=true;

-- =============================================================================
-- Core Test Tables (simplified versions)
-- =============================================================================

-- 1. Workspaces (Projects)
CREATE TABLE IF NOT EXISTS workspaces (
    workspace_id        TEXT PRIMARY KEY,
    root_path           TEXT NOT NULL UNIQUE,
    name                TEXT NOT NULL,
    current_commit      TEXT,
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_workspaces_root_path ON workspaces(root_path);

-- 2. Files  
CREATE TABLE IF NOT EXISTS files (
    file_id             TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    relative_path       TEXT NOT NULL,
    absolute_path       TEXT NOT NULL,
    language            TEXT,
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id),
    UNIQUE (workspace_id, relative_path)
);

CREATE INDEX IF NOT EXISTS idx_files_workspace ON files(workspace_id);
CREATE INDEX IF NOT EXISTS idx_files_language ON files(language);

-- 3. Symbols (Simplified version for testing)
CREATE TABLE IF NOT EXISTS symbols (
    symbol_id           TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    file_id             TEXT NOT NULL,
    git_commit_hash     TEXT,
    name                TEXT NOT NULL,
    qualified_name      TEXT,
    kind                TEXT NOT NULL,
    start_line          INTEGER NOT NULL,
    start_column        INTEGER NOT NULL,
    end_line            INTEGER NOT NULL,
    end_column          INTEGER NOT NULL,
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id),
    FOREIGN KEY (file_id) REFERENCES files(file_id)
);

CREATE INDEX IF NOT EXISTS idx_symbols_workspace ON symbols(workspace_id, indexed_at DESC);
CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id, indexed_at DESC);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(workspace_id, name);

-- 4. Call Graph (Simplified)
CREATE TABLE IF NOT EXISTS call_graph (
    call_id             TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    caller_symbol_id    TEXT NOT NULL,
    callee_symbol_id    TEXT NOT NULL,
    file_id             TEXT NOT NULL,
    git_commit_hash     TEXT,
    call_line           INTEGER NOT NULL,
    call_column         INTEGER NOT NULL,
    call_type           TEXT DEFAULT 'direct',
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id),
    FOREIGN KEY (caller_symbol_id) REFERENCES symbols(symbol_id),
    FOREIGN KEY (callee_symbol_id) REFERENCES symbols(symbol_id),
    FOREIGN KEY (file_id) REFERENCES files(file_id)
);

CREATE INDEX IF NOT EXISTS idx_call_graph_caller ON call_graph(caller_symbol_id);
CREATE INDEX IF NOT EXISTS idx_call_graph_callee ON call_graph(callee_symbol_id);
CREATE INDEX IF NOT EXISTS idx_call_graph_workspace ON call_graph(workspace_id, indexed_at DESC);

-- 5. Symbol References (Simplified)
CREATE TABLE IF NOT EXISTS symbol_references (
    reference_id        TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    source_symbol_id    TEXT NOT NULL,
    target_symbol_id    TEXT NOT NULL,
    file_id             TEXT NOT NULL,
    git_commit_hash     TEXT,
    ref_line            INTEGER NOT NULL,
    ref_column          INTEGER NOT NULL,
    reference_kind      TEXT DEFAULT 'use',
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id),
    FOREIGN KEY (source_symbol_id) REFERENCES symbols(symbol_id),
    FOREIGN KEY (target_symbol_id) REFERENCES symbols(symbol_id),
    FOREIGN KEY (file_id) REFERENCES files(file_id)
);

CREATE INDEX IF NOT EXISTS idx_references_source ON symbol_references(source_symbol_id);
CREATE INDEX IF NOT EXISTS idx_references_target ON symbol_references(target_symbol_id);
CREATE INDEX IF NOT EXISTS idx_references_workspace ON symbol_references(workspace_id, indexed_at DESC);

-- 6. LSP Cache (Simplified)
CREATE TABLE IF NOT EXISTS lsp_cache (
    cache_key           TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    method              TEXT NOT NULL,
    file_id             TEXT NOT NULL,
    git_commit_hash     TEXT,
    request_params      JSON NOT NULL,
    position_line       INTEGER,
    position_column     INTEGER,
    response_data       JSON NOT NULL,
    response_type       TEXT,
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_accessed       TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    access_count        INTEGER DEFAULT 1,
    response_time_ms    INTEGER,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id),
    FOREIGN KEY (file_id) REFERENCES files(file_id)
);

CREATE INDEX IF NOT EXISTS idx_lsp_cache_workspace_method ON lsp_cache(workspace_id, method);
CREATE INDEX IF NOT EXISTS idx_lsp_cache_file ON lsp_cache(file_id);
CREATE INDEX IF NOT EXISTS idx_lsp_cache_position ON lsp_cache(file_id, position_line, position_column);

-- =============================================================================
-- Default KV Store Table (for DatabaseBackend trait)
-- =============================================================================

-- Key-value store for general cache operations
CREATE TABLE IF NOT EXISTS kv_store (
    key                 TEXT PRIMARY KEY,
    value               BLOB NOT NULL,
    updated_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_kv_store_updated ON kv_store(updated_at);

-- =============================================================================
-- Tree Metadata Table (for tree management)
-- =============================================================================

CREATE TABLE IF NOT EXISTS tree_metadata (
    tree_name           TEXT PRIMARY KEY,
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    entry_count         INTEGER DEFAULT 0
);

-- =============================================================================
-- Basic Optimization
-- =============================================================================

-- Analyze tables for query optimization
ANALYZE workspaces;
ANALYZE files;  
ANALYZE symbols;
ANALYZE call_graph;
ANALYZE symbol_references;
ANALYZE lsp_cache;
ANALYZE kv_store;
ANALYZE tree_metadata;

-- Commit the entire DDL transaction
COMMIT;