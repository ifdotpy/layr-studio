//! Conversion of the values in an owned stored-text column between plain TEXT
//! and stored format 1, in short resumable batches.
//!
//! [`Direction::Compress`] is the background pass. [`Direction::Inflate`] is
//! its reverse for an operator who must return the state to a release that
//! reads only plain TEXT. A reader scans one bounded rowid window and converts
//! the values outside the writer; each converted value must decode to the
//! exact original text before it is used. The writer batch then replaces a
//! value only when the row still holds the scanned value (compare and set),
//! reads every replaced value back inside the same transaction, and commits
//! only when the read-back values are the converted values and decode to the
//! originals. The batch records a SHA-256 of the original texts in
//! the database's batch receipts. Nothing is deleted: a value is replaced only
//! by another form of the same text.

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use studio_db_api::{DbQuery, Table};

use crate::consolidation_kinds::CanvasMetadataKind;
use crate::metadata_schema::{Metadata, StoredTextBatches};
use crate::migration_progress;
use crate::peer_update_settings::{Canvas, canvas_read, canvas_tx};
use crate::stored_text;
use crate::{CachedSql, StoreError, StoreResult};

/// A column whose values the conversion passes rewrite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactedColumn {
    /// Database that owns the values and their compaction metadata.
    pub database: CompactionDatabase,
    /// Table that holds the column.
    pub table: &'static str,
    /// Unique text key of a row, compared before a value is replaced.
    pub key: &'static str,
    /// The owned stored-text column.
    pub column: &'static str,
    /// Name of the compaction progress and its batch receipts, in the
    /// database that owns the column.
    pub progress: &'static str,
    /// Name of the column's trained dictionary in
    /// `analytics_stored_text_dictionaries`, or `None` for frames without one.
    pub dictionary: Option<&'static str>,
}

/// Declared database ownership of a compacted column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionDatabase {
    Canvas,
    Analytics,
}

/// `runtime_item_fulltext.body`, owned by [`stored_text`].
pub const FULLTEXT_BODY: CompactedColumn = CompactedColumn {
    database: CompactionDatabase::Canvas,
    table: "runtime_item_fulltext",
    key: "id",
    column: "body",
    progress: "stored-text-v1:runtime_item_fulltext.body",
    dictionary: None,
};

/// `analytics_items.record` in `analytics.sqlite3`, compressed with its
/// trained dictionary. `main.` skips the decoding view of the same name.
pub const ANALYTICS_ITEMS_RECORD: CompactedColumn = CompactedColumn {
    database: CompactionDatabase::Analytics,
    table: "main.analytics_items",
    key: "id",
    column: "record",
    progress: "stored-text-v1:analytics_items.record",
    dictionary: Some(crate::stored_text_dictionary::ANALYTICS_ITEMS_RECORD),
};

/// Legacy Analytics batch records of the compaction and inflate passes.
pub(crate) fn prepare_batches(connection: &Connection) -> rusqlite::Result<()> {
    crate::settings_schema::StoredTextBatches::descriptor().install_if_absent(connection)
}

/// Canvas batch receipts share the canonical metadata target.
pub(crate) fn prepare_canvas_batches(connection: &Connection) -> rusqlite::Result<()> {
    Metadata::descriptor().install_if_absent(connection)
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=Option<i64>,decode=Metadata::BATCH,
    tables(rows=Metadata),columns(batch=Metadata::BATCH,kind=Metadata::KIND,name=Metadata::TEXT_KEY),
    sql="SELECT MAX({batch}) AS {batch} FROM {rows} WHERE {kind}=?1 AND {name}=?2")]
struct BatchHigh<'a> {
    #[db(column=Metadata::KIND)]
    kind: CanvasMetadataKind,
    #[db(column=Metadata::TEXT_KEY)]
    name: Option<&'a str>,
}

/// Rowids that one reader window inspects. Unit: rows.
pub const COMPACTION_WINDOW_ROWS: i64 = 512;
/// Values that one batch converts at most. Unit: rows.
pub const COMPACTION_BATCH_ROWS: usize = 64;
/// Text bytes that one batch converts at most; a single larger value is
/// still converted alone. Unit: bytes.
pub const COMPACTION_BATCH_BYTES: usize = 8 * 1024 * 1024;

/// Which form a pass writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// Plain TEXT to stored format 1.
    Compress,
    /// Stored format 1 to plain TEXT.
    Inflate,
}

impl Direction {
    fn source_type(self) -> &'static str {
        match self {
            Self::Compress => "text",
            Self::Inflate => "blob",
        }
    }

    fn batch_name(self, column: CompactedColumn) -> String {
        match self {
            Self::Compress => column.progress.to_owned(),
            Self::Inflate => format!("{}:inflate", column.progress),
        }
    }
}

/// Saved progress of the compaction pass of one column.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompactionProgress {
    /// The next scan starts after this rowid.
    pub cursor: i64,
    /// True when one complete pass reached the end of the table.
    pub complete: bool,
}

/// One scanned value, converted and checked in memory.
#[derive(Clone, Debug)]
pub struct CompactionCandidate {
    rowid: i64,
    key: String,
    text: String,
    from: SqlValue,
    to: SqlValue,
}

/// The values of one reader window that a pass converts.
#[derive(Clone, Debug)]
pub struct CompactionWindow {
    pub direction: Direction,
    pub candidates: Vec<CompactionCandidate>,
    /// The next window starts after this rowid.
    pub next_cursor: i64,
    /// True when the window reached the end of the table.
    pub done: bool,
}

/// Result of one writer batch.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompactionBatch {
    pub converted: usize,
    /// Rows that changed after the scan and were not replaced.
    pub skipped: usize,
    /// Bytes of the converted texts.
    pub plain_bytes: u64,
    /// Bytes of their compressed forms.
    pub stored_bytes: u64,
}

