//! DbTable adapter for existing payload-id JsonRecord owners and exact stored text.
use crate::records::{JsonRecord, RecordKeyMode};
use crate::{StoreError, StoreResult};
use rusqlite::Connection;
use std::marker::PhantomData;
use studio_db_api::Codec;
pub use studio_runtime_schema::raw_json::{RawExtensions, deserialize_with_raw_extensions};

/// Exact id/record write adapters; callers explicitly select only their authorized operations.
macro_rules! exact_json_row_writes {
    ($row:ident { $($method:ident => $operation:ident),+ $(,)? }) => {
        impl $row {
            $(pub fn $method(
                transaction: &rusqlite::Transaction<'_>, id: &str, raw: String,
            ) -> $crate::StoreResult<()> {
                $crate::peer_update_settings::canvas_tx(transaction).$operation(&Self {
                    id: id.into(),
                    record: $crate::json_record_codec::ExactJson::from_stored(raw)?,
                })?;
                Ok(())
            })+
        }
    };
}
pub(crate) use exact_json_row_writes;

/// An immutable typed JSON value together with its exact admitted storage text.
/// Unlike `RawJsonRecord`, its owner need not carry a payload ID.
#[derive(Clone, Debug, PartialEq)]
pub struct ExactJson<T> {
    value: T,
    raw: Box<str>,
}

impl<T: serde::de::DeserializeOwned> ExactJson<T> {
    pub fn from_stored(raw: String) -> studio_json::Result<Self> {
        let value = studio_json::from_str(&raw)?;
        Ok(Self {
            value,
            raw: raw.into_boxed_str(),
        })
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }
}

impl<T: serde::de::DeserializeOwned + serde::Serialize> ExactJson<T> {
    pub fn from_value(value: &T) -> studio_json::Result<Self> {
        Self::from_stored(studio_json::to_string(value)?)
    }
}

/// Exact JSON storage for a named serde owner, including SQL-nullable columns.
pub struct ExactJsonCodec<T>(PhantomData<T>);

impl<T: serde::de::DeserializeOwned> Codec<ExactJson<T>> for ExactJsonCodec<T> {
    type Stored = String;

    fn encode(_connection: &Connection, value: &ExactJson<T>) -> rusqlite::Result<String> {
        Ok(value.raw().to_owned())
    }

    fn decode(_connection: &Connection, value: String) -> rusqlite::Result<ExactJson<T>> {
        ExactJson::from_stored(value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    }
}

impl<T: serde::de::DeserializeOwned> Codec<Option<ExactJson<T>>> for ExactJsonCodec<T> {
    type Stored = Option<String>;

    fn encode(
        connection: &Connection,
        value: &Option<ExactJson<T>>,
    ) -> rusqlite::Result<Option<String>> {
        value
            .as_ref()
            .map(|value| <Self as Codec<ExactJson<T>>>::encode(connection, value))
            .transpose()
    }

