//! Replays recorded management commands through the pure `decide_with_effects` function.
//! Each case is `command_replay/<case>.json` with `records`, `action` and `now`;
//! its expected outcome is `command_replay/<case>.expected.json`.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use studio_json::{Value, json};
use studio_runtime_services::agent_management::{
    Action, ManagementSnapshot, Records, decide_with_effects,
};

#[derive(Deserialize)]
struct Case {
    records: Records,
    action: Action,
    now: f64,
}

fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path).expect("replay fixture")
}

/// Canonical outcome: writes and cancel_pending sorted by agent id, every
/// lifecycle effect in emission order with its full contents, and the result.
fn outcome(name: &str, case: Case) -> Value {
    let snapshot = ManagementSnapshot::from_records(case.records)
        .unwrap_or_else(|error| panic!("{name}: snapshot: {error}"));
    let decision = decide_with_effects(&snapshot, &case.action, case.now)
        .unwrap_or_else(|error| panic!("{name}: refused: {error}"));
    let mut writes: Vec<Value> = decision
        .writes
        .iter()
        .map(|write| json!({"table": write.table, "record": write.record}))
        .collect();
    writes.sort_by_key(|write| write["record"]["id"].to_string());
    let mut cancel_pending = decision.cancel_pending;
    cancel_pending.sort();
    let requeue: Vec<Value> = decision
        .requeue
        .iter()
        .map(|requeue| {
            json!({"agent":requeue.agent,"from_epoch":requeue.from_epoch,
                "to_epoch":requeue.to_epoch,"interrupted":requeue.interrupted,
                "reason":requeue.reason})
        })
        .collect();
    let effects: Vec<Value> = decision
        .effects
        .iter()
        .map(|effect| {
            json!({
                "kind": effect.kind(),
                "before": effect.before(),
                "stopped": effect.stopped(),
                "at": effect.at(),
            })
        })
        .collect();
    let mut outcome = json!({
        "writes": writes,
        "cancel_pending": cancel_pending,
        "effects": effects,
        "result": decision.result,
    });
    if !requeue.is_empty() {
        outcome["requeue"] = json!(requeue);
    }
    outcome
}

#[test]
fn recorded_management_commands_replay_through_decide() {
    let directory =
        Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"))
            .join("tests/command_replay");
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&directory)
        .expect("replay fixture directory")
        .map(|entry| entry.expect("replay fixture entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
                && !path.to_string_lossy().ends_with(".expected.json")
        })
        .collect();
    cases.sort();
    assert!(cases.len() >= 5, "replay cases are missing");
    for path in cases {
        let name = path
            .file_name()
            .expect("case name")
            .to_string_lossy()
            .into_owned();
        let stem = path
            .file_stem()
            .expect("case stem")
            .to_string_lossy()
            .into_owned();
        let case: Case = studio_json::from_str(&read_text(&path)).expect("replay case");
        let expected: Value = studio_json::from_str(&read_text(
            &path.with_file_name(format!("{stem}.expected.json")),
        ))
        .expect("replay expected outcome");
        assert_eq!(outcome(&name, case), expected, "{name}");
    }
}
