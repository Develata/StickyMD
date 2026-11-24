//! P16-R01 and P16-R02 from the runs of one `mac-resources` campaign (plan 11
//! #phase-16-macos-host, "资源运行进账本"). Each row is judged from what every run recorded — how
//! it ended, its cleanup and its two raw samples — never from a status the tool wrote for a
//! scenario or the campaign. Read back from evidence, every valid run's readings are recomputed
//! from its samples and the Mach timebase and must equal the readings written beside them; the
//! runs of a scenario must be its repetitions 1..=n in order. The same rule writes the evidence's
//! `rows` and, from the evidence's bytes, the ledger's records; evidence whose stated rows
//! disagree with its runs is refused. `10` measures on macOS 27 and memory at scale 2; a
//! campaign measured elsewhere leaves the row NOT_TESTED. No run yet confirms that its scenario
//! opened in the view its configuration names (review F7), so until Develata decides how to
//! confirm it, complete runs within the lines leave the row NOT_TESTED, never PASS; a FAIL stays
//! a FAIL. Two samples at least 60 s apart bound every 60 s window inside them from above, so
//! R02 holds on the CPU time within the 60 s budget, compared exactly in raw ticks; they show a
//! 60 s window above the line only when the window was exactly 60 s, so a FAIL needs that, and a
//! later sample leaves the scenario NOT_TESTED (review round 3, B3; awaiting Develata's decision
//! on a third sample).
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::model::{self, Metrics, SCENARIOS, Sample};
use crate::release::json::Value;
use crate::resource_plan::CPU_SECONDS;

/// `10`'s target table: idle CPU (a 60 s average) above 0.1% is a hard failure. R02 compares each
/// scenario's median over its valid runs with it (USER 2026-11-24, plan 11 #phase-16-macos-host).
pub(super) const CPU_HARD_FAILURE_PERCENT: f64 = 0.1;
/// `10` #macos-gates: memory is measured at display scale 2.
const MEMORY_SCALE: f64 = 2.0;
/// `10`: on the macOS host the measurement environment reads Release build + macOS 27.
const MACOS_MAJOR: &str = "27";

/// `10`'s hard line on a 60 s window, in nanoseconds of CPU: 0.1% of 60 s.
const BUDGET_NANOSECONDS: u128 = 60_000_000;

/// What a valid run read: its `phys_footprint` after the warm-up and its CPU time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Reading {
    pub footprint: u64,
    /// The CPU over the measured window, as a percent of that window (shown, not judged).
    pub idle_cpu: f64,
    /// The CPU time exactly, as nanoseconds times [`Self::denom`]: ticks times the Mach
    /// timebase's numerator (review round 3: the 60 ms budget compared in raw ticks).
    pub cpu_scaled: u128,
    /// The Mach timebase's denominator.
    pub denom: u128,
    /// The measured window, in seconds: at least 60, at most 60.5.
    pub window: f64,
}

impl Reading {
    /// The reading of a valid run from its two samples and their metrics; `None` for readings
    /// no valid run can have.
    pub(super) fn of(
        metrics: &Metrics,
        before: &Sample,
        after: &Sample,
        timebase: (u32, u32),
    ) -> Option<Self> {
        let ticks = u128::from(after.user_ticks.checked_sub(before.user_ticks)?)
            + u128::from(after.system_ticks.checked_sub(before.system_ticks)?);
        Some(Self {
            footprint: metrics.footprint_bytes,
            idle_cpu: metrics.idle_cpu_percent,
            cpu_scaled: ticks * u128::from(timebase.0),
            denom: u128::from(timebase.1),
            window: after.elapsed_seconds - before.elapsed_seconds,
        })
        .filter(|r| r.idle_cpu.is_finite() && r.idle_cpu >= 0.0 && r.footprint > 0 && r.denom > 0)
    }

    /// The CPU time as a percent of the 60 s budget, for display: at least the average of any
    /// 60 s window inside the measured one.
    fn budget_percent(&self) -> f64 {
        self.cpu_scaled as f64 / self.denom as f64 / (CPU_SECONDS as f64 * 1e7)
    }

