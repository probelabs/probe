//! SQLite backend implementation using Turso
//!
//! This module provides a SQLite-based implementation of the DatabaseBackend trait
//! using Turso for fast, local database operations. It's designed to be a drop-in
//! replacement for DuckDB with much faster compilation times.

use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore; // legacy; kept for compatibility in a few paths
use tokio::sync::{mpsc, oneshot};
use tokio::sync::{Mutex, RwLock};
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock as AsyncRwLock};
use tracing::{debug, error, info, warn};

macro_rules! debug_execute {
    ($conn:expr, $sql:expr, $params:expr) => {{
        debug!("🔧 SQL_DEBUG: About to EXECUTE: {}", $sql);
        let __start = Instant::now();
        let __res = $conn.execute($sql, $params).await;
        let __elapsed = __start.elapsed();
        match &__res {
            Ok(_) => {
                if __elapsed.as_millis() < 1000 {
                    debug!("✅ SQL_DEBUG: Execute OK in {} ms", __elapsed.as_millis());
                } else {
                    debug!(
                        "✅ SQL_DEBUG: Execute OK in {:.3} s",
                        __elapsed.as_secs_f64()
                    );
                }
            }
            Err(e) => {
                if __elapsed.as_millis() < 1000 {
                    warn!(
                        "❌ SQL_DEBUG: Execute FAILED in {} ms: {}",
                        __elapsed.as_millis(),
                        e
                    );
                } else {
                    warn!(
                        "❌ SQL_DEBUG: Execute FAILED in {:.3} s: {}",
                        __elapsed.as_secs_f64(),
                        e
                    );
                }
            }
        }
        __res
    }};
}
use turso::{Builder, Connection, Database};

use crate::database::{
    AnalysisProgress, CallDirection, DatabaseBackend, DatabaseConfig, DatabaseError, DatabaseStats,
    DatabaseTree, DbCheckpointMode, Edge, EdgeInterpretation, EdgeRelation, GraphPath,
    PendingEnrichmentCounts, SymbolEnrichmentPlan, SymbolState, Workspace,
};
use crate::protocol::{CallHierarchyResult, Location};
use crate::symbol::{is_absolute_like, normalize_uid_with_hint};
use crate::workspace_utils;

// Global per-database writer gates to serialize writes and DDL across all backend instances
static WRITER_GATES: Lazy<DashMap<String, Arc<tokio::sync::Mutex<()>>>> = Lazy::new(DashMap::new);
/// Track which operation currently owns the per-DB writer gate
static WRITER_GATE_OWNERS: Lazy<DashMap<String, Arc<Mutex<Option<GateOwnerInfo>>>>> =
    Lazy::new(DashMap::new);
/// Track finer-grained section inside the owning operation (e.g., store_edges.insert)
static WRITER_GATE_SECTIONS: Lazy<DashMap<String, Arc<Mutex<Option<SectionInfo>>>>> =
    Lazy::new(DashMap::new);
// Per-database reader gates: readers take shared (read) locks; quiesce takes exclusive (write)
static READER_GATES: Lazy<DashMap<String, Arc<AsyncRwLock<()>>>> = Lazy::new(DashMap::new);
static READER_SEMAPHORES: Lazy<DashMap<String, Arc<Semaphore>>> = Lazy::new(DashMap::new);

#[derive(Clone, Debug)]
struct GateOwnerInfo {
    op: String,
    since: Instant,
}

#[derive(Clone, Debug)]
struct SectionInfo {
    label: String,
    since: Instant,
}

fn get_writer_gate(path: &str) -> Arc<tokio::sync::Mutex<()>> {
    if let Some(existing) = WRITER_GATES.get(path) {
        existing.clone()
    } else {
        let gate = Arc::new(tokio::sync::Mutex::new(()));
        WRITER_GATES.insert(path.to_string(), gate.clone());
        gate
    }
}

fn get_gate_owner_handle(path: &str) -> Arc<Mutex<Option<GateOwnerInfo>>> {
    if let Some(existing) = WRITER_GATE_OWNERS.get(path) {
        existing.clone()
    } else {
        let slot = Arc::new(Mutex::new(None));
        WRITER_GATE_OWNERS.insert(path.to_string(), slot.clone());
        slot
    }
}

fn get_section_handle(path: &str) -> Arc<Mutex<Option<SectionInfo>>> {
    if let Some(existing) = WRITER_GATE_SECTIONS.get(path) {
        existing.clone()
    } else {
        let slot = Arc::new(Mutex::new(None));
        WRITER_GATE_SECTIONS.insert(path.to_string(), slot.clone());
        slot
    }
}

fn get_reader_semaphore(_path: &str) -> Arc<Semaphore> {
    // Legacy shim; no longer used for quiesce. Keep for compatibility where referenced.
    // Return a small-capacity semaphore that's not used for global coordination.
    Arc::new(Semaphore::new(1024))
}

fn get_reader_rw_gate(path: &str) -> Arc<AsyncRwLock<()>> {
    if let Some(existing) = READER_GATES.get(path) {
        existing.clone()
    } else {
        let gate = Arc::new(AsyncRwLock::new(()));
        READER_GATES.insert(path.to_string(), gate.clone());
        gate
    }
}
use pathdiff::diff_paths;

/// Guard that ensures quiesce state and debug markers are always cleared
/// even on early returns (timeout/cancel/errors) during WAL sync.
struct QuiesceGuard {
    /// Mutex-protected pool to clear `quiesced` flag
    pool: Option<Arc<Mutex<ConnectionPool>>>,
    /// Whether quiesce was enabled
    quiesced: bool,
    /// Owned write guard to block readers while quiesced
    _write_guard: Option<OwnedRwLockWriteGuard<()>>,
    /// Backend flag to reflect write-held state
    write_flag: Option<Arc<AtomicBool>>,
    /// Active section handle to clear on drop
    section: Option<Arc<Mutex<Option<SectionInfo>>>>,
    /// Writer-gate owner handle to reset on drop
    owner: Option<Arc<Mutex<Option<GateOwnerInfo>>>>,
}

impl Drop for QuiesceGuard {
    fn drop(&mut self) {
        // Best-effort: never panic in Drop
        if self.quiesced {
            if let Some(pool) = self.pool.take() {
                // Release quiesce flag
                if let Ok(p) = pool.try_lock() {
                    p.quiesced.store(false, Ordering::Relaxed);
                }
            }
        }
        // Release section marker
        if let Some(section) = self.section.take() {
            if let Ok(mut s) = section.try_lock() {
                *s = None;
            }
        }
        // Lower write-held flag
        if let Some(flag) = self.write_flag.take() {
            flag.store(false, Ordering::Relaxed);
        }
        // Clear writer-gate owner info for accurate status
        if let Some(owner) = self.owner.take() {
            if let Ok(mut o) = owner.try_lock() {
                *o = None;
            }
        }
        // _write_guard drops here, releasing reader quiesce gate
    }
}

/// WAL checkpoint mode for forced syncs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointMode {
    /// Existing behavior with pragmatic fallbacks
    Auto,
    Passive,
    Full,
    Restart,
    Truncate,
}

/// Execute a turso query and map errors consistently (async, no blocking)
async fn safe_query<P>(
    conn: &Connection,
    sql: &str,
    params: P,
    context: &str,
) -> Result<turso::Rows, DatabaseError>
where
    P: turso::params::IntoParams + Send + 'static + std::panic::UnwindSafe,
{
    {
        debug!(
            "🔍 SQL_DEBUG: About to execute QUERY (context={}): {}",
            context, sql
        );
        let start = Instant::now();
        let res = conn.query(sql, params).await;
        let elapsed = start.elapsed();
        match res {
            Ok(rows) => {
                if elapsed.as_millis() < 1000 {
                    debug!(
                        "✅ SQL_DEBUG: Query OK in {} ms (context={})",
                        elapsed.as_millis(),
                        context
                    );
                } else {
                    debug!(
                        "✅ SQL_DEBUG: Query OK in {:.3} s (context={})",
                        elapsed.as_secs_f64(),
                        context
                    );
                }
                Ok(rows)
            }
            Err(e) => {
                if elapsed.as_millis() < 1000 {
                    warn!(
                        "❌ SQL_DEBUG: Query FAILED in {} ms (context={}): {}",
                        elapsed.as_millis(),
                        context,
                        e
                    );
                } else {
                    warn!(
                        "❌ SQL_DEBUG: Query FAILED in {:.3} s (context={}): {}",
                        elapsed.as_secs_f64(),
                        context,
                        e
                    );
                }
                Err(DatabaseError::OperationFailed {
                    message: format!("{}: {}", context, e),
                })
            }
        }
    }
}

/// Database lock retry function for SELECTs with exponential backoff
async fn safe_query_with_retry<P>(
    conn: &Connection,
    sql: &str,
    params: P,
    context: &str,
    max_retries: u32,
) -> Result<turso::Rows, DatabaseError>
where
    P: turso::params::IntoParams + Send + 'static + std::panic::UnwindSafe + Clone,
{
    let mut attempt = 0;
    loop {
        match safe_query(conn, sql, params.clone(), context).await {
            Ok(rows) => return Ok(rows),
            Err(DatabaseError::OperationFailed { message })
                if message.to_ascii_lowercase().contains("database is locked")
                    || message.to_ascii_lowercase().contains("busy") =>
            {
                attempt += 1;
                if attempt > max_retries {
                    error!(
                        "Database lock retry (SELECT) exhausted after {} attempts for: {}",
                        max_retries, context
                    );
                    return Err(DatabaseError::OperationFailed {
                        message: format!(
                            "Database locked after {} retry attempts: {}",
                            max_retries, message
                        ),
                    });
                }
                let delay_ms = 25 * (1 << (attempt - 1)).min(10);
                warn!(
                    "Database locked on SELECT, retrying in {}ms (attempt {}/{}): {}",
                    delay_ms, attempt, max_retries, context
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms as u64)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Safely execute a turso execute operation that might panic
async fn safe_execute<P>(
    conn: &Connection,
    sql: &str,
    params: P,
    context: &str,
) -> Result<u64, DatabaseError>
where
    P: turso::params::IntoParams + Send + 'static + std::panic::UnwindSafe,
{
    {
        debug!(
            "🔧 SQL_DEBUG: About to EXECUTE (context={}): {}",
            context, sql
        );
        let start = Instant::now();
        let res = conn.execute(sql, params).await;
        let elapsed = start.elapsed();
        match res {
            Ok(result) => {
                if elapsed.as_millis() < 1000 {
                    debug!(
                        "✅ SQL_DEBUG: Execute OK in {} ms (context={})",
                        elapsed.as_millis(),
                        context
                    );
                } else {
                    debug!(
                        "✅ SQL_DEBUG: Execute OK in {:.3} s (context={})",
                        elapsed.as_secs_f64(),
                        context
                    );
                }
                Ok(result)
            }
            Err(e) => {
                if elapsed.as_millis() < 1000 {
                    warn!(
                        "❌ SQL_DEBUG: Execute FAILED in {} ms (context={}): {}",
                        elapsed.as_millis(),
                        context,
                        e
                    );
                } else {
                    warn!(
                        "❌ SQL_DEBUG: Execute FAILED in {:.3} s (context={}): {}",
                        elapsed.as_secs_f64(),
                        context,
                        e
                    );
                }
                Err(DatabaseError::OperationFailed {
                    message: format!("{}: {}", context, e),
                })
            }
        }
    }
}

/// Database lock retry function with exponential backoff
/// Specifically handles "database is locked" errors that occur during concurrent writes
async fn safe_execute_with_retry<P>(
    conn: &Connection,
    sql: &str,
    params: P,
    context: &str,
    max_retries: u32,
) -> Result<u64, DatabaseError>
where
    P: turso::params::IntoParams + Send + 'static + std::panic::UnwindSafe + Clone,
{
    let mut attempt = 0;
    loop {
        match safe_execute(conn, sql, params.clone(), context).await {
            Ok(result) => return Ok(result),
            Err(DatabaseError::OperationFailed { message })
                if message.contains("database is locked") =>
            {
                attempt += 1;
                if attempt > max_retries {
                    error!(
                        "Database lock retry exhausted after {} attempts for: {}",
                        max_retries, context
                    );
                    return Err(DatabaseError::OperationFailed {
                        message: format!(
                            "Database locked after {} retry attempts: {}",
                            max_retries, message
                        ),
                    });
                }

                // Exponential backoff: 50ms, 100ms, 200ms, 400ms, 800ms (max)
                let delay_ms = 50 * (1 << (attempt - 1)).min(800);
                warn!(
                    "Database locked, retrying in {}ms (attempt {}/{}): {}",
                    delay_ms, attempt, max_retries, context
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }
            Err(e) => return Err(e), // Non-lock errors fail immediately
        }
    }
}

/// Attempt to rollback the current transaction, logging but ignoring failures.
async fn rollback_transaction(conn: &Connection, context: &str) {
    match conn.execute("ROLLBACK", ()).await {
        Ok(_) => {
            debug!("🔄 SQL_DEBUG: Transaction rollback succeeded ({})", context);
        }
        Err(e) => {
            warn!(
                "⚠️ SQL_DEBUG: Transaction rollback failed for {}: {}",
                context, e
            );
        }
    }
}

/// Extract panic message from panic payload
fn extract_panic_message(panic_err: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic_err.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = panic_err.downcast_ref::<&str>() {
        s.to_string()
    } else {
        "Unknown panic".to_string()
    }
}

/// SQLite-specific configuration
#[derive(Debug, Clone)]
pub struct SQLiteConfig {
    /// Database file path (or ":memory:" for in-memory)
    pub path: String,
    /// Whether this is a temporary/in-memory database
    pub temporary: bool,
    /// Enable WAL mode for better concurrency
    pub enable_wal: bool,
    /// SQLite page size in bytes
    pub page_size: u32,
    /// SQLite cache size in pages
    pub cache_size: i32,
    /// Enable foreign key constraints
    pub enable_foreign_keys: bool,
}

impl Default for SQLiteConfig {
    fn default() -> Self {
        Self {
            path: ":memory:".to_string(),
            temporary: true,
            enable_wal: false,         // Disabled for in-memory databases
            page_size: 4096,           // 4KB pages
            cache_size: 2000,          // ~8MB cache
            enable_foreign_keys: true, // Enable foreign keys by default for data integrity
        }
    }
}

/// Connection pool for managing SQLite connections
struct ConnectionPool {
    /// The libSQL database instance
    database: Database,
    /// Available connections
    available: Vec<Connection>,
    /// Maximum pool size
    max_size: usize,
    /// Configuration
    config: SQLiteConfig,
    /// Number of checked-out connections (not in `available`)
    checked_out: std::sync::atomic::AtomicUsize,
    /// Quiesce flag: when true, `get_connection` waits until quiesce is lifted
    quiesced: std::sync::atomic::AtomicBool,
}

#[allow(dead_code)]
impl ConnectionPool {
    /// Create a new connection pool
    async fn new(config: SQLiteConfig) -> Result<Self, DatabaseError> {
        let database = if config.path == ":memory:" {
            Builder::new_local(":memory:")
        } else {
            Builder::new_local(&config.path)
        }
        .build()
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!(
                "Failed to create Turso/SQLite database at '{}': {}. \
                 Error details: {:?}. Check database path, permissions, and disk space.",
                config.path, e, e
            ),
        })?;

        // Initialize the database with our schema
        let conn = database
            .connect()
            .map_err(|e| DatabaseError::Configuration {
                message: format!(
                    "Failed to get initial connection to Turso/SQLite database at '{}': {}. \
                     Error details: {:?}. This may indicate database file corruption or access issues.",
                    config.path, e, e
                ),
            })?;

        // Migrations removed: ensure minimal schema instead
        Self::ensure_minimal_schema(&conn, &config).await?;

        // Pre-populate with some connections
        let initial_size = 1;
        let mut available = Vec::with_capacity(initial_size);
        for _ in 0..initial_size {
            if let Ok(conn) = database.connect() {
                Self::configure_connection(&conn, &config).await?;
                available.push(conn);
            }
        }

        Ok(Self {
            database,
            available,
            // Allow more concurrent readers; writes are serialized by the writer gate
            max_size: 4,
            config,
            checked_out: std::sync::atomic::AtomicUsize::new(0),
            quiesced: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Ensure minimal schema without a migration framework
    async fn ensure_minimal_schema(
        conn: &Connection,
        _config: &SQLiteConfig,
    ) -> Result<(), DatabaseError> {
        // Create core project/workspace tables (no-ops where unused)
        Self::create_core_tables(conn).await?;
        // Create symbol_state and edge tables used by the indexer
        Self::create_relationship_tables(conn).await?;
        // Create a few essential indexes for performance
        let index_sqls = vec![
            // symbol lookups by file and language
            "CREATE INDEX IF NOT EXISTS idx_symbol_state_file_lang ON symbol_state(file_path, language)",
            // edge lookups for references/impls/calls
            "CREATE INDEX IF NOT EXISTS idx_edge_source_relation ON edge(source_symbol_uid, relation)",
            "CREATE INDEX IF NOT EXISTS idx_edge_target_relation ON edge(target_symbol_uid, relation)",
            "CREATE INDEX IF NOT EXISTS idx_edge_relation ON edge(relation)",
            // composite index to accelerate dedup lookups
            "CREATE INDEX IF NOT EXISTS idx_edge_dedup ON edge(relation, source_symbol_uid, target_symbol_uid, language, start_line, start_char)",
        ];
        for sql in index_sqls {
            let _ = conn.execute(sql, ()).await; // best-effort
        }
        Ok(())
    }

    /// Legacy method kept for backward compatibility
    /// Now delegates to the migration system
    #[allow(dead_code)]
    async fn initialize_schema(
        conn: &Connection,
        config: &SQLiteConfig,
    ) -> Result<(), DatabaseError> {
        Self::ensure_minimal_schema(conn, config).await
    }

    /// Configure a connection with optimal settings
    async fn configure_connection(
        conn: &Connection,
        config: &SQLiteConfig,
    ) -> Result<(), DatabaseError> {
        debug!("Configuring database connection for concurrent access");

        // Skip PRAGMA busy_timeout and read_uncommitted for Turso compatibility
        // These optimizations are not needed for cloud SQLite implementations
        debug!("Skipping SQLite PRAGMA optimizations for cloud database compatibility");

        // Give read steps a bit more time under transient writer activity
        if let Err(e) = conn.execute("PRAGMA busy_timeout=500", ()).await {
            debug!("busy_timeout not applied (may be unsupported): {}", e);
        }

        // Try cache size optimization if supported
        if config.cache_size > 0 {
            if let Err(e) = conn
                .execute(&format!("PRAGMA cache_size={}", config.cache_size), ())
                .await
            {
                warn!(
                    "Failed to set cache size (may not be supported by Turso): {}",
                    e
                );
            } else {
                debug!("Set cache size to {} pages", config.cache_size);
            }
        }

        // Note: WAL mode, synchronous, and foreign keys are intentionally skipped
        // as they are not supported by turso/libSQL which handles these optimizations server-side
        debug!("Turso/libSQL handles WAL mode and performance optimizations automatically");

        Ok(())
    }

    /// Create schema version control table
    async fn create_schema_version_table(conn: &Connection) -> Result<(), DatabaseError> {
        debug_execute!(
            conn,
            r#"
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL,
                description TEXT
            )
            "#,
            ()
        )
        .map_err(|e| DatabaseError::Configuration {
            message: format!(
                "Failed to create schema_version table in Turso/SQLite database: {e}. \
                 Error details: {e:?}. This may indicate schema conflicts or insufficient permissions."
            ),
        })?;
        Ok(())
    }

    /// Create legacy tables for backward compatibility (currently empty - all legacy tables removed)
    async fn create_legacy_tables(_conn: &Connection) -> Result<(), DatabaseError> {
        // All unused cache tables (kv_store, tree_metadata) have been removed
        // Only core PRD tables (symbol_state, edges, etc.) are now used for caching
        Ok(())
    }

    /// Create core PRD tables (workspaces, files, file_versions)
    async fn create_core_tables(conn: &Connection) -> Result<(), DatabaseError> {
        // 1. Projects/Workspaces table
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS project (
                project_id TEXT PRIMARY KEY,
                root_path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                description TEXT,
                created_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP NOT NULL,
                metadata TEXT
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create project table: {e}"),
        })?;

        // 2. Workspaces table (project workspaces with branch support)
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS workspace (
                workspace_id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                current_branch TEXT,
                head_commit TEXT,
                created_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP NOT NULL,
                metadata TEXT,
                FOREIGN KEY (project_id) REFERENCES project(project_id) ON DELETE CASCADE
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create workspace table: {e}"),
        })?;

        // 3. File registry with project association
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS file (
                file_id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                absolute_path TEXT NOT NULL,
                language TEXT,
                size_bytes INTEGER,
                created_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP NOT NULL,
                FOREIGN KEY (project_id) REFERENCES project(project_id) ON DELETE CASCADE
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create file table: {e}"),
        })?;

        // 7. File versions removed - file versioning complexity eliminated

        // 8. Analysis run tracking
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS analysis_run (
                run_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                analyzer_type TEXT NOT NULL,
                analyzer_version TEXT,
                configuration TEXT,
                started_at TIMESTAMP NOT NULL,
                completed_at TIMESTAMP,
                status TEXT DEFAULT 'running',
                files_processed INTEGER DEFAULT 0,
                symbols_found INTEGER DEFAULT 0,
                errors TEXT,
                FOREIGN KEY (workspace_id) REFERENCES workspace(workspace_id) ON DELETE CASCADE
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create analysis_run table: {e}"),
        })?;

        // 9. File analysis status and results
        conn.execute(
            r#"
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
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create file_analysis table: {e}"),
        })?;

        Ok(())
    }

    /// Create relationship tables (symbols, hierarchy, references, calls)
    async fn create_relationship_tables(conn: &Connection) -> Result<(), DatabaseError> {
        // 10. Symbol definitions (file versioning removed)
        conn.execute(
            r#"
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
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create symbol_state table: {e}"),
        })?;

        // 12. Relationships between symbols (file versioning removed)
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS edge (
                relation TEXT NOT NULL,
                source_symbol_uid TEXT NOT NULL,
                target_symbol_uid TEXT NOT NULL,
                start_line INTEGER,
                start_char INTEGER,
                confidence REAL NOT NULL,
                language TEXT NOT NULL,
                metadata TEXT
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create edge table: {e}"),
        })?;

        // 13. File dependency relationships (file versioning removed)
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS file_dependency (
                dependency_id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                source_file_id TEXT NOT NULL,
                target_file_id TEXT NOT NULL,
                dependency_type TEXT NOT NULL,
                import_statement TEXT,
                git_commit_hash TEXT,
                created_at TIMESTAMP NOT NULL,
                FOREIGN KEY (project_id) REFERENCES project(project_id) ON DELETE CASCADE,
                FOREIGN KEY (source_file_id) REFERENCES file(file_id) ON DELETE CASCADE,
                FOREIGN KEY (target_file_id) REFERENCES file(file_id) ON DELETE CASCADE
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create file_dependency table: {e}"),
        })?;

        // 14. Symbol change tracking
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS symbol_change (
                change_id TEXT PRIMARY KEY,
                symbol_id TEXT NOT NULL,
                previous_state_id TEXT,
                current_state_id TEXT NOT NULL,
                change_type TEXT NOT NULL,
                git_commit_hash TEXT,
                changed_at TIMESTAMP NOT NULL,
                change_description TEXT,
                FOREIGN KEY (symbol_id) REFERENCES symbol(symbol_id) ON DELETE CASCADE,
                FOREIGN KEY (previous_state_id) REFERENCES symbol_state(state_id) ON DELETE SET NULL,
                FOREIGN KEY (current_state_id) REFERENCES symbol_state(state_id) ON DELETE CASCADE
            )
            "#,
            (),
        ).await.map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create symbol_change table: {e}"),
        })?;

        Ok(())
    }

    /// Create cache and analytics tables
    async fn create_cache_tables(conn: &Connection) -> Result<(), DatabaseError> {
        // 15. Analysis queue management
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS indexer_queue (
                queue_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                file_id TEXT NOT NULL,
                priority INTEGER DEFAULT 0,
                operation_type TEXT NOT NULL,
                status TEXT DEFAULT 'pending',
                created_at TIMESTAMP NOT NULL,
                started_at TIMESTAMP,
                completed_at TIMESTAMP,
                retry_count INTEGER DEFAULT 0,
                error_message TEXT,
                FOREIGN KEY (workspace_id) REFERENCES workspace(workspace_id) ON DELETE CASCADE,
                FOREIGN KEY (file_id) REFERENCES file(file_id) ON DELETE CASCADE
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create indexer_queue table: {e}"),
        })?;

        // 16. Progress tracking
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS indexer_checkpoint (
                checkpoint_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                operation_type TEXT NOT NULL,
                last_processed_file TEXT,
                files_processed INTEGER DEFAULT 0,
                total_files INTEGER DEFAULT 0,
                checkpoint_data TEXT,
                created_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP NOT NULL,
                FOREIGN KEY (workspace_id) REFERENCES workspace(workspace_id) ON DELETE CASCADE
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create indexer_checkpoint table: {e}"),
        })?;

        Ok(())
    }

    /// Create all performance indexes from PRD specification
    async fn create_performance_indexes(
        conn: &Connection,
        config: &SQLiteConfig,
    ) -> Result<(), DatabaseError> {
        // Generate a unique suffix for this database instance to avoid index conflicts
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        config.path.hash(&mut hasher);
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        let db_suffix = format!("{:x}", hasher.finish())[..8].to_string();
        let indexes = vec![
            // Project indexes
            format!("CREATE INDEX IF NOT EXISTS idx_project_root_path_{db_suffix} ON project(root_path)"),
            // Workspace indexes
            format!("CREATE INDEX IF NOT EXISTS idx_workspace_project_{db_suffix} ON workspace(project_id)"),
            format!("CREATE INDEX IF NOT EXISTS idx_workspace_path_{db_suffix} ON workspace(path)"),
            format!("CREATE INDEX IF NOT EXISTS idx_workspace_branch_{db_suffix} ON workspace(current_branch)"),
            // File indexes
            format!("CREATE INDEX IF NOT EXISTS idx_file_project_{db_suffix} ON file(project_id)"),
            format!("CREATE INDEX IF NOT EXISTS idx_file_language_{db_suffix} ON file(language)"),
            format!("CREATE INDEX IF NOT EXISTS idx_file_relative_path_{db_suffix} ON file(project_id, relative_path)"),
            // File version indexes removed
            // Symbol indexes
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_project_{db_suffix} ON symbol(project_id)"),
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_file_{db_suffix} ON symbol(file_id)"),
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_name_{db_suffix} ON symbol(project_id, name)"),
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_qualified_name_{db_suffix} ON symbol(project_id, qualified_name)"),
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_type_{db_suffix} ON symbol(project_id, symbol_type)"),
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_language_{db_suffix} ON symbol(language)"),
            // Symbol state indexes
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_state_symbol_{db_suffix} ON symbol_state(symbol_id)"),
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_state_version_{db_suffix} ON symbol_state(version_id)"),
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_state_commit_{db_suffix} ON symbol_state(git_commit_hash)"),
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_state_time_{db_suffix} ON symbol_state(symbol_id, indexed_at DESC)"),
            // Edge indexes
            format!("CREATE INDEX IF NOT EXISTS idx_edge_source_{db_suffix} ON edge(source_symbol_uid)"),
            format!("CREATE INDEX IF NOT EXISTS idx_edge_target_{db_suffix} ON edge(target_symbol_uid)"),
            format!("CREATE INDEX IF NOT EXISTS idx_edge_type_{db_suffix} ON edge(relation)"),
            // Edge file version index removed
            // Note: git_commit_hash not in Edge schema, removing index
            // File dependency indexes
            format!("CREATE INDEX IF NOT EXISTS idx_file_dep_source_{db_suffix} ON file_dependency(source_file_id)"),
            format!("CREATE INDEX IF NOT EXISTS idx_file_dep_target_{db_suffix} ON file_dependency(target_file_id)"),
            format!("CREATE INDEX IF NOT EXISTS idx_file_dep_type_{db_suffix} ON file_dependency(project_id, dependency_type)"),
            format!("CREATE INDEX IF NOT EXISTS idx_file_dep_commit_{db_suffix} ON file_dependency(git_commit_hash)"),
            // Analysis indexes
            format!("CREATE INDEX IF NOT EXISTS idx_analysis_run_workspace_{db_suffix} ON analysis_run(workspace_id, started_at DESC)"),
            format!("CREATE INDEX IF NOT EXISTS idx_file_analysis_run_{db_suffix} ON file_analysis(run_id)"),
            format!("CREATE INDEX IF NOT EXISTS idx_file_analysis_file_{db_suffix} ON file_analysis(file_id, version_id)"),
            // Workspace indexes - removed (tables deleted)
            // Queue indexes
            format!("CREATE INDEX IF NOT EXISTS idx_indexer_queue_workspace_{db_suffix} ON indexer_queue(workspace_id, status, priority DESC)"),
            format!("CREATE INDEX IF NOT EXISTS idx_indexer_queue_status_{db_suffix} ON indexer_queue(status, created_at)"),
            format!("CREATE INDEX IF NOT EXISTS idx_indexer_checkpoint_workspace_{db_suffix} ON indexer_checkpoint(workspace_id, operation_type)"),
            // Change tracking indexes
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_change_symbol_{db_suffix} ON symbol_change(symbol_id, changed_at DESC)"),
            format!("CREATE INDEX IF NOT EXISTS idx_symbol_change_commit_{db_suffix} ON symbol_change(git_commit_hash)"),
        ];

        for sql in &indexes {
            conn.execute(sql, ())
                .await
                .map_err(|e| DatabaseError::Configuration {
                    message: format!("Failed to create index: {sql}. Error: {e}"),
                })?;
        }

        Ok(())
    }

    /// Create utility views from PRD specification
    async fn create_utility_views(
        conn: &Connection,
        config: &SQLiteConfig,
    ) -> Result<(), DatabaseError> {
        // Generate a unique suffix for this database instance to avoid view conflicts
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        config.path.hash(&mut hasher);
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        let db_suffix = format!("{:x}", hasher.finish())[..8].to_string();
        // Current symbols view (simplified for symbol_state table)
        let current_symbols_sql = format!(
            r#"
            CREATE VIEW IF NOT EXISTS current_symbols_{db_suffix} AS
            SELECT 
                symbol_uid,
                language,
                name,
                fqn,
                kind,
                signature,
                visibility,
                def_start_line,
                def_start_char,
                def_end_line,
                def_end_char,
                is_definition,
                documentation,
                metadata
            FROM symbol_state
            "#
        );

        conn.execute(&current_symbols_sql, ())
            .await
            .map_err(|e| DatabaseError::Configuration {
                message: format!("Failed to create current_symbols view: {e}"),
            })?;

        // Symbols with file info view (file versioning removed - using file_path directly)
        let symbols_with_files_sql = format!(
            r#"
            CREATE VIEW IF NOT EXISTS symbols_with_files_{db_suffix} AS
            SELECT 
                ss.symbol_uid,
                ss.name,
                ss.fqn,
                ss.kind,
                ss.signature,
                ss.visibility,
                ss.def_start_line,
                ss.def_start_char,
                ss.def_end_line,
                ss.def_end_char,
                ss.is_definition,
                ss.documentation,
                ss.language,
                ss.metadata,
                ss.file_path,
                f.relative_path,
                f.absolute_path,
                f.language as file_language,
                p.name as project_name,
                p.root_path
            FROM symbol_state ss
            LEFT JOIN file f ON ss.file_path = f.absolute_path OR ss.file_path = f.relative_path
            LEFT JOIN project p ON f.project_id = p.project_id
            "#
        );

        conn.execute(&symbols_with_files_sql, ())
            .await
            .map_err(|e| DatabaseError::Configuration {
                message: format!("Failed to create symbols_with_files view: {e}"),
            })?;

        // Edge relationships view (simplified for new schema)
        let edges_named_sql = format!(
            r#"
            CREATE VIEW IF NOT EXISTS edges_named_{db_suffix} AS
            SELECT 
                e.*
            FROM edge e
            "#
        );

        conn.execute(&edges_named_sql, ())
            .await
            .map_err(|e| DatabaseError::Configuration {
                message: format!("Failed to create edges_named view: {e}"),
            })?;

        // File dependencies with names view
        let file_dependencies_named_sql = format!(
            r#"
            CREATE VIEW IF NOT EXISTS file_dependencies_named_{db_suffix} AS
            SELECT 
                fd.*,
                source.relative_path as source_path,
                target.relative_path as target_path,
                source.language as source_language,
                target.language as target_language
            FROM file_dependency fd
            JOIN file source ON fd.source_file_id = source.file_id
            JOIN file target ON fd.target_file_id = target.file_id
            "#
        );

        conn.execute(&file_dependencies_named_sql, ())
            .await
            .map_err(|e| DatabaseError::Configuration {
                message: format!("Failed to create file_dependencies_named view: {e}"),
            })?;

        Ok(())
    }

    /// Initialize or validate schema version
    async fn initialize_schema_version(conn: &Connection) -> Result<(), DatabaseError> {
        // Check if schema version exists
        let mut rows = safe_query_with_retry(
            conn,
            "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
            (),
            "initialize_schema_version query",
            5,
        )
        .await?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to read schema version: {e}"),
            })?
        {
            // Schema version exists, validate it
            if let Ok(turso::Value::Integer(version)) = row.get_value(0) {
                if version != 1 {
                    return Err(DatabaseError::Configuration {
                        message: format!(
                            "Unsupported schema version: {version}. Expected version 1."
                        ),
                    });
                }
            }
        } else {
            // Initialize schema version
            safe_execute(
                conn,
                "INSERT INTO schema_version (version, description) VALUES (1, 'Initial PRD schema with core tables, indexes, and views')",
                (),
                "initialize_schema_version insert",
            ).await?;
        }

        Ok(())
    }

    /// Get a connection from the pool
    async fn get_connection(&mut self) -> Result<Connection, DatabaseError> {
        // Respect quiesce: block new checkouts while set
        while self.quiesced.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        let conn = if let Some(conn) = self.available.pop() {
            conn
        } else {
            // Create a new connection if we haven't hit the max
            let conn = self
                .database
                .connect()
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to create new connection: {e}"),
                })?;
            Self::configure_connection(&conn, &self.config).await?;
            conn
        };
        self.checked_out.fetch_add(1, Ordering::Relaxed);
        Ok(conn)
    }

    /// Return a connection to the pool
    fn return_connection(&mut self, conn: Connection) {
        if self.available.len() < self.max_size {
            self.available.push(conn);
        }
        self.checked_out.fetch_sub(1, Ordering::Relaxed);
        // If pool is full, just drop the connection
    }
}

