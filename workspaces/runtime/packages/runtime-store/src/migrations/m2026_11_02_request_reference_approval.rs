//! An answer command refers to its approval, so `request_results` admits the `approval`
//! reference kind. The kind check is a generated trigger, not a table CHECK, so the
//! migration replaces the two triggers and rebuilds no table.
use rusqlite::Transaction;
use studio_runtime_schema::persistence::{RequestReferenceKind, enum_check_triggers};

use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "request_results")? {
        return Ok(());
    }
    tx.execute_batch(&enum_check_triggers::<RequestReferenceKind>(
        "request_results",
        "ref_kind",
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// The approval kind is checked by a trigger, and a row of another kind is still refused.
    #[test]
    fn approval_is_a_valid_reference_kind_and_others_are_refused() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE request_results (
               id TEXT PRIMARY KEY, signature TEXT NOT NULL,
               status TEXT NOT NULL DEFAULT 'applied',
               ref_kind TEXT, ref_id TEXT, ref_rev INTEGER, result TEXT, created INTEGER NOT NULL);",
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        tx.execute(
            "INSERT INTO request_results VALUES('a','s','applied','approval','x',0,NULL,1)",
            [],
        )
        .expect("approval is a valid kind");
        let refused = tx.execute(
            "INSERT INTO request_results VALUES('b','s','applied','bogus','x',0,NULL,2)",
            [],
        );
        assert!(refused.is_err());
    }
}
