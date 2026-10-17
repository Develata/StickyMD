//! What each decomposed source requires, the oracle `OBLIGATIONS` is checked against: one
//! assertion list per source with its required parameters, and the USER retirements with the
//! exact entries they authorize. Kept beside the tests, apart from the runtime table, so a
//! reduction in `OBLIGATIONS` cannot silently shrink the requirement too.

/// What one source requires: an assertion and the parameters it must cover (`""` when it
/// has none).
#[derive(Clone, Copy, Debug)]
pub(super) struct Assertion {
    pub(super) source: &'static str,
    pub(super) id: &'static str,
    pub(super) params: &'static [&'static str],
}

/// A USER retirement: the decision authorizes exactly this `(source, assertion, param)`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Retirement {
    pub(super) decision: &'static str,
    pub(super) source: &'static str,
    pub(super) assertion: &'static str,
    pub(super) param: &'static str,
}

const NONE: &[&str] = &[""];
const DPI: &[&str] = &["dpi=100", "dpi=125", "dpi=150", "dpi=200"];
const EDGES: &[&str] = &["edge=left", "edge=right", "edge=top"];
const OPACITY: &[&str] = &["opacity=40", "opacity=70", "opacity=96", "opacity=100"];
const SWITCH: &[&str] = &["to=dark", "to=light"];

const fn a(source: &'static str, id: &'static str, params: &'static [&'static str]) -> Assertion {
    Assertion { source, id, params }
}

pub(super) const RETIREMENTS: &[Retirement] = &[Retirement {
    decision: "RETIRE-P08-M12-ACCESSIBILITY",
    source: "P08-M12",
    assertion: "accessibility",
    param: "",
}];

