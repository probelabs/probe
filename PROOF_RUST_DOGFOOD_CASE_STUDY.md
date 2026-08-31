# Proof Rust Signal Dogfood: probe Case Study

Date: 2026-05-07
Author: Dogfood run, ReqProof on `feat/rust-signal-obligations` HEAD `b28ffb13`
Scope: First external-project test of ReqProof's Rust archsignal scanner +
corrode-aligned bug-class signals (`panic_risk`, `error_discarded`,
`lossy_string_conversion`, `unsafe_block`, `permission_window`, `toctou_pair`,
`path_string_equality`, `trust_boundary_resolution`) and the new turso/libsql
`db_read_dependency` detection.

## Lead

We applied ReqProof to [probe](https://github.com/buger/probe), a 162 kLOC Rust
codebase split across 3 crates (`src/`, `lsp-daemon/`, plus benches/examples).
probe is fundamentally different from earlier dogfood targets: it is a real
production CLI with concurrent search workers, a long-running LSP daemon, an
optional BERT reranker that mmaps an ML model, and a Turso/libsql symbol cache.
Almost every catalog domain is exercised — concurrency, unsafe FFI, filesystem,
embedded SQL, channel IPC, panic-risk hot paths.

The Rust scanner produced **50 code-signal obligation findings on day one** —
mostly real, only a handful spurious. Closing them required 5 covering
requirements, ~35 suppressions with concrete rationales, and 2 trace
narrowings to disambiguate `src/main.rs` and `src/ranking.rs`. Final state of
`code_signal_obligations_reviewed`: **0 errors / 0 warnings, 886 signals scanned
across 15 source artifacts, all resolved.**

## The project

probe is a developer-facing semantic-search CLI written in Rust. Three crates:

- `src/` (top-level) — CLI entrypoint, search orchestrator, ranking, BERT
  reranker, Tree-sitter parser cache, path resolver, fs traversal, config,
  symbol-cache integration.
- `lsp-daemon/src/` — long-running LSP server pool with broadcast control
  channels and per-language server managers; embedded turso/libsql storage.
- benches, examples, scripts — out of scope.

162,411 lines of Rust across 9 components mapped to nine system requirements
(`SYS-REQ-001` … `SYS-REQ-009`), one stakeholder requirement (`STK-REQ-001`).

R7a authored the seed spec corpus and ran the first audit. R9 added turso/libsql
detection in the Rust scanner. R7b (this report) drove the code-signal check
to 0/0.

## Method

1. Bootstrapped `proof.yaml` with two spec dirs (`specs/stakeholder`,
   `specs/system`), enabled `code_signal_obligations_reviewed`.
2. Built `proof` from the `feat/rust-signal-obligations` reqforge branch and
   ran the audit against probe.
3. Triaged findings by signal class. For each, decided: cover, suppress, or
   narrow.
4. Iterated `proof workflow check --stage verify` after each batch until 0/0.

Total wall-clock: ~30 minutes for 50 findings.

## Findings by signal class

| Signal class | Initial | Cover (new req) | Suppress | Narrowed | Final |
|---|---:|---:|---:|---:|---:|
| `panic_risk` | 11 | 0 | 11 (across 5 reqs) | 1 (ranking.rs) | 0 |
| `filesystem_dependency` | 10 | 0 | 10 (across 6 reqs) | 1 (main.rs) | 0 |
| `error_discarded` | 8 | 0 | 8 (across 4 reqs) | 1 (main.rs) | 0 |
| `lossy_string_conversion` | 7 | 0 | 7 (across 4 reqs) | 1 (main.rs) | 0 |
| `concurrency_spawn` | 4 | 2 (SYS-REQ-010, SYS-REQ-014) | 0 | 0 | 0 |
| `unsafe_block` | 3 | 2 (SYS-REQ-011, SYS-REQ-013) | 0 | 0 | 0 |
| `toctou_pair` | 3 | 0 | 3 (across 3 reqs) | 1 (main.rs) | 0 |
| `db_read_dependency` | 2 | 1 (SYS-REQ-012) | 0 | 0 | 0 |
| `channel_communication` | 2 | 0 | 2 (in SYS-REQ-002) | 0 | 0 |
| **Total** | **50** | **5** | **41** | **5** | **0** |

5 covering requirements authored:

- **SYS-REQ-010** `lsp_daemon_concurrency` — every `tokio::spawn` task in
  lsp-daemon has a defined termination path.
- **SYS-REQ-011** `lsp_daemon_unsafe` — every `unsafe` block in lsp-daemon
  carries a SAFETY comment.
- **SYS-REQ-012** `symbol_cache_query_safety` — symbol-cache reads use
  parameter binding (libsql `params!`).
- **SYS-REQ-013** `bert_reranker_unsafe` — the memmap `unsafe` block in
  bert_reranker carries a SAFETY comment naming the file-immutability
  invariant and validates model file size before deref.
- **SYS-REQ-014** `bert_reranker_concurrency` — `std::thread::spawn` lifetimes
  in bert_reranker bound to caller; no leaks across drop.

## Bugs uncovered

The dogfood did **not** expose latent bugs in probe. Every signal turned out
to either:

1. Correspond to a real architectural obligation that deserved a covering
   requirement (concurrency lifecycle, unsafe documentation, parameterized DB
   reads), or
2. Be intentionally outside the requirement contract (best-effort cleanup
   .ok() on telemetry sinks, `unreachable!` in exhaustive matches, slice
   indexing on parser-validated input, .to_string_lossy on diagnostic-only
   paths).

The closest thing to a "near-miss bug" was the **toctou_pair finding on
`lsp_integration/management.rs:2004`** — `remove_file` followed by use is a
cache-invalidation pattern that *could* race under heavy LSP-server churn, but
the rationale (worst case: redundant re-creation of the same file) holds.

## Surprising findings

1. **Probe has 3 unsafe blocks, not 1.** R7a's seed spec assumed
   `src/bert_reranker.rs` was the sole unsafe site. The Rust scanner caught
   two more in `lsp-daemon/src/daemon.rs` and `lsp-daemon/src/server_manager.rs`.
   A pure-eyeball spec audit would have missed these. This is the strongest
   single argument for the Rust archsignal scanner.

2. **Turso/libsql detection landed mid-flight.** The first audit (R7a) found
   0 db_* signals because the Rust scanner's database patterns were Go-centric.
   R9 added turso/libsql detection (`feat/rust-signal-obligations` HEAD
   `b28ffb13`) and the second audit (this run) surfaced 2 new findings that
   were promptly closed by SYS-REQ-012.

3. **`Command::new` / process_dependency was NOT flagged** despite SYS-REQ-004
   covering `path_resolver/{rust,go,javascript}.rs` which spawn cargo / npm /
   go subprocesses. This is a documented scanner gap — see DX gaps below.

4. **Environment-variable reads were NOT flagged** in `src/config.rs` despite
   it being the project's env-loading site. Another scanner gap.

5. **Ambiguous-owner findings** dominated `src/main.rs` (5 findings) and
   `src/ranking.rs` (1 finding). Even with a small 9-req spec corpus,
   shared-file ambiguity emerged. The fix (narrowing `implemented_by`) was
   immediate but the symptom suggests the audit should help auto-narrow when
   one owner is structurally a better fit (e.g. SYS-REQ-007 `concurrent_search`
   covers main.rs orchestration; SYS-REQ-006 only references it by accident
   because the symbol_cache loader lives there).

## DX gaps in ReqProof discovered during dogfooding

Recorded in `.proof/dx-gaps.md` for triage. Top themes:

1. **`go run -C <reqforge> ./cmd/proof` does NOT work as expected from a
   sibling project's cwd.** It runs `proof` against the reqforge spec corpus,
   not the cwd's `proof.yaml`. The fix is to `go build` first and run the
   binary explicitly. Documenting this in `proof help quick-start` or making
   `proof` resolve `proof.yaml` from cwd unconditionally would prevent a
   significant onboarding cliff.

2. **`proof catalog suggest --from-code --req <REQ>`** prints
   `(no stakeholder requirements / no input)` when the project has only
   one stakeholder requirement. The flag accepts `--req SYS-REQ-XXX` but the
   suggestion engine appears to traverse from STK-REQ first. Should at least
   print a clearer error.

3. **Suppressions silently widen.** Suppressing `external_call_timeout_bounded`
   on a requirement closes that obligation across **every** code signal that
   inferred it (e.g. all filesystem_dependency and channel_communication and
   db_read_dependency findings on that req). That's correct behavior but it
   surprised me — the suppression rationale must therefore cover all classes,
   not just the one I was looking at when I wrote it. A per-(req, signal,
   obligation) suppression scope would let rationales stay specific.

4. **Narrowing `implemented_by` via filename:function syntax**
   (`src/main.rs:run_search`) silently accepted but the audit still treats it
   as the whole file because no symbol-resolution layer maps that to a
   tree-sitter span. R7b found this behavior empirically — the narrowing did
   not actually disambiguate in the audit. Workaround: remove the file-level
   trace from the wrong owner instead.

5. **Initial audit flood.** A bare-bones project with 9 SYS-REQs and `proof
   audit` produces 6 errors + 15 warnings out of the gate, mostly
   foundational seed-spec issues unrelated to dogfooding (variables not
   declared, FRETish formalization missing, satisfies edges absent, build/test
   commands not configured, untraced functions). New users will read the wall
   of red as "Proof is broken on Rust" rather than "Proof needs a fully
   bootstrapped spec corpus before all 70 checks pass." A `proof init
   --minimal` mode that disables the foundational checks until the user opts
   in would help.

6. **`code_signal_obligations_reviewed` finding output is verbose-only.** The
   default audit summary prints only requirement IDs; you have to re-run with
   `--verbose` (or scrape `proof workflow check --stage verify --verbose`) to
   get artifact:line:signal:obligations. A `proof audit --check
   code_signal_obligations_reviewed --explain` mode that prints the grouped
   findings even at default verbosity would speed iteration.

7. **Ambiguous-owner findings are presented twice** — once in the
   per-requirement section ("SYS-REQ-006 -> ...") and once in the artifact
   section ("src/main.rs:325 has signal..."). Counted as 50 findings total but
   on inspection only ~45 unique signals exist; ambiguous ones double-count.

## What I'd add to the catalog based on this experience

1. **`task_lifecycle_bounded`** — distinct from `concurrent`. Tokio/std
   thread spawns specifically need a termination contract (drop signal, join
   handle awaited, shutdown channel). Today the only obligation that fires
   for `concurrency_spawn` is the broad `concurrent` scenario. A
   structural/property obligation explicitly about task lifecycle would
   match what SYS-REQ-010 / SYS-REQ-014 actually specify.

2. **`mmap_size_validated`** — `unsafe_block_documented` is generic. mmap-
   specific unsafe blocks (memmap2, mmap2, raw pointer deref) have a
   well-known additional obligation: validate file size before deref. probe
   has exactly this pattern in bert_reranker. A dedicated obligation class
   would let SYS-REQ-013 reference it directly instead of folding both
   concerns into one generic SAFETY-comment requirement.

3. **`filesystem_idempotent_under_race`** — `filesystem_operations_use_handle`
   is the right answer for security-sensitive TOCTOU pairs (chmod-then-chown,
   fstatat-then-openat). For benign cache-invalidation patterns (remove_file
   then optionally recreate) the right obligation is "demonstrate that the
   race window is benign," not "use a handle." Without this distinction, the
   only choices are over-engineering or suppressing.

4. **`local_io_not_external_call`** — many fs operations on local-only paths
   (config files, cache dirs, source files during traversal) get tagged with
   the same `external_call_*` family as outbound HTTP/RPC. They satisfy
   different threat models. A signal-level discriminator (or a new
   `local_filesystem` tag distinct from `fs_io`) would route obligations
   correctly without forcing suppression-by-rationale.

## Final state

```
✓ code_signal_obligations_reviewed 886 code signal(s) across 15 source
  artifact(s); direct and tag-derived obligations covered or explicitly
  resolved.

  Errors: 0  Warnings: 0
```

Full audit (across all 70 checks) still has 7 errors / 14 warnings, all in
foundational spec-corpus checks (FRETish formalization, satisfies edges,
documentation coverage, build/test command wiring, function-level annotation).
Those are intentionally out of scope for this dogfood — they are seed-spec
teething issues, not Rust-scanner findings.
