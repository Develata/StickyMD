//! What the FSEvents backend reports for the note directory. `notify_probe <empty-dir>`.
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};

fn drain(rx: &mpsc::Receiver<notify::Result<notify::Event>>, label: &str, start: Instant) {
    let deadline = Duration::from_millis(1500);
    let mut first = None;
    let mut seen = Vec::new();
    while let Ok(event) = rx.recv_timeout(deadline.saturating_sub(start.elapsed())) {
        first.get_or_insert(start.elapsed());
        match event {
            Ok(event) => {
                let names: Vec<_> = event
                    .paths
                    .iter()
                    .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                    .collect();
                seen.push(format!("{:?} {names:?}", event.kind));
            }
            Err(error) => seen.push(format!("error {error}")),
        }
    }
    println!("{label}: first_event={first:?} count={} {seen:?}", seen.len());
}

fn main() {
    let root = std::env::args().nth(1).expect("empty directory");
    let note_dir = Path::new(&root).join("note");
    fs::create_dir_all(note_dir.join("images")).unwrap();
    let note = note_dir.join("note.md");
    fs::write(&note, "base").unwrap();
    std::thread::sleep(Duration::from_secs(2)); // keep the creation out of the stream
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx).unwrap();
    watcher.watch(&note_dir, RecursiveMode::NonRecursive).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    drain(&rx, "stale events right after watch", Instant::now());

    let start = Instant::now();
    fs::write(&note, "in place").unwrap();
    drain(&rx, "external in-place write", start);

    let temp = note_dir.join("note.md.tmp");
    fs::write(&temp, "replaced").unwrap();
    let start = Instant::now();
    fs::rename(&temp, &note).unwrap();
    drain(&rx, "atomic replace by rename", start);

    fs::write(&temp, "swapped").unwrap();
    let (from, to) = (
        std::ffi::CString::new(temp.as_os_str().as_bytes()).unwrap(),
        std::ffi::CString::new(note.as_os_str().as_bytes()).unwrap(),
    );
    let start = Instant::now();
    assert_eq!(unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_SWAP) }, 0);
    fs::remove_file(&temp).unwrap();
    drain(&rx, "swap then remove temporary", start);

    let start = Instant::now();
    fs::write(note_dir.join("images/inner.png"), "x").unwrap();
    drain(&rx, "write in images/ (non-recursive watch)", start);

    let start = Instant::now();
    fs::remove_file(&note).unwrap();
    drain(&rx, "note removed", start);

    let moved = Path::new(&root).join("moved");
    let start = Instant::now();
    fs::rename(&note_dir, &moved).unwrap();
    drain(&rx, "watched directory moved away", start);
    let start = Instant::now();
    fs::write(moved.join("after.md"), "x").unwrap();
    drain(&rx, "write in moved directory", start);
}
