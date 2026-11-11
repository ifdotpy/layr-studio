//! Indexed agent fields as typed columns of `runtime_agents`.
//!
//! Storage format introduced on 2026-10-20; the dated migrations apply the explicit-provider cutover of 2026-10-26. Every agent
//! field that an index or an indexed query reads is a generated column
//! of `runtime_agents`. STORED columns are computed on write; VIRTUAL columns
//! used by covering indexes are computed when the index is maintained.
//! Indexes are plain column indexes, and queries filter and sort on the columns. The record stays the one source of truth: a column cannot
//! disagree with its record, and every writer (including raw test fixtures)
//! keeps writing `(id, record)` only.
//!
//! Before this format, about twenty JSON expression indexes re-evaluated
//! their `json_extract` keys and predicates for the old and the new record on
//! every agent write. A generated column is evaluated once, for the new record;
//! the old key comes from the stored row.
//!
//! [`AGENT_COLUMNS`] owns the column list: name, SQLite type, value and
//! constraint. A column holds exactly what its value expression yields:
//! `record->>'$.path'` is `json_extract(record,'$.path')` (JSON true and false
//! read as 1 and 0, JSON null and an absent field as SQL NULL). A column
//! declared without a type keeps the JSON value's own SQL type, for fields
//! whose stored JSON type is not fixed. There are no CHECK constraints: the
//! record schema owns field types and values, and normal readers refuse an
//! unknown status or invalid record by name; held evidence stays separate. A
//! CHECK would instead make such a row unwritable. Malformed JSON is refused
//! at write.

use std::sync::LazyLock;

use crate::StoreResult;

/// Typed successor contracts; installed only by the reviewed family cutover.
pub mod typed;

/// The agents table.
pub const AGENT_TABLE: &str = "runtime_agents";

/// One generated column of [`AGENT_TABLE`].
pub struct AgentColumn {
    /// Whether the value is materialized in the table.
    pub stored: bool,
    /// Column name, used verbatim in SQL.
    pub name: &'static str,
    /// Declared SQLite type (affinity); empty keeps the value's own type.
    pub sql_type: &'static str,
    /// SQL expression over `record` that computes the column.
    pub value: &'static str,
    /// Column constraint (`NOT NULL`), or empty.
    pub constraint: &'static str,
}

const fn column(
    name: &'static str,
    sql_type: &'static str,
    value: &'static str,
    constraint: &'static str,
) -> AgentColumn {
    AgentColumn {
        stored: true,
        name,
        sql_type,
        value,
        constraint,
    }
}

const fn virtual_column(name: &'static str, value: &'static str) -> AgentColumn {
    AgentColumn {
        name,
        sql_type: "",
        value,
        constraint: "",
        stored: false,
    }
}

/// SQL truth test of the record field at JSON path `$path` with the
/// truthiness of the agent writers: absent, null, false, 0, "", [] and {} are
/// false. The value of `agent_archived` and `review_archive_failed`.
macro_rules! truthy {
    ($path:literal) => {
        concat!(
            "NOT (json_type(record,'",
            $path,
            "') IS NULL",
            " OR json_type(record,'",
            $path,
            "') IN ('null','false')",
            " OR (json_type(record,'",
            $path,
            "') IN ('integer','real') AND record->>'",
            $path,
            "'=0)",
            " OR (json_type(record,'",
            $path,
            "')='text' AND record->>'",
            $path,
            "'='')",
            " OR (json_type(record,'",
            $path,
            "')='array' AND json_array_length(record,'",
            $path,
            "')=0)",
            " OR (json_type(record,'",
            $path,
            "')='object' AND record->>'",
            $path,
            "'='{}'))"
        )
    };
}

/// The explicit provider column is also the Canvas cutover evidence for 2026-10-26.
const PROVIDER_COLUMN: AgentColumn =
    column("provider", "TEXT", "record->>'$.provider'", "NOT NULL");

/// Indexed predicate for an agent with unfinished authentication proof fanout.
/// Both server queries and the migration index consume this owner.
pub const AUTH_RESUME_PROOF_PENDING_PREDICATE: &str = "auth_resume_proof_pending=1";

pub(crate) const AUTH_RESUME_PROOF_COLUMN: AgentColumn = virtual_column(
    "auth_resume_proof_pending",
    "CASE WHEN json_valid(record) THEN CASE WHEN json_type(record,'$.authResumeProofPending')='object' AND json(json_extract(record,'$.authResumeProofPending')) <> '{}' THEN 1 ELSE 0 END ELSE 0 END",
);