    /// CPU time some 60 s window inside the measured one is shown to hold: all of it when the
    /// window was exactly 60 s; none otherwise, since two samples do not say where in a longer
    /// window it was spent (review round 3, B3).
    fn proven_scaled(&self) -> u128 {
        if self.window <= CPU_SECONDS as f64 {
            self.cpu_scaled
        } else {
            0
        }
    }
}

/// Whether the median of CPU times scaled by `denom` exceeds the 60 s budget, exactly: the
/// middle value, or the sum of the two middle ones against twice the budget.
fn median_over(mut values: Vec<u128>, denom: u128) -> bool {
    values.sort_unstable();
    let budget = BUDGET_NANOSECONDS * denom;
    match values.len() {
        0 => false,
        n if n % 2 == 1 => values[n / 2] > budget,
        n => values[n / 2 - 1] + values[n / 2] > 2 * budget,
    }
}

/// One scenario as the rows judge it: its memory gate and, per run in order, the reading of a
/// valid run (ended `VALID`, cleaned up, readings from its two samples) or `None`.
#[derive(Clone, Debug)]
pub(super) struct Scenario {
    pub name: &'static str,
    pub limit: u64,
    pub runs: Vec<Option<Reading>>,
}

/// Where the campaign was measured: the system's product version and the main display's scale.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Conditions {
    pub macos_version: String,
    pub scale: [f64; 2],
    /// Active displays: the scale is the campaign's only when there is one.
    pub displays: u32,
}

impl Conditions {
    fn macos_27(&self) -> bool {
        self.macos_version == MACOS_MAJOR
            || self
                .macos_version
                .strip_prefix(MACOS_MAJOR)
                .is_some_and(|rest| rest.starts_with('.'))
    }

    fn scale_2(&self) -> bool {
        self.displays == 1 && self.scale == [MEMORY_SCALE, MEMORY_SCALE]
    }

