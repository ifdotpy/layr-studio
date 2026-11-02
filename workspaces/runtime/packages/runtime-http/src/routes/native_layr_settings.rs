//! Read-only native workspace settings.
use super::manifest::{OperationStatus, RouteGroup, get};
use crate::generated::{NativeLayrSettingsResponse, NativeLayrSettingsResponseVersion};
use crate::{error::ApiError, router::HttpState};
use axum::{Json, extract::State};
use studio_runtime_services::layr_native::{capability, settings, store_path, versions};

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![get(
        "/api/native-layr/settings",
        OperationStatus::Implemented,
        get_settings,
    )])
}

async fn get_settings(
    State(state): State<HttpState>,
) -> Result<Json<NativeLayrSettingsResponse>, ApiError> {
    studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-http/routes/native_layr_settings",
        move || {
            Json(settings_response(
                std::path::Path::new(&state.state_dir),
                capability::resolve_binary().ok().as_deref(),
            ))
        },
    )
    .await
    .map_err(|_| ApiError::internal())
}

fn settings_response(
    state: &std::path::Path,
    binary: Option<&std::path::Path>,
) -> NativeLayrSettingsResponse {
    let store = settings::selected_store(state);
    let root = store
        .as_ref()
        .cloned()
        .unwrap_or_else(|_| store_path(state));
    let version = binary
        .map(|binary| versions::binary_version(binary, &root))
        .transpose();
    let (store_root, store_error) = match store {
        Ok(root) => match root.into_os_string().into_string() {
            Ok(root) => (Some(root), None),
            Err(_) => (None, Some("Non-UTF-8 store root".to_owned())),
        },
        Err(error) => (None, Some(error.to_string())),
    };
    let (version, version_error) = match version {
        Ok(version) => (
            version.map(|identity| NativeLayrSettingsResponseVersion {
                version: identity.version,
                build: identity.build,
            }),
            None,
        ),
        Err(error) => (None, Some(error.to_string())),
    };
    NativeLayrSettingsResponse {
        version,
        store_root,
        error: store_error.or(version_error),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

<<<<<<< head
    #[test]
    fn reports_installed_identity_and_missing_binary() {
        let temp = tempfile::tempdir().expect("state");
        let binary = temp.path().join("layr");
        std::fs::write(&binary, "#!/bin/sh\nprintf '%s' '{\"binary\":{\"format\":1,\"version\":\"0.14.2\",\"build\":\"a6879be\"},\"store\":null,\"compatible\":true,\"state\":\"compatible\"}'\n").expect("fixture");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
=======
    #[tokio::test]
    async fn reports_installed_identity_and_missing_binary() {
        let temp = tempfile::tempdir().expect("state");
        let binary = temp.path().join("layr");
        tokio::fs::write(&binary, "#!/bin/sh\nprintf '%s' '{\"binary\":{\"format\":1,\"version\":\"0.14.2\",\"build\":\"a6879be\"},\"store\":null,\"compatible\":true,\"state\":\"compatible\"}'\n").await.expect("fixture");
        tokio::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
            .await
>>>>>>> agent-91aa5720c4daebdb35f5cf1935a651227062746655e5604dbba3659502b898bc
            .expect("executable");
        let found = settings_response(temp.path(), Some(&binary));
        let version = found.version.expect("version");
        assert_eq!(version.version, "0.14.2");
        assert_eq!(version.build, "a6879be");
        assert_eq!(
            found.store_root.as_deref(),
            store_path(temp.path()).to_str()
        );
        let missing = settings_response(temp.path(), None);
        assert!(missing.version.is_none());
        assert_eq!(missing.store_root, found.store_root);
    }
}

inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "native_layr_settings", routes: Some(routes), dependencies: &[],
    }
}
