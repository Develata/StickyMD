//! The account the process runs as, from the user database rather than the environment: whoever
//! starts the process can set `$HOME` to anything.
//!
//! plan_ref: docs/plan/05_document_persistence.md#note-root

use std::ffi::{CStr, OsStr};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

/// Upper bound for the `getpwuid_r` buffer; an entry larger than this is refused, not truncated.
const BUFFER_LIMIT: usize = 1 << 20;

/// The real user id of the process.
pub fn real_user_id() -> u32 {
    // SAFETY: getuid has no preconditions and always succeeds.
    unsafe { libc::getuid() }
}

/// The home directory of the real user, as the user database records it (`getpwuid_r`).
pub fn home_directory() -> io::Result<PathBuf> {
    let uid = real_user_id();
    // SAFETY: sysconf only reads a configuration value.
    let suggested = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let mut size = usize::try_from(suggested)
        .ok()
        .filter(|size| *size > 0)
        .unwrap_or(4096)
        .min(BUFFER_LIMIT);
    loop {
        let mut buffer = vec![0 as libc::c_char; size];
        // SAFETY: an all-zero passwd is a valid value for getpwuid_r to overwrite.
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        // SAFETY: `entry`, `buffer` (with its length) and `found` are writable for the call, and
        // the strings it stores point into `buffer`, which outlives every read below.
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                &mut entry,
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut found,
            )
        };
        if status == libc::ERANGE && size < BUFFER_LIMIT {
            size = (size * 2).min(BUFFER_LIMIT);
            continue;
        }
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status));
        }
        if found.is_null() || entry.pw_dir.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "the user database has no home directory for this user",
            ));
        }
        // SAFETY: `pw_dir` is a NUL-terminated string inside `buffer`, which is still alive.
        let directory = unsafe { CStr::from_ptr(entry.pw_dir) };
        let path = PathBuf::from(OsStr::from_bytes(directory.to_bytes()));
        return if path.is_absolute() {
            Ok(path)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the home directory in the user database is not an absolute path",
            ))
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the shell's `~user` expands to, which reads the same user database.
    fn database_home() -> PathBuf {
        let output = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("eval echo ~$(/usr/bin/id -un)")
            .env("HOME", "/var/empty/not-the-home")
            .output()
            .unwrap();
        assert!(output.status.success());
        PathBuf::from(String::from_utf8(output.stdout).unwrap().trim_end())
    }

    #[test]
    fn the_home_directory_comes_from_the_user_database() {
        let home = home_directory().unwrap();
        assert!(home.is_absolute(), "{home:?}");
        assert_eq!(home, database_home());
    }

    /// The child test prints what it finds while `$HOME` names somewhere else.
    #[test]
    fn a_changed_home_variable_is_ignored() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "user::tests::print_home_directory",
                "--include-ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("HOME", "/var/empty/not-the-home")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let printed = stdout
            .lines()
            .find_map(|line| line.split_once("HOME_DIRECTORY=").map(|(_, home)| home))
            .expect("the child printed the home directory");
        assert_eq!(PathBuf::from(printed), database_home());
    }

    #[test]
    #[ignore = "run by a_changed_home_variable_is_ignored in a child process"]
    fn print_home_directory() {
        println!("HOME_DIRECTORY={}", home_directory().unwrap().display());
    }
}
