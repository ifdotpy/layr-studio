//! Project grants: the local record of one pairing with a peer server, per project
//! (decision 2026-11-01, "Pairing per project").
//!
//! One row per granted capability. Rows of one pairing share `grant_id`. A
//! revocation sets `revoked_at` on every row of the pairing and is final: the
//! check trigger refuses to clear it again.
//!
//! One owner per rule:
//! - [`Capability`] owns the capability vocabulary. The check triggers are generated
//!   from it ([`check_triggers_sql`]).
//! - [`active_grant`] is the only reader that authorizes a peer request.

use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::str::FromStr;
use strum::VariantNames;

use crate::StoreResult;
use crate::peer_update_settings::{Canvas, canvas_tx};
use studio_db_api::{DbTable, db_query, db_update};

pub const PROJECT_GRANTS_TABLE: &str = <GrantRow as studio_db_api::Table>::NAME;

/// What a paired project may do. Closed: a new value needs an owner decision.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    VariantNames,
    strum::EnumString,
    strum::AsRefStr,
)]
pub enum Capability {
    /// The paired leads send each other messages.
    LeadMessages,
}

impl rusqlite::types::ToSql for Capability {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.as_ref().into())
    }
}
impl rusqlite::types::FromSql for Capability {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Self::from_str(value.as_str()?)
            .map_err(|error| rusqlite::types::FromSqlError::Other(Box::new(error)))
    }
}
impl studio_db_api::DbValue for Capability {
    const SQL_TYPE: &'static str = "TEXT";
}

/// One row of a pairing: one capability of one grant.
#[derive(Clone, Debug, Eq, PartialEq, DbTable)]
#[db(table = "project_grants", database = Canvas, state_check = crate::state_check::count_legacy_rows::<GrantRow>,
    legacy_ddl = "CREATE TABLE project_grants(\n            grant_id TEXT NOT NULL,\n            peer TEXT NOT NULL,\n            peer_public_key TEXT NOT NULL,\n            local_project TEXT NOT NULL,\n            remote_project TEXT NOT NULL,\n            capability TEXT NOT NULL,\n            created_at INTEGER NOT NULL,\n            revoked_at INTEGER, peer_label TEXT NOT NULL DEFAULT '',\n            PRIMARY KEY(grant_id, capability)\n        )",
    legacy_index = "CREATE INDEX project_grants_active\n            ON project_grants(peer, local_project) WHERE revoked_at IS NULL"
)]
pub struct GrantRow {
    #[db(key)]
    pub grant_id: String,
    pub peer: String,
    pub peer_public_key: String,
    pub local_project: String,
    pub remote_project: String,
    #[db(key)]
    pub capability: Capability,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
    /// The peer server label, known at pairing time (migration 2026-11-02-project-grant-peer-label).
    pub peer_label: String,
}

/// The check trigger names, in creation order.
pub const CHECK_TRIGGERS: [&str; 2] =
    ["project_grants_check_insert", "project_grants_check_update"];

fn in_list(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| format!("'{value}'"))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn tables_sql() -> String {
    use studio_db_api::Table;
    GrantRow::descriptor()
        .ddl_statements()
        .map(|sql| {
            format!(
                "{};",
                sql.replacen("TABLE ", "TABLE IF NOT EXISTS ", 1).replacen(
                    "INDEX ",
                    "INDEX IF NOT EXISTS ",
                    1
                )
            )
        })
        .collect()
}

