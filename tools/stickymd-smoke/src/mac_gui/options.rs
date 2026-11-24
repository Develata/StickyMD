//! Standalone parsing without filesystem access.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use std::path::PathBuf;
/// The rows `mac-gui` runs; mac-ledger records no other row from its evidence.
pub(crate) const IDS: &[&str] = &[
    "W01", "W02", "W03", "W04", "W05", "W06", "W07", "W08", "W09", "W10", "W11", "W12", "W15",
    "K02", "I01", "I02", "I03", "E01", "F01", "R03",
];
pub(super) const HELP: &str =
    "usage: stickymd-smoke mac-gui --app <StickyMD.app> [--zip <release ZIP>] [--out <dir>] [--only <ids>] [--batch <name>] [--allow-host]
VM ONLY by default: kern.hv_vmm_present must be 1, checked before app/output access.
--allow-host bypasses only the VM interlock; NEVER use on the owner's desktop.
--only comma-separated W01..W12,W15,K02,I01,I02,I03,E01,F01,R03 (P16- prefix accepted).
--out default: dist/evidence/mac-gui; mac-gui.json is written atomically.
--batch names the acceptance batch this run is for (1-64 of A-Z a-z 0-9 . _ -); it is written
        into mac-gui.json, and mac-ledger counts I01-I03 and R03 only for the batch named there.
--zip   the release ZIP R03 opens as downloaded, instead of zipping --app itself. Before any row
        runs, its StickyMD/StickyMD.app must have the bundle manifest and executable SHA-256 of
        --app, or the run is refused; its SHA-256 is written into mac-gui.json (zip_sha256), and
        the macOS release gate counts R03 only from the candidate's ZIP. Refused unless R03 runs.
Permissions (preflight only, no prompts):
  W01: Accessibility (Dock tree). R03: PostEvent (Return posted to the product alone).
  W02/W03/W04/W06/W07/W08/W09/W11/W12/K02/E01/F01: Accessibility + PostEvent.
  F01: Screen Recording additionally enables its optional screenshot.
W11/W12 remove write permission from the fixture's own note directory to make the save fail,
and give it back before cleanup.
Missing required permissions refuse before copying/launching. Use an unlocked dedicated
macOS VM, US/ABC keyboard input source, and an unwind build.
PASS/FAIL/NOT_TESTED per row and subcheck; FAIL exits 1, NOT_TESTED alone exits 0.
--help and unit tests do not perform real GUI runs.";
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub app: PathBuf,
    pub out: PathBuf,
    pub only: Vec<String>,
    pub batch: Option<String>,
    /// The release ZIP R03 opens (plan 11 #macos-release-gate); without it R03 zips `app`.
    pub zip: Option<PathBuf>,
    pub allow_host: bool,
    pub help: bool,
}
pub(crate) fn parse(args: &[String]) -> Result<Options, String> {
    let mut result = Options {
        app: PathBuf::new(),
        out: "dist/evidence/mac-gui".into(),
        only: Vec::new(),
        batch: None,
        zip: None,
        allow_host: false,
        help: false,
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut iter = args.iter();
    while let Some(key) = iter.next() {
        if !seen.insert(key) {
            return Err(format!("duplicate option: {key}"));
        }
        match key.as_str() {
            "--help" | "-h" => result.help = true,
            "--allow-host" => result.allow_host = true,
            "--app" | "--out" | "--only" | "--batch" | "--zip" => {
                let value = iter
                    .next()
                    .filter(|v| !v.starts_with("--"))
                    .ok_or_else(|| format!("{key} needs a value"))?;
                match key.as_str() {
                    "--app" => result.app = value.into(),
                    "--out" => result.out = value.into(),
                    "--zip" => result.zip = Some(value.into()),
                    "--batch" if crate::mac_ledger::is_batch(value) => {
                        result.batch = Some(value.clone());
                    }
                    "--batch" => {
                        return Err(format!("--batch {value:?} is not 1-64 of [A-Za-z0-9._-]"));
                    }
                    _ => {
                        for id in value.split(',') {
                            let id = id.strip_prefix("P16-").unwrap_or(id);
                            if !IDS.contains(&id) || result.only.iter().any(|s| s == id) {
                                return Err(format!("unknown or repeated scenario: {id}"));
                            }
                            result.only.push(id.into());
                        }
                    }
                }
            }
            _ => return Err(format!("unknown mac-gui option: {key}")),
        }
    }
    if !result.help && result.app.as_os_str().is_empty() {
        return Err(HELP.into());
    }
    // Only R03 opens the ZIP; a ZIP beside a run without R03 would name a package nothing used.
    if result.zip.is_some() && !result.only.is_empty() && !result.only.iter().any(|id| id == "R03")
    {
        return Err("--zip is only for R03, which this run does not include".into());
    }
    Ok(result)
}
