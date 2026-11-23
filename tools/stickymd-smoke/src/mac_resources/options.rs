//! Arguments for the standalone macOS measurement command.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use std::path::PathBuf;

pub(super) const HELP: &str =
    "usage: stickymd-smoke mac-resources --app <StickyMD.app> [--out <dir>] [--repetitions N]
  --app          Release bundle to copy into a fresh program directory per run
  --out          Evidence directory (default: dist/evidence/mac-resources)
  --repetitions  Runs per scenario, at least 5 (default: 5)
  --help         Print help without measuring or launching anything

macOS only. Requires an unlocked test desktop with its display on.
Source, Preview, Split: wait for a PID-owned on-screen window, warm up 30 s,
then measure 60 s of idle CPU and wake-ups. Hidden opens in Source, waits for
the window, chooses the menu-bar item 隐藏 through Accessibility (this binary
needs the permission; checked before any run), waits until the window is off
screen, warms up 30 s from then and measures the same way; the window must
still be off screen at both samples.
P16-R01: every run valid and within 72/96/128/48 MiB. P16-R02: each scenario's
median idle CPU over its valid runs at most 0.1% (plan 10's hard failure line;
USER 2026-11-24). Wake-ups are observations, with no gate.
Exit 0 means both rows PASS; it is not release readiness.
Off macOS, measurement exits non-zero with NOT_TESTED.
Writes mac-resources.json (schema 5) atomically, with the bundle's manifest and
executable SHA-256, its input fingerprint, this driver's SHA-256, sw_vers and
kern.hv_vmm_present; `finished` is true only after the last run. A finished
run in the VM can be recorded with `mac-ledger record --evidence <file>
--image <digest>`; no acceptance matrix is updated.";

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
