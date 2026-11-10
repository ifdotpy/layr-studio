use std::{process::Stdio, time::Duration};
use studio_runtime_store::RuntimeStore;

#[tokio::test]
async fn startup_reseed_holds_unreadable_active_and_terminal_monitors_and_serves_health() {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    let state = tempfile::tempdir().expect("state");
    let store = RuntimeStore::open(Some(state.path())).await.unwrap();
    store.transaction(|tx| {
        tx.execute("DELETE FROM sync_entity_meta WHERE key=?1", [studio_runtime_store::WORK_DETAILS_SEED_MARKER])?;
        tx.execute("INSERT INTO runtime_agents(id,record) VALUES('owner',?1)",
            [studio_json::json!({"id":"owner","rootId":"owner","compactions":0,"provider":"codex","epoch":0,"status":"completed","autoWake":true,"accountKey":"default"}).to_string()])?;
        for (id, status) in [("bad-active", "running"), ("bad-terminal", "completed")] {
            tx.execute("INSERT INTO runtime_monitors(id,record) VALUES(?1,?2)", (id,
                studio_json::json!({"id":id,"agent":"owner","status":status,"created":1,"epoch":"invalid"}).to_string()))?;
        }
        tx.execute("INSERT INTO runtime_monitors(id,record) VALUES('bad-base64',?1)",
            [studio_json::json!({"id":"bad-base64","agent":"owner","epoch":0,"status":"running","requestId":"fixture","created":0,"activityAt":0,"activityGeneration":0,"bytes":0,"command":"","cwd":std::env::temp_dir(),"interactive":false,"livenessCommand":"","log":std::env::temp_dir().join("held.log"),"stallTimeoutSeconds":0,"stallWakeGeneration":-1,"successExitCodes":[0],"tail":"","timeout_ms":3600000,"wakeOn":"exit","outputLineTail":{"stdout":"!"}}).to_string()])?;
        Ok(())
    }).await.unwrap();
    drop(store);
    let mut child = tokio::process::Command::new(
        std::env::var_os("CARGO_BIN_EXE_studio-runtime-server").expect("server binary"),
    )
    .args(["--port", "0"])
    .env("CODEX_AGENTS_STATE_DIR", state.path())
    .env_remove("CODEX_BACKEND_BUILD")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true)
    .spawn()
    .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stderr = child.stderr.take().unwrap();
    let address = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                let mut error = String::new();
                stderr.read_to_string(&mut error).await.unwrap();
                panic!("server must reach readiness after reseeding: {error}");
            };
            if let Some(address) = line.strip_prefix("Codex Canvas: http://") {
                break address.to_owned();
            }
        }
    })
    .await
    .expect("bounded startup");
    let mut stream = tokio::net::TcpStream::connect(&address).await.unwrap();
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(
        response.contains("200 OK") && response.contains("ready"),
        "{response}"
    );
    child.kill().await.unwrap();
    let db = rusqlite::Connection::open(state.path().join("canvas.sqlite3")).unwrap();
    let holds: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM runtime_error_incidents WHERE id LIKE 'monitor-hold:bad-%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(holds, 3);
}

#[tokio::test]
async fn startup_serves_health_and_refuses_commands_while_database_is_busy() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let state = tempfile::tempdir().expect("state");
    drop(RuntimeStore::open(Some(state.path())).await.expect("fresh"));
    let db = rusqlite::Connection::open(state.path().join("canvas.sqlite3")).expect("database");
    db.execute_batch("BEGIN IMMEDIATE")
        .expect("hold migration writer");
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").expect("port");
    let port = reservation.local_addr().expect("address").port();
    drop(reservation);
    let mut child = tokio::process::Command::new(
        std::env::var_os("CARGO_BIN_EXE_studio-runtime-server")
            .expect("cargo sets CARGO_BIN_EXE_studio-runtime-server"),
    )
    .args(["--port", &port.to_string()])
    .env("CODEX_AGENTS_STATE_DIR", state.path())
    .env_remove("CODEX_BACKEND_BUILD")
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .kill_on_drop(true)
    .spawn()
    .expect("server");
    async fn request(port: u16, request: &str) -> String {
        // Like the sibling startup tests, allow 30 s: the host may scan a freshly linked
        // binary for several seconds at its first launch, before the server runs.
        let mut stream = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                match tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
                    Ok(stream) => break stream,
                    Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("listener");
        stream.write_all(request.as_bytes()).await.expect("request");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .await
            .expect("response");
        response
    }
    let status = request(
        port,
        "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(
        status.contains("200 OK") && status.contains("migrating"),
        "{status}"
    );
    let command = request(port, "POST /api/commands HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
    assert!(
        command.contains("503 Service Unavailable") && command.contains("migrating"),
        "{command}"
    );
    child.kill().await.expect("interrupt server");
    db.execute_batch("ROLLBACK").expect("release writer");
    drop(db);
    drop(
        RuntimeStore::open(Some(state.path()))
            .await
            .expect("resume"),
    );
}

