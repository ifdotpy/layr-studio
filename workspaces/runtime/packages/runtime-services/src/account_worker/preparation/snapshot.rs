//! Checked projections of stored rows and immutable runtime preparation resources.
use super::*;
use crate::provider_adapter::{ProviderKind, codex_user_mcp_servers};
use crate::{model_catalog::ModelCatalogService, thread_prepare as thread, turn_prepare as turn};
use rusqlite::OptionalExtension;
use serde::{Deserialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};
use studio_json::Map;
use studio_runtime_schema::provider_errors::is_misalignment_policy_violation;
use studio_runtime_store::path_identity::CanonicalPath;
use studio_runtime_store::{AgentRecord, Transaction};

#[derive(Clone)]
pub struct PreparationResources {
    pub instructions: String,
    pub thread_config: Map<String, Value>,
    pub tools: Vec<thread::ToolDefinition>,
    pub legacy_tools: Vec<thread::ToolDefinition>,
    pub skills_root: PathBuf,
    pub role_guidance: BTreeMap<PathBuf, String>,
    pub model_catalog: ModelCatalogService,
    pub account_home: String,
    /// Filesystem observations captured before SQL callbacks; retained by commit guards.
    pub filesystem_inputs: Option<Arc<PreparationFilesystem>>,
    /// The shared (base) Codex home whose `node_repl` and Chrome plugin the
    /// browser setup copies; empty leaves host Codex threads without a browser.
    pub shared_home: String,
    pub claude_account_pin: Value,
    pub account_driver: Option<Arc<crate::account_driver::AccountDriver>>,
    /// The host's registry-backed session surface for preparation calls that
    /// need to coordinate another live account.
    pub account_session: Option<Arc<dyn crate::account_session_runtime::AccountSessionPort>>,
}
/// Account guidance and receipt path evidence captured outside SQLite callbacks.
#[derive(Clone, Default)]
pub struct PreparationFilesystem {
    browser: Option<crate::browser::AccountBrowser>,
    mcp_servers: Vec<String>,
    receipt_cwd: Option<(String, String, bool)>,
    receipt: bool,
    native_layr: Option<Result<NativeLayrFilesystem, ()>>,
}

#[derive(Clone)]
struct NativeLayrFilesystem {
    identity: crate::layr_native::settings::StoreAssociation,
    prepared_identity: Option<crate::layr_native::settings::StoreAssociation>,
    is_reviewer: bool,
    root: PathBuf,
    environment: Map<String, Value>,
    state_root: PathBuf,
    reviewer: Option<(String, Map<String, Value>)>,
}

impl PreparationResources {
    pub fn bundled(
        account_home: String,
        model_catalog: ModelCatalogService,
    ) -> Result<Self, WorkerError> {
        let root = CanonicalPath::existing(studio_runtime_schema::contracts::repository_root())
            .map_err(|_| invalid("repository root"))?;
        Self::bundled_at(account_home, model_catalog, root.to_path_buf())
    }

    pub fn bundled_at(
        account_home: String,
        model_catalog: ModelCatalogService,
        root: PathBuf,
    ) -> Result<Self, WorkerError> {
        #[derive(Deserialize)]
        struct Bundled {
            instructions: String,
            thread_config: Map<String, Value>,
            tools: Vec<thread::ToolDefinition>,
        }
        let mut data: Bundled =
            studio_json::from_str(include_str!("../preparation-resources.json"))
                .map_err(|_| invalid("preparation resources"))?;
        let mut legacy_tools: Vec<thread::ToolDefinition> = studio_json::from_str(include_str!(
            "../../../../../../../.agents/skills/codex-workspace/references/native-tools.json"
        ))
        .map_err(|_| invalid("legacy tools"))?;
        for tool in data.tools.iter_mut().chain(legacy_tools.iter_mut()) {
            studio_runtime_schema::review::apply_tool_contract(&tool.name, &mut tool.input_schema)
                .map_err(|_| invalid("review tool schema"))?;
            studio_runtime_schema::task_results::apply_tool_contract(
                &tool.name,
                &mut tool.input_schema,
            );
            studio_runtime_schema::task_gate::apply_tool_contract(
                &tool.name,
                &mut tool.input_schema,
            );
            studio_runtime_schema::task_command::apply_tool_contract(
                &tool.name,
                &mut tool.input_schema,
            );
            if tool.name == "orchestration_task" {
                tool.description.push_str(&format!(" gate on create/update runs a managed acceptance check at the submitted Git commit. set_default_gate sets the lead default for new tasks. Gate timeout defaults to {} ms. Layr and non-Git submissions report not_run and require override_reason. Failed or unknown gates refuse acceptance unless the lead supplies override_reason. Submit again to rerun a gate.", studio_runtime_schema::task_gate::DEFAULT_TIMEOUT_MS));
            }
            if tool.name == "orchestration_spawn" {
                let properties = tool
                    .input_schema
                    .get_mut("properties")
                    .and_then(|value| value.pointer_mut("/agents/items/properties"))
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| invalid("spawn properties"))?;
                properties.insert("permission_mode".into(), json!({
                    "type": "string",
                    "enum": crate::claude_controls::PERMISSION_MODES,
                    "description": "Claude child permission mode. Defaults to the parent's mode. Bypass requires a bypass parent. Reviewers always use plan."
                }));
            }
        }
        let skills_root = root.join(".agents/skills");
        let mut role_guidance = BTreeMap::new();
        for relative in [
            "codex-orchestrator/SKILL.md",
            "codex-subagent/SKILL.md",
            "codex-orchestrator/references/native-role.md",
            "codex-orchestrator/references/legacy-move-role.md",
            "codex-subagent/references/native-role.md",
        ] {
            role_guidance.insert(
                PathBuf::from(relative),
                std::fs::read_to_string(skills_root.join(relative))
                    .map_err(|_| refusal("thread-preparation/missing-guidance".into()))?,
            );
        }
        Ok(Self {
            role_guidance,
            instructions: data.instructions,
            thread_config: data.thread_config,
            tools: data.tools,
            legacy_tools,
            skills_root,
            model_catalog,
            account_home,
            filesystem_inputs: None,
            shared_home: String::new(),
            claude_account_pin: Value::Null,
            account_driver: None,
            account_session: None,
        })
    }

    fn capture_filesystem(mut self, receipt_cwd: Option<(String, String)>) -> Self {
        studio_runtime_store::debug_assert_outside_sql();
        let browser = (!self.account_home.is_empty() && !self.shared_home.is_empty()).then(|| {
            crate::browser::account_browser(
                Path::new(&self.account_home),
                Path::new(&self.shared_home),
            )
        });
        let mcp_servers = if self.account_home.is_empty() {
            Vec::new()
        } else {
            codex_user_mcp_servers(&self.account_home)
        };
        let receipt_cwd = receipt_cwd.map(|(stored, prepared)| {
            let same = studio_runtime_store::path_identity::same_text(&stored, &prepared);
            (stored, prepared, same)
        });
        self.filesystem_inputs = Some(Arc::new(PreparationFilesystem {
            browser,
            mcp_servers,
            receipt_cwd,
            receipt: false,
            native_layr: None,
        }));
        self
    }

    fn capture_native_layr(
        mut self,
        state_root: &Path,
        record: &Value,
        prepared_cwd: Option<&Path>,
    ) -> Result<Self, WorkerError> {
        if crate::layr_native::is_native(record) {
            let identity = crate::layr_native::settings::store_association(record)
                .map_err(|_| invalid("native-layr/store"))?;
            let prepared_identity = if matches!(
                identity,
                crate::layr_native::settings::StoreAssociation::LegacyLine {
                    reviewer: false,
                    ..
                }
            ) {
                prepared_cwd.map(
                    |cwd| crate::layr_native::settings::StoreAssociation::LegacyLine {
                        cwd: cwd.to_path_buf(),
                        reviewer: false,
                    },
                )
            } else {
                None
            };
            let root = identity
                .resolve(state_root)
                .map_err(|_| invalid("native-layr/store"))?;
            let environment = crate::layr_native::thread_environment_in_store(state_root, &root);
            let inputs = self
                .filesystem_inputs
                .as_mut()
                .ok_or_else(|| invalid("thread guidance filesystem inputs"))?;
            let reviewer = record["layrScratch"].as_str().map(|scratch| {
                (
                    scratch.to_owned(),
                    crate::layr_native::reviewer_environment(state_root, scratch),
                )
            });
            Arc::make_mut(inputs).native_layr = Some(Ok(NativeLayrFilesystem {
                identity,
                prepared_identity,
                is_reviewer: record["role"] == "reviewer",
                root,
                environment,
                state_root: state_root.to_path_buf(),
                reviewer,
            }));
        }
        Ok(self)
    }

    fn native_layr<'a>(
        &'a self,
        state_root: &Path,
        record: &Value,
    ) -> Result<&'a NativeLayrFilesystem, WorkerError> {
        let captured = self
            .filesystem_inputs
            .as_ref()
            .and_then(|inputs| inputs.native_layr.as_ref())
            .ok_or_else(|| invalid("native-layr/filesystem inputs"))?
            .as_ref()
            .map_err(|_| invalid("native-layr/store"))?;
        let identity = crate::layr_native::settings::store_association(record)
            .map_err(|_| invalid("native-layr/store"))?;
        if (captured.identity != identity && captured.prepared_identity.as_ref() != Some(&identity))
            || captured.state_root != state_root
            || captured.is_reviewer != (record["role"] == "reviewer")
        {
            return Err(self.native_layr_changed("native-layr/store-changed"));
        }
        Ok(captured)
    }

    fn native_layr_changed(&self, reason: &'static str) -> WorkerError {
        if self
            .filesystem_inputs
            .as_ref()
            .is_some_and(|inputs| inputs.receipt)
        {
            invalid(reason)
        } else {
            refusal(reason.into())
        }
    }

    pub(super) fn monitor_environment(
        &self,
        state_root: &Path,
        record: &Value,
    ) -> Result<Option<Map<String, Value>>, WorkerError> {
        if !crate::layr_native::is_native(record) {
            return Ok(None);
        }
        let captured = self.native_layr(state_root, record)?;
        if record["role"] == "reviewer" {
            let scratch = record["layrScratch"]
                .as_str()
                .ok_or_else(|| invalid("native-reviewer/scratch"))?;
            let (_, environment) = captured
                .reviewer
                .as_ref()
                .filter(|(stored, _)| stored == scratch)
                .ok_or_else(|| self.native_layr_changed("native-layr/scratch-changed"))?;
            return Ok(Some(environment.clone()));
        }
        Ok(Some(captured.environment.clone()))
    }

    pub(super) fn validate_native_layr_source(
        &self,
        state_root: &Path,
        record: &Value,
    ) -> Result<(), WorkerError> {
        if crate::layr_native::is_native(record) {
            let captured = self.native_layr(state_root, record)?;
            let identity = crate::layr_native::settings::store_association(record)
                .map_err(|_| invalid("native-layr/store"))?;
            if captured.identity != identity {
                return Err(refusal("native-layr/store-changed".into()));
            }
        }
        Ok(())
    }

    pub(crate) async fn for_sql(
        &self,
        state_root: &Path,
        record: &Value,
        prepared_cwd: Option<&Path>,
    ) -> Result<Self, WorkerError> {
        let resources = self.clone();
        let state_root = state_root.to_path_buf();
        let record = record.clone();
        let prepared_cwd = prepared_cwd.map(Path::to_path_buf);
        studio_runtime_process::blocking_pool::spawn_blocking(
            "runtime-services/thread-guidance/filesystem",
            move || {
                resources.capture_filesystem(None).capture_native_layr(
                    &state_root,
                    &record,
                    prepared_cwd.as_deref(),
                )
            },
        )
        .await
        .map_err(|_| WorkerError::TaskFailed)?
    }

    pub(crate) async fn for_receipt(
        &self,
        store: &RuntimeStore,
        agent_id: &str,
        prepared_cwd: Option<&str>,
    ) -> Result<Self, WorkerError> {
        let record = if prepared_cwd.is_some() {
            let agent_id = agent_id.to_owned();
            match store
                .read_async("runtime-services/thread-receipt/cwd", move |tx| {
                    Ok::<_, WorkerError>(
                        AgentRepository::get(tx, &AgentId::from(agent_id.as_str()))?
                            .map(|agent| agent.value()),
                    )
                })
                .await
            {
                Ok(record) => record,
                Err(error) if crate::account_worker::agent_fault::data_fault(&error).is_some() => {
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        let receipt_cwd = record
            .as_ref()
            .and_then(|record| record["cwd"].as_str())
            .zip(prepared_cwd)
            .map(|(stored, prepared)| (stored.to_owned(), prepared.to_owned()));
        let resources = self.clone();
        let state_root = store.root().to_path_buf();
        studio_runtime_process::blocking_pool::spawn_blocking(
            "runtime-services/thread-receipt/filesystem",
            move || {
                let mut resources = resources.capture_filesystem(receipt_cwd);
                if let Some(inputs) = resources.filesystem_inputs.as_mut() {
                    Arc::make_mut(inputs).receipt = true;
                }
                Ok(match record {
                    Some(record) => {
                        match resources
                            .clone()
                            .capture_native_layr(&state_root, &record, None)
                        {
                            Ok(captured) => captured,
                            Err(_) => {
                                let mut resources = resources;
                                if let Some(inputs) = resources.filesystem_inputs.as_mut() {
                                    Arc::make_mut(inputs).native_layr = Some(Err(()));
                                }
                                resources
                            }
                        }
                    }
                    None => resources,
                })
            },
        )
        .await
        .map_err(|_| WorkerError::TaskFailed)?
    }

    pub fn with_account_session(
        mut self,
        account_session: Option<Arc<dyn crate::account_session_runtime::AccountSessionPort>>,
    ) -> Self {
        self.account_session = account_session;
        self
    }
}
pub(super) fn decode<T: DeserializeOwned>(value: Value, location: &str) -> Result<T, WorkerError> {
    studio_json::from_value(value).map_err(|_| invalid(location))
}
pub(super) fn invalid(location: &str) -> WorkerError {
    StoreError::InvalidStoredObject {
        table: "turn_preparation",
        id: location.into(),
    }
    .into()
}
pub(super) fn agent(tx: &Transaction<'_>, id: &str) -> Result<AgentRecord, WorkerError> {
    AgentRepository::get(tx, &AgentId::from(id))?.ok_or_else(|| invalid(id))
}
fn table_exists(tx: &rusqlite::Connection, table: &str) -> Result<bool, WorkerError> {
    Ok(tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name=?1 AND type='table')",
        [table],
        |r| r.get::<_, bool>(0),
    )?)
}
/// One `id,record` row of an optional feature table.
fn record_by_id(
    tx: &rusqlite::Connection,
    table: &'static str,
    id: &str,
) -> Result<Option<String>, WorkerError> {
    use rusqlite::OptionalExtension;
    if !table_exists(tx, table)? {
        return Ok(None);
    }
    Ok(tx
        .prepare_cached(&format!("SELECT record FROM {table} WHERE id=?1"))?
        .query_row([id], |r| r.get(0))
        .optional()?)
}
/// Complaints addressed to `lead`, in id order, through the
/// `runtime_conversion_complaints_leadId` index.
const LEAD_COMPLAINTS_SQL: &str =
    "SELECT record FROM runtime_complaints WHERE json_extract(record,'$.leadId')=?1 ORDER BY id";
