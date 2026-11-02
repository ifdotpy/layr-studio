//! Indexed reads of `runtime_agents` for hot writer-transaction paths. Each
//! query is written to match an index on the agent columns of
//! [`crate::agent_columns`] so that no caller reads or parses the whole table
//! (tombstones and finished agents dominate it). This module owns those
//! indexes; the query-plan test below fails when a query stops using its index.

use std::sync::LazyLock;

use rusqlite::{Connection, params};

use crate::StoreResult;
use crate::domain::{
    ACTIVE_STATUS_INDEX, CAPACITY_STATUS_INDEX, CHAT_ACTIVE_STATUSES, CHAT_CAPACITY_STATUSES,
    unfinished_turn_sql,
};

/// A deleted agent (tombstone) never occupies a slot. Together with the chat
/// capacity statuses ([`CHAT_CAPACITY_STATUSES`]) this is the one owner of every
/// slot query. A deleted agent keeps its status, so the live check stays.
const CAPACITY_LIVE: &str = "deleted_at IS NULL";
const REMOTE_WORKER_KEY: &str = "COALESCE(remote_worker,0)";
/// Agents by team root, in rowid order within a root.
pub const ROOT_INDEX: &str = "runtime_agent_root_v2";
/// Live (not deleted) agents by team root.
pub const LIVE_ROOT_INDEX: &str = "runtime_agent_live_root_v2";
/// Agents by account key, live agents (`deleted_at IS NULL`) first.
pub const ACCOUNT_INDEX: &str = "runtime_agent_account";
/// Agents by provider thread within an account.
/// Retired agent index on the `status` column, replaced by
/// [`ACTIVE_STATUS_INDEX`]. The 2026-10-31 migration still drops it by this name.
pub const ACTIVE_INDEX: &str = "runtime_agent_active";
/// SQL listing `(id, root_id)` of the agents that hold a running or reserved
/// turn: an active chat status, an unfinished turn of the chat, or a start
/// reservation whose input events are still reserved. It reads
/// [`ACTIVE_STATUS_INDEX`] and `turns`.
pub fn reserved_turn_input() -> &'static str {
    static SQL: LazyLock<String> = LazyLock::new(|| {
        format!(
            "SELECT a.id,a.root_id FROM chats INDEXED BY {ACTIVE_STATUS_INDEX} \
               JOIN runtime_agents a ON a.id=chats.id \
               WHERE chats.status IN {CHAT_ACTIVE_STATUSES} \
             UNION SELECT a.id,a.root_id FROM turns t JOIN runtime_agents a ON a.id=t.chat \
               WHERE {unfinished} \
             UNION SELECT a.id,a.root_id FROM turns t JOIN runtime_agents a ON a.id=t.chat \
               WHERE t.attempt_active_at_reservation=1 AND EXISTS (\
               SELECT 1 FROM json_each(a.record,'$.startAttempt.events') AS input \
               JOIN runtime_events AS event ON event.id=input.value \
               WHERE event.agent=a.id \
               AND event.epoch=json_extract(a.record,'$.startAttempt.epoch') \
               AND event.status IN ('reserved','dispatching','uncertain')) ORDER BY 1",
            unfinished = unfinished_turn_sql("t."),
        )
    });
    &SQL
}
/// Agents holding a workspace operation, by working directory.
pub const WORKSPACE_OPERATION_INDEX: &str = "runtime_agent_workspace_operation";
/// SQL predicate: the agent row holds a workspace operation. One owner for
/// the index predicate and its queries.
macro_rules! holds_workspace_operation {
    () => {
        "typeof(workspace_operation)='text' AND workspace_operation<>''"
    };
}

/// Indexed reads of agent records by team root or by slot-occupying status.
pub struct AgentIndexedReads;

