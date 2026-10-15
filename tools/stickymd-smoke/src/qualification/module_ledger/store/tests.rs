//! Store cleanup must keep everything still referenced and never act on partial scans.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::record::LedgerRecord;
use super::{LedgerStore, RETAINED_RECORDS};

struct TempStore(PathBuf);

impl TempStore {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "stickymd-ledger-store-{}-{nonce}",
            std::process::id()
        )))
    }

    fn store(&self) -> LedgerStore {
        LedgerStore::at(self.0.clone())
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        // Created exclusively by this test under the temp directory.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn digest(index: usize) -> String {
    format!("{index:064x}")
}

/// Distinct archived evidence per run, so every record names its own file.
fn evidence_bytes(index: usize) -> Vec<u8> {
    format!("{{\"run\":{index}}}").into_bytes()
}

/// A complete record for run `index`; newer runs carry larger timestamps in callers.
fn record(index: usize, recorded_at: u64) -> LedgerRecord {
    LedgerRecord {
        module_id: "g4".to_owned(),
        input_fingerprint: digest(index),
        origin_source_commit: "a".repeat(40),
        origin_version: "0.1.0".to_owned(),
        origin_exe_sha256: "c".repeat(64),
        origin_zip_sha256: "d".repeat(64),
        evidence_sha256: crate::integrity::sha256_bytes(&evidence_bytes(index)).unwrap(),
        recorded_at_unix: recorded_at,
    }
}

fn evidence_path(store: &LedgerStore, index: usize) -> PathBuf {
    store.evidence(&record(index, 0).evidence_file()).unwrap()
}

fn record_path(store: &LedgerStore, index: usize) -> PathBuf {
    store.module_record("g4", &digest(index)).unwrap()
}

fn write_record(store: &LedgerStore, index: usize, recorded_at: u64) {
    let evidence = evidence_path(store, index);
    fs::create_dir_all(evidence.parent().unwrap()).unwrap();
    fs::write(evidence, evidence_bytes(index)).unwrap();
    let path = record_path(store, index);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, record(index, recorded_at).render()).unwrap();
}

/// One record per index; `recorded_at_unix` equals the index so newer is larger.
fn write_records(store: &LedgerStore, count: usize) {
    for index in 1..=count {
        write_record(store, index, index as u64);
    }
}

#[test]
fn evidence_that_no_longer_matches_its_record_stops_reference_scans() {
    let temp = TempStore::new();
    let store = temp.store();
    write_records(&store, 2);
    assert_eq!(store.module_evidence_documents("g4").unwrap().len(), 2);
    // Still valid JSON, but not the archived bytes: it must not shrink the reference set.
    fs::write(evidence_path(&store, 1), b"{\"results\":[]}").unwrap();
    assert!(store.module_evidence_documents("g4").is_err());
    // A record that is not complete JSON stops pruning as well.
    let record = record_path(&store, 2);
    let text = fs::read_to_string(&record).unwrap();
    fs::write(&record, text.trim_end().trim_end_matches('}')).unwrap();
    let guard = store.write_guard().unwrap();
    assert!(store.prune_module(&guard, "g4", &record).is_err());
}

#[test]
fn pruning_keeps_the_newest_records_and_exactly_their_evidence() {
    let temp = TempStore::new();
    let store = temp.store();
    write_records(&store, RETAINED_RECORDS + 3);
    // Another module's evidence shares the directory and must survive.
    let other = store.evidence(&format!("g5-{}.json", digest(99))).unwrap();
    fs::write(&other, b"{}").unwrap();
    let guard = store.write_guard().unwrap();
    let newest = record_path(&store, RETAINED_RECORDS + 3);
    store.prune_module(&guard, "g4", &newest).unwrap();
    for index in 1..=RETAINED_RECORDS + 3 {
        let kept = index > 3;
        assert_eq!(record_path(&store, index).is_file(), kept, "record {index}");
        assert_eq!(
            evidence_path(&store, index).is_file(),
            kept,
            "evidence {index}"
        );
    }
    assert!(other.is_file());
}

#[test]
fn an_unreadable_record_stops_cleanup_without_deleting_anything() {
    let temp = TempStore::new();
    let store = temp.store();
    write_records(&store, RETAINED_RECORDS + 2);
    fs::write(record_path(&store, 5), b"not a record").unwrap();
    let orphan = store.evidence(&format!("g4-{}.json", digest(77))).unwrap();
    fs::write(&orphan, b"{}").unwrap();
    let guard = store.write_guard().unwrap();
    let newest = record_path(&store, RETAINED_RECORDS + 2);
    assert!(store.prune_module(&guard, "g4", &newest).is_err());
    for index in 1..=RETAINED_RECORDS + 2 {
        assert!(record_path(&store, index).is_file());
    }
    assert!(
        orphan.is_file(),
        "unknown references are never treated as unused"
    );
}

