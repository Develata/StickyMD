//! Where release writers may not write: the coordinator-owned qualification storage of
//! the verification tool (plan 11 #module-success-ledger, #manual-observation-log). Release commands
//! write where their caller says; a release output never belongs in that storage, so the guard
//! refuses the whole evidence directory rather than naming each coordinator-owned file in it,
//! which this crate does not know. Paths are compared by filesystem identity (aliases resolved,
//! case folded on Windows), so a junction, 8.3 name or `..` cannot make a reserved destination
//! look ordinary.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use crate::path_identity::{is_within, normalize, reaches};
use std::fs;
use std::path::{Path, PathBuf};

/// The evidence directory: receipts, ledgers, the manual store and release decisions.
const EVIDENCE: &str = "dist/evidence";
/// The internally owned resource diagnostics cache.
const RESOURCE_DIAGNOSTICS: &str = "target/resource-diagnostics";

/// The links (symbolic links, Windows junctions) found inside the reserved directories, by their
/// resolved targets: a coordinator-owned directory of the evidence tree may live elsewhere, and a
/// caller may name it by its real location. Taken once per command.
pub struct Guard<'a> {
    root: &'a Path,
    link_targets: Vec<String>,
}

impl<'a> Guard<'a> {
    /// Walks the reserved directories without following links. An absent reserved directory
    /// holds no link; any other error reading them refuses, since the walk would be incomplete.
    pub fn new(root: &'a Path) -> Result<Self, String> {
        let mut link_targets = Vec::new();
        let resolved = |path: &Path| {
            fs::canonicalize(path).map(|_| normalize(path)).map_err(|error| {
                format!(
                    "{} is a link that does not resolve ({error}); release outputs cannot be guarded around it",
                    path.display()
                )
            })
        };
        for reserved in [EVIDENCE, RESOURCE_DIAGNOSTICS] {
            let top_path = root.join(reserved);
            match fs::symlink_metadata(&top_path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!("cannot inspect {}: {error}", top_path.display()));
                }
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    link_targets.push(resolved(&top_path)?);
                }
                Ok(_) => {}
            }
            let mut pending = vec![top_path];
            let mut top = true;
            while let Some(directory) = pending.pop() {
                let entries = match fs::read_dir(&directory) {
                    Ok(entries) => entries,
                    Err(error) if top && error.kind() == std::io::ErrorKind::NotFound => break,
                    Err(error) => {
                        return Err(format!(
                            "cannot read {} to guard release outputs: {error}",
                            directory.display()
                        ));
                    }
                };
                top = false;
                for entry in entries {
                    let entry = entry
                        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
                    let kind = entry.file_type().map_err(|error| {
                        format!("cannot inspect {}: {error}", entry.path().display())
                    })?;
                    if kind.is_symlink() {
                        link_targets.push(resolved(&entry.path())?);
                    } else if kind.is_dir() {
                        pending.push(entry.path());
                    }
                }
            }
        }
        Ok(Self { root, link_targets })
    }

    /// Refuses `path` as a file a release command writes: the writer's spelling rules first, on
    /// the path as given and as `std::path::absolute` resolves it (writers do both), so no parent
    /// is created for a path a writer would refuse; then the reserved storage, for the file and
    /// for every directory the write could create on the way (see [`Self::check_directory`]).
    pub fn check_file(&self, path: &Path) -> Result<(), String> {
        crate::atomic::check_spelling(path)?;
        #[cfg(windows)]
        crate::path_identity::validate_output_spelling(path)?;
        let resolved = resolve(path)?;
        crate::atomic::check_spelling(&resolved)?;
        #[cfg(windows)]
        crate::path_identity::validate_output_spelling(&resolved)?;
        self.check_ancestors(path)
    }

    /// Refuses `directory` as a directory a release command writes into.
    pub fn check_directory(&self, directory: &Path) -> Result<(), String> {
        self.check_ancestors(directory)
    }

    /// `create_dir_all` creates the missing lexical ancestors of a path one by one, each resolved
    /// by the platform as `std::path::absolute` resolves it. So the path and every lexical
    /// ancestor are resolved that way and checked: on Windows `absolute` folds `..`
    /// (`dist/evidence\\..\\x` becomes `dist\\x`), and for any directory the final location
    /// alone does not show a reserved directory the path passes through
    /// (`target/resource-diagnostics/probe/../../release`). Only an ancestor that is nothing but a
    /// drive or a root is skipped: it exists and is never created. Relative ancestors such as `..`
    /// are checked, since they resolve against the working directory. Resolving with the writers'
    /// own function keeps the guard and the writer on the same file, whatever form the path takes.
    fn check_ancestors(&self, path: &Path) -> Result<(), String> {
        resolve(path)?;
        for ancestor in checked_ancestors(path) {
            self.check(&resolve(ancestor)?)?;
        }
        Ok(())
    }

    fn check(&self, path: &Path) -> Result<(), String> {
        let root = self.root;
        let target = normalize(&root.join(path));
        let in_link = self.link_targets.iter().any(|linked| {
            target == *linked
                || target
                    .strip_prefix(linked.as_str())
                    .is_some_and(|suffix| suffix.starts_with('/'))
        });
        if in_link
            || reaches(root, path, EVIDENCE)
            || is_within(root, path, RESOURCE_DIAGNOSTICS)
            || is_ledger_store(root, path)
        {
            return Err(format!(
                "{} is coordinator-owned qualification storage; release outputs go elsewhere",
                path.display()
            ));
        }
        Ok(())
    }
}