/// The indexed agent fields; see the module documentation. Changing this list
/// changes the table and needs a new dated migration.
pub const AGENT_COLUMNS: [AgentColumn; 35] = [
    AUTH_RESUME_PROOF_COLUMN,
    virtual_column("account_transfer_id", "record->>'$.accountTransferId'"),
    virtual_column(
        "native_release_reset_pending",
        "record->>'$.nativeRelease.resetPending'",
    ),
    virtual_column("is_lead", "record->>'$.isLead'"),
    column("root_id", "TEXT", "record->>'$.rootId'", ""),
    column("parent_id", "TEXT", "record->>'$.parentId'", ""),
    column(
        "account_key",
        "TEXT",
        "COALESCE(record->>'$.accountKey','default')",
        "NOT NULL",
    ),
    PROVIDER_COLUMN,
    column("role", "TEXT", "record->>'$.role'", ""),
    column("deleted_at", "", "record->>'$.deletedAt'", ""),
    column("cwd", "TEXT", "record->>'$.cwd'", ""),
    column(
        "workspace_operation",
        "",
        "record->>'$.workspaceOperation'",
        "",
    ),
    column("remote_worker", "", "record->>'$.remoteWorker'", ""),
    column("remote_anchor", "", "record->>'$.remoteAnchor'", ""),
    column(
        "native_failure_hold",
        "",
        "record->>'$.nativeFailureHold'",
        "",
    ),
    column(
        "steer_rejected_turn_id",
        "TEXT",
        "record->>'$.steerRejectedTurnId'",
        "",
    ),
    column(
        "native_tool_refresh_id",
        "TEXT",
        "record->>'$.nativeToolRefreshId'",
        "",
    ),
    column(
        "context_repair_phase",
        "TEXT",
        "record->>'$.contextRepair.phase'",
        "",
    ),
    column(
        "source_cleanup_phase",
        "TEXT",
        "record->>'$.contextRepair.sourceCleanup.phase'",
        "",
    ),
    column(
        "source_cleanup_account_key",
        "TEXT",
        "record->>'$.contextRepair.sourceCleanup.accountKey'",
        "",
    ),
    column(
        "source_cleanup_thread_id",
        "TEXT",
        "record->>'$.contextRepair.sourceCleanup.threadId'",
        "",
    ),
    column(
        "has_disconnect_recovery",
        "INTEGER",
        "COALESCE(json_type(record,'$.disconnectRecovery')='object',0)",
        "NOT NULL",
    ),
    column(
        "disconnect_recovery_stage",
        "TEXT",
        "record->>'$.disconnectRecovery.stage'",
        "",
    ),
    column(
        "has_restart_recovery",
        "INTEGER",
        "COALESCE(json_type(record,'$.restartRecovery')='object',0)",
        "NOT NULL",
    ),
    column(
        "restart_recovery_stage",
        "TEXT",
        "record->>'$.restartRecovery.stage'",
        "",
    ),
    column(
        "has_workspace_cleanup_intent",
        "INTEGER",
        "COALESCE(json_type(record,'$.workspaceCleanupIntent')='object',0)",
        "NOT NULL",
    ),
    column(
        "agent_archived",
        "INTEGER",
        truthy!("$.agentArchive"),
        "NOT NULL",
    ),
    column(
        "review_archive_failed",
        "INTEGER",
        truthy!("$.reviewArchiveError"),
        "NOT NULL",
    ),
    column(
        "review_archive_marked",
        "INTEGER",
        "record->>'$.reviewArchiveScheduled' IS NOT NULL",
        "NOT NULL",
    ),
    column(
        "review_archive_scheduled",
        "INTEGER",
        "COALESCE(json_type(record,'$.reviewArchiveScheduled')='text' \
         AND record->>'$.reviewArchiveScheduled'<>'',0)",
        "NOT NULL",
    ),
    column(
        "review_archive_next_at",
        "",
        "record->>'$.reviewArchiveNextAt'",
        "",
    ),
    // Episode presence, deadline in Unix seconds, and durable exclusion identity.
    column(
        "idle_episode_type",
        "TEXT",
        "json_type(record,'$.idleCompactionEpisode')",
        "",
    ),
    column(
        "idle_episode_fire_at",
        "",
        "record->>'$.idleCompactionEpisode.fireAt'",
        "",
    ),
    column(
        "idle_episode_id",
        "",
        "record->>'$.idleCompactionEpisode.id'",
        "",
    ),
    // A legacy image workspace that startup has not yet finished.
    column(
        "legacy_unsettled",
        "INTEGER",
        "COALESCE(record->>'$.workspaceMode'='image' OR record->>'$.imageWorkspace'=1,0) \
         AND COALESCE(record->>'$.legacyWorkspaceSettled',0)=0",
        "NOT NULL",
    ),
];

