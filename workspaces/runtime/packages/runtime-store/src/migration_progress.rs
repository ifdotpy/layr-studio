//! Progress authority follows the ledger table in the current SQLite snapshot.
use studio_db_api::{Read, Tx};

use crate::metadata_schema::PayloadMigrations as Target;
use crate::metadata_values::StoredBoolean;
use crate::peer_update_settings::Canvas;
use crate::settings_schema::PayloadMigrations as Source;
use crate::{StoreError, StoreResult};

#[derive(studio_db_api::DbProjection)]
#[db(of=crate::metadata_schema::Metadata)]
struct OnlineState {
    text_key: Option<String>,
    migration_cursor: Option<i64>,
    migration_complete: Option<i64>,
    migration_status: Option<studio_runtime_schema::persistence::MigrationStatus>,
    migration_updated: Option<f64>,
    migration_error: Option<String>,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=OnlineState,projection=OnlineState,
    tables(rows=crate::metadata_schema::Metadata),
    columns(kind=crate::metadata_schema::Metadata::KIND,name=crate::metadata_schema::Metadata::TEXT_KEY,cursor=crate::metadata_schema::Metadata::MIGRATION_CURSOR,complete=crate::metadata_schema::Metadata::MIGRATION_COMPLETE,status=crate::metadata_schema::Metadata::MIGRATION_STATUS,updated=crate::metadata_schema::Metadata::MIGRATION_UPDATED,error=crate::metadata_schema::Metadata::MIGRATION_ERROR),
    sql="SELECT {name},{cursor},{complete},{status},{updated},{error} FROM {rows} WHERE {kind}=?1 AND {name} LIKE ?2 ORDER BY {name}")]
struct OnlineRows<'a> {
    #[db(column=crate::metadata_schema::Metadata::KIND)]
    kind: crate::consolidation_kinds::CanvasMetadataKind,
    #[db(column=crate::metadata_schema::Metadata::TEXT_KEY)]
    pattern: Option<&'a str>,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=Option<i64>,decode=Source::COPY_SOURCE_ROWS,
    tables(rows=crate::metadata_schema::Metadata),
    columns(kind=crate::metadata_schema::Metadata::KIND,migration=crate::metadata_schema::Metadata::DIRTY_MIGRATION,count=Source::COPY_SOURCE_ROWS),
    sql="SELECT count(*) AS {count} FROM {rows} WHERE {kind}=?1 AND {migration}=?2")]
struct DirtyCount<'a> {
    #[db(column=crate::metadata_schema::Metadata::KIND)]
    kind: crate::consolidation_kinds::CanvasMetadataKind,
    #[db(column=crate::metadata_schema::Metadata::DIRTY_MIGRATION)]
    migration: Option<&'a str>,
}

pub(crate) fn progress(
    db: &studio_db_api::rusqlite::Connection,
) -> studio_db_api::rusqlite::Result<Vec<crate::online_migration::Progress>> {
    use crate::consolidation_kinds::CanvasMetadataKind as Kind;
    let read = crate::peer_update_settings::canvas_read(db);
    let pattern = format!("{}%", crate::online_migration::PROGRESS_PREFIX);
    read.collect(
        &OnlineRows::QUERY,
        &OnlineRows {
            kind: Kind::PayloadMigrations,
            pattern: Some(&pattern),
        },
    )?
    .into_iter()
    .map(|row| {
        let name = row
            .text_key
            .ok_or(studio_db_api::rusqlite::Error::InvalidQuery)?;
        let dirty = read
            .named(
                &DirtyCount::QUERY,
                &DirtyCount {
                    kind: Kind::OnlineMigrationDirty,
                    migration: Some(&name),
                },
            )?
            .unwrap_or(0);
        Ok(crate::online_migration::Progress {
            name: name[crate::online_migration::PROGRESS_PREFIX.len()..].to_owned(),
            cursor: row
                .migration_cursor
                .ok_or(studio_db_api::rusqlite::Error::InvalidQuery)?,
            complete: row
                .migration_complete
                .ok_or(studio_db_api::rusqlite::Error::InvalidQuery)?
                != 0,
            status: row
                .migration_status
                .ok_or(studio_db_api::rusqlite::Error::InvalidQuery)?
                .as_str()
                .to_owned(),
            updated: row.migration_updated,
            error: row.migration_error,
            dirty,
        })
    })
    .collect()
}