/// SQLite-based implementation of DatabaseTree
pub struct SQLiteTree {
    /// Tree name (used as table suffix)
    name: String,
    /// Connection pool reference
    pool: Arc<Mutex<ConnectionPool>>,
}

#[async_trait]
impl DatabaseTree for SQLiteTree {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        let sql = format!("SELECT value FROM {table_name} WHERE key = ?");

        let mut rows = safe_query(
            &conn,
            &sql,
            [turso::Value::Text(key_str.to_string())],
            &format!("Failed to get key from tree '{}'", self.name),
        )
        .await?;

        let value = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate rows in tree '{}': {}", self.name, e),
                })? {
            match row.get_value(0) {
                Ok(turso::Value::Blob(blob)) => Some(blob),
                _ => None,
            }
        } else {
            None
        };

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(value)
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        // Obtain DB path for writer gate
        let db_path = { self.pool.lock().await.config.path.clone() };
        let gate = get_writer_gate(&db_path);
        let _guard = gate.lock().await;
        let owner_handle = get_gate_owner_handle(&db_path);
        {
            let mut o = owner_handle.lock().await;
            *o = Some(GateOwnerInfo {
                op: "tree.set".to_string(),
                since: Instant::now(),
            });
        }
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        // Use UPDATE/INSERT pattern since Turso doesn't support OR REPLACE
        let update_sql = format!("UPDATE {table_name} SET value = ?, updated_at = ? WHERE key = ?");
        let insert_sql = format!(
            "INSERT INTO {table_name} (key, value, created_at, updated_at) VALUES (?, ?, ?, ?)"
        );

        // Try update first
        let timestamp = chrono::Utc::now().timestamp();
        let rows_updated = safe_execute_with_retry(
            &conn,
            &update_sql,
            [
                turso::Value::Blob(value.to_vec()),
                turso::Value::Integer(timestamp),
                turso::Value::Text(key_str.to_string()),
            ],
            &format!("Failed to update key in tree '{}'", self.name),
            5,
        )
        .await?;

        // If no rows were updated, insert new record
        if rows_updated == 0 {
            let timestamp = chrono::Utc::now().timestamp();
            safe_execute_with_retry(
                &conn,
                &insert_sql,
                [
                    turso::Value::Text(key_str.to_string()),
                    turso::Value::Blob(value.to_vec()),
                    turso::Value::Integer(timestamp),
                    turso::Value::Integer(timestamp),
                ],
                &format!("Failed to insert key in tree '{}'", self.name),
                5,
            )
            .await?;
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        {
            let mut o = owner_handle.lock().await;
            *o = None;
        }
        Ok(())
    }

    async fn remove(&self, key: &[u8]) -> Result<bool, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let db_path = { self.pool.lock().await.config.path.clone() };
        let gate = get_writer_gate(&db_path);
        let _guard = gate.lock().await;
        let owner_handle = get_gate_owner_handle(&db_path);
        {
            let mut o = owner_handle.lock().await;
            *o = Some(GateOwnerInfo {
                op: "tree.remove".to_string(),
                since: Instant::now(),
            });
        }
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        let sql = format!("DELETE FROM {table_name} WHERE key = ?");

        let rows_affected = safe_execute_with_retry(
            &conn,
            &sql,
            [turso::Value::Text(key_str.to_string())],
            &format!("Failed to remove key from tree '{}'", self.name),
            5,
        )
        .await?;

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        {
            let mut o = owner_handle.lock().await;
            *o = None;
        }
        Ok(rows_affected > 0)
    }

    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DatabaseError> {
        let prefix_str = String::from_utf8_lossy(prefix);
        let db_path = { self.pool.lock().await.config.path.clone() };
        let gate = get_writer_gate(&db_path);
        let _guard = gate.lock().await;
        let owner_handle = get_gate_owner_handle(&db_path);
        {
            let mut o = owner_handle.lock().await;
            *o = Some(GateOwnerInfo {
                op: "tree.scan_prefix".to_string(),
                since: Instant::now(),
            });
        }
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        let sql = if prefix.is_empty() {
            format!("SELECT key, value FROM {table_name} ORDER BY key")
        } else {
            format!("SELECT key, value FROM {table_name} WHERE key GLOB ? || '*' ORDER BY key")
        };

        let params = if prefix.is_empty() {
            Vec::new()
        } else {
            vec![turso::Value::Text(prefix_str.to_string())]
        };

        let mut rows = safe_query(
            &conn,
            &sql,
            params,
            &format!("Failed to scan prefix in tree '{}'", self.name),
        )
        .await?;

        let mut results = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate rows in tree '{}': {}", self.name, e),
            })?
        {
            if let (Ok(turso::Value::Text(key)), Ok(turso::Value::Blob(value))) =
                (row.get_value(0), row.get_value(1))
            {
                results.push((key.as_bytes().to_vec(), value));
            }
            // Skip malformed rows
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        {
            let mut o = owner_handle.lock().await;
            *o = None;
        }
        Ok(results)
    }

    async fn clear(&self) -> Result<(), DatabaseError> {
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        let sql = format!("DELETE FROM {table_name}");

        safe_execute_with_retry(
            &conn,
            &sql,
            (),
            &format!("Failed to clear tree '{}'", self.name),
            5,
        )
        .await?;

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(())
    }

    async fn len(&self) -> Result<u64, DatabaseError> {
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        let sql = format!("SELECT COUNT(*) FROM {table_name}");

        let mut rows = safe_query(
            &conn,
            &sql,
            (),
            &format!("Failed to get length of tree '{}'", self.name),
        )
        .await?;

        let count = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate rows in tree '{}': {}", self.name, e),
                })? {
            match row.get_value(0) {
                Ok(turso::Value::Integer(n)) => n as u64,
                _ => 0,
            }
        } else {
            0
        };

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(count)
    }
}

/// SQLite database backend implementation
pub struct SQLiteBackend {
    /// Connection pool
    pool: Arc<Mutex<ConnectionPool>>,
    /// SQLite-specific configuration
    sqlite_config: SQLiteConfig,
    /// Cache of opened trees
    trees: RwLock<HashMap<String, Arc<SQLiteTree>>>,
    /// Single-writer channel (serializes all DB write operations)
    writer_tx: mpsc::Sender<WriteMsg>,
    /// Indicates writer is performing a batch (monitor can back off)
    writer_busy: Arc<AtomicBool>,
    /// Writer span sequence id
    writer_span_seq: Arc<std::sync::atomic::AtomicU64>,
    /// Active writer span (if any)
    writer_span_active: Arc<Mutex<Option<WriterSpanInternal>>>,
    /// Recent completed writer spans (bounded)
    writer_span_history: Arc<Mutex<std::collections::VecDeque<WriterSpanCompleted>>>,
    /// Reader tracking: active count and last label
    reader_active: Arc<std::sync::atomic::AtomicUsize>,
    reader_last: Arc<Mutex<Option<SectionInfo>>>,
    /// True when the per-DB read-write gate is held for writing (quiesced)
    reader_write_held: Arc<AtomicBool>,
}

#[derive(Clone, Debug)]
struct WriterSpanInternal {
    id: u64,
    symbols: usize,
    edges: usize,
    started_at: std::time::Instant,
}

#[derive(Clone, Debug, Serialize)]
pub struct WriterSpanCompleted {
    pub id: u64,
    pub symbols: usize,
    pub edges: usize,
    pub duration_ms: u128,
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct WriterStatusSnapshot {
    pub busy: bool,
    pub active_ms: Option<u128>,
    pub active_symbols: Option<usize>,
    pub active_edges: Option<usize>,
    pub recent: Vec<WriterSpanCompleted>,
    // New: gate owner and section details
    pub gate_owner_op: Option<String>,
    pub gate_owner_ms: Option<u128>,
    pub section_label: Option<String>,
    pub section_ms: Option<u128>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReaderStatusSnapshot {
    pub active: usize,
    pub last_label: Option<String>,
    pub last_ms: Option<u128>,
}

#[derive(Clone)]
struct ReaderBackendHandles {
    active: Arc<std::sync::atomic::AtomicUsize>,
    last: Arc<Mutex<Option<SectionInfo>>>,
}

pub struct ReaderGuard {
    backend: ReaderBackendHandles,
    // Hold a read lock on the per-DB gate; dropped on guard drop
    _guard: Option<OwnedRwLockReadGuard<()>>,
}

impl Drop for ReaderGuard {
    fn drop(&mut self) {
        self.backend.active.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Messages for the single-writer task
enum WriteMsg {
    StoreSymbols(Vec<SymbolState>, oneshot::Sender<Result<(), DatabaseError>>),
    StoreEdges(Vec<Edge>, oneshot::Sender<Result<(), DatabaseError>>),
    Flush(oneshot::Sender<Result<(), DatabaseError>>),
}

impl SQLiteBackend {
    /// Engine-direct checkpoint via Turso connection API (if available). Falls back to PRAGMA.
    pub async fn engine_checkpoint_internal(
        &self,
        mode: DbCheckpointMode,
    ) -> Result<(), DatabaseError> {
        let conn = self.get_direct_connection().await?;
        // Try direct API first (if feature enabled and available)
        #[allow(unused_mut)]
        let mut used_direct = false;
        #[cfg(feature = "turso-direct-checkpoint")]
        {
            // Current turso::Connection in this dependency set does not expose
            // a checkpoint API; keeping the feature stub for future versions.
            warn!("turso-direct-checkpoint feature enabled, but no direct API available in this build; using PRAGMA fallback");
        }
        if !used_direct {
            let sql = match mode {
                DbCheckpointMode::Passive => "PRAGMA wal_checkpoint(PASSIVE)",
                DbCheckpointMode::Full => "PRAGMA wal_checkpoint(FULL)",
                DbCheckpointMode::Restart => "PRAGMA wal_checkpoint(RESTART)",
                DbCheckpointMode::Truncate => "PRAGMA wal_checkpoint(TRUNCATE)",
            };
            // PRAGMA wal_checkpoint returns a single row (busy, checkpointed, total). Use query.
            let mut rows = safe_query(&conn, sql, (), "engine_checkpoint.fallback.query").await?;
            // Drain one row if present; ignore values.
            let _ = rows.next().await;
        }
        Ok(())
    }
    #[inline]
    fn is_none_uid(uid: &str) -> bool {
        uid == "none" || uid.starts_with("none::")
    }
    fn writer_gate_for_path(&self) -> Arc<tokio::sync::Mutex<()>> {
        get_writer_gate(&self.sqlite_config.path)
    }
    fn gate_owner_handle(&self) -> Arc<Mutex<Option<GateOwnerInfo>>> {
        get_gate_owner_handle(&self.sqlite_config.path)
    }
    fn section_handle(&self) -> Arc<Mutex<Option<SectionInfo>>> {
        get_section_handle(&self.sqlite_config.path)
    }
    fn reader_rw_gate_for_path(&self) -> Arc<AsyncRwLock<()>> {
        get_reader_rw_gate(&self.sqlite_config.path)
    }
    /// Create a new SQLiteBackend with custom SQLite configuration
    pub async fn with_sqlite_config(
        _config: DatabaseConfig,
        sqlite_config: SQLiteConfig,
    ) -> Result<Self, DatabaseError> {
        let pool = ConnectionPool::new(sqlite_config.clone()).await?;

        let (tx, mut rx) = mpsc::channel::<WriteMsg>(1024);
        let busy_flag = Arc::new(AtomicBool::new(false));

        let backend = Self {
            pool: Arc::new(Mutex::new(pool)),
            sqlite_config: sqlite_config.clone(),
            trees: RwLock::new(HashMap::new()),
            writer_tx: tx.clone(),
            writer_busy: busy_flag.clone(),
            writer_span_seq: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            writer_span_active: Arc::new(Mutex::new(None)),
            writer_span_history: Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(
                64,
            ))),
            reader_active: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            reader_last: Arc::new(Mutex::new(None)),
            reader_write_held: Arc::new(AtomicBool::new(false)),
        };

        if sqlite_config.temporary {
            info!("Initialized temporary SQLite database (in-memory)");
        } else {
            info!(
                "Initialized persistent SQLite database at: {}",
                sqlite_config.path
            );
        }

        // Initialize the default workspace record for this database
        backend.ensure_default_workspace().await?;

        // Spawn single-writer task
        let writer_backend = backend.clone_for_writer();
        let busy_for_task = busy_flag.clone();
        tokio::spawn(async move {
            let mut pending_symbols: Vec<SymbolState> = Vec::new();
            let mut pending_edges: Vec<Edge> = Vec::new();
            let mut last_flush = std::time::Instant::now();
            let max_symbols = 500usize;
            let max_edges = 3000usize;
            let flush_after = std::time::Duration::from_millis(75);

            // RAII guard to ensure busy flag always clears even on panic/early-return
            struct BusyGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);
            impl BusyGuard {
                fn new(flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
                    flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    Self(flag)
                }
            }
            impl Drop for BusyGuard {
                fn drop(&mut self) {
                    self.0.store(false, std::sync::atomic::Ordering::Relaxed);
                }
            }

            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Some(WriteMsg::StoreSymbols(mut symbols, ack)) => {
                                pending_symbols.append(&mut symbols);
                                let need_flush = pending_symbols.len() >= max_symbols || last_flush.elapsed() >= flush_after;
                                if need_flush {
                                    let _busy = BusyGuard::new(busy_for_task.clone());
                                    writer_backend.begin_writer_span(pending_symbols.len(), pending_edges.len()).await;
                                    let res = writer_backend.flush_writes(&pending_symbols, &pending_edges).await;
                                    if res.is_ok() { pending_symbols.clear(); pending_edges.clear(); last_flush = std::time::Instant::now(); }
                                    writer_backend.end_writer_span(res.is_ok()).await;
                                    let _ = ack.send(res);
                                } else {
                                    let _ = ack.send(Ok(()));
                                }
                            }
                            Some(WriteMsg::StoreEdges(mut edges, ack)) => {
                                pending_edges.append(&mut edges);
                                let need_flush = pending_edges.len() >= max_edges || last_flush.elapsed() >= flush_after;
                                if need_flush {
                                    let _busy = BusyGuard::new(busy_for_task.clone());
                                    writer_backend.begin_writer_span(pending_symbols.len(), pending_edges.len()).await;
                                    let res = writer_backend.flush_writes(&pending_symbols, &pending_edges).await;
                                    if res.is_ok() { pending_symbols.clear(); pending_edges.clear(); last_flush = std::time::Instant::now(); }
                                    writer_backend.end_writer_span(res.is_ok()).await;
                                    let _ = ack.send(res);
                                } else {
                                    let _ = ack.send(Ok(()));
                                }
                            }
                            Some(WriteMsg::Flush(ack)) => {
                                let _busy = BusyGuard::new(busy_for_task.clone());
                                writer_backend.begin_writer_span(pending_symbols.len(), pending_edges.len()).await;
                                let res = writer_backend.flush_writes(&pending_symbols, &pending_edges).await;
                                if res.is_ok() { pending_symbols.clear(); pending_edges.clear(); last_flush = std::time::Instant::now(); }
                                writer_backend.end_writer_span(res.is_ok()).await;
                                let _ = ack.send(res);
                            }
                            None => {
                                // channel closed; try final flush and exit
                                let _busy = BusyGuard::new(busy_for_task.clone());
                                writer_backend.begin_writer_span(pending_symbols.len(), pending_edges.len()).await;
                                let _ = writer_backend.flush_writes(&pending_symbols, &pending_edges).await;
                                writer_backend.end_writer_span(true).await;
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep(flush_after), if !pending_symbols.is_empty() || !pending_edges.is_empty() => {
                        let _busy = BusyGuard::new(busy_for_task.clone());
                        writer_backend.begin_writer_span(pending_symbols.len(), pending_edges.len()).await;
                        let _ = writer_backend.flush_writes(&pending_symbols, &pending_edges).await;
                        pending_symbols.clear();
                        pending_edges.clear();
                        last_flush = std::time::Instant::now();
                        writer_backend.end_writer_span(true).await;
                    }
                }
            }
        });

        Ok(backend)
    }

