# LSP Indexing Reliability & Persistence Fix Plan

This document defines a complete, implementation‑ready plan to make LSP indexing produce the same high‑quality results you see with `extract --lsp`/search enrichment, and to persist those results consistently in the database for later use.

The plan is split into small, verifiable work items with code pointers, config changes, test strategy, acceptance criteria, and rollout guidance. It is designed so another engineer/agent can copy the steps and implement them reliably.

---

## 1) Context & Symptoms

- `extract --lsp` works well: you get call hierarchy and references reliably.
- Indexer “pre‑warm” LSP calls in Phase 1 don’t produce usable results for downstream queries.
- Indexer Phase 2 (LSP enrichment workers) often doesn’t enrich/persist references or call hierarchy as expected.

Observed user symptoms:
- “Indexer did not find references/call hierarchy” while `extract --lsp` did.
- DB doesn’t contain expected edges after indexing.

---

## 2) Root Cause Summary

1. Position accuracy mismatch in indexer
   - Phase 1 and Phase 2 use raw AST line/column or DB values without the per‑language/server cursor adjustments implemented by `extract`’s `PositionAnalyzer`.
   - Many LSP servers require the caret to be exactly on the identifier; being off by a few chars yields empty results.

2. Phase 1 “pre‑warm” doesn’t persist
   - The old universal cache was removed. `index_symbols_with_lsp` computes results but discards them (not written to DB, not cached persistently).
   - Status: Completed — indexing now persists call hierarchy and references to DB by default.

3. Phase 2 enrichment points at the wrong workspace DB
   - Worker startup/monitor uses `std::env::current_dir()` to fetch a DB cache. That can differ from the actual indexing workspace, resulting in “no orphan symbols” and no enrichment.

4. Phase 1 only calls call hierarchy
   - References are not invoked during Phase 1 even when enabled in config; this reduces warm coverage (if we keep Phase 1 warm path).
   - Status: Completed — references are also fetched and persisted during indexing.

5. Inconsistent gating and timeouts across phases
   - Phase 1 doesn’t fully honor `LspCachingConfig` flags/timeouts; search/extract do.
   - Status: Completed — timeouts and operation gating in Phase 1 now respect `LspCachingConfig` (operation enable/disable and `lsp_operation_timeout_ms`).

---

## 3) Objectives

1. Make indexer LSP calls as reliable as `extract --lsp` by fixing positions before calling servers.
2. Ensure enrichment results are persisted to DB (now persisted during indexing and by enrichment workers by default).
3. Ensure Phase 2 reads/writes the DB for the actual indexing workspace (not the current process CWD).
4. Respect `LspCachingConfig` knobs consistently (which ops to run; timeouts; limits).
5. Add observability to prove coverage and help future debugging.

---

## 4) Work Items (Implementation‑Ready)

### W1. Align LSP cursor positions before calling servers — Status: Completed

Notes:
- Introduced shared resolver `lsp_daemon::position::resolve_symbol_position(...)` and reused it everywhere (daemon + CLI), then applied any analyzer offset on top.

Goal: Use the same accuracy that `extract --lsp` achieves by placing caret on the identifier reliably.

- Code pointers
  - Phase 1: `lsp-daemon/src/indexing/manager.rs::index_symbols_with_lsp`
  - Phase 2: `lsp-daemon/src/indexing/lsp_enrichment_worker.rs` (calls to `server_manager.call_hierarchy(...)` and `server_manager.references(...)`).
  - Utilities you can leverage now:
    - `lsp-daemon/src/lsp_database_adapter.rs`:
      - `resolve_symbol_at_location(file_path, line, column, language) -> Result<String>` resolves the symbol UID using tree-sitter; adapt this flow to get corrected (line,column) on the identifier (see below).

- Changes
  1) Add a small helper in `LspDatabaseAdapter`:
     - `pub fn resolve_symbol_position(file_path: &Path, line: u32, column: u32, language: &str) -> Result<(u32, u32)>`
       - Internally reuse the existing `find_symbol_at_position`/parsing path to return the identifier node’s start `(line,column)` if found; else return the input `(line,column)` (no worse than today).
  2) In Phase 1 and Phase 2, before each LSP op, call `resolve_symbol_position` to “snap” the caret onto the identifier.
  3) Keep honoring existing 0/1‑based conversions handled inside LSP call methods (don’t double-convert).

- Acceptance
  - For a sample Rust/TS/Python repo with a caller→callee, Phase 1 and Phase 2 call hierarchy now returns non‑empty arrays at a much higher rate (parity with `extract --lsp`).


