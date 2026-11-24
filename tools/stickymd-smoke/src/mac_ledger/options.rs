//! Standalone parsing without filesystem access.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use std::path::PathBuf;

/// A batch name: 1–64 ASCII letters, digits, `.`, `_` or `-`. Here, not in the record module,
/// because the parser is compiled on every host.
pub(crate) fn is_batch(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

pub(super) const HELP: &str =
    "usage: stickymd-smoke mac-ledger <plan|record|receipt|waive|revoke> [options]
  The product input fingerprint of a checkout is printed by `stickymd-package app-inputs`:
  build-app.sh writes it into the bundle, so it is computed by the packaging crate.
  plan    --app <StickyMD.app> --driver <stickymd-smoke> --image <digest> --system <file> [--batch <name>]
          Lists the mac-gui rows to run, the rows whose PASS is reused (same product inputs,
          driver, VM image and VM system), and the rows the USER or agents still have to judge
          for these inputs. It writes no ledger and launches no app.
  record  --evidence <mac-gui.json> --image <digest>
          Appends every row of a finished run in the VM. The same evidence is never recorded
          twice for a row. Records are not ordered by time: any FAIL, NOT_TESTED or revoked PASS
          of a row under an identity stops reuse of that row under it.
  receipt --app <StickyMD.app> --rows <ids> --quote <the USER's words> [--batch <name>]
          Appends a USER receipt per Manual row for the app's product inputs on this desktop.
  receipt --release-source <R> --candidate-sha256 <digest> --rows <ids> --quote <the USER's words> [--batch <name>] [--system <file>] [--outcome <result>]
          Appends a release receipt per row, bound to the promoted macOS candidate the USER
          checked: <digest> is what `release macos-candidate` printed then, and must still be
          the candidate the resolver returns for R. Rows: a Manual Phase 16 row, or a download
          path P18-D01..P18-D05 (these need --system, the sw_vers output of the system the path
          was walked on; P18-D04 and P18-D05 also need --outcome, one row at a time:
          D04 gatekeeper-blocked|translocated-prompt-exit|portable-start, D05 blocked|not-blocked).
  waive   --release-source <R> --candidate-sha256 <digest> --rows <ids> --quote <the USER's words> [--batch <name>]
          Appends the USER's waiver per row for that candidate. Only rows the release gate lets
          the USER waive are accepted; a waiver never stands for a FAIL.
  revoke  --rows <id> (--evidence-sha256 <digest> | --app <StickyMD.app> | --release-source <R>) --quote <the USER's words>
          Appends the USER's withdrawal of an automated PASS (named by the digest of the evidence
          it was recorded from), of the row's receipts for the app's product inputs, or of the
          row's release receipts, waivers and download records for R's candidate. A withdrawn
          result stays in the ledger as history. After a withdrawn PASS no PASS of that row under
          the same identity is reused (for I01-I03 and R03: in the same batch).
  --ledger <file> defaults to dist/evidence/mac-gui-ledger/records.jsonl under the repository.
  --image is the Tart image digest the VM was cloned from (the VM cannot see it).
  --system is a file holding the target VM's sw_vers output, taken just before the run.
  --batch names the acceptance batch (1-64 of A-Z a-z 0-9 . _ -). I01-I03 and R03 count only
          results of the batch being planned; every other row ignores it. A run's batch is the one
          mac-gui --batch wrote into its evidence; record takes it from there.
  --quote <words> refuses words that start with -- or are -h; write --quote=<words> to pass
          such words verbatim.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Plan,
    Record,
    Receipt,
    Waive,
    Revoke,
}

