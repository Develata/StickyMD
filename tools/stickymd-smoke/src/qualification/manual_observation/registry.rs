//! The Phase 12 manual cases: tier, session and the scenarios a PASS must cover. The case
//! set equals the `Manual` rows of `docs/acceptance-cases/phase-12.md` (tested), and the
//! recorder, the reduction and the waiver checks all read this one table.

use crate::cli::ManualSession;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(in crate::qualification) enum Tier {
    A,
    B,
    C,
}

impl Tier {
    pub(in crate::qualification) const fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Case {
    pub(in crate::qualification) id: &'static str,
    pub(in crate::qualification) tier: Tier,
    pub(in crate::qualification) session: ManualSession,
    /// Scenario IDs, unique within the case; a case passes only when every one passes.
    pub(in crate::qualification) scenarios: &'static [&'static str],
}

const fn case(
    id: &'static str,
    tier: Tier,
    session: ManualSession,
    scenarios: &'static [&'static str],
) -> Case {
    Case {
        id,
        tier,
        session,
        scenarios,
    }
}

use ManualSession::{M1, M2, M3, M4, M5};
use Tier::{A, B, C};

pub(in crate::qualification) const CASES: &[Case] = &[
    case(
        "P12-M01",
        A,
        M1,
        &["pinyin-100", "pinyin-150", "pinyin-200"],
    ),
    case(
        "P12-M02",
        A,
        M1,
        &["wetype-100", "wetype-150", "wetype-200"],
    ),
    case(
        "P12-M05",
        A,
        M2,
        &["restore-click", "restore-tray", "restore-sensor"],
    ),
    case("P12-M11", A, M2, &["left-dock-mixed-dpi"]),
    case("P12-M12", A, M2, &["right-dock-mixed-dpi"]),
    case("P12-M18", A, M2, &["compact-source"]),
    case("P12-M19", A, M2, &["compact-preview"]),
    case("P12-M20", A, M2, &["compact-split"]),
    case(
        "P12-M21",
        A,
        M1,
        &[
            "source-50",
            "source-100",
            "source-300",
            "preview-50",
            "preview-100",
            "preview-300",
            "split-50",
            "split-100",
            "split-300",
            "wheel-and-reset",
        ],
    ),
    case("P12-M22", A, M2, &["opacity-40"]),
    case(
        "P12-M23",
        A,
        M2,
        &[
            "light",
            "dark",
            "system",
            "system-to-dark",
            "system-to-light",
        ],
    ),
    case("P12-M24", A, M1, &["representative-markdown"]),
    case("P12-M25", A, M1, &["formulas"]),
    case("P12-M26", A, M3, &["image-formats"]),
    case("P12-M34", B, M5, &["clean-vm-first-run"]),
    case("P12-M35", B, M4, &["drag", "dock", "tray-restore"]),
    case("P12-M36", B, M4, &["drag", "dock", "ime", "preview"]),
    case("P12-M37", B, M4, &["disconnect-current-monitor"]),
    case("P12-M38", B, M4, &["input", "preview", "dock"]),
    case("P12-M39", B, M4, &["input", "preview", "dock"]),
    case("P12-M40", B, M4, &["input", "preview", "dock"]),
    case("P12-M41", C, M5, &["sleep-resume"]),
    case("P12-M42", C, M5, &["rdp-reconnect"]),
    case("P12-M43", C, M4, &["secondary-left", "secondary-above"]),
];

/// Phase 8 manual obligations that no P12 case owns yet. Readiness lists them on every run,
/// whatever the verdict, so a passing matrix is never presented as complete.
pub(in crate::qualification) const KNOWN_GAPS: &[&str] = &[
    "P08-M01/M02: left and right dock at 100% DPI and the >16 DIP undock",
    "P08-M03: top-dock visual (horizontal sensor strip and width)",
    "P08-M04: real IME on the right and top docks",
    "P08-M05: Quit cancelled by a failed save; Topmost kept in sync with the tray",
    "P08-D031: Alt+F4 hides to the tray",
    "P08-M06: opacity 70/96/100 across every layer",
    "P08-M08: second launch from a minimized or docked instance, with foreground activation",
    "P08-M09: two portable copies with separate tray icons used independently",
    "P08-M10: primary-monitor switch and above/below arrangements",
    "P08-M12: Windows 11 shell visual (corners, shadow, topmost button, control fade, edge and corner resize)",
    "plan 09: scrollbar grab and no text overlap under each DPI and theme",
    "plan 09: real-desktop Esc and collapse-button collapse",
];

pub(in crate::qualification) fn find(id: &str) -> Option<&'static Case> {
    CASES.iter().find(|case| case.id == id)
}

/// Canonical text of the table; its digest binds an observation context to this registry.
pub(in crate::qualification) fn canonical() -> String {
    let mut text = String::from("stickymd-manual-registry-v1\n");
    for case in CASES {
        text.push_str(case.id);
        text.push('|');
        text.push_str(case.tier.as_str());
        text.push('|');
        text.push_str(case.session.as_str());
        for scenario in case.scenarios {
            text.push('|');
            text.push_str(scenario);
        }
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{CASES, KNOWN_GAPS, Tier};

    #[test]
    fn registry_equals_the_phase12_manual_rows_with_unique_scenarios() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let text = std::fs::read_to_string(root.join("docs/acceptance-cases/phase-12.md")).unwrap();
        let manual = text
            .lines()
            .filter(|line| line.starts_with("| P12-M"))
            .filter_map(|line| {
                let cells = line
                    .trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect::<Vec<_>>();
                (cells.get(2) == Some(&"Manual")).then(|| cells[0].to_owned())
            })
            .collect::<Vec<_>>();
        let registered = CASES
            .iter()
            .map(|case| case.id.to_owned())
            .collect::<Vec<_>>();
        assert_eq!(registered, manual);
        for case in CASES {
            assert!(!case.scenarios.is_empty(), "{}", case.id);
            let mut ids = case.scenarios.to_vec();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), case.scenarios.len(), "{}", case.id);
            for scenario in case.scenarios {
                assert!(
                    scenario.bytes().all(|byte| byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'-'),
                    "{}/{scenario}",
                    case.id
                );
            }
        }
        let count = |tier| CASES.iter().filter(|case| case.tier == tier).count();
        assert_eq!((count(Tier::A), count(Tier::B), count(Tier::C)), (14, 7, 3));
        assert!(!KNOWN_GAPS.is_empty());
    }
}
