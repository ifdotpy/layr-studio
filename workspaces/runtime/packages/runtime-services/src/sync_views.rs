//! Canonical renderer entity views built from records and explicit context.

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};
use studio_runtime_schema::provider_errors::is_misalignment_policy_violation;

use crate::event_write::truthy;
use crate::submit_gates::{AgentAuthorityRow, Deletion, PeerTeamRow, StoredField};

/// Derived list-attention fields. Compatibility agent envelopes omit these;
/// view-scoped summaries own them. Unit: OpenAPI field names.
pub(crate) const AGENT_SUMMARY_ATTENTION_FIELDS: [&str; 3] =
    ["hasQuestion", "hasApproval", "hasBackgroundWork"];

/// Maximum last-answer text retained in an agent sync view. Unit: Unicode scalar values.
pub(crate) const AGENT_SYNC_LAST_ANSWER_CHARS: usize = 4_000;
pub(crate) const SYNC_TEXT_CHARS: usize = 12_000;
/// Maximum items in a projected sync array. Unit: array elements.
pub(crate) const SYNC_ARRAY_ITEMS: usize = 200;
/// Maximum sidebar order entries in a sync view. Unit: array elements.
const SYNC_SIDEBAR_ORDER_ITEMS: usize = 10_000;
/// Maximum recursive sync projection depth. Unit: nested JSON levels.
const SYNC_JSON_DEPTH: usize = 32;

pub(crate) fn bound_opaque_json(value: &mut Value, field: &str, depth: usize) {
    if depth > SYNC_JSON_DEPTH {
        *value = Value::Null;
        return;
    }
    match value {
        Value::String(text) => *text = bound_sync_text(text, field),
        Value::Array(items) => {
            items.truncate(if field == "sidebarOrder" {
                SYNC_SIDEBAR_ORDER_ITEMS
            } else {
                SYNC_ARRAY_ITEMS
            });
            for item in items {
                bound_opaque_json(item, "", depth + 1);
            }
        }
        Value::Object(fields) => {
            for (name, item) in fields {
                bound_opaque_json(item, name, depth + 1);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

pub(crate) fn bound_raw_json(
    raw: &serde_json::value::RawValue,
    field: &str,
    depth: usize,
) -> Result<Box<serde_json::value::RawValue>, serde_json::Error> {
    let mut value: Value = serde_json::from_str(raw.get())?;
    bound_opaque_json(&mut value, field, depth);
    serde_json::value::to_raw_value(&value)
}

const AGENT_SHORT_STRING_OBJECTS: &[&str] = &[
    "activity",
    "nativeStatus",
    "nativeSafetyBuffering",
    "nativeSafetyRetry",
    "nativeTurnError",
    "nativeThreadBlock",
    "connectionCheck",
    "readState",
    "startAttempt",
    "capacityRetry",
    "usageResume",
    "contextRepairWait",
];

/// The committed API schema is parsed once and shared with HTTP validation.
pub fn openapi_document() -> &'static Value {
    static DOCUMENT: OnceLock<Value> = OnceLock::new();
    DOCUMENT.get_or_init(|| {
        serde_json::from_str(include_str!("../../runtime-http/openapi.json"))
            .expect("the committed OpenAPI document is valid JSON")
    })
}

/// Hash of the committed API schema, the current contract version.
pub fn api_schema_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| studio_runtime_schema::api_schema::schema_hash(openapi_document()))
}

fn schema_definition<'a>(schema: &'a Value, document: &'a Value) -> &'a Value {
    schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix('#'))
        .and_then(|pointer| document.pointer(pointer))
        .unwrap_or(schema)
}

fn collect_schema_properties<'a>(
    schema: &'a Value,
    document: &'a Value,
    properties: &mut BTreeMap<&'a str, &'a Value>,
    depth: usize,
) {
    if depth > SYNC_JSON_DEPTH {
        return;
    }
    let schema = schema_definition(schema, document);
    if let Some(fields) = schema.get("properties").and_then(Value::as_object) {
        properties.extend(fields.iter().map(|(name, value)| (name.as_str(), value)));
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(variants) = schema.get(keyword).and_then(Value::as_array) {
            for variant in variants {
                collect_schema_properties(variant, document, properties, depth + 1);
            }
        }
    }
}

type SchemaProjectionProperties = HashMap<usize, BTreeMap<&'static str, &'static Value>>;

fn schema_projection_properties() -> &'static SchemaProjectionProperties {
    static PROPERTIES: OnceLock<SchemaProjectionProperties> = OnceLock::new();
    PROPERTIES.get_or_init(|| {
        fn visit(
            schema: &'static Value,
            document: &'static Value,
            properties: &mut SchemaProjectionProperties,
        ) {
            if schema.is_object() {
                let is_schema = ["$ref", "properties", "allOf", "anyOf", "oneOf", "items"]
                    .iter()
                    .any(|key| schema.get(*key).is_some());
                if is_schema {
                    let mut fields = BTreeMap::new();
                    collect_schema_properties(schema, document, &mut fields, 0);
                    properties.insert(schema as *const Value as usize, fields);
                }
                if let Some(object) = schema.as_object() {
                    for child in object.values() {
                        visit(child, document, properties);
                    }
                }
            } else if let Some(items) = schema.as_array() {
                for child in items {
                    visit(child, document, properties);
                }
            }
        }

        let document = openapi_document();
        let mut properties = HashMap::new();
        if let Some(schemas) = document.pointer("/components/schemas") {
            visit(schemas, document, &mut properties);
        }
        properties
    })
}

pub(crate) fn bound_sync_text(text: &str, field: &str) -> String {
    bound_sync_text_mode(text, field, false)
}

