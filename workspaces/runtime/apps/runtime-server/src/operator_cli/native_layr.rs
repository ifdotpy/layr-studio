//! Host Layr installation and migration do not open a second Studio backend.
use super::Result;
use std::{path::PathBuf, process::ExitCode};
use studio_runtime_store::path_identity::CanonicalPath;

fn should_print_progress(
    last: Option<&(
        std::time::Instant,
        studio_runtime_schema::layr_progress::CopyPhase,
    )>,
    value: &studio_runtime_schema::layr_progress::CopyProgress,
) -> bool {
    value.total == Some(value.done)
        || !last.is_some_and(|(at, phase)| {
            *phase == value.phase
                && at.elapsed()
                    < studio_runtime_services::layr_native::migration::PROGRESS_PUBLICATION_INTERVAL
        })
}

pub(super) fn run(args: &[String]) -> Result<ExitCode> {
    let mut args = args.iter();
    let command = args
        .next()
        .map(String::as_str)
        .ok_or("Use native-layr <configure|map|migrate|paths>")?;
    let mut state = None;
    let mut store_root = None;
    let mut mapped_project = None;
    let mut default_store = false;
    let mut dry_run = false;
    let mut projects = Vec::new();
    let mut without_git = std::collections::BTreeSet::new();
    let mut skip_worktrees = Vec::new();
    let mut worktrees = studio_runtime_services::layr_native::migration::Worktrees::All;
    let mut worktrees_explicit = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--store-root" => {
                store_root = Some(PathBuf::from(
                    args.next()
                        .ok_or("--store-root requires an absolute folder")?,
                ))
            }
            "--project" => {
                mapped_project = Some(
                    args.next()
                        .ok_or("--project requires an existing project name")?
                        .clone(),
                )
            }
            "--default-store" => default_store = true,
            "--state" => {
                state = Some(PathBuf::from(
                    args.next().ok_or("--state requires a directory")?,
                ))
            }
            "--dry-run" => dry_run = true,
            "--skip-worktree" => {
                let path = PathBuf::from(
                    args.next()
                        .ok_or("--skip-worktree requires a worktree path")?,
                );
                let path = if path.is_absolute() {
                    path
                } else {
                    std::env::current_dir()?.join(path)
                };
                skip_worktrees.push(
                    CanonicalPath::existing(&path)
                        .map(CanonicalPath::into_path_buf)
                        .unwrap_or(path),
                );
            }
            "--worktrees" => {
                worktrees_explicit = true;
                worktrees = studio_runtime_services::layr_native::migration::Worktrees::parse(
                    args.next().ok_or("--worktrees requires dirty or all")?,
                )
                .ok_or("--worktrees requires dirty or all")?;
            }
            "--without-git" => {
                let path = PathBuf::from(
                    args.next()
                        .ok_or("--without-git requires a project directory")?,
                );
                without_git
                    .insert(CanonicalPath::existing(&path).map(CanonicalPath::into_path_buf)?);
                projects.push(path);
            }
            value if value.starts_with('-') => {
                return Err(format!("Unknown native-layr option: {value}").into());
            }
            value => projects.push(PathBuf::from(value)),
        }
    }
    if command != "migrate"
        && (!without_git.is_empty() || !skip_worktrees.is_empty() || worktrees_explicit)
    {
        return Err(
            "--without-git, --skip-worktree and --worktrees are available only for native-layr migrate".into(),
        );
    }
    if command != "configure" && (store_root.is_some() || default_store) {
        return Err(
            "--store-root and --default-store are available only for native-layr configure".into(),
        );
    }
    if command != "map" && mapped_project.is_some() {
        return Err("--project is available only for native-layr map".into());
    }
    let state = state.unwrap_or(studio_runtime_store::resolve_state_dir()?.into_path_buf());
    let state = if state.is_absolute() {
        state
    } else {
        std::env::current_dir()?.join(state)
    };
    match command {
        "configure" => {
            if default_store == store_root.is_some() {
                return Err("Supply either --store-root or --default-store".into());
            }
            let default_root = studio_runtime_services::layr_native::store_path(&state);
            let root = studio_runtime_services::layr_native::settings::validate_root(
                &state,
                store_root.as_deref().unwrap_or(&default_root),
            )?;
            for project in &projects {
                let project = if project.is_absolute() {
                    project.clone()
                } else {
                    std::env::current_dir()?.join(project)
                };
                studio_runtime_services::layr_native::settings::check_project_overlap(
                    root.as_path(),
                    &project,
                )?;
            }
            println!(
                "{}",
                studio_runtime_services::layr_native::settings::configure_resolved(
                    &state,
                    &root,
                    default_store
                )?
            );
        }
        "map" => {
            if projects.len() != 1 {
                return Err("Supply one source folder and --project <existing name>".into());
            }
            println!(
                "{}",
                studio_runtime_services::layr_native::mapping::bind(
                    &state,
                    &projects[0],
                    mapped_project
                        .as_deref()
                        .ok_or("Supply --project <existing name>")?
                )?
            );
        }
        "paths" => println!(
            "{}",
            studio_json::json!({
                "store":studio_runtime_services::layr_native::settings::selected_store(&state)?,
                "binary":studio_runtime_services::layr_native::resolve_binary().ok(),
                "socket":studio_runtime_services::layr_native::selected_socket_path(&state)?,
            })
        ),
        "migrate" => {
            if projects.is_empty() {
                return Err("Supply at least one project folder".into());
            }
            let mut seen = std::collections::BTreeSet::new();
            for project in projects {
                let project = CanonicalPath::existing(project).map(CanonicalPath::into_path_buf)?;
                if !seen.insert(project.clone()) {
                    continue;
                }
                let options = studio_runtime_services::layr_native::migration::Options {
                    without_git: without_git.contains(&project),
                    worktrees,
                    skip_worktrees: skip_worktrees.clone(),
                };
                let last = std::sync::Mutex::new(
                    None::<(
                        std::time::Instant,
                        studio_runtime_schema::layr_progress::CopyPhase,
                    )>,
                );
                let progress: studio_runtime_services::layr_native::migration::ProgressSink =
                    std::sync::Arc::new(
                        move |value: studio_runtime_schema::layr_progress::CopyProgress| {
                            let mut last = last.lock().expect("progress lock");
                            if !should_print_progress(last.as_ref(), &value) {
                                return;
                            }
                            eprintln!(
                                "{}",
                                studio_json::to_string(&value).expect("progress serialization")
                            );
                            *last = Some((std::time::Instant::now(), value.phase));
                        },
                    );
                println!(
                    "{}",
                    studio_json::to_string_pretty(
                        &studio_runtime_services::layr_native::migration::migrate_with_progress(
                            &state,
                            &project,
                            dry_run,
                            options,
                            Some(&progress)
                        )?
                    )?
                );
            }
        }
        _ => {
            return Err("Use native-layr <configure|map|migrate|paths>".into());
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use studio_runtime_schema::layr_progress::{CopyPhase, CopyProgress, CopyUnit};

    #[test]
    fn native_layr_upgrade_store_is_not_an_operator_command() {
        let fixture = tempfile::tempdir().expect("fixture");
        let state = fixture.path().join("absent-state");
        let error = run(&[
            "upgrade-store".into(),
            "--state".into(),
            state.to_str().expect("state path").into(),
        ])
        .expect_err("unsupported command")
        .to_string();
        assert_eq!(error, "Use native-layr <configure|map|migrate|paths>");
        assert!(!state.exists());
    }

    #[test]
    fn final_count_is_printed_inside_throttle_interval() {
        let mut value = CopyProgress {
            phase: CopyPhase::Staging,
            done: 0,
            total: Some(1),
            unit: CopyUnit::Files,
        };
        assert!(should_print_progress(None, &value));
        let last = Some((std::time::Instant::now(), value.phase.clone()));
        assert!(!should_print_progress(last.as_ref(), &value));
        value.done = 1;
        assert!(should_print_progress(last.as_ref(), &value));
        value.phase = CopyPhase::CreatingLine;
        value.total = None;
        assert!(should_print_progress(last.as_ref(), &value));
    }
}
