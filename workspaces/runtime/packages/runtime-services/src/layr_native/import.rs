//! Select and admit the complete tree before writing imported entries.
use super::{NativeLayrError, private_tree::Directory};
mod filesystem_ffi;
use crate::workspace_paths::vm_archive_identity;
#[cfg(target_os = "macos")]
pub(super) use filesystem_ffi::is_copy_on_write;
use filesystem_ffi::{case_sensitive, clone_file};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    path::{Path, PathBuf},
};
use studio_runtime_store::path_identity::CanonicalPath;

type Result<T> = std::result::Result<T, NativeLayrError>;
/// Safety factor for measured metadata growth, dimensionless.
const METADATA_SAFETY_FACTOR: u64 = 2;
/// Peak metadata growth and entry count from the APFS cache fixture, in bytes and entries.
const CACHE_MEASURED_PEAK_BYTES: u64 = 4_079_616;
const CACHE_MEASURED_ENTRIES: u64 = 4_161;
/// Peak growth and source entries from the APFS line-new fixture, in bytes and entries.
const LINE_NEW_MEASURED_PEAK_BYTES: u64 = 3_960_832;
const LINE_NEW_MEASURED_ENTRIES: u64 = 8_517;
/// Peak growth and source entries from the APFS migrated-line fixture, in bytes and entries.
const MIGRATED_MEASURED_PEAK_BYTES: u64 = 12_185_600;
const MIGRATED_MEASURED_ENTRIES: u64 = 8_517;
/// Final import metadata allowance per entry, in bytes; twice the largest measured peak.
const ENTRY_METADATA_BYTES: u64 = {
    let cache =
        (METADATA_SAFETY_FACTOR * CACHE_MEASURED_PEAK_BYTES).div_ceil(CACHE_MEASURED_ENTRIES);
    let migrated =
        (METADATA_SAFETY_FACTOR * MIGRATED_MEASURED_PEAK_BYTES).div_ceil(MIGRATED_MEASURED_ENTRIES);
    if cache > migrated { cache } else { migrated }
};
/// Temporary main-tree clone allowance per entry, in bytes.
const LINE_NEW_ENTRY_METADATA_BYTES: u64 =
    (METADATA_SAFETY_FACTOR * LINE_NEW_MEASURED_PEAK_BYTES).div_ceil(LINE_NEW_MEASURED_ENTRIES);
/// Minimum unused capacity retained after estimated allocations, in bytes.
const SPACE_MARGIN_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Size {
    pub files: u64,
    pub entries: u64,
    pub cache_entries: u64,
    pub metadata_bytes: u64,
    pub inherited_entries: u64,
    pub inherited_metadata_bytes: u64,
    pub transient_metadata_bytes: u64,
    pub cross_volume_bytes: u64,
    pub git_head_bytes: u64,
    pub git_transfer_bytes: u64,
    pub logical_bytes: u64,
    pub tracked_bytes: u64,
    pub cache_bytes: u64,
    pub estimated_new_bytes: u64,
}

impl Size {
    pub fn include(&mut self, other: Self) {
        self.files = self.files.saturating_add(other.files);
        self.entries = self.entries.saturating_add(other.entries);
        self.cache_entries = self.cache_entries.saturating_add(other.cache_entries);
        self.metadata_bytes = self.metadata_bytes.saturating_add(other.metadata_bytes);
        self.inherited_entries = self
            .inherited_entries
            .saturating_add(other.inherited_entries);
        self.inherited_metadata_bytes = self
            .inherited_metadata_bytes
            .saturating_add(other.inherited_metadata_bytes);
        // Each migration runs sequentially, so temporary line clones need only the largest allowance.
        self.transient_metadata_bytes = self
            .transient_metadata_bytes
            .max(other.transient_metadata_bytes);
        self.cross_volume_bytes = self
            .cross_volume_bytes
            .saturating_add(other.cross_volume_bytes);
        self.git_head_bytes = self.git_head_bytes.saturating_add(other.git_head_bytes);
        self.git_transfer_bytes = self
            .git_transfer_bytes
            .saturating_add(other.git_transfer_bytes);
        self.logical_bytes = self.logical_bytes.saturating_add(other.logical_bytes);
        self.tracked_bytes = self.tracked_bytes.saturating_add(other.tracked_bytes);
        self.cache_bytes = self.cache_bytes.saturating_add(other.cache_bytes);
        self.refresh_estimate();
    }
    fn refresh_estimate(&mut self) {
        self.estimated_new_bytes = self
            .metadata_bytes
            .saturating_add(self.transient_metadata_bytes)
            .saturating_add(self.cross_volume_bytes)
            .saturating_add(self.git_head_bytes)
            .saturating_add(self.git_transfer_bytes);
    }
    pub fn preflight(&self, destination: &Path) -> Result<()> {
        let at = existing_ancestor(destination)?;
        let free = crate::disk_space::free_bytes(at).ok_or_else(|| {
            NativeLayrError::Refused(format!("Cannot read free space at {}", at.display()))
        })?;
        self.require_free(free)
    }
    fn require_free(&self, free: u64) -> Result<()> {
        let margin = SPACE_MARGIN_BYTES.max(self.estimated_new_bytes / 10);
        let required = self.estimated_new_bytes.saturating_add(margin);
        if free < required {
            return Err(NativeLayrError::Refused(format!(
                "Insufficient free space for native Layr import: {free} bytes available; {required} bytes required ({estimate} estimated new bytes plus {margin} bytes margin). Git HEAD materialization and Git object transfer require new storage.",
                estimate = self.estimated_new_bytes
            )));
        }
        Ok(())
    }
}