fn bound_sync_text_mode(text: &str, field: &str, agent_short_strings: bool) -> String {
    let limit = if agent_short_strings {
        2_000
    } else {
        match field {
            "overview" => 9_000,
            "error" | "tail" | "description" | "command" | "query" => 2_000,
            "lastAnswer" => AGENT_SYNC_LAST_ANSWER_CHARS,
            "text" => 4_000,
            _ => SYNC_TEXT_CHARS,
        }
    };
    text.chars().take(limit).collect()
}
fn bound_sync_value(value: &Value, field: &str, agent_short_strings: bool) -> Value {
    match value {
        Value::String(text) => {
            Value::String(bound_sync_text_mode(text, field, agent_short_strings))
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
            value.clone()
        }
    }
}

fn project_schema_value(
    value: &Value,
    schema: &Value,
    field: &str,
    document: &Value,
    depth: usize,
    agent_record: bool,
    agent_short_strings: bool,
) -> Value {
    if depth > SYNC_JSON_DEPTH {
        return Value::Null;
    }
    let schema = schema_definition(schema, document);
    match value {
        Value::Object(source) => {
            let properties = schema_projection_properties().get(&(schema as *const Value as usize));
            if properties.is_none_or(BTreeMap::is_empty) {
                if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                    return Value::Object(Map::new());
                }
                return Value::Object(
                    source
                        .iter()
                        .map(|(name, child)| {
                            (
                                name.clone(),
                                project_schema_value(
                                    child,
                                    &Value::Null,
                                    name,
                                    document,
                                    depth + 1,
                                    agent_record,
                                    agent_short_strings,
                                ),
                            )
                        })
                        .collect(),
                );
            }
            Value::Object(
                source
                    .iter()
                    .filter_map(|(name, child)| {
                        properties
                            .and_then(|properties| properties.get(name.as_str()))
                            .map(|field_schema| {
                                (name.clone(), {
                                    let short_strings = agent_short_strings
                                        || (agent_record
                                            && depth == 0
                                            && AGENT_SHORT_STRING_OBJECTS.contains(&name.as_str()));
                                    project_schema_value(
                                        child,
                                        field_schema,
                                        name,
                                        document,
                                        depth + 1,
                                        agent_record,
                                        short_strings,
                                    )
                                })
                            })
                    })
                    .collect(),
            )
        }
        Value::Array(items) => {
            let item_schema = schema.get("items").unwrap_or(&Value::Null);
            let limit = if field == "sidebarOrder" {
                SYNC_SIDEBAR_ORDER_ITEMS
            } else {
                SYNC_ARRAY_ITEMS
            };
            Value::Array(
                items
                    .iter()
                    .take(limit)
                    .map(|item| {
                        project_schema_value(
                            item,
                            item_schema,
                            "",
                            document,
                            depth + 1,
                            agent_record,
                            agent_short_strings,
                        )
                    })
                    .collect(),
            )
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            bound_sync_value(value, field, agent_short_strings)
        }
    }
}

use studio_runtime_schema::process_controls::TaskProjection;

pub fn project_task_record(record: &studio_runtime_store::TaskRecord) -> Option<TaskProjection> {
    use studio_runtime_schema::task_record::ProcessId;
    let id = record.id().as_ref();
    if id.is_empty() {
        return None;
    }
    let bounded = |text: &str, field| bound_sync_text(text, field);
    let optional =
        |value: &Option<String>, field| value.as_deref().map(|text| bounded(text, field));
    let (tail, truncated) = record.tail.as_deref().map_or((None, false), |text| {
        let summary = bounded(text, "tail");
        if summary == text {
            (Some(summary), false)
        } else {
            let length = summary.chars().count();
            let suffix = text.chars().rev().take(length).collect::<String>();
            (Some(suffix.chars().rev().collect()), true)
        }
    });
    Some(TaskProjection {
        display: crate::process_controls::task_record_display(record),
        id: bounded(id, "id"),
        agent: bounded(record.agent(), "agent"),
        created: record.created_number().clone(),
        kind: record.kind(),
        status: record.status(),
        turn_id: bounded(record.turn_id(), "turnId"),
        name: bounded(&record.name, "name"),
        command: optional(&record.command, "command"),
        cwd: optional(&record.cwd, "cwd"),
        duration_ms: record.duration_ms.clone(),
        error: optional(&record.error, "error"),
        exit_code: record.exit_code,
        finished: record.finished,
        interactive: record.interactive(),
        output_file: optional(&record.output_file, "outputFile"),
        output_truncated: if truncated {
            Some(true)
        } else {
            record.output_truncated
        },
        process_id: record.process_id().map(|pid| match pid {
            ProcessId::Text(text) => ProcessId::Text(bounded(text, "processId")),
            ProcessId::Signed(value) => ProcessId::Signed(*value),
            ProcessId::Unsigned(value) => ProcessId::Unsigned(*value),
        }),
        query: optional(&record.query, "query"),
        tail,
    })
}

/// Public work summary; archive is opaque server-produced JSON.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkProjection {
    id: String,
    #[serde(skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing")]
    root_id: studio_runtime_schema::queue_command::Field<String>,
    status: studio_runtime_schema::persistence::WorkStatus,
    #[serde(skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing")]
    title: studio_runtime_schema::queue_command::Field<String>,
    #[serde(skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing")]
    agent: studio_runtime_schema::queue_command::Field<String>,
    #[serde(skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing")]
    archive: studio_runtime_schema::queue_command::Field<studio_runtime_schema::raw_json::RawJson>,
}

