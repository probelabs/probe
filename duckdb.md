# DuckDB Backend Implementation Guide

## Table of Contents
1. [Architecture Overview](#architecture-overview)
2. [Core Concepts](#core-concepts)
3. [Complete Schema Design](#complete-schema-design)
4. [Query Patterns](#query-patterns)
5. [Implementation Details](#implementation-details)
6. [Migration Strategy](#migration-strategy)
7. [Performance Optimizations](#performance-optimizations)
8. [API Reference](#api-reference)

---

## Architecture Overview

### Design Philosophy
The DuckDB backend replaces sled's key-value storage with a relational, columnar database optimized for both:
- **Real-time LSP caching**: Fast lookups for language server operations
- **Graph analytics**: Complex queries over code relationships and dependencies

### Key Innovation: Git-Aware Current State
Instead of traditional versioning with branches/snapshots, we use a hybrid approach:
- **Git commit hash**: For committed files at a specific point in time
- **Indexed timestamp**: For modified files (we take the most recent version)
- **Dynamic file lists**: Query-time injection of which files are modified

This allows instant branch switching (no re-indexing) while maintaining live updates for edited files.

### System Flow
```
File Change Detection → Indexer → DuckDB Storage → Query Engine
                            ↓                           ↑
                     Git Metadata                Dynamic File Lists
```

---

## Core Concepts

### 1. Current State Definition
The "current state" of the codebase is:
- **For unmodified files**: Symbols from the current git commit
- **For modified files**: Most recently indexed version (max indexed_at)
- **For non-git repos**: Always most recently indexed version

### 2. Storage Strategy
- Store multiple versions of symbols (different commits + timestamps)
- Query filters determine which version to use (commit hash or max timestamp)
- Old versions cleaned up periodically (configurable retention)

### 3. Consistency Model
- File-level consistency: Each file's symbols are consistent within itself
- Cross-file eventual consistency: References updated asynchronously
- Transaction boundaries: Per-file indexing is atomic

---

## Complete Schema Design

### Core Tables

```sql
-- 1. Workspaces (Projects)
CREATE TABLE workspaces (
    workspace_id        TEXT PRIMARY KEY,     -- UUID or hash of root path
    root_path           TEXT NOT NULL UNIQUE,
    name                TEXT NOT NULL,
    is_git_repo         BOOLEAN DEFAULT FALSE,
    current_commit      TEXT,                 -- Current git HEAD commit
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_indexed        TIMESTAMP,
    metadata            JSON                  -- Language configs, settings
);

CREATE INDEX idx_workspaces_root_path ON workspaces(root_path);

-- 2. Files and Versions
CREATE TABLE files (
    file_id             TEXT PRIMARY KEY,     -- workspace_id:relative_path
    workspace_id        TEXT NOT NULL,
    relative_path       TEXT NOT NULL,        -- Relative to workspace root
    absolute_path       TEXT NOT NULL,
    language            TEXT,
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    UNIQUE (workspace_id, relative_path)
);

CREATE INDEX idx_files_workspace ON files(workspace_id);
CREATE INDEX idx_files_language ON files(language);

-- 3. File Versions (Content tracking)
CREATE TABLE file_versions (
    version_id          TEXT PRIMARY KEY,     -- file_id:content_hash
    file_id             TEXT NOT NULL,
    content_hash        TEXT NOT NULL,        -- SHA256 of file content
    git_commit_hash     TEXT,                 -- NULL for uncommitted changes
    
    -- File metadata
    size_bytes          INTEGER,
    line_count          INTEGER,
    last_modified       TIMESTAMP,
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE,
    UNIQUE (file_id, content_hash)
);

CREATE INDEX idx_file_versions_file_time ON file_versions(file_id, indexed_at DESC);
CREATE INDEX idx_file_versions_commit ON file_versions(git_commit_hash) WHERE git_commit_hash IS NOT NULL;

-- 4. Symbols (Core entities)
CREATE TABLE symbols (
    symbol_id           TEXT PRIMARY KEY,     -- Unique identifier (hash)
    workspace_id        TEXT NOT NULL,
    file_id             TEXT NOT NULL,
    version_id          TEXT NOT NULL,
    
    -- Git-aware versioning
    git_commit_hash     TEXT,                 -- NULL for uncommitted
    
    -- Symbol information
    name                TEXT NOT NULL,
    qualified_name      TEXT,                 -- Fully qualified name
    kind                TEXT NOT NULL,        -- function, class, variable, etc.
    
    -- Location in file
    start_line          INTEGER NOT NULL,
    start_column        INTEGER NOT NULL,
    end_line            INTEGER NOT NULL,
    end_column          INTEGER NOT NULL,
    
    -- Additional metadata
    signature           TEXT,                 -- Function signature, type info
    documentation       TEXT,                 -- Doc comments
    visibility          TEXT,                 -- public, private, protected
    modifiers           TEXT[],               -- static, async, const, etc.
    
    -- Indexing metadata
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    confidence          DOUBLE DEFAULT 1.0,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_versions(version_id) ON DELETE CASCADE
);

-- Critical indexes for symbol queries
CREATE INDEX idx_symbols_workspace_current 
    ON symbols(workspace_id, git_commit_hash, indexed_at DESC);
CREATE INDEX idx_symbols_file_time 
    ON symbols(file_id, indexed_at DESC);
CREATE INDEX idx_symbols_name 
    ON symbols(workspace_id, name);
CREATE INDEX idx_symbols_kind 
    ON symbols(workspace_id, kind);
CREATE INDEX idx_symbols_qualified_name 
    ON symbols(workspace_id, qualified_name);

-- 5. Symbol Hierarchy (Containment relationships)
CREATE TABLE symbol_hierarchy (
    hierarchy_id        TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    parent_symbol_id    TEXT NOT NULL,
    child_symbol_id     TEXT NOT NULL,
    git_commit_hash     TEXT,
    relationship_type   TEXT DEFAULT 'contains',  -- contains, extends, implements
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (parent_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (child_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    UNIQUE (parent_symbol_id, child_symbol_id, git_commit_hash)
);

CREATE INDEX idx_hierarchy_parent ON symbol_hierarchy(parent_symbol_id);
CREATE INDEX idx_hierarchy_child ON symbol_hierarchy(child_symbol_id);
CREATE INDEX idx_hierarchy_workspace ON symbol_hierarchy(workspace_id, git_commit_hash, indexed_at DESC);

-- 6. Symbol References (Usage relationships)
CREATE TABLE symbol_references (
    reference_id        TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    source_symbol_id    TEXT NOT NULL,        -- Symbol making the reference
    target_symbol_id    TEXT NOT NULL,        -- Symbol being referenced
    file_id             TEXT NOT NULL,        -- File containing the reference
    version_id          TEXT NOT NULL,
    git_commit_hash     TEXT,
    
    -- Reference location
    ref_line            INTEGER NOT NULL,
    ref_column          INTEGER NOT NULL,
    ref_end_line        INTEGER,
    ref_end_column      INTEGER,
    
    -- Reference metadata
    reference_kind      TEXT DEFAULT 'use',   -- use, import, type, call
    is_definition       BOOLEAN DEFAULT FALSE,
    is_declaration      BOOLEAN DEFAULT FALSE,
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (source_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (target_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_versions(version_id) ON DELETE CASCADE
);

CREATE INDEX idx_references_source ON symbol_references(source_symbol_id);
CREATE INDEX idx_references_target ON symbol_references(target_symbol_id);
CREATE INDEX idx_references_file ON symbol_references(file_id, version_id);
CREATE INDEX idx_references_workspace ON symbol_references(workspace_id, git_commit_hash, indexed_at DESC);

-- 7. Call Graph (Function call relationships)
CREATE TABLE call_graph (
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
    call_type           TEXT DEFAULT 'direct', -- direct, virtual, callback, async
    argument_count      INTEGER,
    is_recursive        BOOLEAN DEFAULT FALSE,
    indexed_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (caller_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (callee_symbol_id) REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_versions(version_id) ON DELETE CASCADE
);

CREATE INDEX idx_call_graph_caller ON call_graph(caller_symbol_id);
CREATE INDEX idx_call_graph_callee ON call_graph(callee_symbol_id);
CREATE INDEX idx_call_graph_workspace ON call_graph(workspace_id, git_commit_hash, indexed_at DESC);

-- 8. LSP Cache (Cached language server responses)
CREATE TABLE lsp_cache (
    cache_key           TEXT PRIMARY KEY,     -- Method-specific cache key
    workspace_id        TEXT NOT NULL,
    method              TEXT NOT NULL,        -- textDocument/definition, etc.
    file_id             TEXT NOT NULL,
    version_id          TEXT NOT NULL,
    git_commit_hash     TEXT,
    
    -- Request context
    request_params      JSON NOT NULL,        -- Original LSP request parameters
    position_line       INTEGER,
    position_column     INTEGER,
    
    -- Cached response
    response_data       JSON NOT NULL,        -- LSP response data
    response_type       TEXT,                 -- locations, hover, symbols, etc.
    
    -- Cache metadata
    created_at          TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_accessed       TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    access_count        INTEGER DEFAULT 1,
    response_time_ms    INTEGER,              -- Time to generate response
    
    FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES file_versions(version_id) ON DELETE CASCADE
);

CREATE INDEX idx_lsp_cache_workspace_method ON lsp_cache(workspace_id, method);
CREATE INDEX idx_lsp_cache_file ON lsp_cache(file_id, version_id);
CREATE INDEX idx_lsp_cache_position ON lsp_cache(file_id, position_line, position_column);
CREATE INDEX idx_lsp_cache_access ON lsp_cache(workspace_id, last_accessed);

-- 9. Cache Statistics (Performance monitoring)
CREATE TABLE cache_statistics (
    stat_id             INTEGER PRIMARY KEY AUTOINCREMENT,
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

CREATE INDEX idx_cache_stats_workspace ON cache_statistics(workspace_id, date);
```

### Utility Views

```sql
-- Current symbols view (handles git + timestamp logic)
CREATE VIEW current_symbols AS
WITH latest_modified AS (
    SELECT DISTINCT ON (symbol_id, workspace_id)
        symbol_id,
        workspace_id,
        indexed_at
    FROM symbols
    WHERE git_commit_hash IS NULL  -- Modified files only
    ORDER BY symbol_id, workspace_id, indexed_at DESC
)
SELECT DISTINCT ON (s.symbol_id, s.workspace_id)
    s.*
FROM symbols s
LEFT JOIN latest_modified lm 
    ON s.symbol_id = lm.symbol_id 
    AND s.workspace_id = lm.workspace_id
WHERE 
    (s.git_commit_hash IS NULL AND s.indexed_at = lm.indexed_at)  -- Latest modified
    OR 
    (s.git_commit_hash = (
        SELECT current_commit 
        FROM workspaces 
        WHERE workspace_id = s.workspace_id
    ))  -- Current commit
ORDER BY s.symbol_id, s.workspace_id, s.indexed_at DESC;

-- Symbol with file info
CREATE VIEW symbols_with_files AS
SELECT 
    s.*,
    f.relative_path,
    f.absolute_path,
    f.language as file_language
FROM symbols s
JOIN files f ON s.file_id = f.file_id;

-- Call graph with symbol names
CREATE VIEW call_graph_named AS
SELECT 
    cg.*,
    caller.name as caller_name,
    caller.qualified_name as caller_qualified,
    callee.name as callee_name,
    callee.qualified_name as callee_qualified
FROM call_graph cg
JOIN symbols caller ON cg.caller_symbol_id = caller.symbol_id
JOIN symbols callee ON cg.callee_symbol_id = callee.symbol_id;

-- Reference graph with symbol names
CREATE VIEW references_named AS
SELECT 
    sr.*,
    source.name as source_name,
    source.qualified_name as source_qualified,
    target.name as target_name,
    target.qualified_name as target_qualified
FROM symbol_references sr
JOIN symbols source ON sr.source_symbol_id = source.symbol_id
JOIN symbols target ON sr.target_symbol_id = target.symbol_id;
```

---

## Query Patterns

### 1. Get Current Symbols with Modified Files

```sql
-- Dynamic injection of modified files
WITH modified_files (file_path) AS (
    VALUES 
        ('src/main.rs'),
        ('src/lib.rs'),
        ('src/module/helper.rs')
        -- Dynamically inject more files as needed
),
current_state AS (
    SELECT DISTINCT ON (s.symbol_id)
        s.*,
        f.relative_path,
        CASE 
            WHEN mf.file_path IS NOT NULL THEN 'modified'
            ELSE 'committed'
        END as state
    FROM symbols s
    JOIN files f ON s.file_id = f.file_id
    LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
    WHERE s.workspace_id = $1  -- Workspace ID parameter
      AND (
        -- Latest version for modified files (max timestamp)
        (mf.file_path IS NOT NULL AND s.indexed_at = (
            SELECT MAX(s2.indexed_at)
            FROM symbols s2
            WHERE s2.symbol_id = s.symbol_id
              AND s2.workspace_id = s.workspace_id
              AND s2.file_id = s.file_id
        ))
        OR
        -- Git commit version for unmodified files
        (mf.file_path IS NULL AND s.git_commit_hash = $2)  -- Current commit parameter
      )
    ORDER BY s.symbol_id, s.indexed_at DESC
)
SELECT * FROM current_state;
```

### 2. Find All References to a Symbol

```sql
WITH modified_files (file_path) AS (
    VALUES ('src/main.rs'), ('src/lib.rs')  -- Dynamic list
)
SELECT DISTINCT
    sr.reference_id,
    sr.ref_line,
    sr.ref_column,
    f.relative_path,
    sr.reference_kind
FROM symbol_references sr
JOIN files f ON sr.file_id = f.file_id
LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
WHERE sr.workspace_id = $1
  AND sr.target_symbol_id = $2  -- Symbol we're finding references to
  AND (
    -- Latest version for modified files
    (mf.file_path IS NOT NULL AND sr.indexed_at = (
        SELECT MAX(sr2.indexed_at)
        FROM symbol_references sr2
        WHERE sr2.reference_id = sr.reference_id
    ))
    OR
    -- Git commit version for unmodified files
    (mf.file_path IS NULL AND sr.git_commit_hash = $3)
  )
ORDER BY f.relative_path, sr.ref_line, sr.ref_column;
```

### 3. Get Call Hierarchy (Incoming/Outgoing Calls)

```sql
-- Incoming calls (who calls this function)
WITH modified_files (file_path) AS (
    VALUES ('src/main.rs'), ('src/lib.rs')
),
incoming_calls AS (
    SELECT DISTINCT
        cg.caller_symbol_id,
        caller.name as caller_name,
        caller.qualified_name,
        f.relative_path,
        cg.call_line,
        cg.call_column
    FROM call_graph cg
    JOIN symbols caller ON cg.caller_symbol_id = caller.symbol_id
    JOIN files f ON cg.file_id = f.file_id
    LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
    WHERE cg.workspace_id = $1
      AND cg.callee_symbol_id = $2  -- Function being called
      AND (
        -- Latest version for modified files
        (mf.file_path IS NOT NULL AND cg.indexed_at = (
            SELECT MAX(cg2.indexed_at)
            FROM call_graph cg2
            WHERE cg2.call_id = cg.call_id
        ))
        OR
        -- Git commit version for unmodified files
        (mf.file_path IS NULL AND cg.git_commit_hash = $3)
      )
)
SELECT * FROM incoming_calls;

-- Outgoing calls (what this function calls)
WITH modified_files (file_path) AS (
    VALUES ('src/main.rs'), ('src/lib.rs')
),
outgoing_calls AS (
    SELECT DISTINCT
        cg.callee_symbol_id,
        callee.name as callee_name,
        callee.qualified_name,
        f.relative_path,
        cg.call_line,
        cg.call_column
    FROM call_graph cg
    JOIN symbols callee ON cg.callee_symbol_id = callee.symbol_id
    JOIN files f ON cg.file_id = f.file_id
    LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
    WHERE cg.workspace_id = $1
      AND cg.caller_symbol_id = $2  -- Function making calls
      AND (
        -- Latest version for modified files
        (mf.file_path IS NOT NULL AND cg.indexed_at = (
            SELECT MAX(cg2.indexed_at)
            FROM call_graph cg2
            WHERE cg2.call_id = cg.call_id
        ))
        OR
        -- Git commit version for unmodified files
        (mf.file_path IS NULL AND cg.git_commit_hash = $3)
      )
)
SELECT * FROM outgoing_calls;
```

### 4. LSP Cache Lookup

```sql
WITH modified_files (file_path) AS (
    VALUES ('src/main.rs')
)
SELECT 
    lc.response_data,
    lc.response_type
FROM lsp_cache lc
JOIN files f ON lc.file_id = f.file_id
LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
WHERE lc.workspace_id = $1
  AND lc.method = $2  -- e.g., 'textDocument/definition'
  AND lc.file_id = $3
  AND lc.position_line = $4
  AND lc.position_column = $5
  AND (
    -- Latest version for modified files
    (mf.file_path IS NOT NULL AND lc.created_at = (
        SELECT MAX(lc2.created_at)
        FROM lsp_cache lc2
        WHERE lc2.cache_key = lc.cache_key
    ))
    OR
    -- Git commit version for unmodified files
    (mf.file_path IS NULL AND lc.git_commit_hash = $6)
  )
ORDER BY lc.created_at DESC
LIMIT 1;
```

### 5. Complex Graph Queries

```sql
-- Find all paths from symbol A to symbol B (call chains)
WITH RECURSIVE call_paths AS (
    -- Base case: direct calls from A
    SELECT 
        caller_symbol_id,
        callee_symbol_id,
        1 as depth,
        ARRAY[caller_symbol_id, callee_symbol_id] as path
    FROM call_graph
    WHERE workspace_id = $1
      AND caller_symbol_id = $2  -- Starting symbol
      AND (is_latest = TRUE OR git_commit_hash = $3)
    
    UNION ALL
    
    -- Recursive case: extend the path
    SELECT 
        cp.caller_symbol_id,
        cg.callee_symbol_id,
        cp.depth + 1,
        cp.path || cg.callee_symbol_id
    FROM call_paths cp
    JOIN call_graph cg ON cp.callee_symbol_id = cg.caller_symbol_id
    WHERE cg.workspace_id = $1
      AND cp.depth < 10  -- Limit depth to prevent infinite recursion
      AND NOT cg.callee_symbol_id = ANY(cp.path)  -- Prevent cycles
      AND (cg.is_latest = TRUE OR cg.git_commit_hash = $3)
)
SELECT * FROM call_paths
WHERE callee_symbol_id = $4  -- Target symbol
ORDER BY depth, path;

-- Find all symbols affected by changing a given symbol
WITH RECURSIVE affected_symbols AS (
    -- Direct references to the changed symbol
    SELECT DISTINCT source_symbol_id as symbol_id, 1 as depth
    FROM symbol_references
    WHERE workspace_id = $1
      AND target_symbol_id = $2
      AND (is_latest = TRUE OR git_commit_hash = $3)
    
    UNION
    
    -- Direct callers of the changed symbol
    SELECT DISTINCT caller_symbol_id as symbol_id, 1 as depth
    FROM call_graph
    WHERE workspace_id = $1
      AND callee_symbol_id = $2
      AND (is_latest = TRUE OR git_commit_hash = $3)
    
    UNION
    
    -- Recursively find symbols that depend on affected symbols
    SELECT DISTINCT sr.source_symbol_id, a.depth + 1
    FROM affected_symbols a
    JOIN symbol_references sr ON sr.target_symbol_id = a.symbol_id
    WHERE sr.workspace_id = $1
      AND a.depth < 5
      AND (sr.is_latest = TRUE OR sr.git_commit_hash = $3)
)
SELECT 
    a.symbol_id,
    a.depth,
    s.name,
    s.qualified_name,
    s.kind
FROM affected_symbols a
JOIN symbols s ON a.symbol_id = s.symbol_id
ORDER BY a.depth, s.name;
```

---

## Implementation Details

### DuckDB Backend Structure

```rust
// lsp-daemon/src/database/duckdb_backend.rs

use anyhow::{Context, Result};
use async_trait::async_trait;
use duckdb::{Connection, params};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct DuckDBBackend {
    /// Connection pool for concurrent access
    connections: Arc<Mutex<Vec<Connection>>>,
    
    /// Configuration
    config: DuckDBConfig,
    
    /// Current workspace state
    workspace_id: String,
    current_commit: Option<String>,
    modified_files: Arc<Mutex<Vec<String>>>,
}

pub struct DuckDBConfig {
    /// Database file path (or :memory: for in-memory)
    pub path: String,
    
    /// Maximum number of connections in pool
    pub max_connections: usize,
    
    /// Memory limit for DuckDB
    pub memory_limit_gb: f64,
    
    /// Number of threads for parallel execution
    pub threads: usize,
    
    /// Enable/disable query result caching
    pub enable_query_cache: bool,
}

impl DuckDBBackend {
    pub async fn new(config: DuckDBConfig) -> Result<Self> {
        // Initialize connection pool
        let mut connections = Vec::new();
        for _ in 0..config.max_connections {
            let conn = Connection::open(&config.path)?;
            
            // Configure DuckDB settings
            conn.execute(&format!(
                "SET memory_limit = '{}GB'", 
                config.memory_limit_gb
            ), [])?;
            conn.execute(&format!(
                "SET threads = {}", 
                config.threads
            ), [])?;
            
            connections.push(conn);
        }
        
        Ok(Self {
            connections: Arc::new(Mutex::new(connections)),
            config,
            workspace_id: String::new(),
            current_commit: None,
            modified_files: Arc::new(Mutex::new(Vec::new())),
        })
    }
    
    /// Initialize schema (create all tables)
    pub async fn initialize_schema(&self) -> Result<()> {
        let conn = self.get_connection().await?;
        
        // Execute schema creation SQL
        conn.execute_batch(include_str!("schema.sql"))?;
        
        Ok(())
    }
    
    /// Set current workspace context
    pub async fn set_workspace_context(
        &mut self,
        workspace_id: String,
        commit: Option<String>,
        modified_files: Vec<String>,
    ) -> Result<()> {
        self.workspace_id = workspace_id;
        self.current_commit = commit;
        *self.modified_files.lock().await = modified_files;
        Ok(())
    }
    
    /// Get current symbols with dynamic file list
    pub async fn get_current_symbols(&self) -> Result<Vec<Symbol>> {
        let conn = self.get_connection().await?;
        let modified = self.modified_files.lock().await;
        
        // Build dynamic VALUES clause for modified files
        let values_clause = if modified.is_empty() {
            String::from("SELECT NULL WHERE FALSE")  // Empty CTE
        } else {
            let values = modified
                .iter()
                .map(|f| format!("('{}')", f))
                .collect::<Vec<_>>()
                .join(", ");
            format!("VALUES {}", values)
        };
        
        let query = format!(
            r#"
            WITH modified_files (file_path) AS (
                {}
            ),
            current_state AS (
                SELECT DISTINCT ON (s.symbol_id)
                    s.*,
                    f.relative_path
                FROM symbols s
                JOIN files f ON s.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE s.workspace_id = ?
                  AND (
                    (mf.file_path IS NOT NULL AND s.is_latest = TRUE)
                    OR
                    (mf.file_path IS NULL AND s.git_commit_hash = ?)
                  )
                ORDER BY s.symbol_id, s.is_latest DESC, s.indexed_at DESC
            )
            SELECT * FROM current_state
            "#,
            values_clause
        );
        
        let mut stmt = conn.prepare(&query)?;
        let symbols = stmt.query_map(
            params![&self.workspace_id, &self.current_commit],
            |row| {
                Ok(Symbol {
                    symbol_id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    // ... map other fields
                })
            }
        )?;
        
        Ok(symbols.collect::<Result<Vec<_>, _>>()?)
    }
    
    /// Insert or update symbol
    pub async fn upsert_symbol(&self, symbol: &Symbol) -> Result<()> {
        let conn = self.get_connection().await?;
        
        // Mark previous versions as not latest
        conn.execute(
            "UPDATE symbols SET is_latest = FALSE 
             WHERE workspace_id = ? AND symbol_id = ? AND is_latest = TRUE",
            params![&self.workspace_id, &symbol.symbol_id],
        )?;
        
        // Insert new version
        conn.execute(
            r#"
            INSERT INTO symbols (
                symbol_id, workspace_id, file_id, version_id,
                git_commit_hash, is_latest, name, qualified_name,
                kind, start_line, start_column, end_line, end_column,
                signature, documentation, visibility, modifiers
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                &symbol.symbol_id,
                &self.workspace_id,
                &symbol.file_id,
                &symbol.version_id,
                &symbol.git_commit_hash,
                &symbol.is_latest,
                &symbol.name,
                &symbol.qualified_name,
                &symbol.kind,
                &symbol.start_line,
                &symbol.start_column,
                &symbol.end_line,
                &symbol.end_column,
                &symbol.signature,
                &symbol.documentation,
                &symbol.visibility,
                &symbol.modifiers.join(","),
            ],
        )?;
        
        Ok(())
    }
    
    /// Query call hierarchy
    pub async fn get_call_hierarchy(
        &self,
        symbol_id: &str,
        direction: CallDirection,
    ) -> Result<Vec<CallInfo>> {
        let conn = self.get_connection().await?;
        let modified = self.modified_files.lock().await;
        
        let values_clause = build_values_clause(&modified);
        
        let query = match direction {
            CallDirection::Incoming => {
                format!(
                    r#"
                    WITH modified_files (file_path) AS ({}),
                    incoming AS (
                        SELECT DISTINCT
                            cg.caller_symbol_id,
                            s.name,
                            s.qualified_name,
                            f.relative_path,
                            cg.call_line,
                            cg.call_column
                        FROM call_graph cg
                        JOIN symbols s ON cg.caller_symbol_id = s.symbol_id
                        JOIN files f ON cg.file_id = f.file_id
                        LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                        WHERE cg.workspace_id = ?
                          AND cg.callee_symbol_id = ?
                          AND (
                            (mf.file_path IS NOT NULL AND cg.is_latest = TRUE)
                            OR
                            (mf.file_path IS NULL AND cg.git_commit_hash = ?)
                          )
                    )
                    SELECT * FROM incoming
                    "#,
                    values_clause
                )
            }
            CallDirection::Outgoing => {
                // Similar query for outgoing calls
                format!(/* ... */)
            }
        };
        
        // Execute and map results
        // ...
        
        Ok(vec![])
    }
    
    /// Get LSP cache entry
    pub async fn get_lsp_cache(
        &self,
        method: &str,
        file_id: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<serde_json::Value>> {
        let conn = self.get_connection().await?;
        let modified = self.modified_files.lock().await;
        
        // Check if this file is modified
        let is_modified = modified.iter().any(|f| {
            // Check if file_id matches modified file
            file_id.ends_with(f)
        });
        
        let query = r#"
            SELECT response_data
            FROM lsp_cache
            WHERE workspace_id = ?
              AND method = ?
              AND file_id = ?
              AND position_line = ?
              AND position_column = ?
              AND (
                (? = TRUE AND is_latest = TRUE)
                OR
                (? = FALSE AND git_commit_hash = ?)
              )
            ORDER BY created_at DESC
            LIMIT 1
        "#;
        
        let mut stmt = conn.prepare(query)?;
        let mut rows = stmt.query_map(
            params![
                &self.workspace_id,
                method,
                file_id,
                line,
                column,
                is_modified,
                is_modified,
                &self.current_commit,
            ],
            |row| {
                let json_str: String = row.get(0)?;
                Ok(serde_json::from_str(&json_str).unwrap())
            },
        )?;
        
        Ok(rows.next().transpose()?)
    }
    
    /// Insert LSP cache entry
    pub async fn insert_lsp_cache(
        &self,
        entry: &LspCacheEntry,
    ) -> Result<()> {
        let conn = self.get_connection().await?;
        
        conn.execute(
            r#"
            INSERT OR REPLACE INTO lsp_cache (
                cache_key, workspace_id, method, file_id, version_id,
                git_commit_hash, is_latest, request_params,
                position_line, position_column, response_data,
                response_type, response_time_ms
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                &entry.cache_key,
                &self.workspace_id,
                &entry.method,
                &entry.file_id,
                &entry.version_id,
                &entry.git_commit_hash,
                &entry.is_latest,
                &entry.request_params.to_string(),
                &entry.position_line,
                &entry.position_column,
                &entry.response_data.to_string(),
                &entry.response_type,
                &entry.response_time_ms,
            ],
        )?;
        
        // Update access count and timestamp
        conn.execute(
            r#"
            UPDATE lsp_cache 
            SET last_accessed = CURRENT_TIMESTAMP,
                access_count = access_count + 1
            WHERE cache_key = ?
            "#,
            params![&entry.cache_key],
        )?;
        
        Ok(())
    }
    
    /// Clean up old versions
    pub async fn cleanup_old_versions(&self, keep_days: i64) -> Result<usize> {
        let conn = self.get_connection().await?;
        
        // Delete old symbols that aren't latest and older than keep_days
        let deleted = conn.execute(
            r#"
            DELETE FROM symbols
            WHERE is_latest = FALSE
              AND indexed_at < datetime('now', ? || ' days')
              AND workspace_id = ?
            "#,
            params![-keep_days, &self.workspace_id],
        )?;
        
        Ok(deleted)
    }
    
    // Helper function to get connection from pool
    async fn get_connection(&self) -> Result<Connection> {
        let mut pool = self.connections.lock().await;
        pool.pop()
            .ok_or_else(|| anyhow::anyhow!("No available connections"))
    }
}

// Helper function to build VALUES clause for modified files
fn build_values_clause(modified_files: &[String]) -> String {
    if modified_files.is_empty() {
        String::from("SELECT NULL WHERE FALSE")
    } else {
        let values = modified_files
            .iter()
            .map(|f| format!("('{}')", f))
            .collect::<Vec<_>>()
            .join(", ");
        format!("VALUES {}", values)
    }
}
```

### DatabaseBackend Trait Implementation

```rust
#[async_trait]
impl DatabaseBackend for DuckDBBackend {
    type Tree = DuckDBTree;

    async fn new(config: DatabaseConfig) -> Result<Self, DatabaseError> {
        let duckdb_config = DuckDBConfig {
            path: config.path
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ":memory:".to_string()),
            max_connections: 8,
            memory_limit_gb: 2.0,
            threads: num_cpus::get(),
            enable_query_cache: true,
        };
        
        let backend = Self::new(duckdb_config)
            .await
            .map_err(|e| DatabaseError::Configuration {
                message: e.to_string(),
            })?;
        
        backend.initialize_schema()
            .await
            .map_err(|e| DatabaseError::Configuration {
                message: e.to_string(),
            })?;
        
        Ok(backend)
    }

    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError> {
        // Implement key-value semantics using a dedicated KV table
        let key_str = String::from_utf8_lossy(key);
        let conn = self.get_connection().await
            .map_err(|e| DatabaseError::OperationFailed {
                message: e.to_string(),
            })?;
        
        let mut stmt = conn.prepare(
            "SELECT value FROM kv_store WHERE key = ? AND workspace_id = ?"
        ).map_err(|e| DatabaseError::OperationFailed {
            message: e.to_string(),
        })?;
        
        let mut rows = stmt.query_map(
            params![&key_str, &self.workspace_id],
            |row| row.get::<_, Vec<u8>>(0),
        ).map_err(|e| DatabaseError::OperationFailed {
            message: e.to_string(),
        })?;
        
        Ok(rows.next().transpose().map_err(|e| {
            DatabaseError::OperationFailed {
                message: e.to_string(),
            }
        })?)
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let conn = self.get_connection().await
            .map_err(|e| DatabaseError::OperationFailed {
                message: e.to_string(),
            })?;
        
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value, workspace_id) VALUES (?, ?, ?)",
            params![&key_str, value, &self.workspace_id],
        ).map_err(|e| DatabaseError::OperationFailed {
            message: e.to_string(),
        })?;
        
        Ok(())
    }

    // ... implement other trait methods
}
```

---

## Migration Strategy

### Phase 1: Parallel Implementation
1. Implement DuckDB backend alongside sled
2. Add feature flag to switch between backends
3. Implement data migration utilities

### Phase 2: Data Migration
```rust
pub async fn migrate_from_sled(
    sled_db: &SledBackend,
    duckdb: &mut DuckDBBackend,
) -> Result<MigrationStats> {
    let mut stats = MigrationStats::default();
    
    // 1. Migrate workspaces
    for workspace in sled_db.get_workspaces().await? {
        duckdb.insert_workspace(&workspace).await?;
        stats.workspaces += 1;
    }
    
    // 2. Migrate symbols with versioning
    for symbol in sled_db.get_all_symbols().await? {
        let migrated_symbol = Symbol {
            symbol_id: generate_symbol_id(&symbol),
            workspace_id: symbol.workspace_id,
            git_commit_hash: None,  // Will be populated from git
            is_latest: true,        // All sled symbols become latest
            // ... map other fields
        };
        duckdb.upsert_symbol(&migrated_symbol).await?;
        stats.symbols += 1;
    }
    
    // 3. Migrate LSP cache
    for cache_entry in sled_db.get_all_cache_entries().await? {
        duckdb.insert_lsp_cache(&cache_entry).await?;
        stats.cache_entries += 1;
    }
    
    // 4. Rebuild relationships (references, calls, hierarchy)
    rebuild_relationships(duckdb).await?;
    
    Ok(stats)
}
```

### Phase 3: Validation & Cutover
1. Run both backends in parallel for validation
2. Compare query results
3. Performance benchmarking
4. Gradual cutover with fallback option

---

## Performance Optimizations

### 1. Connection Pooling
- Maintain pool of 8-16 connections
- Round-robin or least-recently-used selection
- Connection warming on startup

### 2. Query Optimization
- Prepared statements for frequent queries
- Query result caching for read-heavy operations
- Batch inserts for indexing operations

### 3. Index Strategy
- Cover indexes for hot queries
- Partial indexes for is_latest = TRUE
- Periodic ANALYZE for statistics updates

### 4. Memory Management
```sql
-- Configure DuckDB memory settings
PRAGMA memory_limit='4GB';
PRAGMA threads=8;
PRAGMA enable_object_cache=true;

-- Use temporary tables for large operations
CREATE TEMP TABLE batch_symbols AS SELECT * FROM symbols WHERE FALSE;
-- Insert batch data
INSERT INTO batch_symbols VALUES ...;
-- Merge into main table
INSERT INTO symbols SELECT * FROM batch_symbols;
```

### 5. Maintenance Operations
```sql
-- Vacuum to reclaim space
VACUUM;

-- Analyze tables for query optimization
ANALYZE symbols;
ANALYZE call_graph;
ANALYZE symbol_references;

-- Checkpoint to persist changes
CHECKPOINT;
```

---

## API Reference

### Core Types

```rust
#[derive(Debug, Clone)]
pub struct Symbol {
    pub symbol_id: String,
    pub workspace_id: String,
    pub file_id: String,
    pub version_id: String,
    pub git_commit_hash: Option<String>,
    pub is_latest: bool,
    pub name: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub signature: Option<String>,
    pub documentation: Option<String>,
    pub visibility: Visibility,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum SymbolKind {
    Function,
    Class,
    Interface,
    Struct,
    Enum,
    Variable,
    Constant,
    Property,
    Method,
    Constructor,
    Parameter,
    TypeParameter,
    Module,
    Namespace,
}

#[derive(Debug, Clone)]
pub struct CallInfo {
    pub symbol_id: String,
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub call_type: CallType,
}

#[derive(Debug, Clone)]
pub enum CallType {
    Direct,
    Virtual,
    Callback,
    Async,
    Recursive,
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub source_symbol_id: String,
    pub target_symbol_id: String,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub reference_kind: ReferenceKind,
}

#[derive(Debug, Clone)]
pub enum ReferenceKind {
    Use,
    Import,
    Type,
    Call,
    Read,
    Write,
}

#[derive(Debug, Clone)]
pub struct LspCacheEntry {
    pub cache_key: String,
    pub workspace_id: String,
    pub method: String,
    pub file_id: String,
    pub version_id: String,
    pub git_commit_hash: Option<String>,
    pub is_latest: bool,
    pub request_params: serde_json::Value,
    pub position_line: u32,
    pub position_column: u32,
    pub response_data: serde_json::Value,
    pub response_type: String,
    pub response_time_ms: u32,
}
```

### Main API Methods

```rust
impl DuckDBBackend {
    // Workspace management
    pub async fn create_workspace(&self, workspace: &Workspace) -> Result<()>;
    pub async fn update_workspace_commit(&self, workspace_id: &str, commit: &str) -> Result<()>;
    pub async fn set_modified_files(&self, files: Vec<String>) -> Result<()>;
    
    // Symbol operations
    pub async fn get_current_symbols(&self) -> Result<Vec<Symbol>>;
    pub async fn get_symbol_by_id(&self, symbol_id: &str) -> Result<Option<Symbol>>;
    pub async fn upsert_symbol(&self, symbol: &Symbol) -> Result<()>;
    pub async fn delete_symbol(&self, symbol_id: &str) -> Result<()>;
    
    // Reference operations
    pub async fn add_reference(&self, reference: &Reference) -> Result<()>;
    pub async fn get_references_to(&self, symbol_id: &str) -> Result<Vec<Reference>>;
    pub async fn get_references_from(&self, symbol_id: &str) -> Result<Vec<Reference>>;
    
    // Call graph operations
    pub async fn add_call(&self, call: &CallInfo) -> Result<()>;
    pub async fn get_incoming_calls(&self, symbol_id: &str) -> Result<Vec<CallInfo>>;
    pub async fn get_outgoing_calls(&self, symbol_id: &str) -> Result<Vec<CallInfo>>;
    
    // LSP cache operations
    pub async fn get_lsp_cache(&self, method: &str, file_id: &str, line: u32, column: u32) -> Result<Option<serde_json::Value>>;
    pub async fn insert_lsp_cache(&self, entry: &LspCacheEntry) -> Result<()>;
    pub async fn invalidate_lsp_cache(&self, file_id: &str) -> Result<()>;
    
    // Graph queries
    pub async fn find_call_paths(&self, from: &str, to: &str, max_depth: u32) -> Result<Vec<Vec<String>>>;
    pub async fn find_affected_symbols(&self, symbol_id: &str, max_depth: u32) -> Result<Vec<Symbol>>;
    pub async fn get_symbol_dependencies(&self, symbol_id: &str) -> Result<Vec<Symbol>>;
    
    // Maintenance
    pub async fn vacuum(&self) -> Result<()>;
    pub async fn analyze(&self) -> Result<()>;
    pub async fn cleanup_old_versions(&self, keep_days: i64) -> Result<usize>;
    pub async fn get_statistics(&self) -> Result<DatabaseStats>;
}
```

---

## Configuration Examples

### Development Configuration
```toml
[database]
backend = "duckdb"
path = ":memory:"  # In-memory for development

[database.duckdb]
max_connections = 4
memory_limit_gb = 1.0
threads = 4
enable_query_cache = true

[database.retention]
keep_versions_days = 7
cleanup_interval_hours = 24
```

### Production Configuration
```toml
[database]
backend = "duckdb"
path = "/var/lib/probe/probe.duckdb"

[database.duckdb]
max_connections = 16
memory_limit_gb = 8.0
threads = 16
enable_query_cache = true

[database.retention]
keep_versions_days = 30
cleanup_interval_hours = 6

[database.backup]
enabled = true
path = "/var/backups/probe"
interval_hours = 24
keep_backups = 7
```

---

## Monitoring & Observability

### Key Metrics to Track
1. **Query Performance**
   - Average query time by operation type
   - P95/P99 latencies
   - Slow query log

2. **Cache Effectiveness**
   - Hit rate by LSP method
   - Cache size and growth rate
   - Invalidation frequency

3. **Storage Metrics**
   - Database size on disk
   - Number of symbols/references/calls
   - Version retention statistics

4. **Indexing Performance**
   - Files indexed per second
   - Symbol extraction rate
   - Relationship building time

### Monitoring Queries

```sql
-- Cache hit rate by method
SELECT 
    method,
    SUM(cache_hits) as hits,
    SUM(cache_misses) as misses,
    ROUND(SUM(cache_hits) * 100.0 / (SUM(cache_hits) + SUM(cache_misses)), 2) as hit_rate
FROM cache_statistics
WHERE date >= CURRENT_DATE - INTERVAL 7 DAY
GROUP BY method
ORDER BY hit_rate DESC;

-- Database growth over time
SELECT 
    DATE(indexed_at) as date,
    COUNT(*) as symbols_added,
    SUM(COUNT(*)) OVER (ORDER BY DATE(indexed_at)) as total_symbols
FROM symbols
GROUP BY DATE(indexed_at)
ORDER BY date DESC
LIMIT 30;

-- Most referenced symbols (hotspots)
SELECT 
    s.name,
    s.qualified_name,
    s.kind,
    COUNT(DISTINCT sr.source_symbol_id) as reference_count
FROM symbols s
JOIN symbol_references sr ON s.symbol_id = sr.target_symbol_id
WHERE s.workspace_id = ?
  AND (s.is_latest = TRUE OR s.git_commit_hash = ?)
GROUP BY s.symbol_id, s.name, s.qualified_name, s.kind
ORDER BY reference_count DESC
LIMIT 20;
```

---

## Troubleshooting Guide

### Common Issues

1. **Slow Queries**
   - Run `EXPLAIN ANALYZE` on slow queries
   - Check if statistics are up-to-date (`ANALYZE` tables)
   - Verify indexes are being used
   - Consider adding covering indexes

2. **High Memory Usage**
   - Reduce `memory_limit_gb` setting
   - Decrease connection pool size
   - Enable query result limits
   - Clear old versions more aggressively

3. **Disk Space Issues**
   - Run `VACUUM` to reclaim space
   - Reduce `keep_versions_days`
   - Archive old data to separate database
   - Compress backup files

4. **Inconsistent Results**
   - Verify git commit hash is correct
   - Check modified files list is accurate
   - Ensure indexes are complete
   - Look for transaction conflicts

### Debug Queries

```sql
-- Check current state configuration
SELECT 
    w.workspace_id,
    w.current_commit,
    COUNT(DISTINCT f.file_id) as total_files,
    SUM(CASE WHEN fv.is_latest THEN 1 ELSE 0 END) as modified_files
FROM workspaces w
JOIN files f ON w.workspace_id = f.workspace_id
JOIN file_versions fv ON f.file_id = fv.file_id
GROUP BY w.workspace_id, w.current_commit;

-- Find duplicate symbols
SELECT 
    name,
    qualified_name,
    COUNT(*) as versions,
    SUM(CASE WHEN is_latest THEN 1 ELSE 0 END) as latest_versions
FROM symbols
WHERE workspace_id = ?
GROUP BY name, qualified_name
HAVING COUNT(*) > 1
ORDER BY versions DESC;

-- Verify index usage
EXPLAIN ANALYZE
SELECT * FROM symbols
WHERE workspace_id = ?
  AND (is_latest = TRUE OR git_commit_hash = ?);
```

---

## Future Enhancements

### Near-term (v2.0)
1. **Incremental Indexing Optimization**
   - Track file dependencies for smart re-indexing
   - Parallel indexing with work stealing
   - Incremental cache warming

2. **Advanced Graph Queries**
   - Cycle detection in call graphs
   - Dead code analysis
   - Module dependency visualization

3. **Performance Improvements**
   - Materialized views for common queries
   - Query result caching layer
   - Adaptive index recommendations

### Long-term (v3.0)
1. **Multi-workspace Support**
   - Cross-project symbol resolution
   - Monorepo optimizations
   - Federated queries

2. **Time-travel Debugging**
   - Query code state at any commit
   - Diff analysis between versions
   - Historical trend analysis

3. **AI Integration**
   - Embedding storage for semantic search
   - Code similarity analysis
   - Intelligent cache pre-warming

4. **Distributed Architecture**
   - Read replicas for scaling
   - Sharding by workspace
   - Cloud-native deployment

---

## Conclusion

This DuckDB backend design provides:
1. **Efficient git-aware versioning** without expensive re-indexing
2. **Powerful graph queries** for code analysis
3. **High-performance LSP caching** with minimal overhead
4. **Scalable architecture** ready for future enhancements

The key innovation is the combination of git commit tracking with an is_latest flag, allowing instant branch switches while maintaining live editing support. The schema is optimized for both current LSP operations and future graph analytics, making it a solid foundation for advanced code intelligence features.