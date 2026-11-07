//! Experimental virtual agent lines (Layr `docs/views.md`). A virtual line is a LayrFS mount of
//! the line's changes over the tree of its start state, so a new line costs one mount instead of
//! a clone of the whole tree. Layr refuses most commands inside such a line. Studio runs only the
//! supported subset there: `status`, `save -m` and `rev-parse` in the line; `merge`, `ref set`,
//! `line show` and `line delete` from main. A virtual line has no `.git` folder and no cache
//! overlays. docs/native-layr.md, "Virtual agent lines (experimental)", lists each choice.
use super::{NativeLayrError, runtime::run_with_environment};
use crate::bounded_child::{self, ChildError, Request};
use std::{fs, path::Path, time::Duration};
use studio_json::Value;

type Result<T> = std::result::Result<T, NativeLayrError>;

/// Layr makes and mounts virtual lines only with this variable.
const EXPERIMENT: (&str, &str) = ("LAYR_EXPERIMENTAL_VIEWS", "1");

/// True when this server can make virtual lines: macOS, and a Layr build with `layr line show`.
/// That command without a line prints its own usage, which names `--virtual`; an older build
/// prints the usage of `layr line` without `show`. Every Layr command makes a missing store,
/// so the probe runs only against a store that exists (its `projects` folder); it changes
/// nothing there.
pub fn supported(binary: &Path, root: &Path) -> bool {
    if !cfg!(target_os = "macos") || !root.join("projects").is_dir() {
        return false;
    }
    probe(binary, root)
}

fn probe(binary: &Path, root: &Path) -> bool {
    let Some(root) = root.to_str() else {
        return false;
    };
    let output = bounded_child::run_without_git_environment(Request {
        program: binary.as_os_str(),
        git_directory: None,
        args: &["line", "show"],
        environment: &[("LAYR_ROOT", root), ("LAYR_DIRECT", "1")],
        input: None,
        stdout_limit: 4096,
        stderr_limit: 4096,
        timeout: Duration::from_secs(10),
    });
    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout.bytes).contains("--virtual"),
        Err(ChildError::Failed(message)) => message.contains("--virtual"),
        Err(_) => false,
    }
}

/// Whether `line` of `project` is virtual. Layr keeps a local record of each view in
/// `projects/<project>/meta/vlines/<line id>/view.json` (Layr `docs/views.md`, "Local record");
/// it stays when the mount is gone, for example after a restart.
pub(super) fn is_virtual(root: &Path, project: &str, line: &str) -> bool {
    let folder = root.join("projects").join(project).join("meta/vlines");
    let Ok(entries) = fs::read_dir(folder) else {
        return false;
    };
    entries.flatten().any(|entry| {
        fs::read(entry.path().join("view.json"))
            .ok()
            .and_then(|bytes| studio_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|view| view["name"] == line)
    })
}

/// Make `line` from `start` as a virtual line. Layr removes a half-made view on failure.
pub(super) fn create(state_dir: &Path, main: &Path, line: &str, start: &str) -> Result<()> {
    run_with_environment(
        state_dir,
        main,
        &["line", "new", line, start, "--view", "virtual"],
        &[EXPERIMENT],
    )
    .map(drop)
}

/// Mount `line` again when its mount is gone. The command does nothing for a mounted line.
pub(super) fn mount(state_dir: &Path, main: &Path, line: &str) -> Result<()> {
    run_with_environment(
        state_dir,
        main,
        &["line", "show", line, "--virtual"],
        &[EXPERIMENT],
    )
    .map(drop)
}

/// Layr empties its trash in a background process that inherits the working folder. In a
/// virtual line that folder is the mount, and `line delete` then fails with "Resource busy".
/// A zero budget makes the command leave the trash to a later command and start no process.
const NO_TRASH_PROCESS: (&str, &str) = ("LAYR_TRASH_BUDGET_MS", "0");