pub fn project_work_record(
    record: &studio_runtime_store::WorkItemRecord,
) -> Option<WorkProjection> {
    use studio_runtime_schema::queue_command::Field;
    let id = record.id().as_ref();
    if id.is_empty() {
        return None;
    }
    let field = |value: &Field<String>, name| match value {
        Field::Missing => Field::Missing,
        Field::Null => Field::Null,
        Field::Value(value) => Field::Value(bound_sync_text(value, name)),
    };
    Some(WorkProjection {
        id: bound_sync_text(id, "id"),
        status: record.status(),
        root_id: field(record.root_id_field(), "rootId"),
        title: field(record.title_field(), "title"),
        agent: field(record.agent_field(), "agent"),
        archive: match record.archive_field() {
            Field::Missing => Field::Missing,
            Field::Null => Field::Null,
            Field::Value(raw) => Field::Value(studio_runtime_schema::raw_json::RawJson(
                bound_raw_json(&raw.0, "archive", 1).ok()?,
            )),
        },
    })
}

/// Filter a stored record through the committed renderer DTO schema.
pub fn project_persisted_record(collection: &str, record: &Value) -> Option<Value> {
    let schema_name = match collection {
        "agent" => "AgentEntityDto",
        "agentSummary" => "AgentSummaryEntityDto",
        "agentDetail" => "AgentDetailEntityDto",

        _ => return None,
    };
    record
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())?;
    let document = openapi_document();
    let schema = document.pointer(&format!("/components/schemas/{schema_name}"))?;
    // Cleanup receipts contain private operation identities. Publish only the
    // retained folder and the reason the cleanup owner recorded.
    let mut public_record = std::borrow::Cow::Borrowed(record);
    if collection == "agent"
        && let Some(intent) = record
            .get("workspaceCleanupIntent")
            .filter(|v| v.is_object())
    {
        public_record.to_mut()["workspaceCleanupIntent"] = json!({
            "phase": intent.get("phase").and_then(Value::as_str).unwrap_or("unknown"),
            "reason": intent.pointer("/receipt/reason").or_else(|| intent.get("reason")).filter(|value| value.is_string()).map(|value| bound_sync_value(value, "error", false)),
            "path": intent.pointer("/effect/identity/cwd").or_else(|| intent.get("path")).or_else(|| record.get("cwd")).and_then(Value::as_str),
        });
    }
    let mut projected = project_schema_value(
        &public_record,
        schema,
        "",
        document,
        0,
        collection == "agent",
        false,
    );
    if matches!(collection, "agent" | "agentDetail")
        && let Some(gate) = record.get("taskDefaultGate")
    {
        projected["taskDefaultGate"] = gate.clone();
    }
    Some(projected)
}

#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct AgentViewContext {
    pub account_destinations: Vec<studio_runtime_schema::account_destination::AccountDestination>,
    pub transfer_can_cancel: bool,
    pub team_counts: Option<studio_runtime_schema::agent_progress::TeamCounts>,
    pub project_server_id: Option<String>,
    pub launcher_alive: bool,
    pub empty_lead: bool,
    pub has_question: bool,
    pub has_approval: bool,
    pub latest_work_result_file: Option<String>,
    pub command_progress: Vec<studio_runtime_schema::agent_progress::AgentCommandProgress>,
    pub stall: Option<studio_runtime_schema::agent_progress::AgentStall>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountChangeAgent {
    #[serde(default)]
    is_lead: Option<bool>,
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    in_flight: Option<bool>,
}

/// Minimal displayable record for an agent whose stored schema failed. Keep
/// valid hierarchy and lead identity so the renderer does not detach a held
/// child from its root or render a lead as a worker.
pub(crate) fn invalid_agent_view(id: &str, record: &Value, diagnostic: &str) -> Value {
    let mut view = json!({
        "id": id,
        "rootId": record.get("rootId").and_then(Value::as_str).unwrap_or(id),
        "epoch": record.get("epoch").and_then(Value::as_i64).unwrap_or_default(),
        "status": "failed",
        "error": diagnostic,
        "nativeFailureHold": true,
        "autoWake": false
    });
    if let Some(is_lead) = record.get("isLead").filter(|value| value.is_boolean()) {
        view["isLead"] = is_lead.clone();
    }
    view
}

/// Canonical wait projection used by both sync/HTTP and lead status.
pub(crate) fn agent_start_wait(
    record: &Value,
) -> Result<Option<studio_runtime_schema::agent_progress::AgentStartWait>, serde_json::Error> {
    use studio_runtime_schema::agent_progress::AgentStartWait;
    // Older private native holds can carry a diagnostic object. Decode the
    // envelope without exposing or inspecting its diagnostic fields.
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum NativeHold {
        Flag(bool),
        Diagnostic(BTreeMap<String, serde::de::IgnoredAny>),
    }
    impl NativeHold {
        fn active(self) -> bool {
            match self {
                Self::Flag(active) => active,
                Self::Diagnostic(fields) => !fields.is_empty(),
            }
        }
    }
    // Agent records are open documents; this typed projection reads only wait ownership.
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Source {
        status: Option<studio_runtime_store::AgentStatus>,
        #[serde(default)]
        epoch: i64,
        #[serde(default)]
        auto_wake: bool,
        #[serde(default)]
        native_failure_hold: Option<NativeHold>,
        account_transfer_id: Option<String>,
        #[serde(default, deserialize_with = "present_wait")]
        start_wait: Option<AgentStartWait>,
    }
    fn present_wait<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Option<AgentStartWait>, D::Error> {
        <AgentStartWait as serde::Deserialize>::deserialize(d).map(Some)
    }
    let source = <Source as serde::Deserialize>::deserialize(record)?;
    if !matches!(
        source.status,
        Some(
            studio_runtime_store::AgentStatus::Queued | studio_runtime_store::AgentStatus::Starting
        )
    ) {
        return Ok(None);
    }
    if source.status == Some(studio_runtime_store::AgentStatus::Queued) {
        let hold = if !source.auto_wake {
            Some(AgentStartWait::Paused {
                epoch: source.epoch,
            })
        } else if source.native_failure_hold.is_some_and(NativeHold::active) {
            Some(AgentStartWait::NativeFailure {
                epoch: source.epoch,
            })
        } else if source.account_transfer_id.is_some() {
            Some(AgentStartWait::AccountTransfer {
                epoch: source.epoch,
            })
        } else {
            None
        };
        if hold.is_some() {
            return Ok(hold);
        }
    }
    let persisted = source
        .start_wait
        .filter(|wait| wait.epoch() == source.epoch);
    if source.status == Some(studio_runtime_store::AgentStatus::Starting) {
        return Ok(persisted);
    }
    Ok(Some(persisted.unwrap_or(AgentStartWait::DispatchPending {
        epoch: source.epoch,
    })))
}