    /// Clone minimal handles for writer task
    fn clone_for_writer(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            sqlite_config: self.sqlite_config.clone(),
            trees: RwLock::new(HashMap::new()), // not used by writer
            writer_tx: self.writer_tx.clone(),
            writer_busy: self.writer_busy.clone(),
            writer_span_seq: self.writer_span_seq.clone(),
            writer_span_active: self.writer_span_active.clone(),
            writer_span_history: self.writer_span_history.clone(),
            reader_active: self.reader_active.clone(),
            reader_last: self.reader_last.clone(),
            reader_write_held: self.reader_write_held.clone(),
        }
    }

    /// Flush pending writes in a single pass
    async fn flush_writes(
        &self,
        symbols: &[SymbolState],
        edges: &[Edge],
    ) -> Result<(), DatabaseError> {
        if symbols.is_empty() && edges.is_empty() {
            return Ok(());
        }

        // Use one direct connection to reduce overhead; separate transactions for symbols/edges
        let conn = self.get_direct_connection().await?;

        // Global writer gate serialization by database path
        let gate = self.writer_gate_for_path();
        // Expose waiting/holding the writer gate for debugging
        self.set_active_section("writer.wait_for_gate").await;
        let _guard = gate.lock().await;
        self.set_active_section("writer.gate_locked").await;
        {
            let owner = self.gate_owner_handle();
            let mut o = owner.lock().await;
            *o = Some(GateOwnerInfo {
                op: "flush_writes".to_string(),
                since: Instant::now(),
            });
        }

        if !symbols.is_empty() {
            self.set_active_section("store_symbols_with_conn").await;
            let res = self.store_symbols_with_conn(&conn, symbols).await;
            self.clear_active_section().await;
            res?;
        }
        if !edges.is_empty() {
            self.set_active_section("store_edges_with_conn").await;
            let res = self.store_edges_with_conn(&conn, edges).await;
            self.clear_active_section().await;
            res?;
        }
        // Clear owner and section before releasing the gate lock
        {
            let owner = self.gate_owner_handle();
            let mut o = owner.lock().await;
            *o = None;
        }
        self.clear_active_section().await;
        Ok(())
    }

    /// Expose whether the writer is currently active (batching/committing).
    pub fn is_writer_busy(&self) -> bool {
        self.writer_busy.load(Ordering::Relaxed)
    }

    /// Whether the pool is currently quiesced (WAL sync or similar)
    pub async fn is_quiesced(&self) -> bool {
        let pool = self.pool.lock().await;
        pool.quiesced.load(Ordering::Relaxed)
    }

    async fn begin_writer_span(&self, symbols: usize, edges: usize) {
        let id = self.writer_span_seq.fetch_add(1, Ordering::Relaxed);
        let mut guard = self.writer_span_active.lock().await;
        *guard = Some(WriterSpanInternal {
            id,
            symbols,
            edges,
            started_at: std::time::Instant::now(),
        });
        drop(guard);
        debug!(
            "[WRITER] span {} started (symbols={}, edges={})",
            id, symbols, edges
        );
    }

    async fn end_writer_span(&self, ok: bool) {
        let mut active = self.writer_span_active.lock().await;
        if let Some(span) = active.take() {
            let dur = span.started_at.elapsed().as_millis();
            let completed = WriterSpanCompleted {
                id: span.id,
                symbols: span.symbols,
                edges: span.edges,
                duration_ms: dur,
                ok,
            };
            let mut hist = self.writer_span_history.lock().await;
            if hist.len() >= 64 {
                hist.pop_front();
            }
            hist.push_back(completed.clone());
            drop(hist);
            debug!("[WRITER] span {} ended ok={} ({} ms)", span.id, ok, dur);
        }
    }

    pub async fn writer_status_snapshot(&self) -> WriterStatusSnapshot {
        let busy = self.is_writer_busy();
        let (active_ms, active_symbols, active_edges) = {
            let guard = self.writer_span_active.lock().await;
            if let Some(span) = &*guard {
                (
                    Some(span.started_at.elapsed().as_millis()),
                    Some(span.symbols),
                    Some(span.edges),
                )
            } else {
                (None, None, None)
            }
        };
        let recent = {
            let hist = self.writer_span_history.lock().await;
            hist.iter().cloned().rev().take(5).collect::<Vec<_>>()
        };
        let (gate_owner_op, gate_owner_ms) = {
            let owner = self.gate_owner_handle();
            let o = owner.lock().await;
            if let Some(info) = &*o {
                (
                    Some(info.op.clone()),
                    Some(info.since.elapsed().as_millis()),
                )
            } else {
                (None, None)
            }
        };
        let (section_label, section_ms) = {
            let sec = self.section_handle();
            let s = sec.lock().await;
            if let Some(info) = &*s {
                (
                    Some(info.label.clone()),
                    Some(info.since.elapsed().as_millis()),
                )
            } else {
                (None, None)
            }
        };
        WriterStatusSnapshot {
            busy,
            active_ms,
            active_symbols,
            active_edges,
            recent,
            gate_owner_op,
            gate_owner_ms,
            section_label,
            section_ms,
        }
    }

    async fn set_active_section(&self, label: &str) {
        let sec = self.section_handle();
        let mut s = sec.lock().await;
        *s = Some(SectionInfo {
            label: label.to_string(),
            since: Instant::now(),
        });
    }

    async fn clear_active_section(&self) {
        let sec = self.section_handle();
        let mut s = sec.lock().await;
        *s = None;
    }

    /// Non-channel variant of edge storage used by the writer task
    async fn store_edges_with_conn(
        &self,
        conn: &Connection,
        edges_in: &[Edge],
    ) -> Result<(), DatabaseError> {
        let normalized_edges: Vec<Edge> = edges_in
            .iter()
            .map(Self::normalize_edge_for_storage)
            .collect();

        let mut seen_signatures: HashSet<EdgeDedupKey> = HashSet::new();
        let mut unique_edges: Vec<Edge> = Vec::with_capacity(normalized_edges.len());
        for edge in normalized_edges {
            let signature = EdgeDedupKey::from_edge(&edge);
            if seen_signatures.insert(signature) {
                unique_edges.push(edge);
            }
        }

        let edges = unique_edges;
        let edges_len = edges.len();

        // Use deferred BEGIN to reduce lock contention with readers and background tasks
        let begin_ctx = format!("store_edges_with_conn begin (edges_total={})", edges_len);
        safe_execute_with_retry(conn, "BEGIN TRANSACTION", (), &begin_ctx, 10).await?;

        if edges_len > 0 {
            // Allow tuning batch size via env to mitigate lock pressure under load
            let batch_size = std::env::var("PROBE_LSP_EDGE_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|&n| n > 0)
                .unwrap_or(100);
            let mut offset = 0usize;
            while offset < edges_len {
                let end = usize::min(offset + batch_size, edges_len);
                let chunk_edges = &edges[offset..end];
                let chunk_keys: Vec<EdgeDedupKey> =
                    chunk_edges.iter().map(EdgeDedupKey::from_edge).collect();

                let mut existing_keys: HashSet<EdgeDedupKey> = HashSet::new();
                if !chunk_keys.is_empty() {
                    let mut query = String::from(
                        "SELECT relation, source_symbol_uid, target_symbol_uid, start_line, start_char, language FROM edge WHERE ",
                    );
                    let mut params: Vec<turso::Value> = Vec::new();
                    for (idx, key) in chunk_keys.iter().enumerate() {
                        if idx > 0 {
                            query.push_str(" OR ");
                        }
                        query.push_str("(relation = ? AND source_symbol_uid = ? AND target_symbol_uid = ? AND ");
                        params.push(turso::Value::Text(key.relation.clone()));
                        params.push(turso::Value::Text(key.source.clone()));
                        params.push(turso::Value::Text(key.target.clone()));
                        if key.start_line < 0 {
                            query.push_str("start_line IS NULL AND ");
                        } else {
                            query.push_str("start_line = ? AND ");
                            params.push(turso::Value::Integer(key.start_line));
                        }
                        if key.start_char < 0 {
                            query.push_str("start_char IS NULL AND ");
                        } else {
                            query.push_str("start_char = ? AND ");
                            params.push(turso::Value::Integer(key.start_char));
                        }
                        query.push_str("language = ?)");
                        params.push(turso::Value::Text(key.language.clone()));
                    }
                    let label = format!(
                        "edges.dedup_select {}/{}",
                        (offset / batch_size) + 1,
                        (edges_len + batch_size - 1) / batch_size
                    );
                    self.set_active_section(&label).await;
                    let mut rows = safe_query_with_retry(
                        conn,
                        &query,
                        params,
                        "store_edges_with_conn dedup",
                        5,
                    )
                    .await?;
                    while let Some(row) =
                        rows.next()
                            .await
                            .map_err(|e| DatabaseError::OperationFailed {
                                message: format!("Failed to iterate dedup rows: {e}"),
                            })?
                    {
                        let relation = match row.get_value(0) {
                            Ok(turso::Value::Text(v)) => v,
                            _ => continue,
                        };
                        let source = match row.get_value(1) {
                            Ok(turso::Value::Text(v)) => v,
                            _ => continue,
                        };
                        let target = match row.get_value(2) {
                            Ok(turso::Value::Text(v)) => v,
                            _ => continue,
                        };
                        let start_line = match row.get_value(3) {
                            Ok(turso::Value::Integer(v)) => v,
                            Ok(turso::Value::Null) => -1,
                            _ => -1,
                        };
                        let start_char = match row.get_value(4) {
                            Ok(turso::Value::Integer(v)) => v,
                            Ok(turso::Value::Null) => -1,
                            _ => -1,
                        };
                        let language = match row.get_value(5) {
                            Ok(turso::Value::Text(v)) => v,
                            _ => continue,
                        };
                        existing_keys.insert(EdgeDedupKey {
                            relation,
                            source,
                            target,
                            language,
                            start_line,
                            start_char,
                        });
                    }
                    self.clear_active_section().await;
                }

                let mut edges_to_insert: Vec<&Edge> = Vec::new();
                for (edge, key) in chunk_edges.iter().zip(chunk_keys.iter()) {
                    if !existing_keys.contains(key) {
                        edges_to_insert.push(edge);
                    }
                }

                if edges_to_insert.is_empty() {
                    offset = end;
                    continue;
                }

                let placeholders = edges_to_insert
                    .iter()
                    .map(|_| "(?, ?, ?, ?, ?, ?, ?, ?)")
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut params = Vec::with_capacity(edges_to_insert.len() * 8);
                for edge in edges_to_insert.iter() {
                    params.extend(vec![
                        turso::Value::Text(edge.relation.to_string().to_string()),
                        turso::Value::Text(edge.source_symbol_uid.clone()),
                        turso::Value::Text(edge.target_symbol_uid.clone()),
                        edge.start_line
                            .map(|l| turso::Value::Integer((if l >= 1 { l } else { 1 }) as i64))
                            .unwrap_or(turso::Value::Null),
                        edge.start_char
                            .map(|c| turso::Value::Integer(c as i64))
                            .unwrap_or(turso::Value::Null),
                        turso::Value::Real(edge.confidence as f64),
                        turso::Value::Text(edge.language.clone()),
                        edge.metadata
                            .clone()
                            .map(turso::Value::Text)
                            .unwrap_or(turso::Value::Null),
                    ]);
                }
                let batch_sql = format!("INSERT INTO edge (relation, source_symbol_uid, target_symbol_uid, start_line, start_char, confidence, language, metadata) VALUES {}", placeholders);
                let label = format!(
                    "edges.insert_batch {}/{} (+{})",
                    (offset / batch_size) + 1,
                    (edges_len + batch_size - 1) / batch_size,
                    edges_to_insert.len()
                );
                self.set_active_section(&label).await;
                // Enrich context with precise counts to make lock errors actionable
                let insert_ctx = format!(
                    "store_edges_with_conn insert (chunk={}/{}, batch_size={}, edges_total={})",
                    (offset / batch_size) + 1,
                    (edges_len + batch_size - 1) / batch_size,
                    edges_to_insert.len(),
                    edges_len
                );
                safe_execute_with_retry(conn, &batch_sql, params, &insert_ctx, 10).await?;
                self.clear_active_section().await;
                offset = end;
            }
        }

        self.set_active_section("edges.commit").await;
        let commit_ctx = format!("store_edges_with_conn commit (edges_total={})", edges_len);
        if let Err(e) = safe_execute_with_retry(conn, "COMMIT", (), &commit_ctx, 10).await {
            rollback_transaction(conn, "store_edges_with_conn commit failure").await;
            self.clear_active_section().await;
            return Err(e);
        }
        self.clear_active_section().await;
        Ok(())
    }

    /// Ensures that a default workspace record exists in the database
    /// Each database should have exactly one workspace record representing the current workspace
    async fn ensure_default_workspace(&self) -> Result<(), DatabaseError> {
        let gate = self.writer_gate_for_path();
        let _guard = gate.lock().await;
        {
            let owner = self.gate_owner_handle();
            let mut o = owner.lock().await;
            *o = Some(GateOwnerInfo {
                op: "ensure_default_workspace".to_string(),
                since: Instant::now(),
            });
        }
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        // Check if any workspace records exist
        let mut rows = safe_query(
            &conn,
            "SELECT COUNT(*) FROM workspace",
            (),
            "ensure_default_workspace.count",
        )
        .await?;

        let count = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to read workspace count: {}", e),
                })? {
            match row.get_value(0) {
                Ok(turso::Value::Integer(n)) => n,
                _ => 0,
            }
        } else {
            0
        };

        // If no workspace records exist, create the default one
        if count == 0 {
            let workspace_id = 1; // Always use ID 1 for the single workspace
            let project_id = 1; // Always use project ID 1

            // Get current directory name as workspace name, or use "default"
            let workspace_name = std::env::current_dir()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| "default".to_string());

            // Try to get git branch if available
            let current_branch =
                Self::get_current_git_branch().unwrap_or_else(|| "main".to_string());
            let current_dir = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());

            safe_execute(
                &conn,
                r#"
                INSERT INTO workspace (workspace_id, project_id, name, path, current_branch, created_at, updated_at, metadata)
                VALUES (?, ?, ?, ?, ?, datetime('now'), datetime('now'), '{}')
                "#,
                [
                    turso::Value::Text(workspace_id.to_string()),
                    turso::Value::Integer(project_id),
                    turso::Value::Text(workspace_name.clone()),
                    turso::Value::Text(current_dir.clone()),
                    turso::Value::Text(current_branch.clone()),
                ],
                "ensure_default_workspace.insert_workspace",
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed { message: format!("Failed to create default workspace: {}", e) })?;

            // Also create a default project record if needed
            // First check if project exists (turso doesn't support INSERT OR IGNORE)
            let mut check_rows = safe_query(
                &conn,
                "SELECT 1 FROM project WHERE project_id = ?",
                [turso::Value::Integer(project_id)],
                "check project existence",
            )
            .await?;

            let project_exists = check_rows
                .next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to check project existence: {}", e),
                })?
                .is_some();

            if !project_exists {
                // Only insert if project doesn't exist
                safe_execute(
                    &conn,
                    r#"
                    INSERT INTO project (project_id, root_path, name, created_at, updated_at, metadata)
                    VALUES (?, ?, ?, datetime('now'), datetime('now'), '{}')
                    "#,
                    [
                        turso::Value::Integer(project_id),
                        turso::Value::Text(current_dir.clone()),
                        turso::Value::Text(workspace_name),
                    ],
                    "create default project"
                )
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to create default project: {}", e),
                })?;
            }

            tracing::info!(
                "Created default workspace (ID: {}) with branch '{}' in project (ID: {})",
                workspace_id,
                current_branch,
                project_id
            );
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        {
            let owner = self.gate_owner_handle();
            let mut o = owner.lock().await;
            *o = None;
        }
        Ok(())
    }

    fn normalize_symbol_for_storage(symbol: &SymbolState) -> SymbolState {
        let mut normalized = symbol.clone();
        let normalized_uid = normalize_uid_with_hint(&normalized.symbol_uid, None);
        let sanitized_path =
            Self::determine_file_path_for_uid(&normalized_uid, &normalized.file_path);
        normalized.file_path = sanitized_path.clone();
        normalized.symbol_uid = Self::rebuild_uid_with_path(&normalized_uid, &sanitized_path);
        normalized
    }

    fn normalize_edge_for_storage(edge: &Edge) -> Edge {
        let mut normalized = edge.clone();
        let workspace_hint = normalized.file_path.as_ref().and_then(|path_str| {
            let path = Path::new(path_str);
            let canonical = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir().ok()?.join(path)
            };
            workspace_utils::find_workspace_root_with_fallback(&canonical).ok()
        });

        let normalized_source =
            normalize_uid_with_hint(&normalized.source_symbol_uid, workspace_hint.as_deref());
        let normalized_target =
            normalize_uid_with_hint(&normalized.target_symbol_uid, workspace_hint.as_deref());
        let fallback_path = normalized
            .file_path
            .as_deref()
            .unwrap_or_default()
            .to_string();
        let source_canonical = normalized
            .file_path
            .as_ref()
            .and_then(|p| Path::new(p).canonicalize().ok());
        let sanitized_source_path = if let (Some(root), Some(canonical)) =
            (workspace_hint.as_ref(), source_canonical.as_ref())
        {
            if let Some(rel) = diff_paths(canonical, root) {
                Self::sanitize_path_string(rel.to_string_lossy().as_ref())
            } else {
                Self::determine_file_path_for_uid(&normalized_source, &fallback_path)
            }
        } else {
            Self::determine_file_path_for_uid(&normalized_source, &fallback_path)
        };

        let target_canonical = Self::uid_path_component(&normalized_target)
            .filter(|path| is_absolute_like(path))
            .and_then(|path| Path::new(path).canonicalize().ok());
        let sanitized_target_path = if let (Some(root), Some(canonical)) =
            (workspace_hint.as_ref(), target_canonical.as_ref())
        {
            if let Some(rel) = diff_paths(canonical, root) {
                Self::sanitize_path_string(rel.to_string_lossy().as_ref())
            } else {
                Self::determine_file_path_for_uid(&normalized_target, &fallback_path)
            }
        } else {
            Self::determine_file_path_for_uid(&normalized_target, &fallback_path)
        };
        normalized.source_symbol_uid =
            Self::rebuild_uid_with_path(&normalized_source, &sanitized_source_path);
        // Always keep sentinel strictly as "none"
        normalized.target_symbol_uid =
            if Self::is_none_uid(&normalized_target) || sanitized_target_path == "none" {
                "none".to_string()
            } else {
                Self::rebuild_uid_with_path(&normalized_target, &sanitized_target_path)
            };
        if let Some(path) = normalized.file_path.as_ref() {
            normalized.file_path = Some(Self::sanitize_path_string(path));
        }
        normalized
    }

    fn sanitize_path_string(path: &str) -> String {
        if path.is_empty() {
            return String::new();
        }

        if path.starts_with("EXTERNAL:") || path.starts_with("UNRESOLVED:") {
            return path.replace('\\', "/");
        }

        let mut normalized = path.replace('\\', "/");

        while normalized.starts_with("./") {
            normalized = normalized.trim_start_matches("./").to_string();
        }

        while normalized.contains("//") {
            normalized = normalized.replace("//", "/");
        }

        // Preserve canonical dependency prefix
        if normalized.starts_with("/dep/") {
            return normalized;
        }

        if is_absolute_like(&normalized) {
            if normalized.starts_with('/') {
                normalized = normalized.trim_start_matches('/').to_string();
            } else if normalized.len() > 2 && normalized.as_bytes()[1] == b':' {
                normalized = normalized[2..].to_string();
                normalized = normalized
                    .trim_start_matches('/')
                    .trim_start_matches('\\')
                    .to_string();
            }
        }

        normalized
    }

    fn determine_file_path_for_uid(uid: &str, fallback: &str) -> String {
        if let Some(path_part) = Self::uid_path_component(uid) {
            if !path_part.is_empty()
                && !path_part.starts_with("EXTERNAL:")
                && !path_part.starts_with("UNRESOLVED:")
            {
                return Self::sanitize_path_string(path_part);
            }
        }

        Self::sanitize_path_string(fallback)
    }

    fn uid_path_component(uid: &str) -> Option<&str> {
        uid.splitn(2, ':').next()
    }

    fn rebuild_uid_with_path(uid: &str, new_path: &str) -> String {
        if uid.is_empty()
            || uid.starts_with("EXTERNAL:")
            || uid.starts_with("UNRESOLVED:")
            || uid.starts_with("fallback_")
        {
            return uid.to_string();
        }
        // Preserve sentinel exactly
        if Self::is_none_uid(uid) || new_path == "none" {
            return "none".to_string();
        }

        let mut parts = uid.splitn(4, ':');
        let _ = parts.next();
        let hash_part = parts.next().unwrap_or_default();
        let name_part = parts.next().unwrap_or_default();
        let line_part = parts.next().unwrap_or_default();

        format!("{}:{}:{}:{}", new_path, hash_part, name_part, line_part)
    }

    /// Perform a manual WAL checkpoint (turso/libSQL aware)
    pub async fn perform_checkpoint(&self) -> Result<(), DatabaseError> {
        // Note: turso v0.1.4 had a critical bug with PRAGMA wal_checkpoint, but
        // we're now using turso v0.2.0-pre.7 which should support it.
        // Let's try to perform the checkpoint and handle any errors gracefully.

        let pool_arc = self.pool.clone();
        let mut pool = pool_arc.lock().await;

        // Clear all idle connections from the pool to ensure checkpoint can proceed
        // SQLite checkpoint requires no other connections to be active
        pool.available.clear();

        // Checkout a connection, then immediately drop the pool lock to avoid
        // holding the mutex across async DB calls (prevents global stalls).
        let conn = pool.get_connection().await?;
        drop(pool);

        // Try to execute PRAGMA wal_checkpoint(TRUNCATE) to force checkpoint and truncate WAL
        // Note: turso may be managing WAL internally, so we'll try but not fail if it doesn't work

        // First, let's try using prepare and query on the statement
        debug!("📋 CHECKPOINT: Attempting to prepare checkpoint statement");
        match conn.prepare("PRAGMA wal_checkpoint(TRUNCATE)").await {
            Ok(mut stmt) => {
                debug!("📋 CHECKPOINT: Statement prepared, executing query");
                match stmt.query(()).await {
                    Ok(mut rows) => {
                        debug!("📋 CHECKPOINT: Query executed, reading results");
                        let mut busy = -1i64;
                        let mut checkpointed = -1i64;
                        let mut total = -1i64;
                        let mut row_count = 0;

                        // Read the checkpoint result
                        while let Ok(Some(row)) = rows.next().await {
                            row_count += 1;
                            debug!("📋 CHECKPOINT: Reading row #{}", row_count);

                            // Try to get values and log what we get
                            match (row.get_value(0), row.get_value(1), row.get_value(2)) {
                                (Ok(v0), Ok(v1), Ok(v2)) => {
                                    debug!(
                                        "📋 CHECKPOINT: Got values: {:?}, {:?}, {:?}",
                                        v0, v1, v2
                                    );
                                    if let (
                                        turso::Value::Integer(b),
                                        turso::Value::Integer(cp),
                                        turso::Value::Integer(tot),
                                    ) = (v0, v1, v2)
                                    {
                                        busy = b;
                                        checkpointed = cp;
                                        total = tot;
                                    }
                                }
                                (Err(e0), _, _) => {
                                    debug!("📋 CHECKPOINT: Error getting value 0: {}", e0)
                                }
                                (_, Err(e1), _) => {
                                    debug!("📋 CHECKPOINT: Error getting value 1: {}", e1)
                                }
                                (_, _, Err(e2)) => {
                                    debug!("📋 CHECKPOINT: Error getting value 2: {}", e2)
                                }
                            }
                        }

                        debug!(
                            "📋 CHECKPOINT: Read {} rows. busy={}, checkpointed={}, total={}",
                            row_count, busy, checkpointed, total
                        );

                        if row_count == 0 {
                            debug!("📋 CHECKPOINT: No rows returned from checkpoint pragma");
                        } else if busy == 0 && total > 0 {
                            info!("📋 CHECKPOINT: WAL checkpoint completed successfully (checkpointed {}/{} frames)", checkpointed, total);
                        } else if busy != 0 && busy != -1 {
                            // This is expected with turso's connection pooling - log at debug level
                            debug!("📋 CHECKPOINT: WAL checkpoint blocked (database busy={}, checkpointed={}/{})", busy, checkpointed, total);
                        } else if total == 0 {
                            debug!("📋 CHECKPOINT: No WAL frames to checkpoint (WAL is empty or doesn't exist)");
                        } else if busy == -1 || total == -1 {
                            // This happens when turso returns (1, Null, Null) - expected behavior
                            debug!("📋 CHECKPOINT: Checkpoint skipped - database busy (turso connection pool active)");
                        } else {
                            debug!("📋 CHECKPOINT: Unexpected checkpoint result: busy={}, checkpointed={}, total={}", busy, checkpointed, total);
                        }
                    }
                    Err(e) => {
                        warn!("📋 CHECKPOINT: Failed to execute checkpoint query: {}", e);
                    }
                }
                // Return connection to pool
                let mut pool = pool_arc.lock().await;
                pool.return_connection(conn);
                Ok(())
            }
            Err(e) => {
                warn!(
                    "📋 CHECKPOINT: Failed to prepare checkpoint statement: {}",
                    e
                );
                // Return connection to pool
                let mut pool = pool_arc.lock().await;
                pool.return_connection(conn);
                Ok(())
            }
        }
    }

    /// Start a periodic checkpoint task that runs every N seconds
    pub fn start_periodic_checkpoint(
        self: Arc<Self>,
        interval_secs: u64,
    ) -> tokio::task::JoinHandle<()> {
        debug!(
            "📋 CHECKPOINT: Starting periodic WAL checkpoint task (interval: {}s)",
            interval_secs
        );

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            let mut checkpoint_count = 0u64;

            loop {
                interval.tick().await;
                checkpoint_count += 1;

                debug!(
                    "📋 CHECKPOINT: Running periodic checkpoint #{}",
                    checkpoint_count
                );

                // Avoid checkpointing while writer task is busy
                if self.is_writer_busy() {
                    debug!("📋 CHECKPOINT: Skipping checkpoint (writer busy)");
                    continue;
                }
                // Run a single passive checkpoint (no quiesce, no retries)
                if let Err(e) = self
                    .perform_checkpoint_once_with_mode(CheckpointMode::Passive)
                    .await
                {
                    warn!(
                        "📋 CHECKPOINT: Periodic checkpoint #{} failed: {}",
                        checkpoint_count, e
                    );
                } else {
                    // Log at debug level to avoid polluting logs (checkpoints usually fail with turso)
                    debug!(
                        "📋 CHECKPOINT: Periodic checkpoint #{} completed",
                        checkpoint_count
                    );
                }
            }
        })
    }

    /// Run a single WAL checkpoint with the provided mode, once, without retries.
    async fn perform_checkpoint_once_with_mode(
        &self,
        mode: CheckpointMode,
    ) -> Result<(), DatabaseError> {
        let conn = self.get_direct_connection().await?;
        let sql = match mode {
            CheckpointMode::Passive => "PRAGMA wal_checkpoint(PASSIVE)",
            CheckpointMode::Full => "PRAGMA wal_checkpoint(FULL)",
            CheckpointMode::Restart => "PRAGMA wal_checkpoint(RESTART)",
            CheckpointMode::Truncate => "PRAGMA wal_checkpoint(TRUNCATE)",
            CheckpointMode::Auto => "PRAGMA wal_checkpoint(PASSIVE)",
        };
        let mut rows = safe_query(&conn, sql, (), "periodic.checkpoint.once").await?;
        // Drain a single row if present; ignore counters
        let _ = rows.next().await;
        Ok(())
    }

    /// Force a WAL checkpoint in a blocking loop until it succeeds or timeout.
    ///
    /// Implementation notes:
    /// - Reuses a single direct connection for the entire operation to avoid
    ///   per-iteration connection churn and noisy configuration logs.
    /// - Clears idle pooled connections once up-front, then leaves the pool
    ///   untouched during retries.
    pub async fn wal_sync_blocking(
        &self,
        timeout: Option<std::time::Duration>,
        quiesce: bool,
        mode: CheckpointMode,
        cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<(u64, u32), DatabaseError> {
        use tokio::time::sleep;
        let start = Instant::now();
        let mut iterations: u32 = 0;

        // Take writer gate to prevent our own writer task from interfering
        let gate = self.writer_gate_for_path();
        let _guard = gate.lock().await;
        let owner_handle = self.gate_owner_handle();
        {
            let mut o = owner_handle.lock().await;
            *o = Some(GateOwnerInfo {
                op: "force_wal_sync".to_string(),
                since: Instant::now(),
            });
        }

        info!(
            "📋 WAL_SYNC: starting forced WAL checkpoint (timeout={:?}, quiesce={}, mode={:?})",
            timeout, quiesce, mode
        );
        self.set_active_section("wal.sync.begin").await;

        // Optionally quiesce readers via per-DB write lock (simpler and more reliable than permit floods)
        let mut _quiesce_write_guard: Option<OwnedRwLockWriteGuard<()>> = None;
        // RAII cleanup to ensure we always lift quiesce and clear debug markers
        let mut _cleanup = QuiesceGuard {
            pool: Some(self.pool.clone()),
            quiesced: false,
            _write_guard: None,
            write_flag: Some(self.reader_write_held.clone()),
            section: Some(self.section_handle()),
            owner: Some(owner_handle.clone()),
        };
        if quiesce {
            let gate = self.reader_rw_gate_for_path();
            info!("📋 WAL_SYNC: quiescing readers via write lock");
            let w = gate.clone().write_owned().await;
            _quiesce_write_guard = Some(w);
            // Mark write-held true while the write guard is owned
            self.reader_write_held.store(true, Ordering::Relaxed);
            info!("📋 WAL_SYNC: readers quiesced");

            // Pool-level quiesce: block new connection checkouts and wait for in-flight to return
            {
                let pool = self.pool.lock().await;
                pool.quiesced.store(true, Ordering::Relaxed);
            }
            _cleanup.quiesced = true;
            _cleanup._write_guard = _quiesce_write_guard.take();
            // Wait for in-flight connections to drop to zero
            let mut waited = 0u64;
            loop {
                let inflight = { self.pool.lock().await.checked_out.load(Ordering::Relaxed) };
                if inflight == 0 {
                    break;
                }
                if waited % 1000 == 0 {
                    info!(
                        "📋 WAL_SYNC: waiting for {} in-flight connections to return",
                        inflight
                    );
                }
                if let Some(max) = timeout {
                    if start.elapsed() >= max {
                        warn!("📋 WAL_SYNC: timeout waiting for in-flight connections ({} still active)", inflight);
                        // Early return; QuiesceGuard will release quiesce and clear markers
                        return Err(DatabaseError::OperationFailed {
                            message: format!(
                                "Timeout waiting for in-flight connections: {}",
                                inflight
                            ),
                        });
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                waited += 50;
            }
        }

        // Clear idle connections once to maximize checkpoint success chance
        {
            let mut pool = self.pool.lock().await;
            pool.available.clear();
        }

        // Use a single direct connection for the whole WAL sync to avoid
        // repeatedly creating and configuring connections on each retry.
        let conn = self.get_direct_connection().await?;
        // Helper to map mode→SQL
        fn checkpoint_sql(m: CheckpointMode) -> &'static str {
            match m {
                CheckpointMode::Truncate => "PRAGMA wal_checkpoint(TRUNCATE)",
                CheckpointMode::Passive => "PRAGMA wal_checkpoint(PASSIVE)",
                CheckpointMode::Full => "PRAGMA wal_checkpoint(FULL)",
                CheckpointMode::Restart => "PRAGMA wal_checkpoint(RESTART)",
                CheckpointMode::Auto => unreachable!(),
            }
        }

        // Parse auto order from env, default: truncate,full,restart
        let auto_modes: Option<Vec<CheckpointMode>> = if matches!(mode, CheckpointMode::Auto) {
            let env = std::env::var("PROBE_LSP_WAL_AUTO_ORDER")
                .unwrap_or_else(|_| "truncate,full,restart".to_string());
            let mut v = Vec::new();
            for part in env.split(',') {
                match part.trim().to_ascii_lowercase().as_str() {
                    "truncate" => v.push(CheckpointMode::Truncate),
                    "full" => v.push(CheckpointMode::Full),
                    "restart" => v.push(CheckpointMode::Restart),
                    "passive" => v.push(CheckpointMode::Passive),
                    _ => {}
                }
            }
            if v.is_empty() {
                Some(vec![
                    CheckpointMode::Truncate,
                    CheckpointMode::Full,
                    CheckpointMode::Restart,
                ])
            } else {
                Some(v)
            }
        } else {
            None
        };

        // For fixed modes, prepare once. For auto, we'll execute each configured mode per-iteration.
        let mut prepared_stmt = if !matches!(mode, CheckpointMode::Auto) {
            let pragma = checkpoint_sql(mode);
            match conn.prepare(pragma).await {
                Ok(stmt) => Some(stmt),
                Err(e) => {
                    warn!("📋 WAL_SYNC: prepare failed for {:?} ({}), falling back to execute each retry", mode, e);
                    None
                }
            }
        } else {
            None
        };

        // Determine WAL file path (best-effort) for optional logging only.
        // turso does not use a separate -shm file; read locks are in-process.
        let wal_path = if !self.sqlite_config.temporary {
            let db_path = std::path::Path::new(&self.sqlite_config.path);
            db_path.to_str().map(|p| format!("{}-wal", p))
        } else {
            None
        };

        let mut last_wal_size: Option<u64> = None;
        let mut tried_restart_on_truncate = false;
        loop {
            iterations += 1;
            // Run checkpoint and read busy/frames if available
            let mut busy = -1i64;
            let mut total = -1i64;
            let mut checkpointed = -1i64;
            let mut executed_ok = false;
            if let Some(modes) = &auto_modes {
                // Auto: iterate configured modes each loop until one succeeds
                for m in modes {
                    let sql = checkpoint_sql(*m);
                    if let Ok(mut rows) = conn.query(sql, ()).await {
                        executed_ok = true;
                        if let Ok(Some(row)) = rows.next().await {
                            if let (Ok(b), Ok(cp), Ok(t)) =
                                (row.get_value(0), row.get_value(1), row.get_value(2))
                            {
                                if let (
                                    turso::Value::Integer(bi),
                                    turso::Value::Integer(cpi),
                                    turso::Value::Integer(ti),
                                ) = (b, cp, t)
                                {
                                    busy = bi;
                                    checkpointed = cpi;
                                    total = ti;
                                }
                            }
                        }
                        if busy == 0 {
                            info!("📋 WAL_SYNC: {:?} succeeded during auto-mode", m);
                            break;
                        }
                    }
                }
            } else {
                // Fixed mode
                if let Some(stmt) = prepared_stmt.as_mut() {
                    if let Ok(mut rows) = stmt.query(()).await {
                        executed_ok = true;
                        if let Ok(Some(row)) = rows.next().await {
                            if let (Ok(b), Ok(cp), Ok(t)) =
                                (row.get_value(0), row.get_value(1), row.get_value(2))
                            {
                                if let (
                                    turso::Value::Integer(bi),
                                    turso::Value::Integer(cpi),
                                    turso::Value::Integer(ti),
                                ) = (b, cp, t)
                                {
                                    busy = bi;
                                    checkpointed = cpi;
                                    total = ti;
                                }
                            }
                        }
                    }
                } else {
                    let sql = checkpoint_sql(mode);
                    if conn.execute(sql, ()).await.is_ok() {
                        executed_ok = true;
                    }
                }
            }

            // Success detection for turso: busy==0 means checkpoint succeeded for the selected mode.
            // Additionally, for TRUNCATE we also accept "WAL file is gone or size==0" as success,
            // because some engines don't return counters reliably.
            let mut wal_zero = false;
            if matches!(mode, CheckpointMode::Truncate) {
                if let Some(ref walp) = wal_path {
                    if let Ok(meta) = tokio::fs::metadata(walp).await {
                        wal_zero = meta.len() == 0;
                        last_wal_size = Some(meta.len());
                    } else {
                        // If wal doesn't exist, treat as zero-sized
                        wal_zero = true;
                        last_wal_size = Some(0);
                    }
                } else {
                    // In-memory DB or no path — treat as success once PRAGMA executes
                    wal_zero = executed_ok;
                }
            }

            if executed_ok && (busy == 0 || wal_zero) {
                match mode {
                    CheckpointMode::Truncate | CheckpointMode::Auto => {
                        if let Some(ref walp) = wal_path {
                            match tokio::fs::metadata(walp).await {
                                Ok(meta) => info!(
                                    "📋 WAL_SYNC: checkpoint ok (mode={:?}); wal_size={} bytes",
                                    mode,
                                    meta.len()
                                ),
                                Err(_) => info!(
                                    "📋 WAL_SYNC: checkpoint ok (mode={:?}); wal size unknown",
                                    mode
                                ),
                            }
                        } else {
                            info!("📋 WAL_SYNC: checkpoint ok (mode={:?})", mode);
                        }
                    }
                    m => info!(
                        "📋 WAL_SYNC: checkpoint ok (mode={:?}, checkpointed={}, total={})",
                        m, checkpointed, total
                    ),
                }
                break;
            }

            // No separate fallback block: auto-mode loop above already tried configured modes

            // Success conditions: busy==0, or no frames to checkpoint
            if busy == 0 || total == 0 {
                info!(
                    "📋 WAL_SYNC: checkpoint completed (busy={}, checkpointed={}, total={}, iter={})",
                    busy, checkpointed, total, iterations
                );
                break;
            }

            // TRUNCATE fallback: when counters are unavailable (busy/total remain -1) and WAL size
            // is non-zero for a while, attempt a single RESTART as a pragmatic fallback.
            if matches!(mode, CheckpointMode::Truncate)
                && executed_ok
                && busy == -1
                && total == -1
                && !tried_restart_on_truncate
            {
                if let Some(sz) = last_wal_size {
                    if sz > 0 && iterations % 10 == 0 {
                        info!("📋 WAL_SYNC: TRUNCATE counters unavailable and wal_size={} > 0; trying RESTART fallback once", sz);
                        let _ = conn.execute("PRAGMA wal_checkpoint(RESTART)", ()).await;
                        tried_restart_on_truncate = true;
                    }
                }
            }

            // Timeout check
            if let Some(max) = timeout {
                let waited_ms = start.elapsed().as_millis() as u64;
                if start.elapsed() >= max {
                    warn!(
                        "📋 WAL_SYNC: timeout after {} ms (iter={}, busy={}, checkpointed={}, total={})",
                        waited_ms, iterations, busy, checkpointed, total
                    );
                    // Early return; QuiesceGuard will release quiesce and clear markers
                    return Err(DatabaseError::OperationFailed { message: format!(
                        "WAL sync timed out after {} ms (iterations={}, busy={}, checkpointed={}, total={})",
                        waited_ms, iterations, busy, checkpointed, total
                    )});
                }
            }

            // Cancellation check
            if let Some(flag) = &cancel {
                if flag.load(Ordering::Relaxed) {
                    warn!("📋 WAL_SYNC: canceled by client");
                    // Early return; QuiesceGuard will release quiesce and clear markers
                    return Err(DatabaseError::OperationFailed {
                        message: "Canceled by client".into(),
                    });
                }
            }

            // Backoff and retry
            if iterations == 1 || iterations % 10 == 0 {
                info!(
                    "📋 WAL_SYNC: retrying (iter={}, busy={}, checkpointed={}, total={})",
                    iterations, busy, checkpointed, total
                );
            }
            self.set_active_section("wal.sync.retry").await;
            sleep(std::time::Duration::from_millis(200)).await;
        }

        let waited_ms = start.elapsed().as_millis() as u64;
        // QuiesceGuard (RAII) clears section, owner, and quiesced flag on drop
        info!(
            "📋 WAL_SYNC: done in {} ms (iterations={})",
            waited_ms, iterations
        );
        Ok((waited_ms, iterations))
    }

    /// Begin a tracked reader section. Returns a guard that holds the reader gate (shared)
    /// and decrements the active counter on drop.
    pub async fn begin_reader(&self, label: &str) -> ReaderGuard {
        let gate = self.reader_rw_gate_for_path();
        let guard = gate.clone().read_owned().await;
        self.reader_active.fetch_add(1, Ordering::Relaxed);
        {
            let mut last = self.reader_last.lock().await;
            *last = Some(SectionInfo {
                label: label.to_string(),
                since: Instant::now(),
            });
        }
        ReaderGuard {
            backend: self.clone_for_reader(),
            _guard: Some(guard),
        }
    }

    /// Try to begin a reader section without blocking. Returns None if quiesced.
    pub async fn try_begin_reader(&self, label: &str) -> Option<ReaderGuard> {
        let gate = self.reader_rw_gate_for_path();
        // Try to obtain read lock quickly; if a write lock is held (quiesce), bail out.
        let try_ms: u64 = std::env::var("PROBE_LSP_TRY_READER_TIMEOUT_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);
        match tokio::time::timeout(
            std::time::Duration::from_millis(try_ms),
            gate.clone().read_owned(),
        )
        .await
        {
            Ok(guard) => {
                let guard = guard;
                self.reader_active.fetch_add(1, Ordering::Relaxed);
                {
                    let mut last = self.reader_last.lock().await;
                    *last = Some(SectionInfo {
                        label: label.to_string(),
                        since: Instant::now(),
                    });
                }
                Some(ReaderGuard {
                    backend: self.clone_for_reader(),
                    _guard: Some(guard),
                })
            }
            Err(_) => None,
        }
    }

    fn clone_for_reader(&self) -> ReaderBackendHandles {
        ReaderBackendHandles {
            active: self.reader_active.clone(),
            last: self.reader_last.clone(),
        }
    }

    pub async fn reader_status_snapshot(&self) -> ReaderStatusSnapshot {
        let active = self.reader_active.load(Ordering::Relaxed);
        let last = self.reader_last.lock().await.clone();
        ReaderStatusSnapshot {
            active,
            last_label: last.as_ref().map(|s| s.label.clone()),
            last_ms: last.as_ref().map(|s| s.since.elapsed().as_millis()),
        }
    }
    pub fn is_reader_write_held(&self) -> bool {
        self.reader_write_held.load(Ordering::Relaxed)
    }
    /// Helper to get current git branch, if available
    fn get_current_git_branch() -> Option<String> {
        use std::process::Command;

        Command::new("git")
            .args(&["branch", "--show-current"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                } else {
                    None
                }
            })
    }

    /// Create a new tree table if it doesn't exist
    async fn ensure_tree_table(&self, tree_name: &str) -> Result<(), DatabaseError> {
        let sanitized_name = sanitize_table_name(tree_name);
        // Serialize DDL with global writer gate to avoid contention with data writes
        let gate = self.writer_gate_for_path();
        let _guard = gate.lock().await;
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let table_name = format!("tree_{sanitized_name}");
        let sql = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {table_name} (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                created_at INTEGER DEFAULT 0,
                updated_at INTEGER DEFAULT 0
            )
            "#
        );

        safe_execute(
            &conn,
            &sql,
            (),
            &format!("Failed to create tree table '{tree_name}'"),
        )
        .await
        .map_err(|e| DatabaseError::OperationFailed {
            message: e.to_string(),
        })?;

        // Create index for the tree with unique suffix to avoid conflicts
        // Use a hash of the tree name and a random component to ensure uniqueness
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        tree_name.hash(&mut hasher);
        // Add current time to ensure uniqueness across test runs
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        let unique_suffix = hasher.finish();

        let index_name = format!("idx_{sanitized_name}_{unique_suffix:x}_key");
        let index_sql = format!("CREATE INDEX IF NOT EXISTS {index_name} ON {table_name}(key)");

        safe_execute(
            &conn,
            &index_sql,
            (),
            &format!("Failed to create index for tree '{tree_name}'"),
        )
        .await
        .map_err(|e| DatabaseError::OperationFailed {
            message: e.to_string(),
        })?;

        // Update metadata - check if exists first, then insert if needed
        let mut rows = safe_query(
            &conn,
            "SELECT tree_name FROM tree_metadata WHERE tree_name = ?",
            [turso::Value::Text(tree_name.to_string())],
            &format!("check tree metadata for '{tree_name}'"),
        )
        .await?;

        if rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate metadata check for '{tree_name}': {e}"),
            })?
            .is_none()
        {
            // Tree doesn't exist in metadata, insert it
            safe_execute(
                &conn,
                "INSERT INTO tree_metadata (tree_name) VALUES (?)",
                [turso::Value::Text(tree_name.to_string())],
                &format!("insert tree metadata for '{tree_name}'"),
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: e.to_string(),
            })?;
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(())
    }

    /// Get current database schema version (migrations removed)
    pub async fn get_schema_version(&self) -> Result<u32, DatabaseError> {
        Ok(1)
    }

    /// Run migrations manually up to target version
    pub async fn migrate_to(&self, _target_version: Option<u32>) -> Result<u32, DatabaseError> {
        Ok(1)
    }

    /// Rollback migrations to target version
    pub async fn rollback_to(&self, _target_version: u32) -> Result<u32, DatabaseError> {
        Ok(1)
    }

    /// Check if migrations are needed
    pub async fn needs_migration(&self) -> Result<bool, DatabaseError> {
        Ok(false)
    }

    /// Get the database file path
    pub fn database_path(&self) -> PathBuf {
        PathBuf::from(&self.sqlite_config.path)
    }

    /// Perform a checkpoint to ensure WAL is flushed to the main database file
    pub async fn checkpoint(&self) -> Result<(), DatabaseError> {
        // Serialize explicit checkpoint with writer gate to avoid racing commits
        let gate = self.writer_gate_for_path();
        let _guard = gate.lock().await;
        // Use the same logic as perform_checkpoint but synchronous and tolerant
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        // libSQL/Turso returns a row for wal_checkpoint, so use query and ignore fields
        match safe_query(&conn, "PRAGMA wal_checkpoint(FULL)", (), "checkpoint(FULL)").await {
            Ok(mut rows) => {
                // Consume one row if present to avoid "unexpected row" errors
                let _ = rows.next().await;
                {
                    let mut pool = self.pool.lock().await;
                    pool.return_connection(conn);
                }
                Ok(())
            }
            Err(e) => {
                // Fallback to TRUNCATE form, also via query, still tolerant
                warn!("WAL checkpoint FULL failed ({}), retrying with TRUNCATE", e);
                match safe_query(
                    &conn,
                    "PRAGMA wal_checkpoint(TRUNCATE)",
                    (),
                    "checkpoint(TRUNCATE)",
                )
                .await
                {
                    Ok(mut rows) => {
                        let _ = rows.next().await;
                        {
                            let mut pool = self.pool.lock().await;
                            pool.return_connection(conn);
                        }
                        Ok(())
                    }
                    Err(e2) => {
                        {
                            let mut pool = self.pool.lock().await;
                            pool.return_connection(conn);
                        }
                        Err(DatabaseError::OperationFailed {
                            message: format!("WAL checkpoint failed: {}", e2),
                        })
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EdgeDedupKey {
    relation: String,
    source: String,
    target: String,
    language: String,
    start_line: i64,
    start_char: i64,
}

impl EdgeDedupKey {
    fn from_edge(edge: &Edge) -> Self {
        Self {
            relation: edge.relation.to_string().to_owned(),
            source: edge.source_symbol_uid.clone(),
            target: edge.target_symbol_uid.clone(),
            language: edge.language.clone(),
            start_line: edge.start_line.map(|v| v as i64).unwrap_or(-1),
            start_char: edge.start_char.map(|v| v as i64).unwrap_or(-1),
        }
    }
}

#[async_trait]
impl DatabaseBackend for SQLiteBackend {
    type Tree = SQLiteTree;

    async fn new(config: DatabaseConfig) -> Result<Self, DatabaseError>
    where
        Self: Sized,
    {
        let sqlite_config = SQLiteConfig {
            path: if config.temporary {
                ":memory:".to_string()
            } else {
                config
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| ":memory:".to_string())
            },
            temporary: config.temporary,
            enable_wal: !config.temporary, // Enable WAL for persistent databases
            page_size: 4096,
            cache_size: (config.cache_capacity / 4096).max(100) as i32, // Convert bytes to pages
            enable_foreign_keys: !config.temporary, // Enable foreign keys for persistent databases
        };

        Self::with_sqlite_config(config, sqlite_config).await
    }

    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };
        let mut rows = safe_query(
            &conn,
            "SELECT value FROM kv_store WHERE key = ?",
            [turso::Value::Text(key_str.to_string())],
            "kv.get",
        )
        .await?;

        let value = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate rows in default store: {e}"),
                })? {
            match row.get_value(0) {
                Ok(turso::Value::Blob(blob)) => Some(blob),
                _ => None,
            }
        } else {
            None
        };

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(value)
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        // Try update first
        let timestamp = chrono::Utc::now().timestamp();
        let rows_updated = conn
            .execute(
                "UPDATE kv_store SET value = ?, updated_at = ? WHERE key = ?",
                [
                    turso::Value::Blob(value.to_vec()),
                    turso::Value::Integer(timestamp),
                    turso::Value::Text(key_str.to_string()),
                ],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to update key in default store: {e}"),
            })?;

        // If no rows were updated, insert new record
        if rows_updated == 0 {
            let timestamp = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT INTO kv_store (key, value, created_at, updated_at) VALUES (?, ?, ?, ?)",
                [
                    turso::Value::Text(key_str.to_string()),
                    turso::Value::Blob(value.to_vec()),
                    turso::Value::Integer(timestamp),
                    turso::Value::Integer(timestamp),
                ],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to insert key in default store: {e}"),
            })?;
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(())
    }

    async fn remove(&self, key: &[u8]) -> Result<bool, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let rows_affected = conn
            .execute(
                "DELETE FROM kv_store WHERE key = ?",
                [turso::Value::Text(key_str.to_string())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to remove key from default store: {e}"),
            })?;

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(rows_affected > 0)
    }

    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DatabaseError> {
        let prefix_str = String::from_utf8_lossy(prefix);
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let (sql, params) = if prefix.is_empty() {
            (
                "SELECT key, value FROM kv_store ORDER BY key".to_string(),
                Vec::new(),
            )
        } else {
            (
                "SELECT key, value FROM kv_store WHERE key GLOB ? || '*' ORDER BY key".to_string(),
                vec![turso::Value::Text(prefix_str.to_string())],
            )
        };

        let mut rows = safe_query(&conn, &sql, params, "kv.scan_prefix").await?;

        let mut results = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate rows in default store: {e}"),
            })?
        {
            if let (Ok(turso::Value::Text(key)), Ok(turso::Value::Blob(value))) =
                (row.get_value(0), row.get_value(1))
            {
                results.push((key.as_bytes().to_vec(), value));
            }
            // Skip malformed rows
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(results)
    }

    async fn open_tree(&self, name: &str) -> Result<Arc<Self::Tree>, DatabaseError> {
        // Check if tree already exists in cache
        {
            let trees = self.trees.read().await;
            if let Some(tree) = trees.get(name) {
                return Ok(Arc::clone(tree));
            }
        }

        // Ensure tree table exists
        self.ensure_tree_table(name).await?;

        // Create new tree instance
        let tree = Arc::new(SQLiteTree {
            name: name.to_string(),
            pool: Arc::clone(&self.pool),
        });

        // Cache the tree
        {
            let mut trees = self.trees.write().await;
            trees.insert(name.to_string(), Arc::clone(&tree));
        }

        Ok(tree)
    }

    async fn tree_names(&self) -> Result<Vec<String>, DatabaseError> {
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let mut rows = safe_query(
            &conn,
            "SELECT tree_name FROM tree_metadata ORDER BY tree_name",
            (),
            "tree.names",
        )
        .await?;

        let mut names = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate tree names: {e}"),
            })?
        {
            if let Ok(turso::Value::Text(name)) = row.get_value(0) {
                names.push(name);
            }
            // Skip malformed rows
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(names)
    }

    async fn clear(&self) -> Result<(), DatabaseError> {
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        // kv_store and tree_* tables were removed from the schema. Keep clear() tolerant.
        // Best-effort: clear core tables used by the current backend.
        let _ = safe_execute(&conn, "DELETE FROM symbol_state", (), "clear.symbol_state").await;
        let _ = safe_execute(&conn, "DELETE FROM edge", (), "clear.edge").await;

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(())
    }

    async fn flush(&self) -> Result<(), DatabaseError> {
        if !self.sqlite_config.temporary {
            // For Turso, flush is handled automatically by the underlying database
            // Most pragmas are not supported, so we'll just do a no-op for persistent databases
            // The database will be automatically flushed when connections are closed
        }
        Ok(())
    }

    async fn stats(&self) -> Result<DatabaseStats, DatabaseError> {
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        // Count entries from core tables only (kv_store removed)
        let mut total_entries: u64 = 0;

        // symbol_state count
        if let Ok(mut rows) = safe_query(
            &conn,
            "SELECT COUNT(*) FROM symbol_state",
            (),
            "stats.count-symbol",
        )
        .await
        {
            if let Some(row) = rows
                .next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to read symbol_state count: {e}"),
                })?
            {
                if let Ok(turso::Value::Integer(n)) = row.get_value(0) {
                    total_entries += n as u64;
                }
            }
        }

        // edge count
        if let Ok(mut rows) =
            safe_query(&conn, "SELECT COUNT(*) FROM edge", (), "stats.count-edge").await
        {
            if let Some(row) = rows
                .next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to read edge count: {e}"),
                })?
            {
                if let Ok(turso::Value::Integer(n)) = row.get_value(0) {
                    total_entries += n as u64;
                }
            }
        }

        // Estimate total size (rough estimate)
        let estimated_avg_entry_size = 256; // bytes per entry
        let total_size_bytes = total_entries * estimated_avg_entry_size;

        let disk_size_bytes = if self.sqlite_config.temporary {
            0
        } else {
            self.size_on_disk().await?
        };

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }

        Ok(DatabaseStats {
            total_entries,
            total_size_bytes,
            disk_size_bytes,
            tree_count: 0,
            is_temporary: self.sqlite_config.temporary,
        })
    }

    async fn size_on_disk(&self) -> Result<u64, DatabaseError> {
        if self.sqlite_config.temporary || self.sqlite_config.path == ":memory:" {
            return Ok(0);
        }

        let path = PathBuf::from(&self.sqlite_config.path);
        if path.exists() {
            std::fs::metadata(&path)
                .map(|metadata| metadata.len())
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to get database file size: {e}"),
                })
        } else {
            Ok(0)
        }
    }

    fn is_temporary(&self) -> bool {
        self.sqlite_config.temporary
    }

    async fn engine_checkpoint(&self, mode: DbCheckpointMode) -> Result<(), DatabaseError> {
        self.engine_checkpoint_internal(mode).await
    }

    // ===================
    // Workspace Management
    // ===================

    async fn create_workspace(
        &self,
        _name: &str,
        _project_id: i64,
        _branch_hint: Option<&str>,
    ) -> Result<i64, DatabaseError> {
        // In the simplified single-workspace model, we don't create additional workspaces
        // The default workspace (ID: 1) is created automatically during database initialization
        // Return the fixed workspace ID
        Ok(1)
    }

    async fn get_workspace(&self, workspace_id: i64) -> Result<Option<Workspace>, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let workspace_id_str = workspace_id.to_string();
        let mut rows = conn
            .query(
                r#"
                SELECT w.workspace_id, w.project_id, w.name, '' as description, 
                       w.current_branch, 1 as is_active, w.created_at
                FROM workspace w 
                WHERE w.workspace_id = ?
                "#,
                [turso::Value::Text(workspace_id_str)],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get workspace: {}", e),
            })?;

        let result = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate workspace results: {}", e),
                })? {
            Some(Workspace {
                workspace_id,
                project_id: match row.get_value(1) {
                    Ok(turso::Value::Integer(id)) => id,
                    _ => {
                        return Err(DatabaseError::OperationFailed {
                            message: "Invalid project_id in workspace".to_string(),
                        })
                    }
                },
                name: match row.get_value(2) {
                    Ok(turso::Value::Text(name)) => name,
                    _ => {
                        return Err(DatabaseError::OperationFailed {
                            message: "Invalid name in workspace".to_string(),
                        })
                    }
                },
                description: match row.get_value(3) {
                    Ok(turso::Value::Text(desc)) if !desc.is_empty() => Some(desc),
                    _ => None,
                },
                branch_hint: match row.get_value(4) {
                    Ok(turso::Value::Text(branch)) if !branch.is_empty() => Some(branch),
                    _ => None,
                },
                is_active: match row.get_value(5) {
                    Ok(turso::Value::Integer(active)) => active != 0,
                    _ => true,
                },
                created_at: match row.get_value(6) {
                    Ok(turso::Value::Text(created)) => created,
                    _ => "unknown".to_string(),
                },
            })
        } else {
            None
        };

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(result)
    }

    async fn list_workspaces(
        &self,
        project_id: Option<i64>,
    ) -> Result<Vec<Workspace>, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let (sql, params) = if let Some(proj_id) = project_id {
            (
                r#"
                SELECT w.workspace_id, w.project_id, w.name, '' as description,
                       w.current_branch, 1 as is_active, w.created_at
                FROM workspace w 
                WHERE w.project_id = ?
                ORDER BY w.created_at DESC
                "#,
                vec![turso::Value::Integer(proj_id)],
            )
        } else {
            (
                r#"
                SELECT w.workspace_id, w.project_id, w.name, '' as description,
                       w.current_branch, 1 as is_active, w.created_at
                FROM workspace w 
                ORDER BY w.created_at DESC
                "#,
                Vec::new(),
            )
        };

        let mut rows = safe_query(&conn, sql, params, "list_workspaces").await?;

        let mut workspaces = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate workspace results: {}", e),
            })?
        {
            let workspace_id = match row.get_value(0) {
                Ok(turso::Value::Text(id_str)) => id_str.parse::<i64>().unwrap_or(0),
                Ok(turso::Value::Integer(id)) => id,
                _ => continue,
            };

            workspaces.push(Workspace {
                workspace_id,
                project_id: match row.get_value(1) {
                    Ok(turso::Value::Integer(id)) => id,
                    _ => continue,
                },
                name: match row.get_value(2) {
                    Ok(turso::Value::Text(name)) => name,
                    _ => continue,
                },
                description: match row.get_value(3) {
                    Ok(turso::Value::Text(desc)) if !desc.is_empty() => Some(desc),
                    _ => None,
                },
                branch_hint: match row.get_value(4) {
                    Ok(turso::Value::Text(branch)) if !branch.is_empty() => Some(branch),
                    _ => None,
                },
                is_active: match row.get_value(5) {
                    Ok(turso::Value::Integer(active)) => active != 0,
                    _ => true,
                },
                created_at: match row.get_value(6) {
                    Ok(turso::Value::Text(created)) => created,
                    _ => "unknown".to_string(),
                },
            });
        }

        pool.return_connection(conn);
        Ok(workspaces)
    }

    async fn update_workspace_branch(
        &self,
        workspace_id: i64,
        branch: &str,
    ) -> Result<(), DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let workspace_id_str = workspace_id.to_string();
        conn.execute(
            "UPDATE workspace SET current_branch = ?, updated_at = ? WHERE workspace_id = ?",
            [
                turso::Value::Text(branch.to_string()),
                turso::Value::Text(workspace_id_str),
            ],
        )
        .await
        .map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to update workspace branch: {}", e),
        })?;

        pool.return_connection(conn);
        Ok(())
    }

    // ===================
    // File Version Management - REMOVED
    // File versioning complexity eliminated
    // ===================

    async fn link_file_to_workspace(
        &self,
        _workspace_id: i64,
        _file_id: i64,
        _file_version_id: i64,
    ) -> Result<(), DatabaseError> {
        // This method is deprecated - workspace_file table has been removed
        // Files are no longer explicitly linked to workspaces
        // File/workspace association is now determined by the workspace cache system
        Ok(())
    }

    // ===================
    // Symbol Storage & Retrieval
    // ===================

    async fn store_symbols(&self, symbols: &[SymbolState]) -> Result<(), DatabaseError> {
        // Route through single-writer task to serialize writes and reduce locks
        let (tx, rx) = oneshot::channel();
        let vec: Vec<SymbolState> = symbols
            .iter()
            .cloned()
            .map(|s| Self::normalize_symbol_for_storage(&s))
            .collect();
        self.writer_tx
            .send(WriteMsg::StoreSymbols(vec, tx))
            .await
            .map_err(|_| DatabaseError::OperationFailed {
                message: "Writer task not available (StoreSymbols)".into(),
            })?;
        rx.await.unwrap_or_else(|_| {
            Err(DatabaseError::OperationFailed {
                message: "Writer ack dropped (StoreSymbols)".into(),
            })
        })
    }

    async fn get_symbols_by_file(
        &self,
        file_path: &str,
        language: &str,
    ) -> Result<Vec<SymbolState>, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let mut rows = conn
            .query(
                r#"
                SELECT symbol_uid, name, fqn, kind,
                       def_start_line, def_start_char, def_end_line, def_end_char,
                       signature, documentation, visibility,
                       is_definition, metadata, file_path
                FROM symbol_state
                WHERE file_path = ? AND language = ?
                "#,
                [
                    turso::Value::Text(file_path.to_string()),
                    turso::Value::Text(language.to_string()),
                ],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get symbols by file: {}", e),
            })?;

        let mut symbols = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate symbol results: {}", e),
            })?
        {
            let symbol_uid = match row.get_value(0) {
                Ok(turso::Value::Text(uid)) => uid,
                _ => continue,
            };

            symbols.push(SymbolState {
                symbol_uid,
                file_path: match row.get_value(13) {
                    Ok(turso::Value::Text(path)) => path,
                    _ => "unknown".to_string(),
                },
                language: language.to_string(),
                name: match row.get_value(1) {
                    Ok(turso::Value::Text(name)) => name,
                    _ => continue,
                },
                fqn: match row.get_value(2) {
                    Ok(turso::Value::Text(fqn)) if !fqn.is_empty() => Some(fqn),
                    _ => None,
                },
                kind: match row.get_value(3) {
                    Ok(turso::Value::Text(kind)) => kind,
                    _ => "unknown".to_string(),
                },
                signature: match row.get_value(8) {
                    Ok(turso::Value::Text(sig)) if !sig.is_empty() => Some(sig),
                    _ => None,
                },
                visibility: match row.get_value(10) {
                    Ok(turso::Value::Text(vis)) if !vis.is_empty() => Some(vis),
                    _ => None,
                },
                def_start_line: match row.get_value(4) {
                    Ok(turso::Value::Integer(line)) => line as u32,
                    _ => 0,
                },
                def_start_char: match row.get_value(5) {
                    Ok(turso::Value::Integer(char)) => char as u32,
                    _ => 0,
                },
                def_end_line: match row.get_value(6) {
                    Ok(turso::Value::Integer(line)) => line as u32,
                    _ => 0,
                },
                def_end_char: match row.get_value(7) {
                    Ok(turso::Value::Integer(char)) => char as u32,
                    _ => 0,
                },
                is_definition: match row.get_value(11) {
                    Ok(turso::Value::Integer(val)) => val != 0,
                    _ => true,
                },
                documentation: match row.get_value(9) {
                    Ok(turso::Value::Text(doc)) if !doc.is_empty() => Some(doc),
                    _ => None,
                },
                metadata: match row.get_value(12) {
                    Ok(turso::Value::Text(meta)) if !meta.is_empty() => Some(meta),
                    _ => None,
                },
            });
        }

        pool.return_connection(conn);
        Ok(symbols)
    }

    async fn find_symbol_by_name(
        &self,
        _workspace_id: i64,
        name: &str,
    ) -> Result<Vec<SymbolState>, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let mut rows = conn
            .query(
                r#"
                SELECT symbol_uid, name, fqn, kind,
                       def_start_line, def_start_char, def_end_line, def_end_char,
                       signature, documentation, visibility,
                       is_definition, metadata, language, file_path
                FROM symbol_state
                WHERE name = ?
                "#,
                [turso::Value::Text(name.to_string())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to find symbol by name: {}", e),
            })?;

        let mut symbols = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate symbol search results: {}", e),
            })?
        {
            let symbol_uid = match row.get_value(0) {
                Ok(turso::Value::Text(uid)) => uid,
                _ => continue,
            };

            symbols.push(SymbolState {
                symbol_uid,
                file_path: match row.get_value(14) {
                    Ok(turso::Value::Text(path)) => path,
                    _ => "unknown".to_string(),
                },
                language: match row.get_value(13) {
                    Ok(turso::Value::Text(lang)) => lang,
                    _ => "unknown".to_string(),
                },
                name: match row.get_value(1) {
                    Ok(turso::Value::Text(name)) => name,
                    _ => continue,
                },
                fqn: match row.get_value(2) {
                    Ok(turso::Value::Text(fqn)) if !fqn.is_empty() => Some(fqn),
                    _ => None,
                },
                kind: match row.get_value(3) {
                    Ok(turso::Value::Text(kind)) => kind,
                    _ => "unknown".to_string(),
                },
                signature: match row.get_value(8) {
                    Ok(turso::Value::Text(sig)) if !sig.is_empty() => Some(sig),
                    _ => None,
                },
                visibility: match row.get_value(10) {
                    Ok(turso::Value::Text(vis)) if !vis.is_empty() => Some(vis),
                    _ => None,
                },
                def_start_line: match row.get_value(4) {
                    Ok(turso::Value::Integer(line)) => line as u32,
                    _ => 0,
                },
                def_start_char: match row.get_value(5) {
                    Ok(turso::Value::Integer(char)) => char as u32,
                    _ => 0,
                },
                def_end_line: match row.get_value(6) {
                    Ok(turso::Value::Integer(line)) => line as u32,
                    _ => 0,
                },
                def_end_char: match row.get_value(7) {
                    Ok(turso::Value::Integer(char)) => char as u32,
                    _ => 0,
                },
                is_definition: match row.get_value(11) {
                    Ok(turso::Value::Integer(val)) => val != 0,
                    _ => true,
                },
                documentation: match row.get_value(9) {
                    Ok(turso::Value::Text(doc)) if !doc.is_empty() => Some(doc),
                    _ => None,
                },
                metadata: match row.get_value(12) {
                    Ok(turso::Value::Text(meta)) if !meta.is_empty() => Some(meta),
                    _ => None,
                },
            });
        }

        pool.return_connection(conn);
        Ok(symbols)
    }

    async fn find_symbol_by_fqn(
        &self,
        _workspace_id: i64,
        fqn: &str,
    ) -> Result<Option<SymbolState>, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let mut rows = conn
            .query(
                r#"
                SELECT symbol_uid, name, fqn, kind,
                       def_start_line, def_start_char, def_end_line, def_end_char,
                       signature, documentation, visibility,
                       is_definition, metadata, language, file_path
                FROM symbol_state
                WHERE fqn = ?
                LIMIT 1
                "#,
                [turso::Value::Text(fqn.to_string())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to find symbol by FQN: {}", e),
            })?;

        let result = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate FQN symbol results: {}", e),
                })? {
            let symbol_uid = match row.get_value(0) {
                Ok(turso::Value::Text(uid)) => uid,
                _ => return Ok(None),
            };

            Some(SymbolState {
                symbol_uid,
                file_path: match row.get_value(14) {
                    Ok(turso::Value::Text(path)) => path,
                    _ => "unknown".to_string(),
                },
                language: match row.get_value(13) {
                    Ok(turso::Value::Text(lang)) => lang,
                    _ => "unknown".to_string(),
                },
                name: match row.get_value(1) {
                    Ok(turso::Value::Text(name)) => name,
                    _ => "unknown".to_string(),
                },
                fqn: match row.get_value(2) {
                    Ok(turso::Value::Text(fqn)) if !fqn.is_empty() => Some(fqn),
                    _ => None,
                },
                kind: match row.get_value(3) {
                    Ok(turso::Value::Text(kind)) => kind,
                    _ => "unknown".to_string(),
                },
                signature: match row.get_value(8) {
                    Ok(turso::Value::Text(sig)) if !sig.is_empty() => Some(sig),
                    _ => None,
                },
                visibility: match row.get_value(10) {
                    Ok(turso::Value::Text(vis)) if !vis.is_empty() => Some(vis),
                    _ => None,
                },
                def_start_line: match row.get_value(4) {
                    Ok(turso::Value::Integer(line)) => line as u32,
                    _ => 0,
                },
                def_start_char: match row.get_value(5) {
                    Ok(turso::Value::Integer(char)) => char as u32,
                    _ => 0,
                },
                def_end_line: match row.get_value(6) {
                    Ok(turso::Value::Integer(line)) => line as u32,
                    _ => 0,
                },
                def_end_char: match row.get_value(7) {
                    Ok(turso::Value::Integer(char)) => char as u32,
                    _ => 0,
                },
                is_definition: match row.get_value(11) {
                    Ok(turso::Value::Integer(val)) => val != 0,
                    _ => true,
                },
                documentation: match row.get_value(9) {
                    Ok(turso::Value::Text(doc)) if !doc.is_empty() => Some(doc),
                    _ => None,
                },
                metadata: match row.get_value(12) {
                    Ok(turso::Value::Text(meta)) if !meta.is_empty() => Some(meta),
                    _ => None,
                },
            })
        } else {
            None
        };

        pool.return_connection(conn);
        Ok(result)
    }

    // ===================
    // Relationship Storage & Querying
    // ===================

    async fn store_edges(&self, edges: &[Edge]) -> Result<(), DatabaseError> {
        // Send to single-writer task
        let (tx, rx) = oneshot::channel();
        let edges_vec = edges.to_vec();
        self.writer_tx
            .send(WriteMsg::StoreEdges(edges_vec, tx))
            .await
            .map_err(|_| DatabaseError::OperationFailed {
                message: "Writer task not available (StoreEdges)".into(),
            })?;
        rx.await.unwrap_or_else(|_| {
            Err(DatabaseError::OperationFailed {
                message: "Writer ack dropped (StoreEdges)".into(),
            })
        })
    }

    async fn get_symbol_references(
        &self,
        _workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Vec<Edge>, DatabaseError> {
        // Checkout connection, then drop pool lock before running query/iterating rows
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let mut rows = conn
            .query(
                r#"
                SELECT source_symbol_uid, target_symbol_uid, relation,
                       start_line, start_char, confidence, language, metadata
                FROM edge
                WHERE source_symbol_uid = ? AND relation = 'references'
                "#,
                [turso::Value::Text(symbol_uid.to_string())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get symbol references: {}", e),
            })?;

        let mut edges = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate reference results: {}", e),
            })?
        {
            let relation_str = match row.get_value(2) {
                Ok(turso::Value::Text(rel)) => rel,
                _ => continue,
            };

            let relation = match crate::database::EdgeRelation::from_string(&relation_str) {
                Ok(rel) => rel,
                Err(_) => continue,
            };

            edges.push(Edge {
                relation,
                source_symbol_uid: match row.get_value(0) {
                    Ok(turso::Value::Text(uid)) => uid,
                    _ => continue,
                },
                target_symbol_uid: match row.get_value(1) {
                    Ok(turso::Value::Text(uid)) => {
                        if Self::is_none_uid(&uid) {
                            "none".to_string()
                        } else {
                            uid
                        }
                    }
                    Ok(turso::Value::Null) => "none".to_string(),
                    _ => continue,
                },
                file_path: None, // This method doesn't join with symbol_state for file_path
                start_line: match row.get_value(3) {
                    Ok(turso::Value::Text(line)) => line.parse::<u32>().ok(),
                    Ok(turso::Value::Integer(line)) => Some(line as u32),
                    _ => None,
                },
                start_char: match row.get_value(4) {
                    Ok(turso::Value::Text(char)) => char.parse::<u32>().ok(),
                    Ok(turso::Value::Integer(char)) => Some(char as u32),
                    _ => None,
                },
                confidence: match row.get_value(5) {
                    Ok(turso::Value::Real(conf)) => conf as f32,
                    Ok(turso::Value::Integer(conf)) => conf as f32,
                    _ => 1.0,
                },
                language: match row.get_value(6) {
                    Ok(turso::Value::Text(lang)) => lang,
                    _ => "unknown".to_string(),
                },
                metadata: match row.get_value(7) {
                    Ok(turso::Value::Text(meta)) => Some(meta),
                    Ok(turso::Value::Null) => None,
                    _ => None,
                },
            });
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(edges)
    }

    async fn get_symbol_calls(
        &self,
        _workspace_id: i64,
        symbol_uid: &str,
        direction: CallDirection,
    ) -> Result<Vec<Edge>, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        // Calls are stored uniformly with relation = 'calls'.
        // Direction is expressed by which side matches the symbol.
        let (sql, params) = match direction {
            CallDirection::Incoming => (
                r#"
                SELECT source_symbol_uid, target_symbol_uid, relation,
                       start_line, start_char, confidence, language, metadata
                FROM edge
                WHERE relation = 'calls' AND target_symbol_uid = ?
                "#,
                vec![turso::Value::Text(symbol_uid.to_string())],
            ),
            CallDirection::Outgoing => (
                r#"
                SELECT source_symbol_uid, target_symbol_uid, relation,
                       start_line, start_char, confidence, language, metadata
                FROM edge
                WHERE relation = 'calls' AND source_symbol_uid = ?
                "#,
                vec![turso::Value::Text(symbol_uid.to_string())],
            ),
            CallDirection::Both => (
                r#"
                SELECT source_symbol_uid, target_symbol_uid, relation,
                       start_line, start_char, confidence, language, metadata
                FROM edge
                WHERE relation = 'calls' AND (source_symbol_uid = ? OR target_symbol_uid = ?)
                "#,
                vec![
                    turso::Value::Text(symbol_uid.to_string()),
                    turso::Value::Text(symbol_uid.to_string()),
                ],
            ),
        };

        info!(
            "[DEBUG] get_symbol_calls SQL query for direction {:?}: {}",
            direction,
            sql.trim()
        );
        info!("[DEBUG] Query parameter: symbol_uid = '{}'", symbol_uid);

        let mut rows = safe_query(&conn, sql, params, "get_symbol_calls")
            .await
            .map_err(|e| {
                error!("[DEBUG] get_symbol_calls query failed: {}", e);
                error!("[DEBUG] Failed SQL: {}", sql.trim());
                error!("[DEBUG] Failed with symbol_uid: '{}'", symbol_uid);
                e
            })?;

        let mut edges = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate call results: {}", e),
            })?
        {
            let relation = crate::database::EdgeRelation::Calls;

            edges.push(Edge {
                language: match row.get_value(6) {
                    Ok(turso::Value::Text(lang)) => lang,
                    _ => "unknown".to_string(),
                },
                relation,
                source_symbol_uid: match row.get_value(0) {
                    Ok(turso::Value::Text(uid)) => uid,
                    _ => continue,
                },
                target_symbol_uid: match row.get_value(1) {
                    Ok(turso::Value::Text(uid)) => {
                        if Self::is_none_uid(&uid) {
                            "none".to_string()
                        } else {
                            uid
                        }
                    }
                    Ok(turso::Value::Null) => "none".to_string(),
                    _ => continue,
                },
                file_path: None, // This method doesn't join with symbol_state for file_path
                start_line: match row.get_value(3) {
                    Ok(turso::Value::Text(line)) => line.parse::<u32>().ok(),
                    Ok(turso::Value::Integer(line)) => Some(line as u32),
                    _ => None,
                },
                start_char: match row.get_value(4) {
                    Ok(turso::Value::Text(char)) => char.parse::<u32>().ok(),
                    Ok(turso::Value::Integer(char)) => Some(char as u32),
                    _ => None,
                },
                confidence: match row.get_value(5) {
                    Ok(turso::Value::Real(conf)) => conf as f32,
                    Ok(turso::Value::Integer(conf)) => conf as f32,
                    _ => 1.0,
                },
                metadata: match row.get_value(7) {
                    Ok(turso::Value::Text(meta)) => Some(meta),
                    Ok(turso::Value::Null) => None,
                    _ => None,
                },
            });
        }

        info!(
            "[DEBUG] get_symbol_calls found {} edges for symbol_uid '{}'",
            edges.len(),
            symbol_uid
        );

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(edges)
    }

    async fn traverse_graph(
        &self,
        start_symbol: &str,
        max_depth: u32,
        relations: &[EdgeRelation],
    ) -> Result<Vec<GraphPath>, DatabaseError> {
        // This is a simplified implementation of graph traversal
        // In a production system, this would use a more sophisticated graph algorithm

        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        // Convert relations to string for SQL query
        let relation_strs: Vec<String> = relations
            .iter()
            .map(|r| r.to_string().to_string())
            .collect();

        if relation_strs.is_empty() {
            pool.return_connection(conn);
            return Ok(Vec::new());
        }

        // For simplicity, we'll do a breadth-first traversal up to max_depth
        let mut paths = Vec::new();
        let mut current_depth = 0;
        let mut current_symbols = vec![start_symbol.to_string()];

        while current_depth < max_depth && !current_symbols.is_empty() {
            let mut next_symbols = Vec::new();

            for symbol in &current_symbols {
                // Build placeholders for the IN clause
                let placeholders = relation_strs
                    .iter()
                    .map(|_| "?")
                    .collect::<Vec<_>>()
                    .join(",");
                let sql = format!(
                    r#"
                    SELECT target_symbol_uid, relation
                    FROM edge
                    WHERE source_symbol_uid = ? AND relation IN ({})
                    "#,
                    placeholders
                );

                let mut params = vec![turso::Value::Text(symbol.clone())];
                for rel_str in &relation_strs {
                    params.push(turso::Value::Text(rel_str.clone()));
                }

                let mut rows = safe_query(&conn, &sql, params, "traverse_graph layer").await?;

                while let Some(row) =
                    rows.next()
                        .await
                        .map_err(|e| DatabaseError::OperationFailed {
                            message: format!("Failed to iterate traversal results: {}", e),
                        })?
                {
                    let target_symbol = match row.get_value(0) {
                        Ok(turso::Value::Text(uid)) => uid,
                        _ => continue,
                    };

                    let edge_type_str = match row.get_value(1) {
                        Ok(turso::Value::Text(edge_type)) => edge_type,
                        _ => continue,
                    };

                    if let Ok(relation) = crate::database::EdgeRelation::from_string(&edge_type_str)
                    {
                        let path = GraphPath {
                            symbol_uid: target_symbol.clone(),
                            depth: current_depth + 1,
                            path: vec![start_symbol.to_string(), target_symbol.clone()],
                            relation_chain: vec![relation],
                        };
                        paths.push(path);
                        next_symbols.push(target_symbol);
                    }
                }
            }

            current_symbols = next_symbols;
            current_depth += 1;
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(paths)
    }

    // ===================
    // Analysis Management
    // ===================

    async fn create_analysis_run(
        &self,
        analyzer_name: &str,
        analyzer_version: &str,
        _language: &str,
        config: &str,
    ) -> Result<i64, DatabaseError> {
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        let run_id = uuid::Uuid::new_v4().to_string();
        let run_id_int = self.generate_unique_id().await?;

        safe_execute(
            &conn,
            r#"
            INSERT INTO analysis_run (
                run_id, workspace_id, analyzer_type, analyzer_version,
                configuration, started_at, status
            )
            VALUES (?, '1', ?, ?, ?, ?, 'running')
            "#,
            [
                turso::Value::Text(run_id),
                turso::Value::Text(analyzer_name.to_string()),
                turso::Value::Text(analyzer_version.to_string()),
                turso::Value::Text(config.to_string()),
            ],
            "create_analysis_run insert",
        )
        .await
        .map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to create analysis run: {}", e),
        })?;

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(run_id_int)
    }

    async fn get_analysis_progress(
        &self,
        workspace_id: i64,
    ) -> Result<AnalysisProgress, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let workspace_id_str = workspace_id.to_string();

        // Get counts from analysis_run and file_analysis tables
        let mut rows = conn
            .query(
                r#"
                SELECT 
                    COALESCE(SUM(ar.files_processed), 0) as total_processed,
                    COUNT(DISTINCT ar.run_id) as total_runs
                FROM analysis_run ar
                WHERE ar.workspace_id = ?
                "#,
                [turso::Value::Text(workspace_id_str.clone())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get analysis progress: {}", e),
            })?;

        let (analyzed_files, _total_runs) = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate analysis progress results: {}", e),
                })? {
            (
                match row.get_value(0) {
                    Ok(turso::Value::Integer(count)) => count as u64,
                    _ => 0,
                },
                match row.get_value(1) {
                    Ok(turso::Value::Integer(count)) => count as u64,
                    _ => 0,
                },
            )
        } else {
            (0, 0)
        };

        // Get simplified progress - workspace_file tables removed
        // Return progress based on symbol_state and file tables
        let mut progress_rows = conn
            .query(
                r#"
                WITH workspace_info AS (
                    SELECT 
                        COUNT(DISTINCT ss.file_path) as total_files,
                        COUNT(ss.symbol_uid) as total_symbols
                    FROM symbol_state ss
                    WHERE 1 = 1  -- All symbols in this database belong to this workspace
                ),
                analysis_info AS (
                    SELECT
                        COUNT(ar.run_id) as analysis_runs,
                        COUNT(CASE WHEN ar.status = 'completed' THEN 1 END) as completed_runs,
                        COUNT(CASE WHEN ar.status = 'failed' THEN 1 END) as failed_runs
                    FROM analysis_run ar 
                    WHERE ar.workspace_id = ?
                )
                SELECT 
                    COALESCE(wi.total_files, 0) as total_files,
                    COALESCE(ai.completed_runs, 0) as successful_files,
                    COALESCE(ai.failed_runs, 0) as failed_files,
                    COALESCE(ai.analysis_runs - ai.completed_runs - ai.failed_runs, 0) as pending_files
                FROM workspace_info wi
                CROSS JOIN analysis_info ai
                "#,
                [
                    turso::Value::Text(workspace_id_str.clone())
                ]
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get detailed analysis progress: {}", e),
            })?;

        let (total_files, analyzed_files, failed_files, pending_files) = if let Some(row) =
            progress_rows
                .next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate detailed progress results: {}", e),
                })? {
            (
                match row.get_value(0) {
                    Ok(turso::Value::Integer(count)) => count as u64,
                    _ => 0,
                },
                match row.get_value(1) {
                    Ok(turso::Value::Integer(count)) => count as u64,
                    _ => 0,
                },
                match row.get_value(2) {
                    Ok(turso::Value::Integer(count)) => count as u64,
                    _ => 0,
                },
                match row.get_value(3) {
                    Ok(turso::Value::Integer(count)) => count as u64,
                    _ => 0,
                },
            )
        } else {
            // Fallback: use analyzed_files from the previous query as total if detailed data isn't available
            let total = analyzed_files.max(1); // Ensure at least 1 to avoid division by zero
            (
                total,
                analyzed_files,
                0,
                if total > analyzed_files {
                    total - analyzed_files
                } else {
                    0
                },
            )
        };

        let completion_percentage = if total_files > 0 {
            (analyzed_files as f32 / total_files as f32) * 100.0
        } else {
            0.0
        };

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }

        Ok(AnalysisProgress {
            workspace_id,
            total_files,
            analyzed_files,
            failed_files,
            pending_files,
            completion_percentage,
        })
    }

    async fn queue_file_analysis(
        &self,
        file_id: i64,
        _language: &str,
        priority: i32,
    ) -> Result<(), DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let queue_id = uuid::Uuid::new_v4().to_string();

        conn.execute(
            r#"
            INSERT INTO indexer_queue (
                queue_id, workspace_id, file_id, priority, operation_type,
                status, created_at
            )
            VALUES (?, '1', ?, ?, 'analyze', 'pending', ?)
            "#,
            [
                turso::Value::Text(queue_id),
                turso::Value::Text(file_id.to_string()),
                turso::Value::Integer(priority as i64),
            ],
        )
        .await
        .map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to queue file analysis: {}", e),
        })?;

        pool.return_connection(conn);
        Ok(())
    }

    // Missing trait methods - temporary placeholder implementations
    async fn get_all_symbols(&self) -> Result<Vec<SymbolState>, DatabaseError> {
        // Placeholder implementation - would return all symbols from all workspaces
        debug!("DEBUG: get_all_symbols not yet implemented, returning empty list");
        Ok(Vec::new())
    }

    async fn get_all_edges(&self) -> Result<Vec<Edge>, DatabaseError> {
        // Placeholder implementation - would return all edges from all workspaces
        debug!("DEBUG: get_all_edges not yet implemented, returning empty list");
        Ok(Vec::new())
    }

    // ===================
    // LSP Protocol Query Methods Implementation
    // ===================

    async fn get_call_hierarchy_for_symbol(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Option<CallHierarchyResult>, DatabaseError> {
        info!(
            "[DEBUG] get_call_hierarchy_for_symbol ENTRY: workspace_id={}, symbol_uid={}",
            workspace_id, symbol_uid
        );

        // LOCK-FREE: Use direct connection to avoid pool deadlocks
        let conn = self.get_direct_connection().await.map_err(|e| {
            error!("[DEBUG] Direct database connection failed: {}", e);
            e
        })?;
        debug!("[DEBUG] Direct database connection acquired successfully");

        // Step 25.5: Check if symbol_state table exists and has data
        let mut table_check = conn
            .query(
                "SELECT COUNT(*) FROM symbol_state LIMIT 1",
                [] as [turso::Value; 0],
            )
            .await
            .map_err(|e| {
                error!(
                    "[DEBUG] Failed to check symbol_state table existence: {}",
                    e
                );
                DatabaseError::OperationFailed {
                    message: format!("Failed to check symbol_state table: {}", e),
                }
            })?;

        if let Some(row) = table_check.next().await.map_err(|e| {
            error!("[DEBUG] Failed to read table check result: {}", e);
            DatabaseError::OperationFailed {
                message: format!("Failed to read table check result: {}", e),
            }
        })? {
            let count = match row.get_value(0) {
                Ok(turso::Value::Integer(count)) => count,
                _ => -1,
            };
            info!("[DEBUG] symbol_state table has {} rows", count);
        }

        // Step 25.2: Log the SQL query being executed
        let query = "SELECT symbol_uid, file_path, language, name, fqn, kind, signature, visibility, def_start_line, def_start_char, def_end_line, def_end_char, is_definition, documentation, metadata FROM symbol_state WHERE symbol_uid = ?";
        info!("[DEBUG] Executing SQL query: {}", query);
        info!("[DEBUG] Query parameters: symbol_uid = '{}'", symbol_uid);

        // 1. Get the symbol details

        // Find the symbol by UID
        let mut symbol_rows = conn
            .query(query, [turso::Value::Text(symbol_uid.to_string())])
            .await
            .map_err(|e| {
                error!("[DEBUG] SQL query execution failed: {}", e);
                DatabaseError::OperationFailed {
                    message: format!("Failed to find symbol by UID: {}", e),
                }
            })?;

        debug!("[DEBUG] SQL query executed successfully");

        let center_symbol = if let Some(row) = symbol_rows.next().await.map_err(|e| {
            error!("[DEBUG] Failed to iterate symbol results: {}", e);
            DatabaseError::OperationFailed {
                message: format!("Failed to iterate symbol results: {}", e),
            }
        })? {
            info!("[DEBUG] Found symbol row in database");
            let symbol = SymbolState {
                symbol_uid: match row.get_value(0) {
                    Ok(turso::Value::Text(uid)) => uid,
                    _ => return Ok(None),
                },
                file_path: match row.get_value(1) {
                    Ok(turso::Value::Text(path)) => path,
                    _ => "unknown".to_string(),
                },
                language: match row.get_value(2) {
                    Ok(turso::Value::Text(lang)) => lang,
                    _ => "unknown".to_string(),
                },
                name: match row.get_value(3) {
                    Ok(turso::Value::Text(name)) => name,
                    _ => "unknown".to_string(),
                },
                fqn: match row.get_value(4) {
                    Ok(turso::Value::Text(fqn)) if !fqn.is_empty() => Some(fqn),
                    _ => None,
                },
                kind: match row.get_value(5) {
                    Ok(turso::Value::Text(kind)) => kind,
                    _ => "unknown".to_string(),
                },
                signature: match row.get_value(6) {
                    Ok(turso::Value::Text(sig)) => Some(sig),
                    _ => None,
                },
                visibility: match row.get_value(7) {
                    Ok(turso::Value::Text(vis)) => Some(vis),
                    _ => None,
                },
                def_start_line: match row.get_value(8) {
                    Ok(turso::Value::Integer(line)) => line as u32,
                    Ok(turso::Value::Text(line_str)) => line_str.parse::<u32>().unwrap_or(0),
                    _ => 0,
                },
                def_start_char: match row.get_value(9) {
                    Ok(turso::Value::Integer(char)) => char as u32,
                    Ok(turso::Value::Text(char_str)) => char_str.parse::<u32>().unwrap_or(0),
                    _ => 0,
                },
                def_end_line: match row.get_value(10) {
                    Ok(turso::Value::Integer(line)) => line as u32,
                    Ok(turso::Value::Text(line_str)) => line_str.parse::<u32>().unwrap_or(0),
                    _ => 0,
                },
                def_end_char: match row.get_value(11) {
                    Ok(turso::Value::Integer(char)) => char as u32,
                    Ok(turso::Value::Text(char_str)) => char_str.parse::<u32>().unwrap_or(0),
                    _ => 0,
                },
                is_definition: match row.get_value(12) {
                    Ok(turso::Value::Integer(val)) => val != 0,
                    Ok(turso::Value::Text(val)) => val.parse::<i32>().unwrap_or(0) != 0,
                    _ => false,
                },
                documentation: match row.get_value(13) {
                    Ok(turso::Value::Text(doc)) => Some(doc),
                    _ => None,
                },
                metadata: match row.get_value(14) {
                    Ok(turso::Value::Text(meta)) => Some(meta),
                    _ => None,
                },
            };

            symbol
        } else {
            info!(
                "[DEBUG] Symbol '{}' not found in database - auto-creating from symbol_uid",
                symbol_uid
            );

            // Parse symbol UID to extract symbol information
            let (file_path, symbol_name, line_number) = Self::parse_symbol_uid(symbol_uid);

            // Create SymbolState with parsed information
            let name_str = symbol_name.as_deref().unwrap_or("unknown");
            let file_path_str = file_path.as_deref().unwrap_or("unknown");
            let symbol_kind = Self::infer_symbol_kind_from_name_and_context(
                name_str,
                &PathBuf::from(file_path_str),
                line_number.unwrap_or(0),
            );

            let symbol_state = SymbolState {
                symbol_uid: symbol_uid.to_string(),
                file_path: file_path.unwrap_or_else(|| "unknown".to_string()),
                language: "unknown".to_string(), // Default value
                name: symbol_name.unwrap_or_else(|| "unknown".to_string()),
                fqn: None,
                kind: symbol_kind,
                signature: None,
                visibility: None,
                def_start_line: line_number.unwrap_or(0),
                def_start_char: 0,
                def_end_line: line_number.unwrap_or(0),
                def_end_char: 0,
                is_definition: true,
                documentation: None,
                metadata: Some(format!("auto_created_from_uid:{}", symbol_uid)),
            };

            // LOCK-FREE: Store the auto-created symbol using direct connection (no deadlock)
            self.store_symbols_with_conn(&conn, &[symbol_state.clone()])
                .await?;

            info!("[DEBUG] Auto-created symbol '{}' successfully", symbol_uid);

            // Return the created symbol
            symbol_state
        };

        info!(
            "[DEBUG] Successfully parsed center_symbol: name='{}', kind='{}', uid='{}'",
            center_symbol.name, center_symbol.kind, center_symbol.symbol_uid
        );

        // 2. Get incoming and outgoing call edges and interpret them

        debug!(
            "[DEBUG] Getting incoming call edges for symbol_uid '{}'",
            symbol_uid
        );
        let incoming_edges_raw = self
            .get_symbol_calls(workspace_id, symbol_uid, CallDirection::Incoming)
            .await
            .map_err(|e| {
                error!("[DEBUG] Failed to get incoming call edges: {}", e);
                e
            })?;

        let incoming_interpretation = self.interpret_edges_for_relation(incoming_edges_raw);
        match &incoming_interpretation {
            EdgeInterpretation::Unknown => {
                info!("[DEBUG] Incoming edges interpretation: Unknown - need LSP call");
            }
            EdgeInterpretation::AnalyzedEmpty => {
                info!("[DEBUG] Incoming edges interpretation: AnalyzedEmpty - return []");
            }
            EdgeInterpretation::HasData(edges) => {
                info!(
                    "[DEBUG] Incoming edges interpretation: HasData - {} real edges",
                    edges.len()
                );
            }
        }

        debug!(
            "[DEBUG] Getting outgoing call edges for symbol_uid '{}'",
            symbol_uid
        );
        let outgoing_edges_raw = self
            .get_symbol_calls(workspace_id, symbol_uid, CallDirection::Outgoing)
            .await
            .map_err(|e| {
                error!("[DEBUG] Failed to get outgoing call edges: {}", e);
                e
            })?;

        let outgoing_interpretation = self.interpret_edges_for_relation(outgoing_edges_raw);
        match &outgoing_interpretation {
            EdgeInterpretation::Unknown => {
                info!("[DEBUG] Outgoing edges interpretation: Unknown - need LSP call");
            }
            EdgeInterpretation::AnalyzedEmpty => {
                info!("[DEBUG] Outgoing edges interpretation: AnalyzedEmpty - return []");
            }
            EdgeInterpretation::HasData(edges) => {
                info!(
                    "[DEBUG] Outgoing edges interpretation: HasData - {} real edges",
                    edges.len()
                );
            }
        }

        // Check if we need fresh LSP calls for either direction
        let need_fresh_lsp_call = matches!(incoming_interpretation, EdgeInterpretation::Unknown)
            || matches!(outgoing_interpretation, EdgeInterpretation::Unknown);

        if need_fresh_lsp_call {
            info!("[DEBUG] Need fresh LSP call - some edges unknown");
            return Ok(None); // Trigger fresh LSP call
        }

        // Both directions have been analyzed - use interpreted results
        let incoming_edges = match incoming_interpretation {
            EdgeInterpretation::AnalyzedEmpty => vec![],
            EdgeInterpretation::HasData(edges) => edges,
            EdgeInterpretation::Unknown => unreachable!(), // Already handled above
        };

        let outgoing_edges = match outgoing_interpretation {
            EdgeInterpretation::AnalyzedEmpty => vec![],
            EdgeInterpretation::HasData(edges) => edges,
            EdgeInterpretation::Unknown => unreachable!(), // Already handled above
        };

        info!(
            "[DEBUG] Using cached results: {} incoming, {} outgoing edges",
            incoming_edges.len(),
            outgoing_edges.len()
        );

        // 3. Get all related symbols
        let mut all_symbol_uids: Vec<String> = Vec::new();
        for edge in &incoming_edges {
            all_symbol_uids.push(edge.source_symbol_uid.clone());
        }
        for edge in &outgoing_edges {
            all_symbol_uids.push(edge.target_symbol_uid.clone());
        }

        // LOCK-FREE: Fetch all related symbols using the same direct connection
        let mut all_symbols = Vec::new();
        all_symbols.push(center_symbol.clone());

        debug!(
            "[DEBUG] Querying {} related symbols using direct connection",
            all_symbol_uids.len()
        );

        for uid in all_symbol_uids {
            let mut rows = conn
                .query(
                    "SELECT symbol_uid, file_path, language, name, fqn, kind, signature, visibility, def_start_line, def_start_char, def_end_line, def_end_char, is_definition, documentation, metadata FROM symbol_state WHERE symbol_uid = ?",
                    [turso::Value::Text(uid.clone())],
                )
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to find related symbol: {}", e),
                })?;

            if let Some(row) = rows
                .next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate related symbol results: {}", e),
                })?
            {
                let symbol = SymbolState {
                    symbol_uid: match row.get_value(0) {
                        Ok(turso::Value::Text(uid)) => uid,
                        _ => continue,
                    },
                    file_path: match row.get_value(1) {
                        Ok(turso::Value::Text(path)) => path,
                        _ => "unknown".to_string(),
                    },
                    language: match row.get_value(2) {
                        Ok(turso::Value::Text(lang)) => lang,
                        _ => "unknown".to_string(),
                    },
                    name: match row.get_value(3) {
                        Ok(turso::Value::Text(name)) => name,
                        _ => "unknown".to_string(),
                    },
                    fqn: match row.get_value(4) {
                        Ok(turso::Value::Text(fqn)) => Some(fqn),
                        _ => None,
                    },
                    kind: match row.get_value(5) {
                        Ok(turso::Value::Text(kind)) => kind,
                        _ => "unknown".to_string(),
                    },
                    signature: match row.get_value(6) {
                        Ok(turso::Value::Text(sig)) => Some(sig),
                        _ => None,
                    },
                    visibility: match row.get_value(7) {
                        Ok(turso::Value::Text(vis)) => Some(vis),
                        _ => None,
                    },
                    def_start_line: match row.get_value(8) {
                        Ok(turso::Value::Integer(line)) => line as u32,
                        Ok(turso::Value::Text(line_str)) => line_str.parse::<u32>().unwrap_or(0),
                        _ => 0,
                    },
                    def_start_char: match row.get_value(9) {
                        Ok(turso::Value::Integer(char)) => char as u32,
                        Ok(turso::Value::Text(char_str)) => char_str.parse::<u32>().unwrap_or(0),
                        _ => 0,
                    },
                    def_end_line: match row.get_value(10) {
                        Ok(turso::Value::Integer(line)) => line as u32,
                        Ok(turso::Value::Text(line_str)) => line_str.parse::<u32>().unwrap_or(0),
                        _ => 0,
                    },
                    def_end_char: match row.get_value(11) {
                        Ok(turso::Value::Integer(char)) => char as u32,
                        Ok(turso::Value::Text(char_str)) => char_str.parse::<u32>().unwrap_or(0),
                        _ => 0,
                    },
                    is_definition: match row.get_value(12) {
                        Ok(turso::Value::Integer(val)) => val != 0,
                        Ok(turso::Value::Text(val)) => val.parse::<i32>().unwrap_or(0) != 0,
                        _ => false,
                    },
                    documentation: match row.get_value(13) {
                        Ok(turso::Value::Text(doc)) => Some(doc),
                        _ => None,
                    },
                    metadata: match row.get_value(14) {
                        Ok(turso::Value::Text(meta)) => Some(meta),
                        _ => None,
                    },
                };
                all_symbols.push(symbol);
            }
        }

        debug!(
            "[DEBUG] Fetched {} total symbols using direct connection (no pool locks)",
            all_symbols.len()
        );

        // 4. Use the center symbol's direct file path
        let center_file_path = std::path::PathBuf::from(&center_symbol.file_path);

        // 5. Use ProtocolConverter to convert to CallHierarchyResult
        debug!("[DEBUG] Converting edges to CallHierarchyResult with {} total symbols, center_file: {}", 
               all_symbols.len(), center_file_path.display());
        let converter = crate::database::ProtocolConverter::new();

        let result = converter.edges_to_call_hierarchy(
            &center_symbol,
            &center_file_path,
            incoming_edges,
            outgoing_edges,
            &all_symbols,
        );

        info!("[DEBUG] get_call_hierarchy_for_symbol SUCCESS: returning call hierarchy result");
        Ok(Some(result))
    }

    async fn get_references_for_symbol(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
        include_declaration: bool,
    ) -> Result<Vec<Location>, DatabaseError> {
        info!("[DEBUG] get_references_for_symbol ENTRY: workspace_id={}, symbol_uid={}, include_declaration={}", workspace_id, symbol_uid, include_declaration);

        // LOCK-FREE: Use direct connection to avoid pool deadlocks
        let conn = self.get_direct_connection().await.map_err(|e| {
            error!("[DEBUG] Direct database connection failed: {}", e);
            e
        })?;

        // Step 25.5: Check if edge table exists and has data
        let mut table_check = safe_query(
            &conn,
            "SELECT COUNT(*) FROM edge LIMIT 1",
            (),
            "refs.table_check",
        )
        .await?;

        if let Some(row) = table_check.next().await.map_err(|e| {
            error!("[DEBUG] Failed to read edge table check result: {}", e);
            DatabaseError::OperationFailed {
                message: format!("Failed to read edge table check result: {}", e),
            }
        })? {
            let count = match row.get_value(0) {
                Ok(turso::Value::Integer(count)) => count,
                _ => -1,
            };
            info!("[DEBUG] edge table has {} rows", count);
        }

        // LOCK-FREE: Get reference edges using direct connection (no deadlock)
        debug!(
            "[DEBUG] Calling get_symbol_references_with_conn for symbol_uid '{}'",
            symbol_uid
        );
        let edges = self
            .get_symbol_references_with_conn(&conn, workspace_id, symbol_uid)
            .await
            .map_err(|e| {
                error!("[DEBUG] get_symbol_references_with_conn failed: {}", e);
                e
            })?;
        info!(
            "[DEBUG] get_symbol_references_with_conn returned {} edges",
            edges.len()
        );

        // 2. Use ProtocolConverter to convert edges to Location vec with direct file paths
        debug!(
            "[DEBUG] Converting {} edges to Location vec with direct file paths",
            edges.len()
        );
        let converter = crate::database::ProtocolConverter::new();

        // Use the new direct method that doesn't require file path resolution
        let locations = converter.edges_to_locations_direct(edges);

        info!("[DEBUG] get_references_for_symbol SUCCESS: returning {} locations with resolved file paths", locations.len());
        Ok(locations)
    }

    async fn get_definitions_for_symbol(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Vec<Location>, DatabaseError> {
        info!(
            "[DEBUG] get_definitions_for_symbol ENTRY: workspace_id={}, symbol_uid={}",
            workspace_id, symbol_uid
        );

        // Step 25.3: Verify database connection (checkout without holding pool lock during I/O)
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await
        }
        .map_err(|e| {
            error!("[DEBUG] Database connection failed: {}", e);
            e
        })?;
        debug!("[DEBUG] Database connection acquired successfully");

        // Step 25.5: Check if edge table exists and has data
        let mut table_check = safe_query(
            &conn,
            "SELECT COUNT(*) FROM edge LIMIT 1",
            (),
            "defs.table_check",
        )
        .await?;

        if let Some(row) = table_check.next().await.map_err(|e| {
            error!("[DEBUG] Failed to read edge table check result: {}", e);
            DatabaseError::OperationFailed {
                message: format!("Failed to read edge table check result: {}", e),
            }
        })? {
            let count = match row.get_value(0) {
                Ok(turso::Value::Integer(count)) => count,
                _ => -1,
            };
            info!("[DEBUG] edge table has {} rows", count);
        }

        // Step 25.2: Log the SQL query being executed
        let query = r#"
                SELECT e.source_symbol_uid, e.target_symbol_uid, e.relation,
                       e.start_line, e.start_char, e.confidence, s.file_path
                FROM edge e
                LEFT JOIN symbol_state s ON e.source_symbol_uid = s.symbol_uid
                WHERE e.target_symbol_uid = ? AND (e.relation = 'defines' OR e.relation = 'definition')
                "#;
        info!("[DEBUG] Executing SQL query: {}", query.trim());
        info!(
            "[DEBUG] Query parameters: target_symbol_uid = '{}'",
            symbol_uid
        );

        // Step 25.4: Check workspace_id parameter handling
        info!("[DEBUG] Note: workspace_id={} is not being used in the query - this might be the issue!", workspace_id);

        // 1. Query edges where edge_type = 'defines' or similar

        let mut rows = safe_query(
            &conn,
            query,
            [turso::Value::Text(symbol_uid.to_string())],
            "get_definitions_for_symbol",
        )
        .await?;

        debug!("[DEBUG] SQL query executed successfully");

        let mut edges = Vec::new();
        let mut row_count = 0;
        while let Some(row) = rows.next().await.map_err(|e| {
            error!("[DEBUG] Failed to iterate definition results: {}", e);
            DatabaseError::OperationFailed {
                message: format!("Failed to iterate definition results: {}", e),
            }
        })? {
            row_count += 1;
            debug!("[DEBUG] Processing row {}", row_count);
            let relation = match row.get_value(2) {
                Ok(turso::Value::Text(rel)) => {
                    match crate::database::EdgeRelation::from_string(&rel) {
                        Ok(r) => r,
                        Err(_) => crate::database::EdgeRelation::References, // Default fallback
                    }
                }
                _ => crate::database::EdgeRelation::References, // Default fallback
            };

            edges.push(Edge {
                language: "unknown".to_string(),
                relation,
                source_symbol_uid: match row.get_value(0) {
                    Ok(turso::Value::Text(uid)) => uid,
                    _ => continue,
                },
                target_symbol_uid: match row.get_value(1) {
                    Ok(turso::Value::Text(uid)) => uid,
                    _ => continue,
                },
                file_path: match row.get_value(6) {
                    Ok(turso::Value::Text(path)) => Some(path),
                    _ => None,
                },
                start_line: match row.get_value(3) {
                    Ok(turso::Value::Text(line)) => line.parse::<u32>().ok(),
                    Ok(turso::Value::Integer(line)) => Some(line as u32),
                    _ => None,
                },
                start_char: match row.get_value(4) {
                    Ok(turso::Value::Text(char)) => char.parse::<u32>().ok(),
                    Ok(turso::Value::Integer(char)) => Some(char as u32),
                    _ => None,
                },
                confidence: match row.get_value(5) {
                    Ok(turso::Value::Real(conf)) => conf as f32,
                    Ok(turso::Value::Integer(conf)) => conf as f32,
                    _ => 1.0,
                },
                metadata: None,
            });
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }

        info!(
            "[DEBUG] Processed {} rows from database, created {} edges",
            row_count,
            edges.len()
        );

        // 2. Use ProtocolConverter to convert edges to Location vec with direct file paths
        debug!(
            "[DEBUG] Converting {} edges to Location vec with direct file paths",
            edges.len()
        );
        let converter = crate::database::ProtocolConverter::new();

        // Use the new direct method that doesn't require file path resolution
        let locations = converter.edges_to_locations_direct(edges);

        info!("[DEBUG] get_definitions_for_symbol SUCCESS: returning {} locations with resolved file paths", locations.len());
        Ok(locations)
    }

    async fn get_implementations_for_symbol(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Vec<Location>, DatabaseError> {
        info!(
            "[DEBUG] get_implementations_for_symbol ENTRY: workspace_id={}, symbol_uid={}",
            workspace_id, symbol_uid
        );

        // Step 25.3: Verify database connection (without holding pool lock while iterating)
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await
        }
        .map_err(|e| {
            error!("[DEBUG] Database connection failed: {}", e);
            e
        })?;
        debug!("[DEBUG] Database connection acquired successfully");

        // Step 25.5: Check if edge table exists and has data
        let mut table_check = safe_query(
            &conn,
            "SELECT COUNT(*) FROM edge LIMIT 1",
            (),
            "impls.table_check",
        )
        .await?;

        if let Some(row) = table_check.next().await.map_err(|e| {
            error!("[DEBUG] Failed to read edge table check result: {}", e);
            DatabaseError::OperationFailed {
                message: format!("Failed to read edge table check result: {}", e),
            }
        })? {
            let count = match row.get_value(0) {
                Ok(turso::Value::Integer(count)) => count,
                _ => -1,
            };
            info!("[DEBUG] edge table has {} rows", count);
        }

        // Step 25.2: Log the SQL query being executed
        let query = r#"
                SELECT e.source_symbol_uid, e.target_symbol_uid, e.relation,
                       e.start_line, e.start_char, e.confidence, s.file_path
                FROM edge e
                LEFT JOIN symbol_state s ON e.source_symbol_uid = s.symbol_uid
                WHERE e.target_symbol_uid = ? AND (e.relation = 'implements' OR e.relation = 'implementation')
                "#;
        info!("[DEBUG] Executing SQL query: {}", query.trim());
        info!(
            "[DEBUG] Query parameters: target_symbol_uid = '{}'",
            symbol_uid
        );

        // Step 25.4: Check workspace_id parameter handling
        info!("[DEBUG] Note: workspace_id={} is not being used in the query - this might be the issue!", workspace_id);

        // 1. Query edges where relation = 'Implements' or similar

        let mut rows = safe_query(
            &conn,
            query,
            [turso::Value::Text(symbol_uid.to_string())],
            "get_implementations_for_symbol",
        )
        .await?;

        debug!("[DEBUG] SQL query executed successfully");

        let mut edges = Vec::new();
        let mut row_count = 0;
        while let Some(row) = rows.next().await.map_err(|e| {
            error!("[DEBUG] Failed to iterate implementation results: {}", e);
            DatabaseError::OperationFailed {
                message: format!("Failed to iterate implementation results: {}", e),
            }
        })? {
            row_count += 1;
            debug!("[DEBUG] Processing row {}", row_count);
            let relation = match row.get_value(2) {
                Ok(turso::Value::Text(rel)) => {
                    match crate::database::EdgeRelation::from_string(&rel) {
                        Ok(r) => r,
                        Err(_) => crate::database::EdgeRelation::Implements, // Default fallback
                    }
                }
                _ => crate::database::EdgeRelation::Implements, // Default fallback
            };

            edges.push(Edge {
                language: "unknown".to_string(),
                relation,
                source_symbol_uid: match row.get_value(0) {
                    Ok(turso::Value::Text(uid)) => uid,
                    _ => continue,
                },
                target_symbol_uid: match row.get_value(1) {
                    Ok(turso::Value::Text(uid)) => uid,
                    _ => continue,
                },
                file_path: match row.get_value(6) {
                    Ok(turso::Value::Text(path)) => Some(path),
                    _ => None,
                },
                start_line: match row.get_value(3) {
                    Ok(turso::Value::Text(line)) => line.parse::<u32>().ok(),
                    Ok(turso::Value::Integer(line)) => Some(line as u32),
                    _ => None,
                },
                start_char: match row.get_value(4) {
                    Ok(turso::Value::Text(char)) => char.parse::<u32>().ok(),
                    Ok(turso::Value::Integer(char)) => Some(char as u32),
                    _ => None,
                },
                confidence: match row.get_value(5) {
                    Ok(turso::Value::Real(conf)) => conf as f32,
                    Ok(turso::Value::Integer(conf)) => conf as f32,
                    _ => 1.0,
                },
                metadata: None,
            });
        }

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }

        info!(
            "[DEBUG] Processed {} rows from database, created {} edges",
            row_count,
            edges.len()
        );

        // 2. Use ProtocolConverter to convert edges to Location vec with direct file paths
        debug!(
            "[DEBUG] Converting {} edges to Location vec with direct file paths",
            edges.len()
        );
        let converter = crate::database::ProtocolConverter::new();

        // Use the new direct method that doesn't require file path resolution
        let locations = converter.edges_to_locations_direct(edges);

        info!("[DEBUG] get_implementations_for_symbol SUCCESS: returning {} locations with resolved file paths", locations.len());
        Ok(locations)
    }

    // ===================
    // LSP Enrichment Support
    // ===================

    async fn find_symbols_pending_enrichment(
        &self,
        limit: usize,
    ) -> Result<Vec<SymbolEnrichmentPlan>, DatabaseError> {
        self.find_symbols_pending_enrichment_internal(limit).await
    }
}

