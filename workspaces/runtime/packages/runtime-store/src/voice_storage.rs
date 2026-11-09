//! Physical voice table contracts; timestamps remain fractional Unix seconds.
use crate::json_record_codec::{ExactJson, ExactJsonCodec};
use crate::peer_update_settings::Canvas;
use serde::Deserialize;
use studio_db_api::DbTable;

/// Current writers validate this selector; ExactJson retains every other field verbatim.
#[derive(Deserialize)]
pub struct VoiceRecordsPayload {
    pub utterance_order: Option<i64>,
}
#[derive(Deserialize)]
pub struct VoiceApprovalsResultPayload {
    pub status: Option<String>,
}
#[derive(Deserialize)]
#[serde(transparent)]
pub struct VoiceDeliveryRecordsPayload(pub Vec<String>);
pub type VoiceProviderScopePayload = studio_runtime_schema::persistence::ProviderScope;

#[derive(DbTable)]
#[db(table="voice_approvals", database=Canvas, legacy_ddl="CREATE TABLE voice_approvals (id TEXT PRIMARY KEY, agent TEXT NOT NULL, speech TEXT NOT NULL, transcript TEXT NOT NULL, status TEXT NOT NULL, result TEXT)")]
#[db(state_check=crate::state_check::count_legacy_rows::<VoiceApprovalRow>)]
pub struct VoiceApprovalRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    pub speech: String,
    pub transcript: String,
    pub status: String,
    #[db(codec=ExactJsonCodec<VoiceApprovalsResultPayload>)]
    pub result: Option<ExactJson<VoiceApprovalsResultPayload>>,
}

#[derive(DbTable)]
#[db(table="voice_deliveries", database=Canvas, legacy_ddl="CREATE TABLE voice_deliveries (\n                  id TEXT PRIMARY KEY, agent TEXT NOT NULL, records TEXT NOT NULL, text TEXT NOT NULL, edited_text TEXT)")]
#[db(state_check=crate::state_check::count_legacy_rows::<VoiceDeliveryRow>)]
pub struct VoiceDeliveryRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    #[db(codec=ExactJsonCodec<VoiceDeliveryRecordsPayload>)]
    pub records: ExactJson<VoiceDeliveryRecordsPayload>,
    pub text: String,
    pub edited_text: Option<String>,
}

#[derive(DbTable)]
#[db(table="voice_records", database=Canvas, legacy_ddl="CREATE TABLE voice_records (\n                  seq INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT UNIQUE NOT NULL,\n                  agent TEXT NOT NULL, session TEXT NOT NULL, kind TEXT NOT NULL,\n                  text TEXT NOT NULL, item_id TEXT NOT NULL, previous_item_id TEXT NOT NULL,\n                  payload TEXT NOT NULL, created REAL NOT NULL)", legacy_index="CREATE INDEX voice_records_agent ON voice_records(agent,seq)")]
#[db(state_check=crate::state_check::count_legacy_rows::<VoiceRecordRow>)]
pub struct VoiceRecordRow {
    #[db(key, generated)]
    pub seq: i64,
    pub id: String,
    pub agent: String,
    pub session: String,
    pub kind: String,
    pub text: String,
    pub item_id: String,
    pub previous_item_id: String,
    #[db(codec=ExactJsonCodec<VoiceRecordsPayload>)]
    pub payload: ExactJson<VoiceRecordsPayload>,
    pub created: f64,
}

#[derive(DbTable, Default)]
#[db(table="voice_sessions", database=Canvas, legacy_ddl="CREATE TABLE voice_sessions (\n                  id TEXT PRIMARY KEY, agent TEXT NOT NULL, created REAL NOT NULL,\n                  ended REAL, sdp TEXT, answer TEXT, native_thread TEXT,\n                  account_key TEXT, connection_id TEXT, state TEXT, error TEXT, provider_scope TEXT)")]
#[db(state_check=crate::state_check::count_legacy_rows::<VoiceSessionRow>)]
pub struct VoiceSessionRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    pub created: f64,
    pub ended: Option<f64>,
    pub sdp: Option<String>,
    pub answer: Option<String>,
    pub native_thread: Option<String>,
    pub account_key: Option<String>,
    pub connection_id: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    #[db(codec=ExactJsonCodec<VoiceProviderScopePayload>)]
    pub provider_scope: Option<ExactJson<VoiceProviderScopePayload>>,
}

