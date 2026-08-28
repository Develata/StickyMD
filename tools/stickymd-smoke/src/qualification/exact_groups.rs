//! Phase 12 manual rows promoted to exact-candidate desktop evidence.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

pub(crate) const G3_PHASE12_CASES: [&str; 5] =
    ["P12-M28", "P12-M29", "P12-M30", "P12-M32", "P12-M33"];

pub(crate) const G4_PHASE12_CASES: [&str; 13] = [
    "P12-M06", "P12-M07", "P12-M08", "P12-M09", "P12-M10", "P12-M13", "P12-M14", "P12-M15",
    "P12-M16", "P12-M17", "P12-M27", "P12-M31", "P12-M44",
];

pub(crate) fn group_for_phase12_case(id: &str) -> Option<&'static str> {
    if G3_PHASE12_CASES.contains(&id) {
        Some("G3")
    } else if G4_PHASE12_CASES.contains(&id) {
        Some("G4")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{G3_PHASE12_CASES, G4_PHASE12_CASES, group_for_phase12_case};

    #[test]
    fn exact_groups_are_disjoint_and_cover_eighteen_phase12_rows() {
        let ids = G3_PHASE12_CASES
            .into_iter()
            .chain(G4_PHASE12_CASES)
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), 18);
        assert_eq!(group_for_phase12_case("P12-M28"), Some("G3"));
        assert_eq!(group_for_phase12_case("P12-M06"), Some("G4"));
        assert_eq!(group_for_phase12_case("P12-M11"), None);
    }
}
