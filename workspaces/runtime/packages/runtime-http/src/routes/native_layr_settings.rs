//! Native workspace settings.
use super::extractors::ApiJson;
use super::manifest::{OperationStatus, RouteGroup, get, post};
use crate::generated::{NativeLayrSettingsResponse, NativeLayrSettingsResponseVersion};
use crate::{error::ApiError, router::HttpState};
use axum::{Json, extract::State};
use studio_runtime_services::layr_native::{
    capability, settings, store_path, versions, virtual_lines_supported,
};

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![
        get(
            "/api/native-layr/settings",
            OperationStatus::Implemented,
            get_settings,
        ),
        post(
            "/api/native-layr/settings",
            OperationStatus::Implemented,
            set_settings,
        ),
    ])
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

async fn set_settings(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<crate::generated::NativeLayrSettingsRequest>,
) -> Result<Json<NativeLayrSettingsResponse>, ApiError> {
    let state_dir = std::path::PathBuf::from(&state.state_dir);
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let projects = store
        .read_async("native-layr/settings/projects", |db| {
            studio_runtime_store::ProjectsRepository::owned(db)
        })
        .await
        .map_err(|_| ApiError::internal())?;
    let local = store.local_server_id().map_err(|_| ApiError::internal())?;
    let result = studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-http/routes/native_layr_settings/write",
        move || {
            // A request with only `virtualAgentLines` keeps the store root.
            if body.store_root.is_none()
                && let Some(enabled) = body.virtual_agent_lines
            {
                set_virtual_agent_lines(
                    &state_dir,
                    enabled,
                    capability::resolve_binary().ok().as_deref(),
                )?;
                return Ok(Json(settings_response(
                    &state_dir,
                    capability::resolve_binary().ok().as_deref(),
                )));
            }
            let requested = body
                .store_root
                .as_ref()
                .and_then(Option::as_deref)
                .filter(|s| !s.is_empty());
            let default_root = store_path(&state_dir);
            let root = settings::validate_root(
                &state_dir,
                requested.map(std::path::Path::new).unwrap_or(&default_root),
            )
            .map_err(|e| ApiError::native_layr_store(e.to_string()))?;
            settings::check_projects_overlap(root.as_path(), &projects, local.as_deref())
                .map_err(|e| ApiError::native_layr_store(e.to_string()))?;
            if body.virtual_agent_lines == Some(true)
                && !capability::resolve_binary().ok().is_some_and(|binary| virtual_lines_supported(&binary, root.as_path()))
            {
                return Err(ApiError::native_layr_store(
                    "Virtual agent lines need a server and a Layr build that support them",
                ));
            }
            settings::configure_resolved_with_views(&state_dir, &root, requested.is_none(), body.virtual_agent_lines).map_err(|e| {
                match e {
                    studio_runtime_services::layr_native::NativeLayrError::Refused(_) => {
                        ApiError::native_layr_store(e.to_string())
                    }
                    studio_runtime_services::layr_native::NativeLayrError::Conflict(_)
                    | studio_runtime_services::layr_native::NativeLayrError::Unknown(_)
                    | studio_runtime_services::layr_native::NativeLayrError::Io(_)
                    | studio_runtime_services::layr_native::NativeLayrError::Store(_)
                    | studio_runtime_services::layr_native::NativeLayrError::Worker => {
                        ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                    }
                }
            })?;
            Ok::<_, ApiError>(Json(settings_response(
                &state_dir,
                capability::resolve_binary().ok().as_deref(),
            )))
        },
    )
    .await
    .map_err(|_| ApiError::internal())??;
    use studio_runtime_schema::{
        resource_notification::ResourceNotifyRequest,
        resource_ref::{EmptyResource, ResourceRef},
    };
    if let Ok(request) = ResourceNotifyRequest::new(
        uuid::Uuid::new_v4().to_string(),
        vec![ResourceRef::NativeLayrSettingsResource(EmptyResource {})],
    ) && let Err(error) = state.resource_notifications.publish_once(&request)
    {
        eprintln!("Could not publish native Layr settings change: {error}");
    }
    state.event_wake.notify_one();
    Ok(result)
}

