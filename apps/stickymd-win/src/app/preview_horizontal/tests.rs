//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-horizontal-scroll

use super::*;
use crate::app::headless_tests::fixture;
use stickymd_render::preview::{PreviewPipeline, PreviewSelection, PreviewTheme};

#[test]
fn horizontal_wheel_preserves_vertical_intent_and_shift_uses_the_vertical_axis() {
    assert_eq!(horizontal_delta(0.0, 48.0, false), 0.0);
    assert_eq!(horizontal_delta(0.0, 48.0, true), 48.0);
    assert_eq!(horizontal_delta(-48.0, 0.0, false), -48.0);
    assert_eq!(horizontal_delta(-48.0, 0.0, true), -48.0);
    assert_eq!(horizontal_delta(2.0, 48.0, false), 0.0);
}

#[test]
fn horizontal_session_never_mutates_note_or_vertical_offset_and_releases_drag_guard() {
    let mut app = fixture(&format!("```\n{}\n```", "code ".repeat(60)));
    let before = app.coordinator.snapshot();
    let frame = PreviewPipeline::new()
        .build(
            &before,
            320,
            200,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    let target = frame.horizontal_target_at(25.0, 25.0).unwrap();
    app.preview_scroll_y = 13.0;
    app.apply_preview_horizontal_offset(target, 48.0);
    let accumulated = app.preview_horizontal.offsets.offset(target) + 48.0;
    app.apply_preview_horizontal_offset(target, accumulated);
    assert_eq!(app.preview_horizontal.offsets.offset(target), 96.0);
    app.preview_horizontal.drag = Some(HorizontalDrag {
        target,
        start_x: 0.0,
    });
    assert!(app.window_guards().dragging);
    app.cancel_scrollbar_drag();
    assert!(!app.window_guards().dragging);
    assert_eq!(app.preview_scroll_y, 13.0);
    assert_eq!(app.coordinator.snapshot(), before);
    assert_eq!(
        app.preview_horizontal
            .offsets
            .for_layout(before.generation, 400, 1.0),
        PreviewHorizontalOffsets::default()
    );
    assert_eq!(
        app.preview_horizontal.offsets.for_layout(
            before.generation.checked_next().unwrap(),
            320,
            1.0
        ),
        PreviewHorizontalOffsets::default()
    );
}
