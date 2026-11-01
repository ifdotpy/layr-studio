//! Preserve attributes through descriptors, without following copied links.
use super::*;
use rustix::fs::{Mode, OFlags, XattrFlags, fchmod, fgetxattr, flistxattr, fsetxattr, openat};

/// Write one attribute. A read-only entry gets owner write access only during this write.
fn set_attribute(file: &File, name: &std::ffi::OsStr, value: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    match fsetxattr(file, name, value, XattrFlags::empty()) {
        Err(error)
            if std::io::Error::from(error).kind() == std::io::ErrorKind::PermissionDenied =>
        {
            let mode = (file.metadata()?.mode() & 0o7777) as u16;
            fchmod(file, Mode::from_raw_mode(mode | 0o200))?;
            let written = fsetxattr(file, name, value, XattrFlags::empty());
            fchmod(file, Mode::from_raw_mode(mode))?;
            written.map_err(std::io::Error::from)
        }
        result => result.map_err(std::io::Error::from),
    }
}

fn copy_descriptors(source: &File, destination: &File) -> std::io::Result<bool> {
    let count = match flistxattr(source, &mut [] as &mut [u8]) {
        Ok(count) => count,
        Err(error) if std::io::Error::from(error).kind() == std::io::ErrorKind::Unsupported => {
            return Ok(false);
        }
        Err(error) => return Err(error.into()),
    };
    let mut names = vec![0; count];
    let count = flistxattr(source, names.as_mut_slice())?;
    for name in names[..count]
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::OsStr::from_bytes(name);
        let count = fgetxattr(source, name, &mut [] as &mut [u8])?;
        let mut value = vec![0; count];
        let count = fgetxattr(source, name, value.as_mut_slice())?;
        let mut existing = vec![0; count];
        if fgetxattr(destination, name, existing.as_mut_slice())
            .is_ok_and(|n| n == count && existing == value)
        {
            continue;
        }
        set_attribute(destination, name, &value[..count])?;
    }
    Ok(count != 0)
}

/// Copy the attributes of one entry, naming its path in any error.
pub(super) fn copy(
    source: &Path,
    relative: &Path,
    destination: &Directory,
    target: &Path,
    symlink: bool,
) -> std::io::Result<bool> {
    copy_attributes(source, relative, destination, target, symlink).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!(
                "Cannot copy attributes of {}: {error}",
                source.join(relative).display()
            ),
        )
    })
}

fn copy_attributes(
    source: &Path,
    relative: &Path,
    destination: &Directory,
    target: &Path,
    symlink: bool,
) -> std::io::Result<bool> {
    let (source_parent, source_name) = open_parent(source, relative)?;
    let (destination_parent, destination_name) = destination.parent(target, false)?;
    if !symlink {
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let source = File::from(openat(&source_parent, &source_name, flags, Mode::empty())?);
        let destination = File::from(openat(
            &destination_parent.0,
            &destination_name,
            flags,
            Mode::empty(),
        )?);
        return copy_descriptors(&source, &destination);
    }
    #[cfg(target_os = "macos")]
    {
        let flags = OFlags::RDONLY | OFlags::SYMLINK | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let source = File::from(openat(&source_parent, &source_name, flags, Mode::empty())?);
        let destination = File::from(openat(
            &destination_parent.0,
            &destination_name,
            flags,
            Mode::empty(),
        )?);
        if !source.metadata()?.file_type().is_symlink()
            || !destination.metadata()?.file_type().is_symlink()
        {
            return Err(std::io::Error::other("An attribute link changed"));
        }
        copy_descriptors(&source, &destination)
    }
    #[cfg(target_os = "linux")]
    {
        use rustix::fs::{lgetxattr, llistxattr, lsetxattr};
        use std::os::fd::AsRawFd;
        let source =
            PathBuf::from(format!("/proc/self/fd/{}", source_parent.as_raw_fd())).join(source_name);
        let destination = PathBuf::from(format!(
            "/proc/self/fd/{}",
            destination_parent.0.as_raw_fd()
        ))
        .join(destination_name);
        let count = llistxattr(&source, &mut [] as &mut [u8])?;
        let mut names = vec![0; count];
        let count = llistxattr(&source, names.as_mut_slice())?;
        for name in names[..count]
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            use std::os::unix::ffi::OsStrExt;
            let name = std::ffi::OsStr::from_bytes(name);
            let count = lgetxattr(&source, name, &mut [] as &mut [u8])?;
            let mut value = vec![0; count];
            let count = lgetxattr(&source, name, value.as_mut_slice())?;
            lsetxattr(&destination, name, &value[..count], XattrFlags::empty())?;
        }
        Ok(count != 0)
    }
}
