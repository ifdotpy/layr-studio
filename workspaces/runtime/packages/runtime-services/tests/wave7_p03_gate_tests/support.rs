//! Test-only gates suspend the real callers after preparation, before their writer.
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use studio_runtime_store::{AgentRepository, RuntimeStore};

struct Gate {
    prepared: mpsc::SyncSender<()>,
    release: mpsc::Receiver<()>,
}
static GATE: Mutex<Option<Gate>> = Mutex::new(None);
const DEADLINE: Duration = Duration::from_secs(30);

pub fn arm() -> (mpsc::Receiver<()>, mpsc::SyncSender<()>) {
    let (prepared, ready) = mpsc::sync_channel(0);
    let (release, proceed) = mpsc::sync_channel(0);
    let old = GATE.lock().expect("gate").replace(Gate {
        prepared,
        release: proceed,
    });
    assert!(old.is_none(), "one gated request per caller test");
    (ready, release)
}

pub fn after_preparation(agent: &str) {
    if agent != "late-operation-recipient" {
        return;
    }
    let gate = GATE.lock().expect("gate").take();
    if let Some(gate) = gate {
        gate.prepared.send(()).expect("preparation observed");
        gate.release
            .recv_timeout(DEADLINE)
            .expect("operation committed before admission");
    }
}

pub async fn start_late_operation(
    store: Arc<RuntimeStore>,
    ready: mpsc::Receiver<()>,
    release: mpsc::SyncSender<()>,
) {
    studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/tests/wave7_p03_gate_tests/support",
        move || {
            ready
                .recv_timeout(DEADLINE)
                .expect("caller completed preparation");
        },
    )
    .await
    .expect("gate observer task");
    store
        .transaction(move |tx| {
            let mut other = AgentRepository::get(tx, &"other".into())?.expect("idle other");
            assert!(other.workspace_operation.value().is_none());
            other.workspace_operation =
                studio_runtime_schema::queue_command::Field::Value("restore".into());
            AgentRepository::put(tx, &other)
        })
        .await
        .expect("unrelated operation commits while request is gated");
    release.send(()).expect("release caller");
}

pub async fn read<T, F>(store: Arc<RuntimeStore>, operation: F) -> T
where
    T: Send + 'static,
    F: FnOnce(&rusqlite::Connection) -> T + Send + 'static,
{
    studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/wave7_p03_gate_tests",
        move || {
            let connection = store.test_reader().expect("test store reader");
            operation(&connection)
        },
    )
    .await
    .expect("test store read task")
}

pub struct Fixture {
    pub store: Arc<RuntimeStore>,
    _root: tempfile::TempDir,
}
impl Fixture {
    #[allow(clippy::disallowed_methods)]
    pub async fn new(label: &str) -> Self {
        let root = tempfile::Builder::new()
            .prefix(&format!("cs-w7-p03-gate-{label}-"))
            .tempdir()
            .expect("isolated gate fixture");
        let state = root.path().join("state");
        std::fs::create_dir(&state).expect("fixture state directory");
        let store = Arc::new(RuntimeStore::open(Some(&state)).await.expect("store"));
        let workspace = root.path().join("workspace");
        std::fs::create_dir(&workspace).expect("recipient workspace");
        let cwd = root.path().to_string_lossy().into_owned();
        store
            .transaction(move |tx| {
                let mut agent: studio_runtime_store::AgentRecord =
                    serde_json::from_str(include_str!("agent.json"))?;
                agent.cwd = Some(workspace.to_string_lossy().into_owned());
                agent.host_project_path = studio_runtime_schema::queue_command::Field::Value(
                    workspace.to_string_lossy().into_owned(),
                );
                agent.id = "late-operation-recipient".into();
                agent.root_id = agent.id.clone();
                AgentRepository::put(tx, &agent)?;
                agent.id = "other".into();
                agent.root_id = agent.id.clone();
                agent.cwd = Some(cwd);
                AgentRepository::put(tx, &agent)
            })
            .await
            .expect("recipient and idle unrelated workspace");
        Self { store, _root: root }
    }
}
