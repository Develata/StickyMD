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

/// One record per index; `recorded_at_unix` equals the index so newer is larger.
fn write_records(store: &LedgerStore, count: usize) {
    for index in 1..=count {
        let evidence = format!("g4-{}.json", digest(index));
        fs::create_dir_all(store.root().join("evidence")).unwrap();
        fs::write(store.evidence(&evidence).unwrap(), b"{}").unwrap();
        let record = store.module_record("g4", &digest(index)).unwrap();
        fs::create_dir_all(record.parent().unwrap()).unwrap();
        fs::write(
            record,
            format!("{{\"evidence_file\":\"{evidence}\",\"recorded_at_unix\":{index}}}"),
        )
        .unwrap();
    }
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
    store.prune_module(&guard, "g4").unwrap();
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
    assert!(store.prune_module(&guard, "g4").is_err());
    for index in 1..=RETAINED_RECORDS + 2 {
        assert!(store.module_record("g4", &digest(index)).unwrap().is_file());
    }
    assert!(
        orphan.is_file(),
        "unknown references are never treated as unused"
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
