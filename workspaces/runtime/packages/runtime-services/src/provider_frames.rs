//! Raw provider frames, in and out, go to the rotating `provider-frames.jsonl` in the
//! state folder (#630 item 114). They help debugging and replay; they are not a source
//! of state and never touch the store writer. The log has its own writer thread; a
//! full queue drops a line instead of slowing the session.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use studio_json::{Value, json};
use studio_runtime_process::operational_log::OperationalLog;
use studio_runtime_process::provider_seam::JournalPayload;

/// The one frame log of `state_dir`; every session of this process shares its writer.
/// None when the file cannot be opened: the frames are only for debugging.
#[must_use]
pub fn for_state_dir(state_dir: &Path) -> Option<OperationalLog> {
    static LOGS: OnceLock<Mutex<HashMap<PathBuf, OperationalLog>>> = OnceLock::new();
    let mut logs = LOGS.get_or_init(Default::default).lock().ok()?;
    if let Some(log) = logs.get(state_dir) {
        return Some(log.clone());
    }
    let log = OperationalLog::provider_frames(state_dir).ok()?;
    logs.insert(state_dir.to_path_buf(), log.clone());
    Some(log)
}

/// The log line of a frame Studio writes to the provider.
#[must_use]
pub fn outgoing(session: &str, clock_ms: u64, message: &Value) -> Vec<u8> {
    line(json!({"clock": clock_ms, "dir": "out", "session": session, "frame": message}))
}

/// The log line of a journal entry the provider sent; None for entries that are not
/// stdout (stderr and exit already have their own records).
#[must_use]
pub fn incoming(
    session: &str,
    clock_ms: u64,
    cursor: u64,
    payload: &JournalPayload,
) -> Option<Vec<u8>> {
    let record = match payload {
        JournalPayload::Frame(frame) => {
            let encoded = studio_runtime_providers::encode_line(frame).ok()?;
            let frame: Value = studio_json::from_slice(&encoded).ok()?;
            json!({"clock": clock_ms, "dir": "in", "session": session, "cursor": cursor, "frame": frame})
        }
        JournalPayload::RawStdout { bytes, .. } => json!({
            "clock": clock_ms, "dir": "in", "session": session, "cursor": cursor,
            "raw": String::from_utf8_lossy(bytes),
        }),
        JournalPayload::Stderr(_) | JournalPayload::Exit(_) | JournalPayload::Other { .. } => {
            return None;
        }
    };
    Some(line(record))
}

/// One JSON line. `at` is wall time in Unix milliseconds; `clock` is the session clock
/// that orders the session's own deadlines.
fn line(mut record: Value) -> Vec<u8> {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    record["at"] = json!(at);
    let mut line = record.to_string().into_bytes();
    line.push(b'\n');
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_runtime_providers::{DecodedJournalPayload, decode_journal_entry};

    #[test]
    fn lines_keep_the_frame_and_its_direction() {
        let out = outgoing("account:a", 5, &json!({"id":1,"method":"turn/start"}));
        let mut out: Value = studio_json::from_slice(&out).expect("json line");
        assert!(out["at"].as_u64().is_some_and(|at| at > 0));
        out.as_object_mut().expect("object").remove("at");
        assert_eq!(
            out,
            json!({"clock":5,"dir":"out","session":"account:a","frame":{"id":1,"method":"turn/start"}})
        );
        let decoded = decode_journal_entry(
            3,
            1,
            br#"{"method":"turn/started","params":{"turn":{"id":"t"}}}"#.to_vec(),
        );
        let DecodedJournalPayload::Frame(frame) = decoded.payload else {
            panic!("frame");
        };
        let line = incoming("account:a", 6, 3, &JournalPayload::Frame(frame)).expect("stdout line");
        let line: Value = studio_json::from_slice(&line).expect("json line");
        assert_eq!(line["dir"], "in");
        assert_eq!(line["cursor"], 3);
        assert_eq!(line["frame"]["method"], "turn/started");
        assert!(incoming("account:a", 6, 4, &JournalPayload::Stderr(json!("x"))).is_none());
    }

    #[test]
    fn one_writer_per_state_folder() {
        let root = tempfile::tempdir().expect("root");
        let first = for_state_dir(root.path()).expect("log");
        first.write_line(&outgoing("account:a", 1, &json!({"id":1})));
        first.flush().expect("flush");
        let second = for_state_dir(root.path()).expect("same log");
        second.flush().expect("flush");
        let text =
            std::fs::read_to_string(root.path().join("provider-frames.jsonl")).expect("file");
        assert_eq!(text.lines().count(), 1);
    }
}
