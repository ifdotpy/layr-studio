//! Persisted project metadata and its renderer sync projection.

use crate::CachedSql;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use studio_json::{Value, json};
use studio_runtime_schema::persistence::ProjectRemovalReason;
use studio_runtime_schema::project_record::{ProjectLive, ProjectRecord, ProjectRemoval};

use crate::request_results::AdmittedRequest;

use crate::peer_update_settings::{Canvas, canvas_read};
use crate::publication_schema::Publications;
use crate::schema::{ConditionalTableRequirement, SchemaRequirements, TableRequirement};
use crate::{StoreError, StoreResult, SyncDocumentsRepository};

#[derive(studio_db_api::DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
    tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),
    predicates(entities=rows),columns(id=Publications::ENTITY_ID,collection=Publications::COLLECTION_ORDER,deleted=Publications::DELETED,payload=Publications::ENTITY_PAYLOAD,rowid=Publications::SOURCE_ROWID),
    sql="SELECT {id} FROM {rows} WHERE {entities} AND {collection}='peerTeam' AND {deleted}=0 AND json_extract({payload},'$.value.projectPath')=?1 ORDER BY {rowid}")]
struct VisiblePeerTeams<'a> {
    #[db(column=Publications::ENTITY_PAYLOAD)]
    project_path: Option<&'a str>,
}

const PROJECT_TABLE: &[TableRequirement] = &[TableRequirement {
    name: "runtime_projects",
    columns: &["id", "record"],
}];
const SIDEBAR_TABLE: &[TableRequirement] = &[TableRequirement {
    name: <crate::metadata_schema::Metadata as studio_db_api::Table>::NAME,
    columns: &[
        crate::metadata_schema::Metadata::__DB_COLUMN_KIND,
        crate::metadata_schema::Metadata::__DB_COLUMN_IDENTITY,
        crate::metadata_schema::Metadata::__DB_COLUMN_SOURCE_ROWID,
        crate::metadata_schema::Metadata::__DB_COLUMN_TEXT_KEY,
        crate::metadata_schema::Metadata::__DB_COLUMN_SIDEBAR_RECORD,
    ],
}];
const AGENT_TABLE: &[TableRequirement] = &[TableRequirement {
    name: "runtime_agents",
    columns: &["id", "record"],
}];
const ROOM_TABLE: &[TableRequirement] = &[TableRequirement {
    name: "runtime_rooms",
    columns: &["id", "record"],
}];

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[],
    conditional_tables: &[
        ConditionalTableRequirement {
            table_exists: "runtime_agents",
            tables: AGENT_TABLE,
        },
        ConditionalTableRequirement {
            table_exists: "runtime_rooms",
            tables: ROOM_TABLE,
        },
    ],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[PROJECT_TABLE[0], SIDEBAR_TABLE[0]],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

#[derive(Clone, Copy, Debug, Default)]
pub struct ProjectsRepository;

impl ProjectsRepository {
    /// Distinguish a destructive project lifetime fence from sidebar-only removal.
    /// The recorded sequence is the exact project tombstone's durable sequence.
    pub fn record_destructive_removal(
        tx: &Transaction<'_>,
        id: &str,
        request_id: &str,
        deleted_at: i64,
    ) -> StoreResult<()> {
        let seq = super::sync_documents::publication_sequence(tx, "project", id, true)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let removal = ProjectRemoval {
            reason: ProjectRemovalReason::Delete,
            sequence: seq,
            request_id: request_id.to_owned(),
            deleted_at,
        };
        crate::peer_update_settings::canvas_tx(tx).upsert(&crate::planning_work::ProjectRow {
            id: id.to_owned(),
            record: crate::json_record_codec::ExactJson::from_stored(
                json!({"recordKind":"removal","id":id,"removal":removal,"peerTeams":[]})
                    .to_string(),
            )?,
            removal_reason: None,
        })?;
        Ok(())
    }
    pub fn is_destructive_removal(
        connection: &Connection,
        id: &str,
        seq: i64,
    ) -> StoreResult<bool> {
        let raw: Option<String> = connection
            .query_row(
                // JSON expression selectors retain the existing SQL semantics.
                "SELECT json_extract(record,'$.removal') FROM runtime_projects WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let removal: Option<ProjectRemoval> =
            raw.map(|raw| studio_json::from_str(&raw)).transpose()?;
        Ok(removal.is_some_and(|removal| {
            removal.reason == ProjectRemovalReason::Delete && removal.sequence == seq
        }))
    }

    pub fn list(connection: &Connection) -> StoreResult<Vec<(String, Value)>> {
        Ok(Self::owned(connection)?
            .into_iter()
            .filter(|(_, project)| {
                project.get("sidebarHidden").and_then(Value::as_bool) != Some(true)
            })
            .collect())
    }

    /// Active project owners, including projects hidden from navigation.
    pub fn owned(connection: &Connection) -> StoreResult<Vec<(String, Value)>> {
        let mut statement = connection.prepare_cached(
            // JSON expression selectors retain the existing SQL semantics.
            "SELECT id,record FROM runtime_projects WHERE json_extract(record,'$.removal') IS NULL ORDER BY json_extract(record,'$.created'),id",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (id, raw) = row?;
                let value =
                    studio_json::from_str(&raw).map_err(|_| StoreError::InvalidStoredRecord {
                        object: "runtime_projects",
                        id: id.clone(),
                    })?;
                Ok((id, value))
            })
            .collect()
    }

