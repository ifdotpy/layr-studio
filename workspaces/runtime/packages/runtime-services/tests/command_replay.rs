//! Replays recorded management commands through the pure `decide_with_effects` function.
//! Each case is `command_replay/<case>.json` with `records`, `action` and `now`;
//! its expected outcome is `command_replay/<case>.expected.json`.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use studio_runtime_services::agent_management::{
    Action, ManagementSnapshot, Records, decide_with_effects,
};

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).expect("replay fixture");
    serde_json::from_str(&text).expect("replay fixture JSON")
}

/// Canonical outcome: writes and cancel_pending sorted by agent id, every
/// lifecycle effect in emission order with its full contents, and the result.
fn outcome(name: &str, records: Records, action: &Action, now: f64) -> Value {
    let snapshot = ManagementSnapshot::from_records(records)
        .unwrap_or_else(|error| panic!("{name}: snapshot: {error}"));
    let decision = decide_with_effects(&snapshot, action, now)
        .unwrap_or_else(|error| panic!("{name}: refused: {error}"));
    let mut writes: Vec<Value> = decision
        .writes
        .iter()
        .map(|write| json!({"table": write.table, "record": write.record}))
        .collect();
    writes.sort_by_key(|write| write["record"]["id"].to_string());
    let mut cancel_pending = decision.cancel_pending;
    cancel_pending.sort();
    json!({
        "writes": writes,
        "cancel_pending": cancel_pending,
        "effects": serde_json::to_value(&decision.effects).expect("effects"),
        "result": decision.result,
    })
}

#[test]
fn recorded_management_commands_replay_through_decide() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/command_replay");
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
        let case = read_json(&path);
        let expected = read_json(&path.with_file_name(format!("{stem}.expected.json")));
        let records: Records =
            serde_json::from_value(case["records"].clone()).expect("replay records");
        let action: Action = serde_json::from_value(case["action"].clone()).expect("replay action");
        let now = case["now"].as_f64().expect("replay time");
        assert_eq!(outcome(&name, records, &action, now), expected, "{name}");
    }
}
