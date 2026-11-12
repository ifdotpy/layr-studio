//! Temporary stdio children behind the async provider seam. A task owns the
//! process group through bounded cleanup even if its caller drops. A reap timeout
//! reports the child identity; it does not prove termination or cap surviving children.
use crate::{child_priority::ChildPriority, operational_log::OperationalLog, provider_seam::*, *};
use process_wrap::tokio::{CommandWrap, ProcessGroup};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{self, Write as IoWrite},
    process::Stdio,
    sync::{Arc, Mutex, OnceLock},
};
#[cfg(test)]
use studio_runtime_providers::decode_journal_entry_borrowed;
use studio_runtime_providers::provider_diagnostics::{
    REPLY_TIMING_DIAGNOSTIC_PREFIX, ReplyTimingDiagnostic, STDIN_TIMING_DIAGNOSTIC_PREFIX,
    StdinTimingDiagnostic,
};
use studio_runtime_providers::{
    CodexIncomingMethod, DecodedJournalPayload, Frame, Method, ProviderKind, RequestId,
    ResponseBody,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::Command,
    sync::{mpsc, watch},
    time::Instant,
};

// POSIX ESRCH: the process group exited before the stop signal.
const NO_SUCH_PROCESS: i32 = 3;
const REAP_GRACE: std::time::Duration = std::time::Duration::from_secs(1);
const STDERR_LINE_LIMIT: usize = 64 * 1024;
// Set to 1 only for the announced provider stdin timing profile.
const STDIN_TIMING_DIAGNOSTICS_ENV: &str = "CODEX_RUNTIME_STDIN_TIMING_DIAGNOSTICS";
/// Requests whose physical stdin write and reply arrival the diagnostic journals.
const WITNESSED_METHODS: [Method; 3] =
    [Method::TurnStart, Method::ThreadStart, Method::ThreadResume];

static STDIN_TIMING_DIAGNOSTICS_ENABLED: OnceLock<bool> = OnceLock::new();

fn stdin_timing_diagnostics_enabled() -> bool {
    *STDIN_TIMING_DIAGNOSTICS_ENABLED
        .get_or_init(|| std::env::var(STDIN_TIMING_DIAGNOSTICS_ENV).is_ok_and(|value| value == "1"))
}

fn witnessed_method(write: &PreparedWrite) -> Option<Method> {
    write
        .frame()
        .method()
        .and_then(Method::from_wire)
        .filter(|method| WITNESSED_METHODS.contains(method))
}

/// Witnessed requests written but not yet answered, shared by the writer and reader.
#[derive(Clone, Default)]
struct PendingReplies(Arc<Mutex<HashMap<RequestId, (OperationId, Method)>>>);

impl PendingReplies {
    fn register(&self, write: &PreparedWrite, method: Method) {
        let Some(id) = write.frame().id() else {
            return;
        };
        if let Ok(mut pending) = self.0.lock() {
            pending.insert(id, (write.operation_id().clone(), method));
        }
    }

    fn take(&self, id: &RequestId) -> Option<(OperationId, Method)> {
        self.0.lock().ok()?.remove(id)
    }
}

/// Reader-side half of the diagnostic: journals the reply to a witnessed request.
#[derive(Clone)]
struct ReplyWitness {
    handle: ProviderHandle,
    generation: Generation,
    pending: PendingReplies,
}

impl ReplyWitness {
    fn line(&self, frame: &Frame, sample_timestamp: impl FnOnce() -> u128) -> Option<Vec<u8>> {
        let Frame::Response { id, body, .. } = frame else {
            return None;
        };
        let (operation_id, method) = self.pending.take(id)?;
        let diagnostic = ReplyTimingDiagnostic {
            provider_handle: self.handle.as_str(),
            generation: self.generation.0,
            operation_id: &operation_id.0,
            request_id: id,
            method: method.as_str(),
            reply_at_ns: sample_timestamp(),
            outcome: match body {
                ResponseBody::Result(_)
                | ResponseBody::CodexUnparsedResult { .. }
                | ResponseBody::Uncorrelated(_)
                | ResponseBody::CodexResult(_)
                | ResponseBody::CodexServerResult(_) => "result",
                ResponseBody::Error(_) | ResponseBody::CodexError(_) => "error",
            },
        };
        diagnostic_line(REPLY_TIMING_DIAGNOSTIC_PREFIX.as_bytes(), &diagnostic)
    }
}

fn diagnostic_line(prefix: &[u8], diagnostic: &impl Serialize) -> Option<Vec<u8>> {
    let payload = studio_json::to_vec(diagnostic).ok()?;
    let mut line = Vec::with_capacity(prefix.len() + payload.len() + 1);
    line.extend_from_slice(prefix);
    line.extend_from_slice(&payload);
    line.push(b'\n');
    Some(line)
}

fn stdin_timing_diagnostic_line(
    handle: &ProviderHandle,
    generation: Generation,
    write: &PreparedWrite,
    stdin_flushed_at_ns: u128,
) -> Option<Vec<u8>> {
    let method = witnessed_method(write)?;
    let request_id = write.frame().id()?;
    let diagnostic = StdinTimingDiagnostic {
        provider_handle: handle.as_str(),
        generation: generation.0,
        operation_id: &write.operation_id().0,
        request_id,
        method: method.as_str(),
        stdin_flushed_at_ns,
    };
    diagnostic_line(STDIN_TIMING_DIAGNOSTIC_PREFIX.as_bytes(), &diagnostic)
}

fn write_provider_diagnostic_best_effort(line: &[u8]) {
    // The verifier treats a missing diagnostic as incomplete; diagnostic I/O
    // errors must never alter provider write or receipt behavior.
    match io::stderr().lock().write_all(line) {
        Ok(()) | Err(_) => {}
    }
}

/// For each leader PID: whether the leader was still an unreaped child of this
/// process when cleanup signalled its group.
#[cfg(test)]
static LEADER_UNREAPED_AT_SIGNAL: Mutex<std::collections::BTreeMap<u32, bool>> =
    Mutex::new(std::collections::BTreeMap::new());

async fn reap_with_deadline<T>(
    pid: Option<u32>,
    wait: impl std::future::Future<Output = io::Result<T>>,
) -> io::Result<T> {
    tokio::time::timeout(REAP_GRACE, wait)
        .await
        .unwrap_or_else(|_| {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("provider child {pid:?} outlived reap grace; termination is unconfirmed"),
            ))
        })
}

pub struct StdioProviderTaskFactory {
    /// End of a bounded one-off exchange; `None` for a persistent account session.
    pub deadline: Option<Instant>,
    /// Bytes in one stdout line, the provider's JSON-RPC frame.
    pub max_frame_bytes: usize,
    /// Total stdout bytes of a bounded one-off exchange; `None` for a persistent
    /// account session, whose stdout is bounded per frame only.
    pub max_output_bytes: Option<usize>,
    pub stderr_log: Option<OperationalLog>,
}

enum StdioIoCompletion {
    Read(Result<(), AdapterError>),
    Write(Result<(), AdapterError>),
}

impl ProviderTaskFactory for StdioProviderTaskFactory {
    fn open(
        &self,
        handle: ProviderHandle,
        options: OpenOptions,
        shutdown: Shutdown,
    ) -> OpenFuture<'_> {
        self.open_with_priority(handle, options, shutdown, ChildPriority::Normal)
    }
}

