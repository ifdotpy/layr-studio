//! Host Layr workspaces. The Studio state directory owns the store and guard.
use std::path::{Path, PathBuf};
use studio_json::{Value, json};
use studio_runtime_store::path_identity::CanonicalPath;

pub use studio_runtime_store::NATIVE_LAYR_DIRECTORY;
/// Store directory below the native installation directory, outside project checkouts.
pub const STORE_DIRECTORY: &str = "store";
/// Length of a SHA-256 project ID, in hexadecimal bytes.
pub(super) const PROJECT_ID_HEX_BYTES: usize = 64;

pub fn store_path(state_dir: &Path) -> PathBuf {
    state_dir.join(NATIVE_LAYR_DIRECTORY).join(STORE_DIRECTORY)
}

pub fn binary_path(state_dir: &Path) -> PathBuf {
    state_dir.join(NATIVE_LAYR_DIRECTORY).join("bin/layr")
}

/// Dedicated Linux root-daemon socket for this Studio state directory.
pub fn socket_path(state_dir: &Path) -> PathBuf {
    state_dir.join(NATIVE_LAYR_DIRECTORY).join("layr.sock")
}

pub fn selected_socket_path(state_dir: &Path) -> Result<PathBuf, NativeLayrError> {
    Ok(socket_path_in_store(
        state_dir,
        &settings::selected_store(state_dir)?,
    ))
}

fn socket_path_in_store(state_dir: &Path, root: &Path) -> PathBuf {
    settings::metadata(state_dir, root).join("layr.sock")
}

pub fn is_native(record: &Value) -> bool {
    record["workspaceMode"] == "layr-native"
}

/// Creation records intent only. `workspace_ready_git` owns filesystem effects.
pub(crate) fn configure_new_chat(
    record: &mut studio_runtime_store::AgentRecord,
    source: &crate::project_path::ProjectPath,
) {
    record.workspace_mode = Some(studio_runtime_schema::records::WorkspaceMode::LayrNative);
    record.execution_mode = Some(studio_runtime_store::AgentExecutionMode::Native);
    record.host_project_path =
        studio_runtime_schema::queue_command::Field::Value(source.as_str().to_owned());
    record.layr_ready = Some(false);
    record.worktree = Some(false);
    record.worktree_ready = Some(false);
    record.workspace_in_git = Some(false);
}

pub fn configure_child(
    record: &mut studio_runtime_store::AgentRecord,
    parent: &studio_runtime_store::AgentRecord,
    root: &studio_runtime_store::AgentRecord,
) {
    let source = root
        .host_project_path
        .value()
        .map(String::as_str)
        .or(root.cwd.as_deref())
        .unwrap_or_default();
    configure_new_chat(
        record,
        &crate::project_path::ProjectPath::from_stored(source),
    );
    record.cwd = parent.cwd.clone();
    if root.layr_project_id.is_some() {
        record.layr_project_id.clone_from(&root.layr_project_id);
    }
    if root.layr_store_root.is_some() {
        record.layr_store_root.clone_from(&root.layr_store_root);
    }
    if record.layr_store_root.is_none()
        && root.layr_ready == Some(true)
        && let Some(cwd) = root.layr_read_cwd.as_deref().or(root.cwd.as_deref())
        && let Some(store) = settings::root_from_cwd(Path::new(cwd))
    {
        record.layr_store_root = Some(store.to_string_lossy().into_owned());
    }
}

#[cfg(test)]
fn fixture_agent(id: &str, lead: bool) -> studio_runtime_store::AgentRecord {
    let mut record = studio_runtime_store::AgentRecord::test_defaults();
    record.id = id.into();
    record.is_lead = Some(lead);
    record.auto_wake = Some(true);
    record
}

#[cfg(test)]
fn fixture_native_parent(value: &Value) -> studio_runtime_store::AgentRecord {
    use studio_runtime_schema::queue_command::Field;
    let mut record = fixture_agent(
        value["id"].as_str().unwrap_or_default(),
        value["isLead"] == true,
    );
    record.cwd = value["cwd"].as_str().map(str::to_owned);
    record.host_project_path = value["hostProjectPath"]
        .as_str()
        .map(str::to_owned)
        .map_or(Field::Missing, Field::Value);
    record.layr_project_id = value["layrProjectId"].as_str().map(Into::into);
    record.layr_store_root = value["layrStoreRoot"].as_str().map(str::to_owned);
    record.layr_read_cwd = value["layrReadCwd"].as_str().map(Into::into);
    record.layr_ready = value["layrReady"].as_bool();
    record
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_creation_preserves_host_execution_and_source_identity() {
        let mut record = studio_runtime_store::AgentRecord::test_defaults();
        record.cwd = Some("/code".into());
        record.is_lead = Some(true);
        configure_new_chat(
            &mut record,
            &crate::project_path::ProjectPath::from_stored("/code"),
        );
        assert_eq!(
            record.workspace_mode,
            Some(studio_runtime_schema::records::WorkspaceMode::LayrNative)
        );
        assert_eq!(
            record.execution_mode,
            Some(studio_runtime_store::AgentExecutionMode::Native)
        );
        assert_eq!(
            record.host_project_path.value().map(String::as_str),
            Some("/code")
        );
        assert_eq!(record.layr_ready, Some(false));
        assert_eq!(
            store_path(Path::new("/state")),
            Path::new("/state/layr-native/store")
        );
        assert_eq!(
            binary_path(Path::new("/state")),
            Path::new("/state/layr-native/bin/layr")
        );
    }

    #[test]
    fn native_child_inherits_project_and_keeps_reviewer_role() {
        let mut root = studio_runtime_store::AgentRecord::test_defaults();
        root.cwd = Some("/store/main".into());
        root.host_project_path = studio_runtime_schema::queue_command::Field::Value("/code".into());
        root.layr_project_id = Some("project".into());
        let mut child = studio_runtime_store::AgentRecord::test_defaults();
        child.role = studio_runtime_schema::queue_command::Field::Value(
            studio_runtime_schema::creation_intent::AgentRole::Reviewer,
        );
        configure_child(&mut child, &root, &root);
        assert_eq!(
            child.role.value(),
            Some(&studio_runtime_schema::creation_intent::AgentRole::Reviewer)
        );
        assert_eq!(
            child.host_project_path.value().map(String::as_str),
            Some("/code")
        );
        assert_eq!(child.layr_project_id.as_deref(), Some("project"));
        assert_eq!(child.worktree, Some(false));
    }
}

