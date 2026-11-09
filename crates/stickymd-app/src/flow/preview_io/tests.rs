//! Admission of Preview results.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

use stickymd_render::preview::PreviewTheme;

use super::*;

#[test]
fn current_layout_requires_generation_and_every_layout_field_but_not_offset_or_selection() {
    let old = Generation::initial();
    let generation = old.checked_next().unwrap();
    let viewport = PreviewViewport {
        horizontal: Default::default(),
        width_px: 500,
        height_px: 400,
        scale: 1.25,
        scroll_y: 42.0,
        selection: PreviewSelection::default(),
        theme: PreviewTheme::Light,
    };
    for current in [
        viewport.clone(),
        PreviewViewport {
            scroll_y: 1000.0,
            selection: PreviewSelection::caret(5),
            ..viewport.clone()
        },
    ] {
        assert!(current_preview_layout(
            Some(generation),
            generation,
            Some(viewport.clone()),
            Some(current)
        ));
    }
    for frame_generation in [None, Some(old), Some(generation.checked_next().unwrap())] {
        assert!(!current_preview_layout(
            frame_generation,
            generation,
            Some(viewport.clone()),
            Some(viewport.clone())
        ));
    }
    assert!(!current_preview_layout(
        Some(generation),
        generation,
        None,
        Some(viewport.clone())
    ));
    assert!(!current_preview_layout(
        Some(generation),
        generation,
        Some(viewport.clone()),
        None
    ));
    for stale in [
        PreviewViewport {
            width_px: 499,
            ..viewport.clone()
        },
        PreviewViewport {
            height_px: 399,
            ..viewport.clone()
        },
        PreviewViewport {
            scale: 1.5,
            ..viewport.clone()
        },
        PreviewViewport {
            theme: PreviewTheme::Dark,
            ..viewport.clone()
        },
    ] {
        assert!(!current_preview_layout(
            Some(generation),
            generation,
            Some(stale.clone()),
            Some(viewport.clone())
        ));
        assert!(!current_preview_layout(
            Some(generation),
            generation,
            Some(viewport.clone()),
            Some(stale.clone())
        ));
    }
}

/// Only a completion for the current document and viewport is offered for display; the
/// tray-hidden release wins over every other condition.
#[test]
fn completion_route_admits_only_the_current_build() {
    let current = Generation::initial().checked_next().unwrap();
    let older = Generation::initial();
    for generation in [current, older] {
        for viewport in [None, Some(false), Some(true)] {
            for succeeded in [true, false] {
                assert_eq!(
                    completion_route(true, generation, current, viewport, succeeded),
                    CompletionRoute::ReleaseHidden
                );
            }
        }
    }
    for viewport in [None, Some(false), Some(true)] {
        assert_eq!(
            completion_route(false, older, current, viewport, true),
            CompletionRoute::Rebuild
        );
    }
    assert_eq!(
        completion_route(false, current, current, None, true),
        CompletionRoute::Drop
    );
    for succeeded in [true, false] {
        assert_eq!(
            completion_route(false, current, current, Some(false), succeeded),
            CompletionRoute::Relayout {
                reuse_tree: succeeded
            }
        );
    }
    for succeeded in [true, false] {
        assert_eq!(
            completion_route(false, current, current, Some(true), succeeded),
            CompletionRoute::Admit
        );
    }
}

/// Plan 09: an old scroll request's background receipt never makes the new position jump
/// back. The receipt carries the viewport it was built for (the worker echoes it), which
/// differs from the current viewport in its offset alone while its layout is the same; the
/// identity sees the difference, so the receipt is only re-laid out for the current
/// viewport (reusing its tree when it built one) and never admitted for display. The receipt
/// for the current offset is admitted.
#[test]
fn a_receipt_for_an_older_offset_is_relaid_out_and_never_shown() {
    let generation = Generation::initial().checked_next().unwrap();
    let current = PreviewViewport {
        horizontal: Default::default(),
        width_px: 500,
        height_px: 646,
        scale: 1.25,
        scroll_y: 1200.0,
        selection: PreviewSelection::default(),
        theme: PreviewTheme::Light,
    };
    for older in [0.0, 600.0, 1199.0, 1199.75, 1200.25, 4000.0] {
        let receipt = PreviewViewport {
            scroll_y: older,
            ..current.clone()
        };
        assert!(receipt.same_layout(&current), "{older}");
        assert_ne!(
            receipt, current,
            "the offset alone tells them apart: {older}"
        );
        for succeeded in [true, false] {
            assert_eq!(
                completion_route(
                    false,
                    generation,
                    generation,
                    Some(receipt == current),
                    succeeded
                ),
                CompletionRoute::Relayout {
                    reuse_tree: succeeded
                },
                "{older}"
            );
        }
    }
    assert_eq!(
        completion_route(
            false,
            generation,
            generation,
            Some(current == current),
            true
        ),
        CompletionRoute::Admit
    );
}

#[test]
fn new_preview_generation_clears_old_selection_but_repaint_keeps_it() {
    let old = Generation::initial();
    let new = old.checked_next().unwrap();
    let selection = PreviewSelection {
        anchor: 2,
        active: 8,
    };
    assert_eq!(
        selection_for_generation(Some(old), old, selection),
        selection
    );
    assert_eq!(
        selection_for_generation(Some(old), new, selection),
        PreviewSelection::default()
    );
}
