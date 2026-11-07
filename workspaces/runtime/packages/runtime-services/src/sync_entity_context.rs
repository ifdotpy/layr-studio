//! Persisted context needed to build canonical renderer entity views.

use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::de::DeserializeOwned;
use studio_json::Value;
#[cfg(test)]
use studio_json::json;
use studio_runtime_store::CachedSql;
use studio_runtime_store::{StoreResult, system_reads as records};

use crate::event_write::truthy;
use crate::submit_gates::{AgentAuthorityRow, Deletion, ProjectAuthorityRow};
use crate::sync_policy::table_exists;
use crate::sync_views::{AgentViewContext, RoomViewContext, TransferObservation};

fn transfer_observation(transaction: &Connection, record: &Value) -> TransferObservation {
    if record.get("isLead").and_then(Value::as_bool) != Some(true) {
        return TransferObservation::Absent;
    }
    if let Some(summary) = record.get("accountTransfer")
        && summary.get("kind").and_then(Value::as_str) == Some("unreadable")
    {
        return TransferObservation::Unreadable {
            reason: summary
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("Transfer data is unreadable")
                .to_owned(),
        };
    }
    let id = record
        .get("accountTransferId")
        .and_then(Value::as_str)
        .or_else(|| {
            let summary = record.get("accountTransfer")?;
            (summary.get("status").and_then(Value::as_str) == Some("pending"))
                .then(|| summary.get("id").and_then(Value::as_str))
                .flatten()
        });
    let Some(id) = id else {
        return TransferObservation::Absent;
    };
    match crate::account_transfer_driver::load(transaction, id) {
        Ok(snapshot) if snapshot.can_cancel() => TransferObservation::Cancellable,
        Ok(_) => TransferObservation::Held,
        Err(_) => TransferObservation::Unreadable {
            reason: "Transfer data is unreadable".into(),
        },
    }
}