pub mod capability;
mod git_guard;
pub mod mapping;
pub mod settings;
pub mod versions;
#[cfg(test)]
mod warm_store_tests;
pub use capability::{NativeLayrCapability, capability, resolve_binary};
#[cfg(test)]
fn test_binary() -> Option<std::ffi::OsString> {
    std::env::var_os("STUDIO_LAYR_NATIVE_TEST_BINARY")
}
#[cfg(test)]
pub(crate) fn override_layr_binary(binary: PathBuf) -> capability::BinaryOverride {
    capability::override_binary(binary)
}
mod failed_import;
mod private_tree;
#[cfg(test)]
pub(crate) mod review_fixture;
mod runtime;
pub use runtime::{
    NativeLayrError, agent_value, evidence, main_path, merge, operation, prepare, project_id,
    remove, run, save, save_turn,
};

/// Thread commands must resolve Studio's host binary before any user Cargo installation.
pub fn thread_environment(state_root: &Path) -> studio_json::Map<String, Value> {
    thread_environment_in_store(state_root, &store_path(state_root))
}

pub fn thread_environment_for_record(
    state_root: &Path,
    record: &Value,
) -> Result<studio_json::Map<String, Value>, NativeLayrError> {
    Ok(thread_environment_in_store(
        state_root,
        &settings::record_store(state_root, record)?,
    ))
}

pub(crate) fn thread_environment_in_store(
    state_root: &Path,
    root: &Path,
) -> studio_json::Map<String, Value> {
    let bin = binary_path(state_root);
    let path = format!(
        "{}:{}",
        bin.parent().unwrap_or(Path::new("/")).display(),
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())
    );
    let mut environment =
        transport_environment_in_store(state_root, root, cfg!(target_os = "linux"));
    environment.insert("PATH".into(), json!(path));
    environment
}

/// The state of the folder overlay at `folder` in the line at `cwd` (`overlay list --json`).
#[cfg(test)]
pub(crate) fn overlay_folder_state(state: &Path, cwd: &Path, folder: &str) -> Option<String> {
    let overlays: Value = studio_json::from_str(
        &runtime::run(state, cwd, &["overlay", "list", "--json"]).expect("overlay list"),
    )
    .expect("overlay list JSON");
    overlays
        .as_array()?
        .iter()
        .find(|overlay| overlay["folder"].as_str() == Some(folder))
        .and_then(|overlay| overlay["state"].as_str().map(str::to_owned))
}

#[cfg(test)]
fn transport_environment(state_root: &Path, uses_service: bool) -> studio_json::Map<String, Value> {
    transport_environment_in_store(state_root, &store_path(state_root), uses_service)
}

fn transport_environment_in_store(
    state_root: &Path,
    root: &Path,
    uses_service: bool,
) -> studio_json::Map<String, Value> {
    let mut environment = studio_json::Map::from_iter([
        ("LAYR_ROOT".into(), json!(root)),
        (
            "LAYR_DIRECT".into(),
            json!(if uses_service { "0" } else { "1" }),
        ),
    ]);
    if uses_service {
        environment.insert(
            "LAYR_SOCKET".into(),
            json!(socket_path_in_store(state_root, root)),
        );
    }
    environment
}
mod import;
mod inventory;
pub mod migration;
mod switch;
pub use switch::switch_chat;

#[cfg(test)]
mod receipt_tests {
    use super::*;

    #[test]
    fn linux_native_environment_selects_the_owned_privileged_service() {
        let env = transport_environment(Path::new("/studio"), true);
        assert_eq!(env["LAYR_ROOT"], "/studio/layr-native/store");
        assert_eq!(env["LAYR_DIRECT"], "0");
        assert_eq!(env["LAYR_SOCKET"], "/studio/layr-native/layr.sock");
    }

    #[test]
    fn completed_operations_replay_without_repeating_the_effect() {
        let state = tempfile::tempdir().expect("fixture operation succeeds");
        let intent = json!({"line":"worker"});
        let first = operation(state.path(), "create-worker", &intent, || {
            Ok(json!({"stateId":"full-state"}))
        })
        .expect("fixture operation succeeds");
        let again = operation(state.path(), "create-worker", &intent, || {
            panic!("must not repeat")
        })
        .expect("fixture operation succeeds");
        assert_eq!(first, again);
        assert!(
            operation(
                state.path(),
                "create-worker",
                &json!({"line":"other"}),
                || panic!("changed identity must not run")
            )
            .is_err()
        );
    }

    #[test]
    fn unknown_operations_stay_held_after_a_lost_result() {
        let state = tempfile::tempdir().expect("fixture operation succeeds");
        let intent = json!({"state":"reviewed"});
        assert!(matches!(
            operation(state.path(), "merge", &intent, || Err(
                NativeLayrError::Unknown("lost response".into())
            )),
            Err(NativeLayrError::Unknown(_))
        ));
        assert!(matches!(
            operation(state.path(), "merge", &intent, || panic!(
                "unknown merge must not repeat"
            )),
            Err(NativeLayrError::Unknown(_))
        ));
    }

    #[test]
    fn explicit_failures_replay_the_same_refusal() {
        let state = tempfile::tempdir().expect("fixture operation succeeds");
        let intent = json!({"state":"reviewed"});
        assert!(
            operation(state.path(), "merge", &intent, || Err(
                NativeLayrError::Refused("line moved".into())
            ))
            .is_err()
        );
        let result = operation(state.path(), "merge", &intent, || {
            panic!("failed merge must not repeat")
        });
        assert!(
            result
                .expect_err("operation must fail")
                .to_string()
                .contains("line moved")
        );
    }

