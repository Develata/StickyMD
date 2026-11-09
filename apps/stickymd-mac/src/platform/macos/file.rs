//! The file operations POSIX offers only through a descriptor or a flag std does not expose.
//!
//! plan_ref: docs/plan/05_document_persistence.md#macos-storage

use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

fn c_string(value: &OsStr) -> io::Result<CString> {
    CString::new(value.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL byte"))
}

fn check(result: libc::c_int) -> io::Result<()> {
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn file_from(descriptor: libc::c_int) -> io::Result<File> {
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `descriptor` was just returned by a successful open and nothing else owns it.
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

fn regular(file: File) -> io::Result<File> {
    if file.metadata()?.file_type().is_file() {
        Ok(file)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ))
    }
}

/// Asks the drive itself to reach stable storage; `fsync` stops at the drive's cache.
pub fn full_sync(file: &File) -> io::Result<()> {
    // SAFETY: the descriptor is valid for the lifetime of `file`; F_FULLFSYNC takes no argument.
    check(unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) }.min(0))
}

/// Moves `from` to `to` only if nothing is at `to`.
pub fn rename_path_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    let from = c_string(from.as_os_str())?;
    let to = c_string(to.as_os_str())?;
    // SAFETY: both pointers are NUL-terminated strings that outlive the call.
    check(unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) })
}

/// Opens an image the note refers to: read-only, regular file, no symbolic link anywhere in the
/// path, and on this machine rather than a network volume.
pub fn open_local_image(path: &Path) -> io::Result<File> {
    // No libc constant yet; from <sys/fcntl.h>.
    const O_NOFOLLOW_ANY: libc::c_int = 0x2000_0000;
    let c_path = c_string(path.as_os_str())?;
    // SAFETY: `c_path` is NUL-terminated and outlives the call.
    let descriptor = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | O_NOFOLLOW_ANY | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    let file = regular(file_from(descriptor)?)?;
    // SAFETY: a zeroed statfs is a valid value for fstatfs to overwrite.
    let mut volume: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: the descriptor is valid and `volume` is writable.
    check(unsafe { libc::fstatfs(file.as_raw_fd(), &mut volume) })?;
    if volume.f_flags & libc::MNT_LOCAL as u32 == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "images on network volumes are not read",
        ));
    }
    Ok(file)
}

/// What identifies one version of one file. Renaming keeps it; replacing or writing changes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileObservation {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl FileObservation {
    pub fn of(file: &File) -> io::Result<Self> {
        Ok(Self::from_metadata(&file.metadata()?))
    }

    /// What is at `path` itself: a symbolic link there is observed as the link, not its target.
    pub fn of_path(path: &Path) -> io::Result<Self> {
        Ok(Self::from_metadata(&std::fs::symlink_metadata(path)?))
    }

    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn same_file_as(&self, other: &Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
}

/// The Note Directory, as one descriptor every storage port goes through, so that the note, its
/// images and the configuration cannot end up in different directories.
#[derive(Clone, Debug)]
pub struct SharedDirectory(Arc<Mutex<DirectoryHandle>>);

impl SharedDirectory {
    pub fn open(path: &Path) -> io::Result<Self> {
        Ok(Self::new(DirectoryHandle::open(path)?))
    }

    pub fn new(directory: DirectoryHandle) -> Self {
        Self(Arc::new(Mutex::new(directory)))
    }

    pub fn with<R>(&self, use_directory: impl FnOnce(&DirectoryHandle) -> R) -> R {
        use_directory(&self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// The directory was deleted and made again: everyone moves to the new one together.
    pub fn replace(&self, directory: DirectoryHandle) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = directory;
    }
}

/// A directory held open, so that every operation names it by descriptor: when the directory is
/// moved, the operations follow it instead of a path that now means something else.
#[derive(Debug)]
pub struct DirectoryHandle {
    directory: File,
}

impl DirectoryHandle {
    /// Refuses a symbolic link at `path`.
    pub fn open(path: &Path) -> io::Result<Self> {
        let path = c_string(path.as_os_str())?;
        // SAFETY: `path` is NUL-terminated and outlives the call.
        let descriptor = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        Ok(Self {
            directory: file_from(descriptor)?,
        })
    }

    /// Opens a directory inside this one; a symbolic link at `name` is refused.
    pub fn open_child(&self, name: &str) -> io::Result<Self> {
        let name = c_string(OsStr::new(name))?;
        // SAFETY: the descriptor is valid while `self` lives; `name` outlives the call.
        let descriptor = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        Ok(Self {
            directory: file_from(descriptor)?,
        })
    }

