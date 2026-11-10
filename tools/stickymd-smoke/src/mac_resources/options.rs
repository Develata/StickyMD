//! Arguments for the standalone macOS measurement command.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use std::path::PathBuf;

pub(super) const HELP: &str =
    "usage: stickymd-smoke mac-resources --app <StickyMD.app> [--out <dir>] [--repetitions N]
  --app          Release bundle to copy into a fresh program directory per run
  --out          Evidence directory (default: dist/evidence/mac-resources)
  --repetitions  Runs per visible scenario, at least 5 (default: 5)
  --help         Print help without measuring or launching anything

macOS only. Requires an unlocked test desktop with its display on.
Source, Preview, Split: wait for a PID-owned on-screen window, warm up 30 s,
then measure 60 s of idle CPU and wake-ups. Memory maxima must be within
72/96/128 MiB. CPU and wake-ups are observations, with no gate.
Hidden: NOT_TESTED; a later GUI driver must use the menu-bar menu (48 MiB gate).
Exit 0 means all requested visible runs are valid and within their memory gates;
it does not mean full P16-R01/P16-R02 acceptance or release readiness.
Off macOS, measurement exits non-zero with NOT_TESTED.
Writes mac-resources.json atomically; no acceptance matrix is updated.";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub(super) app: PathBuf,
    pub(super) out: PathBuf,
    pub(super) repetitions: usize,
    pub(super) help: bool,
}

pub(crate) fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        app: PathBuf::new(),
        out: PathBuf::from("dist/evidence/mac-resources"),
        repetitions: crate::resource_plan::REPETITIONS,
        help: args == ["--help"] || args == ["-h"],
    };
    if options.help {
        return Ok(options);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut args = args.iter();
    while let Some(argument) = args.next() {
        let (key, value) = if let Some(pair) = argument.split_once('=') {
            pair
        } else {
            (
                argument.as_str(),
                args.next()
                    .ok_or_else(|| format!("missing value for {argument}"))?
                    .as_str(),
            )
        };
        if !seen.insert(key) {
            return Err(format!("duplicate option {key}"));
        }
        if value.is_empty() || value.starts_with("--") {
            return Err(format!("missing value for {key}"));
        }
        match key {
            "--app" => options.app = value.into(),
            "--out" => options.out = value.into(),
            "--repetitions" => {
                options.repetitions = value
                    .parse()
                    .map_err(|_| "repetitions must be an integer of at least 5")?;
                if options.repetitions < crate::resource_plan::REPETITIONS {
                    return Err("repetitions must be at least 5".into());
                }
            }
            _ => return Err(format!("unknown mac-resources option {key}")),
        }
    }
    if options.app.as_os_str().is_empty() {
        return Err(format!("--app is required\n{HELP}"));
    }
    Ok(options)
}
