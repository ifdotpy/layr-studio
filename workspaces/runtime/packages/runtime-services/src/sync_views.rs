//! Canonical renderer entity views built from records and explicit context.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};
use studio_json::{Map, Value, json};
use studio_runtime_schema::provider_errors::is_misalignment_policy_violation;
use studio_runtime_store::entity_publication::WorkHistoryKind;
use studio_runtime_store::{StoreError, StoreResult};

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
    raw: &studio_json::OwnedRawJson,
    field: &str,
    depth: usize,
) -> Result<studio_json::OwnedRawJson, studio_json::Error> {
    let mut value: Value = studio_json::from_str(raw.as_str())?;
    bound_opaque_json(&mut value, field, depth);
    studio_json::to_raw_value(&value)
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
        studio_json::from_str(include_str!("../../runtime-http/openapi.json"))
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

fn project_agent_legacy_field<T: serde::Serialize>(
    value: &T,
    name: &str,
) -> Result<studio_runtime_schema::raw_json::RawJson, studio_json::Error> {
    // public shape is a schema-filtered subset of the stored owner; filtering stays with project_schema_value
    let value = serde::Serialize::serialize(value, studio_json::value::Serializer)?;
    Ok(project_agent_property(&value, name))
}

/// `SCHEMA_FILTERED_AGENT_FIELDS` is the Rust-only generator's field list.
fn project_agent_property(value: &Value, name: &str) -> studio_runtime_schema::raw_json::RawJson {
    let document = openapi_document();
    let schema = document
        .pointer("/components/schemas/AgentEntityDto")
        .expect("agent schema exists");
    let property = schema_projection_properties()[&(schema as *const Value as usize)][name];
    studio_runtime_schema::raw_json::RawJson(
        studio_json::to_raw_value(&project_schema_value(
            value,
            property,
            name,
            document,
            1,
            true,
            AGENT_SHORT_STRING_OBJECTS.contains(&name),
        ))
        .expect("a projected JSON value serializes"),
    )
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

/// Work detail document: source metadata with empty history lists, the
/// submission refusal, and the server policy. `metadata` is validated before
/// the policy is attached. Serializes as one flat object, `policy` last.
#[derive(serde::Serialize)]
pub struct WorkDetailView {
    #[serde(flatten)]
    pub metadata: Value,
    pub policy: studio_runtime_schema::task_gate::WorkDetailPolicy,
}

/// One work collection row. Serializes as the bare document it wraps.
#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum WorkView {
    Summary(WorkProjection),
    Detail(WorkDetailView),
    Entry(WorkHistoryEntry),
}

pub fn work_detail_view(
    item: &studio_runtime_store::WorkItemRecord,
    record: &Value,
    refusal: Value,
    statuses: &BTreeMap<String, studio_runtime_store::WorkStatus>,
) -> StoreResult<WorkDetailView> {
    let mut metadata = Value::Object(
        record
            .as_object()
            .ok_or(StoreError::RecordContract(
                "work detail source is not an object",
            ))?
            .iter()
            .filter(|(field, _)| {
                !matches!(
                    field.as_str(),
                    "results" | "decisions" | "releases" | "policy"
                ) && !crate::work_policy::PRIVATE_WORK_FIELDS.contains(&field.as_str())
            })
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect(),
    );
    for field in ["results", "decisions", "releases"] {
        metadata[field] = json!([]);
    }
    metadata["submissionRefusal"] = refusal;
    Ok(WorkDetailView {
        metadata,
        policy: crate::work_policy::detail_policy(item, statuses),
    })
}

/// Generated renderer envelopes for work history rows.
#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum WorkHistoryEntry {
    Result(studio_runtime_schema::generated::WorkResultEntityDto),
    Decision(studio_runtime_schema::generated::WorkDecisionEntityDto),
    Release(studio_runtime_schema::generated::WorkReleaseEntityDto),
}

impl WorkHistoryEntry {
    pub fn id(&self) -> &str {
        match self {
            Self::Result(entry) => &entry.id,
            Self::Decision(entry) => &entry.id,
            Self::Release(entry) => &entry.id,
        }
    }
}

/// One history entry, keyed `{task}:{index}`. None when the index is absent.
pub fn work_history_view(
    id: &str,
    record: &Value,
    kind: WorkHistoryKind,
    index: usize,
) -> Option<WorkHistoryEntry> {
    let value = record[kind.field()].as_array()?.get(index)?;
    Some(work_history_entry(
        kind,
        id,
        studio_json::to_raw_value(&record["rootId"]).ok()?,
        index,
        studio_json::to_raw_value(value).ok()?,
    ))
}

/// The published entry of one history row, with its stored value forwarded intact.
pub fn work_history_entry(
    kind: WorkHistoryKind,
    id: &str,
    root_id: studio_json::OwnedRawJson,
    index: usize,
    value: studio_json::OwnedRawJson,
) -> WorkHistoryEntry {
    let row_id = format!("{id}:{index}");
    let task_id = id.to_owned();
    let index = index as u64;
    match kind {
        WorkHistoryKind::Result => {
            WorkHistoryEntry::Result(studio_runtime_schema::generated::WorkResultEntityDto {
                id: row_id,
                index,
                root_id,
                task_id,
                value,
            })
        }
        WorkHistoryKind::Decision => {
            WorkHistoryEntry::Decision(studio_runtime_schema::generated::WorkDecisionEntityDto {
                id: row_id,
                index,
                root_id,
                task_id,
                value,
            })
        }
        WorkHistoryKind::Release => {
            WorkHistoryEntry::Release(studio_runtime_schema::generated::WorkReleaseEntityDto {
                id: row_id,
                index,
                root_id,
                task_id,
                value,
            })
        }
    }
}

/// Every work collection row for one record: summary, detail, then each
/// result, decision and release entry. Publication writes these rows, except
/// that metadata-only writes skip history and the per-index diff writes only
/// the changed entries.
pub fn work_views(
    item: &studio_runtime_store::WorkItemRecord,
    record: &Value,
    refusal: Value,
    statuses: &BTreeMap<String, studio_runtime_store::WorkStatus>,
) -> StoreResult<Vec<(&'static str, WorkView)>> {
    let id: &str = item.id().as_ref();
    let mut rows = Vec::new();
    if let Some(summary) = project_work_record(item) {
        rows.push(("work", WorkView::Summary(summary)));
    }
    rows.push((
        "workDetail",
        WorkView::Detail(work_detail_view(item, record, refusal, statuses)?),
    ));
    for kind in WorkHistoryKind::ALL {
        let length = record[kind.field()].as_array().map_or(0, Vec::len);
        for index in 0..length {
            if let Some(entry) = work_history_view(id, record, kind, index) {
                rows.push((kind.collection(), WorkView::Entry(entry)));
            }
        }
    }
    Ok(rows)
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
    if matches!(collection, "agent" | "agentSummary") {
        projected["projectPath"] = crate::project_path::project_path(record)
            .map(|path| Value::String(path.as_str().to_owned()))
            .unwrap_or(Value::Null);
    }
    Some(projected)
}

#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct AgentViewContext {
    pub account_destinations: Vec<studio_runtime_schema::account_destination::AccountDestination>,
    pub transfer: TransferObservation,
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

#[derive(Clone, Debug, Default)]
pub enum TransferObservation {
    #[default]
    Absent,
    Cancellable,
    Held,
    Unreadable {
        reason: String,
    },
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
    if let Some(provider) = record.get("provider")
        && let Ok(provider) =
            <studio_runtime_schema::ProviderKind as serde::Deserialize>::deserialize(provider)
    {
        view["provider"] = serde::Serialize::serialize(&provider, studio_json::value::Serializer)
            .expect("provider serializes");
    }
    for field in ["name", "cwd"] {
        if let Some(value) = record.get(field).filter(|value| value.is_string()) {
            view[field] = value.clone();
        }
    }
    view
}

/// Canonical wait projection used by both sync/HTTP and lead status.
pub(crate) fn agent_start_wait(
    record: &Value,
) -> Result<Option<studio_runtime_schema::agent_progress::AgentStartWait>, studio_json::Error> {
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
    Ok(project_agent_start_wait(
        source.status.as_ref(),
        source.epoch,
        source.auto_wake,
        source.native_failure_hold.is_some_and(NativeHold::active),
        source.account_transfer_id.is_some(),
        source.start_wait,
    ))
}

fn project_agent_start_wait(
    status: Option<&studio_runtime_store::AgentStatus>,
    epoch: i64,
    auto_wake: bool,
    native_failure_hold: bool,
    account_transfer: bool,
    start_wait: Option<studio_runtime_schema::agent_progress::AgentStartWait>,
) -> Option<studio_runtime_schema::agent_progress::AgentStartWait> {
    use studio_runtime_schema::agent_progress::AgentStartWait;
    if !matches!(
        status,
        Some(
            studio_runtime_store::AgentStatus::Queued | studio_runtime_store::AgentStatus::Starting
        )
    ) {
        return None;
    }
    if status == Some(&studio_runtime_store::AgentStatus::Queued) {
        let hold = if !auto_wake {
            Some(AgentStartWait::Paused { epoch })
        } else if native_failure_hold {
            Some(AgentStartWait::NativeFailure { epoch })
        } else if account_transfer {
            Some(AgentStartWait::AccountTransfer { epoch })
        } else {
            None
        };
        if hold.is_some() {
            return hold;
        }
    }
    let persisted = start_wait.filter(|wait| wait.epoch() == epoch);
    if status == Some(&studio_runtime_store::AgentStatus::Starting) {
        return persisted;
    }
    Some(persisted.unwrap_or(AgentStartWait::DispatchPending { epoch }))
}

pub(crate) fn agent_start_wait_record(
    record: &studio_runtime_store::AgentRecord,
) -> Result<Option<studio_runtime_schema::agent_progress::AgentStartWait>, ()> {
    use studio_runtime_store::NativeFailureHold;
    let native_failure_hold = record
        .native_failure_hold
        .value()
        .map(NativeFailureHold::projected_truthiness)
        .transpose()
        .map_err(|_| ())?
        .unwrap_or(false);
    Ok(project_agent_start_wait(
        Some(&record.status),
        record.epoch,
        record.auto_wake.unwrap_or(false),
        native_failure_hold,
        record.account_transfer_id.value().is_some(),
        record.start_wait.clone(),
    ))
}

