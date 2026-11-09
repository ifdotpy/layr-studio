//! Pilot typed table. Canvas adapters own the capability; the physical schema is unchanged.
use studio_db_api::{Database, DbTable, Tx, db_schema};

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

pub(crate) fn canvas_tx<'c>(transaction: &'c rusqlite::Transaction<'c>) -> Tx<'c, Canvas> {
    Tx::from_adapter(transaction, &Canvas { _private: () })
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
        canvas_tx(transaction).get::<Self>(&(1,))
    }
    pub fn upsert(&self, transaction: &rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
        canvas_tx(transaction).upsert(self)
    }
}

pub(crate) fn registry() -> [studio_db_api::Registration; 1] {
    db_schema![Canvas; PeerUpdateSettings]
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::Table;
    #[test]
    fn pilot_ddl_matches_catalog_and_existing_state() {
        #[derive(serde::Deserialize)]
        struct Catalog {
            #[serde(rename = "canvas.sqlite3")]
            canvas: Objects,
        }
        #[derive(serde::Deserialize)]
        struct Objects {
            sqlite_master: Vec<Entry>,
        }
        #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
        struct Entry {
            create_order: usize,
            name: String,
            sql: String,
            table: String,
            r#type: String,
        }
        let snapshot: Catalog =
            studio_json::from_str(crate::fresh_state::SNAPSHOT).expect("catalog");
        let entry = snapshot
            .canvas
            .sqlite_master
            .iter()
            .find(|entry| entry.name == PeerUpdateSettings::NAME)
            .expect("pilot catalog entry");
        assert_eq!(entry.sql, PeerUpdateSettings::DDL);
        let exported: Entry = studio_json::from_str(
            &PeerUpdateSettings::descriptor()
                .catalog_json(entry.create_order)
                .expect("export"),
        )
        .expect("JSON");
        assert_eq!(&exported, entry);
        let mut db = rusqlite::Connection::open_in_memory().expect("db");
        db.execute_batch("CREATE TABLE runtime_peer_update_settings(id INTEGER PRIMARY KEY CHECK(id=1), auto_update INTEGER NOT NULL); INSERT INTO runtime_peer_update_settings VALUES(1,0)").expect("pre-existing state");
        let [registration] = registry();
        assert_eq!((registration.check)(&db).expect("state check"), (1, 0));
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
