//! Manual heavy-tier elapsed-time benchmark. Never registered as an ordinary test.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::trace::{TraceEvent, TraceEventCodes};
use rusqlite::{Connection, OpenFlags, types::Value as SqlValue};

use studio_runtime_schema::persistence::{EventStatus, RequestIdentity, ToolRequestStatus};
use studio_runtime_schema::provider_kind::ProviderKind;
use studio_runtime_schema::queue_command::Field;
use studio_runtime_schema::transcript_record::TranscriptRole;
use studio_runtime_schema::usage::{UsageEvidence, UsageObservation, UsageTokens};
use studio_runtime_services::{monitor_output::MonitorOutput, tool_requests};
use studio_runtime_store::analytics_capture::{
    AnalyticsCaptureRepository as Capture, CaptureTable,
};
use studio_runtime_store::request_results::{Admission, RequestResults, ResultEnvelope};
use studio_runtime_store::{
    AgentActivity, AgentRecord, AgentRepository, AgentStatus, EventWriteRepository,
    MonitorOperation, MonitorRecord, QueueEvent, QueueWriteRepository, RuntimeStore, StoreError,
    StoreResult, SyncDocumentsRepository, TranscriptRepository, monitor_runtime,
};

#[path = "storage_benchmark/hot_paths.rs"]
mod hot_paths;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const AGENT: &str = "storage-benchmark-agent";
const MONITOR: &str = "storage-benchmark-monitor";
const FAMILIES: [&str; 8] = [
    "agent_activity_tokens",
    "transcript_append",
    "tool_request_receipt",
    "queue_enqueue",
    "queue_consume",
    "monitor_output_append",
    "analytics_usage_cost",
    "sse_catch_up",
];
// Benchmark budget owners: wall seconds, fixture rows, samples per family, database bytes.
// Hot-path copies include the 5.7 GiB main fixture. Unit: database + WAL bytes.
// Identical hot-path frames per revision; timeout fails instead of truncating.
const HOT_FIXED_FRAMES: u64 = 100;
const HOT_DATABASE_BUDGET_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const DEFAULT_SECONDS: u64 = 60;
const DEFAULT_ROWS: u64 = 1000;
const MAX_SAMPLES: u64 = 10_000;
const BUSY_RETRY_LIMIT: i32 = 15_000;
const BUSY_DELAY_MS: u64 = 1;
const DATABASE_BUDGET_BYTES: u64 = 2 * 1024 * 1024 * 1024;
static BUSY_NS: AtomicU64 = AtomicU64::new(0);
static BUSY_CALLS: AtomicU64 = AtomicU64::new(0);
static READ_SQL: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

