//! `mac-latency`: end-to-end input latency of the real app in the test VM, a diagnostic beside
//! plan 10's edit benchmark (which stays the gate). It launches a copied bundle with the product's
//! opt-in recorder (`STICKYMD_DIAGNOSTIC_INPUT_LATENCY`), types through CGEvent, asks the app to
//! write its timestamps (SIGUSR1), and pairs every key with the first frame whose drawing started
//! after the app saw it. Stages: event to main thread, main thread to the start of drawing,
//! drawing through `present` returned. What follows `present` is not measured. Several `--app`
//! values are interleaved run by run, for A/B.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod model;
#[cfg(target_os = "macos")]
mod runner;

use std::path::PathBuf;

pub(crate) const HELP: &str =
    "usage: stickymd-smoke mac-latency --app <StickyMD.app> [--app <B.app> ...] [--out <dir>]
       [--repetitions N] [--keys N] [--interval-ms N] [--idle-keys N] [--idle-ms N] [--allow-host]
VM ONLY by default (kern.hv_vmm_present = 1). Needs Accessibility + PostEvent, an unwind build,
an unlocked desktop and the US/ABC input source.
Each repetition runs every app (in the order given) through four cases: Source view, typing 'x'
at the end of a 100 KiB and of a 1 MiB note, as a sustained burst (--keys at --interval-ms,
defaults 120 at 40, at least 40: each key's down/up takes about 25 ms) and after idle gaps
(--idle-keys at --idle-ms, defaults 10 at 3000). Keys are sent only while the app is frontmost.
--repetitions defaults to 3. Evidence: <out>/mac-latency.json (default dist/evidence/mac-latency).
A run is VALID only when injected keys = recorded keys = characters added to the note and every
key found its frame. Raw recordings are kept under <out>/raw. A cancel or a failed cleanup stops
the campaign. Latency is an observation; plan 10's edit benchmark stays the gate.";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub apps: Vec<PathBuf>,
    pub out: PathBuf,
    pub repetitions: usize,
    pub keys: usize,
    pub interval_ms: u64,
    pub idle_keys: usize,
    pub idle_ms: u64,
    pub allow_host: bool,
    pub help: bool,
}

pub(crate) fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        apps: Vec::new(),
        out: "dist/evidence/mac-latency".into(),
        repetitions: 3,
        keys: 120,
        interval_ms: 40,
        idle_keys: 10,
        idle_ms: 3000,
        allow_host: false,
        help: false,
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut iter = args.iter();
    while let Some(key) = iter.next() {
        if key != "--app" && !seen.insert(key.clone()) {
            return Err(format!("duplicate option: {key}"));
        }
        match key.as_str() {
            "--help" | "-h" => options.help = true,
            "--allow-host" => options.allow_host = true,
            _ => {
                let value = iter
                    .next()
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| format!("{key} needs a value"))?;
                let number = |min: u64, max: u64| -> Result<u64, String> {
                    value
                        .parse::<u64>()
                        .ok()
                        .filter(|n| (min..=max).contains(n))
                        .ok_or_else(|| format!("{key} must be an integer in {min}..={max}"))
                };
                match key.as_str() {
                    "--app" => options.apps.push(value.into()),
                    "--out" => options.out = value.into(),
                    "--repetitions" => options.repetitions = number(1, 50)? as usize,
                    "--keys" => options.keys = number(1, 2000)? as usize,
                    "--interval-ms" => options.interval_ms = number(40, 1000)?,
                    "--idle-keys" => options.idle_keys = number(1, 200)? as usize,
                    "--idle-ms" => options.idle_ms = number(100, 60_000)?,
                    _ => return Err(format!("unknown mac-latency option: {key}")),
                }
            }
        }
    }
    if !options.help && options.apps.is_empty() {
        return Err(HELP.into());
    }
    Ok(options)
}

pub(crate) fn execute(options: &Options) -> Result<(), String> {
    if options.help {
        println!("{HELP}");
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    return runner::execute(options);
    #[cfg(not(target_os = "macos"))]
    Err("mac-latency requires macOS; no application was launched".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn several_apps_keep_their_order_and_numbers_are_bounded() {
        let options = parse(&args(&["--app", "a.app", "--app", "b.app", "--keys", "50"])).unwrap();
        assert_eq!(
            options.apps,
            [PathBuf::from("a.app"), PathBuf::from("b.app")]
        );
        assert_eq!((options.keys, options.repetitions), (50, 3));
        assert!(parse(&args(&["--app", "a.app", "--interval-ms", "39"])).is_err());
        assert!(parse(&args(&["--app", "a.app", "--keys", "1", "--keys", "2"])).is_err());
        assert!(
            parse(&args(&["--keys", "5"])).is_err(),
            "an app is required"
        );
        assert!(parse(&args(&["--help"])).unwrap().help);
    }
}