/// Generated `BEFORE INSERT` and `BEFORE UPDATE` check triggers. A revoked row
/// cannot change again, so a revocation cannot be undone.
pub fn check_triggers_sql() -> String {
    let valid = format!(
        "NEW.capability IN ({}) AND length(NEW.peer)>0 AND length(NEW.peer_public_key)>0 \
         AND length(NEW.local_project)>0 AND length(NEW.remote_project)>0 \
         AND (NEW.revoked_at IS NULL OR NEW.revoked_at>=NEW.created_at)",
        in_list(Capability::VARIANTS),
    );
    let final_revocation = "OLD.revoked_at IS NOT NULL AND NEW.revoked_at IS NOT OLD.revoked_at";
    let mut sql = String::new();
    for name in CHECK_TRIGGERS {
        let (event, guard) = if name.ends_with("insert") {
            ("INSERT", format!("NOT ({valid})"))
        } else {
            ("UPDATE", format!("NOT ({valid}) OR ({final_revocation})"))
        };
        sql.push_str(&format!(
            "DROP TRIGGER IF EXISTS {name};
             CREATE TRIGGER {name} BEFORE {event} ON {PROJECT_GRANTS_TABLE} WHEN {guard}
             BEGIN SELECT RAISE(ABORT,'invalid {PROJECT_GRANTS_TABLE} row'); END;\n"
        ));
    }
    sql
}

/// Create the table, its index and its triggers. The migration runs it once.
#[cfg(test)]
pub(crate) fn install(transaction: &Transaction<'_>) -> StoreResult<()> {
    transaction.execute_batch(&tables_sql())?;
    transaction.execute_batch(&check_triggers_sql())?;
    Ok(())
}

/// Insert the rows of a new pairing. A pairing is one `grant_id` with its capabilities.
pub fn insert_rows(transaction: &Transaction<'_>, rows: &[GrantRow]) -> StoreResult<()> {
    for row in rows {
        canvas_tx(transaction).insert(&GrantRow {
            revoked_at: None,
            ..row.clone()
        })?;
    }
    Ok(())
}

/// The active rows of the pairing with `peer` that grants `local_project`. Empty
/// when no pairing is active: an unknown peer, a revoked pairing or another project.
pub fn active_rows(
    transaction: &Transaction<'_>,
    peer: &str,
    local_project: &str,
) -> StoreResult<Vec<GrantRow>> {
    Ok(db_query!(
        canvas_tx(transaction),
        GrantRow,
        filter(
            GrantRow::PEER.eq(&peer.to_owned()),
            GrantRow::LOCAL_PROJECT.eq(&local_project.to_owned()),
            GrantRow::REVOKED_AT.is_null()
        ),
        order_by(GrantRow::CAPABILITY.asc)
    )?)
}