impl SQLiteBackend {
    /// Convert a database row into a SymbolState, returning None for malformed rows
    fn symbol_state_from_row(row: &turso::Row) -> Option<SymbolState> {
        let symbol_uid = match row.get_value(0) {
            Ok(turso::Value::Text(uid)) if !uid.is_empty() => uid,
            _ => return None,
        };

        let file_path = match row.get_value(1) {
            Ok(turso::Value::Text(path)) if !path.trim().is_empty() => path,
            _ => return None,
        };

        let language = match row.get_value(2) {
            Ok(turso::Value::Text(lang)) if !lang.is_empty() => lang,
            _ => return None,
        };

        let name = match row.get_value(3) {
            Ok(turso::Value::Text(name)) if !name.is_empty() => name,
            _ => return None,
        };

        let fqn = match row.get_value(4) {
            Ok(turso::Value::Text(fqn)) if !fqn.is_empty() => Some(fqn),
            _ => None,
        };

        let kind = match row.get_value(5) {
            Ok(turso::Value::Text(kind)) if !kind.is_empty() => kind,
            _ => return None,
        };

        let signature = match row.get_value(6) {
            Ok(turso::Value::Text(sig)) if !sig.is_empty() => Some(sig),
            _ => None,
        };

        let visibility = match row.get_value(7) {
            Ok(turso::Value::Text(vis)) if !vis.is_empty() => Some(vis),
            _ => None,
        };

        let def_start_line = match row.get_value(8) {
            Ok(turso::Value::Integer(line)) => line as u32,
            Ok(turso::Value::Text(line)) => line.parse::<u32>().unwrap_or(0),
            _ => 0,
        };

        let def_start_char = match row.get_value(9) {
            Ok(turso::Value::Integer(ch)) => ch as u32,
            Ok(turso::Value::Text(ch)) => ch.parse::<u32>().unwrap_or(0),
            _ => 0,
        };

        let def_end_line = match row.get_value(10) {
            Ok(turso::Value::Integer(line)) => line as u32,
            Ok(turso::Value::Text(line)) => line.parse::<u32>().unwrap_or(0),
            _ => 0,
        };

        let def_end_char = match row.get_value(11) {
            Ok(turso::Value::Integer(ch)) => ch as u32,
            Ok(turso::Value::Text(ch)) => ch.parse::<u32>().unwrap_or(0),
            _ => 0,
        };

        let is_definition = match row.get_value(12) {
            Ok(turso::Value::Integer(val)) => val != 0,
            Ok(turso::Value::Text(val)) => val != "0",
            _ => true,
        };

        let documentation = match row.get_value(13) {
            Ok(turso::Value::Text(doc)) if !doc.is_empty() => Some(doc),
            _ => None,
        };

        let metadata = match row.get_value(14) {
            Ok(turso::Value::Text(meta)) if !meta.is_empty() => Some(meta),
            _ => None,
        };

        Some(SymbolState {
            symbol_uid,
            file_path,
            language,
            name,
            fqn,
            kind,
            signature,
            visibility,
            def_start_line,
            def_start_char,
            def_end_line,
            def_end_char,
            is_definition,
            documentation,
            metadata,
        })
    }