    /// Where the directory is now, after any move.
    pub fn current_path(&self) -> io::Result<std::path::PathBuf> {
        let mut path = [0_u8; libc::PATH_MAX as usize];
        // SAFETY: F_GETPATH writes a NUL-terminated path of at most PATH_MAX bytes to the buffer,
        // which outlives the call; the descriptor is valid while `self` lives.
        check(unsafe {
            libc::fcntl(
                self.directory.as_raw_fd(),
                libc::F_GETPATH,
                path.as_mut_ptr(),
            )
        })?;
        let length = path
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(path.len());
        Ok(OsStr::from_bytes(&path[..length]).into())
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            directory: self.directory.try_clone()?,
        })
    }

    fn open_at(&self, name: &str, flags: libc::c_int) -> io::Result<File> {
        let name = c_string(OsStr::new(name))?;
        // SAFETY: the descriptor is valid while `self` lives; `name` outlives the call; the mode
        // is only read when `flags` includes O_CREAT.
        let descriptor = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                0o644 as libc::c_uint,
            )
        };
        regular(file_from(descriptor)?)
    }

    /// Opens a regular file for reading; a link, directory or FIFO at `name` is refused.
    pub fn open_read(&self, name: &str) -> io::Result<File> {
        self.open_at(name, libc::O_RDONLY)
    }

    /// `O_TRUNC` would empty every other name of the file, the note itself if someone linked it
    /// here, so the check comes before anything is lost.
    pub fn create_or_truncate(&self, name: &str) -> io::Result<File> {
        let file = self.open_at(name, libc::O_WRONLY | libc::O_CREAT)?;
        if file.metadata()?.nlink() != 1 {
            return Err(io::Error::other("the file has another name"));
        }
        file.set_len(0)?;
        Ok(file)
    }

    /// Fails with `AlreadyExists` if anything is at `name`.
    pub fn create_new(&self, name: &str) -> io::Result<File> {
        self.open_at(name, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)
    }

    /// Atomically replaces whatever is at `to`, a link included, without following it.
    pub fn replace(&self, from: &str, to: &str) -> io::Result<()> {
        let from = c_string(OsStr::new(from))?;
        let to = c_string(OsStr::new(to))?;
        let directory = self.directory.as_raw_fd();
        // SAFETY: the descriptor is valid while `self` lives; both names outlive the call.
        check(unsafe { libc::renameat(directory, from.as_ptr(), directory, to.as_ptr()) })
    }

    /// Moves `from` to `to` in `target` only if nothing is at `to`: `AlreadyExists` otherwise.
    pub fn rename_no_replace_into(&self, from: &str, target: &Self, to: &str) -> io::Result<()> {
        let from = c_string(OsStr::new(from))?;
        let to = c_string(OsStr::new(to))?;
        // SAFETY: both descriptors are valid while their handles live; both names outlive the call.
        check(unsafe {
            libc::renameatx_np(
                self.directory.as_raw_fd(),
                from.as_ptr(),
                target.directory.as_raw_fd(),
                to.as_ptr(),
                libc::RENAME_EXCL,
            )
        })
    }

    pub fn rename_no_replace(&self, from: &str, to: &str) -> io::Result<()> {
        self.rename_no_replace_into(from, self, to)
    }

    pub fn remove_file(&self, name: &str) -> io::Result<()> {
        let name = c_string(OsStr::new(name))?;
        // SAFETY: the descriptor is valid while `self` lives; `name` outlives the call.
        check(unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) })
    }

    /// Makes the directory's own entries durable, which is what a rename changes.
    pub fn sync(&self) -> io::Result<()> {
        full_sync(&self.directory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use std::io::Write;

    fn directory(label: &str) -> (std::path::PathBuf, DirectoryHandle) {
        let root = unique_temp_path(label);
        fs::create_dir(&root).unwrap();
        let handle = DirectoryHandle::open(&root).unwrap();
        (root, handle)
    }

    #[test]
    fn a_link_at_the_directory_or_the_file_is_refused() {
        let (root, handle) = directory("file-link");
        fs::write(root.join("real"), b"x").unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join("link")).unwrap();
        assert!(handle.open_read("real").is_ok());
        assert!(handle.open_read("link").is_err());
        let linked_directory = root.with_extension("link");
        std::os::unix::fs::symlink(&root, &linked_directory).unwrap();
        assert!(DirectoryHandle::open(&linked_directory).is_err());
        fs::remove_file(linked_directory).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_local_image_is_opened_only_along_a_path_without_links() {
        let (root, _handle) = directory("file-image");
        let real_root = fs::canonicalize(&root).unwrap();
        fs::write(real_root.join("a.png"), b"x").unwrap();
        std::os::unix::fs::symlink(real_root.join("a.png"), real_root.join("link.png")).unwrap();
        fs::create_dir(real_root.join("real")).unwrap();
        fs::write(real_root.join("real/b.png"), b"x").unwrap();
        std::os::unix::fs::symlink(real_root.join("real"), real_root.join("linked")).unwrap();
        assert!(open_local_image(&real_root.join("a.png")).is_ok());
        assert!(open_local_image(&real_root.join("link.png")).is_err());
        assert!(open_local_image(&real_root.join("linked/b.png")).is_err());
        assert!(open_local_image(&real_root.join("real")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_rename_without_replace_refuses_an_occupied_name() {
        let (root, handle) = directory("file-excl");
        fs::write(root.join("a"), b"A").unwrap();
        fs::write(root.join("b"), b"B").unwrap();
        let error = handle.rename_no_replace("a", "b").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(root.join("b")).unwrap(), b"B");
        handle.rename_no_replace("a", "c").unwrap();
        assert_eq!(fs::read(root.join("c")).unwrap(), b"A");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replacing_a_link_leaves_what_it_pointed_at() {
        let (root, handle) = directory("file-replace-link");
        fs::write(root.join("victim"), b"keep").unwrap();
        std::os::unix::fs::symlink(root.join("victim"), root.join("target")).unwrap();
        fs::write(root.join("new"), b"new").unwrap();
        handle.replace("new", "target").unwrap();
        assert_eq!(fs::read(root.join("victim")).unwrap(), b"keep");
        assert!(
            !fs::symlink_metadata(root.join("target"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(root).unwrap();
    }

    /// A hard link at the temporary's name is the note itself: emptying it would lose the text.
    #[test]
    fn a_temporary_with_another_name_is_refused_and_keeps_its_bytes() {
        let (root, handle) = directory("file-hard-link");
        fs::write(root.join("note.md"), b"note").unwrap();
        fs::hard_link(root.join("note.md"), root.join("note.md.tmp")).unwrap();
        handle.create_or_truncate("note.md.tmp").unwrap_err();
        assert_eq!(fs::read(root.join("note.md")).unwrap(), b"note");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_stale_temporary_is_emptied() {
        let (root, handle) = directory("file-stale-temporary");
        fs::write(root.join("note.md.tmp"), b"old").unwrap();
        let file = handle.create_or_truncate("note.md.tmp").unwrap();
        assert_eq!(file.metadata().unwrap().len(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn operations_follow_a_moved_directory() {
        let (root, handle) = directory("file-moved");
        let moved = root.with_extension("moved");
        fs::rename(&root, &moved).unwrap();
        let mut file = handle.create_new("inside").unwrap();
        file.write_all(b"x").unwrap();
        assert!(moved.join("inside").is_file());
        fs::remove_dir_all(moved).unwrap();
    }

    #[test]
    fn a_deleted_directory_takes_no_new_entries() {
        let (root, handle) = directory("file-removed");
        fs::remove_dir(&root).unwrap();
        let error = handle.create_or_truncate("a").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn an_observation_survives_a_rename_and_not_a_replacement() {
        let (root, handle) = directory("file-observe");
        fs::write(root.join("a"), b"one").unwrap();
        let before = FileObservation::of(&handle.open_read("a").unwrap()).unwrap();
        fs::rename(root.join("a"), root.join("b")).unwrap();
        let renamed = FileObservation::of(&handle.open_read("b").unwrap()).unwrap();
        assert!(before.same_file_as(&renamed));
        fs::write(root.join("c"), b"two").unwrap();
        handle.replace("c", "b").unwrap();
        let replaced = FileObservation::of(&handle.open_read("b").unwrap()).unwrap();
        assert!(!before.same_file_as(&replaced));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn syncing_a_file_and_a_directory_succeeds_on_this_volume() {
        let (root, handle) = directory("file-sync");
        let file = handle.create_new("a").unwrap();
        full_sync(&file).unwrap();
        handle.sync().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