/// Build the managed renderer view from a record and explicit runtime context.
pub fn agent_entity_view(record: &Value, context: &AgentViewContext) -> Option<Value> {
    let id = record.get("id").and_then(Value::as_str)?;
    #[cfg(test)]
    crate::scheduler::publication_work::projection(id);
    let mut view = record.as_object()?.clone();
    view.insert(
        studio_runtime_schema::agent_progress::START_WAIT_FIELD.into(),
        serde_json::to_value(agent_start_wait(record).ok()?).ok()?,
    );

    view.insert(
        "commandProgress".to_owned(),
        serde_json::to_value(&context.command_progress).ok()?,
    );
    view.insert(
        "stall".to_owned(),
        serde_json::to_value(&context.stall).ok()?,
    );
    let remote_server = record
        .pointer("/remoteWorker/server")
        .and_then(Value::as_str)
        .filter(|server| !server.is_empty());
    view.insert(
        "serverId".to_owned(),
        remote_server
            .or(context.project_server_id.as_deref())
            .map_or(Value::Null, |value| Value::String(value.to_owned())),
    );
    view.insert("id".to_owned(), Value::String(id.to_owned()));
    view.insert("kind".to_owned(), Value::String("agent".to_owned()));
    view.insert("source".to_owned(), Value::String("managed".to_owned()));
    let progress = crate::account_transfer::progress_view(
        record.get("accountTransfer"),
        context.transfer_can_cancel,
    )
    .ok()?;
    if let Some(summary) = view
        .get_mut("accountTransfer")
        .and_then(Value::as_object_mut)
    {
        summary.remove("canCancel");
    }
    let source = <AccountChangeAgent as serde::Deserialize>::deserialize(record).ok()?;
    if source.is_lead == Some(true) {
        let destinations = context
            .account_destinations
            .iter()
            .cloned()
            .map(|mut destination| {
                if crate::account_transfer_driver::account_is_excluded(
                    record.get("excludedAccountKeys"),
                    destination.account_key.as_str(),
                ) {
                    destination.eligibility =
                        studio_runtime_schema::agent_progress::AccountEligibility::Unavailable {
                            reason: "This account is disabled for this chat".into(),
                        };
                }
                destination
            })
            .collect::<Vec<_>>();
        view.insert(
            "accountDestinations".into(),
            serde::Serialize::serialize(&destinations, serde_json::value::Serializer).ok()?,
        );
    }
    let account_change = if source.is_lead != Some(true) {
        studio_runtime_schema::agent_progress::AccountChange::Unavailable {
            reason: "Choose the lead chat to change its account".into(),
        }
    } else if matches!(
        progress,
        studio_runtime_schema::agent_progress::AccountTransferProgress::Waiting { .. }
            | studio_runtime_schema::agent_progress::AccountTransferProgress::Moving { .. }
            | studio_runtime_schema::agent_progress::AccountTransferProgress::Blocked { .. }
    ) {
        studio_runtime_schema::agent_progress::AccountChange::Unavailable {
            reason: "Finish or cancel the current transfer first".into(),
        }
    } else if context.empty_lead
        && source.thread_id.as_deref().is_none_or(str::is_empty)
        && source.in_flight != Some(true)
    {
        studio_runtime_schema::agent_progress::AccountChange::Select
    } else {
        studio_runtime_schema::agent_progress::AccountChange::Transfer
    };
    view.insert(
        "accountChange".into(),
        serde::Serialize::serialize(&account_change, serde_json::value::Serializer).ok()?,
    );
    view.insert(
        "accountTransferProgress".into(),
        serde::Serialize::serialize(&progress, serde_json::value::Serializer).ok()?,
    );
    let block = native_thread_block(record);
    let can_send = block.is_none()
        && !truthy(record.get("movedTo"))
        && !truthy(record.get("moveImportPending"));
    view.insert("canSend".to_owned(), Value::Bool(can_send));
    view.insert(
        "launcherAlive".to_owned(),
        Value::Bool(context.launcher_alive),
    );
    view.insert("nextTurnSettingsSupported".to_owned(), Value::Bool(true));
    view.insert("readStateSupported".to_owned(), Value::Bool(true));
    // False attention is represented by absence, preserving the legacy DTO's
    // wire shape while a replacement summary clears any previous true flag.
    for (field, enabled) in AGENT_SUMMARY_ATTENTION_FIELDS.into_iter().zip([
        context.has_question,
        context.has_approval,
        !context.command_progress.is_empty(),
    ]) {
        if enabled {
            view.insert(field.to_owned(), Value::Bool(true));
        } else {
            view.remove(field);
        }
    }
    if let Some(block) = block {
        view.insert("nativeThreadBlock".to_owned(), block);
    } else {
        view.remove("nativeThreadBlock");
    }
    if truthy(record.get("isLead")) {
        if let Some(counts) = &context.team_counts {
            view.insert("teamCounts".to_owned(), serde_json::to_value(counts).ok()?);
        }
        view.remove("overview");
        view.insert("empty".to_owned(), Value::Bool(context.empty_lead));
    } else {
        let task = record
            .get("prompt")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let result = if truthy(record.get("lastCompletedTurn"))
            && !truthy(record.get("turnId"))
            && !truthy(record.get("inFlight"))
            && record.get("status").and_then(Value::as_str) == Some("completed")
        {
            record
                .get("lastAnswer")
                .and_then(Value::as_str)
                .unwrap_or_default()
        } else {
            ""
        };
        view.insert(
            "overview".to_owned(),
            json!({
                "task":task.chars().take(4000).collect::<String>(),
                "taskTruncated":task.chars().count()>4000,
                "result":result.chars().take(4000).collect::<String>(),
                "resultTruncated":result.chars().count()>4000,
                "resultTurnId":if result.is_empty(){Value::Null}else{record.get("lastCompletedTurn").cloned().unwrap_or(Value::Null)},
                "resultFile":context.latest_work_result_file
            }),
        );
    }
    project_persisted_record("agent", &Value::Object(view))
}

