//! In-process release verification with the same human/JSON stream boundary as other tasks.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use crate::release::{PackageOptions, verify_package};
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

fn run_with_options(
    root: &Path,
    options: &PackageOptions,
    capture_output: bool,
) -> Result<(), String> {
    if !capture_output {
        return verify_package(root, options, &mut std::io::stdout().lock());
    }
    let mut output = Vec::new();
    let result = verify_package(root, options, &mut output);
    let detail = super::captured_failure_detail(&output, &[]);
    let label = super::task_label(&super::Task::VerifyPackage);
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
