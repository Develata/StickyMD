//! Phase 1E spike — portable persistence primitives (directory identity,
//! single-instance, writable check, atomic save, crash recovery, conflict rules).
//!
//! plan_ref: docs/plan/05_document_persistence.md
//!
//! Spike code, deletable. Run `cargo run --release` for the end-to-end demo,
//! `cargo test` for the logic + filesystem integration tests.

mod logic;
#[cfg(windows)]
mod win32;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use logic::{
    decide_external_change, decide_recovery, detect_newline_style, identity_name, to_disk,
    ExternalChangeAction, NewlineStyle,
};

const NOTE: &str = "note.md";

/// Portable atomic save: write temp -> FlushFileBuffers -> ReplaceFileW/MoveFileExW.
fn atomic_save(dir: &Path, filename: &str, content: &[u8]) -> std::io::Result<()> {
    let target = dir.join(filename);
    let temp = dir.join(format!("{filename}.tmp"));
    {
        let mut f = fs::File::create(&temp)?;
        f.write_all(content)?;
        #[cfg(windows)]
        win32::flush_file_buffers(&f)?;
        #[cfg(not(windows))]
        f.sync_all()?;
    }
    #[cfg(windows)]
    win32::atomic_replace(&target, &temp)?;
    #[cfg(not(windows))]
    fs::rename(&temp, &target)?;
    // Defensive: ensure no temp survives a successful replace.
    let _ = fs::remove_file(&temp);
    Ok(())
}

/// Writable check per plan 05: create dir, create+write+flush+delete a probe file.
fn writable_check(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let probe = dir.join(".spike-writetest");
    {
        let mut f = fs::File::create(&probe)?;
        f.write_all(b"stickymd-writable-probe")?;
        f.sync_all()?;
    }
    fs::remove_file(&probe)?;
    Ok(())
}

fn read_hash(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|b| logic::sha256_hex(&b))
}

fn unique_base(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("stickymd-spike-{tag}-{}-{nanos}", std::process::id()))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--second-instance-probe") {
        std::process::exit(second_instance_probe());
    }
    run_demo();
}

/// Child mode: pretend to be a second instance launched from the same directory.
#[cfg(windows)]
fn second_instance_probe() -> i32 {
    let mutex_name = std::env::var("SPIKE_MUTEX").unwrap_or_default();
    let event_name = std::env::var("SPIKE_EVENT").unwrap_or_default();
    match win32::try_acquire_instance(&mutex_name) {
        Ok(_guard) => {
            println!("PROBE=FIRST (unexpected: no first instance held the mutex)");
            0
        }
        Err(()) => {
            println!("PROBE=SECOND_INSTANCE_DETECTED");
            match win32::signal_activate_event(&event_name) {
                Ok(()) => println!("PROBE=ACTIVATE_EVENT_SIGNALED"),
                Err(e) => println!("PROBE=ACTIVATE_EVENT_FAILED {e}"),
            }
            0
        }
    }
}
#[cfg(not(windows))]
fn second_instance_probe() -> i32 {
    println!("PROBE=SKIPPED (non-windows)");
    0
}

fn run_demo() {
    println!("=== Phase 1E persistence spike ===");
    let base = unique_base("demo");
    let note_dir = base.join("note");
    fs::create_dir_all(&note_dir).expect("create base");

    demo_identity(&base, &note_dir);
    demo_writable(&base);
    demo_single_instance(&base);
    demo_atomic_save(&note_dir);
    demo_recovery(&note_dir);
    demo_conflict_rules();

    let _ = fs::remove_dir_all(&base);
    println!("=== demo complete ===");
}

#[cfg(windows)]
fn demo_identity(base: &Path, note_dir: &Path) {
    println!("\n[1] canonical directory identity");
    let canon_base = win32::canonical_dir(base).expect("canonical base");
    let canon_note = win32::canonical_dir(note_dir).expect("canonical note");
    let id_base = identity_name(&canon_base);
    let id_note = identity_name(&canon_note);
    let id_base_again = identity_name(&win32::canonical_dir(base).unwrap());
    println!("  canonical(base) = {canon_base}");
    println!("  id(base)        = {id_base}");
    println!("  stable(repeat)  = {}", id_base == id_base_again);
    println!("  distinct(subdir)= {}", id_base != id_note);
}
#[cfg(not(windows))]
fn demo_identity(_base: &Path, _note_dir: &Path) {
    println!("\n[1] canonical directory identity: SKIPPED (non-windows)");
}