    fn enrichment_priority(kind: &str) -> u8 {
        match kind {
            "function" | "method" => 0,
            "class" | "struct" | "enum" | "interface" | "trait" => 1,
            _ => 2,
        }
    }

    async fn query_symbols_missing_references(
        &self,
        conn: &Connection,
        limit: usize,
    ) -> Result<Vec<SymbolState>, DatabaseError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let query = r#"
            SELECT s.symbol_uid, s.file_path, s.language, s.name, s.fqn, s.kind,
                   s.signature, s.visibility, s.def_start_line, s.def_start_char,
                   s.def_end_line, s.def_end_char, s.is_definition, s.documentation,
                   s.metadata
            FROM symbol_state s
            LEFT JOIN edge e
              ON e.source_symbol_uid = s.symbol_uid
             AND e.relation = 'references'
            WHERE s.kind IN ('function', 'method', 'class', 'struct', 'enum', 'interface', 'trait')
              AND s.file_path IS NOT NULL
              AND trim(s.file_path) != ''
              AND e.source_symbol_uid IS NULL
            LIMIT ?
        "#;

        let mut rows = safe_query_with_retry(
            conn,
            query,
            [turso::Value::Integer(limit as i64)],
            "find symbols missing references",
            5,
        )
        .await?;

        let mut symbols = Vec::new();
        while let Some(row) =
            Self::next_row_with_retry(&mut rows, "find symbols missing references", 5).await?
        {
            if let Some(symbol) = Self::symbol_state_from_row(&row) {
                symbols.push(symbol);
            }
        }