    pub fn get(connection: &Connection, id: &str) -> StoreResult<Option<(String, Value)>> {
        let raw = connection
            .query_row(
                // JSON expression selectors retain the existing SQL semantics.
                "SELECT record FROM runtime_projects WHERE id=?1 AND json_extract(record,'$.removal') IS NULL",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        raw.map(|raw| {
            Ok((
                id.to_owned(),
                studio_json::from_str(&raw).map_err(|_| StoreError::InvalidStoredRecord {
                    object: "runtime_projects",
                    id: id.to_owned(),
                })?,
            ))
        })
        .transpose()
    }

    pub fn get_record(connection: &Connection, id: &str) -> StoreResult<Option<ProjectLive>> {
        let raw: Option<String> = connection
            .query_row(
                // JSON expression selectors retain the existing SQL semantics.
                "SELECT record FROM runtime_projects WHERE id=?1 AND json_extract(record,'$.removal') IS NULL",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        raw.map(|raw| {
            let record: ProjectRecord =
                studio_json::from_str(&raw).map_err(|_| StoreError::InvalidStoredRecord {
                    object: "runtime_projects",
                    id: id.to_owned(),
                })?;
            match record {
                ProjectRecord::Live(project) if project.id == id => Ok(*project),
                ProjectRecord::Live(_) | ProjectRecord::Removal(_) => {
                    Err(StoreError::InvalidStoredRecord {
                        object: "runtime_projects",
                        id: id.to_owned(),
                    })
                }
            }
        })
        .transpose()
    }

    pub fn list_records(connection: &Connection) -> StoreResult<Vec<(String, ProjectLive)>> {
        let mut statement = connection.prepare_cached(
            // JSON expression selectors retain the existing SQL semantics.
            "SELECT id,record FROM runtime_projects WHERE json_extract(record,'$.removal') IS NULL AND COALESCE(json_type(record,'$.sidebarHidden'),'')!='true' ORDER BY json_extract(record,'$.created'),id",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (id, raw) = row?;
                let project: ProjectRecord =
                    studio_json::from_str(&raw).map_err(|_| StoreError::InvalidStoredRecord {
                        object: "runtime_projects",
                        id: id.clone(),
                    })?;
                match project {
                    ProjectRecord::Live(project) if project.id == id => Ok((id, *project)),
                    ProjectRecord::Live(_) | ProjectRecord::Removal(_) => {
                        Err(StoreError::InvalidStoredRecord {
                            object: "runtime_projects",
                            id,
                        })
                    }
                }
            })
            .collect()
    }

    pub fn location_request(
        connection: &Connection,
        project: &str,
        operation: &str,
    ) -> StoreResult<Option<AdmittedRequest>> {
        let raw: Option<String> = connection.query_row(
            // JSON expression selectors retain the existing SQL semantics.
            "SELECT json_extract(intent.value,'$.request') FROM runtime_projects,json_each(record,'$.locationIntents') AS intent WHERE runtime_projects.id=?1 AND intent.key=?2",
            params![project,operation], |row|row.get(0),
        ).optional()?;
        raw.map(|raw| studio_json::from_str(&raw))
            .transpose()
            .map_err(Into::into)
    }

    pub fn put_location_request(
        tx: &Transaction<'_>,
        project: &str,
        operation: &str,
        request: &AdmittedRequest,
    ) -> StoreResult<()> {
        if let Some(previous) = Self::location_request(tx, project, operation)?
            && previous != *request
        {
            return Err(crate::request_results::RequestResultError::Conflict {
                id: request.id().to_owned(),
            }
            .into());
        }
        let changed=tx.execute(
            // JSON expression selectors retain the existing SQL semantics.
            "UPDATE runtime_projects SET record=json_patch(record,json_object('locationIntents',json_object(?2,json_object('request',json(?3))))) WHERE id=?1 AND json_extract(record,'$.removal') IS NULL",
            params![project,operation,studio_json::to_string(request)?],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidStoredRecord {
                object: "runtime_projects",
                id: project.to_owned(),
            });
        }
        Ok(())
    }

