use anyhow::{Context, Result};
use lsp_daemon::database::{sqlite_backend::SQLiteConfig, DatabaseConfig, SQLiteBackend};
use std::env;
use std::path::{Path, PathBuf};

#[tokio::main]
async fn main() -> Result<()> {
    // Resolve the workspace cache directory heuristically:
    // 1) PROBE_LSP_WORKSPACE_CACHE_DIR if set (points directly to a workspace dir)
    // 2) $HOME/.cache/probe/lsp/workspaces/<single-entry>/
    // 3) CWD-derived sanitized fallback under ~/.cache/probe/lsp/workspaces
    let db_dir = resolve_workspace_cache_dir()?;
    let db_path = db_dir.join("cache.db");
    println!("Using DB: {}", db_path.display());

    let db_cfg = DatabaseConfig {
        ..Default::default()
    };
    let sqlite_cfg = SQLiteConfig {
        path: db_path.to_string_lossy().to_string(),
        temporary: false,
        enable_wal: true,
        page_size: 4096,
        cache_size: 0,
        enable_foreign_keys: true,
    };

    let backend = SQLiteBackend::with_sqlite_config(db_cfg, sqlite_cfg)
        .await
        .context("open backend")?;

    // Quick counts by language for JS/TS
    println!("\n-- Language counts (JS/TS) --");
    for (lang, cnt) in backend.language_counts_js_ts().await? {
        println!("{:<12} {}", lang, cnt);
    }

    // Check for absolute vs relative file_path anomalies
    println!("\n-- Absolute vs Relative paths (sample) --");
    let (abs_cnt, rel_cnt) = backend.count_abs_rel_paths().await?;
    println!("absolute_paths: {}", abs_cnt);
    println!("relative_paths: {}", rel_cnt);

    // Probe specific files from the user’s logs
    let targets = vec![
        ("src/mcp/index.ts", None::<&str>),
        ("src/agent/ProbeAgent.d.ts", None::<&str>),
    ];

    for (file, _uid) in targets {
        println!("\n== Symbols for file_path = '{}' ==", file);
        let rows = backend.get_symbols_by_file_exact(file, 50).await?;
        if rows.is_empty() {
            println!("(no rows)");
        } else {
            for s in rows {
                println!(
                    "- [{}] {} {} @ L{} :: {}",
                    s.language,
                    s.kind,
                    s.name,
                    s.def_start_line + 1,
                    s.symbol_uid
                );
            }
        }
    }

    // Show a few JS/TS rows where the relative path begins with 'src/' to confirm mapping basis
    println!("\n-- Sample JS/TS rows with file_path LIKE 'src/%' --");
    // Reuse the exact file method on a small set to show sample rows for JS/TS under src/
    // We list distinct file_paths first using a bounded query through the backend’s pool
    let _ = backend.try_begin_reader("db_check_js.sample").await;
    // Fallback simple plan: probe a few common JS/TS files under src/
    for probe in [
        "src/mcp/index.ts",
        "src/agent/ProbeAgent.d.ts",
        "src/agent/schemaUtils.js",
    ] {
        let rows = backend.get_symbols_by_file_exact(probe, 10).await?;
        if !rows.is_empty() {
            println!("\n-- Sample for {} --", probe);
            for s in rows {
                println!("{} :: {} :: {}", s.file_path, s.name, s.symbol_uid);
            }
        }
    }

    Ok(())
}

fn resolve_workspace_cache_dir() -> Result<PathBuf> {
    // 1) explicit
    if let Ok(p) = env::var("PROBE_LSP_WORKSPACE_CACHE_DIR") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
    }

    // 2) scan default base
    let base = dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("probe")
        .join("lsp")
        .join("workspaces");

    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&base)
        .ok()
        .into_iter()
        .flat_map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    candidates.sort();
    if let Some(dir) = candidates.first() {
        return Ok(dir.clone());
    }

    // 3) fallback: cwd folder name under base (sanitized-ish)
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let folder = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace");
    let fallback = base.join(folder);
    std::fs::create_dir_all(&fallback).ok();
    Ok(fallback)
}
