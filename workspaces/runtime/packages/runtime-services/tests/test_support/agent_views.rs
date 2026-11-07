//! Projection assertions shared by service and HTTP frozen corpora.

use std::collections::BTreeMap;
use studio_json::Value;

/// Remove server projections that postdate the frozen Python agent oracle.
pub(crate) fn normalize_frozen_projection(fields: &mut studio_json::Map<String, Value>) {
    if fields.get("kind").and_then(Value::as_str) == Some("agent") {
        for key in [
            "accountChange",
            "accountTransferProgress",
            "accountDestinations",
            "manualConnectionCheck",
        ] {
            fields.remove(key);
        }
    }
}

#[allow(dead_code)]
pub(crate) fn materialize_agent_defaults(value: &mut Value) {
    let fields = value.as_object_mut().expect("agent fixture");
    fields
        .entry("capacity")
        .or_insert_with(|| studio_json::json!(studio_runtime_store::AgentCapacity::default()));
    fields
        .entry("naming")
        .or_insert_with(|| studio_json::json!(studio_runtime_store::AgentNaming::default()));
}

pub(crate) fn normalize_legacy_agent(value: &mut Value) {
    normalize_agent(
        value,
        studio_runtime_store::migrations::normalize_agent_fixtures,
    );
}

#[allow(dead_code)]
pub(crate) fn normalize_creation_agent(value: &mut Value) {
    normalize_agent(
        value,
        studio_runtime_store::migrations::normalize_agent_creation_fixtures,
    );
}

fn normalize_agent(
    value: &mut Value,
    apply: fn(&rusqlite::Transaction<'_>) -> studio_runtime_store::StoreResult<()>,
) {
    let mut db = rusqlite::Connection::open_in_memory().expect("fixture database");
    db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL)")
        .expect("fixture agents");
    db.execute(
        "INSERT INTO runtime_agents VALUES('fixture',?1)",
        [value.to_string()],
    )
    .expect("fixture agent");
    let tx = db.transaction().expect("fixture transaction");
    apply(&tx).expect("normalize legacy agent");
    let normalized: String = tx
        .query_row("SELECT record FROM runtime_agents", [], |row| row.get(0))
        .expect("normalized agent");
    *value = studio_json::from_str(&normalized).expect("normalized agent JSON");
}

