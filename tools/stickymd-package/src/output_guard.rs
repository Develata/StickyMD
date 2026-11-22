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

    /// Refuses `path` (resolved against the working directory, as the writers resolve it) as a
    /// file a release command writes: the writer's spelling rules first, so no parent is created
    /// for a path the writer would refuse, then the reserved storage.
    pub fn check_file(&self, path: &Path) -> Result<(), String> {
        let path = resolve(path)?;
        crate::atomic::check_spelling(&path)?;
        #[cfg(windows)]
        crate::path_identity::validate_output_spelling(&path)?;
        self.check(&path)
    }

    /// Refuses `directory` as a directory a release command writes into.
    pub fn check_directory(&self, directory: &Path) -> Result<(), String> {
        self.check(&resolve(directory)?)
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

/// `path` against the working directory, as the writers resolve it. Elsewhere than Windows,
/// `std::path::absolute` keeps `..` and every name as written.
#[cfg(not(windows))]
fn resolve(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path)
        .map_err(|error| format!("cannot resolve output {}: {error}", path.display()))
}

/// `path` against the working directory, as the writers resolve it, and kept as written: no `..`
/// collapsed and no device spelling mapped, so the spelling rules see the name the writer will be
/// asked to create and [`reaches`] sees a reserved directory the path passes through.
/// `std::path::absolute` cannot serve whole: on Windows it goes through `GetFullPathNameW`, which
/// turns `dist/evidence\..\x` into `dist\x` and `NUL:` into `\\.\NUL`.
#[cfg(windows)]
fn resolve(path: &Path) -> Result<PathBuf, String> {
    resolve_windows(path, &std::env::current_dir, &|head: &Path| {
        std::path::absolute(head)
    })
}

/// Only the head that Win32 gives a base to is resolved, and the rest is appended as written:
/// - an absolute path is kept as given;
/// - a drive-relative path (`C:x`) takes that drive's working directory (`absolute("C:")`);
/// - a root-relative path (`\x`) takes the current drive's root (`absolute("\")`);
/// - any other relative path takes the working directory, which must not be verbatim (`\\?\`):
///   joining onto a verbatim base folds `..` too.
///
/// An empty path, or one holding a NUL, names nothing and is refused, as `absolute` refuses it.
#[cfg(windows)]
fn resolve_windows(
    path: &Path,
    current_dir: &dyn Fn() -> std::io::Result<PathBuf>,
    absolute: &dyn Fn(&Path) -> std::io::Result<PathBuf>,
) -> Result<PathBuf, String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Component, Prefix};
    let fail = |why: String| format!("cannot resolve output {}: {why}", path.display());
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.is_empty() || wide.contains(&0) {
        return Err(fail("the path is empty or holds a NUL".to_owned()));
    }
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let (head, base) = match path.components().next() {
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)) => {
            let head = prefix.as_os_str();
            (head.encode_wide().count(), absolute(Path::new(head)))
        }
        Some(Component::Prefix(_)) => {
            return Err(fail("an unsupported relative prefix".to_owned()));
        }
        Some(Component::RootDir) => (1, absolute(Path::new("\\"))),
        _ => {
            let directory = current_dir().map_err(|error| fail(error.to_string()))?;
            if matches!(
                directory.components().next(),
                Some(Component::Prefix(prefix)) if prefix.kind().is_verbatim()
            ) {
                return Err(fail(
                    "the working directory is a verbatim path, where joining would fold `..`; give an absolute output path"
                        .to_owned(),
                ));
            }
            return Ok(directory.join(path));
        }
    };
    let base = base.map_err(|error| fail(error.to_string()))?;
    let rest = OsString::from_wide(&wide[head..]);
    Ok(base.join(rest))
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
    use std::path::Path;

    /// Relative outputs join the working directory with their `..` kept, so a reserved
    /// directory the path passes through is still seen; an absolute path is kept as given; an
    /// empty path names nothing. The expected text is built apart from the code under test.
    #[test]
    fn outputs_resolve_against_the_working_directory_keeping_parents() {
        let cwd = std::env::current_dir().unwrap();
        let separator = std::path::MAIN_SEPARATOR;
        let relative = format!("dist{separator}evidence{separator}..{separator}x.txt");
        let resolved = super::resolve(Path::new(&relative)).unwrap();
        assert_eq!(
            resolved.as_os_str(),
            format!("{}{separator}{relative}", cwd.display()).as_str()
        );
        assert!(
            resolved
                .components()
                .any(|part| part == std::path::Component::ParentDir)
        );
        let absolute = cwd.join("a").join("..").join("b");
        assert_eq!(super::resolve(&absolute).unwrap(), absolute);
        assert!(super::resolve(Path::new("")).is_err());
        assert!(super::check_file(&cwd, Path::new("")).is_err());
    }

    /// Windows: only the head Win32 gives a base to is resolved; the rest, `..`, `/` and device
    /// spellings included, is kept as written. Expected text is built from `absolute` of the head
    /// alone, apart from the code under test.
    #[cfg(windows)]
    #[test]
    fn windows_paths_keep_their_spelling_after_the_head() {
        use std::path::PathBuf;
        let text = |path: PathBuf| path.into_os_string().into_string().unwrap();
        for kept in [r"C:\a\evidence\..\x.txt", r"C:\a\fresh\NUL:"] {
            assert_eq!(text(super::resolve(Path::new(kept)).unwrap()), kept);
        }
        let cwd = text(std::env::current_dir().unwrap());
        assert_eq!(
            text(super::resolve(Path::new(r"dist/evidence\..\x.txt")).unwrap()),
            format!(r"{cwd}\dist/evidence\..\x.txt")
        );
        let root = text(std::path::absolute(Path::new("\\")).unwrap());
        assert_eq!(
            text(super::resolve(Path::new(r"\out\..\SHA256SUMS.txt")).unwrap()),
            format!(r"{}\out\..\SHA256SUMS.txt", root.trim_end_matches('\\'))
        );
        let drive = &cwd[..2];
        let drive_cwd = text(std::path::absolute(Path::new(drive)).unwrap());
        assert_eq!(
            text(super::resolve(Path::new(&format!(r"{drive}out\..\NUL:"))).unwrap()),
            format!(r"{}\out\..\NUL:", drive_cwd.trim_end_matches('\\'))
        );
        for refused in ["", "a\0b"] {
            assert!(super::resolve(Path::new(refused)).is_err(), "{refused:?}");
        }
        let verbatim = super::resolve_windows(
            Path::new(r"dist\evidence\..\x.txt"),
            &|| Ok(PathBuf::from(r"\\?\C:\repo")),
            &|head: &Path| std::path::absolute(head),
        )
        .unwrap_err();
        assert!(verbatim.contains("verbatim"), "{verbatim}");
    }
}
