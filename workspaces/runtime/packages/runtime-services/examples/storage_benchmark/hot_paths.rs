//! P0-C workloads reuse the storage driver's copy, budgets and timing machinery.
use super::*;
use studio_runtime_store::{AgentIndexedReads, SchedulerRepository, TerminalsRepository};

pub const FAMILIES: [&str; 9] = [
    "agent_account_selection",
    "agent_root_selection",
    "agent_wake_selection",
    "queue_dispatch_selection",
    "queue_repair_metadata",
    "queue_latest_200_publication",
    "transcript_paging",
    "transcript_search",
    "terminal_replay",
];

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

pub async fn operation(store: &RuntimeStore, family: usize, n: u64) -> Result<()> {
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
        _ => {
            store
                .read_async("storage-benchmark/hot-path", move |tx| read(tx, family))
                .await?;
        }
    }
    Ok(())
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
