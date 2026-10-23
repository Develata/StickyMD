//! The exact-candidate cases of G3, G4 and G5 and the check labels a passing run of each
//! records (plan 11: exact receipts are schema 2).
//!
//! One static declaration per group: the desktop runners pair it with their operations, the
//! evidence validator and the coverage mapping read it, and the CLI case options are tested
//! against it. A label is `name` or `name:key=value[,key=value…]`; names, keys and values use
//! lowercase ASCII letters, digits and `-`, and keys are strictly ascending. Each case lists its
//! labels in ascending byte order, the order a recorded set iterates in, so "recorded equals
//! declared" is one slice comparison. A case's check function records a label right after the
//! assertion the label names holds; the runner accepts a passing case only when the recorded
//! set equals the declaration. Equality proves every declared check was reached, not what the
//! assertion behind a label checks; that correspondence is a code-review obligation.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::collections::BTreeSet;

/// One exact case and the labels a passing run records, ascending.
#[derive(Debug)]
pub(crate) struct DeclaredCase {
    pub(crate) id: &'static str,
    pub(crate) checks: &'static [&'static str],
}

const fn case(id: &'static str, checks: &'static [&'static str]) -> DeclaredCase {
    DeclaredCase { id, checks }
}

/// Clipboard, export, recovery and asset safety.
pub(crate) const G3: &[DeclaredCase] = &[
    case("G3-01", G3_01),
    case("G3-02", G3_02),
    case("G3-03", G3_03),
    case("G3-04", G3_04),
    case("G3-05", G3_05),
];

/// Tray, docking, editor shortcuts, conversion, identity and real IMEs.
pub(crate) const G4: &[DeclaredCase] = &[
    case("G4-01", G4_01),
    case("G4-02", G4_02),
    case("G4-03", G4_03),
    case("G4-04", G4_04),
    case("G4-05", G4_05),
    case("G4-06", G4_06),
];

/// Screenshot cases: their evidence is the archived screenshots, so no labels.
pub(crate) const G5: &[DeclaredCase] = &[
    case("G5-01", &[]),
    case("G5-02", &[]),
    case("G5-03", &[]),
    case("G5-04", &[]),
];

pub(crate) const G3_01: &[&str] = &[
    "paste-format-priority:producer=browser",
    "paste-image-markdown:producer=browser",
    "paste-image-markdown:producer=explorer",
    "paste-image-markdown:producer=snipping",
    "paste-single-managed-asset:producer=browser",
    "paste-single-managed-asset:producer=explorer",
    "paste-single-managed-asset:producer=snipping",
    "redo-restores-asset:producer=browser",
    "redo-restores-asset:producer=explorer",
    "redo-restores-asset:producer=snipping",
    "undo-removes-asset:producer=browser",
    "undo-removes-asset:producer=explorer",
    "undo-removes-asset:producer=snipping",
];

pub(crate) const G3_02: &[&str] = &[
    "export-copies-asset-once",
    "export-leaves-working-note",
    "export-publishes-file",
    "export-rewrites-local-asset-only",
];

pub(crate) const G3_03: &[&str] = &[
    "kill-after-asset-publication",
    "kill-after-trash-move",
    "kill-at-publication-failure",
    "kill-before-autosave-keeps-old",
    "kill-note-complete:offset=0",
    "kill-note-complete:offset=15",
    "kill-note-complete:offset=250",
    "kill-note-complete:offset=75",
    "recovery-choice-enables-dock-escape",
    "recovery-escape-keeps-pending",
    "restart-after-asset-publication",
    "restart-flushed-temp",
    "restart-note-complete:offset=0",
    "restart-note-complete:offset=15",
    "restart-note-complete:offset=250",
    "restart-note-complete:offset=75",
    "restart-referenced-trash",
];

pub(crate) const G3_04: &[&str] = &[
    "export-publishes-file",
    "paste-replaces-note",
    "redo-replaces-note",
    "sole-process:stage=pre-exit",
    "tray-exit-closes-process",
    "undo-restores-note",
    "user-asset-intact:stage=export",
    "user-asset-intact:stage=paste",
    "user-asset-intact:stage=restart",
    "user-asset-intact:stage=tray-exit",
    "user-asset-intact:stage=undo",
];

pub(crate) const G3_05: &[&str] = &[
    "fake-asset-intact:stage=restart",
    "fake-asset-intact:stage=tray-exit",
    "sole-process:stage=pre-exit",
    "tray-exit-closes-process",
];

pub(crate) const G4_01: &[&str] = &[
    "alt-f4-hides-without-exit",
    "close-hides-window",
    "close-keeps-process",
    "sole-process:stage=pre-exit",
    "sole-process:stage=startup",
    "tray-hide-keeps-process",
    "tray-hide-show-retains-window-and-source",
    "tray-icon-present",
    "tray-menu-items",
    "tray-quit-exits",
    "tray-quit-save-failure-keeps-visible-process",
    "tray-quit-save-failure-retains-text",
    "tray-quit-save-failure-retry",
    "tray-quit-save-failure-title",
    "tray-quit-saves-latest",
    "tray-show-restores-window",
    "tray-show-retains-source",
    "tray-topmost-controls-sync",
    "tray-topmost-sensor-keeps-config",
];

pub(crate) const G4_02: &[&str] = &[
    "covered-sensor-real-hover",
    "dock-collapse-delay:edge=left,pin=off",
    "dock-collapse-delay:edge=right,pin=off",
    "dock-collapse-delay:edge=right,pin=on",
    "dock-collapse-delay:edge=top,pin=off",
    "dock-focus-guard:edge=left,pin=off",
    "dock-focus-guard:edge=right,pin=off",
    "dock-focus-guard:edge=right,pin=on",
    "dock-focus-guard:edge=top,pin=off",
    "dock-leave-delay:edge=left,pin=off",
    "dock-leave-delay:edge=right,pin=off",
    "dock-leave-delay:edge=right,pin=on",
    "dock-leave-delay:edge=top,pin=off",
    "dock-reexpand:edge=left,pin=off",
    "dock-reexpand:edge=right,pin=off",
    "dock-reexpand:edge=right,pin=on",
    "dock-reexpand:edge=top,pin=off",
    "dock-reveal-delay:edge=left,pin=off",
    "dock-reveal-delay:edge=right,pin=off",
    "dock-reveal-delay:edge=right,pin=on",
    "dock-reveal-delay:edge=top,pin=off",
    "dock-snap:edge=left,pin=off",
    "dock-snap:edge=right,pin=off",
    "dock-snap:edge=right,pin=on",
    "dock-snap:edge=top,pin=off",
    "escape-floating-keeps-window",
    "escape-preview-clears-selection",
    "escape-preview-empty-collapses",
    "escape-repeat-keydowns-independent",
    "escape-search-closes-before-dock",
    "escape-shift-collapses",
    "manual-collapse-button",
    "opacity-escape-cancels-drag",
    "opacity-escape-cancels-field",
    "opacity-numeric-blur-commits",
    "opacity-numeric-enter-commits",
    "opacity-slider-release-commits",
    "pin-off-config",
    "pin-on-reaches-hwnd",
    "pin-survives-autohide",
    "preview-scroll-keeps-note:mode=preview",
    "preview-scroll-keeps-note:mode=split",
    "preview-scroll-native-capture:mode=preview",
    "preview-scroll-native-capture:mode=split",
    "preview-scroll-outside-both-ends:mode=preview",
    "preview-scroll-outside-both-ends:mode=split",
    "preview-scroll-stops-on-blur:mode=preview",
    "preview-scroll-stops-on-blur:mode=split",
    "preview-scroll-stops-on-release:mode=preview",
    "preview-scroll-stops-on-release:mode=split",
    "scroll-drag-outside-finishes",
    "scroll-drag-stops-on-blur",
    "scroll-drag-stops-on-release",
    "scroll-keeps-note-and-selection",
];

pub(crate) const G4_03: &[&str] = &[
    "ctrl-insert-copies-selection",
    "preview-ignores-traditional-edits",
    "search-escape-returns-to-source",
    "search-find-only-guards",
    "search-preview-restores-on-replace",
    "search-replacement-advances-past-insert",
    "search-replacement-one-undo",
    "search-select-before-replace",
    "search-view-shortcuts-retain-session",
    "shift-delete-cuts",
    "shift-insert-dib-publishes-asset",
    "shift-insert-file-drop-publishes-asset",
    "shift-insert-pastes",
    "traditional-redo",
    "traditional-undo",
];

pub(crate) const G4_04: &[&str] = &[
    "math-convert-durable",
    "math-convert-source-refresh",
    "math-undo-durable",
    "math-undo-one-step",
];

pub(crate) const G4_05: &[&str] = &[
    "close-hides-window",
    "collapsed-keeps-note-and-config",
    "collapsed-second-instance-exits",
    "collapsed-wake-activates-primary",
    "collapsed-wakes-primary",
    "junction-keeps-note-and-config",
    "junction-second-instance-exits",
    "junction-wakes-primary",
    "portable-copies-note-config-and-exit-isolated",
    "portable-tray-visibility-isolated:copy=first,visible=false",
    "portable-tray-visibility-isolated:copy=first,visible=true",
    "portable-tray-visibility-isolated:copy=second,visible=false",
    "portable-tray-visibility-isolated:copy=second,visible=true",
    "second-launch-exits:state=hidden",
    "second-launch-exits:state=minimized",
    "second-launch-immediate-input:state=hidden",
    "second-launch-immediate-input:state=minimized",
    "second-launch-keeps-durable-files:state=hidden",
    "second-launch-keeps-durable-files:state=minimized",
    "second-launch-restores-focused-window:state=hidden",
    "second-launch-restores-focused-window:state=minimized",
    "sole-process-after-collapsed-wake",
    "sole-process-after-junction",
];

pub(crate) const G4_06: &[&str] = &[
    "ime-cancel-non-mutating:ime=pinyin",
    "ime-cancel-non-mutating:ime=wetype",
    "ime-commit-undo:edge=left,ime=pinyin",
    "ime-commit-undo:edge=left,ime=wetype",
    "ime-commit-undo:edge=right,ime=pinyin",
    "ime-commit-undo:edge=right,ime=wetype",
    "ime-commit-undo:edge=top,ime=pinyin",
    "ime-commit-undo:edge=top,ime=wetype",
    "ime-commit-undo:ime=pinyin,mode=source",
    "ime-commit-undo:ime=pinyin,mode=split",
    "ime-commit-undo:ime=pinyin,opacity=100",
    "ime-commit-undo:ime=pinyin,opacity=40",
    "ime-commit-undo:ime=pinyin,opacity=70",
    "ime-commit-undo:ime=pinyin,opacity=96",
    "ime-commit-undo:ime=wetype,mode=source",
    "ime-commit-undo:ime=wetype,mode=split",
    "ime-commit-undo:ime=wetype,opacity=100",
    "ime-commit-undo:ime=wetype,opacity=40",
    "ime-commit-undo:ime=wetype,opacity=70",
    "ime-commit-undo:ime=wetype,opacity=96",
    "ime-composition-no-collapse:edge=left,ime=pinyin",
    "ime-composition-no-collapse:edge=left,ime=wetype",
    "ime-composition-no-collapse:edge=right,ime=pinyin",
    "ime-composition-no-collapse:edge=right,ime=wetype",
    "ime-composition-no-collapse:edge=top,ime=pinyin",
    "ime-composition-no-collapse:edge=top,ime=wetype",
    "ime-escape-keeps-dock:edge=left,ime=pinyin",
    "ime-escape-keeps-dock:edge=left,ime=wetype",
    "ime-escape-keeps-dock:edge=right,ime=pinyin",
    "ime-escape-keeps-dock:edge=right,ime=wetype",
    "ime-escape-keeps-dock:edge=top,ime=pinyin",
    "ime-escape-keeps-dock:edge=top,ime=wetype",
    "ime-find-no-replace:ime=pinyin",
    "ime-find-no-replace:ime=wetype",
    "ime-find-query-selects-term:ime=pinyin",
    "ime-find-query-selects-term:ime=wetype",
    "ime-focus-loss-keeps-text:ime=pinyin",
    "ime-focus-loss-keeps-text:ime=wetype",
    "ime-preedit-not-durable:ime=pinyin",
    "ime-preedit-not-durable:ime=wetype",
    "ime-profile-activated:ime=pinyin",
    "ime-profile-activated:ime=wetype",
    "ime-refocus-commit-undo:ime=pinyin",
    "ime-refocus-commit-undo:ime=wetype",
    "ime-replace-commit:ime=pinyin",
    "ime-replace-commit:ime=wetype",
    "ime-replace-undo:ime=pinyin",
    "ime-replace-undo:ime=wetype",
    "ime-search-composition-blocks-preview:ime=pinyin",
    "ime-search-composition-blocks-preview:ime=wetype",
    "ime-search-escape-retains-query:ime=pinyin",
    "ime-search-escape-retains-query:ime=wetype",
    "ime-selection-commit-one-undo:ime=pinyin",
    "ime-selection-commit-one-undo:ime=wetype",
    "numeric-composition-refuses-focus:ime=pinyin,owner=search",
    "numeric-composition-refuses-focus:ime=pinyin,owner=source",
    "numeric-composition-refuses-focus:ime=wetype,owner=search",
    "numeric-composition-refuses-focus:ime=wetype,owner=source",
    "numeric-idle-export-priority:ime=pinyin",
    "numeric-idle-export-priority:ime=wetype",
    "numeric-native-commit:commit=blur,ime=pinyin,owner=preview",
    "numeric-native-commit:commit=blur,ime=pinyin,owner=search",
    "numeric-native-commit:commit=blur,ime=pinyin,owner=source",
    "numeric-native-commit:commit=blur,ime=wetype,owner=preview",
    "numeric-native-commit:commit=blur,ime=wetype,owner=search",
    "numeric-native-commit:commit=blur,ime=wetype,owner=source",
    "numeric-native-commit:commit=enter,ime=pinyin,owner=preview",
    "numeric-native-commit:commit=enter,ime=pinyin,owner=search",
    "numeric-native-commit:commit=enter,ime=pinyin,owner=source",
    "numeric-native-commit:commit=enter,ime=wetype,owner=preview",
    "numeric-native-commit:commit=enter,ime=wetype,owner=search",
    "numeric-native-commit:commit=enter,ime=wetype,owner=source",
];

/// The labels one case run recorded; recording a label again keeps one copy.
#[cfg(any(windows, test))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Checks(BTreeSet<String>);

#[cfg(any(windows, test))]
impl Checks {
    /// Records `name` with `params` (in any order) once the assertion it names holds.
    pub(crate) fn record(&mut self, name: &str, params: &[(&str, &str)]) {
        self.0.insert(label(name, params));
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    /// The recorded labels, ascending.
    pub(crate) fn into_sorted(self) -> Vec<String> {
        self.0.into_iter().collect()
    }
}

/// The canonical label of `name` with `params`: keys ascending.
#[cfg(any(windows, test))]
fn label(name: &str, params: &[(&str, &str)]) -> String {
    let mut sorted = params.to_vec();
    sorted.sort_unstable_by_key(|&(key, _)| key);
    let mut label = name.to_owned();
    for (index, (key, value)) in sorted.into_iter().enumerate() {
        label.push(if index == 0 { ':' } else { ',' });
        label.push_str(key);
        label.push('=');
        label.push_str(value);
    }
    label
}

/// `None` when `recorded` lists exactly `declared` (ascending, each once); otherwise what
/// differs, every list ascending so the description is deterministic.
pub(crate) fn mismatch(declared: &[&str], recorded: &[impl AsRef<str>]) -> Option<String> {
    if recorded.len() == declared.len()
        && recorded
            .iter()
            .zip(declared)
            .all(|(recorded, declared)| recorded.as_ref() == *declared)
    {
        return None;
    }
    let declared_set = declared.iter().copied().collect::<BTreeSet<_>>();
    let recorded_set = recorded.iter().map(AsRef::as_ref).collect::<BTreeSet<_>>();
    let missing = declared_set.difference(&recorded_set).collect::<Vec<_>>();
    let unexpected = recorded_set.difference(&declared_set).collect::<Vec<_>>();
    let mut parts = Vec::new();
    if !missing.is_empty() {
        parts.push(format!("missing {missing:?}"));
    }
    if !unexpected.is_empty() {
        parts.push(format!("unexpected {unexpected:?}"));
    }
    if parts.is_empty() {
        parts.push("labels repeated or out of order".to_owned());
    }
    Some(parts.join("; "))
}

#[cfg(test)]
mod tests {
    use super::{Checks, G3, G4, G5, label, mismatch};

    /// Name, keys and values from the label alphabet; keys strictly ascending.
    fn is_canonical(label: &str) -> bool {
        let token = |part: &str| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        };
        let Some((name, params)) = label.split_once(':') else {
            return token(label);
        };
        let mut previous: Option<&str> = None;
        token(name)
            && params.split(',').all(|pair| {
                let Some((key, value)) = pair.split_once('=') else {
                    return false;
                };
                let ascending = previous.is_none_or(|previous| previous < key);
                previous = Some(key);
                token(key) && token(value) && ascending
            })
    }

    #[test]
    fn every_declared_label_is_canonical_and_each_case_lists_them_ascending_once() {
        for (group, cases) in [("G3", G3), ("G4", G4), ("G5", G5)] {
            for (index, case) in cases.iter().enumerate() {
                assert_eq!(case.id, format!("{group}-{:02}", index + 1));
                for label in case.checks {
                    assert!(is_canonical(label), "{}: {label}", case.id);
                }
                assert!(
                    case.checks.windows(2).all(|pair| pair[0] < pair[1]),
                    "{} labels must be strictly ascending",
                    case.id
                );
            }
        }
        assert!(G3.iter().chain(G4).all(|case| !case.checks.is_empty()));
        assert!(G5.iter().all(|case| case.checks.is_empty()));
        let total = G3
            .iter()
            .chain(G4)
            .map(|case| case.checks.len())
            .sum::<usize>();
        assert_eq!(total, 236);
    }

    #[test]
    fn the_canonical_form_rejects_every_other_spelling() {
        for good in ["a", "a-1", "a:k=v", "a:a=1,b=2"] {
            assert!(is_canonical(good), "{good}");
        }
        for bad in [
            "",
            "A",
            "a b",
            "a:",
            "a:k",
            "a:k=",
            "a:=v",
            "a:b=1,a=2",
            "a:a=1,a=2",
            "a:k=v,",
            "a:k=v:w",
            "a:k=v=w",
        ] {
            assert!(!is_canonical(bad), "{bad}");
        }
    }

    #[test]
    fn a_recorded_label_is_canonical_whatever_the_parameter_order() {
        assert_eq!(label("dock-snap", &[]), "dock-snap");
        assert_eq!(
            label("dock-snap", &[("pin", "on"), ("edge", "right")]),
            "dock-snap:edge=right,pin=on"
        );
        let mut checks = Checks::default();
        checks.record("b", &[]);
        checks.record("a", &[("k", "v")]);
        checks.record("b", &[]);
        let mut other = Checks::default();
        other.record("c", &[]);
        checks.extend(other);
        assert_eq!(checks.into_sorted(), ["a:k=v", "b", "c"]);
    }

    #[test]
    fn a_mismatch_names_every_missing_and_unexpected_label() {
        let declared: &[&str] = &["a", "b", "c"];
        assert_eq!(mismatch(declared, &["a", "b", "c"]), None);
        assert_eq!(
            mismatch(declared, &["a", "c", "d"]).unwrap(),
            r#"missing ["b"]; unexpected ["d"]"#
        );
        assert_eq!(
            mismatch(declared, &["a", "b", "b", "c"]).unwrap(),
            "labels repeated or out of order"
        );
        assert_eq!(
            mismatch(declared, &["b", "a", "c"]).unwrap(),
            "labels repeated or out of order"
        );
        assert_eq!(mismatch(&[], &[] as &[&str]), None);
    }
}