/// The `CREATE TABLE` statement of [`AGENT_TABLE`] named `name`. The columns
/// precede `record`: a row's large record spills to overflow pages, and a
/// column stored after it could be read only by walking that chain.
pub(crate) fn column_sql(column: &AgentColumn) -> String {
    format!(
        "\"{}\" {} GENERATED ALWAYS AS ({}) {} {}",
        column.name,
        column.sql_type,
        column.value,
        if column.stored { "STORED" } else { "VIRTUAL" },
        column.constraint
    )
}

pub(crate) fn table_sql(name: &str) -> String {
    let columns = AGENT_COLUMNS
        .iter()
        .map(column_sql)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "CREATE TABLE IF NOT EXISTS \"{name}\"(id TEXT PRIMARY KEY,{columns},record TEXT NOT NULL,{})",
        crate::schema::receipt_owner_columns()
            .into_iter()
            .filter(|(table, _, _)| *table == AGENT_TABLE)
            .map(|(_, name, definition)| format!("{name} {definition}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// The table as a fresh schema-only test fixture declares it.
pub fn agent_table_sql() -> &'static str {
    static SQL: LazyLock<String> = LazyLock::new(|| table_sql(AGENT_TABLE));
    &SQL
}

/// `batch` with its `CREATE TABLE runtime_agents(...)` statement replaced by
/// this format's table, for test fixtures that declare their own schema.
#[doc(hidden)]
pub fn fixture_sql(batch: &str) -> String {
    let batch = crate::planning_work::fixture_sql(batch);
    let batch = batch.as_str();
    let Some(start) = batch.find("CREATE TABLE runtime_agents") else {
        return batch.to_owned();
    };
    let end = batch[start..]
        .find(';')
        .map_or(batch.len(), |offset| start + offset);
    format!(
        "{}{}; CREATE INDEX IF NOT EXISTS runtime_agent_live_root_v2 ON runtime_agents(root_id,id) WHERE deleted_at IS NULL{}",
        &batch[..start],
        agent_table_sql(),
        &batch[end..]
    )
}

/// `SELECT` of the declared (not generated) columns of `table`, for test
/// oracles that compare whole rows with states written before 2026-10-20.
#[doc(hidden)]
pub fn select_declared_sql(connection: &rusqlite::Connection, table: &str) -> StoreResult<String> {
    let quoted = table.replace('"', "\"\"");
    let columns = connection
        .prepare(&format!("PRAGMA table_info(\"{quoted}\")"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .map(|name| name.map(|name| format!("\"{}\"", name.replace('"', "\"\""))))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("SELECT {} FROM \"{quoted}\"", columns.join(",")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn unused_agent_columns_are_not_computed_on_write() {
        let connection = Connection::open_in_memory().expect("memory SQLite opens");
        connection
            .execute_batch(agent_table_sql())
            .expect("table creates");
        for column in [
            "native_reset_pending",
            "capacity_retry_id",
            "capacity_retry_status",
            "capacity_retry_due_at",
            "start_attempt_action",
            "status",
            "in_flight",
            "turn_id",
        ] {
            let exists: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_xinfo('runtime_agents') WHERE name=?1)",
                [column], |row| row.get(0),
            ).expect("schema reads");
            assert!(
                !exists,
                "unused generated column {column} must not be computed"
            );
        }
    }

    /// A record write updates the columns; malformed JSON is refused.
    #[test]
    fn columns_follow_the_record_and_refuse_malformed_json() {
        let connection = Connection::open_in_memory().expect("memory SQLite opens");
        connection
            .execute_batch(agent_table_sql())
            .expect("table creates");
        connection
            .execute(
                "INSERT INTO runtime_agents(id,record) VALUES('a',?1)",
                [r#"{"compactions":0,"id":"a","provider":"codex","rootId":"a","epoch":0,"status":"idle","accountKey":"default"}"#],
            )
            .expect("agent inserts");
        connection
            .execute(
                "UPDATE runtime_agents SET record=json_set(record,'$.status','queued') WHERE id='a'",
                [],
            )
            .expect("agent updates");
        let status: String = connection
            .query_row(
                "SELECT record->>'$.status' FROM runtime_agents",
                [],
                |row| row.get(0),
            )
            .expect("columns");
        assert_eq!(status, "queued");
        connection
            .execute(
                "INSERT INTO runtime_agents(id,record) VALUES('b','{bad json')",
                [],
            )
            .expect_err("malformed JSON is refused");
    }
}
