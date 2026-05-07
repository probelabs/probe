# Probe dogfood — finding inventory (first run)

Date: 2026-05-07
Branch: feat/proof-rust-dogfood
Total findings: 49

## By signal class

### panic_risk (12 findings)
- lsp-daemon/src/daemon.rs (SYS-REQ-002) — `.expect()` (panics on Err/None)
- src/bert_reranker.rs (SYS-REQ-009) — slice indexing (panics on out-of-bounds)
- src/config.rs (SYS-REQ-008) — slice indexing (panics on out-of-bounds)
- src/grep.rs (SYS-REQ-003) — `unreachable!` macro
- src/language/tree_cache.rs (SYS-REQ-005) — `.unwrap()` (panics on Err/None)
- src/lsp_integration/client.rs (SYS-REQ-006) — slice indexing (panics on out-of-bounds)
- src/lsp_integration/management.rs (SYS-REQ-006) — slice indexing (panics on out-of-bounds)
- src/main.rs:197 (SYS-REQ-006, SYS-REQ-007 — AMBIGUOUS) — `.unwrap()` (panics on Err/None)
- src/path_resolver/go.rs (SYS-REQ-004) — slice indexing (panics on out-of-bounds)
- src/path_resolver/javascript.rs (SYS-REQ-004) — slice indexing (panics on out-of-bounds)
- src/path_resolver/rust.rs (SYS-REQ-004) — slice indexing (panics on out-of-bounds)
- src/ranking.rs:374 (SYS-REQ-001, SYS-REQ-007 — AMBIGUOUS) — slice indexing (panics on out-of-bounds)

### error_discarded (8 findings)
- lsp-daemon/src/daemon.rs (SYS-REQ-002) — `.ok()` on Result
- lsp-daemon/src/server_manager.rs (SYS-REQ-002) — `.ok()` on Result
- src/bert_reranker.rs (SYS-REQ-009) — `let _ = <call>`
- src/grep.rs (SYS-REQ-003) — `let _ = <call>`
- src/lsp_integration/client.rs (SYS-REQ-006) — `.ok()` on Result
- src/lsp_integration/management.rs (SYS-REQ-006) — `let _ = <call>`
- src/main.rs:325 (SYS-REQ-006, SYS-REQ-007 — AMBIGUOUS) — `.ok()` on Result
- src/path_safety.rs (SYS-REQ-003) — `.ok()` on Result

### filesystem_dependency (10 findings)
- lsp-daemon/src/daemon.rs (SYS-REQ-002) — `std::fs::read_to_string`
- src/bert_reranker.rs (SYS-REQ-009) — `std::fs::read_to_string`
- src/config.rs (SYS-REQ-008) — `std::fs::create_dir_all`
- src/grep.rs (SYS-REQ-003) — `std::fs::File::open`
- src/lsp_integration/client.rs (SYS-REQ-006) — `tokio::fs::read_dir`
- src/lsp_integration/management.rs (SYS-REQ-006) — `std::fs::remove_file`
- src/main.rs:1102 (SYS-REQ-006, SYS-REQ-007 — AMBIGUOUS) — `std::fs::read_to_string`
- src/path_resolver/javascript.rs (SYS-REQ-004) — `std::fs::write`
- src/path_resolver/rust.rs (SYS-REQ-004) — `std::fs::read_dir`
- src/path_safety.rs (SYS-REQ-003) — `std::fs::symlink_metadata`

### process_dependency (0 findings)
None detected by the scanner. Note: SYS-REQ-004 covers `Command::new` callers in `src/path_resolver/{rust,go,javascript}.rs`, but the Rust scanner did not flag any `process_local`/`Command::new` signals — likely a scanner gap (worth noting for the engine team).

### db_read_dependency / db_write_dependency / database_dependency (0 findings)
None detected. SYS-REQ-006 covers Turso/SQLite usage but the Rust scanner did not flag any database signals — also likely a scanner gap (probe uses `turso`/`libsql`/raw SQL builders that the Go-centric pattern set may not match).

### concurrency_spawn / channel_communication / sync_primitive (6 findings)
**concurrency_spawn (4):**
- lsp-daemon/src/daemon.rs (SYS-REQ-002) — `tokio::spawn`
- lsp-daemon/src/pool.rs (SYS-REQ-002) — `tokio::spawn`
- lsp-daemon/src/server_manager.rs (SYS-REQ-002) — `tokio::spawn`
- src/bert_reranker.rs (SYS-REQ-009) — `std::thread::spawn`

**channel_communication (2):**
- lsp-daemon/src/daemon.rs (SYS-REQ-002) — `tokio::sync::mpsc::channel`
- lsp-daemon/src/server_manager.rs (SYS-REQ-002) — `tokio::sync::broadcast::channel`

**sync_primitive (0):** none flagged.

### time_dependency / random_dependency / environment_dependency (0 findings)
None flagged. SYS-REQ-008 (config_loader) carries `environment` tag but the scanner did not find a direct env signal in `src/config.rs`. Probable scanner gap or env reads happen behind a wrapper.

### http_client_dependency / http_server_dependency / network_dependency (0 findings)
None flagged. probe's network surface is JSON-RPC over stdio/socket to LSP servers, not HTTP — these classes are correctly silent here.

