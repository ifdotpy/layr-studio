//! Indexed monitor window selection using the monitor owner's generated selectors.
use crate::item_monitor_terminal::MonitorsRow;
use crate::peer_update_settings::{Canvas, canvas_read};
use crate::receipt_tables::ErrorIncidentRow;
use studio_db_api::DbQuery;
use studio_runtime_schema::studio_errors::MonitorStatus;

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,String),
 tables(rows=MonitorsRow,holds=ErrorIncidentRow),indexes(rows=MonitorsRow::INDEX_RUNTIME_MONITOR_WINDOW_STATUS),
 columns(id=MonitorsRow::ID,record=MonitorsRow::RECORD,status=MonitorsRow::MONITOR_STATUS,hold_id=ErrorIncidentRow::ID),
 sql="SELECT {id},{record} FROM {rows} WHERE {status} IN (?1,?2,?3)
 AND NOT EXISTS(SELECT 1 FROM {holds} WHERE {hold_id}=?4||runtime_monitors.{id})")]
struct ActiveRows<'a> {
    #[db(column=MonitorsRow::MONITOR_STATUS)]
    first: MonitorStatus,
    #[db(column=MonitorsRow::MONITOR_STATUS)]
    second: MonitorStatus,
    #[db(column=MonitorsRow::MONITOR_STATUS)]
    third: MonitorStatus,
    #[db(column=ErrorIncidentRow::ID)]
    hold: &'a str,
}
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,String),
 tables(rows=MonitorsRow,holds=ErrorIncidentRow),indexes(rows=MonitorsRow::INDEX_RUNTIME_MONITOR_WINDOW_STATUS),
 columns(id=MonitorsRow::ID,record=MonitorsRow::RECORD,status=MonitorsRow::MONITOR_STATUS,created=MonitorsRow::MONITOR_CREATED,hold_id=ErrorIncidentRow::ID),
 sql="SELECT {id},{record} FROM {rows} WHERE {status}=?1
 AND NOT EXISTS(SELECT 1 FROM {holds} WHERE {hold_id}=?2||runtime_monitors.{id}) ORDER BY {created} DESC LIMIT ?3")]
struct TerminalRows<'a> {
    #[db(column=MonitorsRow::MONITOR_STATUS)]
    status: MonitorStatus,
    #[db(column=ErrorIncidentRow::ID)]
    hold: &'a str,
    #[db(column=MonitorsRow::ROWID)]
    rows: i64,
}
fn status(value: &str) -> rusqlite::Result<MonitorStatus> {
    MonitorStatus::ALL
        .iter()
        .copied()
        .find(|status| status.as_str() == value)
        .ok_or(rusqlite::Error::InvalidQuery)
}
pub struct MonitorWindowQueries;
impl MonitorWindowQueries {
    pub fn active(db: &rusqlite::Connection) -> rusqlite::Result<Vec<(String, String)>> {
        let values = crate::startup_recovery::ACTIVE_MONITOR_STATUSES;
        canvas_read(db).collect(
            &ActiveRows::QUERY,
            &ActiveRows {
                first: status(values[0])?,
                second: status(values[1])?,
                third: status(values[2])?,
                hold: crate::MONITOR_HOLD_PREFIX,
            },
        )
    }
    pub fn terminal(
        db: &rusqlite::Connection,
        value: &str,
        rows: i64,
    ) -> rusqlite::Result<Vec<(String, String)>> {
        canvas_read(db).collect(
            &TerminalRows::QUERY,
            &TerminalRows {
                status: status(value)?,
                hold: crate::MONITOR_HOLD_PREFIX,
                rows,
            },
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monitor_windows_search_generated_status_and_creation_index() {
        let db = crate::fresh_state::test_canvas();
        let params = TerminalRows {
            status: MonitorStatus::Completed,
            hold: crate::MONITOR_HOLD_PREFIX,
            rows: 100,
        };
        let plan = crate::query_plan_test_support::explain(&db, TerminalRows::QUERY.sql(), &params)
            .unwrap();
        assert!(
            plan.iter()
                .any(|step| step.contains("runtime_monitor_window_status")),
            "{plan:?}"
        );
        assert!(
            plan.iter()
                .all(|step| !step.contains("SCAN ") && !step.contains("TEMP B-TREE")),
            "{plan:?}"
        );
    }
}
