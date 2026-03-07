# High-Confidence LSP Indexer/CLI Hardening Plan

This plan lists the changes we consider high‑confidence (low risk, high value), with rationale, status, and how to validate. All items below are either implemented or ready to land as small, isolated patches.

## 1) Filter Self‑Loop Edges (calls/refs/impls)
- Rationale: Edges where `source_symbol_uid == target_symbol_uid` are meaningless and surfaced as audit noise (EID010).
- Change: Drop such edges right before `store_edges(...)` in the enrichment worker for CH/Refs/Impls.
- Status: Implemented.
- Touchpoints: `lsp-daemon/src/indexing/lsp_enrichment_worker.rs`.
- Validate:
  - Logs: no more `EID010 self-loop` lines.
  - Edge counts unchanged or slightly reduced; no regressions in queries.

## 2) Persist ‘none’ Sentinels for Policy Skips
- Rationale: We already persist durable ‘none’ edges for empty/anomaly/fs-missing. Extending this to policy skips (skiplists, “not impl candidate”) aligns DB “pending” counts with reality and prevents re‑planning.
- Change: When skipping Refs/Impls by policy, write `create_none_*_edges(uid)` with metadata `policy_skip_*`, then mark complete.
- Status: Implemented.
- Touchpoints: `lsp-daemon/src/indexing/lsp_enrichment_worker.rs`.
- Validate:
  - `index-status` shows lower DB “Queue” numbers where only policy‑skipped work remained.
  - Edge Audit (see §4) shows POLICY_SKIP counters increasing as expected.

## 3) Edge Audit: Counters for Policy Skips
- Rationale: Make policy resolutions observable alongside EIDxxx audit.
- Change: Add counters for `policy_skip_references`, `policy_skip_impls`, `policy_skip_impls_not_candidate` and print them in `index-status`.
- Status: Implemented.
- Touchpoints: `lsp-daemon/src/edge_audit.rs`, `lsp-daemon/src/protocol.rs`, `src/lsp_integration/management.rs` (printing).
- Validate:
  - `index-status` → Database → Edge Audit includes POLICY_SKIP lines.

## 4) Offload CPU/File Bursts with `spawn_blocking`
- Rationale: Avoid starving IPC/log/status by moving heavy work off the core async runtime.
- Change: Rust impl‑header detection (file read + tree‑sitter parse) moved into `tokio::task::spawn_blocking`.
- Status: Implemented.
- Touchpoints: `lsp-daemon/src/indexing/lsp_enrichment_worker.rs`.
- Validate:
  - Under heavy enrichment, `probe lsp status`/`logs` no longer time out or drop with Broken pipe.
  - CPU spikes in btop don’t correlate with stalled IPC.

## 5) CLI `index-status`: Offline Fallback (Non‑blocking)
- Rationale: Status must never hang. If the daemon socket is slow/unavailable, print a quick snapshot from the workspace DB directly.
- Change: Try daemon with a short timeout; on failure, open the local DB (try `cache.db` and its parent directory) and print:
  - Symbols / Edges / Files (COUNTs)
  - Pending (approx) for refs/impls/calls via lightweight `NOT EXISTS` checks
- Status: Implemented.
- Touchpoints: `src/lsp_integration/management.rs`.
- Validate:
  - Kill or stall daemon; run `./target/release/probe lsp index-status` ⇒ shows `Indexing Status (offline)` with counts.
  - With daemon responsive, path remains unchanged.

## 6) LSP Availability Gating (Binary Missing)
- Rationale: Prevent futile work/loops when a language server isn’t installed; surface counts in status.
- Change: Skip enqueue/processing if `which::which` for that server fails; report in status (Missing LSP section).
- Status: Implemented.
- Touchpoints: planner/worker gating; status printing.
- Validate:
  - No `[workspace-init]` failure loops in logs for missing servers.
  - `index-status` shows ‘Missing LSP’ summary.

## 7) CLI Logs Resilience (Optional – Already Implemented)
- Rationale: Logs must never break. If IPC times out or breaks (Broken pipe), keep printing from persisted JSON tail.
- Change: Follow mode seamlessly falls back to file‑tail; non‑follow prints from JSON tail when IPC is slow.
- Status: Implemented.
- Touchpoints: `src/lsp_integration/management.rs`.
- Validate:
  - `probe lsp logs -f` continues across daemon restarts; `-n N` never times out.

---

## Validation Checklist (Quick)
- Edge Audit shows POLICY_SKIP counters; no EID010.
- `index-status` works online and offline; offline shows correct counts and pending.
- Logs: `-n` prints promptly; `-f` keeps streaming across restarts/socket errors.
- Under load, `status` and `logs` do not hang; Broken pipe incidents reduced.

## Follow‑Ups (Not Required for “High‑Confidence” Scope)
- Dedicated “control” runtime thread in daemon for IPC/log/status with 1‑second precomputed snapshot.
- Logger persistence fallback to `/tmp/probe/logs/lsp` if configured path fails.