fn lead_complaints(tx: &rusqlite::Connection, lead: &str) -> Result<Vec<String>, WorkerError> {
    if !table_exists(tx, "runtime_complaints")? {
        return Ok(Vec::new());
    }
    Ok(tx
        .prepare_cached(LEAD_COMPLAINTS_SQL)?
        .query_map([lead], |r| r.get(0))?
        .collect::<Result<_, _>>()?)
}
/// Stored rows for exactly `ids`, keyed by id; absent ids are skipped.
pub(super) fn records_by_ids(
    db: &rusqlite::Connection,
    table: &str,
    ids: &std::collections::BTreeSet<String>,
) -> Result<BTreeMap<String, String>, WorkerError> {
    let mut rows = BTreeMap::new();
    if ids.is_empty()
        || !db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name=?1 AND type='table')",
            [table],
            |r| r.get::<_, bool>(0),
        )?
    {
        return Ok(rows);
    }
    let mut query = db.prepare_cached(&format!("SELECT record FROM {table} WHERE id=?1"))?;
    for id in ids {
        if let Some(raw) = query
            .query_row([id], |r| r.get::<_, String>(0))
            .optional()?
        {
            let raw = if table == studio_runtime_store::agent_volatile::AGENT_TABLE {
                studio_runtime_store::agent_volatile::merged_text(db, id, raw)?
            } else {
                raw
            };
            rows.insert(id.clone(), raw);
        }
    }
    Ok(rows)
}

/// The events authorization checks: radio turns are checked by the radio
/// service instead when the intent carries that authority.
fn authorization_events(intent: &TurnIntent) -> Vec<crate::submit_gates::StoredEvent> {
    intent
        .events
        .iter()
        .filter(|event| !(intent.radio_authorized && event.kind == "radio_turn"))
        .map(|e| crate::submit_gates::StoredEvent {
            id: e.id.clone(),
            kind: e.kind.clone(),
            text: e.text.clone(),
        })
        .collect()
}

/// The stored rows one authorization decision reads, without the resolved
/// paths: the recipient, senders, rooms, complaints and the follow-up
/// metadata the events name, plus the few projects. Whole agent, room,
/// complaint and event-metadata tables are never read.
fn authorization_inputs(
    db: &rusqlite::Connection,
    intent: &TurnIntent,
) -> Result<crate::submit_gates::AuthorizationRecords, WorkerError> {
    event_authorization_inputs(db, &intent.agent_id, authorization_events(intent))
}

/// [`authorization_inputs`] of `events` for their recipient `agent_id`.
pub(in crate::account_worker) fn event_authorization_inputs(
    db: &rusqlite::Connection,
    agent_id: &str,
    events: Vec<crate::submit_gates::StoredEvent>,
) -> Result<crate::submit_gates::AuthorizationRecords, WorkerError> {
    let meta_ids = crate::submit_gates::followup_event_ids(&events)
        .map(str::to_owned)
        .collect();
    let event_meta = records_by_ids(db, "runtime_event_meta", &meta_ids)?;
    let mut scope =
        crate::submit_gates::AuthorizationScope::of_events(agent_id, &events, &event_meta);
    let rooms = records_by_ids(db, "runtime_rooms", &scope.rooms)?;
    scope.add_private_members(&rooms);
    Ok(crate::submit_gates::AuthorizationRecords {
        recipient_id: agent_id.to_owned(),
        agents: crate::submit_gates::decode_authority_rows(records_by_ids(
            db,
            "runtime_agents",
            &scope.agents,
        )?),
        projects: crate::submit_gates::decode_authority_rows(
            studio_runtime_store::ProjectsRepository::owned(db)?
                .into_iter()
                .map(|(id, project)| (id, project.to_string()))
                .collect(),
        ),
        resolved_paths: BTreeMap::new(),
        rooms,
        complaints: records_by_ids(db, "runtime_complaints", &scope.complaints)?,
        event_meta,
        events,
    })
}

/// Read the rows whose paths [`FilesystemSnapshot::capture_for_authorization`] resolves.
pub(super) async fn authorization_rows(
    store: &RuntimeStore,
    intent: &TurnIntent,
) -> Result<crate::submit_gates::AuthorizationRecords, WorkerError> {
    let intent = intent.clone();
    store
        .read_async(
            "runtime-services/account_worker_authorization_rows",
            move |tx| authorization_inputs(tx, &intent),
        )
        .await
}

/// The resolved paths that one authorization decision over `rows` reads:
/// every agent cwd and project path. Run it before SQL.
pub(in crate::account_worker) fn authorization_paths(
    rows: &crate::submit_gates::AuthorizationRecords,
) -> Result<BTreeMap<String, String>, WorkerError> {
    let mut resolved_paths = BTreeMap::new();
    for row in rows.agents.values() {
        let row = row.as_ref().map_err(|_| invalid("cwd"))?;
        if let Some(path) = row.cwd.as_deref().filter(|path| !path.is_empty()) {
            let resolved = CanonicalPath::ancestor_nonstrict(Path::new(path))
                .map_err(|_| invalid("cwd"))?
                .to_string_lossy()
                .into_owned();
            resolved_paths.insert(path.to_owned(), resolved);
        }
    }
    for row in rows.projects.values() {
        let row = row.as_ref().map_err(|_| invalid("path"))?;
        if let crate::submit_gates::ProjectAuthorityRow::Active { path, .. } = row
            && !path.is_empty()
        {
            let resolved =
                CanonicalPath::ancestor_nonstrict(Path::new(path)).map_err(|_| invalid("path"))?;
            resolved_paths.insert(path.to_owned(), resolved.to_string_lossy().into_owned());
        }
    }
    Ok(resolved_paths)
}

/// Filesystem observations are captured on the preparation executor, before SQL.
/// Authorization still validates current stored owners in its writer transaction.
#[derive(Clone)]
pub(super) struct FilesystemSnapshot {
    resolved_paths: BTreeMap<String, String>,
}

impl FilesystemSnapshot {
    /// Capture the observations of one admission: the resolved paths of the
    /// rows its authorization decision reads. `rows` come from
    /// [`authorization_rows`].
    pub fn capture_for_authorization(
        rows: &crate::submit_gates::AuthorizationRecords,
    ) -> Result<Self, WorkerError> {
        Ok(Self {
            resolved_paths: authorization_paths(rows)?,
        })
    }
}

pub(super) fn authorize(
    tx: &Transaction<'_>,
    intent: &TurnIntent,
    filesystem: &FilesystemSnapshot,
    prepared_host: Option<&crate::host_project_runtime::PreparedHost>,
) -> Result<(), WorkerError> {
    if intent.radio_authorized {
        let prepared = prepared_host.ok_or_else(|| invalid("radio/prepared-host"))?;
        let radio_events = intent
            .events
            .iter()
            .filter(|event| event.kind == "radio_turn")
            .map(|event| {
                studio_runtime_store::QueueEventRepository::get(
                    tx,
                    &studio_runtime_store::QueueEventId::from(event.id.as_str()),
                )?
                .ok_or_else(|| invalid("radio/event"))
            })
            .collect::<Result<Vec<_>, WorkerError>>()?;
        if radio_events.len() != 1 {
            return Err(refusal("radio/turn-selection-changed".into()));
        }
        let selected = crate::radio_runtime::select_pending(
            tx,
            prepared,
            &agent(tx, &intent.agent_id)?.value(),
            &radio_events,
            now(),
        )
        .map_err(|error| refusal(error.to_string()))?;
        if selected.len() != 1 || selected[0].id.0 != radio_events[0].id.0 {
            return Err(refusal("radio/turn-selection-changed".into()));
        }
    }
    let mut rows = authorization_inputs(tx, intent)?;
    rows.resolved_paths = filesystem.resolved_paths.clone();
    let checked = crate::submit_gates::AuthorizationSnapshot::from_records(&rows)
        .map_err(|e| refusal(e.to_string()))?;
    crate::submit_gates::authorize_events(&checked).map_err(|e| refusal(e.to_string()))
}
fn optional<T: DeserializeOwned>(record: &Value, key: &str) -> Result<Option<T>, WorkerError> {
    record
        .get(key)
        .filter(|v| !v.is_null())
        .cloned()
        .map(|v| decode(v, key))
        .transpose()
}
fn flag(record: &Value, key: &str) -> Result<bool, WorkerError> {
    Ok(optional(record, key)?.unwrap_or(false))
}
fn text(record: &Value, key: &str) -> Result<Option<String>, WorkerError> {
    optional(record, key)
}
// Only text can match a native catalog notice. Structured provider errors
// remain in the stored agent and do not match that notice.
fn catalog_notice_error(record: &Value) -> Result<Option<String>, WorkerError> {
    match record.get("error") {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        None | Some(Value::Null | Value::Object(_)) => Ok(None),
        Some(_) => Err(invalid("error")),
    }
}
fn stored<T: DeserializeOwned>(
    record: &Value,
    key: &str,
) -> Result<thread::Stored<Option<T>>, WorkerError> {
    Ok(if record.get(key).is_some() {
        thread::Stored::Present(optional(record, key)?)
    } else {
        thread::Stored::Absent
    })
}
pub(super) fn guidance(
    resources: &PreparationResources,
    record: &Value,
    state_root: &Path,
) -> Result<thread::GuidanceBundle, WorkerError> {
    let role = role_guidance(resources, record)?;
    let mut thread_config = resources.thread_config.clone();
    // A managed host worker runs without the host account's user MCP servers.
    // (Claude workers get the same rule from the adapter's `userMcp` param.)
    if record["parentId"].is_string() && ProviderKind::of_record(record) == Ok(ProviderKind::Codex)
    {
        let servers: Map<String, Value> = resources
            .filesystem_inputs
            .as_ref()
            .map(|inputs| inputs.mcp_servers.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|name| (name, studio_json::json!({"enabled": false})))
            .collect();
        if !servers.is_empty() {
            thread_config.insert("mcp_servers".into(), Value::Object(servers));
        }
    }
    if !resources.account_home.is_empty() && resources.filesystem_inputs.is_none() {
        return Err(invalid("thread guidance filesystem inputs"));
    }
    let browser = browser_guidance(resources, record, &mut thread_config);
    // Python `codex_browser_host.thread_environment`, at the same call site as
    // `configure_browser`: host Codex threads send their sign-in links to Studio.
    if ProviderKind::of_record(record) == Ok(ProviderKind::Codex)
        && let Some(agent_id) = record["id"].as_str()
    {
        thread_config.extend(
            crate::browser_environment::thread_environment(state_root, agent_id)
                .into_iter()
                .map(|(key, value)| (key, Value::String(value))),
        );
    }
    if crate::layr_native::is_native(record) {
        for (name, value) in resources
            .native_layr(state_root, record)?
            .environment
            .clone()
        {
            thread_config.insert(format!("shell_environment_policy.set.{name}"), value);
        }
        if record["role"] == "reviewer"
            && let Some(scratch) = record["layrScratch"].as_str()
        {
            thread_config.insert("shell_environment_policy.set.TMPDIR".into(), json!(scratch));
        }
        thread_config.insert(
            "shell_environment_policy.exclude".into(),
            json!(["LAYR_SOCKET", "LAYR_PROJECT", "LAYR_LINE", "LAYR_SESSION"]),
        );
    }
    Ok(thread::GuidanceBundle {
        instructions: resources.instructions.clone(),
        role,
        thread_config,
        browser,
    })
}

/// Python `codex_browser.configure_browser`: every host Codex agent (lead,
/// worker or reviewer) gets the account's native Chrome setup; Claude agents get none.
fn browser_guidance(
    resources: &PreparationResources,
    record: &Value,
    thread_config: &mut Map<String, Value>,
) -> Option<thread::BrowserGuidance> {
    if ProviderKind::of_record(record) != Ok(ProviderKind::Codex)
        || resources.account_home.is_empty()
        || resources.shared_home.is_empty()
    {
        return None;
    }
    let browser = resources.filesystem_inputs.as_ref()?.browser.clone()?;
    crate::browser::report_browser_status(
        record["accountKey"].as_str().unwrap_or("default"),
        browser.unavailable_reason.as_deref(),
    );
    for (key, value) in browser.overrides {
        // The order in which Codex applies these keys is not part of its
        // contract, and a whole `mcp_servers` table applied after
        // `mcp_servers.node_repl` could replace it. A worker's table of
        // disabled servers therefore carries the browser server itself, which
        // also replaces its own disabled entry.
        match (
            key.strip_prefix("mcp_servers."),
            thread_config
                .get_mut("mcp_servers")
                .and_then(Value::as_object_mut),
        ) {
            (Some(server), Some(servers)) => {
                servers.insert(server.to_owned(), value);
            }
            _ => {
                thread_config.insert(key, value);
            }
        }
    }
    Some(thread::BrowserGuidance {
        instructions: browser.developer_instructions.map(str::to_owned),
        skill_roots: browser.skill_roots,
    })
}

