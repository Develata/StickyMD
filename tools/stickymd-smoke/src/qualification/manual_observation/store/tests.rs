//! Context lifecycle: declaration, reset, carry-over and every way a log can be lost.

use std::fs;
use std::path::{Path, PathBuf};

use super::{LoadError, OBSERVATION_LOG, declare, load, lock, reset};
use crate::qualification::manual_observation::log::{self, MAX_OBSERVATIONS, Status, fixtures};
use crate::qualification::manual_observation::target::Role::{Current, Previous};
use crate::qualification::manual_observation::target::{TARGET_RECEIPT, fixtures::target};

const CANDIDATE: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "stickymd-manual-store-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Created exclusively by this test under the temp directory.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn declared(root: &Path) -> String {
    declare(root, CANDIDATE, &promoted, target().roles, &none)
        .unwrap()
        .context_id
}

/// Appends observations to the current log as a recorder would.
fn record(
    root: &Path,
    entries: &[(
        u64,
        &str,
        &str,
        crate::qualification::manual_observation::target::Role,
        Status,
    )],
) {
    let mut context = load(root, CANDIDATE).unwrap();
    let start = context.log.observations.len() as u64;
    let batch = entries
        .iter()
        .enumerate()
        .map(|(offset, (base, case, scenario, role, status))| {
            let mut observation = fixtures::observation(
                start + offset as u64 + 1,
                *base,
                case,
                scenario,
                *role,
                *status,
            );
            observation.session = "11111111111111111111111111111111".to_owned();
            observation
        })
        .collect();
    context.log.append(batch).unwrap();
    crate::atomic_evidence::write(
        &root.join(OBSERVATION_LOG),
        log::render(&context.log).as_bytes(),
    )
    .unwrap();
}

fn none(_: &str) -> Result<bool, String> {
    Ok(false)
}

/// The candidate is still the one the command started with.
fn promoted() -> Result<String, String> {
    Ok(CANDIDATE.to_owned())
}

/// Approval of exactly the reset decision for the log as it is now.
/// Answers whether a decision key is approved, as the release decision receipt would.
type Approval = Box<dyn Fn(&str) -> Result<bool, String>>;

fn approval_for_current_log(root: &Path) -> Approval {
    let bytes = fs::read(root.join(OBSERVATION_LOG)).unwrap_or_default();
    let key = format!(
        "RESET-MANUAL-LOG-{}",
        crate::integrity::sha256_bytes(&bytes).unwrap()
    );
    Box::new(move |decision| Ok(decision == key))
}

#[test]
fn declaring_targets_creates_a_bound_empty_log() {
    let scratch = Scratch::new();
    assert_eq!(
        load(&scratch.0, CANDIDATE).unwrap_err(),
        LoadError::NoContext
    );
    let context_id = declared(&scratch.0);
    let context = load(&scratch.0, CANDIDATE).unwrap();
    assert_eq!(context.target.context_id, context_id);
    assert_eq!(context.log.header.context_id, context_id);
    assert!(context.log.observations.is_empty());
    assert_eq!(
        load(&scratch.0, &"d".repeat(64)).unwrap_err(),
        LoadError::OtherCandidate
    );
    // Nothing recorded yet: re-declaring replaces the context without a decision.
    assert_ne!(declared(&scratch.0), context_id);
}

