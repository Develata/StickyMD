//! How the gate judges a source-test receipt (plan 11 #macos-readiness-check): identity first,
//! then recorded failures, then completeness. Completeness is checked test by test: every test a
//! binary lists ran and ended as its listing says, and the receipt's targets are R's as
//! `cargo metadata` lists them on R, apart from the receipt.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::{
    Case, CaseOutcome, Counts, EXPLICIT, Exit, Explicit, PACKAGES, Receipt, Target, command,
    driver_command, explicit_command, explicit_target, ignored,
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
    let explicit = |item: &str| match receipt.explicit.iter().find(|e| e.item == item) {
        Some(entry) => explicit_run(entry, item == "same-key"),
        None => Outcome::NotTested(format!("the receipt has no explicit run for {item}")),
    };
    Verdicts {
        tests: main(receipt, targets),
        same_key: explicit("same-key"),
        s06: explicit("P16-S06"),
    }
}

/// The SHA-256 of the `mac-gui` driver the receipt's run built on R, or why it names none: only a
/// receipt of R (the same identity the rows need) built with the documented command can say which
/// driver is R's (review round 4, BLOCKER 1).
pub(in super::super) fn driver_of(receipt: &Receipt, release: &str) -> Result<String, String> {
    identity(receipt, release)?;
    if receipt.driver.command != driver_command() {
        return Err(format!(
            "the receipt's driver was built with {:?}, not the documented command",
            receipt.driver.command
        ));
    }
    receipt
        .driver
        .sha256
        .as_ref()
        .map(|sha256| sha256.to_ascii_lowercase())
        .map_err(|error| format!("the run on R built no driver: {error}"))
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
        // With the four above, this holds exactly when nothing was measured; it also refuses a
        // sum that overflows. A separate `measured == 0` would be implied (review round 3).
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
    // A failure the run printed counts however the process ended; then how it ended: an exit code
    // or a signal is a result, a process that never started or could not be followed is not
    // (review round 4).
    if let Some(run) = receipt.binaries.iter().find(|run| {
        run.counts.failed > 0 || run.cases.iter().any(|c| c.outcome == CaseOutcome::Failed)
    }) {
        return Outcome::Fail(format!("{}: a test failed", named(&run.target)));
    }
    match receipt.exit {
        Exit::Code(0) => {}
        Exit::Code(_) | Exit::Signal => {
            return Outcome::Fail(format!("cargo test ended with {:?}", receipt.exit));
        }
        Exit::NotStarted | Exit::Unknown => {
            return Outcome::NotTested(format!("cargo test has no result: {:?}", receipt.exit));
        }
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
    // Every test a listing marks ignored is classified, whatever form its `ignore` took in the
    // source (review round 4, BLOCKER 3).
    for run in &receipt.binaries {
        if let Some(case) = run.cases.iter().find(|case| {
            case.ignored
                && ignored::role(&run.target.package, &run.target.name, &case.name).is_none()
        }) {
            return Outcome::NotTested(format!(
                "{} is ignored in {} but not classified; classify it first (plan 11)",
                case.name,
                named(&run.target)
            ));
        }
    }
    Outcome::Pass
}

/// An explicit run: the stickymd-mac binary ran exactly its tests, each named in that binary's
/// own listing, and each ok; P18-L08 also needs both volumes as its tests measured them.
fn explicit_run(entry: &Explicit, needs_volume: bool) -> Outcome {
    if matches!(entry.exit, Exit::Code(code) if code != 0)
        || entry.exit == Exit::Signal
        || entry.cases.iter().any(|c| c.outcome == CaseOutcome::Failed)
        || entry.counts.is_some_and(|counts| counts.failed > 0)
    {
        return Outcome::Fail(format!(
            "the explicit run for {} ended with {:?} or recorded a failure",
            entry.item, entry.exit
        ));
    }
    if entry.exit != Exit::Code(0) {
        return Outcome::NotTested(format!(
            "the explicit run for {} has no result: {:?}",
            entry.item, entry.exit
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
    // A failure returned above, so `ran == passed == both` and `adds_up` leave no ignored or
    // measured test: separate checks of those two would be implied (review round 3), and each
    // condition here has its own single-fact sample.
    if ran != wanted || !all_ok || counts.ran != both || counts.passed != both || !counts.adds_up()
    {
        return Outcome::NotTested(format!(
            "the explicit run for {} ran {ran:?} ({counts:?}), not exactly {wanted:?} all ok",
            entry.item
        ));
    }
    // The binary's own listing, taken before the main run, so a main run that stopped early
    // (Cargo stops at the first failing binary) does not void this one (review round 4).
    let mut listed: Vec<&str> = entry.listed.iter().map(String::as_str).collect();
    listed.sort_unstable();
    if listed != wanted {
        return Outcome::NotTested(format!(
            "stickymd-mac's binary lists {listed:?} of {wanted:?}"
        ));
    }
    // P18-L08's two volumes, as the tests measured them where their assertions ran (review
    // round 3, B4): the temporary one must ignore case, the mounted image must not.
    if needs_volume
        && (entry.temporary_volume.as_deref() != Some("case-insensitive")
            || entry.image_volume.as_deref() != Some("case-sensitive"))
    {
        return Outcome::NotTested(format!(
            "the tests did not measure a case-insensitive temporary volume and a case-sensitive image (temporary {:?}, image {:?})",
            entry.temporary_volume, entry.image_volume
        ));
    }
    Outcome::Pass
}