### W2. Persist indexing LSP results by default — Status: Completed

Goal: Persist call hierarchy and references directly during indexing using `LspDatabaseAdapter`, lowering the burden on enrichment.

- Code pointers
  - `lsp-daemon/src/indexing/manager.rs::index_symbols_with_lsp`
  - `lsp-daemon/src/lsp_database_adapter.rs` (`convert_call_hierarchy_to_database`, `convert_references_to_database`)

- Changes
  - Persisted call hierarchy (symbols + edges) during indexing.
  - Persisted references (edges) during indexing.
  - No new flags (best default UX).


### W3. Make enrichment use the correct workspace DB — Status: Completed

Goal: Ensure enrichment reads and writes the DB matching the indexing workspace root, not `current_dir()`.

- Code pointers
  - `lsp-daemon/src/indexing/manager.rs`:
    - `start_phase2_lsp_enrichment()`
    - `spawn_phase2_enrichment_monitor()`
    - `queue_orphan_symbols_for_enrichment()`

- Changes
  1) Store the `workspace_root: PathBuf` in `IndexingManager` when `start_indexing(root_path)` is called (new field on the struct).
  2) Replace `std::env::current_dir()?` with the stored `workspace_root` in all Phase 2 calls to `workspace_cache_router.cache_for_workspace(...)`.
  3) When fetching orphan symbols and when starting workers, always pass cache adapter for `workspace_root`.

- Acceptance
  - On a multi‑workspace test or when starting the indexer from a parent directory, Phase 2 still finds orphan symbols and produces edges for the intended workspace.


### W4. Respect `LspCachingConfig` consistently in Phase 1 — Status: Completed

Changes:
- Phase 1 readiness probe and LSP ops use `lsp_operation_timeout_ms` (with a 5s cap for the probe loop).
- Phase 1 gates per-symbol LSP ops via `should_perform_operation(CallHierarchy|References)`.

Goal: Make Phase 1 call the right LSP ops when and only when enabled; use the configured timeout.

- Code pointers
  - `lsp-daemon/src/indexing/config.rs::LspCachingConfig`
  - `lsp-daemon/src/indexing/manager.rs::index_symbols_with_lsp`

- Changes
  1) Use `should_perform_operation(&LspOperation::CallHierarchy)` and `References` to guard Phase 1 calls.
  2) Use `lsp_operation_timeout_ms` for both call hierarchy and references in Phase 1 (same as Phase 2 workers do).
  3) Ensure both phases log which ops were skipped due to config.

- Acceptance
  - Flipping config flags changes which LSP ops Phase 1 performs; timeouts match config.


### W5. Observability & diagnostics — Status: Completed

Goal: Make it obvious what happened: how many symbols we tried, how many succeeded, and where data got persisted.

- Code pointers
  - `lsp-daemon/src/indexing/manager.rs` (Phase 1)
  - `lsp-daemon/src/indexing/lsp_enrichment_worker.rs` (Phase 2)

- Changes
  1) Added counters in Phase 1 and Phase 2 (see below) and exposed them via `IndexingStatusInfo`:
     - Indexing (prewarm): `lsp_indexing` includes `positions_adjusted`, `call_hierarchy_success`, `symbols_persisted`, `edges_persisted`, `references_found`, `reference_edges_persisted`, `lsp_calls`.
     - Enrichment: `lsp_enrichment` includes `active_workers`, `symbols_processed`, `symbols_enriched`, `symbols_failed`, `edges_created`, `reference_edges_created`, `positions_adjusted`, `call_hierarchy_success`, `references_found`, `queue_stats`, `success_rate`.
  2) Added final summaries + per‑file logs; added `[WORKSPACE_ROUTING]` logs for DB path.

- Acceptance
  - Logs clearly show success rates and which DB was used; developers can troubleshoot quickly.


### W6. Tests (minimum meaningful coverage) — Status: Partial

Goal: Prove the fixes work end‑to‑end.

- Add integration tests in `lsp-daemon` (where feasible, or keep them simple/unit‑style with small sample files):
  1) Position correction:
     - Implemented: unit tests verify `resolve_symbol_position(...)` snaps (Rust/Python).
     - Existing: DB persistence test for extracted symbols (AST path) succeeds.
     - TODO: small integration-smoke to assert DB edges exist post-indexing on a minimal sample (no live LSP servers).
  2) Enrichment workspace routing:
     - TODO: assert enrichment uses indexing root for DB (no `current_dir()` usage).
  3) Indexing persistence:
     - Implemented by default; TODO: assert symbols/edges (incl. reference edges) exist after indexing.

