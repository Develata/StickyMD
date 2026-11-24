//! How the gate judges a source-test receipt (plan 11 #macos-readiness-check): identity first,
//! then recorded failures, then completeness. Completeness is checked test by test: every test a
//! binary lists ran and ended as its listing says, and the receipt's targets are R's as
//! `cargo metadata` lists them on R, apart from the receipt.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::{
    Case, CaseOutcome, Counts, EXPLICIT, Explicit, PACKAGES, Receipt, Run, Target, command,
    explicit_command, explicit_target,
};

/// A gate verdict from a receipt: not established (`NotTested`), a recorded failure, or PASS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum Outcome {
    Pass,
    Fail(String),
    NotTested(String),
}

/// The verdicts a receipt carries: the default tests, P18-L08's two volumes, P16-S06.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct Verdicts {
    pub tests: Outcome,
    pub same_key: Outcome,
    pub s06: Outcome,
}

pub(in super::super) fn judge(receipt: &Receipt, release: &str, targets: &[Target]) -> Verdicts {
    if let Err(reason) = identity(receipt, release) {
        return Verdicts {
            tests: Outcome::NotTested(reason.clone()),
            same_key: Outcome::NotTested(reason.clone()),
            s06: Outcome::NotTested(reason),
        };
    }
    let listed = receipt
        .binaries
        .iter()
        .find(|run| run.target == explicit_target());
    let explicit = |item: &str| match receipt.explicit.iter().find(|e| e.item == item) {
        Some(entry) => explicit_run(entry, listed, item == "same-key"),
        None => Outcome::NotTested(format!("the receipt has no explicit run for {item}")),
    };
    Verdicts {
        tests: main(receipt, targets),
        same_key: explicit("same-key"),
        s06: explicit("P16-S06"),
    }
}

/// R, both ends clean at R, the commands exactly, a macOS arm64 host.
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
    let items: Vec<&str> = receipt.explicit.iter().map(|e| e.item.as_str()).collect();
    let wanted: Vec<&str> = EXPLICIT.iter().map(|(item, _)| *item).collect();
    if items != wanted {
        return Err(format!("the explicit runs are {items:?}, not {wanted:?}"));
    }
    for (entry, (_, tests)) in receipt.explicit.iter().zip(EXPLICIT) {
        if entry.command != explicit_command(tests) || entry.tests != tests {
            return Err(format!(
                "the explicit run for {} is not the required command",
                entry.item
            ));
        }
    }
    if receipt.host.product_name != "macOS" || receipt.host.arch != "arm64" {
        return Err(format!(
            "the host is {} {}, not macOS arm64",
            receipt.host.product_name, receipt.host.arch
        ));
    }
    Ok(())
}

/// The cases agree with the counts and with themselves: each listed test ran once and ended as
/// its listing says (an ignored test ignored, every other one ok or failed).
fn cases_agree(counts: &Counts, cases: &[Case]) -> Result<(), String> {
    let count = |outcome: CaseOutcome| cases.iter().filter(|c| c.outcome == outcome).count() as u64;
    if let Some(case) = cases.iter().find(|c| c.outcome == CaseOutcome::NotRun) {
        return Err(format!("{} is listed but did not run", case.name));
    }
    if let Some(case) = cases
        .iter()
        .find(|c| c.ignored != (c.outcome == CaseOutcome::Ignored))
    {
        return Err(format!(
            "{} ended {} though its listing says ignored={}",
            case.name,
            case.outcome.label(),
            case.ignored
        ));
    }
    let mut names: Vec<&str> = cases.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("a test is listed twice".to_owned());
    }
    if counts.ran != cases.len() as u64
        || counts.passed != count(CaseOutcome::Ok)
        || counts.failed != count(CaseOutcome::Failed)
        || counts.ignored != count(CaseOutcome::Ignored)
        || counts.measured != 0
        || !counts.adds_up()
    {
        return Err(format!(
            "the counts {counts:?} are not the {} listed tests' outcomes",
            cases.len()
        ));
    }
    Ok(())
}

fn named(target: &Target) -> String {
    format!("{} {} {}", target.package, target.kind, target.name)
}

