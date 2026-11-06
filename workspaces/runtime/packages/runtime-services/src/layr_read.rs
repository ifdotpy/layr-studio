//! Read-only Layr commands for native reviewers.

use std::sync::OnceLock;

<<<<<<< head
use base64::Engine;
=======
>>>>>>> agent-8b610cda59ed4865310e15f0285f6e5ba521d8474d0ed359b5e088286474ab31
use studio_json::{Value, json};

use crate::managed_host_tools::{Actor, DispatchedOperation, OperationResult};

/// The tool name.
pub(crate) const TOOL: &str = "layr_read";
/// The layr commands that a reviewer may run, after optional `--line <line>` pairs.
pub(crate) const COMMANDS: [&str; 9] = [
    "blame",
    "diff",
    "grep",
    "log",
    "ls-files",
    "merge-base",
    "rev-parse",
    "show",
    "status",
];
/// Largest argument count of one call.
const MAX_ARGUMENTS: usize = 64;
/// Longest argument, in characters.
const MAX_ARGUMENT_CHARS: usize = 4096;
/// Request ID prefix of one exact tool request.
const REQUEST_ID_PREFIX: &str = "layr-read:";

/// The tool definition for reviewers.
pub(crate) fn tool_definition() -> Option<&'static crate::thread_prepare::ToolDefinition> {
    static DEFINITION: OnceLock<Option<crate::thread_prepare::ToolDefinition>> = OnceLock::new();
    DEFINITION
        .get_or_init(|| {
            studio_json::from_value(json!({
                "type": "function", "name": TOOL,
                "description": format!(
                    "Run one read-only layr command as your Linux user in your read-only line, outside your sandbox: {}. \
                     args is the argument list after `layr`, for example [\"diff\", \"<base>\", \"<state>\"] or \
                     [\"show\", \"<state>:<path>\"]. [\"--line\", \"<line>\", \"log\"] reads another line of your project. \
                     Use head for the current named state. log --json writes JSON Lines; status --json writes one JSON object. \
                     Write notes and copies only in $TMPDIR.",
                    COMMANDS.join(", ")
                ),
                "inputSchema": {"type": "object", "properties": {
                    "args": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": MAX_ARGUMENTS}},
                    "required": ["args"]},
            }))
            .ok()
        })
        .as_ref()
}

/// Native reviewers share the read tool contract and can run a sandboxed temporary check.
pub(crate) fn native_tool_definition() -> Option<&'static crate::thread_prepare::ToolDefinition> {
    static DEFINITION: OnceLock<Option<crate::thread_prepare::ToolDefinition>> = OnceLock::new();
    DEFINITION.get_or_init(|| {
        let mut definition = tool_definition()?.clone();
        definition.description = "Read native Layr states with show, diff, log and the history read commands. Use args [\"try\",\"<state>\",\"--\",\"<command>\",...] for a check in a temporary copy. The check can write only that copy and $TMPDIR, without network. Main and worker lines stay read-only. Supply arguments after layr.".into();
        Some(definition)
    }).as_ref()
}

/// The argument list after `layr`, or the refusal text.
pub(crate) fn command(args: &Value) -> Result<Vec<String>, String> {
    let argv: Vec<String> = match args.get("args") {
        Some(Value::Array(items))
            if !items.is_empty()
                && items.len() <= MAX_ARGUMENTS
                && items.iter().all(|item| {
                    item.as_str().is_some_and(|item| {
                        !item.contains('\0') && item.chars().count() <= MAX_ARGUMENT_CHARS
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
    let mut index = 0;
    while index < argv.len() && argv[index] == "--line" {
        index += 2;
    }
    if index >= argv.len() || !COMMANDS.contains(&argv[index].as_str()) {
        return Err(format!(
            "layr_read runs only {}, after optional --line <line>",
            COMMANDS.join(", ")
        ));
    }
    Ok(argv)
}

/// The guest request identity of tool request `request_id`: a retry of the
/// same request gets the first answer of the guest, and the command does not
/// run again.
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
    args: &Value,
    request_id: &str,
) -> DispatchedOperation {
    let record = actor.record().value();
    if record["role"] != "reviewer" || record["layrReady"] != true {
        return refused("layr_read requires a ready Layr reviewer");
    }
    let argv = match native_command(args) {
        Ok(argv) => argv,
        Err(error) => return refused(&error),
    };
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
                    let output = if argv.first().map(String::as_str) == Some("try") {
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

fn native_command(args: &Value) -> Result<Vec<String>, String> {
    if let Ok(argv) = command(args) {
        return Ok(argv);
    }
    let items = args["args"]
        .as_array()
        .filter(|items| (4..=MAX_ARGUMENTS).contains(&items.len()))
        .ok_or_else(|| {
            "Supply a native Layr read command or try <state> -- <command>".to_owned()
        })?;
    if items[0] != "try"
        || items[2] != "--"
        || items.iter().any(|item| {
            item.as_str().is_none_or(|value| {
                value.contains('\0') || value.chars().count() > MAX_ARGUMENT_CHARS
            })
        })
    {
        return Err("Supply a native Layr read command or try <state> -- <command>".into());
    }
    Ok(items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect())
}

fn refused(message: &str) -> DispatchedOperation {
    DispatchedOperation::NotSent(OperationResult::reply(&message))
}

fn unknown(message: &str) -> DispatchedOperation {
    DispatchedOperation::Unknown(OperationResult::reply(&message))
}