/// Save the current layer as an automatic state without moving the named head. The same
/// operation is used at quiet points and before removal; it never skips unsaved work.
pub(super) fn save_all(state_dir: &Path, cwd: &Path) -> Result<String> {
    run_with_environment(state_dir, cwd, &["save"], &[NO_TRASH_PROCESS])?;
    super::runtime::working_state(state_dir, cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};
    use studio_json::json;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("mode");
        path
    }

    #[test]
    fn probe_accepts_only_a_layr_build_with_line_show() {
        let dir = tempfile::tempdir().expect("fixture");
        // Output of a build with virtual lines (ifdotpy/layr#420) and of layr 0.40.1.
        let views = script(
            dir.path(),
            "layr-views",
            "echo 'usage: layr line show <line> --folder [<path>] | --none | --virtual' >&2; exit 2",
        );
        let old = script(
            dir.path(),
            "layr-old",
            "echo 'usage: layr line new/list/delete/rename/move' >&2; exit 2",
        );
        let missing = dir.path().join("missing");
        assert!(probe(&views, dir.path()));
        assert!(!probe(&old, dir.path()));
        assert!(!probe(&missing, dir.path()));
        // No store yet: no probe, so Layr does not make one.
        assert!(!supported(&views, dir.path()));
        fs::create_dir(dir.path().join("projects")).expect("store");
        assert_eq!(supported(&views, dir.path()), cfg!(target_os = "macos"));
    }

    #[test]
    fn a_line_is_virtual_only_with_its_layr_view_record() {
        let root = tempfile::tempdir().expect("store");
        let views = root.path().join("projects/project/meta/vlines");
        fs::create_dir_all(views.join("line-id")).expect("view folder");
        fs::write(
            views.join("line-id/view.json"),
            json!({"line":"line-id","name":"agent-a","base":"state","lower":"/lower","mounted":false})
                .to_string(),
        )
        .expect("view record");
        fs::create_dir_all(views.join("broken")).expect("broken view");
        fs::write(views.join("broken/view.json"), "{").expect("broken record");
        assert!(is_virtual(root.path(), "project", "agent-a"));
        assert!(!is_virtual(root.path(), "project", "agent-b"));
        assert!(!is_virtual(root.path(), "other", "agent-a"));
    }

    #[test]
    fn every_status_list_counts_as_a_change() {
        let clean = json!({"staged":[],"unstaged":[],"untracked":[],"conflicts":[]});
        assert!(!has_changes(&clean));
        for key in ["staged", "unstaged", "untracked", "conflicts"] {
            let mut changed = clean.clone();
            changed[key] = json!([{"path":"a.txt","status":"M"}]);
            assert!(has_changes(&changed), "{key}");
        }
    }

    /// A fake Layr that refuses in a virtual line what `layr` refuses there (Layr
    /// `views::guard`), and records each command with `LAYR_EXPERIMENTAL_VIEWS`.
    const FAKE_LAYR: &str = r##"#!/bin/sh
