//! How the macOS release gate judges each Phase 16 row: one table, held to the matrix by a test
//! (every P16 row exactly once, with exactly the Mode it was classified under) and checked again
//! against the matrix the gate reads at run time.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

/// The evidence a Phase 16 row needs for a release (plan 11 #macos-readiness-check).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Class {
    /// Does not depend on the release bytes: the source tests on R stand for it.
    Source,
    /// Does not depend on the release bytes, but its test is `#[ignore]` (it mounts a disk
    /// image), so the default source tests on R do not run it (S06).
    IgnoredSource,
    /// The package integrity class of the gate stands for it (the `.app` the script builds).
    Package,
    /// Run by `mac-gui`: a ledger PASS on the candidate's bytes.
    Gui,
    /// Judged by the USER: a release receipt or waiver bound to the candidate.
    Manual,
    /// Measured by `mac-resources`: a ledger PASS on the candidate's bytes, as for `Gui`.
    Resources,
    /// V03: baseline and new package alternating through `mac-latency` and `mac-resources`;
    /// `mac-latency` is not in the ledger and has no release rule, so only a waiver holds it.
    Comparison,
    /// R03: App Translocation of the release ZIP itself.
    Translocation,
    /// A Windows regression of the shared layer; Windows readiness judges it.
    Windows,
}

/// Every P16 row, its class and the matrix Mode it was classified under, in matrix order.
pub(super) const ROWS: &[(&str, Class, &str)] = &[
    ("B01", Class::Source, "Automated"),
    ("B02", Class::Source, "Automated"),
    ("S01", Class::Source, "Automated"),
    ("S02", Class::Source, "Automated"),
    ("S03", Class::Source, "Automated"),
    ("S04", Class::Source, "Automated"),
    ("S05", Class::Source, "Automated"),
    ("S06", Class::IgnoredSource, "Automated"),
    ("S07", Class::Source, "Automated"),
    ("S08", Class::Source, "Automated"),
    ("S09", Class::Source, "Automated"),
    ("S10", Class::Source, "Automated"),
    ("S11", Class::Source, "Automated"),
    ("S12", Class::Source, "Automated"),
    ("S13", Class::Source, "Automated"),
    ("S14", Class::Source, "Automated"),
    ("S15", Class::Source, "Automated"),
    ("W01", Class::Manual, "Manual"),
    ("W02", Class::Gui, "Automated"),
    ("W03", Class::Gui, "Automated"),
    ("W04", Class::Gui, "Automated"),
    ("W05", Class::Manual, "Manual"),
    ("W06", Class::Manual, "Manual"),
    ("W07", Class::Gui, "Automated"),
    ("W08", Class::Gui, "Automated"),
    ("W09", Class::Gui, "Automated"),
    ("W10", Class::Gui, "Automated"),
    ("W11", Class::Gui, "Automated"),
    ("W12", Class::Gui, "Automated"),
    ("W13", Class::Source, "Automated"),
    ("W14", Class::Source, "Automated"),
    ("W15", Class::Windows, "—"),
    ("W16", Class::Windows, "—"),
    ("W17", Class::Source, "Automated"),
    ("K01", Class::Source, "Automated"),
    ("K02", Class::Gui, "Automated"),
    ("I01", Class::Manual, "Manual"),
    ("I02", Class::Manual, "Manual"),
    ("I03", Class::Manual, "Manual"),
    ("C01", Class::Source, "Automated"),
    ("C02", Class::Source, "Automated"),
    ("C03", Class::Source, "Automated"),
    ("C04", Class::Source, "Automated"),
    ("E01", Class::Gui, "Automated"),
    ("F01", Class::Manual, "Manual"),
    ("P01", Class::Package, "Automated"),
    ("R01", Class::Resources, "Automated"),
    ("R02", Class::Resources, "Automated"),
    ("R03", Class::Translocation, "Automated"),
    ("V01", Class::Source, "Automated"),
    ("V02", Class::Source, "Automated"),
    ("V03", Class::Comparison, "Automated"),
    ("V04", Class::Manual, "Manual"),
    ("V05", Class::Windows, "Manual"),
];

