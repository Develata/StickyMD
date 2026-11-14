//! The program directory and the note layout inside it.
//!
//! plan_ref: docs/plan/05_document_persistence.md#program-directory-identity

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::platform::macos::file::{full_sync, rename_path_no_replace};

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub program_dir: PathBuf,
    pub note_dir: PathBuf,
    #[cfg(test)]
    pub note_file: PathBuf,
    #[cfg(test)]
    pub note_tmp: PathBuf,
    pub config_file: PathBuf,
    pub config_tmp: PathBuf,
    pub images_dir: PathBuf,
    pub trash_dir: PathBuf,
}

/// The directory that is the note's identity: the one holding the `.app`, or next to the
/// executable when it is not in a bundle.
fn program_dir_of(executable: &Path) -> Option<&Path> {
    let parent = executable.parent()?;
    let bundled = (|| {
        let contents = parent.parent()?;
        let bundle = contents.parent()?;
        let in_bundle = parent.file_name()? == "MacOS"
            && contents.file_name()? == "Contents"
            && bundle.extension()? == "app";
        in_bundle.then(|| bundle.parent()).flatten()
    })();
    Some(bundled.unwrap_or(parent))
}

impl RuntimePaths {
    pub fn resolve_current() -> Result<Self, RuntimePathsError> {
        let executable = std::env::current_exe().map_err(RuntimePathsError::CurrentExecutable)?;
        Self::from_executable(&executable)
    }

    pub fn from_executable(executable: &Path) -> Result<Self, RuntimePathsError> {
        let real = fs::canonicalize(executable).map_err(RuntimePathsError::Canonicalize)?;
        let program_dir = program_dir_of(&real)
            .ok_or(RuntimePathsError::ExecutableHasNoParent)?
            .to_owned();
        let note_dir = program_dir.join("note");
        Ok(Self {
            #[cfg(test)]
            note_file: note_dir.join("note.md"),
            #[cfg(test)]
            note_tmp: note_dir.join("note.md.tmp"),
            config_file: note_dir.join("config.toml"),
            config_tmp: note_dir.join("config.toml.tmp"),
            images_dir: note_dir.join("images"),
            trash_dir: note_dir.join(".trash"),
            note_dir,
            program_dir,
        })
    }

    /// The downloaded-app sandbox path macOS uses for a quarantined bundle. It is only used to
    /// word the message: such a directory fails the writable check on its own.
    pub fn is_translocated(&self) -> bool {
        self.program_dir
            .components()
            .any(|component| component.as_os_str() == "AppTranslocation")
    }

    /// Verifies create, write, flush, delete and non-replacing rename before any durable file.
    pub fn verify_program_directory_writable(&self) -> Result<(), RuntimePathsError> {
        for suffix in 0..128_u8 {
            let name = format!(".stickymd-write-test-{}-{suffix}", std::process::id());
            let probe = self.program_dir.join(&name);
            let renamed = self.program_dir.join(format!("{name}-renamed"));
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&probe) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(RuntimePathsError::WritableProbeCreate(error)),
            };
            let written = file
                .write_all(b"StickyMD writable probe")
                .map_err(RuntimePathsError::WritableProbeWrite)
                .and_then(|()| full_sync(&file).map_err(RuntimePathsError::WritableProbeFlush));
            drop(file);
            if let Err(error) = written {
                let _ = fs::remove_file(&probe);
                return Err(error);
            }
            if let Err(error) = rename_path_no_replace(&probe, &renamed) {
                let _ = fs::remove_file(&probe);
                return Err(if error.raw_os_error() == Some(libc_enotsup()) {
                    RuntimePathsError::VolumeUnsupported(error)
                } else {
                    RuntimePathsError::WritableProbeRename(error)
                });
            }
            return fs::remove_file(&renamed).map_err(RuntimePathsError::WritableProbeCleanup);
        }
        Err(RuntimePathsError::WritableProbeNameExhausted)
    }

    pub fn ensure_layout(&self) -> Result<(), RuntimePathsError> {
        ensure_directory(&self.note_dir)?;
        ensure_directory(&self.images_dir)?;
        ensure_directory(&self.trash_dir)
    }
}

/// `ENOTSUP`, the answer of volumes that cannot rename without replacing.
const fn libc_enotsup() -> i32 {
    45
}

fn ensure_directory(path: &Path) -> Result<(), RuntimePathsError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(RuntimePathsError::NotADirectory(path.to_owned())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(RuntimePathsError::CreateDirectory)
        }
        Err(error) => Err(RuntimePathsError::CreateDirectory(error)),
    }
}