    #[test]
    fn project_identity_is_shared_by_migration_and_chats_with_a_logical_binding() {
        let mut record = json!({"hostProjectPath":"/project"});
        let before = project_id(&record).expect("fixture operation succeeds");
        record["projectId"] = json!("logical-project");
        assert_eq!(
            project_id(&record).expect("fixture operation succeeds"),
            before
        );
    }

    #[test]
    fn native_environment_selects_the_managed_binary() {
        let env = thread_environment(Path::new("/studio"));
        assert_eq!(env["LAYR_ROOT"], "/studio/layr-native/store");
        assert_eq!(
            env["LAYR_DIRECT"],
            if cfg!(target_os = "linux") { "0" } else { "1" }
        );
        assert!(
            env["PATH"]
                .as_str()
                .expect("fixture operation succeeds")
                .starts_with("/studio/layr-native/bin:")
        );
    }

    #[test]
    #[ignore = "requires the installed Layr host binary and APFS or btrfs"]
    fn real_host_import_worker_merge_archive_and_warm_caches() {
        let _binary_override = crate::layr_native::override_layr_binary(std::path::PathBuf::from(
            crate::layr_native::test_binary().expect("test binary"),
        ));
        use std::{fs, path::PathBuf};
        let binary = PathBuf::from(
            crate::layr_native::test_binary().expect("Set STUDIO_LAYR_NATIVE_TEST_BINARY"),
        );
        let state = tempfile::tempdir().expect("fixture operation succeeds");
        let source = tempfile::tempdir().expect("fixture operation succeeds");
        fs::write(source.path().join("code.txt"), "base\n").expect("fixture operation succeeds");
        fs::write(source.path().join(".gitignore"), "node_modules/\n.env\n")
            .expect("fixture operation succeeds");
        fs::write(
            source.path().join(".layrattributes"),
            "node_modules regenerable\n",
        )
        .expect("fixture operation succeeds");
        fs::write(source.path().join(".env"), "TEST_VALUE=local\n")
            .expect("fixture operation succeeds");
        fs::create_dir(source.path().join("node_modules")).expect("fixture operation succeeds");
        fs::write(source.path().join("node_modules/old"), "warm import\n")
            .expect("fixture operation succeeds");
        let git = |args: &[&str]| {
            crate::bounded_child::run(crate::bounded_child::Request {
                program: std::ffi::OsStr::new("git"),
                git_directory: Some(source.path().as_os_str()),
                args,
                environment: &[],
                input: None,
                stdout_limit: 64 * 1024,
                stderr_limit: 64 * 1024,
                timeout: std::time::Duration::from_secs(60),
            })
            .expect("fixture Git command succeeds");
        };
        git(&["init", "-q"]);
        git(&["-c", "core.hooksPath=/dev/null", "add", "."]);
        git(&[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-qm",
            "Base",
        ]);
        fs::create_dir_all(
            binary_path(state.path())
                .parent()
                .expect("fixture operation succeeds"),
        )
        .expect("fixture operation succeeds");
        fs::copy(binary, binary_path(state.path())).expect("fixture operation succeeds");
        let mut lead = fixture_agent("lead", true);
        lead.cwd = Some(source.path().to_string_lossy().into_owned());
        configure_new_chat(
            &mut lead,
            &crate::project_path::ProjectPath::from_stored(
                CanonicalPath::existing(source.path())
                    .map(CanonicalPath::into_path_buf)
                    .expect("fixture operation succeeds")
                    .to_str()
                    .expect("fixture operation succeeds"),
            ),
        );
        let mut lead = lead.value();
        let ready =
            runtime::prepare_files(state.path(), &lead).expect("fixture operation succeeds");
        for (key, value) in ready.as_object().expect("fixture operation succeeds") {
            lead[key] = value.clone();
        }
        let main = PathBuf::from(lead["cwd"].as_str().expect("fixture operation succeeds"));
        assert!(main.join(".env").is_file());
        assert_eq!(
            fs::read_to_string(main.join("node_modules/old")).expect("imported cache"),
            "warm import\n"
        );
        assert_eq!(
            overlay_folder_state(state.path(), &main, "node_modules").as_deref(),
            Some("cache")
        );
        let real_git: PathBuf = studio_json::from_slice(
            &fs::read(binary_path(state.path()).with_file_name("git-real.json"))
                .expect("saved real Git path"),
        )
        .expect("real Git path JSON");
        for program in [binary_path(state.path()).with_file_name("git"), real_git] {
            let result =
                crate::bounded_child::run_without_git_environment(crate::bounded_child::Request {
                    program: program.as_os_str(),
                    git_directory: Some(main.as_os_str()),
                    args: &[
                        "-c",
                        "user.name=Fixture",
                        "-c",
                        "user.email=fixture@example.test",
                        "commit",
                        "--allow-empty",
                        "-m",
                        "Must refuse",
                    ],
                    environment: &[],
                    input: None,
                    stdout_limit: 4096,
                    stderr_limit: 4096,
                    timeout: std::time::Duration::from_secs(30),
                });
            // The wrapper refuses with the layr hint. The real Git meets the read only
            // `.git` folder that layr makes, which refuses first.
            assert!(
                matches!(result,Err(crate::bounded_child::ChildError::Failed(ref reason)) if reason.contains("layr save -m") || reason.contains("Permission denied")),
                "native Git commit guard: {result:?}"
            );
        }
        let parent = fixture_native_parent(&lead);
        let mut worker = fixture_agent("worker", false);
        worker.role = studio_runtime_schema::queue_command::Field::Value(
            studio_runtime_schema::creation_intent::AgentRole::Implementer,
        );
        configure_child(&mut worker, &parent, &parent);
        let mut worker = worker.value();
        let ready =
            runtime::prepare_files(state.path(), &worker).expect("fixture operation succeeds");
        for (key, value) in ready.as_object().expect("fixture operation succeeds") {
            worker[key] = value.clone();
        }
        let cwd = PathBuf::from(worker["cwd"].as_str().expect("fixture operation succeeds"));
        assert_eq!(
            fs::read_to_string(cwd.join("node_modules/old")).expect("imported warm cache"),
            "warm import\n"
        );
        fs::write(cwd.join("code.txt"), "worker result\n").expect("fixture operation succeeds");
        run(state.path(), &cwd, &["add", "code.txt"]).expect("fixture operation succeeds");
        run(state.path(), &cwd, &["save", "-m", "Worker result"])
            .expect("fixture operation succeeds");
        let state_id =
            run(state.path(), &cwd, &["rev-parse", "head"]).expect("fixture operation succeeds");
        assert_eq!(
            evidence(state.path(), &worker, &state_id)
                .expect("fixture operation succeeds")
                .state_id,
            state_id
        );
        assert!(evidence(state.path(), &worker, &state_id[..8]).is_err());
        merge(state.path(), &lead, &worker, &state_id, "accept-worker")
            .expect("fixture operation succeeds");
        assert_eq!(
            fs::read_to_string(main.join("code.txt")).expect("fixture operation succeeds"),
            "worker result\n"
        );
        let mut reviewer = fixture_agent("reviewer", false);
        reviewer.role = studio_runtime_schema::queue_command::Field::Value(
            studio_runtime_schema::creation_intent::AgentRole::Reviewer,
        );
        configure_child(&mut reviewer, &parent, &parent);
        let mut reviewer = reviewer.value();
        let ready =
            runtime::prepare_files(state.path(), &reviewer).expect("fixture operation succeeds");
        for (key, value) in ready.as_object().expect("fixture operation succeeds") {
            reviewer[key] = value.clone();
        }
        assert_eq!(reviewer["cwd"], reviewer["layrScratch"]);
        assert_eq!(
            Path::new(
                reviewer["layrReadCwd"]
                    .as_str()
                    .expect("fixture operation succeeds")
            ),
            main
        );
        let args = vec!["try".into(),state_id.clone(),"--".into(),"/bin/sh".into(),"-c".into(),
            "printf local > review-check; printf note > \"$TMPDIR/note\"; if printf forbidden > \"$1/code.txt\"; then exit 7; fi".into(),
            "review-check".into(),main.to_string_lossy().into_owned()];
        reviewer_try(state.path(), &reviewer, &args).expect("fixture operation succeeds");
        let environment = reviewer_try(
            state.path(),
            &reviewer,
            &[
                "try".into(),
                state_id.clone(),
                "--".into(),
                "/usr/bin/env".into(),
            ],
        )
        .expect("fixture operation succeeds");
        assert!(
            !environment
                .lines()
                .any(|line| line.starts_with("LAYR_ROOT=")
                    || line.starts_with("LAYR_DIRECT=")
                    || line.starts_with("LAYR_SOCKET=")),
            "Reviewer check inherited the runtime store environment"
        );
        assert_eq!(
            fs::read_to_string(main.join("code.txt")).expect("fixture operation succeeds"),
            "worker result\n"
        );
        assert!(!main.join("review-check").exists());
        assert!(
            PathBuf::from(
                reviewer["layrScratch"]
                    .as_str()
                    .expect("fixture operation succeeds")
            )
            .join("note")
            .is_file()
        );
        save(state.path(), &worker, "worker-turn-end").expect("fixture operation succeeds");
        remove(state.path(), &worker, "archive-worker").expect("fixture operation succeeds");
        assert!(!cwd.exists());
        assert!(
            run(
                state.path(),
                &main,
                &["show", &format!("{state_id}:code.txt")]
            )
            .expect("fixture operation succeeds")
            .contains("worker result")
        );
        let worktree = tempfile::tempdir().expect("fixture operation succeeds");
        git(&[
            "worktree",
            "add",
            "-b",
            "migration-fixture",
            worktree
                .path()
                .to_str()
                .expect("fixture operation succeeds"),
        ]);
        fs::write(worktree.path().join("code.txt"), "migrated worktree\n")
            .expect("fixture operation succeeds");
        fs::write(worktree.path().join(".env"), "TEST_VALUE=migrated\n")
            .expect("fixture operation succeeds");
        let plan = migration::migrate(state.path(), source.path(), true)
            .expect("fixture operation succeeds");
        assert_eq!(
            plan["lines"]
                .as_array()
                .expect("fixture operation succeeds")
                .len(),
            1
        );
        let imported = migration::migrate(state.path(), source.path(), false)
            .expect("fixture operation succeeds");
        assert_eq!(imported["projectId"], lead["layrProjectId"]);
        let migrated = main.parent().expect("fixture operation succeeds").join(
            imported["lines"][0]["line"]
                .as_str()
                .expect("fixture operation succeeds"),
        );
        assert_eq!(
            fs::read_to_string(migrated.join("code.txt")).expect("fixture operation succeeds"),
            "migrated worktree\n"
        );
        assert_eq!(
            fs::read_to_string(migrated.join(".env")).expect("fixture operation succeeds"),
            "TEST_VALUE=migrated\n"
        );
        let again = migration::migrate(state.path(), source.path(), false)
            .expect("fixture operation succeeds");
        assert_eq!(imported["lines"], again["lines"]);
        println!("native host evidence store: {}", state.path().display());
    }
}