    fn describe(&self) -> String {
        format!(
            "measured on macOS {} at scale {}x{} with {} display(s)",
            self.macos_version, self.scale[0], self.scale[1], self.displays
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RowVerdict {
    /// `R01` or `R02`, without the `P16-` prefix, as the ledger stores rows.
    pub row: &'static str,
    /// `PASS`, `FAIL` or `NOT_TESTED`.
    pub verdict: &'static str,
    pub detail: String,
    /// The runs are complete and within the line in `10`'s environment: the row would be PASS
    /// once the views are confirmed.
    pub measured: bool,
}

/// Why complete runs within the lines are not yet a PASS (review F7).
pub(super) const VIEW_UNCONFIRMED: &str = "but no run confirmed that its scenario opened in the configured view (review F7; awaiting Develata's decision)";

fn bytes(bytes: u64) -> String {
    format!("{bytes} B ({:.3} MiB)", bytes as f64 / 1048576.0)
}

/// Whether a scenario ran `repetitions` times (at least the plan's five) and every run is valid.
fn complete(scenario: &Scenario, repetitions: usize) -> bool {
    repetitions >= crate::resource_plan::REPETITIONS
        && scenario.runs.len() == repetitions
        && scenario.runs.iter().all(Option::is_some)
}

fn valid(scenario: &Scenario) -> impl Iterator<Item = Reading> + '_ {
    scenario.runs.iter().flatten().copied()
}

/// The two rows. Outside `10`'s environment a row is NOT_TESTED; otherwise a FAIL needs an
/// observed violation; all four scenarios complete within the line are `measured`, which is
/// NOT_TESTED until the views are confirmed ([`VIEW_UNCONFIRMED`]); anything else is
/// NOT_TESTED, naming what is missing.
pub(super) fn judge(
    scenarios: &[Scenario],
    repetitions: usize,
    conditions: &Conditions,
) -> [RowVerdict; 2] {
    let all = scenarios.len() == SCENARIOS.len();
    let incomplete: Vec<String> = scenarios
        .iter()
        .filter(|s| !complete(s, repetitions))
        .map(|s| {
            format!(
                "{} ({} of {repetitions} runs valid)",
                s.name,
                valid(s).count()
            )
        })
        .collect();
    let missing = if !all {
        format!(
            "{} of {} scenarios measured",
            scenarios.len(),
            SCENARIOS.len()
        )
    } else {
        format!("incomplete: {}", incomplete.join(", "))
    };
    let held = all && incomplete.is_empty();
    // R01: every valid run within its scenario's gate; one run over it is a violation.
    let over: Vec<String> = scenarios
        .iter()
        .filter_map(|s| {
            let max = valid(s).map(|r| r.footprint).max()?;
            (max > s.limit).then(|| format!("{} {} > {}", s.name, bytes(max), bytes(s.limit)))
        })
        .collect();
    let r01 = if !(conditions.macos_27() && conditions.scale_2()) {
        (
            "NOT_TESTED",
            format!(
                "{}; `10` measures memory on macOS 27 at scale 2",
                conditions.describe()
            ),
        )
    } else if !over.is_empty() {
        ("FAIL", format!("over the memory gate: {}", over.join(", ")))
    } else if held {
        let maxima: Vec<String> = scenarios
            .iter()
            .map(|s| {
                format!(
                    "{} max {}",
                    s.name,
                    bytes(valid(s).map(|r| r.footprint).max().unwrap_or(0))
                )
            })
            .collect();
        (
            "MEASURED",
            format!("all runs within the memory gates: {}", maxima.join(", ")),
        )
    } else {
        ("NOT_TESTED", missing.clone())
    };
    // R02: each complete scenario's median CPU time against the 60 s budget at the hard line,
    // exactly in raw ticks. The measured window overshoots 60 s by scheduling (at most 0.5 s):
    // within the budget bounds every 60 s window inside it, so the scenario holds; above the
    // line is shown only by runs whose window was exactly 60 s (review B8, round 3 B3), so a
    // FAIL needs their median above it; otherwise the scenario cannot be told either way.
    struct Cpu {
        name: &'static str,
        over: bool,
        within: bool,
        shown: f64,
    }
    let cpu: Vec<Cpu> = scenarios
        .iter()
        .filter(|s| complete(s, repetitions))
        .filter_map(|s| {
            let runs: Vec<Reading> = valid(s).collect();
            let denom = runs.first()?.denom;
            if runs.iter().any(|r| r.denom != denom) {
                return None;
            }
            Some(Cpu {
                name: s.name,
                over: median_over(runs.iter().map(Reading::proven_scaled).collect(), denom),
                within: !median_over(runs.iter().map(|r| r.cpu_scaled).collect(), denom),
                shown: model::summarize(runs.iter().map(Reading::budget_percent))?.median,
            })
        })
        .collect();
    let above: Vec<String> = cpu
        .iter()
        .filter(|c| c.over)
        .map(|c| format!("{} median {:.6}% in 60 s windows", c.name, c.shown))
        .collect();
    let undecided: Vec<String> = cpu
        .iter()
        .filter(|c| !c.over && !c.within)
        .map(|c| format!("{} median {:.6}% of the 60 s budget", c.name, c.shown))
        .collect();
    let listed = cpu
        .iter()
        .map(|c| format!("{} {:.6}%", c.name, c.shown))
        .collect::<Vec<_>>()
        .join(", ");
    let r02 = if !conditions.macos_27() {
        (
            "NOT_TESTED",
            format!("{}; `10` measures on macOS 27", conditions.describe()),
        )
    } else if !above.is_empty() {
        (
            "FAIL",
            format!(
                "idle CPU above {CPU_HARD_FAILURE_PERCENT}% (`10` hard failure): {}",
                above.join(", ")
            ),
        )
    } else if !undecided.is_empty() {
        (
            "NOT_TESTED",
            format!(
                "above the {CPU_HARD_FAILURE_PERCENT}% line over the 60 s budget, but a window longer than 60 s does not show a 60 s window above it (review round 3, B3; awaiting Develata's decision): {}",
                undecided.join(", ")
            ),
        )
    } else if held {
        (
            "MEASURED",
            format!(
                "median idle CPU within {CPU_HARD_FAILURE_PERCENT}% of the 60 s budget: {listed}"
            ),
        )
    } else {
        ("NOT_TESTED", missing)
    };
    let row = |row: &'static str, (verdict, detail): (&'static str, String)| {
        if verdict == "MEASURED" {
            RowVerdict {
                row,
                verdict: "NOT_TESTED",
                detail: format!("{detail}; {VIEW_UNCONFIRMED}"),
                measured: true,
            }
        } else {
            RowVerdict {
                row,
                verdict,
                detail,
                measured: false,
            }
        }
    };
    [row("R01", r01), row("R02", r02)]
}

fn number(value: &Value) -> Result<f64, String> {
    match value {
        Value::Number(text) => text
            .parse::<f64>()
            .map_err(|_| format!("{text:?} is not a number")),
        other => Err(format!("{other:?} is not a number")),
    }
}

fn small(value: &Value) -> Result<u32, String> {
    u32::try_from(value.unsigned()?).map_err(|e| e.to_string())
}

fn sample(value: &Value) -> Result<Sample, String> {
    let count = |key: &str| value.field(key)?.unsigned();
    Ok(Sample {
        elapsed_seconds: number(value.field("elapsed_seconds")?)?,
        footprint_bytes: count("phys_footprint_bytes")?,
        user_ticks: count("user_ticks")?,
        system_ticks: count("system_ticks")?,
        pkg_idle_wakeups: count("pkg_idle_wakeups")?,
        interrupt_wakeups: count("interrupt_wakeups")?,
    })
}

fn metrics(value: &Value) -> Result<Metrics, String> {
    Ok(Metrics {
        footprint_bytes: value.field("footprint_bytes")?.unsigned()?,
        idle_cpu_percent: number(value.field("idle_cpu_percent")?)?,
        interrupt_wakeups_per_second: number(value.field("interrupt_wakeups_per_second")?)?,
        idle_wakeups_per_second: number(value.field("idle_wakeups_per_second")?)?,
    })
}

/// The reading of one run as written, or why the evidence cannot be what this tool writes. A
/// VALID run has exactly two samples whose window and counters give exactly the readings beside
/// them (`model::metrics`, the same function that wrote them; numbers round-trip exactly).
/// How a run of this tool can end (`runner`); any other status is not this tool's.
const STATUSES: [&str; 7] = [
    "VALID",
    "ERROR",
    "EXITED_EARLY",
    "READINESS_TIMEOUT",
    "HIDE_TIMEOUT",
    "NOT_HIDDEN",
    "DISPLAY_CHANGED",
];

fn optional_number(value: &Value, key: &str) -> Result<Option<f64>, String> {
    match value.field(key)? {
        Value::Null => Ok(None),
        other => number(other).map(Some),
    }
}

/// A VALID run's life as the tool drives it (review B1): a process started, its window ready
/// within 30 s, for Hidden hidden through the menu after that, then 30 s of warm-up before the
/// first sample; the process did not exit.
fn lifecycle(run: &Value, hidden: bool, first_sample: f64) -> Result<(), String> {
    let refused = |what: &str| Err(format!("a VALID run {what}, which the tool never writes"));
    if matches!(run.field("pid")?, Value::Null) {
        return refused("names no process");
    }
    if optional_number(run, "exit_observed_seconds")?.is_some()
        || !matches!(run.field("exit_status")?, Value::Null)
    {
        return refused("records an exit");
    }
    let Some(ready) = optional_number(run, "ready_seconds")? else {
        return refused("was never ready");
    };
    if !(0.0..=30.0).contains(&ready) {
        return refused("was ready outside the 30 s readiness wait");
    }
    let hidden_at = optional_number(run, "hidden_seconds")?;
    let waited = optional_number(run, "status_item_wait_seconds")?;
    let start = match (hidden, hidden_at, waited) {
        (true, Some(at), Some(_)) if at >= ready => at,
        (false, None, None) => ready,
        _ => {
            return refused(
                "does not record the hiding its scenario needs, or records one it does not",
            );
        }
    };
    if first_sample < start + crate::resource_plan::WARMUP_SECONDS as f64 {
        return refused("sampled before the 30 s warm-up ended");
    }
    Ok(())
}

fn reading(
    run: &Value,
    timebase: (u32, u32),
    finished: bool,
    hidden: bool,
) -> Result<Option<Reading>, String> {
    let cleaned = matches!(run.field("cleanup_error")?, Value::Null);
    // The tool stops a campaign at the first failed cleanup, before marking it finished.
    if finished && !cleaned {
        return Err("a finished campaign holds a run with a cleanup error, which stops the tool before it finishes".into());
    }
    let status = run.field("status")?.string()?;
    if !STATUSES.contains(&status) {
        return Err(format!("run status {status:?} is not one this tool writes"));
    }
    if status != "VALID" {
        return Ok(None);
    }
    if !cleaned {
        return Err("a VALID run with a cleanup error: the tool marks such a run ERROR".into());
    }
    let samples = run
        .field("samples")?
        .array()?
        .iter()
        .map(sample)
        .collect::<Result<Vec<_>, _>>()?;
    let [before, after] = samples.as_slice() else {
        return Err(format!(
            "a VALID run has {} samples; the tool takes exactly two",
            samples.len()
        ));
    };
    lifecycle(run, hidden, before.elapsed_seconds)?;
    let recomputed = model::metrics(before, after, timebase.0, timebase.1)
        .map_err(|e| format!("a VALID run's samples give no reading: {e}"))?;
    let written = run.field("metrics")?;
    if matches!(written, Value::Null) || metrics(written)? != recomputed {
        return Err("a VALID run's written readings differ from those its samples give".into());
    }
    Ok(Reading::of(&recomputed, before, after, timebase))
}

/// The value of `key: <value>` in `sw_vers` text.
fn sw_vers<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))
        .map(str::trim)
}

