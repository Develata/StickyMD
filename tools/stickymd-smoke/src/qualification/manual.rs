//! Manual acceptance commands: the case registry, the observation context status, the
//! USER's target declaration, an approved reset, the observation recorder, and the export,
//! standalone recording and import of observations made on another machine. Nothing here
//! infers a manual PASS: every status is an explicit human answer.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

mod exchange;
#[cfg(windows)]
mod fixture;
// Launching the candidate reuses the Windows-only exact harness; the loop and the
// selection are portable and tested everywhere.
#[cfg_attr(
    all(not(windows), not(test)),
    expect(
        dead_code,
        reason = "the loop records only on Windows; its tests run everywhere"
    )
)]
mod flow;
#[cfg(windows)]
mod instance;
mod recorder;
mod standalone;

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use crate::cli::{GuidedSession, ManualCommand, StandaloneRecord};

use super::manual_observation::coverage;
use super::manual_observation::reduce::{self, Binding, Verdict};
use super::manual_observation::registry::CASES;
use super::manual_observation::store;
use super::manual_observation::target::{self, Role, RoleTarget, Target};
use super::{decisions, guided, receipt};
use flow::{io_error, read_line};
use recorder::Selection;

pub(super) fn execute(root: &Path, command: ManualCommand) -> Result<(), String> {
    match command {
        ManualCommand::List => {
            list();
            Ok(())
        }
        ManualCommand::Status => status(root),
        ManualCommand::Target => declare_target(root),
        ManualCommand::Reset => reset(root),
        ManualCommand::Run { session } => recorder::record(root, Selection::Cases(session)),
        ManualCommand::Guided { session } => recorder::record(root, Selection::Guided(session)),
        ManualCommand::Export { selection, task } => exchange::export(root, selection, &task),
        ManualCommand::Import { bundle } => exchange::import(root, &bundle),
    }
}

pub(super) fn record_standalone(options: &StandaloneRecord) -> Result<(), String> {
    standalone::record(options)
}

fn unix_now() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))
}

fn list() {
    for case in CASES {
        println!(
            "{} [Tier {}] {}: {}",
            case.id,
            case.tier.as_str(),
            case.session.as_str(),
            case.scenarios.join(", ")
        );
    }
    for session in [GuidedSession::G1, GuidedSession::G2] {
        let steps = guided::STEPS
            .iter()
            .filter(|step| step.session == session)
            .map(|step| format!("{}({})", step.id, step.case_ids.join(",")))
            .collect::<Vec<_>>();
        println!("{}: {}", session.as_str(), steps.join("; "));
    }
    println!("Open manual coverage gaps (plan 11 manual obligation mapping):");
    for gap in coverage::gaps() {
        println!("  - {}", coverage::describe(gap));
    }
    println!("Retired manual obligations:");
    for retired in coverage::retired() {
        println!("  - {}", coverage::describe(retired));
    }
}

/// The reduced verdict per case and role, before tier and waiver policy (readiness).
fn status(root: &Path) -> Result<(), String> {
    let (candidate, digest) = current_candidate(root)?;
    let context = {
        let _guard = store::lock(root, false)?;
        store::load(root, &digest).map_err(|error| error.message())?
    };
    let binding = Binding {
        target: &context.target,
        exe_sha256: &candidate.exe_sha256,
    };
    println!("Manual observation context {}", context.target.context_id);
    for cell in reduce::reduce(&context.log, binding) {
        let verdict = match cell.verdict {
            Verdict::Pass => "PASS".to_owned(),
            Verdict::Fail(scenarios) => format!("FAIL ({})", scenarios.join(", ")),
            Verdict::NotTested { covered, missing } => format!(
                "NOT TESTED ({} of {} scenarios; missing {})",
                covered.len(),
                cell.case.scenarios.len(),
                missing.join(", ")
            ),
            Verdict::Invalid(reason) => format!("INVALID ({reason})"),
        };
        println!("{} {}: {verdict}", cell.case.id, cell.role.as_str());
    }
    let log = &context.log;
    for ((case, role, scenario), indices) in reduce::unresolved_fails(log, Some(binding)) {
        for index in indices {
            let failed = &log.observations[index as usize];
            let note = log
                .notes
                .iter()
                .find(|note| note.session == failed.session && note.seq == failed.seq)
                .map_or("(no note)", |note| note.text.as_str());
            println!(
                "Unresolved FAIL {case}/{scenario} on {}: {}#{} at log position {index}: {note}",
                role.as_str(),
                failed.session,
                failed.seq
            );
        }
    }
    Ok(())
}

fn declare_target(root: &Path) -> Result<(), String> {
    require_terminal()?;
    let (candidate, digest) = current_candidate(root)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    let roles = prompt_roles(&mut input, &mut output)?;
    // Called inside the store's exclusive lock, so an approval is read when it is used.
    let target = store::declare(
        root,
        store::Promoted {
            receipt_sha256: &digest,
            exe_sha256: &candidate.exe_sha256,
        },
        &|| promoted_digest(root),
        roles,
        &|key| decisions::is_approved(root, key),
    )?;
    println!(
        "MANUAL_TARGETS={} context={}",
        root.join(target::TARGET_RECEIPT).display(),
        target.context_id
    );
    Ok(())
}

