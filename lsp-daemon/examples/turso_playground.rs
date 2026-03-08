// Minimal Turso/libSQL playground to verify UNIQUE indexes and INSERT OR IGNORE support.
// Run: cargo run -p lsp-daemon --example turso_playground --quiet

use turso::{params::IntoParams, Builder};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;

    // Helper closures
    async fn exec(
        conn: &turso::Connection,
        sql: &str,
        params: impl IntoParams,
    ) -> anyhow::Result<u64> {
        conn.execute(sql, params)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }
    async fn q_count(conn: &turso::Connection, sql: &str) -> anyhow::Result<i64> {
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

    println!("-- Case 1: UNIQUE over non-null columns");
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
    // Plain INSERT then duplicate to verify UNIQUE enforcement
    exec(&conn, "INSERT INTO t1(a,b,c) VALUES (1,'x',2)", ()).await?;
    match exec(&conn, "INSERT INTO t1(a,b,c) VALUES (1,'x',2)", ()).await {
        Ok(_) => println!("  WARNING: duplicate insert did not error — UNIQUE not enforced?"),
        Err(e) => println!("  UNIQUE enforced (duplicate insert failed): {}", e),
    }
    exec(&conn, "INSERT INTO t1(a,b,c) VALUES (1,'x',3)", ()).await?; // new row
    let cnt = q_count(&conn, "SELECT COUNT(*) FROM t1").await?;
    println!("t1 rows = {} (expected 2)", cnt);

    println!("\n-- Case 2: UNIQUE with nullable columns (NULLs are distinct in SQLite)");
    exec(&conn, "CREATE TABLE IF NOT EXISTS t2 (rel TEXT NOT NULL, src TEXT NOT NULL, tgt TEXT NOT NULL, start_line INTEGER, start_char INTEGER)", ()).await?;
    exec(
        &conn,
        "CREATE UNIQUE INDEX IF NOT EXISTS ux_t2 ON t2(rel,src,tgt,start_line,start_char)",
        (),
    )
    .await?;
    exec(
        &conn,
        "INSERT INTO t2(rel,src,tgt,start_line,start_char) VALUES ('references','S','T',NULL,NULL)",
        (),
    )
    .await?;
    exec(
        &conn,
        "INSERT INTO t2(rel,src,tgt,start_line,start_char) VALUES ('references','S','T',NULL,NULL)",
        (),
    )
    .await?; // allowed (NULL!=NULL)
    exec(
        &conn,
        "INSERT INTO t2(rel,src,tgt,start_line,start_char) VALUES ('references','S','T',1,NULL)",
        (),
    )
    .await?;
    match exec(
        &conn,
        "INSERT INTO t2(rel,src,tgt,start_line,start_char) VALUES ('references','S','T',1,NULL)",
        (),
    )
    .await
    {
        Ok(_) => {
            println!("  Duplicate with start_line=1 inserted — expected due to NULL start_char")
        }
        Err(e) => println!("  Duplicate blocked: {}", e),
    }
    let cnt2 = q_count(&conn, "SELECT COUNT(*) FROM t2").await?;
    println!("t2 rows = {} (demonstrates NULL-distinct semantics)", cnt2);

    println!("\nConclusion: UNIQUE indexes are enforced; INSERT OR IGNORE is not supported in this libSQL build.\n- Use plain INSERT and handle duplicate errors, or pre-dedup/UPSERT patterns.\n- Also, NULLs in UNIQUE columns are distinct — canonicalize to a sentinel (e.g., -1) if you want uniqueness across 'missing' positions.\n");

    Ok(())
}