pub(super) struct Plan {
    source: PathBuf,
    entries: Vec<Entry>,
    pub caches: Vec<PathBuf>,
    pub repositories: Vec<Repository>,
    pub refusals: Vec<String>,
    pub excluded_worktree_folders: Vec<PathBuf>,
    pub plain_git_metadata: Vec<metadata::PlainMetadata>,
    pub worktree_data: Vec<PathBuf>,
    pub size: Size,
    destination_volume: VolumeId,
}

impl Plan {
    #[cfg(test)]
    pub fn new(source: &Path, worktree: bool, destination: &Path) -> Result<Self> {
        Self::new_with_git_policy(source, worktree, destination, false)
    }

    pub fn new_with_git_policy(
        source: &Path,
        worktree: bool,
        destination: &Path,
        without_git: bool,
    ) -> Result<Self> {
        Self::select(source, worktree, destination, without_git, None)
    }

    pub fn worktree(
        source: &Path,
        project: &Path,
        repository: &Path,
        destination: &Path,
    ) -> Result<Self> {
        Self::select(
            source,
            true,
            destination,
            false,
            Some((project, repository)),
        )
    }

    fn select(
        source: &Path,
        worktree: bool,
        destination: &Path,
        without_git: bool,
        project: Option<(&Path, &Path)>,
    ) -> Result<Self> {
        let source = CanonicalPath::existing(source).map(CanonicalPath::into_path_buf)?;
        if !source.is_dir() {
            return Err(NativeLayrError::Refused(
                "Project source is not a directory".into(),
            ));
        }
        let project_root =
            CanonicalPath::existing(project.map_or(source.as_path(), |(root, _)| root))
                .map(CanonicalPath::into_path_buf)?;
        let marker = source.join(".git");
        let ignored = if marker.exists()
            && (!without_git
                || metadata::plain_reason(&source, &marker).is_ok_and(|reason| reason.is_none()))
            && metadata::validate_repository(&project_root, &source).is_ok()
        {
            ignored_paths(&source)?
        } else {
            BTreeSet::new()
        };
        let mut tree = Selection {
            project_root: project_root.clone(),
            without_git,
            ignored,
            ..Selection::default()
        };
        collect_tree(&source, Path::new(""), false, false, worktree, &mut tree)?;
        let git_backed = !worktree
            && tree
                .repositories
                .iter()
                .any(|repo| repo.relative.as_os_str().is_empty() && repo.included);
        for repo in tree
            .repositories
            .iter()
            .filter(|repo| repo.included && !repo.cache)
        {
            if let Ok(objects) = metadata::object_paths(&project_root, &source.join(&repo.relative))
            {
                for target in objects {
                    if target.starts_with(&source)
                        && !tree
                            .entries
                            .iter()
                            .any(|entry| !entry.cache && source.join(&entry.relative) == target)
                    {
                        tree.refusals.push(format!("Internal Git object store is outside the versioned selection: {}. Retain its resolved target in main before import.", target.display()));
                    }
                }
            }
        }
        let mut entries = tree.entries;
        if tree
            .repositories
            .iter()
            .any(|repo| repo.kind == "submodule")
        {
            tree.refusals
                .push("Submodules are not supported in native Layr import yet".into());
        }
        // Layr creates this bridge even when the source is a plain folder.
        let generated_names = [PathBuf::from(".git")];
        let destination_case_sensitive = case_sensitive(destination)?;
        validate_names(
            entries
                .iter()
                .map(|entry| entry.relative.as_path())
                .chain(generated_names.iter().map(PathBuf::as_path)),
            destination_case_sensitive,
            cfg!(target_os = "macos"),
        )?;
        let destination_volume = volume_id(existing_ancestor(destination)?)?;
        let mut volumes = BTreeMap::new();
        let mut size = Size::default();
        let git_objects = if git_backed {
            metadata::object_paths(&project_root, &source).unwrap_or_else(|error| {
                tree.refusals.push(error.to_string());
                BTreeSet::new()
            })
        } else {
            BTreeSet::new()
        };
        let mut git_bytes = 0u64;
        for entry in &mut entries {
            let meta = &entry.metadata;
            size.entries = size.entries.saturating_add(1);
            size.cache_entries = size.cache_entries.saturating_add(u64::from(entry.cache));
            size.metadata_bytes = size.metadata_bytes.saturating_add(ENTRY_METADATA_BYTES);
            if !meta.is_dir() {
                if entry.cache {
                    size.cache_bytes = size.cache_bytes.saturating_add(meta.len());
                } else {
                    size.tracked_bytes = size.tracked_bytes.saturating_add(meta.len());
                }
                size.files += 1;
                size.logical_bytes = size.logical_bytes.saturating_add(meta.len());
            }
            if meta.is_file() {
                let volume = match volumes.get(&meta.dev()) {
                    Some(value) => *value,
                    None => {
                        let value = volume_id(&source.join(&entry.relative))?;
                        volumes.insert(meta.dev(), value);
                        value
                    }
                };
                entry.cloneable = volume == destination_volume;
            }
            if meta.is_file() && !entry.cloneable {
                size.cross_volume_bytes = size.cross_volume_bytes.saturating_add(meta.len());
            }
            if meta.is_file()
                && (entry.relative.starts_with(".git")
                    || git_objects
                        .iter()
                        .any(|objects| source.join(&entry.relative).starts_with(objects)))
            {
                git_bytes = git_bytes.saturating_add(meta.len());
            }
        }
        if git_backed {
            match git_allocation(&source, &entries, destination_case_sensitive) {
                Ok(bytes) => size.git_head_bytes = bytes,
                Err(error) => tree.refusals.push(error.to_string()),
            }
            // Fetch can create a new pack and its temporary output before removing it.
            size.git_transfer_bytes = git_bytes.saturating_mul(2);
        }
        size.refresh_estimate();
        Ok(Self {
            source,
            entries,
            size,
            destination_volume,
            caches: tree.caches,
            repositories: tree.repositories,
            refusals: tree.refusals,
            excluded_worktree_folders: tree.excluded_worktree_folders,
            plain_git_metadata: tree.plain_git_metadata,
            worktree_data: tree.worktree_data,
        })
    }

