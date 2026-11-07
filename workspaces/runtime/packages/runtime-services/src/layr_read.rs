//! Read-only Layr operations for native reviewers.

use std::sync::OnceLock;

use studio_json::{Value, json};

use crate::managed_host_tools::{Actor, DispatchedOperation, OperationResult};

/// The tool name.
pub(crate) const TOOL: &str = "layr_read";
/// Largest argument count of one call.
const MAX_ARGUMENTS: usize = 64;
/// Longest argument, in characters.
const MAX_ARGUMENT_CHARS: usize = 4096;
/// Request ID prefix of one exact tool request.
const REQUEST_ID_PREFIX: &str = "layr-read:";
const REFUSAL: &str = "Supply a read-only Layr command or try <state> -- <command>";

/// The read-only tool definition.
pub(crate) fn tool_definition() -> Option<&'static crate::thread_prepare::ToolDefinition> {
    static DEFINITION: OnceLock<crate::thread_prepare::ToolDefinition> = OnceLock::new();
    Some(DEFINITION.get_or_init(|| crate::thread_prepare::ToolDefinition {
        name: TOOL.into(),
        description: "Read Layr history with show, diff, log, blame, grep, ls-files, merge-base, rev-parse and status; inspect changes with change list/show/log/diff, attestations and view show. An optional --line <line> prefix selects a line for reads. Use try <state> -- <command> for a check without network in a temporary try line; Layr records the check result as an attestation. Existing lines, states, refs, changes and queues remain unchanged. The check may write only its temporary copy and $TMPDIR.".into(),
        input_schema: studio_json::Map::from_iter([
            ("type".into(), json!("object")),
            ("properties".into(), json!({"args": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": MAX_ARGUMENTS}})),
            ("required".into(), json!(["args"])),
        ]),
        extra: studio_json::Map::from_iter([("type".into(), json!("function"))]),
    }))
}

/// Offer the tool only to Layr state reviewers.
pub(crate) fn tool_definition_for(
    assignment: &studio_runtime_schema::review::ReviewAssignment,
) -> Option<&'static crate::thread_prepare::ToolDefinition> {
    match assignment.target() {
        studio_runtime_schema::review::ReviewTarget::LayrState { .. } => tool_definition(),
        studio_runtime_schema::review::ReviewTarget::GitRange { .. } => None,
    }
}

/// A validated read request. Read arguments are passed through to Layr.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LayrReadCommand {
    Show(Vec<String>),
    Diff(Vec<String>),
    Log(Vec<String>),
    Blame(Vec<String>),
    Grep(Vec<String>),
    LsFiles(Vec<String>),
    MergeBase(Vec<String>),
    RevParse(Vec<String>),
    Status(Vec<String>),
    ChangeList(Vec<String>),
    ChangeShow(Vec<String>),
    ChangeLog(Vec<String>),
    ChangeDiff(Vec<String>),
    Attestations(Vec<String>),
    ViewShow(Vec<String>),
    Try {
        state: studio_runtime_schema::review::LayrStateId,
        argv: TryCommand,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TryCommand {
    Retest(Vec<String>),
    Bisect(Vec<String>),
    Other(Vec<String>),
}

impl TryCommand {
    fn parse(argv: Vec<String>) -> Self {
        match argv.get(0..2).map(|a| (a[0].as_str(), a[1].as_str())) {
            Some(("layr", "retest")) => Self::Retest(argv),
            Some(("layr", "bisect")) => Self::Bisect(argv),
            _ => Self::Other(argv),
        }
    }
    fn argv(&self) -> &[String] {
        match self {
            Self::Retest(a) | Self::Bisect(a) | Self::Other(a) => a,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ReadRequest {
    line: Option<String>,
    command: LayrReadCommand,
}

/// Parse the argument list once before dispatch.
pub(crate) fn command(args: &Value) -> Result<ReadRequest, String> {
    let argv: Vec<String> = match args.get("args") {
        Some(Value::Array(items))
            if !items.is_empty()
                && items.len() <= MAX_ARGUMENTS
                && items.iter().all(|item| {
                    item.as_str().is_some_and(|s| {
                        !s.contains('\0') && s.chars().count() <= MAX_ARGUMENT_CHARS
                    })
                }) =>
        {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        }
        _ => return Err(format!("Supply args as 1 to {MAX_ARGUMENTS} strings")),
    };
    let mut parts = argv.as_slice();
    let line = if parts.first().is_some_and(|s| s == "--line") {
        if parts.len() < 3 || parts[1].is_empty() || parts[1].starts_with('-') {
            return Err(REFUSAL.into());
        }
        let line = Some(parts[1].clone());
        parts = &parts[2..];
        line
    } else {
        None
    };
    let rest = |n| parts[n..].to_vec();
    let command = match parts.first().map(String::as_str) {
        Some("show") => LayrReadCommand::Show(rest(1)),
        Some("diff") => LayrReadCommand::Diff(rest(1)),
        Some("log") => LayrReadCommand::Log(rest(1)),
        Some("blame") => LayrReadCommand::Blame(rest(1)),
        Some("grep") => LayrReadCommand::Grep(rest(1)),
        Some("ls-files") => LayrReadCommand::LsFiles(rest(1)),
        Some("merge-base") => LayrReadCommand::MergeBase(rest(1)),
        Some("rev-parse") => LayrReadCommand::RevParse(rest(1)),
        Some("status") => LayrReadCommand::Status(rest(1)),
        Some("change") if parts.len() >= 2 => match parts[1].as_str() {
            "list" => LayrReadCommand::ChangeList(rest(2)),
            "show" => LayrReadCommand::ChangeShow(rest(2)),
            "log" => LayrReadCommand::ChangeLog(rest(2)),
            "diff" => LayrReadCommand::ChangeDiff(rest(2)),
            _ => return Err(REFUSAL.into()),
        },
        Some("attestations") => LayrReadCommand::Attestations(rest(1)),
        Some("view") if parts.get(1).is_some_and(|s| s == "show") => {
            LayrReadCommand::ViewShow(rest(2))
        }
        Some("try")
            if line.is_none() && parts.len() >= 4 && parts[2] == "--" && !parts[3].is_empty() =>
        {
            let state = studio_runtime_schema::review::LayrStateId::try_from(parts[1].clone())
                .map_err(|_| REFUSAL.to_owned())?;
            LayrReadCommand::Try {
                state,
                argv: TryCommand::parse(rest(3)),
            }
        }
        _ => return Err(REFUSAL.into()),
    };
    Ok(ReadRequest { line, command })
}

impl ReadRequest {
    fn argv(&self) -> Vec<String> {
        let mut result = Vec::new();
        if let Some(line) = &self.line {
            result.extend(["--line".into(), line.clone()]);
        }
        let (prefix, args): (&[&str], &[String]) = match &self.command {
            LayrReadCommand::Show(a) => (&["show"], a),
            LayrReadCommand::Diff(a) => (&["diff"], a),
            LayrReadCommand::Log(a) => (&["log"], a),
            LayrReadCommand::Blame(a) => (&["blame"], a),
            LayrReadCommand::Grep(a) => (&["grep"], a),
            LayrReadCommand::LsFiles(a) => (&["ls-files"], a),
            LayrReadCommand::MergeBase(a) => (&["merge-base"], a),
            LayrReadCommand::RevParse(a) => (&["rev-parse"], a),
            LayrReadCommand::Status(a) => (&["status"], a),
            LayrReadCommand::ChangeList(a) => (&["change", "list"], a),
            LayrReadCommand::ChangeShow(a) => (&["change", "show"], a),
            LayrReadCommand::ChangeLog(a) => (&["change", "log"], a),
            LayrReadCommand::ChangeDiff(a) => (&["change", "diff"], a),
            LayrReadCommand::Attestations(a) => (&["attestations"], a),
            LayrReadCommand::ViewShow(a) => (&["view", "show"], a),
            LayrReadCommand::Try { state, argv } => {
                result.extend(["try".into(), state.as_str().into(), "--".into()]);
                return result
                    .into_iter()
                    .chain(argv.argv().iter().cloned())
                    .collect();
            }
        };
        result.extend(prefix.iter().map(|s| (*s).to_owned()));
        result.extend_from_slice(args);
        result
    }
}

/// The stable identity for one exact tool request.
pub(crate) fn identity(actor_id: &str, request_id: &str) -> String {
    format!(
        "{REQUEST_ID_PREFIX}{}",
        studio_runtime_store::sha256_hex(format!("{actor_id}:{request_id}").as_bytes())
    )
}

#[cfg(test)]
mod tests;

/// Native reviewers retain their provider sandbox. This port permits history reads only.
pub(crate) async fn run_native(
    store: &studio_runtime_store::RuntimeStore,
    actor: &Actor,
    assignment: &studio_runtime_schema::review::ReviewAssignment,
    args: &Value,
    request_id: &str,
) -> DispatchedOperation {
    let record = actor.record().value();
    if record["role"] != "reviewer" || tool_definition_for(assignment).is_none() {
        return refused("layr_read requires a Layr state reviewer");
    }
    let request = match command(args) {
        Ok(request) => request,
        Err(error) => return refused(&error),
    };
    let is_try = matches!(request.command, LayrReadCommand::Try { .. });
    let argv = request.argv();
    let state_dir = store.root().to_owned();
    let request_id = identity(actor.record().id.0.as_str(), request_id);
    let outcome = studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/native-layr/read",
        move || {
            crate::layr_native::operation(
                &state_dir,
                &request_id,
                &json!({"agent":record["id"],"args":argv}),
                || {
                    let cwd = crate::layr_native::reviewer_read_cwd(&state_dir, &record)?;
                    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
                    let output = if is_try {
                        crate::layr_native::reviewer_try(&state_dir, &record, &argv)?
                    } else {
                        crate::layr_native::run(&state_dir, &cwd, &refs)?
                    };
                    Ok(json!({"exitCode":0,"stdout":output,"stderr":""}))
                },
            )
        },
    )
    .await;
    match outcome {
        Ok(Ok(value)) => DispatchedOperation::Applied(OperationResult::reply(&value)),
        Ok(Err(error @ crate::layr_native::NativeLayrError::Refused(_))) => {
            refused(&error.to_string())
        }
        Ok(Err(error)) => unknown(&error.to_string()),
        Err(_) => unknown("Native Layr read worker failed"),
    }
}

fn refused(message: &str) -> DispatchedOperation {
    DispatchedOperation::NotSent(OperationResult::reply(&message))
}

fn unknown(message: &str) -> DispatchedOperation {
    DispatchedOperation::Unknown(OperationResult::reply(&message))
}
