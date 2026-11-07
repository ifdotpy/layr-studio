//! Checked typed agent and task snapshots for pure notification rules.
use super::{NotificationDispatchError, stored};
use studio_json::Value;
use studio_runtime_store::{AgentRecord, TaskRecord};

pub(super) struct NotificationSnapshot {
    agent: AgentRecord,
    task: Option<TaskRecord>,
    item: Option<Value>,
}
impl NotificationSnapshot {
    pub(super) fn from_records(
        agent: AgentRecord,
        task: Option<TaskRecord>,
        item: Option<Value>,
    ) -> Result<Self, NotificationDispatchError> {
        super::validate_agent_write_fields(&agent, agent.id.as_ref())?;
        if let Some(record) = &item {
            checked("runtime_items", record)?;
        }
        Ok(Self { agent, task, item })
    }
    pub(super) fn agent(&self) -> &AgentRecord {
        &self.agent
    }
    pub(super) fn task(&self) -> Option<&TaskRecord> {
        self.task.as_ref()
    }
    pub(super) fn item(&self) -> Option<&Value> {
        self.item.as_ref()
    }
}
pub(super) fn checked(
    table: &'static str,
    record: &Value,
) -> Result<(), NotificationDispatchError> {
    let id = record["id"].as_str().unwrap_or("<missing>");
    let schema = studio_runtime_schema::table_schema(table).ok_or_else(|| stored(table, id))?;
    if !studio_runtime_schema::validate(schema, record).map_err(|_| stored(table, id))? {
        return Err(stored(table, id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn notification_agent_write_preserves_legacy_row_bytes_in_typed_snapshot() {
        let mut agent = studio_runtime_store::AgentRecord::new(
            "agent".into(),
            "agent".into(),
            1,
            studio_runtime_store::AgentStatus::Idle,
            "default".into(),
        );
        agent
            .extra
            .insert("futureExtension".into(), studio_json::json!([1, 2, 3]));
        let allocation = agent.extra["futureExtension"].as_array().unwrap().as_ptr();
        let snapshot = super::NotificationSnapshot::from_records(agent, None, None).unwrap();
        assert_eq!(
            allocation,
            snapshot.agent().extra["futureExtension"]
                .as_array()
                .unwrap()
                .as_ptr()
        );
    }
}
