//! Descriptor-bound lookups used by writer transactions.
use crate::agent_tables::AgentsStorageRow;
use crate::agent_tables::SessionsStorageRow;
use crate::item_monitor_terminal::MonitorsRow;
use crate::peer_update_settings::Canvas;
use crate::planning_work::TaskRow;
use crate::queue_schema::ExecutionRunsRow;

#[studio_db_api::db_queries]
mod queries {
    use super::*;

    #[derive(studio_db_api::DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,String),
        tables(runs=ExecutionRunsRow,agents=AgentsStorageRow,sessions=SessionsStorageRow),indexes(runs=ExecutionRunsRow::INDEX_RUNTIME_EXECUTION_RUN_TERMINAL),
        columns(id=ExecutionRunsRow::ID,record=ExecutionRunsRow::RECORD,agent=ExecutionRunsRow::AGENT,turn=ExecutionRunsRow::TURN,account=ExecutionRunsRow::ACCOUNT,thread=ExecutionRunsRow::THREAD,created=ExecutionRunsRow::CREATED,
            agent_id=AgentsStorageRow::ID,key=AgentsStorageRow::ACCOUNT_KEY,chat=SessionsStorageRow::CHAT,session_thread=SessionsStorageRow::THREAD_ID,state=SessionsStorageRow::STATE),
        sql="SELECT {id},{record} FROM {runs} WHERE {agent}=?1 AND {turn}=?2
            AND {account}=(SELECT {key} FROM {agents} WHERE {agent_id}=?1)
            AND {thread} IS (SELECT {session_thread} FROM {sessions} WHERE {chat}=?1 AND {state}<>'ended') ORDER BY {created},{id}")]
    pub struct TerminalRuns {
        #[db(column=AgentsStorageRow::ID)]
        pub agent: String,
        #[db(column=ExecutionRunsRow::TURN)]
        pub turn: Option<String>,
    }

    // Unary + prevents the account index from replacing the bounded session/ID
    // lookup. Both the stored account column and its argument are text.
    // Selection order is immaterial: the caller accepts exactly one ID.
    #[derive(studio_db_api::DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=many,result=String,
        decode=AgentsStorageRow::ID,tables(agents=AgentsStorageRow,sessions=SessionsStorageRow),
        columns(id=AgentsStorageRow::ID,account=AgentsStorageRow::ACCOUNT_KEY,chat=SessionsStorageRow::CHAT,thread=SessionsStorageRow::THREAD_ID,state=SessionsStorageRow::STATE),
        sql="SELECT {id} FROM {agents} WHERE {id} IN (SELECT {chat} FROM {sessions} WHERE {thread}=?1 AND {state}<>'ended') AND +{account}=?2 AND (?3 IS NULL OR {id}=?3) LIMIT 2")]
    pub struct ThreadAgentIds {
        #[db(column=SessionsStorageRow::THREAD_ID)]
        pub thread: String,
        #[db(column=AgentsStorageRow::ACCOUNT_KEY)]
        pub account: String,
        #[db(column=AgentsStorageRow::ROOT_ID)]
        pub actor: Option<crate::AgentId>,
    }

    #[derive(studio_db_api::DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=many,result=String,
        decode=TaskRow::ID,tables(tasks=TaskRow,agents=AgentsStorageRow),
        columns(task_id=TaskRow::ID,agent_id=AgentsStorageRow::ID,record=TaskRow::RECORD),
        sql="SELECT t.{task_id} AS {task_id} FROM {tasks} t CROSS JOIN {agents} a ON a.{agent_id}=json_extract(t.{record},'$.agent')
            WHERE a.account_key=?1 AND json_extract(t.{record},'$.itemId')=?2
            AND json_extract(t.{record},'$.turnId')=?3 AND +a.{agent_id}=json_extract(t.{record},'$.agent')
            AND t.{task_id}=a.{agent_id}||':'||?2 AND a.deleted_at IS NOT NULL LIMIT 2")]
    pub struct DeletedCommandIds {
        #[db(column=AgentsStorageRow::ID)]
        pub account: String,
        #[db(column=TaskRow::ID)]
        pub item: String,
        #[db(column=TaskRow::ID)]
        pub turn: String,
    }

    // The descending first non-null value equals MAX; optional cardinality owns
    // the empty case without pretending the descriptor's non-null f64 is nullable.
    #[derive(studio_db_api::DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=optional,result=f64,
        decode=MonitorsRow::MONITOR_CREATED,tables(monitors=MonitorsRow),
        columns(record=MonitorsRow::RECORD,created=MonitorsRow::MONITOR_CREATED),
        sql="SELECT json_extract({record},'$.created') AS {created} FROM {monitors}
            WHERE json_extract({record},'$.status')='completed' AND json_extract({record},'$.created')>=?1
            AND json_extract({record},'$.ruleId')=?2 ORDER BY json_extract({record},'$.created') DESC LIMIT 1")]
    pub struct LastRuleSuccess {
        #[db(column=MonitorsRow::MONITOR_CREATED)]
        pub since: f64,
        #[db(column=MonitorsRow::ID)]
        pub rule: String,
    }
}
pub use queries::{DeletedCommandIds, LastRuleSuccess, TerminalRuns, ThreadAgentIds};

