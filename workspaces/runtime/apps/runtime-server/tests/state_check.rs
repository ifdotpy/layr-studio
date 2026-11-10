use std::fs;
use std::process::Command;

#[test]
#[allow(clippy::disallowed_methods)]
fn state_check_reads_fixture_rows_without_changing_or_disclosing_them() {
    let directory = tempfile::tempdir().expect("owned test directory");
    let root = directory.path().to_path_buf();
    fs::create_dir_all(&root).expect("temporary state directory is created");
    let database = root.join("canvas.sqlite3");
    let db = rusqlite::Connection::open(&database).expect("fixture database opens");
    db.execute_batch(include_str!("fixtures/state-check.sql"))
        .expect("fixture rows insert");
    drop(db);
    let before = fs::read(&database).expect("fixture database is readable");

    let supervisor_path = root.join("supervisor.sqlite3");
    let supervisor = rusqlite::Connection::open(&supervisor_path).expect("supervisor fixture");
    supervisor.execute_batch("CREATE TABLE events(handle TEXT, payload TEXT); INSERT INTO events VALUES('private-handle','invalid JSON')").expect("previously unvalidated history");
    drop(supervisor);
    let supervisor_before = fs::read(&supervisor_path).expect("supervisor bytes");

    let result = Command::new(
        std::env::var_os("CARGO_BIN_EXE_studio-runtime-server")
            .expect("cargo sets CARGO_BIN_EXE_studio-runtime-server"),
    )
    .env("PATH", root.join("empty-path"))
    .env("CODEX_AGENTS_PYTHON", root.join("missing-python"))
    .arg("state-check")
    .arg(&root)
    .output()
    .expect("state-check command starts");
    assert!(
        result.status.success(),
        "state-check failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = String::from_utf8(result.stdout).expect("state-check output is UTF-8");
    let report: studio_json::Value =
        studio_json::from_str(&output).expect("state-check prints JSON");
    let tables = report["tables"].as_array().expect("table reports exist");
    let names: std::collections::BTreeSet<_> = tables
        .iter()
        .map(|table| table["table"].as_str().expect("table name is a string"))
        .collect();
    assert_eq!(names.len(), tables.len(), "one report per table");
    for name in [
        "handles",
        "child_identities",
        "degraded_handles",
        "operations",
        "operation_writes",
        "events",
        "recovery_events",
        "supervisor_state",
    ] {
        assert!(names.contains(name), "supervisor registry coverage: {name}");
        let table = tables
            .iter()
            .find(|table| table["table"] == name)
            .expect("single report");
        assert_eq!(table["row_count"], u64::from(name == "events"));
        assert_eq!(table["rejected_count"], 0);
    }
    assert_eq!(
        supervisor_before,
        fs::read(supervisor_path).expect("unchanged supervisor")
    );
    // Frozen reports on main c26e8fc0 before wave 2; new reports may be added freely.
    const PRE_WAVE_TWO_REPORTS: &[&str] = &[
        "runtime_access_clients",
        "runtime_access_discovered",
        "runtime_access_invites",
        "runtime_account_transfers",
        "runtime_agent_moves",
        "runtime_agents",
        "runtime_annotations",
        "runtime_assets",
        "runtime_capacity_retries",
        "runtime_chat_messages",
        "runtime_checkpoints",
        "runtime_complaints",
        "runtime_event_meta",
        "runtime_events",
        "runtime_execution_attempts",
        "runtime_execution_effects",
        "runtime_execution_runs",
        "runtime_federation_inbox",
        "runtime_federation_outbox",
        "runtime_federation_peers",
        "runtime_federation_rooms",
        "runtime_federation_settings",
        "runtime_items",
        "runtime_monitors",
        "runtime_peer_update_settings",
        "runtime_plans",
        "runtime_profiles",
        "runtime_projects",
        "runtime_requests",
        "runtime_rooms",
        "runtime_rules",
        "runtime_server_inbox",
        "runtime_tasks",
        "runtime_tool_requests",
        "runtime_usage_resumes",
        "runtime_work",
        "runtime_workspace_operations",
        "sync_entities",
        "user_terminals",
    ];
    assert!(
        PRE_WAVE_TWO_REPORTS.iter().all(|name| names.contains(name)),
        "pre-wave-two reports must remain present"
    );
    // Registry and system checker owners define current coverage for the CLI.
    let expected: std::collections::BTreeSet<_> = studio_runtime_store::check_state_readonly(&root)
        .expect("registry and system reports read")
        .tables
        .into_iter()
        .map(|table| table.table)
        .collect();
    assert_eq!(names, expected, "registry and system report coverage");
    for retired in ["runtime_budget", "runtime_budget_usage"] {
        assert!(
            !names.contains(retired),
            "retired token-budget tables have no reports"
        );
    }
    assert!(
        tables
            .iter()
            .any(|table| table["table"] == "runtime_workspace_operations")
    );
    for name in ["runtime_agent_moves", "runtime_projects"] {
        let table = tables
            .iter()
            .find(|table| table["table"] == name)
            .expect("relocation table is checked");
        assert_eq!(table["row_count"], 0);
        assert_eq!(table["rejected_count"], 0);
    }
    let peer_settings = tables
        .iter()
        .find(|table| table["table"] == "runtime_peer_update_settings")
        .expect("registered pilot is checked");
    assert_eq!(peer_settings["row_count"], 1);
    assert_eq!(peer_settings["rejected_count"], 0);
    let agents = tables
        .iter()
        .find(|table| table["table"] == "runtime_agents")
        .expect("agent table is checked");
    assert_eq!(agents["row_count"], 2);
    assert_eq!(agents["rejected_count"], 1);
    assert_eq!(agents["failures"][0]["pointer"], "/epoch");
    assert_eq!(agents["failures"][0]["count"], 1);
    let tasks = tables
        .iter()
        .find(|table| table["table"] == "runtime_tasks")
        .expect("task table is checked");
    assert_eq!(tasks["row_count"], 1);
    assert_eq!(tasks["rejected_count"], 0);
    let attempts = tables
        .iter()
        .find(|table| table["table"] == "runtime_execution_attempts")
        .expect("Claude attempts are checked");
    assert_eq!(attempts["row_count"], 2);
    assert_eq!(attempts["rejected_count"], 1);
    assert_eq!(attempts["failures"][0]["pointer"], "/epoch");
    for private_value in [
        "attempt-good",
        "attempt-bad",
        "valid-secret-id",
        "invalid-secret-id",
        "task-secret-id",
        "root-secret",
        "private-record-content",
    ] {
        assert!(!output.contains(private_value));
    }
    assert_eq!(
        fs::read(&database).expect("database remains readable"),
        before
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
#[allow(clippy::disallowed_methods)]
async fn startup_keeps_its_executable_after_the_path_is_unlinked() {
    use std::os::fd::AsRawFd;
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
    use tokio::time::timeout;

    let root = tempfile::tempdir().expect("owned startup directory");
    let binary = root.path().join("runtime");
    fs::copy(env!("CARGO_BIN_EXE_studio-runtime-server"), &binary).expect("copy executable");
    let executable = fs::File::open(&binary).expect("keep executable open");
    fs::remove_file(&binary).expect("unlink executable before its identity step");
    let mut child = tokio::process::Command::new(format!(
        "/proc/{}/fd/{}",
        std::process::id(),
        executable.as_raw_fd()
    ))
    .args(["--port", "0"])
    .env_clear()
    .env("HOME", root.path())
    .env("PATH", root.path().join("empty-path"))
    .env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR"))
    .env("CODEX_AGENTS_STATE_DIR", root.path().join("state"))
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true)
    .spawn()
    .expect("execute the open file after unlink");
    let mut stdout = BufReader::new(child.stdout.take().expect("startup output"));
    let mut line = String::new();
    let read = timeout(Duration::from_secs(30), stdout.read_line(&mut line)).await;
    if !matches!(read, Ok(Ok(count)) if count > 0) {
        drop(child.kill().await);
        let mut stderr = String::new();
        child
            .stderr
            .take()
            .expect("startup diagnostics")
            .read_to_string(&mut stderr)
            .await
            .expect("read diagnostics");
        panic!("server did not start from its unlinked executable: {stderr}");
    }
    assert!(
        line.starts_with("Codex Canvas: http://127.0.0.1:"),
        "{line}"
    );
    child.kill().await.expect("stop fixture");
    child.wait().await.expect("reap fixture");
}
