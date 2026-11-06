//! Project folder identity at creation and in agent projections.
use serde::Serialize;
use std::path::Path;
use studio_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub(crate) struct ProjectPath(String);

impl ProjectPath {
    pub(crate) fn new(path: String, store: &Path) -> Result<Self, &'static str> {
        let is_line = |root: &Path| {
            let projects = root.join("projects");
            let Ok(relative) = Path::new(&path).strip_prefix(projects) else {
                return false;
            };
            let mut parts = relative.components();
            parts.next().is_some()
                && parts.next().is_some_and(|part| part.as_os_str() == "lines")
                && parts.next().is_some()
        };
        if is_line(store)
            || studio_runtime_store::path_identity::CanonicalPath::missing_tail(store)
                .is_some_and(|root| is_line(root.as_path()))
        {
            return Err("Select the project folder, not a Layr line");
        }
        Ok(Self(path))
    }

    /// Stored records predate this validation and are projected without filesystem I/O.
    pub(crate) fn from_stored(path: &str) -> Self {
        Self(path.to_owned())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) fn project_path(record: &Value) -> Option<ProjectPath> {
    project_path_from_fields(
        record.get("hostProjectPath").and_then(Value::as_str),
        record.get("cwd").and_then(Value::as_str),
    )
}

pub(crate) fn project_path_from_fields(
    host_project_path: Option<&str>,
    cwd: Option<&str>,
) -> Option<ProjectPath> {
    // Summary and full use the same unbounded path, including paths over 12,000 characters.
    host_project_path.or(cwd).map(ProjectPath::from_stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn unresolved_store_still_checks_lexical_line_paths() {
        let dir = tempfile::tempdir().expect("temporary store parent");
        let store = dir.path().join("store");
        std::os::unix::fs::symlink("missing", &store).expect("create dangling store link");
        assert!(
            ProjectPath::new(dir.path().join("project").to_string_lossy().into(), &store).is_ok()
        );
        assert!(
            ProjectPath::new(
                store
                    .join("projects/id/lines/main")
                    .to_string_lossy()
                    .into(),
                &store
            )
            .is_err()
        );
    }
}
