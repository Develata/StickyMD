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
                    let actual = projection.ime_caret_rect(0);
                    if let Some(expected) = expected
                        && expected.x + expected.width > width as f32
                    {
                        // The eager path let long compositions escape the pane.
                        // Preserve vertical metrics, but assert visibility rather
                        // than copying that bug into the reference contract.
                        let actual = actual.unwrap();
                        assert_eq!(actual.y, expected.y);
                        assert_eq!(actual.height, expected.height);
                        assert_eq!(actual.width, expected.width);
                        assert!(actual.x >= origin.x);
                        assert!(actual.x + actual.width <= width as f32);
                    } else {
                        assert_eq!(actual, expected);
                    }
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

#[test]
fn long_preedit_keeps_candidate_caret_inside_source_viewport() {
    let document = DocumentState::loaded("prefix 中文", LineEnding::Lf, None);
    let mut projection = SourceProjection::new(&document.snapshot(), 150, 160, 1.5);
    let composition = "long preedit text ".repeat(8);
    projection.set_preedit(Some(PreeditVisual {
        cursor: Some(composition.len()..composition.len()),
        text: composition,
        replacement: Selection::caret("prefix ".len()),
    }));
    let caret = projection.ime_caret_rect("prefix ".len()).unwrap();
    assert!(
        caret.x >= 0.0 && caret.x + caret.width <= 150.0,
        "candidate caret is outside the Source pane: {caret:?}"
    );
}

#[test]
fn wrapped_replacement_caret_remains_available() {
    let document = DocumentState::loaded("prefix 中文\nnext row", LineEnding::Lf, None);
    let mut projection = SourceProjection::new(&document.snapshot(), 200, 240, 3.0);
    projection.ensure_caret_visible(7).unwrap();
    let caret = projection.caret_rect(7).unwrap();
    assert!(caret.y >= 0.0 && caret.y + caret.height <= 240.0);
    let scroll = projection.scroll();
    projection.ensure_caret_visible(7).unwrap();
    assert_eq!(projection.caret_rect(7), Some(caret));
    assert_eq!(projection.scroll(), scroll);
    projection.scroll_to_anchor(ScrollAnchor::point(7)).unwrap();
    let aligned = projection.caret_rect(7).unwrap();
    assert!((aligned.y - projection.padding()).abs() < 0.01);
}

#[test]
fn preedit_reveal_paints_at_candidate_and_clips_without_mutating_source() {
    for (text, replacement) in [
        ("", Selection::caret(0)),
        (
            "prefix 中文\nnext row",
            Selection::new("prefix 中文".len(), 7),
        ),
    ] {
        let document = DocumentState::loaded(text, LineEnding::Lf, None);
        let snapshot = document.snapshot();
        let mut projection = SourceProjection::new(&snapshot, 200, 240, 1.0);
        for scale in [0.5, 1.5, 3.0] {
            projection.set_viewport(200, 240, scale);
            projection
                .ensure_caret_visible(replacement.start())
                .unwrap();
            let origin = projection.caret_rect(replacement.start()).unwrap();
            let scroll = projection.scroll();
            for theme in [SourceTheme::Light, SourceTheme::Dark] {
                let mut baseline = Pixmap::new(200, 240).unwrap();
                projection
                    .paint(&mut baseline, replacement, true, false, None, theme)
                    .unwrap();
                for composition in [
                    "long preedit text ".repeat(8),
                    "中文 e\u{301} 🙂 tail ".repeat(8),
                    "אבגדהוזחט ".repeat(8),
                ] {
                    let middle = composition.char_indices().nth(8).unwrap().0;
                    for cursor in [
                        None,
                        Some(0..0),
                        Some(0..middle),
                        Some(middle..composition.len()),
                        Some(composition.len()..composition.len()),
                    ] {
                        let preedit = PreeditVisual {
                            text: composition.clone(),
                            cursor,
                            replacement,
                        };
                        projection.set_preedit(Some(preedit.clone()));
                        let caret = projection.ime_caret_rect(replacement.active.byte).unwrap();
                        assert!(caret.x >= origin.x && caret.x + caret.width <= 200.0);
                        let mut painted = baseline.clone();
                        projection
                            .paint(&mut painted, replacement, true, false, None, theme)
                            .unwrap();
                        if preedit.cursor.is_some() {
                            // Use the normal caret painter as the palette/shape
                            // reference, translated to the reported IME rectangle.
                            let mut expected = Pixmap::new(200, 240).unwrap();
                            projection
                                .paint_caret_overlay(
                                    &mut expected,
                                    replacement.start(),
                                    caret.x - origin.x,
                                    caret.y - origin.y,
                                    theme,
                                )
                                .unwrap();
                            for (expected, actual) in expected.pixels().iter().zip(painted.pixels())
                            {
                                // Only opaque interior pixels are independent
                                // of the underlying glyph/background blend.
                                if expected.alpha() == 255 {
                                    assert_eq!(actual, expected, "{scale} {caret:?}");
                                }
                            }
                        }
                        if preedit.cursor == Some(composition.len()..composition.len()) {
                            // None has the same end-aligned text/viewport but
                            // hides the caret. Composite over that frame to also
                            // check subpixel carets with no opaque interior.
                            projection.set_preedit(Some(PreeditVisual {
                                cursor: None,
                                ..preedit.clone()
                            }));
                            let mut expected = baseline.clone();
                            projection
                                .paint(&mut expected, replacement, true, false, None, theme)
                                .unwrap();
                            assert_ne!(expected.data(), painted.data());
                            projection
                                .paint_caret_overlay(
                                    &mut expected,
                                    replacement.start(),
                                    caret.x - origin.x,
                                    caret.y - origin.y,
                                    theme,
                                )
                                .unwrap();
                            assert!(expected.data() == painted.data(), "{scale} {caret:?}");
                            projection.set_preedit(Some(preedit.clone()));
                        }
                        for y in 0..240 {
                            for x in 0..200 {
                                if (x as f32) < origin.x.floor()
                                    || (y as f32) < origin.y.floor()
                                    || (y as f32) >= (origin.y + origin.height).ceil()
                                {
                                    assert_eq!(painted.pixel(x, y), baseline.pixel(x, y));
                                }
                            }
                        }
                        assert_eq!(projection.preedit(), Some(&preedit));
                        assert_eq!(projection.projected_text(), text);
                        assert_eq!(projection.projected_generation(), snapshot.generation);
                        assert_eq!(projection.scroll(), scroll);
                        projection.set_preedit(None);
                        projection
                            .paint(&mut painted, replacement, true, false, None, theme)
                            .unwrap();
                        assert_eq!(painted.data(), baseline.data());
                    }
                }
            }
        }
    }
}