fn main(receipt: &Receipt, targets: &[Target]) -> Outcome {
    if receipt.exit_code != Some(0) {
        return Outcome::Fail(format!("cargo test exited with {:?}", receipt.exit_code));
    }
    if let Some(run) = receipt.binaries.iter().find(|run| {
        run.counts.failed > 0 || run.cases.iter().any(|c| c.outcome == CaseOutcome::Failed)
    }) {
        return Outcome::Fail(format!("{}: a test failed", named(&run.target)));
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
    let mut listed = targets.to_vec();
    listed.sort();
    if expected != listed {
        let missing: Vec<String> = listed
            .iter()
            .filter(|target| !expected.contains(target))
            .map(named)
            .collect();
        return Outcome::NotTested(format!(
            "the receipt's test list is not R's (cargo metadata): missing {missing:?}, {} listed beside them",
            expected
                .iter()
                .filter(|target| !listed.contains(target))
                .count()
        ));
    }
    if ran != expected {
        return Outcome::NotTested(format!(
            "the run covered {} of the {} test binaries and doc-test libraries Cargo listed",
            ran.iter()
                .filter(|target| expected.contains(target))
                .count(),
            expected.len()
        ));
    }
    if let Some(package) = PACKAGES
        .iter()
        .find(|package| !expected.iter().any(|target| target.package == **package))
    {
        return Outcome::NotTested(format!("no test binary of {package} ran"));
    }
    for run in &receipt.binaries {
        if run.counts.filtered_out > 0 {
            return Outcome::NotTested(format!("{} filtered tests out", named(&run.target)));
        }
        if let Err(reason) = cases_agree(&run.counts, &run.cases) {
            return Outcome::NotTested(format!("{}: {reason}", named(&run.target)));
        }
    }
    Outcome::Pass
}

/// An explicit run: the stickymd-mac binary ran exactly its tests, each named as the main run's
/// listing of that binary names it, and each ok; P18-L08 also needs the probed temporary volume.
fn explicit_run(entry: &Explicit, listed: Option<&Run>, needs_volume: bool) -> Outcome {
    if entry.exit_code != Some(0) || entry.cases.iter().any(|c| c.outcome == CaseOutcome::Failed) {
        return Outcome::Fail(format!(
            "the explicit run for {} exited with {:?} or recorded a failure",
            entry.item, entry.exit_code
        ));
    }
    if entry.target.as_ref() != Some(&explicit_target()) {
        return Outcome::NotTested(format!(
            "the explicit run for {} is not attributed to stickymd-mac's binary",
            entry.item
        ));
    }
    let Some(counts) = entry.counts else {
        return Outcome::NotTested(format!(
            "the explicit run for {} printed no result",
            entry.item
        ));
    };
    let mut ran: Vec<&str> = entry.cases.iter().map(|c| c.name.as_str()).collect();
    ran.sort_unstable();
    let mut wanted: Vec<&str> = entry.tests.iter().map(String::as_str).collect();
    wanted.sort_unstable();
    let all_ok = entry.cases.iter().all(|c| c.outcome == CaseOutcome::Ok);
    let both = wanted.len() as u64;
    if ran != wanted
        || !all_ok
        || counts.ran != both
        || counts.passed != both
        || counts.ignored != 0
        || counts.measured != 0
        || !counts.adds_up()
    {
        return Outcome::NotTested(format!(
            "the explicit run for {} ran {ran:?} ({counts:?}), not exactly {wanted:?} all ok",
            entry.item
        ));
    }
    let Some(listed) = listed else {
        return Outcome::NotTested("stickymd-mac's binary did not run in the main run".to_owned());
    };
    if let Some(test) = wanted
        .iter()
        .find(|test| !listed.cases.iter().any(|c| c.name == **test))
    {
        return Outcome::NotTested(format!("{test} is not a test stickymd-mac's binary lists"));
    }
    if needs_volume && entry.temporary_volume.as_deref() != Some("case-insensitive") {
        return Outcome::NotTested(format!(
            "the temporary volume was not shown case-insensitive ({:?})",
            entry.temporary_volume
        ));
    }
    Outcome::Pass
}
