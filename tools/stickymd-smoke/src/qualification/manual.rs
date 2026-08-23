//! Interactive human-evidence recorder; it never infers a manual PASS.

use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use super::json;
use super::receipt::{self, Candidate};

const MANUAL_RECEIPT: &str = "dist/evidence/manual-acceptance.json";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManualCase {
    id: String,
    action: String,
    expected: String,
}

pub(super) fn record(root: &Path) -> Result<(), String> {
    if !io::stdin().is_terminal() {
        return Err(
            "manual acceptance requires an interactive human terminal; piped answers are rejected"
                .to_owned(),
        );
    }
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    let cases = read_manual_cases(root)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    let mut recorded = Vec::with_capacity(cases.len());
    for case in cases {
        writeln!(output, "\nCase ID: {}", case.id).map_err(io_error)?;
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
        recorded.push((case, status, note));
    }
    let document = render_manual_receipt(&candidate, &recorded);
    receipt::write_receipt(root, MANUAL_RECEIPT, &document)?;
    println!("MANUAL_RECEIPT={}", root.join(MANUAL_RECEIPT).display());
    Ok(())
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
        });
    }
    if cases.is_empty() {
        return Err("phase-12 manual matrix contains no P12-M cases".to_owned());
    }
    Ok(cases)
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
    recorded: &[(ManualCase, &'static str, String)],
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
    for (index, (case, status, note)) in recorded.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            concat!(
                "{{\"case_id\":\"{}\",",
                "\"status\":\"{}\",",
                "\"source_commit\":\"{}\",",
                "\"exe_sha256\":\"{}\",",
                "\"note\":\"{}\"}}"
            ),
            json::escape(&case.id),
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
    use super::{parse_manual_cases, prompt_status};
    use std::io::Cursor;

    #[test]
    fn matrix_parser_exposes_only_explicit_manual_cases() {
        let content = concat!(
            "| P12-A01 | automated | Automated | command | AUTOMATED PASS |\n",
            "| P12-M01 | perform action | Manual | observe result | NOT TESTED |\n",
        );
        let cases = parse_manual_cases(content).expect("manual cases");
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].id, "P12-M01");
        assert_eq!(cases[0].action, "perform action");
        assert_eq!(cases[0].expected, "observe result");
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
