//! Bounded, credential-free native Codex compatibility checks for native-runtime.
//!
//! Python parity: `codex_native_binary.py:87-96` (isolated child environment),
//! `:117-175` (bounded command and version), `:272-303` (protocol schema),
//! `:382-453` (app-server smoke), and `:463-478` (Code Mode handshake).

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, de::IgnoredAny};
use studio_json::{Value, json};
use studio_runtime_providers::codex;
use studio_runtime_schema::json_object::JsonObject;
use studio_runtime_services::{
    native_runtime::{
        CompatibilityEvidence, CompatibilityProbe, CompatibilityRefusal, CompatibilityResult,
        NativeRuntimeError,
    },
    provider_binaries::NativePlatform,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const SMOKE_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_FRAME_BYTES: usize = 1024 * 1024;

pub(crate) struct SystemCompatibilityProbe {
    runtime: tokio::runtime::Handle,
}

impl SystemCompatibilityProbe {
    pub(crate) fn new(runtime: tokio::runtime::Handle) -> Self {
        Self { runtime }
    }

    #[allow(
        clippy::disallowed_methods,
        reason = "native compatibility probes run on labeled native-runtime blocking workers"
    )]
    fn block_on<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        self.runtime.block_on(future)
    }
}

impl CompatibilityProbe for SystemCompatibilityProbe {
    fn version(&self, executable: &Path) -> Result<String, NativeRuntimeError> {
        let executable = executable.to_owned();
        let temp = ProbeHome::new()?;
        let output = self.block_on(run(
            &executable,
            &["--version"],
            temp.path(),
            COMMAND_TIMEOUT,
        ))?;
        studio_runtime_services::provider_binaries::codex_cli_version(&output)
            .map_err(|_| NativeRuntimeError::InvalidVersion)
    }