impl AgentIndexedReads {
    /// Canonical DDL of the indexes this module owns (Rust-owned).
    #[doc(hidden)]
    pub fn index_definitions() -> Vec<crate::OwnedRuntimeIndexDefinition> {
        [
            (ROOT_INDEX, "root_id)".to_owned()),
            (ACCOUNT_INDEX, "account_key,deleted_at)".to_owned()),
            (
                LIVE_ROOT_INDEX,
                "root_id,id) WHERE deleted_at IS NULL".to_owned(),
            ),
            (
                WORKSPACE_OPERATION_INDEX,
                concat!("cwd) WHERE ", holds_workspace_operation!()).to_owned(),
            ),
        ]
        .into_iter()
        .map(|(name, rest)| crate::OwnedRuntimeIndexDefinition {
            name,
            table: "runtime_agents",
            create_sql: format!("CREATE INDEX IF NOT EXISTS {name} ON runtime_agents({rest}"),
        })
        .collect()
    }

    #[doc(hidden)]
    pub fn prepare_index(connection: &Connection) -> StoreResult<()> {
        if crate::schema::table_exists(connection, "runtime_agents")? {
            for definition in Self::index_definitions() {
                connection.execute_batch(&definition.create_sql)?;
            }
        }
        Ok(())
    }

    /// SQL listing `(id, record)` of every agent of one team root, in rowid order.
    #[doc(hidden)]
    pub const TEAM_RECORDS_SQL: &'static str = "SELECT id,record FROM runtime_agents \
         WHERE root_id=?1 ORDER BY rowid";

    /// `(id, record)` of every agent whose `rootId` is `root_id`, each record
    /// with its volatile fields.
    pub fn team_records(
        connection: &Connection,
        root_id: &str,
    ) -> StoreResult<Vec<(String, String)>> {
        let mut statement = connection.prepare_cached(Self::TEAM_RECORDS_SQL)?;
        let rows = statement
            .query_map([root_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<(String, String)>, _>>()?;
        crate::agent_volatile::merged_rows(connection, rows)
    }

    /// Unmerged records for named agents and, for broadcasts, one team root.
    pub const MESSAGE_RECORDS_SQL: &'static str = "SELECT id,record FROM runtime_agents WHERE id IN (?1,?2,?3,?4) OR root_id=?5 ORDER BY rowid";
    pub const MESSAGE_PREFIX_SQL: &'static str = "SELECT id,record FROM runtime_agents WHERE id IN (?1,?2,?3,?4) OR root_id=?5 OR (id>=?6 AND id<?7) ORDER BY rowid";
    const MESSAGE_FINAL_PREFIX_SQL: &'static str = "SELECT id,record FROM runtime_agents WHERE id IN (?1,?2,?3,?4) OR root_id=?5 OR id GLOB ?6 ORDER BY rowid";

    pub fn message_exact_team(connection: &Connection, id: &str, root: &str) -> StoreResult<bool> {
        Ok(connection.query_row("SELECT EXISTS(SELECT 1 FROM runtime_agents WHERE id=?1 AND root_id=?2 AND (deleted_at IS NULL OR deleted_at=0))", params![id, root], |row| row.get(0))?)
    }

    pub fn message_records(
        connection: &Connection,
        ids: [Option<&str>; 4],
        root: Option<&str>,
        prefix: Option<&str>,
    ) -> StoreResult<Vec<(String, String)>> {
        let upper = prefix.and_then(prefix_upper);
        let final_pattern = prefix
            .filter(|_| upper.is_none())
            .map(|prefix| format!("{prefix}*"));
        let mut parameters = ids.into_iter().chain([root]).collect::<Vec<_>>();
        let sql = match (prefix, upper.as_deref()) {
            (Some(prefix), Some(upper)) => {
                parameters.extend([Some(prefix), Some(upper)]);
                Self::MESSAGE_PREFIX_SQL
            }
            (Some(_), None) => {
                parameters.push(final_pattern.as_deref());
                Self::MESSAGE_FINAL_PREFIX_SQL
            }
            (None, _) => Self::MESSAGE_RECORDS_SQL,
        };
        let mut statement = connection.prepare_cached(sql)?;
        Ok(statement
            .query_map(rusqlite::params_from_iter(parameters), |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<Result<_, _>>()?)
    }

    /// SQL counting the live (not deleted) team members that are not finished.
    /// A finished member is completed, failed, interrupted, or paused without
    /// auto-wake. The chat row holds the status and auto-wake; a missing row
    /// counts as no status, as an absent agent status did.
    #[doc(hidden)]
    pub const ACTIVE_TEAM_MEMBERS_SQL: &'static str = "SELECT COUNT(*) FROM runtime_agents INDEXED BY runtime_agent_live_root_v2 \
         LEFT JOIN chats ON chats.id=runtime_agents.id \
         WHERE root_id=?1 AND deleted_at IS NULL \
         AND NOT (COALESCE(chats.status,'') IN ('completed','failed','interrupted') \
         OR (chats.status IS 'paused' AND chats.auto_wake IS 0))";

    pub fn active_team_members(connection: &Connection, root_id: &str) -> StoreResult<i64> {
        Ok(connection
            .prepare_cached(Self::ACTIVE_TEAM_MEMBERS_SQL)?
            .query_row([root_id], |row| row.get(0))?)
    }

    /// SQL for the ids of agents holding a workspace operation (same index).
    #[doc(hidden)]
    pub const WORKSPACE_OPERATION_IDS_SQL: &'static str = concat!(
        "SELECT id FROM runtime_agents WHERE ",
        holds_workspace_operation!()
    );

    /// SQL for the `id,record` rows of agents holding a workspace operation, in
    /// index order (same index).
    #[doc(hidden)]
    pub const WORKSPACE_OPERATION_RECORDS_SQL: &'static str = concat!(
        "SELECT id,record FROM runtime_agents WHERE ",
        holds_workspace_operation!()
    );

    /// SQL for the distinct working directories of live agents and of any agent
    /// (live or not) holding a workspace operation: the directories a message
    /// admission may have to compare. Tombstones cannot start an operation.
    #[doc(hidden)]
    pub const ADMISSION_CWDS_SQL: &'static str = concat!(
        "SELECT cwd FROM runtime_agents WHERE deleted_at IS NULL \
         UNION SELECT cwd FROM runtime_agents WHERE ",
        holds_workspace_operation!()
    );

