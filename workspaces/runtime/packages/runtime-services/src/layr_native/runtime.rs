//! Native effects run outside SQLite. Durable receipts prevent uncertain retries.
use super::*;
use crate::bounded_child::{self, ChildError, Request};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    sync::Arc,
    time::Duration,
};
use studio_runtime_schema::layr_progress::{CopyPhase, CopyProgress, CopyUnit};
use studio_runtime_schema::queue_command::Field;
use studio_runtime_store::path_identity::CanonicalPath;
use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, RuntimeStore, StoreError};

#[derive(Debug, thiserror::Error)]
pub enum NativeLayrError {
    #[error("Native Layr merge conflict: {0}")]
    Conflict(String),
    #[error("Native Layr: {0}")]
    Refused(String),
    #[error("Native Layr operation has an unknown outcome; inspect its receipt: {0}")]
    Unknown(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("Native Layr worker failed")]
    Worker,
}

type Result<T> = std::result::Result<T, NativeLayrError>;

fn assigned_reviewer(record: &Value) -> Result<bool> {
    let Some(value) = record
        .get("reviewAssignment")
        .filter(|value| !value.is_null())
    else {
        return Ok(false);
    };
    let assignment: studio_runtime_schema::review::ReviewAssignment =
        serde::Deserialize::deserialize(value)
            .map_err(|error| NativeLayrError::Refused(error.to_string()))?;
    Ok(matches!(
        assignment.target(),
        studio_runtime_schema::review::ReviewTarget::LayrState { .. }
    ))
}
pub type ProgressSink = Arc<dyn Fn(CopyProgress) + Send + Sync>;

/// Minimum interval between progress publications, in milliseconds.
pub(super) const PROGRESS_PUBLICATION_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) fn progress_channel() -> (
    ProgressSink,
    tokio::sync::watch::Receiver<Option<CopyProgress>>,
) {
    let (sender, receiver) = tokio::sync::watch::channel(None);
    let sink = Arc::new(move |value| {
        let _ = sender.send_replace(Some(value));
    });
    (sink, receiver)
}

pub(crate) async fn next_progress(
    receiver: &mut tokio::sync::watch::Receiver<Option<CopyProgress>>,
    last: Option<tokio::time::Instant>,
) -> Option<CopyProgress> {
    receiver.changed().await.ok()?;
    if let Some(last) = last {
        tokio::time::sleep_until(last + PROGRESS_PUBLICATION_INTERVAL).await;
    }
    receiver.borrow_and_update().clone()
}

fn parse_import_progress(line: &[u8]) -> Option<CopyProgress> {
    let text = std::str::from_utf8(line).ok()?;
    let (count, reference) = text
        .strip_prefix("layr: imported ")?
        .split_once(" commits of ")?;
    if reference.is_empty() {
        return None;
    }
    Some(CopyProgress {
        phase: CopyPhase::ImportingCommits,
        done: count.parse().ok()?,
        total: None,
        unit: CopyUnit::Commits,
    })
}

pub(crate) fn progress_guard(current: &Value, expected: &Value) -> bool {
    current["id"] == expected["id"]
        && current["epoch"] == expected["epoch"]
        && matches!(current["status"].as_str(), Some("queued" | "starting"))
        && studio_runtime_store::workspace_ready::workspace_generation(current)
            == studio_runtime_store::workspace_ready::workspace_generation(expected)
}

pub(crate) fn report(
    sink: Option<&ProgressSink>,
    phase: CopyPhase,
    done: u64,
    total: Option<u64>,
    unit: CopyUnit,
) {
    if let Some(sink) = sink {
        sink(CopyProgress {
            phase,
            done,
            total,
            unit,
        });
    }
}

const COMMAND_TIMEOUT: Duration = Duration::from_secs(300);
/// Deadline for the potentially long first import, in seconds (24 hours).
const IMPORT_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);
const OUTPUT_BYTES: usize = 2 * 1024 * 1024;

// Receipt intent and result are opaque evidence; status controls replay only.
#[derive(Clone, Copy, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum ReceiptStatus {
    Pending,
    Completed,
    Failed,
    Conflict,
    #[default]
    #[serde(other)]
    Unknown,
}

fn receipt_status<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<ReceiptStatus, D::Error> {
    use serde::Deserialize;
    let value = Value::deserialize(deserializer)?;
    Ok(if value.is_string() {
        ReceiptStatus::deserialize(value).unwrap_or_default()
    } else {
        ReceiptStatus::Unknown
    })
}

// Field order matches the previously sorted JSON object encoding.
#[derive(Default, serde::Deserialize, serde::Serialize)]
struct OperationReceipt {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    error: Field<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(default)]
    intent: Value,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    result: Field<Value>,
    #[serde(default, rename = "startedAt", skip_serializing_if = "Option::is_none")]
    started_at: Option<u64>,
    #[serde(default, deserialize_with = "receipt_status")]
    status: ReceiptStatus,
}
impl OperationReceipt {
    fn pending(id: &str, intent: &Value) -> Self {
        Self {
            id: Some(json!(id)),
            intent: intent.clone(),
            started_at: Some(unix_millis()),
            status: ReceiptStatus::Pending,
            ..Self::default()
        }
    }
    fn completed(id: &str, intent: &Value, result: &Value) -> Self {
        let mut receipt = Self::pending(id, intent);
        receipt.status = ReceiptStatus::Completed;
        receipt.result = if result.is_null() {
            Field::Null
        } else {
            Field::Value(result.clone())
        };
        receipt
    }
    fn failed(id: &str, intent: &Value, error: &str) -> Self {
        let mut receipt = Self::pending(id, intent);
        receipt.status = ReceiptStatus::Failed;
        receipt.error = Field::Value(json!(error));
        receipt
    }
    fn bytes(&self) -> Result<Vec<u8>> {
        studio_json::to_vec(self).map_err(|e| NativeLayrError::Refused(e.to_string()))
    }
}

fn text<'a>(record: &'a Value, key: &str) -> Result<&'a str> {
    record[key]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| NativeLayrError::Refused(format!("Missing {key}")))
}

/// Run the installed binary. Ambient Layr and Git selectors never escape this store.
pub fn run(state_dir: &Path, cwd: &Path, args: &[&str]) -> Result<String> {
    let root = settings::root_from_cwd(cwd)
        .map(Ok)
        .unwrap_or_else(|| settings::selected_store(state_dir))?;
    run_in_store(state_dir, &root, cwd, args)
}

pub(super) fn run_in_store(
    state_dir: &Path,
    root: &Path,
    cwd: &Path,
    args: &[&str],
) -> Result<String> {
    let binary = super::capability::resolve_binary().map_err(|capability| {
        NativeLayrError::Refused(capability.message().unwrap_or("Layr is unavailable").into())
    })?;
    run_with_binary(state_dir, root, cwd, args, &binary)
}

pub(super) fn run_with_binary(
    state_dir: &Path,
    root: &Path,
    cwd: &Path,
    args: &[&str],
    binary: &Path,
) -> Result<String> {
    run_with_binary_progress(state_dir, root, cwd, args, binary, None)
}

