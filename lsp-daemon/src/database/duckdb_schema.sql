-- DuckDB Schema for Probe LSP Cache System
-- This schema implements git-aware versioning using indexed_at timestamps
-- instead of is_latest flags for better performance and simpler queries

-- Wrap entire DDL in single transaction for atomicity
BEGIN TRANSACTION;

-- Enable foreign key constraints
PRAGMA enable_object_cache=true;

-- =============================================================================
-- Core Tables
-- =============================================================================

-- 1. Workspaces (Projects)
CREATE TABLE IF NOT EXISTS workspaces (
    workspace_id        TEXT PRIMARY KEY,      -- UUID or hash of root path
    root_path           TEXT NOT NULL UNIQUE,
    name                TEXT NOT NULL,
    is_git_repo         BOOLEAN DEFAULT FALSE,
    current_commit      TEXT,                  -- Current git HEAD commit
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_indexed        TIMESTAMP,
    metadata            JSON                   -- Language configs, settings
);

CREATE INDEX IF NOT EXISTS idx_workspaces_root_path ON workspaces(root_path);

-- 2. Files and File Tracking
CREATE TABLE IF NOT EXISTS files (
    file_id             TEXT PRIMARY KEY,      -- workspace_id:relative_path
    workspace_id        TEXT NOT NULL,
    relative_path       TEXT NOT NULL,         -- Relative to workspace root
    absolute_path       TEXT NOT NULL,
    language            TEXT,
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    UNIQUE (workspace_id, relative_path)
);

CREATE INDEX IF NOT EXISTS idx_files_workspace ON files(workspace_id);
CREATE INDEX IF NOT EXISTS idx_files_language ON files(language);

-- 3. File Versions (Content tracking)
CREATE TABLE IF NOT EXISTS file_versions (
    version_id          TEXT PRIMARY KEY,      -- file_id:content_hash
    file_id             TEXT NOT NULL,
    content_hash        TEXT NOT NULL,         -- SHA256 of file content
    git_commit_hash     TEXT,                  -- NULL for uncommitted changes
    
    -- File metadata
    size_bytes          INTEGER,
    line_count          INTEGER,
    last_modified       TIMESTAMP,
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE,
    UNIQUE (file_id, content_hash)
);

-- Critical index for file version queries (most recent first)
CREATE INDEX IF NOT EXISTS idx_file_versions_file_time ON file_versions(file_id, indexed_at DESC);
CREATE INDEX IF NOT EXISTS idx_file_versions_commit ON file_versions(git_commit_hash) WHERE git_commit_hash IS NOT NULL;

-- =============================================================================
-- Symbol Tables
-- =============================================================================

-- 4. Symbols (Core entities with git-aware versioning)
CREATE TABLE IF NOT EXISTS symbols (
    symbol_id           TEXT PRIMARY KEY,      -- Unique identifier (hash)
    workspace_id        TEXT NOT NULL,
    file_id             TEXT NOT NULL,
    version_id          TEXT NOT NULL,
    
    -- Git-aware versioning (using indexed_at instead of is_latest)
    git_commit_hash     TEXT,                  -- NULL for uncommitted
    
    -- Symbol information
    name                TEXT NOT NULL,
    qualified_name      TEXT,                  -- Fully qualified name
    kind                TEXT NOT NULL,         -- function, class, variable, etc.
    
    -- Location in file
    start_line          INTEGER NOT NULL,
    start_column        INTEGER NOT NULL,
    end_line            INTEGER NOT NULL,
    end_column          INTEGER NOT NULL,
    
    -- Additional metadata
    signature           TEXT,                  -- Function signature, type info
    documentation       TEXT,                  -- Doc comments
    visibility          TEXT,                  -- public, private, protected
    modifiers           TEXT[],                -- static, async, const, etc.
    
    -- Indexing metadata (indexed_at is the key for versioning)
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    confidence          DOUBLE DEFAULT 1.0,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_versions(version_id) ON DELETE CASCADE
);

-- Critical indexes for symbol queries (indexed_at DESC for latest-first ordering)
CREATE INDEX IF NOT EXISTS idx_symbols_workspace_current 
    ON symbols(workspace_id, git_commit_hash, indexed_at DESC);
CREATE INDEX IF NOT EXISTS idx_symbols_file_time 
    ON symbols(file_id, indexed_at DESC);
CREATE INDEX IF NOT EXISTS idx_symbols_name 
    ON symbols(workspace_id, name);
CREATE INDEX IF NOT EXISTS idx_symbols_kind 
    ON symbols(workspace_id, kind);
CREATE INDEX IF NOT EXISTS idx_symbols_qualified_name 
    ON symbols(workspace_id, qualified_name);

-- =============================================================================
-- Relationship Tables
-- =============================================================================