/// One allowlist for review checks and native Claude reviewer sessions.
pub(crate) fn reviewer_environment(
    state_dir: &Path,
    scratch: &str,
) -> studio_json::Map<String, Value> {
    let mut environment = studio_json::Map::new();
    environment.insert("PATH".into(), thread_environment(state_dir)["PATH"].clone());
    environment.insert("HOME".into(), json!(scratch));
    environment.insert("TMPDIR".into(), json!(scratch));
    environment.insert(
        "LANG".into(),
        json!(std::env::var("LANG").unwrap_or_else(|_| "C.UTF-8".into())),
    );
    environment.insert(
        "TERM".into(),
        json!(std::env::var("TERM").unwrap_or_else(|_| "dumb".into())),
    );
    for name in ["CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN"] {
        let value = std::env::var(name).ok().or_else(|| {
            let suffix = match name {
                "CARGO_HOME" => ".cargo",
                "RUSTUP_HOME" => ".rustup",
                _ => return None,
            };
            let folder = PathBuf::from(std::env::var_os("HOME")?).join(suffix);
            folder
                .is_dir()
                .then(|| folder.to_string_lossy().into_owned())
        });
        if let Some(value) = value {
            environment.insert(name.into(), json!(value));
        }
    }
    environment
}

/// A review check may change only its temporary copy and private scratch folder.
pub fn reviewer_try(
    state_dir: &Path,
    record: &Value,
    argv: &[String],
) -> Result<String, NativeLayrError> {
    if record["role"] != "reviewer" || argv.len() < 4 || argv[0] != "try" || argv[2] != "--" {
        return Err(NativeLayrError::Refused(
            "Use try <state> -- <command> for a reviewer check".into(),
        ));
    }
    let cwd = reviewer_read_cwd(state_dir, record)?;
    let scratch = record["layrScratch"]
        .as_str()
        .ok_or_else(|| NativeLayrError::Refused("Missing reviewer scratch folder".into()))?;
    #[cfg(target_os = "macos")]
    let wrapper = r#"scratch=$1; profile=$2; shift 3; export TMPDIR="$scratch"; exec /usr/bin/sandbox-exec -D "WORKSPACE=$PWD" -D "SCRATCH=$scratch" -p "$profile" "$@""#;
    #[cfg(target_os = "macos")]
    let profile = r#"(version 1)(deny default)(allow process*)(allow sysctl-read)(allow mach-lookup)(allow file-read*)(allow file-write* (subpath (param "WORKSPACE")) (subpath (param "SCRATCH")))"#;
    #[cfg(not(target_os = "macos"))]
    let wrapper = linux_reviewer_wrapper();
    #[cfg(not(target_os = "macos"))]
    let profile = "";
    let state = CanonicalPath::existing(state_dir).map(CanonicalPath::into_path_buf)?;
    let state = state
        .to_str()
        .ok_or_else(|| NativeLayrError::Refused("Non-UTF-8 state path".into()))?;
    let environment: Vec<String> = reviewer_environment(state_dir, scratch)
        .into_iter()
        .map(|(key, value)| format!("{key}={}", value.as_str().unwrap_or_default()))
        .collect();
    let mut args = vec!["try", argv[1].as_str(), "--", "/usr/bin/env", "-i"];
    args.extend(environment.iter().map(String::as_str));
    args.extend([
        "/bin/sh",
        "-c",
        wrapper,
        "studio-review-try",
        scratch,
        profile,
        state,
    ]);
    args.extend(argv[3..].iter().map(String::as_str));
    run(state_dir, &cwd, &args)
}