/// Frozen Python snapshots predate the two view collections. Validate their
/// generated field ownership and values before comparing the legacy snapshot.
pub(crate) fn assert_agent_view_projections(db: &rusqlite::Connection) {
    let rows = studio_runtime_services::entity_scope::agent_snapshot(db)
        .expect("agent views")
        .into_iter()
        .filter(|row| row.collection != "retainedWorktree")
        .map(|row| {
            (
                (row.collection, row.id),
                (row.payload, i64::from(row.deleted)),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let schemas =
        &studio_runtime_services::sync_policy::openapi_document()["components"]["schemas"];
    for ((collection, id), (raw, deleted)) in &rows {
        if collection != "agentSummary" {
            continue;
        }
        let detail = rows
            .get(&("agentDetail".into(), id.clone()))
            .expect("detail accompanies summary");
        let legacy = rows
            .get(&("agent".into(), id.clone()))
            .expect("legacy accompanies summary");
        assert_eq!(*deleted, detail.1, "{id}: both views share deletion");
        assert_eq!(*deleted, legacy.1, "{id}: compatibility deletion");
        if *deleted != 0 {
            continue;
        }
        let summary: Value =
            studio_json::from_str(raw.as_deref().expect("summary payload")).expect("summary JSON");
        let detail: Value = studio_json::from_str(detail.0.as_deref().expect("detail payload"))
            .expect("detail JSON");
        let legacy: Value = studio_json::from_str(legacy.0.as_deref().expect("legacy payload"))
            .expect("legacy JSON");
        assert!(is_agent_view_collection(&summary["collection"]));
        let mut union = studio_json::Map::new();
        for (projection, schema) in [
            (&summary, "AgentSummaryEntityDto"),
            (&detail, "AgentDetailEntityDto"),
        ] {
            assert_eq!(projection["id"], *id);
            assert_eq!(projection["value"]["id"], *id);
            for (field, value) in projection["value"].as_object().expect("view object") {
                assert!(
                    schemas[schema]["properties"].get(field).is_some(),
                    "{schema}.{field} has one OpenAPI owner"
                );
                if let Some(expected) = legacy["value"].get(field) {
                    assert_eq!(value, expected, "{id}/{field}: view equals publication");
                }
                union.insert(field.clone(), value.clone());
            }
        }
        for (field, value) in legacy["value"].as_object().expect("legacy object") {
            // fe1e1f249 retired yoloMode from the split agent views; the
            // compatibility projection can still contain a frozen legacy row.
            if field == "yoloMode" {
                assert!(value.is_boolean(), "{id}: legacy yoloMode is boolean");
                assert!(!union.contains_key(field), "{id}: views omit yoloMode");
                continue;
            }
            assert_eq!(
                union.get(field),
                Some(value),
                "{id}/{field}: views cover legacy fields"
            );
        }
    }
}

pub(crate) fn is_agent_view_collection(collection: &Value) -> bool {
    collection == "agentSummary" || collection == "agentDetail"
}

/// All documents a new client receives, including every page of the durable views.
#[allow(dead_code)]
pub(crate) fn snapshot_rows(db: &rusqlite::Connection) -> Vec<studio_runtime_store::EntityRow> {
    use studio_runtime_services::entity_scope;
    let limit = studio_runtime_schema::shared_values::SYNC_ENTITY_STREAM_MAX_DOCUMENTS;
    let mut page =
        entity_scope::resume(db, None, None, None, None, true, limit).expect("entity snapshot");
    let mut rows = Vec::new();
    loop {
        rows.append(&mut page.rows);
        if page.checkpoint >= page.max_sequence {
            return rows;
        }
        page = entity_scope::continue_page(db, &page, None, None, None, true, limit)
            .expect("entity snapshot page");
    }
}

/// The documents that readers build at read time and the frozen corpora kept as
/// `sync_entities` copies: the legacy `agent` view, the task and monitor windows, the
/// event window and the message receipts.
#[allow(dead_code)]
pub(crate) fn read_time_rows(db: &rusqlite::Connection) -> Vec<studio_runtime_store::EntityRow> {
    studio_runtime_services::entity_scope::agent_snapshot(db)
        .expect("agent views")
        .into_iter()
        .filter(|row| row.collection == "agent")
        .chain(
            studio_runtime_services::entity_scope::live_documents(
                db,
                r#"["task","monitor","event","messageReceipt","project","peerTeam","rateLimit","accounts","accountCost","sessionCost","desktop","model","terminal","queue"]"#,
            )
            .expect("window documents"),
        )
        .collect()
}

/// Collections of [`read_time_rows`].
#[allow(dead_code)]
pub(crate) const READ_TIME_COLLECTIONS: [&str; 6] = [
    "agent",
    "task",
    "monitor",
    "event",
    "messageReceipt",
    "project",
];

/// Readers build a document for every source row; the frozen oracle kept copies only
/// of what it published. Keep the read-time rows of `actual` that the oracle has.
#[allow(dead_code)]
pub(crate) fn keep_oracle_rows(actual: &mut Vec<Value>, expected: &[Value]) {
    let published = expected
        .iter()
        .map(|row| (row["collection"].to_string(), row["id"].to_string()))
        .collect::<std::collections::BTreeSet<_>>();
    actual.retain(|row| {
        !READ_TIME_COLLECTIONS
            .iter()
            .any(|collection| row["collection"] == *collection)
            || published.contains(&(row["collection"].to_string(), row["id"].to_string()))
    });
}