    pub fn existing_main(source: &Path, destination: &Path) -> Result<Self> {
        if !fs::symlink_metadata(source)?.is_dir() {
            return Err(NativeLayrError::Refused(
                "Existing native main is not a private directory".into(),
            ));
        }
        let source = CanonicalPath::existing(source).map(CanonicalPath::into_path_buf)?;
        let mut entries = Vec::new();
        let mut pending = vec![PathBuf::new()];
        let mut size = Size::default();
        while let Some(relative) = pending.pop() {
            let metadata = fs::symlink_metadata(source.join(&relative))?;
            size.entries += 1;
            size.metadata_bytes = size.metadata_bytes.saturating_add(ENTRY_METADATA_BYTES);
            if metadata.is_dir() {
                let mut children =
                    fs::read_dir(source.join(&relative))?.collect::<std::io::Result<Vec<_>>>()?;
                children.sort_by_key(|entry| entry.file_name());
                pending.extend(
                    children
                        .into_iter()
                        .rev()
                        .map(|entry| relative.join(entry.file_name())),
                );
            } else {
                size.files += 1;
                size.logical_bytes = size.logical_bytes.saturating_add(metadata.len());
                size.tracked_bytes = size.tracked_bytes.saturating_add(metadata.len());
            }
            entries.push(Entry {
                relative,
                metadata,
                cache: false,
                cloneable: true,
            });
        }
        size.refresh_estimate();
        Ok(Self {
            source,
            entries,
            caches: Vec::new(),
            repositories: Vec::new(),
            refusals: Vec::new(),
            excluded_worktree_folders: Vec::new(),
            plain_git_metadata: Vec::new(),
            worktree_data: Vec::new(),
            size,
            destination_volume: volume_id(existing_ancestor(destination)?)?,
        })
    }

