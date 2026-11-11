//! P0-C workloads reuse the storage driver's copy, budgets and timing machinery.
use super::*;
use studio_db_api::migration::{CopyPlan, Evidence, decode_key};
use studio_db_api::{DbTable, Table, db_update};
use studio_runtime_store::online_migration::{self, Step, SystemClock};
use studio_runtime_store::peer_update_settings::{Canvas, canvas_tx};
use studio_runtime_store::settings_schema::{OnlineMigrationDirty, PayloadMigrations};
use studio_runtime_store::{AgentIndexedReads, SchedulerRepository, TerminalsRepository};

pub const FAMILIES: [&str; 14] = [
    "agent_account_selection",
    "agent_root_selection",
    "agent_wake_selection",
    "queue_dispatch_selection",
    "queue_repair_metadata",
    "queue_latest_200_publication",
    "transcript_paging",
    "transcript_search",
    "terminal_replay",
    "f1_publication_change",
    "f1_publication_noop",
    "f1_publication_delete",
    "f1_publication_resume",
    "f1_dirty_replay",
];

// Three rows stay live: a changed value, a moved key and a rolling high key.
// The high key advances each frame so step() retains its dirty tracking triggers.
#[derive(DbTable)]
#[db(table = "benchmark_f1_source", database = Canvas)]
struct DirtySource {
    #[db(key)]
    id: i64,
    value: i64,
}
#[derive(DbTable)]
#[db(table = "benchmark_f1_target", database = Canvas)]
struct DirtyTarget {
    #[db(key)]
    id: i64,
    value: i64,
}

