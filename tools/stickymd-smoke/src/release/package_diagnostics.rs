//! Opt-in diagnostics on a freshly built, isolated local package, never qualification evidence.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::*;
use std::{process::Command, time::Instant};

fn inputs(runtime: bool) -> (std::path::PathBuf, PackageOptions) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let directory = std::env::var_os("STICKYMD_PACKAGE_DIAGNOSTIC_DIRECTORY")
        .expect("set STICKYMD_PACKAGE_DIAGNOSTIC_DIRECTORY to this run's isolated local package");
    (
        root,
        PackageOptions {
            directory: Some(directory.into()),
            zip: None,
            checksums: None,
            runtime,
        },
    )
}

fn stable_output(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.starts_with("THIRD_PARTY_NOTICES="))
        .map(str::to_owned)
        .collect()
}

#[test]
#[ignore = "opt-in serial timing on a fresh package; no GUI, remote access or evidence writes"]
fn compare_package_dispatch_paths() {
    let (root, options) = inputs(false);
    let mut direct_ms = Vec::new();
    let mut wrapper_ms = Vec::new();
    for round in 0..6 {
        let mut observed = [Vec::new(), Vec::new()];
        for index in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
            let started = Instant::now();
            if index == 0 {
                verify(&root, &options, &mut observed[index]).unwrap();
            } else {
                let result = Command::new("pwsh.exe")
                    .args(["-NoProfile", "-NonInteractive", "-File"])
                    .arg(root.join("tools/release/verify-package.ps1"))
                    .arg("-PackageDirectory")
                    .arg(options.directory.as_ref().unwrap())
                    .current_dir(&root)
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                observed[index] = result.stdout;
            }
            if round != 0 {
                let measurements = if index == 0 {
                    &mut direct_ms
                } else {
                    &mut wrapper_ms
                };
                measurements.push(started.elapsed().as_secs_f64() * 1000.0);
            }
        }
        assert_eq!(stable_output(&observed[0]), stable_output(&observed[1]));
        assert!(
            stable_output(&observed[0])
                .iter()
                .any(|line| line == "PACKAGE_VERIFY=PASS")
        );
    }
    println!(
        "PACKAGE_DISPATCH_DIRECT_MS={direct_ms:?}\nPACKAGE_DISPATCH_WRAPPER_MS={wrapper_ms:?}"
    );
}

#[test]
#[ignore = "opt-in serial GUI bootstrap on a fresh isolated local package"]
fn direct_package_runtime_preserves_output_and_cleanup() {
    let (root, options) = inputs(true);
    let mut output = Vec::new();
    verify(&root, &options, &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "PACKAGE_RUNTIME=PASS (ASCII, space, Chinese, same-directory and different-directory)"
    ));
    assert!(output.contains("PACKAGE_VERIFY=PASS\nPACKAGE_PATH="));
    let notices = output
        .lines()
        .find_map(|line| line.strip_prefix("THIRD_PARTY_NOTICES="))
        .unwrap();
    assert!(
        !Path::new(notices).parent().unwrap().exists(),
        "successful verification must remove its private snapshot"
    );
    println!("{output}");
}