fn account_destinations(
    transaction: &Connection,
) -> StoreResult<Vec<studio_runtime_schema::account_destination::AccountDestination>> {
    #[derive(serde::Deserialize)]
    struct Entity {
        value: Accounts,
    }
    #[derive(serde::Deserialize)]
    struct Accounts {
        accounts: Vec<Account>,
    }
    #[derive(serde::Deserialize)]
    struct Account {
        id: studio_runtime_schema::account_key::AccountKey,
        eligibility: studio_runtime_schema::agent_progress::AccountEligibility,
    }
    if !table_exists(transaction, "runtime_account_summaries")? {
        return Ok(Vec::new());
    }
    let raw: Option<String> = transaction
        .query_row(
            "SELECT record FROM runtime_account_summaries WHERE id='current'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let entity: Entity = studio_json::from_str(&raw)?;
    Ok(entity
        .value
        .accounts
        .into_iter()
        .map(
            |row| studio_runtime_schema::account_destination::AccountDestination {
                account_key: row.id,
                eligibility: row.eligibility,
            },
        )
        .collect())
}

pub(crate) fn agent_view_context(
    transaction: &Connection,
    record: &Value,
    project_server_id: Option<&str>,
) -> StoreResult<AgentViewContext> {
    let id = record.get("id").and_then(Value::as_str).unwrap_or_default();
    let empty_lead = truthy(record.get("isLead"))
        && !truthy(record.get("deletedAt"))
        && !truthy(record.get("accountTransferId"))
        && record.get("status").and_then(Value::as_str) == Some("idle")
        && !truthy(record.get("threadId"))
        && !truthy(record.get("prompt"))
        && !exists_for_agent(transaction, "runtime_events", "agent", id)?
        && !exists_for_agent(transaction, "runtime_items", "agent", id)?
        && !transaction.query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM runtime_agents WHERE parent_id=?1)",
            [id],
            |row| row.get::<_, bool>(0),
        )?
        && !exists_for_agent(
            transaction,
            "runtime_monitors",
            "json_extract(record,'$.agent')",
            id,
        )?;
    let mut latest: Option<(f64, String)> = None;
    let mut statement = transaction.prepare_cached(
        "SELECT id,record FROM runtime_work WHERE json_extract(record,'$.owner')=?1",
    )?;
    let rows = statement
        .query_map([id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut assigned = Vec::with_capacity(rows.len());
    for (work_id, raw) in rows {
        let assignment = crate::agent_progress::Assignment::decode(&work_id, &raw)?;
        for result in assignment.results().into_iter().flatten() {
            if result.get("agent").and_then(Value::as_str) != Some(id) {
                continue;
            }
            let Some(path) = result.get("resultFile").and_then(Value::as_str) else {
                continue;
            };
            if path.is_empty() {
                continue;
            }
            let created = result
                .get("created")
                .and_then(Value::as_f64)
                .unwrap_or_default();
            if latest.as_ref().is_none_or(|(prior, _)| created > *prior) {
                latest = Some((created, path.to_owned()));
            }
        }
        assigned.push(assignment);
    }
    let progress = crate::agent_progress::derive(transaction, record, &assigned)?;
    let attention = pending_attention(transaction, id, record)?;
    Ok(AgentViewContext {
        account_destinations: account_destinations(transaction)?,
        transfer: transfer_observation(transaction, record),
        team_counts: team_counts(transaction, id, record)?,
        command_progress: progress.commands,
        stall: progress.stall,
        project_server_id: project_server_id.map(str::to_owned),
        launcher_alive: true,
        empty_lead,
        has_question: attention.0,
        has_approval: attention.1,
        latest_work_result_file: latest.map(|(_, path)| path),
    })
}

/// The status is read from the chat row (`domain`), not from the retired agent column.
pub(crate) const TEAM_COUNT_INPUTS_SQL: &str = "SELECT COALESCE((SELECT status FROM chats WHERE id=runtime_agents.id),''),COALESCE(json_extract(record,'$.autoWake'),(SELECT auto_wake FROM chats WHERE id=runtime_agents.id)) FROM runtime_agents INDEXED BY runtime_agent_live_root_v2 WHERE root_id=?1 AND deleted_at IS NULL AND id<>?1 AND COALESCE(json_extract(record,'$.isLead'),0)=0";

/// The team counts of a lead as its view shows them, as JSON text; None for a member.
pub(crate) fn team_counts_text(transaction: &Connection, id: &str) -> StoreResult<Option<String>> {
    let lead: Option<bool> = transaction
        .prepare_cached("SELECT json_extract(record,'$.isLead') FROM runtime_agents WHERE id=?1")?
        .query_row([id], |row| row.get(0))
        .optional()?
        .flatten();
    if lead != Some(true) {
        return Ok(None);
    }
    team_counts(transaction, id, &studio_json::json!({"isLead": true}))?
        .map(|counts| studio_json::to_string(&counts))
        .transpose()
        .map_err(studio_runtime_store::StoreError::from)
}

fn team_counts(
    transaction: &Connection,
    id: &str,
    record: &Value,
) -> StoreResult<Option<studio_runtime_schema::agent_progress::TeamCounts>> {
    if !truthy(record.get("isLead")) {
        return Ok(None);
    }
    let mut total = 0;
    let mut working = 0;
    let mut statement = transaction.prepare_cached(TEAM_COUNT_INPUTS_SQL)?;
    let rows = statement.query_map([id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<bool>>(1)?.unwrap_or(false),
        ))
    })?;
    for row in rows {
        let (status, auto_wake) = row?;
        total += 1;
        if crate::turn_prepare::WorkerStatus::from_agent_status(&status).working(auto_wake) {
            working += 1;
        }
    }
    Ok(Some(studio_runtime_schema::agent_progress::TeamCounts {
        working,
        total,
    }))
}

fn exists_for_agent(
    transaction: &Connection,
    table: &str,
    agent_column: &str,
    agent_id: &str,
) -> StoreResult<bool> {
    if !table_exists(transaction, table)? {
        return Ok(false);
    }
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE {agent_column}=?1)");
    Ok(transaction.query_row(&sql, [agent_id], |row| row.get(0))?)
}

/// Checked record text retained only to recheck the directory's exact snapshot.
#[derive(Default, PartialEq)]
pub struct RoomSources {
    agents: Vec<(String, String)>,
    projects: Vec<(String, String)>,
}

