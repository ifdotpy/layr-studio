pub(super) mod support;

/// The `send` command of `POST /api/commands`; the command id is the message id.
fn send_command(id: &str, room: &str, text: &str, assets: &studio_json::Value) -> String {
    studio_json::json!({
        "id": id, "client": "test", "number": 1, "kind": "send",
        "target": room, "body": {"text": text, "assets": assets}
    })
    .to_string()
}

/// The delivery receipt row that a send response carries.
fn receipt_of(response: &studio_json::Value) -> studio_json::Value {
    response["rows"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|row| row["collection"] == "messageReceipt")
        })
        .map(|row| row["payload"].clone())
        .expect("message receipt row")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_accepts_unrelated_operation_started_after_preparation() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    let fixture = support::Fixture::new("http").await;
    let state = crate::router::HttpState::new(
        "token".into(),
        "workspace".into(),
        "isolated".into(),
        4620,
        None,
        Some(fixture.store.clone()),
        None,
    );
    let app = crate::router::router(state).expect("router");
    let (ready, release) = support::arm();
    let store = fixture.store.clone();
    let operation =
        tokio::spawn(async move { support::start_late_operation(store, ready, release).await });
    let request_id = uuid::Uuid::now_v7().to_string();
    let request = Request::builder()
        .method("POST")
        .uri("/api/commands")
        .header("x-canvas-token", "token")
        .header("host", "127.0.0.1:4620")
        .header("content-type", "application/json")
        .header(
            crate::schema::API_SCHEMA_HASH_HEADER,
            crate::schema::api_schema_hash(),
        )
        .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            12345,
        ))))
        .body(Body::from(send_command(
            &request_id,
            "late-operation-recipient",
            "hello",
            &studio_json::json!([]),
        )))
        .expect("request");
    let response = app.oneshot(request).await.expect("HTTP response");
    operation.await.expect("operation writer joins");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let receipt = receipt_of(
        &studio_json::from_slice(&to_bytes(response.into_body(), 65536).await.expect("body"))
            .expect("command response"),
    );
    assert_eq!(receipt["status"], "queued");
    assert!(
        receipt.get("waitingFor").is_none(),
        "unrelated operation is not a blocker"
    );
}