    pub fn remove_location_request(
        tx: &Transaction<'_>,
        project: &str,
        operation: &str,
    ) -> StoreResult<()> {
        tx.execute(
            // JSON expression selectors retain the existing SQL semantics.
            "UPDATE runtime_projects SET record=json_patch(record,json_object('locationIntents',json_object(?2,NULL))) WHERE id=?1",
            params![project,operation],
        )?;
        Ok(())
    }

    pub fn agents(connection: &Connection) -> StoreResult<Vec<(String, Value)>> {
        let exists: bool = connection.query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='runtime_agents')",
            [], |row| row.get(0),
        )?;
        if !exists {
            return Ok(Vec::new());
        }
        let mut statement = connection.prepare_cached("SELECT id,record FROM runtime_agents")?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (id, raw) = row?;
                let mut value =
                    studio_json::from_str(&raw).map_err(|_| StoreError::InvalidStoredRecord {
                        object: "runtime_agents",
                        id: id.clone(),
                    })?;
                crate::agent_volatile::merge(connection, &id, &mut value)?;
                Ok((id, value))
            })
            .collect()
    }

    pub fn put_live(transaction: &Transaction<'_>, project: &ProjectLive) -> StoreResult<()> {
        let mut project = project.clone();
        if let Some(previous) = Self::get_record(transaction, &project.id)? {
            macro_rules! preserve_missing {
                ($($field:ident),+ $(,)?) => {$({
                    if project.$field.is_missing() {
                        project.$field = previous.$field;
                    }
                })+};
            }
            preserve_missing!(
                peer_teams,
                account_key,
                account_groups,
                account_keys,
                name,
                locations,
                project_aliases,
                locations_revision,
                account_revision,
                account_groups_revision,
                home_server_id,
                created,
                updated,
                sidebar_hidden,
                peer_teams_revision,
                organization_revision,
                folders,
                worker_base_ref,
                worker_base_revision,
                worker_environment,
                worker_environment_revision,
                workspace_policy,
                workspace_policy_revision,
                layr_store_root,
                layr_store_root_revision,
                location_intents,
                registration
            );
        }
        let record = studio_json::to_string(&project)?;
        let stored: String = transaction.query_row(
            // SQLite JSON tagging and RETURNING preserve the existing stored/sync encoding.
            "INSERT INTO runtime_projects(id,record) VALUES (?1,json_set(?2,'$.recordKind','live')) \
             ON CONFLICT(id) DO UPDATE SET record=excluded.record RETURNING record",
            params![project.id, record],
            |row| row.get(0),
        )?;
        let record = studio_json::value::RawValue::from_string(stored)?;
        SyncDocumentsRepository::put_entity(
            transaction,
            "project",
            &project.id,
            record,
            project.sidebar_hidden.value() == Some(&true),
        )?;
        Ok(())
    }

    pub fn put_project(transaction: &Transaction<'_>, project: &Value) -> StoreResult<()> {
        let id = project
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| StoreError::SchemaMismatch("project id is invalid".to_owned()))?;
        let record = studio_json::to_string(project)?;
        transaction.execute(
            // SQLite JSON tagging preserves the existing stored encoding.
            "INSERT INTO runtime_projects(id,record) VALUES (?1,json_set(?2,'$.recordKind','live')) \
             ON CONFLICT(id) DO UPDATE SET record=excluded.record",
            params![id, record],
        )?;
        SyncDocumentsRepository::put_entity(
            transaction,
            "project",
            id,
            project.clone(),
            project.get("sidebarHidden").and_then(Value::as_bool) == Some(true),
        )?;
        Ok(())
    }

    /// Update selected fields while preserving the rest of the stored project.
    pub fn patch_project<T: Serialize>(
        transaction: &Transaction<'_>,
        id: &str,
        patch: &T,
    ) -> StoreResult<()> {
        let patch = studio_json::to_string(patch)?;
        let (record, hidden): (String, bool) = transaction.query_row(
            // JSON expression selectors retain the existing SQL semantics.
            "UPDATE runtime_projects SET record=json_patch(record,json(?2)) WHERE id=?1 AND json_extract(record,'$.removal') IS NULL RETURNING record,json_type(record,'$.sidebarHidden')='true'",
            params![id, patch],
            |row| Ok((row.get(0)?, row.get::<_, Option<bool>>(1)?.unwrap_or(false))),
        )?;
        let record = studio_json::value::RawValue::from_string(record)?;
        SyncDocumentsRepository::put_entity(transaction, "project", id, record, hidden)?;
        Ok(())
    }

    pub fn remove(transaction: &Transaction<'_>, id: &str) -> StoreResult<bool> {
        let Some((_, mut existing)) = Self::get(transaction, id)? else {
            return Ok(false);
        };
        if existing.get("sidebarHidden").and_then(Value::as_bool) == Some(true) {
            return Ok(false);
        }
        existing["sidebarHidden"] = json!(true);
        let removed = transaction.execute(
            // Recovery reads retain their historical admission semantics.
            "UPDATE runtime_projects SET record=?2 WHERE id=?1",
            params![id, existing.to_string()],
        )? > 0;
        if removed {
            SyncDocumentsRepository::put_entity(transaction, "project", id, json!({}), true)?;
            let ids = canvas_read(transaction).collect(
                &VisiblePeerTeams::QUERY,
                &VisiblePeerTeams {
                    project_path: Some(id),
                },
            )?;
            for team_id in ids {
                SyncDocumentsRepository::put_entity(
                    transaction,
                    "peerTeam",
                    &team_id,
                    json!({}),
                    true,
                )?;
            }
            let room_table: bool = transaction.query_row_cached(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='runtime_rooms')",
                [], |row| row.get(0),
            )?;
            if !room_table {
                return Ok(removed);
            }
            let mut rooms = transaction.prepare_cached(
                "SELECT id FROM runtime_rooms WHERE json_extract(record,'$.kind')='private' \
                 AND json_extract(record,'$.projectPath')=?1",
            )?;
            let room_ids = rooms
                .query_map([id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            drop(rooms);
            for room_id in room_ids {
                Self::refresh_private_room(transaction, &room_id)?;
            }
        }
        Ok(removed)
    }

    pub fn refresh_private_room(transaction: &Transaction<'_>, id: &str) -> StoreResult<()> {
        let Some(mut room) = crate::interaction_tables::RoomRow::get(transaction, id)? else {
            return Ok(());
        };
        if room.project_deleted_at.is_some() {
            SyncDocumentsRepository::put_entity(transaction, "room", id, json!({}), true)?;
            return Ok(());
        }
        let members = room.members.clone().unwrap_or_default();
        let mut live = Vec::new();
        for member in &members {
            if let Some((key, agent)) = Self::agent(transaction, member)? {
                if agent.get("id").and_then(Value::as_str) != Some(key.as_str()) {
                    return Err(StoreError::InvalidStoredRecord {
                        object: "runtime_agents",
                        id: key,
                    });
                }
                if agent.get("deletedAt").is_none_or(Value::is_null) {
                    live.push(member.clone());
                }
            }
        }
        if live.is_empty() || live.len() != members.len() {
            SyncDocumentsRepository::put_entity(transaction, "room", id, json!({}), true)?;
            return Ok(());
        }
        room.members = Some(live.clone());
        let project_path = room.project_path.as_ref().map(String::as_str);
        let teams = if let Some(path) = project_path {
            Self::get(transaction, path)?
                .filter(|(_, project)| {
                    project.get("sidebarHidden").and_then(Value::as_bool) != Some(true)
                })
                .and_then(|(_, project)| {
                    project.get("peerTeams").and_then(Value::as_array).cloned()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if let Some(team) = teams.iter().find(|team| {
            let team_members = team.get("members").and_then(Value::as_array);
            team_members.is_some_and(|members| {
                live.iter().all(|member| {
                    members
                        .iter()
                        .any(|id| id.as_str() == Some(member.as_str()))
                })
            })
        }) {
            room.peer_team_id = team
                .get("id")
                .and_then(Value::as_str)
                .map_or(studio_runtime_schema::queue_command::Field::Null, |s| {
                    studio_runtime_schema::queue_command::Field::Value(s.to_owned())
                });
            room.peer_team_name = team
                .get("name")
                .and_then(Value::as_str)
                .map_or(studio_runtime_schema::queue_command::Field::Null, |s| {
                    studio_runtime_schema::queue_command::Field::Value(s.to_owned())
                });
        } else {
            room.peer_team_id = studio_runtime_schema::queue_command::Field::Missing;
            room.peer_team_name = studio_runtime_schema::queue_command::Field::Missing;
        }
        SyncDocumentsRepository::put_entity(transaction, "room", id, room, false)?;
        Ok(())
    }

    pub fn agent(connection: &Connection, id: &str) -> StoreResult<Option<(String, Value)>> {
        let raw = connection
            .query_row(
                "SELECT record FROM runtime_agents WHERE id=?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        raw.map(|raw| {
            let mut record =
                studio_json::from_str(&raw).map_err(|_| StoreError::InvalidStoredRecord {
                    object: "runtime_agents",
                    id: id.to_owned(),
                })?;
            crate::agent_volatile::merge(connection, id, &mut record)?;
            Ok((id.to_owned(), record))
        })
        .transpose()
    }

    /// Preserve the stored row's presence separately from the default sidebar order.
    pub fn sidebar_order_record(connection: &Connection) -> StoreResult<Option<String>> {
        Ok(crate::peer_update_settings::canvas_read(connection)
            .get::<crate::metadata_schema::SidebarOrder>(&(
                crate::consolidation_codec::nullable_text_identity(&Some("current".into()), 0),
            ))
            .map_err(|error| {
                if matches!(&error, rusqlite::Error::FromSqlConversionFailure(..)) {
                    StoreError::InvalidStoredRecord {
                        object: "runtime_sidebar_order",
                        id: "current".into(),
                    }
                } else {
                    StoreError::from(error)
                }
            })?
            .map(|row| row.sidebar_record)
            .map(|raw| raw.raw().to_owned()))
    }

    pub fn sidebar_order(connection: &Connection) -> StoreResult<Value> {
        Self::sidebar_order_record(connection)?
            .map(|raw| {
                studio_json::from_str(&raw).map_err(|_| StoreError::InvalidStoredRecord {
                    object: "runtime_sidebar_order",
                    id: "current".to_owned(),
                })
            })
            .transpose()
            .map(|value| value.unwrap_or_else(|| json!({"revision": 0, "groups": null})))
    }

    pub fn put_sidebar_order(transaction: &Transaction<'_>, value: &Value) -> StoreResult<()> {
        crate::peer_update_settings::canvas_tx(transaction)
            .partition::<crate::metadata_schema::SidebarOrder>()
            .put(&crate::metadata_schema::SidebarOrder {
                identity: crate::consolidation_codec::nullable_text_identity(
                    &Some("current".into()),
                    0,
                ),
                source_rowid: 0,
                text_key: Some("current".into()),
                sidebar_record: crate::metadata_values::SidebarOrderRecord::from_stored(
                    studio_json::to_string(value)?,
                )?,
            })?;
        if super::sync_documents::publication_sequence(transaction, "workspace", "current", false)?
            .is_some()
        {
            SyncDocumentsRepository::patch_entity(
                transaction,
                "workspace",
                "current",
                json!({"sidebarOrder":value}),
            )?;
        }
        Ok(())
    }
}

inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "projects",
        requirements: &REQUIREMENTS,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn canonical_sidebar_write_does_not_create_an_absent_workspace() {
        use studio_db_api::Table;
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        crate::metadata_schema::Metadata::descriptor()
            .install_if_absent(&db)
            .unwrap();
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let transaction = db.transaction().unwrap();
        ProjectsRepository::put_sidebar_order(&transaction, &json!({"revision":1,"groups":null}))
            .unwrap();
        assert_eq!(
            crate::repositories::sync_documents::publication_sequence(
                &transaction,
                "workspace",
                "current",
                false
            )
            .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn canonical_sidebar_write_updates_workspace_and_rolls_back() {
        let root = tempfile::tempdir().unwrap();
        let store = crate::RuntimeStore::open(Some(root.path())).await.unwrap();
        let value = studio_json::json!({"revision":7,"groups":null});
        let expected = value.clone();
        store
            .transaction(move |tx| {
                ProjectsRepository::put_sidebar_order(tx, &value)?;
                let raw = crate::repositories::sync_documents::publication_payload(
                    tx,
                    "workspace",
                    "current",
                    true,
                )?
                .unwrap();
                let workspace: Value = studio_json::from_str(&raw)?;
                assert_eq!(workspace["value"]["sidebarOrder"], value);
                Ok(())
            })
            .await
            .expect("actual canonical sidebar caller");
        let reader = store.test_reader().unwrap();
        assert_eq!(
            ProjectsRepository::sidebar_order(&reader).unwrap(),
            expected
        );
        let result: crate::StoreResult<()> = store
            .transaction(|tx| {
                ProjectsRepository::put_sidebar_order(
                    tx,
                    &studio_json::json!({"revision":8,"groups":[]}),
                )?;
                Err(crate::StoreError::SchemaMismatch("test rollback".into()))
            })
            .await;
        assert!(matches!(result, Err(crate::StoreError::SchemaMismatch(_))));
        assert_eq!(
            ProjectsRepository::sidebar_order(&reader).unwrap(),
            expected
        );
    }
    #[test]
    fn f1_project_hide_reads_canonical_peer_team_scope_and_rolls_back() {
        use studio_db_api::Table;
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        for descriptor in [
            crate::publication_schema::Publications::descriptor(),
            crate::metadata_schema::Metadata::descriptor(),
            crate::change_schema::Changes::descriptor(),
            crate::planning_work::ProjectRow::descriptor(),
        ] {
            descriptor.install_if_absent(&db).unwrap();
        }
        let tx = db.transaction().unwrap();
        crate::peer_update_settings::canvas_tx(&tx)
            .upsert(&crate::planning_work::ProjectRow {
                id: "p".into(),
                record: crate::json_record_codec::ExactJson::from_stored(
                    r#"{"recordKind":"live","id":"p","path":"p"}"#.into(),
                )
                .unwrap(),
                removal_reason: None,
            })
            .unwrap();
        tx.commit().unwrap();
        let tx = db.transaction().unwrap();
        for (collection, id, value, deleted) in [
            (
                "peerTeam",
                "matching",
                studio_json::json!({"projectPath":"p"}),
                false,
            ),
            (
                "peerTeam",
                "first",
                studio_json::json!({"projectPath":"p"}),
                false,
            ),
            (
                "peerTeam",
                "other",
                studio_json::json!({"projectPath":"q"}),
                false,
            ),
            ("peerTeam", "missing", studio_json::json!({}), false),
            (
                "peerTeam",
                "null",
                studio_json::json!({"projectPath":null}),
                false,
            ),
            (
                "peerTeam",
                "deleted",
                studio_json::json!({"projectPath":"p"}),
                true,
            ),
            (
                "other",
                "matching",
                studio_json::json!({"projectPath":"p"}),
                false,
            ),
        ] {
            crate::SyncDocumentsRepository::put_entity(&tx, collection, id, value, deleted)
                .unwrap();
        }
        let identity =
            studio_db_api::migration::encode_key(&("peerTeam".to_owned(), "first".to_owned()));
        let mut first = super::canvas_read(&tx)
            .get::<crate::publication_schema::SyncEntities>(&(identity,))
            .unwrap()
            .unwrap();
        first.source_rowid = -10;
        crate::peer_update_settings::canvas_tx(&tx)
            .partition::<crate::publication_schema::SyncEntities>()
            .put(&first)
            .unwrap();
        let params = super::VisiblePeerTeams {
            project_path: Some("p"),
        };
        let plan = crate::query_plan_test_support::explain(
            &tx,
            super::VisiblePeerTeams::QUERY.sql(),
            &params,
        )
        .unwrap();
        assert!(
            plan.iter().any(|line| line.contains("SEARCH")
                && line.contains("publications_entity_collection_deleted")),
            "{plan:?}"
        );
        let mut counts = Vec::new();
        for history in [0, 2000] {
            for n in 0..history {
                crate::SyncDocumentsRepository::put_entity(
                    &tx,
                    "history",
                    &n.to_string(),
                    (),
                    false,
                )
                .unwrap();
                crate::SyncDocumentsRepository::put_entity(
                    &tx,
                    "peerTeam",
                    &format!("old-{n}"),
                    (),
                    true,
                )
                .unwrap();
            }
            use std::sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            };
            let steps = Arc::new(AtomicUsize::new(0));
            let counter = steps.clone();
            tx.progress_handler(
                1,
                Some(move || {
                    counter.fetch_add(1, Ordering::Relaxed);
                    false
                }),
            )
            .unwrap();
            let ids = super::canvas_read(&tx)
                .collect(&super::VisiblePeerTeams::QUERY, &params)
                .unwrap();
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            counts.push(steps.load(Ordering::Relaxed));
            assert_eq!(ids, ["first", "matching"]);
        }
        eprintln!("peer-team visibility VM steps: {counts:?}");
        assert!(counts[1] <= counts[0] + 20 && counts[1] < 200, "{counts:?}");
        assert!(super::ProjectsRepository::remove(&tx, "p").unwrap());
        assert!(
            super::super::sync_documents::publication_sequence(&tx, "peerTeam", "matching", true)
                .unwrap()
                .is_some()
        );
        for id in ["other", "missing", "null"] {
            assert!(
                super::super::sync_documents::publication_sequence(&tx, "peerTeam", id, false)
                    .unwrap()
                    .is_some()
            );
        }
        assert!(
            super::super::sync_documents::publication_sequence(&tx, "other", "matching", false)
                .unwrap()
                .is_some()
        );
        tx.rollback().unwrap();
        assert_eq!(
            super::ProjectsRepository::get(&db, "p")
                .unwrap()
                .unwrap()
                .1
                .get("sidebarHidden"),
            None
        );
        let tx = db.transaction().unwrap();
        crate::SyncDocumentsRepository::put_entity_payload(
            &tx,
            "peerTeam",
            "malformed",
            "not JSON",
            false,
        )
        .unwrap();
        assert!(super::ProjectsRepository::remove(&tx, "p").is_err());
        tx.rollback().unwrap();
        assert_eq!(
            super::ProjectsRepository::get(&db, "p")
                .unwrap()
                .unwrap()
                .1
                .get("sidebarHidden"),
            None
        );
    }
    use super::*;

    #[test]
    fn sidebar_metadata_preserves_default_legacy_json_and_workspace_presence() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::test_fixtures::canvas::install_canonical(&db).unwrap();
        assert_eq!(
            ProjectsRepository::sidebar_order(&db).unwrap(),
            json!({"revision":0,"groups":null})
        );
        let tx = db.transaction().unwrap();
        for value in [
            json!(null),
            json!(42),
            json!([1, "old"]),
            json!({"groups":false}),
        ] {
            ProjectsRepository::put_sidebar_order(&tx, &value).unwrap();
            assert_eq!(ProjectsRepository::sidebar_order(&tx).unwrap(), value);
        }
        for deleted in [1, -7, 2, 0] {
            crate::test_fixtures::canvas::seed_entity(
                &tx,
                &crate::clock_schema::SyncEntities {
                    collection: "workspace".into(),
                    id: "current".into(),
                    seq: 1,
                    hash: "hash-only".into(),
                    payload: None,
                    deleted,
                },
                1,
            )
            .unwrap();
            let result = ProjectsRepository::put_sidebar_order(&tx, &json!({"revision":7}));
            if deleted == 0 {
                assert!(matches!(
                    result,
                    Err(StoreError::Sqlite(rusqlite::Error::InvalidColumnType(..)))
                ));
            } else {
                result.unwrap();
            }
        }
        SyncDocumentsRepository::put_entity(
            &tx,
            "workspace",
            "current",
            json!({"unrelated":7}),
            false,
        )
        .unwrap();
        let order = json!({"revision":9,"groups":null});
        ProjectsRepository::put_sidebar_order(&tx, &order).unwrap();
        let published = crate::test_fixtures::canvas::entities(&tx)
            .unwrap()
            .into_iter()
            .find(|row| row.collection == "workspace" && row.id == "current")
            .unwrap();
        let envelope: Value = studio_json::from_str(published.payload.as_ref().unwrap()).unwrap();
        assert_eq!(envelope["value"]["sidebarOrder"], order);
        assert_eq!(envelope["value"]["unrelated"], 7);
        ProjectsRepository::put_sidebar_order(&tx, &order).unwrap();
        assert_eq!(
            crate::test_fixtures::canvas::entities(&tx)
                .unwrap()
                .into_iter()
                .find(|row| row.collection == "workspace" && row.id == "current")
                .unwrap()
                .seq,
            published.seq
        );
    }

    #[test]
    fn typed_project_write_keeps_omitted_fields_and_explicit_null() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let original: ProjectLive =
            studio_json::from_str(r#"{"id":"p","path":"/p","name":"Old","sidebarHidden":true}"#)
                .unwrap();
        ProjectsRepository::put_live(&tx, &original).unwrap();
        let update: ProjectLive =
            studio_json::from_str(r#"{"id":"p","path":"/p","name":null}"#).unwrap();
        ProjectsRepository::put_live(&tx, &update).unwrap();
        let saved: String = tx
            .query_row(
                "SELECT record FROM runtime_projects WHERE id='p'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let saved: Value = studio_json::from_str(&saved).unwrap();
        assert_eq!(saved["name"], Value::Null);
        assert_eq!(saved["sidebarHidden"], true);
    }

    #[test]
    fn typed_project_write_preserves_missing_null_and_empty() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let project: ProjectLive = studio_json::from_str(
            r#"{"id":"p","path":"/p","name":null,"peerTeams":[],"locations":null}"#,
        )
        .unwrap();
        ProjectsRepository::put_live(&tx, &project).unwrap();
        let saved: String = tx
            .query_row(
                "SELECT record FROM runtime_projects WHERE id='p'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let value: Value = studio_json::from_str(&saved).unwrap();
        assert_eq!(
            value,
            json!({"recordKind":"live","id":"p","path":"/p","name":null,"peerTeams":[],"locations":null})
        );
        let restored = ProjectsRepository::get_record(&tx, "p").unwrap().unwrap();
        ProjectsRepository::put_live(&tx, &restored).unwrap();
        let again: String = tx
            .query_row(
                "SELECT record FROM runtime_projects WHERE id='p'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(saved, again);
    }

    #[test]
    fn sidebar_hide_retains_pending_location_owner() {
        use crate::request_results::{Admission, RequestResults};
        use studio_runtime_schema::persistence::RequestIdentity;
        let mut db = crate::fresh_state::test_canvas();
        crate::receipt_schema::create(&db).expect("request schema");
        let tx = db.transaction().expect("transaction");
        ProjectsRepository::put_project(&tx, &json!({"id":"p","locations":[]})).expect("project");
        let root = RequestIdentity::issue_root();
        let identity =
            RequestIdentity::new("project-location", &["server", "ui"], &root, &["location"])
                .expect("identity");
        let now = identity.issued_ms();
        let Admission::Fresh(admitted) =
            RequestResults::admit(&tx, &identity, "signature", now).expect("admit")
        else {
            panic!("fresh request")
        };
        ProjectsRepository::put_location_request(&tx, "p", "operation", &admitted).expect("intent");
        assert!(ProjectsRepository::remove(&tx, "p").expect("hide"));
        assert_eq!(
            ProjectsRepository::location_request(&tx, "p", "operation").expect("intent"),
            Some(admitted)
        );
        assert!(ProjectsRepository::list(&tx).expect("sidebar").is_empty());
        assert_eq!(
            ProjectsRepository::get(&tx, "p")
                .expect("owner")
                .expect("retained")
                .1["sidebarHidden"],
            true
        );
        assert!(!ProjectsRepository::remove(&tx, "p").expect("second hide"));
    }

    fn canonical_project_entity(db: &Connection, deleted: crate::metadata_values::StoredBoolean) {
        use crate::peer_update_settings::canvas_tx;
        use crate::publication_collection::CollectionName;
        use crate::publication_schema::{EntityValue, SyncEntities};
        use studio_db_api::migration::encode_key;
        let collection = CollectionName::from_stored("project".into());
        let tx = db.unchecked_transaction().unwrap();
        canvas_tx(&tx)
            .partition::<SyncEntities>()
            .put(&SyncEntities {
                identity: encode_key(&("project".to_owned(), "p".to_owned())),
                source_rowid: 7,
                publication_seq: 17,
                deleted,
                collection_kind: collection.kind(),
                collection_agent: collection.agent().cloned(),
                non_transcript: collection.non_transcript(),
                collection_order: collection,
                entity_id: "p".into(),
                entity_hash: "hash".into(),
                entity_payload: EntityValue::from_stored("project", "p", deleted, None).unwrap(),
            })
            .unwrap();
        tx.commit().unwrap();
    }

    #[test]
    fn canonical_project_caller_preserves_missing_deleted_and_rollback() {
        use crate::metadata_values::StoredBoolean;
        use crate::peer_update_settings::canvas_read;
        use crate::planning_work::ProjectRow;
        use studio_db_api::Table;
        let mut db = Connection::open_in_memory().unwrap();
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&db)
            .unwrap();
        ProjectRow::descriptor().install_if_absent(&db).unwrap();
        for deleted in [
            None,
            Some(StoredBoolean::Valid(false)),
            Some(StoredBoolean::LegacyInteger(2)),
        ] {
            if let Some(deleted) = deleted {
                canonical_project_entity(&db, deleted);
            }
            let tx = db.transaction().unwrap();
            assert!(matches!(
                ProjectsRepository::record_destructive_removal(&tx, "p", "request", 1000),
                Err(StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
            ));
            tx.rollback().unwrap();
            assert!(
                canvas_read(&db)
                    .get::<ProjectRow>(&("p".to_owned(),))
                    .unwrap()
                    .is_none()
            );
        }
        canonical_project_entity(&db, StoredBoolean::Valid(true));
        let tx = db.transaction().unwrap();
        ProjectsRepository::record_destructive_removal(&tx, "p", "request", 1000).unwrap();
        let row = canvas_read(&tx)
            .get::<ProjectRow>(&("p".to_owned(),))
            .unwrap()
            .unwrap();
        assert_eq!(
            row.record.raw(),
            r#"{"id":"p","peerTeams":[],"recordKind":"removal","removal":{"deletedAt":1000,"reason":"delete","requestId":"request","sequence":17}}"#
        );
        tx.rollback().unwrap();
        assert!(
            canvas_read(&db)
                .get::<ProjectRow>(&("p".to_owned(),))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn destructive_fence_survives_sidebar_removal_on_project_owner() {
        let mut db = Connection::open_in_memory().expect("database");
        use studio_db_api::Table;
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&db)
            .unwrap();
        crate::planning_work::ProjectRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        canonical_project_entity(&db, crate::metadata_values::StoredBoolean::Valid(true));
        let tx = db.transaction().expect("transaction");
        ProjectsRepository::record_destructive_removal(&tx, "p", "request", 1000).expect("fence");
        assert!(ProjectsRepository::is_destructive_removal(&tx, "p", 17).expect("fence read"));
        assert!(!ProjectsRepository::remove(&tx, "p").expect("sidebar hide"));
        let record: String = tx
            .query_row(
                "SELECT record FROM runtime_projects WHERE id='p'",
                [],
                |row| row.get(0),
            )
            .expect("durable project tombstone");
        let record: Value = studio_json::from_str(&record).expect("project");
        assert_eq!(record["removal"]["reason"], "delete");
        assert_eq!(record["removal"]["sequence"], 17);
        assert!(!ProjectsRepository::is_destructive_removal(&tx, "p", 18).expect("other lifetime"));
        assert!(
            ProjectsRepository::list(&tx)
                .expect("visible projects")
                .is_empty()
        );
    }
}
