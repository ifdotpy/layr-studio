//! The existing Python multi-server rename journal; no new storage format.

use crate::access_tables::{AccessAuditRow, AccessNameRequestsRow, AccessNameTargetsRow};
use crate::peer_update_settings::canvas_tx;
use crate::{
    StoreResult,
    schema::{SchemaRequirements, TableRequirement},
};
use rusqlite::{OptionalExtension, Transaction};
use studio_db_api::db_update;

const TABLES: &[TableRequirement] = &[
    TableRequirement {
        name: "runtime_access_name_requests",
        columns: &["id", "label", "actor", "applied"],
    },
    TableRequirement {
        name: "runtime_access_name_targets",
        columns: &["id", "server", "label"],
    },
    TableRequirement {
        name: "runtime_access_audit",
        columns: &["seq", "client", "actor", "action", "created"],
    },
];
pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: TABLES,
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

pub struct FederationCredentialsRepository;

impl FederationCredentialsRepository {
    /// Return false on an ID conflict without changing either journal.
    pub fn reserve_name(
        tx: &Transaction<'_>,
        request_id: &str,
        label: &str,
        actor: &str,
    ) -> StoreResult<bool> {
        let prior = canvas_tx(tx).get::<AccessNameTargetsRow>(&(request_id.to_owned(),))?;
        if prior.is_some_and(|row| row.server != "local" || row.label != label) {
            return Ok(false);
        }
        // Raw SQL: this partial projection must not decode unrelated legacy actor/applied fields.
        let prior = tx
            .query_row(
                "SELECT label FROM runtime_access_name_requests WHERE id=?1",
                [request_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if prior.as_deref().is_some_and(|saved| saved != label) {
            return Ok(false);
        }
        canvas_tx(tx).insert_new(&AccessNameTargetsRow {
            id: request_id.into(),
            server: "local".into(),
            label: label.into(),
        })?;
        canvas_tx(tx).insert_new(&AccessNameRequestsRow {
            id: request_id.into(),
            label: label.into(),
            actor: actor.into(),
            applied: 0,
        })?;
        Ok(true)
    }

    pub fn pending_names(tx: &Transaction<'_>) -> StoreResult<Vec<(String, String, String)>> {
        // Raw SQL: legacy rowid order is not a declared descriptor column.
        let mut statement = tx.prepare_cached("SELECT id,label,actor FROM runtime_access_name_requests WHERE applied=0 ORDER BY rowid")?;
        Ok(statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn complete_name(
        tx: &Transaction<'_>,
        id: &str,
        server_id: &str,
        actor: &str,
        now: f64,
    ) -> StoreResult<()> {
        db_update!(
            canvas_tx(tx),
            AccessNameRequestsRow,
            set(AccessNameRequestsRow::APPLIED = &1),
            filter(AccessNameRequestsRow::ID.eq(&id.to_owned()))
        )?;
        canvas_tx(tx).insert_rowid(&AccessAuditRow {
            seq: 0,
            client: server_id.into(),
            actor: actor.into(),
            action: "name".into(),
            created: now,
        })?;
        // Raw SQL: retention uses an aggregate subquery over the monotonic audit sequence.
        tx.execute("DELETE FROM runtime_access_audit WHERE seq <= (SELECT max(seq)-10000 FROM runtime_access_audit)", [])?;
        Ok(())
    }
}

inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "federation_credentials",
        requirements: &REQUIREMENTS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::Table;

    #[test]
    fn matching_name_request_keeps_partial_legacy_admission() {
        let mut db = rusqlite::Connection::open_in_memory().expect("db");
        db.execute_batch(AccessNameTargetsRow::DDL)
            .expect("targets");
        db.execute_batch(AccessNameRequestsRow::DDL)
            .expect("requests");
        db.execute_batch("INSERT INTO runtime_access_name_targets VALUES('r','local','label'); INSERT INTO runtime_access_name_requests VALUES('r','label',x'00',x'00');").expect("legacy row");
        let tx = db.transaction().expect("tx");
        let before = tx.total_changes();
        assert!(
            FederationCredentialsRepository::reserve_name(&tx, "r", "label", "actor")
                .expect("matching label ignores unrelated fields")
        );
        assert!(
            !FederationCredentialsRepository::reserve_name(&tx, "r", "other", "actor")
                .expect("content conflict")
        );
        assert_eq!(
            tx.total_changes(),
            before,
            "retry does not replace either journal"
        );
    }
}