impl StdioProviderTaskFactory {
    /// Open a provider child with the caller's explicit scheduling policy.
    /// Shared helper callers use the trait implementation's inherited priority.
    pub fn open_with_priority(
        &self,
        handle: ProviderHandle,
        options: OpenOptions,
        shutdown: Shutdown,
        priority: ChildPriority,
    ) -> OpenFuture<'_> {
        Box::pin(async move {
            if self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                return Err(AdapterError::Timeout);
            }
            let generation = options.expected_generation.unwrap_or(Generation(1));
            let Some(executable) = options.launch.command.first() else {
                return Err(AdapterError::InvalidConfiguration);
            };
            if self.max_frame_bytes == 0
                || self.max_output_bytes == Some(0)
                || generation.0 == 0
                || options.launch.handled_cursor != 0
                || !options.launch.retained_response_ids.is_empty()
            {
                return Err(AdapterError::InvalidConfiguration);
            }
            let mut command = Command::new(executable);
            command
                .args(&options.launch.command[1..])
                .env_clear()
                .envs(&options.launch.environment)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Some(cwd) = &options.launch.cwd {
                command.current_dir(cwd);
            }
            let mut wrapped = CommandWrap::from(command);
            wrapped.wrap(ProcessGroup::leader());
            priority.apply_before_spawn(&mut wrapped);
            let spawn_started_at = crate::clock::boundary_stamp();
            let mut child = wrapped.spawn().map_err(process_error)?;
            let (commands, mut inbox) = mpsc::channel(LANE_CAPACITY);
            let (entries, journal) = mpsc::channel(LANE_CAPACITY);
            let diagnostic_handle = handle.clone();
            let pending_replies = PendingReplies::default();
            let reply_witness = stdin_timing_diagnostics_enabled().then(|| ReplyWitness {
                handle: handle.clone(),
                generation,
                pending: pending_replies.clone(),
            });
            let (state, status) = watch::channel(AttachmentStatus {
                handle: handle.clone(),
                open_attempted: true,
                info: Some(AttachmentInfo {
                    handle,
                    generation,
                    pid: child.id().unwrap_or(0),
                    acknowledged: EventCursor(0),
                    resumed: false,
                    spawn_started_at: Some(spawn_started_at),
                    initialized_at: None,
                }),
                initialization: InitializationStatus::AwaitingReply,
            });
            let deadline = self.deadline;
            let frame_limit = self.max_frame_bytes;
            let output_limit = self.max_output_bytes;
            let stderr_log = self.stderr_log.clone();
            let stderr_log_for_flush = stderr_log.clone();
            let mut stop = shutdown.subscribe();
            let task = tokio::spawn(async move {
                // Start the stderr owner before selecting on shutdown. Even an
                // immediately cancelled exchange must retain and drain this pipe.
                let stderr_drain = child.stderr().take().map(|mut stderr| {
                    let log = stderr_log.clone();
                    tokio::spawn(async move { drain_stderr(&mut stderr, log.as_ref()).await })
                });
                let stderr_available = stderr_drain.is_some();
                // Keep the reader's buffered bytes, cursor, and pending journal send
                // alive when exit cancels the exchange's writer.
                let stdin = child.stdin().take();
                let stdout = child.stdout().take();
                let reader = async {
                    let stdout = stdout.ok_or(AdapterError::InvalidConfiguration)?;
                    read_entries(
                        stdout,
                        entries,
                        options.clock,
                        generation,
                        frame_limit,
                        output_limit,
                        ReaderProtocol {
                            codec: options.launch.codec,
                            reply_witness,
                        },
                    )
                    .await
                };
                tokio::pin!(reader);
                let mut pending_write = None;
                let exchange = async {
                    let Some(stdin) = stdin else {
                        return StdioIoCompletion::Read(Err(AdapterError::InvalidConfiguration));
                    };
                    if !stderr_available {
                        return StdioIoCompletion::Read(Err(AdapterError::InvalidConfiguration));
                    }
                    tokio::select! {
                        result = write_commands(
                            stdin,
                            &mut inbox,
                            diagnostic_handle,
                            generation,
                            pending_replies,
                            &mut pending_write,
                        ) => StdioIoCompletion::Write(result),
                        result = &mut reader => StdioIoCompletion::Read(result),
                    }
                };
                let bounded = async {
                    match deadline {
                        Some(deadline) => tokio::time::timeout_at(deadline, exchange)
                            .await
                            .unwrap_or(StdioIoCompletion::Read(Err(AdapterError::Timeout))),
                        None => exchange.await,
                    }
                };
                let pid = child.id();
                // The leader's exit is observed without reaping it, so the group ID
                // stays owned until the group signal below (`owned_process_group`).
                let leader_exit = async {
                    match pid {
                        Some(pid) => crate::owned_process_group::exited_unreaped(pid).await,
                        None => Err(io::Error::other("the provider child has no process ID")),
                    }
                };
                let (result, exit_unobserved) = tokio::select! {
                    biased;
                    // Dropping the exchange closed stdin, the provider's exit
                    // request. Keep its final stdout, such as a completed turn,
                    // before the group is killed.
                    _ = Shutdown::cancelled(&mut stop) => {
                        interrupt_stdin_write(&mut pending_write);
                        (drain_stdout_before_cleanup(&mut reader, &mut inbox, &stop, deadline).await, false)
                    },
                    // Observe exit before accepting another stdin write. The drain
                    // retains stdout and serves ACKs without touching the dead stdin.
                    exited = leader_exit => match exited {
                        Ok(()) => {
                            interrupt_stdin_write(&mut pending_write);
                            (drain_stdout_before_cleanup(&mut reader, &mut inbox, &stop, deadline).await, false)
                        },
                        Err(error) => (Err(process_error(error)), true),
                    },
                    // A full journal must never prevent observing and reaping the child.
                    result = bounded => match result {
                        StdioIoCompletion::Write(Err(error)) if matches!(&error,
                            AdapterError::Supervisor(SupervisorError::Io(cause))
                            if cause.kind() == io::ErrorKind::BrokenPipe
                        ) => {
                            // A child can close stdin before exiting. EPIPE must
                            // stop writes without dropping its final stdout.
                            interrupt_stdin_write(&mut pending_write);
                            let drained = drain_stdout_before_cleanup(&mut reader, &mut inbox, &stop, deadline).await;
                            let result = match drained {
                                Err(AdapterError::Supervisor(SupervisorError::ConnectionLost)) => Err(error),
                                result => result,
                            };
                            (result, false)
                        }
                        StdioIoCompletion::Read(result) | StdioIoCompletion::Write(result) => (result, false),
                    },
                };
                interrupt_stdin_write(&mut pending_write);
                close_commands(&mut inbox);
                // Signal the group before the leader is reaped. If its exit could not
                // be observed, the leader may already be reaped: send nothing.
                // Always wait, including when the group already exited (ESRCH).
                #[cfg(test)]
                if let Some(pid) = pid {
                    let unreaped = crate::owned_process_group::exited_unreaped_now(pid).is_ok();
                    LEADER_UNREAPED_AT_SIGNAL
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .insert(pid, unreaped);
                }
                let killed = if exit_unobserved {
                    Ok(())
                } else {
                    child.start_kill()
                };
                #[cfg(test)]
                let killed = match crate::owned_process_group::test_darwin_race_until(pid) {
                    Some(_) => Err(io::Error::from_raw_os_error(
                        rustix::io::Errno::PERM.raw_os_error(),
                    )),
                    None => killed,
                };
                let reaped = reap_with_deadline(pid, child.wait()).await;
                let killed = match killed {
                    Err(error) if error.raw_os_error() == Some(NO_SUCH_PROCESS) => Ok(()),
                    Err(error) if reaped.is_ok() => {
                        let deadline = Instant::now() + REAP_GRACE;
                        if crate::owned_process_group::vanishes_after_reap(pid, &error, deadline)
                            .await
                        {
                            Ok(())
                        } else {
                            Err(error)
                        }
                    }
                    killed => killed,
                };
                let cleanup = killed.and(reaped.map(|_| ())).map_err(process_error);
                // close() fences new admission but outstanding Tokio permits can
                // still enqueue. After bounded reaping, this task owns only the
                // closed channel and rejects late writes without holding cleanup.
                // recv() ends when the remaining permits are sent or dropped.
                drop(tokio::spawn(reject_late_commands(inbox)));
                let stderr_result = match stderr_drain {
                    Some(mut stderr_drain) => {
                        match tokio::time::timeout(REAP_GRACE, &mut stderr_drain).await {
                            Ok(Ok(result)) => result,
                            Ok(Err(_)) => Err(AdapterError::TaskFailed),
                            Err(_) => {
                                stderr_drain.abort();
                                Err(AdapterError::Timeout)
                            }
                        }
                    }
                    None => Ok(()),
                };
                // The log writer is a separate thread; completion includes its flush.
                let stderr_flush = match stderr_log_for_flush {
                    Some(log) => crate::blocking_pool::spawn_blocking(
                        "runtime-process/provider_stdio",
                        move || log.flush(),
                    )
                    .await
                    .map_err(|_| AdapterError::TaskFailed)
                    .and_then(|result| result.map_err(process_error)),
                    None => Ok(()),
                };
                let cleanup_error = cleanup.as_ref().err().map(ToString::to_string);
                drop(state);
                TaskExit {
                    result: cleanup.and(result).and(stderr_result).and(stderr_flush),
                    cleanup_error,
                    recovery_needed: None,
                    // The attachment owns the child and ended it above.
                    provider_retained: false,
                }
            });
            Ok(ProviderAttachment::from_task(
                commands, journal, status, shutdown, task,
            ))
        })
    }
}

/// Delivers the stdout an ended exchange still holds, after leader exit, a
/// closed stdin, or a requested shutdown, until EOF or the bounded grace.
async fn drain_stdout_before_cleanup(
    reader: impl std::future::Future<Output = Result<(), AdapterError>>,
    commands: &mut mpsc::Receiver<AdapterCommand>,
    stop: &watch::Receiver<bool>,
    deadline: Option<Instant>,
) -> Result<(), AdapterError> {
    // A full journal or a descendant holding stdout open must not prevent
    // signalling and reaping the still-owned group. Never extend the caller's
    // deadline. Shutdown does not cut the drain short: frames the provider
    // writes while exiting are part of its session.
    let grace_deadline = Instant::now() + REAP_GRACE;
    let drain_deadline = deadline.map_or(grace_deadline, |end| end.min(grace_deadline));
    let drain = async {
        tokio::select! {
            result = reader => result,
            result = commands_after_exit(commands) => result,
        }
    };
    let result = tokio::time::timeout_at(drain_deadline, drain)
        .await
        .unwrap_or_else(|_| {
            if deadline.is_some_and(|end| end <= grace_deadline) {
                Err(AdapterError::Timeout)
            } else {
                Err(SupervisorError::ConnectionLost.into())
            }
        });
    close_commands(commands);
    // A requested shutdown keeps its successful result whatever ended the drain.
    // A closed sender counts as shutdown, as in `Shutdown::cancelled`.
    if *stop.borrow() || stop.has_changed().is_err() {
        Ok(())
    } else {
        result
    }
}