-- 5. Symbol Hierarchy (Containment relationships)
CREATE TABLE IF NOT EXISTS symbol_hierarchy (
    hierarchy_id        TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    parent_symbol_id    TEXT NOT NULL,
    child_symbol_id     TEXT NOT NULL,
    git_commit_hash     TEXT,
    relationship_type   TEXT DEFAULT 'contains',   -- contains, extends, implements
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (parent_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (child_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    UNIQUE (parent_symbol_id, child_symbol_id, git_commit_hash)
);

CREATE INDEX IF NOT EXISTS idx_hierarchy_parent ON symbol_hierarchy(parent_symbol_id);
CREATE INDEX IF NOT EXISTS idx_hierarchy_child ON symbol_hierarchy(child_symbol_id);
CREATE INDEX IF NOT EXISTS idx_hierarchy_workspace ON symbol_hierarchy(workspace_id, git_commit_hash, indexed_at DESC);

-- 6. Symbol References (Usage relationships)
CREATE TABLE IF NOT EXISTS symbol_references (
    reference_id        TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    source_symbol_id    TEXT NOT NULL,         -- Symbol making the reference
    target_symbol_id    TEXT NOT NULL,         -- Symbol being referenced
    file_id             TEXT NOT NULL,         -- File containing the reference
    version_id          TEXT NOT NULL,
    git_commit_hash     TEXT,
    
    -- Reference location
    ref_line            INTEGER NOT NULL,
    ref_column          INTEGER NOT NULL,
    ref_end_line        INTEGER,
    ref_end_column      INTEGER,
    
    -- Reference metadata
    reference_kind      TEXT DEFAULT 'use',    -- use, import, type, call
    is_definition       BOOLEAN DEFAULT FALSE,
    is_declaration      BOOLEAN DEFAULT FALSE,
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (source_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (target_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_versions(version_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_references_source ON symbol_references(source_symbol_id);
CREATE INDEX IF NOT EXISTS idx_references_target ON symbol_references(target_symbol_id);
CREATE INDEX IF NOT EXISTS idx_references_file ON symbol_references(file_id, version_id);
CREATE INDEX IF NOT EXISTS idx_references_workspace ON symbol_references(workspace_id, git_commit_hash, indexed_at DESC);

-- 7. Call Graph (Function call relationships)
CREATE TABLE IF NOT EXISTS call_graph (
    call_id             TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    caller_symbol_id    TEXT NOT NULL,
    callee_symbol_id    TEXT NOT NULL,
    file_id             TEXT NOT NULL,
    version_id          TEXT NOT NULL,
    git_commit_hash     TEXT,
    
    -- Call site location
    call_line           INTEGER NOT NULL,
    call_column         INTEGER NOT NULL,
    
    -- Call metadata
    call_type           TEXT DEFAULT 'direct',  -- direct, virtual, callback, async
    argument_count      INTEGER,
    is_recursive        BOOLEAN DEFAULT FALSE,
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (caller_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (callee_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_versions(version_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_call_graph_caller ON call_graph(caller_symbol_id);
CREATE INDEX IF NOT EXISTS idx_call_graph_callee ON call_graph(callee_symbol_id);
CREATE INDEX IF NOT EXISTS idx_call_graph_workspace ON call_graph(workspace_id, git_commit_hash, indexed_at DESC);

-- =============================================================================
-- LSP Caching Tables
-- =============================================================================

-- 8. LSP Cache (Cached language server responses)
CREATE TABLE IF NOT EXISTS lsp_cache (
    cache_key           TEXT PRIMARY KEY,      -- Method-specific cache key
    workspace_id        TEXT NOT NULL,
    method              TEXT NOT NULL,         -- textDocument/definition, etc.
    file_id             TEXT NOT NULL,
    version_id          TEXT NOT NULL,
    git_commit_hash     TEXT,
    
    -- Request context
    request_params      JSON NOT NULL,         -- Original LSP request parameters
    position_line       INTEGER,
    position_column     INTEGER,
    
    -- Cached response
    response_data       JSON NOT NULL,         -- LSP response data
    response_type       TEXT,                  -- locations, hover, symbols, etc.
    
    -- Cache metadata
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_accessed       TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    access_count        INTEGER DEFAULT 1,
    response_time_ms    INTEGER,               -- Time to generate response
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_versions(version_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_lsp_cache_workspace_method ON lsp_cache(workspace_id, method);
CREATE INDEX IF NOT EXISTS idx_lsp_cache_file ON lsp_cache(file_id, version_id);
CREATE INDEX IF NOT EXISTS idx_lsp_cache_position ON lsp_cache(file_id, position_line, position_column);
CREATE INDEX IF NOT EXISTS idx_lsp_cache_access ON lsp_cache(workspace_id, last_accessed);

-- 9. Cache Statistics (Performance monitoring)
CREATE TABLE IF NOT EXISTS cache_statistics (
    stat_id             INTEGER PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    method              TEXT NOT NULL,
    date                DATE DEFAULT CURRENT_DATE,
    
    -- Counters
    total_requests      INTEGER DEFAULT 0,
    cache_hits          INTEGER DEFAULT 0,
    cache_misses        INTEGER DEFAULT 0,
    
    -- Performance metrics
    avg_response_time_ms DOUBLE,
    p95_response_time_ms DOUBLE,
    p99_response_time_ms DOUBLE,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    UNIQUE (workspace_id, method, date)
);

CREATE INDEX IF NOT EXISTS idx_cache_stats_workspace ON cache_statistics(workspace_id, date);

-- =============================================================================
-- Utility Views for Current State Queries
-- =============================================================================

-- Current symbols view (handles git + timestamp logic using indexed_at)
-- This view determines "current" symbols by choosing the most recent version
-- for each symbol, taking into account git commit state
CREATE OR REPLACE VIEW current_symbols AS
WITH latest_modified AS (
    -- Find the most recent indexed_at timestamp for each symbol in modified files
    SELECT DISTINCT ON (symbol_id, workspace_id)
        symbol_id,
        workspace_id,
        indexed_at
    FROM symbols
    WHERE git_commit_hash IS NULL  -- Modified files only
    ORDER BY symbol_id, workspace_id, indexed_at DESC
),
current_state AS (
    SELECT DISTINCT ON (s.symbol_id, s.workspace_id)
        s.*
    FROM symbols s
    LEFT JOIN latest_modified lm 
        ON s.symbol_id = lm.symbol_id 
        AND s.workspace_id = lm.workspace_id
    WHERE 
        -- Latest modified version (most recent indexed_at for uncommitted changes)
        (s.git_commit_hash IS NULL AND s.indexed_at = lm.indexed_at)
        OR 
        -- Current commit version (symbols from current git commit)
        (s.git_commit_hash = (
            SELECT current_commit 
            FROM workspaces 
            WHERE workspace_id = s.workspace_id
        ))
    ORDER BY s.symbol_id, s.workspace_id, s.indexed_at DESC
)
SELECT * FROM current_state;

-- Symbol with file information view
CREATE OR REPLACE VIEW symbols_with_files AS
SELECT 
    s.*,
    f.relative_path,
    f.absolute_path,
    f.language as file_language
FROM symbols s
JOIN files f ON s.file_id = f.file_id;

-- Call graph with symbol names for easy querying
CREATE OR REPLACE VIEW call_graph_named AS
SELECT 
    cg.*,
    caller.name as caller_name,
    caller.qualified_name as caller_qualified,
    callee.name as callee_name,
    callee.qualified_name as callee_qualified
FROM call_graph cg
JOIN symbols caller ON cg.caller_symbol_id = caller.symbol_id
JOIN symbols callee ON cg.callee_symbol_id = callee.symbol_id;

-- Reference graph with symbol names for easy querying  
CREATE OR REPLACE VIEW references_named AS
SELECT 
    sr.*,
    source.name as source_name,
    source.qualified_name as source_qualified,
    target.name as target_name,
    target.qualified_name as target_qualified
FROM symbol_references sr
JOIN symbols source ON sr.source_symbol_id = source.symbol_id
JOIN symbols target ON sr.target_symbol_id = target.symbol_id;

-- =============================================================================
-- Default KV Store Table (for DatabaseBackend trait compatibility)
-- =============================================================================

-- Key-value store for general cache operations and backward compatibility
CREATE TABLE IF NOT EXISTS kv_store (
    key                 TEXT PRIMARY KEY,
    value               BLOB NOT NULL,
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_kv_store_key ON kv_store(key);
CREATE INDEX IF NOT EXISTS idx_kv_store_updated ON kv_store(updated_at);

-- =============================================================================
-- Tree Metadata Table (for tree management)
-- =============================================================================

CREATE TABLE IF NOT EXISTS tree_metadata (
    tree_name           TEXT PRIMARY KEY,
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    entry_count         INTEGER DEFAULT 0,
    last_modified       TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- Performance Optimization Settings
-- =============================================================================

-- Analyze tables for query optimization
ANALYZE workspaces;
ANALYZE files;
ANALYZE file_versions;
ANALYZE symbols;
ANALYZE symbol_hierarchy;
ANALYZE symbol_references;
ANALYZE call_graph;
ANALYZE lsp_cache;
ANALYZE cache_statistics;
ANALYZE kv_store;
ANALYZE tree_metadata;

-- Enable optimizations
PRAGMA enable_object_cache=true;
PRAGMA enable_profiling='json';
PRAGMA preserve_insertion_order=false;

-- Commit the entire DDL transaction
COMMIT;