cwd=
if [ "$1" = -C ]; then cwd=$2; shift 2; fi
printf '%s%s|%s\n' "${LAYR_EXPERIMENTAL_VIEWS:-}" "${LAYR_TRASH_BUDGET_MS:+t}" "$*" >> "$LAYR_ROOT/commands"
if [ -n "$cwd" ] && grep -qs "\"name\":\"${cwd##*/}\"" "$cwd"/../../meta/vlines/*/view.json; then
  case "$1" in
    status|diff|save|line|log|show|rev-parse|merge-base|config|fs) ;;
    *) echo "layr $1 does not support virtual lines yet" >&2; exit 1 ;;
  esac
  if [ "$1" = save ] && [ "$2" != -m ]; then echo "a virtual line saves with 'layr save -m <message>' only" >&2; exit 2; fi
fi
case "$*" in
  *store-version*) printf '%s\n' '{"binary":{"format":1,"version":"0.14.2","build":"fixture"},"store":null,"compatible":true,"state":"compatible"}' ;;
  'init '*) mkdir -p "$LAYR_ROOT/projects/$2/lines/main" ;;
  'line show') echo 'usage: layr line show <line> --folder [<path>] | --none | --virtual' >&2; exit 2 ;;
  'line new '*' --view virtual')
    [ "$LAYR_EXPERIMENTAL_VIEWS" = 1 ] || exit 2
    if [ -f "$LAYR_ROOT/no-views" ]; then echo 'cannot mount the virtual line' >&2; exit 1; fi
    mkdir -p "$cwd/../../meta/vlines/$3" "$cwd/../$3"
    printf '{"name":"%s","mounted":true}' "$3" > "$cwd/../../meta/vlines/$3/view.json" ;;
  'line new '*) mkdir -p "$cwd/../$3" ;;
  'line show '*' --virtual') [ "$LAYR_EXPERIMENTAL_VIEWS" = 1 ] || exit 2 ;;
  'status --json')
    if [ -f "$cwd/dirty" ]; then list='[{"old_path":null,"path":"a.txt","status":"M"}]'; else list='[]'; fi
    printf '{"staged":%s,"unstaged":[],"untracked":[],"conflicts":[]}\n' "$list" ;;
  'rev-parse head') if [ -f "$LAYR_ROOT/saved" ]; then echo saved-state; else echo head-state; fi ;;
  'save -m '*) rm "$cwd/dirty"; touch "$LAYR_ROOT/saved" ;;
  'line delete -D '*) rm -rf "$cwd/../$4" ;;
  'overlay list --json') echo '[]' ;;
esac
"##;

    fn prepared(state: &Path, record: &Value) -> Value {
        let mut record = record.clone();
        for (key, value) in super::super::runtime::prepare_files(state, &record)
            .expect("preparation")
            .as_object()
            .expect("prepared fields")
        {
            record[key] = value.clone();
        }
        record
    }

    fn worker(lead: &Value, id: &str) -> Value {
        let lead = studio_runtime_store::AgentRecord::from_value(lead.clone()).expect("lead");
        let mut worker = super::super::fixture_agent(id, false);
        super::super::configure_child(&mut worker, &lead, &lead);
        worker.value()
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn virtual_agent_lines_use_only_what_layr_supports_in_them() {
        let files = tempfile::tempdir().expect("fixture");
        let files = files.path().canonicalize().expect("canonical fixture");
        let state = files.join("state");
        let source = files.join("source");
        fs::create_dir_all(&state).expect("state");
        fs::create_dir_all(&source).expect("source");
        let binary = script(&files, "layr", FAKE_LAYR.trim_start_matches("#!/bin/sh\n"));
        let _binary = super::super::override_layr_binary(binary);
        let mut lead = super::super::fixture_agent("lead", true);
        super::super::configure_new_chat(
            &mut lead,
            &crate::project_path::ProjectPath::from_stored(source.to_str().unwrap()),
        );
        let lead = prepared(&state, &lead.value());
        let root = super::super::store_path(&state);
        let commands = root.join("commands");
        super::super::settings::configure_virtual_agent_lines(&state, true).expect("setting");

        // Preparation makes a virtual line, mounts it (again after a restart) and warms its index.
        fs::write(&commands, "").unwrap();
        let worker = prepared(&state, &worker(&lead, "worker"));
        let line = worker["layrLine"].as_str().expect("line").to_owned();
        let cwd = Path::new(worker["cwd"].as_str().expect("cwd")).to_owned();
        let log = fs::read_to_string(&commands).unwrap();
        let cwd_text = cwd.display();
        assert!(
            log.contains(&format!("1|line new {line} main --view virtual\n")),
            "{log}"
        );
        assert!(
            log.contains(&format!("1|line show {line} --virtual\n")),
            "{log}"
        );
        assert!(log.contains(&format!("|fs warm {cwd_text}\n")), "{log}");
        assert!(is_virtual(
            &root,
            lead["layrProjectId"].as_str().unwrap(),
            &line
        ));
        assert!(!cwd.join(".git").exists(), "no Git hook in a virtual line");
        assert_eq!(worker["layrStateId"], "head-state");

        // The quiet-point save reads the head only: no overlay repair, no automatic save.
        fs::write(&commands, "").unwrap();
        let saved = super::super::save(&state, &worker, "turn-save:worker:1").expect("turn save");
        assert_eq!(saved["stateId"], "head-state");
        assert_eq!(fs::read_to_string(&commands).unwrap(), "|rev-parse head\n");

        // The lead merges a virtual line from main, as a folder line.
        fs::write(&commands, "").unwrap();
        super::super::merge(&state, &lead, &worker, "head-state", "merge-1").expect("merge");
        assert!(
            fs::read_to_string(&commands)
                .unwrap()
                .contains(&format!("|merge {line} --expect head-state\n"))
        );

        // Removal saves unsaved work with a message, archives that state, then deletes the line.
        fs::write(cwd.join("dirty"), "").unwrap();
        fs::write(&commands, "").unwrap();
        let removed = super::super::remove(&state, &worker, "remove-1").expect("removal");
        assert_eq!(removed["stateId"], "saved-state");
        let log = fs::read_to_string(&commands).unwrap();
        // The commands in the line start no trash process, which would keep the mount busy.
        let expected = [
            "t|status --json".to_owned(),
            format!("t|save -m {ARCHIVE_MESSAGE}"),
            "t|rev-parse head".to_owned(),
            format!(
                "|ref set {} saved-state",
                removed["archiveRef"].as_str().unwrap()
            ),
            format!("|line delete -D {line}"),
        ];
        assert_eq!(log.lines().collect::<Vec<_>>(), expected);
        assert!(!cwd.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_refused_or_unwanted_virtual_line_is_a_folder_line() {
        let files = tempfile::tempdir().expect("fixture");
        let files = files.path().canonicalize().expect("canonical fixture");
        let state = files.join("state");
        let source = files.join("source");
        fs::create_dir_all(&state).expect("state");
        fs::create_dir_all(&source).expect("source");
        let binary = script(&files, "layr", FAKE_LAYR.trim_start_matches("#!/bin/sh\n"));
        let _binary = super::super::override_layr_binary(binary);
        let mut lead = super::super::fixture_agent("lead", true);
        super::super::configure_new_chat(
            &mut lead,
            &crate::project_path::ProjectPath::from_stored(source.to_str().unwrap()),
        );
        let lead = prepared(&state, &lead.value());
        let root = super::super::store_path(&state);
        let commands = root.join("commands");

        // Setting off: a folder line, and no virtual-line command.
        fs::write(&commands, "").unwrap();
        let off = prepared(&state, &worker(&lead, "off"));
        let log = fs::read_to_string(&commands).unwrap();
        assert!(log.contains(&format!(
            "|line new {} main\n",
            off["layrLine"].as_str().unwrap()
        )));
        assert!(
            !log.contains("--view") && !log.contains("line show"),
            "{log}"
        );

        // Setting on, but Layr cannot mount: a folder line, and its saves stay automatic.
        super::super::settings::configure_virtual_agent_lines(&state, true).expect("setting");
        fs::write(root.join("no-views"), "").unwrap();
        fs::write(&commands, "").unwrap();
        let refused = prepared(&state, &worker(&lead, "refused"));
        let line = refused["layrLine"].as_str().unwrap();
        let log = fs::read_to_string(&commands).unwrap();
        assert!(
            log.contains(&format!("1|line new {line} main --view virtual\n")),
            "{log}"
        );
        assert!(log.contains(&format!("|line new {line} main\n")), "{log}");
        assert!(!is_virtual(
            &root,
            refused["layrProjectId"].as_str().unwrap(),
            line
        ));
        fs::write(&commands, "").unwrap();
        super::super::save(&state, &refused, "turn-save:refused:1").expect("turn save");
        let log = fs::read_to_string(&commands).unwrap();
        assert!(log.contains("|overlay list --json\n|save\n"), "{log}");
    }
}
