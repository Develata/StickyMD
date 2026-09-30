//! Opt-in comparison of runner-style dispatch using a caller-supplied pre-migration entry.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::*;
use std::{process::Command, time::Instant};

#[test]
#[ignore = "opt-in serial cache-hit SBOM timing on a fresh isolated local package"]
fn compare_in_process_and_legacy_dispatch() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let directory =
        std::env::var_os("STICKYMD_SBOM_DIAGNOSTIC_DIRECTORY").expect("fresh package directory");
    let legacy =
        std::env::var_os("STICKYMD_SBOM_LEGACY_SCRIPT").expect("pre-migration wrapper snapshot");
    let guard = TemporaryDirectory::new("sbom-timing-check").unwrap();
    let (plan, _) = syft::prepare(root, None, &guard.path().join("verified")).unwrap();
    assert!(plan.downloads.is_empty(), "timing must never download Syft");
    guard.close().unwrap();
    let options = SbomBuildOptions {
        directory: Some(directory.into()),
        ..Default::default()
    };
    let mut samples = [Vec::new(), Vec::new()];
    for round in 0..6 {
        for index in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
            let start = Instant::now();
            let output = if index == 0 {
                let mut output = Vec::new();
                generate(root, &options, &mut output).unwrap();
                output
            } else {
                let result = Command::new("pwsh")
                    .args(["-NoProfile", "-NonInteractive", "-File"])
                    .arg(&legacy)
                    .arg("-PackageDirectory")
                    .arg(options.directory.as_ref().unwrap())
                    .current_dir(root)
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                result.stdout
            };
            if round > 0 {
                samples[index].push(start.elapsed().as_secs_f64());
            }
            assert!(String::from_utf8_lossy(&output).contains("SYFT_VERSION=1.50.0"));
            sbom::validate_file(&options.directory.as_ref().unwrap().join("SBOM.spdx.json"))
                .unwrap();
        }
    }
    println!(
        "SBOM_DIRECT_SECONDS={:?}\nSBOM_LEGACY_SECONDS={:?}",
        samples[0], samples[1]
    );
}