fn dirty_evidence(id: i64, value: i64) -> rusqlite::Result<Evidence> {
    Ok(Evidence {
        identity: vec![SqlValue::Integer(id)],
        ordering: vec![],
        values: vec![SqlValue::Integer(value)],
    })
}
fn dirty_plan() -> CopyPlan<DirtySource, DirtyTarget> {
    CopyPlan {
        name: "benchmark_f1_dirty",
        source_keys: &["id"],
        source_key: |row| (row.id,),
        target_key: Clone::clone,
        transform: |row| {
            Ok(DirtyTarget {
                id: row.id,
                value: row.value,
            })
        },
        source_evidence: |row| dirty_evidence(row.id, row.value),
        target_evidence: |row| dirty_evidence(row.id, row.value),
        target_select: DirtyTarget::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

fn check_cursor(tx: &rusqlite::Transaction<'_>, expected: i64) -> StoreResult<()> {
    let typed = canvas_tx(tx);
    let progress = typed
        .get::<PayloadMigrations>(&("online:benchmark_f1_dirty".to_owned(),))?
        .ok_or(StoreError::RecordContract("dirty probe has no ledger"))?;
    let mark = progress
        .cursor_key
        .as_deref()
        .map(decode_key::<(i64,)>)
        .transpose()?;
    let dirty = studio_db_api::db_query!(
        typed.read(),
        OnlineMigrationDirty,
        filter(OnlineMigrationDirty::MIGRATION.eq(&"online:benchmark_f1_dirty".to_owned()))
    )?;
    if progress.complete != 0
        || progress.cursor != expected
        || mark != Some((expected,))
        || progress.copy_source_rows != Some(3)
        || progress.copy_target_rows != Some(3)
        || !dirty.is_empty()
    {
        return Err(StoreError::RecordContract(
            "dirty probe cursor, coverage or queue did not advance",
        ));
    }
    Ok(())
}

fn dirty_replay(tx: &rusqlite::Transaction<'_>, n: i64) -> StoreResult<()> {
    let mut typed = canvas_tx(tx);
    typed.insert_new(&DirtySource { id: n, value: n })?;
    typed.update_existing(&DirtySource { id: -1, value: n })?;
    typed.delete::<DirtySource>(&(n - 1,))?;
    let from = if n % 2 == 0 { -2 } else { -3 };
    let to = if from == -2 { -3 } else { -2 };
    db_update!(
        typed,
        DirtySource,
        set(DirtySource::ID = &to),
        filter(DirtySource::ID.eq(&from))
    )?;
    if online_migration::step(tx, &dirty_plan(), &SystemClock::new())? != Step::Progress {
        return Err(StoreError::RecordContract(
            "dirty probe unexpectedly completed",
        ));
    }
    check_cursor(tx, n)?;
    // Fail a partial batch instead of reporting a different amount of work.
    for (id, value) in [(n, n), (-1, n), (to, 0)] {
        if typed.get::<DirtyTarget>(&(id,))?.map(|row| row.value) != Some(value) {
            return Err(StoreError::RecordContract(
                "dirty probe did not replay all writes",
            ));
        }
    }
    if typed.get::<DirtyTarget>(&(n - 1,))?.is_some()
        || typed.get::<DirtyTarget>(&(from,))?.is_some()
    {
        return Err(StoreError::RecordContract(
            "dirty probe retained a deleted key",
        ));
    }
    Ok(())
}

pub enum Observation {
    Other,
    Write {
        changed: bool,
        before: i64,
        after: i64,
    },
    Resume {
        high: i64,
        rows: usize,
        resumed: Option<i64>,
    },
}

fn publication(tx: &rusqlite::Transaction<'_>, family: usize, n: u64) -> StoreResult<Observation> {
    #[derive(Serialize)]
    struct Revision {
        revision: u64,
    }
    let before = SyncDocumentsRepository::entity_sequence(tx)?;
    if family == 12 {
        let replay = SyncDocumentsRepository::entities_after(tx, before - 1, 8)?;
        if !replay.iter().any(|row| {
            row.collection == "benchmark_f1"
                && row.id == "publication"
                && row.deleted
                && row.sequence == before
        }) {
            return Err(StoreError::RecordContract(
                "publication resume lost tombstone",
            ));
        }
        Ok(Observation::Resume {
            high: before,
            rows: replay.len(),
            resumed: replay.last().map(|row| row.sequence),
        })
    } else {
        let changed = SyncDocumentsRepository::put_entity(
            tx,
            "benchmark_f1",
            "publication",
            Revision { revision: n },
            family == 11,
        )?;
        let after = SyncDocumentsRepository::entity_sequence(tx)?;
        if changed != (family != 10)
            || (family == 10 && after != before)
            || (family != 10 && after <= before)
        {
            return Err(StoreError::RecordContract(
                "publication change/no-op clock mismatch",
            ));
        }
        Ok(Observation::Write {
            changed,
            before,
            after,
        })
    }
}

#[derive(Serialize)]
pub struct ProcessReport {
    frames: u64,
    cpu_ms: u64,
    cpu_ms_per_frame: Option<f64>,
    final_rss_bytes: u64,
}
pub struct ProcessMeasurement {
    system: sysinfo::System,
    pid: sysinfo::Pid,
    initial_cpu_ms: u64,
}
impl ProcessMeasurement {
    fn refresh(&mut self) -> Result<(u64, u64)> {
        self.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[self.pid]),
            true,
            sysinfo::ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory(),
        );
        let process = self
            .system
            .process(self.pid)
            .ok_or("benchmark process disappeared")?;
        Ok((process.accumulated_cpu_time(), process.memory()))
    }
    pub fn start() -> Result<Self> {
        let mut measurement = Self {
            system: sysinfo::System::new(),
            pid: sysinfo::get_current_pid()?,
            initial_cpu_ms: 0,
        };
        measurement.initial_cpu_ms = measurement.refresh()?.0;
        Ok(measurement)
    }
    pub fn finish(&mut self, frames: u64) -> Result<ProcessReport> {
        let (cpu, rss) = self.refresh()?;
        let cpu_ms = cpu.saturating_sub(self.initial_cpu_ms);
        Ok(ProcessReport {
            frames,
            cpu_ms,
            cpu_ms_per_frame: (frames > 0).then(|| cpu_ms as f64 / frames as f64),
            final_rss_bytes: rss,
        })
    }
}