    fn check(
        &self,
        bundle: &Path,
        platform: NativePlatform,
    ) -> Result<CompatibilityResult, NativeRuntimeError> {
        let executable = bundle.join(platform.executable_name());
        let temp = ProbeHome::new()?;
        match self.block_on(check_bundle(&executable, bundle, temp.path())) {
            Ok(evidence) => Ok(CompatibilityResult::Approved(evidence)),
            Err(CompatibilityRefusal::ProtocolSchema) => Ok(CompatibilityResult::Refused(
                CompatibilityRefusal::ProtocolSchema,
            )),
            Err(CompatibilityRefusal::ProviderSmoke) => Ok(CompatibilityResult::Refused(
                CompatibilityRefusal::ProviderSmoke,
            )),
            Err(CompatibilityRefusal::CompanionCheck) => Ok(CompatibilityResult::Refused(
                CompatibilityRefusal::CompanionCheck,
            )),
            Err(CompatibilityRefusal::UnsupportedPlatform) => Ok(CompatibilityResult::Refused(
                CompatibilityRefusal::UnsupportedPlatform,
            )),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompanionReady {
    #[serde(rename = "type")]
    kind: String,
    selected_version: f64,
    #[serde(rename = "capabilities")]
    _capabilities: Vec<IgnoredAny>,
}
fn companion_ready(bytes: &[u8]) -> Result<(), ()> {
    let JsonObject(ready): JsonObject<CompanionReady> =
        studio_json::from_slice(bytes).map_err(|_| ())?;
    (ready.kind == "connection/ready" && ready.selected_version == 1.0)
        .then_some(())
        .ok_or(())
}

async fn check_bundle(
    executable: &Path,
    bundle: &Path,
    home: &Path,
) -> Result<CompatibilityEvidence, CompatibilityRefusal> {
    let names = if cfg!(windows) {
        ["codex-code-mode-host.exe"]
    } else {
        ["codex-code-mode-host"]
    };
    let mut companions = BTreeMap::new();
    for name in names {
        let path = bundle.join(name);
        let help = run(&path, &["--help"], home, COMMAND_TIMEOUT)
            .await
            .map_err(|_| CompatibilityRefusal::CompanionCheck)?;
        let help = String::from_utf8_lossy(&help);
        if !help.contains(&format!("Usage: {name}")) || !help.contains("--listen") {
            return Err(CompatibilityRefusal::CompanionCheck);
        }
        let hello = studio_json::to_vec(&json!({
            "type":"connection/hello",
            "supportedVersions":[1],
            "requiredCapabilities":[],
            "optionalCapabilities":[]
        }))
        .map_err(|_| CompatibilityRefusal::CompanionCheck)?;
        let mut frame = u32::try_from(hello.len())
            .map_err(|_| CompatibilityRefusal::CompanionCheck)?
            .to_le_bytes()
            .to_vec();
        frame.extend(hello);
        let reply = run_input(&path, &["--listen", "stdio"], home, &frame, COMMAND_TIMEOUT)
            .await
            .map_err(|_| CompatibilityRefusal::CompanionCheck)?;
        if reply.len() < 4
            || usize::try_from(u32::from_le_bytes([reply[0], reply[1], reply[2], reply[3]])).ok()
                != Some(reply.len() - 4)
        {
            return Err(CompatibilityRefusal::CompanionCheck);
        }
        companion_ready(&reply[4..]).map_err(|_| CompatibilityRefusal::CompanionCheck)?;
        companions.insert(name.to_owned(), json!({"smoke":true}));
    }

    let schema_dir = home.join("schemas");
    fs::create_dir(&schema_dir).map_err(|_| CompatibilityRefusal::ProtocolSchema)?;
    run(
        executable,
        &[
            "app-server",
            "generate-json-schema",
            "--experimental",
            "--out",
            schema_dir
                .to_str()
                .ok_or(CompatibilityRefusal::ProtocolSchema)?,
        ],
        home,
        COMMAND_TIMEOUT,
    )
    .await
    .map_err(|_| CompatibilityRefusal::ProtocolSchema)?;
    let schema = schema_check(&schema_dir)?;
    smoke(executable, home)
        .await
        .map_err(|_| CompatibilityRefusal::ProviderSmoke)?;
    Ok(CompatibilityEvidence {
        schema,
        smoke: json!(["initialize", "config/read", "thread/loaded/list"]),
        companions,
    })
}

fn schema_check(directory: &Path) -> Result<Value, CompatibilityRefusal> {
    let read = |name: &str| -> Result<Value, CompatibilityRefusal> {
        let path = directory.join(name);
        let metadata = fs::metadata(&path).map_err(|_| CompatibilityRefusal::ProtocolSchema)?;
        if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
            return Err(CompatibilityRefusal::ProtocolSchema);
        }
        let bytes = fs::read(path).map_err(|_| CompatibilityRefusal::ProtocolSchema)?;
        let value: Value =
            studio_json::from_slice(&bytes).map_err(|_| CompatibilityRefusal::ProtocolSchema)?;
        value
            .as_object()
            .ok_or(CompatibilityRefusal::ProtocolSchema)?;
        Ok(value)
    };
    let requests = read("ClientRequest.json")?;
    let methods = requests
        .get("oneOf")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|variant| {
            variant
                .pointer("/properties/method/enum")
                .and_then(Value::as_array)
        })
        .flatten()
        .filter_map(Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let required = [
        "initialize",
        "config/read",
        "thread/loaded/list",
        "turn/start",
        "thread/metadata/update",
        "thread/queue/list",
        "thread/backgroundTerminals/list",
        "thread/settings/update",
        "model/list",
    ];
    if required.iter().any(|method| !methods.contains(method)) {
        return Err(CompatibilityRefusal::ProtocolSchema);
    }
    let turn = read("v2/TurnStartParams.json")?;
    if turn.pointer("/properties/cyberAccessProgram").is_none()
        || !["standard", "daybreakBlue", "daybreakRed"]
            .iter()
            .all(|program| {
                turn.pointer("/definitions/CyberAccessProgram/enum")
                    .and_then(Value::as_array)
                    .is_some_and(|values| values.iter().any(|value| value == program))
            })
    {
        return Err(CompatibilityRefusal::ProtocolSchema);
    }
    let models = read("v2/ModelListResponse.json")?;
    if models
        .pointer("/definitions/Model/properties/availableAccessPrograms")
        .is_none()
    {
        return Err(CompatibilityRefusal::ProtocolSchema);
    }
    let usage = read("v2/ThreadTokenUsageUpdatedNotification.json")?;
    if !studio_runtime_schema::usage::codex_usage_schema_compatible(&usage) {
        return Err(CompatibilityRefusal::ProtocolSchema);
    }
    let errors = read("v2/ErrorNotification.json")?;
    if !studio_runtime_schema::provider_errors::codex_error_schema_compatible(&errors) {
        return Err(CompatibilityRefusal::ProtocolSchema);
    }
    Ok(json!({"methods":required,"daybreak":true,"usage":true,"errors":true}))
}

async fn smoke(executable: &Path, home: &Path) -> Result<(), ()> {
    let mut command = Command::new(executable);
    command
        .args([
            "app-server",
            "--listen",
            "stdio://",
            "-c",
            "cli_auth_credentials_store=\"file\"",
        ])
        .current_dir(home)
        .env_clear()
        .envs(safe_environment(home))
        .kill_on_drop(true)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let mut child = command.spawn().map_err(|_| ())?;
    let mut input = child.stdin.take().ok_or(())?;
    let output = child.stdout.take().ok_or(())?;
    let mut reader = BufReader::new(output);
    let deadline = tokio::time::Instant::now() + SMOKE_TIMEOUT;
    let result = async {
        let initialize = codex::ClientRequest::Initialize {
            id: codex::RequestId::Variant2(Box::new(1)),
            params: studio_runtime_providers::codex_initialize_params(
                studio_runtime_providers::ClientName::UpdateProbe,
            ),
        };
        write_frame(&mut input, &initialize).await?;
        let codex::ClientRequestResult::Initialize(_) = response(
            &mut reader,
            1,
            codex::ClientRequestMethod::Initialize,
            deadline,
        )
        .await?
        else {
            return Err(());
        };
        write_frame(&mut input, &codex::ClientNotification::Initialized).await?;
        let config = codex::ClientRequest::ConfigRead {
            id: codex::RequestId::Variant2(Box::new(2)),
            params: codex::ConfigReadParams {
                cwd: None,
                include_layers: Some(false),
            },
        };
        write_frame(&mut input, &config).await?;
        let codex::ClientRequestResult::ConfigRead(_) = response(
            &mut reader,
            2,
            codex::ClientRequestMethod::ConfigRead,
            deadline,
        )
        .await?
        else {
            return Err(());
        };
        let loaded = codex::ClientRequest::ThreadLoadedList {
            id: codex::RequestId::Variant2(Box::new(3)),
            params: codex::ThreadLoadedListParams {
                cursor: None,
                limit: None,
            },
        };
        write_frame(&mut input, &loaded).await?;
        let codex::ClientRequestResult::ThreadLoadedList(loaded) = response(
            &mut reader,
            3,
            codex::ClientRequestMethod::ThreadLoadedList,
            deadline,
        )
        .await?
        else {
            return Err(());
        };
        if !loaded.data.is_empty() {
            return Err(());
        }
        Ok(())
    }
    .await;
    drop(input);
    let _ignored = child.kill().await;
    let _ignored = child.wait().await;
    result
}

async fn write_frame(
    input: &mut tokio::process::ChildStdin,
    value: &impl serde::Serialize,
) -> Result<(), ()> {
    let mut bytes = studio_json::to_vec(value).map_err(|_| ())?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(());
    }
    bytes.push(b'\n');
    input.write_all(&bytes).await.map_err(|_| ())?;
    input.flush().await.map_err(|_| ())
}

async fn response<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    id: u64,
    method: codex::ClientRequestMethod,
    deadline: tokio::time::Instant,
) -> Result<codex::ClientRequestResult, ()> {
    loop {
        let line = tokio::time::timeout_at(deadline, bounded_line(reader, MAX_FRAME_BYTES))
            .await
            .map_err(|_| ())??;
        if let Some(result) =
            studio_runtime_providers::decode_probe_response(&line, id, method).map_err(|_| ())?
        {
            return Ok(result);
        }
    }
}

