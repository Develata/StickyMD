//! A kqueue vnode watch (`EVFILT_VNODE`) over a tree of plain files and folders: what R03's
//! "nothing written" reads besides its two snapshots (USER 2026-11-24, option A). Adapter only:
//! it reports the raw flags each registered entry collected; what counts as a write is the rule
//! in `translocation`. kqueue keeps a pending flag per registered vnode until it is read, so
//! events coalesce but are not lost. The constants and the `struct kevent` layout are those of
//! the macOS SDK's `sys/event.h` and `sys/fcntl.h`; a test holds the layout.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use std::{
    fs::{File, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::fs::OpenOptionsExt,
    },
    path::PathBuf,
};

const EVFILT_VNODE: i16 = -4;
const EV_ADD: u16 = 0x0001;
const EV_CLEAR: u16 = 0x0020;
const EV_RECEIPT: u16 = 0x0040;
const EV_ERROR: u16 = 0x4000;
/// A descriptor for event notification only: it does not keep the volume busy.
const O_EVTONLY: i32 = 0x0000_8000;
const O_NOFOLLOW: i32 = 0x0000_0100;
/// Reads of a drained queue are bounded: with `EV_CLEAR` an entry reports again only when it
/// changes again, and nothing should write while the watch is read.
const DRAIN_ROUNDS: usize = 64;

/// `struct kevent` (`sys/event.h`). `udata` is `void *` there; it carries the entry's index.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct KEvent {
    ident: usize,
    filter: i16,
    flags: u16,
    fflags: u32,
    data: isize,
    udata: usize,
}

/// `struct timespec` (`sys/_types/_timespec.h`).
#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

unsafe extern "C" {
    fn kqueue() -> i32;
    fn kevent(
        kq: i32,
        changelist: *const KEvent,
        nchanges: i32,
        eventlist: *mut KEvent,
        nevents: i32,
        timeout: *const Timespec,
    ) -> i32;
}

/// One `kevent` call with a zero timeout: applies `changes`, then returns at once with what is
/// pending, at most `events.len()`.
fn call(queue: &OwnedFd, changes: &[KEvent], events: &mut [KEvent]) -> Result<usize, String> {
    let zero = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let nchanges = i32::try_from(changes.len()).map_err(|e| e.to_string())?;
    let nevents = i32::try_from(events.len()).map_err(|e| e.to_string())?;
    // SAFETY: `queue` is an open kqueue this watch owns; `changes` and `events` are valid for
    // `nchanges` and `nevents` elements of the SDK's `struct kevent` layout; `zero` outlives the
    // call; kevent writes at most `nevents` elements and returns their count or -1.
    let n = unsafe {
        kevent(
            queue.as_raw_fd(),
            changes.as_ptr(),
            nchanges,
            events.as_mut_ptr(),
            nevents,
            &zero,
        )
    };
    usize::try_from(n).map_err(|_| format!("kevent: {}", io::Error::last_os_error()))
}

/// The registered entries, each kept open for the watch's lifetime.
pub(in crate::mac_gui) struct Watch {
    queue: OwnedFd,
    _files: Vec<File>,
    names: Vec<String>,
}