/// Turn virtual agent lines on only where this server and its Layr build can make them.
fn set_virtual_agent_lines(
    state_dir: &std::path::Path,
    enabled: bool,
    binary: Option<&std::path::Path>,
) -> Result<(), ApiError> {
    if enabled && !virtual_agent_lines_available(state_dir, binary) {
        return Err(ApiError::native_layr_store(
            "Virtual agent lines need a server and a Layr build that support them",
        ));
    }
    settings::configure_virtual_agent_lines(state_dir, enabled)
        .map_err(|e| ApiError::native_layr_store(e.to_string()))
}

fn virtual_agent_lines_available(
    state: &std::path::Path,
    binary: Option<&std::path::Path>,
) -> bool {
    let root = settings::selected_store(state).unwrap_or_else(|_| store_path(state));
    binary.is_some_and(|binary| virtual_lines_supported(binary, &root))
}

pub(super) fn settings_response(
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
        configured: settings::configured_store(state).ok().flatten().is_some(),
        virtual_agent_lines: settings::virtual_agent_lines(state),
        virtual_agent_lines_available: virtual_agent_lines_available(state, binary),
        readiness: Some(capability::for_store(state, &root)),
        version,
        store_root,
        error: store_error.or(version_error),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn reports_installed_identity_and_missing_binary() {
        let temp = tempfile::tempdir().expect("state");
        let binary = temp.path().join("layr");
        tokio::fs::write(&binary, "#!/bin/sh\nprintf '%s' '{\"binary\":{\"format\":1,\"version\":\"0.14.2\",\"build\":\"a6879be\"},\"store\":null,\"compatible\":true,\"state\":\"compatible\"}'\n").await.expect("fixture");
        tokio::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
            .await
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
        assert!(!found.virtual_agent_lines && !found.virtual_agent_lines_available);
    }

    #[tokio::test]
    async fn virtual_agent_lines_turn_on_only_where_layr_supports_them() {
        let temp = tempfile::tempdir().expect("state");
        tokio::fs::create_dir_all(store_path(temp.path()).join("projects"))
            .await
            .expect("store");
        let old = temp.path().join("layr-old");
        let views = temp.path().join("layr-views");
        for (binary, usage) in [
            (&old, "usage: layr line new/list/delete/rename/move"),
            (
                &views,
                "usage: layr line show <line> --folder [<path>] | --none | --virtual",
            ),
        ] {
            tokio::fs::write(binary, format!("#!/bin/sh\necho '{usage}' >&2; exit 2\n"))
                .await
                .expect("fixture");
            tokio::fs::set_permissions(binary, std::fs::Permissions::from_mode(0o755))
                .await
                .expect("executable");
        }
        assert!(set_virtual_agent_lines(temp.path(), true, Some(&old)).is_err());
        assert!(set_virtual_agent_lines(temp.path(), true, None).is_err());
        assert!(!settings::virtual_agent_lines(temp.path()));
        let on = set_virtual_agent_lines(temp.path(), true, Some(&views));
        assert_eq!(on.is_ok(), cfg!(target_os = "macos"));
        let response = settings_response(temp.path(), Some(&views));
        assert_eq!(
            response.virtual_agent_lines_available,
            cfg!(target_os = "macos")
        );
        assert_eq!(response.virtual_agent_lines, cfg!(target_os = "macos"));
        // Turning off needs no support, so an older Layr can still turn it off.
        set_virtual_agent_lines(temp.path(), false, Some(&old)).expect("off");
        assert!(!settings_response(temp.path(), Some(&old)).virtual_agent_lines);
    }
}

inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "native_layr_settings", routes: Some(routes), dependencies: &[],
    }
}