/// True when the peer key of `peer` has at least one active grant, for any project.
pub fn has_active_peer(transaction: &Transaction<'_>, peer: &str) -> StoreResult<bool> {
    // Raw SQL: authorization tests an aggregate without decoding unrelated grant fields.
    let count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM project_grants WHERE peer=?1 AND revoked_at IS NULL",
        params![peer],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Every row, active or revoked, for the owner's list.
pub fn all_rows(transaction: &Transaction<'_>) -> StoreResult<Vec<GrantRow>> {
    Ok(db_query!(
        canvas_tx(transaction),
        GrantRow,
        order_by(
            GrantRow::CREATED_AT.asc,
            GrantRow::GRANT_ID.asc,
            GrantRow::CAPABILITY.asc
        )
    )?)
}

/// The rows of one pairing, active or revoked. Empty when the grant id is unknown.
pub fn rows_of_grant(transaction: &Transaction<'_>, grant_id: &str) -> StoreResult<Vec<GrantRow>> {
    Ok(db_query!(
        canvas_tx(transaction),
        GrantRow,
        filter(GrantRow::GRANT_ID.eq(&grant_id.to_owned())),
        order_by(GrantRow::CAPABILITY.asc)
    )?)
}

/// Revoke one pairing. Returns the number of rows changed: zero when it was already revoked.
pub fn revoke(transaction: &Transaction<'_>, grant_id: &str, now: i64) -> StoreResult<usize> {
    Ok(db_update!(
        canvas_tx(transaction),
        GrantRow,
        set(GrantRow::REVOKED_AT = &Some(now)),
        filter(
            GrantRow::GRANT_ID.eq(&grant_id.to_owned()),
            GrantRow::REVOKED_AT.is_null()
        )
    )?)
}

/// The distinct public keys of the active pairings. The peer link accepts exactly these.
pub fn active_peer_keys(transaction: &Transaction<'_>) -> StoreResult<Vec<String>> {
    // Raw SQL: DISTINCT projects only public keys, preserving the authorization boundary.
    let mut query = transaction.prepare(
        "SELECT DISTINCT peer_public_key FROM project_grants WHERE revoked_at IS NULL ORDER BY peer_public_key",
    )?;
    Ok(query
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

/// The state of a pairing by id: `None` when unknown, `Some(true)` when active and
/// `Some(false)` when revoked.
pub fn grant_active(transaction: &Transaction<'_>, grant_id: &str) -> StoreResult<Option<bool>> {
    // Raw SQL: aggregate NULL distinguishes an unknown pairing from a revoked pairing.
    let active: Option<bool> = transaction.query_row(
        "SELECT MAX(revoked_at IS NULL) FROM project_grants WHERE grant_id=?1",
        [grant_id],
        |row| row.get(0),
    )?;
    Ok(active)
}

/// The capabilities of the active pairing for `peer` and `local_project`, if any.
pub fn active_capabilities(
    transaction: &Transaction<'_>,
    peer: &str,
    local_project: &str,
) -> StoreResult<BTreeSet<Capability>> {
    Ok(active_rows(transaction, peer, local_project)?
        .into_iter()
        .map(|row| row.capability)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> rusqlite::Connection {
        let mut db = rusqlite::Connection::open_in_memory().expect("database");
        let tx = db.transaction().expect("transaction");
        install(&tx).expect("grant table installs");
        tx.commit().expect("commit");
        db
    }

    fn row(capability: &str) -> rusqlite::Result<usize> {
        let db = database();
        db.execute(
            "INSERT INTO project_grants(grant_id,peer,peer_public_key,local_project,remote_project,capability,created_at,revoked_at) VALUES('g','peer','PEM','a','b',?1,10,NULL)",
            [capability],
        )
    }

    #[test]
    fn capability_column_accepts_only_the_enum_values() {
        assert_eq!(row("LeadMessages"), Ok(1));
        assert!(row("ReadProjectState").is_err());
    }

    #[test]
    fn revocation_is_final() {
        let db = database();
        db.execute(
            "INSERT INTO project_grants(grant_id,peer,peer_public_key,local_project,remote_project,capability,created_at,revoked_at) VALUES('g','peer','PEM','a','b','LeadMessages',10,NULL)",
            [],
        )
        .expect("insert");
        db.execute("UPDATE project_grants SET revoked_at=20", [])
            .expect("revoke");
        assert!(
            db.execute("UPDATE project_grants SET revoked_at=NULL", [])
                .is_err()
        );
    }

    #[test]
    fn revocation_cannot_precede_creation() {
        let db = database();
        assert!(
            db.execute(
                "INSERT INTO project_grants(grant_id,peer,peer_public_key,local_project,remote_project,capability,created_at,revoked_at) VALUES('g','peer','PEM','a','b','LeadMessages',10,5)",
                [],
            )
            .is_err()
        );
    }

    #[test]
    fn an_active_grant_marks_the_peer_for_any_project() {
        let mut db = database();
        let tx = db.transaction().expect("transaction");
        insert_rows(
            &tx,
            &[GrantRow {
                grant_id: "g1".into(),
                peer: "peer-1".into(),
                peer_public_key: "PEM".into(),
                local_project: "proj-a".into(),
                remote_project: "proj-b".into(),
                capability: Capability::LeadMessages,
                created_at: 1,
                revoked_at: None,
                peer_label: String::new(),
            }],
        )
        .expect("grant written");
        assert!(has_active_peer(&tx, "peer-1").expect("query"));
        assert!(!has_active_peer(&tx, "peer-2").expect("query"));
        revoke(&tx, "g1", 2).expect("revoked");
        assert!(!has_active_peer(&tx, "peer-1").expect("query"));
    }
}