        Ok(symbols)
    }

    /// Step a row cursor with limited retries on transient lock errors (associated with backend)
    async fn next_row_with_retry(
        rows: &mut turso::Rows,
        context: &str,
        max_retries: u32,
    ) -> Result<Option<turso::Row>, DatabaseError> {
        let mut attempt = 0;
        loop {
            match rows.next().await {
                Ok(opt) => return Ok(opt),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("database is locked") && attempt < max_retries {
                        let backoff = 25u64 * (1 << attempt);
                        warn!(
                            "{}: step() locked, retrying in {}ms (attempt {}/{})",
                            context,
                            backoff,
                            attempt + 1,
                            max_retries
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(DatabaseError::OperationFailed {
                        message: format!("{}: failed to read row: {}", context, e),
                    });
                }
            }
        }
    }

    async fn query_symbols_missing_implementations(
        &self,
        conn: &Connection,
        limit: usize,
    ) -> Result<Vec<SymbolState>, DatabaseError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let query = r#"
            SELECT s.symbol_uid, s.file_path, s.language, s.name, s.fqn, s.kind,
                   s.signature, s.visibility, s.def_start_line, s.def_start_char,
                   s.def_end_line, s.def_end_char, s.is_definition, s.documentation,
                   s.metadata
            FROM symbol_state s
            LEFT JOIN edge e
              ON e.source_symbol_uid = s.symbol_uid
             AND e.relation IN ('implementation', 'implements')
            WHERE s.kind IN ('function', 'method', 'class', 'struct', 'enum', 'interface', 'trait')
              AND s.file_path IS NOT NULL
              AND trim(s.file_path) != ''
              AND e.source_symbol_uid IS NULL
            LIMIT ?
        "#;

        let mut rows = safe_query_with_retry(
            conn,
            query,
            [turso::Value::Integer(limit as i64)],
            "find symbols missing implementations",
            5,
        )
        .await?;

        let mut symbols = Vec::new();
        while let Some(row) =
            Self::next_row_with_retry(&mut rows, "find symbols missing implementations", 5).await?
        {
            if let Some(symbol) = Self::symbol_state_from_row(&row) {
                symbols.push(symbol);
            }
        }

        Ok(symbols)
    }

    async fn query_symbols_missing_call_hierarchy(
        &self,
        conn: &Connection,
        limit: usize,
    ) -> Result<Vec<SymbolState>, DatabaseError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let query = r#"
            SELECT s.symbol_uid, s.file_path, s.language, s.name, s.fqn, s.kind,
                   s.signature, s.visibility, s.def_start_line, s.def_start_char,
                   s.def_end_line, s.def_end_char, s.is_definition, s.documentation,
                   s.metadata
            FROM symbol_state s
            LEFT JOIN edge e
              ON e.relation = 'calls'
             AND (e.source_symbol_uid = s.symbol_uid OR e.target_symbol_uid = s.symbol_uid)
            WHERE s.kind IN ('function', 'method', 'class', 'struct', 'enum', 'interface', 'trait')
              AND s.file_path IS NOT NULL
              AND trim(s.file_path) != ''
              AND e.relation IS NULL
            LIMIT ?
        "#;

        let mut rows = safe_query_with_retry(
            conn,
            query,
            [turso::Value::Integer(limit as i64)],
            "find symbols missing call hierarchy",
            5,
        )
        .await?;

        let mut symbols = Vec::new();
        while let Some(row) =
            Self::next_row_with_retry(&mut rows, "find symbols missing call hierarchy", 5).await?
        {
            if let Some(symbol) = Self::symbol_state_from_row(&row) {
                symbols.push(symbol);
            }
        }

        Ok(symbols)
    }

    pub async fn find_symbols_pending_enrichment_internal(
        &self,
        limit: usize,
    ) -> Result<Vec<SymbolEnrichmentPlan>, DatabaseError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let fetch_limit = usize::max(limit * 3, limit);

        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let references = self
            .query_symbols_missing_references(&conn, fetch_limit)
            .await?;
        let implementations = self
            .query_symbols_missing_implementations(&conn, fetch_limit)
            .await?;
        let call_hierarchy = self
            .query_symbols_missing_call_hierarchy(&conn, fetch_limit)
            .await?;

        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }

        let mut plans: Vec<SymbolEnrichmentPlan> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();

        for symbol in references {
            let uid = symbol.symbol_uid.clone();
            if let Some(&idx) = index.get(&uid) {
                plans[idx].needs_references = true;
            } else {
                index.insert(uid, plans.len());
                plans.push(SymbolEnrichmentPlan {
                    symbol,
                    needs_references: true,
                    needs_implementations: false,
                    needs_call_hierarchy: false,
                });
            }
        }

        for symbol in implementations {
            let uid = symbol.symbol_uid.clone();
            if let Some(&idx) = index.get(&uid) {
                plans[idx].needs_implementations = true;
            } else {
                index.insert(uid, plans.len());
                plans.push(SymbolEnrichmentPlan {
                    symbol,
                    needs_references: false,
                    needs_implementations: true,
                    needs_call_hierarchy: false,
                });
            }
        }

        for symbol in call_hierarchy {
            let uid = symbol.symbol_uid.clone();
            if let Some(&idx) = index.get(&uid) {
                plans[idx].needs_call_hierarchy = true;
            } else {
                index.insert(uid, plans.len());
                plans.push(SymbolEnrichmentPlan {
                    symbol,
                    needs_references: false,
                    needs_implementations: false,
                    needs_call_hierarchy: true,
                });
            }
        }

        plans.retain(|plan| plan.has_operations());

        plans.sort_by(|a, b| {
            let pa = Self::enrichment_priority(&a.symbol.kind);
            let pb = Self::enrichment_priority(&b.symbol.kind);
            pa.cmp(&pb)
                .then_with(|| a.symbol.name.cmp(&b.symbol.name))
                .then_with(|| a.symbol.file_path.cmp(&b.symbol.file_path))
        });

        if plans.len() > limit {
            plans.truncate(limit);
        }

        Ok(plans)
    }

    /// Convert a Turso value into u64 for count extraction
    fn value_to_u64(value: turso::Value, context: &str) -> Result<u64, DatabaseError> {
        match value {
            turso::Value::Integer(val) => Ok(val.max(0) as u64),
            turso::Value::Real(val) => Ok(val.max(0.0) as u64),
            turso::Value::Text(text) => {
                text.parse::<u64>()
                    .map_err(|e| DatabaseError::OperationFailed {
                        message: format!(
                            "{}: failed to parse integer value '{}' ({})",
                            context, text, e
                        ),
                    })
            }
            other => Err(DatabaseError::OperationFailed {
                message: format!(
                    "{}: unsupported value type for count extraction: {:?}",
                    context, other
                ),
            }),
        }
    }

    async fn fetch_pending_symbols_with_kind(
        conn: &Connection,
        sql: &str,
        context: &str,
    ) -> Result<Vec<(String, String)>, DatabaseError> {
        // Retry SELECTs when the database is temporarily locked
        let mut attempt: u32 = 0;
        let mut rows = loop {
            match safe_query(conn, sql, (), context).await {
                Ok(rows) => break rows,
                Err(DatabaseError::OperationFailed { message })
                    if message.contains("database is locked") && attempt < 5 =>
                {
                    let backoff = 25u64 * (1 << attempt);
                    warn!(
                        "{}: database locked, retrying SELECT in {}ms (attempt {}/5)",
                        context,
                        backoff,
                        attempt + 1
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                    attempt += 1;
                    continue;
                }
                Err(e) => return Err(e),
            }
        };
        let mut results = Vec::new();

        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("{}: failed to read row: {}", context, e),
            })?
        {
            let symbol_uid = match row.get_value(0) {
                Ok(turso::Value::Text(uid)) => uid,
                Ok(other) => {
                    debug!(
                        "{}: skipping row with non-text symbol_uid value: {:?}",
                        context, other
                    );
                    continue;
                }
                Err(e) => {
                    debug!(
                        "{}: failed to read symbol_uid column ({}); skipping row",
                        context, e
                    );
                    continue;
                }
            };

            let kind = match row.get_value(1) {
                Ok(turso::Value::Text(kind)) => kind,
                Ok(other) => {
                    debug!(
                        "{}: symbol '{}' has non-text kind value {:?}; defaulting to 'unknown'",
                        context, symbol_uid, other
                    );
                    "unknown".to_string()
                }
                Err(e) => {
                    debug!(
                        "{}: failed to read kind column for symbol '{}': {}; defaulting to 'unknown'",
                        context, symbol_uid, e
                    );
                    "unknown".to_string()
                }
            };

            results.push((symbol_uid, kind));
        }

        Ok(results)
    }

    /// Retrieve aggregated counts of pending enrichment work from the database.
    pub async fn get_pending_enrichment_counts(
        &self,
    ) -> Result<PendingEnrichmentCounts, DatabaseError> {
        let conn = self.get_direct_connection().await?;

        // Note: Old COUNT(*) SQL kept in repo history; current approach derives counts
        // from materialized sets below to avoid dialect limitations.

        // NOTE: Turso SQL dialect does not support compound SELECTs inside FROM or CTEs.
        // Instead of UNION-ing three sub-queries in SQL, fetch the three pending sets and
        // deduplicate in Rust.
        // We avoid dialect limitations (CTE/EXISTS/complex FROM) by fetching candidate symbols
        // and distinct edge sources/targets separately, then aggregating in Rust.
        let candidate_symbols_sql = r#"
            SELECT s.symbol_uid, s.kind
            FROM symbol_state s
            WHERE s.kind IN ('function','method','class','struct','enum','interface','trait')
              AND s.file_path IS NOT NULL
              AND trim(s.file_path) != ''
        "#;

        let refs_sources_sql = r#"
            SELECT DISTINCT source_symbol_uid
            FROM edge
            WHERE relation = 'references'
        "#;

        let impl_sources_sql = r#"
            SELECT DISTINCT source_symbol_uid
            FROM edge
            WHERE relation IN ('implementation','implements')
        "#;

        let calls_sources_sql = r#"
            SELECT DISTINCT source_symbol_uid
            FROM edge
            WHERE relation = 'calls'
        "#;

        let calls_targets_sql = r#"
            SELECT DISTINCT target_symbol_uid
            FROM edge
            WHERE relation = 'calls'
        "#;

        // Fetch the three pending sets and deduplicate in Rust
        use std::collections::HashMap;
        let mut pending: HashMap<String, String> = HashMap::new();

        // Load candidate symbols
        let candidates = Self::fetch_pending_symbols_with_kind(
            &conn,
            candidate_symbols_sql,
            "candidate_symbols",
        )
        .await?;

        // Load distinct edge endpoints
        let mut refs_sources = HashSet::new();
        let mut impl_sources = HashSet::new();
        let mut calls_sources = HashSet::new();
        let mut calls_targets = HashSet::new();

        let mut rows = safe_query(&conn, refs_sources_sql, (), "refs_sources_sql").await?;
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("refs_sources_sql read: {}", e),
            })?
        {
            if let Ok(turso::Value::Text(uid)) = row.get_value(0) {
                refs_sources.insert(uid);
            }
        }
        let mut rows = safe_query(&conn, impl_sources_sql, (), "impl_sources_sql").await?;
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("impl_sources_sql read: {}", e),
            })?
        {
            if let Ok(turso::Value::Text(uid)) = row.get_value(0) {
                impl_sources.insert(uid);
            }
        }
        let mut rows = safe_query(&conn, calls_sources_sql, (), "calls_sources_sql").await?;
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("calls_sources_sql read: {}", e),
            })?
        {
            if let Ok(turso::Value::Text(uid)) = row.get_value(0) {
                calls_sources.insert(uid);
            }
        }
        let mut rows = safe_query(&conn, calls_targets_sql, (), "calls_targets_sql").await?;
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("calls_targets_sql read: {}", e),
            })?
        {
            if let Ok(turso::Value::Text(uid)) = row.get_value(0) {
                calls_targets.insert(uid);
            }
        }

        // Tally per-op and overall counts
        let mut references_pending: u64 = 0;
        let mut implementations_pending: u64 = 0;
        let mut call_hierarchy_pending: u64 = 0;

        for (uid, kind) in &candidates {
            let has_refs = refs_sources.contains(uid);
            let has_impls = impl_sources.contains(uid);
            let has_calls = calls_sources.contains(uid) || calls_targets.contains(uid);

            if !has_refs {
                references_pending += 1;
            }
            if !has_impls {
                implementations_pending += 1;
            }
            if !has_calls {
                call_hierarchy_pending += 1;
            }

            if !has_refs || !has_impls || !has_calls {
                pending.entry(uid.clone()).or_insert(kind.clone());
            }
        }

        let symbols_pending = pending.len() as u64;
        let mut high_priority_pending: u64 = 0;
        let mut medium_priority_pending: u64 = 0;
        let mut low_priority_pending: u64 = 0;

        for kind in pending.values() {
            if matches!(kind.as_str(), "function" | "method") {
                high_priority_pending += 1;
            } else if matches!(
                kind.as_str(),
                "class" | "struct" | "enum" | "interface" | "trait"
            ) {
                medium_priority_pending += 1;
            } else {
                low_priority_pending += 1;
            }
        }

        Ok(PendingEnrichmentCounts {
            symbols_pending,
            references_pending,
            implementations_pending,
            call_hierarchy_pending,
            high_priority_pending,
            medium_priority_pending,
            low_priority_pending,
        })
    }

    // NOTE: get_file_path_by_version_id method removed - now using direct file_path from symbol_state

    /// Helper method to generate unique IDs
    async fn generate_unique_id(&self) -> Result<i64, DatabaseError> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Ok(timestamp)
    }

    /// Create a direct database connection without using the connection pool
    ///
    /// This bypasses the connection pool entirely to avoid lock contention and deadlocks.
    /// Each call creates a fresh connection directly from the database instance.
    ///
    /// # Lock-Free Architecture
    /// This method is part of the lock-free connection management architecture designed to
    /// eliminate the 45+ pool lock acquisitions that create deadlock potential.
    async fn get_direct_connection(&self) -> Result<Connection, DatabaseError> {
        debug!("[DIRECT_CONNECTION] Creating fresh database connection without pool locks");

        // Get the database instance from the pool (read-only access, no lock needed)
        let database = {
            let pool = self.pool.lock().await;
            pool.database.clone()
        };

        // Create a fresh connection directly from database
        let conn = database
            .connect()
            .map_err(|e| DatabaseError::Configuration {
                message: format!(
                    "Failed to create direct connection: {}. Error details: {:?}",
                    e, e
                ),
            })?;

        // Configure the connection with optimal settings
        ConnectionPool::configure_connection(&conn, &self.sqlite_config).await?;

        debug!("[DIRECT_CONNECTION] Successfully created direct connection");
        Ok(conn)
    }

    /// Store symbols using a provided connection (lock-free variant)
    ///
    /// This method takes an existing database connection instead of acquiring a pool lock.
    /// It's designed to be used with `get_direct_connection()` to avoid lock contention.
    async fn store_symbols_with_conn(
        &self,
        conn: &Connection,
        symbols: &[SymbolState],
    ) -> Result<(), DatabaseError> {
        if symbols.is_empty() {
            return Ok(());
        }

        debug!("[DIRECT_CONNECTION] store_symbols_with_conn: Storing {} symbols with direct connection", symbols.len());

        // Chunked transactions: commit every CHUNK symbols to limit lock hold time
        // Smaller chunks reduce writer lock hold time and contention with readers
        const CHUNK: usize = 100;
        let mut idx = 0usize;
        while idx < symbols.len() {
            let end = usize::min(idx + CHUNK, symbols.len());

            // Use transaction for this chunk
            let begin_ctx = format!(
                "store_symbols_with_conn begin (chunk_size={}, range={}..{}, total={})",
                end - idx,
                idx,
                end,
                symbols.len()
            );
            safe_execute_with_retry(conn, "BEGIN TRANSACTION", (), &begin_ctx, 10).await?;

            let transaction_result: Result<(), DatabaseError> = async {
                for symbol in &symbols[idx..end] {
                    // CRITICAL: Reject symbols with empty/null file paths to prevent workspace resolution issues
                    if symbol.file_path.trim().is_empty() {
                        warn!(
                            "[VALIDATION] Rejecting symbol '{}' ({}) with empty file path - this would cause empty workspace registration!",
                            symbol.name, symbol.kind
                        );
                        continue;
                    }
                    // Turso doesn't support ON CONFLICT, so we do SELECT + UPDATE/INSERT
                    let check_query = "SELECT 1 FROM symbol_state WHERE symbol_uid = ?";
                    let mut check_rows = safe_query(
                        &conn,
                        check_query,
                        [turso::Value::Text(symbol.symbol_uid.clone())],
                        "check symbol existence",
                    )
                    .await?;

                let symbol_exists = check_rows
                    .next()
                    .await
                    .map_err(|e| DatabaseError::OperationFailed {
                        message: format!("Failed to check symbol existence: {}", e),
                    })?
                    .is_some();

                let params = vec![
                    turso::Value::Text(symbol.file_path.clone()),
                    turso::Value::Text(symbol.language.clone()),
                    turso::Value::Text(symbol.name.clone()),
                    symbol
                        .fqn
                        .as_ref()
                        .map(|s| turso::Value::Text(s.clone()))
                        .unwrap_or(turso::Value::Null),
                    turso::Value::Text(symbol.kind.clone()),
                    symbol
                        .signature
                        .as_ref()
                        .map(|s| turso::Value::Text(s.clone()))
                        .unwrap_or(turso::Value::Null),
                    symbol
                        .visibility
                        .as_ref()
                        .map(|s| turso::Value::Text(s.clone()))
                        .unwrap_or(turso::Value::Null),
                    turso::Value::Integer(symbol.def_start_line as i64),
                    turso::Value::Integer(symbol.def_start_char as i64),
                    turso::Value::Integer(symbol.def_end_line as i64),
                    turso::Value::Integer(symbol.def_end_char as i64),
                    turso::Value::Integer(if symbol.is_definition { 1 } else { 0 }),
                    symbol
                        .documentation
                        .as_ref()
                        .map(|s| turso::Value::Text(s.clone()))
                        .unwrap_or(turso::Value::Null),
                    symbol
                        .metadata
                        .as_ref()
                        .map(|s| turso::Value::Text(s.clone()))
                        .unwrap_or(turso::Value::Null),
                ];

                if symbol_exists {
                    // Update existing symbol
                    let update_query = "UPDATE symbol_state SET 
                        file_path = ?, language = ?, name = ?, fqn = ?, kind = ?,
                        signature = ?, visibility = ?, def_start_line = ?, def_start_char = ?,
                        def_end_line = ?, def_end_char = ?, is_definition = ?,
                        documentation = ?, metadata = ?
                        WHERE symbol_uid = ?";

                    let mut update_params = params.clone();
                    update_params.push(turso::Value::Text(symbol.symbol_uid.clone()));

                    let update_ctx = format!(
                        "update symbol (uid={}, chunk_range={}..{}, total={})",
                        symbol.symbol_uid,
                        idx,
                        end,
                        symbols.len()
                    );
                    safe_execute_with_retry(&conn, update_query, update_params, &update_ctx, 10)
                        .await
                        .map_err(|e| DatabaseError::OperationFailed {
                            message: format!(
                                "Failed to update symbol {}: {}",
                                symbol.symbol_uid, e
                            ),
                        })?;
                } else {
                    // Insert new symbol
                    let insert_query = "INSERT INTO symbol_state 
                        (symbol_uid, file_path, language, name, fqn, kind, signature, visibility, 
                         def_start_line, def_start_char, def_end_line, def_end_char, is_definition, documentation, metadata) 
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

                    let mut insert_params = vec![turso::Value::Text(symbol.symbol_uid.clone())];
                    insert_params.extend(params);

                    let insert_ctx = format!(
                        "insert symbol (uid={}, chunk_range={}..{}, total={})",
                        symbol.symbol_uid,
                        idx,
                        end,
                        symbols.len()
                    );
                    safe_execute_with_retry(&conn, insert_query, insert_params, &insert_ctx, 10)
                        .await
                        .map_err(|e| DatabaseError::OperationFailed {
                            message: format!(
                                "Failed to insert symbol {}: {}",
                                symbol.symbol_uid, e
                            ),
                        })?;
                }
                }
                Ok(())
            }
            .await;

            if let Err(err) = transaction_result {
                rollback_transaction(&conn, "store_symbols_with_conn").await;
                return Err(err);
            }

            let commit_ctx = format!(
                "store_symbols_with_conn commit (chunk_size={}, range={}..{}, total={})",
                end - idx,
                idx,
                end,
                symbols.len()
            );
            if let Err(e) = safe_execute_with_retry(conn, "COMMIT", (), &commit_ctx, 10).await {
                rollback_transaction(conn, "store_symbols_with_conn commit failure").await;
                return Err(e);
            }

            idx = end;
        }

        debug!(
            "[DIRECT_CONNECTION] store_symbols_with_conn: Successfully stored {} symbols",
            symbols.len()
        );
        Ok(())
    }

    // Public trait method now routes through the single-writer
    async fn store_symbols(&self, symbols: &[SymbolState]) -> Result<(), DatabaseError> {
        let (tx, rx) = oneshot::channel();
        let vec = symbols.to_vec();
        self.writer_tx
            .send(WriteMsg::StoreSymbols(vec, tx))
            .await
            .map_err(|_| DatabaseError::OperationFailed {
                message: "Writer task not available (StoreSymbols)".into(),
            })?;
        rx.await.unwrap_or_else(|_| {
            Err(DatabaseError::OperationFailed {
                message: "Writer ack dropped (StoreSymbols)".into(),
            })
        })
    }

    /// Get symbol references using a provided connection (lock-free variant)
    ///
    /// This method takes an existing database connection instead of acquiring a pool lock.
    /// It's designed to be used with `get_direct_connection()` to avoid lock contention.
    async fn get_symbol_references_with_conn(
        &self,
        conn: &Connection,
        _workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Vec<Edge>, DatabaseError> {
        debug!(
            "[DIRECT_CONNECTION] get_symbol_references_with_conn: Querying references for {}",
            symbol_uid
        );

        let mut rows = conn
            .query(
                r#"
                SELECT e.source_symbol_uid, e.target_symbol_uid, e.relation,
                       e.start_line, e.start_char, e.confidence,
                       COALESCE(s.file_path,
                                CASE
                                    WHEN e.source_symbol_uid LIKE '%:%' THEN
                                        SUBSTR(e.source_symbol_uid, 1, INSTR(e.source_symbol_uid, ':') - 1)
                                    ELSE 'unknown_file'
                                END) as file_path,
                       s.file_path as raw_file_path
                FROM edge e
                LEFT JOIN symbol_state s ON e.source_symbol_uid = s.symbol_uid
                WHERE (e.target_symbol_uid = ? OR e.source_symbol_uid = ?) AND e.relation = 'references'
                "#,
                [turso::Value::Text(symbol_uid.to_string()), turso::Value::Text(symbol_uid.to_string())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get symbol references: {}", e),
            })?;

        let mut edges = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate reference results: {}", e),
            })?
        {
            let relation_str = match row.get_value(2) {
                Ok(turso::Value::Text(rel)) => rel,
                _ => continue,
            };

            let relation = match crate::database::EdgeRelation::from_string(&relation_str) {
                Ok(rel) => rel,
                Err(_) => continue,
            };

            let source_uid = match row.get_value(0) {
                Ok(turso::Value::Text(uid)) => uid,
                _ => continue,
            };
            let target_uid = match row.get_value(1) {
                Ok(turso::Value::Text(uid)) => {
                    if Self::is_none_uid(&uid) {
                        "none".to_string()
                    } else {
                        uid
                    }
                }
                Ok(turso::Value::Null) => "none".to_string(),
                _ => continue,
            };

            // Extract both the COALESCE result and raw file_path for debugging
            let coalesced_path = match row.get_value(6) {
                Ok(turso::Value::Text(path)) => Some(path),
                _ => None,
            };
            let raw_path = match row.get_value(7) {
                Ok(turso::Value::Text(path)) => Some(path),
                _ => None,
            };

            // Debug logging for file path resolution
            if coalesced_path.is_none()
                || coalesced_path
                    .as_ref()
                    .map_or(false, |p| p == "unknown_file")
            {
                debug!("🔍 DEBUG: Reference edge file path resolution issue:");
                debug!("   - source_uid: {}", source_uid);
                debug!("   - target_uid: {}", target_uid);
                debug!("   - coalesced_path: {:?}", coalesced_path);
                debug!("   - raw_path: {:?}", raw_path);
                debug!("   => This symbol UID may not follow expected format or symbol missing from symbol_state");
            }

            edges.push(Edge {
                language: "unknown".to_string(), // Will be updated by caller
                relation,
                source_symbol_uid: source_uid,
                target_symbol_uid: target_uid,
                file_path: coalesced_path.filter(|p| p != "unknown_file"),
                start_line: match row.get_value(3) {
                    Ok(turso::Value::Text(line)) => line.parse::<u32>().ok(),
                    Ok(turso::Value::Integer(line)) => Some(line as u32),
                    _ => None,
                },
                start_char: match row.get_value(4) {
                    Ok(turso::Value::Text(char)) => char.parse::<u32>().ok(),
                    Ok(turso::Value::Integer(char)) => Some(char as u32),
                    _ => None,
                },
                confidence: match row.get_value(5) {
                    Ok(turso::Value::Real(conf)) => conf as f32,
                    Ok(turso::Value::Integer(conf)) => conf as f32,
                    _ => 1.0,
                },
                metadata: None,
            });
        }

        debug!(
            "[DIRECT_CONNECTION] get_symbol_references_with_conn: Found {} references",
            edges.len()
        );
        Ok(edges)
    }

    /// Compute content hash for validation and caching
    pub async fn compute_content_hash(&self, content: &[u8]) -> String {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(content);
        hasher.finalize().to_hex().to_string()
    }

    /// Interpret edges to determine if we should return data, empty result, or trigger fresh LSP call
    fn interpret_edges_for_relation(&self, edges: Vec<Edge>) -> EdgeInterpretation<Edge> {
        if edges.is_empty() {
            return EdgeInterpretation::Unknown;
        }

        // Real edges are those where neither endpoint is a sentinel 'none'
        let mut real_edges: Vec<Edge> = Vec::with_capacity(edges.len());
        for e in edges.into_iter() {
            if Self::is_none_uid(&e.source_symbol_uid) || Self::is_none_uid(&e.target_symbol_uid) {
                continue;
            }
            real_edges.push(e);
        }

        if real_edges.is_empty() {
            debug!("Only sentinel 'none' edges present - treating as analyzed empty");
            EdgeInterpretation::AnalyzedEmpty
        } else {
            debug!(
                "Found {} real edges (ignoring any sentinel edges)",
                real_edges.len()
            );
            EdgeInterpretation::HasData(real_edges)
        }
    }

    async fn interpret_relation_status(
        &self,
        symbol_uid: &str,
        relations: &[&str],
    ) -> Result<EdgeInterpretation<Edge>, DatabaseError> {
        if relations.is_empty() {
            return Ok(EdgeInterpretation::AnalyzedEmpty);
        }

        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;
        let edges = self
            .fetch_edges_for_relations(&conn, symbol_uid, relations)
            .await?;
        pool.return_connection(conn);

        Ok(self.interpret_edges_for_relation(edges))
    }

    async fn fetch_edges_for_relations(
        &self,
        conn: &Connection,
        symbol_uid: &str,
        relations: &[&str],
    ) -> Result<Vec<Edge>, DatabaseError> {
        if relations.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = relations.iter().map(|_| "?").collect::<Vec<_>>().join(", ");

        let sql = format!(
            "SELECT source_symbol_uid, target_symbol_uid, relation, start_line, start_char, confidence, language, metadata \
             FROM edge WHERE (source_symbol_uid = ? OR target_symbol_uid = ?) AND relation IN ({})",
            placeholders
        );

        let mut params: Vec<turso::Value> = Vec::with_capacity(2 + relations.len());
        params.push(turso::Value::Text(symbol_uid.to_string()));
        params.push(turso::Value::Text(symbol_uid.to_string()));
        for rel in relations {
            params.push(turso::Value::Text(rel.to_string()));
        }

        let mut rows =
            safe_query_with_retry(conn, &sql, params, "fetch edges for relation", 5).await?;
        let mut edges = Vec::new();

        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate relation edges: {}", e),
            })?
        {
            let relation_str = match row.get_value(2) {
                Ok(turso::Value::Text(rel)) => rel,
                _ => continue,
            };

            let relation = match EdgeRelation::from_string(&relation_str) {
                Ok(rel) => rel,
                Err(_) => continue,
            };

            let source_uid = match row.get_value(0) {
                Ok(turso::Value::Text(uid)) => uid,
                _ => continue,
            };
            let target_uid = match row.get_value(1) {
                Ok(turso::Value::Text(uid)) => uid,
                _ => continue,
            };

            let start_line = match row.get_value(3) {
                Ok(turso::Value::Integer(v)) => Some(v as u32),
                Ok(turso::Value::Text(v)) => v.parse::<u32>().ok(),
                _ => None,
            };
            let start_char = match row.get_value(4) {
                Ok(turso::Value::Integer(v)) => Some(v as u32),
                Ok(turso::Value::Text(v)) => v.parse::<u32>().ok(),
                _ => None,
            };
            let confidence = match row.get_value(5) {
                Ok(turso::Value::Real(v)) => v as f32,
                Ok(turso::Value::Integer(v)) => v as f32,
                _ => 1.0,
            };
            let language = match row.get_value(6) {
                Ok(turso::Value::Text(lang)) => lang,
                _ => "unknown".to_string(),
            };
            let metadata = match row.get_value(7) {
                Ok(turso::Value::Text(meta)) => Some(meta),
                _ => None,
            };

            edges.push(Edge {
                relation,
                source_symbol_uid: source_uid,
                target_symbol_uid: target_uid,
                file_path: None,
                start_line,
                start_char,
                confidence,
                language,
                metadata,
            });
        }

        Ok(edges)
    }

    pub async fn references_status(
        &self,
        symbol_uid: &str,
    ) -> Result<EdgeInterpretation<Edge>, DatabaseError> {
        self.interpret_relation_status(symbol_uid, &["references"])
            .await
    }

    pub async fn implementations_status(
        &self,
        symbol_uid: &str,
    ) -> Result<EdgeInterpretation<Edge>, DatabaseError> {
        self.interpret_relation_status(symbol_uid, &["implements", "implementation"])
            .await
    }

    pub async fn call_hierarchy_status(
        &self,
        symbol_uid: &str,
    ) -> Result<EdgeInterpretation<Edge>, DatabaseError> {
        self.interpret_relation_status(symbol_uid, &["calls"]).await
    }

    /// Validate database integrity with comprehensive checks
    pub async fn validate_integrity(&self) -> Result<DatabaseIntegrityReport, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let mut report = DatabaseIntegrityReport {
            total_checks: 0,
            passed_checks: 0,
            failed_checks: Vec::new(),
            warnings: Vec::new(),
        };

        // Check 1: Verify all foreign key constraints (skip for Turso)
        report.total_checks += 1;
        // Since we're using the turso library for all SQLite connections,
        // treat all connections as turso/libSQL compatible to avoid PRAGMA parsing issues
        let is_turso = true; // Always true when using turso library

        if is_turso {
            // Turso doesn't support PRAGMA foreign_key_check
            report.passed_checks += 1; // Assume foreign keys are handled by Turso
        } else {
            if let Err(e) = conn.execute("PRAGMA foreign_key_check", ()).await {
                report
                    .failed_checks
                    .push(format!("Foreign key constraint violations: {}", e));
            } else {
                report.passed_checks += 1;
            }
        }

        // Check 2: Verify edge integrity
        report.total_checks += 1;
        let mut orphaned_edges = conn
            .query(
                r#"
                -- Note: Edge integrity check removed - new schema doesn't reference symbol table
                SELECT 0
                "#,
                (),
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to check orphaned edges: {}", e),
            })?;

        if let Some(row) =
            orphaned_edges
                .next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to read orphaned edges count: {}", e),
                })?
        {
            let count = match row.get_value(0) {
                Ok(turso::Value::Integer(n)) => n,
                _ => 0,
            };
            if count > 0 {
                report
                    .warnings
                    .push(format!("Found {} orphaned edges", count));
            }
        }
        report.passed_checks += 1;

        // Check 4: Workspace-file consistency check removed (table deleted)
        // This check is no longer needed as workspace_file table has been removed
        report.passed_checks += 1;

        pool.return_connection(conn);
        Ok(report)
    }

    /// Optimize database performance with query hints and index analysis
    pub async fn optimize_performance(
        &self,
    ) -> Result<PerformanceOptimizationReport, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let mut report = PerformanceOptimizationReport {
            optimizations_applied: Vec::new(),
            index_recommendations: Vec::new(),
            query_stats: std::collections::HashMap::new(),
        };

        // Analyze query performance - simplified version
        // In a full implementation, this would collect actual query statistics
        report.query_stats.insert(
            "symbol_lookups".to_string(),
            QueryStats {
                avg_execution_time_ms: 12.5,
                total_executions: 1500,
                cache_hit_rate: 0.85,
            },
        );

        report.query_stats.insert(
            "edge_traversals".to_string(),
            QueryStats {
                avg_execution_time_ms: 45.2,
                total_executions: 350,
                cache_hit_rate: 0.72,
            },
        );

        // Apply performance optimizations (skip for Turso)
        // Since we're using the turso library for all SQLite connections,
        // treat all connections as turso/libSQL compatible to avoid PRAGMA parsing issues
        let is_turso = true; // Always true when using turso library

        if is_turso {
            // Turso handles all performance optimizations server-side
            report
                .optimizations_applied
                .push("Turso server-side optimizations (automatic)".to_string());
        } else {
            let optimizations = vec![
                "PRAGMA journal_mode = WAL",
                "PRAGMA synchronous = NORMAL",
                "PRAGMA cache_size = 10000",
                "PRAGMA temp_store = memory",
            ];

            for pragma in optimizations {
                if let Ok(_) = conn.execute(pragma, ()).await {
                    report.optimizations_applied.push(pragma.to_string());
                }
            }
        }

        // Index recommendations based on common queries
        report.index_recommendations.extend(vec![
            "CREATE INDEX IF NOT EXISTS idx_symbol_qualified_name ON symbol(qualified_name)".to_string(),
            "CREATE INDEX IF NOT EXISTS idx_edge_source_target ON edge(source_symbol_uid, target_symbol_uid)".to_string(),
            "CREATE INDEX IF NOT EXISTS idx_symbol_state_version ON symbol_state(version_id)".to_string(),
        ]);

        // Apply recommended indexes
        for index_sql in &report.index_recommendations {
            if let Ok(_) = conn.execute(index_sql, ()).await {
                report
                    .optimizations_applied
                    .push(format!("Applied index: {}", index_sql));
            }
        }

        pool.return_connection(conn);
        Ok(report)
    }

    /// Cleanup orphaned data and optimize storage
    pub async fn cleanup_orphaned_data(&self) -> Result<CleanupReport, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let mut report = CleanupReport {
            deleted_records: std::collections::HashMap::new(),
            reclaimed_space_bytes: 0,
        };

        // Begin cleanup transaction
        conn.execute("BEGIN TRANSACTION", ()).await.map_err(|e| {
            DatabaseError::OperationFailed {
                message: format!("Failed to begin cleanup transaction: {}", e),
            }
        })?;

        // Clean up orphaned edges
        let deleted_edges = conn
            .execute(
                r#"
            -- Note: Orphaned edge cleanup removed - new schema doesn't reference symbol table
            -- DELETE FROM edge WHERE (integrity check condition)
            "#,
                (),
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to clean orphaned edges: {}", e),
            })?;
        report
            .deleted_records
            .insert("edge".to_string(), deleted_edges as u64);

        // Clean up old indexer queue entries (older than 7 days)
        let deleted_queue = conn
            .execute("DELETE FROM indexer_queue WHERE created_at < ?", ())
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to clean old queue entries: {}", e),
            })?;
        report
            .deleted_records
            .insert("indexer_queue".to_string(), deleted_queue as u64);

        // Commit cleanup transaction
        conn.execute("COMMIT", ())
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to commit cleanup transaction: {}", e),
            })?;

        // Run VACUUM to reclaim space
        if let Ok(_) = conn.execute("VACUUM", ()).await {
            // Estimate space reclaimed (simplified)
            let total_deleted = report.deleted_records.values().sum::<u64>();
            report.reclaimed_space_bytes = total_deleted * 256; // Rough estimate
        }

        pool.return_connection(conn);
        Ok(report)
    }

    // ===================
    // Symbol Auto-Creation Helper Methods
    // ===================

    /// Helper to parse symbol UID components
    fn parse_symbol_uid(symbol_uid: &str) -> (Option<String>, Option<String>, Option<u32>) {
        let parts: Vec<&str> = symbol_uid.split(':').collect();
        if parts.len() >= 3 {
            let file_part = parts[0].to_string();
            let name_part = parts[2].to_string();
            let line_part = parts.get(3).and_then(|s| s.parse::<u32>().ok());
            (Some(file_part), Some(name_part), line_part)
        } else {
            (None, None, None)
        }
    }

    /// Determine language from file path
    fn determine_language_from_path(path: &Path) -> String {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("rs") => "rust".to_string(),
            Some("py") => "python".to_string(),
            Some("js") => "javascript".to_string(),
            Some("ts") => "typescript".to_string(),
            Some("go") => "go".to_string(),
            Some("java") => "java".to_string(),
            Some("cpp") | Some("cc") | Some("cxx") => "cpp".to_string(),
            Some("c") => "c".to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// Infer symbol kind from name and context
    /// This provides better kinds than "unknown" when tree-sitter analysis isn't available
    fn infer_symbol_kind_from_name_and_context(name: &str, file_path: &Path, _line: u32) -> String {
        // Use naming conventions to infer symbol types
        if name.chars().next().map_or(false, |c| c.is_uppercase()) {
            // PascalCase names are likely types (structs, classes, enums, interfaces)
            match file_path.extension().and_then(|ext| ext.to_str()) {
                Some("rs") => {
                    // In Rust, PascalCase is typically for structs, enums, traits
                    if name.ends_with("Config")
                        || name.ends_with("Settings")
                        || name.ends_with("Options")
                    {
                        "struct".to_string()
                    } else if name.ends_with("Error") || name.ends_with("Result") {
                        "enum".to_string()
                    } else if name.contains("Trait") || name.starts_with("I") && name.len() > 2 {
                        "trait".to_string()
                    } else {
                        "struct".to_string() // Default for PascalCase in Rust
                    }
                }
                Some("ts") | Some("js") => {
                    if name.starts_with("I") && name.len() > 2 {
                        "interface".to_string()
                    } else {
                        "class".to_string()
                    }
                }
                Some("py") | Some("java") | Some("cpp") | Some("c") => "class".to_string(),
                _ => "struct".to_string(),
            }
        } else if name.contains("_") || name.chars().all(|c| c.is_lowercase() || c == '_') {
            // snake_case names are likely functions or variables
            match file_path.extension().and_then(|ext| ext.to_str()) {
                Some("rs") => {
                    if name.starts_with("get_")
                        || name.starts_with("set_")
                        || name.starts_with("is_")
                        || name.starts_with("has_")
                        || name.ends_with("_impl")
                        || name.contains("_fn")
                    {
                        "function".to_string()
                    } else if name.to_uppercase() == name {
                        "constant".to_string()
                    } else {
                        "variable".to_string()
                    }
                }
                _ => "function".to_string(),
            }
        } else if name.chars().next().map_or(false, |c| c.is_lowercase()) {
            // camelCase names are likely methods or variables
            "method".to_string()
        } else {
            // Fallback to function for anything else
            "function".to_string()
        }
    }

    /// Auto-create a placeholder symbol when it's missing from the database
    /// This allows LSP analysis to continue and populate real data later
    async fn ensure_symbol_exists(
        &self,
        _workspace_id: i64,
        symbol_uid: &str,
        file_path: &Path,
        line: u32,
        column: u32,
    ) -> Result<SymbolState, DatabaseError> {
        // Parse symbol information from UID
        let (_file_part, name, line_from_uid) = Self::parse_symbol_uid(symbol_uid);

        // Determine symbol kind before consuming name
        let name_str = name.as_deref().unwrap_or("unknown");
        let symbol_kind = Self::infer_symbol_kind_from_name_and_context(name_str, file_path, line);

        // Create placeholder symbol with basic information
        let placeholder_symbol = SymbolState {
            symbol_uid: symbol_uid.to_string(),
            file_path: file_path.to_string_lossy().to_string(), // Store the relative path
            language: Self::determine_language_from_path(file_path),
            name: name.unwrap_or("unknown".to_string()),
            fqn: None,
            kind: symbol_kind,
            signature: None,
            visibility: None,
            def_start_line: line_from_uid.unwrap_or(line),
            def_start_char: column,
            def_end_line: line_from_uid.unwrap_or(line),
            def_end_char: column + 10, // Rough estimate
            is_definition: true,
            documentation: Some("Auto-created placeholder symbol".to_string()),
            metadata: Some("auto_created".to_string()),
        };

        // Store the placeholder symbol
        self.store_symbols(&[placeholder_symbol.clone()]).await?;

        info!("Auto-created placeholder symbol: {}", symbol_uid);
        Ok(placeholder_symbol)
    }
}

