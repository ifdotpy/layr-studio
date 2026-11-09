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
    KEY_SCOPED_PUBLICATIONS
        .iter()
        .map(|pattern| format!("{row}.collection NOT LIKE '{pattern}'"))
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
        let team_scoped = team_scoped_publication_sql(row);
        let fields = PUBLICATION_SCOPES
            .iter()
            .map(|field| {
                format!(
                    "'{field}',json({})",
                    scope_selector_sql(
                        &format!("{row}.payload"),
                        &format!("$.value.{field}"),
                        "json_quote(value)"
                    )
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        Some(format!(
            "CASE WHEN {team_scoped} AND json_valid({row}.payload) THEN json_object({fields}) END"
        ))
    } else {
        None
    }
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
    let team_scoped = team_scoped_publication_sql("OLD");
    let publication_changed = PUBLICATION_SCOPES
        .iter()
        .map(|field| scope_changed_sql("OLD.payload", "NEW.payload", &format!("$.value.{field}")))
        .collect::<Vec<_>>()
        .join(" OR ");
    install_guard(
        tx,
        "sync_entities",
        &format!(
            "{team_scoped} AND json_valid(OLD.payload) AND (NEW.payload IS NULL OR (json_valid(NEW.payload) AND ({publication_changed})))"
        ),
    )?;
    install_json_guard(tx, "sync_entities", "payload", true)?;
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
            "payload",
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
    if crate::schema::table_exists(tx, table)? {
        let duplicated = duplicate_scopes_sql(table);
        let updated_duplicate = if table == "sync_entities" {
            format!("NEW.payload IS NOT OLD.payload AND ({duplicated})")
        } else if table == "runtime_items" || JSON_SCOPES.iter().any(|(name, _)| *name == table) {
            format!("NEW.record IS NOT OLD.record AND ({duplicated})")
        } else {
            duplicated.clone()
        };
        let identity = if table == "sync_entities" {
            "previous.id=NEW.id AND previous.collection=NEW.collection"
        } else {
            "previous.id=NEW.id"
        };
        let key_changed = if table == "sync_entities" {
            "OLD.id IS NOT NEW.id OR OLD.collection IS NOT NEW.collection"
        } else {
            "OLD.id IS NOT NEW.id"
        };
        let mirror_changed = if table == "sync_entities" {
            let team_scoped = team_scoped_publication_sql("OLD");
            let mirrors = ["id", "collection"]
                .iter()
                .map(|field| {
                    selector_changed_sql("OLD.payload", "NEW.payload", &format!("$.{field}"))
                })
                .collect::<Vec<_>>()
                .join(" OR ");
            format!(
                "{team_scoped} AND json_valid(OLD.payload) AND json_valid(NEW.payload) AND ({mirrors})"
            )
        } else if table == "runtime_items" || JSON_SCOPES.iter().any(|(name, _)| *name == table) {
            let mirror = selector_changed_sql("OLD.record", "NEW.record", "$.id");
            format!("json_valid(OLD.record) AND json_valid(NEW.record) AND ({mirror})")
        } else {
            "0".to_owned()
        };
        let inserted_change =
            format!("({changed}) OR ({mirror_changed})").replace("OLD.", "previous.");
        let deleted_guard = if let Some(scope) = deleted_scope_sql(table, "NEW") {
            let team_scoped = if table == "sync_entities" {
                team_scoped_publication_sql("NEW")
            } else {
                "1".to_owned()
            };
            let evidence_table = if table == "sync_entities" {
                "'sync_entities:'||NEW.collection".to_owned()
            } else {
                format!("'{table}'")
            };
            let fields = JSON_SCOPES
                .iter()
                .find(|(name, _)| *name == table)
                .map(|(_, fields)| *fields)
                .unwrap_or(if table == "sync_entities" {
                    PUBLICATION_SCOPES
                } else {
                    &["agent"]
                });
            let changed = fields
                .iter()
                .map(|field| {
                    scope_changed_sql(
                        "previous.scope",
                        &format!("({scope})"),
                        &format!("$.{field}"),
                    )
                })
                .collect::<Vec<_>>()
                .join(" OR ");
            format!(
                " OR ({team_scoped} AND EXISTS(SELECT 1 FROM durable_rows previous WHERE previous.tbl={evidence_table} AND previous.id=NEW.id AND previous.deleted=1 AND previous.scope IS NOT NULL AND ({changed})))"
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
        if table == "sync_entities" {
            let scope = deleted_scope_sql(table, "OLD")
                .expect("sync entity publications declare their deleted scope");
            // Zero is evidence, not a publication sequence: allocators and floors
            // stay unchanged. Existing durable tombstone retention prunes it.
            tx.execute_batch(&format!(
                "DROP TRIGGER IF EXISTS immutable_scope_sync_entities_delete;
                 CREATE TRIGGER immutable_scope_sync_entities_delete
                 AFTER DELETE ON sync_entities WHEN {scope} IS NOT NULL
                 BEGIN
                   INSERT INTO durable_rows(tbl,id,seq,deleted,at,scope)
                   VALUES('sync_entities:'||OLD.collection,OLD.id,0,1,CAST(strftime('%s','now') AS INTEGER),{scope})
                   ON CONFLICT(tbl,id) DO UPDATE SET at=excluded.at,scope=excluded.scope;
                 END;"
            ))?;
        }
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
    result
}

#[cfg(test)]
mod tests {
    use crate::{AgentId, AgentRepository};

    #[test]
    fn key_scoped_raw_payload_can_gain_tombstone_envelope_but_team_scoped_cannot() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        for collection in ["transcript:fixture", "model", "rateLimit", "fixture"] {
            tx.execute("INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES(?1,'row',1,'one','{\"id\":\"row\",\"unknown\":[1,2]}',0)", [collection]).unwrap();
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
    fn duplicate_publication_container_cannot_disagree_with_scoped_readers() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let payload = r#"{"collection":"fixture","id":"x","value":{"rootId":"alpha"},"value":{"rootId":"beta"}}"#;
        assert!(tx.execute("INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('fixture','x',1,'fixture',?1,0)", [payload]).is_err());
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
                tx.execute("INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('fixture','x',1,'fixture','{\"collection\":\"fixture\",\"id\":\"x\",\"value\":{\"rootId\":\"alpha\"}}',0)", []).unwrap();
            }
            if route == "restore" {
                tx.execute("DELETE FROM runtime_plans WHERE id='x'", [])
                    .unwrap();
                tx.execute("DELETE FROM sync_entities WHERE collection='fixture'", [])
                    .unwrap();
            }
            let row_sql = if route == "update" {
                "UPDATE runtime_plans SET record=?1 WHERE id='x'"
            } else {
                "INSERT OR REPLACE INTO runtime_plans(id,record) VALUES('x',?1)"
            };
            let publication_sql = if route == "update" {
                "UPDATE sync_entities SET payload=?1 WHERE collection='fixture'"
            } else {
                "INSERT OR REPLACE INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('fixture','x',1,'fixture',?1,0)"
            };
            assert!(
                tx.execute(row_sql, [r#"{"id":"x","rootId":"alpha","rootId":"beta"}"#])
                    .is_err(),
                "{route}: row"
            );
            assert!(tx.execute(publication_sql, [r#"{"collection":"fixture","id":"x","value":{"rootId":"alpha","rootId":"beta"}}"#]).is_err(), "{route}: publication");
        }
    }

    #[test]
    fn historical_duplicate_scope_uses_last_member_as_known_owner() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        super::seed_legacy_reparented_rows(&tx, || {
            tx.execute("INSERT INTO runtime_plans(id,record) VALUES('x','{\"id\":\"x\",\"rootId\":1,\"rootId\":\"alpha\"}')", [])?;
            tx.execute("INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('fixture','x',1,'fixture','{\"collection\":\"fixture\",\"id\":\"x\",\"value\":{\"rootId\":1,\"rootId\":\"alpha\"}}',0)", [])?;
            Ok(())
        }).unwrap();
        tx.execute(
            "UPDATE sync_entities SET hash='unchanged-payload' WHERE collection='fixture'",
            [],
        )
        .expect("metadata can change without admitting duplicate payload bytes");
        assert!(
            tx.execute(
                "UPDATE sync_entities SET payload=payload||' ' WHERE collection='fixture'",
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
        assert!(tx.execute("UPDATE sync_entities SET payload='{\"collection\":\"fixture\",\"id\":\"x\",\"value\":{\"rootId\":\"beta\"}}' WHERE collection='fixture'", []).is_err());
        tx.execute(
            "UPDATE runtime_plans SET record='{\"id\":\"x\",\"rootId\":\"alpha\"}' WHERE id='x'",
            [],
        )
        .expect("same last owner can be canonicalized");
        tx.execute("UPDATE sync_entities SET payload='{\"collection\":\"fixture\",\"id\":\"x\",\"value\":{\"rootId\":\"alpha\"}}' WHERE collection='fixture'", []).expect("same published owner can be canonicalized");
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
            tx.execute("INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('fixture','x',1,'fixture',json_object('collection','fixture','id','x','value',json(?1)),0)", [original]).unwrap();
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
                assert!(tx.execute("UPDATE sync_entities SET payload=json_set(payload,'$.value.rootId',json(?1)) WHERE collection='fixture'", [invalid]).is_err());
                assert!(tx.execute("INSERT OR REPLACE INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('fixture','x',1,'fixture',json_object('collection','fixture','id','x','value',json_set(?1,'$.rootId',json(?2))),0)", [original, invalid]).is_err());
            }
            tx.execute("DELETE FROM runtime_plans WHERE id='x'", [])
                .unwrap();
            tx.execute("DELETE FROM sync_entities WHERE collection='fixture'", [])
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
                assert!(tx.execute("INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('fixture','x',1,'fixture',json_object('collection','fixture','id','x','value',json_set(?1,'$.rootId',json(?2))),0)", [original, invalid]).is_err());
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
                    "DELETE FROM sync_entities WHERE collection=?1",
                    [collection],
                )
                .unwrap();
            }
            let sql = if collection == "updated" {
                "UPDATE sync_entities SET payload=json_set(payload,'$.value.rootId','repaired') WHERE collection=?1"
            } else {
                "INSERT OR REPLACE INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES(?1,'x',1,'fixture',json_object('collection',?1,'id','x','value',json_object('rootId','repaired','agent','known')),0)"
            };
            tx.execute(sql, [collection])
                .expect("repair does not move known agent selector");
            assert!(tx.execute("UPDATE sync_entities SET payload=json_set(payload,'$.value.agent','other') WHERE collection=?1", [collection]).is_err());
        }
    }

    #[test]
    fn key_scoped_publications_ignore_payload_scope_but_keep_sql_identity() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        for collection in ["rateLimit", "model", "transcript:alpha"] {
            crate::SyncDocumentsRepository::put_entity(
                &tx,
                collection,
                "x",
                studio_json::json!({"agent":"alpha"}),
                false,
            )
            .unwrap();
            tx.execute("UPDATE sync_entities SET payload=json_set(payload,'$.value.agent','beta','$.value.rootId','beta') WHERE collection=?1 AND id='x'", [collection]).unwrap();
            let error = tx
                .execute(
                    "UPDATE sync_entities SET id='y' WHERE collection=?1 AND id='x'",
                    [collection],
                )
                .expect_err("scope fixed by key requires immutable key");
            assert!(error.to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
            tx.execute(
                "DELETE FROM sync_entities WHERE collection=?1 AND id='x'",
                [collection],
            )
            .unwrap();
            let evidence: i64 = tx
                .query_row(
                    "SELECT count(*) FROM durable_rows WHERE tbl='sync_entities:'||?1 AND id='x'",
                    [collection],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(evidence, 0);
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
                        "DELETE FROM sync_entities WHERE collection='fixture' AND id='x'",
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
        let (seq, scope): (i64, String) = guarded
            .query_row(
                "SELECT seq,scope FROM durable_rows WHERE tbl='sync_entities:fixture' AND id='x'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(seq, 0, "scope evidence does not allocate a sequence");
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
            "DELETE FROM sync_entities WHERE collection='fixture' AND id='x'",
            [],
        )
        .unwrap();
        tx.execute(
            "UPDATE durable_rows SET at=0 WHERE tbl='sync_entities:fixture'",
            [],
        )
        .unwrap();
        tx.execute("INSERT INTO runtime_plans(id,record) VALUES('plan','{\"id\":\"plan\",\"rootId\":\"alpha\"}')", []).unwrap();
        let floor = crate::durable_seq::high_and_floor(&tx).unwrap().1;
        tx.execute("DELETE FROM runtime_plans WHERE id='plan'", [])
            .unwrap();
        let remaining: i64 = tx
            .query_row(
                "SELECT count(*) FROM durable_rows WHERE tbl='sync_entities:fixture'",
                [],
                |row| row.get(0),
            )
            .unwrap();
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
            "DELETE FROM sync_entities WHERE collection='fixture' AND id='x'",
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
                "INSERT OR REPLACE INTO sync_entities(collection,id,seq,hash,payload,deleted) SELECT collection,id,seq,hash,json_set(payload,?1,'y'),deleted FROM sync_entities WHERE collection='fixture' AND id='x'",
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
            "UPDATE sync_entities SET deleted=1, payload=json_remove(payload,'$.value.rootId','$.value.agent') WHERE collection='fixture' AND id='global'",
            [],
        )
        .unwrap();
        let error = tx.execute(
            "UPDATE sync_entities SET deleted=0, payload=json_set(payload,'$.value.agent','beta') WHERE collection='fixture' AND id='global'",
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
        let error = tx.execute_batch("UPDATE sync_entities SET collection='transcript:alpha' WHERE collection='fixture' AND id='x';
            UPDATE sync_entities SET collection='fixture',payload=json_set(payload,'$.value.agent','beta') WHERE collection='transcript:alpha' AND id='x';")
            .expect_err("collection identity is immutable even for a transcript cache");
        assert!(error.to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
        let owner: String = tx.query_row("SELECT json_extract(payload,'$.value.agent') FROM sync_entities WHERE collection='fixture' AND id='x'", [], |row| row.get(0)).unwrap();
        assert_eq!(owner, "alpha");
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
            assert!(tx.execute("UPDATE sync_entities SET payload=json_set(payload,?1,'y') WHERE collection='fixture' AND id='x'", [path]).expect_err("published identity is immutable").to_string().contains(super::IMMUTABLE_SCOPE_ERROR));
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
        let tombstone: String = tx
            .query_row(
                "SELECT payload FROM sync_entities WHERE collection='fixture' AND id='published'",
                [],
                |row| row.get(0),
            )
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
        assert_eq!(
            crate::bootstrap::SYNC_SCHEMA
                .matches(super::SCOPE_TOMBSTONE_FUNCTION)
                .count(),
            10,
            "entity_scopes::SCOPE_TOMBSTONE_FUNCTION owns every SQL tombstone call"
        );
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