#[tokio::test]
async fn group_send_returns_public_recipient_status() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use futures_util::StreamExt;
    use tower::ServiceExt;

    let fixture = support::Fixture::new("group-send").await;
    fixture
        .store
        .transaction_with_error(|tx| {
            studio_runtime_services::coordination::create_chat(
                tx,
                "group",
                "Team",
                &["late-operation-recipient".into()],
            )
            .map(|_| ())
        })
        .await
        .expect("create group");
    let state = crate::router::HttpState::new(
        "token".into(),
        "workspace".into(),
        "isolated".into(),
        4620,
        None,
        Some(fixture.store.clone()),
        None,
    );
    let app = crate::router::router(state).expect("router");
    let stream_request = Request::builder()
        .uri("/api/sync/stream?protocol=4&resources=%5B%7B%22kind%22%3A%22state%22%7D%5D&tokenRates=false")
        .header("x-canvas-token", "token").header("host", "127.0.0.1:4620")
        .header(crate::schema::API_SCHEMA_HASH_HEADER, crate::schema::api_schema_hash())
        .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 12345))))
        .body(Body::empty()).expect("stream request");
    let stream_response = app
        .clone()
        .oneshot(stream_request)
        .await
        .expect("stream response");
    assert_eq!(stream_response.status(), axum::http::StatusCode::OK);
    let mut stream = stream_response.into_body().into_data_stream();
    async fn resource(stream: &mut axum::body::BodyDataStream) -> studio_json::Value {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let bytes = stream.next().await.expect("frame").expect("bytes");
                let frame = std::str::from_utf8(&bytes).expect("UTF-8");
                if frame.starts_with("event: resources\n") {
                    return studio_json::from_str(
                        frame
                            .lines()
                            .find_map(|line| line.strip_prefix("data: "))
                            .expect("data"),
                    )
                    .expect("JSON");
                }
            }
        })
        .await
        .expect("resource event deadline")
    }
    resource(&mut stream).await;
    let id = uuid::Uuid::now_v7().to_string();
    let request = Request::builder()
        .method("POST")
        .uri("/api/commands")
        .header("x-canvas-token", "token")
        .header("host", "127.0.0.1:4620")
        .header("content-type", "application/json")
        .header(
            crate::schema::API_SCHEMA_HASH_HEADER,
            crate::schema::api_schema_hash(),
        )
        .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            12345,
        ))))
        .body(Body::from(send_command(
            &id,
            "group",
            "hello",
            &studio_json::json!([]),
        )))
        .expect("request");
    let response = app.oneshot(request).await.expect("HTTP response");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let receipt = receipt_of(
        &studio_json::from_slice(&to_bytes(response.into_body(), 65536).await.expect("body"))
            .expect("command response"),
    );
    assert_eq!(
        receipt["deliveries"]["late-operation-recipient"]["status"],
        "queued"
    );
    let stored_id = id.clone();
    let stored: String = support::read(fixture.store.clone(), move |connection| {
        connection
            .query_row(
                "SELECT text FROM messages WHERE id=?1",
                [&stored_id],
                |row| row.get(0),
            )
            .expect("committed message")
    })
    .await;
    assert_eq!(stored, "hello");
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let event = resource(&mut stream).await;
            if event["resourceVersions"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|version| {
                    version["entityChanges"]["documents"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|row| {
                            row["collection"] == "chat"
                                && row["id"] == "group"
                                && row["value"]["messageCount"] == 1
                                && row["value"]["tail"] == "hello"
                        })
                })
            {
                break;
            }
        }
    })
    .await
    .expect("committed chat change arrives over SSE");
}

#[tokio::test]
async fn http_accepts_uploaded_asset_id_with_empty_message_text() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    let fixture = support::Fixture::new("pending-asset").await;
    let state = crate::router::HttpState::new(
        "token".into(),
        "workspace".into(),
        "isolated".into(),
        4620,
        None,
        Some(fixture.store.clone()),
        None,
    );
    let app = crate::router::router(state).expect("router");
    let asset_id = "00000000-0000-4000-8000-000000000003";
    let request_id = uuid::Uuid::now_v7().to_string();
    let asset = studio_runtime_services::payload_store::upload(
        &fixture.store,
        "late-operation-recipient",
        "pending.txt",
        Some(asset_id),
        "YWJj",
    )
    .await
    .expect("upload through the shared payload owner");
    let request = Request::builder()
        .method("POST")
        .uri("/api/commands")
        .header("x-canvas-token", "token")
        .header("host", "127.0.0.1:4620")
        .header("content-type", "application/json")
        .header(
            crate::schema::API_SCHEMA_HASH_HEADER,
            crate::schema::api_schema_hash(),
        )
        .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            12345,
        ))))
        .body(Body::from(send_command(
            &request_id,
            "late-operation-recipient",
            "",
            &studio_json::json!([asset["id"].clone()]),
        )))
        .expect("request");
    let response = app.oneshot(request).await.expect("HTTP response");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let receipt = receipt_of(
        &studio_json::from_slice(&to_bytes(response.into_body(), 65536).await.expect("body"))
            .expect("command response"),
    );
    assert_eq!(receipt["status"], "queued");
    let raw = support::read(fixture.store.clone(), move |connection| {
        connection
            .query_row(
                "SELECT record FROM runtime_event_meta WHERE id=?1",
                [&request_id],
                |row| row.get::<_, String>(0),
            )
            .expect("saved attachment metadata")
    })
    .await;
    let metadata: studio_json::Value = studio_json::from_str(&raw).expect("metadata JSON");
    assert_eq!(metadata["assets"], studio_json::json!([asset["id"]]));
}