/// [`Guard::check_file`] with a fresh snapshot.
pub fn check_file(root: &Path, path: &Path) -> Result<(), String> {
    Guard::new(root)?.check_file(path)
}

/// [`Guard::check_directory`] with a fresh snapshot.
pub fn check_directory(root: &Path, directory: &Path) -> Result<(), String> {
    Guard::new(root)?.check_directory(directory)
}

/// `path` and its lexical ancestors, without the empty one and without those made only of a drive
/// prefix and a root.
fn checked_ancestors(path: &Path) -> impl Iterator<Item = &Path> {
    path.ancestors().filter(|ancestor| {
        !ancestor.as_os_str().is_empty()
            && !ancestor.components().all(|part| {
                matches!(
                    part,
                    std::path::Component::Prefix(_) | std::path::Component::RootDir
                )
            })
    })
}

/// `path` against the working directory, as the writers resolve it. An empty path, and on
/// Windows one holding a NUL, is refused here as the writers refuse it.
fn resolve(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path)
        .map_err(|error| format!("cannot resolve output {}: {error}", path.display()))
}

/// The store always lives at `<git common dir>/stickymd/qualification-ledger`. Matching
/// that segment pair on the alias-resolved path needs no git query, so an unavailable
/// git cannot turn the protection off while an existing store is still on disk.
pub fn is_ledger_store(root: &Path, path: &Path) -> bool {
    // Case-folded: on NTFS a differently cased spelling names the same directory.
    let target = normalize(&root.join(path));
    if format!("{target}/")
        .to_ascii_lowercase()
        .contains(STORE_SEGMENTS)
    {
        return true;
    }
    // The store directory itself may be a junction to another place, which removes the
    // segment from the resolved target; compare with the resolved store root as well.
    match git_common_dir_from_files(root) {
        Ok(None) => false,
        Ok(Some(common)) => {
            let store = normalize(&common.join("stickymd").join("qualification-ledger"));
            let (target, store) = (target.to_ascii_lowercase(), store.to_ascii_lowercase());
            target == store
                || target
                    .strip_prefix(&store)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        }
        // A `.git` that cannot be interpreted leaves the store location unknown.
        Err(_) => true,
    }
}

const STORE_SEGMENTS: &str = "/stickymd/qualification-ledger/";

/// The git common directory from `.git` metadata files alone, so the reserved-path
/// check never depends on running git. `None` when the root has no `.git` at all,
/// which proves no clone-wide store belongs to it.
pub fn git_common_dir_from_files(root: &Path) -> Result<Option<PathBuf>, String> {
    let dot_git = root.join(".git");
    let git_dir = match fs::metadata(&dot_git) {
        Ok(metadata) if metadata.is_dir() => dot_git,
        Ok(_) => {
            // A linked worktree: `gitdir: <path>` names its private git directory.
            let text = fs::read_to_string(&dot_git)
                .map_err(|error| format!("cannot read {}: {error}", dot_git.display()))?;
            let pointer = text
                .trim()
                .strip_prefix("gitdir:")
                .ok_or_else(|| format!("{} is not a gitdir file", dot_git.display()))?
                .trim();
            existing_directory(root, pointer, "gitdir")?
        }
        // Only a truly absent entry proves there is no store; a dangling link is not absence.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !entry_exists(&dot_git) => {
            return Ok(None);
        }
        Err(error) => return Err(format!("cannot inspect {}: {error}", dot_git.display())),
    };
    let commondir = git_dir.join("commondir");
    match fs::read_to_string(&commondir) {
        Ok(text) => Ok(Some(existing_directory(
            &git_dir,
            text.trim(),
            "commondir",
        )?)),
        // A gitfile may name the repository itself (`--separate-git-dir`).
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !entry_exists(&commondir) => {
            Ok(Some(git_dir))
        }
        Err(error) => Err(format!("cannot read git commondir: {error}")),
    }
}

