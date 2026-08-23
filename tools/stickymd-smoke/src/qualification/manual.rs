//! Interactive human-evidence recorder; it never infers a manual PASS.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use crate::cli::{GuidedSession, ManualCommand, ManualSession};
use crate::qualification_environment::{self, QualificationEnvironmentStatus};

use super::guided;
use super::manual_receipt::{
    MANUAL_RECEIPT, ManualObservation, ManualTier, load_observations, persist, read_manual_cases,
};
use super::receipt;

pub(super) fn execute(root: &Path, command: ManualCommand) -> Result<(), String> {
    let environment = qualification_environment::inspect();
    println!("Environment: {}", environment.summary());
    match command {
        ManualCommand::List => list(root),
        ManualCommand::Status => status(root),
        ManualCommand::Run { session } => {
            if environment.status != QualificationEnvironmentStatus::Valid {
                return Err("Qualification environment is blocked by locked/non-interactive desktop. Unlock the active Windows session and rerun the Phase 14 evidence campaign.".to_owned());
            }
            record(root, session)
        }
        ManualCommand::Guided { session } => {
            if environment.status != QualificationEnvironmentStatus::Valid {
                return Err("Qualification environment is blocked by locked/non-interactive desktop. Unlock the active Windows session and rerun the Phase 14 evidence campaign.".to_owned());
            }
            record_guided(root, session)
        }
    }
}

fn list(root: &Path) -> Result<(), String> {
    let cases = read_manual_cases(root)?;
    for session in all_sessions() {
        let ids = cases
            .iter()
            .filter(|case| case.session == session)
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>();
        println!("{}: {}", session.as_str(), ids.join(", "));
    }
    for session in [GuidedSession::G1, GuidedSession::G2, GuidedSession::G3] {
        let steps = guided::STEPS
            .iter()
            .filter(|step| step.session == session)
            .map(|step| format!("{}({})", step.id, step.case_ids.join(",")))
            .collect::<Vec<_>>();
        println!("{}: {}", session.as_str(), steps.join("; "));
    }
    Ok(())
}

fn status(root: &Path) -> Result<(), String> {
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    let cases = read_manual_cases(root)?;
    let observations = load_observations(root, &candidate, &cases)?;
    for session in all_sessions() {
        let mut passed = 0;
        let mut failed = 0;
        let mut not_tested = 0;
        for case in cases.iter().filter(|case| case.session == session) {
            match observations
                .get(&case.id)
                .map(|observation| observation.status.as_str())
                .unwrap_or("NOT_TESTED")
            {
                "MANUAL_PASS" => passed += 1,
                "MANUAL_FAIL" => failed += 1,
                _ => not_tested += 1,
            }
        }
        println!(
            "{}: MANUAL_PASS={passed} MANUAL_FAIL={failed} NOT_TESTED={not_tested}",
            session.as_str()
        );
    }
    for tier in [ManualTier::A, ManualTier::B, ManualTier::C] {
        let mut passed = 0;
        let mut failed = 0;
        let mut not_tested = 0;
        for case in cases.iter().filter(|case| case.tier == tier) {
            match observations
                .get(&case.id)
                .map(|observation| observation.status.as_str())
                .unwrap_or("NOT_TESTED")
            {
                "MANUAL_PASS" => passed += 1,
                "MANUAL_FAIL" => failed += 1,
                _ => not_tested += 1,
            }
        }
        println!(
            "Tier {}: MANUAL_PASS={passed} MANUAL_FAIL={failed} NOT_TESTED={not_tested}",
            tier.as_str()
        );
    }
    Ok(())
}

fn all_sessions() -> [ManualSession; 5] {
    [
        ManualSession::M1,
        ManualSession::M2,
        ManualSession::M3,
        ManualSession::M4,
        ManualSession::M5,
    ]
}

fn record(root: &Path, session: Option<ManualSession>) -> Result<(), String> {
    if !io::stdin().is_terminal() {
        return Err(
            "manual acceptance requires an interactive human terminal; piped answers are rejected"
                .to_owned(),
        );
    }
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    let cases = read_manual_cases(root)?;
    let mut observations = load_observations(root, &candidate, &cases)?;
    let selected = cases
        .iter()
        .filter(|case| session.is_none_or(|session| case.session == session))
        .cloned()
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err("selected manual session contains no cases".to_owned());
    }

    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    for case in selected {
        writeln!(
            output,
            "\nSession {} / Case ID: {}",
            case.session.as_str(),
            case.id
        )
        .map_err(io_error)?;
        writeln!(output, "What to do: {}", case.action).map_err(io_error)?;
        writeln!(output, "Expected: {}", case.expected).map_err(io_error)?;
        writeln!(
            output,
            "Artifact: commit={} exe={} zip={}",
            candidate.source_commit, candidate.exe_sha256, candidate.zip_sha256
        )
        .map_err(io_error)?;
        let status = prompt_status(&mut input, &mut output)?;
        write!(output, "Note (optional; Enter to leave blank): ").map_err(io_error)?;
        output.flush().map_err(io_error)?;
        let note = read_line(&mut input)?;
        observations.insert(
            case.id,
            ManualObservation {
                status: status.to_owned(),
                note,
            },
        );
        persist(root, &candidate, &cases, &observations)?;
    }
    println!("MANUAL_RECEIPT={}", root.join(MANUAL_RECEIPT).display());
    Ok(())
}

fn record_guided(root: &Path, session: Option<GuidedSession>) -> Result<(), String> {
    if !io::stdin().is_terminal() {
        return Err(
            "guided manual acceptance requires an interactive human terminal; piped answers are rejected"
                .to_owned(),
        );
    }
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    let cases = read_manual_cases(root)?;
    let known = cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let selected = guided::STEPS
        .iter()
        .filter(|step| session.is_none_or(|session| step.session == session))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err("selected guided manual session contains no steps".to_owned());
    }
    for step in &selected {
        for case_id in step.case_ids {
            if !known.contains(case_id) {
                return Err(format!(
                    "guided step {} references unknown case {case_id}",
                    step.id
                ));
            }
        }
    }

    let mut observations = load_observations(root, &candidate, &cases)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    for step in selected {
        writeln!(
            output,
            "\nGuided {} / Step {} / Cases: {}",
            step.session.as_str(),
            step.id,
            step.case_ids.join(", ")
        )
        .map_err(io_error)?;
        writeln!(output, "What to do: {}", step.action).map_err(io_error)?;
        writeln!(output, "Expected: {}", step.expected).map_err(io_error)?;
        let status = prompt_status(&mut input, &mut output)?;
        write!(output, "Observation (optional; Enter to leave blank): ").map_err(io_error)?;
        output.flush().map_err(io_error)?;
        let note = read_line(&mut input)?;
        for case_id in step.case_ids {
            observations.insert(
                (*case_id).to_owned(),
                ManualObservation {
                    status: status.to_owned(),
                    note: format!("{}: {note}", step.id),
                },
            );
        }
        persist(root, &candidate, &cases, &observations)?;
    }
    println!("MANUAL_RECEIPT={}", root.join(MANUAL_RECEIPT).display());
    Ok(())
}

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
        return Err("manual recorder reached end of input".to_owned());
    }
    Ok(line.trim().to_owned())
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::prompt_status;
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
}
