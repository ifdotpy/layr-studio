use super::*;
use crate::peer_update_settings::{canvas_read, canvas_tx};
use studio_db_api::Table;

fn descriptors() -> [studio_db_api::Descriptor; 7] {
    [
        AssetsStorageRow::descriptor(),
        CompactionsStorageRow::descriptor(),
        ContextManifestsStorageRow::descriptor(),
        ContextRemindersStorageRow::descriptor(),
        ModelStatusStorageRow::descriptor(),
        ProfilesStorageRow::descriptor(),
        TranscriptRevisionsStorageRow::descriptor(),
    ]
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
#[derive(Deserialize, Debug, PartialEq, Eq)]
struct Entry {
    create_order: usize,
    name: String,
    sql: String,
    table: String,
    r#type: String,
}

#[test]
fn context_settings_descriptors_match_catalog_and_keep_existing_rows_exact() {
    let catalog: Catalog = studio_json::from_str(crate::fresh_state::SNAPSHOT).unwrap();
    let mut db = rusqlite::Connection::open_in_memory().unwrap();
    for descriptor in descriptors() {
        let entry = catalog
            .canvas
            .sqlite_master
            .iter()
            .find(|entry| entry.name == descriptor.name)
            .unwrap();
        assert_eq!(descriptor.ddl, entry.sql, "{} DDL", descriptor.name);
        let exported: Entry =
            studio_json::from_str(&descriptor.catalog_json(entry.create_order).unwrap()).unwrap();
        assert_eq!(&exported, entry);
        // Install the old catalog first, then prove descriptor installation leaves it intact.
        db.execute_batch(&entry.sql).unwrap();
        descriptor.install_if_absent(&db).unwrap();
        for index in descriptor.indexes {
            let entry = catalog
                .canvas
                .sqlite_master
                .iter()
                .find(|entry| entry.name == index.name)
                .unwrap();
            assert_eq!(entry.sql, index.sql);
        }
    }
    let manifest =
        " {\"epoch\":[\"t\",2],\"versions\":{\"worktreeReminder\":\"v1\"},\"sequence\":4} ";
    let status = " [ {\"kind\":\"agent\",\"id\":\"a\",\"name\":null,\"status\":\"idle\",\"inFlight\":null,\"parentId\":null,\"error\":\"\",\"future\": { \"n\": 1.00 }} ] ";
    db.execute(
        "INSERT INTO runtime_context_manifests VALUES('a','t',2,4,'e',?1)",
        [manifest],
    )
    .unwrap();
    db.execute(
        "INSERT INTO runtime_model_status VALUES('a','r',1.25,?1)",
        [status],
    )
    .unwrap();
    db.execute(
        "INSERT INTO runtime_transcript_revisions(agent) VALUES('a')",
        [],
    )
    .unwrap();
    let tx = db.transaction().unwrap();
    let mut typed = canvas_tx(&tx);
    let row = typed
        .get::<ContextManifestsStorageRow>(&("a".into(), "t".into(), 2))
        .unwrap()
        .unwrap();
    assert_eq!(row.record.raw(), manifest);
    typed.upsert(&row).unwrap();
    let row = typed
        .get::<ModelStatusStorageRow>(&("a".into(), "r".into()))
        .unwrap()
        .unwrap();
    assert_eq!(row.at, Some(1.25));
    assert_eq!(row.record.as_ref().unwrap().raw(), status);
    assert_eq!(
        row.record.as_ref().unwrap().value().0[0].key().as_deref(),
        Some("agent:a")
    );
    typed.upsert(&row).unwrap();
    assert_eq!(
        typed
            .get::<TranscriptRevisionsStorageRow>(&("a".into(),))
            .unwrap()
            .unwrap()
            .revision,
        0
    );
    tx.commit().unwrap();
    assert_eq!(
        db.query_row("SELECT record FROM runtime_context_manifests", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        manifest
    );
    assert_eq!(
        db.query_row("SELECT record FROM runtime_model_status", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        status
    );
    for descriptor in descriptors() {
        let report = crate::canvas_schema::registry()
            .into_iter()
            .find(|registration| registration.descriptor.name == descriptor.name)
            .unwrap();
        let check = (report.check)(&db).unwrap();
        assert_eq!(check.rejected_count, 0, "{} state check", descriptor.name);
    }
}

#[test]
fn context_settings_manifest_order_and_reminder_retry_keep_receipt_boundaries() {
    let mut db = rusqlite::Connection::open_in_memory().unwrap();
    ensure_context_tables(&db).unwrap();
    CompactionsStorageRow::descriptor()
        .install_if_absent(&db)
        .unwrap();
    let tx = db.transaction().unwrap();
    for (sequence, version, event) in [
        (4, "new", "accepted"),
        (2, "old", "stale"),
        (4, "new", "retried"),
    ] {
        write_manifest(
            &tx,
            &ContextManifestsStorageRow {
                agent: "a".into(),
                thread_id: "t".into(),
                compactions: 1,
                sequence,
                event_id: event.into(),
                record: ExactJson::from_value(&ContextManifest {
                    epoch: ContextEpoch(Some("t".into()), 1),
                    sequence: sequence as u64,
                    versions: BTreeMap::from([("worktreeReminder".into(), version.into())]),
                })
                .unwrap(),
            },
        )
        .unwrap();
    }
    assert_eq!(manifest(&tx, "a", "t", 1).unwrap().unwrap().sequence, 4);
    assert_eq!(manifests(&tx, "a").unwrap().len(), 1);
    assert_eq!(
        reminders(&tx, "a").unwrap(),
        std::collections::BTreeSet::from(["new".into(), "old".into()])
    );
    let receipt = canvas_read(&tx)
        .get::<ContextRemindersStorageRow>(&("a".into(), "new".into()))
        .unwrap()
        .unwrap();
    assert_eq!(receipt.event_id, "accepted");
    remember_compaction(&tx, "c", "a").unwrap();
    remember_compaction(&tx, "c", "other").unwrap();
    assert!(was_compacted(&tx, "c").unwrap());
    assert_eq!(
        canvas_read(&tx)
            .get::<CompactionsStorageRow>(&("c".into(),))
            .unwrap()
            .unwrap()
            .agent,
        "a"
    );
    tx.rollback().unwrap();
    assert!(manifests(&db, "a").unwrap().is_empty());
    assert!(!was_compacted(&db, "c").unwrap());
}

#[test]
fn context_settings_payload_owners_preserve_status_admission() {
    for raw in [
        r#"[{"kind":"agent","id":"a","status":"idle","inFlight":false,"parentId":null,"error":""}]"#,
        r#"[{"kind":"agent","id":"a","name":null,"status":"idle","inFlight":false,"parentId":null,"error":"","waitsForEvent":null}]"#,
        r#"[{"kind":"monitor","id":"m","agent":null,"status":"done","exitCode":1.5,"error":""}]"#,
        r#"[{"kind":"future","id":"m"}]"#,
    ] {
        assert!(
            ExactJson::<ModelStatusRecordPayload>::from_stored(raw.into()).is_err(),
            "{raw}"
        );
    }
    let duplicate = r#"[{"kind":"agent","id":"old","id":"a","name":null,"status":"idle","inFlight":false,"parentId":null,"error":""}]"#;
    let record = ExactJson::<ModelStatusRecordPayload>::from_stored(duplicate.into()).unwrap();
    assert_eq!(record.value().0[0].key().as_deref(), Some("agent:a"));
    assert_eq!(record.raw(), duplicate);
    let raw = r#"[{"kind":"monitor","id":"m","agent":null,"status":"done","exitCode":18446744073709551615,"error":"","future":{"raw":1.00}}]"#;
    let record = ExactJson::<ModelStatusRecordPayload>::from_stored(raw.into()).unwrap();
    assert_eq!(record.value().0[0].key().as_deref(), Some("monitor:m"));
    assert_eq!(studio_json::to_string(record.value()).unwrap(), raw);
}

#[test]
fn context_settings_new_reports_only_count_legacy_rows() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    for descriptor in descriptors() {
        descriptor.install_if_absent(&db).unwrap();
    }
    assert!(
        !crate::canvas_schema::registry()
            .iter()
            .any(|entry| entry.descriptor.name == "runtime_budget")
    );
    db.execute_batch(
        "INSERT INTO runtime_model_status VALUES(NULL,NULL,NULL,'[]');
        INSERT INTO runtime_compactions VALUES(NULL,'a');
        INSERT INTO runtime_context_manifests VALUES('a','t',-1,-1,'e','[]');
        INSERT INTO runtime_context_reminders VALUES('a','v','e');
        INSERT INTO runtime_transcript_revisions VALUES(NULL,0);",
    )
    .unwrap();
    for descriptor in descriptors()
        .into_iter()
        .filter(|entry| !matches!(entry.name, "runtime_assets" | "runtime_profiles"))
    {
        let registration = crate::canvas_schema::registry()
            .into_iter()
            .find(|entry| entry.descriptor.name == descriptor.name)
            .unwrap();
        let report = (registration.check)(&db).unwrap();
        assert_eq!(report.row_count, 1, "{} rows", registration.descriptor.name);
        assert_eq!(
            report.rejected_count, 0,
            "{} keeps prior acceptance",
            registration.descriptor.name
        );
        assert!(report.failures.is_empty());
    }
}
