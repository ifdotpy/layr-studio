use std::sync::atomic::AtomicBool;
use studio_json::{Value, json};
use studio_runtime_schema::usage::ledger::LedgerEpoch;
use studio_runtime_services::analytics_history::{self, BatchLimits, HistorySnapshot, RolloutPath};
use studio_runtime_services::analytics_rollout::{RolloutAction, RolloutContext};

fn parse(line: &str) -> Value {
    studio_json::from_str(line).expect("synthetic JSON")
}

fn context() -> RolloutContext {
    RolloutContext::from_record(json!({"threadId":"thread"})).expect("context")
}

fn batch(raw: &str) -> Value {
    let root = tempfile::tempdir().expect("profile");
    let path = root.path().join("rollout.jsonl");
    std::fs::write(&path, raw).expect("fixture");
    let agent = json!({"id":"agent","rootId":"agent","threadId":"thread","accountKey":"default","provider":"codex"});
    analytics_history::prepare(
        HistorySnapshot::from_records(agent, None, LedgerEpoch::Current).expect("snapshot"),
        root.path(),
        RolloutPath::Found(path),
        BatchLimits::default(),
        100.,
        &AtomicBool::new(false),
    )
    .expect("batch")
    .checkpoint()
    .clone()
}

#[test]
fn rollout_characterizes_actions_and_context() {
    let mut context = context();
    let lines: Vec<_> = include_str!("fixtures/rollout/records.jsonl")
        .lines()
        .collect();
    let actions: Vec<_> = lines
        .iter()
        .enumerate()
        .flat_map(|(i, line)| {
            context
                .actions(&parse(line), &format!("row{i}"), 100.)
                .expect("actions")
        })
        .collect();
    assert_eq!(
        actions,
        vec![
            RolloutAction::Event {
                method: "turn/started".into(),
                params: json!({"threadId":"thread","turnId":"turn","turn":{"id":"turn"},"_analyticsTimestampSource":"record"}),
                at: 3.
            },
            RolloutAction::Payload {
                params: json!({"type":"message","role":"assistant","content":"synthetic","_analyticsId":"row3","_analyticsTurnId":"turn","_analyticsTimestampSource":"record"}),
                at: 4.
            },
            RolloutAction::Event {
                method: "analytics/compaction".into(),
                params: json!({"threadId":"thread","turnId":"turn","id":"row4","window_number":2,"first_window_id":null,"previous_window_id":null,"window_id":null,"compaction_response_id":null,"_analyticsTimestampSource":"record"}),
                at: 5.
            },
            RolloutAction::Payload {
                params: json!({"type":"message","role":"user","content":"replacement","_analyticsCategory":"compactionReplacement","_analyticsId":"row4:replacement_history:0","_analyticsTurnId":"turn","_analyticsTimestampSource":"record"}),
                at: 5.
            },
            RolloutAction::Payload {
                params: json!({"type":"message","role":"assistant","content":"guardian","_analyticsCategory":"compactionGuardian","_analyticsId":"row4:guardian_history:0","_analyticsTurnId":"turn","_analyticsTimestampSource":"record"}),
                at: 5.
            },
            RolloutAction::Event {
                method: "item/completed".into(),
                params: json!({"threadId":"thread","turnId":"turn","item":{"id":"item","type":"agentMessage","content":[{"text":"synthetic"}],"durationMs":1500.0,"text":"synthetic"},"_analyticsTimestampSource":"record"}),
                at: 6.
            },
            RolloutAction::Event {
                method: "turn/completed".into(),
                params: json!({"threadId":"thread","turnId":"turn","turn":{"id":"turn","status":"completed","error":null},"durationMs":null,"timeToFirstTokenMs":null,"_analyticsTimestampSource":"record"}),
                at: 7.
            },
        ]
    );
    assert_eq!(
        context.record(),
        &json!({"threadId":"thread","turnId":"turn","model":"model","effort":"high","window":8192})
    );
}

#[test]
fn rollout_characterizes_history_counts_and_legacy_header() {
    let raw = include_str!("fixtures/rollout/anomalies.jsonl");
    let checkpoint = batch(raw);
    assert_eq!(checkpoint["status"], "current");
    assert_eq!(checkpoint["coverage"], "partial");
    assert_eq!(checkpoint["importedRecords"], 7);
    assert_eq!(checkpoint["malformedLines"], 2);
    assert_eq!(checkpoint["offset"], raw.len());
    assert_eq!(checkpoint["context"]["window"], 4096);
    assert_eq!(checkpoint["context"]["turnId"], "duplicate");
}

