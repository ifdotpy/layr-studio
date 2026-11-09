//! Durable conditions that keep a loaded provider thread, or an account's
//! provider process, in use. An account worker unloads an idle thread only
//! while none holds; any doubt (an unknown start, unsettled input or request)
//! keeps the thread loaded.
use super::*;
use crate::startup_recovery::{active_monitor_predicate, unsettled_tool_request_predicate};

/// Agent statuses whose turn may still be starting or running.
const ACTIVE_AGENT_STATUSES: &str = "'queued','starting','running','approval'";
/// Context repair phases that still read or replace the agent's thread.
const ACTIVE_CONTEXT_REPAIR_PHASES: &str = "'preparing','submitted','unknown','ready'";
/// Context repair source cleanup phases that still own the source thread.
const ACTIVE_SOURCE_CLEANUP_PHASES: &str = "'planned','submitted'";

/// SQL predicate over the `runtime_agents` row (unqualified `record`): the
/// agent has a turn, input, request, monitor, or operation in progress.
fn agent_in_use() -> String {
    let mut checks = vec![
        format!(
            "COALESCE({},'') IN ({ACTIVE_AGENT_STATUSES})",
            crate::domain::chat_status_sql("runtime_agents.id")
        ),
        // The unfinished turn of the chat, which also covers the provider turn.
        crate::domain::chat_busy_sql("runtime_agents.id"),
        format!(
            "COALESCE(json_array_length({}),0)>0",
            crate::agent_volatile::field_sql("runtime_agents", "activeTools")
        ),
        "workspace_operation IS NOT NULL".to_owned(),
        "COALESCE(account_transfer_id,'')!=''".to_owned(),
        "COALESCE(native_tool_refresh_id,'')!=''".to_owned(),
        "COALESCE(native_release_reset_pending,0)!=0".to_owned(),
        format!("context_repair_phase IN ({ACTIVE_CONTEXT_REPAIR_PHASES})"),
        format!("({})", unresolved_sql()),
        format!(
            "EXISTS(SELECT 1 FROM runtime_events e WHERE e.agent=runtime_agents.id \
             AND e.status IN ({UNSETTLED_EVENT_STATUSES}))"
        ),
    ];
    for (table, predicate, agent) in [
        ("runtime_monitors", active_monitor_predicate(), "agent"),
        (
            "runtime_tool_requests",
            unsettled_tool_request_predicate(),
            // schema::receipt_owner_columns defines agent with
            // "WHEN 'managed_tool' THEN record->>'$.agent'". The predicate
            // restricts kind to managed_tool, so this equals json_extract(record,'$.agent')
            // while allowing the status/agent index to exclude unrelated callbacks.
            "agent",
        ),
    ] {
        checks.push(format!(
            "EXISTS(SELECT 1 FROM {table} WHERE {predicate} \
             AND {agent}=runtime_agents.id)"
        ));
    }
    // An open approval (pending or being answered) holds the thread.
    checks.push(
        "EXISTS(SELECT 1 FROM approvals WHERE state IN ('pending','answering') \
             AND agent=runtime_agents.id)"
            .to_owned(),
    );
    checks.join(" OR ")
}

fn exists(connection: &Connection, sql: &str, values: &[&str]) -> StoreResult<bool> {
    Ok(connection.query_row(
        &format!("SELECT EXISTS({sql})"),
        rusqlite::params_from_iter(values),
        |row| row.get(0),
    )?)
}

/// A pending account transfer that reads from or writes to `?1`.
fn transfer_pending(connection: &Connection, account: &str) -> StoreResult<bool> {
    exists(
        connection,
        "SELECT 1 FROM runtime_account_transfers t WHERE json_extract(t.record,'$.status')='pending' \
         AND (t.target_account_key=?1 OR EXISTS(SELECT 1 FROM \
         json_each(t.record,'$.members') m WHERE json_extract(m.value,'$.sourceAccountKey')=?1))",
        &[account],
    )
}

fn transfer_pending_predicate() -> String {
    "EXISTS(SELECT 1 FROM runtime_account_transfers t \
        WHERE json_extract(t.record,'$.status')='pending' \
        AND (t.target_account_key=?1 OR EXISTS(SELECT 1 FROM \
        json_each(t.record,'$.members') m WHERE json_extract(m.value,'$.sourceAccountKey')=?1)))"
        .to_owned()
}