/// The conditions, from one environment record whose parts agree (review B3): the system's
/// version and build as `sw_vers` reports them and as the kernel does, a version that is only
/// digits and dots, and a scale that is the display's pixels over its points.
fn conditions(environment: &Value) -> Result<Conditions, String> {
    let text = |key: &str| -> Result<&str, String> { environment.field(key)?.string() };
    let os = text("os")?;
    let version = text("macos_version")?;
    let build = text("os_build")?;
    if sw_vers(os, "ProductVersion") != Some(version) || sw_vers(os, "BuildVersion") != Some(build)
    {
        return Err(format!(
            "the environment disagrees with itself: sw_vers {os:?}, kernel version {version:?} build {build:?}"
        ));
    }
    let parts: Vec<&str> = version.split('.').collect();
    if !(2..=3).contains(&parts.len())
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(format!("{version:?} is not a macOS version"));
    }
    let display = environment.field("main_display")?;
    let size = |key: &str| number(display.field(key)?);
    let scale = [
        size("pixel_width")? / size("point_width")?,
        size("pixel_height")? / size("point_height")?,
    ];
    if scale != [size("scale_x")?, size("scale_y")?] {
        return Err("the display's scale is not its pixels over its points".into());
    }
    Ok(Conditions {
        macos_version: version.to_owned(),
        scale,
        displays: u32::try_from(environment.field("display_count")?.unsigned()?)
            .map_err(|e| e.to_string())?,
    })
}