#[test]
fn a_context_with_observations_is_replaced_only_after_an_approved_reset() {
    let scratch = Scratch::new();
    let old = declared(&scratch.0);
    record(
        &scratch.0,
        &[
            (
                0,
                "P12-M24",
                "representative-markdown",
                Current,
                Status::Fail,
            ),
            (
                0,
                "P12-M24",
                "representative-markdown",
                Current,
                Status::Fail,
            ),
            (0, "P12-M25", "formulas", Current, Status::Pass),
            (0, "P12-M26", "image-formats", Previous, Status::Fail),
        ],
    );
    // A retest that saw the M26 FAIL (log length 4) resolves it.
    record(
        &scratch.0,
        &[(4, "P12-M26", "image-formats", Previous, Status::Pass)],
    );
    let error = declare(&scratch.0, CANDIDATE, &promoted, target().roles, &none).unwrap_err();
    assert!(error.contains("RESET-MANUAL-LOG-"), "{error}");
    let error = reset(&scratch.0, CANDIDATE, &promoted, &none).unwrap_err();
    assert!(error.contains("RESET-MANUAL-LOG-"), "{error}");
    assert_eq!(load(&scratch.0, CANDIDATE).unwrap().target.context_id, old);

    let approval = approval_for_current_log(&scratch.0);
    let new = reset(&scratch.0, CANDIDATE, &promoted, &*approval)
        .unwrap()
        .context_id;
    assert_ne!(new, old);
    let context = load(&scratch.0, CANDIDATE).unwrap();
    let reset_info = context.log.header.reset.as_ref().unwrap();
    assert_eq!(reset_info.previous_context_id, old);
    // The two unresolved M24 FAILs merge into one; the resolved M26 FAIL is not carried.
    let [carried] = context.log.observations.as_slice() else {
        panic!("{:?}", context.log.observations);
    };
    assert_eq!(
        (carried.case.as_str(), carried.status, carried.base),
        ("P12-M24", Status::Fail, 0)
    );
    assert_eq!(
        carried
            .carried_from
            .iter()
            .map(|origin| (origin.context_id.as_str(), origin.index))
            .collect::<Vec<_>>(),
        vec![(old.as_str(), 0), (old.as_str(), 1)]
    );
    // The replaced files are archived, not deleted.
    let archived = fs::read_dir(scratch.0.join("dist/evidence/manual-archive"))
        .unwrap()
        .count();
    assert_eq!(archived, 1);
    // A decision approves only the exact bytes it names.
    assert!(reset(&scratch.0, CANDIDATE, &promoted, &*approval).is_err());
}

#[test]
fn a_full_log_resets_into_a_context_with_room_to_retest() {
    let scratch = Scratch::new();
    declared(&scratch.0);
    let mut context = load(&scratch.0, CANDIDATE).unwrap();
    context.log.observations = (1..=MAX_OBSERVATIONS as u64)
        .map(|seq| fixtures::observation(seq, 0, "P12-M41", "sleep-resume", Current, Status::Fail))
        .collect();
    crate::atomic_evidence::write(
        &scratch.0.join(OBSERVATION_LOG),
        log::render(&context.log).as_bytes(),
    )
    .unwrap();
    let approval = approval_for_current_log(&scratch.0);
    reset(&scratch.0, CANDIDATE, &promoted, &*approval).unwrap();
    let context = load(&scratch.0, CANDIDATE).unwrap();
    assert_eq!(context.log.observations.len(), 1);
    assert_eq!(
        context.log.observations[0].carried_from.len(),
        MAX_OBSERVATIONS
    );
    record(
        &scratch.0,
        &[(1, "P12-M41", "sleep-resume", Current, Status::Pass)],
    );
}

/// The same log with a well-formed but different registry digest.
fn other_registry(good: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(good.to_vec()).unwrap();
    let key = "\"registry_sha256\":\"";
    let start = text.find(key).unwrap() + key.len();
    format!(
        "{}{}{}",
        &text[..start],
        "f".repeat(64),
        &text[start + 64..]
    )
    .into_bytes()
}

#[test]
fn a_missing_empty_corrupt_or_mismatched_log_is_lost_not_fresh() {
    let scratch = Scratch::new();
    declared(&scratch.0);
    let log_path = scratch.0.join(OBSERVATION_LOG);
    let good = fs::read(&log_path).unwrap();
    for (label, bytes) in [
        ("missing", None),
        ("zero bytes", Some(Vec::new())),
        ("corrupt", Some(b"{\"schema_version\":1".to_vec())),
        ("other registry", Some(other_registry(&good))),
    ] {
        match &bytes {
            None => fs::remove_file(&log_path).unwrap(),
            Some(bytes) => fs::write(&log_path, bytes).unwrap(),
        }
        let error = load(&scratch.0, CANDIDATE).unwrap_err();
        assert!(matches!(error, LoadError::Broken(_)), "{label}: {error:?}");
        fs::write(&log_path, &good).unwrap();
    }
    // A target edited after the log was written no longer matches its digest.
    let target_path = scratch.0.join(TARGET_RECEIPT);
    let text = fs::read_to_string(&target_path).unwrap();
    fs::write(&target_path, text.replacen("2026-10-14", "2026-10-15", 1)).unwrap();
    assert!(matches!(
        load(&scratch.0, CANDIDATE).unwrap_err(),
        LoadError::Broken(_)
    ));
    // Replacing a lost context needs the reset decision for its current bytes.
    let error = declare(&scratch.0, CANDIDATE, &promoted, target().roles, &none).unwrap_err();
    assert!(error.contains("RESET-MANUAL-LOG-"), "{error}");
}

