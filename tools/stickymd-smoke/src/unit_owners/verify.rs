//! Runs the unit-test owner verification on Windows.
//!
//! Every command goes through `managed_process::run_bounded_output` (deadline, per-stream cap,
//! descendants ended with the process) and must exit successfully; a timeout, an overlong
//! stream, a parse failure or a non-zero exit fails the task, and no owner counts as verified.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use super::{Owner, TestArtifact, parse_artifacts, parse_list};
use crate::evidence::SourceIdentity;
use crate::managed_process::run_bounded_output;

/// The tests shard has already built the test executables; a cold build still fits.
const BUILD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// About 0.2 MiB for the whole workspace on Rust 1.97.1.
const BUILD_OUTPUT_LIMIT: u64 = 16 * 1024 * 1024;
const LIST_TIMEOUT: Duration = Duration::from_secs(60);
const OWNER_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// `--list` of the largest owner executable is about 30 KiB; one owner's output is small.
const TEST_OUTPUT_LIMIT: u64 = 1024 * 1024;
const FAILURE_TAIL: usize = 4 * 1024;

/// Verifies `owners` (all in `packages`, or anywhere in the workspace for `None`) and returns
/// their identities, ascending, only when every owner was listed once, not ignored, and
/// passed in its own process. No owners means no build at all.
pub(crate) fn verify(
    root: &Path,
    owners: &BTreeSet<Owner>,
    packages: Option<&[&str]>,
    identity: &SourceIdentity,
) -> Result<Vec<String>, String> {
    if owners.is_empty() {
        return Ok(Vec::new());
    }
    identity.unchanged(root, "before unit-test owner verification")?;
    let artifacts = build(root, packages)?;
    let verified = verify_built(&artifacts, owners, &mut run_test_executable);
    identity.unchanged(root, "after unit-test owner verification")?;
    verified
}

/// Invokes one test executable with `args`: `(artifact, args, timeout, label)`.
type RunTest<'a> = dyn FnMut(&TestArtifact, &[&str], Duration, &str) -> Result<Output, String> + 'a;

/// Lists and runs `owners` in the built `artifacts`. Every owner of an executable whose
/// listing fails is unverified; any failure fails the whole verification.
fn verify_built(
    artifacts: &[TestArtifact],
    owners: &BTreeSet<Owner>,
    run: &mut RunTest<'_>,
) -> Result<Vec<String>, String> {
    let mut by_executable: BTreeMap<usize, Vec<&Owner>> = BTreeMap::new();
    for owner in owners {
        by_executable
            .entry(executable_of(artifacts, owner)?)
            .or_default()
            .push(owner);
    }
    let mut failures = Vec::new();
    for (index, owners) in by_executable {
        let artifact = &artifacts[index];
        if let Err(error) = listed_once_and_not_ignored(artifact, &owners, run) {
            failures.push(error);
            continue;
        }
        for owner in owners {
            let label = format!("owner {owner}");
            let outcome = run(
                artifact,
                &["--exact", owner.path, "--nocapture"],
                OWNER_TIMEOUT,
                &label,
            )
            .and_then(|output| succeeded(&output, &label));
            if let Err(error) = outcome {
                failures.push(error);
            }
        }
    }
    if !failures.is_empty() {
        return Err(format!(
            "unit-test owner verification failed: {}",
            failures.join("; ")
        ));
    }
    let mut verified = owners.iter().map(ToString::to_string).collect::<Vec<_>>();
    verified.sort();
    Ok(verified)
}

fn build(root: &Path, packages: Option<&[&str]>) -> Result<Vec<TestArtifact>, String> {
    let mut command = Command::new("cargo");
    command
        .args(["test", "--locked", "--no-run", "--message-format=json"])
        .current_dir(root)
        .stdin(Stdio::null());
    match packages {
        None => {
            command.arg("--workspace");
        }
        Some(packages) => {
            for package in packages {
                command.args(["-p", package]);
            }
        }
    }
    let output = run_bounded_output(
        &mut command,
        BUILD_TIMEOUT,
        "unit-test owner build",
        BUILD_OUTPUT_LIMIT,
    )?;
    succeeded(&output, "unit-test owner build")?;
    parse_artifacts(&text(&output.stdout, "unit-test owner build")?)
}

