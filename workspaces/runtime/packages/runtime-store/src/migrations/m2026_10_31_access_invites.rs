//! Discard legacy invitations that cannot be decoded as strict credentials.
use crate::StoreResult;
use rusqlite::Transaction;
use studio_runtime_schema::{http_ingress::Object, multi_server_protocol::PersistedAccessInvite};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_access_invites")? {
        return Ok(());
    }
    let mut statement = tx.prepare("SELECT id,record FROM runtime_access_invites ORDER BY id")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for (id, raw) in rows {
        let valid = serde_json::from_str::<Object<PersistedAccessInvite>>(&raw)
            .is_ok_and(|invite| invite.0.invite_id() == id);
        if !valid {
            tx.execute("DELETE FROM runtime_access_invites WHERE id=?1", [&id])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn keeps_valid_invites_and_discards_invalid_invites_once() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(&format!(r#"CREATE TABLE runtime_access_invites(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            INSERT INTO runtime_access_invites VALUES
              ('good','{{"inviteId":"good","status":"open","expires":1,"tokenHash":"{0}","tailscaleUser":"owner","grant":"observe"}}'),
              ('unknown','{{"inviteId":"unknown","status":"open","expires":1,"tokenHash":"{0}","tailscaleUser":"owner","extra":true}}'),
              ('invalid','{{"inviteId":"other","status":"open","expires":1,"tokenHash":"{0}","tailscaleUser":"owner"}}');"#, "a".repeat(64))).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let rows: Vec<String> = tx
            .prepare("SELECT id FROM runtime_access_invites ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(rows, ["good"]);
        let changes = tx.total_changes();
        super::apply(&tx).unwrap();
        assert_eq!(tx.total_changes(), changes);
    }
}