impl Command {
    fn name(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Record => "record",
            Self::Receipt => "receipt",
            Self::Waive => "waive",
            Self::Revoke => "revoke",
        }
    }

    /// The options this command accepts; any other is refused rather than silently ignored.
    fn accepts(self, key: &str) -> bool {
        let allowed: &[&str] = match self {
            Self::Plan => &[
                "--ledger", "--app", "--driver", "--image", "--system", "--batch",
            ],
            Self::Record => &["--ledger", "--evidence", "--image"],
            Self::Receipt => &[
                "--ledger",
                "--app",
                "--release-source",
                "--candidate-sha256",
                "--system",
                "--outcome",
                "--rows",
                "--quote",
                "--batch",
            ],
            Self::Waive => &[
                "--ledger",
                "--release-source",
                "--candidate-sha256",
                "--rows",
                "--quote",
                "--batch",
            ],
            Self::Revoke => &[
                "--ledger",
                "--rows",
                "--evidence-sha256",
                "--app",
                "--release-source",
                "--quote",
            ],
        };
        allowed.contains(&key)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Options {
    pub command: Option<Command>,
    pub ledger: Option<PathBuf>,
    pub app: Option<PathBuf>,
    pub driver: Option<PathBuf>,
    pub image: Option<String>,
    pub system: Option<PathBuf>,
    pub evidence: Option<PathBuf>,
    /// For `revoke`: the digest of the evidence an automated PASS was recorded from, lowercased.
    pub evidence_sha256: Option<String>,
    pub batch: Option<String>,
    /// The release commit R whose promoted candidate a release record is bound to.
    pub release: Option<String>,
    /// The candidate receipt digest the USER checked, lowercased (decision (c)).
    pub candidate_sha256: Option<String>,
    /// The observed result of P18-D04 or P18-D05 (decision (b)).
    pub outcome: Option<String>,
    pub rows: Vec<String>,
    pub quote: Option<String>,
    pub help: bool,
}

pub(crate) fn parse(args: &[String]) -> Result<Options, String> {
    let mut result = Options::default();
    let mut rest = args.iter();
    let command = match rest.next().map(String::as_str) {
        Some("inputs") => {
            return Err(
                "mac-ledger inputs moved to the packaging crate: run `cargo run -p stickymd-package -- app-inputs`"
                    .to_owned(),
            );
        }
        Some("plan") => Command::Plan,
        Some("record") => Command::Record,
        Some("receipt") => Command::Receipt,
        Some("waive") => Command::Waive,
        Some("revoke") => Command::Revoke,
        Some("--help" | "-h") | None => {
            result.help = true;
            return Ok(result);
        }
        Some(other) => return Err(format!("unknown mac-ledger command {other}\n{HELP}")),
    };
    result.command = Some(command);
    let mut seen = std::collections::BTreeSet::new();
    while let Some(argument) = rest.next() {
        // `--quote=<words>` is the one inline form: it carries the USER's words verbatim, even
        // words that look like an option.
        let (key, inline) = match argument.split_once('=') {
            Some(("--quote", words)) => ("--quote", Some(words.to_owned())),
            _ => (argument.as_str(), None),
        };
        let key = &key.to_owned();
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate option: {key}"));
        }
        if matches!(key.as_str(), "--help" | "-h") {
            result.help = true;
            continue;
        }
        if !command.accepts(key) {
            return Err(format!(
                "mac-ledger {} does not take {key}\n{HELP}",
                command.name()
            ));
        }
        // A separate value must not look like an option, so a missing value cannot swallow the
        // next option; the inline `--quote=` form is taken as written.
        let value = match inline {
            Some(words) => words,
            None => rest
                .next()
                .filter(|v| !v.starts_with("--") && v.as_str() != "-h")
                .ok_or_else(|| format!("{key} needs a value"))?
                .clone(),
        };
        let value = &value;
        match key.as_str() {
            "--ledger" => result.ledger = Some(value.into()),
            "--app" => result.app = Some(value.into()),
            "--driver" => result.driver = Some(value.into()),
            "--image" => result.image = Some(value.clone()),
            "--system" => result.system = Some(value.into()),
            "--evidence" => result.evidence = Some(value.into()),
            "--evidence-sha256" => result.evidence_sha256 = Some(value.to_ascii_lowercase()),
            "--batch" => {
                if !is_batch(value) {
                    return Err(format!("--batch {value:?} is not 1-64 of [A-Za-z0-9._-]"));
                }
                result.batch = Some(value.clone());
            }
            "--quote" => result.quote = Some(value.clone()),
            "--release-source" => result.release = Some(value.to_ascii_lowercase()),
            "--candidate-sha256" => result.candidate_sha256 = Some(value.to_ascii_lowercase()),
            "--outcome" => result.outcome = Some(value.clone()),
            "--rows" => {
                for id in value.split(',') {
                    let id = id.trim();
                    let id = id.strip_prefix("P16-").unwrap_or(id);
                    if id.is_empty() || result.rows.iter().any(|r| r == id) {
                        return Err(format!("empty or repeated row: {id:?}"));
                    }
                    result.rows.push(id.to_owned());
                }
            }
            _ => unreachable!("accepts() admitted only the options matched here"),
        }
    }
    if result.help {
        return Ok(result);
    }
    let missing = |name: &str| format!("mac-ledger needs {name}\n{HELP}");
    let quoted = |result: &Options| match &result.quote {
        Some(quote) if !quote.trim().is_empty() => Ok(()),
        _ => Err(missing("--quote with the USER's words")),
    };
    match command {
        Command::Plan => {
            result.app.as_ref().ok_or_else(|| missing("--app"))?;
            result.driver.as_ref().ok_or_else(|| missing("--driver"))?;
            result.image.as_ref().ok_or_else(|| missing("--image"))?;
            result.system.as_ref().ok_or_else(|| missing("--system"))?;
        }
        Command::Record => {
            result
                .evidence
                .as_ref()
                .ok_or_else(|| missing("--evidence"))?;
            result.image.as_ref().ok_or_else(|| missing("--image"))?;
        }
        Command::Receipt => {
            if result.app.is_some() == result.release.is_some() {
                return Err(missing("exactly one of --app and --release-source"));
            }
            if (result.system.is_some() || result.outcome.is_some()) && result.release.is_none() {
                return Err(missing("--release-source with --system or --outcome"));
            }
            if result.release.is_some() && result.candidate_sha256.is_none() {
                return Err(missing("--candidate-sha256 with --release-source"));
            }
            if result.rows.is_empty() {
                return Err(missing("--rows"));
            }
            quoted(&result)?;
        }
        Command::Waive => {
            result
                .release
                .as_ref()
                .ok_or_else(|| missing("--release-source"))?;
            result
                .candidate_sha256
                .as_ref()
                .ok_or_else(|| missing("--candidate-sha256"))?;
            if result.rows.is_empty() {
                return Err(missing("--rows"));
            }
            quoted(&result)?;
        }
        Command::Revoke => {
            if result.rows.len() != 1 {
                return Err(missing("--rows with exactly one row"));
            }
            let targets = [
                result.app.is_some(),
                result.evidence_sha256.is_some(),
                result.release.is_some(),
            ];
            if targets.iter().filter(|given| **given).count() != 1 {
                return Err(missing(
                    "exactly one of --evidence-sha256, --app and --release-source",
                ));
            }
            quoted(&result)?;
        }
    }
    Ok(result)
}