pub(crate) fn dirty(
    read: &Read<'_, Canvas>,
    name: &str,
    limit: i64,
) -> StoreResult<Vec<studio_db_api::migration::DirtyKey>> {
    use crate::metadata_schema::OnlineMigrationDirty as Current;
    use crate::settings_schema::OnlineMigrationDirty as Legacy;
    if read.table_exists::<Source>()? {
        return Ok(studio_db_api::db_query!(
            read,
            Legacy,
            filter(Legacy::MIGRATION.eq_ref(name)),
            order_by(Legacy::KEY.asc),
            limit(limit)
        )?
        .into_iter()
        .map(|row| row.key)
        .collect());
    }
    Ok(studio_db_api::db_query!(
        read,
        Current,
        filter(Current::DIRTY_MIGRATION.eq(&name.to_owned())),
        order_by(Current::DIRTY_KEY.asc),
        limit(limit)
    )?
    .into_iter()
    .map(|row| row.dirty_key)
    .collect())
}

pub(crate) fn clear_dirty(
    tx: &mut Tx<'_, Canvas>,
    name: &str,
    key: &studio_db_api::migration::DirtyKey,
) -> StoreResult<()> {
    if tx.read().table_exists::<Source>()? {
        tx.delete::<crate::settings_schema::OnlineMigrationDirty>(&(name.to_owned(), key.clone()))?;
    } else {
        tx.delete::<crate::metadata_schema::OnlineMigrationDirty>(&(
            studio_db_api::migration::encode_key(&(name.to_owned(), key.clone())),
        ))?;
    }
    Ok(())
}

/// Only runner-owned fields; staged historical ledgers need not have cutover columns.
#[derive(studio_db_api::DbProjection)]
#[db(of=Source)]
pub(crate) struct State {
    pub name: String,
    pub cursor: i64,
    pub complete: i64,
    pub status: String,
    pub updated: Option<f64>,
    pub error: Option<String>,
    pub cursor_key: Option<Vec<u8>>,
    pub copy_source_rows: Option<i64>,
    pub copy_target_rows: Option<i64>,
}

/// Admission of historical ledgers needs only the two columns present before online keys.
#[derive(studio_db_api::DbProjection)]
#[db(of=Source)]
struct Completion {
    complete: i64,
    status: String,
}

pub(crate) fn completion(
    read: &Read<'_, Canvas>,
    name: &str,
) -> StoreResult<Option<(i64, String)>> {
    if read.table_exists::<Source>()? {
        return Ok(studio_db_api::db_query!(
            read,
            Source,
            project(Completion),
            filter(Source::NAME.eq_ref(name))
        )?
        .into_iter()
        .next()
        .map(|row| (row.complete, row.status)));
    }
    Ok(load(read, name)?.map(|row| (row.complete, row.status)))
}

pub(crate) fn load(read: &Read<'_, Canvas>, name: &str) -> StoreResult<Option<State>> {
    if read.table_exists::<Source>()? {
        return Ok(studio_db_api::db_query!(
            read,
            Source,
            project(State),
            filter(Source::NAME.eq_ref(name))
        )?
        .into_iter()
        .next());
    }
    let key = Some(name.to_owned());
    let identity = crate::consolidation_codec::nullable_text_identity(&key, 0);
    Ok(read.get::<Target>(&(identity,))?.map(|row| State {
        name: name.to_owned(),
        cursor: row.migration_cursor,
        complete: row.migration_complete.as_stored(),
        status: row.migration_status.as_str().to_owned(),
        updated: row.migration_updated,
        error: row.migration_error,
        cursor_key: row.migration_cursor_key,
        copy_source_rows: row.migration_source_rows,
        copy_target_rows: row.migration_target_rows,
    }))
}

