//! Host store selection. Existing chats retain their recorded store.
use super::*;
use std::fs;

type Result<T> = std::result::Result<T, NativeLayrError>;

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub store_root: Option<PathBuf>,
    /// New worker lines are virtual Layr lines (`virtual_lines`). Off by default; the key is
    /// written only when on, so a Studio without the setting still reads the file.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub virtual_agent_lines: bool,
}

fn path(state: &Path) -> PathBuf {
    state.join(NATIVE_LAYR_DIRECTORY).join("settings.json")
}

fn read(state: &Path) -> Result<Settings> {
    match fs::read(path(state)) {
        Ok(bytes) => studio_json::from_slice::<Settings>(&bytes)
            .map_err(|e| NativeLayrError::Refused(format!("Invalid native Layr settings: {e}"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e.into()),
    }
}

fn write(state: &Path, settings: &Settings) -> Result<()> {
    fs::create_dir_all(state.join(NATIVE_LAYR_DIRECTORY))?;
    crate::atomic_file::replace(
        &path(state),
        studio_json::to_string(settings)
            .map_err(|e| NativeLayrError::Refused(e.to_string()))?
            .as_bytes(),
        Some(0o600),
        true,
    )?;
    Ok(())
}

/// Whether new worker lines should be virtual. Invalid settings select folder lines.
pub fn virtual_agent_lines(state: &Path) -> bool {
    read(state).is_ok_and(|settings| settings.virtual_agent_lines)
}

/// Turn virtual agent lines on or off. The store root stays. Existing lines keep their view.
pub fn configure_virtual_agent_lines(state: &Path, enabled: bool) -> Result<()> {
    let mut settings = read(state)?;
    settings.virtual_agent_lines = enabled;
    write(state, &settings)
}

pub fn configured_store(state: &Path) -> Result<Option<PathBuf>> {
    let settings = read(state)?;
    if settings
        .store_root
        .as_ref()
        .is_some_and(|root| !root.is_absolute())
    {
        return Err(NativeLayrError::Refused(
            "The native Layr store root must be an existing absolute folder".into(),
        ));
    }
    Ok(settings.store_root)
}

/// The configured spelling is usable for path validation even while the store is offline.
pub(crate) fn store_for_validation(state: &Path) -> PathBuf {
    configured_store(state)
        .ok()
        .flatten()
        .unwrap_or_else(|| store_path(state))
}

pub fn selected_store(state: &Path) -> Result<PathBuf> {
    match configured_store(state)? {
        Some(root) if root.is_absolute() && root.is_dir() => {
            Ok(CanonicalPath::existing(root)?.into_path_buf())
        }
        Some(_) => Err(NativeLayrError::Refused(
            "The native Layr store root must be an existing absolute folder".into(),
        )),
        None => Ok(store_path(state)),
    }
}

pub fn resolve_for_new_chat(state: &Path, root: Option<&Path>) -> Result<PathBuf> {
    match root {
        Some(root) => Ok(root.to_owned()),
        None => selected_store(state),
    }
}

pub fn check_project_overlap(root: &Path, project: &Path) -> Result<()> {
    let project = CanonicalPath::missing_tail(project)
        .ok_or_else(|| NativeLayrError::Refused("Cannot resolve the project folder".into()))?
        .into_path_buf();
    // Compare canonical paths on both sides: /var is a link to /private/var on macOS.
    let root = CanonicalPath::missing_tail(root)
        .ok_or_else(|| NativeLayrError::Refused("Cannot resolve the Layr store".into()))?
        .into_path_buf();
    let root = root.as_path();
    if root.starts_with(&project) || project.starts_with(root) {
        return Err(NativeLayrError::Refused(
            "The Layr store must be outside the project folder".into(),
        ));
    }
    Ok(())
}

/// Check every registered folder on this server, including logical project locations.
pub fn check_projects_overlap(
    root: &Path,
    projects: &[(String, Value)],
    local: Option<&str>,
) -> Result<()> {
    for (_, project) in projects {
        let paths = project
            .get("path")
            .and_then(Value::as_str)
            .into_iter()
            .chain(
                project
                    .get("locations")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|location| {
                        (location.get("serverId").and_then(Value::as_str) == local
                            && local.is_some())
                        .then(|| location.get("path").and_then(Value::as_str))
                        .flatten()
                    }),
            );
        for path in paths.filter(|path| Path::new(path).is_absolute()) {
            check_project_overlap(root, Path::new(path))?;
        }
    }
    Ok(())
}

pub fn resolve_root(root: &Path) -> Result<CanonicalPath> {
    if !root.is_absolute() || !root.is_dir() {
        return Err(NativeLayrError::Refused(
            "The native Layr store root must be an existing absolute folder".into(),
        ));
    }
    Ok(CanonicalPath::existing(root)?)
}

pub fn validate_resolved_root(state: &Path, root: &CanonicalPath) -> Result<()> {
    let state = CanonicalPath::existing(state)?.into_path_buf();
    if (root.starts_with(&state) || state.starts_with(root.as_path()))
        && !same_root(root, &store_path(&state))
    {
        return Err(NativeLayrError::Refused(
            "The native Layr store must be outside the Studio state directory".into(),
        ));
    }
    let capability = super::capability::for_store(&state, root);
    if !capability.is_ready() {
        return Err(NativeLayrError::Refused(
            capability
                .message()
                .unwrap_or("Native Layr is unavailable")
                .into(),
        ));
    }
    Ok(())
}

pub fn validate_root(state: &Path, root: &Path) -> Result<CanonicalPath> {
    let root = resolve_root(root)?;
    validate_resolved_root(state, &root)?;
    Ok(root)
}

/// Change only the destination for new imports and mappings. Existing roots stay intact.
pub fn configure(state: &Path, root: Option<&Path>) -> Result<Value> {
    let validated = validate_root(state, root.unwrap_or(&store_path(state)))?;
    configure_resolved(state, &validated, root.is_none())
}

/// Persist the same canonical root that passed validation and overlap checks.
pub fn configure_resolved(state: &Path, root: &CanonicalPath, use_default: bool) -> Result<Value> {
    configure_resolved_with_views(state, root, use_default, None)
}

/// Persist one validated settings request in one atomic replacement.
pub fn configure_resolved_with_views(
    state: &Path,
    root: &CanonicalPath,
    use_default: bool,
    virtual_agent_lines: Option<bool>,
) -> Result<Value> {
    let mut settings = read(state)?;
    settings.store_root = (!use_default).then(|| root.as_path().to_owned());
    if let Some(enabled) = virtual_agent_lines {
        settings.virtual_agent_lines = enabled;
    }
    write(state, &settings)?;
    Ok(json!({"storeRoot":root}))
}

#[cfg(test)]
pub(crate) fn fixture_select(state: &Path, root: &Path) {
    fs::create_dir_all(state.join(NATIVE_LAYR_DIRECTORY)).expect("settings folder");
    fs::write(
        path(state),
        studio_json::to_string(&Settings {
            store_root: Some(root.to_owned()),
            virtual_agent_lines: false,
        })
        .unwrap(),
    )
    .expect("settings");
}

pub(super) fn same_root(left: &Path, right: &Path) -> bool {
    CanonicalPath::existing(left)
        .map(CanonicalPath::into_path_buf)
        .unwrap_or_else(|_| left.to_owned())
        == CanonicalPath::existing(right)
            .map(CanonicalPath::into_path_buf)
            .unwrap_or_else(|_| right.to_owned())
}

/// Keep the original metadata location for old Studio imports.
pub(super) fn metadata(state: &Path, root: &Path) -> PathBuf {
    let base = state.join(NATIVE_LAYR_DIRECTORY);
    if same_root(root, &store_path(state)) {
        base
    } else {
        base.join("stores").join(studio_runtime_store::sha256_hex(
            root.to_string_lossy().as_bytes(),
        ))
    }
}

pub(super) fn root_from_cwd(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors().find_map(|line| {
        let lines = line.parent()?;
        let project = lines.parent()?;
        let projects = project.parent()?;
        if lines.file_name()? == "lines" && projects.file_name()? == "projects" {
            Some(projects.parent()?.to_owned())
        } else {
            None
        }
    })
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum StoreAssociation {
    Recorded(PathBuf),
    LegacyLine { cwd: PathBuf, reviewer: bool },
    Selected,
}

impl StoreAssociation {
    pub(crate) fn resolve(&self, state: &Path) -> Result<PathBuf> {
        match self {
            Self::Recorded(root) => Ok(root.clone()),
            Self::LegacyLine { cwd, .. } => root_from_cwd(cwd).ok_or_else(|| {
                NativeLayrError::Refused("Invalid native Layr cwd association".into())
            }),
            Self::Selected => selected_store(state),
        }
    }
}

pub(crate) fn store_association(record: &Value) -> Result<StoreAssociation> {
    if let Some(root) = record["layrStoreRoot"].as_str() {
        let root = PathBuf::from(root);
        if !root.is_absolute() {
            return Err(NativeLayrError::Refused(
                "Invalid native Layr store association".into(),
            ));
        }
        return Ok(StoreAssociation::Recorded(root));
    }
    // Records from before store selection have an authoritative saved line path.
    if record["layrReady"] == true {
        let cwd = record[if record["role"] == "reviewer" {
            "layrReadCwd"
        } else {
            "cwd"
        }]
        .as_str()
        .ok_or_else(|| NativeLayrError::Refused("Missing native Layr cwd".into()))?;
        return Ok(StoreAssociation::LegacyLine {
            cwd: PathBuf::from(cwd),
            reviewer: record["role"] == "reviewer",
        });
    }
    Ok(StoreAssociation::Selected)
}

pub fn record_store(state: &Path, record: &Value) -> Result<PathBuf> {
    store_association(record)?.resolve(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_resolved_reset_retains_the_builtin_default() {
        let state = tempfile::tempdir().unwrap();
        let selected = tempfile::tempdir().unwrap();
        let root = CanonicalPath::existing(selected.path()).unwrap();
        configure_resolved(state.path(), &root, true).unwrap();
        assert!(configured_store(state.path()).unwrap().is_none());
        assert_eq!(
            selected_store(state.path()).unwrap(),
            store_path(state.path())
        );
    }

    #[cfg(unix)]
    #[test]
    fn configure_persists_the_checked_canonical_root_after_link_changes() {
        let state = tempfile::tempdir().unwrap();
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let link = state.path().join("store-link");
        std::os::unix::fs::symlink(first.path(), &link).unwrap();
        let checked = CanonicalPath::existing(&link).unwrap();
        let project = tempfile::tempdir().unwrap();
        check_project_overlap(checked.as_path(), project.path()).unwrap();
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(second.path(), &link).unwrap();
        configure_resolved(state.path(), &checked, false).unwrap();
        assert_eq!(
            configured_store(state.path()).unwrap().unwrap(),
            checked.into_path_buf()
        );
    }

    #[test]
    fn native_layr_rejects_project_store_overlap() {
        let project = tempfile::tempdir().expect("project");
        let store = project.path().join("store");
        assert!(check_project_overlap(&store, project.path()).is_err());
        assert!(check_project_overlap(project.path(), &store).is_err());
        let other = tempfile::tempdir().expect("other");
        assert!(check_project_overlap(other.path(), project.path()).is_ok());
        let projects = vec![(
            "project:logical".to_owned(),
            json!({"path":"project:logical", "locations":[{"serverId":"local", "path":project.path()}]}),
        )];
        assert!(check_projects_overlap(&store, &projects, Some("local")).is_err());
        assert!(check_projects_overlap(&store, &projects, Some("remote")).is_ok());
    }

    #[test]
    fn native_layr_rejects_a_store_inside_the_state_directory() {
        let state = tempfile::tempdir().expect("state");
        let root = state.path().join("other");
        fs::create_dir(&root).unwrap();
        let error = validate_root(state.path(), &root).unwrap_err();
        assert!(error.to_string().contains("outside the Studio state"));
    }

    #[test]
    fn native_layr_chat_store_override_wins_over_default() {
        let state = tempfile::tempdir().expect("state");
        let default = tempfile::tempdir().expect("default");
        let override_root = tempfile::tempdir().expect("override");
        std::fs::create_dir_all(state.path().join(NATIVE_LAYR_DIRECTORY)).unwrap();
        std::fs::write(
            path(state.path()),
            studio_json::to_string(&Settings {
                store_root: Some(default.path().to_owned()),
                virtual_agent_lines: false,
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            resolve_for_new_chat(state.path(), Some(override_root.path())).unwrap(),
            override_root.path()
        );
        assert_eq!(
            resolve_for_new_chat(state.path(), None).unwrap(),
            CanonicalPath::existing(default.path())
                .unwrap()
                .into_path_buf()
        );
    }

    #[test]
    fn native_layr_root_switch_keeps_existing_chats_on_their_original_store() {
        let state = tempfile::tempdir().expect("state");
        let old = store_path(state.path());
        let other = tempfile::tempdir().expect("other store");
        let record = json!({"layrStoreRoot": old, "layrReady":true});
        fs::create_dir_all(state.path().join(NATIVE_LAYR_DIRECTORY)).expect("settings folder");
        fs::write(
            state
                .path()
                .join(NATIVE_LAYR_DIRECTORY)
                .join("settings.json"),
            json!({"storeRoot":other.path()}).to_string(),
        )
        .expect("settings");
        assert!(same_root(
            &selected_store(state.path()).expect("selected store"),
            other.path()
        ));
        assert_eq!(
            record_store(state.path(), &record).expect("bound root"),
            old
        );
        assert_eq!(
            thread_environment_for_record(state.path(), &record).expect("environment")["LAYR_ROOT"],
            json!(old)
        );
        let legacy = json!({"layrReady":true,"cwd":old.join("projects/old/lines/main")});
        assert_eq!(
            record_store(state.path(), &legacy).expect("legacy root"),
            old
        );
        assert_ne!(
            metadata(state.path(), &old),
            metadata(state.path(), other.path())
        );
    }

    #[test]
    fn virtual_agent_lines_are_off_by_default_and_survive_a_store_change() {
        let state = tempfile::tempdir().unwrap();
        assert!(!virtual_agent_lines(state.path()));
        configure_virtual_agent_lines(state.path(), true).unwrap();
        assert!(virtual_agent_lines(state.path()));
        let selected = tempfile::tempdir().unwrap();
        let root = CanonicalPath::existing(selected.path()).unwrap();
        configure_resolved(state.path(), &root, false).unwrap();
        assert!(virtual_agent_lines(state.path()));
        assert_eq!(
            configured_store(state.path()).unwrap().unwrap(),
            root.as_path()
        );
        // Off removes the key, so a Studio without the setting reads the file again.
        configure_virtual_agent_lines(state.path(), false).unwrap();
        let text = fs::read_to_string(path(state.path())).unwrap();
        assert!(!text.contains("virtualAgentLines"), "{text}");
        assert_eq!(
            configured_store(state.path()).unwrap().unwrap(),
            root.as_path()
        );
        fs::write(path(state.path()), r#"{"virtualAgentLines":"yes"}"#).unwrap();
        assert!(!virtual_agent_lines(state.path()));
        assert!(configure_virtual_agent_lines(state.path(), true).is_err());
    }

    #[test]
    fn native_layr_invalid_settings_never_select_the_default_store() {
        let state = tempfile::tempdir().expect("state");
        fs::create_dir_all(state.path().join(NATIVE_LAYR_DIRECTORY)).expect("folder");
        fs::write(path(state.path()), "{broken").expect("settings");
        assert!(selected_store(state.path()).is_err());
        assert!(configure(state.path(), Some(Path::new("relative"))).is_err());
    }

    #[test]
    fn native_layr_selected_store_has_its_own_service_socket() {
        let state = tempfile::tempdir().expect("state");
        let other = tempfile::tempdir().expect("store");
        fixture_select(state.path(), other.path());
        let env = super::super::transport_environment_in_store(
            state.path(),
            &selected_store(state.path()).expect("store"),
            true,
        );
        assert_eq!(
            env["LAYR_SOCKET"],
            json!(selected_socket_path(state.path()).expect("socket"))
        );
        assert_ne!(env["LAYR_SOCKET"], json!(socket_path(state.path())));
    }
}
