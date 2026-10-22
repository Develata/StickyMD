//! Contract checks of the projection operation used by the real shell.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars

use super::*;
use crate::interaction::ImeSignal;
use stickymd_core::{DocumentState, LineEnding, Selection};
use stickymd_render::preview::{PreviewPipeline, PreviewSelection, PreviewTheme};

fn fixture() -> (SourceProjection, PreviewFrame) {
    // Heading and paragraph layout heights differ from Source, so synchronization
    // cannot accidentally pass by copying a scroll fraction between the panes.
    let text: String = (0..120)
        .map(|n| format!("# Heading {n}\n\nparagraph {n} with content\n\n"))
        .collect();
    let document = DocumentState::loaded(&text, LineEnding::Lf, None);
    let source = SourceProjection::new(&document.snapshot(), 350, 220, 1.0);
    let frame = PreviewPipeline::new()
        .build(
            &document.snapshot(),
            350,
            220,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    assert!(frame.document_height() > 2_000.0);
    (source, frame)
}

#[test]
fn split_sync_uses_only_the_current_driver_and_requires_the_switch_and_current_frame() {
    let (mut source, frame) = fixture();
    let mut session = EditorSession::default();
    for mode in [ViewMode::Source, ViewMode::Preview, ViewMode::Split] {
        for enabled in [false, true] {
            for current in [false, true] {
                source.scroll_to_fraction(0.0);
                let mut preview_y = 75.0;
                let repaint = ScrollApplication {
                    source: Some(&mut source),
                    frame: Some(&frame),
                    preview_y: &mut preview_y,
                    session: &mut session,
                }
                .apply(
                    ScrollPane::Source,
                    ScrollRequest::Fraction(0.7),
                    mode,
                    enabled,
                    current,
                );
                assert!(source.scroll().line > 50, "Source driver must move");
                let sync = mode == ViewMode::Split && enabled;
                assert_eq!(repaint, sync);
                if sync && current {
                    let expected = frame
                        .y_for_scroll_anchor(source.scroll_anchor())
                        .unwrap()
                        .clamp(0.0, frame.document_height() - frame.height() as f32);
                    assert_eq!(preview_y, expected);
                    assert!(preview_y > 500.0);
                } else {
                    assert_eq!(preview_y, 75.0, "disabled/stale peer must keep its offset");
                }
                source.scroll_to_fraction(0.0);
                let before = source.scroll();
                let repaint = ScrollApplication {
                    source: Some(&mut source),
                    frame: Some(&frame),
                    preview_y: &mut preview_y,
                    session: &mut session,
                }
                .apply(
                    ScrollPane::Preview,
                    ScrollRequest::Pixels(800.0),
                    mode,
                    enabled,
                    current,
                );
                assert!(repaint);
                if sync && current {
                    assert!(
                        source.scroll().line > 0,
                        "Preview driver must move its Source peer"
                    );
                    let anchor = frame.scroll_anchor_at_y(preview_y).unwrap();
                    let byte = source.scroll_anchor().source_byte;
                    assert!(
                        byte.abs_diff(anchor.source_byte) < 100,
                        "semantic peer: {byte} vs {anchor:?}"
                    );
                } else {
                    assert_eq!(source.scroll(), before);
                }
                assert_eq!(session.scroll.line, source.scroll().line);
            }
        }
    }
}

#[test]
fn pane_scroll_preserves_text_generation_selection_caret_and_preedit() {
    let (mut source, frame) = fixture();
    let original = source.projected_text().to_owned();
    let generation = source.projected_generation();
    let mut session = EditorSession {
        selection: Selection::caret(7),
        preferred_x: Some(31.0),
        focused: true,
        ..EditorSession::default()
    };
    session.handle_ime(ImeSignal::Enabled, generation, 0);
    session.handle_ime(
        ImeSignal::Preedit {
            text: "组合中".to_owned(),
            cursor: Some(0..6),
        },
        generation,
        1,
    );
    source.set_preedit(session.preedit_visual());
    let before = session.clone();
    let preedit = format!("{:?}", source.preedit());
    let mut preview_y = 0.0;
    for pane in [ScrollPane::Source, ScrollPane::Preview] {
        for request in [
            ScrollRequest::Pixels(120.0),
            ScrollRequest::Fraction(0.73),
            ScrollRequest::Pixels(-42.0),
            ScrollRequest::Fraction(0.0),
            ScrollRequest::Fraction(1.0),
        ] {
            ScrollApplication {
                source: Some(&mut source),
                frame: Some(&frame),
                preview_y: &mut preview_y,
                session: &mut session,
            }
            .apply(pane, request, ViewMode::Split, true, true);
            assert_eq!(source.projected_text(), original);
            assert_eq!(source.projected_generation(), generation);
            assert_eq!(session.selection, before.selection);
            assert_eq!(session.ime, before.ime);
            assert_eq!(session.preferred_x, before.preferred_x);
            assert_eq!(session.dragging_selection, before.dragging_selection);
            assert_eq!(session.caret_visible, before.caret_visible);
            assert_eq!(session.focused, before.focused);
            assert_eq!(format!("{:?}", source.preedit()), preedit);
        }
    }
    // Canonical text/generation/dirty have no mutable route through this helper:
    // only the render projections and session scroll coordinates are borrowed.
}

#[test]
fn preview_scroll_uses_cached_document_height_and_missing_frames_keep_existing_rules() {
    let (mut source, frame) = fixture();
    let mut session = EditorSession::default();
    let mut preview_y = 0.0;
    let max = frame.document_height() - frame.height() as f32;
    for (request, expected) in [
        (ScrollRequest::Fraction(1.0), max),
        (ScrollRequest::Pixels(-100.0), max - 100.0),
        (ScrollRequest::Pixels(10_000_000.0), max),
        (ScrollRequest::Fraction(0.0), 0.0),
    ] {
        ScrollApplication {
            source: Some(&mut source),
            frame: Some(&frame),
            preview_y: &mut preview_y,
            session: &mut session,
        }
        .apply(ScrollPane::Preview, request, ViewMode::Preview, false, true);
        assert_eq!(preview_y, expected);
    }
    preview_y = 42.0;
    ScrollApplication {
        source: None,
        frame: None,
        preview_y: &mut preview_y,
        session: &mut session,
    }
    .apply(
        ScrollPane::Preview,
        ScrollRequest::Fraction(1.0),
        ViewMode::Preview,
        false,
        false,
    );
    assert_eq!(preview_y, 42.0);
    ScrollApplication {
        source: None,
        frame: None,
        preview_y: &mut preview_y,
        session: &mut session,
    }
    .apply(
        ScrollPane::Preview,
        ScrollRequest::Pixels(10.0),
        ViewMode::Preview,
        false,
        false,
    );
    assert_eq!(preview_y, 0.0);
}