    pub fn saved_main(state: &Path, main: &Path, destination: &Path) -> Result<Self> {
        let lines: super::mapping::response::Lines =
            studio_json::from_str(&super::run(state, main, &["line", "list", "--json"])?)
                .map_err(|e| NativeLayrError::Refused(e.to_string()))?;
        let head = lines
            .main_head()
            .ok_or_else(|| NativeLayrError::Refused("Missing saved main state".into()))?;
        let project = main
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| NativeLayrError::Refused("Missing private project directory".into()))?;
        let mut layers = Vec::new();
        #[cfg(target_os = "macos")]
        {
            let mut head = head.to_owned();
            let mut seen = BTreeSet::new();
            loop {
                if head.len() != 36
                    || uuid::Uuid::parse_str(&head).is_err()
                    || !seen.insert(head.clone())
                {
                    return Err(NativeLayrError::Refused(
                        "Invalid or cyclic saved main state".into(),
                    ));
                }
                let full = project.join("states").join(&head);
                if full.exists() {
                    layers.push(Self::existing_main(&full, destination)?);
                    break;
                }
                let delta = project.join("deltas").join(&head);
                let layer = Self::existing_main(&delta, destination)?;
                let count =
                    rustix::fs::lgetxattr(&delta, "com.layr.subvolume", &mut [] as &mut [u8])
                        .map_err(std::io::Error::from)?;
                let mut bytes = vec![0; count];
                rustix::fs::lgetxattr(&delta, "com.layr.subvolume", bytes.as_mut_slice())
                    .map_err(std::io::Error::from)?;
                let marker: studio_json::Value = studio_json::from_slice(&bytes)
                    .map_err(|e| NativeLayrError::Refused(e.to_string()))?;
                head = marker["base"]
                    .as_str()
                    .ok_or_else(|| NativeLayrError::Refused("Saved main layer has no base".into()))?
                    .to_owned();
                layers.push(layer);
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if head.len() != 36 || uuid::Uuid::parse_str(head).is_err() {
                return Err(NativeLayrError::Refused(
                    "Invalid or cyclic saved main state".into(),
                ));
            }
            let full = project.join("states").join(head);
            if !full.exists() {
                return Err(NativeLayrError::Refused(
                    "Saved main state is unavailable".into(),
                ));
            }
            layers.push(Self::existing_main(&full, destination)?);
        }
        // Summing layers also charges removed and replaced entries. It is a read-only upper bound.
        let mut result = layers
            .pop()
            .ok_or_else(|| NativeLayrError::Refused("Missing saved main tree".into()))?;
        for layer in layers {
            result.size.include(layer.size);
            result.entries.extend(layer.entries);
        }
        saved_caches::include(&mut result, project, destination)?;
        result.entries.sort_by(|a, b| a.relative.cmp(&b.relative));
        Ok(result)
    }

    pub fn line_new_metadata_bytes(&self) -> u64 {
        self.size
            .entries
            .saturating_mul(LINE_NEW_ENTRY_METADATA_BYTES)
    }

    pub fn migration_size(&self, main: &Self, repository: &Path) -> Size {
        let mut size = self.size;
        // collect_tree records entries in component order, with each subtree contiguous.
        let first = main
            .entries
            .partition_point(|entry| entry.relative.as_path() < repository);
        let replaced =
            main.entries[first..].partition_point(|entry| entry.relative.starts_with(repository));
        size.inherited_entries = main.size.entries.saturating_sub(replaced as u64);
        size.inherited_metadata_bytes = size.inherited_entries.saturating_mul(ENTRY_METADATA_BYTES);
        size.metadata_bytes = size
            .metadata_bytes
            .saturating_add(size.inherited_metadata_bytes);
        size.transient_metadata_bytes = main
            .line_new_metadata_bytes()
            .saturating_sub(size.metadata_bytes);
        size.refresh_estimate();
        size
    }

    pub fn has_versioned_files_under(&self, path: &Path) -> bool {
        self.entries.iter().any(|entry| {
            !entry.cache && !entry.metadata.is_dir() && entry.relative.starts_with(path)
        })
    }

