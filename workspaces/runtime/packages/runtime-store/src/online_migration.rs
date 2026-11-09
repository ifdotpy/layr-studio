//! Online migration of a large table in small batches, while the single writer
//! keeps serving commands. See `docs/online-migrations.md`.
//!
//! One batch is one write transaction. It ends when the batch time limit would
//! be exceeded, and the next batch starts after a pause that lets queued
//! command writes run first. The high-water mark lives in
//! `runtime_payload_migrations` (name `online:<NAME>`), so a restart resumes
//! from the last committed batch.
//!
//! Source rows that change during the migration reach the target through
//! triggers. They record the changed key in `runtime_online_migration_dirty`.
//! Every batch first re-copies dirty keys, then copies new rows. The last batch
//! sees an empty dirty set and no rows above the mark in one transaction, drops
//! the triggers and records completion. The owner switches to the target in
//! that final transaction through `finish`, before another legacy write.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use studio_db_api::migration::{
    CopyPlan, DirtyKey, MigrationKey, decode_key, encode_key, identifier, key_sql,
};
use studio_db_api::{MutableTable, Table};
use studio_runtime_schema::persistence::MigrationStatus;

use crate::{RuntimeStore, StoreError, StoreResult};

/// Time limit of one batch transaction, in milliseconds. A batch stops before a
/// row whose estimated cost (the slowest row so far) would cross it. The first
/// row of a batch always runs, so one row slower than the limit still advances.
pub const BATCH_TIME_LIMIT_MS: u64 = 50;

/// Most rows read or re-copied by one batch, in rows.
pub const BATCH_MAX_ROWS: usize = 500;

/// Pause between two batches, in milliseconds. Command writes queue in the
/// writer during the pause and run before the next batch.
pub const BATCH_PAUSE_MS: u64 = 50;

const DIRTY_TABLE: &str = "runtime_online_migration_dirty";

/// A monotonic clock. Tests replace it to control batch timing.
pub trait Clock {
    fn now(&self) -> Duration;
}

/// The real monotonic clock.
pub struct SystemClock(Instant);

impl SystemClock {
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

/// One migration: the source rows, the transform and the target write.
/// Keys retain their original typed components, including zero and negative integers.
pub trait OnlineMigration: Send + Sync + 'static {
    type Key: MigrationKey;
    fn name(&self) -> &str {
        Self::NAME
    }
    /// A source row after reading.
    type Row;
    /// The value written to the target.
    type Out;
    /// Unique name made of `[a-z0-9_]`. It names the progress row and triggers.
    const NAME: &'static str;
    /// Source table. Writes to it are tracked by triggers until completion.
    const SOURCE_TABLE: &'static str;
    /// Complete ordered source key, with a tie key for nullable identities.
    const SOURCE_KEYS: &'static [&'static str];
    fn source_keys(&self) -> &[&str] {
        Self::SOURCE_KEYS
    }
    fn prepare(&self, _tx: &Transaction<'_>) -> StoreResult<()> {
        Ok(())
    }

    /// Up to `limit` rows with a key above `after`, ordered by key.
    fn read_after(
        &self,
        db: &Connection,
        after: Option<&Self::Key>,
        limit: usize,
    ) -> StoreResult<Vec<(Self::Key, Self::Row)>>;
    /// The current source row of `key`, or `None` when it is gone.
    fn read_one(&self, db: &Connection, key: &Self::Key) -> StoreResult<Option<Self::Row>>;
    /// Pure conversion of one row. No I/O.
    fn transform(&self, row: Self::Row) -> StoreResult<Self::Out>;
    /// Write the target row of `key`. Must be idempotent (upsert): a batch can
    /// run again after a crash or when a dirty key is copied twice.
    fn write(&self, db: &Connection, key: &Self::Key, out: Self::Out) -> StoreResult<()>;
    /// Delete the target row of `key` after its source row was deleted.
    fn remove(&self, db: &Connection, key: &Self::Key) -> StoreResult<()>;
    /// Verify and switch durable writer authority before this transaction commits.
    fn finish(&self, _tx: &Transaction<'_>) -> StoreResult<()> {
        Ok(())
    }
}

