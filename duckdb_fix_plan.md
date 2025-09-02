# DuckDB Initialization Deadlock Fix Plan

## Problem Analysis

After migrating from sled to DuckDB, 52 out of 278 tests are hanging due to DuckDB initialization deadlocks. All tests were passing before sled removal, indicating the issue is in the DuckDB backend initialization.

## Root Causes (Big Brain Analysis)

1. **DDL during pool creation**: Multiple connections executing schema concurrently, causing exclusive lock contention
2. **Non-idempotent schema gaps**: Race conditions during CREATE TABLE operations
3. **Pre-populating connection pool**: Every connection tries to run DDL, amplifying contention
4. **Global PRAGMAs inside schema**: `PRAGMA threads=8;` executes repeatedly, requiring locks
5. **Shared DB files across tests**: Multiple test threads bootstrapping same catalog concurrently

## Implementation Plan

### Phase 1: Make Schema Idempotent and Transactional

**File**: `lsp-daemon/src/database/duckdb_schema.sql`

**Changes needed**:
- Wrap entire DDL in single transaction (`BEGIN TRANSACTION;` ... `COMMIT;`)
- Ensure all CREATE statements use `IF NOT EXISTS`
- Remove global `PRAGMA threads=8;` from schema file
- Keep DDL batch minimal and atomic

**Benefits**:
- Prevents duplicate object creation races
- Minimal lock time with single transaction
- Removes global setting contention

### Phase 2: Add Process-Local Guard

**File**: `lsp-daemon/src/database/duckdb_backend.rs`

**Changes needed**:
- Add `init_guard: Arc<OnceCell<()>>` field to DuckDBBackend
- Use OnceCell to ensure schema runs once per process
- Prevents redundant schema initialization within single process

### Phase 3: Create Bootstrap Helper Module

**New File**: `lsp-daemon/src/database/duckdb_bootstrap.rs`

**Implementation**:
```rust
use std::path::Path;
use once_cell::sync::OnceCell;
use fs2::FileExt;
use duckdb::{Connection, Result as DuckDBResult};

/// Process-local guard to ensure bootstrap runs once per process
static BOOTSTRAP_GUARD: OnceCell<()> = OnceCell::new();

/// Bootstrap DuckDB database with schema in a safe, serialized manner
pub fn bootstrap_database(db_path: &Path) -> anyhow::Result<()> {
    // File lock for cross-process safety
    let lock_path = db_path.with_extension("lock");
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&lock_path)?;
    
    lock_file.lock_exclusive()?;
    
    // Process-local guard
    BOOTSTRAP_GUARD.get_or_try_init(|| -> anyhow::Result<()> {
        // Single connection bootstrap (no pool)
        let conn = Connection::open(db_path)?;
        
        // Execute schema in single transaction
        let schema = include_str!("duckdb_schema.sql");
        conn.execute_batch(schema)?;
        
        Ok(())
    })?;
    
    lock_file.unlock()?;
    Ok(())
}
```

**Features**:
- Single bootstrap connection (no pool)
- File lock for cross-process serialization
- OnceCell for per-process "run once" guarantee
- Atomic DDL execution

### Phase 4: Update DuckDB Backend Initialization

**File**: `lsp-daemon/src/database/duckdb_backend.rs`

**Changes needed**:
1. Add bootstrap call before pool creation:
```rust
// Bootstrap schema before creating pool
crate::database::duckdb_bootstrap::bootstrap_database(&db_path)?;
```

2. Update connection pool configuration:
```rust
// Test-friendly pool config
let pool = Pool::builder()
    .max_size(if cfg!(test) { 2 } else { pool_size })
    .min_idle(None) // No pre-population
    .connection_customizer(Box::new(DuckDBConnectionCustomizer))
    .build(manager)?;
```

3. Create connection customizer for PRAGMAs:
```rust
#[derive(Debug)]
struct DuckDBConnectionCustomizer;

impl CustomizeConnection<DuckdbConnection, duckdb::Error> for DuckDBConnectionCustomizer {
    fn on_acquire(&self, conn: &mut DuckdbConnection) -> Result<(), duckdb::Error> {
        // Set per-connection PRAGMAs (no DDL here!)
        conn.execute_batch(&format!(
            "PRAGMA threads={};", 
            if cfg!(test) { 1 } else { num_cpus::get().max(1) }
        ))?;
        Ok(())
    }
}
```

### Phase 5: Update Dependencies

**File**: `lsp-daemon/Cargo.toml`

**Add dependencies**:
```toml
fs2 = "0.4"
once_cell = "1.19"  # If not already present
```

### Phase 6: Update Module Exports

**File**: `lsp-daemon/src/database/mod.rs`

**Add**:
```rust
pub mod duckdb_bootstrap;
```

## Expected Results

After implementing all phases:
- **278/278 tests passing** (100% success rate restored)
- **No initialization deadlocks** or hangs
- **Safe concurrent test execution**
- **Deterministic DuckDB startup**
- **Maintained analytics capabilities**

## Implementation Strategy

1. **Phase-by-phase implementation** using @agent-architect
2. **Test after each phase** to ensure progress
3. **Rollback capability** if any phase breaks functionality
4. **Verification testing** after complete implementation

## Key Principles Applied

- **Single responsibility**: Bootstrap separate from pool management
- **Idempotency**: Safe to run bootstrap multiple times
- **Concurrency safety**: File locks + OnceCell for all scenarios
- **Test awareness**: Different configurations for test vs production
- **Minimal disruption**: Keep existing API intact

## Files Modified Summary

1. `lsp-daemon/src/database/duckdb_schema.sql` - Idempotent schema
2. `lsp-daemon/src/database/duckdb_backend.rs` - Bootstrap integration  
3. `lsp-daemon/src/database/duckdb_bootstrap.rs` - NEW: Safe bootstrap
4. `lsp-daemon/src/database/mod.rs` - Module exports
5. `lsp-daemon/Cargo.toml` - Dependencies

This plan addresses all root causes identified by big brain analysis and provides a systematic approach to restore 100% test success rate.