//! The packaging CLI refuses malformed requests before any output, with exit code 1.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use std::process::Command;

#[test]
fn malformed_packaging_requests_never_emit_a_success_marker() {
    for args in [
        vec![],
        vec!["unknown-command"],
        vec!["notices"],
        vec!["notices", "--destination", "one", "--destination", "two"],
        vec!["prepare-package"],
        vec!["prepare-package", "--exe", "missing.exe"],
        vec!["syft-plan", "--runtime"],
        vec!["syft-publish", "--kind", "unknown", "--input", "file"],
        vec!["syft-verify"],
        vec!["checksums", "--zip", "missing"],
        vec!["publish-sbom", "--input", "missing"],
        vec!["publish-package", "--input", "missing"],
        vec!["prepare-sbom"],
        vec!["checksums", "--zip=a", "--output=b", "--runtime"],
        vec!["macos-sbom", "--zip", "a", "--allow-missing"],
        vec!["notices", "--destination", "x", "--exact-candidate"],
        vec!["app-inputs", "--home"],
        // The verification commands stay in stickymd-smoke.
        vec!["verify-package"],
        vec!["workspace-version"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_stickymd-package"))
            .args(&args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
}
