//! Minimal Turso/libSQL playground to verify UNIQUE indexes and INSERT OR IGNORE.
//! Run with: `cargo test -p lsp-daemon turso_unique -- --nocapture`

use anyhow::Result;
use turso::{params::IntoParams, Builder};

async fn exec(conn: &turso::Connection, sql: &str, params: impl IntoParams) -> Result<u64> {
    conn.execute(sql, params)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))
}

async fn q_count(conn: &turso::Connection, sql: &str) -> Result<i64> {
    let mut rows = conn
        .query(sql, ())
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let mut val = 0i64;
    if let Some(row) = rows.next().await.map_err(|e| anyhow::anyhow!("{}", e))? {
        if let Ok(turso::Value::Integer(n)) = row.get_value(0) {
            val = n;
        }
    }
    Ok(val)
}

#[tokio::test]
async fn turso_unique_and_or_ignore_supported() -> Result<()> {
    // In-memory database
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;

    // Case 1: UNIQUE over non-null columns
    exec(
        &conn,
        "CREATE TABLE IF NOT EXISTS t1 (a INTEGER NOT NULL, b TEXT NOT NULL, c INTEGER NOT NULL)",
        (),
    )
    .await?;
    exec(
        &conn,
        "CREATE UNIQUE INDEX IF NOT EXISTS ux_t1 ON t1(a,b,c)",
        (),
    )
    .await?;

    // INSERT OR IGNORE supported?
    exec(
        &conn,
        "INSERT OR IGNORE INTO t1(a,b,c) VALUES (1,'x',2)",
        (),
    )
    .await?;
    exec(
        &conn,
        "INSERT OR IGNORE INTO t1(a,b,c) VALUES (1,'x',2)",
        (),
    )
    .await?; // duplicate
    exec(
        &conn,
        "INSERT OR IGNORE INTO t1(a,b,c) VALUES (1,'x',3)",
        (),
    )
    .await?; // new

    let cnt = q_count(&conn, "SELECT COUNT(*) FROM t1").await?;
    assert_eq!(
        cnt, 2,
        "OR IGNORE + UNIQUE should suppress exact duplicates (t1)"
    );

    // Case 2: UNIQUE including nullable columns (SQLite treats NULLs as distinct)
    exec(&conn, "CREATE TABLE IF NOT EXISTS t2 (rel TEXT NOT NULL, src TEXT NOT NULL, tgt TEXT NOT NULL, start_line INTEGER, start_char INTEGER)", ()).await?;
    exec(
        &conn,
        "CREATE UNIQUE INDEX IF NOT EXISTS ux_t2 ON t2(rel,src,tgt,start_line,start_char)",
        (),
    )
    .await?;

    // Two rows differing only by NULLs are NOT considered duplicates in SQLite
    exec(&conn, "INSERT OR IGNORE INTO t2(rel,src,tgt,start_line,start_char) VALUES ('references','S','T',NULL,NULL)", ()).await?;
    exec(&conn, "INSERT OR IGNORE INTO t2(rel,src,tgt,start_line,start_char) VALUES ('references','S','T',NULL,NULL)", ()).await?; // remains 2 because NULL!=NULL for UNIQUE
    exec(&conn, "INSERT OR IGNORE INTO t2(rel,src,tgt,start_line,start_char) VALUES ('references','S','T',1,NULL)", ()).await?; // new
    exec(&conn, "INSERT OR IGNORE INTO t2(rel,src,tgt,start_line,start_char) VALUES ('references','S','T',1,NULL)", ()).await?; // duplicate of previous -> ignored

    let cnt2 = q_count(&conn, "SELECT COUNT(*) FROM t2").await?;
    assert_eq!(
        cnt2, 3,
        "UNIQUE with NULLs allows duplicates unless NULLs are canonicalized (t2)"
    );

    Ok(())
}