fn demo_writable(base: &Path) {
    println!("\n[2] writable check");
    let ok_dir = base.join("note");
    match writable_check(&ok_dir) {
        Ok(()) => println!("  writable(note/)      = PASS"),
        Err(e) => println!("  writable(note/)      = FAIL {e}"),
    }
    // Failure injection: a path under an existing FILE cannot be created as a dir.
    let blocker = base.join("blocker.txt");
    fs::write(&blocker, b"file").unwrap();
    let bad_dir = blocker.join("note");
    match writable_check(&bad_dir) {
        Ok(()) => println!("  writable(blocked)    = FAIL (expected Err)"),
        Err(_) => println!("  writable(blocked)    = PASS (rejected as expected)"),
    }
}

#[cfg(windows)]
fn demo_single_instance(base: &Path) {
    println!("\n[3] single-instance (named mutex + activate event)");
    let canon = win32::canonical_dir(base).expect("canonical");
    let id = identity_name(&canon);
    let mutex_name = format!("{id}-mtx");
    let event_name = format!("{id}-evt");

    let _first = match win32::try_acquire_instance(&mutex_name) {
        Ok(g) => {
            println!("  first instance acquired mutex");
            g
        }
        Err(()) => {
            println!("  FAIL: could not acquire mutex as first instance");
            return;
        }
    };
    let evt = win32::create_activate_event(&event_name).expect("create event");

    let exe = std::env::current_exe().expect("current exe");
    let out = std::process::Command::new(&exe)
        .arg("--second-instance-probe")
        .env("SPIKE_MUTEX", &mutex_name)
        .env("SPIKE_EVENT", &event_name)
        .output()
        .expect("spawn probe");
    let probe_out = String::from_utf8_lossy(&out.stdout);
    for line in probe_out.lines() {
        println!("  child: {line}");
    }
    let signaled = win32::wait_activate_event(&evt, 1000);
    println!(
        "  first instance observed activate signal = {}",
        if signaled { "PASS" } else { "FAIL" }
    );
}
#[cfg(not(windows))]
fn demo_single_instance(_base: &Path) {
    println!("\n[3] single-instance: SKIPPED (non-windows)");
}

fn demo_atomic_save(note_dir: &Path) {
    println!("\n[4] atomic save");
    let v1 = b"# StickyMD\nfirst version\n";
    // Internal canonical form is \n; convert to the on-disk style (CRLF) on save.
    let v2_internal = "# StickyMD\nsecond version\nmore lines\n";
    let v2_disk = to_disk(v2_internal, NewlineStyle::Crlf);

    atomic_save(note_dir, NOTE, v1).expect("save v1");
    let h1 = read_hash(&note_dir.join(NOTE)).unwrap();
    let tmp = note_dir.join(format!("{NOTE}.tmp"));
    println!(
        "  v1 landed, no temp leftover = {}",
        h1 == logic::sha256_hex(v1) && !tmp.exists()
    );

    atomic_save(note_dir, NOTE, v2_disk.as_bytes()).expect("save v2");
    let h2 = read_hash(&note_dir.join(NOTE)).unwrap();
    println!(
        "  v2 replaced v1 atomically    = {}",
        h2 == logic::sha256_hex(v2_disk.as_bytes()) && !tmp.exists()
    );
    println!(
        "  newline style of saved v2    = {:?}",
        detect_newline_style(&fs::read_to_string(note_dir.join(NOTE)).unwrap_or_default())
    );
}