pub fn native_thread_block(record: &Value) -> Option<Value> {
    let thread_id = record.get("threadId").and_then(Value::as_str)?;
    if thread_id.is_empty() {
        return None;
    }
    let block = record.get("nativeThreadBlock").and_then(Value::as_object);
    if block
        .and_then(|block| block.get("threadId"))
        .and_then(Value::as_str)
        == Some(thread_id)
    {
        return Some(Value::Object(block?.clone()));
    }
    let blocked = |error: &Value| {
        is_misalignment_policy_violation(error).then(|| json!({"threadId":thread_id,"error":error}))
    };
    if let Some(block) = record.get("error").and_then(blocked) {
        return Some(block);
    }
    let previous = record.get("nativeTurnError")?;
    (truthy(previous.get("turnId")) && previous.get("turnId") == record.get("turnId"))
        .then(|| previous.get("error").and_then(blocked))
        .flatten()
}

#[derive(Clone, Debug, Default)]
pub struct RoomViewContext {
    pub agents: Arc<Vec<AgentAuthorityRow>>,
    pub peer_teams: Arc<Vec<PeerTeamRow>>,
    pub active_agent_indices: Arc<HashMap<String, usize>>,
    pub last_message: Option<studio_runtime_schema::room_record::RoomLastMessage>,
    pub federation: Option<Value>,
}