/// A running command keeps its original provider after its actor changes account.
/// `?1` is the source account key, not the actor's current model account.
fn monitor_source_in_use() -> String {
    // The partial source-account index excludes settled command history.
    format!(
        "EXISTS(SELECT 1 FROM runtime_monitors WHERE {} \
        AND source_account_key=?1)",
        active_monitor_predicate()
    )
}

/// Work item statuses that are done: the owner expects no further turn for them.
/// Any other status (`ready`, `running`, `blocked`, `review`) means a
/// continuation, a rejection, or a follow-up may still reach the owner.
const SETTLED_WORK_STATUSES: &str = "'accepted','cancelled'";

/// SQL predicate over the `runtime_agents` row: a finished managed worker. It
/// is not a lead (leads and ordinary chats are leads), it is not in the
/// `waiting` state (pending children, watches, or a hold), and it owns no open
/// work item. A waiting worker will likely get another turn soon, so it keeps
/// the ordinary idle timer.
fn finished_worker_sql() -> String {
    let mut predicate = format!(
        "COALESCE(is_lead,0)=0 \
         AND COALESCE({},'')!='waiting'",
        crate::domain::chat_status_sql("runtime_agents.id")
    );
    predicate.push_str(&format!(
        " AND NOT EXISTS(SELECT 1 FROM runtime_work w \
             WHERE w.owner=runtime_agents.id \
             AND json_extract(w.record,'$.status') NOT IN ({SETTLED_WORK_STATUSES}))"
    ));
    predicate
}

const ACCOUNT_AGENT_SQL: &str = "SELECT 1 FROM runtime_agents INDEXED BY runtime_agent_idle_unload WHERE account_key=?1 AND deleted_at IS NULL";

/// Find session owners by primary key rather than scanning every account agent
/// for each loaded thread. CROSS JOIN keeps the indexed session lookup first.
fn thread_agent_sql(thread: &str) -> String {
    format!(
        "SELECT 1 FROM sessions s CROSS JOIN runtime_agents \
         WHERE s.thread_id={thread} AND s.state<>'ended' AND runtime_agents.id=s.chat \
         AND runtime_agents.account_key=?1 AND runtime_agents.deleted_at IS NULL"
    )
}