// Generated DTOs use nested options to distinguish absent fields from explicit null.
#[allow(clippy::option_option)]
fn nullable_field<T>(field: studio_runtime_schema::queue_command::Field<T>) -> Option<Option<T>> {
    use studio_runtime_schema::queue_command::Field;
    match field {
        Field::Missing => None,
        Field::Null => Some(None),
        Field::Value(value) => Some(Some(value)),
    }
}

#[allow(clippy::option_option)]
fn bounded_field(
    field: &studio_runtime_schema::queue_command::Field<String>,
    name: &str,
) -> Option<Option<String>> {
    nullable_field(field.clone().map(|value| bound_sync_text(&value, name)))
}

#[allow(clippy::option_option)]
fn filtered_field<T: serde::Serialize>(
    field: &studio_runtime_schema::queue_command::Field<T>,
    name: &str,
) -> Result<Option<Option<studio_runtime_schema::raw_json::RawJson>>, studio_json::Error> {
    use studio_runtime_schema::queue_command::Field;
    match field {
        Field::Missing => Ok(None),
        Field::Null => Ok(Some(None)),
        Field::Value(value) => Ok(Some(Some(project_agent_legacy_field(value, name)?))),
    }
}

#[allow(clippy::option_option)]
fn filtered_option<T: serde::Serialize>(
    value: Option<&T>,
    name: &str,
) -> Result<Option<Option<studio_runtime_schema::raw_json::RawJson>>, studio_json::Error> {
    value
        .map(|value| project_agent_legacy_field(value, name).map(|value| Some(Some(value))))
        .unwrap_or(Ok(None))
}

fn permission_mode(
    mode: studio_runtime_schema::claude_permissions::PermissionMode,
) -> studio_runtime_schema::generated::AgentPermissionMode {
    use studio_runtime_schema::{
        claude_permissions::PermissionMode as Input, generated::AgentPermissionMode as Output,
    };
    match mode {
        Input::Default => Output::Default,
        Input::AcceptEdits => Output::AcceptEdits,
        Input::Auto => Output::Auto,
        Input::Plan => Output::Plan,
        Input::BypassPermissions => Output::BypassPermissions,
    }
}

fn agent_role(
    role: studio_runtime_schema::creation_intent::AgentRole,
) -> studio_runtime_schema::generated::AgentRole {
    use studio_runtime_schema::{
        creation_intent::AgentRole as Input, generated::AgentRole as Output,
    };
    match role {
        Input::Orchestrator => Output::Orchestrator,
        Input::Implementer => Output::Implementer,
        Input::Reviewer => Output::Reviewer,
    }
}

fn bound_review_defaults(
    mut value: studio_runtime_schema::agent_entity_remote::ReviewDefaultsDto,
) -> studio_runtime_schema::agent_entity_remote::ReviewDefaultsDto {
    value.model = value.model.map(|s| bound_sync_text(&s, "model"));
    value.effort = value.effort.map(|s| bound_sync_text(&s, "effort"));
    value
}

fn bound_stall(
    stall: Option<studio_runtime_schema::agent_progress::AgentStall>,
) -> Option<studio_runtime_schema::agent_progress::AgentStall> {
    stall.map(|mut stall| {
        stall.task_id = bound_sync_text(&stall.task_id, "taskId");
        stall
    })
}