### lossy_string_conversion / unsafe_block (10 findings)
**lossy_string_conversion (7):**
- lsp-daemon/src/daemon.rs (SYS-REQ-002) — `.to_string_lossy()`
- src/lsp_integration/client.rs (SYS-REQ-006) — `.to_string_lossy()`
- src/lsp_integration/management.rs (SYS-REQ-006) — `.to_string_lossy()`
- src/main.rs:451 (SYS-REQ-006, SYS-REQ-007 — AMBIGUOUS) — `.to_string_lossy()`
- src/path_resolver/go.rs (SYS-REQ-004) — `String::from_utf8_lossy`
- src/path_resolver/javascript.rs (SYS-REQ-004) — `String::from_utf8_lossy`
- src/path_resolver/rust.rs (SYS-REQ-004) — `.to_string_lossy()`

**unsafe_block (3):**
- lsp-daemon/src/daemon.rs (SYS-REQ-002) — `unsafe` block
- lsp-daemon/src/server_manager.rs (SYS-REQ-002) — `unsafe` block
- src/bert_reranker.rs (SYS-REQ-009) — `unsafe` block

(Note: SYS-REQ-009 was authored on the assumption that `bert_reranker.rs` is the **only** unsafe site. The scanner found 3 — see "Notes for follow-up agents".)

### permission_window / toctou_pair / path_string_equality / trust_boundary_resolution (3 findings)
**toctou_pair (3):**
- src/config.rs (SYS-REQ-008) — `.exists` then later use
- src/lsp_integration/management.rs (SYS-REQ-006) — `std::fs::remove_file` paired with later use
- src/main.rs:1097 (SYS-REQ-006, SYS-REQ-007 — AMBIGUOUS) — `.exists` then later use

**permission_window / path_string_equality / trust_boundary_resolution (0):** none flagged.

## Top 10 owning requirements by finding count

(Counts ambiguous findings against each candidate owner.)

| Requirement | Finding count |
|---|---|
| SYS-REQ-006 (symbol_cache) | 14 |
| SYS-REQ-002 (lsp_daemon) | 12 |
| SYS-REQ-004 (subprocess_runner) | 8 |
| SYS-REQ-007 (concurrent_search) | 6 |
| SYS-REQ-003 (fs_traversal) | 5 |
| SYS-REQ-009 (bert_reranker) | 5 |
| SYS-REQ-008 (config_loader) | 3 |
| SYS-REQ-005 (tree_sitter_parser) | 1 |
| SYS-REQ-001 (search_engine) | 1 |
| SYS-REQ-1086 (n/a) | 0 |

## Notes for follow-up agents

1. **`unsafe_block` count is 3, not 1.** The original SYS-REQ-009 description claimed `src/bert_reranker.rs` is "the only unsafe site". The Rust scanner flagged unsafe blocks in `lsp-daemon/src/daemon.rs` and `lsp-daemon/src/server_manager.rs` as well. Agents covering memory-safety obligations need to inspect those two files and either decompose SYS-REQ-002 to add an `unsafe`-aware child or author distinct memory-safety system requirements per crate.

2. **`src/main.rs` is shared by SYS-REQ-006 + SYS-REQ-007** (5 ambiguous findings: panic_risk:197, error_discarded:325, lossy_string_conversion:451, toctou_pair:1097, filesystem_dependency:1102). Same artifact owns two requirements with different obligation classes (database vs. concurrent). Follow-up agents must narrow `implemented_by` to the actual owning function — not the whole file — before adding covering requirements.

3. **`src/ranking.rs:374` is shared by SYS-REQ-001 + SYS-REQ-007** (1 ambiguous panic_risk finding). Same problem class as above.

4. **Scanner gaps surfaced (NOT for follow-up agents to fix here, but worth noting):**
   - `Command::new` / process spawn signals not flagged in `src/path_resolver/*` despite SYS-REQ-004 explicitly covering subprocess invocation.
   - SQLite/Turso `db_read`/`db_write` signals not flagged in any artifact, despite SYS-REQ-006 covering the Turso symbol cache.
   - Environment variable reads not flagged in `src/config.rs`.
   These should be fed back to the Rust archsignals scanner team — probe is a useful test case for filling those scanner gaps. (Filed in `docs/internal/session-findings.md` if applicable.)

5. **Heavy concentrations:** SYS-REQ-002 (lsp_daemon) has 12 findings spanning 5 distinct signal classes (channel_communication, concurrency_spawn, error_discarded, filesystem_dependency, lossy_string_conversion, panic_risk, unsafe_block). It is a good candidate for decomposition into sub-requirements before authoring covering requirements per signal class.

6. **`src/path_resolver/{rust,go,javascript}.rs` (SYS-REQ-004)** all have the same triple of signals (panic_risk + lossy_string_conversion + filesystem_dependency). Consider a single covering requirement-pattern that decomposes once and applies to all three resolver files.

7. **Other audit findings unrelated to code-signal:** baseline reports 5 errors + 14 warnings across stages: `l0_stakeholder_complete`, `l1_system_complete`, `obligation_baseline`, `quality_clean` (missing rationale), `system_requirements_linked` (no `satisfies` edge), `variables_declared` (20 solver preflight errors), `verify_passes` (9 components fail realizability), `cross_level_complete` (0%), `documentation_coverage` (0%). These are seed-spec teething issues, not code-signal findings, and are intentionally left for future passes.
