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
    state: StoreState,
}

// Temporary local contract until Studio consumes layr-api's StoreState (ifdotpy/layr#364).
#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum StoreState {
    Absent,
    Empty,
    Unmarked,
    Compatible,
    Incompatible,
}

impl std::fmt::Display for StoreState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Absent => "absent",
            Self::Empty => "empty",
            Self::Unmarked => "unmarked",
            Self::Compatible => "compatible",
            Self::Incompatible => "incompatible",
        })
    }
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
    studio_json::from_slice(&result.stdout.bytes).map_err(|e| NativeLayrError::Refused(format!("Layr at {} has no valid store-version response for {root}: {e}. Upgrade this binary; see docs/native-layr.md.", binary.display())))
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
    let socket = Some(super::socket_path_in_store(state, root));
    #[cfg(not(target_os = "linux"))]
    let socket: Option<PathBuf> = {
        let _ = state;
        None
    };
    verify_with_binary(root, &executable, socket.as_deref())
}

pub(super) fn readiness(state: &Path, root: &Path) -> Result<()> {
    verify(state, root).map(|_| ())
}

fn verify_with_binary(
    root: &Path,
    executable: &Path,
    socket: Option<&Path>,
) -> Result<Option<String>> {
    let installed = inspect(executable, root)?;
    #[cfg(target_os = "linux")]
    if let Some(socket) = socket {
        let daemon = daemon_identity(socket).map_err(NativeLayrError::Refused)?;
        check_daemon_identity(&installed.binary, &daemon).map_err(NativeLayrError::Refused)?;
    }
    #[cfg(not(target_os = "linux"))]
    let _ = socket;
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
            .unwrap_or_else(|| installed.state.to_string());
        return Err(NativeLayrError::Refused(format!(
            "Store {} was written by {written}. Studio uses {} (format {}, build {}). See docs/native-layr.md for the switch procedure.",
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
    studio_json::from_slice(&bytes).map_err(|e| error(&e))
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

    #[test]
    fn native_layr_store_version_rejects_unknown_states() {
        for state in ["legacy", "current", "future", "Compatible"] {
            let response = json!({"binary":{"format":1,"version":"0.36.0","build":"fixture"},"store":null,"compatible":true,"state":state});
            let error = studio_json::from_value::<Inspection>(response)
                .err()
                .expect("unknown state must fail decoding");
            assert!(error.to_string().contains("unknown variant"), "{error}");
        }
    }

    #[test]
    fn native_layr_store_version_accepts_the_five_supported_states() {
        for state in ["absent", "empty", "unmarked", "compatible", "incompatible"] {
            let response = json!({"binary":{"format":1,"version":"0.36.0","build":"fixture"},"store":null,"compatible":true,"state":state});
            assert!(
                studio_json::from_value::<Inspection>(response).is_ok(),
                "{state}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn native_layr_store_inspection_never_invokes_upgrade() {
        use std::{fs, os::unix::fs::PermissionsExt};
        let fixture = tempfile::tempdir().expect("fixture");
        let state = fixture.path().join("state");
        let owned = store_path(&state);
        let selected = fixture.path().join("selected-store");
        let binary = fixture.path().join("layr");
        fs::write(
            &binary,
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$2/calls"
if [ "$#" != 4 ] || [ "$1" != --root ] || [ "$3" != store-version ] || [ "$4" != --json ]; then
  printf '%s\n' 'layr store-version: unknown option' >&2
  exit 1
fi
cat "$2/response"
"#,
        )
        .expect("binary fixture");
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).expect("executable");
        for root in [owned, selected] {
            fs::create_dir_all(&root).expect("store");
            fs::write(root.join("response"), r#"{"binary":{"format":1,"version":"0.36.0","build":"fixture"},"store":null,"compatible":true,"state":"unmarked"}"#).expect("unmarked response");
            assert_eq!(
                verify_with_binary(&root, &binary, None).expect("compatible unmarked store"),
                None
            );
            fs::write(root.join("response"), r#"{"binary":{"format":1,"version":"0.36.0","build":"fixture"},"store":{"format":0,"version":"old","build":"old"},"compatible":false,"state":"incompatible","upgrade":{"fromFormat":0,"toFormat":1,"metadataOnly":true}}"#).expect("incompatible response");
            let error = verify_with_binary(&root, &binary, None)
                .expect_err("incompatible store")
                .to_string();
            assert!(error.contains("was written by old"), "{error}");
            let expected = format!("--root {} store-version --json\n", root.display());
            assert_eq!(
                fs::read_to_string(root.join("calls")).expect("calls"),
                expected.repeat(2)
            );
        }
    }

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
}