impl<L, T> OnlineMigration for CopyPlan<L, T>
where
    L: Table<Database = crate::peer_update_settings::Canvas>,
    L::Key: MigrationKey,
    T: MutableTable<Database = L::Database>,
    T::Key: MigrationKey,
{
    type Key = L::Key;
    type Row = L;
    type Out = (T, studio_db_api::migration::Evidence);
    const NAME: &'static str = L::NAME;
    const SOURCE_TABLE: &'static str = L::NAME;
    const SOURCE_KEYS: &'static [&'static str] = &[];
    fn name(&self) -> &str {
        self.name
    }
    fn source_keys(&self) -> &[&str] {
        self.source_keys
    }
    fn prepare(&self, tx: &Transaction<'_>) -> StoreResult<()> {
        T::descriptor().install_if_absent(tx)?;
        let name = progress_name(self);
        let started: bool = tx.query_row(
            "SELECT copy_source_rows IS NOT NULL FROM runtime_payload_migrations WHERE name=?1",
            [&name],
            |row| row.get(0),
        )?;
        if !started {
            self.require_empty(tx)?;
            // One snapshot, atomically with empty-target admission and triggers.
            let source_rows: i64 = tx.query_row(
                &format!("SELECT count(*) FROM ({})", L::SELECT),
                [],
                |row| row.get(0),
            )?;
            tx.execute("UPDATE runtime_payload_migrations SET copy_source_rows=?2,copy_target_rows=0 WHERE name=?1",params![name,source_rows])?;
        }
        Ok(())
    }
    fn read_after(
        &self,
        db: &Connection,
        after: Option<&Self::Key>,
        limit: usize,
    ) -> StoreResult<Vec<(Self::Key, L)>> {
        Ok(CopyPlan::read_after(self, db, after, limit)?)
    }
    fn read_one(&self, db: &Connection, key: &Self::Key) -> StoreResult<Option<L>> {
        Ok(CopyPlan::read_one(self, db, key)?)
    }
    fn transform(&self, row: L) -> StoreResult<Self::Out> {
        let evidence = (self.source_evidence)(&row)?;
        Ok(((self.transform)(row)?, evidence))
    }
    fn write(
        &self,
        db: &Connection,
        key: &Self::Key,
        (out, evidence): Self::Out,
    ) -> StoreResult<()> {
        out.validate()?;
        studio_db_api::migration::copy_verified(
            || {
                let inserted = out.bind(db, |params| {
                    db.prepare_cached(T::INSERT_NEW)?.execute(params)
                })?;
                if inserted == 0 {
                    out.bind(db, |params| db.prepare_cached(T::UPSERT)?.execute(params))?;
                }
                db.execute("UPDATE runtime_payload_migrations SET copy_target_rows=copy_target_rows+?2 WHERE name=?1",params![progress_name(self),i64::from(inserted!=0)])?;
                Ok(true)
            },
            || {
                T::bind_key(&(self.target_key)(key), |params| {
                    db.prepare_cached(T::GET)?.query_row(params, |row| {
                        Ok((self.target_evidence)(&T::read(db, row)?)?.digest()
                            == evidence.digest())
                    })
                })
            },
            || rusqlite::Error::InvalidQuery,
        )?;
        Ok(())
    }
    fn remove(&self, db: &Connection, key: &Self::Key) -> StoreResult<()> {
        let removed = T::bind_key(&(self.target_key)(key), |params| {
            db.prepare_cached(T::DELETE)?.execute(params)
        })?;
        db.execute("UPDATE runtime_payload_migrations SET copy_target_rows=copy_target_rows-?2 WHERE name=?1",params![progress_name(self),i64::from(removed!=0)])?;
        Ok(())
    }
    fn finish(&self, tx: &Transaction<'_>) -> StoreResult<()> {
        let mut typed = crate::peer_update_settings::canvas_tx(tx);
        // Empty start + injective mapping + verified writes make equal counters
        // a bijection. No table aggregates run under the final writer lock.
        let covered:bool=tx.query_row("SELECT (copy_source_rows=copy_target_rows) IS 1 FROM runtime_payload_migrations WHERE name=?1",[progress_name(self)],|row|row.get(0))?;
        if !covered {
            return Err(StoreError::RecordContract("online copy coverage mismatch"));
        }
        (self.finish)(&mut typed)?;
        Ok(())
    }
}

/// Result of one batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// More work remains.
    Progress,
    /// The migration is complete and its triggers are gone.
    Complete,
}

fn progress_name<M: OnlineMigration>(migration: &M) -> String {
    format!("online:{}", migration.name())
}

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

fn trigger_names<M: OnlineMigration>(migration: &M) -> [String; 3] {
    ["ins", "upd", "del"].map(|kind| format!("online_{}_{kind}", migration.name()))
}