pub(super) const ASSERTIONS: &[Assertion] = &[
    a("P08-M01", "dock-visual", DPI),
    a("P08-M01", "timing", NONE),
    a("P08-M01", "undock-16dip", NONE),
    a("P08-M02", "dock-visual", DPI),
    a("P08-M02", "timing", NONE),
    a("P08-M02", "undock-16dip", NONE),
    a("P08-M03", "dock-visual", DPI),
    a("P08-M03", "timing", NONE),
    a(
        "P08-M04",
        "ime-input",
        &[
            "ime=pinyin,mode=source",
            "ime=pinyin,mode=split",
            "ime=wetype,mode=source",
            "ime=wetype,mode=split",
        ],
    ),
    a("P08-M04", "composition-no-collapse", EDGES),
    a(
        "P08-M04",
        "opacity-ime-commit",
        &["opacity=40", "opacity=96", "opacity=100"],
    ),
    a(
        "P08-M04",
        "candidate-visual",
        &[
            "ime=pinyin,edge=left",
            "ime=pinyin,edge=right",
            "ime=pinyin,edge=top",
            "ime=wetype,edge=left",
            "ime=wetype,edge=right",
            "ime=wetype,edge=top",
        ],
    ),
    a(
        "P08-M05",
        "tray-menu",
        &["item=show", "item=hide", "item=quit"],
    ),
    a("P08-M05", "failed-save-quit", NONE),
    a("P08-M05", "failed-save-visible-error", NONE),
    a("P08-M05", "topmost-tray-sync", NONE),
    a("P08-M06", "layers-visual", OPACITY),
    a(
        "P08-M06",
        "commit-modes",
        &["commit=slider-release", "commit=enter", "commit=blur"],
    ),
    a(
        "P08-M07",
        "theme-visual",
        &["theme=light", "theme=dark", "theme=system"],
    ),
    a("P08-M07", "live-switch", SWITCH),
    a(
        "P08-M08",
        "visible-restore-and-focus",
        &["state=hidden", "state=minimized", "state=docked"],
    ),
    a(
        "P08-M08",
        "identity-wake",
        &["state=hidden", "state=docked"],
    ),
    a("P08-M09", "process-window-isolation", NONE),
    a("P08-M09", "note-config-runtime-isolation", NONE),
    a("P08-M09", "quit-isolation", NONE),
    a("P08-M09", "tray-identity", NONE),
    a("P08-M09", "independent-show-hide", NONE),
    a(
        "P08-M10",
        "arrangement",
        &["side=left", "side=right", "side=above", "side=below"],
    ),
    a("P08-M10", "primary-switch", NONE),
    a(
        "P08-M10",
        "disconnect-reconnect",
        &["step=disconnect", "step=reconnect"],
    ),
    a("P08-M10", "sleep-resume", NONE),
    a("P08-M10", "rdp", NONE),
    a("P08-M11", "mixed-dpi-dock", &["edge=left", "edge=right"]),
    a(
        "P08-M11",
        "mixed-dpi-cross-monitor",
        &["what=drag", "what=dock", "what=ime", "what=preview"],
    ),
    a(
        "P08-M11",
        "negative-coordinates",
        &["side=left", "side=above"],
    ),
    a("P08-M12", "supported-builds", NONE),
    a("P08-M12", "corners-shadow", NONE),
    a("P08-M12", "topmost-button", NONE),
    a("P08-M12", "control-fade", NONE),
    a("P08-M12", "accessibility", NONE),
    a("P08-M13", "ime-functional", NONE),
    a("P08-M13", "ime-visual", &["ime=pinyin", "ime=wetype"]),
    a("P08-M14", "preview-selection-visual", NONE),
    a(
        "P08-M14",
        "math-visual",
        &[
            "variant=formulas",
            "theme=light",
            "theme=dark",
            "dpi=100",
            "dpi=125",
            "dpi=150",
            "dpi=200",
        ],
    ),
    a("P08-M15", "image-quality", NONE),
    a("P08-M15", "clipboard-producers", NONE),
    a("P08-M15", "export-dialog", NONE),
    a(
        "P08-M15",
        "crash-reconciliation",
        &["point=note-save", "point=asset-write", "point=trash-move"],
    ),
    a("P08-D004", "shell-visual", NONE),
    a("P08-D005", "rounded-corner", NONE),
    a("P08-D006", "shadow", NONE),
    a("P08-D007", "drag", EDGES),
    a("P08-D008", "resize", NONE),
    a("P08-D010", "mode-controls", NONE),
    a("P08-D012", "topmost-button", NONE),
    a("P08-D013", "topmost-tray-sync", NONE),
    a("P08-D016", "live-theme-change", SWITCH),
    a("P08-D020", "whole-window", OPACITY),
    a("P08-D021", "live-preview", NONE),
    a(
        "P08-D022",
        "numeric-input",
        &["commit=enter", "commit=blur"],
    ),
    a("P08-D027", "tray-icon", NONE),
    a("P08-D028", "tray-menu-items", NONE),
    a("P08-D031", "alt-f4", NONE),
    a("P08-D039", "left-dock", NONE),
    a("P08-D040", "right-dock", NONE),
    a("P08-D041", "top-dock", NONE),
    a("P08-D051", "hover-no-steal", EDGES),
    a("P08-D055", "composition-no-collapse", EDGES),
    a("P08-D055", "composition-guard-wiring", NONE),
    a("P08-D068", "covered-sensor-hover", NONE),
    a("P08-D070", "hover-click-focus", NONE),
    a("P08-D084", "disconnect-fallback", NONE),
    a("P08-D089", "dpi-ime-rect", NONE),
    a("P08-D117", "pinyin-functional", NONE),
    a("P08-D117", "pinyin-visual", NONE),
    a("P08-D118", "wetype-functional", NONE),
    a("P08-D118", "wetype-visual", NONE),
    a("P08-D119", "light", NONE),
    a("P08-D120", "dark", NONE),
    a("P08-D121", "system-live", SWITCH),
    a("P08-D122", "opacity-visual", OPACITY),
    a("P08-D123", "math-visual", NONE),
    a("P08-D124", "image-visual", NONE),
    a("P08-D125", "mode-visual", NONE),
    a("P08-D126", "left-dock-visual", DPI),
    a("P08-D127", "right-dock-visual", DPI),
    a("P08-D128", "top-dock-visual", DPI),
    a("P08-D129", "sensor-visual", EDGES),
    a("P08-D130", "hover-no-focus-visual", EDGES),
    a("P08-D131", "tray-visual-menu", NONE),
    a("P08-D132", "export-dialog", NONE),
    a("P08-D133", "real-dpi", &["dpi=125", "dpi=150", "dpi=200"]),
    a("P08-D134", "dual-monitor", NONE),
    a("P08-D135", "mixed-dpi", NONE),
    a("P08-D136", "disconnect", NONE),
    a(
        "P08-D137",
        "negative-geometry",
        &["side=left", "side=above"],
    ),
    a("P08-D138", "sleep-resume", NONE),
    a("P08-D139", "rdp", NONE),
    a("plan09:esc-1", "popup-close-revert", NONE),
    a("plan09:esc-1", "slider-drag-esc", NONE),
    a("plan09:esc-2", "recovery-pending", NONE),
    a(
        "plan09:esc-2",
        "transient-guards",
        &["state=asset-sync", "state=quitting"],
    ),
    a(
        "plan09:esc-3",
        "composition-cancel",
        &["field=editor", "field=search"],
    ),
    a("plan09:esc-4", "search-close", NONE),
    a(
        "plan09:esc-5",
        "preview-selection",
        &["selection=present", "selection=absent"],
    ),
    a(
        "plan09:esc-6",
        "collapse",
        &[
            "state=docked",
            "state=floating",
            "modifier=shift",
            "modifier=ctrl",
            "repeat",
        ],
    ),
    a("plan09:manual-collapse", "collapse-button", NONE),
    a(
        "plan09:scroll-1",
        "per-pane",
        &["view=source", "view=preview", "view=split"],
    ),
    a("plan09:scroll-1", "split-sync-switch", NONE),
    a("plan09:scroll-2", "gutter-geometry", NONE),
    a(
        "plan09:scroll-2",
        "short-note",
        &["part=hide-thumb", "part=keep-gutter"],
    ),
    a(
        "plan09:scroll-2",
        "gutter-runtime",
        &["aspect=hit-test", "aspect=content-zoom"],
    ),
    a("plan09:scroll-3", "thumb-geometry", NONE),
    a("plan09:scroll-3", "emphasis-paint", NONE),
    a(
        "plan09:scroll-3",
        "emphasis-trigger",
        &["trigger=hover", "trigger=drag"],
    ),
    a("plan09:scroll-3", "drag-mapping", NONE),
    a("plan09:scroll-3", "drag-live-update", NONE),
    a(
        "plan09:scroll-3",
        "ends-reach-document-ends",
        &["view=source", "view=preview"],
    ),
    a("plan09:scroll-3", "track-hit", NONE),
    a("plan09:scroll-3", "track-page-scroll", NONE),
    a(
        "plan09:scroll-4",
        "viewport-only",
        &[
            "state=text",
            "state=generation",
            "state=dirty",
            "state=caret",
            "state=selection",
            "state=preedit",
        ],
    ),
    a(
        "plan09:scroll-4",
        "drag-exit",
        &["end=release", "end=blur", "end=view-switch", "end=resize"],
    ),
    a("plan09:scroll-4", "drag-outside-window", NONE),
    a("plan09:scroll-5", "source-mapping", NONE),
    a("plan09:scroll-5", "preview-document-height", NONE),
    a("plan09:scroll-6", "stale-frame-no-thumb", NONE),
    a("plan09:scroll-6", "stale-receipt-identity", NONE),
    a("plan09:scroll-6", "stale-receipt-guard", NONE),
    a(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        &["dpi=100", "dpi=125", "dpi=150", "dpi=200", "theme"],
    ),
    a("plan09:opacity-1", "whole-window", OPACITY),
    a("plan09:opacity-2", "range-and-input", NONE),
    a("plan09:opacity-3", "live-preview", NONE),
    a("plan09:opacity-3", "no-stepwise-write", NONE),
    a(
        "plan09:opacity-3",
        "commit-modes",
        &["commit=slider-release", "commit=enter", "commit=blur"],
    ),
    a("plan09:quit-1", "keeps-running", NONE),
    a("plan09:quit-1", "memory-text-kept", NONE),
    a("plan09:quit-1", "visible-error", NONE),
    a(
        "plan09:quit-2",
        "established-only-by-save-failure",
        &[
            "cause=note-save-failure",
            "cause=quit-guard",
            "cause=paste-unconverged",
        ],
    ),
    a(
        "plan09:quit-2",
        "cleared-after-event-when-barrier-false",
        NONE,
    ),
    a("plan09:quit-2", "external-load-releases-rebuild", NONE),
    a("plan09:quit-3", "title-priority", NONE),
];
