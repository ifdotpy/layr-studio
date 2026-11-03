//! Store the sha256 digest of each message request signature instead of the canonical
//! body text. A digest is 64 hex characters and never starts with `{` or `[`, so those
//! rows are exactly the text rows written before the digest rule.
use crate::{StoreResult, sha256_hex};
use rusqlite::{Transaction, params};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let mut query = tx.prepare(
        "SELECT id,signature FROM request_results WHERE substr(signature,1,1) IN ('{','[')",
    )?;
    let rows = query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(query);
    for (id, text) in rows {
        tx.execute(
            "UPDATE request_results SET signature=?1 WHERE id=?2",
            params![sha256_hex(text.as_bytes()), id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn text_signatures_become_their_digest_and_digests_stay() {
        let mut db = Connection::open_in_memory().expect("database");
        db.execute_batch(
            "CREATE TABLE request_results(id TEXT PRIMARY KEY, signature TEXT NOT NULL, created INTEGER NOT NULL);
             INSERT INTO request_results VALUES('legacy','{\"agent\": \"lead\", \"text\": \"hello\", \"assets\": []}',1);
             INSERT INTO request_results VALUES('array','[1, 2]',1);
             INSERT INTO request_results VALUES('digest','placeholder',1);",
        )
        .expect("fixture");
        let digest = sha256_hex(b"digest body");
        db.execute(
            "UPDATE request_results SET signature=?1 WHERE id='digest'",
            [&digest],
        )
        .expect("digest fixture");
        let legacy = r#"{"agent": "lead", "text": "hello", "assets": []}"#;
        let tx = db.transaction().expect("transaction");
        apply(&tx).expect("migration applies");
        apply(&tx).expect("migration repeats");
        let signature = |id: &str| -> String {
            tx.query_row(
                "SELECT signature FROM request_results WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .expect("signature reads")
        };
        assert_eq!(signature("legacy"), sha256_hex(legacy.as_bytes()));
        assert_eq!(signature("array"), sha256_hex(b"[1, 2]"));
        assert_eq!(signature("digest"), digest);
    }
}