    #[cfg(test)]
    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.entries
            .iter()
            .filter(|e| !e.metadata.is_dir() && !e.cache && !is_root_git(&e.relative))
            .map(|e| e.relative.as_path())
    }

    /// Clone failures never silently become unbudgeted byte copies.
    pub fn admit(&self) -> Result<()> {
        if !self.refusals.is_empty() {
            return Err(NativeLayrError::Refused(self.refusals.join("\n")));
        }
        Ok(())
    }

    pub fn stage(&self, destination: &Path) -> Result<()> {
        let root = Directory::open(destination)?;
        self.stage_at(&root, Path::new(""))
    }

    pub(super) fn stage_with_progress(
        &self,
        destination: &Path,
        progress: &dyn Fn(u64, u64),
    ) -> Result<()> {
        let root = Directory::open(destination)?;
        self.stage_entries(&root, false, Some(progress))?;
        self.directory_modes(&root, false)
    }

    pub fn stage_at(&self, project: &Directory, relative: &Path) -> Result<()> {
        let destination = project.child(relative, true)?;
        self.stage_open(&destination)
    }

    pub fn stage_open(&self, destination: &Directory) -> Result<()> {
        self.stage_open_with_progress(destination, None)
    }

    pub(super) fn stage_open_with_progress(
        &self,
        destination: &Directory,
        progress: Option<&dyn Fn(u64, u64)>,
    ) -> Result<()> {
        self.stage_entries(destination, false, progress)?;
        self.directory_modes(destination, false)
    }

    fn stage_entries(
        &self,
        destination: &Directory,
        caches: bool,
        progress: Option<&dyn Fn(u64, u64)>,
    ) -> Result<()> {
        use rustix::fs::{Mode, OFlags, openat, symlinkat};
        self.admit()?;
        self.verify_source()?;
        if volume_id_fd(&destination.0)? != self.destination_volume {
            return Err(NativeLayrError::Refused(
                "The staging volume changed after the space preflight".into(),
            ));
        }
        let total = self
            .entries
            .iter()
            .filter(|entry| {
                entry.cache == caches
                    && !entry.relative.as_os_str().is_empty()
                    && !entry.metadata.is_dir()
            })
            .count() as u64;
        let mut done = 0;
        if let Some(progress) = progress {
            progress(done, total);
        }
        for entry in &self.entries {
            if entry.cache != caches || entry.relative.as_os_str().is_empty() {
                continue;
            }
            let relative = &entry.relative;
            let meta = &entry.metadata;
            if meta.is_dir() {
                destination.child(relative, true)?;
                continue;
            }
            let (parent, name) = destination.parent(relative, true)?;
            if meta.file_type().is_symlink() {
                symlinkat(
                    fs::read_link(self.source.join(&entry.relative))?,
                    &parent.0,
                    &name,
                )
                .map_err(std::io::Error::from)?;
            } else {
                let source = open_source(&self.source, &entry.relative)?;
                if vm_archive_identity(&source.metadata()?) != vm_archive_identity(meta) {
                    return Err(source_changed());
                }
                if entry.cloneable {
                    clone_file(&source,&parent.0,&name).map_err(|error|NativeLayrError::Refused(format!("Cannot clone {}: {error}. Native import does not fall back to an unbudgeted byte copy.",self.source.join(&entry.relative).display())))?;
                } else {
                    let mut copy = File::from(
                        openat(
                            &parent.0,
                            &name,
                            OFlags::CREATE
                                | OFlags::EXCL
                                | OFlags::WRONLY
                                | OFlags::NOFOLLOW
                                | OFlags::CLOEXEC,
                            Mode::from_raw_mode(0o600),
                        )
                        .map_err(std::io::Error::from)?,
                    );
                    std::io::copy(&mut &source, &mut copy)?;
                }
                let file = File::from(
                    openat(
                        &parent.0,
                        &name,
                        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(std::io::Error::from)?,
                );
                file.set_permissions(fs::Permissions::from_mode(meta.mode() & 0o7777))?;
                file.set_times(fs::FileTimes::new().set_modified(meta.modified()?))?;
            }
            done += 1;
            if let Some(progress) = progress {
                progress(done, total);
            }
        }
        for entry in self
            .entries
            .iter()
            .filter(|entry| entry.cache == caches && !entry.relative.as_os_str().is_empty())
        {
            attributes::copy(
                &self.source,
                &entry.relative,
                destination,
                &entry.relative,
                entry.metadata.file_type().is_symlink(),
            )?;
        }
        self.verify_source()
    }

    fn directory_modes(&self, destination: &Directory, caches: bool) -> Result<()> {
        for entry in self
            .entries
            .iter()
            .rev()
            .filter(|entry| entry.cache == caches && entry.metadata.is_dir())
        {
            destination
                .child(&entry.relative, false)?
                .set_mode(entry.metadata.mode() & 0o7777)?;
        }
        Ok(())
    }

    /// Restore directory attributes after Layr's folder import creates its own directories.
    pub fn restore_directory_attributes(&self, destination: &Path) -> Result<bool> {
        let destination = Directory::open(destination)?;
        let mut changed = false;
        for entry in self.entries.iter().filter(|entry| {
            entry.metadata.is_dir()
                && !entry.cache
                && !entry.relative.as_os_str().is_empty()
                && !is_root_git(&entry.relative)
        }) {
            changed |= attributes::copy(
                &self.source,
                &entry.relative,
                &destination,
                &entry.relative,
                false,
            )?;
        }
        self.verify_source()?;
        Ok(changed)
    }

    fn verify_source(&self) -> Result<()> {
        for entry in &self.entries {
            let after = fs::symlink_metadata(self.source.join(&entry.relative))?;
            if vm_archive_identity(&after) != vm_archive_identity(&entry.metadata) {
                return Err(source_changed());
            }
        }
        Ok(())
    }
}

fn is_root_git(relative: &Path) -> bool {
    relative.starts_with(".git")
}

fn source_changed() -> NativeLayrError {
    NativeLayrError::Refused(
        "Project source changed during native Layr import. Keep the source quiet and retry.".into(),
    )
}

fn open_parent(
    root: &Path,
    relative: &Path,
) -> std::io::Result<(std::os::fd::OwnedFd, std::ffi::OsString)> {
    use rustix::fs::{Mode, OFlags, open, openat};
    let directory = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut parent = open(root, directory, Mode::empty())?;
    let mut parts = relative.components().peekable();
    while let Some(part) = parts.next() {
        let std::path::Component::Normal(name) = part else {
            return Err(std::io::Error::other(
                "Private path contains a parent traversal",
            ));
        };
        if parts.peek().is_none() {
            return Ok((parent, name.to_owned()));
        }
        parent = openat(&parent, name, directory, Mode::empty())?;
    }
    Err(std::io::Error::other("Missing entry name"))
}

