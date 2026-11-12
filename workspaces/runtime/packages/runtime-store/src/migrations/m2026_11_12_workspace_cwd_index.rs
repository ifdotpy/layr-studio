//! Cover working-directory discovery without reading or sorting archived agent records.
use crate::agent_tables::AgentsStorageRow;
use crate::peer_update_settings::Canvas;
use crate::{StoreResult, Transaction};

pub(super) const DEFINITION: super::MigrationDefinition = super::MigrationDefinition {
    id: "2026-11-12-workspace-cwd-index",
    introduced: super::MigrationDate {
        year: 2026,
        month: 11,
        day: 12,
    },
    target: studio_runtime_schema::studio_errors::MigrationTarget::Canvas,
    supported_source: "Workspace discovery scans and sorts the agent table",
    remove_when: "Supported installations and backups all contain the covering cwd index",
};
pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    crate::queue_schema::install_index::<AgentsStorageRow>(
        tx,
        studio_db_api::query_index::<Canvas, AgentsStorageRow>(
            &AgentsStorageRow::INDEX_RUNTIME_AGENT_WORKSPACE_CWD,
        ),
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn workspace_cwd_upgrade_appends_after_expiry_and_is_atomic() {
        let name = studio_db_api::query_index::<super::Canvas, super::AgentsStorageRow>(
            &super::AgentsStorageRow::INDEX_RUNTIME_AGENT_WORKSPACE_CWD,
        );
        let root = tempfile::tempdir().unwrap();
        let mut db =
            crate::fresh_state::canvas_before_indexes(&root.path().join("canvas.sqlite3"), &[name])
                .unwrap();
        let position = super::super::POST_F1_CATALOG
            .iter()
            .position(|step| step.definition.id == super::DEFINITION.id)
            .unwrap();
        assert_eq!(
            super::super::POST_F1_CATALOG[position - 1].definition.id,
            "2026-11-12-durable-tombstone-expiry"
        );
        let tx = db.transaction().unwrap();
        super::super::write_canvas_catalog_at(&tx, position).unwrap();
        tx.commit().unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        tx.rollback().unwrap();
        assert!(
            !studio_db_api::catalog::export(&db)
                .unwrap()
                .iter()
                .any(|entry| entry.name == name)
        );
        super::super::upgrade_current_canvas(&mut db).unwrap();
        assert!(
            studio_db_api::catalog::export(&db)
                .unwrap()
                .iter()
                .any(|entry| entry.name == name)
        );
        let version = super::super::schema_version(&db).unwrap();
        super::super::upgrade_current_canvas(&mut db).unwrap();
        assert_eq!(super::super::schema_version(&db).unwrap(), version);
    }
}
