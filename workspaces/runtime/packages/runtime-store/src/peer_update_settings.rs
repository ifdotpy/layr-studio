//! Pilot typed table. Canvas adapters own the capability; the physical schema is unchanged.
use studio_db_api::{Database, DbTable, Tx};

/// The marker capability has no public constructor.
/// ```compile_fail
/// use studio_runtime_store::peer_update_settings::Canvas;
/// use studio_db_api::Tx;
/// fn forged<'a>(transaction: &'a rusqlite::Transaction<'a>) -> Tx<'a, Canvas> {
///     Tx::from_adapter(transaction, &Canvas { _private: () })
/// }
/// ```
pub struct Canvas {
    _private: (),
}
impl Database for Canvas {}

pub fn canvas_read(connection: &rusqlite::Connection) -> studio_db_api::Read<'_, Canvas> {
    studio_db_api::Read::from_adapter(connection, &Canvas { _private: () })
}

pub fn canvas_tx<'c>(transaction: &'c rusqlite::Transaction<'c>) -> Tx<'c, Canvas> {
    Tx::from_adapter(transaction, &Canvas { _private: () })
}

pub(crate) fn canvas_write(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<studio_db_api::ConnectionWrite<'_, Canvas>> {
    studio_db_api::ConnectionWrite::from_adapter(connection, &Canvas { _private: () })
}

#[derive(Debug, PartialEq, Eq, DbTable)]
#[db(table = "runtime_peer_update_settings", database = Canvas)]
pub struct PeerUpdateSettings {
    #[db(key, check = "id=1")]
    pub id: i64,
    pub auto_update: i64,
}

