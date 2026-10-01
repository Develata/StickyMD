//! Pure validation of the existing, ordered startup v2 trace.
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

pub(crate) const MILESTONES: [&str; 26] = [
    "process_start",
    "main_enter",
    "program_dir_ready",
    "single_instance_ready",
    "persistence_ready",
    "config_ready",
    "document_ready",
    "event_loop_ready",
    "window_created",
    "surface_ready",
    "display_ready",
    "font_system_begin",
    "source_layout_begin",
    "font_system_end",
    "source_buffer_ready",
    "source_layout_end",
    "source_projection_ready",
    "monitor_ready",
    "tray_ready",
    "window_visible",
    "opacity_ready",
    "topmost_ready",
    "focus_ready",
    "guards_ready",
    "shell_ready",
    "editor_ready",
];

pub(crate) fn parse(content: &str) -> Result<Vec<(String, u128)>, String> {
    let mut lines = content.lines();
    if lines.next() != Some("stickymd_startup_trace_v2") {
        return Err("expected stickymd_startup_trace_v2 header".into());
    }
    ordered(lines, &MILESTONES)
}

pub(crate) fn ordered<'a>(
    mut lines: impl Iterator<Item = &'a str>,
    names: &[&str],
) -> Result<Vec<(String, u128)>, String> {
    let mut values = Vec::with_capacity(names.len());
    for expected in names {
        let line = lines
            .next()
            .ok_or_else(|| format!("missing startup milestone {expected}"))?;
        let (name, value) = line.split_once('=').ok_or("invalid startup trace line")?;
        if name != *expected {
            return Err(format!(
                "startup milestone order mismatch: expected {expected}, got {name}"
            ));
        }
        let value = value
            .parse::<u128>()
            .map_err(|_| format!("invalid startup duration for {name}"))?;
        if values.last().is_some_and(|(_, previous)| *previous > value) {
            return Err("startup milestone durations are not monotonic".into());
        }
        values.push((name.to_owned(), value));
    }
    if lines.next().is_some() {
        return Err("unexpected extra startup milestone".into());
    }
    Ok(values)
}

#[cfg(test)]
pub(crate) fn fixture() -> String {
    include_str!("../tests/fixtures/startup-v2.trace").to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_trace_remains_exactly_twenty_six_monotonic_milestones() {
        let trace = fixture();
        assert_eq!(parse(&trace).unwrap().len(), 26);
        assert_eq!(
            parse(&trace).unwrap(),
            parse(&trace.replace('\n', "\r\n")).unwrap()
        );
        for invalid in [
            trace.replace("_v2", "_v3"),
            trace.replace("tray_ready=1800\n", ""),
            trace.replace("tray_ready=1800", "tray_ready=1800\ntray_ready=1800"),
            trace.replace("tray_ready=1800", "tray_ready=2000"),
            trace.replace("tray_ready=1800", "tray_ready=NaN"),
            format!("{trace}extra=3000\n"),
        ] {
            assert!(parse(&invalid).is_err(), "{invalid}");
        }
    }
}
