//! Atomic persistence for verification evidence files.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn write(path: &Path, contents: &[u8]) -> Result<(), String> {
    check_spelling(path)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| format!("evidence path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create evidence directory {}: {error}",
            parent.display()
        )
    })?;

    let temporary = temporary_path(path)?;
    write_and_publish(&temporary, contents, || replace(&temporary, path))
}

fn temporary_path(target: &Path) -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos();
    temporary_path_at(target, nonce)
}

fn temporary_path_at(target: &Path, nonce: u128) -> Result<PathBuf, String> {
    // Concurrent writers of one target in one process can read the same clock value
    // (Windows clock ticks are coarse under load); the sequence keeps temporaries
    // distinct so the losing writer does not fail on CreateNew.
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("evidence file name is not UTF-8: {}", target.display()))?;
    Ok(target.with_file_name(format!(
        ".{name}.tmp-{}-{nonce}-{sequence}",
        std::process::id()
    )))
}

/// Atomically create a new complete file; preserve the old CreateNew no-overwrite interface.
pub(crate) fn write_new(path: &Path, contents: &[u8]) -> Result<(), String> {
    check_spelling(path)?;
    let parent = path.parent().ok_or("new file path has no parent")?;
    if !parent.is_dir() {
        return Err(format!(
            "destination directory does not exist: {}",
            parent.display()
        ));
    }
    let temporary = temporary_path(path)?;
    write_and_publish(&temporary, contents, || publish_new(&temporary, path))
}

fn write_and_publish(
    temporary: &Path,
    contents: &[u8],
    publish: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    // Cleanup is authorized only after this invocation actually created the temp file.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .map_err(|error| format!("cannot create {}: {error}", temporary.display()))?;
    let written = file.write_all(contents).and_then(|()| file.sync_all());
    drop(file);
    let result = written
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))
        .and_then(|()| publish());
    let cleanup = fs::remove_file(temporary);
    result?;
    match cleanup {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cannot remove temporary output {}: {error}",
            temporary.display()
        )),
    }
}

#[cfg(windows)]
fn replace(temporary: &Path, target: &Path) -> Result<(), String> {
    move_windows(temporary, target, true)
}

#[cfg(windows)]
fn publish_new(temporary: &Path, target: &Path) -> Result<(), String> {
    move_windows(temporary, target, false)
}

