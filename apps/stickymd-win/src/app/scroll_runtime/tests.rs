//! Real shell scroll application without a native application window.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars

use super::*;
use crate::app::headless_tests::fixture;
use crate::config::ViewMode;
use crate::instruction::AppIntent;
use crate::interaction::ImeSignal;
use stickymd_core::{EditKind, Selection};
use stickymd_render::preview::{PreviewPipeline, PreviewSelection, PreviewTheme};
use stickymd_render::source::SourceProjection;
use winit::dpi::{PhysicalPosition, PhysicalSize};

fn prepare(app: &mut StickyApp, mode: ViewMode) -> ViewGeometry {
    app.config
        .update(|c| {
            c.view_mode = mode;
            c.split_scroll_sync = false;
        })
        .unwrap();
    let geometry = app.view_geometry_for(PhysicalSize::new(800, 300), 1.0);
    let snapshot = app.coordinator.snapshot();
    if let Some(pane) = geometry.source {
        app.projection = Some(SourceProjection::new(
            &snapshot,
            pane.width,
            pane.height,
            1.0,
        ));
    }
    if let Some(pane) = geometry.preview {
        app.preview_frame = Some(
            PreviewPipeline::new()
                .build(
                    &snapshot,
                    pane.width,
                    pane.height,
                    1.0,
                    0.0,
                    PreviewSelection::default(),
                    PreviewTheme::Light,
                )
                .unwrap(),
        );
    }
    geometry
}

fn start_drag(app: &mut StickyApp, geometry: ViewGeometry, pane: ScrollPane) -> Scrollbar {
    let bar = app
        .scrollbar_geometry_for(geometry, 1.0, true)
        .into_iter()
        .flatten()
        .find(|bar| bar.pane == pane)
        .unwrap();
    app.cursor_position = PhysicalPosition::new(
        f64::from(bar.track.x + 2),
        bar.thumb_top + bar.thumb_height / 2.0,
    );
    assert!(app.press_scrollbar_at(geometry, 1.0, true));
    assert_eq!(app.scrollbars.drag.unwrap().pane, pane);
    assert!(app.window_guards().dragging);
    bar
}

fn offset(app: &mut StickyApp, pane: ScrollPane) -> f64 {
    match pane {
        ScrollPane::Source => app.projection.as_mut().unwrap().scroll_metrics().fraction,
        ScrollPane::Preview => f64::from(app.preview_scroll_y),
    }
}

#[test]
fn runtime_pointer_moves_apply_each_pane_and_cached_document_height() {
    for pane in [ScrollPane::Source, ScrollPane::Preview] {
        let mut app = fixture(&"paragraph with content\n\n".repeat(150));
        let geometry = prepare(&mut app, ViewMode::Split);
        let pixels = app.preview_frame.as_ref().unwrap().rgba().as_ptr();
        let index = std::sync::Arc::as_ptr(app.preview_frame.as_ref().unwrap().index());
        let document_height = app.preview_frame.as_ref().unwrap().document_height();
        let height = geometry.preview.unwrap().height as f32;
        assert!(document_height > height * 2.0);
        let bar = start_drag(&mut app, geometry, pane);
        let travel = f64::from(bar.track.height) - bar.thumb_height;
        let canonical = app.coordinator.snapshot();
        for fraction in [0.2, 0.7, 0.4, 1.0, 0.0] {
            app.cursor_position.y =
                f64::from(bar.track.y) + fraction * travel + bar.thumb_height / 2.0;
            assert!(app.move_scrollbar_at(Some(geometry), 1.0, true));
            let actual = offset(&mut app, pane);
            let expected = if pane == ScrollPane::Preview {
                fraction * f64::from(document_height - height)
            } else {
                fraction
            };
            assert!(
                (actual - expected).abs() < 0.01,
                "{pane:?}: {actual} != {expected}"
            );
            assert_eq!(app.coordinator.snapshot(), canonical);
            // The input path retains the cached frame/index and never builds a renderer.
            assert_eq!(app.preview_frame.as_ref().unwrap().rgba().as_ptr(), pixels);
            assert_eq!(
                std::sync::Arc::as_ptr(app.preview_frame.as_ref().unwrap().index()),
                index
            );
            assert!(app.preview_worker.is_none());
        }
        // Supplied coordinates may leave the client area; native delivery still needs desktop QA.
        for (y, end) in [(-500.0, 0.0), (10000.0, 1.0)] {
            app.cursor_position = PhysicalPosition::new(-100.0, y);
            assert!(app.move_scrollbar_at(Some(geometry), 1.0, true));
            assert!(app.scrollbars.drag.is_some());
            let expected = if pane == ScrollPane::Preview {
                end * f64::from(document_height - height)
            } else {
                end
            };
            assert!((offset(&mut app, pane) - expected).abs() < 0.01);
        }
    }
}

