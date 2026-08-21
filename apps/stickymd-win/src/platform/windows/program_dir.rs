//! Canonical portable runtime paths and per-directory instance identity.
//!
//! plan_ref: docs/plan/05_document_persistence.md#program-directory-identity

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use stickymd_core::hash_bytes;
use thiserror::Error;

const FILE_ATTRIBUTE_REPARSE_POINT_VALUE: u32 = 0x400;

#[derive(Debug, Clone)]
pub struct ProgramDirectory {
    real_path: PathBuf,
    identity: String,
}

impl ProgramDirectory {
    pub fn resolve_current() -> Result<Self, RuntimePathsError> {
        let executable = std::env::current_exe().map_err(RuntimePathsError::CurrentExecutable)?;
        Self::from_executable(&executable)
    }

    pub fn from_executable(executable: &Path) -> Result<Self, RuntimePathsError> {
        let parent = executable
            .parent()
            .ok_or(RuntimePathsError::ExecutableHasNoParent)?;
        let real_path = fs::canonicalize(parent).map_err(RuntimePathsError::Canonicalize)?;
        let normalized = normalize_identity_path(&real_path);
        let digest = hash_bytes(&normalized);
        Ok(Self {
            real_path,
            identity: digest.to_hex(),
        })
    }

    pub fn real_path(&self) -> &Path {
        &self.real_path
    }

    #[cfg(test)]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn mutex_name(&self) -> String {
        format!("Local\\StickyMD.Mutex.{}", &self.identity[..32])
    }

    pub fn show_event_name(&self) -> String {
        format!("Local\\StickyMD.Show.{}", &self.identity[..32])
    }
}

fn normalize_identity_path(path: &Path) -> Vec<u8> {
    // Keep the Windows path in its native UTF-16 representation. `to_string_lossy`
    // would collapse distinct legal paths containing unpaired surrogate units.
    // Canonicalization stabilizes the stored filesystem casing; ASCII folding
    // additionally covers drive letters and ordinary case-variant spellings.
    let mut output = Vec::with_capacity(path.as_os_str().len() * 2);
    for unit in path.as_os_str().encode_wide() {
        let unit = if unit == u16::from(b'/') {
            u16::from(b'\\')
        } else if unit <= 0x7f && (unit as u8).is_ascii_uppercase() {
            unit + u16::from(b'a' - b'A')
        } else {
            unit
        };
        output.extend_from_slice(&unit.to_le_bytes());
    }
    output
}

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub program_dir: ProgramDirectory,
    pub note_dir: PathBuf,
    pub note_file: PathBuf,
    pub note_tmp: PathBuf,
    pub config_file: PathBuf,
    pub config_tmp: PathBuf,
    pub images_dir: PathBuf,
    pub trash_dir: PathBuf,
}

impl RuntimePaths {
    pub fn resolve_current() -> Result<Self, RuntimePathsError> {
        Self::from_program_dir(ProgramDirectory::resolve_current()?)
    }

    #[cfg(test)]
    pub fn from_executable(executable: &Path) -> Result<Self, RuntimePathsError> {
        Self::from_program_dir(ProgramDirectory::from_executable(executable)?)
    }

    fn from_program_dir(program_dir: ProgramDirectory) -> Result<Self, RuntimePathsError> {
        let note_dir = program_dir.real_path().join("note");
        Ok(Self {
            note_file: note_dir.join("note.md"),
            note_tmp: note_dir.join("note.md.tmp"),
            config_file: note_dir.join("config.toml"),
            config_tmp: note_dir.join("config.toml.tmp"),
            images_dir: note_dir.join("images"),
            trash_dir: note_dir.join(".trash"),
            note_dir,
            program_dir,
        })
    }

    /// Verify create/write/flush/delete semantics before creating durable files.
    pub fn verify_program_directory_writable(&self) -> Result<(), RuntimePathsError> {
        let probe = self
            .program_dir
            .real_path()
            .join(format!(".stickymd-write-test-{}", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .map_err(RuntimePathsError::WritableProbeCreate)?;
        file.write_all(b"StickyMD writable probe")
            .map_err(RuntimePathsError::WritableProbeWrite)?;
        file.sync_all()
            .map_err(RuntimePathsError::WritableProbeFlush)?;
        drop(file);
        fs::remove_file(&probe).map_err(RuntimePathsError::WritableProbeCleanup)
    }

    pub fn ensure_layout(&self) -> Result<(), RuntimePathsError> {
        ensure_directory(&self.note_dir)?;
        // Frozen v1 layout includes these ownership boundaries even before the
        // asset transaction phase implements their behavior.
        ensure_directory(&self.images_dir)?;
        ensure_directory(&self.trash_dir)?;
        Ok(())
    }
}

fn ensure_directory(path: &Path) -> Result<(), RuntimePathsError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT_VALUE != 0 {
                return Err(RuntimePathsError::RuntimeDirectoryIsReparsePoint(
                    path.to_path_buf(),
                ));
            }
            if !metadata.file_type().is_dir() {
                return Err(RuntimePathsError::PathIsNotDirectory(path.to_path_buf()));
            }
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(RuntimePathsError::InspectDirectory {
                path: path.to_path_buf(),
                source,
            });
        }
    }
    fs::create_dir(path).map_err(|source| RuntimePathsError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })?;
    ensure_directory(path)
}

