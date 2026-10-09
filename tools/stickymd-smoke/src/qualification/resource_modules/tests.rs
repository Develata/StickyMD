//! Real repository and receipt regressions; the synthetic PE is never executed.

use super::*;
use std::fs;
use std::path::PathBuf;

const GROUP: ResourceModule = ResourceModule::Window;
/// The clone-wide ledger record for the window group's current inputs.
fn ledger(root: &std::path::Path) -> PathBuf {
    module_ledger::store::LedgerStore::for_repository(root)
        .unwrap()
        .module_record(
            GROUP.ledger_id(),
            &module_ledger::fingerprint::calculate(root, ModuleId::Resource(GROUP)).unwrap(),
        )
        .unwrap()
}

#[test]
fn reuse_revalidates_ignored_success_files_after_planning() {
    let fixture = Fixture::new();
    let root = &fixture.root;
    fixture.record(&fixture.candidate);
    let campaign = Campaign::prepare(root).unwrap();
    assert!(!campaign.needs_run(GROUP));
    assert!(campaign.reuse(root, GROUP).unwrap().is_some());
    let success = module_ledger::compatible_success(root, ModuleId::Resource(GROUP))
        .unwrap()
        .unwrap();
    let ledger = ledger(root);
    let ledger_before = fs::read(&ledger).unwrap();
    let archive_before = fs::read(&success.evidence_path).unwrap();
    let mut incorrectly_reused = Vec::new();
    for (label, path, replacement) in [
        ("missing ledger", &ledger, None),
        (
            "corrupt ledger",
            &ledger,
            Some(b"invalid ledger".as_slice()),
        ),
        ("missing archive", &success.evidence_path, None),
        (
            "corrupt archive",
            &success.evidence_path,
            Some(b"invalid archive".as_slice()),
        ),
    ] {
        match replacement {
            Some(bytes) => crate::atomic_evidence::write(path, bytes).unwrap(),
            None => fs::remove_file(path).unwrap(),
        }
        // These files are ignored, so candidate and input validation still pass.
        receipt::ensure_clean(root).unwrap();
        if campaign.reuse(root, GROUP).is_ok() {
            incorrectly_reused.push(label);
        }
        assert_eq!(fs::read(path).ok().as_deref(), replacement);
        crate::atomic_evidence::write(&ledger, &ledger_before).unwrap();
        crate::atomic_evidence::write(&success.evidence_path, &archive_before).unwrap();
    }
    assert!(incorrectly_reused.is_empty(), "{incorrectly_reused:?}");
}

#[test]
fn reuse_reports_the_current_validated_origin_after_success_rotation() {
    let fixture = Fixture::new();
    fixture.record(&fixture.candidate);
    let campaign = Campaign::prepare(&fixture.root).unwrap();
    let old = module_ledger::compatible_success(&fixture.root, ModuleId::Resource(GROUP))
        .unwrap()
        .unwrap();
    let mut origin = fixture.candidate.clone();
    origin.source_commit = "e".repeat(40);
    origin.zip_sha256 = "f".repeat(64);
    fixture.record(&origin);
    assert!(!old.evidence_path.exists());
    let current = module_ledger::compatible_success(&fixture.root, ModuleId::Resource(GROUP))
        .unwrap()
        .unwrap();
    let reused = campaign.reuse(&fixture.root, GROUP).unwrap().unwrap();
    assert_eq!(reused.status, EvidenceStatus::Passed);
    let detail = reused.detail.unwrap();
    assert!(
        detail.contains(&format!("origin_source={}", origin.source_commit)),
        "{detail}"
    );
    assert!(
        detail.contains(&format!("origin_zip={}", origin.zip_sha256)),
        "{detail}"
    );
    assert!(
        detail.contains(&current.evidence_path.display().to_string()),
        "{detail}"
    );
    assert!(reused.measurements.is_empty() && reused.samples.is_empty());
}

struct Fixture {
    root: PathBuf,
    candidate: receipt::Candidate,
}

impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-resource-reuse-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\n[workspace.package]\nversion = \"0.1.1\"\n",
        )
        .unwrap();
        fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
        fs::write(root.join(".gitignore"), "/dist/\n").unwrap();
        let git = |args: &[&str]| {
            receipt::command_text(&root, "git", args).unwrap();
        };
        git(&["init", "--quiet"]);
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=Smoke Test",
            "-c",
            "user.email=smoke@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=disabled-hooks",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ]);
        let source = crate::qualification::source_freeze::create(&root).unwrap();
        let directory = receipt::candidate_directory(&root);
        let executable = receipt::candidate_executable(&root);
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(
            &executable,
            crate::pe_dependencies::tests::synthetic_pe(None, None),
        )
        .unwrap();
        let zip_name = "StickyMD-0.1.1-windows-x64-portable.zip";
        fs::write(
            directory.join(zip_name),
            b"fixture ZIP identity, never extracted",
        )
        .unwrap();
        fs::write(receipt::candidate_sbom(&root), b"fixture SBOM identity").unwrap();
        let candidate = receipt::Candidate {
            source_commit: source.source_commit,
            version: source.version,
            cargo_lock_sha256: source.cargo_lock_sha256,
            exe_sha256: receipt::sha256(&executable).unwrap(),
            zip_sha256: receipt::sha256(&directory.join(zip_name)).unwrap(),
            sbom_sha256: receipt::sha256(&receipt::candidate_sbom(&root)).unwrap(),
            target: source.target,
            workflow_run_id: 1,
            workflow_attempt: 1,
            artifact_id: 1,
            artifact_name: receipt::RELEASE_ARTIFACT_NAME.into(),
            zip_name: zip_name.into(),
        };
        fs::write(
            directory.join("SHA256SUMS.txt"),
            format!(
                "{} *{zip_name}\n{} *SBOM.spdx.json\n",
                candidate.zip_sha256, candidate.sbom_sha256
            ),
        )
        .unwrap();
        receipt::write_candidate(&root, &candidate).unwrap();
        Self { root, candidate }
    }

    fn record(&self, origin: &receipt::Candidate) {
        let result = crate::resource_plan::tests::valid_resource_result(GROUP);
        let document = crate::resource_plan::tests::document(GROUP, &result)
            .replace(&"a".repeat(40), &origin.source_commit)
            .replace(&"c".repeat(64), &origin.exe_sha256);
        let input = fingerprint::calculate(&self.root, ModuleId::Resource(GROUP)).unwrap();
        let document = format!(
            "{{\"resource_input_fingerprint\":\"{input}\",{}",
            &document[1..]
        );
        receipt::write_receipt(&self.root, GROUP.receipt(), &document).unwrap();
        module_ledger::record_success(&self.root, ModuleId::Resource(GROUP), origin).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
