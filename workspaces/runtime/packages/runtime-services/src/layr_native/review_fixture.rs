//! Shared fake Layr reviewer for crate-internal tests.
use super::*;
use std::fs;

pub(crate) struct FakeReviewer {
    pub record: Value,
    _files: tempfile::TempDir,
    _binary: capability::BinaryOverride,
}

impl FakeReviewer {
    /// Prepare a reviewer at a named state using the same owner as a live worker.
    #[cfg(unix)]
    pub fn new(state: &Path, id: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let files = tempfile::tempdir().unwrap();
        let source = files.path().join("source");
        fs::create_dir_all(state).unwrap();
        fs::create_dir_all(&source).unwrap();
        let binary = files.path().join("layr");
        fs::write(&binary, r##"#!/bin/sh
line_main=$2
printf '%s\n' "$*" >> "$LAYR_ROOT/commands"
case "$*" in
 *store-version*) printf '%s\n' '{"binary":{"format":1,"version":"0.14.2","build":"fixture"},"store":null,"compatible":true,"state":"compatible"}' ;;
 *' init '*) while [ "$1" != "init" ]; do shift; done; mkdir -p "$LAYR_ROOT/projects/$2/lines/main" ;;
 *' line new '*) while [ "$1" != "line" ]; do shift; done; mkdir -p "$line_main/../$3" ;;
 *' rev-parse '*) printf '%s\n' 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' ;;
 *' merge-base '*) printf '%s\n' 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' ;;
 *' diff '*) printf '%s\n' 'fixture review diff' ;;
esac
"##).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        let guard = override_layr_binary(binary);
        #[cfg(target_os = "linux")]
        let daemon = serve_daemon(state);
        let mut lead = fixture_agent("lead", true);
        configure_new_chat(
            &mut lead,
            &crate::project_path::ProjectPath::from_stored(source.to_str().unwrap()),
        );
        let mut parent = lead.value();
        for (key, value) in runtime::prepare_files(state, &parent)
            .unwrap()
            .as_object()
            .unwrap()
        {
            parent[key] = value.clone();
        }
        #[cfg(target_os = "linux")]
        {
            daemon.join().unwrap();
            fs::remove_file(selected_socket_path(state).unwrap()).unwrap();
        }
        #[cfg(target_os = "linux")]
        let daemon = serve_daemon(state);
        let parent = studio_runtime_store::AgentRecord::from_value(parent).unwrap();
        let mut reviewer = fixture_agent(id, false);
        reviewer.role = studio_runtime_schema::queue_command::Field::Value(
            studio_runtime_schema::creation_intent::AgentRole::Reviewer,
        );
        configure_child(&mut reviewer, &parent, &parent);
        let mut record = reviewer.value();
        record["layrBaseState"] = json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        record["reviewAssignment"] = json!({"kind":"layrState", "state":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "base":{"kind":"mergeBaseWithMain"}});
        for (key, value) in runtime::prepare_files(state, &record)
            .unwrap()
            .as_object()
            .unwrap()
        {
            record[key] = value.clone();
        }
        #[cfg(target_os = "linux")]
        {
            daemon.join().unwrap();
            fs::remove_file(selected_socket_path(state).unwrap()).unwrap();
        }
        Self {
            record,
            _files: files,
            _binary: guard,
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn serve_daemon(state: &Path) -> std::thread::JoinHandle<()> {
    let socket = selected_socket_path(state).unwrap();
    fs::create_dir_all(socket.parent().unwrap()).unwrap();
    versions::serve_daemon_identity(
        &socket,
        br#"{"format":1,"version":"0.14.2","build":"fixture"}"#,
    )
}