/// Refuses, before any file exists, spellings whose meaning the publish step cannot keep:
/// - a verbatim (`\\?\`) path containing `/`: Windows does not treat it as a separator
///   there, but joining or re-splitting the name would, so the write could land elsewhere
///   than the guards checked;
/// - an ordinary path whose file name is a DOS device (`NUL`, `CON`, `COM1`, `LPT1`, with
///   or without an extension or a trailing colon): Win32 opens the device, while the
///   extended-length form used for long paths would name a literal file;
/// - an ordinary file name containing `:` (an alternate data stream or a device spelling);
///   evidence files are plain files.
///
/// Public output guards call this too, so a refused spelling never gets as far as creating
/// a parent directory.
#[cfg(windows)]
pub(crate) fn check_spelling(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let verbatim = units.starts_with(&[0x5C, 0x5C, 0x3F, 0x5C]);
    if verbatim && units.contains(&0x2F) {
        return Err(format!(
            "verbatim evidence paths cannot contain '/': {}",
            path.display()
        ));
    }
    let name = path.file_name().and_then(|name| name.to_str());
    if !verbatim && name.is_some_and(|name| name.contains(':')) {
        return Err(format!(
            "evidence file names cannot contain ':': {}",
            path.display()
        ));
    }
    if !verbatim && name.is_some_and(is_dos_device) {
        return Err(format!(
            "evidence file names cannot be Windows device names: {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn check_spelling(_: &Path) -> Result<(), String> {
    Ok(())
}

/// Whether Win32 resolves an ordinary file name to a device: the stem before the first dot
/// (or colon), with trailing spaces ignored, is a reserved name in any case. `COM0` and
/// `LPT0` are ordinary files.
#[cfg(any(windows, test))]
fn is_dos_device(name: &str) -> bool {
    let stem = name
        .split(['.', ':'])
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|device| {
        stem.strip_prefix(device).is_some_and(|number| {
            matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
                || matches!(number, "\u{00B9}" | "\u{00B2}" | "\u{00B3}")
        })
    })
}

/// `MoveFileExW` on an ordinary path is limited to 260 characters (a CI runner's longer
/// temp directory reaches it), while `std` file creation is not. Both files live in one
/// existing directory, so its canonical form (extended-length `\\?\`) joined with the file
/// name names the same file without that limit; the name keeps its Win32 meaning (trailing
/// dots and spaces dropped, as `path_identity` assumes; device names were refused by
/// `check_spelling`). A verbatim path already has no such limit and is used exactly as
/// given. If the directory cannot be resolved, the plain path is used and the move reports
/// its own error.
#[cfg(windows)]
fn extended_length(path: &Path) -> PathBuf {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if units.starts_with(&[0x5C, 0x5C, 0x3F, 0x5C]) {
        return path.to_path_buf();
    }
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return path.to_path_buf();
    };
    let mut name = name.encode_wide().collect::<Vec<_>>();
    while name.last().is_some_and(|unit| matches!(*unit, 0x20 | 0x2e)) {
        name.pop();
    }
    if name.is_empty() {
        return path.to_path_buf();
    }
    fs::canonicalize(parent).map_or_else(
        |_| path.to_path_buf(),
        |directory| directory.join(OsString::from_wide(&name)),
    )
}

#[cfg(windows)]
fn move_windows(temporary: &Path, target: &Path, overwrite: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let from = extended_length(temporary)
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let to = extended_length(target)
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: `from` and `to` are live, NUL-terminated UTF-16 buffers for the duration of the
    // call. Both paths are same-directory evidence files owned by this process. The API does
    // not retain either pointer. Replacement is enabled only for the overwrite interface;
    // new notices retain CreateNew semantics using a same-directory no-replace move.
    let moved = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_WRITE_THROUGH
                | if overwrite {
                    MOVEFILE_REPLACE_EXISTING
                } else {
                    0
                },
        )
    };
    if moved == 0 {
        Err(format!(
            "cannot atomically publish {} as {}: {}",
            temporary.display(),
            target.display(),
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn publish_new(temporary: &Path, target: &Path) -> Result<(), String> {
    fs::hard_link(temporary, target)
        .map_err(|error| format!("cannot publish new file {}: {error}", target.display()))
}

#[cfg(not(windows))]
fn replace(temporary: &Path, target: &Path) -> Result<(), String> {
    fs::rename(temporary, target).map_err(|error| {
        format!(
            "cannot atomically publish {} as {}: {error}",
            temporary.display(),
            target.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::write;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A directory this test creates exclusively and removes on drop, even after a failure.
    #[cfg(windows)]
    struct Scratch(std::path::PathBuf);

    #[cfg(windows)]
    impl Scratch {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("stickymd-{label}-{}-{nonce}", std::process::id()));
            fs::create_dir(&path).expect("exclusive scratch directory");
            Self(path)
        }
    }

    #[cfg(windows)]
    impl Drop for Scratch {
        fn drop(&mut self) {
            // Created exclusively by this test.
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The ordinary and verbatim spellings of `scratch`, whatever `temp_dir()` returned:
    /// a drive path `\\?\C:\x` is ordinary `C:\x`, a UNC path `\\?\UNC\s\x` is `\\s\x`.
    #[cfg(windows)]
    fn spellings(scratch: &Scratch) -> (std::path::PathBuf, std::path::PathBuf) {
        let verbatim = fs::canonicalize(&scratch.0).unwrap();
        let text = verbatim.to_str().unwrap();
        let ordinary = text.strip_prefix(r"\\?\UNC\").map_or_else(
            || text.strip_prefix(r"\\?\").unwrap().to_owned(),
            |rest| format!(r"\\{rest}"),
        );
        (std::path::PathBuf::from(ordinary), verbatim)
    }

    #[cfg(windows)]
    fn utf16_len(path: &std::path::Path) -> usize {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().count()
    }

    /// Evidence under a deep directory (a CI runner's longer temp path) has temporaries past
    /// 260 characters: publishing must not depend on the legacy path limit, for ordinary and
    /// verbatim spellings alike, and an ordinary name keeps its Win32 meaning.
    #[cfg(windows)]
    #[test]
    fn evidence_beyond_the_legacy_path_limit_is_published() {
        let scratch = Scratch::new("long-evidence");
        let (ordinary_root, verbatim_root) = spellings(&scratch);
        for (root, label) in [(&ordinary_root, "ordinary"), (&verbatim_root, "verbatim")] {
            let deep = root.join(label).join("d".repeat(120)).join("e".repeat(60));
            let target = deep.join(format!("resources-source-preview-{}.json", "7".repeat(64)));
            assert!(utf16_len(&target) > 260, "{}", target.display());
            write(&target, b"first").unwrap();
            write(&target, b"second").unwrap();
            assert_eq!(fs::read(&target).unwrap(), b"second", "{label}");
            let notice = deep.join(format!("notice-{}.txt", "8".repeat(64)));
            super::write_new(&notice, b"once").unwrap();
            assert!(super::write_new(&notice, b"twice").is_err());
            assert_eq!(fs::read(&notice).unwrap(), b"once", "{label}");
        }
        // Ordinary: a trailing dot names the same file, as Win32 resolves it.
        let deep = ordinary_root
            .join("ordinary")
            .join("d".repeat(120))
            .join("e".repeat(60));
        let target = deep.join(format!("resources-source-preview-{}.json", "7".repeat(64)));
        write(
            &std::path::PathBuf::from(format!("{}.", target.display())),
            b"third",
        )
        .unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"third");
        // Only the temporary exceeds the limit: pad the target to 250 UTF-16 units; its
        // temporary adds a leading dot and `.tmp-<pid>-<nanos>-<seq>`, at least 29 more.
        let base = ordinary_root.join("near");
        let file = "fits.json";
        let used = utf16_len(&base.join(file)) + 1;
        assert!(used < 250, "the scratch root is too long for this case");
        let fitting = base.join("n".repeat(250 - used)).join(file);
        assert_eq!(utf16_len(&fitting), 250);
        let temporary = super::temporary_path_at(&fitting, u128::from(u64::MAX)).unwrap();
        assert!(utf16_len(&temporary) > 260, "{}", temporary.display());
        write(&fitting, b"near").unwrap();
        assert_eq!(fs::read(&fitting).unwrap(), b"near");
    }

    /// The spellings the publish step cannot keep are refused before anything is created,
    /// by both writers and by the public output guard: a verbatim `/`, an ordinary DOS device
    /// name, and an ordinary name with `:`.
    #[cfg(windows)]
    #[test]
    fn ambiguous_spellings_are_refused_before_writing() {
        let scratch = Scratch::new("ambiguous-evidence");
        let (ordinary, verbatim) = spellings(&scratch);
        fs::create_dir(verbatim.join("alias")).unwrap();
        let slashed =
            std::path::PathBuf::from(format!("{}\\alias/target.json", verbatim.display()));
        for attempt in [write(&slashed, b"x"), super::write_new(&slashed, b"x")] {
            assert!(attempt.unwrap_err().contains("cannot contain '/'"));
        }
        for name in [
            "NUL.json",
            "con",
            "Com1.txt",
            "lpt9",
            "AUX .log",
            "NUL:",
            "notes.json:hidden",
        ] {
            let path = ordinary.join("fresh").join(name);
            for attempt in [write(&path, b"x"), super::write_new(&path, b"x")] {
                assert!(attempt.is_err(), "{name}");
            }
            let guarded = crate::qualification::validate_public_evidence_path(&ordinary, &path);
            assert!(guarded.is_err(), "{name}");
        }
        assert!(
            !ordinary.join("fresh").exists(),
            "a parent was created before refusal"
        );
        assert_eq!(fs::read_dir(verbatim.join("alias")).unwrap().count(), 0);
        // Ordinary names that only resemble devices are files.
        for name in ["COM0.json", "lpt0.txt", "nullable.json"] {
            write(&ordinary.join(name), b"ok").unwrap();
            assert_eq!(fs::read(ordinary.join(name)).unwrap(), b"ok", "{name}");
        }
    }

    #[test]
    fn dos_device_names_are_recognized_in_any_case_and_with_extensions() {
        for name in [
            "NUL",
            "nul.json",
            "CON.txt",
            "com1",
            "LPT9.x",
            "COM\u{00B9}",
            "aux ",
            "CONIN$",
            "NUL:",
            "con:.txt",
        ] {
            assert!(super::is_dos_device(name), "{name}");
        }
        for name in [
            "nullable.json",
            "console",
            "COM",
            "COM0",
            "LPT0.txt",
            "COM10",
            "LPT",
            "notice.json",
            ".NUL.tmp",
        ] {
            assert!(!super::is_dos_device(name), "{name}");
        }
    }

    #[test]
    fn an_identical_clock_value_still_yields_distinct_temporaries() {
        let target = std::env::temp_dir().join("stickymd-temporary-name-probe.json");
        let first = super::temporary_path_at(&target, 42).unwrap();
        let second = super::temporary_path_at(&target, 42).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn concurrent_writers_of_one_target_never_share_a_temporary_path() {
        let target = std::env::temp_dir().join("stickymd-temporary-name-probe.json");
        let names = std::thread::scope(|scope| {
            let handles = (0..8)
                .map(|_| scope.spawn(|| super::temporary_path(&target).unwrap()))
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<std::collections::BTreeSet<_>>()
        });
        assert_eq!(names.len(), 8, "every writer needs its own temporary file");
    }

    #[test]
    fn create_new_publishes_once_and_preserves_existing_bytes_under_races() {
        let root = std::env::temp_dir().join(format!(
            "stickymd-atomic-new-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("中文 output.txt");
        let handles = [b"first".as_slice(), b"second".as_slice()]
            .into_iter()
            .map(|bytes| {
                let path = target.clone();
                std::thread::spawn(move || super::write_new(&path, bytes))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .filter(Result::is_ok)
                .count(),
            1
        );
        let before = std::fs::read(&target).unwrap();
        assert!(before == b"first" || before == b"second");
        assert!(super::write_new(&target, b"overwrite").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), before);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn occupied_temporary_file_is_never_removed_without_ownership() {
        let root = std::env::temp_dir().join(format!(
            "stickymd-atomic-collision-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let occupied = root.join("occupied.tmp");
        std::fs::write(&occupied, b"belongs to another operation").unwrap();
        assert!(
            super::write_and_publish(&occupied, b"replacement", || panic!("must not publish"))
                .is_err()
        );
        assert_eq!(
            std::fs::read(&occupied).unwrap(),
            b"belongs to another operation"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn successful_write_atomically_replaces_previous_evidence() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-atomic-evidence-{nonce}"));
        let target = root.join("module.json");
        write(&target, b"old").expect("initial write");
        write(&target, b"new").expect("replacement write");
        assert_eq!(fs::read(&target).expect("read evidence"), b"new");
        assert_eq!(fs::read_dir(&root).expect("read root").count(), 1);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
