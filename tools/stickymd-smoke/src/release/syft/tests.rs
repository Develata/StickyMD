use super::*;
use crate::{
    integrity,
    release::{json, temporary::TemporaryDirectory},
};
use std::fs;

#[test]
fn checksum_manifest_rejects_missing_duplicate_wrong_hash_and_unsafe_names() {
    let pin = Pin {
        version: "1.50.0",
        archive_hash: &"a".repeat(64),
        checksums_hash: &"b".repeat(64),
    };
    let valid = format!("{}  {}\n", pin.archive_hash, pin.name(Kind::Archive));
    cache::verify_manifest(&valid, &pin).unwrap();
    cache::verify_manifest(&valid.replace("  ", " *"), &pin).unwrap();
    for invalid in [
        String::new(),
        valid.repeat(2),
        valid.replace(&"a".repeat(64), &"c".repeat(64)),
        valid.replace("windows_amd64.zip", "linux_amd64.zip"),
        valid.replace("  syft", "  ../syft"),
        valid.replace(&"a".repeat(64), "abc"),
    ] {
        assert!(cache::verify_manifest(&invalid, &pin).is_err());
    }
}

#[test]
fn cache_state_binds_both_hashes_and_snapshot_survives_later_cache_changes() {
    let temporary = TemporaryDirectory::new("syft-test").unwrap();
    let root = temporary.path().join("中文 cache space");
    fs::create_dir(&root).unwrap();
    let source = root.join("download.zip");
    fs::write(&source, b"synthetic archive bytes").unwrap();
    let archive_hash = integrity::sha256(&source).unwrap();
    let manifest = format!("{archive_hash}  syft_1.50.0_windows_amd64.zip\n");
    let manifest_source = root.join("download.txt");
    fs::write(&manifest_source, manifest).unwrap();
    let checksums_hash = integrity::sha256(&manifest_source).unwrap();
    let pin = Pin {
        version: "1.50.0",
        archive_hash: &archive_hash,
        checksums_hash: &checksums_hash,
    };
    assert_eq!(
        cache::pending(&root, &pin).unwrap(),
        [Kind::Archive, Kind::Checksums]
    );
    let output = root.join("verified snapshot");
    let (plan, snapshot) = prepare_with_pin(&root, None, &output, &pin).unwrap();
    assert_eq!(plan.json(), plan_json(&root, None, &pin).unwrap());
    assert!(snapshot.is_none());
    assert!(!output.exists());
    cache::publish(&root, Kind::Archive, &source, &pin).unwrap();
    assert_eq!(cache::pending(&root, &pin).unwrap(), [Kind::Checksums]);
    cache::publish(&root, Kind::Checksums, &manifest_source, &pin).unwrap();
    assert!(cache::pending(&root, &pin).unwrap().is_empty());
    let archive = pin.path(&root, Kind::Archive);
    let checksums = pin.path(&root, Kind::Checksums);
    let (plan, snapshot) = prepare_with_pin(&root, None, &output, &pin).unwrap();
    assert_eq!(plan.json(), plan_json(&root, None, &pin).unwrap());
    let snapshot = snapshot.unwrap();
    fs::write(&archive, "corrupted cache").unwrap();
    assert_eq!(integrity::sha256(&snapshot).unwrap(), archive_hash);
    assert_eq!(cache::pending(&root, &pin).unwrap(), [Kind::Archive]);
    let failed = root.join("failed snapshot");
    assert!(cache::snapshot(&archive, &checksums, &failed, &pin).is_err());
    assert!(!failed.exists());
    fs::write(&source, "unverified download").unwrap();
    assert!(cache::publish(&root, Kind::Archive, &source, &pin).is_err());
    assert_eq!(fs::read_to_string(&archive).unwrap(), "corrupted cache");
    assert!(cache::publish(&root, Kind::Archive, &manifest_source, &pin).is_err());
    assert!(cache::snapshot(&archive, &checksums, &output, &pin).is_err());
    assert_eq!(integrity::sha256(&snapshot).unwrap(), archive_hash);
    fs::write(&checksums, "corrupted manifest").unwrap();
    assert_eq!(
        cache::pending(&root, &pin).unwrap(),
        [Kind::Archive, Kind::Checksums]
    );
}

#[test]
fn production_plan_keeps_pins_and_external_override_distinct() {
    let temporary = TemporaryDirectory::new("syft-plan").unwrap();
    let root = temporary.path();
    let plan = plan_json(root, None, &PIN).unwrap();
    let plan = json::parse(&plan).unwrap();
    assert_eq!(plan.field("version").unwrap().string().unwrap(), "1.50.0");
    assert_eq!(
        plan.field("download_attempts").unwrap().unsigned().unwrap(),
        3
    );
    assert_eq!(plan.field("downloads").unwrap().array().unwrap().len(), 2);
    assert!(plan_json(root, Some(&root.join("missing.exe")), &PIN).is_err());
    let external = root.join("provided 工具.exe");
    fs::write(&external, "caller supplied").unwrap();
    let plan = plan_json(root, Some(&external), &PIN).unwrap();
    let output = root.join("external snapshot");
    let (prepared, snapshot) = prepare_with_pin(root, Some(&external), &output, &PIN).unwrap();
    assert_eq!(prepared.json(), plan);
    assert!(snapshot.is_none());
    assert!(!output.exists());
    assert!(plan.contains("\"external\":true"));
    assert!(
        json::parse(&plan)
            .unwrap()
            .field("downloads")
            .unwrap()
            .array()
            .unwrap()
            .is_empty()
    );
    assert!(!root.join("target").exists());
}
