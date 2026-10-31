//! The obligation data: one entry per `(source, assertion, parameter)`, in source order.
//! Edited together with the documents it decomposes (the digests in `tests` say when).
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

use super::{Obligation, Target};
use crate::qualification::exact_checks;
use crate::unit_owners::{Owner, OwnerTarget};

const fn o(
    source: &'static str,
    assertion: &'static str,
    param: &'static str,
    target: Target,
) -> Obligation {
    Obligation {
        source,
        assertion,
        param,
        target,
    }
}

const fn m(case: &'static str, scenario: &'static str) -> Target {
    Target::Manual { case, scenario }
}

const fn exact(case: &'static str, checks: &'static [&'static str]) -> Target {
    Target::Exact { case, checks }
}

/// Owned by these unit tests together, once the headless receipt verifies every one.
const fn tests(owners: &'static [Owner]) -> Target {
    Target::Test(owners)
}

/// A unit test of the `stickymd-win` binary.
const fn win(path: &'static str) -> Owner {
    Owner {
        package: "stickymd-win",
        target: OwnerTarget::Bin("stickymd-win"),
        path,
    }
}

/// A unit test of the `stickymd-render` library.
const fn render(path: &'static str) -> Owner {
    Owner {
        package: "stickymd-render",
        target: OwnerTarget::Lib,
        path,
    }
}

/// Both real IMEs and all input owners; application-state companions are required
/// by OPACITY_RANGE_AND_INPUT in the same source-bound headless receipt.
const G4_NUMERIC_ENTER: &[&str] = &[
    "numeric-composition-refuses-focus:ime=pinyin,owner=search",
    "numeric-composition-refuses-focus:ime=pinyin,owner=source",
    "numeric-composition-refuses-focus:ime=wetype,owner=search",
    "numeric-composition-refuses-focus:ime=wetype,owner=source",
    "numeric-idle-export-priority:ime=pinyin",
    "numeric-idle-export-priority:ime=wetype",
    "numeric-native-commit:commit=enter,ime=pinyin,owner=preview",
    "numeric-native-commit:commit=enter,ime=pinyin,owner=search",
    "numeric-native-commit:commit=enter,ime=pinyin,owner=source",
    "numeric-native-commit:commit=enter,ime=wetype,owner=preview",
    "numeric-native-commit:commit=enter,ime=wetype,owner=search",
    "numeric-native-commit:commit=enter,ime=wetype,owner=source",
];
const G4_NUMERIC_BLUR: &[&str] = &[
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
];
/// The Esc decision with each guard alone (asset reconcile, input disabled while quitting) and
/// under every modifier; USER 2026-10-15 approved unit-only coverage of these Esc rows.
const ESC_GUARDS: &[Owner] = &[win("app::escape::tests::each_guard_alone_ignores_esc")];
const ESC_MODIFIERS: &[Owner] = &[win("app::escape::tests::modifiers_never_change_esc")];
/// Scrollbar decisions (plan 09 vertical scrollbars) a single test, or tests together, prove.
const SCROLL_EMPHASIS_DRAG: &[Owner] = &[win(
    "app::scrollbar::tests::hovering_or_dragging_a_bar_emphasizes_only_that_bar",
)];
const SCROLL_EMPHASIS_HOVER: &[Owner] = &[
    win("app::scrollbar::tests::the_gutter_is_the_band_right_of_each_pane"),
    win("app::scrollbar::tests::hovering_or_dragging_a_bar_emphasizes_only_that_bar"),
];
const SCROLL_EMPHASIS_PAINT: &[Owner] = &[win(
    "app::scrollbar::tests::scrollbar_paints_inside_gutter_and_avoids_resize_border_at_each_dpi",
)];
const SCROLL_DRAG_MAPPING: &[Owner] = &[win(
    "app::scrollbar::tests::scrollbar_drag_keeps_grab_offset_and_clamps_outside_window",
)];
const SCROLL_TRACK_HIT: &[Owner] = &[win(
    "app::scrollbar::tests::a_track_press_pages_toward_the_click_by_most_of_the_pane",
)];
const SCROLL_SOURCE_ENDS: &[Owner] = &[
    render(
        "source::scrollbar::tests::scrollbar_jumps_to_both_ends_without_shaping_intervening_lines",
    ),
    render(
        "source::scrollbar::tests::scrollbar_handles_a_single_wrapped_line_and_resized_short_notes",
    ),
];
/// The quit-cancelled notice and the title (plan 09 tray lifecycle).
const QUIT_NOTICE_ONLY_SAVE: &[Owner] = &[win(
    "app::title::tests::only_a_failed_note_save_establishes_the_notice",
)];
const QUIT_EXTERNAL_LOAD: &[Owner] = &[win(
    "flow::persistence::tests::loading_external_content_discharges_a_superseded_recreate",
)];
const QUIT_TITLE_PRIORITY: &[Owner] = &[
    win("app::title::tests::each_state_has_its_title_and_higher_states_win"),
    win("app::title::tests::the_title_has_one_native_writer"),
];
/// C2b-4 owners (2026-10-19): the scrollbar decisions extracted from `StickyApp` into pure
/// functions (`view_bars`, `preview_offset_at`, `preview_offset_by`), the gutter geometry, the
/// Preview receipt identity and the opacity drag.
const VIEW_BARS: Owner =
    win("app::scrollbar::tests::each_view_has_a_bar_for_exactly_its_shown_panes");
const GUTTER_GEOMETRY: Owner = win(
    "app::window_interaction::phase8_interaction_tests::every_view_reserves_the_gutter_clear_of_the_resize_border_and_sync_button",
);
const PREVIEW_POSITIONS: Owner = win(
    "app::scrollbar::tests::the_preview_offset_follows_the_fraction_and_pages_within_the_document",
);
const SCROLL_VIEW_BARS_OWNED: &[Owner] = &[VIEW_BARS];
const SCROLL_GUTTER_GEOMETRY: &[Owner] = &[GUTTER_GEOMETRY];
const SCROLL_THUMB_GEOMETRY: &[Owner] = &[win(
    "app::scrollbar::tests::the_thumb_is_the_visible_share_with_a_grabbable_minimum",
)];
const SCROLL_PREVIEW_ENDS: &[Owner] = &[PREVIEW_POSITIONS];
/// A track press pages by most of the pane (`a_track_press…`), applied to Preview by
/// `preview_offset_by` and to Source by `scroll_by`, both within the document.
const SCROLL_TRACK_PAGING: &[Owner] = &[
    win("app::scrollbar::tests::a_track_press_pages_toward_the_click_by_most_of_the_pane"),
    PREVIEW_POSITIONS,
    render(
        "source::scrollbar::tests::a_source_page_moves_the_viewport_by_its_pixels_within_the_document",
    ),
];
/// The long-note test checks the mapping against itself (jump and read back share the wrapped
/// line height); the single wrapped line checks that height against the real layout.
const SCROLL_SOURCE_MAPPING: &[Owner] = &[
    render(
        "source::scrollbar::tests::the_source_maps_wrapped_offsets_and_measures_only_the_tail_once",
    ),
    render(
        "source::scrollbar::tests::scrollbar_handles_a_single_wrapped_line_and_resized_short_notes",
    ),
];
const SCROLL_STALE_RECEIPT: &[Owner] = &[win(
    "app::preview_runtime::tests::a_receipt_for_an_older_offset_is_relaid_out_and_never_shown",
)];
const SCROLL_SYNC_APPLICATION: &[Owner] = &[win(
    "app::scroll_application::tests::split_sync_uses_only_the_current_driver_and_requires_the_switch_and_current_frame",
)];
const SCROLL_SESSION_APPLICATION: &[Owner] = &[win(
    "app::scroll_application::tests::pane_scroll_preserves_text_generation_selection_caret_and_preedit",
)];
const SCROLL_CURRENT_LAYOUT: &[Owner] = &[
    VIEW_BARS,
    win(
        "app::preview_runtime::tests::current_layout_requires_generation_and_every_layout_field_but_not_offset_or_selection",
    ),
];
/// The slider's 40–100 integer range, the field keeping any input key by key and committing only
/// an integer, clamped (plan 09, AC-024, USER decision D5), editing like a selected text field,
/// ending its edit, which keys and IME events the edited field takes, the committing Enter
/// keeping its repeats, and the default 96. Unit tests own these decisions because exact runs
/// cannot stably produce their state space (integers of any length and sign, non-integer and
/// non-ASCII entries, selected and partly typed values, each modifier, repeat and IME event,
/// each typed through real input); they do not prove native key delivery. Delivering real keys to the
/// field, the key order that keeps save and export first, the Enter and blur commits (a press
/// elsewhere, window focus loss, hiding, quitting), ignoring the keys winit synthesizes on focus
/// changes, turning the IME off while the field is edited and refusing it focus during a
/// composition, and keeping keys away from the note while the field is edited stay with the G4
/// numeric-input obligations (P08-D022). The actual shell application owners below
/// supplement native cases with states that physical input cannot force reliably.
const OPACITY_RANGE_AND_INPUT: &[Owner] = &[
    win("app::controls::phase8_control_tests::the_opacity_range_is_40_to_100_in_integer_steps"),
    win("app::controls::phase8_control_tests::the_opacity_field_edits_like_a_selected_text_field"),
    win(
        "app::controls::phase8_control_tests::the_opacity_field_takes_keys_while_edited_and_commits_when_it_ends",
    ),
    win(
        "app::controls::phase8_control_tests::the_key_that_ends_an_edit_keeps_its_repeats_and_release",
    ),
    win(
        "app::window_interaction::phase8_interaction_tests::the_edited_opacity_field_types_only_plain_text",
    ),
    win(
        "app::window_interaction::phase8_interaction_tests::the_edited_opacity_field_leaves_the_end_of_a_composition_to_its_owner",
    ),
    win(
        "app::window_interaction::runtime_tests::runtime_opacity_enter_keeps_repeats_and_synthetic_release_from_note_and_search",
    ),
    win(
        "app::window_interaction::runtime_tests::runtime_opacity_blur_commits_before_other_click_hide_or_quit",
    ),
    win(
        "app::window_interaction::runtime_tests::runtime_opacity_refuses_composition_and_routes_late_ime_to_field",
    ),
    win(
        "app::window_interaction::runtime_tests::runtime_opacity_key_priority_keeps_escape_and_save_above_field",
    ),
    win(
        "app::window_interaction::runtime_tests::runtime_opacity_recovery_and_busy_export_shortcuts_precede_field",
    ),
];
const OPACITY_NO_STEPWISE_WRITE: &[Owner] = &[win(
    "app::controls::phase8_control_tests::phase8_a_slider_drag_commits_at_most_once_on_release",
)];
// 2026-10-22: actual StickyApp application paths with real canonical/projection state.
// Window facts are supplied at the native boundary; these owners do not prove native delivery.
const RUNTIME_GEOMETRY: &[Owner] = &[win(
    "app::preview_runtime::runtime_tests::actual_config_geometry_keeps_gutters_across_content_zoom_and_note_lengths",
)];
const RUNTIME_GUTTER: &[Owner] = &[win(
    "app::scroll_runtime::tests::runtime_gutter_press_is_consumed_without_caret_or_selection_mutation",
)];
const RUNTIME_SCROLL: &[Owner] = &[win(
    "app::scroll_runtime::tests::runtime_pointer_moves_apply_each_pane_and_cached_document_height",
)];
const RUNTIME_CANONICAL: &[Owner] = &[win(
    "app::scroll_runtime::tests::runtime_scroll_preserves_canonical_state_and_session_for_clean_and_dirty_notes",
)];
const RUNTIME_DRAG_ENDS: &[Owner] = &[win(
    "app::scroll_runtime::tests::runtime_drag_ends_on_release_blur_view_switch_and_resize_for_both_panes",
)];
const RUNTIME_RECEIPT: &[Owner] = &[win(
    "app::preview_runtime::runtime_tests::receipt_application_rejects_every_stale_viewport_field_without_replacing_frame_or_offset",
)];
const RUNTIME_COMPOSITION: &[Owner] = &[win(
    "app::guards::tests::runtime_guards_read_both_compositions_and_focus_loss_cancels_them",
)];
const RUNTIME_QUIT_EFFECT: &[Owner] = &[win(
    "app::window_runtime::phase8_runtime_tests::runtime_every_window_effect_preserves_notice_except_failed_note_quit",
)];
const RUNTIME_EVENT_PASS: &[Owner] = &[
    win(
        "app::lifecycle::runtime_tests::runtime_event_pass_reads_all_barriers_once_after_scheduling",
    ),
    win("app::title::tests::the_notice_settles_at_the_end_of_each_event_pass"),
];
const G4_SCROLL_OUTSIDE: &[&str] = &[
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
];

// Exact owners name the check labels that prove them (`exact_checks`); an obligation without a
// producer, profile or Pin parameter needs every variant.
/// P08-M01 timing: the focused guard, both collapse delays, the sensor reveal and re-expansion on the left edge.
const DOCK_TIMING_LEFT: &[&str] = &[
    "dock-collapse-delay:edge=left,pin=off",
    "dock-focus-guard:edge=left,pin=off",
    "dock-leave-delay:edge=left,pin=off",
    "dock-reexpand:edge=left,pin=off",
    "dock-reveal-delay:edge=left,pin=off",
];
/// P08-M02 timing on the right edge, with Pin off and on.
const DOCK_TIMING_RIGHT: &[&str] = &[
    "dock-collapse-delay:edge=right,pin=off",
    "dock-collapse-delay:edge=right,pin=on",
    "dock-focus-guard:edge=right,pin=off",
    "dock-focus-guard:edge=right,pin=on",
    "dock-leave-delay:edge=right,pin=off",
    "dock-leave-delay:edge=right,pin=on",
    "dock-reexpand:edge=right,pin=off",
    "dock-reexpand:edge=right,pin=on",
    "dock-reveal-delay:edge=right,pin=off",
    "dock-reveal-delay:edge=right,pin=on",
];
/// P08-M03 timing on the top edge.
const DOCK_TIMING_TOP: &[&str] = &[
    "dock-collapse-delay:edge=top,pin=off",
    "dock-focus-guard:edge=top,pin=off",
    "dock-leave-delay:edge=top,pin=off",
    "dock-reexpand:edge=top,pin=off",
    "dock-reveal-delay:edge=top,pin=off",
];
/// A whole dock cycle (snap plus timing) on the left edge.
const DOCK_LEFT: &[&str] = &[
    "dock-collapse-delay:edge=left,pin=off",
    "dock-focus-guard:edge=left,pin=off",
    "dock-leave-delay:edge=left,pin=off",
    "dock-reexpand:edge=left,pin=off",
    "dock-reveal-delay:edge=left,pin=off",
    "dock-snap:edge=left,pin=off",
];
/// A whole dock cycle on the right edge, with Pin off and on.
const DOCK_RIGHT: &[&str] = &[
    "dock-collapse-delay:edge=right,pin=off",
    "dock-collapse-delay:edge=right,pin=on",
    "dock-focus-guard:edge=right,pin=off",
    "dock-focus-guard:edge=right,pin=on",
    "dock-leave-delay:edge=right,pin=off",
    "dock-leave-delay:edge=right,pin=on",
    "dock-reexpand:edge=right,pin=off",
    "dock-reexpand:edge=right,pin=on",
    "dock-reveal-delay:edge=right,pin=off",
    "dock-reveal-delay:edge=right,pin=on",
    "dock-snap:edge=right,pin=off",
    "dock-snap:edge=right,pin=on",
];
/// A whole dock cycle on the top edge.
const DOCK_TOP: &[&str] = &[
    "dock-collapse-delay:edge=top,pin=off",
    "dock-focus-guard:edge=top,pin=off",
    "dock-leave-delay:edge=top,pin=off",
    "dock-reexpand:edge=top,pin=off",
    "dock-reveal-delay:edge=top,pin=off",
    "dock-snap:edge=top,pin=off",
];
/// Real composition keeps a left-docked paper open, for both IMEs.
const COMPOSITION_LEFT: &[&str] = &[
    "ime-composition-no-collapse:edge=left,ime=pinyin",
    "ime-composition-no-collapse:edge=left,ime=wetype",
];
/// An IME commit and Undo at 40% opacity, for both IMEs.
const OPACITY_40_IME: &[&str] = &[
    "ime-commit-undo:ime=pinyin,opacity=40",
    "ime-commit-undo:ime=wetype,opacity=40",
];
/// The whole G4-06 functional matrix of Microsoft Pinyin.
const IME_PINYIN: &[&str] = &[
    "ime-cancel-non-mutating:ime=pinyin",
    "ime-commit-undo:edge=left,ime=pinyin",
    "ime-commit-undo:edge=right,ime=pinyin",
    "ime-commit-undo:edge=top,ime=pinyin",
    "ime-commit-undo:ime=pinyin,mode=source",
    "ime-commit-undo:ime=pinyin,mode=split",
    "ime-commit-undo:ime=pinyin,opacity=100",
    "ime-commit-undo:ime=pinyin,opacity=40",
    "ime-commit-undo:ime=pinyin,opacity=70",
    "ime-commit-undo:ime=pinyin,opacity=96",
    "ime-composition-no-collapse:edge=left,ime=pinyin",
    "ime-composition-no-collapse:edge=right,ime=pinyin",
    "ime-composition-no-collapse:edge=top,ime=pinyin",
    "ime-escape-keeps-dock:edge=left,ime=pinyin",
    "ime-escape-keeps-dock:edge=right,ime=pinyin",
    "ime-escape-keeps-dock:edge=top,ime=pinyin",
    "ime-find-no-replace:ime=pinyin",
    "ime-find-query-selects-term:ime=pinyin",
    "ime-focus-loss-keeps-text:ime=pinyin",
    "ime-preedit-not-durable:ime=pinyin",
    "ime-profile-activated:ime=pinyin",
    "ime-refocus-commit-undo:ime=pinyin",
    "ime-replace-commit:ime=pinyin",
    "ime-replace-undo:ime=pinyin",
    "ime-search-composition-blocks-preview:ime=pinyin",
    "ime-search-escape-retains-query:ime=pinyin",
    "ime-selection-commit-one-undo:ime=pinyin",
    "numeric-composition-refuses-focus:ime=pinyin,owner=search",
    "numeric-composition-refuses-focus:ime=pinyin,owner=source",
    "numeric-idle-export-priority:ime=pinyin",
    "numeric-native-commit:commit=blur,ime=pinyin,owner=preview",
    "numeric-native-commit:commit=blur,ime=pinyin,owner=search",
    "numeric-native-commit:commit=blur,ime=pinyin,owner=source",
    "numeric-native-commit:commit=enter,ime=pinyin,owner=preview",
    "numeric-native-commit:commit=enter,ime=pinyin,owner=search",
    "numeric-native-commit:commit=enter,ime=pinyin,owner=source",
];
/// The whole G4-06 functional matrix of WeType.
const IME_WETYPE: &[&str] = &[
    "ime-cancel-non-mutating:ime=wetype",
    "ime-commit-undo:edge=left,ime=wetype",
    "ime-commit-undo:edge=right,ime=wetype",
    "ime-commit-undo:edge=top,ime=wetype",
    "ime-commit-undo:ime=wetype,mode=source",
    "ime-commit-undo:ime=wetype,mode=split",
    "ime-commit-undo:ime=wetype,opacity=100",
    "ime-commit-undo:ime=wetype,opacity=40",
    "ime-commit-undo:ime=wetype,opacity=70",
    "ime-commit-undo:ime=wetype,opacity=96",
    "ime-composition-no-collapse:edge=left,ime=wetype",
    "ime-composition-no-collapse:edge=right,ime=wetype",
    "ime-composition-no-collapse:edge=top,ime=wetype",
    "ime-escape-keeps-dock:edge=left,ime=wetype",
    "ime-escape-keeps-dock:edge=right,ime=wetype",
    "ime-escape-keeps-dock:edge=top,ime=wetype",
    "ime-find-no-replace:ime=wetype",
    "ime-find-query-selects-term:ime=wetype",
    "ime-focus-loss-keeps-text:ime=wetype",
    "ime-preedit-not-durable:ime=wetype",
    "ime-profile-activated:ime=wetype",
    "ime-refocus-commit-undo:ime=wetype",
    "ime-replace-commit:ime=wetype",
    "ime-replace-undo:ime=wetype",
    "ime-search-composition-blocks-preview:ime=wetype",
    "ime-search-escape-retains-query:ime=wetype",
    "ime-selection-commit-one-undo:ime=wetype",
    "numeric-composition-refuses-focus:ime=wetype,owner=search",
    "numeric-composition-refuses-focus:ime=wetype,owner=source",
    "numeric-idle-export-priority:ime=wetype",
    "numeric-native-commit:commit=blur,ime=wetype,owner=preview",
    "numeric-native-commit:commit=blur,ime=wetype,owner=search",
    "numeric-native-commit:commit=blur,ime=wetype,owner=source",
    "numeric-native-commit:commit=enter,ime=wetype,owner=preview",
    "numeric-native-commit:commit=enter,ime=wetype,owner=search",
    "numeric-native-commit:commit=enter,ime=wetype,owner=source",
];
/// Show, from the exact tray menu, restores the hidden paper with its retained text.
const TRAY_SHOW: &[&str] = &[
    "tray-menu-items",
    "tray-show-restores-window",
    "tray-show-retains-source",
];
/// Quit, from the exact tray menu, saves the latest text and exits.
const TRAY_QUIT: &[&str] = &[
    "tray-menu-items",
    "tray-quit-exits",
    "tray-quit-saves-latest",
];
/// A second launch through a junction wakes the hidden primary and exits.
const WAKE_HIDDEN: &[&str] = &[
    "close-hides-window",
    "junction-second-instance-exits",
    "junction-wakes-primary",
];

pub(super) const OBLIGATIONS: &[Obligation] = &[
    // P08-M01..M03: dock visual at each DPI, timing on the primary monitor, undock.
    o(
        "P08-M01",
        "dock-visual",
        "dpi=100",
        m("P12-M47", "left-dock"),
    ),
    o(
        "P08-M01",
        "dock-visual",
        "dpi=125",
        m("P12-M38", "left-dock"),
    ),
    o(
        "P08-M01",
        "dock-visual",
        "dpi=150",
        m("P12-M39", "left-dock"),
    ),
    o(
        "P08-M01",
        "dock-visual",
        "dpi=200",
        m("P12-M40", "left-dock"),
    ),
    o("P08-M01", "timing", "", exact("G4-02", DOCK_TIMING_LEFT)),
    o("P08-M01", "undock-16dip", "", m("P12-M11", "undock-16dip")),
    o(
        "P08-M02",
        "dock-visual",
        "dpi=100",
        m("P12-M47", "right-dock"),
    ),
    o(
        "P08-M02",
        "dock-visual",
        "dpi=125",
        m("P12-M38", "right-dock"),
    ),
    o(
        "P08-M02",
        "dock-visual",
        "dpi=150",
        m("P12-M39", "right-dock"),
    ),
    o(
        "P08-M02",
        "dock-visual",
        "dpi=200",
        m("P12-M40", "right-dock"),
    ),
    o("P08-M02", "timing", "", exact("G4-02", DOCK_TIMING_RIGHT)),
    o("P08-M02", "undock-16dip", "", m("P12-M12", "undock-16dip")),
    o(
        "P08-M03",
        "dock-visual",
        "dpi=100",
        m("P12-M47", "top-dock"),
    ),
    o(
        "P08-M03",
        "dock-visual",
        "dpi=125",
        m("P12-M38", "top-dock"),
    ),
    o(
        "P08-M03",
        "dock-visual",
        "dpi=150",
        m("P12-M39", "top-dock"),
    ),
    o(
        "P08-M03",
        "dock-visual",
        "dpi=200",
        m("P12-M40", "top-dock"),
    ),
    o("P08-M03", "timing", "", exact("G4-02", DOCK_TIMING_TOP)),
    // P08-M04: IME function is exact; the candidate window is visual.
    o(
        "P08-M04",
        "ime-input",
        "ime=pinyin,mode=source",
        exact("G4-06", &["ime-commit-undo:ime=pinyin,mode=source"]),
    ),
    o(
        "P08-M04",
        "ime-input",
        "ime=pinyin,mode=split",
        exact("G4-06", &["ime-commit-undo:ime=pinyin,mode=split"]),
    ),
    o(
        "P08-M04",
        "ime-input",
        "ime=wetype,mode=source",
        exact("G4-06", &["ime-commit-undo:ime=wetype,mode=source"]),
    ),
    o(
        "P08-M04",
        "ime-input",
        "ime=wetype,mode=split",
        exact("G4-06", &["ime-commit-undo:ime=wetype,mode=split"]),
    ),
    o(
        "P08-M04",
        "composition-no-collapse",
        "edge=left",
        exact("G4-06", COMPOSITION_LEFT),
    ),
    o(
        "P08-M04",
        "composition-no-collapse",
        "edge=right",
        exact(
            "G4-06",
            &[
                "ime-composition-no-collapse:edge=right,ime=pinyin",
                "ime-composition-no-collapse:edge=right,ime=wetype",
            ],
        ),
    ),
    o(
        "P08-M04",
        "composition-no-collapse",
        "edge=top",
        exact(
            "G4-06",
            &[
                "ime-composition-no-collapse:edge=top,ime=pinyin",
                "ime-composition-no-collapse:edge=top,ime=wetype",
            ],
        ),
    ),
    o(
        "P08-M04",
        "opacity-ime-commit",
        "opacity=40",
        exact("G4-06", OPACITY_40_IME),
    ),
    o(
        "P08-M04",
        "opacity-ime-commit",
        "opacity=96",
        exact(
            "G4-06",
            &[
                "ime-commit-undo:ime=pinyin,opacity=96",
                "ime-commit-undo:ime=wetype,opacity=96",
            ],
        ),
    ),
    o(
        "P08-M04",
        "opacity-ime-commit",
        "opacity=100",
        exact(
            "G4-06",
            &[
                "ime-commit-undo:ime=pinyin,opacity=100",
                "ime-commit-undo:ime=wetype,opacity=100",
            ],
        ),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=pinyin,edge=left",
        m("P12-M01", "left-dock"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=pinyin,edge=right",
        m("P12-M01", "right-dock"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=pinyin,edge=top",
        m("P12-M01", "top-dock"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=wetype,edge=left",
        m("P12-M02", "left-dock"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=wetype,edge=right",
        m("P12-M02", "right-dock"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=wetype,edge=top",
        m("P12-M02", "top-dock"),
    ),
    // P08-M05: tray lifecycle.
    o(
        "P08-M05",
        "tray-menu",
        "item=show",
        exact("G4-01", TRAY_SHOW),
    ),
    o(
        "P08-M05",
        "tray-menu",
        "item=hide",
        exact(
            "G4-01",
            &[
                "tray-hide-keeps-process",
                "tray-hide-show-retains-window-and-source",
            ],
        ),
    ),
    o(
        "P08-M05",
        "tray-menu",
        "item=quit",
        exact("G4-01", TRAY_QUIT),
    ),
    o(
        "P08-M05",
        "failed-save-quit",
        "",
        exact(
            "G4-01",
            &[
                "tray-quit-save-failure-keeps-visible-process",
                "tray-quit-save-failure-retains-text",
                "tray-quit-save-failure-title",
                "tray-quit-save-failure-retry",
            ],
        ),
    ),
    o(
        "P08-M05",
        "failed-save-visible-error",
        "",
        m("P12-M05", "exit-save-failure"),
    ),
    o(
        "P08-M05",
        "topmost-tray-sync",
        "",
        exact(
            "G4-01",
            &[
                "tray-topmost-controls-sync",
                "tray-topmost-sensor-keeps-config",
            ],
        ),
    ),
    // P08-M06: opacity.
    o(
        "P08-M06",
        "layers-visual",
        "opacity=40",
        m("P12-M22", "opacity-40"),
    ),
    o(
        "P08-M06",
        "layers-visual",
        "opacity=70",
        m("P12-M22", "opacity-70"),
    ),
    o(
        "P08-M06",
        "layers-visual",
        "opacity=96",
        m("P12-M22", "opacity-96"),
    ),
    o(
        "P08-M06",
        "layers-visual",
        "opacity=100",
        m("P12-M22", "opacity-100"),
    ),
    o(
        "P08-M06",
        "commit-modes",
        "commit=slider-release",
        exact("G4-02", &["opacity-slider-release-commits"]),
    ),
    o(
        "P08-M06",
        "commit-modes",
        "commit=enter",
        exact("G4-02", &["opacity-numeric-enter-commits"]),
    ),
    o(
        "P08-M06",
        "commit-modes",
        "commit=blur",
        exact("G4-02", &["opacity-numeric-blur-commits"]),
    ),
    // P08-M07: theme.
    o(
        "P08-M07",
        "theme-visual",
        "theme=light",
        m("P12-M23", "light"),
    ),
    o(
        "P08-M07",
        "theme-visual",
        "theme=dark",
        m("P12-M23", "dark"),
    ),
    o(
        "P08-M07",
        "theme-visual",
        "theme=system",
        m("P12-M23", "system"),
    ),
    o(
        "P08-M07",
        "live-switch",
        "to=dark",
        m("P12-M23", "system-to-dark"),
    ),
    o(
        "P08-M07",
        "live-switch",
        "to=light",
        m("P12-M23", "system-to-light"),
    ),
    // P08-M08: second launch.
    o(
        "P08-M08",
        "visible-restore-and-focus",
        "state=hidden",
        m("P12-M05", "second-launch-hidden"),
    ),
    o(
        "P08-M08",
        "visible-restore-and-focus",
        "state=minimized",
        m("P12-M05", "second-launch-minimized"),
    ),
    o(
        "P08-M08",
        "visible-restore-and-focus",
        "state=docked",
        m("P12-M05", "second-launch-docked"),
    ),
    o(
        "P08-M08",
        "identity-wake",
        "state=hidden",
        exact("G4-05", WAKE_HIDDEN),
    ),
    o(
        "P08-M08",
        "identity-wake",
        "state=docked",
        exact(
            "G4-05",
            &[
                "collapsed-keeps-note-and-config",
                "collapsed-second-instance-exits",
                "collapsed-wake-activates-primary",
                "collapsed-wakes-primary",
                "sole-process-after-collapsed-wake",
            ],
        ),
    ),
    // P08-M09: two portable copies.
    o(
        "P08-M09",
        "process-window-isolation",
        "",
        exact("G4-05", &["portable-copies-note-config-and-exit-isolated"]),
    ),
    o(
        "P08-M09",
        "note-config-runtime-isolation",
        "",
        exact("G4-05", &["portable-copies-note-config-and-exit-isolated"]),
    ),
    o(
        "P08-M09",
        "quit-isolation",
        "",
        exact("G4-05", &["portable-copies-note-config-and-exit-isolated"]),
    ),
    o(
        "P08-M09",
        "tray-identity",
        "",
        m("P12-M45", "tray-identity"),
    ),
    o(
        "P08-M09",
        "independent-show-hide",
        "",
        m("P12-M45", "independent-show-hide"),
    ),
    // P08-M10: monitor topology.
    o(
        "P08-M10",
        "arrangement",
        "side=left",
        m("P12-M43", "secondary-left"),
    ),
    o(
        "P08-M10",
        "arrangement",
        "side=right",
        m("P12-M35", "secondary-right"),
    ),
    o(
        "P08-M10",
        "arrangement",
        "side=above",
        m("P12-M43", "secondary-above"),
    ),
    o(
        "P08-M10",
        "arrangement",
        "side=below",
        m("P12-M35", "secondary-below"),
    ),
    o(
        "P08-M10",
        "primary-switch",
        "",
        m("P12-M35", "primary-switch"),
    ),
    o(
        "P08-M10",
        "disconnect-reconnect",
        "step=disconnect",
        m("P12-M37", "disconnect-current-monitor"),
    ),
    o(
        "P08-M10",
        "disconnect-reconnect",
        "step=reconnect",
        m("P12-M37", "reconnect-monitor"),
    ),
    o("P08-M10", "sleep-resume", "", m("P12-M41", "sleep-resume")),
    o("P08-M10", "rdp", "", m("P12-M42", "rdp-reconnect")),
    // P08-M11: mixed DPI and negative coordinates.
    o(
        "P08-M11",
        "mixed-dpi-dock",
        "edge=left",
        m("P12-M36", "left-dock-sensor"),
    ),
    o(
        "P08-M11",
        "mixed-dpi-dock",
        "edge=right",
        m("P12-M36", "right-dock-sensor"),
    ),
    o(
        "P08-M11",
        "mixed-dpi-cross-monitor",
        "what=drag",
        m("P12-M36", "drag"),
    ),
    o(
        "P08-M11",
        "mixed-dpi-cross-monitor",
        "what=dock",
        m("P12-M36", "dock"),
    ),
    o(
        "P08-M11",
        "mixed-dpi-cross-monitor",
        "what=ime",
        m("P12-M36", "ime"),
    ),
    o(
        "P08-M11",
        "mixed-dpi-cross-monitor",
        "what=preview",
        m("P12-M36", "preview"),
    ),
    o(
        "P08-M11",
        "negative-coordinates",
        "side=left",
        m("P12-M43", "secondary-left"),
    ),
    o(
        "P08-M11",
        "negative-coordinates",
        "side=above",
        m("P12-M43", "secondary-above"),
    ),
    // P08-M12: Windows 11 shell; every manual case is observed on both target roles.
    o(
        "P08-M12",
        "supported-builds",
        "",
        m("P12-M46", "corners-shadow"),
    ),
    o(
        "P08-M12",
        "corners-shadow",
        "",
        m("P12-M46", "corners-shadow"),
    ),
    o(
        "P08-M12",
        "topmost-button",
        "",
        m("P12-M46", "topmost-button"),
    ),
    o("P08-M12", "control-fade", "", m("P12-M46", "control-fade")),
    o(
        "P08-M12",
        "accessibility",
        "",
        Target::Retired {
            decision: "RETIRE-P08-M12-ACCESSIBILITY",
        },
    ),
    // P08-M13..M15: inherited gates.
    o(
        "P08-M13",
        "ime-functional",
        "",
        exact("G4-06", exact_checks::G4_06),
    ),
    o(
        "P08-M13",
        "ime-visual",
        "ime=pinyin",
        m("P12-M01", "pinyin-100"),
    ),
    o(
        "P08-M13",
        "ime-visual",
        "ime=wetype",
        m("P12-M02", "wetype-100"),
    ),
    o(
        "P08-M14",
        "preview-selection-visual",
        "",
        m("P12-M24", "representative-markdown"),
    ),
    o(
        "P08-M14",
        "math-visual",
        "variant=formulas",
        m("P12-M25", "formulas"),
    ),
    o(
        "P08-M14",
        "math-visual",
        "theme=light",
        m("P12-M23", "light"),
    ),
    o("P08-M14", "math-visual", "theme=dark", m("P12-M23", "dark")),
    o("P08-M14", "math-visual", "dpi=100", m("P12-M47", "preview")),
    o("P08-M14", "math-visual", "dpi=125", m("P12-M38", "preview")),
    o("P08-M14", "math-visual", "dpi=150", m("P12-M39", "preview")),
    o("P08-M14", "math-visual", "dpi=200", m("P12-M40", "preview")),
    o(
        "P08-M15",
        "image-quality",
        "",
        m("P12-M26", "image-formats"),
    ),
    o(
        "P08-M15",
        "clipboard-producers",
        "",
        exact("G3-01", exact_checks::G3_01),
    ),
    o(
        "P08-M15",
        "export-dialog",
        "",
        exact("G3-02", exact_checks::G3_02),
    ),
    // D1: observed publication milestones plus a verified before-save negative control.
    o(
        "P08-M15",
        "crash-reconciliation",
        "point=note-save",
        exact(
            "G3-03",
            &[
                "kill-at-publication-failure",
                "kill-before-autosave-keeps-old",
                "restart-flushed-temp",
            ],
        ),
    ),
    o(
        "P08-M15",
        "crash-reconciliation",
        "point=asset-write",
        exact(
            "G3-03",
            &[
                "kill-after-asset-publication",
                "restart-after-asset-publication",
            ],
        ),
    ),
    o(
        "P08-M15",
        "crash-reconciliation",
        "point=trash-move",
        exact(
            "G3-03",
            &["kill-after-trash-move", "restart-referenced-trash"],
        ),
    ),
    // P08-Dxxx: the definition-of-done rows each M row receipt covers.
    o(
        "P08-D004",
        "shell-visual",
        "",
        m("P12-M46", "corners-shadow"),
    ),
    o(
        "P08-D005",
        "rounded-corner",
        "",
        m("P12-M46", "corners-shadow"),
    ),
    o("P08-D006", "shadow", "", m("P12-M46", "corners-shadow")),
    o("P08-D007", "drag", "edge=left", m("P12-M47", "left-dock")),
    o("P08-D007", "drag", "edge=right", m("P12-M47", "right-dock")),
    o("P08-D007", "drag", "edge=top", m("P12-M47", "top-dock")),
    o("P08-D008", "resize", "", m("P12-M46", "edge-corner-resize")),
    o(
        "P08-D010",
        "mode-controls",
        "",
        m("P12-M46", "controls-in-modes"),
    ),
    o(
        "P08-D012",
        "topmost-button",
        "",
        m("P12-M46", "topmost-button"),
    ),
    o(
        "P08-D013",
        "topmost-tray-sync",
        "",
        exact(
            "G4-01",
            &[
                "tray-topmost-controls-sync",
                "tray-topmost-sensor-keeps-config",
            ],
        ),
    ),
    o(
        "P08-D016",
        "live-theme-change",
        "to=dark",
        m("P12-M23", "system-to-dark"),
    ),
    o(
        "P08-D016",
        "live-theme-change",
        "to=light",
        m("P12-M23", "system-to-light"),
    ),
    o(
        "P08-D020",
        "whole-window",
        "opacity=40",
        m("P12-M22", "opacity-40"),
    ),
    o(
        "P08-D020",
        "whole-window",
        "opacity=70",
        m("P12-M22", "opacity-70"),
    ),
    o(
        "P08-D020",
        "whole-window",
        "opacity=96",
        m("P12-M22", "opacity-96"),
    ),
    o(
        "P08-D020",
        "whole-window",
        "opacity=100",
        m("P12-M22", "opacity-100"),
    ),
    o("P08-D021", "live-preview", "", m("P12-M22", "opacity-40")),
    o(
        "P08-D022",
        "numeric-input",
        "commit=enter",
        exact("G4-06", G4_NUMERIC_ENTER),
    ),
    o(
        "P08-D022",
        "numeric-input",
        "commit=blur",
        exact("G4-06", G4_NUMERIC_BLUR),
    ),
    o(
        "P08-D027",
        "tray-icon",
        "",
        exact("G4-01", &["tray-icon-present"]),
    ),
    o(
        "P08-D028",
        "tray-menu-items",
        "",
        exact("G4-01", &["tray-menu-items"]),
    ),
    o(
        "P08-D031",
        "alt-f4",
        "",
        exact("G4-01", &["alt-f4-hides-without-exit"]),
    ),
    o("P08-D039", "left-dock", "", exact("G4-02", DOCK_LEFT)),
    o("P08-D040", "right-dock", "", exact("G4-02", DOCK_RIGHT)),
    o("P08-D041", "top-dock", "", exact("G4-02", DOCK_TOP)),
    o(
        "P08-D051",
        "hover-no-steal",
        "edge=left",
        m("P12-M47", "left-dock"),
    ),
    o(
        "P08-D051",
        "hover-no-steal",
        "edge=right",
        m("P12-M47", "right-dock"),
    ),
    o(
        "P08-D051",
        "hover-no-steal",
        "edge=top",
        m("P12-M47", "top-dock"),
    ),
    o(
        "P08-D055",
        "composition-no-collapse",
        "edge=left",
        exact("G4-06", COMPOSITION_LEFT),
    ),
    o(
        "P08-D055",
        "composition-no-collapse",
        "edge=right",
        exact(
            "G4-06",
            &[
                "ime-composition-no-collapse:edge=right,ime=pinyin",
                "ime-composition-no-collapse:edge=right,ime=wetype",
            ],
        ),
    ),
    o(
        "P08-D055",
        "composition-no-collapse",
        "edge=top",
        exact(
            "G4-06",
            &[
                "ime-composition-no-collapse:edge=top,ime=pinyin",
                "ime-composition-no-collapse:edge=top,ime=wetype",
            ],
        ),
    ),
    // G4-06 composes while focused, and focus alone blocks auto-collapse; the composition
    // guard wiring and blur cancellation now have actual windowless StickyApp owners.
    o(
        "P08-D055",
        "composition-guard-wiring",
        "",
        tests(RUNTIME_COMPOSITION),
    ),
    // Native hover with a verified ordinary cover and sensor hit target; no pointer messages.
    o(
        "P08-D068",
        "covered-sensor-hover",
        "",
        exact("G4-02", &["covered-sensor-real-hover"]),
    ),
    o(
        "P08-D070",
        "hover-click-focus",
        "",
        m("P12-M05", "restore-sensor"),
    ),
    o(
        "P08-D084",
        "disconnect-fallback",
        "",
        m("P12-M37", "disconnect-current-monitor"),
    ),
    o("P08-D089", "dpi-ime-rect", "", m("P12-M36", "ime")),
    o(
        "P08-D117",
        "pinyin-functional",
        "",
        exact("G4-06", IME_PINYIN),
    ),
    o("P08-D117", "pinyin-visual", "", m("P12-M01", "pinyin-100")),
    o(
        "P08-D118",
        "wetype-functional",
        "",
        exact("G4-06", IME_WETYPE),
    ),
    o("P08-D118", "wetype-visual", "", m("P12-M02", "wetype-100")),
    o("P08-D119", "light", "", m("P12-M23", "light")),
    o("P08-D120", "dark", "", m("P12-M23", "dark")),
    o(
        "P08-D121",
        "system-live",
        "to=dark",
        m("P12-M23", "system-to-dark"),
    ),
    o(
        "P08-D121",
        "system-live",
        "to=light",
        m("P12-M23", "system-to-light"),
    ),
    o(
        "P08-D122",
        "opacity-visual",
        "opacity=40",
        m("P12-M22", "opacity-40"),
    ),
    o(
        "P08-D122",
        "opacity-visual",
        "opacity=70",
        m("P12-M22", "opacity-70"),
    ),
    o(
        "P08-D122",
        "opacity-visual",
        "opacity=96",
        m("P12-M22", "opacity-96"),
    ),
    o(
        "P08-D122",
        "opacity-visual",
        "opacity=100",
        m("P12-M22", "opacity-100"),
    ),
    o("P08-D123", "math-visual", "", m("P12-M25", "formulas")),
    o(
        "P08-D124",
        "image-visual",
        "",
        m("P12-M26", "image-formats"),
    ),
    o(
        "P08-D125",
        "mode-visual",
        "",
        m("P12-M46", "controls-in-modes"),
    ),
    o(
        "P08-D126",
        "left-dock-visual",
        "dpi=100",
        m("P12-M47", "left-dock"),
    ),
    o(
        "P08-D126",
        "left-dock-visual",
        "dpi=125",
        m("P12-M38", "left-dock"),
    ),
    o(
        "P08-D126",
        "left-dock-visual",
        "dpi=150",
        m("P12-M39", "left-dock"),
    ),
    o(
        "P08-D126",
        "left-dock-visual",
        "dpi=200",
        m("P12-M40", "left-dock"),
    ),
    o(
        "P08-D127",
        "right-dock-visual",
        "dpi=100",
        m("P12-M47", "right-dock"),
    ),
    o(
        "P08-D127",
        "right-dock-visual",
        "dpi=125",
        m("P12-M38", "right-dock"),
    ),
    o(
        "P08-D127",
        "right-dock-visual",
        "dpi=150",
        m("P12-M39", "right-dock"),
    ),
    o(
        "P08-D127",
        "right-dock-visual",
        "dpi=200",
        m("P12-M40", "right-dock"),
    ),
    o(
        "P08-D128",
        "top-dock-visual",
        "dpi=100",
        m("P12-M47", "top-dock"),
    ),
    o(
        "P08-D128",
        "top-dock-visual",
        "dpi=125",
        m("P12-M38", "top-dock"),
    ),
    o(
        "P08-D128",
        "top-dock-visual",
        "dpi=150",
        m("P12-M39", "top-dock"),
    ),
    o(
        "P08-D128",
        "top-dock-visual",
        "dpi=200",
        m("P12-M40", "top-dock"),
    ),
    o(
        "P08-D129",
        "sensor-visual",
        "edge=left",
        m("P12-M47", "left-dock"),
    ),
    o(
        "P08-D129",
        "sensor-visual",
        "edge=right",
        m("P12-M47", "right-dock"),
    ),
    o(
        "P08-D129",
        "sensor-visual",
        "edge=top",
        m("P12-M47", "top-dock"),
    ),
    o(
        "P08-D130",
        "hover-no-focus-visual",
        "edge=left",
        m("P12-M47", "left-dock"),
    ),
    o(
        "P08-D130",
        "hover-no-focus-visual",
        "edge=right",
        m("P12-M47", "right-dock"),
    ),
    o(
        "P08-D130",
        "hover-no-focus-visual",
        "edge=top",
        m("P12-M47", "top-dock"),
    ),
    o(
        "P08-D131",
        "tray-visual-menu",
        "",
        m("P12-M05", "restore-tray"),
    ),
    o(
        "P08-D132",
        "export-dialog",
        "",
        exact("G3-02", exact_checks::G3_02),
    ),
    o("P08-D133", "real-dpi", "dpi=125", m("P12-M38", "input")),
    o("P08-D133", "real-dpi", "dpi=150", m("P12-M39", "input")),
    o("P08-D133", "real-dpi", "dpi=200", m("P12-M40", "input")),
    o("P08-D134", "dual-monitor", "", m("P12-M35", "drag")),
    o("P08-D135", "mixed-dpi", "", m("P12-M36", "drag")),
    o(
        "P08-D136",
        "disconnect",
        "",
        m("P12-M37", "disconnect-current-monitor"),
    ),
    o(
        "P08-D137",
        "negative-geometry",
        "side=left",
        m("P12-M43", "secondary-left"),
    ),
    o(
        "P08-D137",
        "negative-geometry",
        "side=above",
        m("P12-M43", "secondary-above"),
    ),
    o("P08-D138", "sleep-resume", "", m("P12-M41", "sleep-resume")),
    o("P08-D139", "rdp", "", m("P12-M42", "rdp-reconnect")),
    // plan 09: Esc, manual collapse, scrollbars, opacity and quit.
    o(
        "plan09:esc-1",
        "popup-close-revert",
        "",
        exact(
            "G4-02",
            &[
                "opacity-escape-cancels-field",
                "opacity-escape-cancels-drag",
            ],
        ),
    ),
    o(
        "plan09:esc-1",
        "slider-drag-esc",
        "",
        exact("G4-02", &["opacity-escape-cancels-drag"]),
    ),
    o(
        "plan09:esc-2",
        "recovery-pending",
        "",
        exact(
            "G3-03",
            &[
                "recovery-escape-keeps-pending",
                "recovery-choice-enables-dock-escape",
            ],
        ),
    ),
    o(
        "plan09:esc-2",
        "transient-guards",
        "state=asset-sync",
        tests(ESC_GUARDS),
    ),
    o(
        "plan09:esc-2",
        "transient-guards",
        "state=quitting",
        tests(ESC_GUARDS),
    ),
    o(
        "plan09:esc-3",
        "composition-cancel",
        "field=editor",
        exact(
            "G4-06",
            &[
                "ime-escape-keeps-dock:edge=left,ime=pinyin",
                "ime-escape-keeps-dock:edge=left,ime=wetype",
                "ime-escape-keeps-dock:edge=right,ime=pinyin",
                "ime-escape-keeps-dock:edge=right,ime=wetype",
                "ime-escape-keeps-dock:edge=top,ime=pinyin",
                "ime-escape-keeps-dock:edge=top,ime=wetype",
                "ime-cancel-non-mutating:ime=pinyin",
                "ime-cancel-non-mutating:ime=wetype",
            ],
        ),
    ),
    o(
        "plan09:esc-3",
        "composition-cancel",
        "field=search",
        exact(
            "G4-06",
            &[
                "ime-search-escape-retains-query:ime=pinyin",
                "ime-search-escape-retains-query:ime=wetype",
            ],
        ),
    ),
    o(
        "plan09:esc-4",
        "search-close",
        "",
        exact("G4-02", &["escape-search-closes-before-dock"]),
    ),
    o(
        "plan09:esc-5",
        "preview-selection",
        "selection=present",
        exact("G4-02", &["escape-preview-clears-selection"]),
    ),
    o(
        "plan09:esc-5",
        "preview-selection",
        "selection=absent",
        exact("G4-02", &["escape-preview-empty-collapses"]),
    ),
    o(
        "plan09:esc-6",
        "collapse",
        "state=docked",
        exact("G4-02", &["escape-preview-empty-collapses"]),
    ),
    o(
        "plan09:esc-6",
        "collapse",
        "state=floating",
        exact("G4-02", &["escape-floating-keeps-window"]),
    ),
    o(
        "plan09:esc-6",
        "collapse",
        "modifier=shift",
        exact("G4-02", &["escape-shift-collapses"]),
    ),
    o(
        "plan09:esc-6",
        "collapse",
        "modifier=ctrl",
        tests(ESC_MODIFIERS),
    ),
    o(
        "plan09:esc-6",
        "collapse",
        "repeat",
        exact("G4-02", &["escape-repeat-keydowns-independent"]),
    ),
    o(
        "plan09:manual-collapse",
        "collapse-button",
        "",
        exact("G4-02", &["manual-collapse-button"]),
    ),
    o(
        "plan09:scroll-1",
        "per-pane",
        "view=source",
        tests(SCROLL_VIEW_BARS_OWNED),
    ),
    o(
        "plan09:scroll-1",
        "per-pane",
        "view=preview",
        tests(SCROLL_VIEW_BARS_OWNED),
    ),
    o(
        "plan09:scroll-1",
        "per-pane",
        "view=split",
        tests(SCROLL_VIEW_BARS_OWNED),
    ),
    o(
        "plan09:scroll-1",
        "split-sync-switch",
        "",
        tests(SCROLL_SYNC_APPLICATION),
    ),
    o(
        "plan09:scroll-2",
        "gutter-geometry",
        "",
        tests(SCROLL_GUTTER_GEOMETRY),
    ),
    o(
        "plan09:scroll-2",
        "short-note",
        "part=hide-thumb",
        tests(SCROLL_VIEW_BARS_OWNED),
    ),
    o(
        "plan09:scroll-2",
        "short-note",
        "part=keep-gutter",
        tests(RUNTIME_GEOMETRY),
    ),
    o(
        "plan09:scroll-2",
        "gutter-runtime",
        "aspect=hit-test",
        tests(RUNTIME_GUTTER),
    ),
    o(
        "plan09:scroll-2",
        "gutter-runtime",
        "aspect=content-zoom",
        tests(RUNTIME_GEOMETRY),
    ),
    o(
        "plan09:scroll-3",
        "thumb-geometry",
        "",
        tests(SCROLL_THUMB_GEOMETRY),
    ),
    o(
        "plan09:scroll-3",
        "emphasis-paint",
        "",
        tests(SCROLL_EMPHASIS_PAINT),
    ),
    o(
        "plan09:scroll-3",
        "emphasis-trigger",
        "trigger=hover",
        tests(SCROLL_EMPHASIS_HOVER),
    ),
    o(
        "plan09:scroll-3",
        "emphasis-trigger",
        "trigger=drag",
        tests(SCROLL_EMPHASIS_DRAG),
    ),
    o(
        "plan09:scroll-3",
        "drag-mapping",
        "",
        tests(SCROLL_DRAG_MAPPING),
    ),
    o(
        "plan09:scroll-3",
        "drag-live-update",
        "",
        tests(RUNTIME_SCROLL),
    ),
    o(
        "plan09:scroll-3",
        "ends-reach-document-ends",
        "view=source",
        tests(SCROLL_SOURCE_ENDS),
    ),
    o(
        "plan09:scroll-3",
        "ends-reach-document-ends",
        "view=preview",
        tests(SCROLL_PREVIEW_ENDS),
    ),
    o("plan09:scroll-3", "track-hit", "", tests(SCROLL_TRACK_HIT)),
    o(
        "plan09:scroll-3",
        "track-page-scroll",
        "",
        tests(SCROLL_TRACK_PAGING),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=text",
        tests(RUNTIME_CANONICAL),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=generation",
        tests(RUNTIME_CANONICAL),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=dirty",
        tests(RUNTIME_CANONICAL),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=caret",
        tests(SCROLL_SESSION_APPLICATION),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=selection",
        tests(SCROLL_SESSION_APPLICATION),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=preedit",
        tests(SCROLL_SESSION_APPLICATION),
    ),
    o(
        "plan09:scroll-4",
        "drag-exit",
        "end=release",
        tests(RUNTIME_DRAG_ENDS),
    ),
    o(
        "plan09:scroll-4",
        "drag-exit",
        "end=blur",
        tests(RUNTIME_DRAG_ENDS),
    ),
    o(
        "plan09:scroll-4",
        "drag-exit",
        "end=view-switch",
        tests(RUNTIME_DRAG_ENDS),
    ),
    o(
        "plan09:scroll-4",
        "drag-exit",
        "end=resize",
        tests(RUNTIME_DRAG_ENDS),
    ),
    o(
        "plan09:scroll-4",
        "drag-outside-window",
        "",
        exact("G4-02", G4_SCROLL_OUTSIDE),
    ),
    o(
        "plan09:scroll-5",
        "source-mapping",
        "",
        tests(SCROLL_SOURCE_MAPPING),
    ),
    o(
        "plan09:scroll-5",
        "preview-document-height",
        "",
        tests(RUNTIME_SCROLL),
    ),
    o(
        "plan09:scroll-6",
        "stale-frame-no-thumb",
        "",
        tests(SCROLL_CURRENT_LAYOUT),
    ),
    o(
        "plan09:scroll-6",
        "stale-receipt-identity",
        "",
        tests(SCROLL_STALE_RECEIPT),
    ),
    o(
        "plan09:scroll-6",
        "stale-receipt-guard",
        "",
        tests(RUNTIME_RECEIPT),
    ),
    o(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        "dpi=100",
        m("P12-M47", "scrollbars"),
    ),
    o(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        "dpi=125",
        m("P12-M38", "scrollbars"),
    ),
    o(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        "dpi=150",
        m("P12-M39", "scrollbars"),
    ),
    o(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        "dpi=200",
        m("P12-M40", "scrollbars"),
    ),
    o(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        "theme",
        m("P12-M23", "scrollbars"),
    ),
    o(
        "plan09:opacity-1",
        "whole-window",
        "opacity=40",
        m("P12-M22", "opacity-40"),
    ),
    o(
        "plan09:opacity-1",
        "whole-window",
        "opacity=70",
        m("P12-M22", "opacity-70"),
    ),
    o(
        "plan09:opacity-1",
        "whole-window",
        "opacity=96",
        m("P12-M22", "opacity-96"),
    ),
    o(
        "plan09:opacity-1",
        "whole-window",
        "opacity=100",
        m("P12-M22", "opacity-100"),
    ),
    o(
        "plan09:opacity-2",
        "range-and-input",
        "",
        tests(OPACITY_RANGE_AND_INPUT),
    ),
    o(
        "plan09:opacity-3",
        "live-preview",
        "",
        m("P12-M22", "opacity-40"),
    ),
    o(
        "plan09:opacity-3",
        "no-stepwise-write",
        "",
        tests(OPACITY_NO_STEPWISE_WRITE),
    ),
    o(
        "plan09:opacity-3",
        "commit-modes",
        "commit=slider-release",
        exact("G4-02", &["opacity-slider-release-commits"]),
    ),
    o(
        "plan09:opacity-3",
        "commit-modes",
        "commit=enter",
        exact("G4-02", &["opacity-numeric-enter-commits"]),
    ),
    o(
        "plan09:opacity-3",
        "commit-modes",
        "commit=blur",
        exact("G4-02", &["opacity-numeric-blur-commits"]),
    ),
    o(
        "plan09:quit-1",
        "keeps-running",
        "",
        exact("G4-01", &["tray-quit-save-failure-keeps-visible-process"]),
    ),
    o(
        "plan09:quit-1",
        "memory-text-kept",
        "",
        exact("G4-01", &["tray-quit-save-failure-retains-text"]),
    ),
    o(
        "plan09:quit-1",
        "visible-error",
        "",
        m("P12-M05", "exit-save-failure"),
    ),
    o(
        "plan09:quit-2",
        "established-only-by-save-failure",
        "cause=note-save-failure",
        exact("G4-01", &["tray-quit-save-failure-title"]),
    ),
    o(
        "plan09:quit-2",
        "established-only-by-save-failure",
        "cause=quit-guard",
        tests(RUNTIME_QUIT_EFFECT),
    ),
    o(
        "plan09:quit-2",
        "established-only-by-save-failure",
        "cause=paste-unconverged",
        tests(QUIT_NOTICE_ONLY_SAVE),
    ),
    o(
        "plan09:quit-2",
        "cleared-after-event-when-barrier-false",
        "",
        tests(RUNTIME_EVENT_PASS),
    ),
    o(
        "plan09:quit-2",
        "external-load-releases-rebuild",
        "",
        tests(QUIT_EXTERNAL_LOAD),
    ),
    o(
        "plan09:quit-3",
        "title-priority",
        "",
        tests(QUIT_TITLE_PRIORITY),
    ),
];
