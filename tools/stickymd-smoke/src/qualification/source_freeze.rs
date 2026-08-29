//! Clean source identity used before a remote release artifact exists.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::path::Path;

use super::{json, receipt};

pub(super) const SOURCE_FREEZE_RECEIPT: &str = "dist/evidence/release-source-freeze.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SourceFreeze {
    pub(super) source_commit: String,
    pub(super) version: String,
    pub(super) cargo_lock_sha256: String,
    pub(super) rustc: String,
    pub(super) target: String,
    pub(super) remote_synced: bool,
}

pub(super) fn create(root: &Path) -> Result<SourceFreeze, String> {
    receipt::ensure_clean(root)?;
    let source_commit = receipt::command_text(root, "git", &["rev-parse", "HEAD"])?;
    receipt::validate_hex(&source_commit, 40, "source commit")?;
    let freeze = SourceFreeze {
        source_commit: source_commit.clone(),
        version: receipt::workspace_version(root)?,
        cargo_lock_sha256: receipt::sha256(&root.join("Cargo.lock"))?,
        rustc: receipt::command_text(root, "rustc", &["--version", "--verbose"])?,
        target: receipt::RELEASE_TARGET.to_owned(),
        remote_synced: receipt::upstream_commit(root)
            .is_ok_and(|upstream| upstream == source_commit),
    };
    write(root, &freeze)?;
    receipt::invalidate_promoted_candidate(root)?;
    Ok(freeze)
}

pub(super) fn read(root: &Path) -> Result<SourceFreeze, String> {
    let document = receipt::read_receipt(&root.join(SOURCE_FREEZE_RECEIPT))?;
    if json::u64_field(&document, "schema_version")? != 1 {
        return Err("release-source-freeze receipt schema is not version 1".to_owned());
    }
    let freeze = SourceFreeze {
        source_commit: json::string_field(&document, "source_commit")?,
        version: json::string_field(&document, "version")?,
        cargo_lock_sha256: json::string_field(&document, "cargo_lock_sha256")?,
        rustc: json::string_field(&document, "rustc")?,
        target: json::string_field(&document, "target")?,
        remote_synced: json::bool_field(&document, "remote_synced")?,
    };
    receipt::validate_hex(&freeze.source_commit, 40, "source commit")?;
    receipt::validate_sha256(&freeze.cargo_lock_sha256, "Cargo.lock SHA-256")?;
    if freeze.target != receipt::RELEASE_TARGET {
        return Err(format!(
            "Source Freeze target is {}, expected {}",
            freeze.target,
            receipt::RELEASE_TARGET
        ));
    }
    Ok(freeze)
}

pub(super) fn validate_against_repository(
    root: &Path,
    freeze: &SourceFreeze,
) -> Result<(), String> {
    receipt::ensure_clean(root)?;
    let head = receipt::command_text(root, "git", &["rev-parse", "HEAD"])?;
    if head != freeze.source_commit {
        return Err(format!(
            "STALE RECEIPT: Source Freeze {} does not match HEAD {head}",
            freeze.source_commit
        ));
    }
    if receipt::workspace_version(root)? != freeze.version {
        return Err("STALE RECEIPT: Source Freeze version differs from Cargo.toml".to_owned());
    }
    if receipt::sha256(&root.join("Cargo.lock"))? != freeze.cargo_lock_sha256 {
        return Err("STALE RECEIPT: Source Freeze Cargo.lock hash changed".to_owned());
    }
    if receipt::command_text(root, "rustc", &["--version", "--verbose"])? != freeze.rustc {
        return Err("STALE RECEIPT: Source Freeze rustc identity changed".to_owned());
    }
    Ok(())
}

fn write(root: &Path, freeze: &SourceFreeze) -> Result<(), String> {
    let document = format!(
        concat!(
            "{{\"schema_version\":1,",
            "\"source_commit\":\"{}\",",
            "\"version\":\"{}\",",
            "\"cargo_lock_sha256\":\"{}\",",
            "\"rustc\":\"{}\",",
            "\"target\":\"{}\",",
            "\"remote_synced\":{}",
            "}}\n"
        ),
        json::escape(&freeze.source_commit),
        json::escape(&freeze.version),
        json::escape(&freeze.cargo_lock_sha256),
        json::escape(&freeze.rustc),
        json::escape(&freeze.target),
        freeze.remote_synced,
    );
    receipt::write_receipt(root, SOURCE_FREEZE_RECEIPT, &document)
}

#[cfg(test)]
mod tests {
    use super::{SOURCE_FREEZE_RECEIPT, SourceFreeze, write};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn source_freeze_round_trips_without_artifact_identity() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-source-freeze-{nonce}"));
        let freeze = SourceFreeze {
            source_commit: "a".repeat(40),
            version: "0.1.0".to_owned(),
            cargo_lock_sha256: "b".repeat(64),
            rustc: "rustc test".to_owned(),
            target: crate::qualification::receipt::RELEASE_TARGET.to_owned(),
            remote_synced: false,
        };
        write(&root, &freeze).expect("write source freeze");
        let document = fs::read_to_string(root.join(SOURCE_FREEZE_RECEIPT)).expect("receipt");
        assert!(!document.contains("exe_sha256"));
        assert!(!document.contains("zip_sha256"));
        assert_eq!(super::read(&root), Ok(freeze));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