// This module owns the typed report contract and named synthetic fixtures.
#[derive(Serialize)]
struct Distribution {
    samples: usize,
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    p99_ms: Option<f64>,
}
#[derive(Serialize)]
struct FamilyReport {
    name: &'static str,
    transaction_wall: Distribution,
    busy_wait: BusyMeasurement,
}
#[derive(Serialize)]
#[serde(tag = "status", content = "distribution", rename_all = "snake_case")]
enum BusyMeasurement {
    Measured(Distribution),
    Unavailable,
}
// Named synthetic fixtures reuse domain owners for their closed values.
#[derive(Serialize)]
struct TranscriptFixture {
    id: String,
    role: TranscriptRole,
    text: &'static str,
}
#[derive(Serialize)]
struct PublicationFixture {
    synthetic: u64,
}
#[derive(Serialize)]
struct AnalyticsAgentFixture {
    id: &'static str,
    provider: ProviderKind,
}
#[derive(Serialize)]
struct EmptyLegacyTokens {}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageFixture {
    id: String,
    agent_id: &'static str,
    root_id: &'static str,
    thread_id: &'static str,
    at: f64,
    response_id: String,
    last: EmptyLegacyTokens,
    total: EmptyLegacyTokens,
    studio_usage: UsageObservation,
}
#[derive(Serialize)]
struct SyntheticReply {
    synthetic: bool,
}
#[derive(Serialize)]
struct CallbackContext {
    agent: &'static str,
}
#[derive(Serialize)]
struct CallbackIdentity {
    context: CallbackContext,
}
#[derive(Serialize)]
struct CallbackPayload {
    identity: CallbackIdentity,
    synthetic: bool,
}
#[derive(Serialize)]
struct DatabaseSize {
    db_bytes: u64,
    wal_bytes: u64,
}
type Sizes = BTreeMap<&'static str, DatabaseSize>;
#[derive(Serialize)]
struct PlanStep {
    id: i64,
    parent: i64,
    detail: String,
}
#[derive(Serialize)]
struct QueryPlan {
    sql: String,
    bindings: &'static str,
    plan: Vec<PlanStep>,
}
#[derive(Serialize)]
struct ReadPlans {
    canvas: Vec<QueryPlan>,
    analytics: Vec<QueryPlan>,
}
#[derive(Serialize)]
struct Build {
    profile: &'static str,
    sqlite: &'static str,
    architecture: &'static str,
}
#[derive(Serialize)]
struct Fixture {
    source: &'static str,
    seed_rows: u64,
    payload: &'static str,
    producer_threads: u8,
}
#[derive(Serialize)]
struct Budget {
    seconds: u64,
    samples_per_family: u64,
    database_bytes: u64,
}
#[derive(Serialize)]
struct BusyReport {
    callbacks: u64,
    sleep_ms: f64,
    terminal_lock_errors: u64,
    retry_delay_ms: u64,
    retry_limit: i32,
}
#[derive(Serialize)]
struct CheckpointReport {
    elapsed_ms: f64,
    busy: i64,
    log_frames: i64,
    checkpointed_frames: i64,
}
#[derive(Serialize)]
struct Report {
    format_version: u8,
    build: Build,
    label: String,
    timestamp_ms: i64,
    fixture: Fixture,
    budget: Budget,
    measured_seconds: f64,
    process: hot_paths::ProcessReport,
    families: Vec<FamilyReport>,
    busy: BusyReport,
    wal_checkpoint: BTreeMap<&'static str, CheckpointReport>,
    initial_sizes: Sizes,
    before_checkpoint_sizes: Sizes,
    final_sizes: Sizes,
    explain_query_plan: ReadPlans,
    measurement: &'static str,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopState {
    state_dir: PathBuf,
}

// Instrumented busy policy is deliberately identical for old/new comparisons:
// sleep 1ms per retry, at most 15s. It replaces SQLite's opaque default backoff.
#[expect(
    clippy::disallowed_methods,
    reason = "SQLite busy callbacks run synchronously on the dedicated writer actor thread, never a Tokio worker"
)]
fn busy(retry: i32) -> bool {
    BUSY_CALLS.fetch_add(1, Ordering::Relaxed);
    if retry >= BUSY_RETRY_LIMIT {
        return false;
    }
    let start = Instant::now();
    std::thread::sleep(Duration::from_millis(BUSY_DELAY_MS));
    BUSY_NS.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
    true
}

fn trace(event: TraceEvent<'_>) {
    if let TraceEvent::Stmt(statement, _) = event {
        let sql = statement.sql();
        if sql.trim_start().to_ascii_uppercase().starts_with("SELECT")
            || sql.trim_start().to_ascii_uppercase().starts_with("WITH")
        {
            READ_SQL
                .lock()
                .expect("SQL collector")
                .insert(sql.into_owned());
        }
    }
}