pub fn progress(
    connection: &Connection,
    column: CompactedColumn,
) -> StoreResult<CompactionProgress> {
    if column.database == CompactionDatabase::Canvas {
        return Ok(
            migration_progress::load(&canvas_read(connection), column.progress)?
                .map(|row| CompactionProgress {
                    cursor: row.cursor,
                    complete: row.complete != 0,
                })
                .unwrap_or_default(),
        );
    }
    Ok(connection
        .query_row_cached(
            "SELECT cursor,complete FROM runtime_payload_migrations WHERE name=?1",
            [column.progress],
            |row| {
                Ok(CompactionProgress {
                    cursor: row.get(0)?,
                    complete: row.get::<_, i64>(1)? != 0,
                })
            },
        )
        .optional()?
        .unwrap_or_default())
}

/// Read and convert the values after `cursor` that `direction` rewrites,
/// bounded by [`COMPACTION_WINDOW_ROWS`], [`COMPACTION_BATCH_ROWS`] and
/// [`COMPACTION_BATCH_BYTES`].
pub fn scan_window(
    connection: &Connection,
    column: CompactedColumn,
    direction: Direction,
    cursor: i64,
) -> StoreResult<CompactionWindow> {
    let end = cursor.saturating_add(COMPACTION_WINDOW_ROWS);
    let last: Option<i64> = connection.query_row_cached(
        &format!("SELECT max(rowid) FROM {}", column.table),
        [],
        |row| row.get(0),
    )?;
    let mut statement = connection.prepare_cached(&format!(
        "SELECT rowid,{key},{column} FROM {table} \
         WHERE rowid>?1 AND rowid<=?2 AND typeof({column})=?3 ORDER BY rowid",
        key = column.key,
        column = column.column,
        table = column.table,
    ))?;
    let dictionary = match (direction, column.dictionary) {
        (Direction::Compress, Some(name)) => Some(
            crate::stored_text_dictionary::current(connection, name)?.ok_or_else(|| {
                StoreError::InvalidStoredObject {
                    table: "analytics_stored_text_dictionaries",
                    id: name.to_owned(),
                }
            })?,
        ),
        _ => None,
    };
    let mut rows = statement.query(params![cursor, end, direction.source_type()])?;
    let mut window = CompactionWindow {
        direction,
        candidates: Vec::new(),
        next_cursor: end,
        done: false,
    };
    let mut bytes = 0_usize;
    while let Some(row) = rows.next()? {
        let rowid: i64 = row.get(0)?;
        if window.candidates.len() == COMPACTION_BATCH_ROWS
            || (!window.candidates.is_empty() && bytes >= COMPACTION_BATCH_BYTES)
        {
            // Resume at this row in the next batch.
            window.next_cursor = rowid - 1;
            return Ok(window);
        }
        let key: String = row.get(1)?;
        let candidate = match direction {
            Direction::Compress => {
                let text: String = row.get(2)?;
                if !window.candidates.is_empty()
                    && bytes.saturating_add(text.len()) > COMPACTION_BATCH_BYTES
                {
                    window.next_cursor = rowid - 1;
                    return Ok(window);
                }
                let stored = match dictionary {
                    Some(dictionary) => {
                        crate::stored_text_dictionary::encode_with(&text, dictionary)
                    }
                    None => stored_text::encode(&text),
                };
                if stored_text::decode_blob(&stored).as_deref() != Ok(text.as_str()) {
                    return Err(round_trip_failure(column, &key));
                }
                CompactionCandidate {
                    rowid,
                    key,
                    from: SqlValue::Text(text.clone()),
                    to: SqlValue::Blob(stored),
                    text,
                }
            }
            Direction::Inflate => {
                let stored: Vec<u8> = row.get(2)?;
                let text = stored_text::decode_blob(&stored)
                    .map_err(|_| round_trip_failure(column, &key))?;
                if !window.candidates.is_empty()
                    && bytes.saturating_add(text.len()) > COMPACTION_BATCH_BYTES
                {
                    window.next_cursor = rowid - 1;
                    return Ok(window);
                }
                CompactionCandidate {
                    rowid,
                    key,
                    from: SqlValue::Blob(stored),
                    to: SqlValue::Text(text.clone()),
                    text,
                }
            }
        };
        bytes += candidate.text.len();
        window.candidates.push(candidate);
    }
    window.done = last.is_none_or(|last| end >= last);
    Ok(window)
}