#[tokio::test]
async fn migration_startup_failure_prevents_listener_and_readiness() {
    let state = tempfile::tempdir().expect("state");
    drop(RuntimeStore::open(Some(state.path())).await.expect("fresh"));
    std::fs::create_dir(state.path().join("accounts")).expect("accounts");
    std::fs::write(
        state.path().join("accounts/registry.json"),
        br#"{"version":2,"accounts":{"broken":{}},"logins":{}}"#,
    )
    .expect("invalid current registry");
    let mut command = tokio::process::Command::new(
        std::env::var_os("CARGO_BIN_EXE_studio-runtime-server")
            .expect("cargo sets CARGO_BIN_EXE_studio-runtime-server"),
    );
    command
        .args(["--port", "0"])
        .env("CODEX_AGENTS_STATE_DIR", state.path())
        .env("CODEX_STUDIO_STARTUP_TRACE", "1")
        .env_remove("CODEX_BACKEND_BUILD")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let result = tokio::time::timeout(Duration::from_secs(30), command.output())
        .await
        .expect("bounded startup")
        .expect("process result");
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stderr.contains("store-open-started"), "{stderr}");
    assert!(stderr.contains("2026-10-26-account-registry"), "{stderr}");
    assert!(!stderr.contains("listener-bound"), "{stderr}");
    assert!(
        !stderr.contains("account-registry-open-started"),
        "strict consumers must not start: {stderr}"
    );
    assert!(!stdout.contains("Codex Canvas:"), "{stdout}");
}

#[tokio::test]
async fn migration_startup_unfinished_receipts_prevent_all_recovery() {
    let state = tempfile::tempdir().expect("state");
    drop(RuntimeStore::open(Some(state.path())).await.expect("fresh"));
    let db = rusqlite::Connection::open(state.path().join("canvas.sqlite3")).expect("database");
    db.execute("UPDATE runtime_payload_migrations SET status='pending',complete=0 WHERE name LIKE '%receipts-tools-finish:%'", []).expect("unfinished receipt checkpoint");
    let mut command = tokio::process::Command::new(
        std::env::var_os("CARGO_BIN_EXE_studio-runtime-server")
            .expect("cargo sets CARGO_BIN_EXE_studio-runtime-server"),
    );
    command
        .args(["--port", "0"])
        .env("CODEX_AGENTS_STATE_DIR", state.path())
        .env("CODEX_STUDIO_STARTUP_TRACE", "1")
        .env_remove("CODEX_BACKEND_BUILD")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let result = tokio::time::timeout(Duration::from_secs(30), command.output())
        .await
        .expect("bounded startup")
        .expect("process");
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("store-open-started"), "{stderr}");
    assert!(stderr.contains("state too old:"), "{stderr}");
    assert!(
        !stderr.contains("account-registry-open-started"),
        "provider recovery must not start: {stderr}"
    );
    assert!(!stderr.contains("listener-bound"), "{stderr}");
    assert_eq!(db.query_row("SELECT status FROM runtime_payload_migrations WHERE name LIKE '%receipts-tools-finish:%'", [], |row| row.get::<_, String>(0)).expect("checkpoint retained"), "pending");
}
