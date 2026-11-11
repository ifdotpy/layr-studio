//! Descriptor-bound lookups used by writer transactions.
use crate::agent_tables::AgentsStorageRow;
use crate::item_monitor_terminal::MonitorsRow;
use crate::peer_update_settings::Canvas;
use crate::planning_work::TaskRow;

#[studio_db_api::db_queries]
mod queries {
    use super::*;

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
pub use queries::{DeletedCommandIds, LastRuleSuccess};

#[cfg(test)]
mod tests {
    #[test]
    fn writer_query_descriptors_prepare_against_canvas() {
        let db = crate::fresh_state::test_canvas();
        super::queries::validate_queries(&db).unwrap();
    }
}
