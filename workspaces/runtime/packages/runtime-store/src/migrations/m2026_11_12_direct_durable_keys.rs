//! Refresh consolidated durable producers without changing their stored key framing.
use crate::{StoreResult, Transaction};

pub(super) const DEFINITION: super::MigrationDefinition = super::MigrationDefinition {
    id: "2026-11-12-direct-durable-keys",
    introduced: super::MigrationDate {
        year: 2026,
        month: 11,
        day: 12,
    },
    target: studio_runtime_schema::studio_errors::MigrationTarget::Canvas,
    supported_source: "Consolidated F1 durable producers with one-row source_key wrappers",
    remove_when: "Supported installations no longer admit the original consolidated producers",
};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for table in crate::durable_seq::DURABLE_TABLES {
        if crate::schema::table_exists(tx, table.name)? {
            tx.execute_batch(&crate::durable_seq::trigger_sql(
                table.name,
                table.record_only,
                table.team_fence,
            ))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn startup_schema_upgrades_predecessor_durable_producers_once() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::RuntimeStore::open(Some(directory.path()))
            .await
            .unwrap();
        let position = super::super::POST_F1_CATALOG
            .iter()
            .position(|step| step.definition.id == super::DEFINITION.id)
            .unwrap();
        store.transaction(move |tx| {
            tx.execute_batch("DROP TRIGGER durable_graph_edges_insert; CREATE TRIGGER durable_graph_edges_insert AFTER INSERT ON graph_edges BEGIN SELECT 1; END;")?;
            crate::clock_schema::SyncEntityMeta::put(tx,super::super::MIGRATION_CATALOG_KEY,&super::super::catalog_marker(tx,&super::super::canvas_catalog_prefix_fingerprint(position))?)?;
            Ok(())
        }).await.unwrap();
        drop(store);
        let store = crate::RuntimeStore::open(Some(directory.path()))
            .await
            .unwrap();
        let version = store.read_async::<_,crate::StoreError>("direct-key-upgrade", |db| {
            let sql: String = db.query_row("SELECT sql FROM sqlite_schema WHERE type='trigger' AND name='durable_graph_edges_insert'",[],|row|row.get(0))?;
            assert!(sql.contains("INSERT INTO changes"));
            assert!(!sql.contains("AS source_key"));
            Ok(db.pragma_query_value(None,"schema_version",|row|row.get::<_,i64>(0))?)
        }).await.unwrap();
        drop(store);
        let store = crate::RuntimeStore::open(Some(directory.path()))
            .await
            .unwrap();
        let reopened = store
            .read_async::<_, crate::StoreError>("direct-key-reopen", |db| {
                Ok(db.pragma_query_value(None, "schema_version", |row| row.get::<_, i64>(0))?)
            })
            .await
            .unwrap();
        assert_eq!(
            version, reopened,
            "current producers are not recreated on every startup"
        );
        assert_eq!(
            super::super::POST_F1_CATALOG[position].definition.id,
            super::DEFINITION.id
        );
    }
}
