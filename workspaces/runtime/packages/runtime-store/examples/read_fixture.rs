use studio_runtime_store::{RuntimeStore, StoreError};
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let store = RuntimeStore::open(Some(dir.path()))
        .await
        .expect("open store");
    store.transaction_with(|tx| -> Result<(), StoreError> {
        tx.execute_batch("CREATE TABLE fixture(id INTEGER PRIMARY KEY, payload TEXT); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<12000) INSERT INTO fixture SELECT x, printf('%08d',x) || hex(zeroblob(256)) FROM n;")?;
        Ok(())
    }).await.expect("populate fixture");
    let db = rusqlite::Connection::open(dir.path().join("canvas.sqlite3"))
        .expect("open checkpoint connection");
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .expect("checkpoint fixture");
    for _ in 0..10 {
        let n = store
            .read_async("fixture/scan", |tx| -> Result<i64, StoreError> {
                Ok(tx.query_row("SELECT sum(length(payload)) FROM fixture", [], |r| r.get(0))?)
            })
            .await
            .expect("read fixture");
        assert_eq!(n, 6_240_000);
    }
}