/// Narrow public session response; declaration order preserves the legacy JSON bytes.
#[derive(studio_db_api::DbProjection, serde::Serialize, Debug, PartialEq)]
#[db(of=VoiceSessionRow)]
pub struct VoiceSessionView {
    pub ended: Option<f64>,
    pub error: Option<String>,
    #[db(field=answer)]
    pub sdp: Option<String>,
    #[db(field=id)]
    pub session_id: String,
    pub state: Option<String>,
}

#[derive(DbTable)]
#[db(table="voice_speech_requests", database=Canvas, legacy_ddl="CREATE TABLE voice_speech_requests (\n                id TEXT PRIMARY KEY, session TEXT NOT NULL, record_id TEXT NOT NULL,\n                state TEXT NOT NULL, error TEXT)")]
#[db(state_check=crate::state_check::count_legacy_rows::<VoiceSpeechRequestRow>)]
pub struct VoiceSpeechRequestRow {
    #[db(key)]
    pub id: String,
    pub session: String,
    pub record_id: String,
    pub state: String,
    pub error: Option<String>,
}

pub(crate) fn prepare(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    for registration in crate::canvas_schema::registry()
        .into_iter()
        .filter(|entry| entry.descriptor.name.starts_with("voice_"))
    {
        registration.descriptor.install_if_absent(connection)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_update_settings::{canvas_read, canvas_tx};
    use crate::{VoiceRecord, VoiceRepository};

    #[test]
    fn voice_repository_projections_preserve_historical_refusal_boundaries() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        prepare(&db).unwrap();
        db.execute_batch("INSERT INTO voice_records(id,agent,session,kind,text,item_id,previous_item_id,payload,created) VALUES('old','a','s','user','','','','not JSON',1.125); INSERT INTO voice_deliveries VALUES('d','a','not JSON','text',NULL); INSERT INTO voice_sessions(id,agent,created,state,provider_scope) VALUES('s','a',1.5,'ready','not JSON')").unwrap();
        assert!(
            canvas_read(&db)
                .get::<VoiceSessionRow>(&("s".into(),))
                .is_err()
        );
        assert_eq!(
            VoiceRepository::record(&db, "old")
                .unwrap()
                .unwrap()
                .payload,
            "not JSON"
        );
        assert_eq!(
            VoiceRepository::records_page(&db, "a", 0, -1)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            VoiceRepository::delivery(&db, "d")
                .unwrap()
                .unwrap()
                .records,
            "not JSON"
        );
        let latest = VoiceRepository::latest_session(&db, "a").unwrap().unwrap();
        assert_eq!(latest.session_id, "s");
        assert_eq!(
            studio_json::to_string(&latest).unwrap(),
            r#"{"ended":null,"error":null,"sdp":null,"session_id":"s","state":"ready"}"#
        );
        assert_eq!(
            VoiceRepository::session(&db, "s", "a").unwrap().unwrap(),
            latest
        );
        let expected = studio_json::to_vec(&(
            1,
            Some((
                "s",
                Some("ready"),
                None::<String>,
                None::<String>,
                None::<f64>,
            )),
        ))
        .unwrap();
        assert_eq!(VoiceRepository::fingerprint(&db, "a").unwrap(), expected);
    }

    #[derive(Deserialize)]
    struct Catalog {
        #[serde(rename = "canvas.sqlite3")]
        canvas: Objects,
    }
    #[derive(Deserialize)]
    struct Objects {
        sqlite_master: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        name: String,
        sql: String,
        table: String,
        r#type: String,
    }

    #[test]
    fn voice_descriptors_match_every_catalog_object_and_legacy_state() {
        let catalog: Catalog =
            studio_json::from_str(crate::fresh_state::SNAPSHOT).expect("catalog");
        let db = rusqlite::Connection::open_in_memory().expect("db");
        for registration in crate::canvas_schema::registry()
            .into_iter()
            .filter(|r| r.descriptor.name.starts_with("voice_"))
        {
            let descriptor = registration.descriptor;
            let mut objects = catalog
                .canvas
                .sqlite_master
                .iter()
                .filter(|e| {
                    e.table == descriptor.name && (e.r#type == "table" || e.r#type == "index")
                })
                .collect::<Vec<_>>();
            objects.sort_by_key(|entry| if entry.r#type == "table" { 0 } else { 1 });
            let generated = descriptor.catalog_entries(0).collect::<Vec<_>>();
            assert_eq!(objects.len(), generated.len(), "{}", descriptor.name);
            for object in objects {
                let entry = generated
                    .iter()
                    .find(|e| e.name == object.name)
                    .expect("generated object");
                assert_eq!(entry.sql, object.sql, "{}", object.name);
                db.execute_batch(&object.sql)
                    .expect("unchanged historical schema");
            }
            assert_eq!(
                (registration.check)(&db)
                    .expect("legacy state check")
                    .counts(),
                (0, 0)
            );
        }
        db.execute_batch("INSERT INTO voice_approvals VALUES(NULL,'a','s','t','historic','not JSON'); INSERT INTO voice_deliveries VALUES(NULL,'a','not JSON','t',NULL); INSERT INTO voice_records(id,agent,session,kind,text,item_id,previous_item_id,payload,created) VALUES('invalid','a','','unknown','','','','not JSON',1.25); INSERT INTO voice_sessions VALUES(NULL,'a',1.25,NULL,NULL,NULL,NULL,NULL,NULL,'historic',NULL,'not JSON'); INSERT INTO voice_speech_requests VALUES(NULL,'s','r','historic',NULL)").expect("legacy evidence");
        for registration in crate::canvas_schema::registry()
            .into_iter()
            .filter(|r| r.descriptor.name.starts_with("voice_"))
        {
            assert_eq!(
                (registration.check)(&db)
                    .expect("unchanged acceptance")
                    .counts(),
                (1, 0),
                "{}",
                registration.descriptor.name
            );
        }
    }

    #[test]
    fn voice_repository_preserves_original_payloads_order_cursors_and_rollback() {
        let mut db = rusqlite::Connection::open_in_memory().expect("db");
        prepare(&db).expect("tables");
        let transaction = db.transaction().expect("tx");
        let payload =
            " \n{ \"unknown\": {\"z\":  1}, \"record_id\": null, \"utterance_order\":1 }\t";
        let record = VoiceRecord {
            sequence: 0,
            id: "record".into(),
            agent: "a".into(),
            session: "s".into(),
            kind: "user".into(),
            text: "voice".into(),
            item_id: "".into(),
            previous_item_id: "".into(),
            payload: payload.into(),
            created: 1.125,
        };
        VoiceRepository::insert_record(&transaction, &record).expect("record");
        let loaded = VoiceRepository::record(&transaction, "record")
            .expect("read")
            .expect("record");
        assert_eq!(
            (loaded.sequence, loaded.payload.as_str(), loaded.created),
            (1, payload, 1.125)
        );
        let records = " \n[\"record\", \"future\"]\t";
        VoiceRepository::insert_delivery(&transaction, "d", "a", records, "text", None)
            .expect("delivery");
        assert_eq!(
            VoiceRepository::delivery(&transaction, "d")
                .expect("read")
                .expect("delivery")
                .records,
            records
        );
        assert!(
            VoiceRepository::records_page(&transaction, "a", 1, 10)
                .expect("cursor")
                .is_empty()
        );
        assert_eq!(
            VoiceRepository::records_page(&transaction, "a", 0, -1)
                .expect("negative unbounded limit")
                .len(),
            1
        );
        let scope = " \n{\"connection\":\"c\",\"generation\":1}\t";
        canvas_tx(&transaction)
            .insert(&VoiceSessionRow {
                id: "s".into(),
                agent: "a".into(),
                created: 1.5,
                provider_scope: Some(ExactJson::from_stored(scope.into()).expect("scope")),
                ..Default::default()
            })
            .expect("session");
        assert_eq!(
            canvas_read(&transaction)
                .get::<VoiceSessionRow>(&("s".into(),))
                .expect("read")
                .expect("session")
                .provider_scope
                .expect("scope")
                .raw(),
            scope
        );
        transaction.rollback().expect("rollback");
        assert!(
            VoiceRepository::record(&db, "record")
                .expect("rolled back")
                .is_none()
        );
    }
}
