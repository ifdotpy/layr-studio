//! Reinstall shared producers with indexed tombstone expiration on every durable source.
use crate::{StoreResult, Transaction};

pub(super) const DEFINITION: super::MigrationDefinition = super::MigrationDefinition {
    id: "2026-11-12-durable-tombstone-expiry",
    introduced: super::MigrationDate {
        year: 2026,
        month: 11,
        day: 12,
    },
    target: studio_runtime_schema::studio_errors::MigrationTarget::Canvas,
    supported_source: "Durable delete triggers walk the full changes partition for expiry",
    remove_when: "Supported installations and backups all contain indexed expiry producers",
};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    crate::bootstrap::install_current_producers(tx)
}

#[cfg(test)]
mod tests {
    use crate::peer_update_settings::{Canvas, canvas_tx};
    use studio_db_api::DbQuery;
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),sql="DROP TRIGGER durable_graph_edges_delete")]
    struct DropPredecessor {}
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),sql="CREATE TRIGGER durable_graph_edges_delete AFTER DELETE ON graph_edges BEGIN SELECT 1; END")]
    struct Predecessor {}

    #[test]
    fn durable_expiry_upgrade_reinstalls_every_producer_once_and_rolls_back() {
        let root = tempfile::tempdir().unwrap();
        let mut db =
            crate::fresh_state::canvas_before_indexes(&root.path().join("canvas.sqlite3"), &[])
                .unwrap();
        let position = super::super::POST_F1_CATALOG
            .iter()
            .position(|step| step.definition.id == super::DEFINITION.id)
            .unwrap();
        assert_eq!(
            super::super::POST_F1_CATALOG[position - 1].definition.id,
            "2026-11-12-scheduler-wake-indexes"
        );
        let tx = db.transaction().unwrap();
        canvas_tx(&tx)
            .named(&DropPredecessor::QUERY, &DropPredecessor {})
            .unwrap();
        canvas_tx(&tx)
            .named(&Predecessor::QUERY, &Predecessor {})
            .unwrap();
        super::super::write_canvas_catalog_at(&tx, position).unwrap();
        tx.commit().unwrap();
        let before = studio_db_api::catalog::export(&db).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        tx.rollback().unwrap();
        assert_eq!(
            format!("{before:?}"),
            format!("{:?}", studio_db_api::catalog::export(&db).unwrap())
        );
        super::super::upgrade_current_canvas(&mut db).unwrap();
        let objects = studio_db_api::catalog::export(&db).unwrap();
        for table in crate::durable_seq::DURABLE_TABLES {
            let name = format!("durable_{}_delete", table.name);
            let ddl = &objects.iter().find(|entry| entry.name == name).unwrap().sql;
            assert_eq!(
                ddl.matches("INDEXED BY changes_durable_tombstone_at")
                    .count(),
                3,
                "{name}: {ddl}"
            );
        }
        let version = super::super::schema_version(&db).unwrap();
        super::super::upgrade_current_canvas(&mut db).unwrap();
        assert_eq!(super::super::schema_version(&db).unwrap(), version);
    }
}