pub(crate) fn reviewer_read_cwd(
    state_dir: &Path,
    record: &Value,
) -> Result<PathBuf, NativeLayrError> {
    if record["role"] != "reviewer" {
        return Err(NativeLayrError::Refused(
            "Read access requires a reviewer".into(),
        ));
    }
    runtime::associated_cwd(state_dir, record)
}

#[cfg(any(not(target_os = "macos"), test))]
fn linux_reviewer_wrapper() -> &'static str {
    r#"scratch=$1; state=$3; shift 3
set -- --tmpfs "$state" --bind "$PWD" /workspace --bind "$scratch" /scratch --proc /proc --dev /dev --chdir /workspace --setenv TMPDIR /scratch --setenv HOME /scratch --setenv PATH "$PATH" --unsetenv LAYR_SOCKET --unsetenv LAYR_ROOT --unsetenv LAYR_PROJECT --unsetenv LAYR_LINE --unsetenv LAYR_SESSION --unsetenv LAYR_DIRECT --remount-ro / -- "$@"
for root in /usr /bin /sbin /lib /lib64 /opt; do
  if [ -e "$root" ]; then set -- --ro-bind "$root" "$root" "$@"; fi
done
for file in /etc/ld.so.cache /etc/ld.so.conf /etc/passwd /etc/group /etc/nsswitch.conf /etc/localtime; do
  if [ -f "$file" ]; then set -- --ro-bind "$file" "$file" "$@"; fi