pub fn require_wal(root: &Path) -> Result<()> {
    for name in ["canvas.sqlite3", "analytics.sqlite3"] {
        let db = Connection::open_with_flags(root.join(name), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mode: String = db.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if mode != "wal" {
            return Err(format!("{name}: expected WAL, got {mode}").into());
        }
    }
    Ok(())
}

fn hot_event(n: u64) -> QueueEvent {
    let mut row = event(n);
    // Exercise admission into the latest window even on a current copied state.
    row.created = now_ms() as f64 / 1000.0 + n as f64 / 1000.0;
    row
}

pub async fn seed(store: &RuntimeStore, rows: u64) -> Result<()> {
    store
        .transaction_with(move |tx| -> StoreResult<()> {
            for n in 0..rows {
                let row = hot_event(n);
                EventWriteRepository::insert_event(tx, &row)?;
                let metadata: studio_runtime_schema::event_metadata::EventMetadataRecord =
                    studio_json::from_str(
                        r#"{"delivery":"next_turn","assets":[],"radioAnswerTurnId":null}"#,
                    )
                    .map_err(|e| StoreError::SchemaValidation(e.to_string()))?;
                QueueWriteRepository::put_typed_metadata(tx, row.id.as_ref(), &metadata)?;
            }
            let pending = QueueWriteRepository::pending_events(tx, AGENT, 1)?;
            if pending.is_empty()
                || pending.iter().any(|row| {
                    QueueWriteRepository::typed_metadata(tx, &row.id)
                        .map_or(true, |value| value.is_none())
                })
            {
                return Err(StoreError::SchemaValidation(
                    "repair benchmark requires nonempty metadata".into(),
                ));
            }
            if !SchedulerRepository::invalid_pending_metadata(tx, AGENT, 1)?.is_empty() {
                return Err(StoreError::SchemaValidation(
                    "repair benchmark metadata is invalid".into(),
                ));
            }
            TerminalsRepository::append_output(
                tx,
                AGENT,
                0,
                &"synthetic terminal replay\n".repeat(rows as usize),
            )?;
            DirtySource::descriptor().install_if_absent(tx)?;
            DirtyTarget::descriptor().install_if_absent(tx)?;
            let mut typed = canvas_tx(tx);
            for id in [
                -1,
                if rows.is_multiple_of(2) { -2 } else { -3 },
                rows as i64 - 1,
            ] {
                typed.insert_new(&DirtySource { id, value: 0 })?;
            }
            online_migration::begin(tx, &dirty_plan())?;
            if online_migration::step(tx, &dirty_plan(), &SystemClock::new())? != Step::Progress {
                return Err(StoreError::RecordContract(
                    "dirty probe initialization completed",
                ));
            }
            check_cursor(tx, rows as i64 - 1)?;
            Ok(())
        })
        .await?;
    Ok(())
}

// Each family invokes an existing production adapter; no benchmark-owned SELECTs.
fn read(tx: &rusqlite::Transaction<'_>, family: usize) -> StoreResult<()> {
    match family {
        0 => {
            std::hint::black_box(AgentIndexedReads::slots_by_account(tx)?);
        }
        1 => {
            std::hint::black_box(AgentIndexedReads::team_records(tx, AGENT)?);
        }
        2 => {
            std::hint::black_box(SchedulerRepository::account_wake_page(
                tx,
                "",
                &BTreeMap::new(),
                32,
                128,
                1700000001.0,
            )?);
        }
        3 => {
            std::hint::black_box(QueueWriteRepository::pending_events(tx, AGENT, 1)?);
        }
        4 => {
            std::hint::black_box(SchedulerRepository::invalid_pending_metadata(tx, AGENT, 1)?);
        }
        8 => {
            std::hint::black_box(TerminalsRepository::archive_chunks(
                tx,
                AGENT,
                0,
                i64::MAX,
                128,
            )?);
        }
        _ => unreachable!(),
    }
    Ok(())
}

pub async fn operation(store: &RuntimeStore, family: usize, n: u64) -> Result<Observation> {
    match family {
        // Insert exercises the actual newest-200 publication triggers including retirement.
        5 => {
            store
                .transaction_with(move |tx| EventWriteRepository::insert_event(tx, &hot_event(n)))
                .await?;
        }
        6 => {
            std::hint::black_box(
                studio_runtime_services::transcript_runtime::page(
                    store,
                    AGENT.into(),
                    Default::default(),
                )
                .await?,
            );
        }
        7 => {
            std::hint::black_box(
                studio_runtime_services::transcript::search(
                    store,
                    AGENT.into(),
                    "synthetic".into(),
                    100,
                )
                .await?,
            );
        }
        9..=12 => {
            return Ok(store
                .transaction_with(move |tx| publication(tx, family, n))
                .await?);
        }
        13 => {
            store
                .transaction_with(move |tx| dirty_replay(tx, n as i64))
                .await?;
        }
        _ => {
            store
                .read_async("storage-benchmark/hot-path", move |tx| read(tx, family))
                .await?;
        }
    }
    Ok(Observation::Other)
}

pub async fn read_plans(store: &RuntimeStore) -> Result<ReadPlans> {
    store
        .read_async(
            "storage-benchmark/hot-trace-start",
            |tx| -> StoreResult<()> {
                tx.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace));
                Ok(())
            },
        )
        .await?;
    // Reader pool connections are not guaranteed to repeat: trace writer reads
    // synchronously for the repository cases; services are captured separately below.
    let mut canvas = store
        .transaction_with(|tx| -> StoreResult<_> {
            tx.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace));
            for family in [0, 1, 2, 3, 4, 8] {
                read(tx, family)?;
            }
            plans(tx)
        })
        .await?;
    operation(store, 6, 0).await?;
    operation(store, 7, 0).await?;
    canvas.extend(
        store
            .read_async("storage-benchmark/hot-service-plans", |tx| plans(tx))
            .await?,
    );
    Ok(ReadPlans {
        canvas,
        analytics: Vec::new(),
    })
}