/// Whether a directory entry exists at all, without following a link it may be.
fn entry_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// A non-empty pointer that resolves to an existing directory; anything else leaves
/// the store location unknown, which the reserved-path check treats as reserved.
fn existing_directory(base: &Path, pointer: &str, label: &str) -> Result<PathBuf, String> {
    if pointer.is_empty() {
        return Err(format!("git {label} pointer is empty"));
    }
    let pointer = Path::new(pointer);
    let directory = if pointer.is_absolute() {
        pointer.to_path_buf()
    } else {
        base.join(pointer)
    };
    if !directory.is_dir() {
        return Err(format!(
            "git {label} does not name a directory: {}",
            directory.display()
        ));
    }
    Ok(directory)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    /// A write whose path passes through reserved storage on its way elsewhere is refused even
    /// where `absolute` folds the `..` (Windows), because the ancestor shows it; a sibling output
    /// is not; an empty path names nothing. On Unix a `\` is part of a name: a directory spelled
    /// `target\resource-diagnostics` is not the reserved diagnostics cache.
    #[test]
    fn a_path_through_reserved_storage_is_refused_by_its_ancestors() {
        let directory = crate::temporary::TemporaryDirectory::new("guard-ancestors").unwrap();
        let root = directory.path();
        fs::create_dir_all(root.join("dist/evidence")).unwrap();
        fs::create_dir_all(root.join("dist/release")).unwrap();
        // Built as text, so that no `join` (which folds `..` on a verbatim base) can remove the
        // `..` before the guard sees it.
        let separator = std::path::MAIN_SEPARATOR;
        let through = std::path::PathBuf::from(format!(
            "{}{separator}dist{separator}evidence{separator}..{separator}SHA256SUMS.txt",
            root.display()
        ));
        assert!(
            through
                .components()
                .any(|part| part == std::path::Component::ParentDir)
        );
        for error in [
            super::check_file(root, &through).unwrap_err(),
            super::check_directory(root, &through).unwrap_err(),
        ] {
            assert!(error.contains("coordinator-owned"), "{error}");
        }
        super::check_file(root, &root.join("dist/release/SHA256SUMS.txt")).unwrap();
        #[cfg(unix)]
        {
            fs::create_dir_all(root.join("target\\resource-diagnostics")).unwrap();
            let lookalike = root
                .join("target\\resource-diagnostics")
                .join("..")
                .join("SHA256SUMS.txt");
            super::check_file(root, &lookalike).unwrap();
            fs::create_dir_all(root.join("target/resource-diagnostics")).unwrap();
            let through_cache = root.join("target/resource-diagnostics/probe/../../release");
            let error = super::check_directory(root, &through_cache).unwrap_err();
            assert!(error.contains("coordinator-owned"), "{error}");
        }
        super::check_directory(root, &root.join("dist/release")).unwrap();
        for empty in [
            super::check_file(root, Path::new("")),
            super::check_directory(root, Path::new("")),
        ] {
            assert!(empty.is_err());
        }
        directory.close().unwrap();
    }

    /// Windows: the spelling rules run on the path as given, before `absolute` could map a device
    /// name or read a leading `C:` of a later component; a NUL is refused.
    #[cfg(windows)]
    #[test]
    fn windows_spellings_are_checked_as_given() {
        let cwd = std::env::current_dir().unwrap();
        for refused in [r"\C:report.txt", r"fresh\NUL:", "C:\\out\\a\0b"] {
            assert!(
                super::check_file(&cwd, Path::new(refused)).is_err(),
                "{refused:?}"
            );
        }
        assert!(super::check_directory(&cwd, Path::new("C:\\out\\a\0b")).is_err());
    }

    /// The ancestors checked: relative `..` steps are kept (they resolve against the working
    /// directory, which may lie inside reserved storage); only a bare drive or root is dropped.
    #[test]
    fn relative_parents_are_checked_and_bare_roots_are_not() {
        let listed = |path: &str| -> Vec<String> {
            super::checked_ancestors(Path::new(path))
                .map(|a| a.to_string_lossy().into_owned())
                .collect()
        };
        let separator = std::path::MAIN_SEPARATOR;
        let parents = format!("..{separator}..{separator}SHA256SUMS.txt");
        assert_eq!(
            listed(&parents),
            [parents.clone(), format!("..{separator}.."), "..".to_owned()]
        );
        let rooted = format!("{separator}out{separator}x.txt");
        assert_eq!(listed(&rooted), [rooted.clone(), format!("{separator}out")]);
        #[cfg(windows)]
        assert_eq!(listed(r"C:\out\x.txt"), [r"C:\out\x.txt", r"C:\out"]);
    }
}
