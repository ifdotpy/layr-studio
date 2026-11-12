//! Offline live-shaped recovery corpus, available only to test callers.
use studio_json::Value;
use studio_runtime_store::{
    AccountTransferRepository, AgentRecord, AgentRepository, MonitorRecord, RuntimeStore,
    StoreError, Table, peer_update_settings::canvas_tx,
};

pub fn rows() -> Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/live-restart/rows.json");
    let text = std::fs::read_to_string(path).expect("live restart fixture");
    assert!(
        text.len() < 100_000,
        "fixture must stay reviewable: {} bytes",
        text.len()
    );
    let fixture: Value = studio_json::from_str(&text).expect("fixture JSON");
    assert_private(&fixture);
    fixture
}

pub async fn seed(store: &RuntimeStore) {
    let fixture = rows();
    store
        .transaction(move |tx| {
            for value in fixture["runtime_agents"].as_array().expect("agents") {
                AgentRepository::put(tx, &AgentRecord::from_value(value.clone())?)?;
            }
            for value in fixture["runtime_account_transfers"]
                .as_array()
                .expect("transfers")
            {
                AccountTransferRepository::put(
                    tx,
                    value["id"].as_str().expect("id"),
                    &value.to_string(),
                )?;
            }
            for value in fixture["runtime_monitors"].as_array().expect("monitors") {
                Table::new(tx).put(&studio_json::from_str::<MonitorRecord>(&value.to_string())?)?;
            }
            for value in fixture["runtime_tool_requests"]
                .as_array()
                .expect("tool requests")
            {
                let mut value = value.clone();
                if value["kind"] == "provider_callback" && !value["payload"]["identity"].is_null() {
                    let owner: crate::tool_requests::CallbackTask<
                        crate::provider_callbacks::CallbackOwner,
                    > = studio_json::from_str(&value.to_string())?;
                    value["originKey"] =
                        studio_json::json!(crate::provider_callbacks::callback_origin_key(
                            &owner.payload.identity.context,
                            &owner.payload.identity.rpc_id
                        ));
                }
                canvas_tx(tx).insert(&ToolRequestsRow {
                    id: value["id"].as_str().expect("id").into(),
                    record: value.to_string().into(),
                    agent: None,
                    kind: None,
                    status: None,
                    request_id: None,
                    origin_key: None,
                    updated: None,
                })?;
            }
            for row in fixture["runtime_events"].as_array().expect("events") {
                studio_runtime_store::QueueEventRepository::put(
                    tx,
                    &studio_runtime_store::QueueEvent {
                        id: row["id"].as_str().expect("id").into(),
                        agent: row["agent"].as_str().expect("agent").into(),
                        kind: row["kind"].as_str().expect("kind").into(),
                        text: row["text"].as_str().expect("text").into(),
                        status: row["status"].as_str().expect("status").into(),
                        created: row["created"].as_f64().expect("created"),
                        epoch: row["epoch"].as_i64().expect("epoch"),
                        turn_id: row["turn_id"].as_str().map(Into::into),
                        error: row["error"].as_str().map(Into::into),
                    },
                )?;
            }
            for row in fixture["runtime_outbox"].as_array().expect("outbox") {
                let mut row = row.clone();
                if row["kind"] == "provider_callback" {
                    let response: crate::provider_callbacks::CallbackResponse =
                        studio_json::from_str(row["payload"].as_str().expect("callback payload"))?;
                    row["id"] = studio_json::json!(crate::provider_callbacks::callback_origin_key(
                        &response.identity.context,
                        &response.identity.rpc_id
                    ));
                }
                studio_runtime_store::outbox::Outbox::put(
                    tx,
                    &studio_runtime_store::outbox::OutboxRow {
                        id: row["id"].as_str().expect("id").into(),
                        kind: studio_json::from_str(&row["kind"].to_string())?,
                        owner_kind: studio_json::from_str(&row["owner_kind"].to_string())?,
                        owner_id: row["owner_id"].as_str().expect("owner").into(),
                        request_id: row["request_id"].as_str().map(Into::into),
                        signature: row["signature"].as_str().expect("signature").into(),
                        status: studio_json::from_str(&row["status"].to_string())?,
                        account: row["account"].as_str().map(Into::into),
                        stream_id: row["stream_id"].as_str().map(Into::into),
                        generation: row["generation"].as_i64(),
                        ordinal: row["ordinal"].as_i64(),
                        next_at: row["next_at"].as_i64(),
                        payload: studio_json::from_str::<Value>(
                            row["payload"].as_str().expect("payload"),
                        )?,
                        created: row["created"].as_i64().expect("created"),
                    },
                )
                .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
            }
            for row in fixture["runtime_tasks"].as_array().expect("tasks") {
                Table::new(tx).put(&studio_json::from_str::<studio_runtime_store::TaskRecord>(
                    &row.to_string(),
                )?)?;
            }
            for row in fixture["runtime_rules"].as_array().expect("rules") {
                Table::new(tx).put(&studio_json::from_str::<studio_runtime_store::RuleRecord>(
                    &row.to_string(),
                )?)?;
            }
            use studio_runtime_store::queue_schema::*;
            for row in fixture["runtime_execution_runs"].as_array().expect("runs") {
                canvas_tx(tx).insert(&ExecutionRunsRow {
                    id: scalar(row, "id"),
                    agent: scalar(row, "agent"),
                    account: scalar(row, "account"),
                    epoch: scalar(row, "epoch"),
                    thread: scalar(row, "thread"),
                    turn: scalar(row, "turn"),
                    created: scalar(row, "created"),
                    record: row["record"].as_str().expect("record").to_owned().into(),
                    status: None,
                })?;
            }
            for row in fixture["runtime_execution_attempts"]
                .as_array()
                .expect("attempts")
            {
                canvas_tx(tx).insert(&ExecutionAttemptsRow {
                    id: scalar(row, "id"),
                    run: scalar(row, "run"),
                    record: row["record"].as_str().expect("record").to_owned().into(),
                    submission: None,
                    turn_status: None,
                })?;
            }
            for row in fixture["runtime_execution_nodes"]
                .as_array()
                .expect("nodes")
            {
                canvas_tx(tx).insert(&ExecutionNodesRow {
                    id: scalar(row, "id"),
                    run: scalar(row, "run"),
                    record: row["record"].as_str().expect("record").to_owned().into(),
                    status: None,
                })?;
            }
            for row in fixture["runtime_execution_effects"]
                .as_array()
                .expect("effects")
            {
                canvas_tx(tx).insert(&ExecutionEffectsRow {
                    id: scalar(row, "id"),
                    run: scalar(row, "run"),
                    kind: scalar(row, "kind"),
                    reference: scalar(row, "reference"),
                    record: row["record"].as_str().expect("record").to_owned().into(),
                    status: None,
                    receipt_enum_guard: None,
                })?;
            }
            for row in fixture["runtime_execution_inputs"]
                .as_array()
                .expect("inputs")
            {
                canvas_tx(tx).insert(&ExecutionInputsRow {
                    run: scalar(row, "run"),
                    event: scalar(row, "event"),
                })?;
            }
            for row in fixture["runtime_account_state"]
                .as_array()
                .expect("account state")
            {
                canvas_tx(tx).insert(&studio_runtime_store::receipt_tables::AccountStateRow {
                    account: scalar(row, "account"),
                    next_generation: scalar(row, "next_generation"),
                    connection_generation: scalar(row, "connection_generation"),
                    connection_id: scalar(row, "connection_id"),
                    journal_ack: scalar(row, "journal_ack"),
                    recovery_required: row["recovery_required"] == 1,
                    streams: json_column(row, "streams"),
                    limits_observation: optional_json_column(row, "limits_observation"),
                    start_failure: optional_json_column(row, "start_failure"),
                })?;
            }
            for row in fixture["approvals"].as_array().expect("approvals") {
                canvas_tx(tx).insert(&studio_runtime_store::interaction_tables::ApprovalsRow {
                    id: scalar(row, "id"),
                    agent: scalar(row, "agent"),
                    turn: scalar(row, "turn"),
                    created: scalar(row, "created"),
                    epoch: scalar(row, "epoch"),
                    kind: scalar(row, "kind"),
                    request: json_column(row, "request"),
                    delivery: optional_json_column(row, "delivery"),
                    detail: json_column(row, "detail"),
                    state: scalar(row, "state"),
                    answer: optional_json_column(row, "answer"),
                    command: scalar(row, "command"),
                    answered_by: optional_json_column(row, "answered_by"),
                    at: scalar(row, "at"),
                    since: scalar(row, "since"),
                    reason: scalar(row, "reason"),
                    deferral_active: scalar(row, "deferral_active"),
                    deferral_at: scalar(row, "deferral_at"),
                    deferral_by: optional_json_column(row, "deferral_by"),
                    fire_at: scalar(row, "fire_at"),
                    hidden_at: scalar(row, "hidden_at"),
                    hidden_by: optional_json_column(row, "hidden_by"),
                })?;
            }
            Ok(())
        })
        .await
        .expect("load anonymized rows through store owners");
}