#[cfg(test)]
mod tests {
    #[test]
    fn terminal_runs_read_only_the_turn_in_creation_and_id_order() {
        use crate::peer_update_settings::{canvas_read, canvas_tx};
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let id = crate::AgentId::from("agent");
        let mut agent = crate::AgentRecord::new(
            id.clone(),
            id,
            0,
            crate::AgentStatus::Waiting,
            "default".into(),
        );
        agent.provider = studio_runtime_schema::queue_command::Field::Value(
            studio_runtime_providers::ProviderKind::Codex,
        );
        agent.thread_id = Some("thread".into());
        crate::AgentRepository::put(&tx, &agent).unwrap();
        let insert = |id: String, turn: String, account: &str, thread: &str, created: f64| {
            canvas_tx(&tx)
                .insert(&super::ExecutionRunsRow {
                    record: studio_json::json!({"id":id,"status":"pending"})
                        .to_string()
                        .into(),
                    id,
                    agent: "agent".into(),
                    account: account.into(),
                    epoch: 0,
                    thread: Some(thread.into()),
                    turn: Some(turn),
                    created,
                    status: None,
                })
                .unwrap();
        };
        for (id, created) in [("b", 1.0), ("a", 1.0), ("c", 2.0)] {
            insert(id.into(), "turn".into(), "default", "thread", created);
        }
        insert(
            "foreign-account".into(),
            "turn".into(),
            "other",
            "thread",
            0.0,
        );
        insert(
            "foreign-thread".into(),
            "turn".into(),
            "default",
            "other",
            0.0,
        );
        let params = super::TerminalRuns {
            agent: "agent".into(),
            turn: Some("turn".into()),
        };
        let read = || canvas_read(&tx).collect(&super::TerminalRuns::QUERY, &params);
        let (small_rows, small) = crate::query_plan_test_support::vm_steps(&tx, read).unwrap();
        for index in 0..2000 {
            insert(
                format!("history-{index}"),
                format!("turn-{index}"),
                "default",
                "thread",
                index as f64,
            );
        }
        let (large_rows, large) = crate::query_plan_test_support::vm_steps(&tx, read).unwrap();
        assert_eq!(small_rows, large_rows);
        assert_eq!(
            large_rows
                .iter()
                .map(|row| row.0.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        eprintln!("terminal runs: {small} -> {large} VM steps behind 2000 unrelated turns");
        assert!(
            large <= small + 20,
            "terminal query visited unrelated turn history: {small} -> {large}"
        );
    }

    #[test]
    fn terminal_runs_have_no_scan_or_sort() {
        let db = crate::fresh_state::test_canvas();
        let params = super::TerminalRuns {
            agent: "agent".into(),
            turn: Some("turn".into()),
        };
        let plan =
            crate::query_plan_test_support::explain(&db, super::TerminalRuns::QUERY.sql(), &params)
                .unwrap();
        assert!(
            plan.iter()
                .all(|step| !step.contains("SCAN ") && !step.contains("TEMP B-TREE")),
            "{plan:?}"
        );
    }
    #[test]
    fn thread_agent_lookup_has_no_scan_or_sort() {
        let db = crate::fresh_state::test_canvas();
        let params = super::ThreadAgentIds {
            thread: "thread".into(),
            account: "default".into(),
            actor: None,
        };
        let plan = crate::query_plan_test_support::explain(
            &db,
            super::ThreadAgentIds::QUERY.sql(),
            &params,
        )
        .unwrap();
        assert!(
            plan.iter()
                .all(|step| !step.contains("SCAN ") && !step.contains("TEMP B-TREE")),
            "{plan:?}"
        );
    }

    #[test]
    fn writer_query_descriptors_prepare_against_canvas() {
        let db = crate::fresh_state::test_canvas();
        super::queries::validate_queries(&db).unwrap();
    }
}