fn plans(db: &Connection) -> StoreResult<Vec<QueryPlan>> {
    db.trace_v2(TraceEventCodes::empty(), None);
    let sql = std::mem::take(&mut *READ_SQL.lock().expect("SQL collector"));
    sql.into_iter()
        .map(|sql| {
            let mut statement = db.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
            let bindings = vec![SqlValue::Null; statement.parameter_count()];
            let plan = statement
                .query_map(rusqlite::params_from_iter(bindings), |row| {
                    Ok(PlanStep {
                        id: row.get(0)?,
                        parent: row.get(1)?,
                        detail: row.get(3)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(QueryPlan {
                sql,
                bindings: "NULL (plan only; no copied payloads)",
                plan,
            })
        })
        .collect()
}

struct Options {
    seconds: u64,
    rows: u64,
    samples: u64,
    state: Option<PathBuf>,
    server: Option<String>,
    label: String,
    hot_paths: bool,
}
fn options() -> Result<Options> {
    let mut options = Options {
        seconds: DEFAULT_SECONDS,
        rows: DEFAULT_ROWS,
        samples: MAX_SAMPLES,
        state: None,
        server: None,
        label: "unlabelled".into(),
        hot_paths: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--hot-paths" {
            options.hot_paths = true;
            options.samples = options.samples.min(HOT_FIXED_FRAMES);
            continue;
        }
        if arg == "--smoke" {
            options.seconds = 1;
            options.rows = 8;
            options.samples = 8;
            continue;
        }
        if arg == "--help" {
            println!(
                "storage_benchmark [--hot-paths] [--smoke] [--seconds 60] [--rows 1000] [--label revision] [--state COPY --server URL]\nJSON: stdout; table: stderr. One sequential producer; max 10000 samples/family.\nCopy mode requires live-state discovery through /api/desktop.\nNo real data is printed. Runs always mutate a disposable temporary database."
            );
            std::process::exit(0);
        }
        let value = args.next().ok_or("missing argument value")?;
        match arg.as_str() {
            "--seconds" => options.seconds = value.parse()?,
            "--rows" => options.rows = value.parse()?,
            "--label" => options.label = value,
            "--state" => options.state = Some(value.into()),
            "--server" => options.server = Some(value),
            _ => return Err(format!("unknown option {arg}").into()),
        }
    }
    if options.seconds == 0 || options.seconds > 600 || options.rows == 0 || options.rows > 10_000 {
        return Err("seconds must be 1..600; rows must be 1..10000".into());
    }
    Ok(options)
}

// Fail closed if live-state discovery fails. VACUUM INTO always creates
// the disposable copy that receives benchmark writes.
fn copy_state(options: &Options, destination: &Path) -> Result<()> {
    let Some(copy) = &options.state else {
        return Ok(());
    };
    let server = options
        .server
        .as_ref()
        .ok_or("--state requires --server to identify the live state")?;
    let url = url::Url::parse(server)?;
    if !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")) {
        return Err("--server must identify this device's loopback server".into());
    }
    let config = ureq::Agent::config_builder()
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(5)))
        .build();
    let body = ureq::Agent::new_with_config(config)
        .get(&format!("{}/api/desktop", server.trim_end_matches('/')))
        .call()?
        .body_mut()
        .read_to_string()?;
    let desktop: DesktopState = studio_json::from_str(&body)?;
    let live =
        studio_runtime_store::path_identity::CanonicalPath::existing_directory(desktop.state_dir)?;
    let copy = studio_runtime_store::path_identity::CanonicalPath::existing_directory(copy)?;
    if copy.starts_with(&live) || live.starts_with(&copy) {
        return Err("refusing live state directory or its ancestors/descendants".into());
    }
    // Validate every database and existing SQLite sidecar before opening any file.
    let staging = tempfile::tempdir_in(destination.parent().ok_or("missing copy parent")?)?;
    let mut inputs = Vec::new();
    let mut files = Vec::new();
    for name in ["canvas.sqlite3", "analytics.sqlite3"] {
        let input = studio_runtime_store::path_identity::CanonicalPath::existing(copy.join(name))?;
        refuse_live_path(input.as_path(), live.as_path())?;
        files.push((name.to_owned(), input.as_path().to_path_buf()));
        for suffix in ["-wal", "-shm"] {
            let mut sidecar = input.as_os_str().to_os_string();
            sidecar.push(suffix);
            let sidecar = PathBuf::from(sidecar);
            match std::fs::symlink_metadata(&sidecar) {
                Ok(_) => {
                    let resolved =
                        studio_runtime_store::path_identity::CanonicalPath::existing(sidecar)?;
                    refuse_live_path(resolved.as_path(), live.as_path())?;
                    files.push((format!("{name}{suffix}"), resolved.as_path().to_path_buf()));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        if std::fs::metadata(input.as_path())?.len() > database_budget(options) / 2 {
            return Err("copy exceeds per-database budget".into());
        }
        inputs.push((name, input));
    }
    let source_bytes = files
        .iter()
        .try_fold(0_u64, |bytes, (_, path)| -> Result<u64> {
            let metadata = std::fs::metadata(path)?;
            if !metadata.is_file() {
                return Err("source database/sidecar must be a file".into());
            }
            bytes
                .checked_add(metadata.len())
                .ok_or_else(|| "source size overflow".into())
        })?;
    let space = rustix::fs::statvfs(destination)?;
    let available_bytes = space
        .f_bavail
        .checked_mul(space.f_frsize)
        .ok_or("free space overflow")?;
    hot_paths::check_copy_budget(source_bytes, database_budget(options), available_bytes)?;
    // SQLite never opens the supplied source, even read-only (WAL can create SHM).
    for (name, input) in files {
        std::fs::copy(input, staging.path().join(name))?;
    }
    for (name, _) in inputs {
        let db = Connection::open_with_flags(
            staging.path().join(name),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        db.busy_timeout(Duration::from_secs(1))?;
        db.execute(
            "VACUUM INTO ?1",
            [destination
                .join(name)
                .to_str()
                .ok_or("invalid destination")?],
        )?;
    }
    Ok(())
}

fn database_budget(options: &Options) -> u64 {
    if options.hot_paths {
        HOT_DATABASE_BUDGET_BYTES
    } else {
        DATABASE_BUDGET_BYTES
    }
}

fn refuse_live_path(path: &Path, live: &Path) -> Result<()> {
    if path.starts_with(live) || live.starts_with(path) {
        return Err("refusing live state path or its ancestors/descendants".into());
    }
    Ok(())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64
}
fn identity(n: u64, now: i64) -> Result<RequestIdentity> {
    let root = uuid::Uuid::from_u128(((now as u128) << 80) | (7 << 76) | (2 << 62) | u128::from(n));
    Ok(RequestIdentity::new(
        "storage_benchmark",
        &[AGENT],
        &root.to_string(),
        &["tool"],
    )?)
}
fn agent(n: Option<u64>) -> Result<AgentRecord> {
    let mut record = AgentRecord::new(
        AGENT.into(),
        AGENT.into(),
        1,
        AgentStatus::Running,
        "default".into(),
    );
    record.provider = Field::Value(ProviderKind::Codex);
    record.auto_wake = Some(true);
    record.compactions = Some(0);
    record.agent_mode = Some(studio_runtime_schema::agent_entity_nested::AgentMode::Multi);
    record.concurrency = Some(1.try_into()?);
    if let Some(n) = n {
        record.events = Some(n);
        record.tokens_used = Some(n);
        record.tail = Some(format!("synthetic output {n}"));
        record.activity = Some(AgentActivity {
            at: Some(1700000001.0 + n as f64 / 1000.0),
            ..Default::default()
        });
    }
    Ok(record)
}
fn usage(n: u64) -> Result<UsageFixture> {
    let response_id = format!("storage-benchmark-response-{n}");
    Ok(UsageFixture {
        id: format!("storage-benchmark-usage-{n}"),
        agent_id: AGENT,
        root_id: AGENT,
        thread_id: "storage-benchmark-thread",
        at: 1700000000.0,
        response_id: response_id.clone(),
        last: EmptyLegacyTokens {},
        total: EmptyLegacyTokens {},
        studio_usage: UsageObservation::Response {
            response_id,
            turn_id: None,
            model: "storage-benchmark-model".into(),
            tokens: UsageTokens::new(100, 10, 0, None, 20, None)?,
            evidence: UsageEvidence::Live,
            parent_tool_use_id: None,
            model_context_window: None,
        },
    })
}
fn write_usage(tx: &rusqlite::Transaction<'_>, row: &UsageFixture) -> StoreResult<()> {
    // Legacy Value API boundary; UsageObservation owns the token evidence.
    Capture::write(
        tx,
        CaptureTable::Usage,
        &studio_runtime_store::projection_value(row),
    )
}
fn transcript(tx: &rusqlite::Transaction<'_>, n: u64) -> StoreResult<()> {
    let row = TranscriptFixture {
        id: format!("storage-benchmark-item-{n}"),
        role: TranscriptRole::Assistant,
        text: "synthetic transcript",
    };
    // Legacy Value API boundary for the typed transcript fixture.
    TranscriptRepository::put_item(
        tx,
        &row.id,
        AGENT,
        &studio_runtime_store::projection_value(&row),
        1700000000.0,
        row.text,
        true,
    )
    .map_err(|e| StoreError::SchemaValidation(e.to_string()))
}
fn event(n: u64) -> QueueEvent {
    QueueEvent {
        id: format!("storage-benchmark-event-{n}").as_str().into(),
        agent: AGENT.into(),
        kind: "user".into(),
        text: "synthetic benchmark message".into(),
        status: "pending".into(),
        created: 1700000000.0 + n as f64 / 1000.0,
        epoch: 1,
        turn_id: None,
        error: None,
    }
}

fn monitor_operation() -> Result<MonitorOperation> {
    Ok(MonitorOperation {
        account_key: "default".to_owned().try_into()?,
        agent: AGENT.to_owned().try_into()?,
        connection_id: "storage-benchmark-connection".to_owned().try_into()?,
        epoch: 1.try_into()?,
        evidence: Default::default(),
    })
}

async fn seed(store: &RuntimeStore, rows: u64) -> Result<()> {
    let root = store.root();
    // Parse a static fixture through its typed owner; no intermediate JSON tree.
    let mut record: MonitorRecord = studio_json::from_str(
        r#"{"id":"fixture","agent":"fixture","epoch":1,"status":"running","command":"synthetic","created":1700000000.0,"tail":"","requestId":"synthetic-request","activityAt":1700000000.0,"activityGeneration":0,"bytes":0,"cwd":"fixture","interactive":false,"livenessCommand":"","log":"fixture","stallTimeoutSeconds":1800,"stallWakeGeneration":-1,"successExitCodes":[0],"timeout_ms":3600000,"wakeOn":"exit"}"#,
    )?;
    record.id = MONITOR.into();
    record.agent = AGENT.into();
    record.cwd = root.to_string_lossy().into_owned();
    record.log = root
        .join("monitor-logs")
        .join(format!("{MONITOR}.log"))
        .to_string_lossy()
        .into_owned();
    record.operation = Some(monitor_operation()?);
    let agent = agent(None)?;
    let usage_rows = (0..rows).map(usage).collect::<Result<Vec<_>>>()?;
    store
        .transaction_with(move |tx| -> StoreResult<()> {
            AgentRepository::put(tx, &agent)?;

            monitor_runtime::put(tx, &record)?;
            for n in 0..rows {
                transcript(tx, n)?;
                SyncDocumentsRepository::put_entity(
                    tx,
                    "storageBenchmark",
                    &format!("row-{n}"),
                    PublicationFixture { synthetic: n },
                    false,
                )?;
            }
            tx.busy_handler(Some(busy))?;
            Ok(())
        })
        .await?;
    store
        .analytics_transaction(move |tx, _| -> StoreResult<()> {
            // Legacy Value API boundary for the named analytics-agent fixture.
            Capture::write(
                tx,
                CaptureTable::Agents,
                &studio_runtime_store::projection_value(&AnalyticsAgentFixture {
                    id: AGENT,
                    provider: ProviderKind::Codex,
                }),
            )?;
            for row in &usage_rows {
                write_usage(tx, row)?;
            }
            tx.busy_handler(Some(busy))?;
            Ok(())
        })
        .await?;
    Ok(())
}

async fn operation(
    store: &RuntimeStore,
    output: &MonitorOutput,
    family: usize,
    n: u64,
    offset: &mut u64,
) -> Result<()> {
    match family {
        0 => {
            let record = agent(Some(n))?;
            store
                .transaction_with(move |tx| AgentRepository::put(tx, &record))
                .await?;
        }
        1 => {
            store.transaction_with(move |tx| transcript(tx, n)).await?;
        }
        2 => {
            let now = now_ms();
            let identity = identity(n, now)?;
            store
                .transaction_with(
                    move |tx| -> std::result::Result<(), tool_requests::ToolRequestError> {
                        let admission =
                            RequestResults::admit(tx, &identity, "synthetic-signature", now)?;
                        let Admission::Fresh(admitted) = admission else {
                            return Err(tool_requests::ToolRequestError::InvalidRequest(
                                "benchmark identity collision",
                            ));
                        };
                        tool_requests::put_callback(
                            tx,
                            &tool_requests::CallbackTask {
                                id: identity.id().into(),
                                origin_key: format!("storage-benchmark-origin-{n}"),
                                status: ToolRequestStatus::Completed,
                                updated: now,
                                payload: CallbackPayload {
                                    identity: CallbackIdentity {
                                        context: CallbackContext { agent: AGENT },
                                    },
                                    synthetic: true,
                                },
                            },
                        )?;
                        RequestResults::settle(
                            tx,
                            &admitted,
                            &ResultEnvelope {
                                actor: Some(AGENT.into()),
                                reply: SyntheticReply { synthetic: true },
                                references: vec![],
                            },
                        )?;
                        Ok(())
                    },
                )
                .await?;
        }
        3 => {
            store
                .transaction_with(move |tx| EventWriteRepository::insert_event(tx, &event(n)))
                .await?;
        }
        4 => {
            store
                .transaction_with(move |tx| {
                    QueueWriteRepository::pending_events(tx, AGENT, 1)?;
                    QueueWriteRepository::update_status(
                        tx,
                        event(n).id.as_ref(),
                        EventStatus::Delivered.as_str(),
                    )
                })
                .await?;
        }
        5 => {
            let op = monitor_operation()?;
            let bytes = b"synthetic monitor output\n";
            output
                .append(
                    store,
                    MONITOR,
                    &op,
                    *offset,
                    bytes,
                    1700000001.0 + n as f64 / 1000.0,
                )
                .await?;
            *offset += bytes.len() as u64;
        }
        6 => {
            let row = usage(n)?;
            store
                .analytics_transaction(move |tx, _| -> StoreResult<()> {
                    write_usage(tx, &row)?;
                    Capture::generation(tx, &[AGENT.into()])
                })
                .await?;
        }
        7 => {
            store
                .read_async("storage-benchmark/catch-up", |tx| {
                    SyncDocumentsRepository::entities_after(tx, 0, 100)?;
                    studio_runtime_store::durable_seq::changes_since(tx, "runtime_agents", Some(0))
                })
                .await?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

async fn read_plans(store: &RuntimeStore) -> Result<ReadPlans> {
    let canvas = store
        .read_async("storage-benchmark/plans", |tx| -> StoreResult<_> {
            tx.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace));
            TranscriptRepository::item(tx, AGENT, "storage-benchmark-item-0")
                .map_err(|e| StoreError::SchemaValidation(e.to_string()))?;
            QueueWriteRepository::pending_events(tx, AGENT, 1)?;
            SyncDocumentsRepository::entities_after(tx, 0, 100)?;
            studio_runtime_store::durable_seq::changes_since(tx, "runtime_agents", Some(0))?;
            let _ =
                RequestResults::replay(tx, "synthetic-missing", "synthetic-signature", now_ms())
                    .map_err(|e| StoreError::SchemaValidation(e.to_string()))?;
            plans(tx)
        })
        .await?;
    let analytics = store
        .analytics_transaction(|tx, _| -> StoreResult<_> {
            tx.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace));
            Capture::usage_response(
                tx,
                AGENT,
                Some("storage-benchmark-thread"),
                "storage-benchmark-response-0",
            )?;
            studio_runtime_store::cost_aggregate::cost_groups(tx, AGENT, &Default::default())?;
            plans(tx)
        })
        .await?;
    Ok(ReadPlans { canvas, analytics })
}

fn percentile(samples: &mut [f64], fraction: f64) -> Option<f64> {
    samples.sort_by(f64::total_cmp);
    if samples.is_empty() {
        None
    } else {
        Some(samples[((samples.len() as f64 * fraction).ceil() as usize).saturating_sub(1)])
    }
}
fn distribution(mut samples: Vec<f64>) -> Distribution {
    Distribution {
        samples: samples.len(),
        p50_ms: percentile(&mut samples, 0.50),
        p95_ms: percentile(&mut samples, 0.95),
        p99_ms: percentile(&mut samples, 0.99),
    }
}
fn sizes(root: &Path) -> Result<Sizes> {
    let mut result = BTreeMap::new();
    for name in ["canvas.sqlite3", "analytics.sqlite3"] {
        let db_bytes = std::fs::metadata(root.join(name))?.len();
        let wal_bytes = std::fs::metadata(root.join(format!("{name}-wal"))).map_or(0, |m| m.len());
        result.insert(
            name,
            DatabaseSize {
                db_bytes,
                wal_bytes,
            },
        );
    }
    Ok(result)
}
fn checkpoints(root: &Path) -> Result<BTreeMap<&'static str, CheckpointReport>> {
    use studio_runtime_store::sqlite_durability::{
        CheckpointMode, checkpoint, configure_synchronous,
    };
    let mut result = BTreeMap::new();
    for name in ["canvas.sqlite3", "analytics.sqlite3"] {
        let db = Connection::open(root.join(name))?;
        configure_synchronous(&db)?;
        let began = Instant::now();
        let outcome = checkpoint(&db, CheckpointMode::Passive)?;
        result.insert(
            name,
            CheckpointReport {
                elapsed_ms: began.elapsed().as_secs_f64() * 1000.0,
                busy: outcome.busy,
                log_frames: outcome.log_frames,
                checkpointed_frames: outcome.checkpointed_frames,
            },
        );
    }
    Ok(result)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let options = options()?;
    if options.hot_paths && sysinfo::System::load_average().one >= 8.0 {
        return Err(format!(
            "hot-path measurement deferred: 1-minute load {} >= 8",
            sysinfo::System::load_average().one
        )
        .into());
    }
    let family_names: &[&str] = if options.hot_paths {
        &hot_paths::FAMILIES
    } else {
        &FAMILIES
    };
    eprintln!(
        "Manual storage benchmark: <= {}s measured workload; one sequential producer; <= {} samples/family; {} byte database budget; no providers or child commands. Run alone, outside ordinary checks.",
        options.seconds,
        options.samples,
        database_budget(&options)
    );
    let directory = tempfile::tempdir()?;
    copy_state(&options, directory.path())?;
    let store = RuntimeStore::open(Some(directory.path())).await?;
    seed(&store, options.rows).await?;
    if options.hot_paths {
        hot_paths::seed(&store, options.rows).await?;
    }
    hot_paths::require_wal(directory.path())?;
    let initial_sizes = sizes(directory.path())?;
    let output = MonitorOutput::default();
    let mut process = hot_paths::ProcessMeasurement::start()?;
    let start = Instant::now();
    let mut samples = vec![Vec::new(); family_names.len()];
    let mut waits = vec![Vec::new(); family_names.len()];
    let mut offset = 0;
    let mut iterations = 0;
    while hot_paths::frame_pending(
        options.hot_paths,
        iterations,
        options.samples,
        start.elapsed(),
        Duration::from_secs(options.seconds),
    )? {
        let n = options.rows + iterations;
        for family in 0..family_names.len() {
            let busy_before = BUSY_NS.load(Ordering::Relaxed);
            let began = Instant::now();
            if options.hot_paths {
                hot_paths::operation(&store, family, n).await?;
            } else {
                operation(&store, &output, family, n, &mut offset).await?;
            }
            samples[family].push(began.elapsed().as_secs_f64() * 1000.0);
            if family != 7 {
                waits[family].push((BUSY_NS.load(Ordering::Relaxed) - busy_before) as f64 / 1e6);
            }
        }
        iterations += 1;
        if iterations % 100 == 0 {
            let current = sizes(directory.path())?;
            let bytes: u64 = current.values().map(|v| v.db_bytes + v.wal_bytes).sum();
            if bytes > database_budget(&options) {
                return Err("database budget exceeded".into());
            }
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    let process = process.finish(iterations)?;
    let before_checkpoint = sizes(directory.path())?;
    let wal_checkpoint = checkpoints(directory.path())?;
    let query_plans = if options.hot_paths {
        hot_paths::read_plans(&store).await?
    } else {
        read_plans(&store).await?
    };
    let mut families = Vec::new();
    eprintln!("family                       samples      p50 ms      p99 ms   busy p99 ms");
    for (i, name) in family_names.iter().enumerate() {
        let latency = distribution(std::mem::take(&mut samples[i]));
        let busy_wait = if options.hot_paths || i == 7 {
            BusyMeasurement::Unavailable
        } else {
            BusyMeasurement::Measured(distribution(std::mem::take(&mut waits[i])))
        };
        let busy_display = match &busy_wait {
            BusyMeasurement::Measured(d) => format!("{:.3}", d.p99_ms.unwrap_or(0.0)),
            BusyMeasurement::Unavailable => "unavailable".into(),
        };
        eprintln!(
            "{name:28} {:7} {:11.3} {:11.3} {:>13}",
            latency.samples,
            latency.p50_ms.unwrap_or(0.0),
            latency.p99_ms.unwrap_or(0.0),
            busy_display
        );
        families.push(FamilyReport {
            name,
            transaction_wall: latency,
            busy_wait,
        });
    }
    let report = Report {
        format_version: 1,
        build: Build {
            profile: if cfg!(debug_assertions) {
                "development"
            } else {
                "optimized"
            },
            sqlite: rusqlite::version(),
            architecture: std::env::consts::ARCH,
        },
        label: options.label.clone(),
        timestamp_ms: now_ms(),
        fixture: Fixture {
            source: if options.state.is_some() {
                "copy"
            } else {
                "synthetic"
            },
            seed_rows: options.rows,
            payload: "fixed synthetic text and token tuple",
            producer_threads: 1,
        },
        budget: Budget {
            seconds: options.seconds,
            samples_per_family: options.samples,
            database_bytes: database_budget(&options),
        },
        measured_seconds: elapsed,
        process,
        families,
        busy: BusyReport {
            callbacks: BUSY_CALLS.load(Ordering::Relaxed),
            sleep_ms: BUSY_NS.load(Ordering::Relaxed) as f64 / 1e6,
            terminal_lock_errors: studio_runtime_store::sqlite_lock_error_count(),
            retry_delay_ms: BUSY_DELAY_MS,
            retry_limit: BUSY_RETRY_LIMIT,
        },
        wal_checkpoint,
        initial_sizes,
        before_checkpoint_sizes: before_checkpoint,
        final_sizes: sizes(directory.path())?,
        explain_query_plan: query_plans,
        measurement: "awaited production repository/service calls including admission, serialization, triggers and commit; monitor includes log sync and two transactions; SSE is a read transaction; fixture creation and query plans excluded; NULL-bound EXPLAIN captures unexpanded production SQL",
    };
    println!("{}", studio_json::to_string_pretty(&report)?);
    drop(store);
    directory.close()?;
    Ok(())
}
