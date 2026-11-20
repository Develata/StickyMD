//! Opening and creating directories one level at a time by descriptor, for the Note Root chain and
//! the note layout: each level is opened relative to the one before, so a link swapped in along
//! the way is refused rather than followed.
//!
//! plan_ref: docs/plan/05_document_persistence.md#note-root

use std::ffi::OsStr;
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;

use crate::file::{DirectoryHandle, c_string, file_from};
use crate::sys::check;

impl DirectoryHandle {
    /// Opens the directory at `path`, following a symbolic link there: the start of a chain,
    /// which the system rather than the program owns. Fails with `NotADirectory` for anything
    /// else.
    pub fn open_following(path: &Path) -> io::Result<Self> {
        let path = c_string(path.as_os_str())?;
        // SAFETY: `path` is NUL-terminated and outlives the call.
        let descriptor = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        Ok(Self {
            directory: file_from(descriptor)?,
        })
    }

    /// The directory `name` inside this one, created when nothing is there: with `0700` when
    /// `private`, else with `0777` as `fs::create_dir` does; the umask narrows either. A symbolic link or any other non-directory at `name` is refused (see
    /// [`is_not_a_plain_directory`]) and left as it is. A directory another process creates
    /// between the two steps is opened like one that was already there.
    pub fn open_or_create_child(&self, name: &str, private: bool) -> io::Result<Self> {
        let mode: libc::mode_t = if private { 0o700 } else { 0o777 };
        match self.open_child(name) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            opened => return opened,
        }
        let c_name = c_string(OsStr::new(name))?;
        // SAFETY: the descriptor is valid while `self` lives; `c_name` outlives the call.
        match check(unsafe { libc::mkdirat(self.directory.as_raw_fd(), c_name.as_ptr(), mode) }) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        self.open_child(name)
    }

    /// The directory's own metadata, wherever it is now.
    pub fn metadata(&self) -> io::Result<std::fs::Metadata> {
        self.directory.metadata()
    }

    /// `(st_dev, st_ino)` of what `name` inside this directory names after following links
    /// (`fstatat` without `AT_SYMLINK_NOFOLLOW`): the entry is looked up in this directory, not
    /// in whatever a path to it names by now.
    pub fn child_identity_following(&self, name: &str) -> io::Result<(u64, u64)> {
        let c_name = c_string(OsStr::new(name))?;
        // SAFETY: a zeroed stat is a valid value for fstatat to overwrite.
        let mut status: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: the descriptor is valid while `self` lives; `c_name` and `status` outlive the
        // call, and `status` is writable.
        check(unsafe {
            libc::fstatat(self.directory.as_raw_fd(), c_name.as_ptr(), &mut status, 0)
        })?;
        // The same conversion `MetadataExt::dev` makes, so the two identities compare.
        Ok((status.st_dev as u64, status.st_ino))
    }
}

/// What opening a child directory answers when the name holds a symbolic link or anything else
/// that is not a directory: macOS reports a link under `O_NOFOLLOW | O_DIRECTORY` as `ENOTDIR`,
/// other systems as `ELOOP`.
pub fn is_not_a_plain_directory(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotADirectory || error.raw_os_error() == Some(libc::ELOOP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    fn root(label: &str) -> (std::path::PathBuf, DirectoryHandle) {
        let path = unique_temp_path(label);
        fs::create_dir(&path).unwrap();
        let handle = DirectoryHandle::open(&path).unwrap();
        (path, handle)
    }

    #[test]
    fn a_missing_child_is_created_with_the_mode_and_an_existing_one_is_opened() {
        let (path, root) = root("child-create");
        let created = root.open_or_create_child("a", true).unwrap();
        let mode = fs::metadata(path.join("a")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        fs::write(path.join("a/marker"), b"").unwrap();
        let again = root.open_or_create_child("a", true).unwrap();
        assert!(
            again
                .observation()
                .unwrap()
                .same_file_as(&created.observation().unwrap())
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn a_link_or_a_file_at_the_name_is_refused_and_left_alone() {
        let (path, root) = root("child-refuse");
        let target = unique_temp_path("child-refuse-target");
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, path.join("link")).unwrap();
        let link = root.open_or_create_child("link", true).unwrap_err();
        assert!(is_not_a_plain_directory(&link), "{link:?}");
        fs::write(path.join("file"), b"x").unwrap();
        let file = root.open_or_create_child("file", true).unwrap_err();
        assert!(is_not_a_plain_directory(&file), "{file:?}");
        assert!(
            fs::symlink_metadata(path.join("link"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(fs::read(path.join("file")).unwrap(), b"x");
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        fs::remove_dir_all(path).unwrap();
        fs::remove_dir(target).unwrap();
    }

    #[test]
    fn the_start_of_a_chain_follows_a_link_and_refuses_a_file() {
        let (path, _root) = root("chain-start");
        let target = path.join("target");
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, path.join("link")).unwrap();
        let opened = DirectoryHandle::open_following(&path.join("link")).unwrap();
        assert_eq!(
            opened.metadata().unwrap().ino(),
            fs::metadata(&target).unwrap().ino()
        );
        fs::write(path.join("file"), b"").unwrap();
        let error = DirectoryHandle::open_following(&path.join("file")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotADirectory);
        fs::remove_dir_all(path).unwrap();
    }
}
