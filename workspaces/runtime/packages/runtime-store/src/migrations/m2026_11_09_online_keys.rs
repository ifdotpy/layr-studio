//! Keep old integer progress readable while adding exact typed key cursors.
use rusqlite::Transaction;

pub(crate) fn apply(tx: &Transaction<'_>) -> crate::StoreResult<()> {
    for (column, storage) in [
        ("cursor_key", "BLOB"),
        ("copy_source_rows", "INTEGER"),
        ("copy_target_rows", "INTEGER"),
    ] {
        let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('runtime_payload_migrations') WHERE name=?1)",[column],|row|row.get(0))?;
        if !exists {
            tx.execute_batch(&format!(
                "ALTER TABLE runtime_payload_migrations ADD COLUMN {column} {storage}"
            ))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use studio_db_api::Table;

    #[test]
    fn fresh_and_migrated_cursor_columns_match_and_preserve_progress() {
        let mut migrated = rusqlite::Connection::open_in_memory().unwrap();
        let fresh = rusqlite::Connection::open_in_memory().unwrap();
        let ddl = crate::settings_schema::PayloadMigrations::DDL;
        migrated
            .execute_batch(&ddl.replace(
                ", cursor_key BLOB, copy_source_rows INTEGER, copy_target_rows INTEGER",
                "",
            ))
            .unwrap();
        migrated.execute_batch("INSERT INTO runtime_payload_migrations(name,cursor,status) VALUES('online:existing',700,'running')").unwrap();
        let tx = migrated.transaction().unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        tx.commit().unwrap();
        fresh.execute_batch(ddl).unwrap();
        let columns = |db: &rusqlite::Connection| {
            db.prepare("SELECT name,type,\"notnull\",dflt_value,hidden FROM pragma_table_xinfo('runtime_payload_migrations') ORDER BY cid").unwrap()
                .query_map([],|row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?,row.get::<_,Option<String>>(3)?,row.get::<_,i64>(4)?))).unwrap()
                .collect::<rusqlite::Result<Vec<_>>>().unwrap()
        };
        assert_eq!(columns(&migrated), columns(&fresh));
        assert_eq!(migrated.query_row("SELECT cursor,cursor_key FROM runtime_payload_migrations WHERE name='online:existing'",[],|row| Ok((row.get::<_,i64>(0)?,row.get::<_,Option<Vec<u8>>>(1)?))).unwrap(),(700,None));
    }
}