fn run_with_binary_progress(
    state_dir: &Path,
    root: &Path,
    cwd: &Path,
    args: &[&str],
    binary: &Path,
    progress: Option<ProgressSink>,
) -> Result<String> {
    // APFS clonefile traversal refuses symlink ancestors, including macOS /var and /tmp.
    let state_dir = CanonicalPath::existing(state_dir).map(CanonicalPath::into_path_buf)?;
    let mut argv = vec![
        "-u",
        "LAYR_SOCKET",
        "-u",
        "LAYR_PROJECT",
        "-u",
        "LAYR_LINE",
        "-u",
        "LAYR_SESSION",
    ];
    let variables: Vec<String> =
        super::transport_environment_in_store(&state_dir, root, cfg!(target_os = "linux"))
            .into_iter()
            .map(|(name, value)| format!("{name}={}", value.as_str().unwrap_or_default()))
            .collect();
    argv.extend(variables.iter().map(String::as_str));
    argv.extend(["GIT_NO_LAZY_FETCH=1", "GIT_OPTIONAL_LOCKS=0"]);
    argv.extend([
        binary
            .to_str()
            .ok_or_else(|| NativeLayrError::Refused("Non-UTF-8 binary path".into()))?,
        "-C",
        cwd.to_str()
            .ok_or_else(|| NativeLayrError::Refused("Non-UTF-8 cwd".into()))?,
    ]);
    argv.extend(args);
    let request = Request {
        program: std::ffi::OsStr::new("/usr/bin/env"),
        git_directory: None,
        args: &argv,
        environment: &[],
        input: None,
        stdout_limit: OUTPUT_BYTES,
        stderr_limit: OUTPUT_BYTES,
        timeout: if args.first() == Some(&"init") {
            IMPORT_TIMEOUT
        } else {
            COMMAND_TIMEOUT
        },
    };
    let output = if args.first() == Some(&"merge") {
        bounded_child::run_layr_merge(request)
    } else if let Some(progress) = progress {
        bounded_child::run_without_git_environment_streaming(
            request,
            Arc::new(move |line| {
                if let Some(value) = parse_import_progress(line) {
                    progress(value);
                }
            }),
        )
    } else {
        bounded_child::run_without_git_environment(request)
    }
    .map_err(|error| match error {
        ChildError::MergeConflict(detail) => NativeLayrError::Conflict(detail),
        ChildError::Failed(detail) => NativeLayrError::Refused(format!(
            "{}: {detail}",
            args.first().copied().unwrap_or("layr")
        )),
        ChildError::Launch(detail) => NativeLayrError::Unknown(detail),
        ChildError::TimedOut => NativeLayrError::Unknown("command deadline".into()),
    })?;
    if output.stdout.truncated {
        return Err(NativeLayrError::Refused(
            "Command output exceeds the native Layr limit".into(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout.bytes)
        .trim()
        .to_owned())
}

pub(super) fn lock(state_dir: &Path, name: &str) -> Result<File> {
    let folder = state_dir.join(NATIVE_LAYR_DIRECTORY).join("locks");
    fs::create_dir_all(&folder)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(folder.join(studio_runtime_store::sha256_hex(name.as_bytes())))?;
    file.lock()?;
    Ok(file)
}

pub fn operation(
    state_dir: &Path,
    id: &str,
    intent: &Value,
    effect: impl FnOnce() -> Result<Value>,
) -> Result<Value> {
    operation_with_preflight(state_dir, id, intent, || Ok(()), effect)
}

pub(super) fn operation_in_store(
    state_dir: &Path,
    root: &Path,
    id: &str,
    intent: &Value,
    effect: impl FnOnce() -> Result<Value>,
) -> Result<Value> {
    operation_with_preflight_in_store(state_dir, root, id, intent, || Ok(()), effect)
}

pub(super) fn operation_with_preflight(
    state_dir: &Path,
    id: &str,
    intent: &Value,
    preflight: impl FnOnce() -> Result<()>,
    effect: impl FnOnce() -> Result<Value>,
) -> Result<Value> {
    let root = settings::selected_store(state_dir)?;
    operation_with_preflight_in_store(state_dir, &root, id, intent, preflight, effect)
}

pub(super) fn operation_with_preflight_in_store(
    state_dir: &Path,
    root: &Path,
    id: &str,
    intent: &Value,
    preflight: impl FnOnce() -> Result<()>,
    effect: impl FnOnce() -> Result<Value>,
) -> Result<Value> {
    operation_with_recovery(state_dir, root, id, intent, preflight, effect, None)
}

fn operation_with_recovery(
    state_dir: &Path,
    root: &Path,
    id: &str,
    intent: &Value,
    preflight: impl FnOnce() -> Result<()>,
    effect: impl FnOnce() -> Result<Value>,
    recover: Option<&dyn Fn() -> Result<Option<Value>>>,
) -> Result<Value> {
    let _lock = lock(state_dir, &format!("{}:{id}", root.display()))?;
    let folder = settings::metadata(state_dir, root).join("receipts");
    fs::create_dir_all(&folder)?;
    let path = folder.join(format!(
        "{}.json",
        studio_runtime_store::sha256_hex(id.as_bytes())
    ));
    let mut retry_import = None;
    if path.exists() {
        // Parse the complete JSON value first: duplicates retain the last key,
        // and ignored data still observes JSON number and nesting limits.
        let value: Value = studio_json::from_slice(&fs::read(&path)?)
            .map_err(|error| NativeLayrError::Refused(error.to_string()))?;
        let receipt: OperationReceipt = if value.is_object() {
            <OperationReceipt as serde::Deserialize>::deserialize(value)
                .map_err(|error| NativeLayrError::Refused(error.to_string()))?
        } else {
            OperationReceipt::default()
        };
        if receipt.intent != *intent {
            return Err(NativeLayrError::Refused(
                "Request ID has different content".into(),
            ));
        }
        match receipt.status {
            ReceiptStatus::Completed => {
                return Ok(receipt.result.value().cloned().unwrap_or(Value::Null));
            }
            // Saves can retry directly. Failed imports discard their private partial project
            // before a fresh import. Pending and unknown effects never run again.
            ReceiptStatus::Failed if intent["kind"] == "save" => {}
            ReceiptStatus::Failed
                if intent["project"]
                    .as_str()
                    .is_some_and(|project| id == format!("import:{project}")) =>
            {
                retry_import = intent["project"].as_str();
            }
            ReceiptStatus::Failed | ReceiptStatus::Conflict => {
                if let Some(recover) = recover
                    && let Some(result) = recover()?
                {
                    crate::atomic_file::replace(
                        &path,
                        &OperationReceipt::completed(id, intent, &result).bytes()?,
                        Some(0o600),
                        true,
                    )?;
                    return Ok(result);
                }
                if matches!(receipt.status, ReceiptStatus::Conflict) {
                    return Err(NativeLayrError::Conflict(
                        receipt
                            .error
                            .value()
                            .and_then(Value::as_str)
                            .unwrap_or("merge conflict")
                            .into(),
                    ));
                }
                return Err(NativeLayrError::Refused(
                    receipt
                        .error
                        .value()
                        .and_then(Value::as_str)
                        .unwrap_or("operation failed")
                        .into(),
                ));
            }
            // A crash can leave a save pending. The line's operation log shows whether it ran.
            // When no save ran, the save retries below with a fresh pending receipt.
            ReceiptStatus::Pending if intent["kind"] == "save" => {
                if let Some(result) = settle_pending_save(state_dir, root, intent, &receipt)? {
                    crate::atomic_file::replace(
                        &path,
                        &OperationReceipt::completed(id, intent, &result).bytes()?,
                        Some(0o600),
                        true,
                    )?;
                    return Ok(result);
                }
            }
            ReceiptStatus::Pending | ReceiptStatus::Unknown => {
                return Err(NativeLayrError::Unknown(path.display().to_string()));
            }
        }
    }
    preflight()?;
    // On Linux this check is read-only and refuses a daemon-owned partial project.
    // Keep the failed receipt intact when no retry can start.
    #[cfg(target_os = "linux")]
    if let Some(project) = retry_import {
        super::failed_import::discard_in_store(state_dir, root, project)?;
    }
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let pending = OperationReceipt::pending(id, intent).bytes()?;
    if path.exists() {
        crate::atomic_file::replace(&path, &pending, Some(0o600), true)?;
    } else {
        let mut file = options.open(&path)?;
        file.write_all(&pending)?;
        file.sync_all()?;
        File::open(&folder)?.sync_all()?;
    }
    let outcome = (|| {
        if !cfg!(target_os = "linux")
            && let Some(project) = retry_import
        {
            super::failed_import::discard_in_store(state_dir, root, project)?;
        }
        effect()
    })();
    let receipt = match &outcome {
        Ok(result) => OperationReceipt::completed(id, intent, result),
        Err(NativeLayrError::Unknown(_)) => return outcome,
        Err(NativeLayrError::Conflict(detail)) => {
            let mut receipt = OperationReceipt::failed(id, intent, detail);
            receipt.status = ReceiptStatus::Conflict;
            receipt
        }
        Err(error) => OperationReceipt::failed(id, intent, &error.to_string()),
    };
    crate::atomic_file::replace(&path, &receipt.bytes()?, Some(0o600), true)?;
    outcome
}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

const OPERATION_LOG_WINDOW: usize = 200;

/// Some(result) when the line's operation log holds a `save` after the receipt started,
/// None when it holds none. A receipt without a start time, or a log window that cannot
/// reach back to that time, proves nothing and stays unknown.
fn settle_pending_save(
    state_dir: &Path,
    root: &Path,
    intent: &Value,
    receipt: &OperationReceipt,
) -> Result<Option<Value>> {
    let unknown = || NativeLayrError::Unknown("pending save without evidence".into());
    let started = receipt.started_at.ok_or_else(unknown)?;
    let cwd = Path::new(intent["cwd"].as_str().ok_or_else(unknown)?);
    let window = OPERATION_LOG_WINDOW.to_string();
    let log = run_in_store(
        state_dir,
        root,
        cwd,
        &["op", "log", "--json", "-n", &window],
    )?;
    let mut records = 0;
    let mut oldest = u64::MAX;
    let mut saved = false;
    for line in log.lines().filter(|line| !line.trim().is_empty()) {
        let record: Value = studio_json::from_str(line)
            .map_err(|error| NativeLayrError::Refused(error.to_string()))?;
        let time = record["time"].as_u64().ok_or_else(unknown)?;
        records += 1;
        oldest = oldest.min(time);
        saved |= record["op"] == "save" && time >= started;
    }
    if saved {
        return Ok(Some(json!({"stateId":working_state(state_dir, cwd)?})));
    }
    if records == OPERATION_LOG_WINDOW && oldest >= started {
        return Err(unknown());
    }
    Ok(None)
}

/// A full working snapshot retains empty directories, modes, and attributes.
pub(super) fn save_imported_tree(state: &Path, cwd: &Path, message: &str) -> Result<()> {
    run(state, cwd, &["save", "--force", "--note", message])?;
    let snapshot = run(state, cwd, &["rev-parse", "head@{0}"])?;
    run(state, cwd, &["reset", "--mixed", snapshot.trim()])?;
    run(state, cwd, &["save", "-m", message, "--allow-empty"])?;
    Ok(())
}

pub fn project_id(record: &Value) -> Result<String> {
    if let Some(id) = record["layrProjectId"].as_str() {
        if super::mapping::valid_name(id) {
            return Ok(id.into());
        }
        return Err(NativeLayrError::Refused(
            "Invalid native Layr project ID".into(),
        ));
    }
    let source = record["hostProjectPath"]
        .as_str()
        .ok_or_else(|| NativeLayrError::Refused("Missing project identity".into()))?;
    Ok(studio_runtime_store::sha256_hex(source.as_bytes()))
}

pub fn main_path(state_dir: &Path, project: &str) -> PathBuf {
    main_path_in_store(&store_path(state_dir), project)
}

/// The main line of `project`. `layr init --from-git` leaves main at the checkout path, outside
/// the store; its metadata then names that folder.
pub(super) fn main_path_in_store(root: &Path, project: &str) -> PathBuf {
    let main = root.join("projects").join(project).join("lines/main");
    if main.exists() {
        return main;
    }
    super::mapping::moved_main(root, project).unwrap_or(main)
}

/// The folder of `line` of `project`. `layr line new` makes it in the store even when main
/// moved out of the store.
pub(super) fn line_path_in_store(root: &Path, project: &str, line: &str) -> PathBuf {
    if line == "main" {
        return main_path_in_store(root, project);
    }
    root.join("projects").join(project).join("lines").join(line)
}

pub(super) fn admit_git_policy(state_dir: &Path, project: &str, without_git: bool) -> Result<()> {
    admit_git_policy_in_store(
        state_dir,
        &settings::selected_store(state_dir)?,
        project,
        without_git,
    )
}

fn admit_git_policy_in_store(
    state_dir: &Path,
    root: &Path,
    project: &str,
    without_git: bool,
) -> Result<()> {
    let ready_path = settings::metadata(state_dir, root).join(format!("{project}.json"));
    if without_git && ready_path.exists() {
        let receipt: Value = studio_json::from_slice(&fs::read(&ready_path)?)
            .map_err(|error| NativeLayrError::Refused(error.to_string()))?;
        if receipt["withoutGit"] != true {
            return Err(NativeLayrError::Refused("This project was already imported with Git metadata. The --without-git option cannot change an existing import.".into()));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_worker_line(
    state_dir: &Path,
    root: &Path,
    main: &Path,
    project: &str,
    line: &str,
    cwd: &Path,
    record: &Value,
    default_start: &str,
    progress: Option<&ProgressSink>,
) -> Result<Value> {
    let requested = record["layrBaseState"].as_str();
    let start = requested.unwrap_or(default_start);
    let mut intent = json!({"project":project,"line":line});
    if requested.is_some() {
        intent["start"] = json!(start);
    }
    operation_with_preflight_in_store(
        state_dir,
        root,
        &format!("line:{}", text(record, "id")?),
        &intent,
        || {
            if requested.is_some() {
                run(state_dir, main, &["rev-parse", start]).map_err(|error| match error {
                    NativeLayrError::Refused(detail) => NativeLayrError::Refused(format!(
                        "Native Layr base revision could not be resolved: {detail}"
                    )),
                    other @ (NativeLayrError::Conflict(_)
                    | NativeLayrError::Unknown(_)
                    | NativeLayrError::Io(_)
                    | NativeLayrError::Store(_)
                    | NativeLayrError::Worker) => other,
                })?;
            }
            Ok(())
        },
        || {
            report(progress, CopyPhase::CreatingLine, 0, None, CopyUnit::Files);
            run(state_dir, main, &["line", "new", line, start])?;
            Ok(json!({"path":cwd,"line":line}))
        },
    )
}

pub(super) fn prepare_files_reporting(
    state_dir: &Path,
    record: &Value,
    without_git: bool,
    progress: Option<&ProgressSink>,
) -> Result<Value> {
    let root = settings::record_store(state_dir, record)?;
    let project = super::mapping::resolve(state_dir, &root, record)?;
    let main = main_path_in_store(&root, &project);
    let metadata = settings::metadata(state_dir, &root);
    let ready_path = metadata.join(format!("{project}.json"));
    if record["layrReady"] == true {
        associated_cwd(state_dir, record)?;
    }
    if without_git && root != store_path(state_dir) {
        return Err(NativeLayrError::Refused(
            "The --without-git option cannot change a shared project".into(),
        ));
    }
    admit_git_policy_in_store(state_dir, &root, &project, without_git)?;
    let mapped = !ready_path.exists()
        && main.is_dir()
        && project != studio_runtime_store::sha256_hex(text(record, "hostProjectPath")?.as_bytes());
    if mapped {
        let source = CanonicalPath::existing(text(record, "hostProjectPath")?)?.into_path_buf();
        super::mapping::verify(&root, &source, &project)?;
    }
    let plan = if !ready_path.exists() && !mapped {
        let source = CanonicalPath::existing(text(record, "hostProjectPath")?)
            .map(CanonicalPath::into_path_buf)?;
        if state_dir.starts_with(&source) || root.starts_with(&source) {
            return Err(NativeLayrError::Refused(
                "The Studio state directory must be outside the project".into(),
            ));
        }
        report(progress, CopyPhase::Sizing, 0, None, CopyUnit::Files);
        let plan = super::import::Plan::new_with_git_policy(
            &source,
            false,
            &state_dir.join(NATIVE_LAYR_DIRECTORY),
            without_git,
        )?;
        plan.admit()?;
        plan.size
            .preflight(&state_dir.join(NATIVE_LAYR_DIRECTORY))?;
        Some((source, plan))
    } else {
        None
    };
    {
        let _binary_lock = lock(state_dir, "binary")?;
        let binary = super::capability::resolve_binary().map_err(|capability| {
            NativeLayrError::Refused(capability.message().unwrap_or("Layr is unavailable").into())
        })?;
        super::git_guard::ensure(state_dir, &binary)?;
    }
    let version_warning = super::versions::verify(state_dir, &root)?;
    fs::create_dir_all(&metadata)?;
    let _project_lock = lock(state_dir, &format!("{}:project:{project}", root.display()))?;
    admit_git_policy_in_store(state_dir, &root, &project, without_git)?;
    if mapped && !ready_path.exists() {
        let source = CanonicalPath::existing(text(record, "hostProjectPath")?)?.into_path_buf();
        let identity = super::mapping::verify(&root, &source, &project)?;
        let receipt = json!({"projectId":project,"source":source,"mainPath":main,"storeRoot":root,"mapped":true,"withoutGit":false,"identity":identity});
        crate::atomic_file::replace(
            &ready_path,
            receipt.to_string().as_bytes(),
            Some(0o600),
            true,
        )?;
    }
    if !ready_path.exists() {
        let (source, plan) =
            plan.ok_or_else(|| NativeLayrError::Refused("Native import readiness changed".into()))?;
        let mut input = json!({"project":project,"source":source});
        if without_git {
            input["withoutGit"] = json!(true);
        }
        operation_with_preflight_in_store(
            state_dir,
            &root,
            &format!("import:{project}"),
            &input,
            || plan.size.preflight(&state_dir.join(NATIVE_LAYR_DIRECTORY)),
            || {
                let staging = tempfile::tempdir_in(state_dir.join(NATIVE_LAYR_DIRECTORY))?;
                if progress.is_some() {
                    plan.stage_with_progress(staging.path(), &|done, total| {
                        report(
                            progress,
                            CopyPhase::Staging,
                            done,
                            Some(total),
                            CopyUnit::Files,
                        )
                    })?;
                } else {
                    plan.stage(staging.path())?;
                }
                // Layr 0.2.2 clears the no-fetch environment. Its Git input must be private.
                if staging.path().join(".git").is_dir() {
                    super::import::validate_private_git(staging.path())?;
                }
                // A client error does not prove the daemon stopped reading the source.
                let staging = staging.keep();
                report(
                    progress,
                    CopyPhase::ImportingCommits,
                    0,
                    None,
                    CopyUnit::Commits,
                );
                let binary = super::capability::resolve_binary().map_err(|capability| {
                    NativeLayrError::Refused(
                        capability.message().unwrap_or("Layr is unavailable").into(),
                    )
                })?;
                run_with_binary_progress(
                    state_dir,
                    &root,
                    state_dir,
                    &[
                        "init",
                        &project,
                        "--from",
                        staging.to_str().unwrap_or_default(),
                    ],
                    &binary,
                    progress.cloned(),
                )
                .map_err(|error| {
                    let retained = format!(
                        " The import staging folder {} is retained; see docs/native-layr.md.",
                        staging.display()
                    );
                    match error {
                        NativeLayrError::Unknown(message) => {
                            NativeLayrError::Unknown(format!("{message}{retained}"))
                        }
                        NativeLayrError::Refused(message) => {
                            NativeLayrError::Refused(format!("{message}{retained}"))
                        }
                        other @ (NativeLayrError::Conflict(_)
                        | NativeLayrError::Io(_)
                        | NativeLayrError::Store(_)
                        | NativeLayrError::Worker) => {
                            NativeLayrError::Refused(format!("{other}{retained}"))
                        }
                    }
                })?;
                run(state_dir, &main, &["git", "remote", "remove", "local"])?;
                fs::remove_dir_all(staging)?;
                run(state_dir, &main, &["protect", "main", "--direct"])?;
                report(
                    progress,
                    CopyPhase::RestoringAttributes,
                    0,
                    None,
                    CopyUnit::Files,
                );
                if plan.restore_directory_attributes(&main)? {
                    save_imported_tree(state_dir, &main, "Preserve imported directory attributes")?;
                }
                report(progress, CopyPhase::WarmingCaches, 0, None, CopyUnit::Files);
                plan.warm_caches(state_dir, &main, Path::new(""))?;
                let receipt = json!({"projectId":project,"source":source,"mainPath":main,"withoutGit":without_git});
                crate::atomic_file::replace(
                    &ready_path,
                    receipt.to_string().as_bytes(),
                    Some(0o600),
                    true,
                )?;
                Ok(receipt)
            },
        )?;
    }
    let imported: Value = studio_json::from_slice(&fs::read(&ready_path)?)
        .map_err(|error| NativeLayrError::Refused(error.to_string()))?;
    let (start, cache_warning) = if imported["mapped"] == true {
        super::mapping::cached_main_start(state_dir, &main)?
    } else {
        ("main".to_owned(), None)
    };
    let reviewer = record["role"] == "reviewer";
    let lead = record["isLead"] == true;
    let assigned = reviewer && assigned_reviewer(record)?;
    let line = if lead || (reviewer && !assigned) {
        "main".to_owned()
    } else {
        format!(
            "agent-{}",
            studio_runtime_store::sha256_hex(text(record, "id")?.as_bytes())
        )
    };
    let cwd = line_path_in_store(&root, &project, &line);
    if !lead && (!reviewer || assigned) {
        create_worker_line(
            state_dir, &root, &main, &project, &line, &cwd, record, &start, progress,
        )?;
    }
    super::git_guard::install_hook(state_dir, &cwd)?;
    let state = run(state_dir, &cwd, &["rev-parse", "head"])?;
    let mut result = json!({"cwd":cwd,"layrProjectId":project,"layrLine":line,"layrStateId":state,"layrReady":true,"layrStoreRoot":root});
    if let Some(warning) = &version_warning {
        eprintln!("Native Layr warning: {warning}");
    }
    result["layrVersionWarning"] = json!(version_warning);
    if let Some(warning) = &cache_warning {
        eprintln!("Native Layr warning: {warning}");
    }
    result["layrCacheWarning"] = json!(cache_warning);
    if reviewer {
        let canonical = crate::reviewer_workspace::ensure_scratch(state_dir, text(record, "id")?)?;
        result["layrScratch"] = json!(canonical);
        result["layrReadCwd"] = result["cwd"].clone();
        if !assigned {
            result["cwd"] = result["layrScratch"].clone();
        }
    }
    Ok(result)
}

pub(crate) async fn publish_progress(
    store: &RuntimeStore,
    expected: &Value,
    progress: Option<CopyProgress>,
) -> Result<()> {
    let expected = expected.clone();
    store
        .transaction_with_error(move |tx| {
            let id = text(&expected, "id")?;
            let Some(mut agent) = AgentRepository::get(tx, &AgentId::from(id))? else {
                return Ok(());
            };
            let current = agent.value();
            if !progress_guard(&current, &expected) {
                return Ok(());
            }
            if let Some(progress) = progress {
                let Some(epoch) = expected["epoch"].as_i64() else {
                    return Ok(());
                };
                agent.start_wait = Some(
                    studio_runtime_schema::agent_progress::AgentStartWait::WorkspaceCopy {
                        epoch,
                        progress,
                    },
                );
            } else if matches!(agent.start_wait.as_ref(), Some(studio_runtime_schema::agent_progress::AgentStartWait::WorkspaceCopy { epoch, .. }) if Some(*epoch) == expected["epoch"].as_i64()) {
                agent.start_wait = None;
            } else {
                return Ok(());
            }
            AgentRepository::put(tx, &agent)?;
            Ok(())
        })
        .await
}

pub async fn prepare(store: Arc<RuntimeStore>, mut record: Value) -> Result<Value> {
    if record["deletedAt"].is_string()
        || record["deletedAt"].is_number()
        || record["autoWake"] != true
    {
        return Err(NativeLayrError::Refused("The agent is not active".into()));
    }
    // The next turn cannot enter its line before the previous quiet-point save.
    let pending: Vec<String> = record["layrTurnSaves"]
        .as_object()
        .into_iter()
        .flat_map(|turns| turns.iter())
        .filter(|(_, save)| save["status"] != "saved")
        .map(|(turn, _)| turn.clone())
        .collect();
    for turn in pending {
        let id = text(&record, "id")?.to_owned();
        save_turn(&store, &id, &turn).await?;
        record = agent_value(&store, &id).await?;
    }
    let state_dir = store.root().to_owned();
    let expected = record.clone();
    let (progress, mut receiver) = progress_channel();
    let mut last_published = None;
    let mut task = studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/layr-native/prepare",
        move || prepare_files_reporting(&state_dir, &expected, false, Some(&progress)),
    );
    let prepared = loop {
        tokio::select! {
            result = &mut task => break result.map_err(|_| NativeLayrError::Worker).and_then(|value| value),
            Some(progress) = next_progress(&mut receiver, last_published) => {
                drop(publish_progress(&store, &record, Some(progress)).await);
                last_published = Some(tokio::time::Instant::now());
            }
        }
    };
    drop(publish_progress(&store, &record, None).await);
    let prepared = prepared?;
    store
        .transaction_with_error(move |tx| {
            let id = text(&record, "id")?;
            let mut current = AgentRepository::get(tx, &AgentId::from(id))?
                .ok_or_else(|| NativeLayrError::Refused("The agent no longer exists".into()))?
                .value();
            if studio_runtime_store::workspace_ready::workspace_generation(&current)
                != studio_runtime_store::workspace_ready::workspace_generation(&record)
            {
                return Err(NativeLayrError::Refused(
                    "The workspace changed during preparation".into(),
                ));
            }
            for (key, value) in prepared
                .as_object()
                .ok_or_else(|| NativeLayrError::Refused("Invalid native workspace".into()))?
            {
                current[key] = value.clone();
            }
            AgentRepository::put(
                tx,
                &AgentRecord::from_value(current.clone()).map_err(StoreError::from)?,
            )?;
            Ok(current)
        })
        .await
}

/// Confirm the stored association against the owned project path before any effect.
pub(super) fn associated_cwd(state_dir: &Path, record: &Value) -> Result<PathBuf> {
    if record["layrReady"] != true {
        return Err(NativeLayrError::Refused(
            "The native Layr workspace is not ready".into(),
        ));
    }
    let project = project_id(record)?;
    let assigned = record["role"] == "reviewer" && assigned_reviewer(record)?;
    let expected_line = if record["isLead"] == true || (record["role"] == "reviewer" && !assigned) {
        "main".to_owned()
    } else {
        format!(
            "agent-{}",
            studio_runtime_store::sha256_hex(text(record, "id")?.as_bytes())
        )
    };
    if text(record, "layrLine")? != expected_line {
        return Err(NativeLayrError::Refused(
            "The native Layr line association changed".into(),
        ));
    }
    let root = settings::record_store(state_dir, record)?;
    let expected = line_path_in_store(&root, &project, &expected_line);
    let reviewer = record["role"] == "reviewer";
    if reviewer {
        let scratch = CanonicalPath::existing(state_dir)
            .map(CanonicalPath::into_path_buf)?
            .join(NATIVE_LAYR_DIRECTORY)
            .join("scratch")
            .join(studio_runtime_store::sha256_hex(
                text(record, "id")?.as_bytes(),
            ));
        if (!assigned
            && CanonicalPath::existing(text(record, "cwd")?).map(CanonicalPath::into_path_buf)?
                != scratch)
            || CanonicalPath::existing(text(record, "layrScratch")?)
                .map(CanonicalPath::into_path_buf)?
                != scratch
        {
            return Err(NativeLayrError::Refused(
                "The reviewer scratch association changed".into(),
            ));
        }
    }
    let actual = CanonicalPath::existing(text(
        record,
        if reviewer && !assigned {
            "layrReadCwd"
        } else {
            "cwd"
        },
    )?)
    .map(CanonicalPath::into_path_buf)?;
    if actual != CanonicalPath::existing(expected).map(CanonicalPath::into_path_buf)? {
        return Err(NativeLayrError::Refused(
            "The native Layr cwd association changed".into(),
        ));
    }
    Ok(actual)
}

fn working_state(state_dir: &Path, cwd: &Path) -> Result<String> {
    match run(state_dir, cwd, &["rev-parse", "head@{0}"]) {
        Ok(state) => Ok(state),
        Err(NativeLayrError::Refused(detail)) if detail.contains("has only 0 automatic states") => {
            run(state_dir, cwd, &["rev-parse", "head"])
        }
        Err(error) => Err(error),
    }
}

/// Repairs each folder overlay at a quiet point: a folder that a build tool made again is a cache.
fn repair_caches(state_dir: &Path, cwd: &Path) -> Result<()> {
    let overlays: Vec<Value> =
        studio_json::from_str(&run(state_dir, cwd, &["overlay", "list", "--json"])?)
            .map_err(|error| NativeLayrError::Refused(error.to_string()))?;
    for overlay in overlays.iter().filter(|overlay| needs_repair(overlay)) {
        run(
            state_dir,
            cwd,
            &["overlay", "repair", text(overlay, "name")?],
        )?;
    }
    Ok(())
}

/// A folder overlay needs a repair only when its folder is a plain folder again. A cache is
/// already one, and an absent folder (for example a cache another line defined) has nothing to
/// repair; repairing it fails and would end every turn save of the line.
fn needs_repair(overlay: &Value) -> bool {
    overlay["folder"].is_string() && !matches!(overlay["state"].as_str(), Some("cache" | "absent"))
}

#[cfg(test)]
pub(super) fn prepare_files(state_dir: &Path, record: &Value) -> Result<Value> {
    prepare_files_reporting(state_dir, record, false, None)
}

#[cfg(test)]
mod repair_tests {
    use super::needs_repair;
    use studio_json::json;

    #[test]
    fn only_a_plain_folder_overlay_is_repaired() {
        assert!(needs_repair(
            &json!({"folder": "target", "state": "plain folder"})
        ));
        assert!(!needs_repair(
            &json!({"folder": "target", "state": "cache"})
        ));
        assert!(!needs_repair(
            &json!({"folder": ".layr-retest", "state": "absent"})
        ));
        assert!(!needs_repair(
            &json!({"name": "notes", "state": "plain folder"})
        ));
    }
}

pub fn save(state_dir: &Path, record: &Value, request_id: &str) -> Result<Value> {
    let cwd = associated_cwd(state_dir, record)?;
    operation_in_store(
        state_dir,
        &settings::record_store(state_dir, record)?,
        request_id,
        &json!({"agent":record["id"],"cwd":cwd,"kind":"save"}),
        || {
            if record["role"] != "reviewer" {
                repair_caches(state_dir, &cwd)?;
                run(state_dir, &cwd, &["save"])?;
            }
            let state = if record["role"] == "reviewer" {
                run(state_dir, &cwd, &["rev-parse", "head"])?
            } else {
                working_state(state_dir, &cwd)?
            };
            Ok(json!({"stateId":state}))
        },
    )
}

pub fn remove(state_dir: &Path, record: &Value, request_id: &str) -> Result<Value> {
    let root = settings::record_store(state_dir, record)?;
    operation_in_store(
        state_dir,
        &root,
        request_id,
        &json!({"agent":record["id"],"line":record["layrLine"],"kind":"remove"}),
        || {
            if record["isLead"] != true
                && (record["role"] != "reviewer" || assigned_reviewer(record)?)
                && record["layrReady"] == true
            {
                let project = project_id(record)?;
                let cwd = associated_cwd(state_dir, record)?;
                let line = text(record, "layrLine")?;
                if line == "main" {
                    return Err(NativeLayrError::Refused("Cannot delete main".into()));
                }
                let state = working_state(state_dir, &cwd)?;
                let archive_ref = format!(
                    "pins/studio/archive/{}",
                    studio_runtime_store::sha256_hex(text(record, "id")?.as_bytes())
                );
                run(
                    state_dir,
                    &main_path_in_store(&root, &project),
                    &["ref", "set", &archive_ref, &state],
                )?;
                run(
                    state_dir,
                    &main_path_in_store(&root, &project),
                    &["line", "delete", "-D", line],
                )?;
                return Ok(json!({"state":"removed","archiveRef":archive_ref,"stateId":state}));
            }
            Ok(json!({"state":"removed"}))
        },
    )
}

pub fn evidence(
    state_dir: &Path,
    record: &Value,
    revision: &str,
) -> Result<studio_runtime_schema::layr_evidence::EvidenceResponse> {
    let cwd = associated_cwd(state_dir, record)?;
    let resolved = run(state_dir, &cwd, &["rev-parse", revision])?;
    if revision != resolved || run(state_dir, &cwd, &["rev-parse", "head"])? != resolved {
        return Err(NativeLayrError::Refused(
            "Submit the full Layr state ID".into(),
        ));
    }
    Ok(studio_runtime_schema::layr_evidence::EvidenceResponse {
        state_id: resolved,
        project_id: record["layrProjectId"].as_str().map(str::to_owned),
        line: record["layrLine"].as_str().map(str::to_owned),
    })
}

pub fn merge(
    state_dir: &Path,
    lead: &Value,
    worker: &Value,
    revision: &str,
    request_id: &str,
) -> Result<Value> {
    if lead["isLead"] != true
        || worker["role"] == "reviewer"
        || lead["layrProjectId"] != worker["layrProjectId"]
        || !settings::same_root(
            &settings::record_store(state_dir, lead)?,
            &settings::record_store(state_dir, worker)?,
        )
    {
        return Err(NativeLayrError::Refused(
            "Invalid native Layr merge owner".into(),
        ));
    }
    let main = associated_cwd(state_dir, lead)?;
    associated_cwd(state_dir, worker)?;
    let root = settings::record_store(state_dir, lead)?;
    let project = project_id(lead)?;
    operation_with_recovery(
        state_dir,
        &root,
        request_id,
        &json!({"lead":lead["id"],"worker":worker["id"],"revision":revision}),
        || Ok(()),
        || {
            let _project_lock = lock(state_dir, &format!("{}:project:{project}", root.display()))?;
            let from_state = run(state_dir, &main, &["rev-parse", "head"])?;
            run(
                state_dir,
                &main,
                &["merge", text(worker, "layrLine")?, "--expect", revision],
            )?;
            Ok(
                json!({"merge":studio_runtime_store::work_intent::MergeReceipt {
                    from_state: Some(from_state),
                    to_state: run(state_dir, &main, &["rev-parse", "head"])?
                }}),
            )
        },
        Some(&|| {
            let _project_lock = lock(state_dir, &format!("{}:project:{project}", root.display()))?;
            let status: Value =
                studio_json::from_str(&run(state_dir, &main, &["status", "--json"])?)
                    .map_err(|e| NativeLayrError::Refused(e.to_string()))?;
            if !["staged", "unstaged", "untracked", "conflicts"]
                .iter()
                .all(|key| status[*key].as_array().is_some_and(Vec::is_empty))
            {
                return Ok(None);
            }
            let head = run(state_dir, &main, &["rev-parse", "head"])?;
            if run(
                state_dir,
                &main,
                &["merge-base", "--is-ancestor", revision, &head],
            )
            .is_err()
            {
                return Ok(None);
            }
            if run(state_dir, &main, &["rev-parse", "head"])? != head {
                return Ok(None);
            }
            Ok(Some(
                json!({"merge":studio_runtime_store::work_intent::MergeReceipt {
                    from_state: None,
                    to_state: head
                }}),
            ))
        }),
    )
}

pub async fn agent_value(store: &RuntimeStore, id: &str) -> Result<Value> {
    let id = id.to_owned();
    store
        .read_async("runtime-services/layr-native/agent", move |tx| {
            AgentRepository::get(tx, &AgentId::from(id.as_str()))?
                .map(|r| r.value())
                .ok_or_else(|| NativeLayrError::Refused("Unknown native Layr agent".into()))
        })
        .await
}

pub async fn save_turn(store: &Arc<RuntimeStore>, agent_id: &str, turn_id: &str) -> Result<()> {
    let record = agent_value(store, agent_id).await?;
    if !is_native(&record) || record["layrReady"] != true {
        return Ok(());
    }
    let state_dir = store.root().to_owned();
    let request_id = format!("turn-save:{agent_id}:{turn_id}");
    let outcome = studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/layr-native/save",
        move || save(&state_dir, &record, &request_id),
    )
    .await
    .map_err(|_| NativeLayrError::Worker)??;
    let agent_id = agent_id.to_owned();
    let turn_id = turn_id.to_owned();
    store
        .transaction_with_error(move |tx| {
            let mut record = AgentRepository::get(tx, &AgentId::from(agent_id.as_str()))?
                .ok_or_else(|| NativeLayrError::Refused("Unknown agent after save".into()))?
                .value();
            record["layrStateId"] = outcome["stateId"].clone();
            if !record["layrTurnSaves"].is_object() {
                record["layrTurnSaves"] = json!({});
            }
            record["layrTurnSaves"][&turn_id] =
                json!({"status":"saved","stateId":outcome["stateId"]});
            AgentRepository::put(
                tx,
                &AgentRecord::from_value(record).map_err(StoreError::from)?,
            )?;
            Ok(())
        })
        .await
}

/// The terminal transaction stages a save before it releases the next turn.
pub(crate) fn stage_turn_save(
    tx: &studio_runtime_store::Transaction<'_>,
    agent_id: &str,
    turn_id: &str,
) -> std::result::Result<(), StoreError> {
    let Some(mut agent) = AgentRepository::get(tx, &AgentId::from(agent_id))? else {
        return Ok(());
    };
    if agent.workspace_mode != Some(studio_runtime_schema::records::WorkspaceMode::LayrNative)
        || agent.layr_ready != Some(true)
    {
        return Ok(());
    }
    let saves = {
        if !agent.layr_turn_saves.is_some() {
            agent.layr_turn_saves =
                studio_runtime_schema::queue_command::Field::Value(Default::default());
        }
        match &mut agent.layr_turn_saves {
            studio_runtime_schema::queue_command::Field::Value(values) => values,
            studio_runtime_schema::queue_command::Field::Missing
            | studio_runtime_schema::queue_command::Field::Null => {
                unreachable!("map initialized above")
            }
        }
    };
    if matches!(
        saves.get(turn_id),
        Some(studio_runtime_store::LayrTurnSave::Saved { .. })
    ) {
        return Ok(());
    }
    saves.insert(
        turn_id.to_owned(),
        studio_runtime_store::LayrTurnSave::Pending,
    );
    AgentRepository::put(tx, &agent)?;
    Ok(())
}

/// Pending receipts survive process exit. A retry returns the same result or stays held.
#[cfg(test)]
mod review_tests {
    use super::*;

    #[test]
    fn unassigned_legacy_reviewer_keeps_main_and_scratch() {
        let fixture = tempfile::tempdir().unwrap();
        let id = "legacy-reviewer";
        let main = main_path(fixture.path(), "project");
        let scratch = crate::reviewer_workspace::ensure_scratch(fixture.path(), id).unwrap();
        fs::create_dir_all(&main).unwrap();
        let record = json!({"id":id,"role":"reviewer","isLead":false,
            "layrReady":true,"layrProjectId":"project","layrLine":"main",
            "cwd":scratch,"layrScratch":scratch,"layrReadCwd":main,
            "layrStoreRoot":super::super::store_path(fixture.path())});
        assert_eq!(associated_cwd(fixture.path(), &record).unwrap(), main);
    }

    #[test]
    fn assigned_reviewer_associates_its_own_line_as_working_copy() {
        let fixture = tempfile::tempdir().unwrap();
        let id = "reviewer";
        let line = format!("agent-{}", studio_runtime_store::sha256_hex(id.as_bytes()));
        let cwd = main_path(fixture.path(), "project")
            .parent()
            .unwrap()
            .join(&line);
        fs::create_dir_all(&cwd).unwrap();
        let scratch = fixture
            .path()
            .join(NATIVE_LAYR_DIRECTORY)
            .join("scratch")
            .join(studio_runtime_store::sha256_hex(id.as_bytes()));
        fs::create_dir_all(&scratch).unwrap();
        let record = json!({"id":id,"role":"reviewer","isLead":false,
            "layrReady":true,"layrProjectId":"project","layrLine":line,"cwd":cwd,
            "layrStoreRoot":super::super::store_path(fixture.path()), "layrScratch":scratch,
            "reviewAssignment":{"kind":"layrState","state":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "base":{"kind":"mergeBaseWithMain"}}});
        assert_eq!(associated_cwd(fixture.path(), &record).unwrap(), cwd);
    }

    /// Linux preparation checks the daemon identity before an import.
    #[cfg(target_os = "linux")]
    fn fixture_daemon(state: &Path) -> std::thread::JoinHandle<()> {
        let socket = super::super::selected_socket_path(state).unwrap();
        fs::create_dir_all(socket.parent().unwrap()).unwrap();
        super::super::versions::serve_daemon_identity(
            &socket,
            br#"{"format":1,"version":"0.14.2","build":"fixture"}"#,
        )
    }

    #[cfg(unix)]
    #[test]
    fn successful_import_removes_staging_remote() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = tempfile::tempdir().unwrap();
        let state = fixture.path().join("state");
        let source = fixture.path().join("source");
        fs::create_dir_all(&state).unwrap();
        fs::create_dir_all(&source).unwrap();
        let binary = fixture.path().join("layr");
        let script = format!(
            r#"#!/bin/sh
case "$*" in
  *store-version*) printf '%s\n' '{{"binary":{{"format":1,"version":"0.14.2","build":"fixture"}},"store":null,"compatible":true,"state":"compatible"}}' ;;
  *' init '*) while [ "$1" != "init" ]; do shift; done; mkdir -p "$LAYR_ROOT/projects/$2/lines/main" ;;
  *' git remote remove local') touch '{}' ;;
esac
"#,
            fixture.path().join("removed").display()
        );
        fs::write(&binary, script).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        let _binary_override = super::super::override_layr_binary(binary);
        #[cfg(target_os = "linux")]
        let _daemon = fixture_daemon(&state);
        let mut record = super::super::fixture_agent("lead", true);
        super::super::configure_new_chat(
            &mut record,
            &crate::project_path::ProjectPath::from_stored(source.to_str().unwrap()),
        );
        prepare_files(&state, &record.value()).expect("fixture import succeeds");
        assert!(
            fixture.path().join("removed").exists(),
            "staging remote was not removed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn assigned_layr_reviewer_prepares_and_archives_the_requested_line() {
        let state = tempfile::tempdir().unwrap();
        let fixture = super::super::review_fixture::FakeReviewer::new(state.path(), "reviewer");
        let record = &fixture.record;
        assert_ne!(record["cwd"], record["layrScratch"]);
        assert_eq!(record["cwd"], record["layrReadCwd"]);
        let line = record["layrLine"].as_str().unwrap();
        let commands =
            fs::read_to_string(super::super::store_path(state.path()).join("commands")).unwrap();
        assert!(commands.contains(&format!("line new {line} aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")));
        remove(state.path(), record, "remove-reviewer").unwrap();
        let commands =
            fs::read_to_string(super::super::store_path(state.path()).join("commands")).unwrap();
        assert!(commands.contains(&format!("line delete -D {line}")));
    }

    #[cfg(unix)]
    #[test]
    fn failed_import_keeps_staging_for_a_daemon_still_reading() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = tempfile::tempdir().unwrap();
        let state = fixture.path().join("state");
        let source = fixture.path().join("source");
        fs::create_dir_all(&state).unwrap();
        fs::create_dir_all(&source).unwrap();
        let binary = fixture.path().join("layr");
        let script = format!(
            r#"#!/bin/sh
case "$*" in
  *store-version*) printf '%s\n' '{{"binary":{{"format":1,"version":"0.14.2","build":"fixture"}},"store":null,"compatible":true,"state":"compatible"}}' ;;
  *' init '*) while [ "$1" != "--from" ]; do shift; done; printf '%s' "$2" > '{}'; exit 1 ;;
