//! The calls whose spelling differs between operating systems; everything else in this crate is
//! plain POSIX. Only macOS is implemented. The Debian host will add `linux.rs` with the same
//! functions:
//!
//! | Function | macOS | Linux |
//! | --- | --- | --- |
//! | `full_sync` | `fcntl(F_FULLFSYNC)` | `fsync` |
//! | `rename_no_replace`, `rename_no_replace_at` | `renamex_np` / `renameatx_np` with `RENAME_EXCL` | `renameat2(RENAME_NOREPLACE)` |
//! | `open_no_links_at` | `openat` with `O_NOFOLLOW_ANY` | `openat2` with `RESOLVE_NO_SYMLINKS` |
//! | `on_local_volume` | `fstatfs` and `MNT_LOCAL` | `fstatfs` `f_type` against network file systems |
//! | `descriptor_path` | `fcntl(F_GETPATH)` | `readlink("/proc/self/fd/N")` |
//! | `user_runtime_dir` | `confstr(_CS_DARWIN_USER_TEMP_DIR)` | `XDG_RUNTIME_DIR` |
//!
//! plan_ref: docs/plan/05_document_persistence.md#macos-storage

use std::io;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    descriptor_path, full_sync, on_local_volume, open_no_links_at, rename_no_replace,
    rename_no_replace_at, user_runtime_dir,
};

/// A call that returns 0 on success and sets `errno` otherwise.
pub(crate) fn check(result: libc::c_int) -> io::Result<()> {
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