/// The one executable that holds `owner`'s target.
fn executable_of(artifacts: &[TestArtifact], owner: &Owner) -> Result<usize, String> {
    let mut matches = artifacts
        .iter()
        .enumerate()
        .filter(|(_, artifact)| artifact.is(owner.package, owner.target))
        .map(|(index, _)| index);
    match (matches.next(), matches.next()) {
        (Some(index), None) => Ok(index),
        (None, _) => Err(format!("no test executable was built for owner {owner}")),
        (Some(_), Some(_)) => Err(format!("several test executables match owner {owner}")),
    }
}

fn listed_once_and_not_ignored(
    artifact: &TestArtifact,
    owners: &[&Owner],
    run: &mut RunTest<'_>,
) -> Result<(), String> {
    let listed = list(artifact, &["--list"], run)?;
    let ignored = list(artifact, &["--list", "--ignored"], run)?;
    let mut counts = BTreeMap::<&str, usize>::new();
    for name in &listed {
        *counts.entry(name.as_str()).or_default() += 1;
    }
    let ignored = ignored.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut problems = Vec::new();
    for owner in owners {
        match counts.get(owner.path).copied().unwrap_or(0) {
            1 => {}
            0 => problems.push(format!("owner {owner} is not listed")),
            count => problems.push(format!("owner {owner} is listed {count} times")),
        }
        if ignored.contains(owner.path) {
            problems.push(format!("owner {owner} is ignored"));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

fn list(
    artifact: &TestArtifact,
    args: &[&str],
    run: &mut RunTest<'_>,
) -> Result<Vec<String>, String> {
    let label = format!("{} {}", artifact.name, args.join(" "));
    let output = run(artifact, args, LIST_TIMEOUT, &label)?;
    succeeded(&output, &label)?;
    let stdout = text(&output.stdout, &label)?;
    parse_list(&stdout)
        .map(|names| names.into_iter().map(str::to_owned).collect())
        .map_err(|error| format!("{label}: {error}"))
}

/// Runs a test executable the way Cargo does, from its package directory, inside the bounded
/// runner. With `--exact <owner> --nocapture` each owner gets its own process and its output
/// goes to the parent's capped pipes.
fn run_test_executable(
    artifact: &TestArtifact,
    args: &[&str],
    timeout: Duration,
    label: &str,
) -> Result<Output, String> {
    let mut command = Command::new(&artifact.executable);
    command
        .args(args)
        .current_dir(&artifact.package_dir)
        .env("CARGO_MANIFEST_DIR", &artifact.package_dir)
        .stdin(Stdio::null());
    run_bounded_output(&mut command, timeout, label, TEST_OUTPUT_LIMIT)
}

/// `Ok(Output)` only means the process ended in time; success is the exit status.
fn succeeded(output: &Output, label: &str) -> Result<(), String> {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{label} exited with {}: {}",
            output.status,
            tail(&output.stderr)
        ))
    }
}

fn text(bytes: &[u8], label: &str) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| format!("{label} wrote non-UTF-8 output"))
}