- Keep tests fast; prefer small snippets (Rust, TS, or Python).


### W7. Configuration & documentation — Status: Completed

Goal: Make the behavior/knobs discoverable and safe.

- Update docs/examples:
  - Clarify that indexing persists call hierarchy and references by default (no flags).
  - Clarify that Phase 2 uses the indexing workspace root, not process CWD.
  - Call out the importance of position normalization.
  - Added README section “LSP Indexing Behavior”.


### W8. Non‑goals/cleanup

- Do not re‑introduce the old universal cache; DB persistence is the source of truth.
- Avoid duplicating expensive work when both Phase 1 persistence and Phase 2 run: rely on cleanup before store and DB upsert/replace semantics already present in `LspDatabaseAdapter` flows.

---

## 5) Detailed Implementation Steps (copy‑paste checklist)

1) Add position resolver
   - [x] In `lsp-daemon/src/lsp_database_adapter.rs`, add `resolve_symbol_position(...) -> Result<(u32,u2 0)>` that returns the identifier’s start `(line,column)` if found via tree‑sitter (use existing internal utilities), else returns the input.
   - [x] Unit test: return corrected positions for simple Rust/Python functions.

2) Use resolver in Phase 1
   - [x] In `index_symbols_with_lsp`, before each LSP call, call `resolve_symbol_position` with `(file, symbol.line, symbol.column, language)`.
   - [x] Apply `lsp_operation_timeout_ms` on LSP requests in Phase 1.
   - [x] Guard ops with `LspCachingConfig::should_perform_operation`.

3) Use resolver in Phase 2
   - [x] In `lsp_enrichment_worker.rs`, before `call_hierarchy(...)` and `references(...)`, call `resolve_symbol_position`.

4) Fix workspace routing for enrichment
   - [x] Add `workspace_root: PathBuf` to `IndexingManager` and set it when `start_indexing(root_path)` is called.
   - [x] Replace all `current_dir()` lookups in Phase 2 methods with `self.workspace_root`.
   - [x] Add debug logs showing the workspace path being used for DB cache.

5) Persist indexing results by default
   - [x] Persist call hierarchy (symbols + edges) during indexing using `LspDatabaseAdapter`.
   - [x] Persist references (edges) during indexing using `LspDatabaseAdapter`.

6) Observability
   - [x] Add counters in both phases, log a summary at end.
   - [x] Expose counters in status structs (IndexingStatusInfo.lsp_indexing, lsp_enrichment).

7) Tests
   - [ ] Add/extend tests as described in W6.

8) Docs
   - [ ] Update README/usage/docs (where appropriate) to describe new flags and expected behavior.

---

## 6) Acceptance Criteria

- Positioning: For sample repos, call hierarchy via indexer matches `extract --lsp` behavior (non‑empty for the same symbols).
- Persistence: DB contains expected edges after indexing (indexing and enrichment both persist by default).
- Workspace routing: Enrichment uses the exact indexing root DB (verified via logs and behavior), not process CWD.
- Config/timeouts: Operation gating + timeouts unified with `LspCachingConfig` (Completed).
- Observability: Logs provide a concise success/fail summary and workspace path; status surfaces counters.

---

## 11) Legacy Tests Modernization

The legacy integration tests under `lsp-daemon/tests` predate major internal changes. Many reference removed modules or older APIs (e.g., `universal_cache`, early `DaemonRequest` shapes). To stabilize the suite and restore meaningful coverage, we recommend a phased approach:

- Issues observed
  - Removed modules: `lsp_daemon::universal_cache::{UniversalCache, CacheLayer}` used throughout.
  - API changes:
    - `DaemonRequest`/`DaemonResponse` field shapes changed; requests like `CallHierarchy` no longer accept a generic `params` field.
    - `LspDaemon::new(...)` returns `Result`, not a `Future` (tests use `.await` incorrectly).
    - Database helpers renamed/reshaped: `SQLiteBackend` (not `SqliteBackend`), `create_none_*_edges(symbol_uid: &str)` now takes a single arg.
  - Unexpected cfg feature flags: tests gate on features like `tree-sitter-rust` which are not defined.
  - Multiple test expectations tied to the old universal cache semantics.

