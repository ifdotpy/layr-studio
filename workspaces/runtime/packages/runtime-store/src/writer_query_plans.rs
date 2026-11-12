//! Short-tier plan witness for SQL executed by real writer callbacks.
//! Only isolated test connections may use this: it temporarily replaces their trace hook.
use std::cell::RefCell;
use std::collections::BTreeSet;

use rusqlite::config::DbConfig;
use rusqlite::trace::{TraceEvent, TraceEventCodes};
use rusqlite::{Connection, Transaction};

thread_local! {
    static STATEMENTS: RefCell<Option<BTreeSet<(String, String)>>> = const { RefCell::new(None) };
}

struct Capture<'a> {
    connection: &'a Connection,
    trigger_eqp: bool,
}
impl Drop for Capture<'_> {
    fn drop(&mut self) {
        self.connection.trace_v2(TraceEventCodes::empty(), None);
        self.connection
            .set_db_config(DbConfig::SQLITE_DBCONFIG_TRIGGER_EQP, self.trigger_eqp)
            .expect("restore trigger plans");
        STATEMENTS.with_borrow_mut(|statements| *statements = None);
    }
}

/// Capture statements actually stepped inside this transaction, including generated
/// db-api statements, then explain each on the same production schema. No SQL inventory
/// is copied into the witness. Values stay in memory and are never printed.
#[track_caller]
pub fn assert_writer_query_plans<T>(
    tx: &Transaction<'_>,
    bounded_json_queries: &[(&str, &str)],
    operation: impl FnOnce() -> T,
) -> T {
    assert!(
        !tx.is_autocommit(),
        "the witness must run inside a transaction"
    );
    STATEMENTS.with_borrow_mut(|statements| {
        assert!(statements.is_none(), "nested writer plan capture");
        *statements = Some(BTreeSet::new());
    });
    let capture = Capture {
        connection: tx,
        trigger_eqp: tx
            .db_config(DbConfig::SQLITE_DBCONFIG_TRIGGER_EQP)
            .expect("trigger plan setting"),
    };
    tx.set_db_config(DbConfig::SQLITE_DBCONFIG_TRIGGER_EQP, true)
        .expect("include trigger plans");
    tx.trace_v2(
        TraceEventCodes::SQLITE_TRACE_STMT,
        Some(|event| {
            if let TraceEvent::Stmt(statement, _) = event {
                STATEMENTS.with_borrow_mut(|statements| {
                    if let Some(statements) = statements {
                        statements.insert((
                            statement.sql().into_owned(),
                            statement.expanded_sql().expect("expanded SQL"),
                        ));
                    }
                });
            }
        }),
    );
    let value = operation();
    tx.trace_v2(TraceEventCodes::empty(), None);
    let statements = STATEMENTS.with_borrow_mut(Option::take).expect("capture");
    assert!(!statements.is_empty(), "writer witness executed no SQL");
    let mut failures = BTreeSet::new();
    for (sql, expanded) in &statements {
        let mut explain = tx
            .prepare(&format!("EXPLAIN QUERY PLAN {expanded}"))
            .expect("explain executed SQL");
        let plan = explain
            .query_map([], |row| row.get::<_, String>(3))
            .expect("plan");
        let mut trigger = None;
        for detail in plan {
            let detail = detail.expect("plan row");
            if let Some(name) = detail.strip_prefix("-- TRIGGER ") {
                trigger = Some(name.to_owned());
            }
            // entity_scopes owns these two row-scope validators. Their JSON walks
            // inspect OLD/NEW payloads or a previous row selected by its full PK;
            // they never enumerate a historical table. Persistent scans still fail.
            let row_scope_json = matches!(
                trigger.as_deref(),
                Some("immutable_scope_sync_entities" | "immutable_scope_sync_entities_insert")
            );
            let bounded_json = detail.contains("VIRTUAL TABLE INDEX")
                && ((row_scope_json && detail.starts_with("SCAN json_each "))
                    || bounded_json_queries
                        .iter()
                        .any(|(allowed, _reason)| *allowed == sql.as_str()));
            // These describe computed rows, not persistent table scans. Covering index
            // scans are admitted explicitly; there is no persistent-table allowlist.
            if detail.starts_with("SCAN ")
                && !bounded_json
                && !detail.contains("USING COVERING INDEX")
                && !detail.starts_with("SCAN rarray VIRTUAL TABLE INDEX")
                && !detail.starts_with("SCAN CONSTANT ROW")
                && !detail.starts_with("SCAN (subquery-")
            {
                failures.insert(format!("{detail}: {sql}"));
            }
        }
    }
    drop(capture);
    assert!(
        failures.is_empty(),
        "writer SQL must search persistent tables:\n{}",
        failures.into_iter().collect::<Vec<_>>().join("\n")
    );
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "SCAN a")]
    fn writer_query_plans_reject_an_aliased_hot_table_scan() {
        let mut db = crate::test_canvas();
        let tx = db.transaction().unwrap();
        assert_writer_query_plans(&tx, &[], || {
            tx.prepare("SELECT a.record FROM runtime_agents a NOT INDEXED")
                .unwrap()
                .query([])
                .unwrap()
                .next()
                .unwrap();
        });
    }

    #[test]
    #[should_panic(expected = "SCAN runtime_agents")]
    fn writer_query_plans_include_trigger_programs() {
        let mut db = crate::test_canvas();
        db.execute_batch("CREATE TRIGGER writer_plan_regression AFTER INSERT ON graph_edges BEGIN SELECT record FROM runtime_agents NOT INDEXED; END;").unwrap();
        let tx = db.transaction().unwrap();
        assert_writer_query_plans(&tx, &[], || {
            crate::CoordinationRepository::put_edge(&tx, "edge", "agent", "room").unwrap();
        });
    }

    #[test]
    #[should_panic(expected = "SCAN source_key")]
    fn writer_query_plans_reject_persistent_alias_inside_named_computed_rows() {
        let mut db = crate::test_canvas();
        let tx = db.transaction().unwrap();
        assert_writer_query_plans(&tx, &[], || {
            tx.prepare("WITH source_key AS (SELECT source_key.record FROM runtime_agents source_key NOT INDEXED LIMIT 5) SELECT * FROM source_key LIMIT 1")
                .unwrap().query([]).unwrap().next().unwrap();
        });
    }

    #[test]
    fn writer_query_plans_cover_receipts_windows_and_record_lookups() {
        let mut db = crate::test_canvas();
        let tx = db.transaction().unwrap();
        assert_writer_query_plans(&tx, &[], || {
            for table in [
                "runtime_agents",
                "runtime_tool_requests",
                "runtime_monitors",
                "runtime_tasks",
            ] {
                crate::system_reads::get(&tx, table, "missing").unwrap();
            }
            crate::QueueEventRepository::get(&tx, &crate::QueueEventId("missing".into())).unwrap();
            crate::system_reads::tool_request_aliases(&tx, "agent", "alias").unwrap();
            crate::durable_seq::window_members(&tx, crate::durable_seq::MONITOR_WINDOW).unwrap();
            crate::durable_seq::join_window(&tx, crate::durable_seq::MONITOR_WINDOW, "monitor")
                .unwrap();
            crate::durable_seq::leave_window(&tx, crate::durable_seq::MONITOR_WINDOW, "monitor")
                .unwrap();
            crate::MonitorsRepository::is_held(&tx, "monitor").unwrap();
            crate::MonitorsRepository::studio_error_candidates(&tx, 0.0).unwrap();
            crate::AgentIndexedReads::team_records(&tx, "root").unwrap();
            crate::CoordinationRepository::put_group(
                &tx,
                &crate::ChatGroup {
                    id: "room".into(),
                    name: "Room".into(),
                    members: Vec::new(),
                    original_members: "[]".into(),
                },
            )
            .unwrap();
            crate::CoordinationRepository::put_edge(&tx, "edge", "agent", "room").unwrap();
            crate::CoordinationRepository::group(&tx, "room").unwrap();
            crate::CoordinationRepository::messages(&tx, "room").unwrap();
            crate::CoordinationRepository::participants(&tx, "room").unwrap();
            crate::CoordinationRepository::remove_edge(&tx, "edge").unwrap();
            let read = crate::peer_update_settings::canvas_read(&tx);
            read.collect(
                &crate::writer_queries::DeletedCommandIds::QUERY,
                &crate::writer_queries::DeletedCommandIds {
                    account: "account".into(),
                    item: "item".into(),
                    turn: "turn".into(),
                },
            )
            .unwrap();
            read.named(
                &crate::writer_queries::LastRuleSuccess::QUERY,
                &crate::writer_queries::LastRuleSuccess {
                    since: 0.0,
                    rule: "rule".into(),
                },
            )
            .unwrap();
        });
    }
}
