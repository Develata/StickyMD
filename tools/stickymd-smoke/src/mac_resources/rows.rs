//! P16-R01 and P16-R02 from the runs of one `mac-resources` campaign (plan 11
//! #phase-16-macos-host, "资源运行进账本"). Each row is judged from what every run recorded — how
//! it ended, its cleanup and its readings — never from a status the tool wrote for a scenario or
//! the campaign. The same rule writes the evidence's `rows` and, from the evidence's bytes, the
//! ledger's records; evidence whose stated rows disagree with its runs is refused.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::model::{self, SCENARIOS};
use crate::release::json::Value;

/// `10`'s target table: idle CPU (a 60 s average) above 0.1% is a hard failure. R02 compares each
/// scenario's median over its valid runs with it (proposal awaiting USER confirmation, plan 11).
pub(super) const CPU_HARD_FAILURE_PERCENT: f64 = 0.1;

/// What a valid run read: its `phys_footprint` after the warm-up and its idle CPU percent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Reading {
    pub footprint: u64,
    pub idle_cpu: f64,
}

/// One scenario as the rows judge it: its memory gate and, per run in order, the reading of a
/// valid run (ended `VALID`, cleaned up, finite readings) or `None`.
#[derive(Clone, Debug)]
pub(super) struct Scenario {
    pub name: &'static str,
    pub limit: u64,
    pub runs: Vec<Option<Reading>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RowVerdict {
    /// `R01` or `R02`, without the `P16-` prefix, as the ledger stores rows.
    pub row: &'static str,
    /// `PASS`, `FAIL` or `NOT_TESTED`.
    pub verdict: &'static str,
    pub detail: String,
}

fn mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / 1048576.0)
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

/// The two rows. A FAIL needs an observed violation; a PASS needs all four scenarios complete;
/// anything else is NOT_TESTED, naming what is missing.
pub(super) fn judge(scenarios: &[Scenario], repetitions: usize) -> [RowVerdict; 2] {
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
            (max > s.limit).then(|| format!("{} {} > {}", s.name, mib(max), mib(s.limit)))
        })
        .collect();
    let r01 = if !over.is_empty() {
        ("FAIL", format!("over the memory gate: {}", over.join(", ")))
    } else if held {
        let maxima: Vec<String> = scenarios
            .iter()
            .map(|s| {
                format!(
                    "{} max {}",
                    s.name,
                    mib(valid(s).map(|r| r.footprint).max().unwrap_or(0))
                )
            })
            .collect();
        (
            "PASS",
            format!("all runs within the memory gates: {}", maxima.join(", ")),
        )
    } else {
        ("NOT_TESTED", missing.clone())
    };
    // R02: each scenario's median idle CPU over its valid runs, against the hard failure line.
    let medians: Vec<(&str, f64)> = scenarios
        .iter()
        .filter_map(|s| {
            Some((
                s.name,
                model::summarize(valid(s).map(|r| r.idle_cpu))?.median,
            ))
        })
        .collect();
    let above: Vec<String> = medians
        .iter()
        .filter(|(_, median)| *median > CPU_HARD_FAILURE_PERCENT)
        .map(|(name, median)| format!("{name} median {median:.4}%"))
        .collect();
    let listed = medians
        .iter()
        .map(|(name, median)| format!("{name} {median:.4}%"))
        .collect::<Vec<_>>()
        .join(", ");
    let r02 = if !above.is_empty() {
        (
            "FAIL",
            format!(
                "idle CPU above {CPU_HARD_FAILURE_PERCENT}% (`10` hard failure): {}",
                above.join(", ")
            ),
        )
    } else if held {
        (
            "PASS",
            format!("median idle CPU at most {CPU_HARD_FAILURE_PERCENT}%: {listed}"),
        )
    } else {
        ("NOT_TESTED", missing)
    };
    [
        RowVerdict {
            row: "R01",
            verdict: r01.0,
            detail: r01.1,
        },
        RowVerdict {
            row: "R02",
            verdict: r02.0,
            detail: r02.1,
        },
    ]
}

fn number(value: &Value) -> Result<f64, String> {
    match value {
        Value::Number(text) => text
            .parse::<f64>()
            .map_err(|_| format!("{text:?} is not a number")),
        other => Err(format!("{other:?} is not a number")),
    }
}

/// The rows a `mac-resources` evidence file (schema 5) establishes, recomputed from its runs.
/// Its scenarios, memory gates and timings must be this tool's, and its stated `rows` must agree
/// with the runs; otherwise it is refused.
pub(crate) fn from_evidence(value: &Value) -> Result<[RowVerdict; 2], String> {
    let unsigned = |key: &str| value.field(key)?.unsigned();
    if unsigned("warmup_seconds")? != crate::resource_plan::WARMUP_SECONDS
        || unsigned("idle_seconds")? != crate::resource_plan::CPU_SECONDS
    {
        return Err("the evidence was measured with other warm-up or idle times".into());
    }
    let repetitions = usize::try_from(unsigned("repetitions")?).map_err(|e| e.to_string())?;
    let listed = value.field("scenarios")?.array()?;
    if listed.len() > SCENARIOS.len() {
        return Err("the evidence lists more scenarios than this tool measures".into());
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
        let mut runs = Vec::new();
        for run in scenario.field("runs")?.array()? {
            let ended = run.field("status")?.string()? == "VALID";
            let cleaned = matches!(run.field("cleanup_error")?, Value::Null);
            let metrics = run.field("metrics")?;
            let reading = if matches!(metrics, Value::Null) {
                None
            } else {
                let idle_cpu = number(metrics.field("idle_cpu_percent")?)?;
                Some(Reading {
                    footprint: metrics.field("footprint_bytes")?.unsigned()?,
                    idle_cpu,
                })
                .filter(|r| r.idle_cpu.is_finite() && r.idle_cpu >= 0.0 && r.footprint > 0)
            };
            runs.push(reading.filter(|_| ended && cleaned));
        }
        scenarios.push(Scenario {
            name: expected.name,
            limit: expected.limit,
            runs,
        });
    }
    let judged = judge(&scenarios, repetitions);
    let stated = value.field("rows")?.array()?;
    let stated: Vec<(String, String)> = stated
        .iter()
        .map(|row| {
            Ok((
                row.field("id")?.string()?.to_owned(),
                row.field("verdict")?.string()?.to_owned(),
            ))
        })
        .collect::<Result<_, String>>()?;
    let expected: Vec<(String, String)> = judged
        .iter()
        .map(|r| (format!("P16-{}", r.row), r.verdict.to_owned()))
        .collect();
    if stated != expected {
        return Err(format!(
            "the evidence states {stated:?}, but its runs give {expected:?}"
        ));
    }
    Ok(judged)
}