async fn bounded_line<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    limit: usize,
) -> Result<Vec<u8>, ()> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().await.map_err(|_| ())?;
        if available.is_empty() {
            return Err(());
        }
        let length = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if line.len().saturating_add(length) > limit {
            return Err(());
        }
        let done = available.get(length.saturating_sub(1)) == Some(&b'\n');
        line.extend_from_slice(&available[..length]);
        reader.consume(length);
        if done {
            line.pop();
            return Ok(line);
        }
    }
}

async fn run(
    program: &Path,
    args: &[&str],
    home: &Path,
    timeout: Duration,
) -> Result<Vec<u8>, NativeRuntimeError> {
    run_input(program, args, home, &[], timeout).await
}

async fn run_input(
    program: &Path,
    args: &[&str],
    home: &Path,
    input: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, NativeRuntimeError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(home)
        .env_clear()
        .envs(safe_environment(home))
        .kill_on_drop(true)
        .stdin(if input.is_empty() {
            std::process::Stdio::null()
        } else {
            std::process::Stdio::piped()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn()?;
    let mut stdin = child.stdin.take();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("native probe stdout is unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("native probe stderr is unavailable"))?;
    let mut stdout_task = tokio::spawn(read_bounded(stdout, MAX_OUTPUT_BYTES));
    let mut stderr_task = tokio::spawn(read_bounded(stderr, MAX_OUTPUT_BYTES));
    let result = tokio::time::timeout(timeout, async {
        if !input.is_empty() {
            let input_pipe = stdin
                .as_mut()
                .ok_or_else(|| io::Error::other("native probe stdin is unavailable"))?;
            input_pipe.write_all(input).await?;
        }
        drop(stdin.take());
        let status = child.wait().await?;
        let stdout = (&mut stdout_task)
            .await
            .map_err(|_| io::Error::other("native probe stdout reader failed"))??;
        let stderr = (&mut stderr_task)
            .await
            .map_err(|_| io::Error::other("native probe stderr reader failed"))??;
        Ok::<_, io::Error>((status, stdout, stderr))
    })
    .await;
    let (status, stdout, stderr) = match result {
        Ok(result) => result?,
        Err(_) => {
            let _ignored = child.kill().await;
            let _ignored = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "native compatibility command timed out",
            )
            .into());
        }
    };
    if stdout.len().saturating_add(stderr.len()) > MAX_OUTPUT_BYTES || !status.success() {
        return Err(io::Error::other("native compatibility command failed").into());
    }
    Ok(stdout)
}

async fn read_bounded<R: AsyncRead + Unpin>(reader: R, limit: usize) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    reader
        .take((limit + 1) as u64)
        .read_to_end(&mut output)
        .await?;
    if output.len() > limit {
        return Err(io::Error::other(
            "native compatibility output exceeded limit",
        ));
    }
    Ok(output)
}