/// The rows a `mac-resources` evidence file (schema 5) establishes, recomputed from its raw
/// samples. Its scenarios, memory gates and timings must be this tool's, each scenario's runs its
/// repetitions in order, and its stated `rows` must agree with the runs; otherwise it is refused.
pub(crate) fn from_evidence(value: &Value) -> Result<[RowVerdict; 2], String> {
    let unsigned = |key: &str| value.field(key)?.unsigned();
    if unsigned("warmup_seconds")? != crate::resource_plan::WARMUP_SECONDS
        || unsigned("idle_seconds")? != crate::resource_plan::CPU_SECONDS
    {
        return Err("the evidence was measured with other warm-up or idle times".into());
    }
    let repetitions = usize::try_from(unsigned("repetitions")?).map_err(|e| e.to_string())?;
    let finished = matches!(value.field("finished")?, Value::Bool(true));
    let base = value.field("mach_timebase")?;
    let timebase = (small(base.field("numer")?)?, small(base.field("denom")?)?);
    // The measured note is this tool's fixture.
    if value.field("fixture_sha256")?.string()?
        != crate::integrity::sha256_bytes(model::fixture().as_bytes())?
        || unsigned("fixture_bytes")? != 20480
        || unsigned("formula_count")? != 20
    {
        return Err("the evidence measured another note than this tool's fixture".into());
    }
    let conditions = conditions(value.field("environment")?)?;
    let listed = value.field("scenarios")?.array()?;
    if listed.len() > SCENARIOS.len() {
        return Err("the evidence lists more scenarios than this tool measures".into());
    }
    // A finished campaign ran every scenario, each its repetitions times (review S3).
    if finished {
        if listed.len() != SCENARIOS.len() {
            return Err(format!(
                "a finished campaign lists {} of {} scenarios",
                listed.len(),
                SCENARIOS.len()
            ));
        }
        if repetitions < crate::resource_plan::REPETITIONS {
            return Err(format!("a finished campaign of {repetitions} repetitions"));
        }
        for scenario in listed {
            if scenario.field("runs")?.array()?.len() != repetitions {
                return Err(format!(
                    "a finished campaign whose {} ran other than {repetitions} times",
                    scenario.field("scenario")?.string()?
                ));
            }
        }
    }
    let mut scenarios = Vec::new();
    for (scenario, expected) in listed.iter().zip(SCENARIOS) {
        let name = scenario.field("scenario")?.string()?;
        if name != expected.name
            || scenario.field("memory_limit_bytes")?.unsigned()? != expected.limit
            || scenario.field("config")?.string()? != model::config(expected.mode)
            || scenario.field("hidden_through_menu_bar")? != &Value::Bool(expected.hidden)
        {
            return Err(format!(
                "scenario {name:?} is not this tool's {:?} (its gate, view or hiding differs)",
                expected.name
            ));
        }
        let listed_runs = scenario.field("runs")?.array()?;
        if listed_runs.len() > repetitions {
            return Err(format!(
                "scenario {name} lists {} runs for {repetitions} repetitions",
                listed_runs.len()
            ));
        }
        let mut runs = Vec::new();
        for (index, run) in listed_runs.iter().enumerate() {
            if run.field("repetition")?.unsigned()? != index as u64 + 1 {
                return Err(format!(
                    "scenario {name}'s runs are not its repetitions 1..={} in order",
                    listed_runs.len()
                ));
            }
            runs.push(
                reading(run, timebase, finished, expected.hidden)
                    .map_err(|e| format!("{name} run {}: {e}", index + 1))?,
            );
        }
        scenarios.push(Scenario {
            name: expected.name,
            limit: expected.limit,
            runs,
        });
    }
    let judged = judge(&scenarios, repetitions, &conditions);
    // Verdict and detail both: two NOT_TESTED rows for different reasons (views unconfirmed,
    // another scale, a missing run) are different results.
    let stated = value.field("rows")?.array()?;
    let stated: Vec<(String, String, String)> = stated
        .iter()
        .map(|row| {
            Ok((
                row.field("id")?.string()?.to_owned(),
                row.field("verdict")?.string()?.to_owned(),
                row.field("detail")?.string()?.to_owned(),
            ))
        })
        .collect::<Result<_, String>>()?;
    let expected: Vec<(String, String, String)> = judged
        .iter()
        .map(|r| {
            (
                format!("P16-{}", r.row),
                r.verdict.to_owned(),
                r.detail.clone(),
            )
        })
        .collect();
    if stated != expected {
        return Err(format!(
            "the evidence states {stated:?}, but its runs give {expected:?}"
        ));
    }
    Ok(judged)
}