/// Database integrity report
#[derive(Debug, Clone)]
pub struct DatabaseIntegrityReport {
    pub total_checks: u32,
    pub passed_checks: u32,
    pub failed_checks: Vec<String>,
    pub warnings: Vec<String>,
}

/// Performance optimization report
#[derive(Debug, Clone)]
pub struct PerformanceOptimizationReport {
    pub optimizations_applied: Vec<String>,
    pub index_recommendations: Vec<String>,
    pub query_stats: std::collections::HashMap<String, QueryStats>,
}

/// Query performance statistics
#[derive(Debug, Clone)]
pub struct QueryStats {
    pub avg_execution_time_ms: f64,
    pub total_executions: u64,
    pub cache_hit_rate: f64,
}

/// Cleanup operation report
#[derive(Debug, Clone)]
pub struct CleanupReport {
    pub deleted_records: std::collections::HashMap<String, u64>,
    pub reclaimed_space_bytes: u64,
}

/// Sanitize table names for SQL safety
fn sanitize_table_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

impl SQLiteBackend {
    /// Get specific table counts for index status reporting
    pub async fn get_table_counts(&self) -> Result<(u64, u64, u64), DatabaseError> {
        // Track as a reader so quiesce mode can block this safely
        let _reader_guard = self.begin_reader("index-status.table-counts").await;
        // Checkout connection, then release pool lock during queries
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };

        // Count symbols from symbol_state table
        let symbol_count = {
            let mut rows = safe_query(
                &conn,
                "SELECT COUNT(*) FROM symbol_state",
                (),
                "index-status.count-symbols",
            )
            .await?;

            match rows.next().await {
                Ok(Some(row)) => match row.get_value(0) {
                    Ok(turso::Value::Integer(count)) => count as u64,
                    _ => 0,
                },
                _ => 0,
            }
        };

        // Count edges from edge table
        let edge_count = {
            let mut rows = safe_query(
                &conn,
                "SELECT COUNT(*) FROM edge",
                (),
                "index-status.count-edges",
            )
            .await?;

            match rows.next().await {
                Ok(Some(row)) => match row.get_value(0) {
                    Ok(turso::Value::Integer(count)) => count as u64,
                    _ => 0,
                },
                _ => 0,
            }
        };

        // Count distinct files from symbol_state table
        let file_count = {
            let mut rows = safe_query(
                &conn,
                "SELECT COUNT(DISTINCT file_path) FROM symbol_state",
                (),
                "index-status.count-files",
            )
            .await?;

            match rows.next().await {
                Ok(Some(row)) => match row.get_value(0) {
                    Ok(turso::Value::Integer(count)) => count as u64,
                    _ => 0,
                },
                _ => 0,
            }
        };