impl RoomSources {
    /// Read the checked rows in a short snapshot; resolve them after the read ends.
    pub fn read(connection: &Connection) -> StoreResult<Self> {
        Self::checked(Self::read_raw(connection)?)
    }

    fn read_raw(connection: &Connection) -> StoreResult<Self> {
        Ok(Self {
            agents: crate::sync_policy::stored_record_rows(connection, "runtime_agents")?,
            projects: crate::sync_policy::stored_record_rows(connection, "runtime_projects")?,
        })
    }

    fn checked(sources: Self) -> StoreResult<Self> {
        for (table, rows) in [
            ("runtime_agents", &sources.agents),
            ("runtime_projects", &sources.projects),
        ] {
            for (id, raw) in rows {
                records::decode(table, id, raw)?;
            }
        }
        Ok(sources)
    }

    /// Read the rows like [`Self::read`], skipping malformed records as the
    /// Python sync projection owner does.
    pub fn read_for_sync(connection: &Connection) -> StoreResult<Self> {
        Ok(Self {
            agents: crate::sync_policy::projection_records(connection, "runtime_agents")?,
            projects: crate::sync_policy::projection_records(connection, "runtime_projects")?,
        })
    }
}

fn room_rows<T: DeserializeOwned>(rows: &[(String, String)], table: &str) -> StoreResult<Vec<T>> {
    rows.iter()
        .map(|(id, raw)| studio_json::from_str(raw).map_err(|_| records::invalid(table, id)))
        .collect()
}