fn scalar<T: serde::de::DeserializeOwned>(row: &Value, key: &str) -> T {
    studio_json::from_str(&row[key].to_string())
        .unwrap_or_else(|error| panic!("fixture column {key}: {error}"))
}
fn json_column<T: serde::de::DeserializeOwned>(row: &Value, key: &str) -> T {
    studio_json::from_str(row[key].as_str().expect("JSON column"))
        .unwrap_or_else(|error| panic!("fixture JSON column {key}: {error}"))
}
fn optional_json_column<T: serde::de::DeserializeOwned>(row: &Value, key: &str) -> Option<T> {
    row[key].as_str().map(|_| json_column(row, key))
}

/// Reviewed vocabulary shared with the offline exporter; regeneration cannot extend it.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublicContract {
    keys: std::collections::BTreeSet<String>,
    literals: std::collections::BTreeSet<String>,
    methods: std::collections::BTreeSet<String>,
    json_fields: std::collections::BTreeSet<String>,
    dynamic_maps: std::collections::BTreeSet<String>,
    placeholder_pattern: String,
    free_text_pattern: String,
}

fn privacy_check(value: &Value, field: &str) -> Result<(), &'static str> {
    static CONTRACT: std::sync::LazyLock<PublicContract> = std::sync::LazyLock::new(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/live-restart/public-contract.json");
        studio_json::from_str(&std::fs::read_to_string(path).expect("public fixture contract"))
            .expect("public fixture vocabulary")
    });
    static PLACEHOLDER: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(&CONTRACT.placeholder_pattern).expect("placeholder grammar")
    });
    static FREE_TEXT: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::RegexBuilder::new(&CONTRACT.free_text_pattern)
            .case_insensitive(true)
            .build()
            .expect("free text fields")
    });
    match value {
        Value::String(text) => {
            if CONTRACT.json_fields.contains(field)
                && let Ok(nested) = studio_json::from_str::<Value>(text)
                && (nested.is_object() || nested.is_array())
            {
                return privacy_check(&nested, "");
            }
            if text.is_empty() || PLACEHOLDER.is_match(text) {
                return Ok(());
            }
            if FREE_TEXT.is_match(field) {
                return Err("non-placeholder free text");
            }
            if field == "method" {
                if !CONTRACT.methods.contains(text) {
                    return Err("nonpublic wire method");
                }
            } else if !CONTRACT.literals.contains(text) {
                return Err("nonpublic fixture literal");
            }
        }
        Value::Array(rows) => {
            for row in rows {
                privacy_check(row, field)?;
            }
        }
        Value::Object(fields) => {
            for (key, value) in fields {
                if !PLACEHOLDER.is_match(key)
                    && (CONTRACT.dynamic_maps.contains(field) || !CONTRACT.keys.contains(key))
                {
                    return Err("nonpublic fixture map key");
                }
                privacy_check(value, key)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn assert_private(value: &Value) {
    privacy_check(value, "").expect("fixture must contain only public contracts and placeholders");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_restart_fixture_privacy_and_size() {
        let fixture = rows();
        let bytes = std::fs::metadata(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/live-restart/rows.json"),
        )
        .expect("fixture size")
        .len();
        println!(
            "fixture privacy: PASS; bytes={bytes}; home paths=0; account markers=0; credentials/keys=0; hostnames=0; real UUIDs=0; long free text=0"
        );
        assert!(fixture["runtime_account_transfers"][0]["members"].is_object());
    }

    #[test]
    fn live_restart_privacy_rejects_nested_private_data() {
        let home = ["", "home", "example", "project"].join("/");
        for value in [
            studio_json::json!({"members":{"alice_smith":{}}}),
            studio_json::json!({"metadata":{"AcmePrivateRepository":null}}),
            studio_json::json!({"members":{"id":null}}),
            studio_json::json!({"name":"Alice"}),
            studio_json::json!({"prompt":"private prompt"}),
            studio_json::json!({"prompt":"native"}),
            studio_json::json!({"method":"AcmePrivateRepository"}),
            studio_json::json!({"record":r#"{"text":"short private prompt"}"#}),
            Value::String("buildbox".into()),
            Value::String("https://internal.dev".into()),
            Value::String("ABCDEFAB-CDEF-4ABC-8DEF-ABCDEFABCDEF".into()),
            Value::String(home),
            Value::String("person@example.invalid".into()),
            Value::String("sk-proj-not-a-real-key-but-must-be-rejected".into()),
            Value::String("private prose ".repeat(20)),
        ] {
            assert!(
                privacy_check(&value, "").is_err(),
                "privacy witness accepted private data"
            );
        }
    }
}
