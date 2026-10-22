//! Receipt application and config-to-geometry checks in the real windowless shell.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars

use super::*;
use crate::app::headless_tests::fixture;
use crate::config::ContentZoomPercent;
use stickymd_render::preview::{PreviewFrame, PreviewPipeline};

fn frame(app: &StickyApp, viewport: PreviewViewport) -> PreviewFrame {
    PreviewPipeline::new()
        .build(
            &app.coordinator.snapshot(),
            viewport.width_px,
            viewport.height_px,
            viewport.scale,
            viewport.scroll_y,
            viewport.selection,
            viewport.theme,
        )
        .unwrap()
}

#[test]
fn receipt_application_rejects_every_stale_viewport_field_without_replacing_frame_or_offset() {
    let mut app = fixture(&"# Heading\n\nlong content for scrolling\n\n".repeat(60));
    app.config
        .update(|c| c.view_mode = ViewMode::Preview)
        .unwrap();
    let current = PreviewViewport {
        width_px: 350,
        height_px: 220,
        scale: 1.0,
        scroll_y: 420.0,
        selection: PreviewSelection {
            anchor: 2,
            active: 7,
        },
        theme: PreviewTheme::Light,
    };
    let generation = app.coordinator.view().generation;
    app.preview_frame = Some(frame(&app, current));
    app.preview_frame_viewport = Some(current);
    app.preview_scroll_y = current.scroll_y;
    app.preview_selection = current.selection;
    let canonical = app.coordinator.snapshot();
    // Offset and selection have the same layout but must still reject a receipt.
    let stale = [
        PreviewViewport {
            scroll_y: 0.0,
            ..current
        },
        PreviewViewport {
            selection: PreviewSelection::default(),
            ..current
        },
        PreviewViewport {
            width_px: 351,
            ..current
        },
        PreviewViewport {
            height_px: 221,
            ..current
        },
        PreviewViewport {
            scale: 1.5,
            ..current
        },
        PreviewViewport {
            theme: PreviewTheme::Dark,
            ..current
        },
    ];
    for viewport in stale {
        let before_pixels = app.preview_frame.as_ref().unwrap().rgba().as_ptr();
        let receipt = frame(&app, viewport);
        app.apply_preview_completion(
            PreviewCompletion {
                generation,
                viewport,
                result: Ok(receipt),
            },
            false,
            Some(current),
        );
        assert_eq!(app.preview_scroll_y, current.scroll_y, "{viewport:?}");
        assert_eq!(app.preview_frame_viewport, Some(current));
        assert_eq!(app.preview_selection, current.selection);
        assert_eq!(
            app.preview_frame.as_ref().unwrap().rgba().as_ptr(),
            before_pixels
        );
        assert_eq!(app.coordinator.snapshot(), canonical);
        // A successful stale build can donate its semantic tree, never its pixels.
        assert_eq!(app.preview_flow.applied_generation(), Some(generation));
    }
    // Positive control: a matching receipt really reaches admission and changes state.
    app.preview_scroll_y = 0.0;
    app.preview_frame = None;
    let receipt = frame(&app, current);
    app.apply_preview_completion(
        PreviewCompletion {
            generation,
            viewport: current,
            result: Ok(receipt),
        },
        false,
        Some(current),
    );
    assert_eq!(app.preview_scroll_y, current.scroll_y);
    assert_eq!(app.preview_frame_viewport, Some(current));
    assert!(app.preview_frame.is_some());
}

#[test]
fn actual_config_geometry_keeps_gutters_across_content_zoom_and_note_lengths() {
    let mut short_panes = Vec::new();
    for (document, text) in ["short".to_owned(), "long content\n".repeat(500)]
        .into_iter()
        .enumerate()
    {
        let mut app = fixture(&text);
        let mut case = 0;
        for mode in [ViewMode::Source, ViewMode::Preview, ViewMode::Split] {
            for dpi in [1.0, 1.5, 2.0] {
                let mut baseline = None;
                for zoom in [50, 100, 300] {
                    app.config
                        .update(|c| {
                            c.view_mode = mode;
                            c.content_zoom_percent = ContentZoomPercent::new_clamped(zoom);
                        })
                        .unwrap();
                    let geometry = app.view_geometry_for(PhysicalSize::new(900, 600), dpi);
                    let panes = [geometry.source, geometry.preview];
                    if document == 0 {
                        short_panes.push(panes);
                    } else {
                        assert_eq!(panes, short_panes[case], "document length changed gutter");
                    }
                    case += 1;
                    if let Some(baseline) = baseline {
                        assert_eq!(panes, baseline);
                    } else {
                        baseline = Some(panes);
                    }
                    for pane in panes.into_iter().flatten() {
                        // The real body rectangle ends before a positive-width gutter.
                        let at = winit::dpi::PhysicalPosition::new(
                            f64::from(pane.x + pane.width) + 1.0,
                            f64::from(pane.y + pane.height / 2),
                        );
                        assert!(
                            crate::app::scrollbar::gutter_at(
                                at,
                                [
                                    (crate::app::scrollbar::ScrollPane::Source, geometry.source),
                                    (crate::app::scrollbar::ScrollPane::Preview, geometry.preview)
                                ],
                                f64::from(dpi),
                            )
                            .is_some()
                        );
                        assert!(!pane.contains(at.x as f32, at.y as f32));
                    }
                }
            }
        }
    }
}