/// Schema-validated snapshot; only the database loader can construct its contents.
pub struct RoomDirectory {
    context: RoomViewContext,
    projects: Vec<ProjectAuthorityRow>,
    sources: RoomSources,
}
impl RoomDirectory {
    pub(crate) fn load(tx: &Transaction<'_>) -> Result<Self, studio_runtime_store::StoreError> {
        Self::prepare(RoomSources::read(tx)?)
    }
    /// Prepare room membership from stored, already resolved path identities.
    /// This is pure; callers retaining the whole directory check [`Self::matches`].
    pub fn prepare(sources: RoomSources) -> StoreResult<Self> {
        let mut directory = Self::resolve(
            room_rows(&sources.agents, "runtime_agents")?,
            room_rows(&sources.projects, "runtime_projects")?,
        )?;
        directory.sources = sources;
        Ok(directory)
    }
    /// Read [`RoomSources`] from `store`, then prepare the room directory.
    pub async fn read_and_prepare(store: &studio_runtime_store::RuntimeStore) -> StoreResult<Self> {
        let sources = store
            .read_async("runtime-services/sync_entity_context", |tx| {
                RoomSources::read(tx)
            })
            .await?;
        studio_runtime_process::blocking_pool::spawn_blocking(
            "runtime-services/sync_entity_context",
            move || Self::prepare(sources),
        )
        .await
        .map_err(|_| studio_runtime_store::StoreError::ReadTaskFailed)?
    }
    /// Use already decoded rows inside the caller's transaction. Retained
    /// directories use `prepare`, which also captures the snapshot for rechecks.
    pub(crate) fn resolve(
        agents: Vec<AgentAuthorityRow>,
        projects: Vec<ProjectAuthorityRow>,
    ) -> StoreResult<Self> {
        let mut peers = Vec::new();
        for project in &projects {
            let ProjectAuthorityRow::Active {
                path, peer_teams, ..
            } = project
            else {
                continue;
            };
            if path.is_empty() {
                continue;
            }
            // Project/agent writers persist resolved host paths. Room membership
            // compares those stored identities; it must not inspect today's filesystem.
            let Some(path) =
                studio_runtime_store::path_identity::lexical(std::path::Path::new(path))
            else {
                // A project handle (for example project:<id>) is not a host
                // directory and cannot bind local peer membership.
                continue;
            };
            for team in peer_teams {
                let mut members = Vec::new();
                for id in &team.members {
                    if agents.iter().any(|a| {
                        a.id.as_ref() == Some(id)
                            && a.root_id.as_ref() == Some(id)
                            && a.is_lead == Some(true)
                            && !a.deleted_at.as_ref().is_some_and(Deletion::is_deleted)
                            && a.parent_id.as_deref().is_none_or(str::is_empty)
                            && a.cwd
                                .as_deref()
                                .and_then(|path| {
                                    studio_runtime_store::path_identity::lexical(
                                        std::path::Path::new(path),
                                    )
                                })
                                .as_ref()
                                == Some(&path)
                    }) && !members.contains(id)
                    {
                        members.push(id.clone());
                    }
                }
                if members.len() >= 2 {
                    let mut team = team.clone();
                    team.members = members;
                    peers.push(team);
                }
            }
        }
        Ok(Self {
            context: RoomViewContext::new(agents, peers),
            projects,
            sources: RoomSources::default(),
        })
    }
    /// Directory of live agents only, which is all a room view reads: peers,
    /// broadcast members and member names all exclude deleted agents.
    pub(crate) fn load_live(tx: &Connection) -> StoreResult<Self> {
        Self::prepare(RoomSources::checked(RoomSources {
            agents: studio_runtime_store::agent_volatile::merged_rows(
                tx,
                crate::sync_policy::live_agent_record_rows(tx)?,
            )?,
            projects: crate::sync_policy::stored_record_rows(tx, "runtime_projects")?,
        })?)
    }
    /// Build the live-agent sync context inside a writer transaction, skipping
    /// malformed rows like `RoomSources::read_for_sync`. Only agent liveness changes take
    /// this path, so path resolution under the writer stays rare.
    pub(crate) fn load_live_for_sync(tx: &Connection) -> StoreResult<Self> {
        Self::prepare(RoomSources {
            agents: crate::sync_policy::live_agent_records(tx)?,
            projects: crate::sync_policy::projection_records(tx, "runtime_projects")?,
        })
    }
    /// Recheck persisted inputs in the consumer's snapshot without resolving paths again.
    pub fn matches(&self, connection: &Connection) -> StoreResult<bool> {
        Ok(self.sources == RoomSources::read_raw(connection)?)
    }
    pub fn matches_for_sync(&self, connection: &Connection) -> StoreResult<bool> {
        Ok(self.sources == RoomSources::read_for_sync(connection)?)
    }
    pub(crate) fn agents(&self) -> &[AgentAuthorityRow] {
        &self.context.agents
    }
    pub(crate) fn projects(&self) -> &[ProjectAuthorityRow] {
        &self.projects
    }
    pub(crate) fn for_room_identity(
        &self,
        tx: &Connection,
        id: &str,
        federated: bool,
        last: bool,
    ) -> Result<RoomViewContext, studio_runtime_store::StoreError> {
        let last_message = if last && records::exists(tx, "runtime_chat_messages")? {
            let row=tx.query_row_cached("SELECT id,seq,text,created,sender,deliveries FROM runtime_chat_messages WHERE room=?1 ORDER BY seq DESC LIMIT 1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,f64>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?))).optional()?;
            row.map(|(key, seq, text, created, sender, deliveries)| {
                let _deliveries: std::collections::BTreeMap<
                    String,
                    studio_runtime_schema::persistence::RecipientDelivery,
                > = studio_json::from_str(&deliveries)
                    .map_err(|_| records::invalid("runtime_chat_messages", &key))?;
                let seq = studio_runtime_schema::room_record::ChatSequence::from_i64(seq)
                    .ok_or_else(|| records::invalid("runtime_chat_messages", &key))?;
                let created = studio_json::Number::from_f64(created)
                    .ok_or_else(|| records::invalid("runtime_chat_messages", &key))?;
                Ok::<_, studio_runtime_store::StoreError>(
                    studio_runtime_schema::room_record::RoomLastMessage {
                        seq,
                        text: text.chars().take(180).collect(),
                        created,
                        sender,
                    },
                )
            })
            .transpose()?
        } else {
            None
        };
        Ok(RoomViewContext {
            last_message,
            federation: if federated {
                records::get(tx, "runtime_federation_rooms", id)?
                    .map(|row| {
                        // The authoritative table schema was checked by get;
                        // this narrower schema belongs only to the sync view.
                        let valid =
                            studio_runtime_schema::validate("PersistedFederationRoomRecord", &row)
                                .map_err(studio_runtime_store::StoreError::SchemaValidation)?;
                        if !valid {
                            return Err(records::invalid("runtime_federation_rooms", id));
                        }
                        Ok(row)
                    })
                    .transpose()?
            } else {
                None
            },
            ..self.context.clone()
        })
    }
}

/// Pending input flags are list data, independent of whether a chat is visible.
fn pending_attention(
    transaction: &Connection,
    id: &str,
    record: &Value,
) -> StoreResult<(bool, bool)> {
    if !table_exists(transaction, "approvals")? {
        return Ok((false, false));
    }
    let epoch = record
        .get("epoch")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    transaction.query_row_cached(
        "SELECT EXISTS(SELECT 1 FROM approvals WHERE agent=?1 AND (epoch IS NULL OR epoch=?2) AND state='pending' AND COALESCE(deferral_active,0)=0 AND kind IN ('question','elicitation')), \
                EXISTS(SELECT 1 FROM approvals WHERE agent=?1 AND (epoch IS NULL OR epoch=?2) AND state='pending' AND COALESCE(deferral_active,0)=0 AND kind NOT IN ('question','elicitation'))",
        rusqlite::params![id, epoch], |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(Into::into)
}

#[cfg(test)]
mod room_preparation_tests {
    use super::*;
    use crate::submit_gates::{PeerTeamRow, StoredField};
    use std::sync::Arc;

    fn agent(id: &str) -> AgentAuthorityRow {
        AgentAuthorityRow {
            created: None,
            id: Some(id.to_owned()),
            root_id: None,
            parent_id: None,
            is_lead: None,
            deleted_at: None,
            cwd: None,
            name: StoredField::Missing,
            shared_room_id: None,
            execution_mode: None,
        }
    }

    #[cfg(unix)]
    #[test]
    fn room_membership_uses_symlink_paths_resolved_by_the_creation_owner() {
        let state = tempfile::tempdir().expect("state");
        let project = tempfile::tempdir().expect("project");
        let link = state.path().join("project-link");
        std::os::unix::fs::symlink(project.path(), &link).expect("project link");
        let first =
            studio_runtime_store::path_identity::CanonicalPath::existing_utf8_directory(&link)
                .expect("resolved linked cwd");
        let second = studio_runtime_store::path_identity::CanonicalPath::existing_utf8_directory(
            project.path(),
        )
        .expect("resolved real cwd");
        let agents = [("first", &first), ("second", &second)]
            .into_iter()
            .map(|(id, cwd)| AgentAuthorityRow {
                root_id: Some(id.to_owned()),
                is_lead: Some(true),
                cwd: Some(cwd.to_str().expect("UTF-8 path").to_owned()),
                ..agent(id)
            })
            .collect();
        let projects = vec![ProjectAuthorityRow::Active {
            id: Some("project".to_owned()),
            path: first.to_str().expect("UTF-8 path").to_owned(),
            peer_teams: vec![PeerTeamRow {
                id: "peers".to_owned(),
                name: "Peers".to_owned(),
                members: vec!["first".to_owned(), "second".to_owned()],
            }],
        }];
        let directory = RoomDirectory::resolve(agents, projects)
            .expect("room membership resolves canonical paths");
        let members = &directory.context.peer_teams;
        assert_eq!(members[0].members, ["first", "second"]);
    }

    #[test]
    fn paths_are_prepared_after_the_read_and_snapshot_is_rechecked() {
        let root = tempfile::tempdir().expect("isolated state");
        let mut connection = Connection::open_in_memory().expect("database");
        connection.execute_batch(&studio_runtime_store::agent_columns::fixture_sql("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT); CREATE TABLE runtime_projects(id TEXT PRIMARY KEY,record TEXT);")).expect("tables");
        connection
            .execute_batch(studio_runtime_store::agent_volatile::agent_volatile_table_sql())
            .expect("volatile agent table creates");
        connection
            .execute(
                "INSERT INTO runtime_projects(id,record) VALUES ('project',?1)",
                [
                    json!({"recordKind":"live","id":"project","path":root.path(),"name":"before","peerTeams":[]})
                        .to_string(),
                ],
            )
            .expect("project");
        let prepared =
            RoomDirectory::prepare(RoomSources::read(&connection).expect("read room sources"))
                .expect("prepare after the read");
        assert!(
            prepared
                .matches(&connection)
                .expect("unchanged source text")
        );
        connection.execute("UPDATE runtime_projects SET record=json_set(record,'$.name','after') WHERE id='project'", []).expect("project edit between prepare and transaction");
        let tx = connection.transaction().expect("read transaction");
        assert!(!prepared.matches(&tx).expect("check database preconditions"));
    }

    #[test]
    fn snapshot_rechecks_unprojected_and_volatile_agent_fields() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(&studio_runtime_store::agent_columns::fixture_sql(
                "CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT);",
            ))
            .expect("agents table");
        connection
            .execute_batch(studio_runtime_store::agent_volatile::agent_volatile_table_sql())
            .expect("volatile table");
        connection.execute("INSERT INTO runtime_agents(id,record) VALUES('agent',?1)", [json!({"compactions":0,"accountKey":"default","id":"agent","rootId":"agent","epoch":1,"status":"idle","provider":"codex"}).to_string()]).expect("agent");
        for sql in [
            "UPDATE runtime_agents SET record=json_set(record,'$.prompt','changed') WHERE id='agent'",
            "INSERT INTO runtime_agent_volatile(id,tail) VALUES('agent','\"changed\"')",
        ] {
            let directory =
                RoomDirectory::prepare(RoomSources::read(&connection).expect("sources"))
                    .expect("directory");
            assert!(directory.matches(&connection).expect("unchanged snapshot"));
            connection.execute(sql, []).expect("source edit");
            assert!(!directory.matches(&connection).expect("changed snapshot"));
        }
    }

    #[test]
    fn sync_context_skips_malformed_persisted_agents_and_projects() {
        let mut connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(
                "CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT); \
                 CREATE TABLE runtime_projects(id TEXT PRIMARY KEY,record TEXT); \
                 INSERT INTO runtime_agents(id,record) VALUES('broken-agent','{'); \
                 INSERT INTO runtime_projects(id,record) VALUES('broken-project','{');",
            )
            .expect("malformed source fixtures");
        connection
            .execute_batch(studio_runtime_store::agent_volatile::agent_volatile_table_sql())
            .expect("volatile agent table creates");

        assert!(RoomSources::read(&connection).is_err());
        let prepared = RoomDirectory::prepare(
            RoomSources::read_for_sync(&connection).expect("sync sources read"),
        )
        .expect("sync context skips malformed source rows");
        assert!(prepared.agents().is_empty());
        assert!(prepared.projects.is_empty());
        let tx = connection.transaction().expect("sync transaction");
        assert!(
            prepared
                .matches_for_sync(&tx)
                .expect("skip malformed rows on recheck")
        );
    }

    #[test]
    fn room_contexts_share_agents_peers_and_the_active_agent_index() {
        let directory = RoomDirectory::resolve(
            vec![
                agent("agent-a"),
                AgentAuthorityRow {
                    deleted_at: Some(Deletion::Number(1.0)),
                    ..agent("agent-b")
                },
            ],
            Vec::new(),
        )
        .expect("directory resolves");
        let connection = Connection::open_in_memory().expect("database");
        let first = directory
            .for_room_identity(&connection, "room", false, false)
            .expect("first room context");
        let second = directory
            .for_room_identity(&connection, "room", false, false)
            .expect("second room context");

        assert!(Arc::ptr_eq(&first.agents, &second.agents));
        assert!(Arc::ptr_eq(&first.peer_teams, &second.peer_teams));
        assert!(Arc::ptr_eq(
            &first.active_agent_indices,
            &second.active_agent_indices
        ));
        assert_eq!(first.active_agent_indices.len(), 1);
    }
}
