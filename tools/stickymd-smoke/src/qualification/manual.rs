//! Manual acceptance commands: the case registry, the observation context status, the
//! USER's target declaration and an approved reset. It never infers a manual PASS.
//!
//! The observation recorder arrives with the environment collector (3-A batch 3); until
//! then `run` and `guided` refuse instead of writing the superseded schema 1 receipt.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use crate::cli::{GuidedSession, ManualCommand};

use super::manual_observation::reduce::{self, Verdict};
use super::manual_observation::registry::{CASES, KNOWN_GAPS};
use super::manual_observation::store;
use super::manual_observation::target::{self, Role, RoleTarget, Target};
use super::{decisions, guided, receipt};

pub(super) fn execute(root: &Path, command: ManualCommand) -> Result<(), String> {
    match command {
        ManualCommand::List => {
            list();
            Ok(())
        }
        ManualCommand::Status => status(root),
        ManualCommand::Target => declare_target(root),
        ManualCommand::Reset => reset(root),
        ManualCommand::Run { .. } | ManualCommand::Guided { .. } => Err(
            "manual observations are recorded into the observation log; its recorder lands in 3-A batch 3, so `acceptance manual run/guided` is unavailable until then".to_owned(),
        ),
    }
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
    println!("Known coverage gaps:");
    for gap in KNOWN_GAPS {
        println!("  - {gap}");
    }
}

/// The reduced verdict per case and role, before tier and waiver policy (readiness).
fn status(root: &Path) -> Result<(), String> {
    let candidate = current_candidate(root)?;
    let context = {
        let _guard = store::lock(root, false)?;
        store::load(root, &candidate).map_err(|error| error.message())?
    };
    println!("Manual observation context {}", context.target.context_id);
    for cell in reduce::reduce(&context.log, &context.target) {
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
    Ok(())
}

fn declare_target(root: &Path) -> Result<(), String> {
    require_terminal()?;
    let candidate = current_candidate(root)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    let roles = prompt_roles(&mut input, &mut output)?;
    // Called inside the store's exclusive lock, so an approval is read when it is used.
    let target = store::declare(
        root,
        &candidate,
        &|| current_candidate(root),
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
    let candidate = current_candidate(root)?;
    let target = store::reset(root, &candidate, &|| current_candidate(root), &|key| {
        decisions::is_approved(root, key)
    })?;
    println!("MANUAL_CONTEXT={}", target.context_id);
    Ok(())
}

/// The digest of the candidate receipt, read once with the candidate it validates.
fn current_candidate(root: &Path) -> Result<String, String> {
    let (candidate, digest) = receipt::read_candidate_snapshot(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    Ok(digest)
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

/// Explicit human status for one scenario; `NOT_TESTED` is answered but never recorded.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the observation recorder (3-A batch 3) uses it")
)]
fn prompt_status<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<&'static str, String> {
    loop {
        write!(output, "Status [P=PASS, F=FAIL, N=NOT TESTED]: ").map_err(io_error)?;
        output.flush().map_err(io_error)?;
        match read_line(input)?.to_ascii_uppercase().as_str() {
            "P" | "PASS" => return Ok("MANUAL_PASS"),
            "F" | "FAIL" => return Ok("MANUAL_FAIL"),
            "N" | "NOT TESTED" | "NOT_TESTED" => return Ok("NOT_TESTED"),
            _ => writeln!(output, "Enter P, F, or N explicitly.").map_err(io_error)?,
        }
    }
}

fn read_line<R: BufRead>(input: &mut R) -> Result<String, String> {
    let mut line = String::new();
    let bytes = input.read_line(&mut line).map_err(io_error)?;
    if bytes == 0 {
        return Err("manual input reached end of input".to_owned());
    }
    Ok(line.trim().to_owned())
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{prompt_roles, prompt_status};
    use std::io::Cursor;

    #[test]
    fn recorder_accepts_only_explicit_human_status_tokens() {
        let mut input = Cursor::new(b"maybe\nP\n".to_vec());
        let mut output = Vec::new();
        assert_eq!(prompt_status(&mut input, &mut output), Ok("MANUAL_PASS"));
        assert!(
            String::from_utf8(output)
                .expect("UTF-8")
                .contains("P, F, or N")
        );
    }

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

    #[test]
    fn the_retired_recorder_refuses_and_writes_nothing() {
        let root = std::env::temp_dir().join(format!("stickymd-manual-run-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for command in [
            crate::cli::ManualCommand::Run { session: None },
            crate::cli::ManualCommand::Guided { session: None },
        ] {
            let error = super::execute(&root, command).unwrap_err();
            assert!(error.contains("observation log"), "{error}");
        }
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        // Created exclusively by this test under the temp directory.
        let _ = std::fs::remove_dir_all(root);
    }
}
