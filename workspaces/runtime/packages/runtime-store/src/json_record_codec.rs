//! DbTable adapter for existing payload-id JsonRecord owners and exact stored text.
use crate::records::{JsonRecord, RecordKeyMode};
use crate::{StoreError, StoreResult};
use rusqlite::Connection;
use std::marker::PhantomData;
use studio_db_api::Codec;

/// Immutable typed record plus its admitted text, like Table::get_with_raw.
pub struct RawJsonRecord<R> {
    record: R,
    raw: studio_json::OwnedRawJson,
}
impl<R: JsonRecord> RawJsonRecord<R> {
    pub fn from_stored(connection: &Connection, row_id: &str, raw: String) -> StoreResult<Self> {
        if R::KEY_MODE != RecordKeyMode::PayloadId {
            return Err(StoreError::RecordContract(
                "JSON column codec requires a payload-id JsonRecord",
            ));
        }
        let record = R::decode_stored(connection, row_id, &raw)?;
        Ok(Self {
            record,
            raw: studio_json::OwnedRawJson::from_string(raw)?,
        })
    }
    pub fn from_record(connection: &Connection, record: &R) -> StoreResult<Self> {
        let row_id = record
            .record_key()
            .ok_or(StoreError::RecordContract("payload-id record has no key"))?
            .as_ref();
        Self::from_stored(
            connection,
            row_id,
            record.encode_stored(connection, row_id)?,
        )
    }
    pub fn check_key(&self, physical_key: &str) -> StoreResult<()> {
        if self.record.record_key().map(AsRef::as_ref) != Some(physical_key) {
            return Err(StoreError::InvalidStoredObject {
                table: R::TABLE,
                id: physical_key.to_owned(),
            });
        }
        Ok(())
    }
    pub fn record(&self) -> &R {
        &self.record
    }
    pub fn raw(&self) -> &str {
        self.raw.as_str()
    }
}
pub struct JsonRecordCodec<R>(PhantomData<R>);
impl<R: JsonRecord> Codec<RawJsonRecord<R>> for JsonRecordCodec<R> {
    type Stored = String;
    fn encode(_connection: &Connection, value: &RawJsonRecord<R>) -> rusqlite::Result<String> {
        // The immutable value was admitted by JsonRecord; retain its exact bytes.
        Ok(value.raw().to_owned())
    }
    fn decode(connection: &Connection, value: String) -> rusqlite::Result<RawJsonRecord<R>> {
        #[derive(serde::Deserialize)]
        struct PayloadKey {
            id: String,
        }
        let key: PayloadKey = studio_json::from_str(&value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
        RawJsonRecord::from_stored(connection, &key.id, value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_update_settings::{Canvas, canvas_tx};
    use crate::records::RecordQueryShape;
    use studio_db_api::{DbTable, Table};
    #[derive(serde::Serialize, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Evidence {
        id: String,
        title: String,
        original: studio_json::OwnedRawJson,
    }
    impl JsonRecord for Evidence {
        type Id = String;
        const TABLE: &'static str = "json_codec_witness";
        const SCHEMA: &'static str = "typed evidence witness";
        const KEY_MODE: RecordKeyMode = RecordKeyMode::PayloadId;
        const QUERY_SHAPE: RecordQueryShape = RecordQueryShape::None;
        const TYPED_SHAPE: bool = true;
        fn record_key(&self) -> Option<&String> {
            Some(&self.id)
        }
    }
    #[derive(DbTable)]
    #[db(table="json_codec_witness",database=Canvas,check=check_evidence_row)]
    struct EvidenceRow {
        #[db(key)]
        id: String,
        #[db(codec = JsonRecordCodec<Evidence>)]
        record: RawJsonRecord<Evidence>,
    }
    fn check_evidence_row(row: &EvidenceRow) -> StoreResult<()> {
        row.record.check_key(&row.id)
    }
    #[test]
    fn json_record_codec_preserves_raw_text_and_owner_validation() {
        let mut db = Connection::open_in_memory().expect("db");
        db.execute_batch(EvidenceRow::DDL).expect("DDL");
        let raw = r#"{ "title": "\u0061", "id": "row", "original": { "unknown": [1,2] } }"#;
        let record = RawJsonRecord::<Evidence>::from_stored(&db, "row", raw.into())
            .expect("owner admission");
        assert_eq!(record.record().title, "a");
        let tx = db.transaction().expect("tx");
        let mut typed = canvas_tx(&tx);
        typed
            .insert(&EvidenceRow {
                id: "row".into(),
                record,
            })
            .expect("insert");
        let row = typed
            .get::<EvidenceRow>(&("row".into(),))
            .expect("get")
            .expect("row");
        assert_eq!(row.record.raw(), raw);
        typed.upsert(&row).expect("upsert retains bytes");
        assert_eq!(
            tx.query_row("SELECT record FROM json_codec_witness", [], |r| r
                .get::<_, String>(0))
                .expect("stored"),
            raw
        );
        assert!(RawJsonRecord::<Evidence>::from_stored(&tx, "wrong", raw.into()).is_err());
        assert!(
            JsonRecordCodec::<Evidence>::decode(
                &tx,
                r#"{"id":"row","title":4,"original":null}"#.into()
            )
            .is_err()
        );
        assert!(
            JsonRecordCodec::<Evidence>::decode(
                &tx,
                r#"{"id":"row","title":"ok","original":null,"extra":0}"#.into()
            )
            .is_err()
        );
        assert!(EvidenceRow::DDL.contains("id TEXT PRIMARY KEY NOT NULL"));
        assert!(
            tx.execute("INSERT INTO json_codec_witness VALUES(NULL,'{}')", [])
                .is_err()
        );
        let mismatched = EvidenceRow {
            id: "wrong".into(),
            record: RawJsonRecord::from_stored(&tx, "row", raw.into()).expect("admitted payload"),
        };
        assert!(
            typed.insert(&mismatched).is_err(),
            "write hook rejects mismatched physical key"
        );
        assert!(
            typed.upsert(&mismatched).is_err(),
            "upsert hook rejects mismatched physical key"
        );
        tx.execute("INSERT INTO json_codec_witness VALUES('wrong',?1)", [raw])
            .expect("corrupt raw fixture");
        assert!(
            typed.get::<EvidenceRow>(&("wrong".into(),)).is_err(),
            "read hook rejects mismatched physical key"
        );
        assert_eq!(
            studio_db_api::check::<EvidenceRow>(&tx).expect("state check"),
            (2, 1)
        );
        let normal = RawJsonRecord::<Evidence>::from_record(&tx, row.record.record())
            .expect("existing encode_stored");
        assert_eq!(normal.record().id, "row");
    }
}
