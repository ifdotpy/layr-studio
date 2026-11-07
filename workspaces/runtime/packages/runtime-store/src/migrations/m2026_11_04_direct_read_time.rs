//! Plans, annotations, rules and approvals get their client documents at read time
//! (`entity_scope` in runtime-services). Number their rows in `durable_rows`, delete the
//! old copies in `sync_entities` and the triggers that wrote tombstones for them. The
//! counter keeps every number already used, so no cursor repeats a number.
use rusqlite::Transaction;

use crate::StoreResult;

/// Delete triggers of the removed copies (`feature_schema` created them).
const RETIRED_TRIGGERS: &[&str] = &[
    "summary_plan_delete",
    "summary_annotation_delete",
    "summary_questionHistory_delete",
];

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for trigger in RETIRED_TRIGGERS {
        tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {trigger}"))?;
    }
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    // A new number for each existing row; the triggers number later writes.
    crate::durable_seq::install(tx)?;
    // The collections of this step; later steps move more collections themselves.
    let list = [
        "plan",
        "annotation",
        "rule",
        "request",
        "questionHistory",
        "complaint",
        "edge",
        "chat",
    ]
    .iter()
    .map(|collection| format!("'{collection}'"))
    .collect::<Vec<_>>()
    .join(",");
    let copies: bool = tx.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection IN ({list}))"),
        [],
        |row| row.get(0),
    )?;
    if copies {
        crate::durable_seq::keep_counter(tx)?;
        tx.execute_batch(&format!(
            "DELETE FROM sync_entities WHERE collection IN ({list})"
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_go_rows_get_numbers_and_the_counter_keeps_its_numbers() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        tx.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('plan','p',999999,'h','{}',0),('workspace','current',5,'h','{}',0)",
            [],
        )
        .expect("old copies");
        tx.execute_batch("DROP TRIGGER IF EXISTS durable_runtime_rules_insert")
            .expect("rows written before the source");
        tx.execute(
            "INSERT INTO runtime_rules(id,record) VALUES('r','{\"id\":\"r\"}')",
            [],
        )
        .expect("rule row");
        tx.execute("DELETE FROM durable_rows WHERE tbl='runtime_rules'", [])
            .expect("an unnumbered row");
        apply(&tx).expect("migration applies");
        let count = |sql: &str| -> i64 { tx.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            count("SELECT COUNT(*) FROM sync_entities WHERE collection='plan'"),
            0
        );
        assert_eq!(
            count("SELECT COUNT(*) FROM sync_entities WHERE collection='workspace'"),
            1
        );
        let rule_seq = count("SELECT seq FROM durable_rows WHERE tbl='runtime_rules' AND id='r'");
        assert!(
            rule_seq > 999_999,
            "a new number is above every used number"
        );
        apply(&tx).expect("the migration is repeatable");
        tx.commit().expect("commit");
    }
}