/// Registers every `(name, path)` with `flags`. Each must be a plain file or folder (opened
/// without following links); any registration that fails fails the watch.
pub(in crate::mac_gui) fn watch(
    entries: &[(String, PathBuf)],
    flags: u32,
) -> Result<Watch, String> {
    // SAFETY: kqueue takes no arguments and returns a new descriptor or -1.
    let fd = unsafe { kqueue() };
    if fd < 0 {
        return Err(format!("kqueue: {}", io::Error::last_os_error()));
    }
    // SAFETY: `fd` was just returned by kqueue, is open, and nothing else owns it.
    let queue = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut files = Vec::with_capacity(entries.len());
    for (name, path) in entries {
        let kind = std::fs::symlink_metadata(path)
            .map_err(|e| format!("{name}: {e}"))?
            .file_type();
        if !(kind.is_file() || kind.is_dir()) {
            return Err(format!(
                "{name} is neither a plain file nor a folder; not watched"
            ));
        }
        files.push(
            OpenOptions::new()
                .read(true)
                .custom_flags(O_EVTONLY | O_NOFOLLOW)
                .open(path)
                .map_err(|e| format!("{name}: {e}"))?,
        );
    }
    let changes: Vec<KEvent> = files
        .iter()
        .enumerate()
        .map(|(index, file)| KEvent {
            ident: file.as_raw_fd() as usize,
            filter: EVFILT_VNODE,
            flags: EV_ADD | EV_CLEAR | EV_RECEIPT,
            fflags: flags,
            data: 0,
            udata: index,
        })
        .collect();
    // With EV_RECEIPT every change comes back once, EV_ERROR set and `data` its errno (0: done).
    let mut receipts = vec![KEvent::default(); changes.len()];
    let returned = call(&queue, &changes, &mut receipts)?;
    if returned != changes.len() {
        return Err(format!(
            "kevent confirmed {returned} of {} registrations",
            changes.len()
        ));
    }
    let mut confirmed = vec![false; changes.len()];
    for receipt in &receipts {
        let name = entries
            .get(receipt.udata)
            .map(|(name, _)| name.as_str())
            .ok_or_else(|| format!("kevent confirmed unknown entry {}", receipt.udata))?;
        if receipt.flags & EV_ERROR == 0 || receipt.data != 0 {
            return Err(format!(
                "{name}: not registered (flags {:#x}, errno {})",
                receipt.flags, receipt.data
            ));
        }
        confirmed[receipt.udata] = true;
    }
    if confirmed.contains(&false) {
        return Err("kevent did not confirm every registration".into());
    }
    Ok(Watch {
        queue,
        _files: files,
        names: entries.iter().map(|(name, _)| name.clone()).collect(),
    })
}

impl Watch {
    /// The registered entries' names, in registration order.
    pub(in crate::mac_gui) fn names(&self) -> &[String] {
        &self.names
    }

    /// Reads the queue until nothing is pending: each entry with the union of the flags it
    /// reported, in registration order; entries that reported nothing are left out.
    pub(in crate::mac_gui) fn drain(self) -> Result<Vec<(String, u32)>, String> {
        let mut flags = vec![0u32; self.names.len()];
        let mut events = vec![KEvent::default(); self.names.len().max(1)];
        for _ in 0..DRAIN_ROUNDS {
            let n = call(&self.queue, &[], &mut events)?;
            if n == 0 {
                return Ok(self
                    .names
                    .into_iter()
                    .zip(flags)
                    .filter(|(_, f)| *f != 0)
                    .collect());
            }
            for event in &events[..n] {
                if event.flags & EV_ERROR != 0 {
                    return Err(format!(
                        "kevent reported an error for entry {}: errno {}",
                        event.udata, event.data
                    ));
                }
                *flags
                    .get_mut(event.udata)
                    .ok_or_else(|| format!("kevent reported unknown entry {}", event.udata))? |=
                    event.fflags;
            }
        }
        Err(format!(
            "the watch still reported events after {DRAIN_ROUNDS} reads"
        ))
    }
}

