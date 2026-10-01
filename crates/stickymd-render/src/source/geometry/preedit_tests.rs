//! Preedit layout equivalence and projection-only failure boundaries.
//! plan_ref: docs/plan/07_editor_and_ime.md#ime-semantics

use super::*;
use crate::source::{SourceTheme, paint::blend_glyph_rect};
use cosmic_text::{Color, SwashCache};
use stickymd_core::{DocumentState, LineEnding, Selection};
use tiny_skia::Pixmap;

fn eager_reference(
    projection: &mut SourceProjection,
    preedit: &PreeditVisual,
    origin: EditorRect,
) -> Buffer {
    let mut overlay = Buffer::new(
        &mut projection.font_system,
        scaled_metrics(projection.scale_factor),
    );
    overlay.set_size(
        Some((projection.width_px as f32 - origin.x).max(1.0)),
        Some(origin.height.max(1.0)),
    );
    overlay.set_wrap(Wrap::None);
    let attrs = Attrs::new().family(Family::Name(projection.fonts.cjk_family));
    overlay.set_text(&preedit.text, &attrs, Shaping::Advanced, Some(Align::Left));
    overlay.shape_until_scroll(&mut projection.font_system, false);
    overlay
}

fn pixels(projection: &mut SourceProjection, overlay: &mut Buffer, origin: EditorRect) -> Pixmap {
    let mut pixmap = Pixmap::new(projection.width_px, projection.height_px).unwrap();
    overlay.draw(
        &mut projection.font_system,
        &mut SwashCache::new(),
        Color::rgb(24, 24, 24),
        |x, y, width, height, color| {
            blend_glyph_rect(
                &mut pixmap,
                x + origin.x as i32,
                y + origin.y as i32,
                width,
                height,
                color,
            );
        },
    );
    pixmap
}

#[test]
fn preedit_matches_eager_buffer_pixels_selection_and_candidate_geometry() {
    for text in ["", "中文 Latin e\u{301} 🙂"] {
        let document = DocumentState::loaded(text, LineEnding::Lf, None);
        let snapshot = document.snapshot();
        let mut projection = SourceProjection::new(&snapshot, 480, 240, 1.0);
        for (width, height, scale) in [(480, 240, 1.0), (150, 160, 1.5), (440, 260, 3.0)] {
            projection.set_viewport(width, height, scale);
            let origin = projection.caret_rect(0).unwrap();
            for composition in [
                "",
                "nihao",
                "你好 e\u{301} 🙂 אב",
                "long preedit text ".repeat(8).as_str(),
            ] {
                let mut preedit = PreeditVisual {
                    text: composition.to_owned(),
                    cursor: None,
                    replacement: Selection::new(text.len(), 0),
                };
                let mut actual = projection.preedit_buffer(&preedit, origin);
                let mut reference = eager_reference(&mut projection, &preedit, origin);
                assert_eq!(
                    pixels(&mut projection, &mut actual, origin).data(),
                    pixels(&mut projection, &mut reference, origin).data(),
                    "{composition:?} scale={scale}"
                );
                let start = Cursor::new(0, 0);
                let end = Cursor::new(0, composition.len());
                assert_eq!(
                    actual
                        .layout_runs()
                        .flat_map(|run| run.highlight(start, end))
                        .collect::<Vec<_>>(),
                    reference
                        .layout_runs()
                        .flat_map(|run| run.highlight(start, end))
                        .collect::<Vec<_>>()
                );
                for cursor in [
                    None,
                    Some(0..0),
                    Some(0..composition.len()),
                    Some(composition.len()..composition.len()),
                ] {
                    preedit.cursor = cursor;
                    projection.set_preedit(Some(preedit.clone()));
                    let mut expected = reference.clone();
                    let cursor = Cursor::new(
                        0,
                        preedit
                            .cursor
                            .as_ref()
                            .map_or(composition.len(), |range| range.end),
                    );
                    expected.shape_until_cursor(&mut projection.font_system, cursor, false);
                    let expected = expected.layout_runs().find_map(|run| {
                        run.cursor_position(&cursor).map(|x| EditorRect {
                            x: origin.x + x,
                            y: origin.y,
                            width: scale.max(1.0),
                            height: run.line_height,
                        })
                    });
                    assert_eq!(projection.ime_caret_rect(0), expected);
                    assert_eq!(projection.preedit(), Some(&preedit));
                    assert_eq!(projection.projected_text(), text);
                    assert_eq!(projection.projected_generation(), snapshot.generation);
                }
            }
            projection.set_preedit(None);
            assert_eq!(projection.ime_caret_rect(0), projection.caret_rect(0));
        }
    }
}

#[test]
fn invalid_preedit_replacement_remains_failure_atomic() {
    let document = DocumentState::loaded("中文", LineEnding::Lf, None);
    let snapshot = document.snapshot();
    let mut projection = SourceProjection::new(&snapshot, 480, 240, 1.0);
    for replacement in [
        Selection::caret(1),
        Selection::new(0, snapshot.text.len() + 1),
    ] {
        let preedit = PreeditVisual {
            text: "nihao".into(),
            cursor: Some(2..2),
            replacement,
        };
        projection.set_preedit(Some(preedit.clone()));
        assert_eq!(projection.ime_caret_rect(0), None);
        assert_eq!(
            projection.paint(
                &mut Pixmap::new(480, 240).unwrap(),
                Selection::caret(0),
                true,
                true,
                None,
                SourceTheme::Light
            ),
            Err(SourceProjectionError::InvalidPosition)
        );
        assert_eq!(projection.projected_text(), snapshot.text.as_ref());
        assert_eq!(projection.projected_generation(), snapshot.generation);
        assert_eq!(projection.preedit(), Some(&preedit));
    }
    projection.set_preedit(None);
    assert_eq!(projection.ime_caret_rect(0), projection.caret_rect(0));
}
