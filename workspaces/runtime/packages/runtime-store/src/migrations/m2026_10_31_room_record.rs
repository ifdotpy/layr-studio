use crate::{StoreError, StoreResult};
use rusqlite::{Transaction, params_from_iter, types::Value as SqlValue};
use studio_runtime_schema::room_record::{FIELDS, RoomRecord};

pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_rooms")? {
        return Ok(());
    }
    let mut statement = tx.prepare("SELECT id,record FROM runtime_rooms ORDER BY id")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let placeholders = (2..FIELDS.len() + 2)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let query = format!(
        "SELECT json_group_object(key, CASE type WHEN 'object' THEN json(value) WHEN 'array' THEN json(value) WHEN 'true' THEN json('true') WHEN 'false' THEN json('false') ELSE value END) FROM (SELECT key,type,value,row_number() OVER (PARTITION BY key ORDER BY id DESC) AS n FROM json_each(?1)) WHERE n=1 AND key IN ({placeholders})"
    );
    let mut normalize = tx.prepare(&query)?;
    let mut normalize_nested = tx.prepare("SELECT CASE WHEN json_type(?1,?2)='object' THEN json_set(?1,?2,json((SELECT json_group_object(key,CASE type WHEN 'object' THEN json(value) WHEN 'array' THEN json(value) WHEN 'true' THEN json('true') WHEN 'false' THEN json('false') ELSE value END) FROM (SELECT key,type,value,row_number() OVER (PARTITION BY key ORDER BY id DESC) AS n FROM json_each(?1,?2)) WHERE n=1))) ELSE ?1 END")?;
    let mut normalize_cursors = tx.prepare("SELECT CASE WHEN json_type(?1,'$.radio.seen')='object' THEN json_set(?1,'$.radio.seen',json((SELECT json_group_object(key,json_object('identity',json(CASE WHEN json_type(value,'$.identity[2]')='null' THEN json_set(json_extract(value,'$.identity'),'$[2]',0) ELSE json_extract(value,'$.identity') END),'seq',json_extract(value,'$.seq'))) FROM (SELECT key,value,row_number() OVER (PARTITION BY key ORDER BY id DESC) AS n FROM json_each(?1,'$.radio.seen')) WHERE n=1))) ELSE ?1 END")?;
    let mut normalize_active_identity = tx.prepare("SELECT CASE WHEN json_type(?1,'$.radio.active.identity[2]')='null' THEN json_set(?1,'$.radio.active.identity[2]',0) ELSE ?1 END")?;
    for (id, raw) in rows {
        let mut args = vec![SqlValue::Text(raw.clone())];
        args.extend(
            FIELDS
                .iter()
                .map(|field| SqlValue::Text((*field).to_owned())),
        );
        let normalized: String = normalize.query_row(params_from_iter(args), |row| row.get(0))?;
        let normalized: String =
            normalize_nested.query_row((&normalized, "$.radio"), |row| row.get(0))?;
        let normalized: String =
            normalize_nested.query_row((&normalized, "$.radio.active"), |row| row.get(0))?;
        let normalized: String =
            normalize_active_identity.query_row([normalized], |row| row.get(0))?;
        let normalized: String = normalize_cursors.query_row([normalized], |row| row.get(0))?;
        let room: RoomRecord =
            serde_json::from_str(&normalized).map_err(|_| StoreError::InvalidStoredRecord {
                object: "runtime_rooms",
                id: id.clone(),
            })?;
        if room.id != id {
            return Err(StoreError::InvalidStoredRecord {
                object: "runtime_rooms",
                id,
            });
        }
        if normalized != raw {
            tx.execute(
                "UPDATE runtime_rooms SET record=?2 WHERE id=?1",
                (&id, &normalized),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn removes_unknown_fields_and_duplicate_keys_idempotently() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_rooms(id TEXT PRIMARY KEY, record TEXT NOT NULL); INSERT INTO runtime_rooms VALUES('r','{"id":"r","kind":"private","members":["a"],"userHidden":false,"extra":1,"customName":"old","customName":"new"}');"#).unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        let first: String = tx
            .query_row("SELECT record FROM runtime_rooms", [], |row| row.get(0))
            .unwrap();
        apply(&tx).unwrap();
        let second: String = tx
            .query_row("SELECT record FROM runtime_rooms", [], |row| row.get(0))
            .unwrap();
        assert_eq!(first, second);
        let room: RoomRecord = serde_json::from_str(&first).unwrap();
        assert_eq!(room.custom_name.as_ref().map(String::as_str), Some("new"));
        assert_eq!(
            room.user_hidden,
            studio_runtime_schema::queue_command::Field::Value(false)
        );
        assert!(!first.contains("extra"));
    }

    #[test]
    fn removes_unknown_radio_cursor_fields() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_rooms(id TEXT PRIMARY KEY, record TEXT NOT NULL); INSERT INTO runtime_rooms VALUES('r','{"id":"r","kind":"private","radio":{"teamId":"a","revision":0,"revision":1,"status":"idle","speaker":null,"next":[],"direct":true,"active":null,"error":null,"seen":{"a":{"identity":["thread",1,2],"seq":3,"future":4},"b":{"identity":["thread",1,null],"seq":0}}}}');"#).unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        let cursor: String = tx
            .query_row(
                "SELECT json_extract(record,'$.radio.seen.a') FROM runtime_rooms",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cursor, r#"{"identity":["thread",1,2],"seq":3}"#);
        let compactions: i64 = tx
            .query_row(
                "SELECT json_extract(record,'$.radio.seen.b.identity[2]') FROM runtime_rooms",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(compactions, 0);
        assert_eq!(
            tx.query_row(
                "SELECT json_extract(record,'$.radio.revision') FROM runtime_rooms",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        apply(&tx).unwrap();
        let again: String = tx
            .query_row(
                "SELECT json_extract(record,'$.radio.seen.a') FROM runtime_rooms",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(again, cursor);
    }
}