    pub fn admission_cwds(connection: &Connection) -> StoreResult<Vec<Option<String>>> {
        Ok(connection
            .prepare_cached(Self::ADMISSION_CWDS_SQL)?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    /// Slot queries drive from the chat capacity index: the chat row holds the
    /// status, the agent row holds the tombstone, root and remote fields.
    #[doc(hidden)]
    pub fn global_slots_sql() -> String {
        format!(
            "SELECT COUNT(*) FROM chats INDEXED BY {CAPACITY_STATUS_INDEX} \
             JOIN runtime_agents a ON a.id=chats.id \
             WHERE chats.id<>?1 AND chats.status IN {CHAT_CAPACITY_STATUSES} \
             AND {CAPACITY_LIVE} AND {REMOTE_WORKER_KEY}=0"
        )
    }

    #[doc(hidden)]
    pub fn team_slots_sql() -> String {
        format!(
            "SELECT COUNT(*) FROM chats INDEXED BY {CAPACITY_STATUS_INDEX} \
             JOIN runtime_agents a ON a.id=chats.id \
             WHERE chats.id<>?1 AND chats.id<>?2 \
             AND a.root_id=?2 \
             AND chats.status IN {CHAT_CAPACITY_STATUSES} AND {CAPACITY_LIVE} AND {REMOTE_WORKER_KEY}=0"
        )
    }

    /// SQL counting the working local agents per account key: live
    /// slot-occupying agents that are not paused. A live paused agent keeps its
    /// slot but sends no provider request, so it must not skew where new workers
    /// are placed (intended; owner semantic). It enumerates the chat capacity
    /// index.
    #[doc(hidden)]
    pub fn account_slots_sql() -> String {
        format!(
            "SELECT a.account_key,COUNT(*) \
             FROM chats INDEXED BY {CAPACITY_STATUS_INDEX} \
             JOIN runtime_agents a ON a.id=chats.id \
             WHERE chats.status IN {CHAT_CAPACITY_STATUSES} \
             AND {CAPACITY_LIVE} AND chats.status<>'paused' \
             AND {REMOTE_WORKER_KEY}=0 GROUP BY 1"
        )
    }

    /// Working (not paused) local agents per account key; idle accounts are absent.
    pub fn slots_by_account(
        connection: &Connection,
    ) -> StoreResult<std::collections::HashMap<String, i64>> {
        Ok(connection
            .prepare_cached(&Self::account_slots_sql())?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?)
    }

    /// Slot-occupying local agents other than `agent_id`, across all teams.
    pub fn global_slots_in_use(connection: &Connection, agent_id: &str) -> StoreResult<i64> {
        Ok(connection
            .prepare_cached(&Self::global_slots_sql())?
            .query_row([agent_id], |row| row.get(0))?)
    }

    /// Slot-occupying local agents of team `root_id` other than `agent_id` and the root.
    pub fn team_slots_in_use(
        connection: &Connection,
        agent_id: &str,
        root_id: &str,
    ) -> StoreResult<i64> {
        Ok(connection
            .prepare_cached(&Self::team_slots_sql())?
            .query_row(params![agent_id, root_id], |row| row.get(0))?)
    }
}

fn prefix_upper(prefix: &str) -> Option<String> {
    for (offset, ch) in prefix.char_indices().rev() {
        let next = (ch as u32).checked_add(1)?;
        if let Some(next) = char::from_u32(if next == 0xd800 { 0xe000 } else { next }) {
            let mut upper = prefix[..offset].to_owned();
            upper.push(next);
            return Some(upper);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ACTIVE_STATUS_INDEX, CAPACITY_STATUS_INDEX, ONE_UNFINISHED_TURN_INDEX};
    use serde_json::{Value, json};

    /// The agents table of the column format with this module's indexes.
    fn database() -> Connection {
        let connection = Connection::open_in_memory().expect("memory SQLite opens");
        connection
            .execute_batch(crate::agent_columns::agent_table_sql())
            .expect("agents table creates");
        connection
            .execute_batch(crate::agent_volatile::agent_volatile_table_sql())
            .expect("volatile agent table creates");
        connection
            .execute_batch(&crate::domain::tables_sql())
            .expect("domain tables create");
        // The reservation branch of the reserved turn query reads the event table.
        connection
            .execute_batch(
                "CREATE TABLE runtime_events(id TEXT PRIMARY KEY, agent TEXT, epoch INTEGER, status TEXT)",
            )
            .expect("event table creates");
        AgentIndexedReads::prepare_index(&connection).expect("indexes create");
        connection
    }

    fn insert(connection: &Connection, record: Value) {
        connection
            .execute(
                "INSERT INTO runtime_agents(id,record) VALUES(?1,?2)",
                params![record["id"].as_str().expect("id"), record.to_string()],
            )
            .expect("agent inserts");
        if record.get("status").is_some() {
            crate::domain::project(connection, record["id"].as_str().expect("id"), &record)
                .expect("chat follows the record");
        }
    }

    fn plan(connection: &Connection, sql: &str, parameters: &[&str]) -> String {
        let mut statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .expect("plan prepares");
        statement
            .query_map(rusqlite::params_from_iter(parameters), |row| {
                row.get::<_, String>(3)
            })
            .expect("plan runs")
            .collect::<Result<Vec<_>, _>>()
            .expect("plan rows")
            .join(" | ")
    }

    fn fixture() -> Connection {
        let connection = database();
        let agents = [
            json!({"id":"lead","provider":"codex","rootId":"lead","status":"running","cwd":"/w"}),
            json!({"id":"a","provider":"codex","rootId":"lead","status":"waiting","cwd":"/w"}),
            json!({"id":"paused-wake","provider":"codex","rootId":"lead","status":"paused","autoWake":true}),
            json!({"id":"paused-off","provider":"codex","rootId":"lead","status":"paused","autoWake":false}),
            json!({"id":"done","provider":"codex","rootId":"lead","status":"completed"}),
            json!({"id":"failed","provider":"codex","rootId":"lead","status":"failed"}),
            json!({"id":"gone","provider":"codex","rootId":"lead","status":"running","deletedAt":5}),
            json!({"id":"tombstone","provider":"codex","rootId":"lead","status":"paused","deletedAt":6}),
            json!({"id":"no-status","provider":"codex","rootId":"lead"}),
            json!({"id":"remote","provider":"codex","rootId":"lead","status":"running","remoteWorker":true}),
            json!({"id":"op","provider":"codex","rootId":"other","status":"running","cwd":"/x","workspaceOperation":"branch"}),
            json!({"id":"op-empty","provider":"codex","rootId":"other","status":"completed","cwd":"/y","workspaceOperation":""}),
            json!({"id":"other-idle","provider":"codex","rootId":"other","status":"paused","cwd":"/z"}),
        ];
        for agent in agents {
            insert(&connection, agent);
        }
        connection
    }

    #[test]
    fn reserved_turn_input_finds_a_busy_chat_through_its_unfinished_turn() {
        let connection = database();
        insert(
            &connection,
            json!({"id":"busy","provider":"codex","rootId":"busy","status":"paused"}),
        );
        connection
            .execute(
                "INSERT INTO turns(id,chat,state) VALUES('t','busy','starting')",
                [],
            )
            .expect("turn inserts");
        let rows = connection
            .prepare(reserved_turn_input())
            .expect("query prepares")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query runs")
            .collect::<Result<Vec<_>, _>>()
            .expect("rows");
        assert_eq!(rows, ["busy"]);
    }

    #[test]
    fn reserved_turn_input_finds_an_active_chat_by_its_status() {
        let connection = database();
        insert(
            &connection,
            json!({"id":"running-chat","provider":"codex","rootId":"running-chat","status":"running"}),
        );
        insert(
            &connection,
            json!({"id":"paused-chat","provider":"codex","rootId":"paused-chat","status":"paused"}),
        );
        let rows = connection
            .prepare(reserved_turn_input())
            .expect("query prepares")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query runs")
            .collect::<Result<Vec<_>, _>>()
            .expect("rows");
        assert_eq!(rows, ["running-chat"]);
    }

    #[test]
    fn reserved_turn_input_uses_the_chat_status_index_and_the_unfinished_turn_index() {
        let connection = database();
        let details = plan(&connection, reserved_turn_input(), &[]);
        assert!(details.contains(ACTIVE_STATUS_INDEX), "{details}");
        assert!(details.contains(ONE_UNFINISHED_TURN_INDEX), "{details}");
    }

    #[test]
    fn indexed_reads_equal_the_full_scan_semantics() {
        let connection = fixture();
        let ids =
            |rows: Vec<(String, String)>| rows.into_iter().map(|row| row.0).collect::<Vec<_>>();
        assert_eq!(
            ids(AgentIndexedReads::team_records(&connection, "lead").expect("team")),
            [
                "lead",
                "a",
                "paused-wake",
                "paused-off",
                "done",
                "failed",
                "gone",
                "tombstone",
                "no-status",
                "remote"
            ]
        );
        // lead, a, paused-wake, no-status, remote; tombstone and finished agents excluded.
        assert_eq!(
            AgentIndexedReads::active_team_members(&connection, "lead").expect("count"),
            5
        );
        assert_eq!(
            AgentIndexedReads::active_team_members(&connection, "other").expect("count"),
            2
        );
        assert_eq!(
            AgentIndexedReads::admission_cwds(&connection).expect("cwds"),
            [
                None,
                Some("/w".to_owned()),
                Some("/x".to_owned()),
                Some("/y".to_owned()),
                Some("/z".to_owned())
            ]
        );
        // Every live status-occupying local agent but the caller; tombstones never count.
        let old_global: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM runtime_agents NOT INDEXED WHERE id<>'a' \
                 AND json_extract(record,'$.status') \
                 IN ('starting','running','waiting','paused','approval') \
                 AND COALESCE(json_extract(record,'$.remoteWorker'),0)=0 \
                 AND json_extract(record,'$.deletedAt') IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("reference count");
        assert_eq!(old_global, 5);
        assert_eq!(
            AgentIndexedReads::global_slots_in_use(&connection, "a").expect("global"),
            old_global
        );
        assert_eq!(
            AgentIndexedReads::team_slots_in_use(&connection, "a", "lead").expect("team"),
            2
        );
        insert(
            &connection,
            json!({"id":"on-b","provider":"codex","rootId":"o","status":"running","accountKey":"b"}),
        );
        let by_account = AgentIndexedReads::slots_by_account(&connection).expect("by account");
        assert_eq!(by_account.len(), 2);
        // Live paused agents hold slots but are not counted as work; tombstones are not counted.
        assert_eq!(by_account["default"], 3);
        assert_eq!(by_account["b"], 1);
    }

    #[test]
    fn message_records_use_both_indexes_and_keep_row_order() {
        let connection = fixture();
        let ids =
            |rows: Vec<(String, String)>| rows.into_iter().map(|row| row.0).collect::<Vec<_>>();
        assert_eq!(
            ids(AgentIndexedReads::message_records(
                &connection,
                [Some("a"), Some("lead"), None, Some("op")],
                None,
                None,
            )
            .unwrap()),
            ["lead", "a", "op"]
        );
        let broadcast = ids(AgentIndexedReads::message_records(
            &connection,
            [Some("lead"), None, None, Some("op")],
            Some("lead"),
            None,
        )
        .unwrap());
        assert_eq!(broadcast.len(), 11);
        assert_eq!(broadcast.last().map(String::as_str), Some("op"));
        let plan_description = plan(
            &connection,
            AgentIndexedReads::MESSAGE_RECORDS_SQL,
            &["a", "lead", "", "op", "lead"],
        );
        assert!(
            plan_description.contains("runtime_agent_root_v2"),
            "{plan_description}"
        );
        assert!(
            !plan_description.contains("SCAN runtime_agents"),
            "{plan_description}"
        );
        let prefix =
            ids(
                AgentIndexedReads::message_records(&connection, [None; 4], None, Some("paused-"))
                    .unwrap(),
            );
        assert_eq!(prefix, ["paused-wake", "paused-off"]);
        let prefix_plan = plan(
            &connection,
            AgentIndexedReads::MESSAGE_PREFIX_SQL,
            &["", "", "", "", "", "paused-", "paused."],
        );
        assert!(
            !prefix_plan.contains("SCAN runtime_agents"),
            "{prefix_plan}"
        );
        assert_eq!(prefix_upper("a\u{10ffff}"), Some("b".into()));
        assert_eq!(prefix_upper("\u{d7ff}"), Some("\u{e000}".into()));
        assert_eq!(prefix_upper("\u{10ffff}"), None);
        let final_plan = plan(
            &connection,
            AgentIndexedReads::MESSAGE_FINAL_PREFIX_SQL,
            &["", "", "", "", "", "\u{10ffff}*"],
        );
        assert!(!final_plan.contains("SCAN runtime_agents"), "{final_plan}");
    }

    #[test]
    fn queries_use_their_indexes_instead_of_scanning_the_table() {
        let connection = fixture();
        for (sql, parameters, index) in [
            (
                AgentIndexedReads::TEAM_RECORDS_SQL.to_owned(),
                vec!["lead"],
                ROOT_INDEX,
            ),
            (
                AgentIndexedReads::ACTIVE_TEAM_MEMBERS_SQL.to_owned(),
                vec!["lead"],
                LIVE_ROOT_INDEX,
            ),
            (
                AgentIndexedReads::ADMISSION_CWDS_SQL.to_owned(),
                vec![],
                LIVE_ROOT_INDEX,
            ),
            (
                AgentIndexedReads::WORKSPACE_OPERATION_IDS_SQL.to_owned(),
                vec![],
                WORKSPACE_OPERATION_INDEX,
            ),
            (
                AgentIndexedReads::WORKSPACE_OPERATION_RECORDS_SQL.to_owned(),
                vec![],
                WORKSPACE_OPERATION_INDEX,
            ),
            (
                AgentIndexedReads::global_slots_sql(),
                vec!["a"],
                CAPACITY_STATUS_INDEX,
            ),
            (
                AgentIndexedReads::account_slots_sql(),
                vec![],
                CAPACITY_STATUS_INDEX,
            ),
            (
                AgentIndexedReads::team_slots_sql(),
                vec!["a", "lead"],
                CAPACITY_STATUS_INDEX,
            ),
        ] {
            let details = plan(&connection, &sql, &parameters);
            assert!(details.contains(index), "{sql}: {details}");
            // A scan is acceptable only through an index (the partial capacity index).
            assert!(
                details
                    .split(" | ")
                    .all(|step| !step.starts_with("SCAN") || step.contains("USING")),
                "{sql}: {details}"
            );
        }
    }
}
