//! Prepared agent identity and visibility columns; family cutover owns installation.
use studio_db_api::DbTable;

use super::{AgentIdCodec, AgentIdentity};
use crate::peer_update_settings::Canvas;

/// Destination descriptor slice shared with publication team visibility.
/// Remaining agent/chat columns are added by the same family owner before cutover.
#[derive(Debug, PartialEq, DbTable)]
#[db(table = "__f2_agents", database = Canvas, strict)]
#[db(index(name = "f2_agents_id", columns(id), unique))]
pub struct TypedAgentRow {
    #[db(key)]
    pub identity: AgentIdentity,
    #[db(codec = AgentIdCodec)]
    pub id: Option<crate::AgentId>,
    #[db(codec = AgentIdCodec)]
    pub root_id: Option<crate::AgentId>,
    /// Deletion timestamp in seconds; NULL denotes a live agent component.
    pub deleted_at: Option<f64>,
    /// Chat-only destination rows must not grant agent team visibility.
    pub agent_present: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::Table;

    #[test]
    fn prepared_agent_visibility_columns_keep_component_presence_and_native_identity() {
        let mut db = crate::fresh_state::test_canvas();
        TypedAgentRow::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
        for (id, agent_present, deleted_at) in [
            ("live", true, None),
            ("deleted", true, Some(123.5)),
            ("chat-only", false, None),
        ] {
            let id: crate::AgentId = id.into();
            let row = TypedAgentRow {
                identity: AgentIdentity::for_agent(&id),
                id: Some(id),
                root_id: agent_present.then(|| "root".into()),
                deleted_at,
                agent_present,
            };
            tx.insert(&row).unwrap();
            assert_eq!(
                tx.get::<TypedAgentRow>(&(row.identity.clone(),)).unwrap(),
                Some(row)
            );
        }
        assert!(
            <AgentIdentity as rusqlite::types::FromSql>::column_result(
                rusqlite::types::ValueRef::Blob(b"invalid-key")
            )
            .is_err()
        );
    }
}