done
for root in "${CARGO_HOME:-}" "${RUSTUP_HOME:-}"; do
  case "$root" in /*) ;; *) continue ;; esac
  case "$root/" in "$state/"*) continue ;; esac
  case "$state/" in "$root/"*) continue ;; esac
  if [ -d "$root" ]; then set -- --ro-bind "$root" "$root" "$@"; fi
done
exec bwrap --unshare-all --die-with-parent --new-session --tmpfs / "$@""#
}

#[cfg(test)]
mod reviewer_isolation_tests {
    #[test]
    fn native_layr_linux_try_does_not_expose_the_host_or_daemon_socket() {
        let wrapper = super::linux_reviewer_wrapper();
        assert!(!wrapper.contains("--ro-bind / /"));
        assert!(wrapper.contains("--tmpfs /"));
        assert!(wrapper.contains("--unsetenv LAYR_SOCKET"));
        assert!(wrapper.contains("--chdir /workspace"));
    }
}

pub(crate) use runtime::stage_turn_save;

#[cfg(test)]
mod turn_save_tests {
    use super::*;
    use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, RuntimeStore, StoreError};

    #[tokio::test]
    async fn native_terminal_stages_a_save_without_resetting_a_completed_receipt() {
        let directory = tempfile::tempdir().expect("fixture operation succeeds");
        let store = RuntimeStore::open(Some(directory.path()))
            .await
            .expect("fixture operation succeeds");
        store.transaction_with(|tx| -> Result<(), StoreError> {
            let record = json!({"compactions":0,"accountKey":"default","provider":"codex","id":"native-save-agent","rootId":"native-save-agent","epoch":0,
                "status":"idle","autoWake":true,"workspaceMode":"layr-native","layrReady":true,"cwd":"/native/main"});
            AgentRepository::put(tx,&AgentRecord::from_value(record).map_err(StoreError::from)?)?;
            stage_turn_save(tx,"native-save-agent","turn-1")?;
            let id = AgentId::from("native-save-agent");
            let mut saved = AgentRepository::get(tx,&id)?.expect("fixture operation succeeds").value();
            assert_eq!(saved["layrTurnSaves"]["turn-1"]["status"],"pending");
            saved["layrTurnSaves"]["turn-1"] = json!({"status":"saved","stateId":"exact-state"});
            AgentRepository::put(tx,&AgentRecord::from_value(saved).map_err(StoreError::from)?)?;
            stage_turn_save(tx,"native-save-agent","turn-1")?;
            assert_eq!(AgentRepository::get(tx,&id)?.expect("fixture operation succeeds").value()["layrTurnSaves"]["turn-1"]["stateId"],"exact-state");
            Ok(())
        }).await.expect("fixture operation succeeds");
    }
}

#[cfg(test)]
mod review_regressions {
    use super::*;
    use std::{fs, path::PathBuf};

    #[cfg(unix)]
    fn fast_fixture() -> (tempfile::TempDir, Value, capability::BinaryOverride) {
        use std::os::unix::fs::PermissionsExt;
        let state = tempfile::tempdir().expect("fixture operation succeeds");
        fs::create_dir_all(
            binary_path(state.path())
                .parent()
                .expect("fixture operation succeeds"),
        )
        .expect("fixture operation succeeds");
        fs::write(binary_path(state.path()),r#"#!/bin/sh
printf '%s\n' "$*" >> "$LAYR_ROOT/commands"
case "$3 $4" in
  'rev-parse head@{0}')
    if [ -f "$LAYR_ROOT/automatic" ]; then cat "$LAYR_ROOT/automatic"; else printf "head@{0}: line has only 0 automatic states\n" >&2; exit 128; fi ;;
  'rev-parse head'|'rev-parse full-named-state') printf 'full-named-state\n' ;;
  'overlay list') printf '[]\n' ;;
  'op log') [ ! -f "$LAYR_ROOT/oplog" ] || cat "$LAYR_ROOT/oplog" ;;
esac
"#).expect("fixture operation succeeds");
        fs::set_permissions(binary_path(state.path()), fs::Permissions::from_mode(0o755))
            .expect("fixture operation succeeds");
        // Cargo's test runner can execute fixtures concurrently in one process.
        let binary_override = override_layr_binary(binary_path(state.path()));
        let project = studio_runtime_store::sha256_hex(b"/fast-source");
        let line = format!("agent-{}", studio_runtime_store::sha256_hex(b"fast-worker"));
        let cwd = main_path(state.path(), &project)
            .parent()
            .expect("fixture operation succeeds")
            .join(&line);
        fs::create_dir_all(&cwd).expect("fixture operation succeeds");
        fs::create_dir_all(main_path(state.path(), &project)).expect("fixture operation succeeds");
        let worker = json!({"id":"fast-worker","workspaceMode":"layr-native","role":"implementer","isLead":false,"layrReady":true,"layrProjectId":project,"layrLine":line,"cwd":cwd});
        (state, worker, binary_override)
    }

    #[cfg(unix)]
    #[test]
    fn resolved_merge_conflict_is_accepted_without_repeating_merge() {
        use std::os::unix::fs::PermissionsExt;
        let (state, worker, _binary_override) = fast_fixture();
        let root = store_path(state.path());
        let main = main_path(state.path(), worker["layrProjectId"].as_str().unwrap());
        let lead = json!({"id":"lead","workspaceMode":"layr-native","role":"lead","isLead":true,"layrReady":true,"layrProjectId":worker["layrProjectId"],"layrLine":"main","cwd":main});
        fs::write(binary_path(state.path()), r#"#!/bin/sh
printf '%s\n' "$*" >> "$LAYR_ROOT/commands"
case "$3 $4" in
  'merge '*) printf 'CONFLICT (content): Merge conflict in file\n'; exit 1 ;;
  'rev-parse head') echo resolved ;;
  'status --json') if [ -f "$LAYR_ROOT/resolved" ]; then echo '{"staged":[],"unstaged":[],"untracked":[],"conflicts":[]}'; else echo '{"staged":[],"unstaged":[],"untracked":[],"conflicts":[{}]}'; fi ;;
  'merge-base --is-ancestor') [ -f "$LAYR_ROOT/ancestor" ] || exit 1 ;;
esac
"#).unwrap();
        fs::set_permissions(binary_path(state.path()), fs::Permissions::from_mode(0o755)).unwrap();
        let first = merge(state.path(), &lead, &worker, "worker-state", "accept");
        assert!(
            matches!(first, Err(NativeLayrError::Conflict(ref message)) if message.contains("CONFLICT"))
        );
        assert!(matches!(
            merge(state.path(), &lead, &worker, "worker-state", "accept"),
            Err(NativeLayrError::Conflict(_))
        ));
        fs::write(root.join("resolved"), "").unwrap();
        assert!(matches!(
            merge(state.path(), &lead, &worker, "worker-state", "accept"),
            Err(NativeLayrError::Conflict(_))
        ));
        fs::write(root.join("ancestor"), "").unwrap();
        let result = merge(state.path(), &lead, &worker, "worker-state", "accept")
            .expect("resolved conflict must recover");
        assert_eq!(result["merge"]["to_state"], "resolved");
        assert!(result["merge"].get("from_state").is_none());
        let commands = fs::read_to_string(root.join("commands")).unwrap();
        assert_eq!(
            commands
                .lines()
                .filter(|line| line.contains(" merge "))
                .count(),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn interrupted_merge_never_runs_again() {
        let (state, worker, _binary_override) = fast_fixture();
        let main = main_path(state.path(), worker["layrProjectId"].as_str().unwrap());
        let lead = json!({"id":"lead","workspaceMode":"layr-native","role":"lead","isLead":true,"layrReady":true,"layrProjectId":worker["layrProjectId"],"layrLine":"main","cwd":main});
        let intent = json!({"lead":"lead","worker":"fast-worker","revision":"worker-state"});
        let result = runtime::operation_in_store(
            state.path(),
            &store_path(state.path()),
            "accept",
            &intent,
            || Err(NativeLayrError::Unknown("interrupted".into())),
        );
        assert!(matches!(result, Err(NativeLayrError::Unknown(_))));
        assert!(matches!(
            merge(state.path(), &lead, &worker, "worker-state", "accept"),
            Err(NativeLayrError::Unknown(_))
        ));
        assert!(!store_path(state.path()).join("commands").exists());
    }

    #[cfg(unix)]
    #[test]
    fn native_layr_fake_evidence_retains_transport_fields() {
        let (state, worker, _binary_override) = fast_fixture();
        let response = evidence(state.path(), &worker, "full-named-state").unwrap();
        assert_eq!(response.state_id, "full-named-state");
        assert_eq!(
            response.project_id.as_deref(),
            worker["layrProjectId"].as_str()
        );
        assert_eq!(response.line.as_deref(), worker["layrLine"].as_str());
        let mut unassociated = worker.clone();
        unassociated["cwd"] = json!(state.path().join("elsewhere"));
        assert!(evidence(state.path(), &unassociated, "full-named-state").is_err());
        assert!(evidence(state.path(), &worker, "head").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn native_layr_unchanged_save_replays_only_explicit_failures() {
        let (state, worker, _binary_override) = fast_fixture();
        let cwd =
            CanonicalPath::existing(worker["cwd"].as_str().expect("fixture operation succeeds"))
                .map(CanonicalPath::into_path_buf)
                .expect("fixture operation succeeds");
        let intent = json!({"agent":worker["id"],"cwd":cwd,"kind":"save"});
        assert!(
            operation(state.path(), "failed-save", &intent, || Err(
                NativeLayrError::Refused("old missing automatic state".into())
            ))
            .is_err()
        );
        assert_eq!(
            save(state.path(), &worker, "failed-save").expect("fixture operation succeeds")["stateId"],
            "full-named-state"
        );
    }

    #[cfg(unix)]
    fn leave_pending(state: &Path, id: &str, intent: &Value) {
        assert!(matches!(
            operation(state, id, intent, || Err(NativeLayrError::Unknown(
                "lost".into()
            ))),
            Err(NativeLayrError::Unknown(_))
        ));
    }

    #[cfg(unix)]
    fn save_intent(worker: &Value) -> Value {
        let cwd =
            CanonicalPath::existing(worker["cwd"].as_str().expect("fixture operation succeeds"))
                .map(CanonicalPath::into_path_buf)
                .expect("fixture operation succeeds");
        json!({"agent":worker["id"],"cwd":cwd,"kind":"save"})
    }

    #[cfg(unix)]
    fn save_commands(state: &Path) -> usize {
        fs::read_to_string(store_path(state).join("commands"))
            .unwrap_or_default()
            .lines()
            .filter(|line| line.ends_with(" save"))
            .count()
    }

    #[cfg(unix)]
    #[test]
    fn pending_save_without_a_save_operation_retries() {
        let (state, worker, _binary_override) = fast_fixture();
        leave_pending(state.path(), "pending-save", &save_intent(&worker));
        assert_eq!(
            save(state.path(), &worker, "pending-save").expect("retry after proof")["stateId"],
            "full-named-state"
        );
        assert_eq!(save_commands(state.path()), 1);
    }

    #[cfg(unix)]
    #[test]
    fn pending_save_with_a_save_operation_becomes_completed_and_does_not_run_again() {
        let (state, worker, _binary_override) = fast_fixture();
        leave_pending(state.path(), "pending-save", &save_intent(&worker));
        fs::write(
            store_path(state.path()).join("oplog"),
            "{\"op\":\"save\",\"time\":99999999999999}\n{\"op\":\"line.create\",\"time\":1}\n",
        )
        .expect("operation log");
        for _ in 0..2 {
            assert_eq!(
                save(state.path(), &worker, "pending-save").expect("completed by evidence")["stateId"],
                "full-named-state"
            );
        }
        assert_eq!(save_commands(state.path()), 0);
    }

    #[cfg(unix)]
    #[test]
    fn pending_save_ignores_a_save_operation_from_before_the_receipt() {
        let (state, worker, _binary_override) = fast_fixture();
        leave_pending(state.path(), "pending-save", &save_intent(&worker));
        fs::write(
            store_path(state.path()).join("oplog"),
            "{\"op\":\"save\",\"time\":1}\n",
        )
        .expect("operation log");
        save(state.path(), &worker, "pending-save").expect("retry");
        assert_eq!(save_commands(state.path()), 1);
    }

    #[cfg(unix)]
    #[test]
    fn pending_import_stays_unknown() {
        let (state, _worker, _binary_override) = fast_fixture();
        let intent = json!({"kind":"import","project":"p"});
        leave_pending(state.path(), "pending-import", &intent);
        fs::write(
            store_path(state.path()).join("oplog"),
            "{\"op\":\"save\",\"time\":99999999999999}\n",
        )
        .expect("operation log");
        assert!(matches!(
            operation(state.path(), "pending-import", &intent, || panic!(
                "never run again"
            )),
            Err(NativeLayrError::Unknown(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn native_layr_archive_sets_the_nonexpiring_pin_before_deleting_the_line() {
        let (state, worker, _binary_override) = fast_fixture();
        fs::write(
            store_path(state.path()).join("automatic"),
            "full-automatic-state\n",
        )
        .expect("fixture operation succeeds");
        let receipt =
            remove(state.path(), &worker, "archive-with-pin").expect("fixture operation succeeds");
        assert_eq!(receipt["stateId"], "full-automatic-state");
        assert!(
            receipt["archiveRef"]
                .as_str()
                .expect("fixture operation succeeds")
                .starts_with("pins/studio/archive/")
        );
        let commands = fs::read_to_string(store_path(state.path()).join("commands"))
            .expect("fixture operation succeeds");
        let commands: Vec<_> = commands.lines().collect();
        let pin = commands
            .iter()
            .position(|line| line.contains(" ref set "))
            .expect("fixture operation succeeds");
        let deletion = commands
            .iter()
            .position(|line| line.contains(" line delete "))
            .expect("fixture operation succeeds");
        assert!(pin < deletion);
        assert!(!commands[pin].contains("--ttl"));
        assert!(commands[pin].ends_with("full-automatic-state"));
    }

    fn fixture() -> (
        tempfile::TempDir,
        tempfile::TempDir,
        Value,
        Value,
        capability::BinaryOverride,
    ) {
        let _binary_override = crate::layr_native::override_layr_binary(std::path::PathBuf::from(
            crate::layr_native::test_binary().expect("test binary"),
        ));
        let state = tempfile::tempdir().expect("fixture operation succeeds");
        let source = tempfile::tempdir().expect("fixture operation succeeds");
        fs::write(source.path().join("code.txt"), "base\n").expect("fixture operation succeeds");
        fs::create_dir_all(
            binary_path(state.path())
                .parent()
                .expect("fixture operation succeeds"),
        )
        .expect("fixture operation succeeds");
        fs::copy(
            crate::layr_native::test_binary().expect("Set STUDIO_LAYR_NATIVE_TEST_BINARY"),
            binary_path(state.path()),
        )
        .expect("fixture operation succeeds");
        let mut lead = fixture_agent("regression-lead", true);
        lead.cwd = Some(source.path().to_string_lossy().into_owned());
        configure_new_chat(
            &mut lead,
            &crate::project_path::ProjectPath::from_stored(
                source.path().to_str().expect("fixture operation succeeds"),
            ),
        );
        let mut lead = lead.value();
        let fields =
            runtime::prepare_files(state.path(), &lead).expect("fixture operation succeeds");
        for (key, value) in fields.as_object().expect("fixture operation succeeds") {
            lead[key] = value.clone();
        }
        let parent = fixture_native_parent(&lead);
        let mut worker = fixture_agent("regression-worker", false);
        worker.role = studio_runtime_schema::queue_command::Field::Value(
            studio_runtime_schema::creation_intent::AgentRole::Implementer,
        );
        configure_child(&mut worker, &parent, &parent);
        let mut worker = worker.value();
        let fields =
            runtime::prepare_files(state.path(), &worker).expect("fixture operation succeeds");
        for (key, value) in fields.as_object().expect("fixture operation succeeds") {
            worker[key] = value.clone();
        }
        (state, source, lead, worker, _binary_override)
    }

    #[test]
    #[ignore = "requires the installed Layr on APFS or root on btrfs"]
    fn native_layr_unchanged_turn_save_and_previously_failed_save_can_finish() {
        let (state, _source, _lead, worker, _binary_override) = fixture();
        let expected = run(
            state.path(),
            Path::new(worker["cwd"].as_str().expect("fixture operation succeeds")),
            &["rev-parse", "head"],
        )
        .expect("fixture operation succeeds");
        assert_eq!(
            save(state.path(), &worker, "unchanged-turn").expect("fixture operation succeeds")["stateId"],
            expected
        );
        let cwd =
            CanonicalPath::existing(worker["cwd"].as_str().expect("fixture operation succeeds"))
                .map(CanonicalPath::into_path_buf)
                .expect("fixture operation succeeds");
        let intent = json!({"agent":worker["id"],"cwd":cwd,"kind":"save"});
        assert!(
            operation(state.path(), "previously-failed-save", &intent, || Err(
                NativeLayrError::Refused("head@{0}: line has only 0 automatic states".into())
            ))
            .is_err()
        );
        assert_eq!(
            save(state.path(), &worker, "previously-failed-save")
                .expect("fixture operation succeeds")["stateId"],
            expected
        );
    }

    #[test]
    #[ignore = "requires the installed Layr on APFS or root on btrfs"]
    fn native_layr_archive_pins_an_unmerged_automatic_state_without_expiry() {
        let (state, _source, lead, worker, _binary_override) = fixture();
        fs::write(
            PathBuf::from(worker["cwd"].as_str().expect("fixture operation succeeds"))
                .join("code.txt"),
            "unmerged work\n",
        )
        .expect("fixture operation succeeds");
        let saved =
            save(state.path(), &worker, "archive-turn-save").expect("fixture operation succeeds");
        let receipt = remove(state.path(), &worker, "archive-worker-with-retention")
            .expect("fixture operation succeeds");
        let reference = receipt["archiveRef"]
            .as_str()
            .expect("archive must keep a durable ref");
        let main = Path::new(lead["cwd"].as_str().expect("fixture operation succeeds"));
        let pins: Value = studio_json::from_str(
            &run(
                state.path(),
                main,
                &["ref", "list", "pins/studio/archive/", "--json"],
            )
            .expect("fixture operation succeeds"),
        )
        .expect("fixture operation succeeds");
        let pin = pins
            .as_array()
            .expect("fixture operation succeeds")
            .iter()
            .find(|pin| pin["name"] == reference)
            .expect("fixture operation succeeds");
        assert!(pin.get("expires_at").is_none_or(Value::is_null));
        assert_eq!(
            run(state.path(), main, &["rev-parse", reference]).expect("fixture operation succeeds"),
            saved["stateId"]
                .as_str()
                .expect("fixture operation succeeds")
        );
        assert!(
            run(
                state.path(),
                main,
                &["show", &format!("{reference}:code.txt")]
            )
            .expect("fixture operation succeeds")
            .contains("unmerged work")
        );
        assert!(!Path::new(worker["cwd"].as_str().expect("fixture operation succeeds")).exists());
    }
}