- Proposed plan
  1) Gate legacy tests behind a feature (Phase A)
     - Add `#![cfg(feature = "legacy-tests")]` to failing integration tests or skip entire files via cfg to restore default `cargo test` health.
     - Keep small, relevant tests enabled (e.g., minimal smoke tests).
  2) Update a representative subset (Phase B)
     - Replace `universal_cache` usages with direct workspace database router queries.
     - Update `DaemonRequest` constructors to explicit fields: `{ request_id, file_path, line, column, workspace_hint }`, etc.
     - Fix API shape issues: remove `.await` on non-futures, rename `SqliteBackend` to `SQLiteBackend`, adjust `create_none_*_edges(...)` calls.
     - Remove or fix cfg feature flags for tree-sitter.
  3) Cleanup (Phase C)
     - Remove obsolete tests that duplicate newer coverage.
     - Add new focused integration tests for: (a) indexing DB edges exist, (b) enrichment uses workspace root, (c) status fields contain counters.

- Immediate small additions (done)
  - Unit tests for position snapping and references JSON parsing.
  - Readme updates to guide expected behavior and counters.

- Next steps
  - Gate legacy tests with a feature to stabilize CI.
  - Migrate a minimal set of high-value tests to new APIs.
  - Add a lightweight smoke test that indexes a tiny sample and asserts DB edges exist (no live LSPs required).

---

## 7) Risks & Mitigations

- Extra LSP load: Position probing adds negligible cost (single parse + snap). Keep concurrency limits.
- Duplicate edges: Use cleanup + DB upsert semantics already present in `LspDatabaseAdapter::store_call_hierarchy_with_cleanup` and upserts for edges.
- Multi‑workspace: Fixing routing eliminates most surprises; add logs for clarity.

---

## 8) Rollout Plan

1) Implement W1/W3 first (positioning + routing) — biggest wins with lowest risk. [Done]
2) Add observability (W5) to confirm improvements in dev/staging. [Partial]
3) Indexing persistence is ON by default — validate overhead/benefits in staging.
4) Land tests and docs (W6/W7).
5) Roll to prod with indexing + enrichment persistence by default; monitor and tune.

---

## 9) Quick Code Map

- Extract/search (reference behavior)
  - `src/extract/processor.rs` — uses `LspClient::get_symbol_info` with precise positions.
  - `src/lsp_integration/client.rs` — `get_call_hierarchy_precise`, `calculate_lsp_position`.
  - `src/search/lsp_enrichment.rs` — batch enrich with shared `LspClient`.

- Indexer
  - Phase 1 orchestration: `lsp-daemon/src/indexing/manager.rs`
    - `index_symbols_with_lsp` — uses resolver; persists call hierarchy + references by default.
  - Phase 2: `lsp-daemon/src/indexing/lsp_enrichment_worker.rs`
    - Direct LSP + DB via `LspDatabaseAdapter`.
  - DB adapter: `lsp-daemon/src/lsp_database_adapter.rs`.
  - Config: `lsp-daemon/src/indexing/config.rs` (`LspCachingConfig`).

---

## 10) Done Definition (for the epic)

- [x] Position normalization used in both phases.
- [x] Enrichment uses the indexing workspace root DB (verified via logs; tests TODO).
- [x] Indexing-time persistence enabled by default (call hierarchy + references).
- [ ] Config/timeouts respected consistently (unify with `LspCachingConfig`).
- [ ] Tests passing; sample repo produces edges (expand coverage per W6).
- [ ] Docs updated.


---

## 12) Remaining Work — Detailed TODOs & Acceptance

This section tracks concrete, verifiable deliverables that remain. It is written so another engineer can pick any item up immediately.

A. Tests — Enrichment Routing (Workspace Root)
- Goal
  - Prove that Phase 2 (enrichment workers) always use the indexing workspace root DB (not `current_dir()`).
- Code Pointers
  - Manager: `lsp-daemon/src/indexing/manager.rs` (stores `workspace_root` during `start_indexing`)
  - Worker: `lsp-daemon/src/indexing/lsp_enrichment_worker.rs` (uses DB router with manager’s root)
  - Router: `lsp-daemon/src/workspace_database_router.rs`
- Implementation Sketch
  1) Create a temp workspace `W`, and another temp directory `D` (not equal to `W`).
  2) Initialize `IndexingManager` with `workspace_root = W` and run a no-op or minimal indexing to prime worker creation.
  3) Change process CWD to `D` inside the test (or simulate where worker would otherwise accidentally use it).
  4) Trigger a minimal enrichment task (e.g., queue one symbol) and verify the worker’s DB path/logs map under `W` (not under `D`).
     - Use the router’s `base_cache_dir` to point inside `W` and assert DB files created inside that subtree.
- Acceptance
  - Test passes if enrichment DB artifacts are created under `W` and no DB files are observed under `D`.
  - Add a targeted `[WORKSPACE_ROUTING]` assert by capturing logs or by inspecting the router’s `get_or_create_workspace_cache()` call side effects.