/// The role skill text of `record`. A turn needs only this; the thread config
/// (with its account MCP read) is the thread snapshot's.
fn role_guidance(resources: &PreparationResources, record: &Value) -> Result<String, WorkerError> {
    let lead = flag(record, "isLead")?;
    let name = if lead {
        "codex-orchestrator"
    } else {
        "codex-subagent"
    };
    let path = resources.skills_root.join(name).join("SKILL.md");
    let role = if let Some(role) = text(record, "nativeRoleGuidance")?.filter(|s| !s.is_empty()) {
        role
    } else {
        let content_path = if record["threadId"].is_string() {
            path.parent()
                .ok_or_else(|| invalid("role path"))?
                .join("references")
                .join(
                    if lead && record["nativeTeleportTool"] != "orchestration_teleport" {
                        "legacy-move-role.md"
                    } else {
                        "native-role.md"
                    },
                )
        } else {
            path.clone()
        };
        let relative = content_path
            .strip_prefix(&resources.skills_root)
            .map_err(|_| invalid("guidance path"))?;
        let content = resources
            .role_guidance
            .get(relative)
            .ok_or_else(|| refusal("thread-preparation/missing-guidance".into()))?;
        if content.trim().is_empty() {
            return Err(refusal("thread-preparation/missing-guidance".into()));
        }
        let content = content.trim().to_owned();
        let shown_root = resources.skills_root.as_path();
        format!(
            "[Studio role skill: {name}]\nSource: {}\nShared tool guidance: {}\n{}\n[End Studio role skill]",
            shown_root.join(name).join("SKILL.md").display(),
            shown_root.join("codex-workspace/SKILL.md").display(),
            content
        )
    };
    Ok(role)
}

/// The catalog an agent's thread uses: its saved source, else legacy for an
/// agent that already has a native thread.
fn catalog_source(record: &Value) -> Result<thread::CatalogSource, WorkerError> {
    Ok(
        optional(record, "nativeToolSource")?.unwrap_or(if record["threadId"].is_string() {
            thread::CatalogSource::Legacy
        } else {
            thread::CatalogSource::Current
        }),
    )
}

/// Providers that resend tools each turn use current definitions.
/// Other agents retain their saved catalog source. Callers apply frozen overrides.
fn catalog_definitions<'a>(
    resources: &'a PreparationResources,
    record: &Value,
) -> Result<&'a Vec<thread::ToolDefinition>, WorkerError> {
    Ok(
        if catalog_source(record)? == thread::CatalogSource::Legacy
            && !crate::provider_adapter::adapter(
                ProviderKind::of_record(record).map_err(|_| invalid("agent/provider"))?,
            )
            .capabilities()
            .tools_on_every_turn
        {
            &resources.legacy_tools
        } else {
            &resources.tools
        },
    )
}

/// The thread role of an agent record: its saved role, else the role of a
/// lead, a native review child, or a worker.
fn role(record: &Value) -> Result<thread::Role, WorkerError> {
    let native_review = studio_runtime_store::truthy(record.get("nativeReview"));
    Ok(optional(record, "role")?.unwrap_or(thread::Role::inferred(
        native_review,
        !native_review && flag(record, "isLead")?,
    )))
}

fn federation_enabled(tx: &Transaction<'_>) -> Result<bool, WorkerError> {
    Ok(record_by_id(tx, "runtime_federation_settings", "global")?
        .map(|raw| studio_json::from_str::<Value>(&raw).map_err(|_| invalid("federation settings")))
        .transpose()?
        .is_some_and(|v| v["enabled"] == true))
}

/// The tool definitions of one agent, as Python's
/// `Runtime.tool_definitions(actor)` reports them to `orchestration_context`.
pub(crate) fn actor_tool_definitions(
    tx: &Transaction<'_>,
    resources: &PreparationResources,
    record: &Value,
) -> Result<Value, WorkerError> {
    let root_id = record["rootId"]
        .as_str()
        .ok_or_else(|| invalid("agent/rootId"))?;
    let root = agent(tx, root_id)?.value();
    let delegation = can_delegate(tx, record, &root)?;
    if let Some(frozen) = record.get("frozenNativeParams").filter(|v| !v.is_null()) {
        let mut definitions = frozen["dynamicTools"]
            .as_array()
            .cloned()
            .ok_or_else(|| invalid("agent/frozenNativeParams"))?;
        if !delegation {
            definitions.retain(|tool| tool["name"] != "orchestration_spawn");
        }
        return Ok(Value::Array(definitions));
    }
    let definitions = catalog_definitions(resources, record)?;
    let cwd = text(record, "cwd")?.unwrap_or_default();
    let filtered = thread::filter_tool_definitions(
        definitions,
        &thread::CatalogFilter {
            provider: studio_runtime_providers::ProviderKind::of_record(record)
                .map_err(|_| invalid("agent/provider"))?,
            role: role(record)?,
            assignment: optional::<studio_runtime_schema::review::ReviewAssignment>(
                record,
                "reviewAssignment",
            )?
            .as_ref(),
            is_lead: flag(record, "isLead")?,
            can_delegate: delegation,
            federation_enabled: federation_enabled(tx)?,
            cwd: &cwd,
            session_tool: thread::session_tool_of(
                optional(record, "nativeTeleportTool")?,
                text(record, "threadId")?.is_some(),
            ),
        },
    )
    .map_err(|_| invalid("tool catalog"))?;
    studio_json::to_value(filtered).map_err(|_| invalid("tool catalog"))
}

