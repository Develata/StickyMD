//! Standalone parsing without filesystem access.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use std::path::PathBuf;

pub(super) const HELP: &str =
    "usage: stickymd-smoke mac-ledger <inputs|plan|record|receipt|revoke> [options]
  inputs  [--home <dir>]
          Prints the product input fingerprint of this checkout and toolchain; build-app.sh
          writes it into the bundle's Info.plist. --home is the caller's home directory, given
          by the caller so that a Cargo configuration cannot redirect where it is looked for.
  plan    --app <StickyMD.app> --driver <stickymd-smoke> --image <digest> --system <file>
          Lists the mac-gui rows to run, the rows whose PASS is reused (same product inputs,
          driver, VM image and VM system), and the manual rows without a USER receipt for these
          inputs. It writes no ledger and launches no app.
  record  --evidence <mac-gui.json> --image <digest>
          Appends every row of a finished run; the evidence must name its driver.
  receipt --app <StickyMD.app> --rows <ids> --quote <the USER's words>
          Appends a USER receipt per row for the app's product inputs on this desktop.
  revoke  --rows <id> (--evidence-sha256 <digest> | --app <StickyMD.app>) --quote <the USER's words>
          Appends the USER's withdrawal of an automated PASS (named by the digest of the evidence
          it was recorded from) or of the row's receipts for the app's product inputs. A
          withdrawn result stays in the ledger as history and is never reused.
  --ledger <file> defaults to dist/evidence/mac-gui-ledger/records.jsonl under the repository.
  --image is the Tart image digest the VM was cloned from (the VM cannot see it).
  --system is a file holding the target VM's sw_vers output, taken just before the run.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Inputs,
    Plan,
    Record,
    Receipt,
    Revoke,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Options {
    pub command: Option<Command>,
    pub ledger: Option<PathBuf>,
    pub app: Option<PathBuf>,
    /// For `inputs`: the home directory whose `.cargo` is Cargo's home, as the caller saw it.
    pub home: Option<PathBuf>,
    pub driver: Option<PathBuf>,
    pub image: Option<String>,
    pub system: Option<PathBuf>,
    pub evidence: Option<PathBuf>,
    /// For `revoke`: the digest of the evidence an automated PASS was recorded from.
    pub evidence_sha256: Option<String>,
    pub rows: Vec<String>,
    pub quote: Option<String>,
    pub help: bool,
}

pub(crate) fn parse(args: &[String]) -> Result<Options, String> {
    let mut result = Options::default();
    let mut rest = args.iter();
    match rest.next().map(String::as_str) {
        Some("inputs") => result.command = Some(Command::Inputs),
        Some("plan") => result.command = Some(Command::Plan),
        Some("record") => result.command = Some(Command::Record),
        Some("receipt") => result.command = Some(Command::Receipt),
        Some("revoke") => result.command = Some(Command::Revoke),
        Some("--help" | "-h") | None => {
            result.help = true;
            return Ok(result);
        }
        Some(other) => return Err(format!("unknown mac-ledger command {other}\n{HELP}")),
    }
    let mut seen = std::collections::BTreeSet::new();
    while let Some(key) = rest.next() {
        if !seen.insert(key) {
            return Err(format!("duplicate option: {key}"));
        }
        if matches!(key.as_str(), "--help" | "-h") {
            result.help = true;
            continue;
        }
        let value = rest
            .next()
            .filter(|v| !v.starts_with("--"))
            .ok_or_else(|| format!("{key} needs a value"))?;
        match key.as_str() {
            "--ledger" => result.ledger = Some(value.into()),
            "--app" => result.app = Some(value.into()),
            "--home" => result.home = Some(value.into()),
            "--driver" => result.driver = Some(value.into()),
            "--image" => result.image = Some(value.clone()),
            "--system" => result.system = Some(value.into()),
            "--evidence" => result.evidence = Some(value.into()),
            "--evidence-sha256" => result.evidence_sha256 = Some(value.clone()),
            "--quote" => result.quote = Some(value.clone()),
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
            _ => return Err(format!("unknown mac-ledger option: {key}")),
        }
    }
    if result.help {
        return Ok(result);
    }
    let missing = |name: &str| format!("mac-ledger needs {name}\n{HELP}");
    match result.command {
        Some(Command::Inputs) => {}
        Some(Command::Plan) => {
            result.app.as_ref().ok_or_else(|| missing("--app"))?;
            result.driver.as_ref().ok_or_else(|| missing("--driver"))?;
            result.image.as_ref().ok_or_else(|| missing("--image"))?;
            result.system.as_ref().ok_or_else(|| missing("--system"))?;
        }
        Some(Command::Record) => {
            result
                .evidence
                .as_ref()
                .ok_or_else(|| missing("--evidence"))?;
            result.image.as_ref().ok_or_else(|| missing("--image"))?;
        }
        Some(Command::Receipt) => {
            result.app.as_ref().ok_or_else(|| missing("--app"))?;
            if result.rows.is_empty() {
                return Err(missing("--rows"));
            }
            match &result.quote {
                Some(quote) if !quote.trim().is_empty() => {}
                _ => return Err(missing("--quote with the USER's words")),
            }
        }
        Some(Command::Revoke) => {
            if result.rows.len() != 1 {
                return Err(missing("--rows with exactly one row"));
            }
            if result.app.is_some() == result.evidence_sha256.is_some() {
                return Err(missing("exactly one of --evidence-sha256 and --app"));
            }
            match &result.quote {
                Some(quote) if !quote.trim().is_empty() => {}
                _ => return Err(missing("--quote with the USER's words")),
            }
        }
        None => unreachable!("a command was matched above"),
    }
    Ok(result)
}