fn close_commands(commands: &mut mpsc::Receiver<AdapterCommand>) {
    commands.close();
    while let Ok(command) = commands.try_recv() {
        command.reject();
    }
}

async fn reject_late_commands(mut commands: mpsc::Receiver<AdapterCommand>) {
    while let Some(command) = commands.recv().await {
        command.reject();
    }
}

struct PendingStdinWrite {
    reply: Option<tokio::sync::oneshot::Sender<CommandReceipt>>,
    /// Bytes accepted by the OS pipe, independent of child reads.
    bytes_written: usize,
}

impl PendingStdinWrite {
    fn new(reply: tokio::sync::oneshot::Sender<CommandReceipt>) -> Self {
        Self {
            reply: Some(reply),
            bytes_written: 0,
        }
    }
}

fn interrupt_stdin_write(pending: &mut Option<PendingStdinWrite>) {
    if let Some(PendingStdinWrite {
        reply: Some(reply),
        bytes_written,
    }) = pending.take()
    {
        // Delivery is best effort when the caller itself has gone. Losing an
        // active caller must not turn accepted bytes into a NotSent claim.
        let _ignored = reply.send(CommandReceipt::WriteInterrupted { bytes_written });
    }
}

async fn commands_after_exit(
    commands: &mut mpsc::Receiver<AdapterCommand>,
) -> Result<(), AdapterError> {
    while let Some(command) = commands.recv().await {
        match command {
            AdapterCommand::Ack { cursor, reply } => {
                if reply.send(CommandReceipt::Acknowledged(cursor)).is_err() {
                    return Err(AdapterError::ConsumerClosed);
                }
            }
            // Zero accepted bytes proves NotSent. The explicit interruption
            // receipt also tells initialization to keep consuming the tail.
            // Never touch stdin or wait for receipt observation on this path.
            AdapterCommand::Write { reply, .. } => {
                let _ignored = reply.send(CommandReceipt::WriteInterrupted { bytes_written: 0 });
            }
        }
    }
    Err(AdapterError::ConsumerClosed)
}

pub(crate) async fn drain_stderr(
    stderr: &mut (impl AsyncRead + Unpin),
    log: Option<&OperationalLog>,
) -> Result<(), AdapterError> {
    if log.is_none() {
        tokio::io::copy(stderr, &mut tokio::io::sink())
            .await
            .map_err(process_error)?;
        return Ok(());
    }
    let log = log.ok_or(AdapterError::InvalidConfiguration)?;
    let mut reader = BufReader::new(stderr);
    let mut bytes = [0_u8; 8192];
    let mut line = Vec::new();
    let mut discarding = false;
    loop {
        let count = tokio::io::AsyncReadExt::read(&mut reader, &mut bytes)
            .await
            .map_err(process_error)?;
        if count == 0 {
            if discarding {
                log.note_dropped_line();
            } else if !line.is_empty() {
                log.write_line(&line);
            }
            return Ok(());
        }
        for byte in &bytes[..count] {
            if !discarding {
                if line.len() == STDERR_LINE_LIMIT {
                    line.clear();
                    discarding = true;
                } else {
                    line.push(*byte);
                }
            }
            if *byte == b'\n' {
                if discarding {
                    log.note_dropped_line();
                    discarding = false;
                } else {
                    log.write_line(&line);
                    line.clear();
                }
            }
        }
    }
}

fn process_error(error: io::Error) -> AdapterError {
    AdapterError::Supervisor(SupervisorError::Io(error))
}

async fn write_commands(
    mut stdin: impl AsyncWrite + Unpin,
    commands: &mut mpsc::Receiver<AdapterCommand>,
    handle: ProviderHandle,
    generation: Generation,
    pending_replies: PendingReplies,
    pending_write: &mut Option<PendingStdinWrite>,
) -> Result<(), AdapterError> {
    while let Some(command) = commands.recv().await {
        match command {
            AdapterCommand::Write {
                write,
                reply,
                observed,
            } => {
                let receipt = pending_write.insert(PendingStdinWrite::new(reply));
                write_one_command(
                    &mut stdin,
                    write,
                    receipt,
                    observed,
                    WriterTimingDiagnostics {
                        handle: &handle,
                        generation,
                        enabled: stdin_timing_diagnostics_enabled(),
                        pending_replies: &pending_replies,
                        sample_timestamp: crate::clock::monotonic_ns,
                        emit_diagnostic: write_provider_diagnostic_best_effort,
                    },
                )
                .await?;
                *pending_write = None;
            }
            AdapterCommand::Ack { cursor, reply } => {
                if reply.send(CommandReceipt::Acknowledged(cursor)).is_err() {
                    return Err(AdapterError::ConsumerClosed);
                }
            }
        }
    }
    Err(AdapterError::ConsumerClosed)
}

struct WriterTimingDiagnostics<'a, Sample, Emit> {
    handle: &'a ProviderHandle,
    generation: Generation,
    enabled: bool,
    pending_replies: &'a PendingReplies,
    sample_timestamp: Sample,
    emit_diagnostic: Emit,
}

async fn write_one_command<Sample, Emit>(
    stdin: &mut (impl AsyncWrite + Unpin),
    write: PreparedWrite,
    receipt: &mut PendingStdinWrite,
    observed: tokio::sync::oneshot::Receiver<()>,
    timing: WriterTimingDiagnostics<'_, Sample, Emit>,
) -> Result<(), AdapterError>
where
    Sample: FnOnce() -> u128,
    Emit: FnOnce(&[u8]),
{
    let witnessed = witnessed_method(&write).filter(|_| timing.enabled);
    if let Some(method) = witnessed {
        // Register before the write: the reply can arrive before the flush returns.
        timing.pending_replies.register(&write, method);
    }
    let bytes = &write.bytes;
    while receipt.bytes_written < bytes.len() {
        let count = stdin
            .write(&bytes[receipt.bytes_written..])
            .await
            .map_err(process_error)?;
        if count == 0 {
            return Err(process_error(io::Error::from(io::ErrorKind::WriteZero)));
        }
        receipt.bytes_written += count;
    }
    stdin.flush().await.map_err(process_error)?;
    let stdin_flushed_at_ns = if witnessed.is_some() {
        Some((timing.sample_timestamp)())
    } else {
        None
    };
    let receipt_sent = receipt
        .reply
        .take()
        .ok_or(AdapterError::InvalidConfiguration)?
        .send(CommandReceipt::Written {
            remote_id: None,
            duplicate: false,
            written_at: Some(crate::clock::boundary_stamp()),
        });
    if let Some(stdin_flushed_at_ns) = stdin_flushed_at_ns
        && let Some(line) = stdin_timing_diagnostic_line(
            timing.handle,
            timing.generation,
            &write,
            stdin_flushed_at_ns,
        )
    {
        (timing.emit_diagnostic)(&line);
    }
    if receipt_sent.is_err() {
        return Err(AdapterError::ConsumerClosed);
    }
    observed.await.map_err(|_| AdapterError::ConsumerClosed)
}

#[derive(Default)]
struct ReaderProtocol {
    codec: ProviderCodec,
    reply_witness: Option<ReplyWitness>,
}

/// The initialize response's userAgent is the vendor's version identity. Keep
/// it verbatim rather than guessing how future CLI version strings are encoded.
#[derive(Serialize, Deserialize)]
struct UnknownMethodDiagnostic<'a> {
    provider: ProviderKind,
    #[serde(rename = "cliVersion", borrow)]
    cli_version: Option<&'a str>,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    variant: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unparsed: Option<bool>,
}

/// Maximum retained unknown diagnostic keys per session; unit: entries.
/// Bounds provider-controlled method/variant retention before diagnostics stop.
const MAX_UNKNOWN_DIAGNOSTIC_KEYS: usize = 256;

#[derive(Default)]
pub struct CodexMethodDiagnostics {
    user_agent: Option<String>,
    unknown: HashSet<(Option<String>, String, Option<String>)>,
    saturation_reported: bool,
}

impl CodexMethodDiagnostics {
    pub fn initialize(&mut self, user_agent: Option<&str>) {
        if self.user_agent.is_none() {
            self.user_agent = user_agent.map(str::to_owned);
        }
    }

    pub fn record(&mut self, provider: Option<ProviderKind>, frame: &Frame) {
        match provider {
            Some(ProviderKind::Codex) => {
                while let Some(line) = self.observe(frame) {
                    write_provider_diagnostic_best_effort(&line);
                }
            }
            Some(ProviderKind::Claude) | None => {}
        }
    }

    /// Observe a correlated reply without cloning its potentially large payload.
    pub fn record_reply(&mut self, provider: Option<ProviderKind>, body: &ResponseBody) {
        if provider == Some(ProviderKind::Codex) {
            while let Some(line) = self.observe_parts(None, Some(body)) {
                write_provider_diagnostic_best_effort(&line);
            }
        }
    }

    pub fn observe(&mut self, frame: &Frame) -> Option<Vec<u8>> {
        let body = match frame {
            Frame::Response { body, .. } => Some(body),
            Frame::KnownMethodUnparsed(_)
            | Frame::CodexClientRequest(_)
            | Frame::CodexServerRequest(_)
            | Frame::CodexNotification(_)
            | Frame::CodexClientNotification(_)
            | Frame::Request(_)
            | Frame::Notification(_)
            | Frame::UnknownMethod(_) => None,
        };
        self.observe_parts(Some(frame), body)
    }