fn safe_environment(home: &Path) -> BTreeMap<&'static str, String> {
    let mut environment = BTreeMap::new();
    for name in [
        "PATH",
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "PATHEXT",
        "LANG",
        "LC_ALL",
        "TMPDIR",
    ] {
        if let Some(value) = std::env::var_os(name) {
            environment.insert(name, value.to_string_lossy().into_owned());
        }
    }
    let home = home.to_string_lossy().into_owned();
    for name in [
        "HOME",
        "USERPROFILE",
        "TEMP",
        "TMP",
        "CODEX_HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "TMPDIR",
    ] {
        environment.insert(name, home.clone());
    }
    environment
}

struct ProbeHome(PathBuf);

impl ProbeHome {
    fn new() -> Result<Self, NativeRuntimeError> {
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(|error| io::Error::other(error.to_string()))?;
        let path = std::env::temp_dir().join(format!(
            "studio-native-probe-{}",
            data_encoding::HEXLOWER.encode(&nonce)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ProbeHome {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.0);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::SystemCompatibilityProbe;
    use std::{fs, os::unix::fs::PermissionsExt};
    use studio_json::json;
    use studio_runtime_services::{
        native_runtime::{CompatibilityProbe, CompatibilityResult},
        provider_binaries::NativePlatform,
    };

    #[test]
    fn probe_results_use_the_pinned_vendor_contract() {
        use studio_runtime_providers::{codex::ClientRequestMethod, decode_probe_response};
        for method in [
            ClientRequestMethod::Initialize,
            ClientRequestMethod::ConfigRead,
            ClientRequestMethod::ThreadLoadedList,
        ] {
            assert!(
                decode_probe_response(br#"{"method":false,"params":3}"#, 1, method)
                    .unwrap()
                    .is_none()
            );
            for raw in [
                br#"{"id":1,"result":null}"#.as_slice(),
                br#"{"id":1,"result":{}}"#,
                br#"{"id":1,"error":{"code":1,"message":"refused"}}"#,
                br#"{"id":2,"result":{"data":[]}}"#,
            ] {
                assert!(decode_probe_response(raw, 1, method).is_err());
            }
        }
        let result = decode_probe_response(
            br#"{"id":1,"result":{"data":[]}}"#,
            1,
            ClientRequestMethod::ThreadLoadedList,
        )
        .unwrap()
        .unwrap();
        assert!(
            matches!(result, super::codex::ClientRequestResult::ThreadLoadedList(result) if result.data.is_empty())
        );
        assert!(
            decode_probe_response(
                br#"{"id":1,"result":{"data":[1]}}"#,
                1,
                ClientRequestMethod::ThreadLoadedList
            )
            .is_err()
        );
    }

    #[test]
    fn companion_ready_validates_the_separate_handshake() {
        assert!(
            super::companion_ready(
                br#"{"type":"connection/ready","selectedVersion":1,"capabilities":[null,{},1]}"#
            )
            .is_ok()
        );
        assert!(
            super::companion_ready(
                br#"{"type":"connection/ready","selectedVersion":1,"capabilities":{}}"#
            )
            .is_err()
        );
        assert!(super::companion_ready(br#"{"type":"bad","type":"connection/ready","selectedVersion":1,"capabilities":[]}"#).is_err());
    }

    #[tokio::test]
    async fn probe_smoke_fake_executable_rejects_invalid_payloads() {
        let root = tempfile::tempdir().unwrap();
        let program = root.path().join("fake");
        for (initialize, config, loaded, valid) in [
            (
                r#"{"codexHome":"/codex-home","platformFamily":"unix","platformOs":"test","userAgent":""}"#,
                r#"{"config":{"future":null},"origins":{}}"#,
                r#"{"data":[]}"#,
                true,
            ),
            (
                r#"{"codexHome":"/codex-home","platformFamily":"unix","platformOs":"test","userAgent":null}"#,
                r#"{"config":{},"origins":{}}"#,
                r#"{"data":[]}"#,
                false,
            ),
            (
                r#"{"codexHome":"/codex-home","platformFamily":"unix","platformOs":"test","userAgent":"x"}"#,
                r#"{"config":[],"origins":{}}"#,
                r#"{"data":[]}"#,
                false,
            ),
            (
                r#"{"codexHome":"/codex-home","platformFamily":"unix","platformOs":"test","userAgent":"x"}"#,
                r#"{"config":{},"origins":{}}"#,
                r#"{"data":[1]}"#,
                false,
            ),
        ] {
            let script = format!(
                r##"#!/bin/sh
read -r line
printf '%s\n' '{{"id":1,"result":{initialize}}}'
read -r line
read -r line
printf '%s\n' '{{"id":2,"result":{config}}}'
read -r line
printf '%s\n' '{{"id":3,"result":{loaded}}}'
"##
            );
            write_executable(&program, &script);
            assert_eq!(super::smoke(&program, root.path()).await.is_ok(), valid);
        }
    }

    #[tokio::test]
    async fn probe_response_skips_notifications_and_keeps_bounded_input() {
        use tokio::io::BufReader;
        let mut reader =
            BufReader::new(&b"{\"method\":false}\n{\"id\":1,\"result\":{\"data\":[]}}\n"[..]);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
        assert!(
            matches!(super::response(&mut reader, 1, super::codex::ClientRequestMethod::ThreadLoadedList, deadline).await, Ok(super::codex::ClientRequestResult::ThreadLoadedList(result)) if result.data.is_empty())
        );
        let oversized = vec![b'x'; super::MAX_FRAME_BYTES + 1];
        let mut reader = BufReader::new(oversized.as_slice());
        assert!(
            super::response(
                &mut reader,
                1,
                super::codex::ClientRequestMethod::ThreadLoadedList,
                deadline
            )
            .await
            .is_err()
        );
        let (_writer, reader) = tokio::io::duplex(1);
        let mut reader = BufReader::new(reader);
        assert!(
            super::response(
                &mut reader,
                1,
                super::codex::ClientRequestMethod::ThreadLoadedList,
                tokio::time::Instant::now()
            )
            .await
            .is_err()
        );
    }

    fn write_executable(path: &std::path::Path, source: &str) {
        fs::write(path, source).expect("fixture executable");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .expect("make fixture executable");
    }

    fn error_notification_schema() -> studio_json::Value {
        let mut leaf: studio_json::Value =
            studio_json::from_str(studio_runtime_schema::provider_errors::CODEX_ERROR_SCHEMA)
                .expect("vendor error leaf");
        let definitions = leaf
            .as_object_mut()
            .expect("leaf object")
            .remove("definitions")
            .expect("referenced definitions");
        json!({"definitions":{"CodexErrorInfo":leaf,"NonSteerableTurnKind":definitions["NonSteerableTurnKind"]}})
    }

    #[test]
    fn schema_check_refuses_changed_usage_and_error_fragments() {
        let directory = tempfile::tempdir().expect("schema fixture");
        let methods = [
            "initialize",
            "config/read",
            "thread/loaded/list",
            "turn/start",
            "thread/metadata/update",
            "thread/queue/list",
            "thread/backgroundTerminals/list",
            "thread/settings/update",
            "model/list",
        ];
        let usage: studio_json::Value =
            studio_json::from_str(studio_runtime_schema::usage::CODEX_USAGE_SCHEMA)
                .expect("vendor");
        for (name, value) in [
            (
                "ClientRequest.json",
                json!({"oneOf":methods.map(|method|json!({"properties":{"method":{"enum":[method]}}}))}),
            ),
            (
                "v2/TurnStartParams.json",
                json!({"properties":{"cyberAccessProgram":{}},"definitions":{"CyberAccessProgram":{"enum":["standard","daybreakBlue","daybreakRed"]}}}),
            ),
            (
                "v2/ModelListResponse.json",
                json!({"definitions":{"Model":{"properties":{"availableAccessPrograms":{}}}}}),
            ),
            ("v2/ThreadTokenUsageUpdatedNotification.json", usage.clone()),
            ("v2/ErrorNotification.json", error_notification_schema()),
        ] {
            let path = directory.path().join(name);
            fs::create_dir_all(path.parent().expect("parent")).expect("fixture directory");
            fs::write(path, value.to_string()).expect("schema file");
        }
        assert!(super::schema_check(directory.path()).is_ok());
        let path = directory
            .path()
            .join("v2/ThreadTokenUsageUpdatedNotification.json");
        let mut description = usage.clone();
        description["description"] = json!("Updated provider documentation");
        let mut documented = description.clone();
        documented["title"] = json!("New title");
        documented["definitions"]["TokenUsageBreakdown"]["properties"]["inputTokens"]["format"] =
            json!("new-format-hint");
        documented["definitions"]["TokenUsageBreakdown"]["properties"]["cacheWriteInputTokens"]["default"] =
            json!(99);
        let mut extended = usage.clone();
        extended["properties"]["futureNotification"] = json!({"type":"string"});
        extended["definitions"]["ThreadTokenUsage"]["properties"]["futureContext"] =
            json!({"type":"string"});
        for compatible in [description, documented, extended] {
            fs::write(&path, compatible.to_string()).expect("compatible schema");
            assert!(
                super::schema_check(directory.path()).is_ok(),
                "documentation and unrelated additions are compatible"
            );
        }
        let mut category = usage.clone();
        category["definitions"]["TokenUsageBreakdown"]["properties"]["futureTokens"] =
            json!({"type":"integer"});
        let mut retyped = usage.clone();
        retyped["definitions"]["TokenUsageBreakdown"]["properties"]["inputTokens"]["type"] =
            json!("number");
        let mut optional = usage.clone();
        optional["definitions"]["TokenUsageBreakdown"]["required"] = json!([
            "cachedInputTokens",
            "outputTokens",
            "reasoningOutputTokens",
            "totalTokens"
        ]);
        let mut missing = usage.clone();
        missing["definitions"]["ThreadTokenUsage"]["properties"]
            .as_object_mut()
            .expect("properties")
            .remove("last");
        let mut notification_type = usage.clone();
        notification_type["properties"]["turnId"]["type"] = json!("integer");
        for incompatible in [category, retyped, optional, missing, notification_type] {
            fs::write(&path, incompatible.to_string()).expect("incompatible schema");
            assert!(matches!(
                super::schema_check(directory.path()),
                Err(studio_runtime_services::native_runtime::CompatibilityRefusal::ProtocolSchema)
            ));
        }
        fs::write(&path, usage.to_string()).expect("restore compatible usage");
        let path = directory.path().join("v2/ErrorNotification.json");
        let errors = error_notification_schema();
        for pointer in [
            "/definitions/CodexErrorInfo/anyOf/0/enum/0",
            "/definitions/CodexErrorInfo/anyOf/1/properties/httpConnectionFailed/properties/httpStatusCode/type",
            "/definitions/NonSteerableTurnKind/enum/0",
        ] {
            let mut changed = errors.clone();
            *changed
                .pointer_mut(pointer)
                .expect("consumed error contract") = json!("drift");
            fs::write(&path, changed.to_string()).expect("changed error schema");
            assert!(matches!(
                super::schema_check(directory.path()),
                Err(studio_runtime_services::native_runtime::CompatibilityRefusal::ProtocolSchema)
            ));
        }
    }

    #[test]
    fn version_probe_runs_candidate_with_isolated_home() {
        let directory = tempfile::tempdir().expect("temporary fixture directory");
        let executable = directory.path().join("codex");
        write_executable(
            &executable,
            "#!/bin/sh\ntest \"$HOME\" = \"$CODEX_HOME\" || exit 7\ntest -z \"${OPENAI_API_KEY:-}\" || exit 8\nprintf 'codex-cli 1.2.3\\n'\n",
        );
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("probe runtime");
        let probe = SystemCompatibilityProbe::new(runtime.handle().clone());
        let version = runtime.block_on(async move {
            studio_runtime_process::blocking_pool::spawn_blocking(
                "runtime-server/native_probe",
                move || probe.version(&executable),
            )
            .await
            .expect("version probe task")
        });
        assert_eq!(version.expect("valid version"), "1.2.3");
    }

    #[test]
    fn compatibility_probe_checks_companion_schema_and_isolated_provider() {
        let directory = tempfile::tempdir().expect("temporary fixture directory");
        let bundle = directory.path().to_owned();
        fs::write(
            bundle.join("usage-schema.json"),
            studio_runtime_schema::usage::CODEX_USAGE_SCHEMA,
        )
        .expect("usage fixture");
        fs::write(
            bundle.join("error-schema.json"),
            error_notification_schema().to_string(),
        )
        .expect("error fixture");
        let codex = bundle.join("codex");
        write_executable(
            &codex,
            r##"#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
assert.equal(process.env.OPENAI_API_KEY, undefined);
assert.equal(process.env.CODEX_API_KEY, undefined);
assert.equal(process.env.HOME, process.env.CODEX_HOME);
if (process.argv.includes('--version')) {
    console.log('codex-cli 1.2.3');
} else if (process.argv.includes('generate-json-schema')) {
    const root = process.argv[process.argv.indexOf('--out') + 1];
    const methods = ['initialize', 'config/read', 'thread/loaded/list', 'turn/start',
        'thread/metadata/update', 'thread/queue/list', 'thread/backgroundTerminals/list',
        'thread/settings/update', 'model/list'];
    const schemas = {
        'ClientRequest.json': {oneOf: methods.map(method => ({properties: {method: {enum: [method]}}}))},
        'v2/ThreadTokenUsageUpdatedNotification.json': JSON.parse(fs.readFileSync(path.join(__dirname, 'usage-schema.json'), 'utf8')),
        'v2/ErrorNotification.json': JSON.parse(fs.readFileSync(path.join(__dirname, 'error-schema.json'), 'utf8')),
        'v2/TurnStartParams.json': {properties: {cyberAccessProgram: {}},
            definitions: {CyberAccessProgram: {enum: ['standard', 'daybreakBlue', 'daybreakRed']}}},
        'v2/ModelListResponse.json': {definitions: {Model: {properties: {availableAccessPrograms: {}}}}},
    };
    for (const [name, value] of Object.entries(schemas)) {
        const file = path.join(root, name);
        fs.mkdirSync(path.dirname(file), {recursive: true});
        fs.writeFileSync(file, JSON.stringify(value));
    }
} else {
    const readline = require('node:readline');
    readline.createInterface({input: process.stdin}).on('line', line => {
        const frame = JSON.parse(line);
        if (frame.method === 'initialized') return;
        const result = {'initialize': {userAgent: 'fixture', codexHome: '/codex-home', platformFamily: 'unix', platformOs: 'test'},
            'config/read': {config: {}, origins: {}}, 'thread/loaded/list': {data: []}}[frame.method];
        console.log(JSON.stringify({id: frame.id, result}));
    });
}
"##,
        );
        let companion = bundle.join("codex-code-mode-host");
        write_executable(
            &companion,
            r##"#!/usr/bin/env node
const fs = require('node:fs');
const assert = require('node:assert/strict');
if (process.argv.includes('--help')) {
    console.log('Usage: codex-code-mode-host --listen ADDRESS');
} else {
    const LENGTH_PREFIX_BYTES = 4;
    const prefix = Buffer.alloc(LENGTH_PREFIX_BYTES);
    fs.readSync(0, prefix);
    const input = Buffer.alloc(prefix.readUInt32LE());
    fs.readSync(0, input);
    assert.equal(JSON.parse(input).type, 'connection/hello');
    const reply = Buffer.from(JSON.stringify({type: 'connection/ready', selectedVersion: 1, capabilities: []}));
    prefix.writeUInt32LE(reply.length);
    fs.writeSync(1, Buffer.concat([prefix, reply]));
}
"##,
        );
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("probe runtime");
        let probe = SystemCompatibilityProbe::new(runtime.handle().clone());
        let result = runtime.block_on(async move {
            studio_runtime_process::blocking_pool::spawn_blocking(
                "runtime-server/native_probe",
                move || probe.check(&bundle, NativePlatform::LinuxX64),
            )
            .await
            .expect("compatibility probe task")
        });
        let result = result.expect("compatibility result");
        let CompatibilityResult::Approved(evidence) = result else {
            panic!("compatible fixture should be approved, got {result:?}");
        };
        assert_eq!(
            evidence.smoke,
            json!(["initialize", "config/read", "thread/loaded/list"])
        );
        assert_eq!(evidence.schema["daybreak"], json!(true));
        assert_eq!(
            evidence.companions["codex-code-mode-host"]["smoke"],
            json!(true)
        );
    }
}