pub(super) fn native_thread_blocked(record: &Value) -> bool {
    let Some(thread) = record["threadId"].as_str().filter(|s| !s.is_empty()) else {
        return false;
    };
    record["nativeThreadBlock"]["threadId"] == thread
        || is_misalignment_policy_violation(&record["error"])
        || (nonempty(&record["nativeTurnError"]["turnId"])
            && record["nativeTurnError"]["turnId"] == record["turnId"]
            && is_misalignment_policy_violation(&record["nativeTurnError"]["error"]))
}
pub(super) fn context_repair_blocked(record: &Value) -> bool {
    nonempty(&record["contextRepairWait"])
        || matches!(
            record["contextRepair"]["phase"].as_str(),
            Some("preparing" | "submitted" | "unknown" | "ready")
        )
}
/// Whether a saved native release keeps this agent's queued input out of
/// thread preparation. Python `Runtime.prepare_locked` refuses a pending tool
/// reset (`resetPending`), then `codex_native_release.reconcile_unknown` acts
/// only on an unsettled `unknown`, `checking` or `unsubscribing` release: a
/// captured reset waits for exact native closure, and an unsettled release on
/// this thread and connection retries `thread/unsubscribe`. This adapter has no
/// native release driver, so it holds in both cases. Every other release
/// prepares normally, including `blocked`, where the inspection found native
/// work and never unsubscribed, and an unsettled release on an earlier
/// connection, which Python also ignores.
pub(super) fn native_release_held(record: &Value, connection: &str) -> bool {
    let release = &record["nativeRelease"];
    if nonempty(&release["resetPending"]) {
        return true;
    }
    let phase = release["phase"].as_str();
    if !matches!(phase, Some("unknown" | "checking" | "unsubscribing")) {
        return false;
    }
    if release.get("resetActorEpoch").is_some() {
        return true;
    }
    if phase == Some("checking") && nonempty(&release["inspectionPhase"]) {
        return false;
    }
    release["threadId"] == record["threadId"]
        && release["connectionId"].as_str() == Some(connection)
}
pub(super) fn nonempty(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Object(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::String(value) => !value.is_empty(),
        Value::Number(value) => value.as_f64() != Some(0.0),
    }
}
fn preserve_complaint(
    tx: &Transaction<'_>,
    agent: &str,
    event: &crate::scheduler::TurnEvent,
) -> Result<bool, WorkerError> {
    if event.kind != "complaint" {
        return Ok(false);
    }
    let Some(entries) = studio_runtime_schema::notification_text::complaint_entries(&event.text)
    else {
        return Ok(true);
    };
    for entry in entries {
        let Some(id) = entry else {
            return Ok(true);
        };
        use rusqlite::OptionalExtension;
        let raw: Option<String> = tx
            .query_row(
                "SELECT record FROM runtime_complaints WHERE id=?1",
                [&id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(raw) = raw else {
            return Ok(true);
        };
        let complaint: studio_runtime_schema::complaint_record::ComplaintRecord =
            studio_json::from_str(&raw).map_err(|_| invalid("complaint"))?;
        if complaint.lead_id() != Some(agent) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Use ready workspace facts, not the lead's inherited workspace preference.
fn host_worktree(
    record: &Value,
    root: &Value,
    role: thread::Role,
    lead: bool,
) -> Result<Option<thread::HostWorktree>, WorkerError> {
    if record["provider"] != ProviderKind::Claude.key()
        || lead
        || role != thread::Role::Implementer
        || record["parentId"].as_str().is_none()
        || !((record["worktree"] == true && record["worktreeReady"] == true)
            || (crate::layr_native::is_native(record) && record["layrReady"] == true))
    {
        return Ok(None);
    }
    let tree = text(record, "cwd")?.ok_or_else(|| invalid("agent/cwd"))?;
    if !Path::new(&tree).is_absolute() {
        return Err(invalid("agent/cwd"));
    }
    // hostProjectPath preserves the source identity. The root paths protect nested workers.
    let mut fence = Vec::new();
    for value in [
        &record["hostProjectPath"],
        &root["cwd"],
        &root["hostProjectPath"],
    ] {
        if let Some(path) = value.as_str().filter(|path| !path.is_empty()) {
            if !Path::new(path).is_absolute() {
                return Err(invalid("agent/workspace-fence"));
            }
            if path != tree && !fence.iter().any(|entry| entry == path) {
                fence.push(path.to_owned());
            }
        }
    }
    if fence.is_empty() {
        return Err(invalid("agent/workspace-fence"));
    }
    Ok(Some(thread::HostWorktree { tree, fence }))
}

/// Python `frozenNativeParams` of a moved agent. Python reads the three saved
/// fields and the tools by key, so a missing one is an invalid stored agent.
fn frozen_params(record: &Value) -> Result<Option<thread::FrozenParams>, WorkerError> {
    let Some(frozen) = record.get("frozenNativeParams").filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let field = |key: &str| {
        frozen
            .get(key)
            .cloned()
            .ok_or_else(|| invalid("agent/frozenNativeParams"))
    };
    let dynamic_tools = studio_json::from_value(field("dynamicTools")?)
        .map_err(|_| invalid("agent/frozenNativeParams"))?;
    Ok(Some(thread::FrozenParams {
        base_instructions: field("baseInstructions")?,
        developer_instructions: field("developerInstructions")?,
        model: field("model")?,
        dynamic_tools,
        effort: frozen.get("effort").cloned(),
        summary: frozen.get("summary").cloned(),
    }))
}

pub(super) fn native_claude_options(
    resources: &PreparationResources,
    record: &Value,
    state_root: &Path,
) -> Result<studio_json::Map<String, Value>, WorkerError> {
    let mut options: studio_json::Map<String, Value> =
        optional(record, "claudeOptions")?.unwrap_or_default();
    if let Some(mode) = record.get("permissionMode").filter(|mode| !mode.is_null()) {
        options.insert("permissionMode".into(), mode.clone());
    }
    let assigned = record
        .get("reviewAssignment")
        .is_some_and(|value| !value.is_null());
    if record["role"] == "reviewer" && (crate::layr_native::is_native(record) || assigned) {
        let agent = if assigned {
            Some(
                studio_runtime_store::AgentRecord::from_value(record.clone())
                    .map_err(|_| invalid("reviewer"))?,
            )
        } else {
            None
        };
        let scratch = match &agent {
            Some(agent) => crate::reviewer_workspace::scratch_dir(agent),
            None => record["layrScratch"].as_str(),
        }
        .ok_or_else(|| invalid("native-reviewer/scratch"))?;
        let mut deny = Vec::new();
        if assigned {
            deny.push(
                crate::reviewer_workspace::read_only_copy(
                    agent.as_ref().ok_or_else(|| invalid("reviewer"))?,
                )
                .ok_or_else(|| invalid("review workspace"))?
                .to_owned(),
            );
        }
        let environment = if crate::layr_native::is_native(record) {
            let captured = resources.native_layr(state_root, record)?;
            deny.push(captured.root.to_string_lossy().into_owned());
            captured
                .reviewer
                .as_ref()
                .filter(|(stored, _)| stored == scratch)
                .ok_or_else(|| resources.native_layr_changed("native-layr/scratch-changed"))?
                .1
                .clone()
        } else {
            crate::layr_native::reviewer_environment(state_root, scratch)
        };
        options.insert("sandbox".into(),json!({"enabled":true,"failIfUnavailable":true,"allowUnsandboxedCommands":false,
            "filesystem":{"allowWrite":[scratch],"denyWrite":deny},
            "network":{"allowedDomains":[],"strictAllowlist":true,"allowUnixSockets":[],"allowAllUnixSockets":false}}));
        options.insert(
            "nativeReviewerEnvironment".into(),
            Value::Object(environment),
        );
    }
    Ok(options)
}

pub(super) fn thread_snapshot(
    tx: &Transaction<'_>,
    resources: &PreparationResources,
    record: &Value,
    connection: &str,
    state_root: &Path,
    account_transfer_owner: Option<&str>,
) -> Result<thread::Snapshot, WorkerError> {
    let mut result = base_thread_snapshot(
        tx,
        resources,
        record,
        connection,
        state_root,
        account_transfer_owner,
    )?;
    if record
        .get("reviewAssignment")
        .is_some_and(|value| !value.is_null())
    {
        let agent = studio_runtime_store::AgentRecord::from_value(record.clone())
            .map_err(|_| invalid("review assignment"))?;
        let assignment = agent
            .review_assignment
            .as_ref()
            .ok_or_else(|| invalid("review assignment"))?;
        let copy = crate::reviewer_workspace::read_only_copy(&agent)
            .ok_or_else(|| invalid("review workspace"))?;
        let scratch = crate::reviewer_workspace::scratch_dir(&agent)
            .ok_or_else(|| invalid("review scratch"))?;
        result.actor.cwd = scratch.to_owned();
        result.actor.thread_sandbox = thread::ThreadSandbox::WorkspaceWrite;
        result
            .guidance
            .thread_config
            .insert("shell_environment_policy.set.TMPDIR".into(), json!(scratch));
        let instruction = match assignment.target() {
            studio_runtime_schema::review::ReviewTarget::LayrState { state, .. } => {
                format!(
                    "Your read-only copy of {state} is at {copy}; your working directory is a scratch folder."
                )
            }
            studio_runtime_schema::review::ReviewTarget::GitRange { head, .. } => {
                format!(
                    "Your read-only copy of {head} is at {copy}; your working directory is a scratch folder."
                )
            }
        };
        result.guidance.instructions.push('\n');
        result.guidance.instructions.push_str(&instruction);
    }
    Ok(result)
}

fn base_thread_snapshot(
    tx: &Transaction<'_>,
    resources: &PreparationResources,
    record: &Value,
    connection: &str,
    state_root: &Path,
    account_transfer_owner: Option<&str>,
) -> Result<thread::Snapshot, WorkerError> {
    if crate::workspace_ready::vm_workspace_unsupported(
        record["executionMode"].as_str(),
        record["workspaceMode"].as_str(),
        record["layrReady"] == true,
        record["layrLine"].as_str().is_some(),
    ) {
        return Err(refusal(
            crate::workspace_ready::WorkspaceRefusal::VmWorkspaceUnsupported.to_string(),
        ));
    }
    let id = record["id"].as_str().ok_or_else(|| invalid("agent/id"))?;
    let root_id = record["rootId"]
        .as_str()
        .ok_or_else(|| invalid("agent/rootId"))?;
    let root = agent(tx, root_id)?.value();
    let source = catalog_source(record)?;
    let definitions = catalog_definitions(resources, record)?.clone();
    let lead = flag(record, "isLead")?;
    let role = role(record)?;
    let owned_transfer = account_transfer_owner
        .map(|transfer_id| {
            crate::account_transfer_driver::owns_reading_member(tx, transfer_id, id)
                .map_err(|error| refusal(format!("account-transfer/{error}")))
        })
        .transpose()?
        .unwrap_or(false);
    let admission = if record
        .get("accountTransferId")
        .is_some_and(|v| !v.is_null())
        && !owned_transfer
    {
        Some(thread::HoldReason::AccountTransfer)
    } else if context_repair_blocked(record) {
        Some(thread::HoldReason::ContextRepair)
    } else if native_release_held(record, connection) {
        Some(thread::HoldReason::NativeRelease)
    } else {
        None
    };
    let frozen = frozen_params(record)?;
    let unsupported = if record.get("nativeToolUpdate").is_some_and(|v| !v.is_null()) {
        Some(thread::Refusal::NativeCatalogMutation)
    } else {
        None
    };
    Ok(thread::Snapshot {
        actor: thread::Actor {
            id: id.into(),
            epoch: decode(record["epoch"].clone(), "agent/epoch")?,
            thread_id: text(record, "threadId")?,
            cwd: text(record, "cwd")?.ok_or_else(|| invalid("agent/cwd"))?,
            thread_sandbox: thread::ThreadSandbox::ReadOnly,
            compactions: optional(record, "compactions")?.unwrap_or(0),
            prepare_attempt: text(record, "prepareAttempt")?,
            native_teleport_tool: optional(record, "nativeTeleportTool")?,
            native_role_guidance: text(record, "nativeRoleGuidance")?,
            native_tool_source: optional(record, "nativeToolSource")?,
            native_tool_update: optional(record, "nativeToolUpdate")?,
            error: catalog_notice_error(record)?,
            native_release: optional(record, "nativeRelease")?,
            root_id: root_id.into(),
            parent_id: text(record, "parentId")?,
            auto_wake: flag(record, "autoWake")?,
            native_effort_present: record.get("nativeEffort").is_some(),
            deleted: record.get("deletedAt").is_some_and(|v| !v.is_null()),
            native_review: record["nativeReview"]["status"] == "pending",
            review_assignment: optional(record, "reviewAssignment")?,
        },
        account: thread::Account {
            key: text(record, "accountKey")?.unwrap_or_else(|| "default".into()),
            provider: studio_runtime_providers::ProviderKind::of_record(record)
                .map_err(|_| invalid("agent/provider"))?,
            connection_id: connection.into(),
            available: true,
            supervisor_identity: None,
        },
        root: thread::Root {
            can_delegate: can_delegate(tx, record, &root)?,
            federation_enabled: federation_enabled(tx)?,
        },
        settings: thread::Settings {
            model: text(record, "model")?,
            effort: text(record, "effort")?,
            native_effort: text(record, "nativeEffort")?,
            fast_mode: flag(record, "fastMode")?,
            yolo_mode: text(record, "permissionMode")?.map(|mode| mode == studio_runtime_schema::claude_permissions::PermissionMode::BypassPermissions.as_str()),
            profile_instructions: text(record, "profileInstructions")?.unwrap_or_default(),
            role,
            is_lead: lead,
            needs_title: record["naming"] == "pending_title",
            claude_options: native_claude_options(resources, record, state_root)?,
            daybreak_enabled: stored(record, "daybreakEnabled")?,
            cyber_access_program: stored(record, "cyberAccessProgram")?,
        },
        guidance: guidance(resources, record, state_root)?,
        catalog: thread::CatalogSnapshot {
            source,
            definitions,
            saved_thread_id: record
                .get("nativeToolCatalog")
                .and_then(|v| v["threadId"].as_str())
                .map(str::to_owned),
            saved_digest: record
                .get("nativeToolCatalog")
                .and_then(|v| v["digest"].as_str())
                .map(str::to_owned),
        },
        workspace: thread::WorkspaceReadiness::Ready,
        host_worktree: host_worktree(record, &root, role, lead)?,
        unsupported,
        admission,
        pending: None,
        loaded: None,
        closed: false,
        frozen,
    })
}

/// Give a receipt-time snapshot the cwd text of its attempt when the stored
/// cwd names the same folder. Preparation sends the canonical cwd of the
/// validated workspace, and the stored cwd can name that folder through a link
/// (macOS `/var` and `/tmp`). The pure receipt reducer compares text, so
/// without this step a valid receipt is stale. A cwd that names another
/// folder stays as stored, and the reducer refuses it.
pub(super) fn align_receipt_cwd(
    current: &mut thread::Snapshot,
    attempt: &thread::Snapshot,
    resources: &PreparationResources,
) {
    if let Some((stored, prepared, same)) = resources
        .filesystem_inputs
        .as_ref()
        .and_then(|inputs| inputs.receipt_cwd.as_ref())
        && *same
        && current.actor.cwd == *stored
        && attempt.actor.cwd == *prepared
    {
        current.actor.cwd.clone_from(&attempt.actor.cwd);
    }
}

pub(super) fn submission_snapshot(
    tx: &Transaction<'_>,
    resources: &PreparationResources,
    record: &Value,
    intent: &TurnIntent,
) -> Result<(turn::PreparationSnapshot, Vec<turn::Event>), WorkerError> {
    let root_id = record["rootId"]
        .as_str()
        .ok_or_else(|| invalid("agent/rootId"))?;
    let root = agent(tx, root_id)?.value();
    let role = role_guidance(resources, record)?;
    let plan=record_by_id(tx, "runtime_plans", root_id)?.map(|s|studio_json::from_str::<Value>(&s).map_err(|_|invalid("plan"))).transpose()?.filter(|v|v["native"].is_object()).map(|v|decode(json!({"id":v["id"],"rootId":v["rootId"],"steps":v["native"].get("plan").cloned().unwrap_or_else(||json!([])),"explanation":v["native"].get("explanation").filter(|v|!v.is_null()).cloned().unwrap_or(json!(""))}),"plan")).transpose()?;
    let mut complaints = Vec::new();
    for raw in lead_complaints(tx, &intent.agent_id)? {
        use studio_runtime_schema::complaint_record::{
            ComplaintRecipient, ComplaintRecord, ComplaintSource, ComplaintStatus,
        };
        let complaint: ComplaintRecord =
            studio_json::from_str(&raw).map_err(|_| invalid("complaint"))?;
        if complaint.lead_id() != Some(intent.agent_id.as_str())
            || complaint.recipient != ComplaintRecipient::Lead
        {
            continue;
        }
        let status = match complaint.status {
            ComplaintStatus::Open => turn::ComplaintStatus::Open,
            ComplaintStatus::InProgress => turn::ComplaintStatus::InProgress,
            ComplaintStatus::Resolved | ComplaintStatus::Declined => continue,
        };
        let authors: Vec<String> = complaint
            .responses
            .into_iter()
            .map(|response| response.author)
            .collect();
        let legacy_user_task = complaint.source_type == Some(ComplaintSource::UserTask);
        if !(legacy_user_task && complaint.status == ComplaintStatus::Open)
            && authors.contains(&intent.agent_id)
        {
            continue;
        }
        let author = complaint
            .author
            .ok_or_else(|| invalid("complaint/author"))?;
        let author_name =
            AgentRepository::get(tx, &AgentId::from(author.as_str()))?.and_then(|agent| agent.name);
        complaints.push(turn::Complaint {
            id: complaint.id,
            lead_id: intent.agent_id.clone(),
            recipient: turn::Recipient::Lead,
            status,
            legacy_user_task,
            response_authors: authors,
            author,
            author_name,
            text: complaint.text.ok_or_else(|| invalid("complaint/text"))?,
            version: complaint
                .version
                .map(u64::try_from)
                .transpose()
                .map_err(|_| invalid("complaint/version"))?,
            created: complaint
                .created
                .as_ref()
                .and_then(|value| value.seconds())
                .ok_or_else(|| invalid("complaint/created"))?,
        });
    }
    ensure_context_tables(tx)?;
    let delivered_manifests =
        studio_runtime_store::context_settings::manifests(tx, &intent.agent_id)
            .map_err(|_| invalid("manifest"))?;
    let epoch = turn::Epoch(
        text(record, "threadId")?,
        optional(record, "compactions")?.unwrap_or(0),
    );
    let idle_summary_return = intent.provider == ProviderKind::Claude
        && record["idleSummaryPrepared"]
            .as_str()
            .is_some_and(|request| !request.is_empty())
        && record["idleSummaryPrepared"] == record["idleCompactionEpisode"]["id"]
        && studio_runtime_store::OrchestrationSettingsRepository::idle_compaction_for(
            tx,
            record["isLead"] == true,
        )?
        .mode
            == studio_runtime_schema::idle_compaction::Mode::CompactOnReturn;
    let restore = idle_summary_return || turn::needs_team_restore(&epoch, &delivered_manifests);
    let team_state = if restore {
        super::team_restore::snapshot(tx, record)?
    } else {
        None
    };
    let mut workers = Vec::new();
    // Only a lead's turn context (finished-worktree reminder) or a batch carrying
    // child events (team progress line) reads the team.
    let child_events = intent
        .events
        .iter()
        .any(|e| matches!(e.kind.as_str(), "child_result" | "child_stopped"));
    let team = if child_events || flag(record, "isLead")? {
        studio_runtime_store::AgentIndexedReads::team_records(tx, root_id)?
    } else {
        Vec::new()
    };
    for (_, raw) in team {
        let worker: AgentRecord = studio_json::from_str(&raw).map_err(|_| invalid("worker"))?;
        let status = turn::WorkerStatus::from_agent_status(worker.status.as_str());
        let deleted = worker.is_deleted();
        workers.push(turn::Worker {
            id: worker.id.0,
            root_id: root_id.into(),
            parent_id: worker.parent_id,
            worktree_ready: worker.worktree_ready.unwrap_or(false),
            deleted,
            status,
            auto_wake: worker.auto_wake.unwrap_or(false),
        });
    }
    let mut assets = Vec::new();
    let ids = intent
        .events
        .iter()
        .map(|e| optional::<Vec<String>>(&e.metadata, "assets"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .flatten()
        .collect::<BTreeSet<_>>();
    for id in ids {
        use rusqlite::OptionalExtension;
        // Legacy attachment projection admits partial records; the complete row decoder is stricter.
        let raw: String = tx
            .query_row(
                "SELECT record FROM runtime_assets WHERE id=?1",
                [&id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| refusal("Unknown attachment".into()))?;
        let v: Value = studio_json::from_str(&raw).map_err(|_| invalid("asset"))?;
        let owner = text(&v, "agentId")?
            .or(text(&v, "agent")?)
            .ok_or_else(|| invalid("asset/agent"))?;
        let owner = agent(tx, &owner)?;
        let owner_deleted = owner.is_deleted();
        assets.push(turn::Asset {
            id,
            owner_id: owner.id.0,
            owner_root_id: owner.root_id.0,
            owner_deleted,
            image: flag(&v, "image")?,
            path: decode(v["path"].clone(), "asset/path")?,
            name: decode(v["name"].clone(), "asset/name")?,
            size: decode(v["size"].clone(), "asset/size")?,
            view: v.as_object().ok_or_else(|| invalid("asset"))?.clone(),
        });
    }
    let delivered_reminders =
        studio_runtime_store::context_settings::reminders(tx, &intent.agent_id)?;
    let events = intent
        .events
        .iter()
        .map(|e| {
            Ok(turn::Event {
                id: e.id.clone(),
                kind: e.kind.clone(),
                text: e.text.clone(),
                created: e.created,
                metadata: turn::EventMetadata {
                    assets: optional(&e.metadata, "assets")?.unwrap_or_default(),
                    accepted_at: optional(&e.metadata, "acceptedAt")?,
                    extra: e
                        .metadata
                        .as_object()
                        .ok_or_else(|| invalid("event metadata"))?
                        .clone(),
                },
                preserve_progress: flag(&e.metadata, "preserveProgress")?,
                preserve_complaint: preserve_complaint(tx, &intent.agent_id, e)?,
            })
        })
        .collect::<Result<Vec<_>, WorkerError>>()?;
    let concurrency = root_concurrency(&root)?;
    Ok((
        turn::PreparationSnapshot {
            idle_summary_return,
            provider: Some(intent.provider),
            actor: turn::Actor {
                id: intent.agent_id.clone(),
                root_id: root_id.into(),
                epoch: turn::Epoch(
                    text(record, "threadId")?,
                    optional(record, "compactions")?.unwrap_or(0),
                ),
                is_lead: flag(record, "isLead")?,
                forked_from: text(record, "forkedFrom")?,
                prepared_context: optional(record, "preparedContext")?,
                delivered_mode: optional(record, "deliveredMode")?,
            },
            root: turn::RootSettings {
                concurrency,
                mode_revision: optional(&root, "agentModeRevision")?.unwrap_or(0),
            },
            delivered_manifests,
            plan,
            complaints,
            workers,
            team_state,
            delivered_reminders,
            assets,
            guidance: turn::GuidanceBundle { role },
            artifact_paths: BTreeMap::new(),
            prepared_at: now(),
            operation_id: String::new(),
        },
        events,
    ))
}
pub(crate) fn ensure_context_tables(tx: &Transaction<'_>) -> Result<(), WorkerError> {
    studio_runtime_store::context_settings::ensure_context_tables(tx)?;
    Ok(())
}

fn root_concurrency(root: &Value) -> Result<u16, WorkerError> {
    let value = if root["agentMode"] == "single"
        && root["subagentConcurrencyVersion"].as_u64().unwrap_or(0) < 2
    {
        0
    } else {
        optional(root, "concurrency")?.unwrap_or(if root["agentMode"] == "single" { 0 } else { 32 })
    };
    if value > 512 {
        return Err(invalid("root/concurrency"));
    }
    Ok(value)
}
fn can_delegate(tx: &Transaction<'_>, record: &Value, root: &Value) -> Result<bool, WorkerError> {
    if record["role"] == "reviewer"
        || !flag(root, "isLead")?
        || text(root, "parentId")?.is_some()
        || root_concurrency(root)? == 0
    {
        return Ok(false);
    }
    let limit = root["workerDefaults"]["delegationDepth"]
        .as_u64()
        .filter(|v| (1..=3).contains(v))
        .unwrap_or(1);
    let root_id = text(root, "id")?.ok_or_else(|| invalid("delegation/root"))?;
    let mut cursor_id = text(record, "id")?.ok_or_else(|| invalid("delegation/id"))?;
    let mut parent_id = text(record, "parentId")?;
    let mut depth = 0;
    let mut seen = BTreeSet::new();
    while cursor_id != root_id {
        if !seen.insert(cursor_id) {
            return Ok(false);
        }
        let Some(parent) = parent_id else {
            return Ok(false);
        };
        let Some(parent) = AgentRepository::get(tx, &AgentId::from(parent.as_str()))? else {
            return Ok(false);
        };
        if parent.root_id.0 != root_id {
            return Ok(false);
        }
        cursor_id = parent.id.0;
        parent_id = parent.parent_id;
        depth += 1;
        if depth >= limit {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod resource_tests {
    #[test]
    fn complaint_entry_decoding_keeps_database_error_order() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_complaints(id TEXT PRIMARY KEY,record TEXT); INSERT INTO runtime_complaints VALUES('bad','{'),('ok','{\"leadId\":\"lead\"}'),('foreign','{\"leadId\":\"other\"}');").unwrap();
        let tx = db.transaction().unwrap();
        let mut event = crate::scheduler::TurnEvent {
            id: "e".into(),
            kind: "complaint".into(),
            text: String::new(),
            created: 0.0,
            epoch: 1,
            error: None,
            metadata: studio_json::json!({}),
        };
        event.text = r#"{"complaints":[{"complaint_id":"bad"},{}]}"#.into();
        assert!(super::preserve_complaint(&tx, "lead", &event).is_err());
        event.text = r#"{"complaints":[{"complaint_id":"ok"}]}"#.into();
        assert!(super::preserve_complaint(&tx, "lead", &event).is_err());
        for (id, lead) in [("ok", "lead"), ("foreign", "other")] {
            let record = format!(
                r#"{{"id":"{id}","author":"a","leadId":"{lead}","status":"open","text":"t","created":1,"readAt":null,"recipient":"lead","responses":[],"updated":null,"version":null}}"#
            );
            tx.execute(
                "UPDATE runtime_complaints SET record=?1 WHERE id=?2",
                [&record, id],
            )
            .unwrap();
        }
        for (text, expected) in [
            (r#"{"complaints":[{}, {"complaint_id":"bad"}]}"#, true),
            (r#"{"complaints":[{"complaint_id":"ok"}]}"#, false),
            (r#"{"complaints":[{"complaint_id":"foreign"}]}"#, true),
            (r#"{"complaints":[]}"#, true),
            (r#"{"complaints":[{"complaint_id":"missing"}]}"#, true),
        ] {
            event.text = text.into();
            assert_eq!(
                super::preserve_complaint(&tx, "lead", &event).unwrap(),
                expected,
                "{text}"
            );
        }
        event.kind = "user".into();
        assert!(!super::preserve_complaint(&tx, "lead", &event).unwrap());
    }
    #[test]
    fn authorization_projects_exclude_destructive_removal_fences() {
        let db = rusqlite::Connection::open_in_memory().expect("database");
        db.execute_batch("CREATE TABLE runtime_projects(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            INSERT INTO runtime_projects VALUES('/deleted','{\"id\":\"/deleted\",\"removal\":{\"reason\":\"delete\",\"sequence\":1,\"requestId\":\"remove\",\"deletedAt\":1}}');
            INSERT INTO runtime_projects VALUES('/active','{\"id\":\"/active\",\"path\":\"/active\",\"peerTeams\":[]}');")
            .expect("project rows");
        let mut rows = super::event_authorization_inputs(&db, "agent", vec![]).expect("inputs");
        assert_eq!(
            rows.projects.keys().map(String::as_str).collect::<Vec<_>>(),
            ["/active"]
        );
        rows.resolved_paths
            .insert("/active".into(), "/active".into());
        crate::submit_gates::AuthorizationSnapshot::from_records(&rows)
            .expect("live project authorizes");
        db.execute(
            "UPDATE runtime_projects SET record='{' WHERE id='/active'",
            [],
        )
        .expect("malformed live row");
        assert!(super::event_authorization_inputs(&db, "agent", vec![]).is_err());
    }

    #[tokio::test]
    async fn delegation_uses_the_typed_parent_chain_and_depth_limit() {
        use super::*;
        let directory = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(directory.path())).await.unwrap();
        store.transaction_with_error(|tx| {
            let root = studio_json::from_str(r#"{"id":"lead","rootId":"lead","isLead":true,"epoch":0,"status":"idle","accountKey":"default","provider":"codex","compactions":0,"concurrency":32,"workerDefaults":{"delegationDepth":2}}"#).unwrap();
            let child = studio_json::from_str(r#"{"id":"child","rootId":"lead","parentId":"lead","epoch":0,"status":"idle","accountKey":"default","provider":"codex","compactions":0}"#).unwrap();
            let leaf = studio_json::from_str(r#"{"id":"leaf","rootId":"lead","parentId":"child","epoch":0,"status":"idle","accountKey":"default","provider":"codex","compactions":0}"#).unwrap();
            for record in [&root, &child, &leaf] {
                let agent: AgentRecord = serde::Deserialize::deserialize(record).unwrap();
                AgentRepository::put(tx, &agent)?;
            }
            assert!(super::can_delegate(tx, &child, &root)?);
            assert!(!super::can_delegate(tx, &leaf, &root)?);
            Ok::<_, WorkerError>(())
        }).await.unwrap();
    }

    #[test]
    fn native_release_phases_hold_only_unsettled_releases() {
        for (patch, held) in [
            (
                json!({"phase":"blocked", "connectionId":"earlier", "error":"native background command is active"}),
                false,
            ),
            (json!({"phase":"blocked"}), false),
            (json!({"phase":null, "inspectionPhase":"superseded"}), false),
            (json!({"phase":"released"}), false),
            (json!({"phase":"resumed"}), false),
            (json!({"phase":"pending"}), false),
            (json!({"phase":"released", "resetPending":true}), true),
            (json!({"phase":"unsubscribing"}), true),
            (
                json!({"phase":"unsubscribing", "connectionId":"earlier"}),
                false,
            ),
            (json!({"phase":"unknown"}), true),
            (json!({"phase":"checking"}), true),
            (
                json!({"phase":"checking", "inspectionPhase":"inspecting"}),
                false,
            ),
            (
                json!({"phase":"unsubscribing", "resetActorEpoch":0, "submittedAt":101}),
                true,
            ),
        ] {
            let mut record = json!({"threadId":"thread", "nativeRelease":{
                "threadId":"thread", "connectionId":"1"
            }});
            record["nativeRelease"]
                .as_object_mut()
                .expect("release object")
                .extend(patch.as_object().expect("patch object").clone());
            assert_eq!(native_release_held(&record, "1"), held, "{patch}");
        }
    }

    fn guidance(
        resources: &super::PreparationResources,
        record: &studio_json::Value,
        state_root: &std::path::Path,
    ) -> Result<crate::thread_prepare::GuidanceBundle, super::WorkerError> {
        super::guidance(
            &resources
                .clone()
                .capture_filesystem(None)
                .capture_native_layr(state_root, record, None)?,
            record,
            state_root,
        )
    }

    use super::*;

    #[test]
    fn reviewer_defaults_are_explained_in_emitted_tools_and_role_guidance() {
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("runtime resources");
        for tools in [&resources.tools, &resources.legacy_tools] {
            let spawn = tools
                .iter()
                .find(|tool| tool.name == "orchestration_spawn")
                .expect("spawn tool");
            assert!(
                spawn
                    .description
                    .contains("explicit review model stored in chat settings"),
                "{}",
                spawn.description
            );
            assert!(!spawn.description.contains("Default: gpt-6-luna"));
        }
        for record in [
            json!({"id":"new", "isLead":true, "provider":"codex"}),
            json!({"id":"native", "isLead":true, "threadId":"thread", "nativeTeleportTool":"orchestration_teleport", "provider":"codex"}),
            json!({"id":"legacy", "isLead":true, "threadId":"thread", "provider":"codex"}),
        ] {
            let text = role_guidance(&resources, &record).expect("emitted role guidance");
            assert!(
                text.contains("explicit review model stored in chat settings"),
                "{text}"
            );
            assert!(!text.contains("Use `gpt-6-luna`"), "{text}");
        }
    }

    #[tokio::test]
    async fn structured_native_errors_keep_thread_preparation_and_holds() {
        let directory = tempfile::tempdir().expect("temporary state");
        let store = RuntimeStore::open(Some(directory.path()))
            .await
            .expect("store");
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources");
        store.transaction_with(move |tx| {
            for (error, context_wait) in [(Value::Null, Value::Null), (json!("plain error"), Value::Null),
                (json!({"message":"Claude Code process exited with code 143"}), Value::Null),
                (json!({"message":"context error","codexErrorInfo":"misalignmentPolicyViolation"}), json!("repair"))] {
                let record = json!({"compactions":0,"id":"lead","rootId":"lead","epoch":1,"status":"queued",
                    "isLead":true,"autoWake":true,"provider":"claude","accountKey":"default",
                    "cwd":"/project","threadId":"existing-thread","error":error,
                    "lastCompletedTurn":"old-turn","lastCompletedTurnStatus":"interrupted",
                    "lastCompletedTurnError":error,"contextRepairWait":context_wait});
                AgentRepository::put(tx, &AgentRecord::from_value(record.clone()).expect("agent"))?;
                let snapshot = thread_snapshot(tx, &resources, &record, "connection", Path::new("/state"), None)?;
                assert_eq!(snapshot.actor.error.as_deref(), error.as_str());
                let plan = thread::plan(&snapshot, "next-attempt", "next-ticket");
                assert_eq!(native_thread_blocked(&record), error["codexErrorInfo"] == "misalignmentPolicyViolation");
                if !context_wait.is_null() {
                    assert_eq!(plan, thread::Outcome::Refuse(thread::Refusal::Blocked(thread::HoldReason::ContextRepair)));
                } else {
                    assert!(matches!(plan, thread::Outcome::Resume(_)), "{plan:?}");
                }
                let saved = agent(tx, "lead")?.value();
                let mut expected = error.clone();
                if let Some(fields) = expected.as_object_mut() {
                    fields.insert("studioRawEvidence".into(), error.clone());
                }
                assert_eq!(saved["error"], expected);
                assert_eq!(saved["lastCompletedTurnError"], expected);
            }
            Ok::<_, WorkerError>(())
        }).await.expect("valid structured errors preserve preparation and holds");
    }

    #[tokio::test]
    async fn malformed_native_error_types_still_block_thread_preparation() {
        let directory = tempfile::tempdir().expect("temporary state");
        let store = RuntimeStore::open(Some(directory.path()))
            .await
            .expect("store");
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources");
        store.transaction_with(move |tx| {
            for error in [json!(false), json!(42), json!([])] {
                let record = json!({"compactions":0,"accountKey":"default","id":"lead","rootId":"lead","epoch":1,"status":"queued",
                    "isLead":true,"provider":"claude","cwd":"/project","threadId":"existing-thread","error":error});
                let mut persisted = record.clone();
                persisted["error"] = Value::Null;
                AgentRepository::put(tx, &AgentRecord::from_value(persisted).expect("agent"))?;
                assert!(matches!(thread_snapshot(tx, &resources, &record, "connection", Path::new("/state"), None),
                    Err(WorkerError::Store(StoreError::InvalidStoredObject { table: "turn_preparation", id })) if id == "error"));
            }
            Ok::<_, WorkerError>(())
        }).await.expect("invalid error shapes remain rejected");
    }

    /// igor-mbp rehearsal 2026-10-22: two moved Claude agents carried Python's
    /// `frozenNativeParams`; Rust refused them (`thread-preparation/FrozenImport`)
    /// and a child result turned the completed parent into a failed agent.
    #[test]
    fn a_moved_agent_keeps_its_frozen_native_parameters() {
        let tool = json!({"name":"orchestration_message","description":"Saved","inputSchema":{"type":"object"}});
        let record = json!({"frozenNativeParams":{"baseInstructions":"base","developerInstructions":"guidance",
            "model":"default","dynamicTools":[tool],"effort":"high"}});
        let frozen = frozen_params(&record).expect("valid").expect("frozen");
        assert_eq!(frozen.model, json!("default"));
        assert_eq!(frozen.developer_instructions, json!("guidance"));
        assert_eq!(frozen.dynamic_tools.len(), 1);
        assert_eq!(frozen.effort, Some(json!("high")));
        assert_eq!(frozen.summary, None);
        assert!(
            frozen_params(&json!({"frozenNativeParams":null}))
                .expect("absent")
                .is_none()
        );
        for missing in [
            "baseInstructions",
            "developerInstructions",
            "model",
            "dynamicTools",
        ] {
            let mut broken = record.clone();
            broken["frozenNativeParams"]
                .as_object_mut()
                .expect("object")
                .remove(missing);
            assert!(frozen_params(&broken).is_err(), "{missing} is required");
        }
        let mut malformed = record;
        malformed["frozenNativeParams"]["dynamicTools"] = json!([{"name":"orchestration_message"}]);
        assert!(
            frozen_params(&malformed).is_err(),
            "a tool needs its schema"
        );
    }

    #[test]
    fn native_thread_blocks_follow_the_current_thread_and_turn() {
        for info in [
            json!({"misalignmentPolicyViolation":{}}),
            json!("misalignmentPolicyViolation"),
        ] {
            assert!(native_thread_blocked(
                &json!({"threadId":"current","error":{"codexErrorInfo":info}})
            ));
        }
        for info in [
            json!({"misalignmentPolicyViolation":{},"futureCode":{"id":42}}),
            json!({"futureCode":{}}),
        ] {
            assert!(!native_thread_blocked(
                &json!({"threadId":"current","error":{"codexErrorInfo":info}})
            ));
        }
        assert!(native_thread_blocked(
            &json!({"threadId":"current","nativeThreadBlock":{"threadId":"current"}})
        ));
        assert!(!native_thread_blocked(
            &json!({"threadId":"current","nativeThreadBlock":{"threadId":"old"}})
        ));
        assert!(native_thread_blocked(
            &json!({"threadId":"current","error":{"codexErrorInfo":"misalignmentPolicyViolation"}})
        ));
        assert!(!native_thread_blocked(
            &json!({"threadId":null,"error":{"codexErrorInfo":"misalignmentPolicyViolation"}})
        ));
        assert!(native_thread_blocked(
            &json!({"threadId":"current","turnId":"turn","nativeTurnError":{"turnId":"turn","error":{"codexErrorInfo":"misalignmentPolicyViolation"}}})
        ));
        assert!(!native_thread_blocked(
            &json!({"threadId":"current","turnId":"new","nativeTurnError":{"turnId":"old","error":{"codexErrorInfo":"misalignmentPolicyViolation"}}})
        ));
    }

    /// The live schema's complaint index serves a lead's complaints; a
    /// submission never scans the complaints of other leads.
    #[test]
    fn lead_complaints_use_the_lead_index() {
        let fixture: Value = studio_json::from_str(include_str!(
            "../../../../runtime-store/tests/fixtures/live-state-schema.json"
        ))
        .expect("live-state schema fixture");
        let connection = rusqlite::Connection::open_in_memory().expect("memory SQLite opens");
        connection
            .execute_batch(
                "CREATE TABLE runtime_complaints(id TEXT PRIMARY KEY, record TEXT NOT NULL)",
            )
            .expect("complaints table creates");
        for object in fixture["canvas.sqlite3"]["sqlite_master"]
            .as_array()
            .expect("fixture lists schema objects")
        {
            if object["table"] == "runtime_complaints"
                && object["type"] == "index"
                && let Some(sql) = object["sql"].as_str()
            {
                connection
                    .execute_batch(sql)
                    .expect("fixture index creates");
            }
        }
        for (id, lead) in [("b", "lead"), ("a", "lead"), ("c", "other")] {
            connection
                .execute(
                    "INSERT INTO runtime_complaints(id,record) VALUES(?1,?2)",
                    [id, &json!({"id":id,"leadId":lead}).to_string()],
                )
                .expect("complaint inserts");
        }
        let plan = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {LEAD_COMPLAINTS_SQL}"))
            .expect("plan prepares")
            .query_map(["lead"], |row| row.get::<_, String>(3))
            .expect("plan runs")
            .collect::<Result<Vec<_>, _>>()
            .expect("plan rows")
            .join(" | ");
        assert!(
            plan.contains("runtime_conversion_complaints_leadId"),
            "{plan}"
        );
        let ids: Vec<String> = lead_complaints(&connection, "lead")
            .expect("lead complaints")
            .iter()
            .map(|raw| studio_json::from_str::<Value>(raw).expect("record")["id"].to_string())
            .collect();
        assert_eq!(ids, ["\"a\"", "\"b\""]);
    }

    #[test]
    fn native_review_without_a_role_is_a_reviewer() {
        for role_value in [None, Some(Value::Null)] {
            let mut record = json!({"isLead":false,"nativeReview":{"requestId":"q"}});
            if let Some(value) = role_value {
                record["role"] = value;
            }
            assert_eq!(role(&record).expect("role"), thread::Role::Reviewer);
        }
        assert_eq!(
            role(&json!({"isLead":false})).expect("role"),
            thread::Role::Implementer
        );
    }

    /// Python stores a review child's `nativeReview` as its review object and
    /// tests it for truth. Reading it as a boolean ended the account worker
    /// (19 October 2026, review children of root 85bc2231).
    #[tokio::test]
    async fn a_native_review_object_marks_the_review_child() {
        let directory = tempfile::tempdir().expect("temporary state directory");
        let store = studio_runtime_store::RuntimeStore::open(Some(directory.path()))
            .await
            .expect("runtime store opens");
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("runtime resources");
        let tools = store
            .transaction_with(move |tx| {
                let root = json!({"compactions":0,"id":"root","rootId":"root","epoch":1,"status":"completed",
                    "isLead":true,"name":"Lead","provider":"codex","accountKey":"default"});
                AgentRepository::put(tx, &AgentRecord::from_value(root).expect("root"))?;
                let review = json!({"id":"review","rootId":"root","parentId":"root",
                    "role":"reviewer","provider":"codex",
                    "reviewAssignment":{"kind":"gitRange","base":"main","head":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                    "nativeReview":{"target":{"kind":"gitRange","base":"main","head":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"requestId":"q"}});
                let tools = actor_tool_definitions(tx, &resources, &review)
                    .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
                Ok::<_, StoreError>(tools)
            })
            .await
            .expect("a review object is read as a review child");
        let names: Vec<_> = tools
            .as_array()
            .expect("reviewer tools are an array")
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect();
        assert_eq!(names, ["orchestration_read"]);
    }

    async fn legacy_vm_restore_refused(kind: &str) {
        let directory = tempfile::tempdir().expect("state");
        let store = Arc::new(
            RuntimeStore::open(Some(directory.path()))
                .await
                .expect("store"),
        );
        let mut record = json!({"compactions":0,"id":"worker","rootId":"worker","epoch":1,
            "status":"paused","isLead":true,"role":"implementer","provider":"codex",
            "model":"fake-model","accountKey":"default","cwd":directory.path(),
            "executionMode":"vm","workspaceMode":"layr","layrReady":true,
            "layrProjectId":"legacy","layrLine":"legacy","nativeEffort":null,
            "effort":null,"autoWake":false});
        if kind == "review" {
            record["role"] = json!("reviewer");
        }
        if matches!(kind, "monitor" | "safety") {
            record
                .as_object_mut()
                .expect("object")
                .remove("executionMode");
            record["autoWake"] = json!(true);
            record["threadId"] = json!("old-thread");
        }
        store
            .transaction(move |tx| {
                AgentRepository::put(tx, &AgentRecord::from_value(record).expect("agent"))
            })
            .await
            .expect("saved agent");
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources");
        let result = match kind {
            "native" | "review" => super::super::pipeline::prepare_native_thread(
                Arc::clone(&store),
                &resources,
                "worker",
                "restore",
                1,
            )
            .await
            .map(|_| ()),
            "transfer" => super::super::pipeline::prepare_account_transfer_native_thread(
                Arc::clone(&store),
                &resources,
                "worker",
                "transfer",
                "restore",
                1,
            )
            .await
            .map(|_| ()),
            "monitor" => super::super::pipeline::prepare_monitor_command(
                Arc::clone(&store),
                &resources,
                "worker",
                1,
            )
            .await
            .map(|_| ()),
            "safety" => super::super::pipeline::prepare_safety_retry(
                Arc::clone(&store),
                &resources,
                &super::super::SafetyRetryPreparation {
                    agent_id: "worker".into(),
                    attempt_id: "retry".into(),
                    generation: 1,
                    source_thread_id: "old-thread".into(),
                    turn_id: "turn".into(),
                    model: "fake-model".into(),
                    input: json!([]),
                },
            )
            .await
            .map(|_| ()),
            _ => unreachable!(),
        };
        let expected = if matches!(kind, "native" | "review") {
            "workspace-readiness/unsupported-execution-mode"
        } else {
            "VmWorkspaceUnsupported"
        };
        assert!(
            matches!(result, Err(crate::account_worker::WorkerError::PreparationRefused(ref reason)) if reason == expected),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn legacy_vm_native_restore_is_refused() {
        legacy_vm_restore_refused("native").await;
    }

    #[tokio::test]
    async fn legacy_vm_transfer_restore_is_refused() {
        legacy_vm_restore_refused("transfer").await;
    }

    #[tokio::test]
    async fn legacy_vm_review_thread_is_refused() {
        legacy_vm_restore_refused("review").await;
    }

    #[tokio::test]
    async fn legacy_vm_monitor_command_is_refused() {
        legacy_vm_restore_refused("monitor").await;
    }

    #[tokio::test]
    async fn legacy_vm_safety_retry_is_refused() {
        legacy_vm_restore_refused("safety").await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn native_layr_paused_legacy_thread_uses_the_canonical_cwd_of_its_saved_alias() {
        let directory = tempfile::tempdir().expect("state");
        let store = Arc::new(
            RuntimeStore::open(Some(directory.path()))
                .await
                .expect("store"),
        );
        let root = crate::layr_native::store_path(store.root());
        let cwd = root.join("projects/project/lines/main");
        std::fs::create_dir_all(&cwd).expect("legacy line");
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(&root, &alias).expect("store alias");
        let stored_cwd = alias.join("projects/project/lines/main");
        let record = json!({"compactions":0,"id":"worker", "rootId":"worker", "epoch":1,
            "status":"paused", "isLead":true, "role":"implementer", "provider":"codex",
            "model":"fake-model",
            "accountKey":"default", "cwd":stored_cwd, "workspaceMode":"layr-native",
            "layrReady":true, "nativeEffort":null, "effort":null, "autoWake":false});
        store
            .transaction(move |tx| {
                AgentRepository::put(tx, &AgentRecord::from_value(record).expect("agent"))
            })
            .await
            .expect("saved legacy worker");
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources");
        let prepared = crate::account_worker::preparation::prepare_native_thread(
            Arc::clone(&store),
            &resources,
            "worker",
            "restore-thread",
            1,
        )
        .await
        .expect("paused legacy thread preparation preserves an equivalent cwd alias");
        assert_eq!(prepared.params["cwd"], json!(cwd));
        assert_eq!(
            prepared.params["config"]["shell_environment_policy.set.LAYR_ROOT"],
            json!(alias)
        );
        let saved = AgentRepository::get(
            &store.test_reader().expect("reader"),
            &AgentId::from("worker"),
        )
        .expect("agent")
        .expect("saved agent")
        .value();
        assert_eq!(saved["cwd"], json!(stored_cwd));
        assert_eq!(saved["status"], "paused");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn native_layr_snapshot_captures_store_aliases_and_reviewer_confinement_before_sql() {
        let directory = tempfile::tempdir().expect("state");
        let store = RuntimeStore::open(Some(directory.path()))
            .await
            .expect("store");
        let state_root = store.root().to_path_buf();
        let default_root = crate::layr_native::store_path(&state_root);
        std::fs::create_dir_all(&default_root).expect("default store");
        let custom = tempfile::tempdir().expect("custom store");
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(&default_root, &alias).expect("default alias");
        crate::layr_native::settings::fixture_select(&state_root, custom.path());
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources");
        // A selected store is canonicalized by settings::configure; a recorded root is kept as given.
        let canonical_custom = CanonicalPath::existing(custom.path())
            .expect("canonical custom")
            .into_path_buf();
        for (root, ready, expected_root) in [
            (json!(alias), false, alias.clone()),
            (json!(custom.path()), false, custom.path().to_owned()),
            (Value::Null, false, canonical_custom),
            (Value::Null, true, default_root.clone()),
        ] {
            let record = json!({"compactions":0,"accountKey":"default","id":"reviewer", "rootId":"reviewer", "epoch":1,
                "isLead":true, "role":"reviewer", "provider":"claude", "status":"queued",
                "cwd":"/scratch", "workspaceMode":"layr-native", "layrReady":ready,
                "layrReadCwd":default_root.join("projects/project/lines/main"),
                "layrStoreRoot":root, "layrScratch":"/scratch"});
            let saved = record.clone();
            store
                .transaction_with(move |tx| {
                    AgentRepository::put(tx, &AgentRecord::from_value(saved).expect("reviewer"))
                })
                .await
                .expect("saved reviewer");
            let expected_environment =
                crate::layr_native::thread_environment_for_record(&state_root, &record)
                    .expect("environment");
            assert_eq!(expected_environment["LAYR_ROOT"], json!(expected_root));
            if record["layrStoreRoot"] == json!(alias) && cfg!(target_os = "linux") {
                assert_eq!(
                    expected_environment["LAYR_SOCKET"],
                    json!(crate::layr_native::socket_path(&state_root))
                );
            }
            let captured = resources
                .for_sql(&state_root, &record, None)
                .await
                .expect("capture before SQL");
            let receipt = resources
                .for_receipt(&store, "reviewer", Some("/scratch"))
                .await
                .expect("receipt capture before SQL");
            let state_root = state_root.clone();
            store
                .transaction_with(move |tx| {
                    for resources in [captured, receipt] {
                        let mut current = record.clone();
                        current["name"] = json!("Renamed after capture");
                        let snapshot = thread_snapshot(
                            tx,
                            &resources,
                            &current,
                            "connection",
                            &state_root,
                            None,
                        )?;
                        for (name, value) in &expected_environment {
                            assert_eq!(
                                &snapshot.guidance.thread_config
                                    [&format!("shell_environment_policy.set.{name}")],
                                value
                            );
                        }
                        let options = &snapshot.settings.claude_options;
                        assert_eq!(
                            options["sandbox"]["filesystem"]["denyWrite"],
                            json!([expected_root])
                        );
                        assert_eq!(
                            options["sandbox"]["filesystem"]["allowWrite"],
                            json!(["/scratch"])
                        );
                        assert_eq!(options["nativeReviewerEnvironment"]["TMPDIR"], "/scratch");
                        assert!(
                            options["nativeReviewerEnvironment"]
                                .get("LAYR_ROOT")
                                .is_none()
                        );
                        for (key, value) in [
                            ("layrStoreRoot", json!("/changed-store")),
                            ("layrScratch", json!("/changed-scratch")),
                            ("role", json!("implementer")),
                        ] {
                            let mut changed = current.clone();
                            changed[key] = value;
                            assert!(
                                thread_snapshot(
                                    tx,
                                    &resources,
                                    &changed,
                                    "connection",
                                    &state_root,
                                    None
                                )
                                .is_err(),
                                "{key}"
                            );
                        }
                        if current["layrStoreRoot"].is_null() {
                            current["layrReady"] = json!(!ready);
                            assert!(
                                thread_snapshot(
                                    tx,
                                    &resources,
                                    &current,
                                    "connection",
                                    &state_root,
                                    None
                                )
                                .is_err()
                            );
                        }
                    }
                    Ok::<_, WorkerError>(())
                })
                .await
                .expect("snapshots do not touch the filesystem inside SQL");
        }
    }

    /// Claude resends its tools on each turn. Old host threads need the current
    /// catalog without changing their saved source or exact moved tool list.
    #[tokio::test]
    async fn an_unfrozen_native_claude_on_an_old_thread_gets_the_current_catalog() {
        let directory = tempfile::tempdir().expect("temporary state directory");
        let store = RuntimeStore::open(Some(directory.path()))
            .await
            .expect("runtime store opens");
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("runtime resources")
            .capture_filesystem(None);
        store
            .transaction_with(move |tx| {
                let failed = |error: WorkerError| StoreError::SchemaMismatch(error.to_string());
                for mode in ["native"] {
                    for (thread_id, source) in [
                        (Some("thread-old"), None),
                        (Some("thread-old"), Some("legacy")),
                        (None, None),
                    ] {
                        let mut record = json!({"compactions":0,"id":"claude", "rootId":"claude", "epoch":1,
                            "status":"running", "isLead":true, "name":"Lead", "provider":"claude",
                            "accountKey":"default", "executionMode":mode, "cwd":"/line"});
                        if let Some(thread_id) = thread_id {
                            record["threadId"] = json!(thread_id);
                        }
                        if let Some(source) = source {
                            record["nativeToolSource"] = json!(source);
                        }
                        AgentRepository::put(
                            tx,
                            &AgentRecord::from_value(record.clone()).expect("agent"),
                        )?;
                        let snapshot = thread_snapshot(
                            tx,
                            &resources,
                            &record,
                            "connection",
                            Path::new("/state"),
                            None,
                        )
                        .map_err(failed)?;
                        let case = format!("{mode} {thread_id:?} {source:?}");
                        let expected_source = if thread_id.is_some() {
                            thread::CatalogSource::Legacy
                        } else {
                            thread::CatalogSource::Current
                        };
                        assert_eq!(snapshot.catalog.source, expected_source, "{case}");
                        let tools = thread::tool_definitions(&snapshot).expect("tools");
                        assert!(
                            tools.iter().any(|tool| tool.name == crate::sign_in::TOOL),
                            "{case}: Claude must receive the sign-in tool"
                        );
                        assert_eq!(snapshot.catalog.definitions, resources.tools, "{case}");
                        assert_eq!(
                            actor_tool_definitions(tx, &resources, &record).map_err(failed)?,
                            json!(tools),
                            "{case}: the context tool reports the same catalog"
                        );
                        let attempt = match thread::plan(&snapshot, "attempt", "ticket") {
                            thread::Outcome::Start(attempt) | thread::Outcome::Resume(attempt) => {
                                attempt
                            }
                            other @ (thread::Outcome::Ready(_)
                            | thread::Outcome::Refuse(_)) => panic!("{case}: {other:?}"),
                        };
                        assert_eq!(
                            attempt.before_submit.set["nativeToolSource"],
                            json!(expected_source),
                            "{case}"
                        );
                        assert_eq!(attempt.params["dynamicTools"], json!(tools), "{case}");

                        let mut frozen = snapshot;
                        let saved_tools = resources
                            .legacy_tools
                            .iter()
                            .filter(|tool| tool.name == "orchestration_title")
                            .cloned()
                            .collect::<Vec<_>>();
                        assert_eq!(saved_tools.len(), 1);
                        let saved = json!({"baseInstructions":"saved base", "developerInstructions":"saved role",
                            "model":"default", "dynamicTools":saved_tools});
                        record["frozenNativeParams"] = saved;
                        frozen.frozen = frozen_params(&record).map_err(failed)?;
                        assert_eq!(
                            thread::tool_definitions(&frozen).expect("exact frozen tools"),
                            saved_tools,
                            "{case}: moved tool snapshots stay exact"
                        );
                        assert_eq!(
                            actor_tool_definitions(tx, &resources, &record).map_err(failed)?,
                            json!(saved_tools),
                            "{case}: frozen context tools stay exact"
                        );
                    }
                }
                Ok::<_, StoreError>(())
            })
            .await
            .expect("Claude keeps current tools and exact frozen boundaries");
    }

    #[test]
    fn only_host_codex_threads_send_sign_in_links_to_studio() {
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("runtime resources");
        let state = tempfile::tempdir().expect("state root");
        let folder = state
            .path()
            .join(crate::browser_environment::BROWSER_HOST_FOLDER);
        std::fs::create_dir_all(&folder).expect("host bridge folder");
        std::fs::write(
            folder.join(crate::browser_environment::BROWSER_LAUNCHER),
            "#!/bin/sh\n",
        )
        .expect("launcher");
        let config = |record: Value| {
            guidance(&resources, &record, state.path())
                .expect("guidance")
                .thread_config
        };
        let host = config(json!({"id":"host", "isLead":true, "provider":"codex"}));
        assert_eq!(
            host["shell_environment_policy.set.STUDIO_BROWSER_SESSION"],
            "host"
        );
        assert!(host.contains_key("shell_environment_policy.set.BROWSER"));
        assert!(host.contains_key("shell_environment_policy.set.STUDIO_BROWSER_SOCKET"));
        {
            let record = json!({"id":"claude", "isLead":true, "provider":"claude"});
            let config = config(record);
            assert!(
                !config
                    .keys()
                    .any(|key| key.starts_with("shell_environment_policy.set.")),
                "{config:?}"
            );
        }
    }

    #[test]
    fn every_host_codex_agent_gets_the_account_browser_like_python() {
        let crate::browser::BrowserHomes {
            root: _root,
            shared,
            account,
            node,
            skills,
        } = crate::browser::browser_homes();
        let mut resources = PreparationResources::bundled(
            account.to_string_lossy().into_owned(),
            ModelCatalogService::new(),
        )
        .expect("runtime resources");
        let bundle = |resources: &PreparationResources, record: Value| {
            guidance(resources, &record, Path::new("/state")).expect("guidance")
        };
        let lead = json!({"id":"l", "isLead":true, "provider":"codex", "accountKey":"profile-a"});
        // Without a shared home (tests and the default constructor) nothing changes.
        assert_eq!(bundle(&resources, lead.clone()).browser, None);
        resources.shared_home = shared.to_string_lossy().into_owned();

        let expected = Some(thread::BrowserGuidance {
            instructions: Some(crate::browser::BROWSER_GUIDANCE.to_owned()),
            skill_roots: vec![skills.to_string_lossy().into_owned()],
        });
        let lead = bundle(&resources, lead);
        assert_eq!(lead.browser, expected);
        let server = &lead.thread_config["mcp_servers.node_repl"];
        assert_eq!(server["command"], json!(node));
        assert_eq!(
            server["env"]["CODEX_HOME"],
            json!(CanonicalPath::existing(&account).expect("account"))
        );
        assert!(server["env"].get("OPENAI_API_KEY").is_none());
        assert!(lead.thread_config.get("mcp_servers").is_none());

        // A worker keeps the browser; its table of disabled user servers
        // carries node_repl, so no key order can drop it.
        for role in ["implementer", "reviewer"] {
            let worker = bundle(
                &resources,
                json!({"id":"w", "parentId":"l", "role":role, "provider":"codex"}),
            );
            assert_eq!(worker.browser, expected, "{role}");
            assert!(worker.thread_config.get("mcp_servers.node_repl").is_none());
            let servers = &worker.thread_config["mcp_servers"];
            assert_eq!(servers["anarlog"], json!({"enabled": false}));
            assert_eq!(servers["node_repl"], *server, "{role}");
        }

        // Claude agents do not use this browser setup.
        {
            let record = json!({"id":"c", "provider":"claude"});
            let other = bundle(&resources, record);
            assert_eq!(other.browser, None);
            assert!(other.thread_config.get("mcp_servers.node_repl").is_none());
        }

        // A disabled plugin clears the skill root and adds no server.
        std::fs::write(
            account.join("config.toml"),
            "[plugins.\"chrome@openai-bundled\"]\nenabled = false\n",
        )
        .expect("disabled plugin");
        let off = bundle(
            &resources,
            json!({"id":"l", "isLead":true, "provider":"codex"}),
        );
        assert_eq!(
            off.browser,
            Some(thread::BrowserGuidance {
                instructions: None,
                skill_roots: Vec::new(),
            })
        );
        assert!(off.thread_config.get("mcp_servers.node_repl").is_none());
    }

    #[test]
    fn only_a_codex_worker_switches_off_the_homes_mcp_servers() {
        let directory = tempfile::tempdir().expect("owned test directory");
        let home = directory.path().to_path_buf();
        std::fs::create_dir_all(&home).expect("home");
        std::fs::write(
            home.join("config.toml"),
            "[mcp_servers.playwright]\ncommand = \"npx\"\n[mcp_servers.\"aws.mcp\"]\ncommand = \"uvx\"\n",
        )
        .expect("config");
        let resources = PreparationResources::bundled(
            home.to_string_lossy().into_owned(),
            ModelCatalogService::new(),
        )
        .expect("runtime resources");
        let servers = |record: Value| {
            guidance(&resources, &record, Path::new("/state"))
                .expect("guidance")
                .thread_config
                .get("mcp_servers")
                .cloned()
        };
        assert_eq!(
            servers(json!({"id":"w", "parentId":"lead", "provider":"codex"})),
            Some(json!({"playwright":{"enabled":false},"aws.mcp":{"enabled":false}}))
        );
        assert_eq!(
            servers(json!({"id":"l", "isLead":true, "provider":"codex"})),
            None
        );
        assert_eq!(servers(json!({"id":"r", "provider":"codex"})), None);
        assert_eq!(
            servers(json!({"id":"c", "parentId":"lead", "provider":"claude"})),
            None
        );
    }
}

#[cfg(test)]
mod workspace_fence_tests {
    use super::*;

    #[test]
    fn legacy_reviewer_store_uses_read_line_not_prepared_scratch() {
        let state = tempfile::tempdir().expect("state");
        let root = crate::layr_native::store_path(state.path());
        let line = root.join("projects/p/lines/main");
        let scratch = state.path().join("reviewer-scratch");
        let record = json!({"workspaceMode":"layr-native", "layrReady":true,
            "role":"reviewer", "cwd":scratch, "layrReadCwd":line, "layrScratch":scratch});
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources")
            .capture_filesystem(None)
            .capture_native_layr(state.path(), &record, Some(&scratch))
            .expect("reviewer capture");
        assert_eq!(
            resources
                .native_layr(state.path(), &record)
                .expect("association")
                .root,
            root
        );
        let env = resources
            .monitor_environment(state.path(), &record)
            .unwrap()
            .unwrap();
        assert_eq!(
            env,
            crate::layr_native::reviewer_environment(state.path(), scratch.to_str().unwrap())
        );
        assert!(env.keys().all(|key| !key.starts_with("LAYR_")));
    }

    #[tokio::test]
    async fn receipt_store_and_scratch_changes_are_isolatable() {
        let state = tempfile::tempdir().expect("state");
        let store = RuntimeStore::open(Some(state.path())).await.expect("store");
        let record = json!({"compactions":0,"accountKey":"default","id":"reviewer", "rootId":"reviewer", "epoch":1,
            "status":"running", "isLead":true, "provider":"claude", "cwd":"/scratch",
            "workspaceMode":"layr-native", "role":"reviewer", "layrStoreRoot":"/store-a", "layrScratch":"/scratch"});
        let saved = record.clone();
        store
            .transaction_with(move |tx| {
                AgentRepository::put(tx, &AgentRecord::from_value(saved).expect("agent"))?;
                Ok::<_, StoreError>(())
            })
            .await
            .expect("save");
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources")
            .for_receipt(&store, "reviewer", Some("/scratch"))
            .await
            .expect("capture");
        let mut changed = record.clone();
        changed["layrStoreRoot"] = json!("/store-b");
        assert!(matches!(
            resources.native_layr(state.path(), &changed),
            Err(WorkerError::Store(StoreError::InvalidStoredObject { .. }))
        ));
        changed = record;
        changed["layrScratch"] = json!("/other-scratch");
        assert!(matches!(
            native_claude_options(&resources, &changed, state.path()),
            Err(WorkerError::Store(StoreError::InvalidStoredObject { .. }))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn native_layr_capture_preserves_alias_identity_and_rejects_changed_association() {
        let state = tempfile::tempdir().expect("state");
        let root = crate::layr_native::store_path(state.path());
        std::fs::create_dir_all(&root).expect("store");
        let alias = state.path().join("store-alias");
        std::os::unix::fs::symlink(&root, &alias).expect("alias");
        let record = json!({"workspaceMode":"layr-native", "layrStoreRoot":alias});
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources")
            .capture_filesystem(None)
            .capture_native_layr(state.path(), &record, None)
            .expect("capture");
        let captured = resources
            .native_layr(state.path(), &record)
            .expect("association");
        if cfg!(target_os = "linux") {
            assert_eq!(
                captured.environment["LAYR_SOCKET"],
                json!(crate::layr_native::socket_path(state.path()))
            );
        } else {
            assert_eq!(captured.environment["LAYR_DIRECT"], "1");
            assert_eq!(captured.environment["LAYR_ROOT"], json!(alias));
        }
        let monitor_env = resources
            .monitor_environment(state.path(), &record)
            .expect("monitor environment")
            .expect("native");
        assert_eq!(monitor_env, captured.environment);
        for provider in ["codex", "claude"] {
            let mut provider_record = record.clone();
            provider_record["provider"] = json!(provider);
            assert_eq!(
                resources
                    .monitor_environment(state.path(), &provider_record)
                    .unwrap(),
                Some(captured.environment.clone()),
                "{provider} monitor environment"
            );
        }
        assert!(monitor_env["PATH"].as_str().unwrap().starts_with(&format!(
                "{}:",
                crate::layr_native::binary_path(state.path())
                    .parent()
                    .unwrap()
                    .display()
            )));
        assert!(
            resources
                .monitor_environment(state.path(), &json!({}))
                .unwrap()
                .is_none()
        );
        let changed = json!({"workspaceMode":"layr-native", "layrStoreRoot":root});
        assert!(resources.native_layr(state.path(), &changed).is_err());

        let line = root.join("projects/p/lines/main");
        std::fs::create_dir_all(&line).expect("line");
        let legacy = json!({"workspaceMode":"layr-native", "layrReady":true, "cwd":alias.join("projects/p/lines/main")});
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources")
            .capture_filesystem(None)
            .capture_native_layr(state.path(), &legacy, Some(&line))
            .expect("legacy capture");
        let normalized = json!({"workspaceMode":"layr-native", "layrReady":true, "cwd":line});
        assert_eq!(
            resources
                .native_layr(state.path(), &normalized)
                .expect("normalized association")
                .environment["LAYR_ROOT"],
            json!(alias)
        );
        let changed = json!({"workspaceMode":"layr-native", "layrReady":true, "cwd":"/other/projects/p/lines/main"});
        assert!(resources.native_layr(state.path(), &changed).is_err());
    }

    #[test]
    fn task_result_schema_native_and_legacy_catalogs_use_the_owner() {
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources");
        for tools in [&resources.tools, &resources.legacy_tools] {
            let task = tools
                .iter()
                .find(|tool| tool.name == "orchestration_task")
                .expect("task");
            assert_eq!(
                task.input_schema["properties"]["result_schema"],
                studio_runtime_schema::task_results::schema_input()
            );
            assert!(
                task.input_schema["properties"]["result"]
                    .get("type")
                    .is_none()
            );
            let spawn = tools
                .iter()
                .find(|tool| tool.name == "orchestration_spawn")
                .expect("spawn");
            assert!(spawn.input_schema["properties"]["agents"]["items"]["properties"]["task_id"]["description"].as_str().expect("description").contains("result_schema"));
        }
    }

    #[test]
    fn claude_child_spawn_catalog_exposes_valid_permission_modes() {
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources");
        for tools in [&resources.tools, &resources.legacy_tools] {
            let spawn = tools
                .iter()
                .find(|tool| tool.name == "orchestration_spawn")
                .expect("spawn");
            let field = &spawn.input_schema["properties"]["agents"]["items"]["properties"]["permission_mode"];
            assert_eq!(
                field["enum"],
                json!(crate::claude_controls::PERMISSION_MODES)
            );
        }
    }

    #[tokio::test]
    async fn claude_child_saved_mode_reaches_start_and_resume() {
        let directory = tempfile::tempdir().expect("temporary state");
        let store = RuntimeStore::open(Some(directory.path()))
            .await
            .expect("store");
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources");
        store.transaction_with(move |tx| {
            AgentRepository::put(tx, &AgentRecord::from_value(json!({"compactions":0,"accountKey":"default","id":"lead","rootId":"lead","epoch":1,
                "status":"running","isLead":true,"cwd":"/project","provider":studio_runtime_schema::ProviderKind::Claude})).expect("lead"))?;
            for mode in crate::claude_controls::PERMISSION_MODES {
                for resumed in [false, true] {
                    let mut record = json!({"compactions":0,"accountKey":"default","id":"worker","rootId":"lead","parentId":"lead","epoch":1,
                        "status":"queued","isLead":false,"role":"implementer","provider":"claude","cwd":"/project",
                        "workspaceMode":"shared","permissionMode":mode});
                    if resumed {
                        record["threadId"] = json!("existing-thread");
                        record["nativeToolSource"] = json!("current");
                    }
                    let snapshot = thread_snapshot(tx, &resources, &record, "connection", Path::new("/state"),
                        None)?;
                    let attempt = match thread::plan(&snapshot, "attempt", "ticket") {
                        thread::Outcome::Start(attempt) | thread::Outcome::Resume(attempt) => attempt,
                        other @ thread::Outcome::Ready(_) | other @ thread::Outcome::Refuse(_) => panic!("expected thread launch, got {other:?}"),
                    };
                    assert_eq!(attempt.params["claude"]["permissionMode"], *mode);
                }
            }
            Ok::<_, WorkerError>(())
        }).await.expect("thread plans");
    }

    #[tokio::test]
    async fn claude_workspace_fence_reaches_thread_launch_options() {
        let directory = tempfile::tempdir().expect("temporary state");
        let store = RuntimeStore::open(Some(directory.path()))
            .await
            .expect("store");
        let resources = PreparationResources::bundled(String::new(), ModelCatalogService::new())
            .expect("resources");
        let native_resources: Vec<_> = ["implementer", "reviewer"].into_iter().map(|role| {
            resources.clone().capture_filesystem(None).capture_native_layr(
                Path::new("/state"), &json!({"workspaceMode":"layr-native", "role":role,
                    "layrStoreRoot":"/state/layr-native/store", "layrScratch":"/private/reviewer-scratch"}),
                None,
            ).expect("native filesystem inputs")
        }).collect();
        store
            .transaction_with(move |tx| {
                let root = json!({"compactions":0,"accountKey":"default","id":"lead","rootId":"lead","epoch":1,"status":"running",
                "isLead":true,"cwd":"/source checkout","hostProjectPath":"/source checkout","provider":studio_runtime_schema::ProviderKind::Claude});
                AgentRepository::put(tx, &AgentRecord::from_value(root).expect("lead"))?;
                for (mode, provider, role, ready, fenced, expected_sandbox) in [
                    (
                        "worktree",
                        "claude",
                        "implementer",
                        true,
                        true,
                        "danger-full-access",
                    ),
                    (
                        "layr-native",
                        "claude",
                        "implementer",
                        true,
                        true,
                        "danger-full-access",
                    ),
                    (
                        "shared",
                        "claude",
                        "implementer",
                        true,
                        false,
                        "danger-full-access",
                    ),
                    ("worktree", "claude", "reviewer", true, false, "read-only"),
                    (
                        "layr-native",
                        "claude",
                        "reviewer",
                        true,
                        false,
                        "read-only",
                    ),
                    (
                        "worktree",
                        "claude",
                        "implementer",
                        false,
                        false,
                        "danger-full-access",
                    ),
                    (
                        "layr-native",
                        "claude",
                        "implementer",
                        false,
                        false,
                        "danger-full-access",
                    ),
                    (
                        "worktree",
                        "codex",
                        "implementer",
                        true,
                        false,
                        "danger-full-access",
                    ),
                ] {
                    let record = json!({"compactions":0,"accountKey":"default","id":"worker","rootId":"lead","parentId":"lead",
                    "epoch":1,"status":"queued","isLead":false,"role":role,"provider":provider,
                    "cwd":"/worker copy:quoted\"path","hostProjectPath":"/parent copy",
                    "workspaceMode":mode,"worktree":mode == "worktree","worktreeReady":ready,
                    "layrReady":mode == "layr-native" && ready,
                    "layrScratch":"/private/reviewer-scratch",
                    "layrStoreRoot":"/state/layr-native/store",
                    "permissionMode":"bypassPermissions"});
                    for resumed in [false, true] {
                        let mut record = record.clone();
                        if resumed {
                            // A current thread retains the catalog selected at start.
                            record["threadId"] = json!("existing-thread");
                            record["nativeToolSource"] = json!("current");
                        }
                        let snapshot = thread_snapshot(
                            tx,
                            if mode == "layr-native" {
                                &native_resources[usize::from(role == "reviewer")]
                            } else {
                                &resources
                            },
                            &record,
                            "connection",
                            Path::new("/state"),
                            None,
                        )?;
                        let attempt = match thread::plan(&snapshot, "attempt", "ticket") {
                            thread::Outcome::Start(attempt) | thread::Outcome::Resume(attempt) => {
                                attempt
                            }
                            other @ (thread::Outcome::Ready(_)
                            | thread::Outcome::Refuse(_)) => panic!("expected thread launch, got {other:?}"),
                        };
                        let options = attempt.params.get("claude").unwrap_or(&Value::Null);
                        if provider == "claude" && role == "reviewer" {
                            assert_eq!(options["permissionMode"], "plan");
                            assert_eq!(attempt.params["sandbox"], "read-only");
                            assert_eq!(attempt.params["approvalPolicy"], "on-request");
                        }
                        if fenced {
                            assert_eq!(options["permissionMode"], "bypassPermissions");
                            assert_eq!(options["env"]["CLAUDE_CODE_HOST_WORKTREE"], record["cwd"]);
                            let fence: Vec<String> = studio_json::from_str(
                                options["env"]["CLAUDE_CODE_HOST_WORKTREE_FENCE"]
                                    .as_str()
                                    .expect("JSON fence"),
                            )
                            .expect("fence array");
                            let expected = vec!["/parent copy", "/source checkout"];
                            assert_eq!(fence, expected);
                            let mut current = snapshot.clone();
                            current.actor.prepare_attempt = Some(attempt.attempt_id.clone());
                            let receipt = thread::ProviderReceipt {
                                identity: attempt.receipt_identity(),
                                thread_id: Some("existing-thread".into()),
                                fields: Map::new(),
                                reattached: false,
                            };
                            assert!(matches!(
                                thread::reduce_receipt(
                                    &attempt,
                                    &current,
                                    Some(&receipt),
                                    None,
                                    json!(1)
                                ),
                                thread::ReceiptOutcome::Accepted { .. }
                            ));
                            current
                                .host_worktree
                                .as_mut()
                                .expect("worker fence")
                                .fence
                                .push("/changed-source".into());
                            assert_eq!(
                                thread::reduce_receipt(
                                    &attempt,
                                    &current,
                                    Some(&receipt),
                                    None,
                                    json!(1)
                                ),
                                thread::ReceiptOutcome::Stale(thread::StaleReason::Settings)
                            );
                        } else {
                            for key in [
                                "CLAUDE_CODE_HOST_WORKTREE",
                                "CLAUDE_CODE_HOST_WORKTREE_FENCE",
                            ] {
                                assert!(
                                    options.get("env").and_then(|env| env.get(key)).is_none(),
                                    "{mode}/{provider}/{role}/{ready}: {options}"
                                );
                            }
                            if mode == "layr-native" && provider == "claude" {
                                assert_eq!(
                                    options["env"]["LAYR_DIRECT"],
                                    if cfg!(target_os = "linux") { "0" } else { "1" }
                                );
                                assert!(options["env"]["LAYR_ROOT"].is_string());
                                if role == "reviewer" {
                                    assert_eq!(options["sandbox"]["enabled"], true);
                                    assert_eq!(options["sandbox"]["failIfUnavailable"], true);
                                    assert_eq!(
                                        options["sandbox"]["filesystem"]["allowWrite"],
                                        json!(["/private/reviewer-scratch"])
                                    );
                                    assert_eq!(
                                        options["sandbox"]["filesystem"]["denyWrite"],
                                        json!([crate::layr_native::store_path(Path::new(
                                            "/state"
                                        ))])
                                    );
                                    assert_eq!(
                                        options["nativeReviewerEnvironment"]["TMPDIR"],
                                        "/private/reviewer-scratch"
                                    );
                                    assert!(
                                        options["nativeReviewerEnvironment"]
                                            .get("LAYR_ROOT")
                                            .is_none()
                                    );
                                }
                            }
                        }
                        assert_eq!(attempt.params["sandbox"], expected_sandbox);
                    }
                }
                Ok::<_, WorkerError>(())
            })
            .await
            .expect("production snapshot and provider params");
    }

    #[test]
    fn claude_workspace_fence_rejects_missing_or_relative_source() {
        let mut record = json!({"provider":"claude","role":"implementer","parentId":"lead",
            "worktree":true,"worktreeReady":true,"cwd":"/worker","hostProjectPath":"relative"});
        assert!(host_worktree(&record, &json!({}), thread::Role::Implementer, false).is_err());
        record["hostProjectPath"] = Value::Null;
        assert!(host_worktree(&record, &json!({}), thread::Role::Implementer, false).is_err());
        // A lead's workspace preference must not turn it into a fenced worker.
        assert!(
            host_worktree(&record, &json!({}), thread::Role::Implementer, true)
                .expect("lead")
                .is_none()
        );
    }
}
