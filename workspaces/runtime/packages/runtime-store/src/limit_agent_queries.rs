//! Narrow account recovery rows. The agent owner owns the predicate and its index.
use crate::agent_tables::AgentsStorageRow;
use crate::peer_update_settings::{Canvas, canvas_read};
use studio_db_api::DbQuery;

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,String),
    tables(agents=AgentsStorageRow),indexes(agents=AgentsStorageRow::INDEX_RUNTIME_AGENT_LIMIT_RECOVERY),predicates(recovery=agents),
    columns(id=AgentsStorageRow::ID,record=AgentsStorageRow::RECORD,account=AgentsStorageRow::ACCOUNT_KEY),
    sql="SELECT {id},json_object(
        'id',{record}->'$.id','accountKey',{record}->'$.accountKey','epoch',{record}->'$.epoch',
        'deletedAt',{record}->'$.deletedAt','model',{record}->'$.model','provider',{record}->'$.provider',
        'error',{record}->'$.error','usageResume',{record}->'$.usageResume',
        'threadId',{record}->'$.threadId','lastCompletedTurn',{record}->'$.lastCompletedTurn')
        FROM {agents} WHERE {recovery} AND {account} IN (?1,CASE WHEN ?1='default' THEN '' ELSE ?1 END)")]
struct RecoveryAgents<'a> {
    #[db(column=AgentsStorageRow::ACCOUNT_KEY)]
    account: &'a str,
}

pub struct LimitAgentQueries;
impl LimitAgentQueries {
    /// Only fields consumed by recovery projection; the BTreeMap caller owns ordering.
    pub fn recovery_agents(
        db: &rusqlite::Connection,
        account: &str,
    ) -> rusqlite::Result<Vec<(String, String)>> {
        canvas_read(db).collect(&RecoveryAgents::QUERY, &RecoveryAgents { account })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_query_work_does_not_grow_with_idle_agents() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        for index in 0..1000 {
            let id = crate::AgentId::from(format!("idle-{index}"));
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
            crate::AgentRepository::put(&tx, &agent).unwrap();
        }
        tx.commit().unwrap();
        let (rows, steps) = crate::query_plan_test_support::vm_steps(&db, || {
            LimitAgentQueries::recovery_agents(&db, "default")
        })
        .unwrap();
        eprintln!(
            "recovery query: {} rows / {steps} VM steps behind 1000 idle agents",
            rows.len()
        );
        assert!(rows.is_empty());
        assert!(
            steps < 80,
            "recovery lookup visited idle account agents: {steps}"
        );
    }

    #[test]
    fn recovery_agents_use_the_owned_index_without_scan_or_sort() {
        let db = crate::fresh_state::test_canvas();
        for account in ["default", "other"] {
            let plan = crate::query_plan_test_support::explain(
                &db,
                RecoveryAgents::QUERY.sql(),
                &RecoveryAgents { account },
            )
            .unwrap();
            assert!(
                plan.iter()
                    .any(|step| step.contains("SEARCH")
                        && step.contains("runtime_agent_limit_recovery")),
                "{plan:?}"
            );
            assert!(
                plan.iter()
                    .all(|step| !step.contains("SCAN ") && !step.contains("TEMP B-TREE")),
                "{plan:?}"
            );
        }
    }
}