impl PeerUpdateSettings {
    pub fn get(transaction: &rusqlite::Transaction<'_>) -> rusqlite::Result<Option<Self>> {
        Ok(canvas_tx(transaction)
            .get::<crate::metadata_schema::PeerUpdateSettings>(&(
                studio_db_api::migration::encode_key(&(1_i64,)),
            ))?
            .map(|row| Self {
                id: row.integer_key,
                auto_update: row.auto_update.as_stored(),
            }))
    }
    pub fn upsert(&self, transaction: &rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
        canvas_tx(transaction)
            .partition::<crate::metadata_schema::PeerUpdateSettings>()
            .put(&crate::metadata_schema::PeerUpdateSettings {
                identity: studio_db_api::migration::encode_key(&(self.id,)),
                source_rowid: self.id,
                integer_key: self.id,
                auto_update: crate::metadata_values::StoredBoolean::from_stored(self.auto_update),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::Table;
    #[test]
    fn metadata_singleton_preserves_absence_integer_evidence_and_rollback() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        crate::metadata_schema::Metadata::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let transaction = db.transaction().unwrap();
        assert_eq!(PeerUpdateSettings::get(&transaction).unwrap(), None);
        for auto_update in [0, 1, -7, i64::MAX] {
            let row = PeerUpdateSettings { id: 1, auto_update };
            row.upsert(&transaction).unwrap();
            assert_eq!(PeerUpdateSettings::get(&transaction).unwrap(), Some(row));
        }
        assert!(
            PeerUpdateSettings {
                id: 2,
                auto_update: 1
            }
            .upsert(&transaction)
            .is_err()
        );
        transaction.rollback().unwrap();
        let transaction = db.transaction().unwrap();
        assert_eq!(PeerUpdateSettings::get(&transaction).unwrap(), None);
    }
    #[test]
    fn pilot_preserves_existing_state() {
        let mut db = rusqlite::Connection::open_in_memory().expect("db");
        db.execute_batch("CREATE TABLE runtime_peer_update_settings(id INTEGER PRIMARY KEY CHECK(id=1), auto_update INTEGER NOT NULL); INSERT INTO runtime_peer_update_settings VALUES(1,0)").expect("pre-existing state");
        // The descriptor is retained solely as F1's migration source reader.
        assert!(
            !crate::canvas_schema::registry()
                .iter()
                .any(|entry| entry.descriptor.name == PeerUpdateSettings::NAME)
        );
        assert_eq!(
            PeerUpdateSettings::state_check(&db)
                .expect("state check")
                .counts(),
            (1, 0)
        );
        let transaction = db.transaction().expect("transaction");
        {
            let mut typed = canvas_tx(&transaction);
            assert_eq!(
                typed
                    .get::<PeerUpdateSettings>(&(1,))
                    .expect("read old row")
                    .expect("row")
                    .auto_update,
                0
            );
            typed
                .upsert(&PeerUpdateSettings {
                    id: 1,
                    auto_update: 1,
                })
                .expect("update");
        }
        transaction.rollback().expect("outer rollback");
        assert_eq!(
            db.query_row(
                "SELECT auto_update FROM runtime_peer_update_settings",
                [],
                |r| r.get::<_, i64>(0)
            )
            .expect("rolled back"),
            0
        );
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use studio_db_api::{Table, db_query};
    #[derive(DbTable)]
    #[db(table="cache_witness",database=Canvas)]
    struct CacheRow {
        #[db(key)]
        id: i64,
        amount: i64,
    }
    #[test]
    fn generated_statements_reuse_preparations_including_variable_arrays() {
        let mut db = rusqlite::Connection::open_in_memory().expect("db");
        db.set_prepared_statement_cache_capacity(crate::STATEMENT_CACHE_CAPACITY);
        rusqlite::vtab::array::load_module(&db).expect("array");
        db.execute_batch(CacheRow::DDL).expect("DDL");
        let count = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&count);
        db.authorizer(Some(move |_: rusqlite::hooks::AuthContext<'_>| {
            observed.fetch_add(1, Ordering::Relaxed);
            rusqlite::hooks::Authorization::Allow
        }))
        .expect("authorizer");
        let transaction = db.transaction().expect("tx");
        let mut tx = canvas_tx(&transaction);
        tx.insert(&CacheRow { id: 1, amount: 10 }).expect("insert");
        let after = count.load(Ordering::Relaxed);
        tx.insert(&CacheRow { id: 2, amount: 20 })
            .expect("cached insert");
        assert_eq!(count.load(Ordering::Relaxed), after);
        assert!(tx.get::<CacheRow>(&(1,)).expect("get").is_some());
        let after = count.load(Ordering::Relaxed);
        assert!(tx.get::<CacheRow>(&(2,)).expect("cached get").is_some());
        assert_eq!(count.load(Ordering::Relaxed), after);
        tx.upsert(&CacheRow { id: 1, amount: 11 }).expect("upsert");
        let after = count.load(Ordering::Relaxed);
        tx.upsert(&CacheRow { id: 2, amount: 21 })
            .expect("cached upsert");
        assert_eq!(count.load(Ordering::Relaxed), after);
        let ids = [1];
        assert_eq!(
            db_query!(
                tx,
                CacheRow,
                limit(3),
                order_by(CacheRow::ID.desc),
                filter(CacheRow::ID.in_(&ids))
            )
            .expect("query")
            .len(),
            1
        );
        let after = count.load(Ordering::Relaxed);
        let ids = [1, 2];
        let rows = db_query!(
            tx,
            CacheRow,
            limit(3),
            order_by(CacheRow::ID.desc),
            filter(CacheRow::ID.in_(&ids))
        )
        .expect("cached query");
        assert_eq!(count.load(Ordering::Relaxed), after);
        assert_eq!(rows[0].id, 2);
        assert_eq!(rows[1].id, 1);
        tx.delete::<CacheRow>(&(1,)).expect("delete");
        let after = count.load(Ordering::Relaxed);
        tx.delete::<CacheRow>(&(2,)).expect("cached delete");
        assert_eq!(count.load(Ordering::Relaxed), after);
    }
}
