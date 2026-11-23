//! The receipt of the source tests on the release commit R (plan 11 #macos-release-gate, the
//! "R 上的源码测试" row; format #macos-readiness-check): what the run recorded, and how the gate
//! judges it. The judgement reads only recorded facts: the exact command, the source identity
//! at start and end, and libtest's own counts per test binary, against the set Cargo said it
//! would run.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

mod format;
pub(super) mod output;
#[cfg(target_os = "macos")]
pub(super) mod run;
#[cfg(test)]
mod tests;

pub(super) use format::{parse, render};

/// Where the receipt lives; the runner replaces it, the gate reads it.
pub(super) const RECEIPT: &str = "dist/evidence/release-source-tests-macos.json";

/// The eight packages the row names, in the order of `release-macos`'s self-check.
pub(super) const PACKAGES: [&str; 8] = [
    "stickymd-core",
    "stickymd-render",
    "stickymd-app",
    "stickymd-desktop",
    "stickymd-posix",
    "stickymd-mac",
    "stickymd-package",
    "stickymd-smoke",
];

/// P18-L08's two tests: the default volume (in the default set) and the case-sensitive one
/// (`#[ignore]`, it mounts a disk image), both run explicitly (plan 11 #macos-release-gate).
pub(super) const SAME_KEY_TESTS: [&str; 2] = [
    "note_root::tests::one_key_is_exclusive_and_another_key_runs_beside_it",
    "note_root::tests::on_a_case_sensitive_volume_case_makes_another_key",
];

/// The one command the row allows: `cargo test --locked`, the eight packages, no filter.
pub(super) fn command() -> Vec<String> {
    let mut argv = vec!["cargo".to_owned(), "test".to_owned(), "--locked".to_owned()];
    for package in PACKAGES {
        argv.push("-p".to_owned());
        argv.push(package.to_owned());
    }
    argv
}

/// The explicit run of both same-key tests, the ignored one included.
pub(super) fn same_key_command() -> Vec<String> {
    let mut argv: Vec<String> = [
        "cargo",
        "test",
        "--locked",
        "-p",
        "stickymd-mac",
        "--bin",
        "stickymd-mac",
        "--",
        "--exact",
        "--include-ignored",
    ]
    .map(str::to_owned)
    .to_vec();
    argv.extend(SAME_KEY_TESTS.map(str::to_owned));
    argv
}

/// One libtest run: a test binary or a library's doc-tests, named without paths.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Target {
    pub package: String,
    /// Cargo's target kinds joined by `,` (`lib`, `bin`, `test`, …), or `doctest`.
    pub kind: String,
    pub name: String,
}

/// libtest's summary of one run: `running <ran> tests` and its `test result:` line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Counts {
    pub ran: u64,
    pub passed: u64,
    pub failed: u64,
    pub ignored: u64,
    pub measured: u64,
    pub filtered_out: u64,
}

