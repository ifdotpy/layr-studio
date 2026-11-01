use crate::{StoreResult, SyncDocumentsRepository};
use rusqlite::Transaction;
use serde::Deserialize;
use serde_json::value::RawValue;
use studio_runtime_schema::usage_limits::{AccountLimitsProjection, RateLimitView};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entity<'a> {
    collection: &'a str,
    id: &'a str,
    #[serde(borrow)]
    value: &'a RawValue,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Current<'a> {
    #[serde(borrow)]
    observation: &'a RawValue,
    projection: AccountLimitsProjection,
}

pub(super) fn apply(tx: &Transaction<'_>, now: f64) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    let rows = {
        let mut query = tx.prepare(
            "SELECT id,typeof(payload),substr(CAST(payload AS BLOB),1,?1),length(CAST(payload AS BLOB)) FROM sync_entities WHERE collection='rateLimit' AND deleted=0",
        )?;
        let rows = query.query_map(
            [crate::account_state::MAX_PERSISTED_DOCUMENT_BYTES as i64 + 1],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (id, storage_type, payload, length) in rows {
        let entity = payload
            .as_deref()
            .filter(|bytes| {
                crate::account_state::persisted_document_within_bounds(bytes)
                    && storage_type == "text"
                    && length.is_some_and(|length| {
                        length <= crate::account_state::MAX_PERSISTED_DOCUMENT_BYTES as i64
                    })
            })
            .and_then(|bytes| serde_json::from_slice::<Entity<'_>>(bytes).ok());
        let valid = entity
            .as_ref()
            .filter(|entity| entity.collection == "rateLimit" && entity.id == id);
        if let Some(entity) = valid {
            if let Ok(current) = serde_json::from_str::<Current<'_>>(entity.value.get()) {
                if RateLimitView::read(current.observation, &id, now).is_ok() {
                    let _projection = current.projection;
                    continue;
                }
            } else if let Ok(view) = RateLimitView::read(entity.value, &id, now) {
                SyncDocumentsRepository::put_entity(tx, "rateLimit", &id, view, false)?;
                continue;
            }
        }
        // A tombstone advances the entity cursor so a client cannot retain invalid cached data.
        SyncDocumentsRepository::put_entity(tx, "rateLimit", &id, (), true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn later_migration_normalizes_existing_observations_without_dropping_accounts() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE sync_entities(collection TEXT,id TEXT,seq INTEGER,hash TEXT,payload TEXT,deleted INTEGER,PRIMARY KEY(collection,id)); CREATE TABLE sync_documents(seq INTEGER); CREATE TABLE sync_generation(id INTEGER,value INTEGER); CREATE TABLE durable_rows(seq INTEGER);").unwrap();
        db.execute("INSERT INTO sync_entities VALUES('rateLimit','a',1,'old',?1,0)", [r#"{"collection":"rateLimit","id":"a","value":{"observation":{"accountKey":"a","data":{"ordinaryUsageAllowed":null,"rateLimits":{"primary":{"usedPercent":80,"vendor":true}}}},"projection":{}}}"#]).unwrap();
        let tx = db.transaction().unwrap();
        super::super::m2026_11_01_normalize_limits::apply(&tx, 100.0).unwrap();
        let payload: String = tx.query_row("SELECT payload FROM sync_entities WHERE collection='rateLimit' AND id='a' AND deleted=0", [], |row| row.get(0)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        let data = &value["value"]["observation"]["data"];
        assert_eq!(data["rateLimits"]["primary"]["usedPercent"], 80.0);
        assert!(data["rateLimits"]["primary"].get("vendor").is_none());
        assert!(
            data.get("ordinaryUsageAllowed")
                .is_some_and(serde_json::Value::is_null)
        );
    }

    #[test]
    fn normalizes_unwrapped_legacy_observation_before_view_migration() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE sync_entities(collection TEXT,id TEXT,seq INTEGER,hash TEXT,payload TEXT,deleted INTEGER,PRIMARY KEY(collection,id)); CREATE TABLE sync_documents(seq INTEGER); CREATE TABLE sync_generation(id INTEGER,value INTEGER); CREATE TABLE durable_rows(seq INTEGER);").unwrap();
        db.execute("INSERT INTO sync_entities VALUES('rateLimit','a',1,'old',?1,0)", [r#"{"collection":"rateLimit","id":"a","value":{"accountKey":"a","data":{"rateLimits":{"primary":{"usedPercent":80}}}}}"#]).unwrap();
        let tx = db.transaction().unwrap();
        super::super::m2026_11_01_normalize_limits::apply(&tx, 100.0).unwrap();
        super::apply(&tx, 100.0).unwrap();
        let payload: String = tx.query_row("SELECT payload FROM sync_entities WHERE collection='rateLimit' AND id='a' AND deleted=0", [], |row| row.get(0)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(
            value["value"]["observation"]["data"]["rateLimits"]["primary"]["usedPercent"],
            80.0
        );
    }

    #[test]
    fn catalog_migration_rolls_back_on_write_failure_and_reopens_idempotently() {
        use studio_runtime_schema::studio_errors::MigrationTarget;
        let mut db = crate::fresh_state::test_canvas();
        db.execute(
            "DELETE FROM sync_entity_meta WHERE key='migration_catalog'",
            [],
        )
        .unwrap();
        let payload =
            r#"{"collection":"rateLimit","id":"a","value":{"accountKey":"a","at":100,"data":{}}}"#;
        db.execute("INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('rateLimit','a',1,'old',?1,0)", [payload]).unwrap();
        db.execute_batch("CREATE TRIGGER reject_limit_view BEFORE UPDATE ON sync_entities WHEN NEW.collection='rateLimit' BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        assert!(super::super::apply_target(&mut db, MigrationTarget::Canvas).is_err());
        let unchanged: String = db
            .query_row(
                "SELECT payload FROM sync_entities WHERE id='a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unchanged, payload);
        db.execute_batch("DROP TRIGGER reject_limit_view").unwrap();
        super::super::apply_target(&mut db, MigrationTarget::Canvas).unwrap();
        let seq: i64 = db
            .query_row("SELECT seq FROM sync_entities WHERE id='a'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let restored = crate::account_state::limits_snapshots(&db).unwrap();
        assert_eq!(restored.snapshots[0].1["accountKey"], "a");
        super::super::apply_target(&mut db, MigrationTarget::Canvas).unwrap();
        let again: i64 = db
            .query_row("SELECT seq FROM sync_entities WHERE id='a'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(seq, again);
    }

    #[test]
    fn wraps_existing_observation_and_advances_entity_sequence() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE sync_entities(collection TEXT,id TEXT,seq INTEGER,hash TEXT,payload TEXT,deleted INTEGER,PRIMARY KEY(collection,id)); CREATE TABLE sync_documents(seq INTEGER); CREATE TABLE sync_generation(id INTEGER,value INTEGER); CREATE TABLE durable_rows(seq INTEGER); INSERT INTO sync_entities VALUES('rateLimit','a',1,'old','{\"collection\":\"rateLimit\",\"id\":\"a\",\"value\":{\"accountKey\":\"a\",\"at\":100,\"data\":{\"rateLimits\":{\"primary\":{\"usedPercent\":80,\"resetsAt\":110,\"windowDurationMins\":10080}}}}}',0);").unwrap();
        db.execute(
            "INSERT INTO sync_entities VALUES('rateLimit','bad',2,'old',?1,0)",
            [r#"{"collection":"rateLimit","id":"bad","value":{"accountKey":"other"}}"#],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sync_entities VALUES('rateLimit','blob',3,'old',x'ff',0)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sync_entities VALUES('rateLimit','utf8',4,'old',CAST(x'ff' AS TEXT),0)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sync_entities VALUES('rateLimit','json',5,'old','{',0)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sync_entities VALUES('rateLimit','validblob',6,'old',CAST(?1 AS BLOB),0)",
            [r#"{"collection":"rateLimit","id":"validblob","value":{"accountKey":"validblob"}}"#],
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx, 100.0).unwrap();
        let (deleted, bad_seq): (i64, i64) = tx
            .query_row(
                "SELECT deleted,seq FROM sync_entities WHERE id='bad'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let blob_deleted: i64 = tx
            .query_row(
                "SELECT deleted FROM sync_entities WHERE id='blob'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(blob_deleted, 1);
        for id in ["utf8", "json", "validblob"] {
            let deleted: i64 = tx
                .query_row(
                    "SELECT deleted FROM sync_entities WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(deleted, 1, "{id}");
        }
        assert_eq!(deleted, 1);
        assert!(bad_seq > 2);
        let (seq, raw): (i64, String) = tx
            .query_row(
                "SELECT seq,payload FROM sync_entities WHERE id='a'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(seq > 1);
        assert_eq!(
            envelope["value"]["observation"]["data"]["rateLimits"]["primary"]["usedPercent"],
            80
        );
        assert_eq!(
            envelope["value"]["projection"]["runway"]["validUntil"],
            110.0
        );
        super::apply(&tx, 101.0).unwrap();
        let unchanged: i64 = tx
            .query_row("SELECT seq FROM sync_entities WHERE id='a'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(unchanged, seq);
    }
}
