//! The note layout inside the Note Root, the writable check on it, and the paths the program
//! only shows or watches. Every write goes through the handles: the root's from `note_root`, and
//! the `note/` handle [`open_note_directory`] returns.
//!
//! plan_ref: docs/plan/05_document_persistence.md#startup-sequence
//! plan_ref: docs/plan/05_document_persistence.md#note-root

use std::io::Write;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::note_root::{Mode, NoteRoot};
use crate::platform::macos::child_directory::is_not_a_plain_directory;
use crate::platform::macos::file::{DirectoryHandle, full_sync};

pub const NOTE_DIRECTORY: &str = "note";
const IMAGES_DIRECTORY: &str = "images";
const TRASH_DIRECTORY: &str = ".trash";
/// `ENOTSUP`, the answer of volumes that cannot rename without replacing.
const ENOTSUP: i32 = 45;

/// Where the note's directories were when the program started. Links in the note and the
/// watcher use these paths; nothing is written through them.
#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub note_dir: PathBuf,
    #[cfg(test)]
    pub note_file: PathBuf,
    #[cfg(test)]
    pub note_tmp: PathBuf,
    pub images_dir: PathBuf,
    pub trash_dir: PathBuf,
}

impl RuntimePaths {
    pub fn of(note_root: &Path) -> Self {
        let note_dir = note_root.join(NOTE_DIRECTORY);
        Self {
            #[cfg(test)]
            note_file: note_dir.join("note.md"),
            #[cfg(test)]
            note_tmp: note_dir.join("note.md.tmp"),
            images_dir: note_dir.join(IMAGES_DIRECTORY),
            trash_dir: note_dir.join(TRASH_DIRECTORY),
            note_dir,
        }
    }
}

/// Verifies create, write, flush, delete and non-replacing rename in the Note Root before any
/// durable file.
pub fn verify_writable(root: &DirectoryHandle) -> Result<(), RuntimePathsError> {
    for suffix in 0..128_u8 {
        let name = format!(".stickymd-write-test-{}-{suffix}", std::process::id());
        let renamed = format!("{name}-renamed");
        let mut file = match root.create_new(&name) {
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
            let _ = root.remove_file(&name);
            return Err(error);
        }
        if let Err(error) = root.rename_no_replace(&name, &renamed) {
            let _ = root.remove_file(&name);
            // A target left by an earlier run with this process ID is not ours to remove: take
            // the next pair of names.
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                continue;
            }
            return Err(if error.raw_os_error() == Some(ENOTSUP) {
                RuntimePathsError::VolumeUnsupported(error)
            } else {
                RuntimePathsError::WritableProbeRename(error)
            });
        }
        return root
            .remove_file(&renamed)
            .map_err(RuntimePathsError::WritableProbeCleanup);
    }
    Err(RuntimePathsError::WritableProbeNameExhausted)
}

/// Creates `note/`, `note/images/` and `note/.trash/` where they are missing, relative to the
/// locked Note Root, and returns the one `note/` handle every later step derives from.
pub fn open_note_directory(root: &DirectoryHandle) -> Result<DirectoryHandle, RuntimePathsError> {
    let note = layout_child(root, NOTE_DIRECTORY)?;
    layout_child(&note, IMAGES_DIRECTORY)?;
    layout_child(&note, TRASH_DIRECTORY)?;
    Ok(note)
}

fn layout_child(
    parent: &DirectoryHandle,
    name: &str,
) -> Result<DirectoryHandle, RuntimePathsError> {
    parent.open_or_create_child(name, false).map_err(|error| {
        if is_not_a_plain_directory(&error) {
            RuntimePathsError::NotADirectory(name.to_owned())
        } else {
            RuntimePathsError::CreateDirectory(error)
        }
    })
}

/// The startup message for a Note Root that failed the writable check (plan 05 startup order):
/// every message about writing starts with 当前目录不可写.
pub fn unwritable_advice(root: &NoteRoot, error: &RuntimePathsError) -> String {
    let unsupported = matches!(error, RuntimePathsError::VolumeUnsupported(_));
    match (&root.mode, unsupported) {
        (Mode::Installed { .. }, true) => {
            format!("便签目录所在的卷不支持安全保存：{}", root.path.display())
        }
        (_, true) => "当前卷不支持安全保存，请将程序移到 APFS 卷。".to_owned(),
        (Mode::Installed { .. }, false) => {
            format!("当前目录不可写：{}", root.path.display())
        }
        (Mode::Portable, false) => "当前目录不可写，请先把 StickyMD.app 拖进“应用程序”，\
             或把整个 StickyMD 文件夹移到别处。"
            .to_owned(),
        (Mode::Development, false) => "当前目录不可写，请将程序移动到有写权限的文件夹。".to_owned(),
    }
}

