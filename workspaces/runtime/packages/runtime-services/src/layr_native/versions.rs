//! Inspect the Layr-owned store format before Studio prepares a native line.
use super::*;
use std::time::Duration;

type Result<T> = std::result::Result<T, NativeLayrError>;

#[derive(serde::Deserialize)]
pub struct Identity {
    format: u64,
    pub version: String,
    pub build: String,
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

/// Read the identity reported by the same bounded store inspection used for verification.
pub fn binary_version(binary: &Path, root: &Path) -> Result<Identity> {
    Ok(inspect(binary, root)?.binary)
}

pub(super) fn verify(state: &Path, root: &Path) -> Result<Option<String>> {
    let executable = super::capability::resolve_binary().map_err(|capability| {
        NativeLayrError::Refused(capability.message().unwrap_or("Layr is unavailable").into())
    })?;
    #[cfg(target_os = "linux")]
    let socket = Some(super::selected_socket_path(state)?);
    #[cfg(not(target_os = "linux"))]
    let socket: Option<std::path::PathBuf> = None;
    verify_with_binary(state, root, &executable, socket.as_deref())
}

fn verify_with_binary(
    state: &Path,
    root: &Path,
    executable: &Path,
    socket: Option<&Path>,
) -> Result<Option<String>> {
    let mut installed = inspect(executable, root)?;
    #[cfg(target_os = "linux")]
    if let Some(socket) = socket {
        let daemon = daemon_identity(socket).map_err(NativeLayrError::Refused)?;
        check_daemon_identity(&installed.binary, &daemon).map_err(NativeLayrError::Refused)?;
    }
    #[cfg(not(target_os = "linux"))]
    let _ = socket;
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

#[cfg(target_os = "linux")]
// Layr src/service.rs negotiate. Remove when Layr exposes the daemon identity (ifdotpy/layr#137).
fn daemon_identity(socket: &Path) -> std::result::Result<Identity, String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    let error = |e: &dyn std::fmt::Display| {
        format!(
            "Cannot inspect the Layr daemon at {}: {e}. Restart the daemon after upgrading Layr.",
            socket.display()
        )
    };
    let address = socket2::SockAddr::unix(socket).map_err(|e| error(&e))?;
    let connection = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)
        .map_err(|e| error(&e))?;
    connection
        .connect_timeout(&address, Duration::from_secs(10))
        .map_err(|e| error(&e))?;
    let mut stream: UnixStream = connection.into();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| error(&e))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| error(&e))?;
    let request = br#"{"hello":true}"#;
    stream
        .write_all(&(request.len() as u32).to_be_bytes())
        .and_then(|()| stream.write_all(request))
        .map_err(|e| error(&e))?;
    let mut kind = [0; 1];
    stream.read_exact(&mut kind).map_err(|e| error(&e))?;
    if kind[0] == b'B' {
        return Err("The Layr daemon is busy; try again.".into());
    }
    let mut header = [0; 4];
    stream.read_exact(&mut header).map_err(|e| error(&e))?;
    let size = u32::from_be_bytes(header) as usize;
    if kind[0] != b'V' || size > 4096 {
        return Err(error(&"invalid daemon version response"));
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes).map_err(|e| error(&e))?;
    serde_json::from_slice(&bytes).map_err(|e| error(&e))
}

#[cfg(target_os = "linux")]
fn check_daemon_identity(client: &Identity, daemon: &Identity) -> std::result::Result<(), String> {
    if client.format != daemon.format
        || client.version != daemon.version
        || client.build != daemon.build
    {
        return Err(format!(
            "Layr daemon {} (format {}, build {}) differs from the installed Layr {} (format {}, build {}). Restart the daemon after upgrading Layr.",
            daemon.version,
            daemon.format,
            daemon.build,
            client.version,
            client.format,
            client.build
        ));
    }
    Ok(())
}

/// Test daemon: answers one version handshake (Layr `src/service.rs` `negotiate`) with `identity`.
#[cfg(all(test, target_os = "linux"))]
pub(super) fn serve_daemon_identity(
    socket: &Path,
    identity: &'static [u8],
) -> std::thread::JoinHandle<()> {
    use std::io::{Read, Write};
    let listener = std::os::unix::net::UnixListener::bind(socket).expect("listener");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut length = [0; 4];
        stream.read_exact(&mut length).expect("length");
        let mut request = vec![0; u32::from_be_bytes(length) as usize];
        stream.read_exact(&mut request).expect("request");
        assert_eq!(request, br#"{"hello":true}"#);
        stream.write_all(b"V").expect("kind");
        stream
            .write_all(&(identity.len() as u32).to_be_bytes())
            .expect("length");
        stream.write_all(identity).expect("response");
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn daemon_handshake_reads_identity_without_a_command() {
        let dir = tempfile::tempdir().expect("socket directory");
        let path = dir.path().join("socket");
        let server =
            serve_daemon_identity(&path, br#"{"format":1,"version":"0.11.0","build":"old"}"#);
        let daemon = daemon_identity(&path).expect("identity");
        assert_eq!(daemon.version, "0.11.0");
        server.join().expect("server");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn busy_daemon_gives_retryable_message() {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixListener;
        let dir = tempfile::tempdir().expect("socket directory");
        let path = dir.path().join("socket");
        let listener = UnixListener::bind(&path).expect("listener");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0; 18];
            stream.read_exact(&mut request).expect("request");
            stream.write_all(b"B").expect("busy response");
        });
        assert!(
            daemon_identity(&path)
                .err()
                .expect("busy")
                .contains("busy; try again")
        );
        server.join().expect("server");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn daemon_identity_must_match_client() {
        let client = Identity {
            format: 1,
            version: "0.14.2".into(),
            build: "build".into(),
        };
        let matching = Identity {
            format: 1,
            version: "0.14.2".into(),
            build: "build".into(),
        };
        let different = Identity {
            format: 1,
            version: "0.11.0".into(),
            build: "old".into(),
        };
        assert_eq!(check_daemon_identity(&client, &matching), Ok(()));
        assert!(
            matches!(check_daemon_identity(&client, &different), Err(reason) if reason.contains("0.11.0") && reason.contains("0.14.2"))
        );
    }

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
        verify_with_binary(&state, &root, &binary_path(&state), None).expect("metadata adoption");
        verify_with_binary(&state, &root, &binary_path(&state), None).expect("repeat is a no-op");
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
        let error = verify_with_binary(&state, &developer, &binary_path(&state), None)
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
