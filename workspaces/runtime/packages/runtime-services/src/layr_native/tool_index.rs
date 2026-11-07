//! LayrFS tool links. The managed bin folder is first on the PATH of native-line agents; links
//! named `rg` and `find` there point at the Layr binary, which answers these tree tools from its
//! in-memory index of the line and runs the real tool for anything it cannot answer identically
//! (Layr `docs/layrfs.md`). A Layr build without `layr tool` would read a link named `rg` as a
//! Layr command, so the links exist only while the binary serves the tools.
use super::NativeLayrError;
use crate::bounded_child::{self, Request};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) const TOOLS: [&str; 2] = ["rg", "find"];

/// `layr tool --help` succeeds only on a Layr build that serves the tools.
fn serves_tools(executable: &Path) -> bool {
    bounded_child::run_without_git_environment(Request {
        program: executable.as_os_str(),
        git_directory: None,
        args: &["tool", "--help"],
        environment: &[],
        input: None,
        stdout_limit: 64 * 1024,
        stderr_limit: 4096,
        timeout: Duration::from_secs(10),
    })
    .is_ok_and(|output| String::from_utf8_lossy(&output.stdout.bytes).contains("usage: layr"))
}

/// Point the tool links in `bin` at `executable`, or remove them when it does not serve the
/// tools or `LAYR_FS_OFF` is set for the server.
pub(super) fn ensure_links(bin: &Path, executable: &Path) -> Result<(), NativeLayrError> {
    let wanted = std::env::var_os("LAYR_FS_OFF").is_none_or(|value| value.is_empty())
        && serves_tools(executable);
    for tool in TOOLS {
        let link = bin.join(tool);
        let current: Option<PathBuf> = fs::read_link(&link).ok();
        if wanted {
            if current.as_deref() == Some(executable) {
                continue;
            }
            // A fresh link replaces the old entry in one rename.
            let staged = bin.join(format!(".{tool}.link-{}", std::process::id()));
            let _ = fs::remove_file(&staged);
            std::os::unix::fs::symlink(executable, &staged)?;
            fs::rename(&staged, &link)?;
        } else if current.is_some() {
            fs::remove_file(&link)?;
        }
    }
    Ok(())
}

/// Start the index build of a line in the background. Until it is ready, the tools run the real
/// programs, so a failure here costs speed only.
pub(super) fn warm(state_dir: &Path, line: &Path) {
    if let Err(error) = super::runtime::run(state_dir, line, &["fs", "warm", path_text(line)]) {
        eprintln!(
            "Native Layr warning: the tool index of {} is not warm: {error}",
            line.display()
        );
    }
}

fn path_text(path: &Path) -> &str {
    path.to_str().unwrap_or(".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("mode");
        path
    }

    #[test]
    fn links_follow_whether_layr_serves_the_tools() {
        let dir = tempfile::tempdir().expect("fixture");
        let bin = dir.path().join("bin");
        fs::create_dir(&bin).expect("bin");
        let serving = script(dir.path(), "layr-new", "echo 'usage: layr <command>'");
        let old = script(
            dir.path(),
            "layr-old",
            "echo \"layr: 'tool' is not a layr command\" >&2; exit 1",
        );

        ensure_links(&bin, &serving).expect("links");
        for tool in TOOLS {
            assert_eq!(fs::read_link(bin.join(tool)).expect("link"), serving);
        }
        // Repeating changes nothing; an older Layr build removes the links.
        ensure_links(&bin, &serving).expect("again");
        ensure_links(&bin, &old).expect("old build");
        for tool in TOOLS {
            assert!(
                fs::symlink_metadata(bin.join(tool)).is_err(),
                "{tool} link stays"
            );
        }
        assert_eq!(
            fs::read_dir(&bin).expect("bin").count(),
            0,
            "no staged link is left"
        );
    }
}
