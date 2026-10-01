//! Validate exact trace pairing and account for the whole legacy shell interval.
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

const MILESTONES: [&str; 6] = [
    "split_mode_begin",
    "split_mode_end",
    "window_show_begin",
    "window_show_end",
    "tool_identity_begin",
    "tool_identity_end",
];

#[derive(Debug, PartialEq)]
pub(super) struct Analysis {
    pub pid: u32,
    pub tray_to_visible_us: u128,
    pub segments_us: [u128; 3],
    pub other_us: u128,
}

pub(super) fn analyze(trace: &str, details: &str) -> Result<Analysis, String> {
    let legacy = crate::startup_trace::parse(trace)?;
    let (details, bound_trace) = split_legacy_trace(details)?;
    if bound_trace.as_bytes() != trace.as_bytes() {
        return Err("startup details belong to a different legacy trace".into());
    }
    let mut lines = details.lines();
    if lines.next() != Some("stickymd_startup_details_v1") {
        return Err("expected stickymd_startup_details_v1 header".into());
    }
    let pid = lines
        .next()
        .and_then(|line| line.strip_prefix("pid="))
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|pid| *pid != 0)
        .ok_or("startup details require a nonzero process ID")?;
    let milestones = crate::startup_trace::ordered(lines, &MILESTONES)?;
    // The parser above established the complete v2 schema, including these names.
    let time = |name| legacy.iter().find(|(key, _)| key == name).unwrap().1;
    let start = time("tray_ready");
    let end = time("window_visible");
    if milestones[0].1 < start || milestones[5].1 > end {
        return Err("startup details fall outside tray_ready..window_visible".into());
    }
    let segments_us = [0, 2, 4].map(|index| milestones[index + 1].1 - milestones[index].1);
    let tray_to_visible_us = end - start;
    // Monotonic, disjoint substeps stay within the parent interval, so neither
    // addition nor subtraction can overflow even for malformed large durations.
    let other_us = tray_to_visible_us - segments_us.iter().sum::<u128>();
    Ok(Analysis {
        pid,
        tray_to_visible_us,
        segments_us,
        other_us,
    })
}

fn split_legacy_trace(details: &str) -> Result<(&str, &str), String> {
    let mut offset = 0;
    for line in details.split_inclusive('\n') {
        if matches!(line, "legacy_trace_begin\n" | "legacy_trace_begin\r\n") {
            return Ok((&details[..offset], &details[offset + line.len()..]));
        }
        offset += line.len();
    }
    Err("startup details lack a complete legacy_trace_begin line".into())
}

impl Analysis {
    pub(super) fn render(&self, json: bool) -> String {
        let [split, show, identity] = self.segments_us;
        if json {
            format!(
                "{{\"schema_version\":1,\"kind\":\"startup-details\",\"status\":\"OBSERVATION_ONLY\",\"qualification_receipt\":false,\"binding\":\"exact_v2_bytes\",\"pid\":{},\"unit\":\"microseconds\",\"tray_to_visible\":{},\"split_mode_application\":{split},\"window_show\":{show},\"tool_window_identity\":{identity},\"other\":{}}}",
                self.pid, self.tray_to_visible_us, self.other_us,
            )
        } else {
            format!(
                "Startup details: OBSERVATION_ONLY; no qualification receipt\npid={} tray_ready..window_visible={} us\nsplit_mode_application={split} us\nwindow_show={show} us\ntool_window_identity={identity} us\nother={} us",
                self.pid, self.tray_to_visible_us, self.other_us,
            )
        }
    }
}
