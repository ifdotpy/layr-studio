#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;
use studio_json::json;

#[test]
fn layr_read_checks_argument_rules() {
    for args in [
        json!(["show", "head:file"]),
        json!(["diff", "a", "b"]),
        json!(["log"]),
        json!(["blame", "file"]),
        json!(["grep", "x"]),
        json!(["ls-files"]),
        json!(["merge-base", "a", "b"]),
        json!(["rev-parse", "head"]),
        json!(["status"]),
        json!(["diff", "x".repeat(MAX_ARGUMENT_CHARS)]),
        Value::Array(std::iter::repeat_n(json!("status"), MAX_ARGUMENTS).collect()),
        json!(["change", "list"]),
        json!(["change", "show", "x"]),
        json!(["change", "log", "x"]),
        json!(["change", "diff", "x"]),
        json!(["attestations", "head"]),
        json!(["view", "show", "x"]),
        json!(["--line", "main", "log"]),
        json!(["try", "a".repeat(32), "--", "layr", "retest", "run"]),
        json!(["try", "a".repeat(32), "--", "layr", "bisect", "run"]),
    ] {
        assert_eq!(
            command(&json!({"args": args})).unwrap().argv(),
            args.as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        );
    }
    for args in [
        json!([]),
        json!(["status", 1]),
        json!(["show", "a\u{0}b"]),
        json!(["diff", "x".repeat(MAX_ARGUMENT_CHARS + 1)]),
        Value::Array(std::iter::repeat_n(json!("status"), MAX_ARGUMENTS + 1).collect()),
        json!(["--line"]),
        json!(["--line", "main"]),
        json!(["--line", "a", "--line", "b", "log"]),
        json!(["change", "create"]),
        json!(["change", "comment"]),
        json!(["view", "save"]),
        json!(["attest"]),
        json!(["save"]),
        json!(["retest", "run"]),
        json!(["bisect", "run"]),
        json!(["try", "head", "--", "true"]),
        json!(["try", "a".repeat(32), "--"]),
        json!(["--line", "main", "try", "a".repeat(32), "--", "true"]),
    ] {
        assert!(command(&json!({"args": args})).is_err(), "{args}");
    }
}

#[test]
fn request_identity_is_stable_and_scoped() {
    assert_eq!(identity("actor", "request"), identity("actor", "request"));
    assert_ne!(identity("actor", "request"), identity("other", "request"));
    assert_ne!(identity("actor", "request"), identity("actor", "other"));
}

#[cfg(unix)]
#[test]
fn parsed_reads_reach_the_layr_executable() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let binary = temp.path().join("layr");
    let record = temp.path().join("argv");
    std::fs::write(&binary, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$RECORD\"\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    for args in [
        json!(["show", "head:example.txt"]),
        json!(["diff", "head", "head"]),
        json!(["change", "list"]),
    ] {
        let request = command(&json!({"args": args})).unwrap();
        let argv = request.argv();
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        crate::bounded_child::run(crate::bounded_child::Request {
            program: binary.as_os_str(),
            git_directory: None,
            args: &args,
            environment: &[("RECORD", record.to_str().unwrap())],
            input: None,
            stdout_limit: 1024,
            stderr_limit: 1024,
            timeout: std::time::Duration::from_secs(5),
        })
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&record).unwrap(),
            format!("{}\n", argv.join("\n"))
        );
    }
}

#[test]
fn tool_is_offered_only_for_layr_assignments() {
    use studio_runtime_schema::review::{
        GitCommit, GitRef, LayrBase, LayrStateId, ReviewAssignment, ReviewTarget, WorkspaceMode,
    };
    let layr = ReviewAssignment::new(
        WorkspaceMode::LayrNative,
        ReviewTarget::LayrState {
            state: LayrStateId::try_from("a".repeat(32)).unwrap(),
            base: LayrBase::MergeBaseWithMain,
        },
    )
    .unwrap();
    let git = ReviewAssignment::new(
        WorkspaceMode::Worktree,
        ReviewTarget::GitRange {
            base: GitRef::try_from("main".to_owned()).unwrap(),
            head: GitCommit::try_from("b".repeat(40)).unwrap(),
        },
    )
    .unwrap();
    assert!(tool_definition_for(&layr).is_some());
    assert!(tool_definition_for(&git).is_none());
}

#[test]
fn try_classifies_retest_and_bisect_inside_the_sandbox() {
    let state = "a".repeat(32);
    let retest = command(&json!({"args":["try", state, "--", "layr", "retest", "run"]})).unwrap();
    assert!(matches!(
        retest.command,
        LayrReadCommand::Try {
            argv: TryCommand::Retest(_),
            ..
        }
    ));
    let bisect = command(&json!({"args":["try", state, "--", "layr", "bisect", "run"]})).unwrap();
    assert!(matches!(
        bisect.command,
        LayrReadCommand::Try {
            argv: TryCommand::Bisect(_),
            ..
        }
    ));
}
#[cfg(unix)]
#[tokio::test]
async fn native_runner_dispatches_reads_and_replays_exact_requests() {
    let directory = tempfile::tempdir().unwrap();
    let fixture =
        crate::layr_native::review_fixture::FakeReviewer::new(directory.path(), "reviewer");
    let actor = Actor::from_record(
        &fixture.record,
        "reviewer",
        fixture.record["epoch"].as_u64().unwrap(),
    )
    .unwrap();
    let assignment = actor.record().review_assignment.as_ref().unwrap();
    let store = studio_runtime_store::RuntimeStore::open(Some(directory.path()))
        .await
        .unwrap();
    let commands = crate::layr_native::store_path(directory.path()).join("commands");
    let before = std::fs::read_to_string(&commands).unwrap();
    for (index, args) in [
        json!(["show", "head:example.txt"]),
        json!(["diff", "head", "head"]),
        json!(["change", "list"]),
    ]
    .into_iter()
    .enumerate()
    {
        let request = format!("request-{index}");
        let first = run_native(&store, &actor, assignment, &json!({"args":args}), &request).await;
        assert!(
            matches!(first, DispatchedOperation::Applied(_)),
            "{:?}",
            first.result().test_value()
        );
        let retry = run_native(&store, &actor, assignment, &json!({"args":args}), &request).await;
        assert_eq!(first.result().test_value(), retry.result().test_value());
    }
    let after = std::fs::read_to_string(&commands).unwrap();
    let new = after.strip_prefix(&before).unwrap();
    let lines: Vec<_> = new.lines().collect();
    assert_eq!(lines.len(), 3, "{new}");
    assert!(lines[0].ends_with("show head:example.txt"));
    assert!(lines[1].ends_with("diff head head"));
    assert!(lines[2].ends_with("change list"));
    let refused = run_native(
        &store,
        &actor,
        assignment,
        &json!({"args":["view", "save"]}),
        "write",
    )
    .await;
    assert!(matches!(refused, DispatchedOperation::NotSent(_)));
    assert_eq!(std::fs::read_to_string(&commands).unwrap(), after);
}
