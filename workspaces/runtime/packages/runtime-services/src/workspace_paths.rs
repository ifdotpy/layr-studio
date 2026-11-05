use std::{fs, path::Path};
use studio_runtime_store::path_identity::CanonicalPath;

type VmArchiveIdentity = (u64, u64, u64, i128, i128);

pub(crate) fn vm_archive_identity(metadata: &fs::Metadata) -> VmArchiveIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (
            metadata.dev(),
            metadata.ino(),
            metadata.size(),
            i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec()),
            i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec()),
        )
    }
    #[cfg(not(unix))]
    {
        (0, 0, metadata.len(), 0, 0)
    }
}

pub(crate) fn read_gitdir_pointer(
    directory: &Path,
    marker: &Path,
) -> std::io::Result<CanonicalPath> {
    let contents = fs::read_to_string(marker)?;
    let line = contents
        .strip_suffix("\r\n")
        .or_else(|| contents.strip_suffix('\n'))
        .or_else(|| contents.strip_suffix('\r'))
        .unwrap_or(&contents);
    if line.contains(['\n', '\r']) || line.trim() != line {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Workspace source has malformed Git pointer metadata",
        ));
    }
    let target = line
        .strip_prefix("gitdir: ")
        .filter(|target| !target.is_empty() && target.trim() == *target)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Workspace source has malformed Git pointer metadata",
            )
        })?;
    CanonicalPath::existing(directory.join(target))
}

pub(crate) fn regenerable_path(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if matches!(
        name.as_ref(),
        "node_modules"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".pytest_cache"
            | ".mypy_cache"
            | ".ruff_cache"
            | ".turbo"
            | ".next"
    ) {
        return true;
    }
    name == "target"
        && path
            .parent()
            .is_some_and(|parent| parent.join("Cargo.toml").is_file())
}
