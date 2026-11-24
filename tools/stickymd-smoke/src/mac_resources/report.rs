//! JSON evidence and a short per-scenario summary; incomplete acceptance stays explicit. Schema 5
//! names both byte identities of the bundle, the driver and the VM, so `mac-ledger record` can
//! keep P16-R01 and P16-R02 for the release gate (plan 11 #phase-16-macos-host).
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::model::{self, Run, Summary};
use super::rows::{self, Conditions, Reading, RowVerdict};

pub(super) fn quote(value: &str) -> String {
    format!("\"{}\"", crate::evidence::escape_json(value))
}
fn optional(value: Option<impl std::fmt::Display>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

pub(super) struct Evidence {
    pub environment: String,
    /// The system version and display scale the rows are judged under (`10`).
    pub conditions: Conditions,
    pub app_hash: String,
    /// The bundle manifest SHA-256: the release gate binds evidence to both byte identities.
    pub bundle_hash: String,
    /// The product input fingerprint the bundle carries, when `build-app.sh` recorded it.
    pub inputs: Option<String>,
    /// This driver's own executable, so the ledger can tell which driver produced the evidence.
    pub driver: String,
    /// True only on the write after the last run: a checkpoint, or a campaign stopped by a failed
    /// cleanup, is never taken for a finished one (mac-ledger records only finished runs).
    pub finished: bool,
    pub fixture_hash: String,
    pub repetitions: usize,
    pub timebase: (u32, u32),
    pub scenarios: Vec<Vec<Run>>,
}

fn summary(value: Option<Summary>) -> String {
    value.map_or_else(
        || "null".into(),
        |s| format!("{{\"median\":{},\"max\":{}}}", s.median, s.max),
    )
}

fn run_json(run: &Run) -> String {
    let samples = run.samples.iter().map(|s| format!(
        "{{\"elapsed_seconds\":{},\"phys_footprint_bytes\":{},\"user_ticks\":{},\"system_ticks\":{},\"pkg_idle_wakeups\":{},\"interrupt_wakeups\":{}}}",
        s.elapsed_seconds, s.footprint_bytes, s.user_ticks, s.system_ticks, s.pkg_idle_wakeups, s.interrupt_wakeups
    )).collect::<Vec<_>>().join(",");
    let metrics = run.metrics.as_ref().map_or_else(
        || "null".into(),
        |m| {
            format!(
                "{{\"footprint_bytes\":{},\"idle_cpu_percent\":{},\"interrupt_wakeups_per_second\":{},\"idle_wakeups_per_second\":{}}}",
                m.footprint_bytes, m.idle_cpu_percent, m.interrupt_wakeups_per_second, m.idle_wakeups_per_second
            )
        },
    );
    format!(
        "{{\"repetition\":{},\"pid\":{},\"status\":{},\"reason\":{},\"ready_seconds\":{},\"status_item_wait_seconds\":{},\"hidden_seconds\":{},\"exit_observed_seconds\":{},\"exit_status\":{},\"samples\":[{}],\"metrics\":{},\"cleanup_error\":{}}}",
        run.repetition,
        optional(run.pid),
        quote(run.status),
        quote(&run.reason),
        optional(run.ready_seconds),
        optional(run.status_item_wait_seconds),
        optional(run.hidden_seconds),
        optional(run.exit_seconds),
        optional(run.exit_status.as_deref().map(quote)),
        samples,
        metrics,
        optional(run.cleanup_error.as_deref().map(quote))
    )
}

impl Evidence {
    /// P16-R01 and P16-R02 from the runs, by the rule the ledger applies to the written bytes.
    pub fn rows(&self) -> [RowVerdict; 2] {
        let scenarios: Vec<rows::Scenario> = self
            .scenarios
            .iter()
            .zip(model::SCENARIOS)
            .map(|(runs, scenario)| rows::Scenario {
                name: scenario.name,
                limit: scenario.limit,
                runs: runs
                    .iter()
                    .map(|run| {
                        let metrics = run.metrics.as_ref()?;
                        (run.status == "VALID" && run.cleanup_error.is_none())
                            .then(|| {
                                let [before, after] = run.samples.as_slice() else {
                                    return None;
                                };
                                Reading::of(metrics, before, after, self.timebase)
                            })
                            .flatten()
                    })
                    .collect(),
            })
            .collect();
        rows::judge(&scenarios, self.repetitions, &self.conditions)
    }

    /// Both rows' runs are complete and within the lines: PASS once the views are confirmed.
    pub fn measured(&self) -> bool {
        self.rows().iter().all(|row| row.measured)
    }

    pub fn json(&self) -> String {
        let mut scenarios = Vec::new();
        for (index, scenario) in model::SCENARIOS.iter().enumerate() {
            let runs = self.scenarios.get(index).map(Vec::as_slice).unwrap_or(&[]);
            let valid = || {
                runs.iter()
                    .filter(|r| r.status == "VALID" && r.cleanup_error.is_none())
                    .filter_map(|r| r.metrics.as_ref())
            };
            let status = if model::passed(runs, self.repetitions, scenario.limit) {
                "PASS"
            } else {
                "INCOMPLETE_OR_FAILED"
            };
            scenarios.push(format!("{{\"scenario\":{},\"config\":{},\"hidden_through_menu_bar\":{},\"memory_status\":{},\"valid_runs\":{},\"memory_limit_bytes\":{},\"footprint_bytes\":{},\"idle_cpu_percent\":{},\"interrupt_wakeups_per_second\":{},\"idle_wakeups_per_second\":{},\"runs\":[{}]}}",
                quote(scenario.name), quote(&model::config(scenario.mode)), scenario.hidden, quote(status), valid().count(), scenario.limit,
                summary(model::summarize(valid().map(|m| m.footprint_bytes as f64))),
                summary(model::summarize(valid().map(|m| m.idle_cpu_percent))),
                summary(model::summarize(valid().map(|m| m.interrupt_wakeups_per_second))),
                summary(model::summarize(valid().map(|m| m.idle_wakeups_per_second))),
                runs.iter().map(run_json).collect::<Vec<_>>().join(",")));
        }
        let rows = self
            .rows()
            .iter()
            .map(|row| {
                format!(
                    "{{\"id\":{},\"verdict\":{},\"detail\":{}}}",
                    quote(&format!("P16-{}", row.row)),
                    quote(row.verdict),
                    quote(&row.detail)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let inputs = self.inputs.as_ref().map_or_else(String::new, |inputs| {
            format!(",\"app_inputs\":{}", quote(inputs))
        });
        format!(
            "{{\"schema_version\":5,\"tool\":\"mac-resources\",\"status\":\"{}\",\"acceptance_status\":\"INCOMPLETE\",\"finished\":{},\"environment\":{}{inputs},\"app_executable_sha256\":{},\"app_bundle_manifest_sha256\":{},\"driver_sha256\":{},\"fixture_sha256\":{},\"fixture_bytes\":20480,\"formula_count\":20,\"repetitions\":{},\"warmup_seconds\":{},\"idle_seconds\":{},\"mach_timebase\":{{\"numer\":{},\"denom\":{}}},\"cpu_gate\":{{\"statistic\":\"median of the valid runs per scenario (USER 2026-11-24)\",\"hard_failure_above_percent\":{}}},\"wakeups_gate\":null,\"cpu_normalization\":\"one logical CPU = 100 percent\",\"rows\":[{rows}],\"scenarios\":[{}]}}\n",
            if self.measured() {
                "SCENARIOS_PASS"
            } else {
                "INCOMPLETE_OR_FAILED"
            },
            self.finished,
            self.environment,
            quote(&self.app_hash),
            quote(&self.bundle_hash),
            quote(&self.driver),
            quote(&self.fixture_hash),
            self.repetitions,
            crate::resource_plan::WARMUP_SECONDS,
            crate::resource_plan::CPU_SECONDS,
            self.timebase.0,
            self.timebase.1,
            rows::CPU_HARD_FAILURE_PERCENT,
            scenarios.join(",")
        )
    }

    pub fn print_summary(&self) {
        for (index, scenario) in model::SCENARIOS.iter().enumerate() {
            let (mode, limit) = (scenario.name, &scenario.limit);
            let runs = self.scenarios.get(index).map(Vec::as_slice).unwrap_or(&[]);
            let valid = || {
                runs.iter()
                    .filter(|r| r.status == "VALID" && r.cleanup_error.is_none())
                    .filter_map(|r| r.metrics.as_ref())
            };
            let memory = model::summarize(valid().map(|m| m.footprint_bytes as f64 / 1048576.0));
            let cpu = model::summarize(valid().map(|m| m.idle_cpu_percent));
            let idle_wakeups = model::summarize(valid().map(|m| m.idle_wakeups_per_second));
            let wakeups = model::summarize(valid().map(|m| m.interrupt_wakeups_per_second));
            println!(
                "{mode}: {} valid runs; footprint median/max={} MiB (limit {} MiB); idle CPU median/max={}%; interrupt wake-ups median/max={}/s; idle wake-ups median/max={}/s; {}",
                valid().count(),
                readable(memory),
                limit / 1048576,
                cpu.map_or_else(
                    || "unavailable".into(),
                    // Six decimals, as the rows judge it (review round 3).
                    |s| format!("{:.6}/{:.6}", s.median, s.max)
                ),
                readable(wakeups),
                readable(idle_wakeups),
                if model::passed(runs, self.repetitions, *limit) {
                    "memory PASS"
                } else {
                    "memory INCOMPLETE_OR_FAILED: missing/invalid run or memory gate exceeded"
                }
            );
        }
        for row in self.rows() {
            println!("P16-{}: {} ({})", row.row, row.verdict, row.detail);
        }
        println!(
            "Wake-ups are observations only. R02 compares each scenario's median idle CPU (USER 2026-11-24)."
        );
    }
}

fn readable(summary: Option<Summary>) -> String {
    summary.map_or_else(
        || "unavailable".into(),
        |s| format!("{:.3}/{:.3}", s.median, s.max),
    )
}