fn open_source(root: &Path, relative: &Path) -> std::io::Result<File> {
    use rustix::fs::{Mode, OFlags, openat};
    let (parent, name) = open_parent(root, relative)?;
    Ok(File::from(openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )?))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VolumeId {
    Other(u64),
    #[cfg(target_os = "linux")]
    Btrfs([u8; 16]),
}

fn volume_id_fd(fd: &impl std::os::fd::AsFd) -> std::io::Result<VolumeId> {
    #[cfg(target_os = "macos")]
    {
        Ok(VolumeId::Other(rustix::fs::fstat(fd)?.st_dev as u64))
    }
    #[cfg(target_os = "linux")]
    {
        filesystem_ffi::volume_id(fd)
    }
}

pub(super) fn existing_ancestor(path: &Path) -> std::io::Result<&Path> {
    let mut at = path;
    loop {
        match fs::metadata(at) {
            Ok(_) => return Ok(at),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                at = at.parent().ok_or(error)?;
            }
            Err(error) => return Err(error),
        }
    }
}

fn case_key(path: &str) -> String {
    let normalizer = icu_normalizer::DecomposingNormalizerBorrowed::new_nfd();
    // Match the installed Layr APFS case key, including compatibility case variants.
    let folded: String = normalizer
        .normalize(path)
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ς' => 'σ',
            'ſ' => 's',
            'ϐ' => 'β',
            'ϑ' => 'θ',
            'ϕ' => 'φ',
            'ϖ' => 'π',
            'ϰ' => 'κ',
            'ϱ' => 'ρ',
            'ϵ' => 'ε',
            c => c,
        })
        .collect();
    normalizer.normalize(&folded).into_owned()
}

fn validate_names<'a>(
    paths: impl Iterator<Item = &'a Path>,
    case_sensitive: bool,
    unicode_normalizing: bool,
) -> Result<()> {
    // Component order keeps each directory's descendants together. Retain only
    // sibling tables along the current path, and normalize each entry once.
    let mut paths: Vec<_> = paths.collect();
    paths.sort_unstable();
    let mut previous = Vec::new();
    let mut levels = vec![std::collections::HashMap::<String, String>::new()];
    let mut collisions = BTreeSet::new();
    for path in paths {
        path.to_str()
            .ok_or_else(|| NativeLayrError::Refused("Non-UTF-8 import path".into()))?;
        let components: Vec<_> = path.components().collect();
        let common = previous
            .iter()
            .zip(&components)
            .take_while(|(a, b)| a == b)
            .count();
        levels.truncate(common + 1);
        for (depth, component) in components.iter().enumerate().skip(common) {
            let name = component
                .as_os_str()
                .to_str()
                .ok_or_else(|| NativeLayrError::Refused("Non-UTF-8 import path".into()))?;
            let key = if !case_sensitive {
                case_key(name)
            } else if unicode_normalizing {
                icu_normalizer::DecomposingNormalizerBorrowed::new_nfd()
                    .normalize(name)
                    .into_owned()
            } else {
                name.to_owned()
            };
            if let Some(old) = levels[depth].get(&key) {
                if old != name {
                    let parent: PathBuf = components[..depth].iter().collect();
                    collisions.insert(format!(
                        "{}, {}",
                        parent.join(old).display(),
                        parent.join(name).display()
                    ));
                }
            } else {
                levels[depth].insert(key, name.to_owned());
            }
            levels.push(std::collections::HashMap::new());
        }
        previous = components;
    }
    let collisions: Vec<_> = collisions.into_iter().collect();
    if !collisions.is_empty() {
        return Err(NativeLayrError::Refused(format!(
            "Import paths differ only in case or Unicode form and cannot coexist on this volume:\n{}",
            collisions.join("\n")
        )));
    }
    Ok(())
}

struct Entry {
    relative: PathBuf,
    metadata: fs::Metadata,
    cache: bool,
    cloneable: bool,
}
pub(super) struct Repository {
    pub relative: PathBuf,
    pub kind: &'static str,
    pub included: bool,
    pub cache: bool,
}
#[derive(Default)]
struct Selection {
    project_root: PathBuf,
    without_git: bool,
    ignored: BTreeSet<PathBuf>,
    entries: Vec<Entry>,
    caches: Vec<PathBuf>,
    repositories: Vec<Repository>,
    plain_git_metadata: Vec<metadata::PlainMetadata>,
    worktree_data: Vec<PathBuf>,
    refusals: Vec<String>,
    excluded_worktree_folders: Vec<PathBuf>,
}