impl RoomViewContext {
    pub(crate) fn new(agents: Vec<AgentAuthorityRow>, peer_teams: Vec<PeerTeamRow>) -> Self {
        let active_agent_indices = agents
            .iter()
            .enumerate()
            .filter(|(_, agent)| !agent.deleted_at.as_ref().is_some_and(Deletion::is_deleted))
            .filter_map(|(index, agent)| agent.id.clone().map(|id| (id, index)))
            .collect();
        Self {
            agents: Arc::new(agents),
            peer_teams: Arc::new(peer_teams),
            active_agent_indices: Arc::new(active_agent_indices),
            last_message: None,
            federation: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ComplaintViewContext<'a> {
    pub agents: &'a [AgentAuthorityRow],
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplaintProjection {
    id: String,
    author: Option<String>,
    lead_id: Option<String>,
    status: studio_runtime_schema::complaint_record::ComplaintStatus,
    created: Option<studio_runtime_schema::complaint_record::ComplaintSeconds>,
    updated: Option<studio_runtime_schema::complaint_record::ComplaintSeconds>,
    read_at: Option<studio_runtime_schema::complaint_record::ComplaintSeconds>,
    title: String,
    recipient: studio_runtime_schema::complaint_record::ComplaintRecipient,
    version: Option<i64>,
    needs_user_response: bool,
    author_name: Option<String>,
    lead_name: Option<String>,
}

pub fn project_complaint_record(
    record: &studio_runtime_schema::complaint_record::ComplaintRecord,
    context: &ComplaintViewContext,
) -> Option<ComplaintProjection> {
    use studio_runtime_schema::complaint_record::{
        ComplaintRecipient, ComplaintSource, ComplaintStatus,
    };
    if record.id.is_empty() {
        return None;
    }
    let author = record.author.as_deref();
    let lead = record.lead_id.as_deref();
    let needs_user_response = record.recipient == ComplaintRecipient::User
        && !matches!(
            record.status,
            ComplaintStatus::Resolved | ComplaintStatus::Declined
        )
        && ((record.source_type == Some(ComplaintSource::UserTask)
            && record.status == ComplaintStatus::Open)
            || !record
                .responses
                .iter()
                .any(|response| response.author == "user"));
    let bound = |value: &str, field| bound_sync_text(value, field);
    let name = |id: Option<&str>, fallback: Option<&str>, field| {
        let value = id.and_then(|id| {
            context
                .agents
                .iter()
                .find(|agent| agent.id.as_deref() == Some(id))
        });
        match value.map(|agent| &agent.name) {
            None | Some(StoredField::Missing) => fallback.map(|name| bound(name, field)),
            Some(StoredField::Null) => None,
            Some(StoredField::Value(name)) => Some(bound(name, field)),
        }
    };
    Some(ComplaintProjection {
        id: bound(&record.id, "id"),
        author: author.map(|value| bound(value, "author")),
        lead_id: lead.map(|value| bound(value, "leadId")),
        status: record.status,
        created: record.created.clone(),
        updated: record.updated.clone(),
        read_at: record.read_at.clone(),
        title: record
            .text
            .as_deref()
            .unwrap_or_default()
            .chars()
            .take(140)
            .collect(),
        recipient: record.recipient,
        version: record.version,
        needs_user_response,
        author_name: name(
            author,
            author.map(|v| if v == "user" { "You" } else { v }),
            "authorName",
        ),
        lead_name: name(lead, lead, "leadName"),
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RetainedWorktreeView<'a> {
    #[serde(borrow)]
    workspace_cleanup_intent:
        Option<studio_runtime_schema::json_object::JsonObject<RetainedIntent<'a>>>,
    remote_worker: Option<serde::de::IgnoredAny>,
}

#[derive(serde::Deserialize)]
struct RetainedIntent<'a> {
    phase: Option<&'a str>,
    #[serde(borrow)]
    effect: Option<studio_runtime_schema::json_object::JsonObject<RetainedEffect<'a>>>,
}

#[derive(serde::Deserialize)]
struct RetainedEffect<'a> {
    kind: Option<&'a str>,
}

/// Public retained-folder entity. Cleanup operation identities never leave the server.
pub fn retained_worktree_entity(member: &Value) -> Option<Value> {
    use serde::Deserialize;
    let view =
        studio_runtime_schema::json_object::JsonObject::<RetainedWorktreeView<'_>>::deserialize(
            member,
        )
        .ok()?
        .0;
    let intent = view.workspace_cleanup_intent?.0;
    if !matches!(intent.phase, Some("not_sent" | "unknown"))
        || !intent
            .effect
            .as_ref()
            .and_then(|effect| effect.0.kind)
            .is_some_and(|kind| {
                crate::workspace_cleanup_runtime::RETAINED_WORKTREE_EFFECTS.contains(&kind)
            })
        || view.remote_worker.is_some()
    {
        return None;
    }
    let projected = project_persisted_record("agent", member)?;
    Some(json!({"id":member["id"], "rootId":member["rootId"],
        "name":projected.get("name"), "cleanup":projected["workspaceCleanupIntent"]}))
}

#[cfg(test)]
mod projection_plan_tests {
    use super::{
        invalid_agent_view, openapi_document, project_persisted_record,
        schema_projection_properties,
    };
    use serde_json::json;

    #[test]
    fn task_entity_payload_hash_is_stable_after_projection_move() {
        #[derive(serde::Serialize)]
        struct Entity<T> {
            collection: &'static str,
            id: &'static str,
            value: T,
        }
        let raw = r#"{"id":"a:i","agent":"a","created":1,"itemId":"i","turnId":"turn","kind":"command","status":"running","type":"commandExecution","name":"commandExecution"}"#;
        let task: studio_runtime_store::TaskRecord = serde_json::from_str(raw).unwrap();
        let payload = serde_json::to_string(&Entity {
            collection: "task",
            id: "a:i",
            value: super::project_task_record(&task).unwrap(),
        })
        .unwrap();
        assert_eq!(
            studio_runtime_store::sha256_hex(payload.as_bytes()),
            "706ea379b3087203fa2bd28bc462b8833b4ed5a61b6b1f36bf890de419b6a642"
        );
    }

    #[test]
    fn destination_eligibility_includes_chat_exclusions() {
        let lead = json!({"id":"lead","rootId":"lead","epoch":1,"isLead":true,
            "provider":"codex","excludedAccountKeys":["blocked"]});
        let context = super::AgentViewContext {
            account_destinations: serde_json::from_str(
                r#"[
                {"accountKey":"blocked","eligibility":{"kind":"available"}},
                {"accountKey":"other","eligibility":{"kind":"available"}}
            ]"#,
            )
            .unwrap(),
            ..Default::default()
        };
        let view = super::agent_entity_view(&lead, &context).unwrap();
        assert_eq!(
            view["accountDestinations"][0]["eligibility"]["kind"],
            "unavailable"
        );
        assert_eq!(
            view["accountDestinations"][1]["eligibility"]["kind"],
            "available"
        );
    }

    #[test]
    fn agent_entity_publishes_account_change_mode() {
        let lead = json!({"id":"lead","rootId":"lead","epoch":1,"isLead":true,"provider":"codex"});
        let mut context = super::AgentViewContext {
            empty_lead: true,
            ..Default::default()
        };
        assert_eq!(
            super::agent_entity_view(&lead, &context).unwrap()["accountChange"],
            json!({"kind":"select"})
        );
        context.empty_lead = false;
        assert_eq!(
            super::agent_entity_view(&lead, &context).unwrap()["accountChange"],
            json!({"kind":"transfer"})
        );
        let worker =
            json!({"id":"worker","rootId":"lead","epoch":1,"isLead":false,"provider":"codex"});
        assert_eq!(
            super::agent_entity_view(&worker, &context).unwrap()["accountChange"]["kind"],
            "unavailable"
        );
    }

    #[test]
    fn complaint_projection_preserves_public_fields() {
        let context = super::ComplaintViewContext::default();
        for recipient in ["user", "lead"] {
            let raw = format!(
                r#"{{"id":"c","leadId":"lead","author":"user","text":"hello","recipient":"{recipient}","version":1,"status":"open","created":1.0,"updated":1.0,"readAt":null,"responses":[]}}"#
            );
            let record: studio_runtime_schema::complaint_record::ComplaintRecord =
                serde_json::from_str(&raw).unwrap();
            let encoded =
                serde_json::to_string(&super::project_complaint_record(&record, &context).unwrap())
                    .unwrap();
            let actual: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            assert_eq!(
                actual,
                json!({"id":"c","leadId":"lead","leadName":"lead","author":"user","authorName":"You","title":"hello","recipient":recipient,"version":1,"status":"open","created":1.0,"updated":1.0,"readAt":null,"needsUserResponse":recipient == "user"})
            );
        }
    }

    #[test]
    fn account_change_rejects_non_boolean_lead_identity() {
        let malformed = json!({"compactions":0,"id":"lead","rootId":"lead","epoch":1,
            "status":"idle","accountKey":"default","isLead":"yes","provider":"codex"});
        let context = super::AgentViewContext {
            empty_lead: true,
            ..Default::default()
        };
        assert!(super::agent_entity_view(&malformed, &context).is_none());
    }

    #[test]
    fn agent_entity_publishes_transfer_actions_from_progress() {
        let lead = json!({"id":"lead","rootId":"lead","epoch":1,"isLead":true,"provider":"codex",
            "accountTransfer":{"id":"move","targetAccountKey":"target","status":"pending",
                "blocked":[{"id":"lead","reason":"History unavailable"}],"canRetry":true,"canCancel":true,"movingNow":0}});
        let context = super::AgentViewContext {
            transfer_can_cancel: true,
            ..Default::default()
        };
        let view = super::agent_entity_view(&lead, &context).unwrap();
        assert_eq!(view["accountTransferProgress"]["kind"], "blocked");
        assert_eq!(
            view["accountTransferProgress"]["actions"],
            json!("retry_cancel")
        );
        for collection in ["agentSummary", "agentDetail"] {
            let projected = project_persisted_record(collection, &view).unwrap();
            assert_eq!(projected["accountTransferProgress"]["kind"], "blocked");
            assert_eq!(projected["accountChange"]["kind"], "unavailable");
        }
    }

    #[test]
    fn opaque_archive_projection_bounds_nested_text_and_arrays() {
        let raw = json!({"id":"w","status":"ready","archive":{"nested":{"long":"x".repeat(12_001),"items":vec![1; 201]}}});
        let record: studio_runtime_store::WorkItemRecord =
            serde::Deserialize::deserialize(&raw).unwrap();
        let view = super::project_work_record(&record).unwrap();
        let encoded = serde_json::to_string(&view).unwrap();
        let view: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            view["archive"]["nested"]["long"].as_str().unwrap().len(),
            12_000
        );
        assert_eq!(
            view["archive"]["nested"]["items"].as_array().unwrap().len(),
            200
        );
    }