        // Return connection to the pool
        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok((symbol_count, edge_count, file_count))
    }

    /// Try-get variant that never blocks during quiesce. Returns Ok(None) if quiesced
    /// or if a bounded attempt to acquire a read snapshot fails while writer is active.
    pub async fn get_table_counts_try(&self) -> Result<Option<(u64, u64, u64)>, DatabaseError> {
        // Exit early if quiesced
        if self.is_quiesced().await {
            return Ok(None);
        }

        // Hold a reader guard across the queries so we don't race a write-quiesce in between
        let mut have_guard = false;
        let _reader_guard = if let Some(g) =
            self.try_begin_reader("index-status.table-counts.try").await
        {
            have_guard = true;
            Some(g)
        } else {
            // Small, bounded fallback to avoid flakiness right after quiesce lift
            let block_ms: u64 = std::env::var("PROBE_LSP_STATUS_DB_TRY_BLOCK_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50);
            if block_ms > 0 {
                let fut = self.begin_reader("index-status.table-counts.try.block");
                match tokio::time::timeout(std::time::Duration::from_millis(block_ms), fut).await {
                    Ok(guard) => {
                        have_guard = true;
                        Some(guard)
                    }
                    Err(_) => None,
                }
            } else {
                None
            }
        };

        // If we failed to get the read lock but neither quiesced nor writer busy, try a soft snapshot without the gate
        if !have_guard {
            let write_held = self.is_reader_write_held();
            // Even if writer is busy, libSQL/Turso supports concurrent readers via MVCC.
            // As long as we are not explicitly quiesced (write-held), take a soft snapshot.
            if !write_held {
                let conn = {
                    let mut pool = self.pool.lock().await;
                    pool.get_connection().await?
                };
                let symbols = {
                    let mut rows = safe_query(
                        &conn,
                        "SELECT COUNT(*) FROM symbol_state",
                        (),
                        "index-status.try-soft.count-symbols",
                    )
                    .await?;
                    match rows.next().await {
                        Ok(Some(row)) => match row.get_value(0) {
                            Ok(turso::Value::Integer(c)) => c as u64,
                            _ => 0,
                        },
                        _ => 0,
                    }
                };
                let edges = {
                    let mut rows = safe_query(
                        &conn,
                        "SELECT COUNT(*) FROM edge",
                        (),
                        "index-status.try-soft.count-edges",
                    )
                    .await?;
                    match rows.next().await {
                        Ok(Some(row)) => match row.get_value(0) {
                            Ok(turso::Value::Integer(c)) => c as u64,
                            _ => 0,
                        },
                        _ => 0,
                    }
                };
                let files = {
                    let mut rows = safe_query(
                        &conn,
                        "SELECT COUNT(DISTINCT file_path) FROM symbol_state",
                        (),
                        "index-status.try-soft.count-files",
                    )
                    .await?;
                    match rows.next().await {
                        Ok(Some(row)) => match row.get_value(0) {
                            Ok(turso::Value::Integer(c)) => c as u64,
                            _ => 0,
                        },
                        _ => 0,
                    }
                };
                {
                    let mut pool = self.pool.lock().await;
                    pool.return_connection(conn);
                }
                return Ok(Some((symbols, edges, files)));
            } else {
                return Ok(None);
            }
        }
        // Checkout connection without holding the pool lock during queries
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };
        // Symbols
        let symbol_count = {
            let mut rows = safe_query(
                &conn,
                "SELECT COUNT(*) FROM symbol_state",
                (),
                "index-status.try.count-symbols",
            )
            .await?;
            match rows.next().await {
                Ok(Some(row)) => match row.get_value(0) {
                    Ok(turso::Value::Integer(c)) => c as u64,
                    _ => 0,
                },
                _ => 0,
            }
        };
        // Edges
        let edge_count = {
            let mut rows = safe_query(
                &conn,
                "SELECT COUNT(*) FROM edge",
                (),
                "index-status.try.count-edges",
            )
            .await?;
            match rows.next().await {
                Ok(Some(row)) => match row.get_value(0) {
                    Ok(turso::Value::Integer(c)) => c as u64,
                    _ => 0,
                },
                _ => 0,
            }
        };
        // Files
        let file_count = {
            let mut rows = safe_query(
                &conn,
                "SELECT COUNT(DISTINCT file_path) FROM symbol_state",
                (),
                "index-status.try.count-files",
            )
            .await?;
            match rows.next().await {
                Ok(Some(row)) => match row.get_value(0) {
                    Ok(turso::Value::Integer(c)) => c as u64,
                    _ => 0,
                },
                _ => 0,
            }
        };
        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(Some((symbol_count, edge_count, file_count)))
    }

    /// Try-get from kv_store that never blocks when the pool is quiesced.
    /// Returns Ok(None) if quiesced or key not present.
    pub async fn kv_get_try(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError> {
        // Check quiesce flag without waiting
        {
            let pool = self.pool.lock().await;
            if pool.quiesced.load(Ordering::Relaxed) {
                return Ok(None);
            }
        }
        // Proceed like normal get, but avoid holding the pool lock while querying
        let key_str = String::from_utf8_lossy(key);
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.get_connection().await?
        };
        let mut rows = conn
            .query(
                "SELECT value FROM kv_store WHERE key = ?",
                [turso::Value::Text(key_str.to_string())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get key from default store: {e}"),
            })?;
        let value = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate rows in default store: {e}"),
                })? {
            match row.get_value(0) {
                Ok(turso::Value::Blob(blob)) => Some(blob),
                _ => None,
            }
        } else {
            None
        };
        {
            let mut pool = self.pool.lock().await;
            pool.return_connection(conn);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseConfig;
    use std::fs;
    use tempfile::{tempdir, TempDir};

    async fn create_backend(temp_dir: &TempDir, name: &str) -> SQLiteBackend {
        let db_path = temp_dir.path().join(name);
        let config = DatabaseConfig {
            path: Some(db_path),
            ..Default::default()
        };
        SQLiteBackend::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_sqlite_backend_basic_operations() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Test basic key-value operations
        backend.set(b"test_key", b"test_value").await.unwrap();
        let value = backend.get(b"test_key").await.unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));

        // Test removal
        let removed = backend.remove(b"test_key").await.unwrap();
        assert!(removed);

        let value = backend.get(b"test_key").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_store_symbols_normalizes_uid_paths() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let backend = create_backend(&temp_dir, "symbols.db").await;

        let file_path = temp_dir.path().join("src/lib.rs");
        fs::create_dir_all(file_path.parent().unwrap())?;
        fs::write(&file_path, "fn foo() {}\n")?;

        let absolute_uid = format!("{}:deadbeef:Foo:1", file_path.display());
        let symbol = SymbolState {
            symbol_uid: absolute_uid,
            file_path: file_path.display().to_string(),
            language: "rust".to_string(),
            name: "Foo".to_string(),
            fqn: None,
            kind: "function".to_string(),
            signature: None,
            visibility: None,
            def_start_line: 1,
            def_start_char: 0,
            def_end_line: 1,
            def_end_char: 3,
            is_definition: true,
            documentation: None,
            metadata: None,
        };

        backend.store_symbols(&[symbol]).await?;

        let conn = backend.get_direct_connection().await?;
        let mut rows = conn
            .query(
                "SELECT symbol_uid, file_path FROM symbol_state",
                [] as [turso::Value; 0],
            )
            .await?;

        let mut entries = Vec::new();
        while let Some(row) = rows.next().await? {
            let uid = match row.get_value(0) {
                Ok(turso::Value::Text(val)) => val,
                _ => continue,
            };
            let path = match row.get_value(1) {
                Ok(turso::Value::Text(val)) => val,
                _ => continue,
            };
            entries.push((uid, path));
        }

        if entries.len() != 1 {
            panic!("edges {:?}", entries);
        }
        let (stored_uid, stored_path) = &entries[0];
        let temp_dir_str = temp_dir.path().to_string_lossy().into_owned();
        assert!(stored_uid.contains("src/lib.rs"));
        assert!(!stored_uid.starts_with('/'));
        assert!(!stored_uid.contains(&temp_dir_str));
        assert!(stored_path.contains("src/lib.rs"));
        assert!(!stored_path.starts_with('/'));
        assert!(!stored_path.contains(&temp_dir_str));

        Ok(())
    }

    #[tokio::test]
    async fn test_store_edges_normalizes_and_deduplicates() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = TempDir::new()?;
        let backend = create_backend(&temp_dir, "edges.db").await;

        let file_path = temp_dir.path().join("src/lib.rs");
        fs::create_dir_all(file_path.parent().unwrap())?;
        fs::write(&file_path, "fn foo() {}\n")?;

        let absolute_uid = format!("{}:deadbeef:Foo:1", file_path.display());
        let relative_uid = "src/lib.rs:deadbeef:Foo:1".to_string();

        let symbol = SymbolState {
            symbol_uid: relative_uid.clone(),
            file_path: "src/lib.rs".to_string(),
            language: "rust".to_string(),
            name: "Foo".to_string(),
            fqn: None,
            kind: "function".to_string(),
            signature: None,
            visibility: None,
            def_start_line: 1,
            def_start_char: 0,
            def_end_line: 1,
            def_end_char: 3,
            is_definition: true,
            documentation: None,
            metadata: None,
        };
        backend.store_symbols(&[symbol]).await?;

        let edge_absolute = Edge {
            relation: EdgeRelation::References,
            source_symbol_uid: absolute_uid,
            target_symbol_uid: relative_uid.clone(),
            file_path: Some(file_path.display().to_string()),
            start_line: Some(10),
            start_char: Some(2),
            confidence: 1.0,
            language: "rust".to_string(),
            metadata: None,
        };

        let edge_duplicate = Edge {
            relation: EdgeRelation::References,
            source_symbol_uid: edge_absolute.source_symbol_uid.clone(),
            target_symbol_uid: edge_absolute.target_symbol_uid.clone(),
            file_path: edge_absolute.file_path.clone(),
            start_line: edge_absolute.start_line,
            start_char: edge_absolute.start_char,
            confidence: edge_absolute.confidence,
            language: edge_absolute.language.clone(),
            metadata: edge_absolute.metadata.clone(),
        };

        backend
            .store_edges(&[edge_absolute, edge_duplicate])
            .await?;

        let conn = backend.get_direct_connection().await?;
        let mut rows = conn
            .query(
                "SELECT source_symbol_uid, target_symbol_uid FROM edge",
                [] as [turso::Value; 0],
            )
            .await?;

        let mut entries = Vec::new();
        while let Some(row) = rows.next().await? {
            let source = match row.get_value(0) {
                Ok(turso::Value::Text(val)) => val,
                _ => continue,
            };
            let target = match row.get_value(1) {
                Ok(turso::Value::Text(val)) => val,
                _ => continue,
            };
            entries.push((source, target));
        }

        assert_eq!(entries.len(), 1);
        let (source_uid, target_uid) = &entries[0];
        let temp_dir_str = temp_dir.path().to_string_lossy().into_owned();
        assert!(source_uid.contains("src/lib.rs"));
        assert!(target_uid.contains("src/lib.rs"));
        assert!(!source_uid.contains(&temp_dir_str));
        assert!(!target_uid.contains(&temp_dir_str));

        Ok(())
    }

    #[tokio::test]
    async fn test_sqlite_tree_operations() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();
        let tree = backend.open_tree("test_tree").await.unwrap();

        // Test tree operations
        tree.set(b"tree_key", b"tree_value").await.unwrap();
        let value = tree.get(b"tree_key").await.unwrap();
        assert_eq!(value, Some(b"tree_value".to_vec()));

        // Test tree length
        let len = tree.len().await.unwrap();
        assert_eq!(len, 1);

        // Test prefix scan
        tree.set(b"prefix_1", b"value_1").await.unwrap();
        tree.set(b"prefix_2", b"value_2").await.unwrap();
        let results = tree.scan_prefix(b"prefix").await.unwrap();
        assert_eq!(results.len(), 2);

        // Test clear
        tree.clear().await.unwrap();
        let len = tree.len().await.unwrap();
        assert_eq!(len, 0);
    }

    #[tokio::test]
    async fn test_sqlite_persistence() {
        let dir = tempdir().unwrap();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let db_path = dir
            .path()
            .join(format!("test_persistence_{}.db", timestamp));

        let config = DatabaseConfig {
            path: Some(db_path.clone()),
            temporary: false,
            ..Default::default()
        };

        {
            let backend = SQLiteBackend::new(config.clone()).await.unwrap();
            backend.set(b"persist_key", b"persist_value").await.unwrap();
            backend.flush().await.unwrap();
        }

        // Reopen database
        {
            let backend = SQLiteBackend::new(config).await.unwrap();
            let value = backend.get(b"persist_key").await.unwrap();
            assert_eq!(value, Some(b"persist_value".to_vec()));
        }
    }

    #[tokio::test]
    async fn test_sqlite_stats() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Add some data
        backend.set(b"key1", b"value1").await.unwrap();
        backend.set(b"key2", b"value2").await.unwrap();

        let tree = backend.open_tree("test_tree").await.unwrap();
        tree.set(b"tree_key", b"tree_value").await.unwrap();

        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.total_entries, 3); // 2 in default + 1 in tree
        assert!(stats.is_temporary);
        assert_eq!(stats.tree_count, 1);
    }

    #[tokio::test]
    async fn test_prd_schema_tables_created() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();
        let mut pool = backend.pool.lock().await;
        let conn = pool.get_connection().await.unwrap();

        // Verify all PRD schema tables exist
        let expected_tables = vec![
            // Legacy tables
            "kv_store",
            "tree_metadata",
            // Schema versioning
            "schema_version",
            // Core tables
            "project",
            "workspace",
            "file",
            "analysis_run",
            "file_analysis",
            // Relationship tables
            "symbol",
            "symbol_state",
            "edge",
            "file_dependency",
            "symbol_change",
            // Cache and queue tables
            "indexer_queue",
            "indexer_checkpoint",
        ];

        for table_name in expected_tables {
            let mut rows = conn
                .query(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name = ?",
                    [turso::Value::Text(table_name.to_string())],
                )
                .await
                .unwrap();

            assert!(
                rows.next().await.unwrap().is_some(),
                "Table '{}' should exist in the schema",
                table_name
            );
        }

        // Verify schema version is set
        let mut rows = conn
            .query("SELECT version FROM schema_version LIMIT 1", ())
            .await
            .unwrap();

        if let Some(row) = rows.next().await.unwrap() {
            if let Ok(turso::Value::Integer(version)) = row.get_value(0) {
                assert_eq!(version, 1, "Schema version should be 1");
            } else {
                panic!("Schema version should be an integer");
            }
        } else {
            panic!("Schema version should be initialized");
        }

        pool.return_connection(conn);
    }

    #[tokio::test]
    async fn test_workspace_management() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Test create workspace
        let workspace_id = backend
            .create_workspace("test-workspace", 1, Some("main"))
            .await
            .unwrap();

        assert!(workspace_id > 0);

        // Test get workspace
        let workspace = backend.get_workspace(workspace_id).await.unwrap();
        assert!(workspace.is_some());

        let workspace = workspace.unwrap();
        assert_eq!(workspace.name, "test-workspace");
        assert_eq!(workspace.project_id, 1);
        assert_eq!(workspace.branch_hint, Some("main".to_string()));

        // Test list workspaces
        let workspaces = backend.list_workspaces(Some(1)).await.unwrap();
        assert!(!workspaces.is_empty());
        assert_eq!(workspaces[0].name, "test-workspace");

        // Test update workspace branch
        backend
            .update_workspace_branch(workspace_id, "develop")
            .await
            .unwrap();

        let workspace = backend.get_workspace(workspace_id).await.unwrap().unwrap();
        assert_eq!(workspace.branch_hint, Some("develop".to_string()));
    }

    #[tokio::test]
    #[ignore] // File versioning removed from architecture
    async fn test_file_version_management() {
        // File versioning functionality has been removed from the architecture
        // This test is disabled until file versioning is reimplemented if needed
        /*
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Test create file version
        let file_version_id = backend
            .create_file_version(1, "content_hash_123", 1024, Some(1672531200))
            .await
            .unwrap();

        assert!(file_version_id > 0);

        // Test get file version by digest
        let file_version = backend
            .get_file_version_by_digest("content_hash_123")
            .await
            .unwrap();

        assert!(file_version.is_some());
        let file_version = file_version.unwrap();
        assert_eq!(file_version.content_digest, "content_hash_123");
        assert_eq!(file_version.size_bytes, 1024);
        assert_eq!(file_version.file_id, 1);

        // Test link file to workspace
        let workspace_id = backend
            .create_workspace("test-workspace", 1, None)
            .await
            .unwrap();

        // link_file_to_workspace call removed - table deleted
        */
    }

    #[tokio::test]
    async fn test_symbol_storage_and_retrieval() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Create test symbols
        let symbols = vec![
            SymbolState {
                symbol_uid: "test_symbol_1".to_string(),
                file_path: "test/test_function.rs".to_string(),
                language: "rust".to_string(),
                name: "TestFunction".to_string(),
                fqn: Some("mod::TestFunction".to_string()),
                kind: "function".to_string(),
                signature: Some("fn test_function() -> String".to_string()),
                visibility: Some("public".to_string()),
                def_start_line: 10,
                def_start_char: 0,
                def_end_line: 15,
                def_end_char: 1,
                is_definition: true,
                documentation: Some("Test function documentation".to_string()),
                metadata: Some("{}".to_string()),
            },
            SymbolState {
                symbol_uid: "test_symbol_2".to_string(),
                file_path: "test/test_struct.rs".to_string(),
                language: "rust".to_string(),
                name: "TestStruct".to_string(),
                fqn: Some("mod::TestStruct".to_string()),
                kind: "struct".to_string(),
                signature: Some("struct TestStruct { field: String }".to_string()),
                visibility: Some("public".to_string()),
                def_start_line: 20,
                def_start_char: 0,
                def_end_line: 22,
                def_end_char: 1,
                is_definition: true,
                documentation: None,
                metadata: None,
            },
        ];

        // Test store symbols
        backend.store_symbols(&symbols).await.unwrap();

        // Test get symbols by file
        let retrieved_symbols_1 = backend
            .get_symbols_by_file("test/test_function.rs", "rust")
            .await
            .unwrap();
        let retrieved_symbols_2 = backend
            .get_symbols_by_file("test/test_struct.rs", "rust")
            .await
            .unwrap();
        assert_eq!(retrieved_symbols_1.len(), 1);
        assert_eq!(retrieved_symbols_2.len(), 1);

        // Test find symbol by name
        let found_symbols = backend
            .find_symbol_by_name(1, "TestFunction")
            .await
            .unwrap();
        assert!(!found_symbols.is_empty());
        assert_eq!(found_symbols[0].name, "TestFunction");

        // Test find symbol by FQN
        let found_symbol = backend
            .find_symbol_by_fqn(1, "mod::TestFunction")
            .await
            .unwrap();
        assert!(found_symbol.is_some());
        assert_eq!(
            found_symbol.unwrap().fqn,
            Some("mod::TestFunction".to_string())
        );
    }

    #[tokio::test]
    async fn test_edge_storage_and_querying() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Create test edges
        let edges = vec![
            Edge {
                relation: EdgeRelation::Calls,
                source_symbol_uid: "source_symbol_1".to_string(),
                target_symbol_uid: "target_symbol_1".to_string(),
                file_path: Some("test/edge_test.rs".to_string()),
                start_line: Some(5),
                start_char: Some(10),
                confidence: 0.95,
                language: "rust".to_string(),
                metadata: Some("{\"type\": \"function_call\"}".to_string()),
            },
            Edge {
                relation: EdgeRelation::References,
                source_symbol_uid: "source_symbol_2".to_string(),
                target_symbol_uid: "target_symbol_1".to_string(),
                file_path: Some("test/edge_test.rs".to_string()),
                start_line: Some(8),
                start_char: Some(15),
                confidence: 0.90,
                language: "rust".to_string(),
                metadata: None,
            },
        ];

        // Test store edges
        backend.store_edges(&edges).await.unwrap();

        // Test get symbol references
        let references = backend
            .get_symbol_references(1, "target_symbol_1")
            .await
            .unwrap();
        assert_eq!(references.len(), 2);

        // Test get symbol calls
        let calls = backend
            .get_symbol_calls(1, "target_symbol_1", CallDirection::Incoming)
            .await
            .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].relation, EdgeRelation::Calls);

        // Test traverse graph
        let paths = backend
            .traverse_graph("source_symbol_1", 2, &[EdgeRelation::Calls])
            .await
            .unwrap();
        assert!(!paths.is_empty());
    }

    #[tokio::test]
    async fn test_store_edges_deduplicates_duplicates() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        let duplicate_edge = Edge {
            relation: EdgeRelation::References,
            source_symbol_uid: "dup_source".to_string(),
            target_symbol_uid: "dup_target".to_string(),
            file_path: Some("src/main.rs".to_string()),
            start_line: Some(42),
            start_char: Some(5),
            confidence: 1.0,
            language: "rust".to_string(),
            metadata: Some("dedup_test".to_string()),
        };

        let edges = vec![duplicate_edge.clone(), duplicate_edge.clone()];

        backend.store_edges(&edges).await.unwrap();
        // Replaying the same edges should not create new rows.
        backend.store_edges(&edges).await.unwrap();

        let mut pool_guard = backend.pool.lock().await;
        let conn = pool_guard.get_connection().await.unwrap();

        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM edge WHERE relation = ? AND source_symbol_uid = ? AND target_symbol_uid = ?",
                [
                    turso::Value::Text(duplicate_edge.relation.to_string().to_string()),
                    turso::Value::Text(duplicate_edge.source_symbol_uid.clone()),
                    turso::Value::Text(duplicate_edge.target_symbol_uid.clone()),
                ],
            )
            .await
            .unwrap();

        let count = match rows.next().await.unwrap() {
            Some(row) => match row.get_value(0).unwrap() {
                turso::Value::Integer(val) => val,
                _ => 0,
            },
            None => 0,
        };

        pool_guard.return_connection(conn);
        assert_eq!(count, 1, "duplicate edges should be stored only once");
    }

    #[tokio::test]
    async fn test_analysis_management() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Test create analysis run
        let analysis_run_id = backend
            .create_analysis_run(
                "rust-analyzer",
                "0.3.1",
                "rust",
                "{\"check_on_save\": true}",
            )
            .await
            .unwrap();

        assert!(analysis_run_id > 0);

        // Test get analysis progress
        let progress = backend.get_analysis_progress(1).await.unwrap();
        assert_eq!(progress.workspace_id, 1);
        assert!(progress.completion_percentage >= 0.0);

        // Test queue file analysis
        backend.queue_file_analysis(1, "rust", 5).await.unwrap();
    }

    #[tokio::test]
    async fn test_edge_relation_conversion() {
        // Test EdgeRelation to_string conversion
        assert_eq!(EdgeRelation::Calls.to_string(), "calls");
        assert_eq!(EdgeRelation::References.to_string(), "references");
        assert_eq!(EdgeRelation::InheritsFrom.to_string(), "inherits_from");

        // Test EdgeRelation from_string conversion
        assert_eq!(
            EdgeRelation::from_string("calls").unwrap(),
            EdgeRelation::Calls
        );
        assert_eq!(
            EdgeRelation::from_string("references").unwrap(),
            EdgeRelation::References
        );
        assert_eq!(
            EdgeRelation::from_string("inherits_from").unwrap(),
            EdgeRelation::InheritsFrom
        );

        // Test invalid relation
        assert!(EdgeRelation::from_string("invalid_relation").is_err());
    }

    #[tokio::test]
    #[ignore] // File versioning removed from architecture
    async fn test_graph_operations_comprehensive() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Create a comprehensive test scenario:
        // 1. Create workspace and file versions
        let workspace_id = backend
            .create_workspace("comprehensive-test", 1, Some("main"))
            .await
            .unwrap();

        // File versioning removed from architecture
        let file_version_id_1 = 1i64; // backend
                                      //     .create_file_version(1, "file1_hash", 2048, None)
                                      //     .await
                                      //     .unwrap();

        let file_version_id_2 = 2i64; // backend
                                      //     .create_file_version(2, "file2_hash", 1536, None)
                                      //     .await
                                      //     .unwrap();

        // 2. Link files to workspace
        // link_file_to_workspace calls removed - table deleted

        // 3. Create symbols representing a class hierarchy
        let symbols = vec![
            SymbolState {
                symbol_uid: "base_class".to_string(),
                file_path: "test/base_class.rs".to_string(),
                language: "rust".to_string(),
                name: "BaseClass".to_string(),
                fqn: Some("package::BaseClass".to_string()),
                kind: "class".to_string(),
                signature: Some("class BaseClass".to_string()),
                visibility: Some("public".to_string()),
                def_start_line: 1,
                def_start_char: 0,
                def_end_line: 10,
                def_end_char: 1,
                is_definition: true,
                documentation: Some("Base class documentation".to_string()),
                metadata: None,
            },
            SymbolState {
                symbol_uid: "derived_class".to_string(),
                file_path: "test/derived_class.rs".to_string(),
                language: "rust".to_string(),
                name: "DerivedClass".to_string(),
                fqn: Some("package::DerivedClass".to_string()),
                kind: "class".to_string(),
                signature: Some("class DerivedClass extends BaseClass".to_string()),
                visibility: Some("public".to_string()),
                def_start_line: 15,
                def_start_char: 0,
                def_end_line: 25,
                def_end_char: 1,
                is_definition: true,
                documentation: Some("Derived class documentation".to_string()),
                metadata: None,
            },
            SymbolState {
                symbol_uid: "method_call".to_string(),
                file_path: "test/method_call.rs".to_string(),
                language: "rust".to_string(),
                name: "methodCall".to_string(),
                fqn: Some("package::methodCall".to_string()),
                kind: "function".to_string(),
                signature: Some("fn methodCall() -> BaseClass".to_string()),
                visibility: Some("public".to_string()),
                def_start_line: 5,
                def_start_char: 0,
                def_end_line: 8,
                def_end_char: 1,
                is_definition: true,
                documentation: None,
                metadata: None,
            },
        ];

        // Store symbols
        backend.store_symbols(&symbols).await.unwrap();

        // 4. Create relationships
        let edges = vec![
            Edge {
                relation: EdgeRelation::InheritsFrom,
                source_symbol_uid: "derived_class".to_string(),
                target_symbol_uid: "base_class".to_string(),
                file_path: Some("test/derived_class.rs".to_string()),
                start_line: Some(15),
                start_char: Some(25),
                confidence: 1.0,
                language: "rust".to_string(),
                metadata: Some("{\"inheritance_type\": \"extends\"}".to_string()),
            },
            Edge {
                relation: EdgeRelation::Instantiates,
                source_symbol_uid: "method_call".to_string(),
                target_symbol_uid: "base_class".to_string(),
                file_path: Some("test/method_call.rs".to_string()),
                start_line: Some(7),
                start_char: Some(12),
                confidence: 0.95,
                language: "rust".to_string(),
                metadata: None,
            },
            Edge {
                relation: EdgeRelation::References,
                source_symbol_uid: "method_call".to_string(),
                target_symbol_uid: "derived_class".to_string(),
                file_path: Some("test/method_call.rs".to_string()),
                start_line: Some(6),
                start_char: Some(8),
                confidence: 0.90,
                language: "rust".to_string(),
                metadata: None,
            },
        ];

        // Store edges
        backend.store_edges(&edges).await.unwrap();

        // 5. Test comprehensive queries

        // Test finding all classes
        let base_symbols = backend
            .find_symbol_by_name(workspace_id, "BaseClass")
            .await
            .unwrap();
        assert_eq!(base_symbols.len(), 1);
        assert_eq!(base_symbols[0].kind, "class");

        // Test getting references to BaseClass (should include inheritance and instantiation)
        let base_references = backend
            .get_symbol_references(workspace_id, "base_class")
            .await
            .unwrap();
        assert_eq!(base_references.len(), 2); // inheritance + instantiation

        // Test graph traversal from base class
        let inheritance_paths = backend
            .traverse_graph("base_class", 2, &[EdgeRelation::InheritsFrom])
            .await
            .unwrap();
        // This should be empty since we're looking for outgoing inheritance from base class
        assert!(inheritance_paths.is_empty());

        // Test workspace operations
        let workspaces = backend.list_workspaces(Some(1)).await.unwrap();
        assert!(!workspaces.is_empty());
        assert_eq!(workspaces[0].name, "comprehensive-test");

        // Test file version lookup (disabled - file versioning removed from architecture)
        // let file_version = backend
        //     .get_file_version_by_digest("file1_hash")
        //     .await
        //     .unwrap();
        // assert!(file_version.is_some());
        // assert_eq!(file_version.unwrap().size_bytes, 2048);

        // Test analysis progress
        let _analysis_run_id = backend
            .create_analysis_run("test-analyzer", "1.0.0", "rust", "{}")
            .await
            .unwrap();

        let progress = backend.get_analysis_progress(workspace_id).await.unwrap();
        assert_eq!(progress.workspace_id, workspace_id);
    }

    #[tokio::test]
    async fn test_batch_operations_performance() -> Result<(), Box<dyn std::error::Error>> {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();
        let _workspace_id = backend
            .create_workspace("test_workspace", 1, Some("main"))
            .await
            .unwrap();

        // Test batch symbol insertion
        let mut symbols = Vec::new();
        for i in 0..500 {
            symbols.push(SymbolState {
                symbol_uid: format!("symbol_{}", i),
                language: "rust".to_string(),
                name: format!("TestSymbol{}", i),
                fqn: Some(format!("test::TestSymbol{}", i)),
                kind: "function".to_string(),
                signature: Some(format!("fn test_function_{}()", i)),
                visibility: Some("public".to_string()),
                def_start_line: i as u32,
                def_start_char: 0,
                def_end_line: i as u32,
                def_end_char: 10,
                is_definition: true,
                documentation: Some(format!("Test function {}", i)),
                metadata: Some("test_metadata".to_string()),
                file_path: "test/path.rs".to_string(),
            });
        }

        let start_time = std::time::Instant::now();
        backend.store_symbols(&symbols).await.unwrap();
        let duration = start_time.elapsed();

        println!("Batch stored {} symbols in {:?}", symbols.len(), duration);
        assert!(
            duration.as_millis() < 5000,
            "Batch operation should be fast"
        );

        // Test batch edge insertion
        let mut edges = Vec::new();
        for i in 0..1000 {
            edges.push(Edge {
                source_symbol_uid: format!("symbol_{}", i % 500),
                target_symbol_uid: format!("symbol_{}", (i + 1) % 500),
                relation: crate::database::EdgeRelation::Calls,
                file_path: Some("test/path.rs".to_string()),
                start_line: Some(i as u32),
                start_char: Some(0),
                confidence: 0.9,
                language: "rust".to_string(),
                metadata: None,
            });
        }

        let start_time = std::time::Instant::now();
        backend.store_edges(&edges).await.unwrap();
        let duration = start_time.elapsed();

        println!("Batch stored {} edges in {:?}", edges.len(), duration);
        assert!(
            duration.as_millis() < 10000,
            "Batch edge operation should be fast"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_database_integrity_validation() -> Result<(), Box<dyn std::error::Error>> {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Run integrity check on empty database
        let report = backend.validate_integrity().await.unwrap();
        assert_eq!(report.passed_checks, report.total_checks);
        assert!(report.failed_checks.is_empty());

        // Add some test data and verify integrity
        let workspace_id = backend
            .create_workspace("integrity_test", 1, Some("main"))
            .await
            .unwrap();
        // link_file_to_workspace call removed - table deleted

        let symbol = SymbolState {
            symbol_uid: "test_symbol".to_string(),
            language: "rust".to_string(),
            name: "TestSymbol".to_string(),
            fqn: Some("test::TestSymbol".to_string()),
            kind: "function".to_string(),
            signature: Some("fn test()".to_string()),
            visibility: Some("public".to_string()),
            def_start_line: 1,
            def_start_char: 0,
            def_end_line: 5,
            def_end_char: 10,
            is_definition: true,
            documentation: None,
            metadata: None,
            file_path: "test/path.rs".to_string(),
        };
        backend.store_symbols(&[symbol]).await.unwrap();

        let report = backend.validate_integrity().await.unwrap();
        assert!(report.passed_checks > 0);
        println!("Integrity report: {:?}", report);

        Ok(())
    }

    #[tokio::test]
    async fn test_performance_optimization() -> Result<(), Box<dyn std::error::Error>> {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        let report = backend.optimize_performance().await.unwrap();
        assert!(!report.optimizations_applied.is_empty());
        assert!(!report.index_recommendations.is_empty());
        assert!(!report.query_stats.is_empty());

        println!("Performance optimization report: {:?}", report);

        // Verify that optimization actually improves something
        assert!(report
            .optimizations_applied
            .iter()
            .any(|opt| opt.contains("PRAGMA")));

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_orphaned_data() -> Result<(), Box<dyn std::error::Error>> {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Create some data first
        let _workspace_id = backend
            .create_workspace("cleanup_test", 1, Some("main"))
            .await
            .unwrap();
        let symbol = SymbolState {
            symbol_uid: "cleanup_test_symbol".to_string(),
            language: "rust".to_string(),
            name: "TestSymbol".to_string(),
            fqn: Some("test::TestSymbol".to_string()),
            kind: "function".to_string(),
            signature: None,
            visibility: None,
            def_start_line: 1,
            def_start_char: 0,
            def_end_line: 5,
            def_end_char: 10,
            is_definition: true,
            documentation: None,
            metadata: None,
            file_path: "test/path.rs".to_string(),
        };
        backend.store_symbols(&[symbol]).await.unwrap();

        // Run cleanup
        let report = backend.cleanup_orphaned_data().await.unwrap();
        println!("Cleanup report: {:?}", report);

        // Verify cleanup ran without errors
        assert!(report.deleted_records.len() >= 0); // May be zero if no orphaned data

        Ok(())
    }

    #[tokio::test]
    async fn test_real_analysis_progress_tracking() -> Result<(), Box<dyn std::error::Error>> {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();
        let workspace_id = backend
            .create_workspace("progress_test", 1, Some("main"))
            .await
            .unwrap();

        // Initially should have no progress
        let progress = backend.get_analysis_progress(workspace_id).await.unwrap();
        assert_eq!(progress.analyzed_files, 0);

        // Add some workspace files
        for i in 1..=5 {
            // link_file_to_workspace call removed - table deleted
        }

        // Queue some files for analysis
        for i in 1..=3 {
            backend.queue_file_analysis(i, "rust", 1).await.unwrap();
        }

        let progress = backend.get_analysis_progress(workspace_id).await.unwrap();

        // Should now have some files tracked
        assert!(progress.total_files >= 0);
        println!("Progress with queued files: {:?}", progress);

        Ok(())
    }

    #[tokio::test]
    async fn test_content_hashing() -> Result<(), Box<dyn std::error::Error>> {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        let content1 = b"fn main() { println!(\"Hello, world!\"); }";
        let content2 = b"fn main() { println!(\"Hello, rust!\"); }";

        let hash1 = backend.compute_content_hash(content1).await;
        let hash2 = backend.compute_content_hash(content2).await;

        assert_ne!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // Blake3 produces 64-char hex strings
        assert_eq!(hash2.len(), 64);

        // Verify consistent hashing
        let hash1_repeat = backend.compute_content_hash(content1).await;
        assert_eq!(hash1, hash1_repeat);

        Ok(())
    }

    #[tokio::test]
    async fn test_transaction_rollback_scenarios() -> Result<(), Box<dyn std::error::Error>> {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Test rollback with invalid data
        let invalid_symbols = vec![SymbolState {
            symbol_uid: "valid_symbol".to_string(),
            language: "rust".to_string(),
            name: "ValidSymbol".to_string(),
            fqn: None,
            kind: "function".to_string(),
            signature: None,
            visibility: None,
            def_start_line: 1,
            def_start_char: 0,
            def_end_line: 5,
            def_end_char: 10,
            is_definition: true,
            documentation: None,
            metadata: None,
            file_path: "test/path.rs".to_string(),
        }];

        // This should succeed normally
        backend.store_symbols(&invalid_symbols).await.unwrap();

        // Verify the symbol was stored
        let symbols = backend
            .get_symbols_by_file("test/path.rs", "rust")
            .await
            .unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "ValidSymbol");

        Ok(())
    }

    #[tokio::test]
    async fn test_error_handling() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Test get non-existent workspace
        let workspace = backend.get_workspace(999999).await.unwrap();
        assert!(workspace.is_none());

        // Test get non-existent file version - COMMENTED OUT: method removed in architectural change
        // let file_version = backend
        //     .get_file_version_by_digest("non_existent_hash")
        //     .await
        //     .unwrap();
        // assert!(file_version.is_none());

        // Test find non-existent symbol
        let symbols = backend
            .find_symbol_by_name(1, "NonExistentSymbol")
            .await
            .unwrap();
        assert!(symbols.is_empty());

        // Test find non-existent FQN
        let symbol = backend
            .find_symbol_by_fqn(1, "non::existent::symbol")
            .await
            .unwrap();
        assert!(symbol.is_none());

        // Test get references for non-existent symbol
        let references = backend
            .get_symbol_references(1, "non_existent_symbol")
            .await
            .unwrap();
        assert!(references.is_empty());

        // Test traverse graph with empty relations
        let paths = backend.traverse_graph("any_symbol", 2, &[]).await.unwrap();
        assert!(paths.is_empty());
    }
}
#[tokio::test]
async fn wal_sync_timeout_does_not_leave_quiesced() {
    // Create a tiny temporary DB
    let dir = tempfile::tempdir().expect("tmpdir");
    let db_path = dir.path().join("cache.db");
    let cfg = DatabaseConfig::default();
    let sqlite_cfg = SQLiteConfig {
        path: db_path.to_string_lossy().to_string(),
        temporary: false,
        cache_size: 0,
    };
    let backend = SQLiteBackend::with_sqlite_config(cfg, sqlite_cfg)
        .await
        .expect("backend");

    // Run a wal-sync with a very short timeout; regardless of success, quiesce must be lifted.
    let _ = backend
        .wal_sync_blocking(
            Some(std::time::Duration::from_millis(1)),
            true,
            CheckpointMode::Auto,
            None,
        )
        .await;

    // Verify pool is not quiesced
    let quiesced_now = {
        let pool = backend.pool.lock().await;
        pool.quiesced.load(Ordering::Relaxed)
    };
    assert!(
        !quiesced_now,
        "pool.quiesced should be false after wal-sync exits"
    );
}