/// What each kind of change raises, measured on APFS (macOS 27.2, 2026-11-24): every change to a
/// file's bytes, a folder's names or a link count is reported under a counted flag, even when
/// the end state equals the start; permissions, extended attributes and times only as
/// `NOTE_ATTRIB`; a write through a shared mapping undone before `msync` not at all.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mac_gui::translocation::{
        NOTE_ATTRIB, NOTE_DELETE, NOTE_EXTEND, NOTE_LINK, NOTE_RENAME, NOTE_WRITE, REGISTERED,
        WRITTEN,
    };
    use std::{
        fs,
        io::{Seek, SeekFrom, Write},
        path::Path,
        time::SystemTime,
    };

    #[test]
    fn the_kevent_size_offsets_and_array_stride_are_the_sdk_s() {
        // What the calls rely on: each field where the SDK's `struct kevent` has it, and arrays
        // of 32-byte elements (review fix round, 12: the SDK packs to 4, this type aligns to 8
        // on arm64; the layout passed is the same).
        assert_eq!(std::mem::size_of::<KEvent>(), 32);
        assert_eq!(std::mem::size_of::<[KEvent; 2]>(), 64);
        assert_eq!(std::mem::offset_of!(KEvent, ident), 0);
        assert_eq!(std::mem::offset_of!(KEvent, filter), 8);
        assert_eq!(std::mem::offset_of!(KEvent, flags), 10);
        assert_eq!(std::mem::offset_of!(KEvent, fflags), 12);
        assert_eq!(std::mem::offset_of!(KEvent, data), 16);
        assert_eq!(std::mem::offset_of!(KEvent, udata), 24);
        assert_eq!(std::mem::size_of::<Timespec>(), 16);
    }

    /// A change made to the watched tree.
    type Change<'a> = &'a dyn Fn(&Path);
    /// A named change, the entries it must report as written, and one flag it must raise.
    type Case<'a> = (&'a str, Change<'a>, &'a [&'a str], u32);

    fn mtime(path: &Path) -> SystemTime {
        fs::metadata(path).unwrap().modified().unwrap()
    }

    fn set_mtime(path: &Path, time: SystemTime) {
        File::open(path).unwrap().set_modified(time).unwrap();
    }

    /// A tree like a small release layout, watched as R03 watches it; `change` runs between
    /// registration and drain, and the result is each entry's flags.
    fn observe(change: &dyn Fn(&Path)) -> Vec<(String, u32)> {
        observe_with_snapshots(change).0
    }

    /// [`observe`], and whether R03's snapshot of the tree is the same after `change` as before.
    fn observe_with_snapshots(change: &dyn Fn(&Path)) -> (Vec<(String, u32)>, bool) {
        let root = tempdir();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("f"), b"hello world").unwrap();
        fs::write(root.join("sub/g"), b"nested").unwrap();
        let entries: Vec<(String, PathBuf)> = [".", "f", "sub", "sub/g"]
            .iter()
            .map(|name| ((*name).to_owned(), root.join(name)))
            .collect();
        let watch = watch(&entries, REGISTERED).unwrap();
        assert_eq!(watch.names(), [".", "f", "sub", "sub/g"]);
        let before = crate::mac_gui::snapshot::tree(&root).unwrap();
        change(&root);
        let flags = watch.drain().unwrap();
        let unchanged = crate::mac_gui::snapshot::tree(&root).unwrap() == before;
        fs::remove_dir_all(&root).unwrap();
        (flags, unchanged)
    }

    fn tempdir() -> PathBuf {
        // A counter as well as the time: tests run in parallel within one process.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "stickymd-vnodes-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    fn written(flags: &[(String, u32)]) -> Vec<&str> {
        flags
            .iter()
            .filter(|(_, f)| f & WRITTEN != 0)
            .map(|(name, _)| name.as_str())
            .collect()
    }

    #[test]
    fn nothing_and_reading_raise_nothing() {
        assert_eq!(observe(&|_| {}), []);
        assert_eq!(
            observe(&|root| {
                fs::read(root.join("f")).unwrap();
                fs::read_dir(root).unwrap().count();
                fs::metadata(root.join("sub/g")).unwrap();
            }),
            []
        );
    }

    /// Changes that end where they began, so a snapshot cannot see them, each raise a counted
    /// flag on the entry they touch.
    #[test]
    fn writes_undone_before_the_end_are_still_reported() {
        let rewrite = |root: &Path, bytes: &[u8]| {
            let path = root.join("f");
            let time = mtime(&path);
            let mut file = OpenOptions::new().write(true).open(&path).unwrap();
            file.write_all(bytes).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(b"hello world").unwrap();
            drop(file);
            set_mtime(&path, time);
        };
        let cases: [Case; 9] = [
            (
                "same bytes",
                &|r| rewrite(r, b"hello world"),
                &["f"],
                NOTE_WRITE,
            ),
            (
                "write and restore",
                &|r| rewrite(r, b"HELLO WORLD"),
                &["f"],
                NOTE_WRITE,
            ),
            (
                "grow and shrink back",
                &|r| {
                    let path = r.join("f");
                    let time = mtime(&path);
                    let file = OpenOptions::new().write(true).open(&path).unwrap();
                    file.set_len(20).unwrap();
                    file.set_len(11).unwrap();
                    drop(file);
                    set_mtime(&path, time);
                },
                &["f"],
                NOTE_EXTEND,
            ),
            (
                "create and remove",
                &|r| {
                    let time = mtime(r);
                    fs::write(r.join("x"), b"x").unwrap();
                    fs::remove_file(r.join("x")).unwrap();
                    set_mtime(r, time);
                },
                &["."],
                NOTE_WRITE,
            ),
            (
                "create and remove in a subfolder",
                &|r| {
                    let sub = r.join("sub");
                    let time = mtime(&sub);
                    fs::write(sub.join("x"), b"x").unwrap();
                    fs::remove_file(sub.join("x")).unwrap();
                    set_mtime(&sub, time);
                },
                &["sub"],
                NOTE_WRITE,
            ),
            (
                "make and remove a folder",
                &|r| {
                    let time = mtime(r);
                    fs::create_dir(r.join("d")).unwrap();
                    fs::remove_dir(r.join("d")).unwrap();
                    set_mtime(r, time);
                },
                &["."],
                NOTE_LINK,
            ),
            (
                "rename away and back",
                &|r| {
                    let time = mtime(r);
                    fs::rename(r.join("f"), r.join("h")).unwrap();
                    fs::rename(r.join("h"), r.join("f")).unwrap();
                    set_mtime(r, time);
                },
                &[".", "f"],
                NOTE_RENAME,
            ),
            (
                "a hard link added and removed",
                &|r| {
                    let time = mtime(r);
                    fs::hard_link(r.join("f"), r.join("h")).unwrap();
                    fs::remove_file(r.join("h")).unwrap();
                    set_mtime(r, time);
                },
                &[".", "f"],
                NOTE_LINK,
            ),
            (
                "replaced atomically by the same bytes",
                &|r| {
                    let time = mtime(r);
                    fs::write(r.join("f.tmp"), b"hello world").unwrap();
                    // The replacement carries the original's time, so no snapshot sees it.
                    set_mtime(&r.join("f.tmp"), mtime(&r.join("f")));
                    fs::rename(r.join("f.tmp"), r.join("f")).unwrap();
                    set_mtime(r, time);
                },
                &[".", "f"],
                NOTE_DELETE,
            ),
        ];
        for (name, change, touched, flag) in cases {
            let (flags, unchanged) = observe_with_snapshots(change);
            // R03's snapshots cannot see any of these (review fix round, 10).
            assert!(unchanged, "{name}: the snapshot saw it");
            assert_eq!(written(&flags), touched, "{name}: {flags:?}");
            assert!(
                flags.iter().any(|(_, f)| f & flag != 0),
                "{name}: {flags:?}"
            );
        }
    }

    /// Permissions, extended attributes and times are attributes, not writes.
    #[test]
    fn attribute_changes_are_reported_apart() {
        use std::os::unix::fs::PermissionsExt;
        let cases: [(&str, Change); 3] = [
            ("permissions", &|r| {
                let path = r.join("f");
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            }),
            ("extended attribute", &|r| {
                for args in [["-w", "x.stickymd", "1"].as_slice(), &["-d", "x.stickymd"]] {
                    let status = std::process::Command::new("/usr/bin/xattr")
                        .args(args)
                        .arg(r.join("f"))
                        .status()
                        .unwrap();
                    assert!(status.success());
                }
            }),
            ("times", &|r| {
                set_mtime(&r.join("f"), SystemTime::UNIX_EPOCH)
            }),
        ];
        for (name, change) in cases {
            let flags = observe(change);
            assert_eq!(flags, [("f".to_owned(), NOTE_ATTRIB)], "{name}");
        }
    }

    /// The premise plan 11 names: a write through a shared mapping, undone before it is synced,
    /// raises nothing counted (the restored time raises `NOTE_ATTRIB`).
    #[test]
    fn a_mapped_write_undone_before_msync_is_not_reported() {
        unsafe extern "C" {
            fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64)
            -> *mut u8;
            fn munmap(addr: *mut u8, len: usize) -> i32;
        }
        let flags = observe(&|r| {
            let path = r.join("f");
            let time = mtime(&path);
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            // SAFETY: maps the 11 bytes of this open file shared (PROT_READ | PROT_WRITE,
            // MAP_SHARED), checks the mapping, writes within it and unmaps it before `file` is
            // dropped; nothing else holds the mapping.
            unsafe {
                let map = mmap(
                    std::ptr::null_mut(),
                    11,
                    0x1 | 0x2,
                    0x1,
                    file.as_raw_fd(),
                    0,
                );
                assert_ne!(map as isize, -1);
                map.write(b'H');
                map.write(b'h');
                assert_eq!(munmap(map, 11), 0);
            }
            drop(file);
            set_mtime(&path, time);
        });
        assert_eq!(written(&flags), Vec::<&str>::new(), "{flags:?}");
    }

    #[test]
    fn links_and_missing_entries_are_not_watched() {
        let root = tempdir();
        std::os::unix::fs::symlink("elsewhere", root.join("link")).unwrap();
        let link = [("link".to_owned(), root.join("link"))];
        let error = watch(&link, REGISTERED).err().unwrap();
        assert!(
            error.contains("neither a plain file nor a folder"),
            "{error}"
        );
        let missing = [("gone".to_owned(), root.join("gone"))];
        assert!(
            watch(&missing, REGISTERED)
                .err()
                .unwrap()
                .starts_with("gone: ")
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