#[derive(Debug, Error)]
pub enum RuntimePathsError {
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
    #[error("not a plain directory: {0}")]
    NotADirectory(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note_root::Roots;
    use crate::test_support::unique_temp_path;
    use std::fs;

    fn hdiutil(verb: &str, arguments: &[&std::ffi::OsStr]) {
        let status = std::process::Command::new("hdiutil")
            .arg(verb)
            .arg("-quiet")
            .args(arguments)
            .status()
            .unwrap();
        assert!(status.success(), "hdiutil {verb} {arguments:?}");
    }

    fn development_root(directory: &Path) -> NoteRoot {
        fs::write(directory.join("stickymd"), b"").unwrap();
        let roots = Roots {
            system_applications: directory.join("no-applications"),
            home: Err("tests have no home".into()),
        };
        NoteRoot::resolve(&directory.join("stickymd"), &roots).unwrap()
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
        let note_root = development_root(&mount);
        let result = verify_writable(&note_root.handle);
        let advice = result
            .as_ref()
            .err()
            .map(|error| unwritable_advice(&note_root, error));
        drop(note_root);
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
        assert_eq!(
            advice.as_deref(),
            Some("当前卷不支持安全保存，请将程序移到 APFS 卷。")
        );
        assert_eq!(left_behind, 0);
    }

    #[test]
    fn the_layout_is_created_once_through_the_root_and_a_file_in_its_place_is_refused() {
        let root = unique_temp_path("paths-layout");
        fs::create_dir(&root).unwrap();
        let note_root = development_root(&root);
        verify_writable(&note_root.handle).unwrap();
        let paths = RuntimePaths::of(&note_root.path);
        let first = open_note_directory(&note_root.handle).unwrap();
        let second = open_note_directory(&note_root.handle).unwrap();
        assert!(
            first
                .observation()
                .unwrap()
                .same_file_as(&second.observation().unwrap())
        );
        assert!(paths.images_dir.is_dir() && paths.trash_dir.is_dir());
        // The executable and note/: the writable check left nothing behind.
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
        fs::remove_dir(&paths.trash_dir).unwrap();
        fs::write(&paths.trash_dir, b"").unwrap();
        assert!(matches!(
            open_note_directory(&note_root.handle),
            Err(RuntimePathsError::NotADirectory(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    /// The layout follows the root's handle, not its old path.
    #[test]
    fn the_layout_is_made_in_the_directory_the_handle_names() {
        let root = unique_temp_path("paths-layout-moved");
        fs::create_dir(&root).unwrap();
        let note_root = development_root(&root);
        let moved = unique_temp_path("paths-layout-moved-to");
        fs::rename(&root, &moved).unwrap();
        fs::create_dir(&root).unwrap();
        open_note_directory(&note_root.handle).unwrap();
        assert!(moved.join("note/.trash").is_dir());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(moved).unwrap();
    }

    /// P18-L11: every unwritable message starts with the prefix the R03 judge looks for.
    #[test]
    fn a_read_only_root_is_refused_with_advice_for_its_mode() {
        use std::os::unix::fs::PermissionsExt;
        let root = unique_temp_path("paths-read-only");
        fs::create_dir(&root).unwrap();
        let mut note_root = development_root(&root);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
        let error = verify_writable(&note_root.handle).unwrap_err();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            matches!(error, RuntimePathsError::WritableProbeCreate(_)),
            "{error:?}"
        );
        assert_eq!(
            unwritable_advice(&note_root, &error),
            "当前目录不可写，请将程序移动到有写权限的文件夹。"
        );
        note_root.mode = Mode::Portable;
        assert_eq!(
            unwritable_advice(&note_root, &error),
            "当前目录不可写，请先把 StickyMD.app 拖进\u{201c}应用程序\u{201d}，或把整个 StickyMD 文件夹移到别处。"
        );
        note_root.mode = Mode::Installed {
            key: "StickyMD".into(),
        };
        let installed = unwritable_advice(&note_root, &error);
        assert!(installed.starts_with("当前目录不可写："), "{installed}");
        assert!(
            installed.ends_with(&root.display().to_string()),
            "{installed}"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