    fn observe_parts(
        &mut self,
        frame: Option<&Frame>,
        body: Option<&ResponseBody>,
    ) -> Option<Vec<u8>> {
        if self.unknown.len() >= MAX_UNKNOWN_DIAGNOSTIC_KEYS {
            if self.saturation_reported {
                return None;
            }
            self.saturation_reported = true;
            return diagnostic_line(
                b"studio_provider_diagnostics_saturated ",
                &studio_json::json!({
                    "provider": ProviderKind::Codex,
                    "cliVersion": self.user_agent.as_deref(),
                    "maxEntries": MAX_UNKNOWN_DIAGNOSTIC_KEYS,
                }),
            );
        }
        if let Some(ResponseBody::CodexResult(
            studio_runtime_providers::codex::ClientRequestResult::Initialize(result),
        )) = body
        {
            self.initialize(Some(&result.user_agent));
        }
        let method = match body {
            Some(ResponseBody::CodexResult(result)) => Some(result.method().as_str()),
            Some(ResponseBody::CodexUnparsedResult { method, .. }) => Some(method.as_str()),
            Some(ResponseBody::CodexServerResult(result)) => Some(result.method().as_str()),
            _ => frame.and_then(Frame::method),
        };
        if matches!(frame, Some(Frame::KnownMethodUnparsed(_)))
            || matches!(body, Some(ResponseBody::CodexUnparsedResult { .. }))
        {
            let method = method?;
            if !self
                .unknown
                .insert((self.user_agent.clone(), method.to_owned(), None))
            {
                return None;
            }
            return diagnostic_line(
                b"studio_provider_unknown_method ",
                &UnknownMethodDiagnostic {
                    provider: ProviderKind::Codex,
                    cli_version: self.user_agent.as_deref(),
                    method,
                    variant: None,
                    unparsed: Some(true),
                },
            );
        }
        let mut nested = None;
        let mut visitor = |kind, variant: &str| {
            if nested.is_none()
                && let Some(method) = method
            {
                let variant = format!("{kind}:{variant}");
                if self.unknown.insert((
                    self.user_agent.clone(),
                    method.to_owned(),
                    Some(variant.clone()),
                )) {
                    nested = Some(variant);
                }
            }
        };
        if let Some(frame) = frame {
            frame.visit_unknown_variants(&mut visitor);
        } else {
            use studio_runtime_providers::codex::VisitUnknown;
            match body {
                Some(ResponseBody::CodexResult(result)) => result.visit_unknowns(&mut visitor),
                Some(ResponseBody::CodexServerResult(result)) => {
                    result.visit_unknowns(&mut visitor)
                }
                _ => {}
            }
        }
        if let Some(variant) = nested {
            return diagnostic_line(
                b"studio_provider_unknown_method ",
                &UnknownMethodDiagnostic {
                    provider: ProviderKind::Codex,
                    cli_version: self.user_agent.as_deref(),
                    method: method?,
                    variant: Some(&variant),
                    unparsed: None,
                },
            );
        }
        let incoming = frame?.codex_incoming_method()?;
        let CodexIncomingMethod::Unknown { method } = incoming else {
            return None;
        };
        if !self
            .unknown
            .insert((self.user_agent.clone(), method.clone(), None))
        {
            return None;
        }
        diagnostic_line(
            b"studio_provider_unknown_method ",
            &UnknownMethodDiagnostic {
                provider: ProviderKind::Codex,
                cli_version: self.user_agent.as_deref(),
                method: &method,
                variant: None,
                unparsed: None,
            },
        )
    }
}

async fn read_entries(
    stdout: impl AsyncRead + Unpin,
    entries: mpsc::Sender<JournalEntry>,
    clock: SessionClock,
    generation: Generation,
    frame_limit: usize,
    output_limit: Option<usize>,
    protocol: ReaderProtocol,
) -> Result<(), AdapterError> {
    let mut reader = BufReader::new(stdout);
    let mut total = 0_usize;
    let mut cursor = 0_u64;
    let mut line = Vec::new();
    loop {
        if !read_stdout_line(
            &mut reader,
            &mut line,
            &mut total,
            frame_limit,
            output_limit,
        )
        .await?
        {
            return Err(SupervisorError::ConnectionLost.into());
        }
        cursor += 1;
        let boundary_at = crate::clock::boundary_stamp();
        let decoded = studio_runtime_providers::decode_journal_with(
            cursor,
            generation.0,
            &line,
            protocol.codec.decode,
        );
        let payload = match decoded.payload {
            DecodedJournalPayload::Frame(frame) => {
                if let Some(line) = protocol
                    .reply_witness
                    .as_ref()
                    .and_then(|witness| witness.line(&frame, crate::clock::monotonic_ns))
                {
                    write_provider_diagnostic_best_effort(&line);
                }
                JournalPayload::Frame(frame)
            }
            DecodedJournalPayload::RawStdout { bytes, error } => {
                JournalPayload::RawStdout { bytes, error }
            }
        };
        entries
            .send(JournalEntry {
                received_at_ms: clock.now_ms(),
                boundary_at: Some(boundary_at),
                cursor: EventCursor(cursor),
                generation,
                payload,
            })
            .await
            .map_err(|_| AdapterError::ConsumerClosed)?;
    }
}

