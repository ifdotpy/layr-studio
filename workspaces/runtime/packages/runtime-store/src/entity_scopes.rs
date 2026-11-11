//! Team ownership is fixed when an entity is created.

use rusqlite::Connection;
use std::collections::BTreeMap;
use studio_json::value::RawValue;

use crate::StoreResult;

/// Error returned by SQLite for any attempt to move an existing entity.
pub const IMMUTABLE_SCOPE_ERROR: &str =
    "The identity and team scope of an existing entity are immutable";
pub const INVALID_RECORD_JSON_ERROR: &str = "A stored record must contain valid JSON";
pub(crate) const SCOPE_TOMBSTONE_FUNCTION: &str = "sync_scope_tombstone";

/// Publications whose agent/account scope is fixed by the SQL key, using LIKE patterns.
const KEY_SCOPED_PUBLICATIONS: &[&str] = &["transcript:%", "rateLimit", "model"];
const PUBLICATION_SCOPES: &[&str] = &["rootId", "agent"];

fn team_scoped_publication_sql(row: &str) -> String {
    let collection = crate::publication_schema::Publications::__DB_COLUMN_COLLECTION_ORDER;
    KEY_SCOPED_PUBLICATIONS
        .iter()
        .map(|pattern| format!("{row}.{collection} NOT LIKE '{pattern}'"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

#[derive(serde::Deserialize)]
#[serde(transparent)]
struct ScopeObject(BTreeMap<String, Box<RawValue>>);

#[derive(serde::Serialize)]
struct ScopeIdentity {
    agent: ScopeSelector,
    #[serde(rename = "rootId")]
    root: ScopeSelector,
}

/// A non-text marker retains unknowability without retaining malformed content.
const UNKNOWN_SCOPE_SELECTOR: u8 = 0;

#[derive(serde::Serialize)]
#[serde(untagged)]
enum ScopeSelector {
    Known(Option<String>),
    Unknown(u8),
}

fn tombstone_selector(raw: Option<Box<RawValue>>) -> ScopeSelector {
    match raw {
        None => ScopeSelector::Known(None),
        Some(raw) => match studio_json::from_str::<Option<String>>(raw.get()) {
            Ok(scope) => ScopeSelector::Known(scope),
            Err(_) => ScopeSelector::Unknown(UNKNOWN_SCOPE_SELECTOR),
        },
    }
}

#[derive(serde::Serialize)]
struct EmptyScope {}

#[derive(serde::Serialize)]
#[serde(untagged)]
enum TombstoneScope {
    Known(ScopeIdentity),
    Unknown(EmptyScope),
}

/// Keep only the last published scope, never deleted content. An initial
/// tombstone without a prior publication has no known scope to constrain.
pub(crate) fn tombstone_payload(
    collection: &str,
    id: &str,
    previous: Option<&str>,
    previous_deleted: Option<bool>,
) -> Result<String, studio_json::Error> {
    #[derive(serde::Serialize)]
    struct Tombstone<'a> {
        collection: &'a str,
        id: &'a str,
        value: TombstoneScope,
    }
    // Map insertion follows the consumers' last-member-wins rule. Raw values
    // retain invalid selector types without retaining any other deleted content.
    let scope = previous
        .and_then(|raw| studio_json::from_str::<ScopeObject>(raw).ok())
        .and_then(|publication| {
            publication
                .0
                .get("value")
                .and_then(|raw| studio_json::from_str::<ScopeObject>(raw.get()).ok())
        });
    let value = match scope {
        Some(mut previous)
            if previous_deleted == Some(false)
                || previous.0.contains_key("agent")
                || previous.0.contains_key("rootId") =>
        {
            TombstoneScope::Known(ScopeIdentity {
                agent: tombstone_selector(previous.0.remove("agent")),
                root: tombstone_selector(previous.0.remove("rootId")),
            })
        }
        _ => TombstoneScope::Unknown(EmptyScope {}),
    };
    studio_json::to_string(&Tombstone {
        collection,
        id,
        value,
    })
}

/// JSON selectors that identify an entity's team or its fixed owning agent.
/// Work item assignees and room/group members deliberately remain mutable.
const JSON_SCOPES: &[(&str, &[&str])] = &[
    ("runtime_agents", &["rootId", "parentId"]),
    ("runtime_tasks", &["agent"]),
    ("runtime_monitors", &["agent"]),
    ("runtime_work", &["rootId"]),
    ("runtime_plans", &["rootId"]),
    ("runtime_annotations", &["rootId"]),
    ("runtime_rules", &["rootId", "agent"]),
    ("runtime_complaints", &["leadId", "agent"]),
    ("runtime_event_meta", &["rootId", "leadId"]),
    ("runtime_requests", &["agent"]),
    ("runtime_rooms", &["rootId"]),
    ("runtime_checkpoints", &["rootId", "agent"]),
];

/// Only immutable ownership selectors survive physical deletion; no source content.
#[derive(serde::Deserialize)]
pub struct DeletedScope {
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default, rename = "rootId")]
    pub root_id: Option<String>,
    #[serde(default, rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(default, rename = "leadId")]
    pub lead_id: Option<String>,
}

