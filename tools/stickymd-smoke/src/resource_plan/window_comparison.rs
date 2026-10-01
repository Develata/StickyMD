//! Matched compact-window and native-style resource observations.
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use super::{ResourceCase, case};

#[derive(Clone, Copy, Debug)]
pub(crate) struct WindowCase {
    pub(crate) fixture: ResourceCase,
    pub(crate) width_dip: u32,
    pub(crate) height_dip: u32,
    #[cfg_attr(not(any(windows, test)), allow(dead_code))]
    pub(crate) normal_style: bool,
}

const fn window(
    label: &'static str,
    view: &'static str,
    compact: bool,
    normal: bool,
) -> WindowCase {
    WindowCase {
        fixture: case(label, view, 20, 0, true),
        width_dip: if compact { 220 } else { 520 },
        height_dip: if compact { 120 } else { 680 },
        normal_style: normal,
    }
}

pub(crate) const CASES: &[WindowCase] = &[
    window("source-default-tool", "source", false, false),
    window("source-compact-tool", "source", true, false),
    window("source-default-normal", "source", false, true),
    window("preview-default-tool", "preview", false, false),
    window("preview-compact-tool", "preview", true, false),
    window("preview-default-normal", "preview", false, true),
    window("split-default-tool", "split", false, false),
    window("split-compact-tool", "split", true, false),
    window("split-default-normal", "split", false, true),
];

pub(crate) fn validate_geometry(
    case: WindowCase,
    width: f64,
    height: f64,
    dpi: f64,
) -> Result<(), String> {
    if !dpi.is_finite()
        || !(96.0..=960.0).contains(&dpi)
        || !width.is_finite()
        || !height.is_finite()
        || (width - f64::from(case.width_dip)).abs() > 96.0 / dpi
        || (height - f64::from(case.height_dip)).abs() > 96.0 / dpi
    {
        return Err(format!(
            "{} has unexpected client size or DPI",
            case.fixture.label
        ));
    }
    Ok(())
}

pub(crate) fn validate_matching_dpi(expected: f64, observed: f64) -> Result<(), String> {
    if !expected.is_finite() || expected != observed {
        return Err("comparison DPI changed".into());
    }
    Ok(())
}

pub(super) fn validate_facts(values: &super::observations::Measurements<'_>) -> Result<(), String> {
    let mut cohort_dpi = None;
    for &case in CASES {
        for run in 1..=super::REPETITIONS {
            for stage in ["before", "after"] {
                let read = |field: &str, unit: &str| {
                    let name = format!("{}.run_{run}.{stage}.{field}", case.fixture.label);
                    values
                        .get(name.as_str())
                        .filter(|(value, actual)| value.is_finite() && *actual == unit)
                        .map(|(value, _)| *value)
                        .ok_or_else(|| format!("missing comparison fact {name}"))
                };
                let dpi = read("dpi", "dpi")?;
                validate_geometry(case, read("width", "dip")?, read("height", "dip")?, dpi)?;
                validate_matching_dpi(*cohort_dpi.get_or_insert(dpi), dpi)?;
                if read("style_verified", "count")? != 1.0 {
                    return Err("comparison native style changed".into());
                }
                for name in ["handles", "gdi_objects", "user_objects"] {
                    let count = read(name, "count")?;
                    if count < 0.0 || count > f64::from(u32::MAX) || count.fract() != 0.0 {
                        return Err("invalid comparison object count".into());
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparisons_match_document_view_zoom_and_cpu_protocol() {
        for cases in CASES.chunks_exact(3) {
            assert!(cases[0].fixture.equivalent(cases[1].fixture));
            assert!(cases[0].fixture.equivalent(cases[2].fixture));
            assert!(cases.iter().all(|c| c.fixture.measure_cpu));
            assert_eq!((cases[1].width_dip, cases[1].height_dip), (220, 120));
            assert!(!cases[0].normal_style && !cases[1].normal_style && cases[2].normal_style);
        }
        assert_eq!(CASES.len(), 9);
    }
    #[test]
    fn observed_geometry_must_match_at_actual_dpi() {
        let compact = CASES[1];
        assert!(validate_geometry(compact, 220.0, 120.0, 144.0).is_ok());
        for (width, height, dpi) in [
            (520.0, 680.0, 144.0),
            (220.0, 120.0, 0.0),
            (f64::NAN, 120.0, 96.0),
        ] {
            assert!(validate_geometry(compact, width, height, dpi).is_err());
        }
        assert!(validate_matching_dpi(144.0, 144.0).is_ok());
        // Both geometries are valid individually, but their resource samples
        // cannot be compared across a monitor/DPI change.
        assert!(validate_geometry(compact, 220.0, 120.0, 192.0).is_ok());
        assert!(validate_matching_dpi(144.0, 192.0).is_err());
        assert!(validate_matching_dpi(f64::NAN, f64::NAN).is_err());
    }
    #[test]
    fn old_or_mislabeled_window_observations_cannot_close_comparison_coverage() {
        use super::super::{tests, validate_receipt};
        use crate::cli::ResourceModule;
        let group = ResourceModule::Window;
        let complete = tests::valid_resource_result(group);
        validate_receipt(&tests::document(group, &complete), group).unwrap();
        for suffix in [
            "before.width",
            "after.dpi",
            "after.style_verified",
            "after.gdi_objects",
        ] {
            let mut result = complete.clone();
            let metric = result
                .measurements
                .iter_mut()
                .find(|m| m.name == format!("source-compact-tool.run_5.{suffix}"))
                .unwrap();
            metric.value = -1.0;
            assert!(
                validate_receipt(&tests::document(group, &result), group).is_err(),
                "{suffix}"
            );
        }
        let mut old = complete;
        old.measurements
            .retain(|m| !m.name.contains(".before.") && !m.name.contains(".after."));
        assert!(validate_receipt(&tests::document(group, &old), group).is_err());
    }
}