// Bytes: staging + VACUUM temporary work + measured DB/WAL must fit the
// advertised two-database-budget temporary-space cap before any file copy.
pub fn check_copy_budget(source_bytes: u64, budget: u64, available_bytes: u64) -> Result<()> {
    let temporary_budget = budget.checked_mul(2).ok_or("copy budget overflow")?;
    let footprint = source_bytes
        .checked_mul(2)
        .and_then(|v| v.checked_add(budget))
        .ok_or("copy size overflow")?;
    if source_bytes > budget || footprint > temporary_budget {
        return Err(
            "combined database/WAL/SHM exceeds copy budget including VACUUM overhead".into(),
        );
    }
    if available_bytes < temporary_budget {
        return Err("insufficient free space for staging, VACUUM and measured database/WAL".into());
    }
    Ok(())
}

pub fn frame_pending(
    hot: bool,
    completed: u64,
    target: u64,
    elapsed: Duration,
    limit: Duration,
) -> Result<bool> {
    if hot && elapsed >= limit {
        return Err(
            "fixed-frame hot-path benchmark timed out; partial run is not comparable".into(),
        );
    }
    Ok(completed < target && (hot || elapsed < limit))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn vacuum_copy_preserves_wal_before_store_validation() {
        use std::io::{Read, Write};

        let source = tempfile::tempdir().unwrap();
        let measured = tempfile::tempdir().unwrap();
        let live = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(source.path())).await.unwrap();
        drop(store);
        require_wal(source.path()).unwrap();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let server = format!("http://{}", listener.local_addr().unwrap());
        let body = format!(
            "{{\"stateDir\":{}}}",
            studio_json::to_string(live.path()).unwrap()
        );
        let discovery = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            socket.read(&mut request).unwrap();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let options = Options {
            seconds: 1,
            rows: 8,
            samples: 8,
            state: Some(source.path().to_path_buf()),
            server: Some(server),
            label: "wal-regression".into(),
            hot_paths: true,
        };
        copy_state(&options, measured.path()).unwrap();
        discovery.join().unwrap();
        require_wal(measured.path()).unwrap();
        RuntimeStore::open(Some(measured.path())).await.unwrap();
    }

    #[test]
    fn f1_probes_are_in_every_fixed_frame() {
        for name in [
            "f1_publication_change",
            "f1_publication_noop",
            "f1_publication_delete",
            "f1_publication_resume",
        ] {
            assert!(FAMILIES.contains(&name));
        }
        assert!(FAMILIES.contains(&"f1_dirty_replay"));
    }
    fn check_frame(tx: &rusqlite::Transaction<'_>, n: u64) -> StoreResult<()> {
        let row = canvas_tx(tx)
            .get::<studio_runtime_store::clock_schema::SyncEntities>(&(
                "benchmark_f1".to_owned(),
                "publication".to_owned(),
            ))?
            .ok_or(StoreError::RecordContract(
                "publication dispatch did no work",
            ))?;
        if row.deleted != 1 || row.seq != SyncDocumentsRepository::entity_sequence(tx)? {
            return Err(StoreError::RecordContract(
                "publication dispatch did not publish a tombstone",
            ));
        }
        check_cursor(tx, n as i64)?;
        let moved = if n % 2 == 0 { -3 } else { -2 };
        let typed = canvas_tx(tx);
        for (id, value) in [(n as i64, n as i64), (-1, n as i64), (moved, 0)] {
            if typed.get::<DirtyTarget>(&(id,))?.map(|row| row.value) != Some(value) {
                return Err(StoreError::RecordContract("dirty dispatch did no work"));
            }
        }
        Ok(())
    }
    #[tokio::test]
    async fn f1_frames_publish_and_replay_all_four_dirty_mutations() {
        let directory = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(directory.path())).await.unwrap();
        super::super::seed(&store, 2).await.unwrap();
        super::seed(&store, 2).await.unwrap();
        // Independent observations must reject a skipped dispatch.
        assert!(
            store
                .read_async("missing-f1-frame", |tx| check_frame(tx, 2))
                .await
                .is_err()
        );
        for n in 2..10 {
            for family in 9..=13 {
                let before = store
                    .read_async("publication-clock-before", |tx| {
                        SyncDocumentsRepository::entity_sequence(tx)
                    })
                    .await
                    .unwrap();
                let observed = operation(&store, family, n).await.unwrap();
                let after = store
                    .read_async("publication-clock-after", |tx| {
                        SyncDocumentsRepository::entity_sequence(tx)
                    })
                    .await
                    .unwrap();
                match (family, observed) {
                    (
                        9..=11,
                        Observation::Write {
                            changed,
                            before: reported_before,
                            after: reported_after,
                        },
                    ) => {
                        assert_eq!((reported_before, reported_after), (before, after));
                        assert_eq!(changed, family != 10);
                        if family == 10 {
                            assert_eq!(after, before);
                        } else {
                            assert!(after > before);
                        }
                    }
                    (
                        12,
                        Observation::Resume {
                            high,
                            rows,
                            resumed,
                        },
                    ) => {
                        assert_eq!((high, after), (before, before));
                        assert_eq!(rows, 1);
                        assert_eq!(resumed, Some(high));
                    }
                    (13, Observation::Other) => {}
                    _ => panic!("publication family {family} returned no observable result"),
                }
            }
            store
                .read_async("verify-f1-frame", move |tx| check_frame(tx, n))
                .await
                .unwrap();
        }
        store
            .transaction_with(|tx| -> StoreResult<()> {
                let mut typed = canvas_tx(tx);
                let mut progress = typed
                    .get::<PayloadMigrations>(&("online:benchmark_f1_dirty".to_owned(),))?
                    .unwrap();
                // Reproduce budget exhaustion after dirty rows match but before the high-key cursor advances.
                progress.cursor = 8;
                progress.cursor_key = Some(studio_db_api::migration::encode_key(&(8i64,)));
                typed.update_existing(&progress)?;
                assert!(check_cursor(tx, 9).is_err());
                Ok(())
            })
            .await
            .unwrap();
    }
    #[test]
    fn sidecars_and_copy_overhead_must_fit_before_copying() {
        let gib = 1024 * 1024 * 1024;
        assert!(
            check_copy_budget(17 * gib, 16 * gib, 100 * gib).is_err(),
            "oversized WAL must refuse"
        );
        assert!(
            check_copy_budget(6 * gib, 16 * gib, 31 * gib).is_err(),
            "32 GiB free-space floor"
        );
        assert!(
            check_copy_budget(15 * gib, 16 * gib, 40 * gib).is_err(),
            "staging, vacuum and WAL headroom"
        );
        assert!(check_copy_budget(6 * gib, 16 * gib, 40 * gib).is_ok());
    }
    #[test]
    fn fixed_frames_fail_instead_of_reporting_partial_work() {
        let limit = Duration::from_secs(60);
        assert!(frame_pending(true, 20, 100, limit, limit).is_err());
        assert!(frame_pending(true, 100, 100, limit, limit).is_err());
        assert!(!frame_pending(true, 100, 100, Duration::from_secs(1), limit).unwrap());
        assert!(frame_pending(true, 20, 100, Duration::from_secs(1), limit).unwrap());
        assert!(!frame_pending(false, 20, 100, limit, limit).unwrap());
    }
    #[tokio::test]
    async fn seeded_repair_validates_nonempty_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(directory.path())).await.unwrap();
        super::super::seed(&store, 2).await.unwrap();
        super::seed(&store, 2).await.unwrap();
        store
            .transaction_with(|tx| -> StoreResult<()> {
                for n in 0..2 {
                    assert!(
                        QueueWriteRepository::typed_metadata(tx, hot_event(n).id.as_ref())?
                            .is_some()
                    );
                }
                assert!(SchedulerRepository::invalid_pending_metadata(tx, AGENT, 1)?.is_empty());
                Ok(())
            })
            .await
            .unwrap();
    }
}