/// A repeat migration can skip only commits retained by the earlier import.
pub(super) fn verify_imported_head(main: &Path, repository: &Path, head: &str) -> Result<()> {
    if !matches!(head.len(), 40 | 64) || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(NativeLayrError::Refused("Invalid skipped HEAD".into()));
    }
    let main = CanonicalPath::existing(main).map(CanonicalPath::into_path_buf)?;
    let folder = main.join(repository);
    if repository.as_os_str().is_empty() {
        super::inventory::git(&main, &["cat-file", "-e", &format!("{head}^{{commit}}")])?;
    } else {
        let marker = folder.join(".git");
        let git = if marker.is_dir() {
            marker
        } else {
            crate::workspace_paths::read_gitdir_pointer(&folder, &marker)?.into_path_buf()
        };
        metadata::validate_stored_git(&main, &git)?;
        let git = git
            .to_str()
            .ok_or_else(|| NativeLayrError::Refused("Non-UTF-8 imported Git path".into()))?;
        super::inventory::git(
            &main,
            &[
                "--git-dir",
                git,
                "cat-file",
                "-e",
                &format!("{head}^{{commit}}"),
            ],
        )?;
    }
    Ok(())
}

fn ignored_paths(repository: &Path) -> Result<BTreeSet<PathBuf>> {
    use std::os::unix::ffi::OsStrExt;
    let paths = super::inventory::git(
        repository,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "-z",
        ],
    )?;
    Ok(paths
        .split(|byte| *byte == 0)
        .filter(|item| !item.is_empty())
        .map(|item| {
            PathBuf::from(std::ffi::OsStr::from_bytes(
                item.strip_suffix(b"/").unwrap_or(item),
            ))
        })
        .collect())
}

fn collect_tree(
    root: &Path,
    relative: &Path,
    mut cache: bool,
    inside_git: bool,
    worktree: bool,
    tree: &mut Selection,
) -> Result<()> {
    if tree.project_root.as_os_str().is_empty() {
        tree.project_root = root.to_owned();
    }
    let path = root.join(relative);
    if !cache && tree.ignored.contains(relative) && !crate::workspace_paths::regenerable_path(&path)
    {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(&path)?;
    let directory = metadata.is_dir();
    if !inside_git && directory {
        if !relative.as_os_str().is_empty()
            && !cache
            && crate::workspace_paths::regenerable_path(&path)
        {
            cache = true;
            tree.caches.push(relative.to_owned());
        }
        let marker = path.join(".git");
        if let Ok(marker_metadata) = fs::symlink_metadata(&marker)
            && !(relative.as_os_str().is_empty() && tree.without_git)
        {
            // Root metadata retains its explicit admission policy and --without-git choice.
            let classification = if relative.as_os_str().is_empty() {
                Ok(None)
            } else {
                metadata::plain_reason(&path, &marker)
            };
            match classification {
                Ok(Some(reason)) => tree.plain_git_metadata.push(metadata::PlainMetadata {
                    relative_path: relative.join(".git"),
                    reason,
                    cache,
                }),
                Err(error) => tree.refusals.push(error.to_string()),
                Ok(None) => {
                    let mut kind = "plain";
                    let mut included = true;
                    if marker_metadata.file_type().is_symlink() {
                        included = false;
                        tree.refusals
                            .push(format!("External Git metadata link: {}", marker.display()));
                    } else if marker_metadata.is_file() {
                        kind = "worktree";
                        if relative.as_os_str().is_empty() && worktree {
                            // Validate this pointer against the original project boundary below.
                        } else {
                            match crate::workspace_paths::read_gitdir_pointer(&path, &marker) {
                                Ok(target)
                                    if internal_worktree(tree.project_root.as_path(), &target) =>
                                {
                                    tree.repositories.push(Repository {
                                        relative: relative.to_owned(),
                                        kind,
                                        included: false,
                                        cache,
                                    });
                                    return Ok(());
                                }
                                Ok(target) if target.starts_with(&tree.project_root) => {
                                    kind = "submodule";
                                }
                                Ok(target) => {
                                    included = false;
                                    tree.refusals.push(format!(
                                        "External Git object store: {} -> {}",
                                        marker.display(),
                                        target.display()
                                    ));
                                }
                                Err(error) => {
                                    included = false;
                                    tree.refusals.push(format!(
                                        "Cannot read Git pointer {}: {error}",
                                        marker.display()
                                    ));
                                }
                            }
                        }
                    }
                    if included
                        && let Err(error) = metadata::validate_repository(&tree.project_root, &path)
                    {
                        included = false;
                        tree.refusals
                            .push(format!("Git metadata at {}: {error}", marker.display()));
                    }
                    if included && !cache && !relative.as_os_str().is_empty() && kind != "submodule"
                    {
                        tree.ignored.extend(
                            ignored_paths(&path)?
                                .into_iter()
                                .map(|ignored| relative.join(ignored)),
                        );
                    }
                    tree.repositories.push(Repository {
                        relative: relative.to_owned(),
                        kind,
                        included,
                        cache,
                    });
                }
            }
        }
    }
    if !directory && !metadata.is_file() && !metadata.file_type().is_symlink() {
        tree.refusals.push(format!(
            "Special file cannot be imported: {}",
            path.display()
        ));
        return Ok(());
    }
    if relative.parent().and_then(Path::file_name) == Some(std::ffi::OsStr::new(".worktrees")) {
        tree.worktree_data.push(relative.to_owned());
    }
    tree.entries.push(Entry {
        relative: relative.to_owned(),
        metadata,
        cache,
        cloneable: false,
    });
    if directory {
        let mut children = fs::read_dir(&path)?.collect::<std::io::Result<Vec<_>>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let name = child.file_name();
            if !inside_git && name == ".worktrees" && child.file_type()?.is_dir() {
                tree.excluded_worktree_folders
                    .push(child_relative_path(root, &child.path())?);
                // Only a registered linked worktree marker stops the normal walk below.
            }
            if relative.as_os_str().is_empty() && (worktree || tree.without_git) && name == ".git" {
                continue;
            }
            let child_relative = relative.join(&name);
            collect_tree(
                root,
                &child_relative,
                cache,
                inside_git || name == ".git",
                worktree,
                tree,
            )?;
        }
    }
    Ok(())
}

