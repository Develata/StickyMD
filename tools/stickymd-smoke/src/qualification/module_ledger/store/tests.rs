//! Store cleanup must keep everything still referenced and never act on partial scans.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// A record whose digest matches the `{}` evidence these tests archive.
fn record_json(evidence: &str, recorded_at: u64) -> String {
    let digest = crate::integrity::sha256_bytes(b"{}").unwrap();
    format!(
        "{{\"evidence_file\":\"{evidence}\",\"evidence_sha256\":\"{digest}\",\"recorded_at_unix\":{recorded_at}}}"
    )
}

/// One record per index; `recorded_at_unix` equals the index so newer is larger.
fn write_records(store: &LedgerStore, count: usize) {
    for index in 1..=count {
        let evidence = format!("g4-{}.json", digest(index));
        fs::create_dir_all(store.root().join("evidence")).unwrap();
        fs::write(store.evidence(&evidence).unwrap(), b"{}").unwrap();
        let record = store.module_record("g4", &digest(index)).unwrap();
        fs::create_dir_all(record.parent().unwrap()).unwrap();
        fs::write(record, record_json(&evidence, index as u64)).unwrap();
    }
}

#[test]
fn evidence_that_no_longer_matches_its_record_stops_reference_scans() {
    let temp = TempStore::new();
    let store = temp.store();
    write_records(&store, 2);
    assert_eq!(store.module_evidence_documents("g4").unwrap().len(), 2);
    // Still valid JSON, but not the archived bytes: it must not shrink the reference set.
    fs::write(
        store.evidence(&format!("g4-{}.json", digest(1))).unwrap(),
        b"{\"results\":[]}",
    )
    .unwrap();
    assert!(store.module_evidence_documents("g4").is_err());
    // A record that is not complete JSON stops pruning as well.
    let record = store.module_record("g4", &digest(2)).unwrap();
    let text = fs::read_to_string(&record).unwrap();
    fs::write(&record, text.trim_end_matches('}')).unwrap();
    let guard = store.write_guard().unwrap();
    assert!(store.prune_module(&guard, "g4", &record).is_err());
}

#[test]
fn pruning_keeps_the_newest_records_and_exactly_their_evidence() {
    let temp = TempStore::new();
    let store = temp.store();
    write_records(&store, RETAINED_RECORDS + 3);
    // Another module's evidence shares the directory and must survive.
    fs::write(
        store.evidence(&format!("g5-{}.json", digest(99))).unwrap(),
        b"{}",
    )
    .unwrap();
    let guard = store.write_guard().unwrap();
    let newest = store
        .module_record("g4", &digest(RETAINED_RECORDS + 3))
        .unwrap();
    store.prune_module(&guard, "g4", &newest).unwrap();
    for index in 1..=RETAINED_RECORDS + 3 {
        let kept = index > 3;
        assert_eq!(
            store.module_record("g4", &digest(index)).unwrap().is_file(),
            kept,
            "record {index}"
        );
        assert_eq!(
            store
                .evidence(&format!("g4-{}.json", digest(index)))
                .unwrap()
                .is_file(),
            kept,
            "evidence {index}"
        );
    }
    assert!(
        store
            .evidence(&format!("g5-{}.json", digest(99)))
            .unwrap()
            .is_file()
    );
}

#[test]
fn an_unreadable_record_stops_cleanup_without_deleting_anything() {
    let temp = TempStore::new();
    let store = temp.store();
    write_records(&store, RETAINED_RECORDS + 2);
    let corrupt = store.module_record("g4", &digest(5)).unwrap();
    fs::write(&corrupt, b"not a record").unwrap();
    let orphan = store.evidence(&format!("g4-{}.json", digest(77))).unwrap();
    fs::write(&orphan, b"{}").unwrap();
    let guard = store.write_guard().unwrap();
    let newest = store
        .module_record("g4", &digest(RETAINED_RECORDS + 2))
        .unwrap();
    assert!(store.prune_module(&guard, "g4", &newest).is_err());
    for index in 1..=RETAINED_RECORDS + 2 {
        assert!(store.module_record("g4", &digest(index)).unwrap().is_file());
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
    write_records(&store, RETAINED_RECORDS + 2);
    // Equal seconds: the published record sorts last by path among the ties.
    for index in 1..=RETAINED_RECORDS + 2 {
        let record = store.module_record("g4", &digest(index)).unwrap();
        let text = fs::read_to_string(&record).unwrap();
        fs::write(
            &record,
            text.replace(
                &format!("\"recorded_at_unix\":{index}}}"),
                "\"recorded_at_unix\":5}",
            ),
        )
        .unwrap();
    }
    let published = store
        .module_record("g4", &digest(RETAINED_RECORDS + 2))
        .unwrap();
    let guard = store.write_guard().unwrap();
    store.prune_module(&guard, "g4", &published).unwrap();
    assert!(
        published.is_file(),
        "a published record is never pruned on a tie"
    );

    // A clock moved backwards: the newest publication carries the oldest timestamp.
    let rolled_back = store.module_record("g4", &digest(500)).unwrap();
    fs::write(
        store.evidence(&format!("g4-{}.json", digest(500))).unwrap(),
        b"{}",
    )
    .unwrap();
    fs::write(
        &rolled_back,
        record_json(&format!("g4-{}.json", digest(500)), 0),
    )
    .unwrap();
    store.prune_module(&guard, "g4", &rolled_back).unwrap();
    assert!(rolled_back.is_file());
    assert!(
        store
            .evidence(&format!("g4-{}.json", digest(500)))
            .unwrap()
            .is_file()
    );
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
    assert!(probe.try_lock().is_ok());
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