pub(crate) fn agent_entity_from_record(
    record: studio_runtime_store::AgentRecord,
    context: &AgentViewContext,
) -> Option<studio_runtime_schema::generated::AgentEntityDto> {
    use studio_runtime_schema::agent_progress::{AccountChange, AccountTransferProgress};
    use studio_runtime_schema::generated::{AgentEntityDto, AgentKind, AgentSource, AgentWorktree};
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_store::transfer_record::AgentTransferState;
    if record.id.0.is_empty() {
        return None;
    }
    #[cfg(test)]
    crate::scheduler::publication_work::projection(&record.id.0);
    // Recovery eligibility still reads the record as Value; typed view deferred.
    let legacy_record = record.value();
    let native_block = native_thread_block(&legacy_record);
    let manual_check = crate::recovery_policy::RecoverySnapshot::from_record(legacy_record)
        .map(|snapshot| snapshot.manual_connection_check())
        .unwrap_or_default();
    let start_wait = agent_start_wait_record(&record).ok()?;
    let can_send =
        native_block.is_none() && !record.moved_to_active() && !record.move_import_pending_active();
    let mut extra = record.extra;
    let is_lead = record.is_lead == Some(true);
    let server_id = extra
        .get("remoteWorker")
        .and_then(|v| v.pointer("/server"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or(context.project_server_id.as_deref())
        .map(|s| bound_sync_text(s, "serverId"));
    let native_thread_block =
        native_block.map(|v| Some(project_agent_property(&v, "nativeThreadBlock")));
    let mut transfer_progress = match &context.transfer {
        TransferObservation::Unreadable { reason } => AccountTransferProgress::Unreadable {
            reason: reason.clone(),
        },
        observation @ (TransferObservation::Absent
        | TransferObservation::Cancellable
        | TransferObservation::Held) => crate::account_transfer::progress_view(
            record
                .account_transfer
                .value()
                .and_then(|value| match value {
                    studio_runtime_store::StoredTransferObject::Known(value) => Some(value),
                    studio_runtime_store::StoredTransferObject::Unreadable(_) => None,
                }),
            matches!(observation, TransferObservation::Cancellable),
        ),
    };
    match &mut transfer_progress {
        AccountTransferProgress::Unreadable { reason }
        | AccountTransferProgress::Waiting { reason, .. }
        | AccountTransferProgress::Blocked { reason, .. } => {
            *reason = bound_sync_text(reason, "reason")
        }
        AccountTransferProgress::None
        | AccountTransferProgress::Moving { .. }
        | AccountTransferProgress::Completed { .. }
        | AccountTransferProgress::Cancelled { .. } => {}
    }
    let account_transfer = match &record.account_transfer {
        Field::Missing => None,
        Field::Null => Some(None),
        Field::Value(studio_runtime_store::StoredTransferObject::Known(
            AgentTransferState::Summary(summary),
        )) => {
            let dto: studio_runtime_schema::agent_entity_transfer::AccountTransferSummaryDto =
                (**summary).clone().into();
            Some(Some(
                project_agent_legacy_field(&dto, "accountTransfer").ok()?,
            ))
        }
        Field::Value(value) => Some(Some(
            project_agent_legacy_field(value, "accountTransfer").ok()?,
        )),
    };
    let account_destinations = if is_lead {
        let destinations = &context.account_destinations;
        Some(project_agent_legacy_field(&destinations, "accountDestinations").ok()?)
    } else {
        extra
            .remove("accountDestinations")
            .map(|v| project_agent_property(&v, "accountDestinations"))
    };
    let account_change = if !is_lead {
        AccountChange::Unavailable {
            reason: "Choose the lead chat to change its account".into(),
        }
    } else if matches!(
        transfer_progress,
        AccountTransferProgress::Waiting { .. }
            | AccountTransferProgress::Moving { .. }
            | AccountTransferProgress::Blocked { .. }
            | AccountTransferProgress::Unreadable { .. }
    ) {
        AccountChange::Unavailable {
            reason: "Finish or cancel the current transfer first".into(),
        }
    } else if context.empty_lead
        && record.thread_id.as_deref().is_none_or(str::is_empty)
        && record.in_flight != Some(true)
    {
        AccountChange::Select
    } else {
        AccountChange::Transfer
    };
    let command_progress = context
        .command_progress
        .iter()
        .take(SYNC_ARRAY_ITEMS)
        .cloned()
        .map(|mut progress| {
            progress.monitor_id = bound_sync_text(&progress.monitor_id, "monitorId");
            progress.stage = bound_sync_text(&progress.stage, "stage");
            progress.last_output_line =
                bound_sync_text(&progress.last_output_line, "lastOutputLine");
            progress
        })
        .collect();
    let team_counts = if is_lead {
        context
            .team_counts
            .as_ref()
            .map(|v| project_agent_legacy_field(v, "teamCounts"))
            .transpose()
            .ok()?
            .or_else(|| {
                extra
                    .remove("teamCounts")
                    .map(|v| project_agent_property(&v, "teamCounts"))
            })
    } else {
        extra
            .remove("teamCounts")
            .map(|v| project_agent_property(&v, "teamCounts"))
    };
    let empty = if is_lead {
        Some(Some(project_agent_property(
            &Value::Bool(context.empty_lead),
            "empty",
        )))
    } else {
        extra
            .remove("empty")
            .map(|v| Some(project_agent_property(&v, "empty")))
    };
    let overview = if is_lead {
        None
    } else {
        let task = record.prompt.value().map_or("", String::as_str);
        let result = if record
            .last_completed_turn
            .value()
            .is_some_and(|s| !s.is_empty())
            && record.turn_id.as_deref().is_none_or(str::is_empty)
            && !record.in_flight.unwrap_or(false)
            && record.status == studio_runtime_store::AgentStatus::Completed
        {
            record.last_answer.as_deref().unwrap_or_default()
        } else {
            ""
        };
        Some(Some(
            studio_runtime_schema::agent_entity_nested::AgentOverview {
                task: Field::Value(task.chars().take(4000).collect()),
                task_truncated: Field::Value(task.chars().count() > 4000),
                result: Field::Value(result.chars().take(4000).collect()),
                result_truncated: Field::Value(result.chars().count() > 4000),
                result_turn_id: if result.is_empty() {
                    Field::Null
                } else {
                    match &record.last_completed_turn {
                        Field::Value(value) => Field::Value(bound_sync_text(value, "resultTurnId")),
                        Field::Null => Field::Null,
                        Field::Missing => Field::Missing,
                    }
                },
                result_file: context
                    .latest_work_result_file
                    .as_ref()
                    .map_or(Field::Null, |s| {
                        Field::Value(bound_sync_text(s, "resultFile"))
                    }),
            },
        ))
    };
    let workspace_cleanup_intent = match extra.remove("workspaceCleanupIntent") {
        Some(value) if value.is_object() => {
            let dto = cleanup_summary(&value, record.cwd.as_deref());
            Some(Some(
                project_agent_legacy_field(&dto, "workspaceCleanupIntent").ok()?,
            ))
        }
        Some(value) => Some(Some(project_agent_property(
            &value,
            "workspaceCleanupIntent",
        ))),
        None => None,
    };
    Some(AgentEntityDto {
        account_change: Some(account_change),
        account_destinations,
        account_key: Some(Some(bound_sync_text(&record.account_key, "accountKey"))),
        account_transfer,
        account_transfer_id: nullable_field(
            record
                .account_transfer_id
                .clone()
                .map(|v| bound_sync_text(v.as_str(), "accountTransferId")),
        ),
        account_transfer_progress: Some(transfer_progress),
        activity: filtered_option(record.activity.as_ref(), "activity").ok()?,
        agent_mode: record.agent_mode.clone().map(Some),
        agent_mode_revision: record.agent_mode_revision.map(|n| Some(n as i64)),
        agent_mode_supported: record.agent_mode_supported.map(Some),
        archived: nullable_field(record.archived.clone()),
        auto_wake: record.auto_wake.map(Some),
        can_send: Some(Some(can_send)),
        capacity: Some(record.capacity.clone()),
        capacity_retry: extra
            .remove("capacityRetry")
            .map(|v| Some(project_agent_property(&v, "capacityRetry"))),
        command_progress: Some(command_progress),
        compactions: Some(record.compactions.map(|n| n as i64)),
        concurrency: record.concurrency.map(|n| Some(n.get())),
        connection_check: filtered_field(&record.connection_check, "connectionCheck").ok()?,
        context_repair_wait: filtered_field(&record.context_repair_wait, "contextRepairWait")
            .ok()?,
        context_usage: filtered_option(record.context_usage.as_ref(), "contextUsage").ok()?,
        created: nullable_field(record.created.clone()),
        cwd: record
            .cwd
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "cwd"))),
        cyber_access_program: filtered_field(&record.cyber_access_program, "cyberAccessProgram")
            .ok()?,
        daybreak_enabled: record.daybreak_enabled.map(Some),
        deleted_at: record.deleted_at.map(Some),
        effort: bounded_field(&record.effort, "effort"),
        empty,
        environment: filtered_field(&record.environment, "environment").ok()?,
        epoch: Some(Some(record.epoch)),
        error: filtered_field(&record.error, "error").ok()?,
        worker_excluded_account_keys: nullable_field(
            record.worker_excluded_account_keys.clone().map(|items| {
                items
                    .into_iter()
                    .take(SYNC_ARRAY_ITEMS)
                    .map(|text| bound_sync_text(text.as_str(), ""))
                    .collect()
            }),
        ),
        reviewer_excluded_account_keys: nullable_field(
            record.reviewer_excluded_account_keys.clone().map(|items| {
                items
                    .into_iter()
                    .take(SYNC_ARRAY_ITEMS)
                    .map(|text| bound_sync_text(text.as_str(), ""))
                    .collect()
            }),
        ),
        execution_mode: filtered_option(record.execution_mode.as_ref(), "executionMode").ok()?,
        fast_mode: record.fast_mode.map(Some),
        has_approval: context.has_approval.then_some(Some(true)),
        has_background_work: (!context.command_progress.is_empty()).then_some(true),
        has_question: context.has_question.then_some(Some(true)),
        has_unread: extra
            .remove("hasUnread")
            .map(|v| Some(project_agent_property(&v, "hasUnread"))),
        host_project_path: bounded_field(&record.host_project_path, "hostProjectPath"),
        id: bound_sync_text(&record.id.0, "id"),
        in_flight: record.in_flight.map(Some),
        is_lead: record.is_lead.map(Some),
        kind: Some(Some(AgentKind::Agent)),
        last_answer: record
            .last_answer
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "lastAnswer"))),
        last_completed_turn: bounded_field(&record.last_completed_turn, "lastCompletedTurn"),
        last_completed_turn_status: bounded_field(
            &record.last_completed_turn_status,
            "lastCompletedTurnStatus",
        ),
        last_event: record
            .last_event
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "lastEvent"))),
        last_read_at: extra
            .remove("lastReadAt")
            .map(|v| Some(project_agent_property(&v, "lastReadAt"))),
        launcher_alive: Some(Some(context.launcher_alive)),
        layr_error: extra
            .remove("layrError")
            .map(|v| Some(project_agent_property(&v, "layrError"))),
        layr_line: match record.layr_line {
            Field::Missing => None,
            Field::Null => Some(Some(project_agent_property(&Value::Null, "layrLine"))),
            Field::Value(value) => Some(Some(project_agent_property(
                &Value::String(value),
                "layrLine",
            ))),
        },
        layr_project_id: record
            .layr_project_id
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "layrProjectId"))),
        legacy_vm_workspace: None,
        layr_ready: record.layr_ready.map(Some),
        layr_state_id: filtered_field(&record.layr_state_id, "layrStateId").ok()?,
        manual_connection_check: Some(
            project_agent_legacy_field(&manual_check, "manualConnectionCheck").ok()?,
        ),
        model: record
            .model
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "model"))),
        move_import_pending: match &record.move_import_pending {
            Field::Missing => None,
            Field::Null => Some(project_agent_property(&Value::Null, "moveImportPending")),
            Field::Value(value) => {
                Some(project_agent_legacy_field(value, "moveImportPending").ok()?)
            }
        },
        moved_from: filtered_field(&record.moved_from, "movedFrom").ok()?,
        moved_to: filtered_field(&record.moved_to, "movedTo").ok()?,
        name: record
            .name
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "name"))),
        naming: Some(record.naming),
        native_error: extra
            .remove("nativeError")
            .map(|v| Some(project_agent_property(&v, "nativeError"))),
        native_limit_error_at: extra
            .remove("nativeLimitErrorAt")
            .map(|v| Some(project_agent_property(&v, "nativeLimitErrorAt"))),
        native_release: filtered_option(record.native_release.as_ref(), "nativeRelease").ok()?,
        native_safety_buffering: filtered_option(
            record.native_safety_buffering.as_ref(),
            "nativeSafetyBuffering",
        )
        .ok()?,
        native_safety_retry: extra
            .remove("nativeSafetyRetry")
            .map(|v| Some(project_agent_property(&v, "nativeSafetyRetry"))),
        native_status: filtered_option(record.native_status.as_ref(), "nativeStatus").ok()?,
        native_thread_block,
        native_turn_error: (!record.native_turn_error.is_missing()).then(|| {
            Some(project_agent_property(
                &studio_runtime_store::projection_value(&record.native_turn_error),
                "nativeTurnError",
            ))
        }),
        next_turn_settings_supported: Some(Some(true)),
        orchestrator_id: extra
            .remove("orchestratorId")
            .map(|v| Some(project_agent_property(&v, "orchestratorId"))),
        orchestrator_name: extra
            .remove("orchestratorName")
            .map(|v| Some(project_agent_property(&v, "orchestratorName"))),
        overview,
        panel_data_version: extra
            .remove("panelDataVersion")
            .map(|v| Some(project_agent_property(&v, "panelDataVersion"))),
        panel_version: extra
            .remove("panelVersion")
            .map(|v| Some(project_agent_property(&v, "panelVersion"))),
        parent_id: record
            .parent_id
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "parentId"))),
        parked_event: record
            .parked_event
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "parkedEvent"))),
        pending_settings: filtered_field(&record.pending_settings, "pendingSettings").ok()?,
        pending_settings_account_key: filtered_field(
            &record.pending_settings_account_key,
            "pendingSettingsAccountKey",
        )
        .ok()?,
        permission_mode: record
            .permission_mode
            .map(|mode| Some(permission_mode(mode))),
        pinned: nullable_field(record.pinned.clone()),
        project: filtered_field(&record.project, "project").ok()?,
        project_folder: bounded_field(&record.project_folder, "projectFolder"),
        project_folder_revision: filtered_field(
            &record.project_folder_revision,
            "projectFolderRevision",
        )
        .ok()?,
        project_id: record
            .project_id
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "projectId"))),
        project_path: Some(
            crate::project_path::project_path_from_fields(
                record.host_project_path.value().map(String::as_str),
                record.cwd.as_deref(),
            )
            .map(|path| path.as_str().to_owned()),
        ),
        project_server_id: record
            .project_server_id
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "projectServerId"))),
        provider: nullable_field(record.provider.clone()),
        queued_settings: extra
            .remove("queuedSettings")
            .map(|v| Some(project_agent_property(&v, "queuedSettings"))),
        quick_create: record.quick_create.map(|value| {
            Some(Box::new(
                studio_runtime_schema::generated::JsonValue::Boolean(value),
            ))
        }),
        read_state: extra
            .remove("readState")
            .map(|v| Some(project_agent_property(&v, "readState"))),
        read_state_supported: Some(Some(true)),
        remote_anchor: extra
            .remove("remoteAnchor")
            .map(|v| Some(project_agent_property(&v, "remoteAnchor"))),
        remote_origin: extra
            .remove("remoteOrigin")
            .map(|v| Some(project_agent_property(&v, "remoteOrigin"))),
        remote_worker: extra
            .remove("remoteWorker")
            .map(|v| Some(project_agent_property(&v, "remoteWorker"))),
        retry_at: extra
            .remove("retryAt")
            .map(|v| Some(project_agent_property(&v, "retryAt"))),
        review_defaults: record
            .review_defaults
            .as_deref()
            .cloned()
            .map(|v| Some(bound_review_defaults(v))),
        role: nullable_field(record.role.clone().map(agent_role)),
        root_id: Some(Some(bound_sync_text(&record.root_id.0, "rootId"))),
        server_id: Some(server_id),
        shared_room_id: bounded_field(&record.shared_room_id, "sharedRoomId"),
        source: Some(Some(AgentSource::Managed)),
        stall: Some(bound_stall(context.stall.clone())),
        start_attempt: filtered_option(record.start_attempt.as_ref(), "startAttempt").ok()?,
        start_wait: Some(start_wait),
        status: Some(Some(bound_sync_text(record.status.as_str(), "status"))),
        status_detail: extra
            .remove("statusDetail")
            .map(|v| Some(project_agent_property(&v, "statusDetail"))),
        subagent_concurrency_version: record.subagent_concurrency_version.map(Some),
        tail: record
            .tail
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "tail"))),
        task_default_gate: nullable_field(record.task_default_gate.clone()),
        team_counts,
        thread_id: record
            .thread_id
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "threadId"))),
        tokens_used: record.tokens_used.map(Some),
        turn_id: record
            .turn_id
            .as_deref()
            .map(|s| Some(bound_sync_text(s, "turnId"))),
        turn_status: extra
            .remove("turnStatus")
            .map(|v| Some(project_agent_property(&v, "turnStatus"))),
        unread_count: extra
            .remove("unreadCount")
            .map(|v| Some(project_agent_property(&v, "unreadCount"))),
        updated: extra
            .remove("updated")
            .map(|v| Some(project_agent_property(&v, "updated"))),
        usage_resume: filtered_field(&record.usage_resume, "usageResume").ok()?,
        voice_state: extra
            .remove("voiceState")
            .map(|v| Some(project_agent_property(&v, "voiceState"))),
        worker_defaults: filtered_field(&record.worker_defaults, "workerDefaults").ok()?,
        workspace_cleanup_intent,
        workspace_mode: record.workspace_mode.map(Some),
        workspace_operation: bounded_field(&record.workspace_operation, "workspaceOperation"),
        worktree: record.worktree.map(|v| Some(AgentWorktree::Boolean(v))),
        worktree_preparation: extra
            .remove("worktreePreparation")
            .map(|v| Some(project_agent_property(&v, "worktreePreparation"))),
    })
}