pub(crate) fn save(tx: &mut Tx<'_, Canvas>, row: State) -> StoreResult<()> {
    if tx.read().table_exists::<Source>()? {
        tx.named(
            &SaveLegacy::QUERY,
            &SaveLegacy {
                name: &row.name,
                cursor: row.cursor,
                complete: row.complete,
                status: &row.status,
                updated: row.updated,
                error: row.error.as_deref(),
                cursor_key: row.cursor_key,
                copy_source_rows: row.copy_source_rows,
                copy_target_rows: row.copy_target_rows,
            },
        )?;
        return Ok(());
    }
    let key = Some(row.name);
    let identity = crate::consolidation_codec::nullable_text_identity(&key, 0);
    let previous = tx.read().get::<Target>(&(identity.clone(),))?;
    let cutover_ms = previous.as_ref().and_then(|row| row.migration_cutover_ms);
    let source_rowid = match previous {
        Some(previous) => previous.source_rowid,
        None => tx
            .read()
            .named(
                &crate::clock_schema::queries::MetaRowidHigh::QUERY,
                &crate::clock_schema::queries::MetaRowidHigh {
                    kind: crate::consolidation_kinds::CanvasMetadataKind::PayloadMigrations,
                },
            )?
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(StoreError::RecordContract(
                "migration progress rowid exhausted",
            ))?,
    };
    tx.partition::<Target>().put(&Target {
        identity,
        source_rowid,
        text_key: key,
        migration_cursor: row.cursor,
        migration_complete: StoredBoolean::from_stored(row.complete),
        migration_status: row
            .status
            .parse()
            .map_err(|_| StoreError::RecordContract("invalid migration progress status"))?,
        migration_updated: row.updated,
        migration_error: row.error,
        migration_cutover_ms: cutover_ms,
        migration_cursor_key: row.cursor_key,
        migration_source_rows: row.copy_source_rows,
        migration_target_rows: row.copy_target_rows,
    })?;
    Ok(())
}

/// Delete only the named progress record in the current transaction's authority.
/// Batch receipts and other migration records are retained.
pub(crate) fn reset(tx: &mut Tx<'_, Canvas>, name: &str) -> StoreResult<()> {
    if tx.read().table_exists::<Source>()? {
        tx.delete::<Source>(&(name.to_owned(),))?;
    } else {
        let identity =
            crate::consolidation_codec::nullable_text_identity(&Some(name.to_owned()), 0);
        tx.partition::<Target>().delete(&(identity,))?;
    }
    Ok(())
}

#[derive(studio_db_api::DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(), tables(rows=Source),
    columns(name=Source::NAME,cursor=Source::CURSOR,complete=Source::COMPLETE,status=Source::STATUS,updated=Source::UPDATED,error=Source::ERROR,cursor_key=Source::CURSOR_KEY,copy_source_rows=Source::COPY_SOURCE_ROWS,copy_target_rows=Source::COPY_TARGET_ROWS),
    sql="INSERT INTO {rows}({name},{cursor},{complete},{status},{updated},{error},{cursor_key},{copy_source_rows},{copy_target_rows}) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT({name}) DO UPDATE SET {cursor}=excluded.{cursor},{complete}=excluded.{complete},{status}=excluded.{status},{updated}=excluded.{updated},{error}=excluded.{error},{cursor_key}=excluded.{cursor_key},{copy_source_rows}=excluded.{copy_source_rows},{copy_target_rows}=excluded.{copy_target_rows}")]