#[test]
fn the_record_just_published_survives_equal_or_older_timestamps() {
    let temp = TempStore::new();
    let store = temp.store();
    // Equal seconds: the published record sorts last by path among the ties.
    for index in 1..=RETAINED_RECORDS + 2 {
        write_record(&store, index, 5);
    }
    let published = record_path(&store, RETAINED_RECORDS + 2);
    let guard = store.write_guard().unwrap();
    store.prune_module(&guard, "g4", &published).unwrap();
    assert!(
        published.is_file(),
        "a published record is never pruned on a tie"
    );

    // A clock moved backwards: the newest publication carries the oldest timestamp.
    write_record(&store, 500, 0);
    let rolled_back = record_path(&store, 500);
    store.prune_module(&guard, "g4", &rolled_back).unwrap();
    assert!(rolled_back.is_file());
    assert!(evidence_path(&store, 500).is_file());
}
#[test]
fn readers_and_writers_exclude_each_other_through_the_os_lock() {
    let temp = TempStore::new();
    let store = temp.store();
    let reader = store.read_guard().unwrap();
    let probe = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(store.root().join(".lock"))
        .unwrap();
    assert!(probe.try_lock().is_err(), "a writer must wait for readers");
    assert!(probe.try_lock_shared().is_ok(), "readers share the lock");
    probe.unlock().unwrap();
    drop(reader);
    let writer = store.write_guard().unwrap();
    assert!(
        probe.try_lock_shared().is_err(),
        "readers wait for the writer"
    );
    drop(writer);
    // On Unix a lock belongs to the open file description. Another test spawning a
    // process at this moment holds a forked copy of the writer's descriptor until its
    // exec closes it, so the release can lag briefly; production waits up to LOCK_WAIT.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while probe.try_lock().is_err() {
        assert!(
            std::time::Instant::now() < deadline,
            "the writer's lock was never released"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(windows)]
#[test]
fn a_linked_lock_file_is_refused_before_it_is_opened() {
    let temp = TempStore::new();
    let store = temp.store();
    fs::create_dir_all(store.root()).unwrap();
    let outside = temp.0.with_extension("outside-lock");
    let link = store.root().join(".lock");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&outside)
        .stdout(std::process::Stdio::null())
        .status()
        .expect("start mklink");
    assert!(status.success(), "create junction");
    assert!(store.read_guard().is_err());
    assert!(store.write_guard().is_err());
    assert!(!outside.exists(), "the link target must not be created");
}

/// Opt-in measurement for the per-guard link scan (`--ignored --nocapture`).
#[test]
#[ignore = "explicit timing profile for the store link scan"]
fn link_scan_profile() {
    let temp = TempStore::new();
    let store = temp.store();
    for module in 0..12 {
        let directory = store.root().join("modules").join(format!("m{module}"));
        fs::create_dir_all(&directory).unwrap();
        for record in 0..8 {
            fs::write(directory.join(format!("{}.json", digest(record))), b"{}").unwrap();
        }
    }
    for kind in ["evidence", "artifacts"] {
        let directory = store.root().join(kind);
        fs::create_dir_all(&directory).unwrap();
        for index in 0..400 {
            fs::write(directory.join(format!("{kind}-{index}")), b"x").unwrap();
        }
    }
    let rounds = 50;
    let started = std::time::Instant::now();
    for _ in 0..rounds {
        drop(store.read_guard().unwrap());
    }
    println!(
        "LINK_SCAN_PROFILE entries={} mean_ms={:.3}",
        12 * 8 + 800,
        started.elapsed().as_secs_f64() * 1000.0 / f64::from(rounds)
    );
}

#[test]
fn keys_and_digests_are_validated_before_becoming_paths() {
    let store = TempStore::new().store();
    for key in ["", "../g4", "g4/..", "-g4", "g 4", "g4\\x"] {
        assert!(store.module_record(key, &digest(1)).is_err(), "{key}");
    }
    assert!(store.module_record("g4", "../escape").is_err());
    assert!(store.evidence("g4-../x.json").is_err());
    assert!(store.evidence("g4-short.json").is_err());
    assert!(store.artifact("not-hex").is_err());
}