/// Build the managed renderer view from a record and explicit runtime context.
pub fn agent_entity_view(record: &Value, context: &AgentViewContext) -> Option<Value> {
    let id = record.get("id").and_then(Value::as_str)?;
    #[cfg(test)]
    crate::scheduler::publication_work::projection(id);
    let mut view = record.as_object()?.clone();
    let manual_check = crate::recovery_policy::RecoverySnapshot::from_record(record.clone())
        .map(|snapshot| snapshot.manual_connection_check())
        .unwrap_or_default();
    view.insert(
        "manualConnectionCheck".into(),
        serde::Serialize::serialize(&manual_check, studio_json::value::Serializer).ok()?,
    );
    view.insert(
        studio_runtime_schema::agent_progress::START_WAIT_FIELD.into(),
        serde::Serialize::serialize(
            &(agent_start_wait(record).ok()?),
            studio_json::value::Serializer,
        )
        .ok()?,
    );

    view.insert(
        "commandProgress".to_owned(),
        serde::Serialize::serialize(&(&context.command_progress), studio_json::value::Serializer)
            .ok()?,
    );
    view.insert(
        "stall".to_owned(),
        serde::Serialize::serialize(&(&context.stall), studio_json::value::Serializer).ok()?,
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
    let progress = match &context.transfer {
        TransferObservation::Unreadable { reason } => {
            studio_runtime_schema::agent_progress::AccountTransferProgress::Unreadable {
                reason: reason.clone(),
            }
        }
        observation @ (TransferObservation::Absent
        | TransferObservation::Cancellable
        | TransferObservation::Held) => {
            let summary = record.get("accountTransfer").filter(|value| !value.is_null())
                .map(<studio_runtime_store::transfer_record::AgentTransferState as serde::Deserialize>::deserialize).transpose().ok()?;
            crate::account_transfer::progress_view(
                summary.as_ref(),
                matches!(observation, TransferObservation::Cancellable),
            )
        }
    };
    if let Some(summary) = record.get("accountTransfer").and_then(|value| {
        <studio_runtime_store::transfer_record::AgentTransferSummary as serde::Deserialize>::deserialize(
            value.clone(),
        )
        .ok()
    }) {
        let dto =
            studio_runtime_schema::agent_entity_transfer::AccountTransferSummaryDto::from(summary);
        view.insert("accountTransfer".into(), serde::Serialize::serialize(&(dto), studio_json::value::Serializer).ok()?);
    } else if let Some(summary) = view
        .get_mut("accountTransfer")
        .and_then(Value::as_object_mut)
    {
        summary.remove("canCancel");
    }
    let source = <AccountChangeAgent as serde::Deserialize>::deserialize(record).ok()?;
    if source.is_lead == Some(true) {
        let destinations = &context.account_destinations;
        view.insert(
            "accountDestinations".into(),
            serde::Serialize::serialize(&destinations, studio_json::value::Serializer).ok()?,
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
            | studio_runtime_schema::agent_progress::AccountTransferProgress::Unreadable { .. }
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
        serde::Serialize::serialize(&account_change, studio_json::value::Serializer).ok()?,
    );
    view.insert(
        "accountTransferProgress".into(),
        serde::Serialize::serialize(&progress, studio_json::value::Serializer).ok()?,
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
            view.insert(
                "teamCounts".to_owned(),
                serde::Serialize::serialize(&(counts), studio_json::value::Serializer).ok()?,
            );
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

fn cleanup_summary(
    value: &Value,
    cwd: Option<&str>,
) -> studio_runtime_schema::agent_entity_nested::WorkspaceCleanupSummaryDto {
    let phase = value
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let reason = value
        .pointer("/receipt/reason")
        .or_else(|| value.get("reason"))
        .and_then(Value::as_str)
        .map(|s| bound_sync_text(s, "error"));
    let path = match value
        .pointer("/effect/identity/cwd")
        .or_else(|| value.get("path"))
    {
        Some(value) => value.as_str().map(str::to_owned),
        None => cwd.map(str::to_owned),
    }
    .map(|path| bound_sync_text(&path, "path"));
    studio_runtime_schema::agent_entity_nested::WorkspaceCleanupSummaryDto {
        phase,
        reason,
        path,
    }
}

pub(crate) fn retained_worktree_from_record(
    member: &studio_runtime_store::AgentRecord,
) -> Option<studio_runtime_schema::generated::RetainedWorktreeEntityDto> {
    let intent = member.extra.get("workspaceCleanupIntent")?.as_object()?;
    if member
        .extra
        .get("remoteWorker")
        .is_some_and(|value| !value.is_null())
        || !matches!(
            intent.get("phase").and_then(Value::as_str),
            Some("not_sent" | "unknown")
        )
        || !intent
            .get("effect")
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str)
            .is_some_and(|kind| {
                crate::workspace_cleanup_runtime::RETAINED_WORKTREE_EFFECTS.contains(&kind)
            })
    {
        return None;
    }
    let cleanup = cleanup_summary(
        member.extra.get("workspaceCleanupIntent")?,
        member.cwd.as_deref(),
    );
    Some(
        studio_runtime_schema::generated::RetainedWorktreeEntityDto {
            id: member.id.0.clone(),
            root_id: member.root_id.0.clone(),
            name: Some(
                member
                    .name
                    .as_ref()
                    .map(|name| bound_sync_text(name, "name")),
            ),
            cleanup,
        },
    )
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
    #[test]
    fn retained_layr_state_preserves_publication_presence_and_projection() {
        for marker in [
            None,
            Some("null"),
            Some("false"),
            Some("0"),
            Some("[]"),
            Some("{}"),
            Some(r#""state-id""#),
            Some(r#"{"future":1}"#),
        ] {
            let suffix = marker
                .map(|marker| format!(r#", "layrStateId":{marker}"#))
                .unwrap_or_default();
            let record = format!(
                r#"{{"id":"a","rootId":"a","epoch":1,"status":"idle","accountKey":"default","provider":"codex"{suffix}}}"#
            );
            let owner: studio_runtime_store::AgentRecord = studio_json::from_str(&record).unwrap();
            if marker == Some(r#""state-id""#) {
                assert!(
                    matches!(owner.layr_state_id.value(), Some(studio_runtime_store::StoredTransferValue::Known(actual)) if actual == "state-id")
                );
            }
            let expected = super::agent_entity_view(
                &studio_json::from_str(&record).unwrap(),
                &Default::default(),
            )
            .unwrap();
            let actual = super::agent_entity_from_record(owner, &Default::default()).unwrap();
            assert_eq!(
                actual
                    .layr_state_id
                    .map(|value| value.map(|raw| raw.0.as_str().to_owned())),
                expected
                    .get("layrStateId")
                    .map(|value| (!value.is_null()).then(|| value.to_string())),
                "{marker:?}"
            );
        }
    }

    use super::{
        invalid_agent_view, openapi_document, project_persisted_record,
        schema_projection_properties,
    };
    use studio_json::{Value, json};

    fn legacy_agent_projection_bytes(mut projected: Value) -> Vec<u8> {
        assert_eq!(
            projected.as_object_mut().unwrap().remove("projectPath"),
            Some(Value::Null)
        );
        studio_json::to_vec(&projected).unwrap()
    }

    #[test]
    fn task_entity_payload_hash_is_stable_after_projection_move() {
        #[derive(serde::Serialize)]
        struct Entity<T> {
            collection: &'static str,
            id: &'static str,
            value: T,
        }
        let raw = r#"{"id":"a:i","agent":"a","created":1,"itemId":"i","turnId":"turn","kind":"command","status":"running","type":"commandExecution","name":"commandExecution"}"#;
        let task: studio_runtime_store::TaskRecord = studio_json::from_str(raw).unwrap();
        let payload = studio_json::to_string(&Entity {
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
    fn agent_projection_uses_host_project_path_or_legacy_cwd() {
        for (record, expected) in [
            (
                json!({"id":"lead", "cwd":"/store/projects/id/lines/main", "hostProjectPath":"/source/project"}),
                json!("/source/project"),
            ),
            (
                json!({"id":"legacy", "cwd":"/source/legacy"}),
                json!("/source/legacy"),
            ),
            (json!({"id":"missing"}), Value::Null),
        ] {
            let view =
                super::agent_entity_view(&record, &super::AgentViewContext::default()).unwrap();
            let summary = super::project_persisted_record("agentSummary", &view).unwrap();
            assert_eq!(summary.get("projectPath"), Some(&expected));
            assert_eq!(view.get("projectPath"), Some(&expected));
        }
    }

    #[test]
    fn declared_project_preserves_publication_presence_and_legacy_projection() {
        for project in [
            None,
            Some(json!(null)),
            Some(json!("team")),
            Some(json!({"future":true})),
            Some(json!(false)),
        ] {
            let mut raw = json!({"id":"agent","rootId":"agent","epoch":1,"status":"idle","accountKey":"default"});
            if let Some(project) = project {
                raw["project"] = project;
            }
            let expected = super::agent_entity_view(&raw, &Default::default()).unwrap();
            let owner = studio_runtime_store::AgentRecord::from_value(raw).unwrap();
            let view = super::agent_entity_from_record(owner, &Default::default()).unwrap();
            assert_eq!(
                view.project
                    .map(|value| value.map(|raw| raw.0.as_str().to_owned())),
                expected
                    .get("project")
                    .map(|value| (!value.is_null()).then(|| value.to_string()))
            );
        }
    }

    #[test]
    fn move_markers_keep_truthiness_and_publication_projection_independent() {
        for key in ["movedTo", "moveImportPending"] {
            for value in [
                None,
                Some(json!(null)),
                Some(json!(false)),
                Some(json!(0)),
                Some(json!("")),
                Some(json!([])),
                Some(json!({})),
                Some(json!(true)),
                Some(json!("peer")),
                Some(json!({"server":"peer","move":"id","future":true})),
            ] {
                let mut raw = json!({"id":"agent","rootId":"agent","epoch":1,"status":"idle","accountKey":"default"});
                if let Some(value) = value {
                    raw[key] = value;
                }
                let expected = super::agent_entity_view(&raw, &Default::default()).unwrap();
                let owner = studio_runtime_store::AgentRecord::from_value(raw).unwrap();
                let view = super::agent_entity_from_record(owner, &Default::default()).unwrap();
                if key == "movedTo" {
                    let expected = expected
                        .get(key)
                        .map(|value| (!value.is_null()).then(|| value.to_string()));
                    assert_eq!(
                        view.moved_to
                            .map(|value| value.map(|raw| raw.0.as_str().to_owned())),
                        expected,
                        "{key}"
                    );
                } else {
                    assert_eq!(
                        view.move_import_pending
                            .map(|raw| raw.0.as_str().to_owned()),
                        expected.get(key).map(ToString::to_string),
                        "{key}"
                    );
                }
                assert_eq!(
                    view.can_send,
                    expected.get("canSend").map(Value::as_bool),
                    "{key}"
                );
            }
        }
    }

    #[test]
    fn agent_projection_adds_only_project_path_and_token_usage() {
        let record = json!({"id":"agent", "rootId":"agent", "epoch":1,
            "isLead":false, "status":"idle", "accountKey":"default", "cwd":"/source/project",
            "projectPath":"/untrusted/stored", "tokensUsed":42});
        let view = super::agent_entity_view(&record, &Default::default()).unwrap();
        assert_eq!(view["projectPath"], "/source/project");
        assert_eq!(view["tokensUsed"], 42);
        let summary = super::project_persisted_record("agentSummary", &view).unwrap();
        let detail = super::project_persisted_record("agentDetail", &view).unwrap();
        assert_eq!(summary["projectPath"], "/source/project");
        assert_eq!(detail["tokensUsed"], 42);
    }

    #[test]
    fn agent_projection_refuses_unknown_status() {
        let record = json!({"id":"agent", "rootId":"agent", "epoch":1,
            "status":"future-agent-state", "accountKey":"default"});
        assert!(studio_runtime_store::AgentRecord::from_value(record.clone()).is_err());
        assert!(super::agent_entity_view(&record, &Default::default()).is_none());
    }

    #[test]
    fn agent_projection_bounds_nested_legacy_values() {
        let mut deep = json!("leaf");
        for _ in 0..40 {
            deep = json!({"child": deep});
        }
        let opaque = json!({"items": (0..300).collect::<Vec<_>>(), "deep": deep,
            "unknownNested": "preserved"});
        let record = json!({"id":"agent", "rootId":"agent", "epoch":1,
            "status":"idle", "turnStatus":"future-turn-state", "accountKey":"default", "isLead":true,
            "nativeStatus":{"message":"x".repeat(2500), "unknownNested":"removed"},
            "movedTo":{"server":"peer", "move":"id", "unknownNested":"removed"},
            "voiceState":opaque, "unknownTopLevel":"removed"});
        let context = super::AgentViewContext {
            account_destinations: (0..300)
                .map(
                    |id| studio_runtime_schema::account_destination::AccountDestination {
                        account_key: format!("account-{id}").try_into().unwrap(),
                        eligibility:
                            studio_runtime_schema::agent_progress::AccountEligibility::Available,
                    },
                )
                .collect(),
            ..Default::default()
        };
        let full = super::agent_entity_view(&record, &context).unwrap();
        let summary = super::project_persisted_record("agentSummary", &full).unwrap();
        let detail = super::project_persisted_record("agentDetail", &full).unwrap();
        for (view, expected) in [
            (
                &full,
                "3e93b7dfddbbf0d71bdc6f1f96d5f7006e285e85c4ab4b8f16fb95d0c374e139",
            ),
            (
                &summary,
                "52d7905f1bb65f9805f1d5354fad6c9f8ef8b147b59e54dc90088b45429cedc9",
            ),
            (
                &detail,
                "35793af0b19edc03e1e9a06687c6b829f3ecb155c2897f3d165f65264c0d7062",
            ),
        ] {
            assert_eq!(
                studio_runtime_store::sha256_hex(studio_json::to_string(view).unwrap().as_bytes()),
                expected
            );
        }
        assert_eq!(
            full["nativeStatus"]["message"].as_str().unwrap().len(),
            2000
        );
        assert_eq!(full["accountDestinations"].as_array().unwrap().len(), 200);
        assert_eq!(
            summary["accountDestinations"].as_array().unwrap().len(),
            200
        );
        assert_eq!(detail["accountDestinations"].as_array().unwrap().len(), 200);
        assert_eq!(detail["voiceState"]["items"].as_array().unwrap().len(), 200);
        assert!(full.get("unknownTopLevel").is_none());
        assert_eq!(full["status"], "idle");
        assert_eq!(summary["status"], "idle");
        assert_eq!(detail["turnStatus"], "future-turn-state");
        assert!(full["nativeStatus"].get("unknownNested").is_none());
        assert!(full["movedTo"].get("unknownNested").is_none());
        let mut nested = &full["voiceState"]["deep"];
        for _ in 0..40 {
            if nested.is_null() {
                break;
            }
            nested = &nested["child"];
        }
        assert!(nested.is_null());
    }

    #[test]
    fn agent_projection_keeps_pending_settings_legacy_shapes() {
        for (pending, full_hash, detail_hash) in [
            (
                json!([1, 2]),
                "8d48bb13f03b71ff558ddc7558cac7fb01f31beecf87466ea4aebc0b8286c056",
                "615c8c0127c8ee682188a889a8162d5d156c419ab59c87ed8d092bc9120455ca",
            ),
            (
                json!({"unknown":42}),
                "490f61e3dc73d4cf4b2634e55430090dddf5e5529b9a3d4ebc798950f16c6075",
                "b52f064c5f0ad7ae2c6c8708fb3c384a0545d4f2bb3e49fd6974ce522cb4d1b6",
            ),
            (
                json!({"model":"m"}),
                "b5e142927a7b0faaa6b21abff36fc1525e6e4791f73c71dac9255f6ae6ff5c2b",
                "bcba321db8239694c9b00bb2b8262c12038f83b2bfeb377a54df7ea1d9d6ab1a",
            ),
            (
                json!({"model":"m", "accountKey":"default", "effort":"high", "fastMode":true, "daybreakEnabled":true, "nativeEffort":"high", "cyberAccessProgram":"test", "updatedAt":123.0}),
                "468c8d0603d02e4e6b8986bdf332a03b8a8fe5eb65069a4801dbf9a49e0c3ed6",
                "c3f1b73b62f8e172b6e0aa9b8ed0ba11e5be3aad4808c777380847c8b907418d",
            ),
        ] {
            let record = json!({"id":"agent", "rootId":"agent", "epoch":1,
                "status":"idle", "accountKey":"default", "isLead":true,
                "pendingSettings":pending});
            let full = super::agent_entity_view(&record, &Default::default()).unwrap();
            let summary = super::project_persisted_record("agentSummary", &full).unwrap();
            let detail = super::project_persisted_record("agentDetail", &full).unwrap();
            let stored = studio_runtime_store::AgentRecord::from_value(record.clone()).unwrap();
            assert_eq!(
                super::project_agent_legacy_field(
                    stored.pending_settings.value().unwrap(),
                    "pendingSettings"
                )
                .unwrap()
                .0
                .as_str(),
                studio_json::to_string(&full["pendingSettings"]).unwrap()
            );
            let stored_full =
                super::agent_entity_view(&stored.value(), &Default::default()).unwrap();
            let stored_summary =
                super::project_persisted_record("agentSummary", &stored_full).unwrap();
            let stored_detail =
                super::project_persisted_record("agentDetail", &stored_full).unwrap();
            let mapped = super::agent_entity_from_record(stored, &Default::default()).unwrap();
            let mapped_summary = crate::agent_operational::summary(&mapped, None).unwrap();
            let mapped_detail =
                studio_runtime_schema::generated::AgentDetailEntityDto::from(&mapped);
            for (actual, expected) in [
                (
                    studio_json::to_string(&mapped).unwrap(),
                    studio_json::to_string(&stored_full).unwrap(),
                ),
                (
                    studio_json::to_string(&mapped_summary).unwrap(),
                    studio_json::to_string(&stored_summary).unwrap(),
                ),
                (
                    studio_json::to_string(&mapped_detail).unwrap(),
                    studio_json::to_string(&stored_detail).unwrap(),
                ),
            ] {
                let actual: Value = studio_json::from_str(&actual).unwrap();
                let mut expected: Value = studio_json::from_str(&expected).unwrap();
                if actual.get("operationalStatus").is_some() {
                    crate::agent_operational::decorate(&mut expected, None).unwrap();
                    expected["projectServerId"] = Value::Null;
                }
                assert_eq!(actual, expected);
            }
            for (view, expected) in [
                (&full, full_hash),
                (
                    &summary,
                    "5d1874e6eca9a93b83922359ea8ddb05121245821eb70bdf21c1adc462cecc2f",
                ),
                (&detail, detail_hash),
            ] {
                assert_eq!(
                    studio_runtime_store::sha256_hex(
                        studio_json::to_string(view).unwrap().as_bytes()
                    ),
                    expected
                );
            }
            assert_eq!(full["pendingSettings"], detail["pendingSettings"]);
        }
    }

    #[test]
    fn agent_projection_layr_line_matches_legacy_presence() {
        for suffix in ["", r#","layrLine":null"#, r#","layrLine":"main""#] {
            let raw = format!(
                r#"{{"id":"agent","rootId":"agent","epoch":1,"status":"idle","accountKey":"default","compactions":0{suffix}}}"#
            );
            let value: Value = studio_json::from_str(&raw).unwrap();
            let legacy = super::agent_entity_view(&value, &Default::default()).unwrap();
            let legacy_detail = super::project_persisted_record("agentDetail", &legacy).unwrap();
            let record = studio_runtime_store::AgentRecord::from_value(value).unwrap();
            let typed = super::agent_entity_from_record(record, &Default::default()).unwrap();
            let detail = studio_runtime_schema::generated::AgentDetailEntityDto::from(&typed);
            let typed =
                serde::Serialize::serialize(&typed, studio_json::value::Serializer).unwrap();
            let detail =
                serde::Serialize::serialize(&detail, studio_json::value::Serializer).unwrap();
            assert_eq!(typed.get("layrLine"), legacy.get("layrLine"));
            assert_eq!(detail.get("layrLine"), legacy_detail.get("layrLine"));
        }
    }

    #[test]
    fn agent_projection_filters_partial_activity_without_narrowing_it() {
        let record = json!({"id":"agent", "rootId":"agent", "epoch":1,
            "status":"idle", "accountKey":"default", "activity":{
                "phase":"working", "tools":[{"name":"tool"}], "dismissed":true}});
        let full = super::agent_entity_view(&record, &Default::default()).unwrap();
        let stored = studio_runtime_store::AgentRecord::from_value(record).unwrap();
        assert_eq!(
            super::project_agent_legacy_field(stored.activity.as_ref().unwrap(), "activity")
                .unwrap()
                .0
                .as_str(),
            studio_json::to_string(&full["activity"]).unwrap()
        );
        assert_eq!(full["activity"]["tools"][0], json!({"name":"tool"}));
        assert!(full["activity"].get("dismissed").is_none());
    }

    #[test]
    fn review_regression_bounds_turn_id_and_retained_path() {
        let long = "x".repeat(12001);
        let raw = json!({"id":"worker","rootId":"lead","epoch":1,"status":"completed",
            "accountKey":"default","lastCompletedTurn":long,"lastAnswer":"answer",
            "workspaceCleanupIntent":{"phase":"unknown","effect":{"kind":"remove_worktree","identity":{"cwd":long}}}});
        let record = studio_runtime_store::AgentRecord::from_value(raw).unwrap();
        let stored = record.value();
        let legacy = super::agent_entity_view(&stored, &Default::default()).unwrap();
        let retained = super::retained_worktree_entity(&stored).unwrap();
        let typed_retained = super::retained_worktree_from_record(&record).unwrap();
        let typed = super::agent_entity_from_record(record, &Default::default()).unwrap();
        let typed = serde::Serialize::serialize(&typed, studio_json::value::Serializer).unwrap();
        let typed_retained =
            serde::Serialize::serialize(&typed_retained, studio_json::value::Serializer).unwrap();
        assert_eq!(
            typed["overview"]["resultTurnId"],
            legacy["overview"]["resultTurnId"]
        );
        assert_eq!(
            typed_retained["cleanup"]["path"],
            retained["cleanup"]["path"]
        );
        assert_eq!(
            typed_retained["cleanup"]["path"].as_str().unwrap().len(),
            12000
        );
    }

    #[test]
    fn typed_retained_worktree_matches_legacy_projection() {
        for intent in [
            json!({"phase":"unknown","effect":{"kind":"remove_worktree"}}),
            json!({"phase":"not_sent","path":"/fallback","reason":"reason",
                "receipt":{"reason":null},"effect":{"kind":"save_worktree_ref","identity":{"cwd":"/path"}}}),
            json!({"phase":"confirmed","effect":{"kind":"remove_worktree"}}),
        ] {
            let raw = json!({"id":"agent","rootId":"lead","epoch":1,"status":"idle",
                "accountKey":"default","name":"name","workspaceCleanupIntent":intent});
            let old = super::retained_worktree_entity(&raw);
            let record = studio_runtime_store::AgentRecord::from_value(raw).unwrap();
            let typed = super::retained_worktree_from_record(&record)
                .map(|v| serde::Serialize::serialize(&v, studio_json::value::Serializer).unwrap());
            assert_eq!(typed, old);
        }
    }

    #[test]
    fn agent_projection_accepts_legacy_transfer_fields() {
        for (transfer, context) in [
            (
                json!({"accountTransfer":{"kind":"unreadable"}}),
                super::AgentViewContext {
                    transfer: super::TransferObservation::Unreadable {
                        reason: "Transfer data is unreadable".into(),
                    },
                    ..Default::default()
                },
            ),
            (
                json!({"accountTransferId":""}),
                super::AgentViewContext::default(),
            ),
        ] {
            let mut raw = json!({"id":"agent", "rootId":"agent", "epoch":1,
                "status":"idle", "accountKey":"default"});
            raw.as_object_mut()
                .unwrap()
                .extend(transfer.as_object().unwrap().clone());
            let legacy_raw = super::agent_entity_view(&raw, &context).unwrap();
            let stored = studio_runtime_store::AgentRecord::from_value(raw).unwrap();
            let legacy = super::agent_entity_view(&stored.value(), &context).unwrap();
            let mapped = super::agent_entity_from_record(stored, &context).unwrap();
            let actual =
                serde::Serialize::serialize(&mapped, studio_json::value::Serializer).unwrap();
            if legacy_raw.get("accountTransferId") == Some(&json!("")) {
                assert!(actual.get("accountTransferId").is_none());
            }
            assert_eq!(actual, legacy);
        }
    }

    #[test]
    fn agent_projection_preserves_malformed_turn_status() {
        let raw = json!({"id":"agent", "rootId":"agent", "epoch":1,
            "status":"idle", "accountKey":"default", "turnStatus":{"future":true}});
        let stored = studio_runtime_store::AgentRecord::from_value(raw).unwrap();
        let legacy = super::agent_entity_view(&stored.value(), &Default::default()).unwrap();
        let mapped = super::agent_entity_from_record(stored, &Default::default()).unwrap();
        assert_eq!(
            mapped
                .turn_status
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .0
                .as_str(),
            r#"{"future":true}"#
        );
        let serialized = studio_json::to_string(&mapped).unwrap();
        let actual: Value = studio_json::from_str(&serialized).unwrap();
        assert_eq!(actual, legacy);
    }

    #[test]
    fn agent_projection_matches_legacy_for_queued_and_worker_records() {
        for raw in [
            json!({"id":"lead", "rootId":"lead", "epoch":2, "status":"queued",
                "accountKey":"default", "isLead":true, "autoWake":true,
                "workerExcludedAccountKeys":["blocked"], "turnStatus":null}),
            json!({"id":"worker", "rootId":"lead", "epoch":2, "status":"completed",
                "accountKey":"default", "isLead":false, "prompt":"task", "lastAnswer":"done",
                "lastCompletedTurn":"turn", "activity":{"phase":"working", "tools":[{"name":"tool"}]}}),
        ] {
            let stored = studio_runtime_store::AgentRecord::from_value(raw).unwrap();
            let context = super::AgentViewContext {
                command_progress: vec![
                    studio_runtime_schema::agent_progress::AgentCommandProgress {
                        monitor_id: "monitor".into(),
                        stage: "stage".into(),
                        started_at: 12.0,
                        last_output_line: "output".into(),
                    },
                ],
                ..Default::default()
            };
            let legacy = super::agent_entity_view(&stored.value(), &context).unwrap();
            let mapped = super::agent_entity_from_record(stored, &context).unwrap();
            let text = studio_json::to_string(&mapped).unwrap();
            let actual: Value = studio_json::from_str(&text).unwrap();
            assert_eq!(actual, legacy);
        }
    }

    #[test]
    fn typed_agent_start_wait_matches_the_stored_projection() {
        for (status, auto_wake, hold, transfer) in [
            ("queued", false, json!(false), Value::Null),
            ("queued", true, json!({"diagnostic":"held"}), Value::Null),
            ("queued", true, json!(false), json!("transfer")),
            ("queued", true, json!(false), Value::Null),
            ("starting", true, json!(false), Value::Null),
            ("idle", true, json!(false), Value::Null),
        ] {
            let raw = json!({"id":"agent", "rootId":"agent", "epoch":2,
                "status":status, "accountKey":"default", "autoWake":auto_wake,
                "nativeFailureHold":hold, "accountTransferId":transfer});
            let stored = studio_runtime_store::AgentRecord::from_value(raw).unwrap();
            let legacy = super::agent_start_wait(&stored.value()).unwrap();
            assert_eq!(super::agent_start_wait_record(&stored).unwrap(), legacy);
        }
    }

    #[test]
    fn agent_projection_decoded_record_compatibility_baseline() {
        let mut nested = json!("leaf");
        for _ in 0..40 {
            nested = json!({"next":nested});
        }
        let record = json!({"id":"agent", "rootId":"agent", "epoch":1,
            "status":"idle", "accountKey":"default", "compactions":0,
            "isLead":true, "cwd":"/fallback", "hostProjectPath":"x".repeat(12_001),
            "error":"e".repeat(2_500), "nativeStatus":{"phase":"safety", "message":"n".repeat(2_500)},
            "taskDefaultGate":{"command":"g".repeat(12_001), "timeout_ms":1000},
            "voiceState":{"items":(0..300).collect::<Vec<_>>(), "deep":nested},
            "workspaceCleanupIntent":{"phase":"unknown", "path":"/intent", "reason":"fallback",
                "effect":{"kind":"remove_worktree", "identity":{"cwd":null}}, "receipt":{"reason":false}},
            "pendingSettings":[1,2], "futureField":"removed"});
        let stored = studio_runtime_store::AgentRecord::from_value(record).unwrap();
        let source = stored.value();
        let full = super::agent_entity_view(&source, &Default::default()).unwrap();
        let summary = super::project_persisted_record("agentSummary", &full).unwrap();
        let detail = super::project_persisted_record("agentDetail", &full).unwrap();
        let mapped = super::agent_entity_from_record(stored.clone(), &Default::default()).unwrap();
        let repeated = super::agent_entity_from_record(stored, &Default::default()).unwrap();
        assert_eq!(
            studio_json::to_string(&mapped).unwrap(),
            studio_json::to_string(&repeated).unwrap()
        );
        assert_eq!(
            studio_json::to_string(&crate::agent_operational::summary(&mapped, None).unwrap())
                .unwrap(),
            studio_json::to_string(&crate::agent_operational::summary(&repeated, None).unwrap())
                .unwrap()
        );
        assert_eq!(
            studio_json::to_string(
                &studio_runtime_schema::generated::AgentDetailEntityDto::from(&mapped)
            )
            .unwrap(),
            studio_json::to_string(
                &studio_runtime_schema::generated::AgentDetailEntityDto::from(&repeated)
            )
            .unwrap()
        );
        let mut mapped_summary = crate::agent_operational::summary(&mapped, None).unwrap();
        let mapped_detail = studio_runtime_schema::generated::AgentDetailEntityDto::from(&mapped);
        assert_eq!(mapped_summary.project_path, mapped.project_path);
        // Compare the old summary after accounting for the approved long-path change.
        mapped_summary.project_path = Some(Some("x".repeat(12_000)));
        for (actual, expected) in [
            (studio_json::to_string(&mapped).unwrap(), &full),
            (studio_json::to_string(&mapped_summary).unwrap(), &summary),
            (studio_json::to_string(&mapped_detail).unwrap(), &detail),
        ] {
            let actual: Value = studio_json::from_str(&actual).unwrap();
            let mut expected = expected.clone();
            if actual.get("operationalStatus").is_some() {
                crate::agent_operational::decorate(&mut expected, None).unwrap();
                expected["projectServerId"] = Value::Null;
            }
            assert_eq!(actual, expected);
        }
        for (view, expected) in [
            (
                &full,
                "2b47b6922bf7e358b0ddd9244470d7fec2380a817de5476e42e4eed3abab964b",
            ),
            (
                &summary,
                "eb3f9ff27da0a945956b312ffc0b285a2f874bd852f0d4398ceb01eee9eccc75",
            ),
            (
                &detail,
                "859f594095fa74c1ba4c1d50de34fa80bda8d5dbfb3a8fac25e4f462c0e04ad9",
            ),
        ] {
            assert_eq!(
                studio_runtime_store::sha256_hex(studio_json::to_string(view).unwrap().as_bytes()),
                expected
            );
        }
        assert_eq!(full["workspaceCleanupIntent"]["reason"], Value::Null);
        assert_eq!(full["workspaceCleanupIntent"]["path"], Value::Null);
        assert_eq!(
            full["taskDefaultGate"]["command"].as_str().unwrap().len(),
            12_001
        );
        assert_eq!(full["projectPath"].as_str().unwrap().len(), 12_001);
        assert_eq!(summary["projectPath"].as_str().unwrap().len(), 12_000);
    }

    #[test]
    fn native_error_projection_preserves_legacy_shapes() {
        for error in [
            json!(null),
            json!("legacy"),
            json!({"turnId":"turn","error":{"codexErrorInfo":"misalignmentPolicyViolation"},"future":1}),
            json!({"turnId":null,"error":false}),
        ] {
            let input = json!({"id":"a","rootId":"a","epoch":1,"status":"idle","accountKey":"default","compactions":0,"threadId":"thread","turnId":"turn","nativeTurnError":error});
            let record = studio_runtime_store::AgentRecord::from_value(input).unwrap();
            let source = record.value();
            let legacy = super::agent_entity_view(&source, &Default::default()).unwrap();
            let typed = super::agent_entity_from_record(record, &Default::default()).unwrap();
            // Recreate the former extra-map field projection inside the same DTO:
            // sync hashes the DTO's serialized bytes, including field order.
            let mut previous = typed.clone();
            previous.native_turn_error = source
                .get("nativeTurnError")
                .map(|value| Some(super::project_agent_property(value, "nativeTurnError")));
            assert_eq!(
                studio_json::to_vec(&typed).unwrap(),
                studio_json::to_vec(&previous).unwrap()
            );
            let projected = studio_runtime_store::projection_value(&typed);
            assert_eq!(projected, legacy);
        }
    }

    #[test]
    fn shared_agent_components_preserve_legacy_projection_values() {
        for native in [json!("idle"), json!({"phase":"safety","message":"waiting"})] {
            let record = json!({"id":"agent", "kind":"agent", "environment":"host",
                "error":"error", "executionMode":"native", "nativeStatus":native,
                "permissionMode":"plan", "worktree":true, "worktreePreparation":"waiting"});
            let mut expected = record.clone();
            expected["projectPath"] = Value::Null;
            assert_eq!(
                super::project_persisted_record("agent", &record).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn lead_destination_eligibility_ignores_worker_exclusions() {
        let lead = json!({"id":"lead","rootId":"lead","epoch":1,"isLead":true,
            "provider":"codex","status":"idle","accountKey":"default","compactions":0,"workerExcludedAccountKeys":["blocked"]});
        let context = super::AgentViewContext {
            account_destinations: studio_json::from_str(
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
            "available"
        );
        assert_eq!(
            view["accountDestinations"][1]["eligibility"]["kind"],
            "available"
        );
        let empty_context = super::AgentViewContext {
            empty_lead: true,
            ..context
        };
        let empty = super::agent_entity_view(&lead, &empty_context).unwrap();
        let stored = studio_runtime_store::AgentRecord::from_value(lead).unwrap();
        let typed = super::agent_entity_from_record(stored, &empty_context).unwrap();
        let typed = serde::Serialize::serialize(&typed, studio_json::value::Serializer).unwrap();
        assert_eq!(
            typed["accountDestinations"][0]["eligibility"]["kind"],
            "available"
        );
        assert_eq!(
            empty["accountDestinations"][0]["eligibility"]["kind"],
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
    fn removed_agent_diagnostics_are_not_projected() {
        let raw = r#"{"id":"agent","rootId":"agent","epoch":1,"status":"idle","accountKey":"default","compactions":0,"checkpointError":"legacy","complaintsPresented":["c"],"workspaceBackend":null}"#;
        let record = studio_json::from_str(raw).unwrap();
        let projected =
            super::agent_entity_view(&record, &super::AgentViewContext::default()).unwrap();
        assert!(projected.get("checkpointError").is_none());
        assert!(projected.get("complaintsPresented").is_none());
        assert!(projected.get("workspaceBackend").is_none());
        assert_eq!(
            studio_runtime_store::sha256_hex(&legacy_agent_projection_bytes(projected)),
            "53bb9c6ebdcfbbd45e72b104f3159233ed2b17c1e4c4e8b4b39c6c4feaab00c1"
        );
    }

    #[test]
    fn removed_compaction_flag_changes_agent_projection() {
        let raw = r#"{"id":"agent","rootId":"agent","epoch":1,"status":"idle","accountKey":"default","compactions":0,"compactionsObservedOnly":false}"#;
        let record = studio_json::from_str(raw).unwrap();
        let projected =
            super::agent_entity_view(&record, &super::AgentViewContext::default()).unwrap();
        assert!(projected.get("compactionsObservedOnly").is_none());
        assert_eq!(
            studio_runtime_store::sha256_hex(&legacy_agent_projection_bytes(projected)),
            "53bb9c6ebdcfbbd45e72b104f3159233ed2b17c1e4c4e8b4b39c6c4feaab00c1"
        );
    }

    #[test]
    fn agent_projection_uses_canonical_capacity() {
        let record = studio_json::json!({"id":"agent","rootId":"agent","epoch":1,"status":"idle","accountKey":"default","compactions":0,"capacity":{"kind":"explicit","max":80}});
        let projected =
            super::agent_entity_view(&record, &super::AgentViewContext::default()).unwrap();
        assert_eq!(projected["capacity"], record["capacity"]);
        assert!(projected.get("maxAgents").is_none());
        assert!(projected.get("maxAgentsExplicit").is_none());
    }

    #[test]
    fn canonical_naming_states_project_to_agent() {
        for naming in ["automatic", "pending_title", "manual"] {
            let record = studio_json::json!({"id":"agent","rootId":"agent","epoch":1,"status":"idle","accountKey":"default","compactions":0,"naming":naming});
            let view =
                super::agent_entity_view(&record, &super::AgentViewContext::default()).unwrap();
            assert_eq!(view["naming"], naming);
            assert!(view.get("manualName").is_none());
        }
    }

    #[test]
    fn chat_organization_flags_project_with_canonical_capacity() {
        let raw = r#"{"id":"agent","rootId":"agent","epoch":1,"status":"idle","accountKey":"default","compactions":0,"archived":true,"pinned":true}"#;
        let agent: studio_runtime_store::AgentRecord = studio_json::from_str(raw).unwrap();
        let projected =
            super::agent_entity_view(&agent.value(), &super::AgentViewContext::default()).unwrap();
        assert_eq!(
            studio_runtime_store::sha256_hex(&legacy_agent_projection_bytes(projected)),
            "80e14a8a655e664574ce206954aa188b700430e6c5c27f4b0fe887101a76cc64"
        );
    }

    #[test]
    fn legacy_image_metadata_does_not_change_agent_projection_hash() {
        let before = r#"{"id":"agent","rootId":"agent","epoch":1,"status":"idle","accountKey":"default","compactions":0,"authResumeAttempt":1,"turnRecovery":{"outcome":"idle"},"lastContextRepairWait":{"status":"superseded"},"lastContextRepairCheck":{"phase":"waiting"},"completedWorkerTurns":2,"supervisorRestore":{"status":"not_restored"},"countedWorkerTurn":true,"imageWorkspaceBaseError":"old","imageWorkspaceBaseRef":"main","imageWorkspaceCreatedAt":2,"imageWorkspaceError":"old","imageWorkspaceHasGit":true,"imageWorkspaceNoticeSent":true,"imageWorkspaceNoticeText":"old","imageWorkspaceRelative":".","imageWorkspaceStartCommit":"abc","imageWorkspaceSubpath":"."}"#;
        let after = r#"{"id":"agent","rootId":"agent","epoch":1,"status":"idle","accountKey":"default","compactions":0,"authResumeAttempt":1}"#;
        let context = super::AgentViewContext::default();
        let project = |raw: &str| {
            let record = studio_json::from_str(raw).unwrap();
            legacy_agent_projection_bytes(super::agent_entity_view(&record, &context).unwrap())
        };
        let legacy = project(before);
        let normalized = project(after);
        assert_eq!(legacy, normalized);
        assert_eq!(
            studio_runtime_store::sha256_hex(&normalized),
            "53bb9c6ebdcfbbd45e72b104f3159233ed2b17c1e4c4e8b4b39c6c4feaab00c1"
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
                studio_json::from_str(&raw).unwrap();
            let encoded = studio_json::to_string(
                &super::project_complaint_record(&record, &context).unwrap(),
            )
            .unwrap();
            let actual: studio_json::Value = studio_json::from_str(&encoded).unwrap();
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
                "blocked":[{"id":"lead","reason":"History unavailable","provider":"codex"}],"canRetry":true,"canCancel":true,"movingNow":0}});
        let context = super::AgentViewContext {
            transfer: super::TransferObservation::Cancellable,
            ..Default::default()
        };
        let view = super::agent_entity_view(&lead, &context).unwrap();
        let progress = view
            .get("accountTransferProgress")
            .and_then(Value::as_object)
            .expect("transfer progress");
        assert_eq!(progress.get("kind"), Some(&json!("blocked")));
        assert_eq!(progress.get("actions"), Some(&json!("retry_cancel")));
        let summary = view
            .get("accountTransfer")
            .and_then(Value::as_object)
            .expect("transfer summary");
        let blocked = summary
            .get("blocked")
            .and_then(Value::as_array)
            .expect("blocked notices")
            .first()
            .and_then(Value::as_object)
            .expect("blocked notice");
        assert_eq!(blocked.get("id"), Some(&json!("lead")));
        assert!(blocked.get("provider").is_none());
        assert!(summary.get("canCancel").is_none());
        for collection in ["agentSummary", "agentDetail"] {
            let projected = project_persisted_record(collection, &view).unwrap();
            assert_eq!(
                projected
                    .get("accountTransferProgress")
                    .and_then(|value| value.get("kind")),
                Some(&json!("blocked"))
            );
            assert_eq!(
                projected
                    .get("accountChange")
                    .and_then(|value| value.get("kind")),
                Some(&json!("unavailable"))
            );
        }
    }

    #[test]
    fn task_projection_matches_schema_projection() {
        for length in [12, 2_100] {
            let raw = json!({"id":"a:item", "agent":"a", "created":1.0, "itemId":"item", "kind":"command", "status":"running", "turnId":"turn", "type":"commandExecution", "name":"run", "tail":"雪".repeat(length), "outputTruncated":false, "command":"x".repeat(length), "sourceThreadId":"private", "interactive":true});
            let task: studio_runtime_store::TaskRecord =
                serde::Deserialize::deserialize(&raw).unwrap();
            let encoded =
                studio_json::to_string(&super::project_task_record(&task).unwrap()).unwrap();
            let mut new: studio_json::Value = studio_json::from_str(&encoded).unwrap();
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
            let value: studio_json::Value = studio_json::from_str(raw).expect("fixture JSON");
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
        assert_eq!(view["cleanup"]["path"], studio_json::Value::Null);
        assert_eq!(view["cleanup"]["reason"], studio_json::Value::Null);
        assert!(!view.to_string().contains("secret"));
        assert!(!view.to_string().contains("private"));
    }

    #[test]
    fn complaint_projection_preserves_integer_seconds() {
        let raw = r#"{"id":"c","author":null,"leadId":null,"text":null,"recipient":"lead","status":"open","created":1,"updated":2,"readAt":null,"version":null,"responses":[]}"#;
        let record = studio_json::from_str(raw).unwrap();
        let view =
            super::project_complaint_record(&record, &super::ComplaintViewContext::default())
                .unwrap();
        let encoded = studio_json::to_string(&view).unwrap();
        let actual: studio_json::Value = studio_json::from_str(&encoded).unwrap();
        assert_eq!(actual["created"], json!(1));
        assert_eq!(actual["updated"], json!(2));
        let stored = studio_json::to_string(&record).unwrap();
        assert_eq!(
            studio_runtime_store::sha256_hex(stored.as_bytes()),
            "413cdeb2d2cd1fb25994cbbac3469ccb6fb478a625197a6ebd205b453f97e49a"
        );
    }

    #[test]
    fn complaint_names_preserve_missing_and_null() {
        let raw = r#"{"id":"complaint","author":"author","leadId":null,"text":"Question","recipient":"lead","status":"open","created":null,"updated":null,"readAt":null,"version":null,"responses":[]}"#;
        let record: studio_runtime_schema::complaint_record::ComplaintRecord =
            studio_json::from_str(raw).unwrap();
        for (agent, expected) in [
            (r#"{"id":"author"}"#, json!("author")),
            (r#"{"id":"author","name":null}"#, json!(null)),
            (r#"{"id":"author","name":"Author"}"#, json!("Author")),
        ] {
            let agents = [studio_json::from_str(agent).expect("typed agent")];
            let view = super::project_complaint_record(
                &record,
                &super::ComplaintViewContext { agents: &agents },
            )
            .expect("complaint view");
            let encoded = studio_json::to_string(&view).unwrap();
            let actual: studio_json::Value = studio_json::from_str(&encoded).unwrap();
            assert_eq!(actual["authorName"], expected);
        }
    }

    /// The agent view publishes token usage but no team budget, also when an
    /// older build stored budget fields. The older view never published an empty pattern list, and
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
            assert_eq!(view["tokensUsed"], 120);
            for field in [
                "tokenBudget",
                "budgetBlocked",
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
            serde::Serialize::serialize(&view, studio_json::value::Serializer).unwrap()
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
        let mut record = record;
        record["name"] = json!("Held chat");
        record["cwd"] = json!("/held");
        let view = invalid_agent_view("child", &record, "invalid field /prompt");
        assert_eq!(
            view["provider"], "codex",
            "held sidebar rows retain a checked provider"
        );
        assert_eq!(view["name"], "Held chat");
        assert_eq!(view["cwd"], "/held");
        assert_eq!(view["rootId"], "lead");
        assert_eq!(view["isLead"], true);
        assert_eq!(view["status"], "failed");

        let malformed_root = json!({"provider":studio_runtime_schema::ProviderKind::Codex,"id":"child","rootId":17,"epoch":4,"isLead":false});
        let fallback = invalid_agent_view("child", &malformed_root, "invalid field /rootId");
        assert_eq!(fallback["rootId"], "child");
        assert_eq!(fallback["isLead"], false);
        let unrepresentable = invalid_agent_view(
            "child",
            &json!({"provider":"future","name":{},"cwd":7}),
            "invalid provider",
        );
        for field in ["provider", "name", "cwd"] {
            assert!(
                unrepresentable.get(field).is_none(),
                "malformed {field} is never invented"
            );
        }
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

#[cfg(test)]
mod work_views_tests {
    use super::work_views;
    use std::collections::BTreeMap;
    use studio_json::{Value, json};

    #[test]
    fn history_entry_preserves_noncanonical_legacy_json_value() {
        use studio_runtime_store::entity_publication::WorkHistoryKind;
        for (root, value) in [
            (r#""lead""#, r#"{ "z": 2, "id": "r0" }"#),
            ("null", "null"),
            ("42", r#"[ "legacy", 1 ]"#),
        ] {
            let root = studio_json::OwnedRawJson::from_string(root.into()).unwrap();
            let value = studio_json::OwnedRawJson::from_string(value.into()).unwrap();
            let expected_root: Value = studio_json::from_str(root.as_str()).unwrap();
            let expected_value: Value = studio_json::from_str(value.as_str()).unwrap();
            for kind in WorkHistoryKind::ALL {
                let row = super::work_history_entry(kind, "w", root.clone(), 0, value.clone());
                let encoded = studio_json::to_string(&row).unwrap();
                let actual: Value = studio_json::from_str(&encoded).unwrap();
                assert_eq!(
                    actual,
                    json!({"id":"w:0","index":0,"rootId":expected_root,"taskId":"w","value":expected_value})
                );
            }
        }
    }

    #[test]
    fn work_views_build_results_decisions_and_releases() {
        let raw_text = r#"{"id":"w","rootId":"lead","title":"work","status":"review","results":[{"id":"r0"},{"id":"r1"}],"decisions":[{"id":"d0"}],"releases":[{"id":"x0"}]}"#;
        let raw: Value = studio_json::from_str(raw_text).unwrap();
        let item: studio_runtime_store::WorkItemRecord = studio_json::from_str(raw_text).unwrap();
        let refusal = json!({"reason": "none"});
        let views = work_views(&item, &raw, refusal, &BTreeMap::new()).unwrap();
        let collections: Vec<&str> = views.iter().map(|(collection, _)| *collection).collect();
        assert_eq!(
            collections,
            [
                "work",
                "workDetail",
                "workResult",
                "workResult",
                "workDecision",
                "workRelease"
            ]
        );
        let texts: Vec<String> = views
            .iter()
            .map(|(_, view)| studio_json::to_string(view).unwrap())
            .collect();
        assert_eq!(
            texts[0],
            r#"{"id":"w","rootId":"lead","status":"review","title":"work"}"#
        );
        // Metadata keys sort first; the policy object is always last.
        assert!(texts[1].starts_with(
            r#"{"decisions":[],"id":"w","releases":[],"results":[],"rootId":"lead","status":"review","submissionRefusal":{"reason":"none"},"title":"work","policy":{"#
        ));
        assert!(texts[1].contains(r#""acceptAvailable":true"#));
        assert_eq!(
            texts[2],
            r#"{"id":"w:0","index":0,"rootId":"lead","taskId":"w","value":{"id":"r0"}}"#
        );
        assert_eq!(
            texts[3],
            r#"{"id":"w:1","index":1,"rootId":"lead","taskId":"w","value":{"id":"r1"}}"#
        );
        assert_eq!(
            texts[4],
            r#"{"id":"w:0","index":0,"rootId":"lead","taskId":"w","value":{"id":"d0"}}"#
        );
        assert_eq!(
            texts[5],
            r#"{"id":"w:0","index":0,"rootId":"lead","taskId":"w","value":{"id":"x0"}}"#
        );
    }
}