struct SaveLegacy<'a> {
    #[db(column=Source::NAME)]
    name: &'a str,
    #[db(column=Source::CURSOR)]
    cursor: i64,
    #[db(column=Source::COMPLETE)]
    complete: i64,
    #[db(column=Source::STATUS)]
    status: &'a str,
    #[db(column=Source::UPDATED)]
    updated: Option<f64>,
    #[db(column=Source::ERROR)]
    error: Option<&'a str>,
    #[db(column=Source::CURSOR_KEY)]
    cursor_key: Option<Vec<u8>>,
    #[db(column=Source::COPY_SOURCE_ROWS)]
    copy_source_rows: Option<i64>,
    #[db(column=Source::COPY_TARGET_ROWS)]
    copy_target_rows: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::Table;

    fn row(cursor: i64) -> State {
        State {
            name: "online:authority_witness".into(),
            cursor,
            complete: 0,
            status: "running".into(),
            updated: Some(1.25),
            error: Some("held".into()),
            cursor_key: Some(vec![0, 2]),
            copy_source_rows: Some(3),
            copy_target_rows: Some(2),
        }
    }

    #[test]
    fn reset_removes_only_named_progress_from_current_authority_and_rolls_back() {
        for legacy in [false, true] {
            let mut db = studio_db_api::rusqlite::Connection::open_in_memory().unwrap();
            Target::descriptor().install_if_absent(&db).unwrap();
            {
                let transaction = db.transaction().unwrap();
                save(
                    &mut crate::peer_update_settings::canvas_tx(&transaction),
                    row(10),
                )
                .unwrap();
                transaction.commit().unwrap();
            }
            if legacy {
                Source::descriptor().install_if_absent(&db).unwrap();
            }
            {
                let transaction = db.transaction().unwrap();
                let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
                if legacy {
                    save(&mut tx, row(20)).unwrap();
                }
                let mut unrelated = row(30);
                unrelated.name = "unrelated-progress".into();
                save(&mut tx, unrelated).unwrap();
                transaction.commit().unwrap();
            }
            {
                let transaction = db.transaction().unwrap();
                let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
                reset(&mut tx, &row(0).name).unwrap();
                assert!(load(&tx.read(), &row(0).name).unwrap().is_none());
                assert_eq!(
                    load(&tx.read(), "unrelated-progress")
                        .unwrap()
                        .unwrap()
                        .cursor,
                    30
                );
                if legacy {
                    let identity =
                        crate::consolidation_codec::nullable_text_identity(&Some(row(0).name), 0);
                    assert_eq!(
                        tx.read()
                            .get::<Target>(&(identity,))
                            .unwrap()
                            .unwrap()
                            .migration_cursor,
                        10
                    );
                }
            }
            let read = crate::peer_update_settings::canvas_read(&db);
            assert_eq!(
                load(&read, &row(0).name).unwrap().unwrap().cursor,
                if legacy { 20 } else { 10 }
            );
        }
    }

    #[test]
    fn progress_endpoint_switches_ledger_and_dirty_counts_in_one_snapshot() {
        use crate::metadata_schema::OnlineMigrationDirty as CurrentDirty;
        use crate::settings_schema::OnlineMigrationDirty as LegacyDirty;
        use studio_db_api::migration::{DirtyKey, encode_key};
        let mut db = studio_db_api::rusqlite::Connection::open_in_memory().unwrap();
        Target::descriptor().install_if_absent(&db).unwrap();
        let name = row(0).name;
        {
            let transaction = db.transaction().unwrap();
            let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
            save(&mut tx, row(10)).unwrap();
            for key in [DirtyKey::Integer(1), DirtyKey::Composite(vec![1, 2])] {
                tx.partition::<CurrentDirty>()
                    .put(&CurrentDirty {
                        identity: encode_key(&(name.clone(), key.clone())),
                        dirty_migration: name.clone(),
                        dirty_key: key,
                    })
                    .unwrap();
            }
            transaction.commit().unwrap();
        }
        Source::descriptor().install_if_absent(&db).unwrap();
        LegacyDirty::descriptor().install_if_absent(&db).unwrap();
        {
            let transaction = db.transaction().unwrap();
            let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
            save(&mut tx, row(20)).unwrap();
            tx.insert(&LegacyDirty {
                migration: name,
                key: DirtyKey::Integer(7),
            })
            .unwrap();
            transaction.commit().unwrap();
        }
        let transaction = db.transaction().unwrap();
        let before = crate::online_migration::progress(&transaction).unwrap();
        assert_eq!((before[0].cursor, before[0].dirty), (20, 1));
        let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
        studio_db_api::migration::retire_source::<Source>(&mut tx).unwrap();
        studio_db_api::migration::retire_source::<LegacyDirty>(&mut tx).unwrap();
        let after = crate::online_migration::progress(&transaction).unwrap();
        assert_eq!((after[0].cursor, after[0].dirty), (10, 2));
        assert_eq!(after[0].name, "authority_witness");
        transaction.rollback().unwrap();
        let restored = crate::online_migration::progress(&db).unwrap();
        assert_eq!((restored[0].cursor, restored[0].dirty), (20, 1));
    }

    #[test]
    fn completed_begin_uses_new_ledger_without_recreating_retired_source() {
        use crate::online_migration::OnlineMigration;
        let mut db = studio_db_api::rusqlite::Connection::open_in_memory().unwrap();
        Target::descriptor().install_if_absent(&db).unwrap();
        let migration = crate::item_seq_backfill::ItemSeqBackfill;
        let mut completed = row(9);
        completed.name = format!(
            "{}{}",
            crate::online_migration::PROGRESS_PREFIX,
            migration.name()
        );
        completed.complete = 1;
        completed.status = "complete".into();
        let name = completed.name.clone();
        let transaction = db.transaction().unwrap();
        save(
            &mut crate::peer_update_settings::canvas_tx(&transaction),
            completed,
        )
        .unwrap();
        crate::online_migration::begin(&transaction, &migration).unwrap();
        let read = crate::peer_update_settings::canvas_read(&transaction);
        assert!(!read.table_exists::<Source>().unwrap());
        assert_eq!(load(&read, &name).unwrap().unwrap().complete, 1);
        transaction.commit().unwrap();
    }

    #[test]
    fn authority_switches_inside_retirement_and_rollback_restores_it() {
        let mut db = studio_db_api::rusqlite::Connection::open_in_memory().unwrap();
        Target::descriptor().install_if_absent(&db).unwrap();
        // Target presence alone is not authority: it may contain an older copy.
        {
            let transaction = db.transaction().unwrap();
            save(
                &mut crate::peer_update_settings::canvas_tx(&transaction),
                row(10),
            )
            .unwrap();
            transaction.commit().unwrap();
        }
        Source::descriptor().install_if_absent(&db).unwrap();
        {
            let transaction = db.transaction().unwrap();
            let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
            save(&mut tx, row(20)).unwrap();
            transaction.commit().unwrap();
        }
        {
            let transaction = db.transaction().unwrap();
            let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
            assert_eq!(load(&tx.read(), &row(0).name).unwrap().unwrap().cursor, 20);
            studio_db_api::migration::retire_source::<Source>(&mut tx).unwrap();
            assert_eq!(load(&tx.read(), &row(0).name).unwrap().unwrap().cursor, 10);
            save(&mut tx, row(30)).unwrap();
            let actual = load(&tx.read(), &row(0).name).unwrap().unwrap();
            assert_eq!(actual.cursor, 30);
            assert_eq!(actual.cursor_key, Some(vec![0, 2]));
            assert_eq!(actual.copy_source_rows, Some(3));
            assert_eq!(actual.copy_target_rows, Some(2));
            assert_eq!(actual.error.as_deref(), Some("held"));
            // Rollback restores both schema authority and each ledger's old contents.
        }
        assert_eq!(
            load(&crate::peer_update_settings::canvas_read(&db), &row(0).name)
                .unwrap()
                .unwrap()
                .cursor,
            20
        );
        {
            let transaction = db.transaction().unwrap();
            let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
            studio_db_api::migration::retire_source::<Source>(&mut tx).unwrap();
            save(&mut tx, row(40)).unwrap();
            transaction.commit().unwrap();
        }
        assert_eq!(
            load(&crate::peer_update_settings::canvas_read(&db), &row(0).name)
                .unwrap()
                .unwrap()
                .cursor,
            40
        );
    }
}
