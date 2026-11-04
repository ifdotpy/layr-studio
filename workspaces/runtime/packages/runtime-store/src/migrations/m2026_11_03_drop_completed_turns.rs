//! Drop `runtime_completed_turns`. The domain model migration copied its rows into
//! `turns`, and `domain::turn_completed` and `domain::complete_turn` read and write
//! only `turns`. Dropping the table also drops its read only triggers.
use rusqlite::Transaction;

use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute_batch("DROP TABLE IF EXISTS runtime_completed_turns")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_completed_turns_table_drops_and_turns_keep_the_turn() {
        let mut db = crate::fresh_state::test_canvas();
        crate::domain::complete_turn(&db, "c", "p1").expect("completed turn");
        let tx = db.transaction().expect("transaction");
        apply(&tx).expect("drop applies");
        tx.commit().expect("commit");
        assert!(!crate::schema::table_exists(&db, "runtime_completed_turns").unwrap());
        assert!(crate::domain::turn_completed(&db, "c", "p1").unwrap());
        let tx = db.transaction().expect("transaction");
        apply(&tx).expect("the drop is repeatable");
        tx.commit().expect("commit");
    }
}
