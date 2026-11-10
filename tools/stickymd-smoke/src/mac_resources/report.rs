//! JSON evidence and a short visible-scenario summary; incomplete acceptance stays explicit.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use super::model::{self, Run, Summary};

pub(super) fn quote(value: &str) -> String {
    format!("\"{}\"", crate::evidence::escape_json(value))
}
fn optional(value: Option<impl std::fmt::Display>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

pub(super) struct Evidence {
    pub environment: String,
    pub app_hash: String,
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
                "{{\"footprint_bytes\":{},\"idle_cpu_percent\":{},\"wakeups_per_second\":{}}}",
                m.footprint_bytes, m.idle_cpu_percent, m.wakeups_per_second
            )
        },
    );
    format!(
        "{{\"repetition\":{},\"pid\":{},\"status\":{},\"reason\":{},\"ready_seconds\":{},\"exit_observed_seconds\":{},\"exit_status\":{},\"samples\":[{}],\"metrics\":{},\"cleanup_error\":{}}}",
        run.repetition,
        optional(run.pid),
        quote(run.status),
        quote(&run.reason),
        optional(run.ready_seconds),
        optional(run.exit_seconds),
        optional(run.exit_status.as_deref().map(quote)),
        samples,
        metrics,
        optional(run.cleanup_error.as_deref().map(quote))
    )
}

impl Evidence {
    pub fn passed(&self) -> bool {
        self.scenarios.len() == model::SCENARIOS.len()
            && self
                .scenarios
                .iter()
                .zip(model::SCENARIOS)
                .all(|(runs, (_, limit))| model::passed(runs, self.repetitions, limit))
    }

    pub fn json(&self) -> String {
        let mut scenarios = Vec::new();
        for (index, (mode, limit)) in model::SCENARIOS.iter().enumerate() {
            let runs = self.scenarios.get(index).map(Vec::as_slice).unwrap_or(&[]);
            let valid = || {
                runs.iter()
                    .filter(|r| r.status == "VALID" && r.cleanup_error.is_none())
                    .filter_map(|r| r.metrics.as_ref())
            };
            let status = if model::passed(runs, self.repetitions, *limit) {
                "PASS"
            } else {
                "INCOMPLETE_OR_FAILED"
            };
            scenarios.push(format!("{{\"scenario\":{},\"config\":{},\"status\":{},\"valid_runs\":{},\"memory_limit_bytes\":{},\"footprint_bytes\":{},\"idle_cpu_percent\":{},\"wakeups_per_second\":{},\"runs\":[{}]}}",
                quote(mode), quote(&model::config(mode)), quote(status), valid().count(), limit,
                summary(model::summarize(valid().map(|m| m.footprint_bytes as f64))),
                summary(model::summarize(valid().map(|m| m.idle_cpu_percent))),
                summary(model::summarize(valid().map(|m| m.wakeups_per_second))),
                runs.iter().map(run_json).collect::<Vec<_>>().join(",")));
        }
        scenarios.push(format!("{{\"scenario\":\"hidden\",\"status\":\"NOT_TESTED\",\"reason\":{},\"memory_limit_bytes\":50331648,\"runs\":[]}}", quote(model::HIDDEN_REASON)));
        format!(
            "{{\"schema_version\":1,\"status\":\"{}\",\"acceptance_status\":\"INCOMPLETE\",\"environment\":{},\"app_executable_sha256\":{},\"fixture_sha256\":{},\"fixture_bytes\":20480,\"formula_count\":20,\"repetitions\":{},\"warmup_seconds\":{},\"idle_seconds\":{},\"mach_timebase\":{{\"numer\":{},\"denom\":{}}},\"cpu_gate\":null,\"wakeups_gate\":null,\"cpu_normalization\":\"one logical CPU = 100 percent\",\"scenarios\":[{}]}}\n",
            if self.passed() {
                "VISIBLE_SCENARIOS_PASS"
            } else {
                "INCOMPLETE_OR_FAILED"
            },
            self.environment,
            quote(&self.app_hash),
            quote(&self.fixture_hash),
            self.repetitions,
            crate::resource_plan::WARMUP_SECONDS,
            crate::resource_plan::CPU_SECONDS,
            self.timebase.0,
            self.timebase.1,
            scenarios.join(",")
        )
    }

    pub fn print_summary(&self) {
        for (index, (mode, limit)) in model::SCENARIOS.iter().enumerate() {
            let runs = self.scenarios.get(index).map(Vec::as_slice).unwrap_or(&[]);
            let valid = || {
                runs.iter()
                    .filter(|r| r.status == "VALID" && r.cleanup_error.is_none())
                    .filter_map(|r| r.metrics.as_ref())
            };
            let memory = model::summarize(valid().map(|m| m.footprint_bytes as f64 / 1048576.0));
            let cpu = model::summarize(valid().map(|m| m.idle_cpu_percent));
            let wakeups = model::summarize(valid().map(|m| m.wakeups_per_second));
            println!(
                "{mode}: {} valid runs; footprint median/max={} MiB (limit {} MiB); idle CPU median/max={}%; wake-ups median/max={}/s; {}",
                valid().count(),
                readable(memory),
                limit / 1048576,
                readable(cpu),
                readable(wakeups),
                if model::passed(runs, self.repetitions, *limit) {
                    "PASS"
                } else {
                    "INCOMPLETE_OR_FAILED: missing/invalid run or memory gate exceeded"
                }
            );
        }
        println!(
            "hidden: NOT_TESTED (limit 48 MiB): {}",
            model::HIDDEN_REASON
        );
        println!("CPU and wake-ups are observations only. Full acceptance remains INCOMPLETE.");
    }
}

fn readable(summary: Option<Summary>) -> String {
    summary.map_or_else(
        || "unavailable".into(),
        |s| format!("{:.3}/{:.3}", s.median, s.max),
    )
}
