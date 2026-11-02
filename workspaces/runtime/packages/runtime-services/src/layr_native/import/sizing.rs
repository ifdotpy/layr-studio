//! Conservative allocation estimates for installed Layr releases.
use super::*;

struct HeadFile {
    path: PathBuf,
    bytes: u64,
}
fn git_head(source: &Path) -> Result<Vec<HeadFile>> {
    let output = match super::super::inventory::git(source, &["ls-tree", "-r", "-l", "-z", "HEAD"])
    {
        Ok(output) => output,
        Err(error) if error.to_string().contains("Not a valid object name HEAD") => {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    let mut files = Vec::new();
    for record in output.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let text = std::str::from_utf8(record)
            .map_err(|_| NativeLayrError::Refused("Non-UTF-8 Git HEAD path".into()))?;
        let (metadata, path) = text
            .split_once('\t')
            .ok_or_else(|| NativeLayrError::Refused("Invalid Git HEAD listing".into()))?;
        let fields: Vec<_> = metadata.split_whitespace().collect();
        if fields.get(1) == Some(&"blob") {
            let bytes = fields
                .get(3)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| NativeLayrError::Refused("Invalid Git HEAD size".into()))?;
            files.push(HeadFile {
                path: PathBuf::from(path),
                bytes,
            });
        }
    }
    Ok(files)
}

pub(super) fn git_allocation(
    source: &Path,
    entries: &[Entry],
    destination_case_sensitive: bool,
) -> Result<u64> {
    let head = git_head(source)?;
    validate_names(
        head.iter().map(|file| file.path.as_path()),
        destination_case_sensitive,
        cfg!(target_os = "macos"),
    )?;
    let selected: BTreeMap<_, _> = entries
        .iter()
        .filter(|entry| entry.metadata.is_file() && !entry.cache)
        .map(|entry| (entry.relative.as_path(), entry))
        .collect();
    Ok(head.iter().fold(0u64, |sum, file| {
        sum.saturating_add(file.bytes).saturating_add(
            selected
                .get(file.path.as_path())
                .map(|entry| entry.metadata.len())
                .unwrap_or(0),
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_conservatively_counts_head_and_source_blobs() {
        let source = tempfile::tempdir().expect("source");
        let git = |args: &[&str]| {
            super::super::super::inventory::git(source.path(), args).expect("fixture Git command")
        };
        git(&["init", "-q"]);
        for (name, bytes) in [
            ("same", "same"),
            ("changed", "before"),
            ("missing", "gone"),
            ("executable", "#!/bin/sh\n"),
        ] {
            fs::write(source.path().join(name), bytes).expect("file");
        }
        fs::set_permissions(
            source.path().join("executable"),
            fs::Permissions::from_mode(0o755),
        )
        .expect("executable mode");
        std::os::unix::fs::symlink("same", source.path().join("link")).expect("link");
        fs::create_dir(source.path().join("node_modules")).expect("cache");
        fs::write(source.path().join("node_modules/cached"), "warm").expect("cache file");
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-qm",
            "Base",
        ]);
        fs::write(source.path().join("changed"), "after!!").expect("change");
        fs::remove_file(source.path().join("missing")).expect("delete");
        let mut tree = Selection::default();
        collect_tree(source.path(), Path::new(""), false, false, false, &mut tree)
            .expect("selection");
        assert_eq!(
            git_allocation(source.path(), &tree.entries, true).expect("estimate"),
            53
        );
    }
}