    #[test]
    fn work_projection_preserves_presence_and_opaque_archive() {
        for raw in [
            r#"{"id":"w","status":"ready"}"#,
            r#"{"id":"w","status":"ready","agent":null,"rootId":"r","title":"work","archive":{"reason":"kept"}}"#,
        ] {
            let record: studio_runtime_store::WorkItemRecord = serde_json::from_str(raw).unwrap();
            let expected: serde_json::Value = serde_json::from_str(raw).unwrap();
            let encoded =
                serde_json::to_string(&super::project_work_record(&record).unwrap()).unwrap();
            let new: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            assert_eq!(new, expected);
        }
    }

    #[test]
    fn task_projection_has_expected_public_fields() {
        for length in [12, 2_100] {
            let raw = json!({"id":"a:item", "agent":"a", "created":1.0, "itemId":"item", "kind":"command", "status":"running", "turnId":"turn", "type":"commandExecution", "name":"run", "tail":"雪".repeat(length), "outputTruncated":false, "command":"x".repeat(length), "sourceThreadId":"private", "interactive":true});
            let task: studio_runtime_store::TaskRecord =
                serde::Deserialize::deserialize(&raw).unwrap();
            let encoded =
                serde_json::to_string(&super::project_task_record(&task).unwrap()).unwrap();
            let mut new: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            new.as_object_mut().unwrap().remove("display");
            let limit = length.min(2_000);
            assert_eq!(
                new,
                json!({"id":"a:item","agent":"a","created":1.0,"kind":"command","status":"running","turnId":"turn","name":"run","tail":"雪".repeat(limit),"outputTruncated":length > 2_000,"command":"x".repeat(limit),"interactive":true})
            );
        }
    }

    #[test]
    fn retained_cleanup_admission_preserves_partial_legacy_fields() {
        for (raw, expected) in [
            (
                r#"{"id":"a","workspaceCleanupIntent":{"phase":"unknown","effect":{"kind":"remove_worktree"}}}"#,
                true,
            ),
            (
                r#"{"id":"a","remoteWorker":null,"workspaceCleanupIntent":{"phase":"not_sent","effect":{"kind":"save_worktree_ref"}}}"#,
                true,
            ),
            (
                r#"{"id":"a","remoteWorker":false,"workspaceCleanupIntent":{"phase":"unknown","effect":{"kind":"remove_worktree"}}}"#,
                false,
            ),
            (
                r#"{"id":"a","remoteWorker":0,"workspaceCleanupIntent":{"phase":"unknown","effect":{"kind":"remove_worktree"}}}"#,
                false,
            ),
            (
                r#"{"id":"a","workspaceCleanupIntent":{"phase":null,"effect":{"kind":"remove_worktree"}}}"#,
                false,
            ),
            (
                r#"{"id":"a","workspaceCleanupIntent":{"phase":"unknown","effect":[]}}"#,
                false,
            ),
            (
                r#"{"id":"a","workspaceCleanupIntent":{"phase":"confirmed","phase":"unknown","effect":{"kind":"remove_worktree"}}}"#,
                true,
            ),
            (
                r#"{"id":"","workspaceCleanupIntent":{"phase":"unknown","effect":{"kind":"remove_worktree"}}}"#,
                false,
            ),
        ] {
            let value: serde_json::Value = serde_json::from_str(raw).expect("fixture JSON");
            let result = super::retained_worktree_entity(&value);
            assert_eq!(result.is_some(), expected, "{raw}");
            if let Some(view) = result {
                assert_eq!(view["rootId"], value["rootId"]);
            }
        }
    }