/// Whether a USER release receipt can stand for the Phase 16 row `row` (stored without its
/// prefix): the rows the USER judges (plan 11 #macos-readiness-check).
pub(crate) fn receipt_row(row: &str) -> bool {
    ROWS.iter()
        .any(|(id, class, _)| *id == row && *class == Class::Manual)
}

/// Whether the USER may waive the Phase 16 row `row` (stored without its prefix) for a release:
/// rows that depend on the app running (plan 11 #macos-release-gate). R03, the package class
/// and the source rows may not be waived; Windows rows are not macOS release rows.
pub(crate) fn waivable(row: &str) -> bool {
    ROWS.iter().any(|(id, class, _)| {
        *id == row
            && matches!(
                class,
                Class::Gui | Class::Manual | Class::Resources | Class::Comparison
            )
    })
}

/// The matrix's P16 rows (`governance::phase16_modes` pairs, `P16-` prefixed) must be exactly the
/// table's, in the same order, each with exactly the Mode it was classified under: a Mode change
/// (for example Manual to Review, whose receipts differ) needs the row classified again.
pub(super) fn check_against(modes: &[(String, String)]) -> Result<(), String> {
    let matrix: Vec<(&str, &str)> = modes
        .iter()
        .filter_map(|(id, mode)| Some((id.strip_prefix("P16-")?, mode.as_str())))
        .collect();
    let table: Vec<&str> = ROWS.iter().map(|(id, _, _)| *id).collect();
    let listed: Vec<&str> = matrix.iter().map(|(id, _)| *id).collect();
    if listed != table {
        return Err(format!(
            "the release gate classifies {table:?}, the Phase 16 matrix lists {listed:?}"
        ));
    }
    for ((id, mode), (_, class, classified)) in matrix.iter().zip(ROWS) {
        if mode != classified {
            return Err(format!(
                "P16-{id} has Mode {mode}, but its release class {class:?} was set for Mode {classified}; classify it again"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix() -> String {
        let root =
            crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
        std::fs::read_to_string(root.join("docs/acceptance-cases/phase-16.md")).unwrap()
    }

    #[test]
    fn the_table_covers_every_phase_16_row_with_a_class_its_mode_allows() {
        let modes = crate::governance::phase16_modes(&matrix()).unwrap();
        check_against(&modes).unwrap();
    }

    #[test]
    fn a_row_added_removed_or_with_a_changed_mode_needs_a_new_classification() {
        let modes = crate::governance::phase16_modes(&matrix()).unwrap();
        let mut missing = modes.clone();
        missing.retain(|(id, _)| id != "P16-S03");
        assert!(check_against(&missing).unwrap_err().contains("classifies"));
        let mut extra = modes.clone();
        extra.push(("P16-X01".into(), "Automated".into()));
        assert!(check_against(&extra).unwrap_err().contains("classifies"));
        let mut changed = modes.clone();
        let w01 = changed.iter_mut().find(|(id, _)| id == "P16-W01").unwrap();
        w01.1 = "Automated".into();
        let error = check_against(&changed).unwrap_err();
        // Manual to Review changes which receipt counts (review S7): also a new classification.
        let mut review = modes.clone();
        let w01 = review.iter_mut().find(|(id, _)| id == "P16-W01").unwrap();
        w01.1 = "Review".into();
        assert!(check_against(&review).unwrap_err().contains("again"));
        assert!(
            error.contains("P16-W01") && error.contains("again"),
            "{error}"
        );
    }

    /// Every `Gui` row, and R03, is one `mac-gui` runs; every row `mac-gui` runs is `Gui`,
    /// `Translocation` or a `Manual` row still judged by hand.
    #[test]
    fn the_gui_rows_are_the_rows_mac_gui_runs() {
        let executable = crate::mac_gui::executable_ids();
        for (id, class, _) in ROWS {
            if matches!(class, Class::Gui | Class::Translocation) {
                assert!(executable.contains(id), "mac-gui does not run P16-{id}");
            }
        }
        for id in executable {
            let class = ROWS
                .iter()
                .find(|(row, _, _)| *row == id)
                .map(|(_, c, _)| *c);
            assert!(
                matches!(
                    class,
                    Some(Class::Gui | Class::Translocation | Class::Manual)
                ),
                "mac-gui runs P16-{id}, classified {class:?}"
            );
        }
    }
}