#[test]
fn an_interrupted_declaration_leaves_a_replaceable_orphan() {
    let scratch = Scratch::new();
    declared(&scratch.0);
    fs::remove_file(scratch.0.join(TARGET_RECEIPT)).unwrap();
    assert!(matches!(
        load(&scratch.0, CANDIDATE).unwrap_err(),
        LoadError::Broken(_)
    ));
    declared(&scratch.0);
    load(&scratch.0, CANDIDATE).unwrap();
}

#[test]
fn shared_readers_coexist_and_exclude_a_writer() {
    let scratch = Scratch::new();
    let first = lock(&scratch.0, false).unwrap();
    let second = lock(&scratch.0, false).unwrap();
    let probe = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(scratch.0.join("dist/evidence/manual.lock"))
        .unwrap();
    assert!(matches!(
        probe.try_lock(),
        Err(fs::TryLockError::WouldBlock)
    ));
    drop((first, second));
    // A Unix flock belongs to the open file description, which a child forked by another
    // test holds until it execs; the release becomes visible within a bounded wait.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match probe.try_lock() {
            Ok(()) => break,
            Err(fs::TryLockError::WouldBlock) => {
                assert!(std::time::Instant::now() < deadline, "lock not released");
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(fs::TryLockError::Error(error)) => panic!("lock failed: {error}"),
        }
    }
}

#[test]
fn a_half_replaced_context_blocks_and_still_carries_its_failures() {
    let scratch = Scratch::new();
    declared(&scratch.0);
    record(
        &scratch.0,
        &[(0, "P12-M25", "formulas", Current, Status::Fail)],
    );
    // An interrupted replacement: a new log beside the old target.
    let log_path = scratch.0.join(OBSERVATION_LOG);
    let mut half = log::parse(&fs::read_to_string(&log_path).unwrap()).unwrap();
    half.header.context_id = "abcdefabcdefabcdefabcdefabcdefab".to_owned();
    crate::atomic_evidence::write(&log_path, log::render(&half).as_bytes()).unwrap();
    assert!(matches!(
        load(&scratch.0, CANDIDATE).unwrap_err(),
        LoadError::Broken(_)
    ));
    assert!(declare(&scratch.0, CANDIDATE, &promoted, target().roles, &none).is_err());
    let approval = approval_for_current_log(&scratch.0);
    declare(&scratch.0, CANDIDATE, &promoted, target().roles, &*approval).unwrap();
    let context = load(&scratch.0, CANDIDATE).unwrap();
    assert_eq!(context.log.observations.len(), 1);
    assert_eq!(context.log.observations[0].case, "P12-M25");
}

/// A real failure between the two writes: the target receipt is read-only, so replacing it
/// fails after the new log is in place. Nothing reads as fresh, and the FAIL survives.
#[cfg(windows)]
#[test]
fn a_failed_target_write_blocks_and_keeps_the_failures() {
    let scratch = Scratch::new();
    declared(&scratch.0);
    record(
        &scratch.0,
        &[(0, "P12-M25", "formulas", Current, Status::Fail)],
    );
    let target_path = scratch.0.join(TARGET_RECEIPT);
    let old_target = fs::read(&target_path).unwrap();
    let writable = fs::metadata(&target_path).unwrap().permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&target_path, read_only).unwrap();
    let approval = approval_for_current_log(&scratch.0);
    assert!(reset(&scratch.0, CANDIDATE, &promoted, &*approval).is_err());
    assert_eq!(fs::read(&target_path).unwrap(), old_target);
    assert!(matches!(
        load(&scratch.0, CANDIDATE).unwrap_err(),
        LoadError::Broken(_)
    ));
    fs::set_permissions(&target_path, writable).unwrap();
    let approval = approval_for_current_log(&scratch.0);
    declare(&scratch.0, CANDIDATE, &promoted, target().roles, &*approval).unwrap();
    let context = load(&scratch.0, CANDIDATE).unwrap();
    assert_eq!(context.log.observations.len(), 1);
    assert_eq!(context.log.observations[0].case, "P12-M25");
}