/// SQL projection shared by durable tombstone writers and immutable INSERT guards.
pub(crate) fn deleted_scope_sql(table: &str, row: &str) -> Option<String> {
    if let Some((_, fields)) = JSON_SCOPES.iter().find(|(name, _)| *name == table) {
        let fields = fields
            .iter()
            .map(|field| {
                format!(
                    "'{field}',json({})",
                    scope_selector_sql(
                        &format!("{row}.record"),
                        &format!("$.{field}"),
                        "json_quote(value)"
                    )
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        Some(format!(
            "CASE WHEN json_valid({row}.record) THEN json_object({fields}) END"
        ))
    } else if ["runtime_items", "runtime_events", "approvals"].contains(&table) {
        Some(format!("json_object('agent',{row}.agent)"))
    } else if table == "sync_entities" {
        let payload = crate::publication_schema::Publications::__DB_COLUMN_ENTITY_PAYLOAD;
        let team_scoped = team_scoped_publication_sql(row);
        let fields = PUBLICATION_SCOPES
            .iter()
            .map(|field| {
                format!(
                    "'{field}',json({})",
                    scope_selector_sql(
                        &format!("{row}.{payload}"),
                        &format!("$.value.{field}"),
                        "json_quote(value)"
                    )
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        Some(format!(
            "CASE WHEN {team_scoped} AND json_valid({row}.{payload}) THEN json_object({fields}) END"
        ))
    } else {
        None
    }
}

/// Fixed publication guards; the authority installer executes this DDL atomically.
pub(crate) fn publication_trigger_sql() -> String {
    use crate::change_schema::Changes;
    use crate::consolidation_kinds::{CanvasChangesKind, CanvasPublicationsKind};
    use crate::peer_update_settings::Canvas;
    use crate::publication_schema::Publications;
    use studio_db_api::{Table, migration::key_sql, query_column as column};

    let publications = Publications::NAME;
    let changes = Changes::NAME;
    let kind = column::<Canvas, _>(Publications::KIND);
    let entity_kind = CanvasPublicationsKind::DB_CODE_SYNCENTITIES;
    let identity = column::<Canvas, _>(Publications::IDENTITY);
    let collection = column::<Canvas, _>(Publications::COLLECTION_ORDER);
    let id = column::<Canvas, _>(Publications::ENTITY_ID);
    let payload = column::<Canvas, _>(Publications::ENTITY_PAYLOAD);
    let team_scoped = team_scoped_publication_sql("OLD");
    let changed = PUBLICATION_SCOPES
        .iter()
        .map(|field| {
            scope_changed_sql(
                &format!("OLD.{payload}"),
                &format!("NEW.{payload}"),
                &format!("$.value.{field}"),
            )
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    let changed = format!(
        "{team_scoped} AND json_valid(OLD.{payload}) AND (NEW.{payload} IS NULL OR (json_valid(NEW.{payload}) AND ({changed})))"
    );
    let mirrors = ["id", "collection"]
        .iter()
        .map(|field| {
            selector_changed_sql(
                &format!("OLD.{payload}"),
                &format!("NEW.{payload}"),
                &format!("$.{field}"),
            )
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    let mirrors = format!(
        "{team_scoped} AND json_valid(OLD.{payload}) AND json_valid(NEW.{payload}) AND ({mirrors})"
    );
    let duplicated = duplicate_scopes_sql("sync_entities");
    let inserted_change = format!("({changed}) OR ({mirrors})").replace("OLD.", "previous.");
    let change_kind = column::<Canvas, _>(Changes::KIND);
    let durable_kind = CanvasChangesKind::DB_CODE_DURABLEROWS;
    let change_identity = column::<Canvas, _>(Changes::IDENTITY);
    let source_rowid = column::<Canvas, _>(Changes::SOURCE_ROWID);
    let source_label = column::<Canvas, _>(Changes::SOURCE_LABEL);
    let durable_id = column::<Canvas, _>(Changes::DURABLE_ID);
    let seq = column::<Canvas, _>(Changes::SEQUENCE);
    let deleted = column::<Canvas, _>(Changes::DELETED);
    let at = column::<Canvas, _>(Changes::DELETED_AT);
    let scope_column = column::<Canvas, _>(Changes::SCOPE);
    let scope = deleted_scope_sql("sync_entities", "NEW").expect("publication scope owner");
    let scope_changed = PUBLICATION_SCOPES
        .iter()
        .map(|field| {
            scope_changed_sql(
                &format!("previous.{scope_column}"),
                &format!("({scope})"),
                &format!("$.{field}"),
            )
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    let new_team_scoped = team_scoped_publication_sql("NEW");
    let deleted_guard = format!(
        "{new_team_scoped} AND EXISTS(SELECT 1 FROM {changes} previous WHERE previous.{change_kind}={durable_kind} AND previous.{source_label}='sync_entities:'||NEW.{collection} AND previous.{durable_id}=NEW.{id} AND previous.{deleted}=1 AND previous.{scope_column} IS NOT NULL AND ({scope_changed}))"
    );
    let old_scope = deleted_scope_sql("sync_entities", "OLD").expect("publication scope owner");
    let key = key_sql(
        &[
            Changes::__DB_COLUMN_SOURCE_LABEL,
            Changes::__DB_COLUMN_DURABLE_ID,
        ],
        "source_key",
    );
    let rowid_high = crate::clock_schema::queries::ChangeRowidHigh::QUERY
        .sql()
        .replace("?1", durable_kind);
    let increment = format!("({rowid_high})+1");
    // Zero remains deletion evidence, not an allocated publication sequence.
    // Collision updates retain the stable source rowid and previously scoped evidence.
    format!(
        "DROP TRIGGER IF EXISTS immutable_scope_sync_entities;
         DROP TRIGGER IF EXISTS immutable_scope_sync_entities_insert;
         DROP TRIGGER IF EXISTS immutable_scope_sync_entities_delete;
         DROP TRIGGER IF EXISTS immutable_scope_sync_entities_json_INSERT;
         DROP TRIGGER IF EXISTS immutable_scope_sync_entities_json_UPDATE;
         CREATE TRIGGER immutable_scope_sync_entities BEFORE UPDATE ON {publications}
         WHEN OLD.{kind}={entity_kind} AND ((NEW.{kind} IS NOT OLD.{kind}) OR (NEW.{identity} IS NOT OLD.{identity}) OR (OLD.{id} IS NOT NEW.{id}) OR (OLD.{collection} IS NOT NEW.{collection}) OR (NEW.{payload} IS NOT OLD.{payload} AND ({duplicated})) OR ({mirrors}) OR ({changed}))
         BEGIN SELECT RAISE(ABORT,'{IMMUTABLE_SCOPE_ERROR}'); END;
         CREATE TRIGGER immutable_scope_sync_entities_insert BEFORE INSERT ON {publications}
         WHEN NEW.{kind}={entity_kind} AND (({duplicated}) OR EXISTS(SELECT 1 FROM {publications} previous WHERE previous.{kind}={entity_kind} AND previous.{id}=NEW.{id} AND previous.{collection}=NEW.{collection} AND ({inserted_change})) OR ({deleted_guard}))
         BEGIN SELECT RAISE(ABORT,'{IMMUTABLE_SCOPE_ERROR}'); END;
         CREATE TRIGGER immutable_scope_sync_entities_delete AFTER DELETE ON {publications}
         WHEN OLD.{kind}={entity_kind} AND {old_scope} IS NOT NULL BEGIN
           INSERT INTO {changes}({change_kind},{change_identity},{source_rowid},{source_label},{durable_id},{seq},{deleted},{at},{scope_column})
           SELECT {durable_kind},{key},COALESCE((SELECT {source_rowid} FROM {changes} WHERE {change_kind}={durable_kind} AND {change_identity}={key}),CASE WHEN typeof({increment})='integer' THEN {increment} ELSE RAISE(ABORT,'durable source rowid exhausted') END),
             {source_label},{durable_id},0,1,CAST(strftime('%s','now') AS INTEGER),{old_scope}
           FROM (SELECT 'sync_entities:'||OLD.{collection} AS {source_label},OLD.{id} AS {durable_id}) AS source_key WHERE 1
           ON CONFLICT({change_kind},{change_identity}) DO UPDATE SET {at}=excluded.{at},{scope_column}=COALESCE({changes}.{scope_column},excluded.{scope_column});
         END;
         CREATE TRIGGER immutable_scope_sync_entities_json_INSERT BEFORE INSERT ON {publications}
         WHEN NEW.{kind}={entity_kind} AND NEW.{payload} IS NOT NULL AND json_valid(NEW.{payload}) IS NOT 1
         BEGIN SELECT RAISE(ABORT,'{INVALID_RECORD_JSON_ERROR}'); END;
         CREATE TRIGGER immutable_scope_sync_entities_json_UPDATE BEFORE UPDATE OF {payload} ON {publications}
         WHEN NEW.{kind}={entity_kind} AND NEW.{payload} IS NOT NULL AND json_valid(NEW.{payload}) IS NOT 1
         BEGIN SELECT RAISE(ABORT,'{INVALID_RECORD_JSON_ERROR}'); END;"
    )
}

pub(crate) fn install(tx: &Connection) -> StoreResult<()> {
    for (table, fields) in JSON_SCOPES {
        let changed = fields
            .iter()
            .map(|field| scope_changed_sql("OLD.record", "NEW.record", &format!("$.{field}")))
            .collect::<Vec<_>>()
            .join(" OR ");
        install_guard(
            tx,
            table,
            &format!("json_valid(OLD.record) AND json_valid(NEW.record) AND ({changed})"),
        )?;
        install_json_guard(tx, table, "record", false)?;
    }
    for table in ["runtime_items", "runtime_events", "approvals"] {
        install_guard(tx, table, "OLD.agent IS NOT NEW.agent")?;
    }
    Ok(())
}

/// Missing/null means global scope and remains immutable. Only a present
/// selector with an invalid non-text type has no known owner to constrain.
fn scope_changed_sql(previous: &str, next: &str, path: &str) -> String {
    let previous_type = scope_selector_sql(previous, path, "type");
    let changed = selector_changed_sql(previous, next, path);
    format!("(COALESCE({previous_type},'null') IN ('text','null') AND ({changed}))")
}

fn selector_changed_sql(previous: &str, next: &str, path: &str) -> String {
    let previous_value = scope_selector_sql(previous, path, "json_array(type,value)");
    let next_value = scope_selector_sql(next, path, "json_array(type,value)");
    format!(
        "COALESCE({previous_value},'[\"null\",null]') IS NOT COALESCE({next_value},'[\"null\",null]')"
    )
}

fn scope_members_sql(record: &str, path: &str) -> String {
    let object = if path.starts_with("$.value.") {
        format!(
            "(SELECT CASE WHEN type='object' THEN value END FROM json_each({record}) WHERE key='value' ORDER BY id DESC LIMIT 1)"
        )
    } else {
        record.to_owned()
    };
    format!("json_each({object})")
}

fn scope_selector_sql(record: &str, path: &str, projection: &str) -> String {
    let field = path.rsplit('.').next().expect("scope selector");
    let members = scope_members_sql(record, path);
    format!("(SELECT {projection} FROM {members} WHERE key='{field}' ORDER BY id DESC LIMIT 1)")
}

fn duplicate_scopes_sql(table: &str) -> String {
    let (column, fields, prefix, scoped) = if table == "sync_entities" {
        (
            crate::publication_schema::Publications::__DB_COLUMN_ENTITY_PAYLOAD,
            PUBLICATION_SCOPES,
            "$.value.",
            team_scoped_publication_sql("NEW"),
        )
    } else if let Some((_, fields)) = JSON_SCOPES.iter().find(|(name, _)| *name == table) {
        ("record", *fields, "$.", "1".to_owned())
    } else if table == "runtime_items" {
        ("record", &[][..], "$.", "1".to_owned())
    } else {
        return "0".to_owned();
    };
    let duplicates = fields
        .iter()
        .map(|field| {
            let members = scope_members_sql(&format!("NEW.{column}"), &format!("{prefix}{field}"));
            format!("(SELECT count(*) FROM {members} WHERE key='{field}')>1")
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    let mut predicates = Vec::new();
    if !duplicates.is_empty() {
        predicates.push(format!("({scoped}) AND ({duplicates})"));
    }
    let mirrors: &[&str] = if table == "sync_entities" {
        &["id", "collection", "value"]
    } else {
        &["id"]
    };
    for field in mirrors {
        predicates.push(format!(
            "(SELECT count(*) FROM json_each(NEW.{column}) WHERE key='{field}')>1"
        ));
    }
    let duplicated = predicates.join(" OR ");
    format!("CASE WHEN json_valid(NEW.{column}) THEN ({duplicated}) ELSE 0 END")
}

fn install_guard(tx: &Connection, table: &str, changed: &str) -> StoreResult<()> {
    use crate::change_schema::Changes;
    use crate::consolidation_kinds::CanvasChangesKind;
    use studio_db_api::Table;

    if crate::schema::table_exists(tx, table)? {
        let duplicated = duplicate_scopes_sql(table);
        let updated_duplicate =
            if table == "runtime_items" || JSON_SCOPES.iter().any(|(name, _)| *name == table) {
                format!("NEW.record IS NOT OLD.record AND ({duplicated})")
            } else {
                duplicated.clone()
            };
        let identity = "previous.id=NEW.id";
        let key_changed = "OLD.id IS NOT NEW.id";
        let mirror_changed =
            if table == "runtime_items" || JSON_SCOPES.iter().any(|(name, _)| *name == table) {
                let mirror = selector_changed_sql("OLD.record", "NEW.record", "$.id");
                format!("json_valid(OLD.record) AND json_valid(NEW.record) AND ({mirror})")
            } else {
                "0".to_owned()
            };
        let inserted_change =
            format!("({changed}) OR ({mirror_changed})").replace("OLD.", "previous.");
        let deleted_guard = if let Some(scope) = deleted_scope_sql(table, "NEW") {
            let changes = Changes::NAME;
            let kind = Changes::__DB_COLUMN_KIND;
            let durable_kind = CanvasChangesKind::DB_CODE_DURABLEROWS;
            let label = Changes::__DB_COLUMN_SOURCE_LABEL;
            let id = Changes::__DB_COLUMN_DURABLE_ID;
            let deleted = Changes::__DB_COLUMN_DELETED;
            let scope_column = Changes::__DB_COLUMN_SCOPE;
            let team_scoped = "1".to_owned();
            let evidence_table = format!("'{table}'");
            let fields = JSON_SCOPES
                .iter()
                .find(|(name, _)| *name == table)
                .map(|(_, fields)| *fields)
                .unwrap_or(&["agent"]);
            let changed = fields
                .iter()
                .map(|field| {
                    scope_changed_sql(
                        &format!("previous.{scope_column}"),
                        &format!("({scope})"),
                        &format!("$.{field}"),
                    )
                })
                .collect::<Vec<_>>()
                .join(" OR ");
            format!(
                " OR ({team_scoped} AND EXISTS(SELECT 1 FROM {changes} previous WHERE previous.{kind}={durable_kind} AND previous.{label}={evidence_table} AND previous.{id}=NEW.id AND previous.{deleted}=1 AND previous.{scope_column} IS NOT NULL AND ({changed})))"
            )
        } else {
            String::new()
        };
        tx.execute_batch(&format!(
            "DROP TRIGGER IF EXISTS immutable_scope_{table};
             DROP TRIGGER IF EXISTS immutable_scope_{table}_insert;
             CREATE TRIGGER immutable_scope_{table}
             BEFORE UPDATE ON {table} WHEN ({updated_duplicate}) OR ({key_changed}) OR ({mirror_changed}) OR ({changed})
             BEGIN SELECT RAISE(ABORT,'{IMMUTABLE_SCOPE_ERROR}'); END;
             CREATE TRIGGER IF NOT EXISTS immutable_scope_{table}_insert
             BEFORE INSERT ON {table}
             WHEN ({duplicated}) OR EXISTS(SELECT 1 FROM {table} previous WHERE {identity} AND ({inserted_change})){deleted_guard}
             BEGIN SELECT RAISE(ABORT,'{IMMUTABLE_SCOPE_ERROR}'); END;"
        ))?;
    }
    Ok(())
}

fn install_json_guard(
    tx: &Connection,
    table: &str,
    column: &str,
    nullable: bool,
) -> StoreResult<()> {
    if crate::schema::table_exists(tx, table)? {
        let non_null = if nullable {
            format!("NEW.{column} IS NOT NULL AND ")
        } else {
            String::new()
        };
        for operation in ["INSERT", "UPDATE"] {
            let event = if operation == "UPDATE" {
                format!("UPDATE OF {column}")
            } else {
                operation.to_owned()
            };
            tx.execute_batch(&format!(
                "CREATE TRIGGER IF NOT EXISTS immutable_scope_{table}_json_{operation}
                 BEFORE {event} ON {table} WHEN {non_null}json_valid(NEW.{column}) IS NOT 1
                 BEGIN SELECT RAISE(ABORT,'{INVALID_RECORD_JSON_ERROR}'); END;"
            ))?;
        }
    }
    Ok(())
}

/// Seed pre-upgrade or corrupt historical evidence, then restore current guards
/// before exercising its recovery. Ordinary fixtures must create their final scope.
#[cfg(any(test, feature = "test-support"))]
pub fn seed_legacy_reparented_rows<T>(
    connection: &Connection,
    seed: impl FnOnce() -> StoreResult<T>,
) -> StoreResult<T> {
    let triggers = connection
        .prepare(
            "SELECT name FROM sqlite_schema WHERE type='trigger' AND name LIKE 'immutable_scope_%'",
        )?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for trigger in triggers {
        connection.execute_batch(&format!("DROP TRIGGER {trigger}"))?;
    }
    let result = seed();
    install(connection)?;
    let read = crate::peer_update_settings::canvas_read(connection);
    if read.table_exists::<crate::publication_schema::Publications>()?
        && !read.table_exists::<crate::settings_schema::PayloadMigrations>()?
    {
        if connection.is_autocommit() {
            let transaction = connection.unchecked_transaction()?;
            crate::bootstrap::install_current_producers(&transaction)?;
            transaction.commit()?;
        } else {
            crate::bootstrap::install_current_producers(connection)?;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::{AgentId, AgentRepository};

    // Bypass payload admission deliberately: these witnesses exercise the SQL
    // guards against raw corruption, including duplicate JSON member names.
    fn raw_publication(
        db: &rusqlite::Connection,
        collection: &str,
        id: &str,
        payload: &str,
        replace: bool,
    ) -> rusqlite::Result<usize> {
        use crate::consolidation_kinds::CanvasPublicationsKind;
        use crate::repositories::sync_documents::publication_queries::EntityRowidHigh;
        let label = crate::publication_collection::CollectionName::from_stored(collection.into());
        let rowid = crate::peer_update_settings::canvas_read(db).named(
            &EntityRowidHigh::QUERY,
            &EntityRowidHigh {
                kind: CanvasPublicationsKind::SyncEntities,
            },
        )? + 1;
        let identity =
            studio_db_api::migration::encode_key(&(collection.to_owned(), id.to_owned()));
        let collection_kind = label.kind();
        let agent = label.agent().map(AsRef::<str>::as_ref);
        let non_transcript = label.non_transcript();
        let params = rusqlite::params![
            CanvasPublicationsKind::DB_CODE_SYNCENTITIES,
            identity,
            rowid,
            collection,
            collection_kind,
            agent,
            id,
            payload,
            non_transcript,
        ];
        if replace {
            db.execute("INSERT OR REPLACE INTO publications(kind,identity,source_rowid,publication_seq,deleted,collection_order,collection_kind,collection_agent,entity_id,entity_hash,entity_payload,non_transcript) VALUES(?1,?2,?3,1,0,?4,?5,?6,?7,'fixture',?8,?9)", params)
        } else {
            db.execute("INSERT INTO publications(kind,identity,source_rowid,publication_seq,deleted,collection_order,collection_kind,collection_agent,entity_id,entity_hash,entity_payload,non_transcript) VALUES(?1,?2,?3,1,0,?4,?5,?6,?7,'fixture',?8,?9)", params)
        }
    }

    mod canonical {
        use crate::change_schema::{Changes, DurableRows};
        use crate::consolidation_kinds::CanvasPublicationsKind;
        use crate::metadata_values::StoredBoolean;
        use crate::peer_update_settings::{canvas_read, canvas_tx};
        use crate::publication_collection::CollectionName;
        use crate::publication_schema::{
            DocumentValue, EntityValue, Publications, SyncDocuments, SyncEntities,
        };
        use studio_db_api::{Table, migration::encode_key};

        fn fixture() -> rusqlite::Connection {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            Publications::descriptor().install_if_absent(&db).unwrap();
            Changes::descriptor().install_if_absent(&db).unwrap();
            db
        }

        fn install(db: &rusqlite::Connection) {
            db.execute_batch(&super::super::publication_trigger_sql())
                .unwrap();
        }

        fn put(db: &rusqlite::Transaction<'_>, id: &str, payload: &str) -> rusqlite::Result<()> {
            canvas_tx(db)
                .partition::<SyncEntities>()
                .put(&entity(id, 1, payload))
        }

        fn entity(id: &str, source_rowid: i64, payload: &str) -> SyncEntities {
            let collection_order = CollectionName::from_stored("fixture".into());
            SyncEntities {
                identity: encode_key(&("fixture".to_owned(), id.to_owned())),
                source_rowid,
                publication_seq: 1,
                deleted: StoredBoolean::Valid(false),
                collection_kind: collection_order.kind(),
                collection_agent: None,
                non_transcript: true,
                collection_order,
                entity_id: id.into(),
                entity_hash: "hash".into(),
                entity_payload: EntityValue::from_stored(
                    "fixture",
                    id,
                    StoredBoolean::Valid(false),
                    Some(payload.into()),
                )
                .unwrap(),
            }
        }

        fn refuse<T: std::fmt::Debug, E: std::fmt::Display + std::fmt::Debug>(
            result: Result<T, E>,
        ) {
            let error = result.expect_err("immutable publication scope");
            assert!(
                error
                    .to_string()
                    .contains(super::super::IMMUTABLE_SCOPE_ERROR),
                "{error}"
            );
        }

        fn evidence(db: &rusqlite::Connection) -> Option<DurableRows> {
            canvas_read(db)
                .get::<DurableRows>(&(encode_key(&(
                    "sync_entities:fixture".to_owned(),
                    "x".to_owned(),
                )),))
                .unwrap()
        }

        #[test]
        fn canonical_publication_fixture_reseed_restores_guards() {
            let mut db = fixture();
            let tx = db.transaction().unwrap();
            install(&tx);
            put(
                &tx,
                "x",
                r#"{"id":"x","collection":"fixture","value":{"agent":"alpha"}}"#,
            )
            .unwrap();
            super::super::seed_legacy_reparented_rows(&tx, || Ok(())).unwrap();
            refuse(put(
                &tx,
                "x",
                r#"{"id":"x","collection":"fixture","value":{"agent":"beta"}}"#,
            ));
            tx.rollback().unwrap();
        }

        #[test]
        fn canonical_publication_duplicate_insert_refusal() {
            let mut connection = fixture();
            let db = connection.transaction().unwrap();
            install(&db);
            for payload in [
                r#"{"id":"x","collection":"fixture","value":{"agent":"alpha","agent":"beta"}}"#,
                r#"{"id":"x","collection":"fixture","value":{"agent":"alpha"},"value":{"agent":"beta"}}"#,
                r#"{"id":"x","id":"y","collection":"fixture","value":{}}"#,
                r#"{"id":"x","collection":"fixture","collection":"other","value":{}}"#,
            ] {
                refuse(db.execute(
                    "INSERT INTO publications(kind,identity,source_rowid,publication_seq,deleted,collection_order,collection_kind,entity_id,entity_hash,entity_payload,non_transcript) VALUES(?1,?2,1,1,0,'fixture',?3,'x','hash',?4,1)",
                    rusqlite::params![CanvasPublicationsKind::DB_CODE_SYNCENTITIES, encode_key(&("fixture".to_owned(),"x".to_owned())), crate::publication_collection::CollectionKind::DB_CODE_OTHER, payload],
                ));
            }
        }

        #[test]
        fn canonical_publication_mirror_scope_key_and_restore_fences() {
            let mut connection = fixture();
            let db = connection.transaction().unwrap();
            install(&db);
            let original =
                r#"{"id":"x","collection":"fixture","value":{"agent":"alpha","rootId":"root"}}"#;
            put(&db, "x", original).unwrap();
            for payload in [
                r#"{"id":"x","collection":"fixture","value":{"agent":"beta","rootId":"root"}}"#,
                r#"{"id":"x","collection":"fixture","value":{"agent":"alpha","rootId":"other"}}"#,
                r#"{"id":"y","collection":"fixture","value":{"agent":"alpha","rootId":"root"}}"#,
                r#"{"id":"x","collection":"other","value":{"agent":"alpha","rootId":"root"}}"#,
            ] {
                refuse(db.execute("UPDATE publications SET entity_payload=?1", [payload]));
                refuse(put(&db, "x", payload));
            }
            for sql in [
                "UPDATE publications SET entity_id='y'",
                "UPDATE publications SET collection_order='transcript:alpha'",
                "UPDATE publications SET identity=x'00'",
            ] {
                refuse(db.execute(sql, []));
            }
            db.execute("UPDATE publications SET deleted=1", []).unwrap();
            refuse(db.execute("UPDATE publications SET deleted=0,entity_payload=json_set(entity_payload,'$.value.agent','beta')", []));
            db.execute(
                "UPDATE publications SET deleted=0,entity_payload=?1",
                [original],
            )
            .unwrap();
            let error = db
                .execute("UPDATE publications SET entity_payload='broken'", [])
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(super::super::INVALID_RECORD_JSON_ERROR)
            );
        }

        #[test]
        fn canonical_publication_hard_delete_last_member_and_collision_evidence() {
            let mut connection = fixture();
            let db = connection.transaction().unwrap();
            let raw = r#"{"id":"x","collection":"fixture","value":{"agent":17},"value":{"agent":17,"agent":"alpha","rootId":"root","secret":"discard"}}"#;
            put(&db, "x", raw).unwrap();
            install(&db);
            db.execute("UPDATE publications SET entity_payload=entity_payload", [])
                .unwrap();
            db.execute("DELETE FROM publications", []).unwrap();
            let first = evidence(&db).unwrap();
            assert_eq!(first.sequence, 0);
            assert_eq!(first.deleted.as_stored(), 1);
            assert_eq!(first.source_label.as_str(), "sync_entities:fixture");
            assert_eq!(first.durable_id, "x");
            let scope = first.scope.unwrap();
            assert_eq!(scope.value().agent.as_deref(), Some("alpha"));
            assert_eq!(scope.value().root_id.as_deref(), Some("root"));
            assert!(!scope.raw().contains("secret"));
            refuse(put(
                &db,
                "x",
                r#"{"id":"x","collection":"fixture","value":{"agent":"beta","rootId":"root"}}"#,
            ));
            put(
                &db,
                "x",
                r#"{"id":"x","collection":"fixture","value":{"agent":"alpha","rootId":"root"}}"#,
            )
            .unwrap();
            db.execute("DELETE FROM publications", []).unwrap();
            let second = evidence(&db).unwrap();
            assert_eq!(first.source_rowid, second.source_rowid);
            assert_eq!(second.scope.unwrap().raw(), scope.raw());
        }

        #[test]
        fn canonical_publication_invalid_last_selector_and_unchanged_raw_bytes() {
            let mut connection = fixture();
            let db = connection.transaction().unwrap();
            let raw = r#"{"id":"x","collection":"fixture","value":{"agent":"alpha","agent":17}}"#;
            put(&db, "x", raw).unwrap();
            install(&db);
            db.execute("UPDATE publications SET entity_payload=entity_payload", [])
                .unwrap();
            refuse(db.execute(
                "UPDATE publications SET entity_payload=entity_payload||' '",
                [],
            ));
            db.execute("DELETE FROM publications", []).unwrap();
            put(
                &db,
                "x",
                r#"{"id":"x","collection":"fixture","value":{"agent":"beta"}}"#,
            )
            .unwrap();
        }

        #[test]
        fn canonical_publication_replace_insert_refuses_scope_and_mirror_moves() {
            let mut connection = fixture();
            let db = connection.transaction().unwrap();
            install(&db);
            put(
                &db,
                "x",
                r#"{"id":"x","collection":"fixture","value":{"agent":"alpha"}}"#,
            )
            .unwrap();
            for payload in [
                r#"{"id":"x","collection":"fixture","value":{"agent":"beta"}}"#,
                r#"{"id":"y","collection":"fixture","value":{"agent":"alpha"}}"#,
            ] {
                refuse(db.execute(
                    "INSERT OR REPLACE INTO publications(kind,identity,source_rowid,publication_seq,deleted,collection_order,collection_kind,entity_id,entity_hash,entity_payload,non_transcript) SELECT kind,identity,source_rowid,publication_seq,deleted,collection_order,collection_kind,entity_id,entity_hash,?1,non_transcript FROM publications",
                    [payload],
                ));
            }
        }

        #[test]
        fn canonical_publication_collision_retains_previous_scoped_evidence() {
            let mut connection = fixture();
            let db = connection.transaction().unwrap();
            put(
                &db,
                "x",
                r#"{"id":"x","collection":"fixture","value":{"agent":"beta"}}"#,
            )
            .unwrap();
            canvas_tx(&db)
                .partition::<DurableRows>()
                .put(&DurableRows {
                    identity: encode_key(&("sync_entities:fixture".to_owned(), "x".to_owned())),
                    source_rowid: -7,
                    source_label: crate::change_schema::DurableSource::from_stored(
                        "sync_entities:fixture".into(),
                    ),
                    durable_id: "x".into(),
                    sequence: 23,
                    deleted: StoredBoolean::Valid(true),
                    deleted_at: 1,
                    scope: Some(
                        crate::json_record_codec::ExactJson::from_stored(
                            r#"{ "agent":"alpha", "rootId":null }"#.into(),
                        )
                        .unwrap(),
                    ),
                })
                .unwrap();
            install(&db);
            canvas_tx(&db)
                .partition::<SyncEntities>()
                .delete(&(encode_key(&("fixture".to_owned(), "x".to_owned())),))
                .unwrap();
            let retained = evidence(&db).unwrap();
            assert_eq!(retained.source_rowid, -7);
            assert_eq!(retained.sequence, 23);
            assert_eq!(retained.deleted.as_stored(), 1);
            assert!(retained.deleted_at > 1);
            assert_eq!(
                retained.scope.unwrap().raw(),
                r#"{ "agent":"alpha", "rootId":null }"#
            );
        }

        #[test]
        fn canonical_publication_document_partitions_excluded() {
            let mut connection = fixture();
            let db = connection.transaction().unwrap();
            install(&db);
            let mut document = SyncDocuments {
                identity: encode_key(&(1_i64,)),
                source_rowid: 1,
                publication_seq: 1,
                deleted: StoredBoolean::Valid(false),
                document_scope: "scope".into(),
                document_id: "x".into(),
                document_payload: DocumentValue::from_stored("scope", "x", "broken".into())
                    .unwrap(),
            };
            canvas_tx(&db)
                .partition::<SyncDocuments>()
                .put(&document)
                .unwrap();
            document.document_scope = "other".into();
            document.document_payload =
                DocumentValue::from_stored("other", "x", "still broken".into()).unwrap();
            canvas_tx(&db)
                .partition::<SyncDocuments>()
                .put(&document)
                .unwrap();
            canvas_tx(&db)
                .delete::<SyncDocuments>(&(document.identity,))
                .unwrap();
            assert!(evidence(&db).is_none());
            let error = put(&db, "x", "broken").unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(super::super::INVALID_RECORD_JSON_ERROR)
            );
        }

        #[test]
        fn canonical_publication_transaction_rollback_restores_row_and_evidence() {
            let mut db = fixture();
            let setup = db.transaction().unwrap();
            install(&setup);
            let original = r#"{"id":"x","collection":"fixture","value":{"agent":"alpha"}}"#;
            put(&setup, "x", original).unwrap();
            setup.commit().unwrap();
            {
                let tx = db.transaction().unwrap();
                tx.execute("DELETE FROM publications", []).unwrap();
                assert!(evidence(&tx).is_some());
                refuse(put(
                    &tx,
                    "x",
                    r#"{"id":"x","collection":"fixture","value":{"agent":"beta"}}"#,
                ));
                tx.rollback().unwrap();
            }
            assert!(evidence(&db).is_none());
            let row = canvas_read(&db)
                .get::<SyncEntities>(&(encode_key(&("fixture".to_owned(), "x".to_owned())),))
                .unwrap()
                .unwrap();
            assert_eq!(row.entity_payload.value().raw(), Some(original));
        }

        #[derive(studio_db_api::DbQuery)]
        #[db_query(database=crate::peer_update_settings::Canvas, mode=write, cardinality=none, result=(),
            tables(rows=Publications), columns(kind=Publications::KIND,id=Publications::ENTITY_ID),
            sql="INSERT OR REPLACE INTO {rows} SELECT * FROM {rows} WHERE {kind}=?1 AND {id}='x'")]
        struct InsertProbe {
            #[db(column=Publications::KIND)]
            kind: CanvasPublicationsKind,
        }

        #[derive(studio_db_api::DbQuery)]
        #[db_query(database=crate::peer_update_settings::Canvas, mode=write, cardinality=none, result=(),
            tables(rows=crate::planning_work::PlanRow),
            sql="INSERT OR REPLACE INTO {rows} SELECT * FROM {rows}")]
        struct SourceInsertProbe;

        #[test]
        fn canonical_source_hard_delete_scope_fence_uses_changes_identity_index() {
            use crate::json_record_codec::ExactJson;
            use crate::planning_work::PlanRow;
            use std::sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            };
            let mut measurements = Vec::new();
            for unrelated in [0, 2_000] {
                let mut connection = fixture();
                PlanRow::descriptor()
                    .install_if_absent(&connection)
                    .unwrap();
                crate::metadata_schema::Metadata::descriptor()
                    .install_if_absent(&connection)
                    .unwrap();
                let db = connection.transaction().unwrap();
                crate::clock_schema::seed_fresh_clock_rows(&db).unwrap();
                for n in 0..unrelated {
                    let id = format!("retained-{n}");
                    canvas_tx(&db)
                        .partition::<DurableRows>()
                        .put(&DurableRows {
                            identity: encode_key(&("runtime_plans".to_owned(), id.clone())),
                            source_rowid: n + 2,
                            source_label: crate::change_schema::DurableSource::from_stored(
                                "runtime_plans".into(),
                            ),
                            durable_id: id,
                            sequence: 0,
                            deleted: StoredBoolean::Valid(true),
                            deleted_at: 1,
                            scope: None,
                        })
                        .unwrap();
                }
                crate::bootstrap::install_current_producers(&db).unwrap();
                let mut plan = PlanRow {
                    id: "x".into(),
                    record: ExactJson::from_stored(
                        r#"{"id":"x","rootId":"alpha","text":"","version":1,"updated":null}"#
                            .into(),
                    )
                    .unwrap(),
                };
                canvas_tx(&db).insert(&plan).unwrap();
                canvas_tx(&db)
                    .delete::<PlanRow>(&("x".to_owned(),))
                    .unwrap();
                super::super::install(&db).unwrap();
                let key = encode_key(&("runtime_plans".to_owned(), "x".to_owned()));
                let deleted = canvas_read(&db)
                    .get::<DurableRows>(&(key,))
                    .unwrap()
                    .unwrap();
                assert_eq!(deleted.deleted.as_stored(), 1);
                assert_eq!(
                    deleted.scope.unwrap().value().root_id.as_deref(),
                    Some("alpha")
                );
                plan.record = ExactJson::from_stored(
                    r#"{"id":"x","rootId":"beta","text":"","version":1,"updated":null}"#.into(),
                )
                .unwrap();
                refuse(canvas_tx(&db).insert(&plan));
                assert!(
                    canvas_read(&db)
                        .get::<PlanRow>(&("x".to_owned(),))
                        .unwrap()
                        .is_none()
                );
                db.set_db_config(
                    rusqlite::config::DbConfig::SQLITE_DBCONFIG_TRIGGER_EQP,
                    true,
                )
                .unwrap();
                let index = studio_db_api::query_index::<
                    crate::peer_update_settings::Canvas,
                    Changes,
                >(&Changes::INDEX_CHANGES_DURABLE_WINDOW_MEMBERS);
                let steps = crate::query_plan_test_support::explain(
                    &db,
                    SourceInsertProbe::QUERY.sql(),
                    &SourceInsertProbe,
                )
                .unwrap();
                assert!(
                    steps.iter().any(|line| line.contains("SEARCH previous")
                        && line.contains(index)
                        && line.contains("source_label=? AND deleted=? AND durable_id=?")),
                    "{steps:?}"
                );
                plan.record = ExactJson::from_stored(
                    r#"{"id":"x","rootId":"alpha","text":"","version":1,"updated":null}"#.into(),
                )
                .unwrap();
                let counter = Arc::new(AtomicUsize::new(0));
                let counted = counter.clone();
                db.progress_handler(
                    1,
                    Some(move || {
                        counted.fetch_add(1, Ordering::Relaxed);
                        false
                    }),
                )
                .unwrap();
                canvas_tx(&db).insert(&plan).unwrap();
                db.progress_handler(0, None::<fn() -> bool>).unwrap();
                measurements.push(counter.load(Ordering::Relaxed));
                eprintln!(
                    "unrelated={unrelated} source_vm={}",
                    measurements.last().unwrap()
                );
            }
            assert!(measurements[1] <= measurements[0] + 100, "{measurements:?}");
        }

        #[test]
        fn canonical_publication_insert_guards_use_indexes_with_flat_vm_work() {
            use std::sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            };
            let mut measurements = Vec::new();
            for unrelated in [0, 2_000] {
                let mut connection = fixture();
                let db = connection.transaction().unwrap();
                let raw = r#"{"id":"x","collection":"fixture","value":{"agent":"alpha"}}"#;
                put(&db, "x", raw).unwrap();
                for n in 0..unrelated {
                    let id = format!("unrelated-{n}");
                    canvas_tx(&db)
                        .partition::<SyncEntities>()
                        .put(&entity(&id, n + 2, raw))
                        .unwrap();
                    canvas_tx(&db)
                        .partition::<DurableRows>()
                        .put(&DurableRows {
                            identity: encode_key(&("unrelated".to_owned(), id.clone())),
                            source_rowid: n + 1,
                            source_label: crate::change_schema::DurableSource::from_stored(
                                "unrelated".into(),
                            ),
                            durable_id: id,
                            sequence: 0,
                            deleted: StoredBoolean::Valid(true),
                            deleted_at: 1,
                            scope: None,
                        })
                        .unwrap();
                }
                install(&db);
                db.set_db_config(
                    rusqlite::config::DbConfig::SQLITE_DBCONFIG_TRIGGER_EQP,
                    true,
                )
                .unwrap();
                let plan = crate::query_plan_test_support::explain(
                    &db,
                    InsertProbe::QUERY.sql(),
                    &InsertProbe {
                        kind: CanvasPublicationsKind::SyncEntities,
                    },
                )
                .unwrap();
                for (alias, index) in [
                    (
                        "previous",
                        studio_db_api::query_index::<
                            crate::peer_update_settings::Canvas,
                            Publications,
                        >(&Publications::INDEX_PUBLICATIONS_ENTITY_IDENTITY),
                    ),
                    (
                        "previous",
                        studio_db_api::query_index::<crate::peer_update_settings::Canvas, Changes>(
                            &Changes::INDEX_CHANGES_DURABLE_WINDOW_MEMBERS,
                        ),
                    ),
                ] {
                    assert!(
                        plan.iter().any(|line| line.contains("SEARCH")
                            && line.contains(alias)
                            && line.contains(index)),
                        "{plan:?}"
                    );
                }
                let steps = Arc::new(AtomicUsize::new(0));
                let counter = Arc::clone(&steps);
                db.progress_handler(
                    1,
                    Some(move || {
                        counter.fetch_add(1, Ordering::Relaxed);
                        false
                    }),
                )
                .unwrap();
                put(&db, "x", raw).unwrap();
                db.progress_handler(0, None::<fn() -> bool>).unwrap();
                measurements.push(steps.load(Ordering::Relaxed));
                eprintln!(
                    "unrelated={unrelated} vm_steps={} trigger_plan={plan:?}",
                    measurements.last().unwrap()
                );
            }
            assert!(measurements[1] <= measurements[0] + 100, "{measurements:?}");
        }
    }

    #[test]
    fn key_scoped_raw_payload_can_gain_tombstone_envelope_but_team_scoped_cannot() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        for collection in ["transcript:fixture", "model", "rateLimit", "fixture"] {
            if collection == "transcript:fixture" {
                let page = studio_json::json!({"items":[],"truncated":false});
                crate::TranscriptRepository::pull_payload(
                    &tx,
                    collection,
                    0,
                    &page,
                    false,
                    Some(1),
                )
                .unwrap();
                crate::TranscriptRepository::cache_revision_payload(
                    &tx,
                    collection,
                    &page.to_string(),
                )
                .unwrap();
                let deleted = crate::TranscriptRepository::pull_payload(
                    &tx,
                    collection,
                    0,
                    &studio_json::json!({}),
                    true,
                    Some(2),
                )
                .expect("key-scoped transcript can gain its deleted cache shape");
                crate::TranscriptRepository::cache_revision_payload(&tx, collection, "{}").unwrap();
                assert_eq!(deleted["documents"][0]["_deleted"], true);
                let revision = crate::test_fixtures::canvas::entities(&tx)
                    .unwrap()
                    .into_iter()
                    .find(|row| row.collection == collection && row.id == "@revision")
                    .unwrap();
                assert_eq!(revision.payload.as_deref(), Some("{}"));
                continue;
            }
            raw_publication(
                &tx,
                collection,
                "row",
                r#"{"id":"row","unknown":[1,2]}"#,
                false,
            )
            .unwrap();
            let result = crate::SyncDocumentsRepository::put_entity_hash(
                &tx, collection, "row", "two", true,
            );
            if collection == "fixture" {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains(super::IMMUTABLE_SCOPE_ERROR)
                );
            } else {
                assert!(result.expect("raw key-scoped payload can gain a tombstone envelope"));
            }
        }
    }

    #[test]
    fn non_revision_transcript_cached_payload_is_refused() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        raw_publication(
            &tx,
            "transcript:fixture",
            "row",
            r#"{"id":"row","unknown":[1,2]}"#,
            false,
        )
        .unwrap();
        let error = crate::SyncDocumentsRepository::put_entity_hash(
            &tx,
            "transcript:fixture",
            "row",
            "two",
            true,
        )
        .expect_err("only the revision identity admits a cached transcript payload");
        assert!(
            error
                .to_string()
                .contains("Only transcript revision rows admit cached payloads"),
            "{error}"
        );
    }

    #[test]
    fn duplicate_publication_container_cannot_disagree_with_scoped_readers() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let payload = r#"{"collection":"fixture","id":"x","value":{"rootId":"alpha"},"value":{"rootId":"beta"}}"#;
        assert!(raw_publication(&tx, "fixture", "x", payload, false).is_err());
    }

    #[test]
    fn duplicate_identity_mirror_cannot_change_last_consumer_identity() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        tx.execute("INSERT INTO runtime_plans(id,record) VALUES('x','{\"id\":\"x\",\"rootId\":\"alpha\"}')", []).unwrap();
        assert!(tx.execute("UPDATE runtime_plans SET record='{\"id\":\"x\",\"id\":\"other\",\"rootId\":\"alpha\"}' WHERE id='x'", []).is_err());
    }

    #[test]
    fn historical_duplicate_agent_can_be_held_without_rewriting_its_record() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let record = r#"{"id":"x","rootId":"alpha","rootId":17,"provider":"codex","accountKey":"default","epoch":1,"compactions":0,"status":"idle"}"#;
        super::seed_legacy_reparented_rows(&tx, || {
            tx.execute(
                "INSERT INTO runtime_agents(id,record) VALUES('x',?1)",
                [record],
            )?;
            Ok(())
        })
        .unwrap();
        AgentRepository::hold(
            &tx,
            "x",
            studio_runtime_schema::persistence::HoldStatus::Invalid,
            Some("invalid root"),
        )
        .expect("quarantine does not admit new duplicate JSON");
        assert!(
            tx.execute(
                "UPDATE runtime_agents SET record=?1 WHERE id='x'",
                [format!("{record} ")]
            )
            .is_err(),
            "rewriting duplicate bytes is refused even when the final owner is unchanged"
        );
        let actual: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='x'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(actual, record);
    }

    #[test]
    fn duplicate_publication_scope_keeps_last_owner_in_soft_tombstone() {
        for previous in [
            r#"{"value":{"agent":"known","rootId":1,"rootId":"alpha","content":"private"}}"#,
            r#"{"value":{"rootId":"wrong"},"value":{"agent":"known","rootId":"alpha","content":"private"}}"#,
        ] {
            let tombstone =
                super::tombstone_payload("fixture", "x", Some(previous), Some(false)).unwrap();
            assert_eq!(
                tombstone,
                r#"{"collection":"fixture","id":"x","value":{"agent":"known","rootId":"alpha"}}"#
            );
        }
        let tombstone = super::tombstone_payload(
            "fixture",
            "x",
            Some(r#"{"value":{"agent":"known","rootId":{"private":"discard"}}}"#),
            Some(false),
        )
        .unwrap();
        assert_eq!(
            tombstone,
            r#"{"collection":"fixture","id":"x","value":{"agent":"known","rootId":0}}"#
        );
    }

    #[test]
    fn duplicate_scope_selectors_cannot_move_rows_or_publications() {
        for route in ["insert", "update", "replace", "restore"] {
            let mut db = crate::fresh_state::test_canvas();
            let tx = db.transaction().unwrap();
            if route != "insert" {
                tx.execute("INSERT INTO runtime_plans(id,record) VALUES('x','{\"id\":\"x\",\"rootId\":\"alpha\"}')", []).unwrap();
                raw_publication(
                    &tx,
                    "fixture",
                    "x",
                    r#"{"collection":"fixture","id":"x","value":{"rootId":"alpha"}}"#,
                    false,
                )
                .unwrap();
            }
            if route == "restore" {
                tx.execute("DELETE FROM runtime_plans WHERE id='x'", [])
                    .unwrap();
                tx.execute(
                    "DELETE FROM publications WHERE kind=2 AND collection_order='fixture'",
                    [],
                )
                .unwrap();
            }
            let row_sql = if route == "update" {
                "UPDATE runtime_plans SET record=?1 WHERE id='x'"
            } else {
                "INSERT OR REPLACE INTO runtime_plans(id,record) VALUES('x',?1)"
            };
            assert!(
                tx.execute(row_sql, [r#"{"id":"x","rootId":"alpha","rootId":"beta"}"#])
                    .is_err(),
                "{route}: row"
            );
            let payload =
                r#"{"collection":"fixture","id":"x","value":{"rootId":"alpha","rootId":"beta"}}"#;
            let result = if route == "update" {
                tx.execute("UPDATE publications SET entity_payload=?1 WHERE kind=2 AND collection_order='fixture'", [payload])
            } else {
                raw_publication(&tx, "fixture", "x", payload, route != "insert")
            };
            assert!(result.is_err(), "{route}: publication");
        }
    }

    #[test]
    fn historical_duplicate_scope_uses_last_member_as_known_owner() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        super::seed_legacy_reparented_rows(&tx, || {
            tx.execute("INSERT INTO runtime_plans(id,record) VALUES('x','{\"id\":\"x\",\"rootId\":1,\"rootId\":\"alpha\"}')", [])?;
            raw_publication(&tx, "fixture", "x", r#"{"collection":"fixture","id":"x","value":{"rootId":1,"rootId":"alpha"}}"#, false)?;
            Ok(())
        }).unwrap();
        tx.execute(
            "UPDATE publications SET entity_hash='unchanged-entity_payload' WHERE kind=2 AND collection_order='fixture'",
            [],
        )
        .expect("metadata can change without admitting duplicate payload bytes");
        assert!(
            tx.execute(
                "UPDATE publications SET entity_payload=entity_payload||' ' WHERE kind=2 AND collection_order='fixture'",
                []
            )
            .is_err()
        );
        assert!(
            tx.execute(
                "UPDATE runtime_plans SET record='{\"id\":\"x\",\"rootId\":\"beta\"}' WHERE id='x'",
                []
            )
            .is_err()
        );
        assert!(tx.execute("UPDATE publications SET entity_payload='{\"collection\":\"fixture\",\"id\":\"x\",\"value\":{\"rootId\":\"beta\"}}' WHERE kind=2 AND collection_order='fixture'", []).is_err());
        tx.execute(
            "UPDATE runtime_plans SET record='{\"id\":\"x\",\"rootId\":\"alpha\"}' WHERE id='x'",
            [],
        )
        .expect("same last owner can be canonicalized");
        tx.execute("UPDATE publications SET entity_payload='{\"collection\":\"fixture\",\"id\":\"x\",\"value\":{\"rootId\":\"alpha\"}}' WHERE kind=2 AND collection_order='fixture'", []).expect("same published owner can be canonicalized");
    }

    #[test]
    fn invalid_scope_selector_repair_keeps_known_selectors_and_identity_immutable() {
        for invalid in ["17", "true", "false", "[]", "{}", "1.5"] {
            let mut db = crate::fresh_state::test_canvas();
            let tx = db.transaction().unwrap();
            tx.execute("INSERT INTO runtime_agents(id,record) VALUES('legacy',json_object('id','legacy','rootId',json(?1),'parentId','parent','provider','codex','accountKey','default','epoch',1,'compactions',0,'status','idle'))", [invalid]).unwrap();
            for sql in [
                "UPDATE runtime_agents SET id='other' WHERE id='legacy'",
                "UPDATE runtime_agents SET record=json_set(record,'$.parentId','other') WHERE id='legacy'",
                "INSERT OR REPLACE INTO runtime_agents(id,record) SELECT id,json_set(record,'$.id','other','$.rootId','repaired') FROM runtime_agents WHERE id='legacy'",
            ] {
                assert!(
                    tx.execute(sql, [])
                        .unwrap_err()
                        .to_string()
                        .contains(super::IMMUTABLE_SCOPE_ERROR)
                );
            }
            tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.rootId','repaired') WHERE id='legacy'", []).expect("a non-text historical root has no known team to move");
            assert!(tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.rootId','other') WHERE id='legacy'", []).is_err());
        }
    }

    #[test]
    fn known_scope_cannot_be_erased_to_enable_repair() {
        for original in [
            r#"{"id":"x"}"#,
            r#"{"id":"x","rootId":null}"#,
            r#"{"id":"x","rootId":""}"#,
            r#"{"id":"x","rootId":"lead"}"#,
            r#"{"id":"x","rootId":"{}"}"#,
            r#"{"id":"x","rootId":"[]"}"#,
            r#"{"id":"x","rootId":"{\"team\":[\"nested\"]}"}"#,
            r#"{"id":"x","rootId":"[\"nested\",{}]"}"#,
        ] {
            let mut db = crate::fresh_state::test_canvas();
            let tx = db.transaction().unwrap();
            tx.execute(
                "INSERT INTO runtime_plans(id,record) VALUES('x',?1)",
                [original],
            )
            .unwrap();
            raw_publication(
                &tx,
                "fixture",
                "x",
                &format!(r#"{{"collection":"fixture","id":"x","value":{original}}}"#),
                false,
            )
            .unwrap();
            for invalid in [
                "17",
                "true",
                "false",
                "[]",
                "{}",
                r#"{"team":["nested"]}"#,
                r#"["nested",{}]"#,
            ] {
                assert!(tx.execute("UPDATE runtime_plans SET record=json_set(record,'$.rootId',json(?1)) WHERE id='x'", [invalid]).is_err());
                assert!(tx.execute("INSERT OR REPLACE INTO runtime_plans(id,record) VALUES('x',json_set(?1,'$.rootId',json(?2)))", [original, invalid]).is_err());
                assert!(tx.execute("UPDATE publications SET entity_payload=json_set(entity_payload,'$.value.rootId',json(?1)) WHERE kind=2 AND collection_order='fixture'", [invalid]).is_err());
                assert!(raw_publication(&tx, "fixture", "x", &format!(r#"{{"collection":"fixture","id":"x","value":{{"id":"x","rootId":{invalid}}}}}"#), true).is_err());
            }
            tx.execute("DELETE FROM runtime_plans WHERE id='x'", [])
                .unwrap();
            tx.execute(
                "DELETE FROM publications WHERE kind=2 AND collection_order='fixture'",
                [],
            )
            .unwrap();
            for invalid in [
                "17",
                "true",
                "false",
                "[]",
                "{}",
                r#"{"team":["nested"]}"#,
                r#"["nested",{}]"#,
            ] {
                assert!(tx.execute("INSERT INTO runtime_plans(id,record) VALUES('x',json_set(?1,'$.rootId',json(?2)))", [original, invalid]).is_err());
                assert!(raw_publication(&tx, "fixture", "x", &format!(r#"{{"collection":"fixture","id":"x","value":{{"id":"x","rootId":{invalid}}}}}"#), false).is_err());
            }
            assert!(tx.execute("INSERT INTO runtime_plans(id,record) VALUES('x','{\"id\":\"x\",\"rootId\":17}')", []).is_err());
            tx.execute(
                "INSERT INTO runtime_plans(id,record) VALUES('x',?1)",
                [original],
            )
            .unwrap();
        }
    }

    #[test]
    fn invalid_publication_and_deleted_scope_selectors_can_be_repaired() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        for collection in ["updated", "replaced", "deleted"] {
            crate::SyncDocumentsRepository::put_entity(
                &tx,
                collection,
                "x",
                studio_json::json!({"agent":"known", "rootId":17}),
                false,
            )
            .unwrap();
            if collection == "deleted" {
                tx.execute(
                    "DELETE FROM publications WHERE kind=2 AND collection_order=?1",
                    [collection],
                )
                .unwrap();
            }
            let repair = if collection == "updated" {
                tx.execute("UPDATE publications SET entity_payload=json_set(entity_payload,'$.value.rootId','repaired') WHERE kind=2 AND collection_order=?1", [collection])
            } else {
                raw_publication(
                    &tx,
                    collection,
                    "x",
                    &format!(
                        r#"{{"collection":"{collection}","id":"x","value":{{"rootId":"repaired","agent":"known"}}}}"#
                    ),
                    true,
                )
            };
            repair.expect("repair does not move known agent selector");
            assert!(tx.execute("UPDATE publications SET entity_payload=json_set(entity_payload,'$.value.agent','other') WHERE kind=2 AND collection_order=?1", [collection]).is_err());
        }
    }

    #[test]
    fn key_scoped_publications_ignore_payload_scope_but_keep_sql_identity() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        for collection in ["rateLimit", "model", "transcript:alpha"] {
            let transcript = collection == "transcript:alpha";
            let id = if transcript { "@revision" } else { "x" };
            let page = studio_json::json!({"items":[],"truncated":false,"value":{"agent":"alpha"}});
            if transcript {
                crate::TranscriptRepository::pull_payload(
                    &tx,
                    collection,
                    0,
                    &page,
                    false,
                    Some(1),
                )
                .unwrap();
                crate::TranscriptRepository::cache_revision_payload(
                    &tx,
                    collection,
                    &page.to_string(),
                )
                .unwrap();
            } else {
                crate::SyncDocumentsRepository::put_entity(
                    &tx,
                    collection,
                    id,
                    studio_json::json!({"agent":"alpha"}),
                    false,
                )
                .unwrap();
            }
            tx.execute("UPDATE publications SET entity_payload=json_set(entity_payload,'$.value.agent','beta','$.value.rootId','beta') WHERE kind=2 AND collection_order=?1 AND entity_id=?2", [collection,id]).unwrap();
            let error = tx
                .execute(
                    "UPDATE publications SET entity_id='y' WHERE kind=2 AND collection_order=?1 AND entity_id=?2",
                    [collection,id],
                )
                .expect_err("scope fixed by key requires immutable key");
            assert!(error.to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
            tx.execute(
                "DELETE FROM publications WHERE kind=2 AND collection_order=?1 AND entity_id=?2",
                [collection, id],
            )
            .unwrap();
            let evidence = crate::test_fixtures::canvas::changes(&tx)
                .unwrap()
                .into_iter()
                .filter(|row| {
                    row.source_label == format!("sync_entities:{collection}")
                        && row.durable_id == id
                })
                .count();
            assert_eq!(evidence, 0);
            if transcript {
                crate::TranscriptRepository::pull_payload(
                    &tx,
                    collection,
                    0,
                    &page,
                    false,
                    Some(2),
                )
                .expect("normalized payload cannot change scope fixed by key");
                crate::TranscriptRepository::cache_revision_payload(
                    &tx,
                    collection,
                    &page.to_string(),
                )
                .unwrap();
            } else {
                crate::SyncDocumentsRepository::put_entity(
                    &tx,
                    collection,
                    "x",
                    studio_json::json!({}),
                    false,
                )
                .expect("normalized payload cannot change scope fixed by key");
            }
        }
    }

    #[test]
    fn scope_evidence_does_not_change_client_deltas_or_sequence_allocation() {
        let mut baseline = crate::fresh_state::test_canvas();
        let mut guarded = crate::fresh_state::test_canvas();
        let baseline = baseline.transaction().unwrap();
        let guarded = guarded.transaction().unwrap();
        baseline
            .execute_batch("DROP TRIGGER immutable_scope_sync_entities_delete")
            .unwrap();
        let snapshot = |tx: &rusqlite::Connection| {
            (
                crate::SyncDocumentsRepository::entities_after(tx, 0, 100).unwrap(),
                crate::durable_seq::DURABLE_TABLES
                    .iter()
                    .map(|table| {
                        crate::durable_seq::changes_since(tx, table.name, Some(0)).unwrap()
                    })
                    .collect::<Vec<_>>(),
                crate::durable_seq::high_and_floor(tx).unwrap(),
                crate::SyncDocumentsRepository::entity_tombstone_floor(tx).unwrap(),
            )
        };
        for step in 0..3 {
            for tx in [&baseline, &guarded] {
                if step == 1 {
                    crate::durable_seq::keep_counter(tx).unwrap();
                    tx.execute(
                        "DELETE FROM publications WHERE kind=2 AND collection_order='fixture' AND entity_id='x'",
                        [],
                    )
                    .unwrap();
                } else {
                    crate::SyncDocumentsRepository::put_entity(
                        tx,
                        "fixture",
                        "x",
                        studio_json::json!({"id":"x","agent":"alpha","secret":"private content"}),
                        false,
                    )
                    .unwrap();
                }
            }
            assert_eq!(snapshot(&guarded), snapshot(&baseline), "step {step}");
        }
        let evidence = crate::test_fixtures::canvas::changes(&guarded)
            .unwrap()
            .into_iter()
            .find(|row| row.source_label == "sync_entities:fixture" && row.durable_id == "x")
            .unwrap();
        let scope = evidence.scope.unwrap();
        assert_eq!(
            evidence.seq, 0,
            "scope evidence does not allocate a sequence"
        );
        assert!(!scope.contains("secret"));
        assert!(!scope.contains("private content"));
    }

    #[test]
    fn publication_scope_evidence_uses_existing_retention_without_advancing_floor() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "x",
            studio_json::json!({"id":"x","agent":"alpha"}),
            false,
        )
        .unwrap();
        tx.execute(
            "DELETE FROM publications WHERE kind=2 AND collection_order='fixture' AND entity_id='x'",
            [],
        )
        .unwrap();
        tx.execute(
            "UPDATE changes SET deleted_at=0 WHERE kind=1 AND source_label='sync_entities:fixture'",
            [],
        )
        .unwrap();
        tx.execute("INSERT INTO runtime_plans(id,record) VALUES('plan','{\"id\":\"plan\",\"rootId\":\"alpha\"}')", []).unwrap();
        let floor = crate::durable_seq::high_and_floor(&tx).unwrap().1;
        tx.execute("DELETE FROM runtime_plans WHERE id='plan'", [])
            .unwrap();
        let remaining = crate::test_fixtures::canvas::changes(&tx)
            .unwrap()
            .into_iter()
            .filter(|row| row.source_label == "sync_entities:fixture")
            .count();
        assert_eq!(remaining, 0);
        assert_eq!(crate::durable_seq::high_and_floor(&tx).unwrap().1, floor);
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "x",
            studio_json::json!({"id":"x","agent":"beta"}),
            false,
        )
        .expect("expired scope evidence follows the existing retention boundary");
    }

    #[test]
    fn physical_publication_delete_retains_scope_for_reinsert() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "x",
            studio_json::json!({"id":"x","agent":"alpha"}),
            false,
        )
        .unwrap();
        tx.execute(
            "DELETE FROM publications WHERE kind=2 AND collection_order='fixture' AND entity_id='x'",
            [],
        )
        .unwrap();
        let error = crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "x",
            studio_json::json!({"id":"x","agent":"beta"}),
            false,
        )
        .expect_err("physical deletion retains the previous publication scope");
        assert!(error.to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "x",
            studio_json::json!({"id":"x","agent":"alpha"}),
            false,
        )
        .expect("same-scope restore remains valid");
    }

    #[test]
    fn replacement_publication_cannot_change_identity_mirrors() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "x",
            studio_json::json!({"id":"x","agent":"alpha"}),
            false,
        )
        .unwrap();
        for field in ["$.id", "$.collection"] {
            let error = tx.execute(
                "INSERT OR REPLACE INTO publications(kind,identity,source_rowid,collection_kind,collection_agent,non_transcript,collection_order,entity_id,publication_seq,entity_hash,entity_payload,deleted) SELECT kind,identity,source_rowid,collection_kind,collection_agent,non_transcript,collection_order,entity_id,publication_seq,entity_hash,json_set(entity_payload,?1,'y'),deleted FROM publications WHERE kind=2 AND collection_order='fixture' AND entity_id='x'",
                [field],
            ).expect_err("replacement inserts retain identity mirrors");
            assert!(error.to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
        }
    }

    #[test]
    fn global_scope_remains_known_after_raw_tombstone_update() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "global",
            studio_json::json!({"id":"global"}),
            false,
        )
        .unwrap();
        tx.execute(
            "UPDATE publications SET deleted=1, entity_payload=json_remove(entity_payload,'$.value.rootId','$.value.agent') WHERE kind=2 AND collection_order='fixture' AND entity_id='global'",
            [],
        )
        .unwrap();
        let error = tx.execute(
            "UPDATE publications SET deleted=0, entity_payload=json_set(entity_payload,'$.value.agent','beta') WHERE kind=2 AND collection_order='fixture' AND entity_id='global'",
            [],
        ).expect_err("global scope remains immutable after deletion");
        assert!(error.to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
    }

    #[test]
    fn updating_task_identity_cannot_overwrite_another_scopes_tombstone() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        for (id, agent) in [("x", "alpha"), ("y", "beta")] {
            let task: crate::TaskRecord = studio_json::from_str(&format!(r#"{{"id":"{id}","agent":"{agent}","itemId":"item","turnId":"turn","created":1,"type":"commandExecution","name":"exec","kind":"command","status":"running"}}"#)).unwrap();
            crate::TaskRepository::put(&tx, &task).unwrap();
            if id == "x" {
                tx.execute("DELETE FROM runtime_tasks WHERE id='x'", [])
                    .unwrap();
            }
        }
        let error = tx
            .execute(
                "UPDATE runtime_tasks SET id='x',record=json_set(record,'$.id','x') WHERE id='y'",
                [],
            )
            .expect_err("a key update cannot replace another scope's tombstone");
        assert!(error.to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
        let tasks = crate::TaskRepository::running_for_agent(&tx, "beta").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id().as_ref(), "y");
        assert!(
            crate::TaskRepository::running_for_agent(&tx, "alpha")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn collection_identity_cannot_be_changed_to_evade_scope_guards() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "x",
            studio_json::json!({"id":"x","agent":"alpha"}),
            false,
        )
        .unwrap();
        let error = tx.execute_batch("UPDATE publications SET collection_order='transcript:alpha' WHERE kind=2 AND collection_order='fixture' AND entity_id='x';
            UPDATE publications SET collection_order='fixture',entity_payload=json_set(entity_payload,'$.value.agent','beta') WHERE kind=2 AND collection_order='transcript:alpha' AND entity_id='x';")
            .expect_err("collection identity is immutable even for a transcript cache");
        assert!(error.to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
        let payload = crate::test_fixtures::canvas::entities(&tx)
            .unwrap()
            .into_iter()
            .find(|row| row.collection == "fixture" && row.id == "x")
            .unwrap()
            .payload
            .unwrap();
        let owner =
            crate::publication_payload::PublicationSelectors::from_envelope(&payload).unwrap();
        assert!(
            matches!(owner.agent, crate::publication_payload::Selector::Text(ref agent) if agent == "alpha")
        );
    }

    #[test]
    fn json_identity_mirrors_cannot_change_on_update() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        tx.execute("INSERT INTO runtime_plans(id,record) VALUES('x','{\"id\":\"x\",\"rootId\":\"alpha\"}')", []).unwrap();
        assert!(
            tx.execute(
                "UPDATE runtime_plans SET record=json_set(record,'$.id','y') WHERE id='x'",
                []
            )
            .expect_err("record identity is immutable")
            .to_string()
            .contains(super::IMMUTABLE_SCOPE_ERROR)
        );
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "x",
            studio_json::json!({"id":"x","agent":"alpha"}),
            false,
        )
        .unwrap();
        for path in ["$.id", "$.collection"] {
            assert!(tx.execute("UPDATE publications SET entity_payload=json_set(entity_payload,?1,'y') WHERE kind=2 AND collection_order='fixture' AND entity_id='x'", [path]).expect_err("published identity is immutable").to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
        }
    }

    #[test]
    fn physical_task_delete_retains_scope_for_reinsert_and_scoped_reader() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let task: crate::TaskRecord = studio_json::from_str(r#"{"id":"scope-test","agent":"alpha","itemId":"item","turnId":"turn","created":1,"type":"commandExecution","name":"exec","kind":"command","status":"running"}"#).unwrap();
        crate::TaskRepository::put(&tx, &task).unwrap();
        assert_eq!(
            crate::TaskRepository::running_for_agent(&tx, "alpha")
                .unwrap()
                .len(),
            1
        );
        tx.execute("DELETE FROM runtime_tasks WHERE id='scope-test'", [])
            .unwrap();
        assert!(
            crate::TaskRepository::running_for_agent(&tx, "alpha")
                .unwrap()
                .is_empty()
        );
        let other: crate::TaskRecord = studio_json::from_str(r#"{"id":"scope-test","agent":"beta","itemId":"item","turnId":"turn","created":1,"type":"commandExecution","name":"exec","kind":"command","status":"running"}"#).unwrap();
        let error = crate::TaskRepository::put(&tx, &other)
            .expect_err("physical deletion retains the previous scope");
        assert!(error.to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
        assert!(
            crate::TaskRepository::running_for_agent(&tx, "beta")
                .unwrap()
                .is_empty()
        );
        crate::TaskRepository::put(&tx, &task).expect("same-owner restore");
        assert_eq!(
            crate::TaskRepository::running_for_agent(&tx, "alpha")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn tombstone_restore_cannot_change_publication_scope() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "published",
            studio_json::json!({"agent":"a","rootId":"alpha","secret":"discard on deletion"}),
            false,
        )
        .unwrap();
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "published",
            studio_json::json!({}),
            true,
        )
        .unwrap();
        let tombstone = crate::test_fixtures::canvas::entities(&tx)
            .unwrap()
            .into_iter()
            .find(|row| row.collection == "fixture" && row.id == "published")
            .unwrap()
            .payload
            .unwrap();
        assert!(!tombstone.contains("secret"));
        assert!(!tombstone.contains("discard on deletion"));
        for value in [
            studio_json::json!({"agent":"b","rootId":"alpha"}),
            studio_json::json!({"agent":"a","rootId":"beta"}),
        ] {
            let error = crate::SyncDocumentsRepository::put_entity(
                &tx,
                "fixture",
                "published",
                value,
                false,
            )
            .expect_err("a tombstone retains the published scope");
            assert!(error.to_string().contains("immutable"), "{error}");
        }
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "fixture",
            "published",
            studio_json::json!({"agent":"a","rootId":"alpha","text":"restored"}),
            false,
        )
        .unwrap();
    }

    fn invalid_intermediate_is_refused(sql: &str) {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let original = crate::PlanRecord::from_value(studio_json::json!({
            "id":"worker","rootId":"lead","text":"","version":1,"updated":null
        }))
        .unwrap();
        crate::WorkItemsRepository::put_plan(&tx, "worker", &original).unwrap();
        let error = tx
            .execute(sql, [])
            .expect_err("invalid intermediate JSON must be refused");
        assert!(error.to_string().contains("JSON"), "{error}");
        let moved = crate::PlanRecord::from_value(studio_json::json!({
            "id":"worker","rootId":"other","text":"","version":1,"updated":null
        }))
        .unwrap();
        assert!(crate::WorkItemsRepository::put_plan(&tx, "worker", &moved).is_err());
        let root: String = tx
            .query_row(
                "SELECT json_extract(record,'$.rootId') FROM runtime_plans WHERE id='worker'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(root, "lead");
    }

    #[test]
    fn invalid_json_update_cannot_erase_scope() {
        invalid_intermediate_is_refused(
            "UPDATE runtime_plans SET record='broken' WHERE id='worker'",
        );
    }

    #[test]
    fn invalid_json_replacement_cannot_erase_scope() {
        invalid_intermediate_is_refused(
            "INSERT OR REPLACE INTO runtime_plans(id,record) VALUES('worker','broken')",
        );
    }

    #[test]
    fn historical_malformed_json_remains_readable_and_can_be_repaired() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        // Model a historical row written before validation existed.
        tx.execute_batch(
            "DROP TRIGGER immutable_scope_runtime_plans_json_INSERT;
            INSERT INTO runtime_plans(id,record) VALUES('legacy','broken');",
        )
        .unwrap();
        super::install(&tx).unwrap();
        let raw: String = tx
            .query_row(
                "SELECT record FROM runtime_plans WHERE id='legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw, "broken");
        let repaired = crate::PlanRecord::from_value(studio_json::json!({
            "id":"legacy","rootId":"repaired","text":"","version":1,"updated":null
        }))
        .unwrap();
        crate::WorkItemsRepository::put_plan(&tx, "legacy", &repaired)
            .expect("the scope of malformed historical JSON is unknowable");
        assert!(
            tx.execute(
                "UPDATE runtime_plans SET record='broken' WHERE id='legacy'",
                []
            )
            .is_err()
        );
    }

    #[test]
    fn tombstone_function_consumers_match_the_named_owner() {
        for (sql, calls) in [
            (crate::event_publication_triggers::trigger_sql(), 5),
            (crate::feature_schema::summary_delete_trigger_sql(), 4),
        ] {
            assert_eq!(
                sql.matches(super::SCOPE_TOMBSTONE_FUNCTION).count(),
                calls,
                "entity_scopes::SCOPE_TOMBSTONE_FUNCTION owns every SQL tombstone call"
            );
        }
    }

    #[test]
    fn existing_agent_scope_is_immutable_for_raw_and_typed_writers() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let original = crate::AgentRecord::from_value(studio_json::json!({
            "id":"worker","rootId":"lead","parentId":"lead","provider":"codex",
            "accountKey":"default","epoch":1,"compactions":0,"status":"idle"
        }))
        .unwrap();
        AgentRepository::put(&tx, &original).unwrap();
        for field in ["rootId", "parentId"] {
            let error = tx.execute(
                "UPDATE runtime_agents SET record=json_set(record,?1,'other') WHERE id='worker'",
                [format!("$.{field}")],
            ).expect_err("raw writers cannot move an existing agent");
            assert!(error.to_string().contains("immutable"), "{error}");
            let error = tx
                .execute(
                    "INSERT OR REPLACE INTO runtime_agents(id,record)
                 SELECT id,json_set(record,?1,'other') FROM runtime_agents WHERE id='worker'",
                    [format!("$.{field}")],
                )
                .expect_err("replacement inserts cannot bypass immutable scope");
            assert!(error.to_string().contains("immutable"), "{error}");
            let mut moved = original.clone();
            if field == "rootId" {
                moved.root_id = AgentId::from("other");
            } else {
                moved.parent_id = Some("other".into());
            }
            let error = AgentRepository::put(&tx, &moved)
                .expect_err("typed writers cannot move an existing agent");
            assert!(error.to_string().contains("immutable"), "{error}");
            assert_eq!(
                AgentRepository::get(&tx, &original.id)
                    .unwrap()
                    .unwrap()
                    .root_id,
                original.root_id
            );
        }
        AgentRepository::put(&tx, &original).expect("same identity remains writable");
    }

    #[test]
    fn existing_task_and_plan_scope_is_immutable_through_typed_repositories() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        for agent in ["first", "second"] {
            let task: crate::TaskRecord = studio_json::from_str(&format!(
                r#"{{"id":"task","agent":"{agent}","itemId":"item","turnId":"turn","created":1,"type":"commandExecution","name":"exec","kind":"command","status":"running"}}"#
            )).unwrap();
            let result = crate::TaskRepository::put(&tx, &task);
            if agent == "first" {
                result.unwrap();
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains(super::IMMUTABLE_SCOPE_ERROR)
                );
            }
            let plan = crate::PlanRecord::from_value(studio_json::json!({
                "id":"plan","rootId":agent,"text":"","version":1,"updated":null
            }))
            .unwrap();
            let result =
                crate::Table::<crate::PlanRecord>::new(&tx).put_at(&AgentId::from("plan"), &plan);
            if agent == "first" {
                result.unwrap();
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains(super::IMMUTABLE_SCOPE_ERROR)
                );
            }
        }
    }

    #[test]
    fn raw_record_scope_guards_refuse_moves_without_publishing_changes() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        for (table, fields) in super::JSON_SCOPES {
            let record = if *table == "runtime_agents" {
                r#"{"id":"scope-fixture","agent":"first","rootId":"first","parentId":"first","leadId":"first","provider":"codex","accountKey":"default","epoch":1,"compactions":0,"status":"idle"}"#
            } else {
                r#"{"id":"scope-fixture","agent":"first","rootId":"first","parentId":"first","leadId":"first"}"#
            };
            tx.execute(
                &format!("INSERT INTO {table}(id,record) VALUES('scope-fixture',?1)"),
                [record],
            )
            .unwrap();
            let high = crate::durable_seq::high(&tx).unwrap();
            for field in *fields {
                let error = tx.execute(&format!("UPDATE {table} SET record=json_set(record,?1,'second') WHERE id='scope-fixture'"), [format!("$.{field}")]).unwrap_err();
                assert!(
                    error.to_string().contains(super::IMMUTABLE_SCOPE_ERROR),
                    "{table}.{field}: {error}"
                );
                assert_eq!(
                    crate::durable_seq::high(&tx).unwrap(),
                    high,
                    "{table}.{field}"
                );
            }
        }
    }
}
