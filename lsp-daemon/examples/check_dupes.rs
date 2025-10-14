use anyhow::Result;
use std::path::{Path, PathBuf};
use turso::{Builder, Connection, Value};

#[tokio::main]
async fn main() -> Result<()> {
    let db_path = resolve_db_path_from_args();
    eprintln!("DB: {}", db_path.display());

    let db = Builder::new_local(&db_path.to_string_lossy())
        .build()
        .await?;
    let conn = db.connect()?;
    // Be gentle under contention
    let _ = conn.execute("PRAGMA busy_timeout=3000", ()).await;

    // Basic table counts
    let symbols = q_count(&conn, "SELECT COUNT(*) FROM symbol_state").await?;
    let edges = q_count(&conn, "SELECT COUNT(*) FROM edge")
        .await
        .unwrap_or(-1);
    println!("symbol_state rows: {}", symbols);
    if edges >= 0 {
        println!("edge rows: {}", edges);
    }

    // Duplicates by symbol_uid
    let dup_total = q_count(&conn, "SELECT COUNT(*) FROM (SELECT symbol_uid FROM symbol_state GROUP BY symbol_uid HAVING COUNT(*) > 1)").await?;
    println!("duplicate symbol_uids: {}", dup_total);

    if dup_total > 0 {
        println!("Top duplicates:");
        let mut rows = conn
            .query(
                "SELECT symbol_uid, COUNT(*) c FROM symbol_state GROUP BY symbol_uid HAVING c > 1 ORDER BY c DESC, symbol_uid LIMIT 20",
                (),
            )
            .await?;
        while let Some(r) = rows.next().await? {
            let uid: String = r.get(0)?;
            let c: i64 = r.get(1)?;
            println!("  {} -> {}", uid, c);
        }

        // Show sample rows for duplicates
        let mut rows = conn
            .query(
                "SELECT s.symbol_uid, s.file_path, s.language, s.name, s.kind, s.def_start_line, s.def_start_char\n                 FROM symbol_state s\n                 JOIN (SELECT symbol_uid FROM symbol_state GROUP BY symbol_uid HAVING COUNT(*) > 1) d\n                   ON s.symbol_uid = d.symbol_uid\n                 ORDER BY s.symbol_uid LIMIT 5",
                (),
            )
            .await?;
        println!("Sample duplicate rows:");
        while let Some(r) = rows.next().await? {
            let uid: String = r.get(0)?;
            let fp: String = r.get(1)?;
            let lang: String = r.get(2)?;
            let name: String = r.get(3)?;
            let kind: String = r.get(4)?;
            let sl: i64 = r.get(5)?;
            let sc: i64 = r.get(6)?;
            println!(
                "  {} | {} | {} | {} | {} @ {}:{}",
                uid, lang, name, kind, fp, sl, sc
            );
        }
    }

    Ok(())
}

async fn q_count(conn: &Connection, sql: &str) -> Result<i64> {
    let mut rows = conn.query(sql, ()).await?;
    if let Some(r) = rows.next().await? {
        if let Value::Integer(n) = r.get_value(0)? {
            return Ok(n);
        }
    }
    Ok(0)
}

fn resolve_db_path_from_args() -> PathBuf {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(p) = args.pop() {
        let path = PathBuf::from(p);
        if path.exists() {
            return path;
        }
    }
    // Fallback to default workspace path based on current dir
    let ws_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    default_workspace_db_path(&ws_root)
}

fn default_workspace_db_path(ws_root: &Path) -> PathBuf {
    let ws_root = ws_root
        .canonicalize()
        .unwrap_or_else(|_| ws_root.to_path_buf());
    let workspace_id = git_remote_id(&ws_root).unwrap_or_else(|| hash_path_for_id(&ws_root));
    let base = dirs::cache_dir()
        .or_else(|| dirs::home_dir())
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("probe")
        .join("lsp")
        .join("workspaces")
        .join(workspace_id)
        .join("cache.db")
}

fn git_remote_id(ws_root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(ws_root)
        .arg("config")
        .arg("--get")
        .arg("remote.origin.url")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if url.is_empty() {
        return None;
    }
    Some(sanitize_remote_for_id(&url))
}

fn sanitize_remote_for_id(url: &str) -> String {
    let mut s = url.to_lowercase();
    for p in ["https://", "http://", "ssh://", "git@", "git://"] {
        if let Some(rem) = s.strip_prefix(p) {
            s = rem.to_string();
        }
    }
    s = s.replace(':', "/");
    if s.ends_with(".git") {
        s.truncate(s.len() - 4);
    }
    let mut out: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

fn hash_path_for_id(path: &Path) -> String {
    let norm = path.to_string_lossy().to_string();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"workspace_id:");
    hasher.update(norm.as_bytes());
    let hash = hasher.finalize();
    let folder = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let safe: String = folder
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}_{}", &hash.to_hex()[..8], safe)
}