#[test]
fn runtime_gutter_press_is_consumed_without_caret_or_selection_mutation() {
    for text in ["short".to_owned(), "long paragraph\n\n".repeat(150)] {
        for mode in [ViewMode::Source, ViewMode::Preview, ViewMode::Split] {
            let mut app = fixture(&text);
            let geometry = prepare(&mut app, mode);
            app.session.selection = Selection::new(4, 1);
            app.preview_selection = PreviewSelection {
                anchor: 1,
                active: 3,
            };
            let canonical = app.coordinator.snapshot();
            for pane in [geometry.source, geometry.preview].into_iter().flatten() {
                // Empty margin beside the track must be consumed even with no thumb.
                app.cursor_position = PhysicalPosition::new(
                    f64::from(pane.x + pane.width + 16),
                    f64::from(pane.y + 35),
                );
                assert!(app.press_scrollbar_at(geometry, 1.0, true));
                assert_eq!(app.session.selection, Selection::new(4, 1));
                assert_eq!(
                    app.preview_selection,
                    PreviewSelection {
                        anchor: 1,
                        active: 3
                    }
                );
                assert!(!app.session.dragging_selection);
                assert!(!app.preview_dragging);
                assert_eq!(app.coordinator.snapshot(), canonical);
            }
            app.cursor_position = PhysicalPosition::new(30.0, 80.0);
            assert!(
                !app.press_scrollbar_at(geometry, 1.0, true),
                "body is not consumed"
            );
        }
    }
}

#[test]
fn runtime_drag_ends_on_release_blur_view_switch_and_resize_for_both_panes() {
    for pane in [ScrollPane::Source, ScrollPane::Preview] {
        for end in ["release", "blur", "view-switch", "resize"] {
            let mut app = fixture(&"long paragraph\n\n".repeat(150));
            let geometry = prepare(&mut app, ViewMode::Split);
            start_drag(&mut app, geometry, pane);
            app.cursor_position.y += 60.0;
            assert!(app.move_scrollbar_at(Some(geometry), 1.0, true));
            let canonical = app.coordinator.snapshot();
            match end {
                "release" => app.handle_mouse_button(ElementState::Released, MouseButton::Left),
                "blur" => app.apply_focus_change(None, false),
                "view-switch" => app.dispatch_preview_intent(
                    crate::instruction::PreviewIntent::SetViewMode(ViewMode::Preview),
                ),
                "resize" => app.resize(PhysicalSize::new(820, 400)),
                _ => unreachable!(),
            }
            assert!(app.scrollbars.drag.is_none(), "{pane:?} {end}");
            assert!(!app.window_guards().dragging);
            let settled_offset = offset(&mut app, pane);
            app.cursor_position.y += 100.0;
            assert!(!app.move_scrollbar_at(Some(geometry), 1.0, true));
            assert_eq!(
                offset(&mut app, pane),
                settled_offset,
                "post-{end} move scrolled {pane:?}"
            );
            assert_eq!(app.coordinator.snapshot(), canonical);
        }
    }
}

#[test]
fn runtime_scroll_preserves_canonical_state_and_session_for_clean_and_dirty_notes() {
    let text = "heading\n\nlong paragraph for scrolling\n\n".repeat(100);
    let mut app = fixture(&text);
    for dirty in [false, true] {
        if dirty {
            let generation = app.coordinator.view().generation;
            app.coordinator
                .dispatch(AppIntent::Edit {
                    expected_generation: generation,
                    selection: Selection::caret(0),
                    inserted: "changed ".into(),
                    kind: EditKind::Typing,
                    timestamp_ms: 1,
                })
                .unwrap();
        }
        let snapshot = app.coordinator.snapshot();
        app.projection = Some(SourceProjection::new(&snapshot, 350, 220, 1.0));
        app.preview_frame = Some(
            PreviewPipeline::new()
                .build(
                    &snapshot,
                    350,
                    220,
                    1.0,
                    0.0,
                    PreviewSelection::default(),
                    PreviewTheme::Light,
                )
                .unwrap(),
        );
        app.session.selection = Selection::new(12, 4);
        app.session
            .handle_ime(ImeSignal::Enabled, snapshot.generation, 2);
        app.session.handle_ime(
            ImeSignal::Preedit {
                text: "nihao".into(),
                cursor: Some(2..4),
            },
            snapshot.generation,
            3,
        );
        let selection = app.session.selection;
        let preedit = app.session.ime.clone();
        assert_eq!(app.coordinator.view().dirty, dirty);
        for mode in [ViewMode::Source, ViewMode::Split, ViewMode::Preview] {
            for sync in [false, true] {
                app.config
                    .update(|config| {
                        config.view_mode = mode;
                        config.split_scroll_sync = sync;
                    })
                    .unwrap();
                for pane in [ScrollPane::Source, ScrollPane::Preview] {
                    for fraction in [0.0, 0.3, 1.0, 0.6] {
                        app.scroll_pane_to_fraction(pane, fraction);
                        app.scroll_pane_by(pane, -27.0);
                        assert_eq!(app.coordinator.snapshot(), snapshot);
                        assert_eq!(app.coordinator.view().dirty, dirty);
                        assert_eq!(app.session.selection, selection);
                        assert_eq!(app.session.ime, preedit);
                    }
                }
            }
        }
        assert!(app.projection.as_ref().unwrap().scroll().line > 0);
        assert!(app.preview_scroll_y > 0.0);
        assert_eq!(app.worker.metrics().note_submitted, 0);
    }
}