#[test]
fn rollout_missing_type_is_ignored_today() {
    // This assertion will be flipped by the typed-row change.
    assert!(
        context()
            .actions(
                &parse(r#"{"payload":{"type":"task_started","turn_id":"missing"}}"#),
                "id",
                1.
            )
            .expect("actions")
            .is_empty()
    );
}

#[test]
fn rollout_non_string_type_is_ignored_today() {
    // This assertion will be flipped by the typed-row change.
    assert!(
        context()
            .actions(
                &parse(r#"{"type":7,"payload":{"type":"task_started"}}"#),
                "id",
                1.
            )
            .expect("actions")
            .is_empty()
    );
}

#[test]
fn rollout_duplicate_type_uses_last_value_today() {
    // This assertion will be flipped by the typed-row change.
    let value = parse(
        r#"{"type":"other","type":"event_msg","payload":{"type":"task_started","turn_id":"duplicate"}}"#,
    );
    assert_eq!(
        context().actions(&value, "id", 1.).expect("actions"),
        vec![RolloutAction::Event {
            method: "turn/started".into(),
            params: json!({"threadId":"thread","turnId":"duplicate","turn":{"id":"duplicate"},"_analyticsTimestampSource":"fileModifiedEstimate"}),
            at: 1.
        }]
    );
}

#[test]
fn rollout_characterizes_usage_and_compaction_replay() {
    let mut context = RolloutContext::from_record(
        json!({"threadId":"thread","turnId":"turn","model":"model","window":8192}),
    )
    .expect("context");
    let actions: Vec<_> = include_str!("fixtures/rollout/usage.jsonl")
        .lines()
        .enumerate()
        .flat_map(|(i, line)| {
            context
                .actions(&parse(line), &format!("usage{i}"), 100.)
                .expect("actions")
        })
        .collect();
    let tokens = json!({"inputTokens":10,"cachedInputTokens":2,"outputTokens":3,"reasoningOutputTokens":1,"totalTokens":13});
    let response = |id: &str, at| RolloutAction::Event {
        method: "thread/tokenUsage/updated".into(),
        params: json!({"_analyticsTimestampSource":"record","threadId":"thread","turnId":"turn","studioUsage":{"evidence":"recovered","kind":"response","model":"model","modelContextWindow":8192,"responseId":id,"tokens":{"cacheReadInput":2,"cacheWriteInput":0,"output":3,"reasoningOutput":1,"uncachedInput":8},"turnId":"turn"}}),
        at,
    };
    assert_eq!(
        actions,
        vec![
            response("response", 10.),
            RolloutAction::Event {
                method: "thread/tokenUsage/updated".into(),
                params: json!({"_analyticsTimestampSource":"record","_analyticsUsageKind":"notice","requestUsageAvailable":true,"responseId":"response","threadId":"thread","turnId":"turn","usageSource":"tokenCount","tokenUsage":{"last":tokens,"total":tokens,"modelContextWindow":8192}}),
                at: 11.
            },
            RolloutAction::Event {
                method: "analytics/compaction".into(),
                params: json!({"_analyticsTimestampSource":"record","compaction_response_id":null,"first_window_id":null,"id":"usage2","previous_window_id":null,"threadId":"thread","turnId":"turn","window_id":null,"window_number":null}),
                at: 12.
            },
            response("replayed", 12.),
        ]
    );
}

#[test]
fn rollout_header_rejects_duplicate_type_even_when_last_value_is_session_meta() {
    let checkpoint =
        batch("{\"type\":\"other\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread\"}}\n");
    assert_eq!(checkpoint["status"], "wrongThread");
    assert_eq!(checkpoint["importedRecords"], 0);
}

#[tokio::test]
async fn rollout_imports_history_entries() {
    let root = tempfile::tempdir().expect("state");
    let agent = crate::analytics_fixtures::seed_state(root.path()).await;
    let path = root.path().join("rollout.jsonl");
    std::fs::write(&path, include_str!("fixtures/rollout/records.jsonl")).expect("fixture");
    let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
        .await
        .expect("store");
    let batch = analytics_history::prepare(
        HistorySnapshot::from_records(agent, None, LedgerEpoch::Current).expect("snapshot"),
        root.path(),
        RolloutPath::Found(path),
        BatchLimits::default(),
        100.,
        &AtomicBool::new(false),
    )
    .expect("batch");
    batch.commit(&store, 100.).await.expect("commit");
    let db = rusqlite::Connection::open(root.path().join("analytics.sqlite3")).expect("database");
    let mut query = db
        .prepare("SELECT record FROM analytics_items ORDER BY rowid")
        .expect("query");
    let rows = query
        .query_map([], |row| row.get::<_, String>(0))
        .expect("rows")
        .map(|r| parse(&r.expect("record")))
        .collect::<Vec<_>>();
    let projection: Vec<_> = rows.iter().map(|row| json!({"type":row["type"],"category":row["category"],"role":row["role"],"at":row["at"],"durationMs":row["durationMs"],"input":row["input"],"output":row["output"],"source":row["source"]})).collect();
    let metric = |n| json!({"bytes":n,"chars":n,"format":"text","imageBytes":0,"imageCount":0,"images":[],"lines":1});
    assert_eq!(
        projection,
        vec![
            json!({"type":"modelMessage","category":"message","role":"assistant","at":4.0,"durationMs":null,"input":null,"output":metric(9),"source":"rollout"}),
            json!({"type":"compactionSnapshot","category":null,"role":null,"at":5.0,"durationMs":null,"input":null,"output":null,"source":"rollout"}),
            json!({"type":"modelMessage","category":"compactionReplacement","role":"user","at":5.0,"durationMs":null,"input":metric(11),"output":null,"source":"rollout"}),
            json!({"type":"modelMessage","category":"compactionGuardian","role":"assistant","at":5.0,"durationMs":null,"input":null,"output":metric(8),"source":"rollout"}),
            json!({"type":"agentMessage","category":null,"role":null,"at":6.0,"durationMs":1500.0,"input":null,"output":metric(9),"source":"rollout"}),
        ]
    );
}