#[derive(Debug, Error)]
pub enum RuntimePathsError {
    #[error("cannot locate the executable: {0}")]
    CurrentExecutable(std::io::Error),
    #[error("cannot resolve the program directory: {0}")]
    Canonicalize(std::io::Error),
    #[error("the executable has no parent directory")]
    ExecutableHasNoParent,
    #[error("cannot create a file: {0}")]
    WritableProbeCreate(std::io::Error),
    #[error("cannot write a file: {0}")]
    WritableProbeWrite(std::io::Error),
    #[error("cannot flush a file: {0}")]
    WritableProbeFlush(std::io::Error),
    #[error("cannot rename a file: {0}")]
    WritableProbeRename(std::io::Error),
    #[error("cannot delete a file: {0}")]
    WritableProbeCleanup(std::io::Error),
    #[error("no free name for the writable check")]
    WritableProbeNameExhausted,
    #[error("this volume cannot rename without replacing: {0}")]
    VolumeUnsupported(std::io::Error),
    #[error("cannot create a directory: {0}")]
    CreateDirectory(std::io::Error),
    #[error("not a plain directory: {}", .0.display())]
    NotADirectory(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;

    fn hdiutil(verb: &str, arguments: &[&std::ffi::OsStr]) {
        let status = std::process::Command::new("hdiutil")
            .arg(verb)
            .arg("-quiet")
            .args(arguments)
            .status()
            .unwrap();
        assert!(status.success(), "hdiutil {verb} {arguments:?}");
    }

    /// P16-S06. Run with `--ignored`.
    #[test]
    #[ignore = "creates and mounts an exFAT disk image"]
    fn an_exfat_volume_is_refused_and_keeps_no_probe_file() {
        let root = unique_temp_path("paths-exfat");
        fs::create_dir(&root).unwrap();
        let image = root.join("volume.dmg");
        let mount = root.join("mount");
        fs::create_dir(&mount).unwrap();
        hdiutil(
            "create",
            &[
                "-size".as_ref(),
                "128m".as_ref(),
                "-fs".as_ref(),
                "ExFAT".as_ref(),
                "-volname".as_ref(),
                "SMDPROBE".as_ref(),
                image.as_os_str(),
            ],
        );
        hdiutil(
            "attach",
            &[
                image.as_os_str(),
                "-nobrowse".as_ref(),
                "-mountpoint".as_ref(),
                mount.as_os_str(),
            ],
        );
        fs::write(mount.join("stickymd"), b"").unwrap();
        let result = RuntimePaths::from_executable(&mount.join("stickymd"))
            .unwrap()
            .verify_program_directory_writable();
        let left_behind = fs::read_dir(&mount)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".stickymd-write-test")
            })
            .count();
        hdiutil("detach", &[mount.as_os_str()]);
        fs::remove_dir_all(&root).unwrap();
        assert!(
            matches!(result, Err(RuntimePathsError::VolumeUnsupported(_))),
            "{result:?}"
        );
        assert_eq!(left_behind, 0);
    }

    #[test]
    fn a_bundle_uses_the_directory_holding_the_app() {
        assert_eq!(
            program_dir_of(Path::new("/x/y/StickyMD.app/Contents/MacOS/stickymd")),
            Some(Path::new("/x/y"))
        );
    }

    #[test]
    fn a_bare_executable_uses_its_own_directory() {
        assert_eq!(
            program_dir_of(Path::new("/x/target/debug/stickymd")),
            Some(Path::new("/x/target/debug"))
        );
        // Only the whole shape is a bundle; a folder that merely looks like part of one is not.
        assert_eq!(
            program_dir_of(Path::new("/x/Contents/MacOS/stickymd")),
            Some(Path::new("/x/Contents/MacOS"))
        );
    }

    #[test]
    fn the_layout_is_created_once_and_a_file_in_its_place_is_refused() {
        let root = unique_temp_path("paths-layout");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("stickymd"), b"").unwrap();
        let paths = RuntimePaths::from_executable(&root.join("stickymd")).unwrap();
        paths.verify_program_directory_writable().unwrap();
        paths.ensure_layout().unwrap();
        paths.ensure_layout().unwrap();
        assert!(paths.trash_dir.is_dir());
        fs::remove_dir(&paths.trash_dir).unwrap();
        fs::write(&paths.trash_dir, b"").unwrap();
        assert!(matches!(
            paths.ensure_layout(),
            Err(RuntimePathsError::NotADirectory(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_translocated_path_is_recognised() {
        let paths = RuntimePaths {
            program_dir: "/private/var/folders/x/AppTranslocation/ABC/d".into(),
            note_dir: PathBuf::new(),
            note_file: PathBuf::new(),
            note_tmp: PathBuf::new(),
            config_file: PathBuf::new(),
            config_tmp: PathBuf::new(),
            images_dir: PathBuf::new(),
            trash_dir: PathBuf::new(),
        };
        assert!(paths.is_translocated());
    }
}
