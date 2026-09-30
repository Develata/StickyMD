//! In-process release verification with the same human/JSON stream boundary as other tasks.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use crate::release::{self, PackageOptions, verify_package};
use std::path::Path;

pub(super) fn run(root: &Path, capture_output: bool) -> Result<(), String> {
    let options = PackageOptions {
        directory: None,
        zip: None,
        checksums: None,
        runtime: true,
    };
    run_with_options(root, &options, capture_output)
}

pub(super) fn build(root: &Path, capture_output: bool) -> Result<(), String> {
    with_output("portable package creation", capture_output, |output| {
        release::build_package(root, &release::PackageBuildOptions::default(), output)
    })
}

pub(super) fn sbom(root: &Path, capture_output: bool) -> Result<(), String> {
    with_output("SPDX SBOM generation", capture_output, |output| {
        release::generate_sbom(root, &release::SbomBuildOptions::default(), output)
    })
}

fn run_with_options(
    root: &Path,
    options: &PackageOptions,
    capture_output: bool,
) -> Result<(), String> {
    with_output("portable package verification", capture_output, |output| {
        verify_package(root, options, output)
    })
}

fn with_output(
    label: &str,
    capture_output: bool,
    action: impl FnOnce(&mut dyn std::io::Write) -> Result<(), String>,
) -> Result<(), String> {
    if !capture_output {
        return action(&mut std::io::stdout().lock());
    }
    let mut output = Vec::new();
    let result = action(&mut output);
    let detail = super::captured_failure_detail(&output, &[]);
    match result {
        Ok(()) => {
            if !detail.is_empty() {
                eprintln!("[{label}]\n{detail}");
            }
            Ok(())
        }
        Err(error) => Err(if detail.is_empty() {
            format!("`{label}` failed: {error}")
        } else {
            format!("`{label}` failed: {error}; {detail}")
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_tasks_enter_rust_rules_and_keep_failure_diagnostics() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("nonexistent-fixture");
        let error = build(&root, true).unwrap_err();
        assert!(
            error.contains("Release executable does not exist"),
            "{error}"
        );
        assert!(error.contains("portable package creation"), "{error}");
        let error = sbom(&root, true).unwrap_err();
        assert!(error.contains("SPDX SBOM generation"), "{error}");
        assert!(!error.contains("cannot start"), "{error}");
        let error = with_output("fixture", true, |output| {
            writeln!(output, "partial diagnostic").unwrap();
            Err("adapter failed".into())
        })
        .unwrap_err();
        assert!(error.contains("adapter failed") && error.contains("partial diagnostic"));
    }

    #[test]
    fn package_task_reaches_rust_validation_without_a_powershell_entrypoint() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let options = PackageOptions {
            directory: Some(root.join("tools/stickymd-smoke/src")),
            zip: Some(root.join("tools/stickymd-smoke/src/release/package.rs")),
            checksums: None,
            runtime: false,
        };
        let error = run_with_options(root, &options, true).unwrap_err();
        assert!(error.contains("cannot resolve"), "{error}");
        assert!(error.contains("portable package verification"), "{error}");
        assert!(!error.contains("cannot start"), "{error}");
    }
}