async fn read_stdout_line<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    line: &mut Vec<u8>,
    total: &mut usize,
    frame_limit: usize,
    output_limit: Option<usize>,
) -> Result<bool, AdapterError> {
    line.clear();
    let wire_limit = frame_limit
        .checked_add(studio_runtime_providers::MAX_FRAME_DELIMITER_BYTES)
        .ok_or(AdapterError::InvalidConfiguration)?;
    loop {
        let available = reader.fill_buf().await.map_err(process_error)?;
        if available.is_empty() {
            if studio_runtime_providers::frame_payload_bytes(line) > frame_limit {
                return Err(AdapterError::OutputLimit { limit: frame_limit });
            }
            return Ok(false);
        }
        let count = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |end| end + 1);
        *total = total
            .checked_add(count)
            .ok_or(AdapterError::InvalidConfiguration)?;
        let frame_len = line
            .len()
            .checked_add(count)
            .ok_or(AdapterError::InvalidConfiguration)?;
        if frame_len > wire_limit {
            return Err(AdapterError::OutputLimit { limit: frame_limit });
        }
        if let Some(limit) = output_limit.filter(|limit| *total > *limit) {
            return Err(AdapterError::OutputLimit { limit });
        }
        line.extend_from_slice(&available[..count]);
        reader.consume(count);
        let payload_len = if line.last() == Some(&b'\n') {
            studio_runtime_providers::frame_payload_bytes(line)
        } else {
            studio_runtime_providers::frame_prefix_payload_bytes(line)
        };
        if payload_len > frame_limit {
            return Err(AdapterError::OutputLimit { limit: frame_limit });
        }
        if line.last() == Some(&b'\n') {
            return Ok(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correlated_result_variants_stop_at_the_diagnostic_key_cap() {
        let mut diagnostics = CodexMethodDiagnostics::default();
        diagnostics.initialize(Some("fixture/1"));
        let mut warnings = 0;
        let mut saturation = 0;
        for index in 0..MAX_UNKNOWN_DIAGNOSTIC_KEYS + 16 {
            let raw = studio_json::OwnedRawJson::from_string(
                studio_json::json!({
                    "turn": {"id":"r", "status":"inProgress", "items":[{
                        "type":format!("futureItem{index}"), "id":"i"
                    }]}
                })
                .to_string(),
            )
            .unwrap();
            let body = ResponseBody::CodexResult(
                studio_runtime_providers::correlate_codex_result(Method::TurnStart, &raw).unwrap(),
            );
            while let Some(line) = diagnostics.observe_parts(None, Some(&body)) {
                if line.starts_with(b"studio_provider_diagnostics_saturated ") {
                    saturation += 1;
                    let value: studio_json::Value = studio_json::from_slice(
                        line.strip_prefix(b"studio_provider_diagnostics_saturated ")
                            .unwrap(),
                    )
                    .unwrap();
                    assert_eq!(value["maxEntries"], MAX_UNKNOWN_DIAGNOSTIC_KEYS);
                    assert_eq!(value["provider"], "codex");
                    assert_eq!(value["cliVersion"], "fixture/1");
                } else {
                    assert!(line.starts_with(b"studio_provider_unknown_method "));
                    warnings += 1;
                }
            }
            assert!(
                diagnostics.unknown.len() <= MAX_UNKNOWN_DIAGNOSTIC_KEYS,
                "distinct correlated result variants must not grow diagnostics past the cap"
            );
        }
        assert_eq!(diagnostics.unknown.len(), MAX_UNKNOWN_DIAGNOSTIC_KEYS);
        assert_eq!(warnings, MAX_UNKNOWN_DIAGNOSTIC_KEYS);
        assert_eq!(saturation, 1);
        let frame = studio_runtime_providers::decode_codex_frame(
            br#"{"method":"future/afterSaturation","params":{}}"#,
        )
        .unwrap();
        assert!(diagnostics.observe(&frame).is_none());
        assert_eq!(diagnostics.unknown.len(), MAX_UNKNOWN_DIAGNOSTIC_KEYS);
    }

    #[test]
    fn malformed_known_notification_stays_routable_and_is_diagnosed_once() {
        let wire = br#"{"method":"turn/completed","params":{"threadId":"t","turn":{"id":"r","status":"completed","error":null}}}"#;
        let frame = studio_runtime_providers::decode_codex_frame(wire).unwrap();
        assert!(matches!(frame, Frame::KnownMethodUnparsed(_)));
        assert_eq!(frame.method(), Some("turn/completed"));
        assert!(frame.id().is_none());
        assert_eq!(
            frame.method_and_params().unwrap().1["turn"]["status"],
            "completed"
        );
        let mut diagnostics = CodexMethodDiagnostics::default();
        diagnostics.initialize(Some("codex-cli/0.167.1"));
        let line = diagnostics
            .observe(&frame)
            .expect("incompatible payload diagnostic");
        let diagnostic: UnknownMethodDiagnostic<'_> = studio_json::from_slice(
            line.strip_prefix(b"studio_provider_unknown_method ")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(diagnostic.method, "turn/completed");
        assert_eq!(diagnostic.unparsed, Some(true));
        assert!(diagnostics.observe(&frame).is_none());
    }

    #[test]
    fn malformed_server_request_is_unparsed_and_cannot_use_typed_egress() {
        let wire = br#"{"id":7,"method":"item/commandExecution/requestApproval","params":{"threadId":"t","turnId":"r","itemId":7}}"#;
        let frame = studio_runtime_providers::decode_codex_frame(wire).unwrap();
        assert!(matches!(frame, Frame::KnownMethodUnparsed(_)));
        assert_eq!(frame.id(), Some(RequestId::Number(7)));
        assert!(studio_runtime_providers::encode_codex_line(&frame).is_err());
    }

    fn assert_future_payload_is_routed_and_diagnosed_once(
        wire: &[u8],
        method: &str,
        variant: &str,
    ) {
        let entry = studio_runtime_providers::decode_journal_with(
            1,
            1,
            wire,
            studio_runtime_providers::decode_codex_frame_with_metadata,
        );
        let DecodedJournalPayload::Frame(frame) = entry.payload else {
            panic!("known frame with future payload must route");
        };
        assert!(matches!(
            frame,
            Frame::CodexNotification(_) | Frame::CodexServerRequest(_)
        ));
        assert_eq!(frame.method(), Some(method));
        let mut diagnostics = CodexMethodDiagnostics::default();
        diagnostics.initialize(Some("codex-cli/0.167.1"));
        let line = diagnostics
            .observe(&frame)
            .expect("first unknown payload diagnostic");
        let diagnostic: UnknownMethodDiagnostic<'_> = studio_json::from_slice(
            line.strip_prefix(b"studio_provider_unknown_method ")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(diagnostic.method, method);
        assert_eq!(diagnostic.variant, Some(variant));
        assert!(diagnostics.observe(&frame).is_none());
    }

    #[test]
    fn future_thread_item_routes_and_is_diagnosed_once() {
        assert_future_payload_is_routed_and_diagnosed_once(
            br#"{"method":"item/started","params":{"threadId":"t","turnId":"r","startedAtMs":0,"item":{"type":"futureItem","id":"i"}}}"#,
            "item/started", "ThreadItem:futureItem",
        );
    }

    #[test]
    fn future_server_request_variant_routes_and_is_diagnosed_once() {
        assert_future_payload_is_routed_and_diagnosed_once(
            br#"{"id":7,"method":"mcpServer/elicitation/request","params":{"mode":"futureMode","message":"Confirm","serverName":"mcp","threadId":"t","turnId":null}}"#,
            "mcpServer/elicitation/request", "McpServerElicitationRequestParams:futureMode",
        );
    }

    #[test]
    fn unknown_codex_methods_log_once_per_initialize_version_and_method() {
        let decode = |value| {
            let decoded = studio_runtime_providers::decode_journal_value(1, 1, &value);
            let DecodedJournalPayload::Frame(frame) = decoded.payload else {
                panic!("frame")
            };
            frame
        };
        let unknown = decode(
            studio_json::json!({"method":"future/notification","params":{"secret":"must not log"}}),
        );
        let mut diagnostics = CodexMethodDiagnostics::default();
        let before = diagnostics.observe(&unknown).expect("first unknown");
        fn parse(line: &[u8]) -> UnknownMethodDiagnostic<'_> {
            studio_json::from_slice(
                line.strip_prefix(b"studio_provider_unknown_method ")
                    .unwrap(),
            )
            .unwrap()
        }
        assert_eq!(parse(&before).cli_version, None);
        assert!(!String::from_utf8(before).unwrap().contains("must not log"));
        assert!(diagnostics.observe(&unknown).is_none());
        let initialized = studio_runtime_providers::decode_codex_frame(br#"{"id":1,"result":{"codexHome":"/codex-home","platformFamily":"unix","platformOs":"test","userAgent":"codex-cli/0.167.1"}}"#).unwrap();
        let Frame::Response {
            body: ResponseBody::Uncorrelated(raw),
            ..
        } = initialized
        else {
            panic!("raw initialize")
        };
        let body = ResponseBody::CodexResult(
            studio_runtime_providers::correlate_codex_result(Method::Initialize, &raw).unwrap(),
        );
        diagnostics.record_reply(Some(ProviderKind::Codex), &body);
        let after = diagnostics.observe(&unknown).expect("new version key");
        assert_eq!(parse(&after).cli_version, Some("codex-cli/0.167.1"));
        assert!(diagnostics.observe(&unknown).is_none());
        let known = decode(
            studio_json::json!({"method":"thread/name/updated","params":{"threadId":"t","threadName":"n"}}),
        );
        assert!(diagnostics.observe(&known).is_none());
        let other = decode(studio_json::json!({"method":"future/other"}));
        assert!(diagnostics.observe(&other).is_some());
    }

    #[tokio::test]
    async fn codex_ingress_preserves_known_unknown_and_malformed_frames() {
        let (entries, mut journal) = mpsc::channel(LANE_CAPACITY);
        let input = b"{\"method\":\"thread/name/updated\",\"params\":{\"threadId\":\"t\",\"threadName\":\"n\"}}\n{\"method\":\"future/notification\",\"params\":{\"opaque\":1}}\nnot-json\n";
        let result = read_entries(
            &input[..],
            entries,
            SessionClock::default(),
            Generation(1),
            4096,
            None,
            ReaderProtocol {
                codec: ProviderCodec::codex(),
                reply_witness: None,
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(AdapterError::Supervisor(SupervisorError::ConnectionLost))
        ));
        let first = journal.recv().await.unwrap();
        let JournalPayload::Frame(frame) = first.payload else {
            panic!("known frame")
        };
        assert!(matches!(
            frame.codex_incoming_method(),
            Some(CodexIncomingMethod::Notification(_))
        ));
        let second = journal.recv().await.unwrap();
        let JournalPayload::Frame(frame) = second.payload else {
            panic!("unknown frame")
        };
        assert!(matches!(
            frame.codex_incoming_method(),
            Some(CodexIncomingMethod::Unknown { .. })
        ));
        assert!(matches!(
            journal.recv().await.unwrap().payload,
            JournalPayload::RawStdout { .. }
        ));
    }

    #[tokio::test]
    async fn interrupted_stdin_write_counts_accepted_bytes_and_never_reports_them_not_sent() {
        for accepted_bytes in [0, 1] {
            let (mut stdin, _child) = tokio::io::duplex(1);
            if accepted_bytes == 0 {
                stdin
                    .write_all(b"x")
                    .await
                    .expect("fill stdin before this request");
            }
            let (reply, receipt) = tokio::sync::oneshot::channel();
            let (_observation, observed) = tokio::sync::oneshot::channel();
            let mut pending = Some(PendingStdinWrite::new(reply));
            let handle = ProviderHandle::for_account("accepted-prefix");
            let replies = PendingReplies::default();
            let mut writer = Box::pin(write_one_command(
                &mut stdin,
                turn_start_write(),
                pending.as_mut().expect("owned receipt"),
                observed,
                WriterTimingDiagnostics {
                    handle: &handle,
                    generation: Generation(1),
                    enabled: false,
                    pending_replies: &replies,
                    sample_timestamp: || 0,
                    emit_diagnostic: |_: &[u8]| {},
                },
            ));
            assert!(
                std::future::poll_fn(|cx| Poll::Ready(writer.as_mut().poll(cx).is_pending())).await
            );
            drop(writer);
            interrupt_stdin_write(&mut pending);
            assert!(
                matches!(receipt.await.expect("explicit interrupted receipt"),
                    CommandReceipt::WriteInterrupted { bytes_written } if bytes_written == accepted_bytes
                )
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn stdout_eof_rejects_queued_writes_and_closes_admission_before_cleanup() {
        let (commands, mut inbox) = mpsc::channel(LANE_CAPACITY);
        let (command, receipt) = AdapterCommand::write(turn_start_write());
        let shutdown = Shutdown::default();
        // Queue the command in the reader's terminal poll. If command service
        // was polled first, it saw an empty inbox; either select order must now
        // choose EOF with a ready, admitted write that has never reached stdin.
        let reader = async {
            commands.try_send(command).expect("admitted at EOF");
            Err(SupervisorError::ConnectionLost.into())
        };
        let result =
            drain_stdout_before_cleanup(reader, &mut inbox, &shutdown.subscribe(), None).await;
        assert!(matches!(
            result,
            Err(AdapterError::Supervisor(SupervisorError::ConnectionLost))
        ));
        assert!(
            inbox.is_closed(),
            "drain must close admission before cleanup"
        );
        assert!(matches!(receipt.receive().await, CommandReceipt::NotSent));
        assert!(matches!(
            commands.try_send(AdapterCommand::ack(EventCursor(1)).0),
            Err(mpsc::error::TrySendError::Closed(_))
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn stdout_exit_drain_bounds_a_full_journal_and_preserves_the_exchange_deadline() {
        for has_deadline in [false, true] {
            let deadline = has_deadline.then(|| Instant::now() + REAP_GRACE / 2);
            let (entries, mut journal) = mpsc::channel(1);
            let shutdown = Shutdown::default();
            let (_commands, mut inbox) = mpsc::channel(LANE_CAPACITY);
            let started = Instant::now();
            let result = drain_stdout_before_cleanup(
                read_entries(
                    &b"first\nsecond\n"[..],
                    entries,
                    SessionClock::default(),
                    Generation(1),
                    1024,
                    None,
                    ReaderProtocol::default(),
                ),
                &mut inbox,
                &shutdown.subscribe(),
                deadline,
            )
            .await;
            if let Some(deadline) = deadline {
                assert!(matches!(result, Err(AdapterError::Timeout)));
                assert_eq!(Instant::now(), deadline);
            } else {
                assert!(matches!(
                    result,
                    Err(AdapterError::Supervisor(SupervisorError::ConnectionLost))
                ));
                assert_eq!(Instant::now() - started, REAP_GRACE);
            }
            let retained = journal.recv().await.expect("first entry retained");
            assert_eq!(retained.cursor, EventCursor(1));
            assert!(journal.recv().await.is_none());
        }
    }

    /// Shutdown keeps the drain going within the same bound: a line written
    /// after the request still arrives, and the result stays successful.
    #[tokio::test(start_paused = true)]
    async fn stdout_exit_drain_bounds_an_inherited_pipe_and_continues_after_shutdown() {
        for cancel in [false, true] {
            let (mut writer, stdout) = tokio::io::duplex(1024);
            writer.write_all(b"final\n").await.expect("final line");
            // Keep the writer alive, like a descendant inheriting stdout.
            let (entries, mut journal) = mpsc::channel(1);
            let shutdown = Shutdown::default();
            let (_commands, mut inbox) = mpsc::channel(LANE_CAPACITY);
            let stop = shutdown.subscribe();
            let started = Instant::now();
            let (result, ()) = tokio::join!(
                drain_stdout_before_cleanup(
                    read_entries(
                        stdout,
                        entries,
                        SessionClock::default(),
                        Generation(1),
                        1024,
                        None,
                        ReaderProtocol::default(),
                    ),
                    &mut inbox,
                    &stop,
                    None,
                ),
                async {
                    let entry = journal.recv().await.expect("final line drained");
                    assert_eq!(entry.cursor, EventCursor(1));
                    if cancel {
                        shutdown.cancel();
                        writer.write_all(b"after\n").await.expect("late line");
                        let entry = journal.recv().await.expect("late line drained");
                        assert_eq!(entry.cursor, EventCursor(2));
                    }
                },
            );
            if cancel {
                assert!(result.is_ok());
            } else {
                assert!(matches!(
                    result,
                    Err(AdapterError::Supervisor(SupervisorError::ConnectionLost))
                ));
            }
            assert_eq!(Instant::now() - started, REAP_GRACE);
            assert!(journal.recv().await.is_none());
            drop(writer);
        }
    }

    /// A group signal after the leader is reaped could reach another group that
    /// reuses the ID. Here the leader exits while a descendant keeps stdout open,
    /// so the exit, not stdout EOF, ends the exchange.
    #[tokio::test]
    async fn cleanup_signals_the_group_before_it_reaps_the_exited_leader() {
        let mut attachment = StdioProviderTaskFactory {
            deadline: Some(Instant::now() + std::time::Duration::from_secs(30)),
            max_frame_bytes: 1024,
            max_output_bytes: Some(4096),
            stderr_log: None,
        }
        .open(
            ProviderHandle::for_account("stdio-group-signal"),
            OpenOptions {
                clock: SessionClock::default(),
                expected_generation: Some(Generation(1)),
                launch: ProviderLaunchSpec::new(
                    vec!["/bin/sh".into(), "-c".into(), "sleep 30 & exit 0".into()],
                    std::collections::BTreeMap::new(),
                    std::env::temp_dir(),
                ),
            },
            Shutdown::default(),
        )
        .await
        .expect("child starts");
        let pid = attachment
            .status
            .borrow()
            .info
            .as_ref()
            .expect("child info")
            .pid;
        let exit = attachment.completion().await;
        assert!(exit.cleanup_error.is_none(), "{:?}", exit.cleanup_error);
        assert_eq!(
            LEADER_UNREAPED_AT_SIGNAL
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&pid)
                .copied(),
            Some(true),
            "the group signal comes before the leader is reaped"
        );
    }

    /// A requested shutdown closes stdin and keeps reading stdout before it kills
    /// the group. The Claude bridge writes its final frames, such as a
    /// `turn/completed`, while it exits on stdin EOF; an immediate kill lost them
    /// and left a finished turn stuck as disconnected.
    #[tokio::test]
    async fn shutdown_delivers_frames_written_on_stdin_eof_before_the_kill() {
        let root = tempfile::tempdir().expect("owned child directory");
        let mut attachment = StdioProviderTaskFactory {
            deadline: None,
            max_frame_bytes: 1024,
            max_output_bytes: None,
            stderr_log: None,
        }
        .open(
            ProviderHandle::for_account("stdio-shutdown-drain"),
            OpenOptions {
                clock: SessionClock::default(),
                expected_generation: Some(Generation(1)),
                launch: ProviderLaunchSpec::new(
                    vec![
                        "/bin/sh".into(),
                        "-c".into(),
                        "cat >/dev/null; printf '%s\\n' \
                         '{\"jsonrpc\":\"2.0\",\"method\":\"turn/completed\",\"params\":{}}'"
                            .into(),
                    ],
                    std::collections::BTreeMap::new(),
                    root.path().to_path_buf(),
                ),
            },
            Shutdown::default(),
        )
        .await
        .expect("child starts");
        let exit = attachment.shutdown().await;
        assert!(exit.result.is_ok(), "{:?}", exit.result);
        assert!(exit.cleanup_error.is_none(), "{:?}", exit.cleanup_error);
        let entry = attachment
            .journal
            .try_recv()
            .expect("the frame written after stdin EOF is journaled");
        assert_eq!(entry.cursor, EventCursor(1));
        assert!(attachment.journal.try_recv().is_err());
    }

    /// Darwin answers EPERM to a kill of an exiting leader, and the group can
    /// still answer EPERM for a moment after the reap. Cleanup must wait for the
    /// group to vanish instead of reporting the first refusal.
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn refused_group_signal_is_forgiven_once_the_reaped_group_vanishes() {
        let attachment = StdioProviderTaskFactory {
            deadline: None,
            max_frame_bytes: 1024,
            max_output_bytes: None,
            stderr_log: None,
        }
        .open(
            ProviderHandle::for_account("stdio-darwin-race"),
            OpenOptions {
                clock: SessionClock::default(),
                expected_generation: Some(Generation(1)),
                launch: ProviderLaunchSpec::new(
                    vec!["/bin/sh".into(), "-c".into(), "cat >/dev/null".into()],
                    std::collections::BTreeMap::new(),
                    std::env::temp_dir(),
                ),
            },
            Shutdown::default(),
        )
        .await
        .expect("child starts");
        let pid = attachment
            .status
            .borrow()
            .info
            .as_ref()
            .expect("child info")
            .pid;
        crate::owned_process_group::TEST_DARWIN_RACE
            .lock()
            .expect("race table")
            .insert(
                pid,
                std::time::Instant::now() + std::time::Duration::from_millis(300),
            );
        let mut attachment = attachment;
        let exit = attachment.shutdown().await;
        assert!(exit.result.is_ok(), "{:?}", exit.result);
        assert!(exit.cleanup_error.is_none(), "{:?}", exit.cleanup_error);
    }
    use std::{
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        task::{Context, Poll},
    };
    use tokio::io::AsyncWrite;

    struct FlushWitness {
        flushed: Arc<AtomicBool>,
        fail_write: bool,
    }

    impl AsyncWrite for FlushWitness {
        fn poll_write(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<io::Result<usize>> {
            if self.fail_write {
                Poll::Ready(Err(io::Error::other("injected write failure")))
            } else {
                Poll::Ready(Ok(bytes.len()))
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            self.flushed.store(true, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    fn turn_start_write() -> PreparedWrite {
        let decoded = decode_journal_entry_borrowed(
            1,
            7,
            br#"{"jsonrpc":"2.0","id":"request-1","method":"turn/start","params":{}}"#,
        );
        let DecodedJournalPayload::Frame(frame) = decoded.payload else {
            panic!("test request decodes as a frame");
        };
        PreparedWrite::new(OperationId("attempt-op".into()), &frame).expect("test request prepares")
    }

    #[test]
    fn provider_stdin_timing_line_matches_shared_golden_contract() {
        let decoded = decode_journal_entry_borrowed(
            1,
            7,
            br#"{"jsonrpc":"2.0","id":"request-1","method":"turn/start","params":{}}"#,
        );
        let DecodedJournalPayload::Frame(frame) = decoded.payload else {
            panic!("golden request decodes as a frame");
        };
        let write =
            PreparedWrite::new(OperationId("attempt-op".into()), &frame).expect("request prepares");
        let line = stdin_timing_diagnostic_line(
            &ProviderHandle::for_account("default"),
            Generation(7),
            &write,
            1_234_567_890,
        )
        .expect("TurnStart produces timing evidence");
        assert_eq!(
            line,
            include_str!("../tests/fixtures/provider-stdin-timing-diagnostic.log").as_bytes()
        );
    }

    fn request_write(method: &str, id: &str) -> PreparedWrite {
        let line = format!(r#"{{"jsonrpc":"2.0","id":"{id}","method":"{method}","params":{{}}}}"#);
        let DecodedJournalPayload::Frame(frame) =
            decode_journal_entry_borrowed(1, 7, line.as_bytes()).payload
        else {
            panic!("test request decodes as a frame");
        };
        PreparedWrite::new(OperationId(format!("op-{id}")), &frame).expect("request prepares")
    }

    fn response(id: &str, body: &str) -> Frame {
        let line = format!(r#"{{"jsonrpc":"2.0","id":"{id}",{body}}}"#);
        let DecodedJournalPayload::Frame(frame) =
            decode_journal_entry_borrowed(1, 7, line.as_bytes()).payload
        else {
            panic!("test response decodes as a frame");
        };
        frame
    }

    async fn write_with(
        write: PreparedWrite,
        enabled: bool,
        pending_replies: &PendingReplies,
    ) -> Vec<Vec<u8>> {
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&emitted);
        let mut stdin = FlushWitness {
            flushed: Arc::new(AtomicBool::new(false)),
            fail_write: false,
        };
        let (reply, _receiver) = tokio::sync::oneshot::channel();
        let (observation, observed) = tokio::sync::oneshot::channel();
        observation.send(()).expect("observation is delivered");
        write_one_command(
            &mut stdin,
            write,
            &mut PendingStdinWrite::new(reply),
            observed,
            WriterTimingDiagnostics {
                handle: &ProviderHandle::for_account("default"),
                generation: Generation(7),
                enabled,
                pending_replies,
                sample_timestamp: || 100,
                emit_diagnostic: move |line: &[u8]| {
                    sink.lock().expect("sink").push(line.to_vec());
                },
            },
        )
        .await
        .expect("write succeeds");
        emitted.lock().expect("sink").clone()
    }

    #[tokio::test]
    async fn prepared_write_encodes_once_through_the_actual_stdio_writer() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static ENCODINGS: AtomicUsize = AtomicUsize::new(0);
        fn encode(frame: &Frame) -> Result<Vec<u8>, studio_runtime_providers::CodecError> {
            ENCODINGS.fetch_add(1, Ordering::SeqCst);
            studio_runtime_providers::encode_line(frame)
        }
        let frame = request_write("turn/start", "encode-once").frame().clone();
        let write = PreparedWrite::new_with_codec(
            OperationId("encode-once".into()),
            &frame,
            ProviderCodec {
                encode,
                ..ProviderCodec::default()
            },
        )
        .expect("preflight");
        assert_eq!(ENCODINGS.load(Ordering::SeqCst), 1);
        write_with(write, false, &PendingReplies::default()).await;
        assert_eq!(
            ENCODINGS.load(Ordering::SeqCst),
            1,
            "the writer must reuse preflight bytes"
        );
    }

    #[tokio::test]
    async fn thread_start_and_resume_writes_and_replies_are_journaled_under_the_flag() {
        let pending = PendingReplies::default();
        let witness = ReplyWitness {
            handle: ProviderHandle::for_account("default"),
            generation: Generation(7),
            pending: pending.clone(),
        };
        for (method, id) in [("thread/start", "r1"), ("thread/resume", "r2")] {
            let lines = write_with(request_write(method, id), true, &pending).await;
            assert_eq!(
                lines,
                vec![
                    format!(
                        "studio_provider_stdin_write {{\"providerHandle\":\"account:default\",\"generation\":7,\"operationId\":\"op-{id}\",\"requestId\":\"{id}\",\"method\":\"{method}\",\"stdinFlushedAtNs\":100}}\n"
                    )
                    .into_bytes()
                ]
            );
        }
        let reply = witness
            .line(&response("r1", r#""result":{}"#), || 250)
            .expect("thread/start reply is journaled");
        assert_eq!(
            String::from_utf8(reply).expect("utf8"),
            "studio_provider_reply {\"providerHandle\":\"account:default\",\"generation\":7,\"operationId\":\"op-r1\",\"requestId\":\"r1\",\"method\":\"thread/start\",\"replyAtNs\":250,\"outcome\":\"result\"}\n"
        );
        let error = witness
            .line(
                &response("r2", r#""error":{"code":-1,"message":"no"}"#),
                || 300,
            )
            .expect("thread/resume error reply is journaled");
        assert!(
            String::from_utf8(error)
                .expect("utf8")
                .contains(r#""method":"thread/resume","replyAtNs":300,"outcome":"error""#)
        );
        // Each request is journaled once; unknown ids and non-responses are ignored.
        assert!(
            witness
                .line(&response("r1", r#""result":{}"#), || 1)
                .is_none()
        );
        assert!(
            witness
                .line(&response("other", r#""result":{}"#), || 1)
                .is_none()
        );
    }

    #[tokio::test]
    async fn thread_methods_are_not_journaled_or_registered_when_the_flag_is_off() {
        let pending = PendingReplies::default();
        for method in ["thread/start", "thread/resume", "turn/start"] {
            assert!(
                write_with(request_write(method, "r1"), false, &pending)
                    .await
                    .is_empty()
            );
        }
        assert!(pending.take(&RequestId::String("r1".into())).is_none());
        // Methods outside the witnessed set stay silent with the flag on.
        assert!(
            write_with(request_write("thread/read", "r3"), true, &pending)
                .await
                .is_empty()
        );
        assert!(pending.take(&RequestId::String("r3".into())).is_none());
    }

    #[tokio::test]
    async fn prepared_diagnostic_requests_preserve_valid_and_malformed_ids() {
        fn prepared(id: Option<studio_json::Value>) -> PreparedWrite {
            let mut fields = studio_json::Map::new();
            fields.insert("method".into(), studio_json::json!("turn/start"));
            if let Some(id) = id {
                fields.insert("id".into(), id);
            }
            let frame = Frame::UnknownMethod(studio_runtime_providers::UnknownMethodFrame {
                method: "turn/start".into(),
                fields,
            });
            PreparedWrite::new(OperationId("fixture".into()), &frame).unwrap()
        }
        for (id, expected) in [
            (studio_json::json!(0), RequestId::Number(0)),
            (studio_json::json!(u64::MAX), RequestId::Number(u64::MAX)),
            (
                studio_json::json!("string-id"),
                RequestId::String("string-id".into()),
            ),
        ] {
            let pending = PendingReplies::default();
            let lines = write_with(prepared(Some(id.clone())), true, &pending).await;
            assert_eq!(lines.len(), 1);
            let json = std::str::from_utf8(&lines[0])
                .unwrap()
                .strip_prefix(STDIN_TIMING_DIAGNOSTIC_PREFIX)
                .unwrap();
            let diagnostic: studio_json::Value = studio_json::from_str(json).unwrap();
            assert_eq!(diagnostic["requestId"], id);
            assert_eq!(
                pending.take(&expected),
                Some((OperationId("fixture".into()), Method::TurnStart))
            );
        }
        for id in [
            None,
            Some(studio_json::Value::Null),
            Some(studio_json::json!(false)),
            Some(studio_json::json!(-1)),
            Some(studio_json::json!(1.5)),
            Some(studio_json::json!({"id":1})),
        ] {
            let pending = PendingReplies::default();
            assert!(write_with(prepared(id), true, &pending).await.is_empty());
            assert!(pending.0.lock().unwrap().is_empty());
        }
        let pending = PendingReplies::default();
        let (mut writer, mut reader) = tokio::io::duplex(1024);
        let (reply, receiver) = tokio::sync::oneshot::channel();
        let (observation, observed) = tokio::sync::oneshot::channel();
        observation.send(()).unwrap();
        write_one_command(
            &mut writer,
            prepared(Some(studio_json::json!(7))),
            &mut PendingStdinWrite::new(reply),
            observed,
            WriterTimingDiagnostics {
                handle: &ProviderHandle::for_account("default"),
                generation: Generation(7),
                enabled: true,
                pending_replies: &pending,
                sample_timestamp: || 100,
                emit_diagnostic: |_: &[u8]| {},
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            receiver.await.unwrap(),
            CommandReceipt::Written { .. }
        ));
        let expected = b"{\"id\":7,\"method\":\"turn/start\"}\n";
        let mut bytes = vec![0; expected.len()];
        tokio::io::AsyncReadExt::read_exact(&mut reader, &mut bytes)
            .await
            .unwrap();
        assert_eq!(&bytes, expected);
        assert!(pending.take(&RequestId::Number(7)).is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn failed_reap_reports_the_unconfirmed_child() {
        let error = super::reap_with_deadline::<()>(Some(12345), std::future::pending())
            .await
            .expect_err("bounded reap");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(error.to_string().contains("12345"));
        assert!(error.to_string().contains("termination is unconfirmed"));
    }

    #[tokio::test]
    async fn stdin_timing_diagnostic_follows_flush_even_if_receipt_receiver_closed() {
        let flushed = Arc::new(AtomicBool::new(false));
        let samples = Arc::new(AtomicUsize::new(0));
        let emitted = Arc::new(AtomicUsize::new(0));
        let mut stdin = FlushWitness {
            flushed: Arc::clone(&flushed),
            fail_write: false,
        };
        let (reply, receiver) = tokio::sync::oneshot::channel();
        drop(receiver);
        let (_observation, observed) = tokio::sync::oneshot::channel();
        let sample_flushed = Arc::clone(&flushed);
        let sample_count = Arc::clone(&samples);
        let emit_flushed = Arc::clone(&flushed);
        let emitted_count = Arc::clone(&emitted);

        let result = write_one_command(
            &mut stdin,
            turn_start_write(),
            &mut PendingStdinWrite::new(reply),
            observed,
            WriterTimingDiagnostics {
                handle: &ProviderHandle::for_account("default"),
                generation: Generation(7),
                enabled: true,
                pending_replies: &PendingReplies::default(),
                sample_timestamp: move || {
                    assert!(sample_flushed.load(Ordering::SeqCst));
                    sample_count.fetch_add(1, Ordering::SeqCst);
                    123
                },
                emit_diagnostic: move |line: &[u8]| {
                    assert!(emit_flushed.load(Ordering::SeqCst));
                    assert!(line.starts_with(STDIN_TIMING_DIAGNOSTIC_PREFIX.as_bytes()));
                    emitted_count.fetch_add(1, Ordering::SeqCst);
                },
            },
        )
        .await;

        assert!(matches!(result, Err(AdapterError::ConsumerClosed)));
        assert_eq!(samples.load(Ordering::SeqCst), 1);
        assert_eq!(emitted.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn failed_stdin_write_does_not_sample_or_emit_timing() {
        let flushed = Arc::new(AtomicBool::new(false));
        let samples = Arc::new(AtomicUsize::new(0));
        let emitted = Arc::new(AtomicUsize::new(0));
        let mut stdin = FlushWitness {
            flushed: Arc::clone(&flushed),
            fail_write: true,
        };
        let (reply, _receiver) = tokio::sync::oneshot::channel();
        let (_observation, observed) = tokio::sync::oneshot::channel();
        let sample_count = Arc::clone(&samples);
        let emitted_count = Arc::clone(&emitted);
        let pending = PendingReplies::default();

        let result = write_one_command(
            &mut stdin,
            turn_start_write(),
            &mut PendingStdinWrite::new(reply),
            observed,
            WriterTimingDiagnostics {
                handle: &ProviderHandle::for_account("default"),
                generation: Generation(7),
                enabled: true,
                pending_replies: &pending,
                sample_timestamp: move || {
                    sample_count.fetch_add(1, Ordering::SeqCst);
                    123
                },
                emit_diagnostic: move |_: &[u8]| {
                    emitted_count.fetch_add(1, Ordering::SeqCst);
                },
            },
        )
        .await;

        assert!(matches!(
            result,
            Err(AdapterError::Supervisor(SupervisorError::Io(_)))
        ));
        assert_eq!(samples.load(Ordering::SeqCst), 0);
        assert_eq!(emitted.load(Ordering::SeqCst), 0);
        assert!(!flushed.load(Ordering::SeqCst));
        assert_eq!(
            pending.take(&RequestId::String("request-1".into())),
            Some((OperationId("attempt-op".into()), Method::TurnStart))
        );
    }

    #[tokio::test]
    async fn stdout_line_buffer_reuses_its_allocation_between_frames() {
        let mut reader = BufReader::new(&b"first\nsecond\n"[..]);
        let mut line = Vec::new();
        let mut total = 0;

        assert!(
            read_stdout_line(&mut reader, &mut line, &mut total, 16, Some(32))
                .await
                .expect("first bounded frame")
        );
        let allocation = (line.as_ptr(), line.capacity());
        assert_eq!(line, b"first\n");

        assert!(
            read_stdout_line(&mut reader, &mut line, &mut total, 16, Some(32))
                .await
                .expect("second bounded frame")
        );
        assert_eq!(line, b"second\n");
        assert_eq!((line.as_ptr(), line.capacity()), allocation);
        assert_eq!(total, b"first\nsecond\n".len());
    }

    #[tokio::test]
    async fn stdout_frame_limit_handles_lf_crlf_and_oversized_payloads() {
        let frame_limit = 32;
        let mut lf_input = vec![b'x'; frame_limit];
        lf_input.push(b'\n');
        let mut lf_reader = BufReader::new(lf_input.as_slice());
        let mut line = Vec::new();
        let mut total = 0;

        assert!(
            read_stdout_line(&mut lf_reader, &mut line, &mut total, frame_limit, None,)
                .await
                .expect("limit-sized LF frame")
        );
        assert_eq!(
            studio_runtime_providers::frame_payload_bytes(&line),
            frame_limit
        );
        assert_eq!(line.len(), frame_limit + 1);
        assert_eq!(line.last(), Some(&b'\n'));
        assert_eq!(total, frame_limit + 1);

        let mut crlf_input = vec![b'x'; frame_limit];
        crlf_input.extend_from_slice(b"\r\n");
        // Place CR at the end of the first BufReader fill to exercise CRLF split across reads.
        let mut crlf_reader = BufReader::with_capacity(frame_limit + 1, crlf_input.as_slice());
        let mut line = Vec::new();
        let mut total = 0;
        assert!(
            read_stdout_line(&mut crlf_reader, &mut line, &mut total, frame_limit, None,)
                .await
                .expect("limit-sized split CRLF frame")
        );
        assert_eq!(
            studio_runtime_providers::frame_payload_bytes(&line),
            frame_limit
        );
        assert_eq!(line.len(), frame_limit + 2);
        assert_eq!(total, frame_limit + 2);

        let mut oversized = vec![b'x'; frame_limit + 1];
        oversized.extend_from_slice(b"\r\n");
        let mut reader = BufReader::with_capacity(frame_limit + 2, oversized.as_slice());
        let mut line = Vec::new();
        let mut total = 0;
        assert!(matches!(
            read_stdout_line(&mut reader, &mut line, &mut total, frame_limit, None).await,
            Err(AdapterError::OutputLimit { limit }) if limit == frame_limit
        ));

        assert_eq!(total, frame_limit + 2);

        let mut unterminated = vec![b'x'; frame_limit];
        unterminated.push(b'\r');
        let mut reader = BufReader::new(unterminated.as_slice());
        let mut line = Vec::new();
        let mut total = 0;
        assert!(matches!(
            read_stdout_line(&mut reader, &mut line, &mut total, frame_limit, None).await,
            Err(AdapterError::OutputLimit { limit }) if limit == frame_limit
        ));
        assert_eq!(total, frame_limit + 1);
    }

    #[tokio::test(start_paused = true)]
    async fn raw_unknown_output_counts_toward_the_total_limit() {
        let (entries, mut journal) = mpsc::channel(LANE_CAPACITY);
        let input = b"not-json\nnot-json\nnot-json\n";
        assert!(matches!(
            read_entries(
                &input[..],
                entries,
                SessionClock::default(),
                Generation(1),
                16,
                Some(20),
                ReaderProtocol::default()
            )
            .await,
            Err(AdapterError::OutputLimit { limit: 20 })
        ));
        for cursor in [1, 2] {
            let entry = journal.recv().await.expect("bounded raw entry");
            assert_eq!(entry.cursor, EventCursor(cursor));
            assert!(matches!(entry.payload, JournalPayload::RawStdout { .. }));
        }
        assert!(journal.recv().await.is_none());
    }
    #[tokio::test(start_paused = true)]
    async fn raw_frame_is_bounded_before_decoding() {
        let (entries, mut journal) = mpsc::channel(LANE_CAPACITY);
        assert!(matches!(
            read_entries(
                &b"123456789"[..],
                entries,
                SessionClock::default(),
                Generation(1),
                8,
                Some(100),
                ReaderProtocol::default()
            )
            .await,
            Err(AdapterError::OutputLimit { limit: 8 })
        ));
        assert!(journal.recv().await.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn persistent_session_output_is_bounded_per_frame_only() {
        let (entries, mut journal) = mpsc::channel(LANE_CAPACITY);
        let input = b"not-json\nnot-json\nnot-json\n";
        assert!(matches!(
            read_entries(
                &input[..],
                entries,
                SessionClock::default(),
                Generation(1),
                16,
                None,
                ReaderProtocol::default()
            )
            .await,
            Err(AdapterError::Supervisor(SupervisorError::ConnectionLost))
        ));
        for cursor in [1, 2, 3] {
            let entry = journal.recv().await.expect("unbounded total entry");
            assert_eq!(entry.cursor, EventCursor(cursor));
        }
    }
}