/// Create the progress row and the change triggers. Safe to call at every start;
/// a completed migration stays complete and gets no triggers.
pub fn begin<M: OnlineMigration>(tx: &Transaction<'_>, migration: &M) -> StoreResult<()> {
    assert!(
        !migration.name().is_empty()
            && migration
                .name()
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
        "online migration name must match [a-z0-9_]+"
    );
    crate::migrations::m2026_11_09_online_keys::apply(tx)?;
    let name = progress_name(migration);
    tx.execute(
        "INSERT OR IGNORE INTO runtime_payload_migrations(name,cursor,complete,status,updated,cursor_key) \
         VALUES(?1,0,0,?2,?3,x'')",
        params![name, MigrationStatus::Running.as_str(), now_seconds()],
    )?;
    // Raw SQL: progress schema is upgraded in stages and cannot require final descriptor columns here.
    let complete: bool = tx.query_row(
        "SELECT complete FROM runtime_payload_migrations WHERE name=?1",
        [&name],
        |row| row.get(0),
    )?;
    if complete {
        return Ok(());
    }
    assert!(
        !migration.source_keys().is_empty(),
        "online migration requires a source key"
    );
    migration.prepare(tx)?;
    crate::settings_schema::OnlineMigrationDirty::descriptor().install_if_absent(tx)?;
    let [ins, upd, del] = trigger_names(migration);
    let table = identifier(M::SOURCE_TABLE);
    // Record all changes. Replaying a key above the cursor is idempotent and
    // avoids imposing an encoding comparison order on composite keys.
    let record = |row: &str| {
        let key = if M::Key::INTEGER {
            format!(
                "{}.{}",
                identifier(row),
                identifier(migration.source_keys()[0])
            )
        } else {
            key_sql(migration.source_keys(), row)
        };
        format!("INSERT OR IGNORE INTO {DIRTY_TABLE}(migration,key) VALUES('{name}',{key});")
    };
    let source_delta = |delta: i64| {
        format!(
            "UPDATE runtime_payload_migrations SET copy_source_rows=copy_source_rows+({delta}) WHERE name='{name}' AND copy_source_rows IS NOT NULL;"
        )
    };
    tx.execute_batch(&format!(
        "CREATE TRIGGER IF NOT EXISTS {ins} AFTER INSERT ON {table} BEGIN {} {} END;\
         CREATE TRIGGER IF NOT EXISTS {upd} AFTER UPDATE ON {table} BEGIN {} {} END;\
         CREATE TRIGGER IF NOT EXISTS {del} AFTER DELETE ON {table} BEGIN {} {} END;",
        record("NEW"),
        source_delta(1),
        record("OLD"),
        record("NEW"),
        record("OLD"),
        source_delta(-1),
    ))?;
    Ok(())
}

/// Stops a batch before it would cross the time limit.
struct Budget<'a> {
    clock: &'a dyn Clock,
    started: Duration,
    slowest: Duration,
    rows: usize,
}

impl Budget<'_> {
    fn allows_next(&self) -> bool {
        let limit = Duration::from_millis(BATCH_TIME_LIMIT_MS);
        self.rows < BATCH_MAX_ROWS
            && (self.rows == 0 || self.clock.now() - self.started + self.slowest < limit)
    }

    fn timed<T>(&mut self, work: impl FnOnce() -> StoreResult<T>) -> StoreResult<T> {
        let before = self.clock.now();
        let value = work()?;
        self.slowest = self.slowest.max(self.clock.now() - before);
        self.rows += 1;
        Ok(value)
    }
}