/// Replace the scanned values that are unchanged, verify every replaced value
/// by reading it back, record the batch hash, and save compaction progress.
/// Any verification failure returns an error, so the caller's transaction
/// rolls back and no value changes.
pub fn apply_window(
    transaction: &Transaction<'_>,
    column: CompactedColumn,
    window: &CompactionWindow,
    now: f64,
) -> StoreResult<CompactionBatch> {
    let replace = format!(
        "UPDATE {table} SET {column}=?1 WHERE rowid=?2 AND {key}=?3 AND {column}=?4",
        table = column.table,
        column = column.column,
        key = column.key,
    );
    let read_back = format!(
        "SELECT {column} FROM {table} WHERE rowid=?1",
        table = column.table,
        column = column.column,
    );
    let mut batch = CompactionBatch::default();
    let mut originals = Sha256::new();
    let mut first = None;
    let mut last = None;
    for candidate in &window.candidates {
        let copied = studio_db_api::migration::copy_verified(
            || {
                Ok(transaction.execute_cached(
                    &replace,
                    params![candidate.to, candidate.rowid, candidate.key, candidate.from],
                )? != 0)
            },
            || {
                let stored: SqlValue = transaction
                    .query_row_cached(&read_back, [candidate.rowid], |row| row.get(0))
                    .map_err(|_| round_trip_failure(column, &candidate.key))?;
                let text = match &stored {
                    SqlValue::Text(text) => text.clone(),
                    SqlValue::Blob(bytes) => stored_text::decode_blob(bytes)
                        .map_err(|_| round_trip_failure(column, &candidate.key))?,
                    SqlValue::Null | SqlValue::Integer(_) | SqlValue::Real(_) => {
                        return Err(round_trip_failure(column, &candidate.key));
                    }
                };
                Ok(stored == candidate.to && text == candidate.text)
            },
            || round_trip_failure(column, &candidate.key),
        )?;
        if !copied {
            batch.skipped += 1;
            continue;
        }
        hash_value(&mut originals, candidate.rowid, candidate.text.as_bytes());
        first.get_or_insert(candidate.rowid);
        last = Some(candidate.rowid);
        batch.converted += 1;
        batch.plain_bytes += candidate.text.len() as u64;
        batch.stored_bytes += match (&candidate.from, &candidate.to) {
            (_, SqlValue::Blob(bytes)) | (SqlValue::Blob(bytes), _) => bytes.len() as u64,
            _ => 0,
        };
    }
    if let (Some(first), Some(last)) = (first, last) {
        let originals = data_encoding::HEXLOWER.encode(&originals.finalize());
        if column.database == CompactionDatabase::Canvas {
            let mut tx = canvas_tx(transaction);
            let name = window.direction.batch_name(column);
            let kind = CanvasMetadataKind::StoredTextBatches;
            let ordinal = tx
                .read()
                .named(
                    &BatchHigh::QUERY,
                    &BatchHigh {
                        kind,
                        name: Some(&name),
                    },
                )?
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(StoreError::RecordContract(
                    "stored-text batch ordinal exhausted",
                ))?;
            let source_rowid = tx
                .read()
                .named(
                    &crate::clock_schema::queries::MetaRowidHigh::QUERY,
                    &crate::clock_schema::queries::MetaRowidHigh { kind },
                )?
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(StoreError::RecordContract(
                    "stored-text batch rowid exhausted",
                ))?;
            tx.partition::<StoredTextBatches>()
                .put(&StoredTextBatches {
                    identity: studio_db_api::migration::encode_key(&(name.clone(), ordinal)),
                    source_rowid,
                    text_key: name,
                    batch: ordinal,
                    first_rowid: first,
                    last_rowid: last,
                    batch_rows: batch.converted as i64,
                    plain_bytes: batch.plain_bytes as i64,
                    stored_bytes: batch.stored_bytes as i64,
                    sha256: originals,
                    batch_created: now,
                })?;
        } else {
            transaction.execute_cached(
            "INSERT INTO runtime_stored_text_batches\
             (name,batch,first_rowid,last_rowid,rows,plain_bytes,stored_bytes,sha256,created) \
             VALUES(?1,(SELECT coalesce(max(batch),0)+1 FROM runtime_stored_text_batches WHERE name=?1),\
             ?2,?3,?4,?5,?6,?7,?8)",
            params![
                window.direction.batch_name(column),
                first,
                last,
                batch.converted as i64,
                batch.plain_bytes as i64,
                batch.stored_bytes as i64,
                originals,
                now
            ],
        )?;
        }
    }
    if window.direction == Direction::Compress {
        save_progress(
            transaction,
            column,
            window.next_cursor,
            false,
            "running",
            now,
        )?;
    }
    Ok(batch)
}