    #[test]
    fn retained_cleanup_prefers_present_nested_values_before_type_checks() {
        let member = json!({"id":"worker","rootId":"root","cwd":"/fallback",
            "workspaceCleanupIntent":{"phase":"unknown","path":"/intent", "reason":"fallback",
                "effect":{"kind":"remove_worktree","identity":{"cwd":null,"private":"secret"}},
                "receipt":{"reason":false,"private":"secret"},"requestId":"private"}});
        let view = super::retained_worktree_entity(&member).expect("retained");
        assert_eq!(view["cleanup"]["path"], serde_json::Value::Null);
        assert_eq!(view["cleanup"]["reason"], serde_json::Value::Null);
        assert!(!view.to_string().contains("secret"));
        assert!(!view.to_string().contains("private"));
    }

    #[test]
    fn complaint_projection_preserves_integer_seconds() {
        let raw = r#"{"id":"c","author":null,"leadId":null,"text":null,"recipient":"lead","status":"open","created":1,"updated":2,"readAt":null,"version":null,"responses":[]}"#;
        let record = serde_json::from_str(raw).unwrap();
        let view =
            super::project_complaint_record(&record, &super::ComplaintViewContext::default())
                .unwrap();
        let encoded = serde_json::to_string(&view).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(actual["created"], json!(1));
        assert_eq!(actual["updated"], json!(2));
        let stored = serde_json::to_string(&record).unwrap();
        assert_eq!(
            studio_runtime_store::sha256_hex(stored.as_bytes()),
            "413cdeb2d2cd1fb25994cbbac3469ccb6fb478a625197a6ebd205b453f97e49a"
        );
    }

    #[test]
    fn complaint_names_preserve_missing_and_null() {
        let raw = r#"{"id":"complaint","author":"author","leadId":null,"text":"Question","recipient":"lead","status":"open","created":null,"updated":null,"readAt":null,"version":null,"responses":[]}"#;
        let record: studio_runtime_schema::complaint_record::ComplaintRecord =
            serde_json::from_str(raw).unwrap();
        for (agent, expected) in [
            (r#"{"id":"author"}"#, json!("author")),
            (r#"{"id":"author","name":null}"#, json!(null)),
            (r#"{"id":"author","name":"Author"}"#, json!("Author")),
        ] {
            let agents = [serde_json::from_str(agent).expect("typed agent")];
            let view = super::project_complaint_record(
                &record,
                &super::ComplaintViewContext { agents: &agents },
            )
            .expect("complaint view");
            let encoded = serde_json::to_string(&view).unwrap();
            let actual: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            assert_eq!(actual["authorName"], expected);
        }
    }

    /// The agent view publishes no team budget or token count, also when an
    /// older build stored them. The older view never published an empty pattern list, and
    /// the client reads a missing one as its default. Set patterns are
    /// published.
    #[test]
    fn sync_views_publish_no_budget_and_only_set_output_patterns() {
        for budget in [json!(null), json!(100)] {
            let view = project_persisted_record(
                "agent",
                &json!({"provider":studio_runtime_schema::ProviderKind::Codex,"id":"lead","rootId":"lead","isLead":true,"tokenBudget":budget,
                        "budgetBlocked":"usage accounting incomplete","tokensUsed":120,
                        "tokenUsageAccounting":"total",
                        "completedWorkerTurns":2,"terminalReason":"worker_max_turns"}),
            )
            .expect("agent view");
            for field in [
                "tokenBudget",
                "budgetBlocked",
                "tokensUsed",
                "tokenUsageAccounting",
                "completedWorkerTurns",
                "terminalReason",
            ] {
                assert!(view.get(field).is_none(), "{field}: {view}");
            }
        }

        let monitor = |patterns| {
            let raw = crate::monitor_policy::complete_test_monitor(
                json!({"id":"monitor","agent":"lead","status":"running","outputPatterns":patterns}),
            );
            let record = serde::Deserialize::deserialize(&raw).unwrap();
            let view = crate::sync_policy::project_monitor_record(&record).unwrap();
            serde::Serialize::serialize(&view, serde_json::value::Serializer).unwrap()
        };
        let unset = monitor(json!([]));
        assert!(unset.get("outputPatterns").is_none(), "{unset}");
        assert_eq!(
            monitor(json!(["ready"]))["outputPatterns"],
            json!(["ready"])
        );
    }

    #[test]
    fn invalid_agent_view_preserves_valid_root_and_lead_identity() {
        let record = json!({"provider":studio_runtime_schema::ProviderKind::Codex,"id":"child","rootId":"lead","epoch":4,"isLead":true});
        let view = invalid_agent_view("child", &record, "invalid field /prompt");
        assert_eq!(view["rootId"], "lead");
        assert_eq!(view["isLead"], true);
        assert_eq!(view["status"], "failed");

        let malformed_root = json!({"provider":studio_runtime_schema::ProviderKind::Codex,"id":"child","rootId":17,"epoch":4,"isLead":false});
        let fallback = invalid_agent_view("child", &malformed_root, "invalid field /rootId");
        assert_eq!(fallback["rootId"], "child");
        assert_eq!(fallback["isLead"], false);
    }

    #[test]
    fn schema_projection_plan_reuses_openapi_property_nodes() {
        let document = openapi_document();
        let schema = document
            .pointer("/components/schemas/AgentEntityDto")
            .expect("agent entity schema");
        let property = schema
            .pointer("/properties/id")
            .expect("agent id property schema");
        let plan = schema_projection_properties();
        let properties = plan
            .get(&(schema as *const _ as usize))
            .expect("prepared agent property projection");

        assert!(std::ptr::eq(
            *properties.get("id").expect("id field"),
            property
        ));
        assert!(std::ptr::eq(plan, schema_projection_properties()));
    }
}