/// Run one batch inside the caller's write transaction. `begin` must have run.
pub fn step<M: OnlineMigration>(
    tx: &Transaction<'_>,
    migration: &M,
    clock: &dyn Clock,
) -> StoreResult<Step> {
    let name = progress_name(migration);
    // Raw SQL: progress schema is upgraded in stages and cannot require final descriptor columns here.
    let complete: bool = tx.query_row(
        "SELECT complete FROM runtime_payload_migrations WHERE name=?1",
        [&name],
        |row| row.get(0),
    )?;
    if complete {
        return Ok(Step::Complete);
    }
    let mut budget = Budget {
        clock,
        started: clock.now(),
        slowest: Duration::ZERO,
        rows: 0,
    };

    // 1. Re-copy changed keys; a dirty key invalidates its prior row evidence.
    let dirty = tx
        .prepare_cached(&format!(
            "SELECT key FROM {DIRTY_TABLE} WHERE migration=?1 ORDER BY key LIMIT ?2"
        ))?
        .query_map(params![name, BATCH_MAX_ROWS as i64], |row| {
            row.get::<_, DirtyKey>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut dirty_left = dirty.len() == BATCH_MAX_ROWS;
    for stored_key in &dirty {
        let key = stored_key.decode::<M::Key>()?;
        if !budget.allows_next() {
            dirty_left = true;
            break;
        }
        budget.timed(|| match migration.read_one(tx, &key)? {
            Some(row) => migration.write(tx, &key, migration.transform(row)?),
            None => migration.remove(tx, &key),
        })?;
        tx.execute(
            &format!("DELETE FROM {DIRTY_TABLE} WHERE migration=?1 AND key=?2"),
            params![name, stored_key],
        )?;
    }

    // 2. Copy rows above the mark, unless dirty keys still wait.
    let (mark, cursor_key): (i64, Option<Vec<u8>>) = tx
        .query_row(
            "SELECT cursor,cursor_key FROM runtime_payload_migrations WHERE name=?1",
            [&name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(StoreError::RecordContract("online migration was not begun"))?;
    let after = match cursor_key.as_deref() {
        Some([]) => None,
        Some(bytes) => Some(decode_key::<M::Key>(bytes)?),
        None if mark == 0 => None,
        None => Some(M::Key::from_values(vec![SqlValue::Integer(mark)])?),
    };
    let mut new_cursor = cursor_key;
    let mut new_mark = mark;
    let mut exhausted = false;
    if !dirty_left {
        let rows = migration.read_after(tx, after.as_ref(), BATCH_MAX_ROWS)?;
        exhausted = rows.is_empty();
        for (key, row) in rows {
            if !budget.allows_next() {
                break;
            }
            budget.timed(|| migration.write(tx, &key, migration.transform(row)?))?;
            new_cursor = Some(encode_key(&key));
            new_mark = if M::Key::INTEGER {
                match key.values()[0] {
                    SqlValue::Integer(value) => value,
                    SqlValue::Null | SqlValue::Real(_) | SqlValue::Text(_) | SqlValue::Blob(_) => {
                        unreachable!()
                    }
                }
            } else {
                new_mark.saturating_add(1)
            };
        }
    }

    // 3. Record the mark, or completion when nothing was left to do.
    let done = exhausted && !dirty_left;
    if done {
        migration.finish(tx)?;
        for trigger in trigger_names(migration) {
            tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {trigger}"))?;
        }
    }
    let status = if done {
        MigrationStatus::Complete
    } else {
        MigrationStatus::Running
    };
    tx.execute(
        "UPDATE runtime_payload_migrations SET cursor=?2,complete=?3,status=?4,updated=?5,error=NULL,cursor_key=?6 \
         WHERE name=?1",
        params![name, new_mark, i64::from(done), status.as_str(), now_seconds(),new_cursor],
    )?;
    Ok(if done { Step::Complete } else { Step::Progress })
}

/// Drive a migration to completion, one batch per write transaction, with
/// [`BATCH_PAUSE_MS`] between batches. Stop it by dropping the future or by
/// returning when `stop` reports true; run it again to resume from the mark.
/// A failed batch rolls back, is recorded in the progress row and is returned.
pub async fn run<M: OnlineMigration>(
    store: &RuntimeStore,
    migration: std::sync::Arc<M>,
    stop: impl Fn() -> bool + Send,
) -> StoreResult<Step> {
    let begin_migration = std::sync::Arc::clone(&migration);
    store
        .transaction(move |tx| begin(tx, begin_migration.as_ref()))
        .await?;
    while !stop() {
        let step_migration = std::sync::Arc::clone(&migration);
        let outcome = store
            .transaction(move |tx| step(tx, step_migration.as_ref(), &SystemClock::new()))
            .await;
        match outcome {
            Ok(Step::Complete) => return Ok(Step::Complete),
            Ok(Step::Progress) => {
                tokio::time::sleep(Duration::from_millis(BATCH_PAUSE_MS)).await;
            }
            Err(error) => {
                let message = error.to_string();
                let name = progress_name(migration.as_ref());
                // The original error is the one to return; a failed status write is only logged.
                if let Err(status_error) =
                    store
                        .transaction(move |tx| {
                            tx.execute(
                            "UPDATE runtime_payload_migrations SET status=?2,error=?3,updated=?4 \
                             WHERE name=?1",
                            params![name, MigrationStatus::Error.as_str(), message, now_seconds()],
                        )?;
                            Ok::<(), StoreError>(())
                        })
                        .await
                {
                    eprintln!("The migration failure was not recorded: {status_error}");
                }
                return Err(error);
            }
        }
    }
    Ok(Step::Progress)
}

/// Progress of one online migration, for the migration status endpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub name: String,
    /// High-water mark: the highest source key copied.
    pub cursor: i64,
    pub complete: bool,
    pub status: String,
    pub updated: Option<f64>,
    pub error: Option<String>,
    /// Changed keys waiting for a re-copy.
    pub dirty: i64,
}

/// Read the progress of every online migration.
pub fn progress(db: &Connection) -> rusqlite::Result<Vec<Progress>> {
    let dirty_table: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [DIRTY_TABLE],
        |row| row.get(0),
    )?;
    let dirty_sql = if dirty_table {
        format!("(SELECT COUNT(*) FROM {DIRTY_TABLE} d WHERE d.migration=m.name)")
    } else {
        "0".to_owned()
    };
    let mut statement = db.prepare(&format!(
        "SELECT substr(m.name,8),m.cursor,m.complete,m.status,m.updated,m.error,{dirty_sql} \
         FROM runtime_payload_migrations m WHERE m.name LIKE 'online:%' ORDER BY m.name"
    ))?;
    let rows = statement
        .query_map([], |row| {
            Ok(Progress {
                name: row.get(0)?,
                cursor: row.get(1)?,
                complete: row.get::<_, i64>(2)? != 0,
                status: row.get(3)?,
                updated: row.get(4)?,
                error: row.get(5)?,
                dirty: row.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::peer_update_settings::Canvas;
    use studio_db_api::{DbTable, migration::Evidence};

    #[derive(DbTable)]
    #[db(table = "copy_source", database = Canvas)]
    struct Source {
        #[db(key)]
        scope: String,
        #[db(key)]
        ordinal: i64,
        value: i64,
        raw: String,
        bytes: Vec<u8>,
    }
    #[derive(DbTable)]
    #[db(table = "copy_target", database = Canvas)]
    struct Target {
        #[db(key)]
        scope: String,
        #[db(key)]
        ordinal: i64,
        value: i64,
        raw: String,
        bytes: Vec<u8>,
    }
    #[derive(studio_db_api::DbEnum)]
    enum CopyKind {
        Copied = 1,
        Other = 2,
    }
    #[derive(DbTable)]
    #[db(table = "copy_partitions", database = Canvas, partitioned_by = CopyKind)]
    struct Partitions {
        #[db(key)]
        kind: CopyKind,
        #[db(key)]
        scope: String,
        #[db(key)]
        ordinal: i64,
        value: i64,
        raw: String,
        bytes: Vec<u8>,
    }
    #[derive(studio_db_api::DbPartition)]
    #[db(of = Partitions, kind = CopyKind::Copied)]
    struct Copied {
        #[db(key)]
        scope: String,
        #[db(key)]
        ordinal: i64,
        value: i64,
        raw: String,
        bytes: Vec<u8>,
    }
    fn evidence(scope: &str, ordinal: i64, value: i64, raw: &str, bytes: &[u8]) -> Evidence {
        Evidence {
            identity: vec![SqlValue::Text(scope.to_owned()), SqlValue::Integer(ordinal)],
            ordering: vec![SqlValue::Integer(ordinal)],
            values: vec![
                SqlValue::Integer(value),
                SqlValue::Text(raw.to_owned()),
                SqlValue::Blob(bytes.to_vec()),
            ],
        }
    }
    fn copy_plan() -> CopyPlan<Source, Target> {
        CopyPlan {
            name: "composite_copy",
            source_keys: &["scope", "ordinal"],
            source_key: |row| (row.scope.clone(), row.ordinal),
            target_key: Clone::clone,
            transform: |row| {
                Ok(Target {
                    scope: row.scope,
                    ordinal: row.ordinal,
                    value: row.value * 2,
                    raw: row.raw,
                    bytes: row.bytes,
                })
            },
            source_evidence: |row| {
                Ok(evidence(
                    &row.scope,
                    row.ordinal,
                    row.value * 2,
                    &row.raw,
                    &row.bytes,
                ))
            },
            target_evidence: |row| {
                Ok(evidence(
                    &row.scope,
                    row.ordinal,
                    row.value,
                    &row.raw,
                    &row.bytes,
                ))
            },
            target_select: Target::SELECT,
            finish: |tx| {
                tx.connection().execute_batch("CREATE TABLE authority(target INTEGER NOT NULL); INSERT INTO authority VALUES(1);
                    CREATE TRIGGER refuse_legacy BEFORE INSERT ON copy_source BEGIN SELECT RAISE(ABORT,'target is authoritative'); END;")
            },
        }
    }
    fn copy_database() -> Connection {
        let db = database(0);
        Source::descriptor().install_if_absent(&db).unwrap();
        Target::descriptor().install_if_absent(&db).unwrap();
        for ordinal in -600..600 {
            db.execute(
                "INSERT INTO copy_source VALUES('a',?1,?1,?2,?3)",
                params![
                    ordinal,
                    " {\"duplicate\":1,\"duplicate\":2} \n",
                    vec![0u8, 255, ordinal as u8]
                ],
            )
            .unwrap();
        }
        db
    }
    fn copy_batch(db: &mut Connection, plan: &CopyPlan<Source, Target>) -> StoreResult<Step> {
        let tx = db.transaction()?;
        let outcome = step(&tx, plan, &ThreadClock)?;
        tx.commit()?;
        Ok(outcome)
    }

    #[test]
    fn final_copy_transaction_does_not_scan_coverage_aggregates() {
        use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
        let mut db = copy_database();
        let plan = copy_plan();
        let tx = db.transaction().unwrap();
        begin(&tx, &plan).unwrap();
        tx.commit().unwrap();
        db.authorizer(Some(|context: AuthContext<'_>| {
            if let AuthAction::Function { function_name } = context.action
                && ["count", "min", "max"].contains(&function_name)
            {
                return Authorization::Deny;
            }
            Authorization::Allow
        }))
        .unwrap();
        for _ in 0..20 {
            if copy_batch(&mut db, &plan).unwrap() == Step::Complete {
                return;
            }
        }
        panic!("copy did not complete")
    }

    #[test]
    fn copy_plan_refuses_stale_target_even_when_counts_and_endpoints_match() {
        let mut db = copy_database();
        db.execute_batch("DELETE FROM copy_source; INSERT INTO copy_source VALUES('a',1,0,'',x''),('a',2,0,'',x''),('a',3,0,'',x''); INSERT INTO copy_target VALUES('a',10,0,'',x''),('a',20,0,'',x''),('a',30,0,'',x'')").unwrap();
        let tx = db.transaction().unwrap();
        assert!(
            begin(&tx, &copy_plan()).is_err(),
            "a copy must refuse a pre-populated target"
        );
    }

    #[test]
    fn non_injective_copy_key_map_cannot_hide_behind_stale_target_rows() {
        for stale in [false, true] {
            let mut db = copy_database();
            db.execute_batch("DELETE FROM copy_source; INSERT INTO copy_source VALUES('a',1,0,'',x''),('a',2,0,'',x''),('a',3,0,'',x''); INSERT INTO copy_target VALUES('a',10,0,'',x''),('a',20,0,'',x''),('a',30,0,'',x'')").unwrap();
            if !stale {
                db.execute("DELETE FROM copy_target", []).unwrap();
            }
            let mut plan = copy_plan();
            plan.target_key = |key| (key.0.clone(), if key.1 == 1 { 10 } else { 30 });
            plan.transform = |row| {
                Ok(Target {
                    scope: row.scope,
                    ordinal: if row.ordinal == 1 { 10 } else { 30 },
                    value: row.value,
                    raw: row.raw,
                    bytes: row.bytes,
                })
            };
            plan.source_evidence = |row| {
                let ordinal = if row.ordinal == 1 { 10 } else { 30 };
                Ok(evidence(
                    &row.scope, ordinal, row.value, &row.raw, &row.bytes,
                ))
            };
            let outcome = (|| -> StoreResult<()> {
                let tx = db.transaction()?;
                begin(&tx, &plan)?;
                tx.commit()?;
                for _ in 0..10 {
                    if copy_batch(&mut db, &plan)? == Step::Complete {
                        return Ok(());
                    }
                }
                panic!("copy did not terminate")
            })();
            assert!(outcome.is_err(), "a non-injective map must not cut over");
            assert!(!crate::schema::table_exists(&db, "authority").unwrap());
        }
    }

    #[test]
    fn composite_copy_resumes_and_replays_concurrent_insert_update_delete() {
        let mut db = copy_database();
        let plan = copy_plan();
        let tx = db.transaction().unwrap();
        begin(&tx, &plan).unwrap();
        tx.commit().unwrap();
        assert_eq!(copy_batch(&mut db, &plan).unwrap(), Step::Progress);
        let counts = |db: &Connection| {
            db.query_row("SELECT copy_source_rows,copy_target_rows FROM runtime_payload_migrations WHERE name='online:composite_copy'",[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?))).unwrap()
        };
        assert_eq!(counts(&db), (1200, 500));
        let mark = db.query_row("SELECT cursor_key FROM runtime_payload_migrations WHERE name='online:composite_copy'",[],|row| row.get::<_,Vec<u8>>(0)).unwrap();
        assert_eq!(
            decode_key::<(String, i64)>(&mark).unwrap(),
            ("a".into(), -101)
        );
        db.execute_batch("CREATE TRIGGER damage AFTER INSERT ON copy_target WHEN NEW.ordinal=0 BEGIN UPDATE copy_target SET bytes=x'00' WHERE scope=NEW.scope AND ordinal=NEW.ordinal; END").unwrap();
        assert!(copy_batch(&mut db, &plan).is_err());
        assert_eq!(
            db.query_row("SELECT count(*) FROM copy_target", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            500
        );
        // A stopped/restarted runner observes command writes between transactions.
        db.execute_batch(
            "DROP TRIGGER damage;
            INSERT INTO copy_source VALUES('a',-1000,7,'new',x'00FF');
            INSERT INTO copy_source VALUES('a',-1001,8,'extra',x'FF00');
            UPDATE copy_source SET value=999,raw=' {\"x\":1.0} ' WHERE ordinal=-590;
            DELETE FROM copy_source WHERE ordinal=-580;
            UPDATE copy_source SET scope='b',ordinal=-42 WHERE ordinal=-570;",
        )
        .unwrap();
        assert_eq!(counts(&db), (1201, 500));
        let tx = db.transaction().unwrap();
        begin(&tx, &copy_plan()).unwrap();
        tx.commit().unwrap();
        assert_eq!(
            counts(&db),
            (1201, 500),
            "resume keeps admission and counters"
        );
        for _ in 0..20 {
            if copy_batch(&mut db, &copy_plan()).unwrap() == Step::Complete {
                break;
            }
        }
        let tx = db.transaction().unwrap();
        assert_eq!(counts(&tx), (1201, 1201));
        plan.verify(&crate::peer_update_settings::canvas_tx(&tx))
            .unwrap();
        assert!(progress(&tx).unwrap()[0].complete);
        assert_eq!(progress(&tx).unwrap()[0].dirty, 0);
        assert!(
            tx.execute("INSERT INTO copy_source VALUES('late',0,0,'',x'')", [])
                .is_err()
        );
        assert_eq!(
            tx.query_row("SELECT target FROM authority", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn small_copy_verifies_before_switch_and_rolls_back_damaged_bytes() {
        let mut db = copy_database();
        db.execute_batch("CREATE TRIGGER damage AFTER INSERT ON copy_target BEGIN UPDATE copy_target SET raw='normalized' WHERE scope=NEW.scope AND ordinal=NEW.ordinal; END").unwrap();
        let tx = db.transaction().unwrap();
        assert!(
            copy_plan()
                .copy_tx(&mut crate::peer_update_settings::canvas_tx(&tx))
                .is_err()
        );
        drop(tx);
        assert_eq!(
            db.query_row("SELECT count(*) FROM copy_target", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(!crate::schema::table_exists(&db, "authority").unwrap());
        db.execute_batch("DROP TRIGGER damage").unwrap();
        let tx = db.transaction().unwrap();
        copy_plan()
            .copy_tx(&mut crate::peer_update_settings::canvas_tx(&tx))
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM copy_target", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1200
        );
    }

    #[test]
    fn copy_into_partition_verifies_only_its_kind_then_retires_the_source() {
        let mut db = copy_database();
        Partitions::descriptor().install_if_absent(&db).unwrap();
        db.execute(
            "INSERT INTO copy_partitions VALUES(2,'a',0,77,'other',x'FF')",
            [],
        )
        .unwrap();
        let plan = CopyPlan::<Source, Copied> {
            name: "partition_copy",
            source_keys: &["scope", "ordinal"],
            source_key: |row| (row.scope.clone(), row.ordinal),
            target_key: Clone::clone,
            transform: |row| {
                Ok(Copied {
                    scope: row.scope,
                    ordinal: row.ordinal,
                    value: row.value * 2,
                    raw: row.raw,
                    bytes: row.bytes,
                })
            },
            source_evidence: |row| {
                Ok(evidence(
                    &row.scope,
                    row.ordinal,
                    row.value * 2,
                    &row.raw,
                    &row.bytes,
                ))
            },
            target_evidence: |row| {
                Ok(evidence(
                    &row.scope,
                    row.ordinal,
                    row.value,
                    &row.raw,
                    &row.bytes,
                ))
            },
            target_select: Copied::SELECT,
            finish: |tx| tx.connection().execute_batch("DROP TABLE copy_source"),
        };
        let tx = db.transaction().unwrap();
        plan.copy_tx(&mut crate::peer_update_settings::canvas_tx(&tx))
            .unwrap();
        tx.commit().unwrap();
        assert!(!crate::schema::table_exists(&db, "copy_source").unwrap());
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM copy_partitions WHERE kind=1",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1200
        );
        assert_eq!(
            db.query_row(
                "SELECT value FROM copy_partitions WHERE kind=2",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            77
        );
    }

    /// Copies `src(id, v)` to `dst(id, v2)` with v2 = v * 2. Each transform
    /// advances the fake clock by `cost_ms`; `CRASH_AT` makes one write fail.
    struct Plain {
        cost_ms: std::sync::atomic::AtomicU64,
    }

    thread_local! {
        static TEST_CLOCK: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        static CRASH_AT: Cell<Option<i64>> = const { Cell::new(None) };
    }

    impl OnlineMigration for Plain {
        type Key = i64;
        type Row = i64;
        type Out = i64;
        const NAME: &'static str = "double";
        const SOURCE_TABLE: &'static str = "src";
        const SOURCE_KEYS: &'static [&'static str] = &["id"];

        fn read_after(
            &self,
            db: &Connection,
            after: Option<&i64>,
            limit: usize,
        ) -> StoreResult<Vec<(i64, i64)>> {
            Ok(db
                .prepare("SELECT id,v FROM src WHERE (?1 IS NULL OR id>?1) ORDER BY id LIMIT ?2")?
                .query_map(params![after, limit as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?)
        }
        fn read_one(&self, db: &Connection, key: &i64) -> StoreResult<Option<i64>> {
            Ok(db
                .query_row("SELECT v FROM src WHERE id=?1", [key], |r| r.get(0))
                .optional()?)
        }
        fn transform(&self, row: i64) -> StoreResult<i64> {
            let cost = self.cost_ms.load(std::sync::atomic::Ordering::Relaxed);
            TEST_CLOCK.with(|c| c.set(c.get() + Duration::from_millis(cost)));
            Ok(row * 2)
        }
        fn write(&self, db: &Connection, key: &i64, out: i64) -> StoreResult<()> {
            if CRASH_AT.with(Cell::get) == Some(*key) {
                return Err(StoreError::RecordContract("simulated crash"));
            }
            db.execute(
                "INSERT OR REPLACE INTO dst(id,v2) VALUES(?1,?2)",
                params![key, out],
            )?;
            Ok(())
        }
        fn remove(&self, db: &Connection, key: &i64) -> StoreResult<()> {
            db.execute("DELETE FROM dst WHERE id=?1", [key])?;
            Ok(())
        }
    }

    struct ThreadClock;
    impl Clock for ThreadClock {
        fn now(&self) -> Duration {
            TEST_CLOCK.with(Cell::get)
        }
    }

    fn database(rows: i64) -> Connection {
        let db = Connection::open_in_memory().expect("memory db");
        db.execute_batch(crate::feature_schema::PAYLOAD_MIGRATIONS_SCHEMA)
            .expect("progress table");
        db.execute_batch(
            "CREATE TABLE src(id INTEGER PRIMARY KEY, v INTEGER NOT NULL);
             CREATE TABLE dst(id INTEGER PRIMARY KEY, v2 INTEGER NOT NULL);",
        )
        .expect("tables");
        for id in 1..=rows {
            db.execute("INSERT INTO src VALUES(?1,?1)", [id])
                .expect("seed");
        }
        db
    }

    fn batch(db: &mut Connection, m: &Plain) -> StoreResult<Step> {
        let tx = db.transaction()?;
        let outcome = step(&tx, m, &ThreadClock)?;
        tx.commit()?;
        Ok(outcome)
    }

    fn begin_now(db: &mut Connection) {
        let tx = db.transaction().expect("tx");
        begin(&tx, &plain(0)).expect("begin");
        tx.commit().expect("commit");
    }

    fn plain(cost_ms: u64) -> Plain {
        TEST_CLOCK.with(|c| c.set(Duration::ZERO));
        CRASH_AT.with(|c| c.set(None));
        Plain {
            cost_ms: cost_ms.into(),
        }
    }

    fn run_all(db: &mut Connection, m: &Plain) {
        for _ in 0..10_000 {
            if batch(db, m).expect("batch") == Step::Complete {
                return;
            }
        }
        panic!("migration did not finish");
    }

    fn dst_rows(db: &Connection) -> Vec<(i64, i64)> {
        db.prepare("SELECT id,v2 FROM dst ORDER BY id")
            .expect("prepare")
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows")
    }

    #[test]
    fn copies_zero_and_negative_keys() {
        let mut db = database(1);
        db.execute_batch("INSERT INTO src VALUES(-2,7),(0,9)")
            .expect("nonpositive keys");
        let m = plain(0);
        begin_now(&mut db);
        run_all(&mut db, &m);
        assert_eq!(dst_rows(&db), vec![(-2, 14), (0, 18), (1, 2)]);
    }

    #[test]
    fn resumes_after_a_crash_in_the_middle_of_a_batch() {
        let mut db = database(1200);
        let m = plain(0);
        begin_now(&mut db);
        assert_eq!(batch(&mut db, &m).expect("first batch"), Step::Progress);
        let committed = progress(&db).expect("progress")[0].cursor;
        assert_eq!(committed, BATCH_MAX_ROWS as i64);
        // The next batch fails at row 700 after writing rows 501..699.
        CRASH_AT.with(|c| c.set(Some(700)));
        assert!(batch(&mut db, &m).is_err());
        // The failed batch rolled back completely.
        assert_eq!(progress(&db).expect("progress")[0].cursor, committed);
        assert_eq!(dst_rows(&db).len(), BATCH_MAX_ROWS);
        // Restart: a fresh `begin` keeps the mark, and the run finishes.
        CRASH_AT.with(|c| c.set(None));
        begin_now(&mut db);
        run_all(&mut db, &m);
        let rows = dst_rows(&db);
        assert_eq!(rows.len(), 1200);
        assert!(rows.iter().all(|(id, v2)| *v2 == id * 2));
        let done = &progress(&db).expect("progress")[0];
        assert!(done.complete);
        assert_eq!(done.status, "complete");
    }

    #[test]
    fn writes_during_the_migration_reach_the_target() {
        let mut db = database(1200);
        let m = plain(0);
        begin_now(&mut db);
        batch(&mut db, &m).expect("first batch");
        // Below the mark: update, delete. Above the mark: insert. Plus a new key.
        db.execute("UPDATE src SET v=1000 WHERE id=3", [])
            .expect("update");
        db.execute("DELETE FROM src WHERE id=4", [])
            .expect("delete");
        db.execute("UPDATE src SET v=7000 WHERE id=900", [])
            .expect("update above");
        db.execute("INSERT INTO src VALUES(5000,5000)", [])
            .expect("insert");
        assert_eq!(
            progress(&db).expect("progress")[0].dirty,
            4,
            "every changed key is replayed"
        );
        run_all(&mut db, &m);
        let rows = dst_rows(&db);
        assert!(rows.contains(&(3, 2000)));
        assert!(!rows.iter().any(|(id, _)| *id == 4));
        assert!(rows.contains(&(900, 14000)));
        assert!(rows.contains(&(5000, 10000)));
        assert_eq!(rows.len(), 1200);
        let triggers: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(triggers, 0, "completion drops the triggers");
    }

    #[test]
    fn a_batch_stays_within_the_time_limit_with_a_slow_transform() {
        let mut db = database(100);
        let m = plain(20);
        begin_now(&mut db);
        for _ in 0..60 {
            let before = ThreadClock.now();
            let done = batch(&mut db, &m).expect("batch");
            let spent = ThreadClock.now() - before;
            assert!(
                spent <= Duration::from_millis(BATCH_TIME_LIMIT_MS),
                "batch took {spent:?}"
            );
            if done == Step::Complete {
                break;
            }
        }
        assert_eq!(dst_rows(&db).len(), 100);
    }

    #[test]
    fn a_row_slower_than_the_limit_still_advances() {
        let mut db = database(3);
        let m = plain(BATCH_TIME_LIMIT_MS * 2);
        begin_now(&mut db);
        batch(&mut db, &m).expect("batch");
        assert_eq!(progress(&db).expect("progress")[0].cursor, 1);
    }
}