/// Adds cache paths to the project `regenerable` setting, which every line reads, and keeps the
/// paths already set. `config -l` always exits 0, so an unset setting reads as an empty list.
fn add_regenerable(state: &Path, cwd: &Path, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let listed = super::run(state, cwd, &["config", "-l"])?;
    let mut all: Vec<String> = match listed
        .lines()
        .find_map(|line| line.strip_prefix("regenerable="))
    {
        Some(value) => studio_json::from_str(value)
            .map_err(|error| NativeLayrError::Refused(error.to_string()))?,
        None => Vec::new(),
    };
    for path in paths {
        if !all.contains(path) {
            all.push(path.clone());
        }
    }
    let value = studio_json::to_string(&all)
        .map_err(|error| NativeLayrError::Refused(error.to_string()))?;
    super::run(state, cwd, &["config", "regenerable", &value])?;
    Ok(())
}

impl Plan {
    pub fn warm_caches(&self, state: &Path, cwd: &Path, relative: &Path) -> Result<()> {
        let project = Directory::open(cwd)?;
        let paths: Vec<String> = self
            .caches
            .iter()
            .map(|cache| relative.join(cache).to_string_lossy().into_owned())
            .collect();
        add_regenerable(state, cwd, &paths)?;
        let destination = project.child(relative, false)?;
        self.stage_entries(&destination, true, None)?;
        self.directory_modes(&destination, true)?;
        if !self.caches.is_empty() {
            super::run(state, cwd, &["save", "--caches"])?;
        }
        Ok(())
    }
}

fn volume_id(path: &Path) -> std::io::Result<VolumeId> {
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    volume_id_fd(&fd)
}

pub(super) fn allocation_model() -> studio_json::Value {
    studio_json::json!({
        "cacheEntryMetadataBytes":ENTRY_METADATA_BYTES,
        "versionedEntryMetadataBytes":ENTRY_METADATA_BYTES,
        "lineNewEntryMetadataBytes":LINE_NEW_ENTRY_METADATA_BYTES,
        "metadataSafetyFactor":METADATA_SAFETY_FACTOR,
        "cacheMeasuredPeakBytes":CACHE_MEASURED_PEAK_BYTES,
        "cacheMeasuredEntries":CACHE_MEASURED_ENTRIES,
        "lineNewMeasuredPeakBytes":LINE_NEW_MEASURED_PEAK_BYTES,
        "lineNewMeasuredEntries":LINE_NEW_MEASURED_ENTRIES,
        "migratedMeasuredPeakBytes":MIGRATED_MEASURED_PEAK_BYTES,
        "migratedMeasuredEntries":MIGRATED_MEASURED_ENTRIES,
        "minimumSpaceMarginBytes":SPACE_MARGIN_BYTES
    })
}

mod attributes;
mod metadata;
mod saved_caches;
mod sizing;
use sizing::git_allocation;
#[cfg(test)]
include!("import/tests.rs");

fn internal_worktree(root: &Path, target: &Path) -> bool {
    target.strip_prefix(root).is_ok_and(|relative| {
        let parts: Vec<_> = relative.components().collect();
        parts
            .windows(2)
            .any(|parts| parts[0].as_os_str() == ".git" && parts[1].as_os_str() == "worktrees")
    })
}

fn child_relative_path(root: &Path, path: &Path) -> Result<PathBuf> {
    path.strip_prefix(root)
        .map(Path::to_owned)
        .map_err(|error| NativeLayrError::Refused(error.to_string()))
}

#[cfg(test)]
mod review_tests;

/// Validate the actual copied Git input before Layr can run a Git child with a cleared environment.
pub(super) fn validate_private_git(staging: &Path) -> Result<()> {
    let staging = CanonicalPath::existing(staging).map(CanonicalPath::into_path_buf)?;
    metadata::validate_repository(&staging, &staging)
}