esac
"#,
            fixture.path().join("staging-path").display()
        );
        fs::write(&binary, script).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        let _binary_override = super::super::override_layr_binary(binary);
        #[cfg(target_os = "linux")]
        let _daemon = fixture_daemon(&state);
        let mut record = super::super::fixture_agent("lead", true);
        super::super::configure_new_chat(
            &mut record,
            &crate::project_path::ProjectPath::from_stored(source.to_str().unwrap()),
        );
        let error = prepare_files(&state, &record.value()).unwrap_err();
        let path = fs::read_to_string(fixture.path().join("staging-path")).expect("init reached");
        assert!(
            error.to_string().contains(&path),
            "error must name retained staging: {error}"
        );
        assert!(
            Path::new(&path).is_dir(),
            "staging disappeared after client error"
        );
    }

    #[test]
    fn import_has_a_longer_deadline_than_regular_commands() {
        let state = tempfile::tempdir().unwrap();
        run_with_binary(
            state.path(),
            state.path(),
            state.path(),
            &["init", "project", "--from", "/source"],
            Path::new("/usr/bin/true"),
        )
        .unwrap();
        let import = bounded_child::TEST_TIMEOUT.get().unwrap();
        run_with_binary(
            state.path(),
            state.path(),
            state.path(),
            &["status"],
            Path::new("/usr/bin/true"),
        )
        .unwrap();
        let regular = bounded_child::TEST_TIMEOUT.get().unwrap();
        assert!(
            import > regular,
            "import and regular commands both use {regular:?}"
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn failed_import_discards_only_its_private_project_before_retry() {
        use std::os::unix::fs::PermissionsExt;
        let state = tempfile::tempdir().expect("state");
        let source = tempfile::tempdir().expect("source");
        let project = studio_runtime_store::sha256_hex(source.path().to_string_lossy().as_bytes());
        let id = format!("import:{project}");
        let intent = json!({"project":project,"source":source.path()});
        let partial = store_path(state.path()).join("projects").join(&project);
        let untouched = store_path(state.path()).join("projects/another-project");
        fs::create_dir_all(&untouched).expect("other project");
        fs::write(untouched.join("keep"), "keep").expect("other file");
        fs::write(source.path().join("evidence"), "source").expect("source file");
        let readonly = partial.join("lines/main/readonly");
        assert!(
            operation(state.path(), &id, &intent, || {
                fs::create_dir_all(&readonly)?;
                fs::write(readonly.join("stale"), "failed import")?;
                std::os::unix::fs::symlink(source.path(), partial.join("source-link"))?;
                fs::set_permissions(&readonly, fs::Permissions::from_mode(0o555))?;
                Err(NativeLayrError::Refused("save: Permission denied".into()))
            })
            .is_err()
        );
        assert!(
            operation(
                state.path(),
                &id,
                &json!({"project":project,"source":"/different-source"}),
                || panic!("A changed request must not discard the partial project")
            )
            .is_err()
        );
        assert!(readonly.join("stale").exists());
        fs::write(source.path().join("evidence"), "current source")
            .expect("edit source before retry");
        let result = operation(state.path(), &id, &intent, || {
            assert!(
                !partial.exists(),
                "Discard the partial project before the retry"
            );
            assert_eq!(fs::read(source.path().join("evidence"))?, b"current source");
            assert_eq!(fs::read(untouched.join("keep"))?, b"keep");
            Ok(json!("rebuilt"))
        })
        .expect("retry the explicit failure");
        assert_eq!(result, json!("rebuilt"));
        assert_eq!(
            operation(state.path(), &id, &intent, || panic!(
                "do not repeat success"
            ))
            .expect("replay"),
            result
        );
    }

    #[test]
    fn import_retry_holds_unknown_outcomes_and_rejects_changed_intent() {
        let state = tempfile::tempdir().expect("state");
        let project = "a".repeat(64);
        let id = format!("import:{project}");
        let intent = json!({"project":project,"source":"/project"});
        let partial = store_path(state.path()).join("projects").join(&project);
        fs::create_dir_all(&partial).expect("partial project");
        fs::write(partial.join("keep"), "unknown import").expect("partial file");
        assert!(
            operation(state.path(), &id, &intent, || Err(
                NativeLayrError::Unknown("lost".into())
            ))
            .is_err()
        );
        assert!(matches!(
            operation(state.path(), &id, &intent, || panic!("hold pending")),
            Err(NativeLayrError::Unknown(_))
        ));
        assert!(
            operation(
                state.path(),
                &id,
                &json!({"project":project,"source":"/other"}),
                || panic!("reject changed intent")
            )
            .is_err()
        );
        assert_eq!(
            fs::read(partial.join("keep")).expect("held partial"),
            b"unknown import"
        );
    }

    #[test]
    fn native_layr_receipt_encoding_preserves_opaque_json_values() {
        for raw in [
            "null",
            &format!(r#"{{"{}":"1"}}"#, studio_json::NUMBER_TOKEN),
            r#"{"nested":[1,null,{"x":true}]}"#,
        ] {
            let value: Value = studio_json::from_str(raw).unwrap();
            // A fixed start time keeps the byte-exact encoding comparable.
            let started = |mut receipt: OperationReceipt| {
                receipt.started_at = Some(7);
                receipt.bytes().unwrap()
            };
            assert_eq!(
                started(OperationReceipt::pending("id", &value)),
                json!({"id":"id","intent":value,"startedAt":7,"status":"pending"})
                    .to_string()
                    .as_bytes()
            );
            assert_eq!(
                started(OperationReceipt::completed("id", &value, &value)),
                json!({"id":"id","intent":value,"startedAt":7,"status":"completed","result":value})
                    .to_string()
                    .as_bytes()
            );
            assert_eq!(
                started(OperationReceipt::failed("id", &value, "problem")),
                json!({"id":"id","intent":value,"startedAt":7,"status":"failed","error":"problem"})
                    .to_string()
                    .as_bytes()
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn failed_import_cleanup_refusal_preserves_original_receipt() {
        let state = tempfile::tempdir().unwrap();
        let root = store_path(state.path());
        let project = "a".repeat(64);
        let id = format!("import:{project}");
        let intent = json!({"project": project, "source": "/source"});
        let folder = settings::metadata(state.path(), &root).join("receipts");
        fs::create_dir_all(&folder).unwrap();
        fs::create_dir_all(root.join("projects").join(&project)).unwrap();
        let path = folder.join(format!(
            "{}.json",
            studio_runtime_store::sha256_hex(id.as_bytes())
        ));
        let original = OperationReceipt::failed(&id, &intent, "original import failure")
            .bytes()
            .unwrap();
        fs::write(&path, &original).unwrap();
        let error = operation_with_preflight_in_store(
            state.path(),
            &root,
            &id,
            &intent,
            || Ok(()),
            || panic!("must not retry"),
        )
        .expect_err("cleanup must refuse");
        assert!(
            error
                .to_string()
                .contains("requires daemon-owned subvolume removal")
        );
        assert_eq!(fs::read(path).unwrap(), original);
    }

    #[test]
    fn native_layr_receipt_replay_preserves_partial_and_uncertain_evidence() {
        use std::cell::Cell;
        let state = tempfile::tempdir().unwrap();
        let root = store_path(state.path());
        let folder = settings::metadata(state.path(), &root).join("receipts");
        fs::create_dir_all(&folder).unwrap();
        let intent = json!({"kind":"inspect","data":[1,null]});
        let calls = Cell::new(0);
        let write = |id: &str, bytes: &[u8]| {
            fs::write(
                folder.join(format!(
                    "{}.json",
                    studio_runtime_store::sha256_hex(id.as_bytes())
                )),
                bytes,
            )
            .unwrap();
        };
        let replay = |id: &str, expected: &Value| {
            operation_with_preflight_in_store(
                state.path(),
                &root,
                id,
                expected,
                || {
                    calls.set(calls.get() + 1);
                    Ok(())
                },
                || {
                    calls.set(calls.get() + 1);
                    Ok(json!("effect"))
                },
            )
        };
        write(
            "done",
            br#"{"intent":{"kind":"inspect","data":[1,null]},"status":"completed"}"#,
        );
        assert_eq!(replay("done", &intent).unwrap(), Value::Null);
        write("duplicate", br#"{"intent":{"kind":"wrong"},"intent":{"kind":"inspect","data":[1,null]},"status":"pending","status":"completed","result":{"opaque":[1,null]}}"#);
        assert_eq!(
            replay("duplicate", &intent).unwrap(),
            json!({"opaque":[1,null]})
        );
        let special: Value = studio_json::from_str(&format!(
            r#"{{"kind":"inspect","data":{{"{}":"1"}}}}"#,
            studio_json::NUMBER_TOKEN
        ))
        .unwrap();
        write("number_key", format!(r#"{{"intent":{{"kind":"inspect","data":{{"{0}":"1"}}}},"status":"completed","result":{{"{0}":"1"}}}}"#, studio_json::NUMBER_TOKEN).as_bytes());
        let expected: Value =
            studio_json::from_str(&format!(r#"{{"{}":"1"}}"#, studio_json::NUMBER_TOKEN)).unwrap();
        assert_eq!(replay("number_key", &special).unwrap(), expected);
        for (id, raw) in [
            (
                "pending",
                br#"{"intent":{"kind":"inspect","data":[1,null]},"status":"pending"}"#.as_slice(),
            ),
            (
                "unknown",
                br#"{"intent":{"kind":"inspect","data":[1,null]},"status":"future"}"#,
            ),
            (
                "wrong_type",
                br#"{"intent":{"kind":"inspect","data":[1,null]},"status":true}"#,
            ),
            (
                "missing",
                br#"{"intent":{"kind":"inspect","data":[1,null]}}"#,
            ),
        ] {
            write(id, raw);
            assert!(
                matches!(replay(id, &intent), Err(NativeLayrError::Unknown(_))),
                "{id}"
            );
        }
        write(
            "mismatch",
            br#"{"intent":{"kind":"wrong"},"status":"completed"}"#,
        );
        assert!(
            matches!(replay("mismatch", &intent), Err(NativeLayrError::Refused(ref message)) if message == "Request ID has different content")
        );
        write("null", b"null");
        assert!(matches!(
            replay("null", &Value::Null),
            Err(NativeLayrError::Unknown(_))
        ));
        write(
            "failed",
            br#"{"intent":{"kind":"inspect","data":[1,null]},"status":"failed","error":7}"#,
        );
        assert!(
            matches!(replay("failed", &intent), Err(NativeLayrError::Refused(ref message)) if message == "operation failed")
        );
        for (id, raw) in [
            ("syntax", b"{".to_vec()),
            ("number", br#"{"ignored":1e999}"#.to_vec()),
            (
                "depth",
                format!("{{\"ignored\":{}0{}}}", "[".repeat(140), "]".repeat(140)).into_bytes(),
            ),
        ] {
            write(id, &raw);
            assert!(
                matches!(replay(id, &intent), Err(NativeLayrError::Refused(ref message)) if message != "Request ID has different content"),
                "{id}"
            );
        }
        let save = json!({"kind":"save"});
        let object_statuses = [
            (
                "object_completed",
                br#"{"intent":{"kind":"save"},"status":{"completed":null}}"#.as_slice(),
            ),
            (
                "object_failed",
                br#"{"intent":{"kind":"save"},"status":{"failed":null}}"#,
            ),
        ]
        .map(|(id, raw)| {
            write(id, raw);
            matches!(replay(id, &save), Err(NativeLayrError::Unknown(_)))
        });
        assert_eq!(
            object_statuses,
            [true, true],
            "object statuses must remain unknown"
        );
        assert_eq!(
            calls.get(),
            0,
            "replayed or malformed receipts cannot run preflight or effect"
        );

        write("save", br#"{"intent":{"kind":"save"},"status":"failed"}"#);
        assert_eq!(replay("save", &save).unwrap(), json!("effect"));
        assert_eq!(replay("save", &save).unwrap(), json!("effect"));
        assert_eq!(calls.get(), 2, "explicit failed save executes once");

        let project = "a".repeat(64);
        let id = format!("import:{project}");
        let import = json!({"project":project});
        write(
            &id,
            format!(r#"{{"intent":{},"status":"failed"}}"#, import).as_bytes(),
        );
        assert_eq!(replay(&id, &import).unwrap(), json!("effect"));
        assert_eq!(replay(&id, &import).unwrap(), json!("effect"));
        assert_eq!(calls.get(), 4, "explicit failed import executes once");
    }

    #[test]
    fn review_preflight_failure_does_not_poison_receipt() {
        let state = tempfile::tempdir().expect("fixture operation succeeds");
        let intent = json!({"kind":"import"});
        assert!(
            operation_with_preflight(
                state.path(),
                "space",
                &intent,
                || Err(NativeLayrError::Refused("Insufficient free space".into())),
                || Ok(json!("must not run"))
            )
            .is_err()
        );
        assert_eq!(
            operation_with_preflight(
                state.path(),
                "space",
                &intent,
                || Ok(()),
                || Ok(json!("retried"))
            )
            .expect("fixture operation succeeds"),
            json!("retried")
        );
    }
}

#[cfg(test)]
mod worker_base_tests {
    use super::*;
    #[test]
    fn concurrent_accepts_record_serial_main_states() {
        use std::io::{Read, Write};
        use std::os::unix::fs::PermissionsExt;
        use std::sync::mpsc;
        use std::time::Duration;
        let state = tempfile::tempdir().unwrap();
        let root = super::super::store_path(state.path());
        let project = "a".repeat(64);
        let main = main_path(state.path(), &project);
        std::fs::create_dir_all(&main).unwrap();
        std::fs::write(root.join("head"), "initial").unwrap();
        for name in ["first", "second", "release"] {
            // rustix has no mknodat on macOS; the fixture runs mkfifo directly.
            #[allow(clippy::disallowed_methods)]
            let created = std::process::Command::new("mkfifo")
                .arg(root.join(name))
                .status()
                .unwrap();
            assert!(created.success());
        }
        let signal = |name: &str| {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(root.join(name))
                .unwrap();
            let mut reader = file.try_clone().unwrap();
            let (sender, receiver) = mpsc::channel();
            let thread = std::thread::spawn(move || {
                let mut byte = [0];
                reader.read_exact(&mut byte).unwrap();
                sender.send(()).unwrap();
            });
            (file, receiver, thread)
        };
        let (_first, first_rx, first_reader) = signal("first");
        let (mut second, second_rx, second_reader) = signal("second");
        let binary = state.path().join("fake-layr");
        std::fs::write(
            &binary,
            r#"#!/bin/sh
if [ "$3" = rev-parse ]; then
  if [ ! -e "$LAYR_ROOT/released" ]; then
    if mkdir "$LAYR_ROOT/entered" 2>/dev/null; then
      printf x > "$LAYR_ROOT/first"
      read unused < "$LAYR_ROOT/release"
    else
      printf x > "$LAYR_ROOT/second"
    fi
  fi
  cat "$LAYR_ROOT/head"
else
  printf '%s' "$4" > "$LAYR_ROOT/head"
fi
"#,
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _binary_override = super::super::override_layr_binary(binary);
        let lead = json!({"id":"lead","isLead":true,"layrReady":true,"layrProjectId":project,"layrLine":"main","cwd":main});
        let (sender, receiver) = mpsc::channel();
        let spawn = |id: &'static str| {
            let line = format!("agent-{}", studio_runtime_store::sha256_hex(id.as_bytes()));
            let cwd = main.parent().unwrap().join(&line);
            std::fs::create_dir_all(&cwd).unwrap();
            let worker =
                json!({"id":id,"layrReady":true,"layrProjectId":project,"layrLine":line,"cwd":cwd});
            let (state, lead, sender) = (state.path().to_owned(), lead.clone(), sender.clone());
            std::thread::spawn(move || {
                sender
                    .send(merge(&state, &lead, &worker, "reviewed", id))
                    .unwrap();
            })
        };
        let first = spawn("one");
        first_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("first merge reached rev-parse");
        let lock_name = format!("{}:project:{project}", root.display());
        let probe = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(
                state
                    .path()
                    .join(NATIVE_LAYR_DIRECTORY)
                    .join("locks")
                    .join(studio_runtime_store::sha256_hex(lock_name.as_bytes())),
            )
            .unwrap();
        let project_lock_held = probe.try_lock().is_err();
        drop(probe);
        let second_thread = spawn("two");
        let second_started_early = second_rx.try_recv().is_ok();
        std::fs::write(root.join("released"), "").unwrap();
        writeln!(
            OpenOptions::new()
                .write(true)
                .open(root.join("release"))
                .unwrap(),
            "go"
        )
        .unwrap();
        if !second_started_early {
            second.write_all(b"x").unwrap();
        }
        first.join().unwrap();
        second_thread.join().unwrap();
        first_reader.join().unwrap();
        second_reader.join().unwrap();
        let mut receipts = receiver
            .try_iter()
            .map(|r| r.unwrap()["merge"].clone())
            .collect::<Vec<_>>();
        assert!(
            project_lock_held,
            "first merge did not hold the project lock during rev-parse"
        );
        assert!(
            !second_started_early,
            "second merge reached rev-parse while the first was held"
        );
        assert_eq!(receipts.len(), 2);
        receipts.sort_by_key(|r| r["from_state"].as_str().unwrap() != "initial");
        assert_eq!(receipts[0]["from_state"], "initial");
        assert_eq!(receipts[0]["to_state"], receipts[1]["from_state"]);
    }
    #[test]
    fn native_layr_worker_line_uses_requested_base() {
        let state = tempfile::tempdir().unwrap();
        let root = super::super::store_path(state.path());
        let main = main_path(state.path(), "project");
        std::fs::create_dir_all(&main).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let binary = state.path().join("fake-layr");
        std::fs::write(
            &binary,
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$LAYR_ROOT/commands"
if [ "$3 $4" = 'rev-parse missing' ]; then echo 'unknown revision' >&2; exit 1; fi
"#,
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _binary_override = super::super::override_layr_binary(binary);
        let record = json!({"id":"worker", "layrBaseState":"requested"});
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = observed.clone();
        let progress: ProgressSink =
            std::sync::Arc::new(move |value| captured.lock().unwrap().push(value));
        let result = create_worker_line(
            state.path(),
            &root,
            &main,
            "project",
            "worker",
            &main,
            &record,
            "main",
            Some(&progress),
        );
        assert_eq!(
            observed.lock().unwrap().as_slice(),
            &[CopyProgress {
                phase: CopyPhase::CreatingLine,
                done: 0,
                total: None,
                unit: CopyUnit::Files
            }]
        );
        assert!(result.is_ok(), "{result:?}");
        let receipt_path = settings::metadata(state.path(), &root)
            .join("receipts")
            .join(format!(
                "{}.json",
                studio_runtime_store::sha256_hex(b"line:worker")
            ));
        let receipt: Value =
            studio_json::from_slice(&std::fs::read(receipt_path).unwrap()).unwrap();
        assert_eq!(receipt["intent"]["start"], "requested");
        let commands = std::fs::read_to_string(root.join("commands")).unwrap();
        assert!(commands.contains("line new worker requested"));
        let replay = create_worker_line(
            state.path(),
            &root,
            &main,
            "project",
            "worker",
            &main,
            &record,
            "main",
            None,
        )
        .unwrap();
        assert_eq!(replay, result.unwrap());
        let changed = json!({"id":"worker", "layrBaseState":"other"});
        assert!(
            create_worker_line(
                state.path(),
                &root,
                &main,
                "project",
                "worker",
                &main,
                &changed,
                "main",
                None
            )
            .is_err()
        );
        let default = json!({"id":"default"});
        let legacy_intent = json!({"project":"project","line":"default"});
        let legacy_result = json!({"path":main,"line":"default"});
        operation_in_store(state.path(), &root, "line:default", &legacy_intent, || {
            Ok(legacy_result.clone())
        })
        .unwrap();
        assert_eq!(
            create_worker_line(
                state.path(),
                &root,
                &main,
                "project",
                "default",
                &main,
                &default,
                "cached-main",
                None
            )
            .unwrap(),
            legacy_result
        );
        let default = json!({"id":"default-new"});
        create_worker_line(
            state.path(),
            &root,
            &main,
            "project",
            "default",
            &main,
            &default,
            "cached-main",
            None,
        )
        .unwrap();
        let missing = json!({"id":"missing", "layrBaseState":"missing"});
        let error = create_worker_line(
            state.path(),
            &root,
            &main,
            "project",
            "missing",
            &main,
            &missing,
            "main",
            None,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("base revision could not be resolved")
        );
        let commands = std::fs::read_to_string(root.join("commands")).unwrap();
        assert!(commands.contains("line new default cached-main"));
        assert!(!commands.contains("line new missing"));
        assert!(!commands.contains("line new worker other"));
    }
}

#[cfg(test)]
mod merge_conflict_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn merge_failure_preserves_conflict_output() {
        let state = tempfile::tempdir().unwrap();
        let binary = state.path().join("layr");
        std::fs::write(
            &binary,
            "#!/bin/sh\nprintf 'CONFLICT (content): Merge conflict in file\\n'\nexit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        let error = run_with_binary(
            state.path(),
            state.path(),
            state.path(),
            &["merge", "worker"],
            &binary,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("CONFLICT (content): Merge conflict in file"),
            "{error}"
        );
    }

    #[test]
    fn merge_conflict_after_long_output_keeps_its_type_and_message() {
        let state = tempfile::tempdir().unwrap();
        let binary = state.path().join("layr");
        std::fs::write(&binary, "#!/bin/sh\ni=0; while [ $i -lt 100 ]; do echo 'Auto-merging a-very-long-file-name'; i=$((i+1)); done\necho 'CONFLICT (content): Merge conflict in important-file'\nexit 1\n").unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        let error = run_with_binary(
            state.path(),
            state.path(),
            state.path(),
            &["merge", "worker"],
            &binary,
        )
        .unwrap_err();
        assert!(
            matches!(error, NativeLayrError::Conflict(ref message) if message.contains("CONFLICT (content): Merge conflict in important-file")),
            "{error}"
        );
    }
}

#[cfg(test)]
mod progress_tests {
    #[tokio::test(start_paused = true)]
    async fn delayed_consumer_publishes_only_the_latest_count_at_the_interval() {
        use studio_runtime_schema::layr_progress::{CopyPhase, CopyProgress, CopyUnit};
        let (send, mut receive) = super::progress_channel();
        let progress = |done| CopyProgress {
            phase: CopyPhase::Staging,
            done,
            total: Some(10),
            unit: CopyUnit::Files,
        };
        send(progress(1));
        let first = super::next_progress(&mut receive, None).await.unwrap();
        assert_eq!(first.done, 1);
        let last = tokio::time::Instant::now();
        send(progress(2));
        let pending = tokio::spawn(async move {
            super::next_progress(&mut receive, Some(last))
                .await
                .unwrap()
        });
        tokio::task::yield_now().await;
        send(progress(3));
        tokio::time::advance(
            super::PROGRESS_PUBLICATION_INTERVAL - std::time::Duration::from_millis(1),
        )
        .await;
        assert!(!pending.is_finished());
        send(progress(4));
        tokio::time::advance(std::time::Duration::from_millis(1)).await;
        assert_eq!(pending.await.unwrap().done, 4);
    }
    #[tokio::test]
    async fn progress_is_published_to_the_agent_and_cleared() {
        use studio_runtime_schema::layr_progress::{CopyPhase, CopyProgress, CopyUnit};
        use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, StoreError};
        let (_dir, store) = crate::entity_publication::test_support::store().await;
        let expected = studio_json::json!({"compactions":0,"accountKey":"default","provider":"codex","id":"a","rootId":"a","epoch":2,"status":"starting","cwd":"/one","autoWake":true});
        let inserted = expected.clone();
        store
            .transaction_with(move |tx| -> Result<(), StoreError> {
                AgentRepository::put(tx, &AgentRecord::from_value(inserted)?)?;
                Ok(())
            })
            .await
            .unwrap();
        let progress = CopyProgress {
            phase: CopyPhase::Staging,
            done: 2,
            total: Some(4),
            unit: CopyUnit::Files,
        };
        super::publish_progress(&store, &expected, Some(progress))
            .await
            .unwrap();
        let (_, summary) =
            crate::entity_publication::test_support::client_entity(&store, "agentSummary", "a")
                .unwrap();
        assert_eq!(summary["startWait"]["progress"]["done"], 2);
        store
            .transaction_with(|tx| -> Result<(), StoreError> {
                let record = AgentRepository::get(tx, &AgentId::from("a"))?
                    .unwrap()
                    .value();
                assert_eq!(
                    crate::sync_views::agent_start_wait(&record)
                        .unwrap()
                        .unwrap()
                        .epoch(),
                    2
                );
                assert_eq!(record["startWait"]["progress"]["done"], 2);
                let summary =
                    crate::sync_views::project_persisted_record("agentSummary", &record).unwrap();
                assert_eq!(summary["startWait"]["progress"]["done"], 2);
                Ok(())
            })
            .await
            .unwrap();
        super::publish_progress(&store, &expected, None)
            .await
            .unwrap();
        store
            .transaction_with(|tx| -> Result<(), StoreError> {
                let record = AgentRepository::get(tx, &AgentId::from("a"))?
                    .unwrap()
                    .value();
                assert!(record.get("startWait").is_none());
                Ok(())
            })
            .await
            .unwrap();
    }
    #[test]
    fn progress_guard_rejects_a_changed_workspace_or_inactive_agent() {
        let expected = studio_json::json!({"id":"a","epoch":2,"status":"starting","cwd":"/one","autoWake":true});
        assert!(super::progress_guard(&expected, &expected));
        for changed in [
            studio_json::json!({"id":"a","epoch":3,"status":"starting","cwd":"/one","autoWake":true}),
            studio_json::json!({"id":"a","epoch":2,"status":"running","cwd":"/one","autoWake":true}),
            studio_json::json!({"id":"a","epoch":2,"status":"starting","cwd":"/other","autoWake":true}),
        ] {
            assert!(!super::progress_guard(&changed, &expected));
        }
    }
    #[test]
    fn git_import_progress_is_typed_at_the_boundary() {
        let progress =
            super::parse_import_progress(b"layr: imported 50 commits of local/main").unwrap();
        assert_eq!(
            progress.phase,
            studio_runtime_schema::layr_progress::CopyPhase::ImportingCommits
        );
        assert_eq!(progress.done, 50);
        assert_eq!(progress.total, None);
        assert_eq!(
            progress.unit,
            studio_runtime_schema::layr_progress::CopyUnit::Commits
        );
        assert!(super::parse_import_progress(b"unrelated diagnostic").is_none());
    }
}
