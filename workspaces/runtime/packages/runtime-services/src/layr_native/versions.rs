//! Inspect the Layr-owned store format before Studio prepares a native line.
use super::*;
use std::time::Duration;

type Result<T> = std::result::Result<T, NativeLayrError>;

#[derive(serde::Deserialize)]
struct Identity {
    format: u64,
    version: String,
    build: String,
}
#[derive(serde::Deserialize)]
struct Inspection {
    binary: Identity,
    store: Option<Identity>,
    compatible: bool,
    state: String,
    #[serde(default)]
    upgrade: Option<Upgrade>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Upgrade {
    from_format: u64,
    to_format: u64,
    metadata_only: bool,
}

fn inspect(binary: &Path, root: &Path) -> Result<Inspection> {
    let root = root
        .to_str()
        .ok_or_else(|| NativeLayrError::Refused("Non-UTF-8 store root".into()))?;
    let result = crate::bounded_child::run_without_git_environment(crate::bounded_child::Request {
        program: binary.as_os_str(), git_directory: None,
        args: &["--root", root, "store-version", "--json"],
        environment: &[("LAYR_DIRECT", "1")], input: None,
        stdout_limit: 16 * 1024, stderr_limit: 4096, timeout: Duration::from_secs(10),
    }).map_err(|e| NativeLayrError::Refused(format!("Cannot inspect Layr at {} for store {root}: {e:?}. Upgrade this binary to the Studio Layr release; see docs/native-layr.md.", binary.display())))?;
    serde_json::from_slice(&result.stdout.bytes).map_err(|e| NativeLayrError::Refused(format!("Layr at {} has no valid store-version response for {root}: {e}. Upgrade this binary; see docs/native-layr.md.", binary.display())))
}

pub(super) fn verify(state: &Path, root: &Path) -> Result<Option<String>> {
    let executable = super::capability::resolve_binary().map_err(|capability| {
        NativeLayrError::Refused(capability.message().unwrap_or("Layr is unavailable").into())
    })?;
    verify_with_binary(state, root, &executable)
}

fn verify_with_binary(state: &Path, root: &Path, executable: &Path) -> Result<Option<String>> {
    let mut installed = inspect(executable, root)?;
    if !installed.compatible
        && settings::owns_store(state, root)
        && let Some(upgrade) = &installed.upgrade
        && upgrade.metadata_only
        && upgrade.from_format < upgrade.to_format
        && upgrade.to_format == installed.binary.format
        && (installed.state == "legacy"
            || installed
                .store
                .as_ref()
                .is_some_and(|version| version.format == upgrade.from_format))
    {
        super::runtime::operation_in_store(
            state,
            root,
            &format!("store-upgrade:{}", upgrade.to_format),
            &json!({"kind":"store-upgrade","root":root,"fromFormat":upgrade.from_format,"toFormat":upgrade.to_format}),
            || {
                super::runtime::run_with_binary(
                    state,
                    root,
                    state,
                    &["store-version", "--upgrade"],
                    executable,
                )?;
                Ok(json!({"storeRoot":root,"format":upgrade.to_format}))
            },
        )?;
        installed = inspect(executable, root)?;
    }
    if !installed.compatible {
        let written = installed
            .store
            .as_ref()
            .map(|store| {
                format!(
                    "{} (format {}, build {})",
                    store.version, store.format, store.build
                )
            })
            .unwrap_or_else(|| installed.state.clone());
        return Err(NativeLayrError::Refused(format!(
            "Store {} was written by {written}. Studio uses {} (format {}, build {}). Use native-layr upgrade-store for a legacy store; see docs/native-layr.md for the switch procedure.",
            root.display(),
            installed.binary.version,
            installed.binary.format,
            installed.binary.build
        )));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn native_layr_owned_store_adopts_metadata_once_and_selected_store_remains_untouched() {
        use std::{fs, os::unix::fs::PermissionsExt};
        let fixture = tempfile::tempdir().expect("fixture");
        let state = fixture.path().join("state");
        let root = store_path(&state);
        fs::create_dir_all(&root).expect("legacy store");
        fs::write(root.join("legacy-data"), "preserved").expect("legacy data");
        let script = r#"#!/bin/sh
case "$*" in
  *--json)
    if [ -f "$2/upgraded" ]; then
      printf '%s\n' '{"binary":{"format":1,"version":"0.5.1","build":"fixture"},"store":{"format":1,"version":"0.5.1","build":"fixture"},"compatible":true,"state":"compatible"}'
    else
      printf '%s\n' '{"binary":{"format":1,"version":"0.5.1","build":"fixture"},"store":null,"compatible":false,"state":"legacy","upgrade":{"fromFormat":0,"toFormat":1,"metadataOnly":true}}'
    fi ;;
  *--upgrade)
    printf 'upgrade\n' >> "$LAYR_ROOT/calls"
    printf 'complete\n' > "$LAYR_ROOT/upgraded" ;;
esac
"#;
        fs::create_dir_all(binary_path(&state).parent().expect("bin")).expect("bin");
        let binary = binary_path(&state);
        fs::write(&binary, script).expect("binary fixture");
        fs::set_permissions(binary, fs::Permissions::from_mode(0o755)).expect("executable");
        verify_with_binary(&state, &root, &binary_path(&state)).expect("metadata adoption");
        verify_with_binary(&state, &root, &binary_path(&state)).expect("repeat is a no-op");
        assert_eq!(
            fs::read_to_string(root.join("calls")).expect("calls"),
            "upgrade\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("legacy-data")).expect("data"),
            "preserved"
        );
        let developer = fixture.path().join("developer-store");
        fs::create_dir(&developer).expect("developer store");
        let error = verify_with_binary(&state, &developer, &binary_path(&state))
            .expect_err("operator store requires explicit upgrade")
            .to_string();
        assert!(error.contains("upgrade-store"));
        assert!(!developer.join("upgraded").exists());
        assert!(
            fs::read_dir(developer)
                .expect("developer store")
                .next()
                .is_none()
        );
    }
}