/// Check for plain rows outside the writer transaction. The caller saves this
/// result in a short transaction. Supported writers store compressed values,
/// so they cannot introduce a plain value after this check.
pub fn completion_progress(
    connection: &Connection,
    column: CompactedColumn,
    cursor: i64,
) -> StoreResult<CompactionProgress> {
    let remaining: Option<i64> = connection
        .query_row_cached(
            &format!(
                "SELECT rowid FROM {} WHERE typeof({})='text' ORDER BY rowid LIMIT 1",
                column.table, column.column
            ),
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(match remaining {
        Some(rowid) => CompactionProgress {
            cursor: rowid.saturating_sub(1),
            complete: false,
        },
        None => CompactionProgress {
            cursor,
            complete: true,
        },
    })
}

/// Save progress. Only a result from [`completion_progress`] can mark a pass
/// complete. The final scan must not hold the serialized writer.
pub fn save_progress(
    transaction: &Transaction<'_>,
    column: CompactedColumn,
    cursor: i64,
    done: bool,
    status: &str,
    now: f64,
) -> StoreResult<()> {
    let status = if done { "complete" } else { status };
    if column.database == CompactionDatabase::Canvas {
        let mut row = canvas_progress(transaction, column)?;
        row.cursor = cursor;
        row.complete = i64::from(done);
        row.status = status.into();
        row.updated = Some(now);
        row.error = None;
        return migration_progress::save(&mut canvas_tx(transaction), row);
    }
    transaction.execute_cached(
        "INSERT INTO runtime_payload_migrations(name,cursor,complete,status,updated,error) \
         VALUES(?1,?2,?3,?4,?5,NULL) ON CONFLICT(name) DO UPDATE SET \
         cursor=excluded.cursor,complete=excluded.complete,status=excluded.status,\
         updated=excluded.updated,error=NULL",
        params![column.progress, cursor, i64::from(done), status, now],
    )?;
    Ok(())
}

/// Forget compaction progress, so that a later compaction pass inspects the
/// whole column again. An inflate pass ends with this.
pub fn reset_progress(transaction: &Transaction<'_>, column: CompactedColumn) -> StoreResult<()> {
    if column.database == CompactionDatabase::Canvas {
        return migration_progress::reset(&mut canvas_tx(transaction), column.progress);
    }
    transaction.execute_cached(
        "DELETE FROM runtime_payload_migrations WHERE name=?1",
        [column.progress],
    )?;
    Ok(())
}

/// Keep the saved cursor and record why the compaction pass stopped.
pub fn record_error(
    transaction: &Transaction<'_>,
    column: CompactedColumn,
    message: &str,
    now: f64,
) -> StoreResult<()> {
    if column.database == CompactionDatabase::Canvas {
        let mut row = canvas_progress(transaction, column)?;
        row.status = "error".into();
        row.updated = Some(now);
        row.error = Some(message.into());
        return migration_progress::save(&mut canvas_tx(transaction), row);
    }
    transaction.execute_cached(
        "INSERT INTO runtime_payload_migrations(name,cursor,complete,status,updated,error) \
         VALUES(?1,0,0,'error',?2,?3) ON CONFLICT(name) DO UPDATE SET \
         status='error',updated=excluded.updated,error=excluded.error",
        params![column.progress, now, message],
    )?;
    Ok(())
}

fn canvas_progress(
    connection: &Connection,
    column: CompactedColumn,
) -> StoreResult<migration_progress::State> {
    Ok(
        migration_progress::load(&canvas_read(connection), column.progress)?.unwrap_or_else(|| {
            migration_progress::State {
                name: column.progress.into(),
                cursor: 0,
                complete: 0,
                status: "pending".into(),
                updated: None,
                error: None,
                cursor_key: None,
                copy_source_rows: None,
                copy_target_rows: None,
            }
        }),
    )
}

/// One value in a batch hash: rowid and byte length as little-endian 64-bit
/// integers, then the text bytes.
fn hash_value(hash: &mut Sha256, rowid: i64, bytes: &[u8]) {
    hash.update(rowid.to_le_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn round_trip_failure(column: CompactedColumn, key: &str) -> StoreError {
    StoreError::InvalidStoredObject {
        table: column.table,
        id: key.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_schema::StoredTextBatches;
    use crate::peer_update_settings::canvas_read;
    use crate::stored_text::StoredText;
    use studio_db_api::Table;
    use studio_db_api::migration::encode_key;

    const NOW: f64 = 2_000_000_000.0;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().expect("database");
        prepare_canvas_batches(&connection).expect("canonical metadata target");
        crate::item_monitor_terminal::ItemFulltextRow::descriptor()
            .install_if_absent(&connection)
            .expect("column");
        connection
    }

    #[test]
    fn canvas_target_defaults_and_batch_rollback_are_atomic() {
        let mut connection = database();
        assert_eq!(
            progress(&connection, FULLTEXT_BODY).expect("absent progress"),
            CompactionProgress::default()
        );
        insert(&connection, "a", SqlValue::Text(body(1)));
        let window = scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("scan");
        {
            let transaction = connection.transaction().expect("transaction");
            apply_window(&transaction, FULLTEXT_BODY, &window, NOW).expect("apply");
            assert_eq!(stored(&transaction, "a"), ("blob".into(), body(1)));
            let receipt = canvas_read(&transaction)
                .get::<StoredTextBatches>(&(encode_key(&(
                    FULLTEXT_BODY.progress.to_owned(),
                    1_i64,
                )),))
                .expect("receipt read")
                .expect("receipt");
            assert_eq!(receipt.sha256, expected_hash(&[(1, &body(1))]));
            assert_eq!(
                progress(&transaction, FULLTEXT_BODY)
                    .expect("progress")
                    .cursor,
                window.next_cursor
            );
            transaction.rollback().expect("rollback");
        }
        assert_eq!(stored(&connection, "a"), ("text".into(), body(1)));
        assert!(
            canvas_read(&connection)
                .get::<StoredTextBatches>(&(encode_key(&(
                    FULLTEXT_BODY.progress.to_owned(),
                    1_i64
                )),))
                .expect("receipt read")
                .is_none()
        );
        assert_eq!(
            progress(&connection, FULLTEXT_BODY).expect("rolled back progress"),
            CompactionProgress::default()
        );
    }

    #[test]
    fn canvas_error_and_reset_preserve_unrelated_progress_and_receipts() {
        use crate::migration_progress::{self, State};
        use crate::peer_update_settings::canvas_tx;
        for legacy in [false, true] {
            let mut connection = database();
            if legacy {
                crate::settings_schema::PayloadMigrations::descriptor()
                    .install_if_absent(&connection)
                    .expect("legacy authority");
            }
            insert(&connection, "a", SqlValue::Text(body(1)));
            let transaction = connection.transaction().expect("transaction");
            let mut tx = canvas_tx(&transaction);
            for name in [FULLTEXT_BODY.progress, "unrelated"] {
                migration_progress::save(
                    &mut tx,
                    State {
                        name: name.into(),
                        cursor: 17,
                        complete: 2,
                        status: "complete".into(),
                        updated: Some(1.25),
                        error: Some("old".into()),
                        cursor_key: Some(vec![0, 255]),
                        copy_source_rows: Some(9),
                        copy_target_rows: Some(8),
                    },
                )
                .expect("seed progress");
            }
            record_error(&transaction, FULLTEXT_BODY, "failed", NOW).expect("error");
            let row = migration_progress::load(&tx.read(), FULLTEXT_BODY.progress)
                .expect("read")
                .expect("row");
            assert_eq!((row.cursor, row.complete), (17, 2));
            assert_eq!(
                (row.status.as_str(), row.error.as_deref(), row.updated),
                ("error", Some("failed"), Some(NOW))
            );
            assert_eq!(
                (row.cursor_key, row.copy_source_rows, row.copy_target_rows),
                (Some(vec![0, 255]), Some(9), Some(8))
            );
            let window =
                scan_window(&transaction, FULLTEXT_BODY, Direction::Compress, 0).expect("scan");
            apply_window(&transaction, FULLTEXT_BODY, &window, NOW).expect("apply");
            let row = migration_progress::load(&tx.read(), FULLTEXT_BODY.progress)
                .expect("read")
                .expect("row");
            assert_eq!(
                (row.cursor, row.complete, row.error),
                (window.next_cursor, 0, None)
            );
            assert_eq!(row.status, "running");
            assert_eq!(
                (row.cursor_key, row.copy_source_rows, row.copy_target_rows),
                (Some(vec![0, 255]), Some(9), Some(8))
            );
            reset_progress(&transaction, FULLTEXT_BODY).expect("reset");
            assert!(
                migration_progress::load(&tx.read(), FULLTEXT_BODY.progress)
                    .expect("read")
                    .is_none()
            );
            let unrelated = migration_progress::load(&tx.read(), "unrelated")
                .expect("read")
                .expect("unrelated");
            assert_eq!(
                (
                    unrelated.cursor,
                    unrelated.complete,
                    unrelated.error.as_deref()
                ),
                (17, 2, Some("old"))
            );
            assert!(
                tx.get::<StoredTextBatches>(&(encode_key(&(
                    FULLTEXT_BODY.progress.to_owned(),
                    1_i64
                )),))
                    .expect("receipt")
                    .is_some()
            );
            record_error(&transaction, FULLTEXT_BODY, "absent", NOW).expect("absent error");
            let row = migration_progress::load(&tx.read(), FULLTEXT_BODY.progress)
                .expect("read")
                .expect("row");
            assert_eq!(
                (
                    row.cursor,
                    row.complete,
                    row.status.as_str(),
                    row.error.as_deref()
                ),
                (0, 0, "error", Some("absent"))
            );
            assert_eq!(
                (row.cursor_key, row.copy_source_rows, row.copy_target_rows),
                (None, None, None)
            );
            transaction.commit().expect("commit");
        }
    }

    fn receipt(name: &str, batch: i64, source_rowid: i64) -> StoredTextBatches {
        StoredTextBatches {
            identity: encode_key(&(name.to_owned(), batch)),
            source_rowid,
            text_key: name.into(),
            batch,
            first_rowid: 1,
            last_rowid: 1,
            batch_rows: 1,
            plain_bytes: 1,
            stored_bytes: 1,
            sha256: "historical".into(),
            batch_created: NOW,
        }
    }

    #[test]
    fn canvas_receipts_keep_per_name_ordinals_separate_from_source_rowids() {
        let mut connection = database();
        insert(&connection, "a", SqlValue::Text(body(1)));
        let window = scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("scan");
        let transaction = connection.transaction().expect("transaction");
        let mut tx = canvas_tx(&transaction);
        tx.partition::<StoredTextBatches>()
            .put(&receipt(FULLTEXT_BODY.progress, 7, 41))
            .expect("prior batch");
        tx.partition::<StoredTextBatches>()
            .put(&receipt("unrelated", 91, 42))
            .expect("unrelated batch");
        apply_window(&transaction, FULLTEXT_BODY, &window, NOW).expect("first batch");
        // Inflate and compress again in the same transaction: each name has its own ordinal.
        let inflate =
            scan_window(&transaction, FULLTEXT_BODY, Direction::Inflate, 0).expect("inflate scan");
        apply_window(&transaction, FULLTEXT_BODY, &inflate, NOW).expect("inflate");
        apply_window(&transaction, FULLTEXT_BODY, &window, NOW).expect("next batch");
        for (name, batch, source_rowid) in [
            (FULLTEXT_BODY.progress.to_owned(), 8_i64, 43),
            (format!("{}:inflate", FULLTEXT_BODY.progress), 1, 44),
            (FULLTEXT_BODY.progress.to_owned(), 9, 45),
        ] {
            let row = tx
                .get::<StoredTextBatches>(&(encode_key(&(name, batch)),))
                .expect("read")
                .expect("receipt");
            assert_eq!(row.source_rowid, source_rowid);
            assert_eq!(row.sha256, expected_hash(&[(1, &body(1))]));
        }
        let unrelated = tx
            .get::<StoredTextBatches>(&(encode_key(&("unrelated".to_owned(), 91_i64)),))
            .expect("read")
            .expect("unrelated");
        assert_eq!(
            (unrelated.source_rowid, unrelated.sha256.as_str()),
            (42, "historical")
        );
        transaction.commit().expect("commit");
    }

    #[test]
    fn canvas_progress_failure_rolls_back_converted_data_and_receipt() {
        use crate::metadata_schema::PayloadMigrations;
        let mut connection = database();
        let transaction = connection.transaction().expect("seed transaction");
        let mut tx = canvas_tx(&transaction);
        migration_progress::save(
            &mut tx,
            canvas_progress(&transaction, FULLTEXT_BODY).expect("default state"),
        )
        .expect("seed state");
        let identity = crate::consolidation_codec::nullable_text_identity(
            &Some(FULLTEXT_BODY.progress.into()),
            0,
        );
        let mut row = tx
            .get::<PayloadMigrations>(&(identity.clone(),))
            .expect("read")
            .expect("state");
        tx.partition::<PayloadMigrations>()
            .delete(&(identity,))
            .expect("delete seed");
        row.text_key = Some("unrelated".into());
        row.identity = crate::consolidation_codec::nullable_text_identity(&row.text_key, 0);
        row.source_rowid = i64::MAX;
        tx.partition::<PayloadMigrations>()
            .put(&row)
            .expect("exhaust source rowids");
        transaction.commit().expect("seed commit");
        insert(&connection, "a", SqlValue::Text(body(1)));
        let window = scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("scan");
        let transaction = connection.transaction().expect("transaction");
        assert!(matches!(
            apply_window(&transaction, FULLTEXT_BODY, &window, NOW),
            Err(StoreError::RecordContract(
                "migration progress rowid exhausted"
            ))
        ));
        assert!(
            canvas_read(&transaction)
                .get::<StoredTextBatches>(&(encode_key(&(
                    FULLTEXT_BODY.progress.to_owned(),
                    1_i64
                )),))
                .expect("uncommitted receipt")
                .is_some()
        );
        transaction.rollback().expect("rollback");
        assert_eq!(stored(&connection, "a"), ("text".into(), body(1)));
        assert!(
            studio_db_api::db_query!(canvas_read(&connection), StoredTextBatches)
                .expect("receipts")
                .is_empty()
        );
        assert_eq!(
            progress(&connection, FULLTEXT_BODY).expect("progress"),
            CompactionProgress::default()
        );
        assert!(
            migration_progress::load(&canvas_read(&connection), "unrelated")
                .expect("unrelated state")
                .is_some()
        );
    }

    #[test]
    fn canvas_receipt_allocators_check_overflow_and_roll_back_values() {
        for (name, batch, source_rowid, message) in [
            (
                "unrelated",
                1,
                i64::MAX,
                "stored-text batch rowid exhausted",
            ),
            (
                FULLTEXT_BODY.progress,
                i64::MAX,
                1,
                "stored-text batch ordinal exhausted",
            ),
        ] {
            let mut connection = database();
            let transaction = connection.transaction().expect("seed transaction");
            canvas_tx(&transaction)
                .partition::<StoredTextBatches>()
                .put(&receipt(name, batch, source_rowid))
                .expect("seed receipt");
            transaction.commit().expect("seed commit");
            insert(&connection, "a", SqlValue::Text(body(1)));
            let window =
                scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("scan");
            let transaction = connection.transaction().expect("transaction");
            assert!(
                matches!(apply_window(&transaction, FULLTEXT_BODY, &window, NOW), Err(StoreError::RecordContract(actual)) if actual == message)
            );
            transaction.rollback().expect("rollback");
            assert_eq!(stored(&connection, "a"), ("text".into(), body(1)));
            let receipts = studio_db_api::db_query!(canvas_read(&connection), StoredTextBatches)
                .expect("receipts");
            assert_eq!(receipts.len(), 1);
            assert_eq!(receipts[0].sha256, "historical");
            assert_eq!(
                progress(&connection, FULLTEXT_BODY).expect("progress"),
                CompactionProgress::default()
            );
        }
    }

    fn insert(connection: &Connection, id: &str, body: rusqlite::types::Value) {
        connection
            .execute(
                "INSERT INTO runtime_item_fulltext(id,body) VALUES(?1,?2)",
                params![id, body],
            )
            .expect("row");
    }

    fn body(id: usize) -> String {
        format!("{{\"aggregatedOutput\":\"line {id} \u{2713}\\n\"}}").repeat(1_000)
    }

    fn stored(connection: &Connection, id: &str) -> (String, String) {
        connection
            .query_row(
                "SELECT typeof(body),body FROM runtime_item_fulltext WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get::<_, StoredText>(1)?.into_string())),
            )
            .expect("stored row")
    }

    fn expected_hash(values: &[(i64, &str)]) -> String {
        let mut hash = Sha256::new();
        for (rowid, text) in values {
            hash.update(rowid.to_le_bytes());
            hash.update((text.len() as u64).to_le_bytes());
            hash.update(text.as_bytes());
        }
        data_encoding::HEXLOWER.encode(&hash.finalize())
    }

    fn finish(connection: &mut Connection, cursor: i64) {
        let saved =
            completion_progress(connection, FULLTEXT_BODY, cursor).expect("verify completion");
        let transaction = connection.transaction().expect("final transaction");
        save_progress(
            &transaction,
            FULLTEXT_BODY,
            saved.cursor,
            saved.complete,
            "running",
            NOW,
        )
        .expect("final progress");
        transaction.commit().expect("final commit");
    }

    #[test]
    fn compaction_converts_plain_values_and_records_their_hash() {
        let mut connection = database();
        let compressed = crate::stored_text::encode(&body(9));
        insert(&connection, "a", rusqlite::types::Value::Text(body(1)));
        insert(
            &connection,
            "b",
            rusqlite::types::Value::Blob(compressed.clone()),
        );
        insert(&connection, "c", rusqlite::types::Value::Text(body(3)));

        let window = scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("scan");
        assert_eq!(
            window.candidates.len(),
            2,
            "only plain values are candidates"
        );
        assert!(window.done);
        let transaction = connection.transaction().expect("transaction");
        let batch = apply_window(&transaction, FULLTEXT_BODY, &window, NOW).expect("apply");
        transaction.commit().expect("commit");

        finish(&mut connection, window.next_cursor);

        assert_eq!((batch.converted, batch.skipped), (2, 0));
        assert_eq!(batch.plain_bytes, (body(1).len() + body(3).len()) as u64);
        assert!(batch.stored_bytes * 10 < batch.plain_bytes);
        assert_eq!(stored(&connection, "a"), ("blob".to_owned(), body(1)));
        assert_eq!(stored(&connection, "c"), ("blob".to_owned(), body(3)));
        let untouched: Vec<u8> = connection
            .query_row(
                "SELECT body FROM runtime_item_fulltext WHERE id='b'",
                [],
                |row| row.get(0),
            )
            .expect("compressed row");
        assert_eq!(untouched, compressed);
        let receipt = canvas_read(&connection)
            .get::<StoredTextBatches>(&(encode_key(&(FULLTEXT_BODY.progress.to_owned(), 1_i64)),))
            .expect("batch row")
            .expect("receipt");
        assert_eq!(
            (receipt.batch_rows, receipt.first_rowid, receipt.last_rowid),
            (2, 1, 3)
        );
        assert_eq!(
            receipt.sha256,
            expected_hash(&[(1, &body(1)), (3, &body(3))])
        );
        assert_eq!(
            (receipt.plain_bytes, receipt.stored_bytes),
            (batch.plain_bytes as i64, batch.stored_bytes as i64)
        );
        assert_eq!(
            (receipt.batch, receipt.source_rowid, receipt.batch_created),
            (1, 1, NOW)
        );
        assert_eq!(
            progress(&connection, FULLTEXT_BODY).expect("progress"),
            CompactionProgress {
                cursor: COMPACTION_WINDOW_ROWS,
                complete: true
            }
        );
        let window =
            scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("rescan");
        assert!(
            window.candidates.is_empty(),
            "a repeated window converts nothing"
        );
    }

    #[test]
    fn inflate_restores_plain_text_for_an_older_release() {
        let mut connection = database();
        insert(&connection, "a", rusqlite::types::Value::Text(body(1)));
        insert(&connection, "b", rusqlite::types::Value::Text(body(2)));
        for direction in [Direction::Compress, Direction::Inflate] {
            let window = scan_window(&connection, FULLTEXT_BODY, direction, 0).expect("scan");
            assert_eq!(window.candidates.len(), 2, "{direction:?}");
            let transaction = connection.transaction().expect("transaction");
            let batch = apply_window(&transaction, FULLTEXT_BODY, &window, NOW).expect("apply");
            assert_eq!(batch.converted, 2, "{direction:?}");
            transaction.commit().expect("commit");
        }
        assert_eq!(stored(&connection, "a"), ("text".to_owned(), body(1)));
        assert_eq!(stored(&connection, "b"), ("text".to_owned(), body(2)));
        let hashes: Vec<(String, String)> = studio_db_api::db_query!(
            canvas_read(&connection),
            StoredTextBatches,
            order_by(StoredTextBatches::TEXT_KEY.asc)
        )
        .expect("batch rows")
        .into_iter()
        .map(|row| (row.text_key, row.sha256))
        .collect();
        let expected = expected_hash(&[(1, &body(1)), (2, &body(2))]);
        assert_eq!(
            hashes,
            vec![
                (FULLTEXT_BODY.progress.to_owned(), expected.clone()),
                (format!("{}:inflate", FULLTEXT_BODY.progress), expected)
            ],
            "both directions hash the same texts"
        );
        let transaction = connection.transaction().expect("transaction");
        reset_progress(&transaction, FULLTEXT_BODY).expect("reset");
        transaction.commit().expect("commit");
        assert_eq!(
            progress(&connection, FULLTEXT_BODY).expect("progress"),
            CompactionProgress::default()
        );
    }

    #[test]
    fn compaction_skips_a_value_changed_after_the_scan() {
        let mut connection = database();
        insert(&connection, "a", rusqlite::types::Value::Text(body(1)));
        let window = scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("scan");
        connection
            .execute(
                "UPDATE runtime_item_fulltext SET body=?1 WHERE id='a'",
                [body(2)],
            )
            .expect("writer changes the row");
        let transaction = connection.transaction().expect("transaction");
        let batch = apply_window(&transaction, FULLTEXT_BODY, &window, NOW).expect("apply");
        transaction.commit().expect("commit");
        assert_eq!((batch.converted, batch.skipped), (0, 1));
        assert_eq!(stored(&connection, "a"), ("text".to_owned(), body(2)));
        let batches = studio_db_api::db_query!(canvas_read(&connection), StoredTextBatches)
            .expect("batch rows");
        assert!(batches.is_empty());
        finish(&mut connection, window.next_cursor);
        assert_eq!(
            progress(&connection, FULLTEXT_BODY).expect("progress"),
            CompactionProgress::default(),
            "a skipped plain row must restart the pass instead of marking it complete"
        );
        let retry =
            scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("retry scan");
        let transaction = connection.transaction().expect("retry transaction");
        apply_window(&transaction, FULLTEXT_BODY, &retry, NOW).expect("retry");
        transaction.commit().expect("retry commit");
        finish(&mut connection, retry.next_cursor);
        assert!(
            progress(&connection, FULLTEXT_BODY)
                .expect("complete progress")
                .complete
        );
        assert_eq!(stored(&connection, "a"), ("blob".to_owned(), body(2)));
    }

    #[test]
    fn compaction_rolls_back_when_a_read_back_value_differs() {
        let mut connection = database();
        insert(&connection, "a", rusqlite::types::Value::Text(body(1)));
        // A damaged write: the stored value is replaced after the update.
        connection
            .execute_batch(
                "CREATE TRIGGER damage AFTER UPDATE ON runtime_item_fulltext BEGIN \
                 UPDATE runtime_item_fulltext SET body=x'01' WHERE rowid=NEW.rowid; END;",
            )
            .expect("trigger");
        let window = scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("scan");
        let transaction = connection.transaction().expect("transaction");
        let error = apply_window(&transaction, FULLTEXT_BODY, &window, NOW)
            .expect_err("a damaged value refuses the batch");
        drop(transaction);
        assert!(matches!(
            error,
            StoreError::InvalidStoredObject { table: "runtime_item_fulltext", id } if id == "a"
        ));
        assert_eq!(stored(&connection, "a"), ("text".to_owned(), body(1)));
        assert_eq!(
            progress(&connection, FULLTEXT_BODY).expect("progress"),
            CompactionProgress::default()
        );
    }

    #[test]
    fn compaction_windows_are_bounded_and_resume_after_the_last_converted_row() {
        let mut connection = database();
        for index in 0..COMPACTION_BATCH_ROWS + 5 {
            insert(
                &connection,
                &format!("r{index:03}"),
                rusqlite::types::Value::Text(body(index)),
            );
        }
        let first =
            scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("first window");
        assert_eq!(first.candidates.len(), COMPACTION_BATCH_ROWS);
        assert_eq!(first.next_cursor, COMPACTION_BATCH_ROWS as i64);
        assert!(!first.done);
        let transaction = connection.transaction().expect("transaction");
        apply_window(&transaction, FULLTEXT_BODY, &first, NOW).expect("first batch");
        transaction.commit().expect("commit");
        assert_eq!(
            progress(&connection, FULLTEXT_BODY).expect("progress"),
            CompactionProgress {
                cursor: COMPACTION_BATCH_ROWS as i64,
                complete: false
            }
        );
        let second = scan_window(
            &connection,
            FULLTEXT_BODY,
            Direction::Compress,
            first.next_cursor,
        )
        .expect("second");
        assert_eq!(second.candidates.len(), 5);
        assert!(second.done);

        // One value above the byte budget is converted alone.
        let connection = database();
        let large = "x".repeat(COMPACTION_BATCH_BYTES + 1);
        insert(&connection, "large", rusqlite::types::Value::Text(large));
        insert(&connection, "next", rusqlite::types::Value::Text(body(1)));
        let window =
            scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("large window");
        assert_eq!(window.candidates.len(), 1);
        assert_eq!(window.next_cursor, 1);
    }

    #[test]
    fn an_empty_final_window_cannot_hide_a_plain_row_before_the_cursor() {
        let mut connection = database();
        insert(&connection, "a", SqlValue::Text(body(1)));
        finish(&mut connection, COMPACTION_WINDOW_ROWS);
        assert_eq!(
            progress(&connection, FULLTEXT_BODY).expect("progress"),
            CompactionProgress::default()
        );
        let status = migration_progress::load(&canvas_read(&connection), FULLTEXT_BODY.progress)
            .expect("status")
            .expect("progress")
            .status;
        assert_eq!(status, "running");
    }

    #[test]
    fn compaction_does_not_add_an_oversize_value_to_an_existing_batch() {
        let connection = database();
        insert(&connection, "small", SqlValue::Text(body(1)));
        insert(
            &connection,
            "large",
            SqlValue::Text("x".repeat(COMPACTION_BATCH_BYTES + 1)),
        );
        let window = scan_window(&connection, FULLTEXT_BODY, Direction::Compress, 0).expect("scan");
        assert_eq!(
            window.candidates.len(),
            1,
            "an oversize value needs its own batch"
        );
        assert_eq!(window.next_cursor, 1);
    }

    #[test]
    fn analytics_records_compress_with_their_dictionary_and_inflate_back() {
        use crate::stored_text_dictionary as dictionaries;
        let mut connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(
                "CREATE TABLE analytics_items (
                    id TEXT PRIMARY KEY, agent TEXT NOT NULL, root TEXT, thread TEXT, turn TEXT,
                    at REAL NOT NULL, type TEXT, name TEXT, is_tool INTEGER NOT NULL, record TEXT NOT NULL);",
            )
            .expect("table");
        dictionaries::ensure_analytics_schema(&connection).expect("schema");
        dictionaries::install_analytics_sql(&connection).expect("sql");
        assert!(
            !connection
                .table_exists(None, Metadata::NAME)
                .expect("no Canvas metadata")
        );
        assert_eq!(
            progress(&connection, ANALYTICS_ITEMS_RECORD).expect("absent progress"),
            CompactionProgress::default()
        );
        let record = |index: usize| {
            studio_json::json!({"id": format!("item-{index:05}"), "agentId": "agent",
                "type": "agentMessage", "output": {"bytes": index}})
            .to_string()
        };
        for index in 0..2_500 {
            connection
                .execute(
                    "INSERT INTO main.analytics_items VALUES (?1,'agent',NULL,NULL,NULL,1.0,'agentMessage',NULL,0,?2)",
                    params![format!("item-{index:05}"), record(index)],
                )
                .expect("row");
        }
        assert!(
            scan_window(&connection, ANALYTICS_ITEMS_RECORD, Direction::Compress, 0).is_err(),
            "a dictionary column is not compressed without its dictionary"
        );
        let samples = (0..2_500)
            .map(|index| record(index).into_bytes())
            .collect::<Vec<_>>();
        let transaction = connection.transaction().expect("transaction");
        let dictionary = dictionaries::insert(
            &transaction,
            dictionaries::ANALYTICS_ITEMS_RECORD,
            dictionaries::train(&samples).expect("trained"),
            NOW,
        )
        .expect("stored");
        transaction.commit().expect("commit");

        let window =
            scan_window(&connection, ANALYTICS_ITEMS_RECORD, Direction::Compress, 0).expect("scan");
        assert_eq!(window.candidates.len(), COMPACTION_BATCH_ROWS);
        let transaction = connection.transaction().expect("transaction");
        apply_window(&transaction, ANALYTICS_ITEMS_RECORD, &window, NOW).expect("apply");
        transaction.commit().expect("commit");
        let (kind, stored, text): (String, Vec<u8>, String) = connection
            .query_row(
                "SELECT typeof(m.record),m.record,v.record FROM main.analytics_items m \
                 JOIN analytics_items v ON v.id=m.id WHERE m.rowid=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("row");
        assert_eq!(kind, "blob");
        assert_eq!(
            zstd_safe::get_dict_id_from_frame(&stored[1..]).map(|id| id.get()),
            Some(dictionary.id)
        );
        assert_eq!(text, record(0));
        assert_eq!(
            progress(&connection, ANALYTICS_ITEMS_RECORD).expect("progress"),
            CompactionProgress {
                cursor: COMPACTION_BATCH_ROWS as i64,
                complete: false
            }
        );

        let window = scan_window(&connection, ANALYTICS_ITEMS_RECORD, Direction::Inflate, 0)
            .expect("inflate scan");
        let transaction = connection.transaction().expect("transaction");
        let batch =
            apply_window(&transaction, ANALYTICS_ITEMS_RECORD, &window, NOW).expect("inflate");
        transaction.commit().expect("commit");
        assert_eq!(batch.converted, COMPACTION_BATCH_ROWS);
        let plain: i64 = connection
            .query_row(
                "SELECT count(*) FROM main.analytics_items WHERE typeof(record)='text'",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(plain, 2_500);
        let transaction = connection.transaction().expect("error transaction");
        record_error(&transaction, ANALYTICS_ITEMS_RECORD, "legacy error", NOW)
            .expect("legacy error");
        assert_eq!(
            progress(&transaction, ANALYTICS_ITEMS_RECORD)
                .expect("error cursor")
                .cursor,
            COMPACTION_BATCH_ROWS as i64
        );
        reset_progress(&transaction, ANALYTICS_ITEMS_RECORD).expect("legacy reset");
        transaction.commit().expect("commit");
        assert_eq!(
            progress(&connection, ANALYTICS_ITEMS_RECORD).expect("reset progress"),
            CompactionProgress::default()
        );
        assert!(
            connection
                .table_exists(None, "runtime_stored_text_batches")
                .expect("legacy receipts")
        );
        assert!(
            !connection
                .table_exists(None, Metadata::NAME)
                .expect("no Canvas metadata")
        );
    }
}