#[test]
fn an_unreadable_approval_is_reported_not_mistaken_for_a_missing_one() {
    let scratch = Scratch::new();
    declared(&scratch.0);
    record(
        &scratch.0,
        &[(0, "P12-M25", "formulas", Current, Status::Fail)],
    );
    let error = reset(&scratch.0, CANDIDATE, &promoted, &|_| {
        Err("release decision receipt stayed locked".to_owned())
    })
    .unwrap_err();
    assert!(error.contains("stayed locked"), "{error}");
}

#[test]
fn the_public_evidence_writer_cannot_target_the_store() {
    let scratch = Scratch::new();
    declared(&scratch.0);
    let log_bytes = fs::read(scratch.0.join(OBSERVATION_LOG)).unwrap();
    for path in [
        OBSERVATION_LOG,
        TARGET_RECEIPT,
        "dist/evidence/manual.lock",
        "dist/evidence/manual-archive/any/manual-observations.json",
        "dist/evidence/./manual-observations.json",
        "dist/evidence/release-decisions.json",
        "dist/evidence/release-decisions.lock",
        "dist/evidence/manual.lock/x.json",
        "dist/evidence/release-decisions.lock/x.json",
        // Creating the parents would create the lock file as a directory.
        "dist/evidence/manual.lock/../diagnostic.json",
        "dist/evidence/release-decisions.lock/../diagnostic.json",
    ] {
        assert!(
            crate::qualification::validate_public_evidence_path(&scratch.0, Path::new(path))
                .is_err(),
            "{path}"
        );
    }
    assert_eq!(
        fs::read(scratch.0.join(OBSERVATION_LOG)).unwrap(),
        log_bytes
    );
    crate::qualification::validate_public_evidence_path(
        &scratch.0,
        Path::new("dist/evidence/diagnostic.json"),
    )
    .unwrap();
}

#[test]
fn a_declaration_outlived_by_a_promote_changes_nothing() {
    let scratch = Scratch::new();
    let context = declared(&scratch.0);
    record(
        &scratch.0,
        &[(0, "P12-M25", "formulas", Current, Status::Fail)],
    );
    let before = fs::read(scratch.0.join(OBSERVATION_LOG)).unwrap();
    // Started for an older candidate; the current one is CANDIDATE.
    let stale = "e".repeat(64);
    let error = declare(&scratch.0, &stale, &promoted, target().roles, &none).unwrap_err();
    assert!(error.contains("candidate changed"), "{error}");
    let error = reset(&scratch.0, &stale, &promoted, &none).unwrap_err();
    assert!(error.contains("candidate changed"), "{error}");
    assert_eq!(fs::read(scratch.0.join(OBSERVATION_LOG)).unwrap(), before);
    assert_eq!(
        load(&scratch.0, CANDIDATE).unwrap().target.context_id,
        context
    );
}

#[cfg(unix)]
#[test]
fn linked_store_files_are_refused() {
    let scratch = Scratch::new();
    declared(&scratch.0);
    let log = scratch.0.join(OBSERVATION_LOG);
    let elsewhere = scratch.0.join("elsewhere.json");
    fs::rename(&log, &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &log).unwrap();
    let error = load(&scratch.0, CANDIDATE).unwrap_err();
    assert!(
        matches!(&error, LoadError::Broken(reason) if reason.contains("link")),
        "{error:?}"
    );
}

/// The reviewer's case: before any store file exists, a path that walks through the lock
/// file name would create it as a directory, so it is refused.
#[test]
fn a_path_through_a_missing_store_file_is_refused() {
    let scratch = Scratch::new();
    assert!(!scratch.0.join("dist/evidence").exists());
    for path in [
        "dist/evidence/manual.lock/../diagnostic.json",
        "dist/evidence/manual-observations.json/../x.json",
    ] {
        assert!(
            crate::qualification::validate_public_evidence_path(&scratch.0, Path::new(path))
                .is_err(),
            "{path}"
        );
    }
    crate::qualification::validate_public_evidence_path(
        &scratch.0,
        Path::new("dist/evidence/./diagnostic.json"),
    )
    .unwrap();
}