#[derive(Debug, Error)]
pub enum RuntimePathsError {
    #[error("cannot determine the current executable: {0}")]
    CurrentExecutable(std::io::Error),
    #[error("the executable path has no parent directory")]
    ExecutableHasNoParent,
    #[error("cannot canonicalize the program directory: {0}")]
    Canonicalize(std::io::Error),
    #[error("cannot create the writable probe: {0}")]
    WritableProbeCreate(std::io::Error),
    #[error("cannot write the writable probe: {0}")]
    WritableProbeWrite(std::io::Error),
    #[error("cannot flush the writable probe: {0}")]
    WritableProbeFlush(std::io::Error),
    #[error("cannot remove the writable probe: {0}")]
    WritableProbeCleanup(std::io::Error),
    #[error("runtime path is not a directory: {path}", path = .0.display())]
    PathIsNotDirectory(PathBuf),
    #[error("runtime directory must not be a junction or symbolic link: {}", .0.display())]
    RuntimeDirectoryIsReparsePoint(PathBuf),
    #[error("cannot inspect runtime directory {path}: {source}")]
    InspectDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot create runtime directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("stickymd-{label}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn runtime_paths_are_derived_once_from_real_program_directory() {
        let root = unique_dir("路径 with spaces");
        fs::create_dir_all(&root).unwrap();
        let executable = root.join("StickyMD.exe");
        fs::write(&executable, b"").unwrap();
        let paths = RuntimePaths::from_executable(&executable).unwrap();
        assert_eq!(paths.note_file, paths.note_dir.join("note.md"));
        assert_eq!(paths.note_tmp, paths.note_dir.join("note.md.tmp"));
        assert_eq!(paths.program_dir.identity().len(), 64);
        paths.verify_program_directory_writable().unwrap();
        paths.ensure_layout().unwrap();
        assert!(paths.images_dir.is_dir());
        assert!(paths.trash_dir.is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dot_segments_resolve_to_the_same_instance_identity() {
        let root = unique_dir("identity");
        fs::create_dir_all(&root).unwrap();
        let direct = ProgramDirectory::from_executable(&root.join("StickyMD.exe")).unwrap();
        let dotted = ProgramDirectory::from_executable(&root.join(".\\StickyMD.exe")).unwrap();
        assert_eq!(direct.identity(), dotted.identity());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unicode_and_ascii_case_variants_keep_the_same_identity() {
        let root = unique_dir("数学MixedCase");
        fs::create_dir_all(&root).unwrap();
        let executable = root.join("StickyMD.exe");
        fs::write(&executable, b"").unwrap();
        let canonical = ProgramDirectory::from_executable(&executable).unwrap();

        let name = root.file_name().unwrap().to_string_lossy();
        let variant = root.with_file_name(name.to_ascii_uppercase());
        let variant_executable = variant.join("stickymd.exe");
        let variant = ProgramDirectory::from_executable(&variant_executable).unwrap();
        assert_eq!(canonical.identity(), variant.identity());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn one_hundred_distinct_directories_have_distinct_instance_identities() {
        let root = unique_dir("identity-set");
        fs::create_dir_all(&root).unwrap();
        let mut identities = HashSet::new();
        for index in 0..100 {
            let directory = root.join(format!("portable-note-{index}"));
            fs::create_dir(&directory).unwrap();
            let identity = ProgramDirectory::from_executable(&directory.join("StickyMD.exe"))
                .unwrap()
                .identity()
                .to_owned();
            assert!(identities.insert(identity));
        }
        assert_eq!(identities.len(), 100);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_layout_rejects_a_note_directory_reparse_point() {
        use std::os::windows::fs::symlink_dir;

        let root = unique_dir("reparse-root");
        let external = unique_dir("reparse-target");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(root.join("StickyMD.exe"), b"").unwrap();
        let note = root.join("note");
        if let Err(error) = symlink_dir(&external, &note) {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                fs::remove_dir_all(root).unwrap();
                fs::remove_dir_all(external).unwrap();
                return;
            }
            panic!("cannot create reparse fixture: {error}");
        }
        let paths = RuntimePaths::from_executable(&root.join("StickyMD.exe")).unwrap();
        let result = paths.ensure_layout();
        assert!(
            matches!(
                &result,
                Err(RuntimePathsError::RuntimeDirectoryIsReparsePoint(_))
            ),
            "unexpected layout result: {result:?}"
        );
        fs::remove_dir(&note).unwrap();
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(external).unwrap();
    }

    #[test]
    fn phase9_writable_probe_failure_is_typed_and_preserves_existing_file() {
        let root = unique_dir("probe-blocked");
        fs::create_dir_all(&root).unwrap();
        let executable = root.join("StickyMD.exe");
        fs::write(&executable, b"").unwrap();
        let paths = RuntimePaths::from_executable(&executable).unwrap();
        let blocker = root.join(format!(".stickymd-write-test-{}", std::process::id()));
        fs::write(&blocker, b"external blocker").unwrap();

        assert!(matches!(
            paths.verify_program_directory_writable(),
            Err(RuntimePathsError::WritableProbeCreate(_))
        ));
        assert_eq!(fs::read(&blocker).unwrap(), b"external blocker");
        assert!(!paths.note_dir.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
