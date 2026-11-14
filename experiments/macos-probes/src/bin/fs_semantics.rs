//! What the file system of one directory really guarantees. `fs_semantics <empty-dir>`.
//! Each line is `name: outcome`, so the same run on several volume formats can be diffed.
use std::ffi::{CString, OsStr};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::{Path, PathBuf};
use std::time::Instant;

const O_NOFOLLOW_ANY: i32 = 0x2000_0000;
const RENAME_NOFOLLOW_ANY: u32 = 0x10;

fn c(path: &Path) -> CString {
    CString::new(path.as_os_str().as_bytes()).unwrap()
}

fn name(text: &str) -> CString {
    CString::new(text).unwrap()
}

fn errno(result: i32) -> String {
    if result == 0 {
        "ok".into()
    } else {
        let error = io::Error::last_os_error();
        format!("errno {} ({error})", error.raw_os_error().unwrap_or(0))
    }
}

fn renamex(from: &Path, to: &Path, flags: u32) -> i32 {
    unsafe { libc::renamex_np(c(from).as_ptr(), c(to).as_ptr(), flags) }
}

fn write(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| format!("<{e}>"))
}

fn open_raw(path: &Path, flags: i32) -> Result<File, String> {
    let fd = unsafe { libc::open(c(path).as_ptr(), flags, 0o644) };
    if fd < 0 {
        Err(errno(-1))
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn fd_path(fd: RawFd) -> String {
    let mut buffer = vec![0u8; libc::PATH_MAX as usize];
    let result = unsafe { libc::fcntl(fd, libc::F_GETPATH, buffer.as_mut_ptr()) };
    if result != 0 {
        return errno(result);
    }
    let end = buffer.iter().position(|b| *b == 0).unwrap();
    String::from_utf8_lossy(&buffer[..end]).into_owned()
}

fn median_ms(mut samples: Vec<f64>) -> (f64, f64) {
    samples.sort_by(f64::total_cmp);
    (samples[samples.len() / 2], *samples.last().unwrap())
}

fn sync_cost(dir: &Path, bytes: usize, full: bool) -> String {
    let path = dir.join("sync.bin");
    let payload = vec![b'x'; bytes];
    let mut samples = Vec::new();
    for _ in 0..15 {
        let mut file = File::create(&path).unwrap();
        file.write_all(&payload).unwrap();
        let start = Instant::now();
        let result = if full {
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) }
        } else {
            unsafe { libc::fsync(file.as_raw_fd()) }
        };
        if result != 0 {
            return errno(result);
        }
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    fs::remove_file(&path).unwrap();
    let (median, max) = median_ms(samples);
    format!("median {median:.2} ms, max {max:.2} ms")
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("empty directory"));
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    let missing = root.join("missing.txt");

    // Publish primitives.
    write(&a, "A");
    write(&b, "B");
    println!("rename_excl onto existing: {} (b={})", errno(renamex(&a, &b, libc::RENAME_EXCL)), read(&b));
    println!("rename_excl to free name: {} (missing={})", errno(renamex(&a, &missing, libc::RENAME_EXCL)), read(&missing));
    fs::rename(&missing, &a).unwrap();
    println!("rename_swap both exist: {} (a={} b={})", errno(renamex(&a, &b, libc::RENAME_SWAP)), read(&a), read(&b));
    println!("rename_swap target missing: {}", errno(renamex(&a, &root.join("nope"), libc::RENAME_SWAP)));

    // Symlink refusal.
    let link = root.join("link.txt");
    symlink(&a, &link).unwrap();
    println!("open O_NOFOLLOW on final symlink: {:?}", open_raw(&link, libc::O_RDONLY | libc::O_NOFOLLOW).map(|_| "opened"));
    let real_dir = root.join("real");
    fs::create_dir(&real_dir).unwrap();
    write(&real_dir.join("f"), "F");
    let dir_link = root.join("dirlink");
    symlink(&real_dir, &dir_link).unwrap();
    println!("open O_NOFOLLOW through parent symlink: {:?}", open_raw(&dir_link.join("f"), libc::O_RDONLY | libc::O_NOFOLLOW).map(|_| "opened"));
    println!("open O_NOFOLLOW_ANY through parent symlink: {:?}", open_raw(&dir_link.join("f"), libc::O_RDONLY | O_NOFOLLOW_ANY).map(|_| "opened"));
    println!("rename onto symlink name (plain): replaces the link, target intact: {}", {
        let t = root.join("t.txt");
        write(&t, "T");
        let result = errno(unsafe { libc::rename(c(&t).as_ptr(), c(&link).as_ptr()) });
        let meta = fs::symlink_metadata(&link).unwrap();
        format!("{result}, link_is_symlink={} a={}", meta.file_type().is_symlink(), read(&a))
    });
    println!("renamex NOFOLLOW_ANY through parent symlink: {}", errno(renamex(&dir_link.join("f"), &dir_link.join("g"), RENAME_NOFOLLOW_ANY)));

    // Identity.
    let f1 = root.join("id.txt");
    write(&f1, "one");
    let before = fs::metadata(&f1).unwrap();
    let held = File::open(&f1).unwrap();
    let f2 = root.join("id-renamed.txt");
    fs::rename(&f1, &f2).unwrap();
    let after = fs::metadata(&f2).unwrap();
    println!("ino survives rename: {}", before.ino() == after.ino() && before.dev() == after.dev());
    println!("F_GETPATH follows a renamed fd: {}", fd_path(held.as_raw_fd()).ends_with("id-renamed.txt"));
    let tmp = root.join("id.tmp");
    write(&tmp, "two");
    let tmp_ino = fs::metadata(&tmp).unwrap().ino();
    fs::rename(&tmp, &f2).unwrap();
    println!("replace changes ino to the temporary's: {}", fs::metadata(&f2).unwrap().ino() == tmp_ino && tmp_ino != before.ino());
    let mut old = String::new();
    (&held).read_to_string(&mut old).unwrap();
    println!("held fd reads the replaced file: {old:?}");
    fs::remove_file(&f2).unwrap();
    println!("held fd after unlink: fstat nlink={}", held.metadata().unwrap().nlink());
    drop(held);

    // Directory fds keep operating on the moved directory.
    let moving = root.join("moving");
    fs::create_dir(&moving).unwrap();
    let dirfd = open_raw(&moving, libc::O_RDONLY | libc::O_DIRECTORY).unwrap();
    fs::rename(&moving, root.join("moved")).unwrap();
    let fd = unsafe { libc::openat(dirfd.as_raw_fd(), name("inside").as_ptr(), libc::O_CREAT | libc::O_WRONLY | libc::O_EXCL, 0o644) };
    println!("openat via dirfd after directory moved: {}, landed_in_moved={}", if fd >= 0 { "ok" } else { "failed" }, root.join("moved/inside").exists());
    if fd >= 0 {
        unsafe { libc::close(fd) };
    }

    // Instance lock on the directory itself: nothing created on disk.
    let lock1 = open_raw(&root, libc::O_RDONLY | libc::O_DIRECTORY).unwrap();
    println!("flock(EX|NB) on directory fd: {}", errno(unsafe { libc::flock(lock1.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) }));
    let lock2 = open_raw(&root, libc::O_RDONLY | libc::O_DIRECTORY).unwrap();
    println!("second description flock(EX|NB): {}", errno(unsafe { libc::flock(lock2.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) }));
    drop(lock1);
    println!("after first closes, flock(EX|NB): {}", errno(unsafe { libc::flock(lock2.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) }));
    drop(lock2);

    // Exclusive lock does not stop a writer that does not ask.
    let locked = root.join("locked.txt");
    write(&locked, "orig");
    match open_raw(&locked, libc::O_RDWR | libc::O_EXLOCK | libc::O_NONBLOCK) {
        Ok(holder) => {
            let other = OpenOptions::new().write(true).truncate(true).open(&locked).map(|mut f| f.write_all(b"written"));
            println!("O_EXLOCK held; unlocked writer: {other:?}, content={}", read(&locked));
            let renamed = root.join("locked2.txt");
            println!("O_EXLOCK held; rename by path: {}, unlink: {}", errno(unsafe { libc::rename(c(&locked).as_ptr(), c(&renamed).as_ptr()) }), errno(unsafe { libc::unlink(c(&renamed).as_ptr()) }));
            drop(holder);
        }
        Err(error) => println!("O_EXLOCK open: {error}"),
    }

    // The guard: swap in, then verify what was swapped out.
    let note = root.join("note.md");
    let temp = root.join("note.md.tmp");
    write(&note, "base");
    write(&temp, "mine");
    write(&note, "external"); // arrives after the pre-publish check of "base"
    let swap = renamex(&temp, &note, libc::RENAME_SWAP);
    let displaced = read(&temp);
    let caught = swap == 0 && displaced != "base";
    let undo = if caught { errno(renamex(&temp, &note, libc::RENAME_SWAP)) } else { "n/a".into() };
    println!("swap-then-verify catches a late replace: swap={} displaced={displaced:?} caught={caught} undo={undo} note={}", errno(swap), read(&note));
    write(&note, "base");
    write(&temp, "mine");
    let mut late_writer = OpenOptions::new().append(true).open(&note).unwrap();
    renamex(&temp, &note, libc::RENAME_SWAP);
    fs::remove_file(&temp).unwrap();
    let late = late_writer.write_all(b"+late").map(|_| "write returned ok");
    println!("in-place writer holding the old file after publish: {late:?}, note={} (its write is lost)", read(&note));
    fs::remove_file(&note).unwrap();

    // Durability.
    println!("fsync 20 KiB: {}", sync_cost(&root, 20 * 1024, false));
    println!("F_FULLFSYNC 20 KiB: {}", sync_cost(&root, 20 * 1024, true));
    println!("fsync 1 MiB: {}", sync_cost(&root, 1024 * 1024, false));
    println!("F_FULLFSYNC 1 MiB: {}", sync_cost(&root, 1024 * 1024, true));
    let d = open_raw(&root, libc::O_RDONLY | libc::O_DIRECTORY).unwrap();
    println!("fsync on directory fd: {}", errno(unsafe { libc::fsync(d.as_raw_fd()) }));
    println!("F_FULLFSYNC on directory fd: {}", errno(unsafe { libc::fcntl(d.as_raw_fd(), libc::F_FULLFSYNC) }));

    // Names.
    let cased = root.join("CaseName.txt");
    write(&cased, "x");
    let lower = root.join("casename.txt");
    println!("case-insensitive lookup: {}", lower.exists());
    if lower.exists() {
        println!("canonicalize(wrong case) ends with: {:?}", fs::canonicalize(&lower).unwrap().file_name().unwrap());
        let file = File::open(&lower).unwrap();
        println!("F_GETPATH(wrong case) ends with: {:?}", Path::new(&fd_path(file.as_raw_fd())).file_name().unwrap());
    }
    let nfc = root.join("caf\u{e9}.txt");
    let nfd = root.join("cafe\u{301}.txt");
    write(&nfc, "x");
    println!("normalization-insensitive lookup: {}", nfd.exists());
    let stored: Vec<_> = fs::read_dir(&root).unwrap().filter_map(|e| {
        let n = e.unwrap().file_name();
        n.to_string_lossy().starts_with("caf").then(|| n.as_bytes().to_vec())
    }).collect();
    println!("stored bytes of NFC name: {:?}", stored.iter().map(|b| String::from_utf8_lossy(b).chars().count()).collect::<Vec<_>>());
    let _ = OsStr::new("");
}