fn reset(root: &Path) -> Result<(), String> {
    let (candidate, digest) = current_candidate(root)?;
    let target = store::reset(
        root,
        store::Promoted {
            receipt_sha256: &digest,
            exe_sha256: &candidate.exe_sha256,
        },
        &|| promoted_digest(root),
        &|key| decisions::is_approved(root, key),
    )?;
    println!("MANUAL_CONTEXT={}", target.context_id);
    Ok(())
}

/// The candidate receipt's digest as it is now, read from the file alone. Commands re-check
/// it under the store lock, which must not wait on repository subprocesses; the full
/// validation (`current_candidate`) ran when the command started, and a different digest
/// is a different candidate.
fn promoted_digest(root: &Path) -> Result<String, String> {
    receipt::read_candidate_snapshot(root).map(|(_, digest)| digest)
}

/// The digest of the candidate receipt, read once with the candidate it validates.
/// The promoted candidate, checked against the repository, and its receipt digest.
fn current_candidate(root: &Path) -> Result<(receipt::Candidate, String), String> {
    let (candidate, digest) = receipt::read_candidate_snapshot(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    Ok((candidate, digest))
}

fn require_terminal() -> Result<(), String> {
    if io::stdin().is_terminal() {
        Ok(())
    } else {
        Err(
            "this manual command needs an interactive human terminal; piped answers are rejected"
                .to_owned(),
        )
    }
}

/// Asks the USER for both roles and validates them with the receipt parser itself.
fn prompt_roles<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<[RoleTarget; 2], String> {
    let mut answers = Vec::new();
    for role in Role::ALL {
        writeln!(
            output,
            "\nTarget role `{}` (from Microsoft's Windows 11 release information):",
            role.as_str()
        )
        .map_err(io_error)?;
        let mut ask = |question: &str| -> Result<String, String> {
            write!(output, "  {question}: ").map_err(io_error)?;
            output.flush().map_err(io_error)?;
            read_line(input)
        };
        let feature_version = ask("feature version (e.g. 25H2)")?;
        let build_branch = ask("CurrentBuild of that version (e.g. 26200)")?;
        let builds = ask("approved full builds, comma-separated (e.g. 26200.6899,26200.7019)")?;
        let editions = ask("allowed EditionID values, comma-separated (e.g. Core,Professional)")?;
        let machine = ask("native machine (AMD64 or ARM64)")?;
        let source = ask("source of the approved builds (URL or document)")?;
        let selected_on = ask("selection date (YYYY-MM-DD)")?;
        let list = |text: &str| {
            text.split(',')
                .map(|item| format!("\"{}\"", super::json::escape(item.trim())))
                .collect::<Vec<_>>()
                .join(",")
        };
        answers.push(format!(
            "{{\"role\":\"{}\",\"feature_version\":\"{}\",\"build_branch\":\"{}\",\"approved_builds\":[{}],\"edition_ids\":[{}],\"native_machine\":\"{}\",\"source\":\"{}\",\"selected_on\":\"{}\"}}",
            role.as_str(),
            super::json::escape(&feature_version),
            super::json::escape(&build_branch),
            list(&builds),
            list(&editions),
            super::json::escape(&machine),
            super::json::escape(&source),
            super::json::escape(&selected_on),
        ));
    }
    let probe = format!(
        "{{\"schema_version\":1,\"context_id\":\"{}\",\"candidate_receipt_sha256\":\"{}\",\"roles\":[{}]}}",
        "0".repeat(32),
        "0".repeat(64),
        answers.join(",")
    );
    let Target { roles, .. } = target::parse(&probe)?;
    Ok(roles)
}

#[cfg(test)]
mod tests {
    use super::prompt_roles;
    use std::io::Cursor;

    #[test]
    fn declared_roles_are_validated_by_the_receipt_parser() {
        let current = "25H2\n26200\n26200.6899, 26200.7019\nCore,Professional\nAMD64\nhttps://learn.microsoft.com\n2026-10-14\n";
        let previous = "24H2\n26100\n26100.6899\nProfessional\nAMD64\nhttps://learn.microsoft.com\n2026-10-14\n";
        let mut output = Vec::new();
        let roles = prompt_roles(
            &mut Cursor::new(format!("{current}{previous}").into_bytes()),
            &mut output,
        )
        .unwrap();
        assert_eq!(roles[0].approved_ubrs, vec![6899, 7019]);
        assert_eq!(roles[1].edition_ids, vec!["Professional".to_owned()]);
        // The same version for both roles is refused.
        let same = format!("{current}{current}");
        assert!(prompt_roles(&mut Cursor::new(same.into_bytes()), &mut Vec::new()).is_err());
    }
}
