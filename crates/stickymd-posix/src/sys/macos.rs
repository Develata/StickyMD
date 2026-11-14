//! The macOS spelling of the calls in `sys`.
//!
//! plan_ref: docs/plan/05_document_persistence.md#macos-storage

use std::ffi::{CStr, OsStr, OsString};
use std::io;
use std::os::fd::RawFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

use super::check;

/// No symbolic link anywhere in the path; no libc constant yet, from <sys/fcntl.h>.
const O_NOFOLLOW_ANY: libc::c_int = 0x2000_0000;

/// Asks the drive itself to reach stable storage; `fsync` stops at the drive's cache.
pub(crate) fn full_sync(descriptor: RawFd) -> io::Result<()> {
    // SAFETY: the caller keeps the descriptor open for the call; F_FULLFSYNC takes no argument.
    check(unsafe { libc::fcntl(descriptor, libc::F_FULLFSYNC) }.min(0))
}

/// Moves `from` to `to` only if nothing is at `to`.
pub(crate) fn rename_no_replace(from: &CStr, to: &CStr) -> io::Result<()> {
    // SAFETY: both pointers are NUL-terminated strings that outlive the call.
    check(unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) })
}

/// Moves `from` in `from_directory` to `to` in `to_directory` only if nothing is at `to`.
pub(crate) fn rename_no_replace_at(
    from_directory: RawFd,
    from: &CStr,
    to_directory: RawFd,
    to: &CStr,
) -> io::Result<()> {
    // SAFETY: the caller keeps both descriptors open for the call; both names outlive it.
    check(unsafe {
        libc::renameatx_np(
            from_directory,
            from.as_ptr(),
            to_directory,
            to.as_ptr(),
            libc::RENAME_EXCL,
        )
    })
}

/// Opens `path` relative to `base` (or absolute) with `flags`, refusing a symbolic link anywhere
/// in the path. Returns the descriptor, or -1 with `errno` set.
pub(crate) fn open_no_links_at(base: RawFd, path: &CStr, flags: libc::c_int) -> libc::c_int {
    // SAFETY: `path` is NUL-terminated and outlives the call; the caller keeps `base` open for it.
    unsafe { libc::openat(base, path.as_ptr(), flags | O_NOFOLLOW_ANY) }
}

/// Whether the open file lies on this machine rather than a network volume.
pub(crate) fn on_local_volume(descriptor: RawFd) -> io::Result<bool> {
    // SAFETY: a zeroed statfs is a valid value for fstatfs to overwrite.
    let mut volume: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: the caller keeps the descriptor open for the call; `volume` is writable.
    check(unsafe { libc::fstatfs(descriptor, &mut volume) })?;
    Ok(volume.f_flags & libc::MNT_LOCAL as u32 != 0)
}

/// Where the open file or directory is now, after any move.
pub(crate) fn descriptor_path(descriptor: RawFd) -> io::Result<PathBuf> {
    let mut path = [0_u8; libc::PATH_MAX as usize];
    // SAFETY: F_GETPATH writes a NUL-terminated path of at most PATH_MAX bytes to the buffer,
    // which outlives the call; the caller keeps the descriptor open for it.
    check(unsafe { libc::fcntl(descriptor, libc::F_GETPATH, path.as_mut_ptr()) })?;
    let length = path
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(path.len());
    Ok(OsStr::from_bytes(&path[..length]).into())
}

/// The per-user directory for this user's sockets. `$TMPDIR` can be overridden by whoever starts
/// the process, and two copies with different values would never find each other; `confstr`
/// answers per user regardless.
pub(crate) fn user_runtime_dir() -> Option<PathBuf> {
    let mut buffer = vec![0_u8; libc::PATH_MAX as usize];
    // SAFETY: the buffer is writable for the length passed.
    let needed = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_TEMP_DIR,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
        )
    };
    if needed == 0 || needed > buffer.len() {
        return None;
    }
    buffer.truncate(needed - 1);
    Some(PathBuf::from(OsString::from_vec(buffer)))
}