/// The last bytes of `bytes` as text, for a failure message.
fn tail(bytes: &[u8]) -> String {
    let start = bytes.len().saturating_sub(FAILURE_TAIL);
    String::from_utf8_lossy(&bytes[start..]).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::os::windows::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::{ExitStatus, Output};
    use std::time::Duration;

    use super::{TestArtifact, verify_built};
    use crate::unit_owners::{Owner, OwnerTarget};

    const LISTED: &str = "a::one: test\na::two: test\na::off: test\n\n3 tests, 0 benchmarks\n";
    const IGNORED: &str = "a::off: test\n\n1 test, 0 benchmarks\n";

    fn artifact() -> TestArtifact {
        TestArtifact {
            package: "p".to_owned(),
            kind: vec!["bin".to_owned()],
            name: "p".to_owned(),
            executable: PathBuf::from("p.exe"),
            package_dir: PathBuf::from("p"),
        }
    }

    fn owner(path: &'static str) -> Owner {
        Owner {
            package: "p",
            target: OwnerTarget::Bin("p"),
            path,
        }
    }

    fn output(code: u32, stdout: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    /// A fake executable: listings from `listed` / `ignored` with their exit codes, and each
    /// owner run exiting with `run_code`; records the owners it ran.
    fn verify(
        owners: &[&'static str],
        (listed, listed_code): (&str, u32),
        (ignored, ignored_code): (&str, u32),
        run_code: u32,
    ) -> (Result<Vec<String>, String>, Vec<String>) {
        let mut ran = Vec::new();
        let owners = owners
            .iter()
            .map(|path| owner(path))
            .collect::<BTreeSet<_>>();
        let mut run = |_: &TestArtifact, args: &[&str], _: Duration, _: &str| {
            Ok(match args {
                ["--list"] => output(listed_code, listed),
                ["--list", "--ignored"] => output(ignored_code, ignored),
                ["--exact", path, "--nocapture"] => {
                    ran.push((*path).to_owned());
                    output(run_code, "")
                }
                other => panic!("unexpected invocation {other:?}"),
            })
        };
        let result = verify_built(&[artifact()], &owners, &mut run);
        (result, ran)
    }

    #[test]
    fn listed_unignored_passing_owners_are_verified_each_in_its_own_run() {
        let (result, ran) = verify(&["a::two", "a::one"], (LISTED, 0), (IGNORED, 0), 0);
        assert_eq!(result.unwrap(), ["p/bin:p/a::one", "p/bin:p/a::two"]);
        assert_eq!(ran, ["a::one", "a::two"]);
    }

    #[test]
    fn a_listing_or_run_failure_verifies_no_owner() {
        // Ignored, missing or listed twice: rejected before any owner runs.
        let twice = "a::one: test\na::one: test\n\n2 tests, 0 benchmarks\n";
        for (owners, listed, ignored) in [
            (["a::off"], LISTED, IGNORED),
            (["a::gone"], LISTED, IGNORED),
            (["a::one"], twice, IGNORED),
        ] {
            let (result, ran) = verify(&owners, (listed, 0), (ignored, 0), 0);
            assert!(result.is_err(), "{owners:?}");
            assert!(ran.is_empty());
        }
        // The ignored listing fails while the owner itself would pass.
        let (result, ran) = verify(&["a::one"], (LISTED, 0), (IGNORED, 101), 0);
        assert!(result.unwrap_err().contains("--ignored"));
        assert!(ran.is_empty());
        // A truncated listing is not a shorter one.
        let (result, _) = verify(&["a::one"], ("a::one: test\n", 0), (IGNORED, 0), 0);
        assert!(result.is_err());
        // The owner runs and fails.
        let (result, ran) = verify(&["a::one"], (LISTED, 0), (IGNORED, 0), 101);
        assert!(result.unwrap_err().contains("owner p/bin:p/a::one"));
        assert_eq!(ran, ["a::one"]);
    }

    #[test]
    fn an_owner_without_its_executable_is_an_error() {
        let mut run =
            |_: &TestArtifact, _: &[&str], _: Duration, _: &str| -> Result<Output, String> {
                panic!("nothing may run")
            };
        let other = Owner {
            package: "q",
            ..owner("a::one")
        };
        assert!(verify_built(&[artifact()], &BTreeSet::from([other]), &mut run).is_err());
        let lib = Owner {
            target: OwnerTarget::Lib,
            ..owner("a::one")
        };
        assert!(verify_built(&[artifact()], &BTreeSet::from([lib]), &mut run).is_err());
    }
}