impl SchedulerRepository {
    /// Read the durable status of known loaded threads and the account in one
    /// bounded query. The candidate list comes from one provider snapshot.
    /// Returns the busy threads, the threads that unload at once (see
    /// [`Self::idle_unload_immediate_threads`]), and whether
    /// the account is in use.
    pub fn idle_unload_usage_snapshot(
        connection: &Connection,
        account: &str,
        threads: &std::collections::BTreeSet<String>,
    ) -> StoreResult<(
        std::collections::BTreeSet<String>,
        std::collections::BTreeSet<String>,
        bool,
    )> {
        let transfer = transfer_pending_predicate();
        let in_use = agent_in_use();
        let source_monitor = monitor_source_in_use();
        let thread_agents = thread_agent_sql("c.thread_id");
        let account_in_use = format!(
            "({transfer} OR {source_monitor} OR EXISTS({ACCOUNT_AGENT_SQL} AND ({in_use})))"
        );
        let thread_in_use = format!(
            "({transfer} OR EXISTS({thread_agents} AND ({in_use})) \
             OR c.thread_id IN (SELECT thread_id FROM cleanup_threads))"
        );
        let sql = format!(
            "WITH candidates(thread_id) AS (SELECT value FROM json_each(?2)), \
             account_usage(in_use) AS MATERIALIZED (SELECT {account_in_use}), \
             cleanup_threads(thread_id) AS MATERIALIZED (SELECT source_cleanup_thread_id FROM runtime_agents \
               WHERE source_cleanup_phase IN ({ACTIVE_SOURCE_CLEANUP_PHASES}) AND source_cleanup_account_key=?1), \
             candidate_usage AS (SELECT c.thread_id, \
               CASE WHEN {thread_in_use} THEN 1 ELSE 0 END AS busy \
               FROM candidates c) \
             SELECT candidate_usage.thread_id,candidate_usage.busy,account_usage.in_use \
             FROM account_usage LEFT JOIN candidate_usage ON 1=1 \
             ORDER BY candidate_usage.thread_id"
        );
        let encoded_threads = studio_json::to_string(threads)?;
        let mut statement = connection.prepare_cached(&sql)?;
        let rows = statement.query_map([account, &encoded_threads], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<bool>>(1)?,
                row.get::<_, bool>(2)?,
            ))
        })?;
        let mut busy_threads = std::collections::BTreeSet::new();
        let mut account_in_use = false;
        for row in rows {
            let (thread, busy, busy_account) = row?;
            account_in_use = busy_account;
            if let (Some(thread), Some(true)) = (thread, busy) {
                busy_threads.insert(thread);
            }
        }
        Ok((
            busy_threads,
            Self::idle_unload_immediate_threads(connection, account, threads)?,
            account_in_use,
        ))
    }

    /// The members of `threads` that unload as soon as they are unused: every
    /// thread that no live lead and no live unfinished worker of `account`
    /// owns. These are the threads of finished managed workers (see
    /// `finished_worker_sql`) and of deleted agents, which nobody resumes
    /// soon; a lead or an unfinished worker keeps its thread for the idle
    /// setting.
    pub fn idle_unload_immediate_threads(
        connection: &Connection,
        account: &str,
        threads: &std::collections::BTreeSet<String>,
    ) -> StoreResult<std::collections::BTreeSet<String>> {
        let encoded_threads = studio_json::to_string(threads)?;
        let finished = finished_worker_sql();
        let thread_agents = thread_agent_sql("c.value");
        let mut statement = connection.prepare_cached(&format!(
            "SELECT c.value FROM json_each(?2) c WHERE NOT EXISTS({thread_agents} AND NOT ({finished}))"
        ))?;
        let rows = statement.query_map([account, &encoded_threads], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Whether `thread` on `account` must stay loaded: an agent that owns it
    /// is in use, a context repair still cleans it up, or a transfer touches
    /// the account. A thread no live agent owns holds nothing by itself.
    pub fn idle_unload_thread_in_use(
        connection: &Connection,
        account: &str,
        thread: &str,
    ) -> StoreResult<bool> {
        if transfer_pending(connection, account)? {
            return Ok(true);
        }
        let in_use = agent_in_use();
        Ok(exists(
            connection,
            &format!(
                "{ACCOUNT_AGENT_SQL} AND id IN (SELECT chat FROM sessions WHERE thread_id=?2 AND state<>'ended') AND ({in_use})"
            ),
            &[account, thread],
        )? || exists(
            connection,
            &format!(
                "SELECT 1 FROM runtime_agents WHERE \
                 source_cleanup_phase IN ({ACTIVE_SOURCE_CLEANUP_PHASES}) \
                 AND source_cleanup_account_key=?1 AND source_cleanup_thread_id=?2"
            ),
            &[account, thread],
        )?)
    }

    /// Whether `account` needs its provider process: any of its agents is in
    /// use (including input for an agent whose thread is not loaded yet), or a
    /// transfer touches the account.
    pub fn idle_unload_account_in_use(connection: &Connection, account: &str) -> StoreResult<bool> {
        if transfer_pending(connection, account)? {
            return Ok(true);
        }
        let source_monitor = monitor_source_in_use();
        if connection.query_row(&format!("SELECT {source_monitor}"), [account], |row| {
            row.get::<_, bool>(0)
        })? {
            return Ok(true);
        }
        let in_use = agent_in_use();
        exists(
            connection,
            &format!("{ACCOUNT_AGENT_SQL} AND ({in_use})"),
            &[account],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ACCOUNT_AGENT_SQL, SchedulerRepository, agent_in_use, finished_worker_sql,
        monitor_source_in_use, transfer_pending_predicate,
    };
    use rusqlite::Connection;
    use studio_json::{Value, json};

    thread_local! {
        static SCHEMA_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static SNAPSHOT_STEPS: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
    }

    #[test]
    fn idle_unload_snapshot_skips_unrelated_running_callbacks() {
        let connection = crate::test_canvas();
        let mut threads = std::collections::BTreeSet::new();
        for n in 0..18 {
            let thread = format!("thread-{n}");
            agent(
                &connection,
                &format!("idle-{n}"),
                json!({"threadId":thread,"isLead":true}),
            );
            threads.insert(thread);
        }
        for n in 0..700 {
            connection
                .execute(
                    "INSERT INTO runtime_tool_requests(id,record) VALUES(?1,?2)",
                    [
                        format!("callback-{n}"),
                        json!({"kind":"provider_callback","status":"running",
                    "payload":{"identity":{"context":{"agent":"unrelated"}}}})
                        .to_string(),
                    ],
                )
                .unwrap();
        }
        connection.trace_v2(
            rusqlite::trace::TraceEventCodes::SQLITE_TRACE_PROFILE,
            Some(|event| {
                if let rusqlite::trace::TraceEvent::Profile(statement, _) = event {
                    SNAPSHOT_STEPS.with(|count| {
                        count.set(
                            count.get() + statement.get_status(rusqlite::StatementStatus::VmStep),
                        );
                    });
                }
            }),
        );
        SNAPSHOT_STEPS.with(|count| count.set(0));
        let (busy, immediate, account_busy) =
            SchedulerRepository::idle_unload_usage_snapshot(&connection, "acct", &threads).unwrap();
        let steps = SNAPSHOT_STEPS.with(std::cell::Cell::get);
        eprintln!("18 idle agents/threads, 700 unrelated running callbacks: {steps} VM steps");
        assert!(busy.is_empty());
        assert!(immediate.is_empty());
        assert!(!account_busy);
        assert!(
            steps < 10_000,
            "idle checks must seek requests by status and agent: {steps} VM steps"
        );
    }

    #[test]
    fn idle_unload_snapshot_work_is_linear_in_loaded_threads() {
        let small = snapshot_steps(50);
        let large = snapshot_steps(100);
        assert!(
            large < 40_000,
            "100 loaded threads must use indexed owner lookups, got {large} VM steps"
        );
        assert!(
            large * 4 < small * 9,
            "doubling agents and threads must stay near linear: {small} -> {large}"
        );
    }

    fn snapshot_steps(agents: usize) -> i32 {
        let connection = database();
        let mut threads = std::collections::BTreeSet::new();
        for n in 0..agents {
            let thread = format!("thread-{n}");
            agent(
                &connection,
                &format!("agent-{n}"),
                json!({"threadId":thread,"isLead":true}),
            );
            threads.insert(thread);
        }
        connection.trace_v2(
            rusqlite::trace::TraceEventCodes::SQLITE_TRACE_PROFILE,
            Some(|event| {
                if let rusqlite::trace::TraceEvent::Profile(statement, _) = event {
                    SNAPSHOT_STEPS.with(|count| {
                        count.set(
                            count.get() + statement.get_status(rusqlite::StatementStatus::VmStep),
                        )
                    });
                }
            }),
        );
        SNAPSHOT_STEPS.with(|count| count.set(0));
        let start = std::time::Instant::now();
        let (busy, immediate, account_busy) =
            SchedulerRepository::idle_unload_usage_snapshot(&connection, "acct", &threads).unwrap();
        let steps = SNAPSHOT_STEPS.with(std::cell::Cell::get);
        eprintln!(
            "idle snapshot: agents={agents} threads={agents} steps={steps} elapsed={:?}",
            start.elapsed()
        );
        assert!(busy.is_empty());
        assert!(immediate.is_empty());
        assert!(!account_busy);
        steps
    }

    #[test]
    fn idle_unload_snapshot_does_not_read_schema() {
        let connection = crate::test_canvas();
        connection.trace_v2(
            rusqlite::trace::TraceEventCodes::SQLITE_TRACE_STMT,
            Some(|event| {
                if let rusqlite::trace::TraceEvent::Stmt(statement, _) = event {
                    let sql = statement.sql().to_ascii_lowercase();
                    if sql.contains("sqlite_master") || sql.contains("sqlite_schema") {
                        SCHEMA_READS.with(|count| count.set(count.get() + 1));
                    }
                }
            }),
        );
        SCHEMA_READS.with(|count| count.set(0));
        SchedulerRepository::idle_unload_usage_snapshot(&connection, "acct", &Default::default())
            .expect("idle snapshot");
        assert_eq!(SCHEMA_READS.with(std::cell::Cell::get), 0);
    }

    fn database() -> Connection {
        let mut connection = Connection::open_in_memory().expect("memory database opens");
        connection
            .execute_batch(crate::agent_columns::agent_table_sql())
            .expect("agents table creates");
        connection
            .execute_batch(
                "CREATE TABLE runtime_events(id TEXT PRIMARY KEY, agent TEXT, kind TEXT, text TEXT,
                   status TEXT, created REAL, epoch INTEGER, turn_id TEXT, error TEXT);
                 CREATE TABLE runtime_monitors(id TEXT PRIMARY KEY, record TEXT NOT NULL, monitor_status TEXT); CREATE INDEX runtime_monitor_agent_status ON runtime_monitors(json_extract(record,'$.agent'),json_extract(record,'$.status'));
                 CREATE TABLE runtime_work(id TEXT PRIMARY KEY, record TEXT NOT NULL); CREATE TABLE runtime_work_results(id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL); CREATE TABLE runtime_work_decisions(id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL); CREATE TABLE runtime_work_releases(id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL);
                 CREATE TABLE runtime_account_transfers(id TEXT PRIMARY KEY, record TEXT NOT NULL); CREATE TABLE runtime_tool_requests(id TEXT PRIMARY KEY, record TEXT NOT NULL, kind TEXT, status TEXT);",
            )
            .expect("schema creates");
        crate::approvals::create_schema(&connection).expect("approvals table creates");
        connection
            .execute_batch(crate::agent_volatile::agent_volatile_table_sql())
            .expect("volatile agent table creates");
        connection
            .execute_batch(&crate::domain::tables_sql())
            .expect("domain tables create");
        let (_, _, definition) = crate::schema::receipt_owner_columns()
            .into_iter()
            .find(|(table, name, _)| *table == "runtime_tool_requests" && *name == "agent")
            .expect("tool request agent owner");
        connection
            .execute_batch(&format!(
                "ALTER TABLE runtime_tool_requests ADD COLUMN agent {definition}"
            ))
            .unwrap();
        {
            let tx = connection.transaction().expect("migration transaction");
            crate::migrations::m2026_11_06_idle_unload::apply(&tx).expect("idle indexes");
            tx.commit().expect("migration commit");
        }
        connection
    }

    fn agent(connection: &Connection, id: &str, fields: Value) {
        let mut record = json!({"compactions":0,"id":id,"provider":"codex","rootId":id,"epoch":1,"status":"completed",
            "accountKey":"acct","threadId":"thread"});
        for (key, value) in fields.as_object().expect("fields").clone() {
            record[key] = value;
        }
        connection
            .execute(
                "INSERT OR REPLACE INTO runtime_agents(id,record) VALUES(?1,?2)",
                [id, &record.to_string()],
            )
            .expect("agent writes");
        crate::domain::project(connection, id, &record).expect("session follows the record");
    }

    fn thread_in_use(connection: &Connection) -> bool {
        SchedulerRepository::idle_unload_thread_in_use(connection, "acct", "thread").expect("reads")
    }

    fn account_in_use(connection: &Connection) -> bool {
        SchedulerRepository::idle_unload_account_in_use(connection, "acct").expect("reads")
    }

    #[test]
    fn idle_query_plans_use_their_indexes() {
        let db = database();
        db.execute_batch("CREATE INDEX runtime_work_owner_status ON runtime_work(json_extract(record,'$.owner'),json_extract(record,'$.status'));").unwrap();
        let plans = [
            (
                format!(
                    "SELECT 1 FROM runtime_agents INDEXED BY runtime_agent_idle_unload WHERE account_key=?1 AND deleted_at IS NULL AND ({})",
                    agent_in_use()
                ),
                vec![
                    "runtime_agent_idle_unload",
                    "runtime_monitor_idle_agent",
                    "runtime_event_idle_agent_status",
                ],
            ),
            (
                format!("{ACCOUNT_AGENT_SQL} AND NOT ({})", finished_worker_sql()),
                vec!["runtime_agent_idle_unload", "runtime_work_idle_owner"],
            ),
            (
                format!("SELECT {}", transfer_pending_predicate()),
                vec!["runtime_transfer_target_account_key"],
            ),
            (
                format!("SELECT {}", monitor_source_in_use()),
                vec!["runtime_monitor_idle_account"],
            ),
        ];
        for (sql, indexes) in plans {
            let mut statement = db.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let plan = statement
                .query_map(["acct"], |row| row.get::<_, String>(3))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
                .join("\n");
            eprintln!("{plan}");
            for index in indexes {
                assert!(
                    plan.lines()
                        .any(|line| line.contains("SEARCH") && line.contains(index)),
                    "missing indexed search {index}: {plan}"
                );
            }
        }
    }

    #[test]
    fn a_settled_agent_or_an_unowned_thread_releases_the_thread() {
        let connection = database();
        assert!(!thread_in_use(&connection), "no owner holds nothing");
        agent(&connection, "a", json!({}));
        assert!(!thread_in_use(&connection));
        assert!(!account_in_use(&connection));
        agent(
            &connection,
            "a",
            json!({"status":"running","deletedAt":1.0}),
        );
        assert!(!thread_in_use(&connection), "a deleted owner holds nothing");
    }

    #[test]
    fn a_transferred_monitor_keeps_its_source_provider_until_its_real_exit() {
        let connection = database();
        crate::StartupRecoveryRepository::prepare_indexes(&connection)
            .expect("active monitor index");
        let sql = format!("EXPLAIN QUERY PLAN SELECT {}", monitor_source_in_use());
        let plan = connection
            .prepare(&sql)
            .expect("prepare source plan")
            .query_map(["acct"], |row| row.get::<_, String>(3))
            .expect("query source plan")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect source plan")
            .join("\n");
        assert!(
            plan.contains("USING INDEX runtime_monitor_idle_account"),
            "{plan}"
        );
        agent(&connection, "a", json!({"accountKey":"target"}));
        connection.execute("INSERT INTO runtime_monitors(id,record,monitor_status) VALUES('watch',?1,'running')",
            [json!({"id":"watch","agent":"a","epoch":1,"status":"running","operation":{"agent":"a","epoch":1,"accountKey":"acct","connectionId":"original"}}).to_string()]).expect("running source command");
        assert!(
            account_in_use(&connection),
            "the command still needs its original provider"
        );
        assert!(
            !thread_in_use(&connection),
            "the process does not keep every old model thread loaded"
        );
        let (_, _, busy) = SchedulerRepository::idle_unload_usage_snapshot(
            &connection,
            "acct",
            &Default::default(),
        )
        .expect("batched source ownership");
        assert!(busy, "the batched idle decision uses the same source owner");
        connection.execute("UPDATE runtime_monitors SET record=json_set(record,'$.status','completed'),monitor_status='completed' WHERE id='watch'",[]).expect("actual command exit");
        assert!(
            !account_in_use(&connection),
            "the real exit releases its source"
        );
    }

    #[test]
    fn a_manual_reset_waits_for_its_own_closure_without_an_idle_unsubscribe() {
        let connection = database();
        agent(
            &connection,
            "a",
            json!({"nativeRelease":{"phase":"released","resetPending":true}}),
        );
        assert!(thread_in_use(&connection));
        assert!(account_in_use(&connection));
        agent(
            &connection,
            "a",
            json!({"nativeRelease":{"phase":"released","resetPending":false,"closedAt":1.0}}),
        );
        assert!(!thread_in_use(&connection));
    }

    #[test]
    fn a_stale_turn_id_without_an_unfinished_turn_releases_the_thread() {
        let connection = database();
        agent(
            &connection,
            "a",
            json!({"status":"completed","turnId":"turn","inFlight":false}),
        );
        assert!(!thread_in_use(&connection), "only an unfinished turn holds");
        agent(
            &connection,
            "a",
            json!({"status":"running","turnId":"turn","inFlight":true}),
        );
        assert!(thread_in_use(&connection), "the unfinished turn holds");
    }

    #[test]
    fn every_in_use_condition_holds_the_thread_and_the_account() {
        let cases = [
            json!({"status":"running"}),
            json!({"status":"queued"}),
            json!({"status":"approval"}),
            json!({"inFlight":true}),
            json!({"activeTools":[{"id":"tool"}]}),
            json!({"workspaceOperation":"merge"}),
            json!({"accountTransferId":"transfer"}),
            json!({"nativeToolRefreshId":"refresh"}),
            json!({"contextRepair":{"phase":"submitted"}}),
            json!({"startAttempt":{"submitted":true}}),
        ];
        for fields in cases {
            let connection = database();
            agent(&connection, "a", fields.clone());
            assert!(thread_in_use(&connection), "{fields} holds the thread");
            assert!(account_in_use(&connection), "{fields} holds the account");
            let (busy, immediate, account_busy) = SchedulerRepository::idle_unload_usage_snapshot(
                &connection,
                "acct",
                &std::collections::BTreeSet::from(["thread".into(), "other".into()]),
            )
            .unwrap();
            assert_eq!(
                busy,
                std::collections::BTreeSet::from(["thread".into()]),
                "snapshot parity: {fields}"
            );
            assert!(account_busy, "snapshot account parity: {fields}");
            assert!(immediate.contains("other"));
            assert!(
                !SchedulerRepository::idle_unload_thread_in_use(&connection, "acct", "other")
                    .expect("reads"),
                "{fields} holds only its own thread"
            );
        }
    }

    #[test]
    fn unsettled_input_requests_and_monitors_hold_the_thread() {
        for status in ["pending", "reserved", "dispatching", "uncertain"] {
            let connection = database();
            agent(&connection, "a", json!({}));
            connection
                .execute(
                    "INSERT INTO runtime_events(id,agent,kind,text,status,created,epoch) \
                     VALUES('e','a','user','hi',?1,0,1)",
                    [status],
                )
                .expect("event writes");
            assert!(
                thread_in_use(&connection),
                "{status} input holds the thread"
            );
        }
        let connection = database();
        agent(&connection, "a", json!({}));
        connection
            .execute(
                "INSERT INTO runtime_events(id,agent,kind,text,status,created,epoch) \
                 VALUES('e','a','user','hi','done',0,1)",
                [],
            )
            .expect("event writes");
        assert!(!thread_in_use(&connection), "settled input holds nothing");
        connection
            .execute(
                "INSERT INTO runtime_monitors(id,record,monitor_status) VALUES('m',?1,'running')",
                [json!({"id":"m","agent":"a","status":"running"}).to_string()],
            )
            .expect("monitor writes");
        assert!(
            thread_in_use(&connection),
            "a running monitor holds the thread"
        );
        connection
            .execute("DELETE FROM runtime_monitors", [])
            .expect("monitor deletes");
        {
            use studio_runtime_schema::approvals as typed;
            let transaction = connection
                .unchecked_transaction()
                .expect("fixture transaction");
            crate::approvals::create(
                &transaction,
                crate::approvals::NewApproval {
                    id: typed::ApprovalId("r".into()),
                    agent: typed::AgentId("a".into()),
                    turn: Some(typed::TurnId("t".into())),
                    epoch: None,
                    request: typed::Request::Exec {
                        command: String::new(),
                        cwd: String::new(),
                        reason: None,
                    },
                    delivery: None,
                    detail: typed::Detail {
                        params: typed::ProviderObject(Default::default()),
                        preview: None,
                        answer_error: None,
                        host_close_reason: None,
                    },
                    fire_at: None,
                },
                typed::Timestamp(1.0),
            )
            .expect("request writes");
            transaction.commit().expect("fixture commit");
        }
        assert!(
            thread_in_use(&connection),
            "a pending request holds the thread"
        );
    }

    #[test]
    fn input_for_an_agent_without_a_loaded_thread_holds_only_the_account() {
        let connection = database();
        agent(
            &connection,
            "new",
            json!({"threadId":null,"status":"queued"}),
        );
        assert!(!thread_in_use(&connection));
        assert!(account_in_use(&connection));
    }

    #[test]
    fn a_pending_transfer_or_source_cleanup_holds_the_thread() {
        let connection = database();
        connection
            .execute(
                "INSERT INTO runtime_account_transfers(id,record) VALUES('t',?1)",
                [
                    json!({"id":"t","status":"pending","targetAccountKey":"other",
                    "members":{"a":{"sourceAccountKey":"acct"}}})
                    .to_string(),
                ],
            )
            .expect("transfer writes");
        assert!(thread_in_use(&connection));
        assert!(account_in_use(&connection));
        connection
            .execute(
                "UPDATE runtime_account_transfers SET record=json_set(record,'$.status','completed')",
                [],
            )
            .expect("transfer settles");
        assert!(!thread_in_use(&connection));
        agent(
            &connection,
            "repair",
            json!({"threadId":"new","contextRepair":{"sourceCleanup":
                {"phase":"planned","accountKey":"acct","threadId":"thread"}}}),
        );
        assert!(
            thread_in_use(&connection),
            "source cleanup still owns the thread"
        );
        for phase in ["planned", "submitted", "completed"] {
            for account in ["acct", "another-account"] {
                agent(
                    &connection,
                    "repair",
                    json!({"threadId":"new","deletedAt":1.0,
                    "contextRepair":{"sourceCleanup":{"phase":phase,"accountKey":account,"threadId":"thread"}}}),
                );
                let (busy, _, account_busy) = SchedulerRepository::idle_unload_usage_snapshot(
                    &connection,
                    "acct",
                    &std::collections::BTreeSet::from(["thread".into(), "cold".into()]),
                )
                .unwrap();
                assert_eq!(busy.contains("thread"), thread_in_use(&connection));
                assert!(!busy.contains("cold"));
                assert_eq!(account_busy, account_in_use(&connection));
            }
        }
    }

    #[test]
    fn one_usage_snapshot_checks_only_the_supplied_threads_and_their_account() {
        let connection = database();
        agent(&connection, "busy", json!({"status":"running"}));
        agent(
            &connection,
            "other-thread",
            json!({"threadId":"other","status":"running"}),
        );
        let candidates = std::collections::BTreeSet::from(["thread".to_owned(), "cold".to_owned()]);
        let (busy, _, account_busy) =
            SchedulerRepository::idle_unload_usage_snapshot(&connection, "acct", &candidates)
                .expect("usage snapshot reads");
        assert_eq!(
            busy,
            std::collections::BTreeSet::from(["thread".to_owned()])
        );
        assert!(
            account_busy,
            "another thread keeps the account process in use"
        );

        agent(&connection, "busy", json!({}));
        agent(&connection, "other-thread", json!({"threadId":"other"}));
        let (busy, _, account_busy) =
            SchedulerRepository::idle_unload_usage_snapshot(&connection, "acct", &candidates)
                .expect("usage snapshot rereads authoritative state");
        assert!(busy.is_empty());
        assert!(!account_busy);
    }

    #[test]
    fn only_a_live_lead_or_unfinished_worker_keeps_its_thread_for_the_idle_setting() {
        let connection = database();
        let threads = std::collections::BTreeSet::from(["thread".to_owned(), "cold".to_owned()]);
        let immediate = || {
            SchedulerRepository::idle_unload_immediate_threads(&connection, "acct", &threads)
                .expect("immediate threads read")
        };
        let all = threads.clone();
        let cold = std::collections::BTreeSet::from(["cold".to_owned()]);
        agent(&connection, "chat", json!({"isLead":true}));
        assert_eq!(immediate(), cold, "a lead or ordinary chat waits");
        let (_, snapshot_immediate, _) =
            SchedulerRepository::idle_unload_usage_snapshot(&connection, "acct", &threads)
                .expect("usage snapshot reads");
        assert_eq!(snapshot_immediate, immediate());
        agent(&connection, "chat", json!({"isLead":false}));
        assert_eq!(
            immediate(),
            all,
            "a finished managed worker unloads at once"
        );
        agent(
            &connection,
            "chat",
            json!({"isLead":false,"status":"waiting"}),
        );
        assert_eq!(immediate(), cold, "a waiting worker keeps the idle timer");
        agent(&connection, "chat", json!({"isLead":false}));
        for status in ["review", "ready", "running", "blocked"] {
            connection
                .execute(
                    "INSERT OR REPLACE INTO runtime_work(id,record) VALUES('w',?1)",
                    [json!({"id":"w","owner":"chat","status":status}).to_string()],
                )
                .expect("work writes");
            assert_eq!(immediate(), cold, "{status} work keeps the idle timer");
        }
        connection
            .execute(
                "UPDATE runtime_work SET record=json_set(record,'$.status','accepted')",
                [],
            )
            .expect("work settles");
        assert_eq!(immediate(), all, "accepted work finishes the worker");
        agent(
            &connection,
            "chat",
            json!({"isLead":false,"status":"waiting","deletedAt":1.0}),
        );
        assert_eq!(
            immediate(),
            all,
            "a deleted worker's thread unloads at once"
        );
        agent(&connection, "chat", json!({"isLead":true,"deletedAt":1.0}));
        assert_eq!(immediate(), all, "a deleted lead's thread unloads at once");
        agent(
            &connection,
            "chat",
            json!({"isLead":true,"accountKey":"other"}),
        );
        assert_eq!(
            immediate(),
            all,
            "another account's lead holds nothing here"
        );
    }
}
