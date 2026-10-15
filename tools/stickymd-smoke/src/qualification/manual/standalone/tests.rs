//! The standalone recorder's bundle file: claimed once, rewritten after every answer,
//! never past the import limit, and kept out of a repository's store.

use std::fs;
use std::path::PathBuf;

use super::{BundleFile, guard_inside_repository};
use crate::qualification::manual_observation::log::{Note, Status, fixtures};
use crate::qualification::manual_observation::store::{Commit, OBSERVATION_LOG};
use crate::qualification::manual_observation::target::Role;
use crate::qualification::manual_observation::transfer::{Bundle, parse_bundle};

/// A directory this test creates exclusively and removes on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "stickymd-bundle-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Created exclusively by this test.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn empty() -> Bundle {
    Bundle {
        task_sha256: "a".repeat(64),
        context_id: "0123456789abcdef0123456789abcdef".to_owned(),
        candidate_receipt_sha256: "c".repeat(64),
        target_sha256: "d".repeat(64),
        registry_sha256: "e".repeat(64),
        base: 0,
        session: fixtures::SESSION.to_owned(),
        observations: Vec::new(),
        notes: Vec::new(),
    }
}

fn on_disk(file: &BundleFile) -> Bundle {
    parse_bundle(&fs::read_to_string(&file.path).unwrap()).unwrap()
}

#[test]
fn every_answer_is_on_disk_before_the_next_question() {
    let scratch = Scratch::new("answers");
    let path = scratch.0.join("bundle.json");
    let mut file = BundleFile::claim(path.clone(), empty()).unwrap();
    assert_eq!(on_disk(&file), empty());
    let fail = fixtures::observation(1, 0, "P12-M25", "formulas", Role::Current, Status::Fail);
    assert_eq!(
        file.add_observation(fail.clone(), &mut || Ok(None)),
        Ok(Commit::Written)
    );
    assert_eq!(on_disk(&file).observations, vec![fail]);
    let note = Note {
        session: fixtures::SESSION.to_owned(),
        seq: 1,
        text: "matrix misaligned".to_owned(),
    };
    file.add_note(note.clone()).unwrap();
    assert_eq!(on_disk(&file).notes, vec![note]);
    // An existing bundle is never claimed again.
    assert!(BundleFile::claim(path, empty()).is_err());
    assert_eq!(on_disk(&file).observations.len(), 1);
}

#[test]
fn a_refused_or_oversized_answer_leaves_the_file_unchanged() {
    let scratch = Scratch::new("refused");
    let mut file = BundleFile::claim(scratch.0.join("bundle.json"), empty()).unwrap();
    let pass = || fixtures::observation(1, 0, "P12-M25", "formulas", Role::Current, Status::Pass);
    let before = fs::read(&file.path).unwrap();
    assert_eq!(
        file.add_observation(pass(), &mut || Ok(Some("instance exited".to_owned()))),
        Ok(Commit::Refused("instance exited".to_owned()))
    );
    assert_eq!(fs::read(&file.path).unwrap(), before);
    assert!(file.bundle.observations.is_empty());
    // Fill the bundle close to the limit in memory, then one more answer would cross it.
    file.bundle.notes = (0..520)
        .map(|seq| Note {
            session: fixtures::SESSION.to_owned(),
            seq,
            text: "x".repeat(2000),
        })
        .collect();
    let mut checked = false;
    let error = file
        .add_observation(pass(), &mut || {
            checked = true;
            Ok(None)
        })
        .unwrap_err();
    assert!(error.contains("would exceed"), "{error}");
    assert!(!checked, "the size is checked before the last check");
    assert_eq!(fs::read(&file.path).unwrap(), before);
}

/// A link outside every repository that leads into one still meets that repository's guard.
#[test]
fn a_link_into_a_repository_cannot_reach_its_store() {
    let scratch = Scratch::new("link");
    let repository = scratch.0.join("repository");
    let store = repository.join(OBSERVATION_LOG);
    let evidence = store.parent().unwrap();
    fs::create_dir_all(evidence).unwrap();
    for marker in ["Cargo.toml", "AGENTS.md"] {
        fs::write(repository.join(marker), "").unwrap();
    }
    let outside = scratch.0.join("outside");
    fs::create_dir(&outside).unwrap();
    let alias = outside.join("alias");
    link_directory(evidence, &alias);
    let through = alias.join(store.file_name().unwrap());
    let error = guard_inside_repository(&through).unwrap_err();
    assert!(error.contains("coordinator-owned"), "{error}");
    assert!(guard_inside_repository(&alias.join("bundle.json")).is_ok());
    assert!(guard_inside_repository(&outside.join("missing").join("bundle.json")).is_err());
}

#[cfg(windows)]
fn link_directory(target: &std::path::Path, link: &std::path::Path) {
    // Junctions need no privilege; mklink expects backslash spellings.
    let spelled = |path: &std::path::Path| path.to_string_lossy().replace('/', "\\");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(spelled(link))
        .arg(spelled(target))
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "create junction");
}

#[cfg(unix)]
fn link_directory(target: &std::path::Path, link: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[test]
fn a_bundle_inside_a_repository_cannot_replace_its_store() {
    let scratch = Scratch::new("inside");
    let repository = scratch.0.join("repository");
    fs::create_dir_all(repository.join(OBSERVATION_LOG).parent().unwrap()).unwrap();
    for marker in ["Cargo.toml", "AGENTS.md"] {
        fs::write(repository.join(marker), "").unwrap();
    }
    assert!(guard_inside_repository(&repository.join(OBSERVATION_LOG)).is_err());
    assert!(guard_inside_repository(&repository.join("manual-bundle.json")).is_ok());
    let outside = Scratch::new("outside");
    assert!(guard_inside_repository(&outside.0.join("bundle.json")).is_ok());
}
