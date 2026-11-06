use rusqlite::Transaction;

use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute_batch(
        "DROP INDEX IF EXISTS runtime_event_repair_candidates_v2;
         CREATE INDEX IF NOT EXISTS runtime_event_repair_candidates_v3 ON runtime_events(agent,created,id)
         WHERE status='delivered' AND kind IN ('monitor_exit','agent_message','work_review','work_decision');",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::apply;

    #[test]
    fn replaces_old_index_and_is_repeatable_on_fresh_schema() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        tx.execute_batch("DROP INDEX IF EXISTS runtime_event_repair_candidates_v3; CREATE INDEX runtime_event_repair_candidates_v2 ON runtime_events(agent,created) WHERE status='delivered' AND kind IN ('monitor_exit','agent_message','work_review','work_decision');").unwrap();
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        let indexes: Vec<String> = tx.prepare("SELECT name FROM sqlite_schema WHERE name IN ('runtime_event_repair_candidates_v2','runtime_event_repair_candidates_v3') ORDER BY name").unwrap().query_map([], |row| row.get(0)).unwrap().map(Result::unwrap).collect();
        assert_eq!(indexes, ["runtime_event_repair_candidates_v3"]);
    }
}
