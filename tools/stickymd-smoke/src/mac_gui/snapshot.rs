//! What R03's "nothing written" compares (review F5): every entry under the folder that holds the
//! bundle, recursively and without following links — each file's size, SHA-256 and modification
//! time, each folder's modification time, each link's target. Rewriting a file, adding one in a
//! subfolder, or creating and removing one (which moves its folder's modification time) all show
//! as a difference; a direct-child name list shows none of them.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use std::{fs, path::Path, time::UNIX_EPOCH};

fn mtime(metadata: &fs::Metadata) -> Result<u128, String> {
    Ok(metadata
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos())
}

fn walk(root: &Path, directory: &Path, into: &mut Vec<String>) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
        let kind = metadata.file_type();
        if kind.is_symlink() {
            let target = fs::read_link(&path).map_err(|e| e.to_string())?;
            into.push(format!("link {name} -> {}", target.display()));
        } else if kind.is_dir() {
            into.push(format!("dir {name} mtime={}", mtime(&metadata)?));
            walk(root, &path, into)?;
        } else if kind.is_file() {
            into.push(format!(
                "file {name} bytes={} sha256={} mtime={}",
                metadata.len(),
                crate::integrity::sha256(&path)?,
                mtime(&metadata)?
            ));
        } else {
            into.push(format!("other {name}"));
        }
    }
    Ok(())
}

/// The sorted entries of `root`, itself first as `.`.
pub(super) fn tree(root: &Path) -> Result<Vec<String>, String> {
    let metadata = fs::symlink_metadata(root).map_err(|e| format!("{}: {e}", root.display()))?;
    let mut entries = vec![format!("dir . mtime={}", mtime(&metadata)?)];
    walk(root, root, &mut entries)?;
    entries[1..].sort();
    Ok(entries)
}

/// The name an entry line of [`tree`] lists: `.` for the root. None for a line `tree` does not
/// write. A name holding the separator that follows it reads short, so it then matches no
/// registered name (fail closed).
pub(super) fn name(line: &str) -> Option<&str> {
    let (kind, rest) = line.split_once(' ')?;
    match kind {
        "file" => rest.rsplit_once(" bytes=").map(|(name, _)| name),
        "dir" => rest.rsplit_once(" mtime=").map(|(name, _)| name),
        "link" => rest.split_once(" -> ").map(|(name, _)| name),
        "other" => Some(rest),
        _ => None,
    }
}

/// The `(name, path)` of `root` and every entry under it, in [`tree`]'s order: what R03's write
/// watch registers before the snapshot is taken.
pub(super) fn entries(root: &Path) -> Result<Vec<(String, std::path::PathBuf)>, String> {
    let mut found = Vec::new();
    collect(root, root, &mut found)?;
    found.sort();
    found.insert(0, (".".to_owned(), root.to_path_buf()));
    Ok(found)
}

fn collect(
    root: &Path,
    directory: &Path,
    into: &mut Vec<(String, std::path::PathBuf)>,
) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
        let is_dir = fs::symlink_metadata(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .is_dir();
        into.push((name, path.clone()));
        if is_dir {
            collect(root, &path, into)?;
        }
    }
    Ok(())
}

/// The entries in `before` or `after` but not both: what a run changed.
pub(super) fn changed(before: &[String], after: &[String]) -> Vec<String> {
    let gone = before
        .iter()
        .filter(|e| !after.contains(e))
        .map(|e| format!("- {e}"));
    let new = after
        .iter()
        .filter(|e| !before.contains(e))
        .map(|e| format!("+ {e}"));
    gone.chain(new).collect()
}

// Links and directory times as macOS has them.
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    /// A folder like the release layout's, with its files' times set back so a rewrite moves
    /// them whatever the file system's time resolution.
    fn layout(root: &Path) {
        fs::create_dir_all(root.join("licenses")).unwrap();
        fs::create_dir_all(root.join("StickyMD.app/Contents")).unwrap();
        for file in [
            "README.txt",
            "licenses/A.txt",
            "StickyMD.app/Contents/Info.plist",
        ] {
            fs::write(root.join(file), file).unwrap();
        }
        let old = SystemTime::now() - Duration::from_secs(3600);
        for path in [
            root.join("README.txt"),
            root.join("licenses/A.txt"),
            root.join("StickyMD.app/Contents/Info.plist"),
            root.join("licenses"),
            root.join("StickyMD.app/Contents"),
            root.join("StickyMD.app"),
            root.to_path_buf(),
        ] {
            fs::File::open(&path).unwrap().set_modified(old).unwrap();
        }
    }

    /// A write to the folder under observation.
    type Change = fn(&Path);

    /// The writes a direct-child name list misses are all differences here.
    #[test]
    fn rewrites_nested_files_and_transient_files_all_show() {
        let directory = std::env::temp_dir().join(format!(
            "stickymd-r03-snapshot-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = directory.join("StickyMD");
        layout(&root);
        let before = tree(&root).unwrap();
        assert_eq!(
            before,
            tree(&root).unwrap(),
            "nothing changed, nothing differs"
        );
        assert!(changed(&before, &before).is_empty());
        // Review #24 S5: each sample changes one thing, and the difference names it.
        let samples: [(&str, Change, &[&str]); 4] = [
            // Same length, time put back: only the content digest tells.
            (
                "rewritten",
                |root| {
                    let path = root.join("README.txt");
                    let time = fs::metadata(&path).unwrap().modified().unwrap();
                    fs::write(&path, "READMEXtxt").unwrap();
                    fs::File::open(&path).unwrap().set_modified(time).unwrap();
                },
                &[
                    "- file README.txt bytes=10 sha256=",
                    "+ file README.txt bytes=10 sha256=",
                ],
            ),
            (
                "nested",
                |root| fs::write(root.join("licenses/B.txt"), "new").unwrap(),
                &["+ file licenses/B.txt", "+ dir licenses mtime="],
            ),
            // Written and removed: only the folder's time tells.
            (
                "transient",
                |root| {
                    fs::write(root.join("note.tmp"), "x").unwrap();
                    fs::remove_file(root.join("note.tmp")).unwrap();
                },
                &["- dir . mtime=", "+ dir . mtime="],
            ),
            (
                "link",
                |root| std::os::unix::fs::symlink("/tmp", root.join("note")).unwrap(),
                &["+ link note -> /tmp"],
            ),
        ];
        for (name, change, expected) in samples {
            layout(&root);
            let before = tree(&root).unwrap();
            change(&root);
            let diff = changed(&before, &tree(&root).unwrap());
            for line in expected {
                assert!(
                    diff.iter().any(|d| d.starts_with(line)),
                    "{name}: {line:?} not in {diff:?}"
                );
            }
            if name == "rewritten" || name == "transient" {
                assert_eq!(diff.len(), 2, "{name}: {diff:?}");
            }
            let _ = fs::remove_file(root.join("licenses/B.txt"));
            let _ = fs::remove_file(root.join("note"));
        }
        fs::remove_dir_all(&directory).unwrap();
    }
}