    fn decode(
        connection: &Connection,
        value: Option<String>,
    ) -> rusqlite::Result<Option<ExactJson<T>>> {
        value
            .map(|value| <Self as Codec<ExactJson<T>>>::decode(connection, value))
            .transpose()
    }
}

/// Immutable typed record plus its admitted text, like Table::get_with_raw.
pub struct RawJsonRecord<R> {
    record: R,
    raw: Box<str>,
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
            raw: raw.into_boxed_str(),
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
        &self.raw
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
        struct PayloadKey {
            id: String,
        }
        impl<'de> serde::Deserialize<'de> for PayloadKey {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct KeyVisitor;
                impl<'de> serde::de::Visitor<'de> for KeyVisitor {
                    type Value = PayloadKey;
                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("a payload object")
                    }
                    fn visit_map<M: serde::de::MapAccess<'de>>(
                        self,
                        mut map: M,
                    ) -> Result<Self::Value, M::Error> {
                        let mut id = None;
                        while let Some(key) = map.next_key::<String>()? {
                            if key == "id" {
                                id = Some(map.next_value::<studio_json::OwnedRawJson>()?);
                            } else {
                                map.next_value::<serde::de::IgnoredAny>()?;
                            }
                        }
                        let raw = id.ok_or_else(|| serde::de::Error::missing_field("id"))?;
                        // The record owner decides duplicate admission; key extraction must not narrow it.
                        let id = studio_json::from_str(raw.as_str())
                            .map_err(serde::de::Error::custom)?;
                        Ok(PayloadKey { id })
                    }
                }
                deserializer.deserialize_map(KeyVisitor)
            }
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
    #[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
    struct ExtensionFields {
        id: String,
    }
    studio_runtime_schema::raw_json_payload!(
        #[derive(Debug, PartialEq)]
        struct ExtensionEvidence { fields: ExtensionFields, extra }
    );
    #[test]
    fn exact_json_raw_extensions_preserve_outer_whitespace() {
        let raw = " \n{\"id\":\"a\",\"future\": { \"x\": [1.00,2] }} \t";
        let evidence = ExactJson::<ExtensionEvidence>::from_stored(raw.into()).unwrap();
        assert_eq!(evidence.raw(), raw);
        assert_eq!(evidence.value().fields.id, "a");
        assert_eq!(
            evidence.value().extra["future"].as_str(),
            r#"{ "x": [1.00,2] }"#
        );
        assert!(
            ExactJson::<ExtensionEvidence>::from_stored(r#"{"id":"a","id":"b"}"#.into()).is_err()
        );
    }
    #[test]
    fn json_record_codec_keeps_owner_duplicate_key_policy() {
        let db = Connection::open_in_memory().unwrap();
        for raw in [
            " \n{\"id\":\"old\",\"id\":\"work\",\"status\":\"ready\",\"future\": { \"x\": 1.00 }}\t",
            r#"{"id":false,"id":"work","status":"ready"}"#,
        ] {
            let record = JsonRecordCodec::<crate::WorkItemRecord>::decode(&db, raw.into()).unwrap();
            assert_eq!(record.record().id().0, "work");
            assert_eq!(record.raw(), raw);
        }
        for raw in [
            r#"{"id":"work","id":false,"status":"ready"}"#,
            r#"{"status":"ready"}"#,
        ] {
            assert!(JsonRecordCodec::<crate::WorkItemRecord>::decode(&db, raw.into()).is_err());
        }
        assert!(
            JsonRecordCodec::<Evidence>::decode(
                &db,
                r#"{"id":"old","id":"new","title":"ok","original":null}"#.into()
            )
            .is_err()
        );
    }

    use crate::peer_update_settings::{Canvas, canvas_tx};
    use crate::records::RecordQueryShape;
    use studio_db_api::{DbTable, Table};

    #[derive(serde::Deserialize)]
    struct ExactEvidence {
        title: String,
        original: studio_json::OwnedRawJson,
    }

    #[derive(DbTable)]
    #[db(table="exact_json_witness",database=Canvas)]
    struct ExactEvidenceRow {
        #[db(key)]
        id: i64,
        #[db(codec = ExactJsonCodec<ExactEvidence>)]
        record: ExactJson<ExactEvidence>,
        #[db(codec = ExactJsonCodec<Option<String>>)]
        nullable: Option<ExactJson<Option<String>>>,
    }

    #[test]
    fn exact_json_codec_preserves_text_types_and_sql_null() {
        let mut db = Connection::open_in_memory().expect("db");
        db.execute_batch(ExactEvidenceRow::DDL).expect("DDL");
        let raw = " \n{ \"title\": \"\\u0061\", \"original\": { \"unknown\": [1, 2.00] } }\t";
        let record = ExactJson::from_stored(raw.into()).expect("typed JSON");
        let tx = db.transaction().expect("tx");
        let mut typed = canvas_tx(&tx);
        typed
            .insert(&ExactEvidenceRow {
                id: 1,
                record,
                nullable: None,
            })
            .expect("insert");
        let mut row = typed
            .get::<ExactEvidenceRow>(&(1,))
            .expect("read")
            .expect("row");
        assert_eq!(row.record.value().title, "a");
        assert_eq!(
            row.record.value().original.as_str(),
            r#"{ "unknown": [1, 2.00] }"#
        );
        assert_eq!(row.record.raw(), raw);
        assert!(row.nullable.is_none());
        row.nullable = Some(ExactJson::from_stored(" \nnull\t".into()).expect("JSON null"));
        typed.upsert(&row).expect("upsert");
        let row = typed
            .get::<ExactEvidenceRow>(&(1,))
            .expect("read")
            .expect("row");
        assert!(row.nullable.expect("SQL text").value().is_none());
        assert_eq!(
            tx.query_row(
                "SELECT record,nullable FROM exact_json_witness",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            )
            .expect("stored"),
            (raw.to_owned(), " \nnull\t".to_owned())
        );
        assert!(
            ExactJson::<ExactEvidence>::from_stored(r#"{"title":4,"original":null}"#.into())
                .is_err()
        );
    }
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
        let raw = " \n{ \"title\": \"\\u0061\", \"id\": \"row\", \"original\": { \"unknown\": [1,2] } }\t";
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
            studio_db_api::check::<EvidenceRow>(&tx)
                .expect("state check")
                .counts(),
            (2, 1)
        );
        let normal = RawJsonRecord::<Evidence>::from_record(&tx, row.record.record())
            .expect("existing encode_stored");
        assert_eq!(normal.record().id, "row");
    }
}