B. Tests — DB Smoke without Runtime Pitfalls
- Goal
  - Provide a cross-platform, deterministic smoke test that stores a minimal call hierarchy and references using SQLite without requiring a specific Tokio runtime flavor.
- Code Pointers
  - Adapter: `lsp-daemon/src/lsp_database_adapter.rs` (convert_* -> store_in_database)
  - SQLite backend: `lsp-daemon/src/database/sqlite_backend.rs`
- Implementation Options
  1) Use `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` on the smoke test so fs IO and sqlite layers work reliably.
  2) OR wrap the body in a runtime builder `tokio::runtime::Builder::new_multi_thread()...build()?.block_on(async { ... })`.
- Acceptance
  - The test writes symbols + edges to a temporary DB, and `get_table_counts()` shows non-zero counts; no panics about “no reactor running”.

C. Tests — Status Counters
- Goal
  - Assert that `IndexingStatusInfo` contains non-decreasing counters and success summaries after indexing completes.
- Code Pointers
  - Protocol/status: `lsp-daemon/src/protocol.rs`
  - Daemon status handler: `lsp-daemon/src/daemon.rs`
- Implementation Sketch
  1) Run a small indexing session over a temp workspace with a few files.
  2) Fetch status (or directly call the status function) and assert `lsp_indexing` fields like `lsp_calls`, `symbols_persisted`, `edges_persisted` are present and > 0 when applicable.
- Acceptance
  - Counters present and non-zero when work occurred; success_rate reported where relevant.

D. Error & Line-Number Robustness — Cross-Cut Tests
- Goal
  - Ensure no user-visible `:0` ever appears again and invalid lines never persist.
- Code Pointers
  - Adapter normalization/warnings: `lsp-daemon/src/lsp_database_adapter.rs`
  - Storage clamping: `lsp-daemon/src/database/sqlite_backend.rs` (store_edges)
  - Display: `src/extract/formatter.rs`
- Implementation Sketch
  1) Already added unit tests for formatter and adapter clamping. Add analogous tests for definitions/implementations if missing.
  2) Add a tiny end-to-end assertion using adapter -> sqlite -> query -> confirm `start_line >= 1` on roundtrip.
- Acceptance
  - Tests prove normalization at conversion and that storage clamps guard against regressions.

E. Legacy Tests Modernization — Phase B (High-Value Subset)
- Goal
  - Migrate a small, representative set of legacy tests to DB-first APIs; allow the rest to remain behind `legacy-tests` feature until replaced.
- Candidates & Edits
  - `lsp-daemon/tests/lsp_integration_tests.rs`
    - Replace `universal_cache` calls with `WorkspaceDatabaseRouter + LspDatabaseAdapter`.
    - Update `DaemonRequest` shapes: replace `params` objects with explicit fields `{ file_path, line, column, workspace_hint }`.
  - `lsp-daemon/tests/lsp_performance_benchmarks.rs`
    - Remove UniversalCache plumbing; switch to direct adapter calls and DB routers.
- Acceptance
  - Selected tests compile and pass without `legacy-tests` feature.
  - Remove their `#![cfg(feature = "legacy-tests")]` gates.

F. Legacy Tests Modernization — Phase C (Cleanup + New Coverage)
- Goal
  - Retire obsolete tests and replace with concise, maintainable ones focused on the new behaviors.
- Tasks
  - Remove tests that depend on removed modules (`universal_cache`) without a realistic replacement path.
  - Add two new concise integration tests:
    1) Indexing edges presence (reads DB and asserts > 0 edges after indexing a sample workspace).
    2) Enrichment workspace-routing test (see A) validating correct DB location and counters.
- Acceptance
  - CI runs without `legacy-tests` feature; no red tests from outdated APIs.

G. Documentation — Final Pass
- Goal
  - Ensure docs make behavior obvious for users (and future maintainers).
- Tasks
  - README: add “No :0 lines” note and position normalization rationale.
  - lsp_fix.md: keep W6 marked Partial until A–D land; then flip to Completed.
  - Add a small troubleshooting note: if users see “line=0” in raw logs, explain normalization + warnings.

H. Rollout & Verification
- Goal
  - Catch regressions early and unblock CI.
- Tasks
  - Land A–D as a single PR, then enable `config_integration_tests.rs` and the non-legacy lsp-daemon tests as blocking.
  - Keep legacy tests behind `legacy-tests` until Phase B/C replacements are merged.
- Acceptance
  - CI green without legacy features; targeted lsp-daemon tests (non-legacy) pass reliably across platforms.