fn demo_recovery(note_dir: &Path) {
    println!("\n[5] crash recovery detection");
    let note = note_dir.join(NOTE);
    let tmp = note_dir.join(format!("{NOTE}.tmp"));
    let current = fs::read(&note).ok();

    // Case A: temp differs from current -> OfferRecovery.
    fs::write(&tmp, b"unsaved crash content").unwrap();
    let d = decide_recovery(Some(&fs::read(&tmp).unwrap()), current.as_deref());
    println!("  temp differs        -> {d:?} (expect OfferRecovery)");

    // Case B: temp identical -> CleanStale.
    let cur_bytes = fs::read(&note).unwrap();
    fs::write(&tmp, &cur_bytes).unwrap();
    let d = decide_recovery(Some(&fs::read(&tmp).unwrap()), Some(&cur_bytes));
    println!("  temp identical      -> {d:?} (expect CleanStale)");

    // Case C: invalid UTF-8 temp -> DiscardTemp.
    fs::write(&tmp, [0xFF, 0xFE, 0x00, 0x81]).unwrap();
    let d = decide_recovery(Some(&fs::read(&tmp).unwrap()), Some(&cur_bytes));
    println!("  temp invalid utf-8  -> {d:?} (expect DiscardTemp)");

    // Case D: no temp -> None.
    fs::remove_file(&tmp).unwrap();
    let d = decide_recovery(None, Some(&cur_bytes));
    println!("  no temp             -> {d:?} (expect None)");
}

fn demo_conflict_rules() {
    println!("\n[6] external change / conflict rules");
    let own = "hash-own";
    let ext = "hash-external";
    let cases = [
        ("own write echoed", decide_external_change(own, Some(own), true), ExternalChangeAction::Ignore),
        ("external + clean", decide_external_change(ext, Some(own), false), ExternalChangeAction::Reload),
        ("external + dirty", decide_external_change(ext, Some(own), true), ExternalChangeAction::Conflict),
    ];
    for (name, got, want) in cases {
        println!(
            "  {name:<18} -> {got:?} ({})",
            if got == want { "PASS" } else { "FAIL" }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_save_lands_and_cleans_temp() {
        let base = unique_base("t-save");
        let dir = base.join("note");
        fs::create_dir_all(&dir).unwrap();
        let content = b"hello atomic";
        atomic_save(&dir, NOTE, content).unwrap();
        let got = fs::read(dir.join(NOTE)).unwrap();
        assert_eq!(got, content);
        assert!(!dir.join(format!("{NOTE}.tmp")).exists());
        // overwrite
        atomic_save(&dir, NOTE, b"v2").unwrap();
        assert_eq!(fs::read(dir.join(NOTE)).unwrap(), b"v2");
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn writable_check_pass_and_fail() {
        let base = unique_base("t-writable");
        let ok = base.join("note");
        assert!(writable_check(&ok).is_ok());
        // impossible path under a file
        let blocker = base.join("f.txt");
        fs::write(&blocker, b"x").unwrap();
        assert!(writable_check(&blocker.join("note")).is_err());
        fs::remove_dir_all(&base).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn single_instance_second_is_detected() {
        let base = unique_base("t-single");
        fs::create_dir_all(&base).unwrap();
        let canon = win32::canonical_dir(&base).unwrap();
        let id = identity_name(&canon);
        let mtx = format!("{id}-mtx");
        let _first = win32::try_acquire_instance(&mtx).expect("first acquires");
        // A second acquisition in the SAME process also sees already-exists.
        assert!(win32::try_acquire_instance(&mtx).is_err());
        drop(_first);
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn newline_style_preserved_on_save() {
        let internal = logic::to_internal("a\r\nb\r\nc");
        assert_eq!(logic::to_disk(&internal, NewlineStyle::Crlf), "a\r\nb\r\nc");
        assert_eq!(detect_newline_style("a\r\nb"), NewlineStyle::Crlf);
    }

    #[test]
    fn recovery_decisions() {
        use logic::RecoveryDecision as RD;
        assert_eq!(decide_recovery(None, Some(b"c")), RD::None);
        assert_eq!(decide_recovery(Some(b"new"), Some(b"old")), RD::OfferRecovery);
        assert_eq!(decide_recovery(Some(b"same"), Some(b"same")), RD::CleanStale);
        assert_eq!(decide_recovery(Some(&[0xFFu8, 0xFE]), Some(b"c")), RD::DiscardTemp);
    }
}
