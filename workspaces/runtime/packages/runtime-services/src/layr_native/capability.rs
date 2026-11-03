//! Read-only host readiness for native workspace selection.
use std::path::{Path, PathBuf};
pub use studio_runtime_schema::native_layr::NativeLayrCapability;

fn from_result(result: Result<(), String>) -> NativeLayrCapability {
    match result {
        Ok(()) => NativeLayrCapability::Ready,
        Err(reason) => NativeLayrCapability::NotReady { reason },
    }
}

/// Linux needs a btrfs store and the root daemon for this exact Studio state.
/// This probe sends no daemon request and never creates directories or a store.
pub fn capability(state_dir: &Path) -> NativeLayrCapability {
    if let Err(unavailable) = resolve_binary() {
        return unavailable;
    }
    #[cfg(target_os = "macos")]
    {
        from_result(
            super::settings::selected_store(state_dir)
                .map(|_| ())
                .map_err(|e| e.to_string()),
        )
    }
    #[cfg(target_os = "linux")]
    {
        from_result(linux_readiness(state_dir))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = state_dir;
        from_result(Err("Native Layr is unavailable on this server.".into()))
    }
}

pub fn resolve_binary() -> Result<PathBuf, NativeLayrCapability> {
    #[cfg(test)]
    if let Some(binary) = TEST_BINARY.with(|binary| binary.borrow().clone()) {
        return Ok(binary);
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    resolve_binary_in_path(&path)
}

#[cfg(test)]
thread_local! {
    pub(super) static TEST_BINARY: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

fn resolve_binary_in_path(path: &std::ffi::OsStr) -> Result<PathBuf, NativeLayrCapability> {
    let binary = std::env::split_paths(path)
        .map(|dir| dir.join("layr"))
        .find(|path| {
            let Ok(metadata) = std::fs::metadata(path) else {
                return false;
            };
            if !matches!(
                super::git_guard::has_marker(path, super::git_guard::LAUNCHER_MARKER),
                Ok(false)
            ) {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                metadata.is_file()
            }
        })
        .ok_or(NativeLayrCapability::NotInstalled)?;
    studio_runtime_store::path_identity::CanonicalPath::existing(binary)
        .map(studio_runtime_store::path_identity::CanonicalPath::into_path_buf)
        .map_err(|error| NativeLayrCapability::NotReady {
            reason: format!("Cannot resolve layr: {error}"),
        })
}

#[cfg(target_os = "linux")]
fn linux_readiness(state_dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let store = super::settings::selected_store(state_dir).map_err(|e| e.to_string())?;
    if !store.is_dir()
        || rustix::fs::statfs(&store)
            .map(|info| info.f_type as u64)
            .ok()
            != Some(linux_raw_sys::general::BTRFS_SUPER_MAGIC as u64)
    {
        return Err(format!(
            "Native Layr requires a btrfs store at {}.",
            store.display()
        ));
    }
    let socket = super::selected_socket_path(state_dir).map_err(|e| e.to_string())?;
    let refusal = || format!("Start the native Layr root daemon at {}.", socket.display());
    let metadata = std::fs::symlink_metadata(&socket).map_err(|_| refusal())?;
    if !metadata.file_type().is_socket() || metadata.uid() != 0 {
        return Err(refusal());
    }
    let table = std::fs::read_to_string("/proc/net/unix").map_err(|_| refusal())?;
    if !has_listener(&table, &socket) {
        return Err(refusal());
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn has_listener(table: &str, socket: &Path) -> bool {
    let Some(expected) = socket.to_str() else {
        return false;
    };
    table.lines().any(|line| {
        // Seven whitespace-separated columns precede the path. Preserve spaces in the path.
        let mut remainder = line.trim_start();
        let mut columns = Vec::with_capacity(7);
        for _ in 0..7 {
            let Some((column, tail)) = remainder.split_once(char::is_whitespace) else {
                return false;
            };
            columns.push(column);
            remainder = tail.trim_start();
        }
        columns[3] == "00010000" // SO_ACCEPTCON: this socket is listening.
            && columns[4] == "0001" // SOCK_STREAM.
            && columns[5] == "01" // Unconnected listener, not an accepted stream.
            && columns[6].parse::<u64>().is_ok_and(|inode| inode != 0)
            && remainder == expected
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn native_capability_skips_studio_launcher() {
        use std::os::unix::fs::PermissionsExt;
        let first = tempfile::tempdir().expect("launcher directory");
        let second = tempfile::tempdir().expect("binary directory");
        let launcher = first.path().join("layr");
        let binary = second.path().join("layr");
        std::fs::write(&launcher, "#!/bin/sh\n# Studio native Layr launcher.\n").expect("launcher");
        std::fs::write(&binary, "#!/bin/sh\nexit 0\n").expect("binary");
        for path in [&launcher, &binary] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
                .expect("executable");
        }
        let path = std::env::join_paths([first.path(), second.path()]).expect("path");
        assert_eq!(
            resolve_binary_in_path(&path),
            Ok(
                studio_runtime_store::path_identity::CanonicalPath::existing(binary)
                    .expect("canonical binary")
                    .into_path_buf()
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn native_capability_resolves_only_an_executable_on_the_server_path() {
        use std::os::unix::fs::PermissionsExt;
        let first = tempfile::tempdir().expect("first path");
        let second = tempfile::tempdir().expect("second path");
        let binary = second.path().join("layr");
        std::fs::write(&binary, "#!/bin/sh\nexit 99\n").expect("fixture");
        let path = std::env::join_paths([first.path(), second.path()]).expect("path");
        assert_eq!(
            resolve_binary_in_path(&path),
            Err(NativeLayrCapability::NotInstalled)
        );
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
            .expect("executable");
        // The resolver returns the canonical path (/private/var on macOS, not the /var alias).
        assert_eq!(
            resolve_binary_in_path(&path),
            Ok(
                studio_runtime_store::path_identity::CanonicalPath::existing(&binary)
                    .expect("canonical fixture")
                    .into_path_buf()
            )
        );
    }

    #[test]
    fn native_capability_listener_matches_the_exact_owned_socket_path() {
        let socket = Path::new("/studio state/layr-native/layr.sock");
        let row =
            "00000000: 00000002 00000000 00010000 0001 01 123 /studio state/layr-native/layr.sock";
        assert!(has_listener(row, socket));
        assert!(!has_listener(
            row,
            Path::new("/other/layr-native/layr.sock")
        ));
        assert!(!has_listener(&format!("{row}.other"), socket));
        assert!(!has_listener("", socket));
    }

    #[test]
    fn native_capability_refuses_bound_stale_or_non_stream_sockets() {
        let socket = Path::new("/studio/layr-native/layr.sock");
        let row = "00000000: 00000002 00000000 00010000 0001 01 123 /studio/layr-native/layr.sock";
        assert!(!has_listener(&row.replace("00010000", "00000000"), socket));
        assert!(!has_listener(&row.replace("0001 01", "0002 01"), socket));
        assert!(!has_listener(&row.replace("0001 01", "0001 03"), socket));
        assert!(!has_listener(&row.replace("01 123", "01 0"), socket));
    }

    #[test]
    fn native_capability_keeps_the_reason_only_for_unavailable_hosts() {
        assert_eq!(from_result(Ok(())), NativeLayrCapability::Ready);
        assert_eq!(
            from_result(Err("btrfs is required".into())),
            NativeLayrCapability::NotReady {
                reason: "btrfs is required".into()
            }
        );
    }
}
