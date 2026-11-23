//! How the macOS release gate judges each Phase 16 row: one table, held to the matrix by a test
//! (every P16 row exactly once, the class agreeing with the row's Mode) and checked again
//! against the matrix the gate reads at run time.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

/// The evidence a Phase 16 row needs for a release (plan 11 #macos-readiness-check).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Class {
    /// Does not depend on the release bytes: the source tests on R stand for it.
    Source,
    /// The package integrity class of the gate stands for it (the `.app` the script builds).
    Package,
    /// Run by `mac-gui`: a ledger PASS on the candidate's bytes.
    Gui,
    /// Judged by the USER: a release receipt or waiver bound to the candidate.
    Manual,
    /// Measured by `mac-resources` / `mac-latency`, whose evidence does not yet bind both byte
    /// identities and is not recorded in the ledger.
    Resources,
    /// R03: App Translocation of the release ZIP itself.
    Translocation,
    /// A Windows regression of the shared layer; Windows readiness judges it.
    Windows,
}

/// Every P16 row and its class, in matrix order.
pub(super) const ROWS: &[(&str, Class)] = &[
    ("B01", Class::Source),
    ("B02", Class::Source),
    ("S01", Class::Source),
    ("S02", Class::Source),
    ("S03", Class::Source),
    ("S04", Class::Source),
    ("S05", Class::Source),
    ("S06", Class::Source),
    ("S07", Class::Source),
    ("S08", Class::Source),
    ("S09", Class::Source),
    ("S10", Class::Source),
    ("S11", Class::Source),
    ("S12", Class::Source),
    ("S13", Class::Source),
    ("S14", Class::Source),
    ("S15", Class::Source),
    ("W01", Class::Manual),
    ("W02", Class::Gui),
    ("W03", Class::Gui),
    ("W04", Class::Gui),
    ("W05", Class::Manual),
    ("W06", Class::Manual),
    ("W07", Class::Gui),
    ("W08", Class::Gui),
    ("W09", Class::Gui),
    ("W10", Class::Gui),
    ("W11", Class::Gui),
    ("W12", Class::Gui),
    ("W13", Class::Source),
    ("W14", Class::Source),
    ("W15", Class::Windows),
    ("W16", Class::Windows),
    ("W17", Class::Source),
    ("K01", Class::Source),
    ("K02", Class::Gui),
    ("I01", Class::Manual),
    ("I02", Class::Manual),
    ("I03", Class::Manual),
    ("C01", Class::Source),
    ("C02", Class::Source),
    ("C03", Class::Source),
    ("C04", Class::Source),
    ("E01", Class::Gui),
    ("F01", Class::Manual),
    ("P01", Class::Package),
    ("R01", Class::Resources),
    ("R02", Class::Resources),
    ("R03", Class::Translocation),
    ("V01", Class::Source),
    ("V02", Class::Source),
    ("V03", Class::Resources),
    ("V04", Class::Manual),
    ("V05", Class::Windows),
];

/// Whether the USER may waive the Phase 16 row `row` (stored without its prefix) for a release:
/// rows that depend on the app running (plan 11 #macos-release-gate). R03, the package class
/// and the source rows may not be waived; Windows rows are not macOS release rows.
pub(crate) fn waivable(row: &str) -> bool {
    ROWS.iter().any(|(id, class)| {
        *id == row && matches!(class, Class::Gui | Class::Manual | Class::Resources)
    })
}

/// Whether a row of `class` may carry the matrix Mode `mode`. A Mode change makes the table
/// disagree, so the row must be classified again (plan 11 #macos-readiness-check).
fn agrees(class: Class, mode: &str) -> bool {
    match class {
        Class::Source | Class::Package | Class::Gui | Class::Resources | Class::Translocation => {
            mode == "Automated"
        }
        Class::Manual => matches!(mode, "Manual" | "Review" | "Agent"),
        // W15 and W16 are waived in the matrix (mode `—`); V05 is the Windows regression receipt.
        Class::Windows => matches!(mode, "—" | "Manual"),
    }
}

/// The matrix's P16 rows (`governance::phase16_modes` pairs, `P16-` prefixed) must be exactly the
/// table's, in the same order, each with a Mode its class allows.
pub(super) fn check_against(modes: &[(String, String)]) -> Result<(), String> {
    let matrix: Vec<(&str, &str)> = modes
        .iter()
        .filter_map(|(id, mode)| Some((id.strip_prefix("P16-")?, mode.as_str())))
        .collect();
    let table: Vec<&str> = ROWS.iter().map(|(id, _)| *id).collect();
    let listed: Vec<&str> = matrix.iter().map(|(id, _)| *id).collect();
    if listed != table {
        return Err(format!(
            "the release gate classifies {table:?}, the Phase 16 matrix lists {listed:?}"
        ));
    }
    for ((id, mode), (_, class)) in matrix.iter().zip(ROWS) {
        if !agrees(*class, mode) {
            return Err(format!(
                "P16-{id} has Mode {mode}, which its release class {class:?} does not allow; classify it again"
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
        for (id, class) in ROWS {
            if matches!(class, Class::Gui | Class::Translocation) {
                assert!(executable.contains(id), "mac-gui does not run P16-{id}");
            }
        }
        for id in executable {
            let class = ROWS.iter().find(|(row, _)| *row == id).map(|(_, c)| *c);
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
