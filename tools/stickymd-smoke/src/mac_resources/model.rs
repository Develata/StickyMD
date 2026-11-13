//! Deterministic inputs, raw observations, and resource calculations.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

/// One measured scenario: the view the note opens in, its memory gate, and whether the window is
/// hidden through the menu-bar menu before the warm-up.
#[derive(Clone, Copy, Debug)]
pub(super) struct Scenario {
    pub name: &'static str,
    pub mode: &'static str,
    pub limit: u64,
    pub hidden: bool,
}

pub(super) const SCENARIOS: [Scenario; 4] = [
    Scenario {
        name: "source",
        mode: "source",
        limit: 72 * 1024 * 1024,
        hidden: false,
    },
    Scenario {
        name: "preview",
        mode: "preview",
        limit: 96 * 1024 * 1024,
        hidden: false,
    },
    Scenario {
        name: "split",
        mode: "split",
        limit: 128 * 1024 * 1024,
        hidden: false,
    },
    // `10`: "Hidden to tray", 48 MiB. It opens in Source, as a first launch does.
    Scenario {
        name: "hidden",
        mode: "source",
        limit: 48 * 1024 * 1024,
        hidden: true,
    },
];
/// The menu-bar item that hides the window.
pub(super) const HIDE_ITEM: &str = "隐藏";
/// How long the window may stay on screen after the menu-bar command.
#[cfg(target_os = "macos")]
pub(super) const HIDE_SECONDS: u64 = 5;

pub(super) fn fixture() -> String {
    let mut note = String::from("# Resource fixture / 资源测量\n\n");
    for index in 1..=20 {
        note.push_str(&format!("## Section {index} / 章节\n\n$$\nx_{{{index}}} = \\frac{{{index}}}{{1 + {index}}}\n$$\n\n"));
        for _ in 0..8 {
            note.push_str("中文草稿与 Latin text: a deterministic paragraph for layout, editing and preview.\n\n");
        }
    }
    const PADDING: &str = "中文与 English text for the resource baseline.\n";
    while note.len() + PADDING.len() < 20 * 1024 {
        note.push_str(PADDING);
    }
    note.extend(std::iter::repeat_n(' ', 20 * 1024 - note.len() - 1));
    note.push('\n');
    note
}

pub(super) fn config(mode: &str) -> String {
    format!("version = 1\nview_mode = \"{mode}\"\n")
}

#[derive(Clone, Debug)]
pub(super) struct Sample {
    pub elapsed_seconds: f64,
    pub footprint_bytes: u64,
    pub user_ticks: u64,
    pub system_ticks: u64,
    pub pkg_idle_wakeups: u64,
    pub interrupt_wakeups: u64,
}

#[derive(Debug, PartialEq)]
pub(super) struct Metrics {
    pub footprint_bytes: u64,
    pub idle_cpu_percent: f64,
    pub interrupt_wakeups_per_second: f64,
    pub idle_wakeups_per_second: f64,
}

#[derive(Debug)]
pub(super) struct Run {
    pub repetition: usize,
    pub pid: Option<u32>,
    pub status: &'static str,
    pub reason: String,
    pub ready_seconds: Option<f64>,
    /// When the window was first seen off screen after the menu-bar command (Hidden only).
    pub hidden_seconds: Option<f64>,
    pub exit_seconds: Option<f64>,
    pub exit_status: Option<String>,
    pub samples: Vec<Sample>,
    pub metrics: Option<Metrics>,
    pub cleanup_error: Option<String>,
}

impl Run {
    pub fn new(repetition: usize) -> Self {
        Self {
            repetition,
            pid: None,
            status: "ERROR",
            reason: String::new(),
            ready_seconds: None,
            hidden_seconds: None,
            exit_seconds: None,
            exit_status: None,
            samples: Vec::new(),
            metrics: None,
            cleanup_error: None,
        }
    }
    pub fn exited(&mut self, elapsed: f64, status: String) {
        self.status = "EXITED_EARLY";
        self.reason =
            "Process exited before sampling completed; exit time is the poll observation time"
                .into();
        self.exit_seconds = Some(elapsed);
        self.exit_status = Some(status);
        self.metrics = None;
    }
}

pub(super) fn metrics(
    before: &Sample,
    after: &Sample,
    numer: u32,
    denom: u32,
) -> Result<Metrics, String> {
    let seconds = after.elapsed_seconds - before.elapsed_seconds;
    if !seconds.is_finite()
        || seconds < crate::resource_plan::CPU_SECONDS as f64
        || before.footprint_bytes == 0
        || after.footprint_bytes == 0
        || numer == 0
        || denom == 0
    {
        return Err("Invalid sampling interval, footprint or Mach timebase".into());
    }
    let delta = |a: u64, b: u64| b.checked_sub(a).ok_or("Resource counter decreased");
    let cpu = u128::from(delta(before.user_ticks, after.user_ticks)?)
        + u128::from(delta(before.system_ticks, after.system_ticks)?);
    // XNU credits package-idle wakeups inside the interrupt branch of thread_unblock:
    // idle is a subset, not an additional source to add to the total.
    Ok(Metrics {
        footprint_bytes: before.footprint_bytes,
        idle_cpu_percent: cpu as f64 * f64::from(numer) / f64::from(denom) / 1e9 / seconds * 100.0,
        interrupt_wakeups_per_second: delta(before.interrupt_wakeups, after.interrupt_wakeups)?
            as f64
            / seconds,
        idle_wakeups_per_second: delta(before.pkg_idle_wakeups, after.pkg_idle_wakeups)? as f64
            / seconds,
    })
}

#[derive(Debug, PartialEq)]
pub(super) struct Summary {
    pub median: f64,
    pub max: f64,
}

pub(super) fn summarize(values: impl Iterator<Item = f64>) -> Option<Summary> {
    let mut values = values.collect::<Vec<_>>();
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    Some(Summary {
        median: if values.len().is_multiple_of(2) {
            values[mid - 1] / 2.0 + values[mid] / 2.0
        } else {
            values[mid]
        },
        max: *values.last()?,
    })
}

pub(super) fn passed(runs: &[Run], expected: usize, limit: u64) -> bool {
    expected >= crate::resource_plan::REPETITIONS
        && runs.len() == expected
        && runs.iter().all(|run| {
            run.status == "VALID"
                && run.cleanup_error.is_none()
                && run
                    .metrics
                    .as_ref()
                    .is_some_and(|metrics| metrics.footprint_bytes <= limit)
        })
}