impl Counts {
    /// libtest's own arithmetic: every test it ran ended in exactly one of these.
    fn adds_up(&self) -> bool {
        self.passed + self.failed + self.ignored + self.measured == self.ran
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Run {
    pub target: Target,
    pub counts: Counts,
}

/// The source identity at one end of the run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Side {
    pub head: String,
    pub clean: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Host {
    pub product_name: String,
    pub product_version: String,
    pub build_version: String,
    pub arch: String,
    /// `kern.hv_vmm_present`: `1` in a virtual machine.
    pub virtual_machine: String,
    pub cargo: String,
    pub rustc: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SameKey {
    pub command: Vec<String>,
    pub tests: Vec<String>,
    pub counts: Option<Counts>,
    pub exit_code: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Receipt {
    pub release_source: String,
    pub host: Host,
    pub start: Side,
    pub end: Side,
    pub command: Vec<String>,
    /// `None` when the process ended by a signal or never started.
    pub exit_code: Option<i32>,
    pub expected: Vec<Target>,
    pub binaries: Vec<Run>,
    pub same_key: SameKey,
    /// What the runner could not establish (discovery or output it could not read).
    pub problems: Vec<String>,
}

/// A gate verdict from a receipt: not established (`NotTested`), a recorded failure, or PASS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Outcome {
    Pass,
    Fail(String),
    NotTested(String),
}

/// The verdicts of the source-test row and of the same-key row (plan 11
/// #macos-readiness-check): identity first, then recorded failures, then completeness.
pub(super) fn judge(receipt: &Receipt, release: &str) -> (Outcome, Outcome) {
    if let Err(reason) = identity(receipt, release) {
        return (
            Outcome::NotTested(reason.clone()),
            Outcome::NotTested(reason),
        );
    }
    (main(receipt), same_key(&receipt.same_key))
}

/// R, both ends clean at R, the two commands exactly, a macOS arm64 host.
fn identity(receipt: &Receipt, release: &str) -> Result<(), String> {
    let release = release.to_ascii_lowercase();
    if receipt.release_source != release {
        return Err(format!(
            "the receipt is for {}, not {release}",
            receipt.release_source
        ));
    }
    for (end, side) in [("start", &receipt.start), ("end", &receipt.end)] {
        if side.head != release || !side.clean {
            return Err(format!(
                "at the {end} the checkout was {} (clean {}), not a clean {release}",
                side.head, side.clean
            ));
        }
    }
    if receipt.command != command() {
        return Err(format!(
            "the command {:?} is not the row's command",
            receipt.command
        ));
    }
    if receipt.same_key.command != same_key_command() || receipt.same_key.tests != SAME_KEY_TESTS
    {
        return Err("the same-key run is not the required command".to_owned());
    }
    if receipt.host.product_name != "macOS" || receipt.host.arch != "arm64" {
        return Err(format!(
            "the host is {} {}, not macOS arm64",
            receipt.host.product_name, receipt.host.arch
        ));
    }
    Ok(())
}

fn main(receipt: &Receipt) -> Outcome {
    if receipt.exit_code != Some(0) {
        return Outcome::Fail(format!(
            "cargo test exited with {:?}",
            receipt.exit_code
        ));
    }
    if let Some(run) = receipt.binaries.iter().find(|run| run.counts.failed > 0) {
        return Outcome::Fail(format!(
            "{} {} {}: {} failed",
            run.target.package, run.target.kind, run.target.name, run.counts.failed
        ));
    }
    if let Some(problem) = receipt.problems.first() {
        return Outcome::NotTested(format!("the runner recorded: {problem}"));
    }
    let mut expected = receipt.expected.clone();
    expected.sort();
    let mut ran: Vec<Target> = receipt
        .binaries
        .iter()
        .map(|run| run.target.clone())
        .collect();
    ran.sort();
    if expected.windows(2).any(|pair| pair[0] == pair[1])
        || ran.windows(2).any(|pair| pair[0] == pair[1])
    {
        return Outcome::NotTested("a test binary is listed twice".to_owned());
    }
    if ran != expected {
        return Outcome::NotTested(format!(
            "the run covered {} of the {} test binaries and doc-test libraries Cargo listed",
            ran.iter().filter(|target| expected.contains(target)).count(),
            expected.len()
        ));
    }
    if let Some(package) = PACKAGES
        .iter()
        .find(|package| !expected.iter().any(|target| target.package == **package))
    {
        return Outcome::NotTested(format!("no test binary of {package} ran"));
    }
    if let Some(run) = receipt
        .binaries
        .iter()
        .find(|run| run.counts.filtered_out > 0 || !run.counts.adds_up())
    {
        return Outcome::NotTested(format!(
            "{} {} {} filtered tests out or does not add up",
            run.target.package, run.target.kind, run.target.name
        ));
    }
    Outcome::Pass
}

fn same_key(run: &SameKey) -> Outcome {
    let Some(counts) = run.counts else {
        return if run.exit_code == Some(0) {
            Outcome::NotTested("the same-key run printed no test result".to_owned())
        } else {
            Outcome::Fail(format!("the same-key run exited with {:?}", run.exit_code))
        };
    };
    if run.exit_code != Some(0) || counts.failed > 0 {
        return Outcome::Fail(format!(
            "the same-key run exited with {:?}, {} failed",
            run.exit_code, counts.failed
        ));
    }
    if counts.passed == SAME_KEY_TESTS.len() as u64 && counts.ignored == 0 && counts.adds_up() {
        Outcome::Pass
    } else {
        Outcome::NotTested(format!(
            "the same-key run passed {} of its {} tests ({} ignored)",
            counts.passed,
            SAME_KEY_TESTS.len(),
            counts.ignored
        ))
    }
}
