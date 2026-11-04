//! Descriptor-based file clones and read-only volume case policy for native import.
use super::Result;
#[cfg(target_os = "macos")]
use super::existing_ancestor;
use std::{fs::File, path::Path};

#[cfg(target_os = "linux")]
#[allow(
    unsafe_code,
    reason = "read the btrfs filesystem UUID through its kernel ioctl"
)]
pub(super) fn volume_id(fd: &impl std::os::fd::AsFd) -> std::io::Result<super::VolumeId> {
    use std::os::fd::AsRawFd;
    if rustix::fs::fstatfs(fd)?.f_type as u64 == linux_raw_sys::general::BTRFS_SUPER_MAGIC as u64 {
        // SAFETY: the kernel fills this correctly sized, zero-initialized ioctl buffer.
        let mut info: linux_raw_sys::btrfs::btrfs_ioctl_fs_info_args =
            unsafe { std::mem::zeroed() };
        // SAFETY: the descriptor and writable buffer remain valid for the ioctl.
        if unsafe {
            nix::libc::ioctl(
                fd.as_fd().as_raw_fd(),
                linux_raw_sys::ioctl::BTRFS_IOC_FS_INFO as _,
                &mut info,
            )
        } < 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(super::VolumeId::Btrfs(info.fsid))
    } else {
        Ok(super::VolumeId::Other(
            rustix::fs::fstatvfs(fd)?.f_fsid as u64,
        ))
    }
}

/// CLONE_NOOWNERCOPY from sys/clonefile.h, a clone flag that keeps destination ownership.
#[cfg(target_os = "macos")]
const CLONE_NOOWNERCOPY: u32 = 0x0002;

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "APFS clone between pinned file and parent directory descriptors"
)]
pub(super) fn clone_file(
    source: &File,
    parent: &std::os::fd::OwnedFd,
    name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
    let name = std::ffi::CString::new(name.as_bytes())?;
    // SAFETY: both descriptors and the NUL-terminated leaf remain live during this call.
    if unsafe {
        libc::fclonefileat(
            source.as_raw_fd(),
            parent.as_raw_fd(),
            name.as_ptr(),
            CLONE_NOOWNERCOPY,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(target_os = "linux")]
pub(super) fn clone_file(
    source: &File,
    parent: &std::os::fd::OwnedFd,
    name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    use rustix::fs::{Mode, OFlags, openat};
    let copy = File::from(openat(
        parent,
        name,
        OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )?);
    rustix::fs::ioctl_ficlone(&copy, source)?;
    Ok(())
}

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "read-only filesystem case policy without a probe file"
)]
pub(super) fn case_sensitive(destination: &Path) -> Result<bool> {
    use std::os::unix::ffi::OsStrExt;
    let at = existing_ancestor(destination)?;
    let path = std::ffi::CString::new(at.as_os_str().as_bytes()).map_err(std::io::Error::other)?;
    // SAFETY: the path is NUL-terminated and lives until pathconf returns.
    match unsafe { libc::pathconf(path.as_ptr(), libc::_PC_CASE_SENSITIVE) } {
        -1 => Err(std::io::Error::last_os_error().into()),
        0 => Ok(false),
        _ => Ok(true),
    }
}
#[cfg(not(target_os = "macos"))]
pub(super) fn case_sensitive(_destination: &Path) -> Result<bool> {
    Ok(true)
}

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "read the volume type without creating a probe file"
)]
pub(in crate::layr_native) fn is_copy_on_write(root: &Path) -> bool {
    use std::os::fd::AsRawFd;
    let Ok(file) = std::fs::File::open(root) else {
        return false;
    };
    // SAFETY: fstatfs fills a valid structure and the descriptor remains open.
    let mut stats: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatfs(file.as_raw_fd(), &mut stats) } != 0 {
        return false;
    }
    // SAFETY: f_fstypename is a NUL-terminated kernel-supplied name.
    unsafe { std::ffi::CStr::from_ptr(stats.f_fstypename.as_ptr()) }.to_bytes() == b"apfs"
}
