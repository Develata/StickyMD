//! Interactive human-evidence recorder; it never infers a manual PASS.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use crate::cli::{ManualCommand, ManualSession};
use crate::qualification_environment::{self, QualificationEnvironmentStatus};

use super::json;
use super::receipt::{self, Candidate};

const MANUAL_RECEIPT: &str = "dist/evidence/manual-acceptance.json";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManualCase {
    id: String,
    action: String,
    expected: String,
    session: ManualSession,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManualObservation {
    status: String,
    note: String,
}

pub(super) fn execute(root: &Path, command: ManualCommand) -> Result<(), String> {
    let environment = qualification_environment::inspect();
    println!("Environment: {}", environment.summary());
    match command {
        ManualCommand::List => list(root),
        ManualCommand::Status => status(root),
        ManualCommand::Run { session } => {
            if environment.status != QualificationEnvironmentStatus::Valid {
                return Err("Qualification environment is blocked by locked/non-interactive desktop. Unlock the active Windows session and rerun Phase 13 evidence campaign.".to_owned());
            }
            record(root, session)
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

fn persist(
    root: &Path,
    candidate: &Candidate,
    cases: &[ManualCase],
    observations: &BTreeMap<String, ManualObservation>,
) -> Result<(), String> {
    receipt::write_receipt(
        root,
        MANUAL_RECEIPT,
        &render_manual_receipt(candidate, cases, observations),
    )
}

fn read_manual_cases(root: &Path) -> Result<Vec<ManualCase>, String> {
    let path = root.join("docs/acceptance-cases/phase-12.md");
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    parse_manual_cases(&content)
}

fn parse_manual_cases(content: &str) -> Result<Vec<ManualCase>, String> {
    let mut cases = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if !line.trim_start().starts_with("| P12-M") {
            continue;
        }
        let cells: Vec<_> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 5 || cells[2] != "Manual" {
            return Err(format!(
                "phase-12 manual row {} must have five columns and Manual mode",
                index + 1
            ));
        }
        cases.push(ManualCase {
            id: cells[0].to_owned(),
            action: cells[1].to_owned(),
            expected: cells[3].to_owned(),
            session: session_for_case(cells[0])?,
        });
    }
    if cases.len() != 44 {
        return Err(format!(
            "phase-12 manual matrix must contain exactly 44 cases; observed {}",
            cases.len()
        ));
    }
    Ok(cases)
}

fn session_for_case(id: &str) -> Result<ManualSession, String> {
    let number = id
        .strip_prefix("P12-M")
        .ok_or_else(|| format!("invalid manual case ID `{id}`"))?
        .parse::<u8>()
        .map_err(|error| format!("invalid manual case ID `{id}`: {error}"))?;
    match number {
        1 | 2 | 21 | 24 | 25 | 27 | 31 => Ok(ManualSession::M1),
        3..=20 | 22 | 23 => Ok(ManualSession::M2),
        26 | 28..=30 | 32 | 33 => Ok(ManualSession::M3),
        35..=40 | 43 => Ok(ManualSession::M4),
        34 | 41 | 42 | 44 => Ok(ManualSession::M5),
        _ => Err(format!("manual case `{id}` has no Phase 13 session")),
    }
}

fn load_observations(
    root: &Path,
    candidate: &Candidate,
    cases: &[ManualCase],
) -> Result<BTreeMap<String, ManualObservation>, String> {
    let mut observations = cases
        .iter()
        .map(|case| {
            (
                case.id.clone(),
                ManualObservation {
                    status: "NOT_TESTED".to_owned(),
                    note: String::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let path = root.join(MANUAL_RECEIPT);
    if !path.is_file() {
        return Ok(observations);
    }
    let document = receipt::read_receipt(&path)?;
    for (key, expected) in [
        ("source_commit", candidate.source_commit.as_str()),
        ("exe_sha256", candidate.exe_sha256.as_str()),
        ("zip_sha256", candidate.zip_sha256.as_str()),
    ] {
        let actual = json::string_field(&document, key)?;
        if actual != expected {
            return Err(format!(
                "STALE RECEIPT: manual {key} is {actual}, expected {expected}"
            ));
        }
    }
    for (id, observation) in parse_observations(&document)? {
        if !observations.contains_key(&id) {
            return Err(format!("manual receipt contains unknown case `{id}`"));
        }
        observations.insert(id, observation);
    }
    Ok(observations)
}

fn parse_observations(document: &str) -> Result<Vec<(String, ManualObservation)>, String> {
    let marker = "\"case_id\":";
    let mut observations = Vec::new();
    let mut offset = 0;
    while let Some(relative) = document[offset..].find(marker) {
        let start = offset + relative;
        let next = document[start + marker.len()..]
            .find(marker)
            .map_or(document.len(), |next| start + marker.len() + next);
        let object = &document[start..next];
        let status = json::string_field(object, "status")?;
        if !matches!(
            status.as_str(),
            "MANUAL_PASS" | "MANUAL_FAIL" | "NOT_TESTED"
        ) {
            return Err(format!("manual receipt contains invalid status `{status}`"));
        }
        observations.push((
            json::string_field(object, "case_id")?,
            ManualObservation {
                status,
                note: json::string_field(object, "note").unwrap_or_default(),
            },
        ));
        offset = next;
    }
    Ok(observations)
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

fn render_manual_receipt(
    candidate: &Candidate,
    cases: &[ManualCase],
    observations: &BTreeMap<String, ManualObservation>,
) -> String {
    let windows_build = environment_value("OS");
    let cpu = environment_value("PROCESSOR_IDENTIFIER");
    let mut output = format!(
        concat!(
            "{{\"schema_version\":1,",
            "\"source_commit\":\"{}\",",
            "\"exe_sha256\":\"{}\",",
            "\"zip_sha256\":\"{}\",",
            "\"operator\":\"USER\",",
            "\"environment\":{{\"windows\":\"{}\",\"cpu\":\"{}\"}},",
            "\"cases\":["
        ),
        json::escape(&candidate.source_commit),
        json::escape(&candidate.exe_sha256),
        json::escape(&candidate.zip_sha256),
        json::escape(&windows_build),
        json::escape(&cpu),
    );
    for (index, case) in cases.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        let observation = observations.get(&case.id);
        let status = observation.map_or("NOT_TESTED", |value| value.status.as_str());
        let note = observation.map_or("", |value| value.note.as_str());
        output.push_str(&format!(
            concat!(
                "{{\"case_id\":\"{}\",",
                "\"session\":\"{}\",",
                "\"status\":\"{}\",",
                "\"source_commit\":\"{}\",",
                "\"exe_sha256\":\"{}\",",
                "\"note\":\"{}\"}}"
            ),
            json::escape(&case.id),
            case.session.as_str(),
            status,
            json::escape(&candidate.source_commit),
            json::escape(&candidate.exe_sha256),
            json::escape(note),
        ));
    }
    output.push_str("]}\n");
    output
}

fn environment_value(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| "UNKNOWN".to_owned())
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{parse_manual_cases, prompt_status, session_for_case};
    use crate::cli::ManualSession;
    use std::io::Cursor;

    #[test]
    fn matrix_parser_exposes_only_explicit_manual_cases() {
        let mut content = String::new();
        for number in 1..=44 {
            content.push_str(&format!(
                "| P12-M{number:02} | action | Manual | expected | NOT TESTED |\n"
            ));
        }
        let cases = parse_manual_cases(&content).expect("manual cases");
        assert_eq!(cases.len(), 44);
        assert_eq!(cases[0].id, "P12-M01");
        assert_eq!(cases[0].session, ManualSession::M1);
    }

    #[test]
    fn every_manual_case_maps_to_exactly_one_session() {
        let mut counts = [0_u8; 5];
        for number in 1..=44 {
            let session = session_for_case(&format!("P12-M{number:02}"))
                .expect("every manual case has a session");
            counts[session as usize] += 1;
        }
        assert_eq!(
            counts.iter().map(|value| u16::from(*value)).sum::<u16>(),
            44
        );
        assert!(counts.iter().all(|count| *count > 0));
    }

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
